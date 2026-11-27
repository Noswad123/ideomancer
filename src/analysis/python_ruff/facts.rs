//! Transactional, private Ruff fact experiment. Not a public evidence backend.
use super::expressions::{Node, Shape};
use super::*;
use crate::analysis::{
    python_facts as shared, python_projection as projection, python_scalars::AsciiJson,
};
use projection::{
    CallObservation, DefinitionKind, HeaderEvent, HeaderGroup, HeaderMetadata, HeaderSchedule,
    Projection, ScopeId,
};
use ruff_python_ast::{self as ast, Expr, Mod, Stmt};
use ruff_python_parser::Parsed;
use ruff_text_size::{Ranged, TextRange};

const FACT_POLICY: &str = "python-ruff-private-facts-4";
const OUTPUT: usize = 32 * 1024 * 1024;

#[path = "metadata.rs"]
mod metadata;

#[derive(Debug, Serialize)]
pub(in crate::analysis) struct CandidateReport<'a> {
    pub(in crate::analysis) identity: ParserIdentity,
    pub(in crate::analysis) cpython_compatibility: CompatibilityStatus,
    pub(in crate::analysis) files: Vec<CandidateFile<'a>>,
    pub(in crate::analysis) facts: Vec<shared::Fact>,
    pub(in crate::analysis) diagnostics: Vec<shared::Warning>,
}

#[derive(Debug, Serialize)]
pub(in crate::analysis) struct CandidateFile<'a> {
    pub(in crate::analysis) path: &'a str,
    pub(in crate::analysis) module: &'a str,
    pub(in crate::analysis) sha256: String,
    pub(in crate::analysis) status: FactStatus,
    pub(in crate::analysis) reason: Option<&'static str>,
    pub(in crate::analysis) failure_line: Option<usize>,
    pub(in crate::analysis) compatibility_nodes: usize,
    pub(in crate::analysis) compatibility_work: usize,
    pub(in crate::analysis) compatibility_depth: usize,
    pub(in crate::analysis) type_ignores: usize,
    pub(in crate::analysis) nodes: usize,
    pub(in crate::analysis) edges: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(in crate::analysis) enum FactStatus {
    AcceptedFacts,
    UnsupportedSyntax,
    SyntaxError,
    UnsupportedEncoding,
    ResourceLimit,
}

#[derive(Debug)]
pub(super) enum Error {
    Syntax { line: usize, reason: &'static str },
    Unsupported(&'static str),
    Resource,
    Cancelled,
    Sink(shared::Failure),
}
pub(super) type Result<T> = std::result::Result<T, Error>;
impl From<shared::Failure> for Error {
    fn from(e: shared::Failure) -> Self {
        Self::Sink(e)
    }
}
impl From<projection::Error> for Error {
    fn from(e: projection::Error) -> Self {
        match e {
            projection::Error::Unsupported => Self::Unsupported("projection"),
            projection::Error::Cancelled => Self::Cancelled,
            projection::Error::WorkingLimit => Self::Resource,
        }
    }
}

fn work<'a>(
    sink: &'a mut shared::FactSink,
    check: &'a mut dyn FnMut() -> bool,
    mut persistent: Option<&'a mut usize>,
) -> impl FnMut(usize) -> std::result::Result<(), projection::Error> + 'a {
    move |bytes| {
        let next = persistent
            .as_deref()
            .copied()
            .unwrap_or(0)
            .checked_add(bytes)
            .ok_or(projection::Error::WorkingLimit)?;
        sink.charge(bytes, &mut || check()).map_err(|e| match e {
            shared::Failure::Cancelled => projection::Error::Cancelled,
            _ => projection::Error::WorkingLimit,
        })?;
        if let Some(persistent) = persistent.as_deref_mut() {
            *persistent = next;
        }
        Ok(())
    }
}

fn fact_identity<F: FnMut(Checkpoint) -> bool>(cancel: &mut Cancellation<F>) -> ParserIdentity {
    let mut result = identity(cancel);
    result.policy = FACT_POLICY;
    let mut hash = Sha256::new();
    // Length framing prevents ambiguous concatenation. This is deliberately a
    // development/root-lock fingerprint, not an arbitrary consumer graph claim.
    for (name, bytes) in [
        (
            "python_ruff.rs",
            include_bytes!("../python_ruff.rs").as_slice(),
        ),
        ("facts.rs", include_bytes!("facts.rs").as_slice()),
        (
            "expressions.rs",
            include_bytes!("expressions.rs").as_slice(),
        ),
        ("metadata.rs", include_bytes!("metadata.rs").as_slice()),
        ("literals.rs", include_bytes!("literals.rs").as_slice()),
        ("source.rs", include_bytes!("source.rs").as_slice()),
        (
            "type_comments.rs",
            include_bytes!("type_comments.rs").as_slice(),
        ),
        (
            "python_facts.rs",
            include_bytes!("../python_facts.rs").as_slice(),
        ),
        (
            "projection.rs",
            include_bytes!("../python_native/projection.rs").as_slice(),
        ),
        (
            "scalars.rs",
            include_bytes!("../python_native/scalars.rs").as_slice(),
        ),
        ("vendor/ruff/provenance.json", PROVENANCE),
        (
            "Cargo.lock",
            include_bytes!("../../../Cargo.lock").as_slice(),
        ),
    ]
    .into_iter()
    .chain(unicode15::identity_inputs().iter().copied())
    {
        hash_frame(&mut hash, name, bytes, cancel);
    }
    result.implementation_sha256 = format!("{:x}", hash.finalize());
    result.sha256.clear();
    result.sha256 = digest(
        &serde_json::to_vec(&result).expect("fixed identity"),
        cancel,
        Checkpoint::Identity,
    );
    result
}

pub(in crate::analysis) fn extract_fact_candidates(
    sources: &[Source],
) -> std::result::Result<CandidateReport<'_>, FrontEndError> {
    let start = Instant::now();
    extract_with(sources, |_| start.elapsed() >= crate::analysis::DEADLINE)
}

fn extract_with<F: FnMut(Checkpoint) -> bool>(
    sources: &[Source],
    check: F,
) -> std::result::Result<CandidateReport<'_>, FrontEndError> {
    extract_impl(
        sources,
        check,
        SemanticLimits::default(),
        #[cfg(test)]
        None,
    )
}

#[derive(Clone, Copy)]
struct SemanticLimits {
    nodes: usize,
    depth: usize,
    edges: usize,
    working: usize,
}
impl Default for SemanticLimits {
    fn default() -> Self {
        Self {
            nodes: 200_000,
            depth: 256,
            edges: 400_000,
            working: 32 * 1024 * 1024,
        }
    }
}

fn extract_impl<'a, F: FnMut(Checkpoint) -> bool>(
    sources: &'a [Source],
    check: F,
    semantic_limits: SemanticLimits,
    #[cfg(test)] mut trace: Option<&mut Vec<Observation>>,
) -> std::result::Result<CandidateReport<'a>, FrontEndError> {
    let mut cancel = Cancellation {
        expired: false,
        check,
    };
    validate_sources(sources, Limits::default(), &mut cancel)?;
    let mut report = CandidateReport {
        identity: fact_identity(&mut cancel),
        cpython_compatibility: CompatibilityStatus::Pending,
        files: sources
            .iter()
            .map(|source| CandidateFile {
                path: &source.path,
                module: &source.module,
                sha256: "0".repeat(64),
                status: FactStatus::ResourceLimit,
                reason: Some("deadline"),
                failure_line: None,
                compatibility_nodes: 0,
                compatibility_work: 0,
                compatibility_depth: 0,
                type_ignores: 0,
                nodes: 0,
                edges: 0,
            })
            .collect(),
        facts: Vec::new(),
        diagnostics: Vec::new(),
    };
    let mut sink = shared::FactSink::new(shared::SinkLimits {
        working: semantic_limits.working,
        ..shared::SinkLimits::default()
    });
    // Every outcome fits before any file mark or parse. Numbers reserve their
    // maximum decimal width and reasons reserve a fixed conservative allowance.
    let overhead =
        shared::measure(&report, OUTPUT, &mut || false).map_err(|_| FrontEndError::ReportLimit)?;
    sink.reserve_output_baseline(
        overhead
            .checked_add(sources.len() * 256)
            .ok_or(FrontEndError::ReportLimit)?,
    )
    .map_err(|_| FrontEndError::ReportLimit)?;
    for source in sources {
        cancel.at(Checkpoint::Admission);
        sink.module_baseline(&source.path, &source.module, &mut || false)
            .map_err(|_| FrontEndError::ReportLimit)?;
    }
    let parser = run_context(
        sources,
        Limits::default(),
        &mut cancel,
        &mut |index, text, parsed, check| {
            let mark = sink.mark();
            let initial = sink.working();
            #[cfg(test)]
            let trace_mark = trace.as_ref().map_or(0, |t| t.len());
            let mut driver = Driver {
                source: &sources[index],
                source_text: text,
                sink: &mut sink,
                check,
                arena: Vec::new(),
                lines: Vec::new(),
                pairs: Vec::new(),
                keywords: Vec::new(),
                comments: None,
                projection: Projection::default(),
                scope: ScopeId::default(),
                owners: Vec::new(),
                nodes: 0,
                edges: 0,
                persistent_work: 0,
                owner_slots: 0,
                arena_work: 0,
                limits: semantic_limits,
                #[cfg(test)]
                trace: trace.as_deref_mut(),
            };
            let result = driver.consume(text, parsed);
            let file = &mut report.files[index];
            file.nodes = driver.nodes;
            file.edges = driver.edges;
            if let Some(plan) = &driver.comments {
                file.compatibility_nodes = plan.compatibility_nodes;
                file.compatibility_work = plan.normalization_work;
                file.compatibility_depth = plan.max_depth;
                file.type_ignores = plan.ignore_count;
            }
            // Drop all owned normalization/scheduling/scope data before refunding.
            drop(driver);
            match result {
                Ok(()) => {
                    let pending = sink.working() - initial;
                    sink.release(pending);
                    #[cfg(test)]
                    assert_eq!(sink.working(), initial);
                    file.status = FactStatus::AcceptedFacts;
                    file.reason = None;
                }
                Err(error) => {
                    sink.rollback(mark);
                    #[cfg(test)]
                    if let Some(trace) = &mut trace {
                        trace.truncate(trace_mark);
                    }
                    (file.status, file.reason) = match error {
                        Error::Syntax { line, reason } => {
                            file.failure_line = Some(line);
                            (FactStatus::SyntaxError, Some(reason))
                        }
                        Error::Unsupported(reason) => (FactStatus::UnsupportedSyntax, Some(reason)),
                        Error::Cancelled | Error::Sink(shared::Failure::Cancelled) => {
                            (FactStatus::ResourceLimit, Some("deadline"))
                        }
                        _ => (FactStatus::ResourceLimit, Some("semantic resource limit")),
                    };
                }
            }
        },
    )?;
    for (file, parsed) in report.files.iter_mut().zip(parser.files) {
        file.sha256 = parsed.sha256;
        if parsed.admission == Admission::InvalidSource {
            file.status = FactStatus::SyntaxError;
            file.reason = parsed.failure_reason;
            file.failure_line = parsed.syntax_error_line.map(|line| line.0);
        } else if parsed.admission == Admission::UnsupportedEncoding {
            file.status = FactStatus::UnsupportedEncoding;
            file.reason = Some(if parsed.diagnostic == Some(Diagnostic::UnresolvedCodec) {
                "unresolved codec membership"
            } else {
                "unsupported codec"
            });
        } else if parsed.admission == Admission::ResourceLimit {
            file.status = FactStatus::ResourceLimit;
            file.reason = Some("frontend resource limit");
        } else if parsed.syntax != RuffSyntax::AcceptedByRuff {
            file.status = if parsed.syntax == RuffSyntax::StructuredSyntax {
                FactStatus::SyntaxError
            } else {
                FactStatus::UnsupportedSyntax
            };
            file.reason = Some("parser diagnostics");
            file.failure_line = parsed
                .syntax_error_line
                .or(parsed.unsupported_version_line)
                .map(|line| line.0);
        }
    }
    let final_result = sink.finalize(&mut || cancel.at(Checkpoint::Final));
    if final_result.is_err() || cancel.at(Checkpoint::Final) {
        sink.retain_baselines();
        #[cfg(test)]
        if let Some(trace) = &mut trace {
            trace.clear();
        }
        for file in &mut report.files {
            file.status = FactStatus::ResourceLimit;
            file.reason = Some("run finalization");
            file.failure_line = None;
        }
    }
    (report.facts, report.diagnostics) = sink.into_records();
    if let Err(error) = shared::measure(&report, OUTPUT, &mut || cancel.at(Checkpoint::Output)) {
        if error != shared::Failure::Cancelled {
            return Err(FrontEndError::ReportLimit);
        }
    }
    if cancel.at(Checkpoint::Final) {
        report.facts.retain(|f| f.kind == "module");
        report.diagnostics.clear();
        #[cfg(test)]
        if let Some(trace) = &mut trace {
            trace.clear();
        }
        for file in &mut report.files {
            file.status = FactStatus::ResourceLimit;
            file.reason = Some("deadline");
            file.failure_line = None;
        }
        shared::measure(&report, OUTPUT, &mut || false).map_err(|_| FrontEndError::ReportLimit)?;
    }
    Ok(report)
}

#[cfg(test)]
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub(super) enum Observation {
    Fact { fact: shared::Fact },
    Warning { warning: shared::Warning },
}

pub(super) struct Driver<'a> {
    source: &'a Source,
    source_text: &'a str,
    pub(super) sink: &'a mut shared::FactSink,
    pub(super) check: &'a mut dyn FnMut() -> bool,
    pub(super) arena: Vec<Node>,
    lines: Vec<usize>,
    pairs: Vec<Paren>,
    keywords: Vec<(TokenKind, TextRange)>,
    comments: Option<type_comments::CommentPlan>,
    projection: Projection,
    scope: ScopeId,
    owners: Vec<(String, bool)>,
    persistent_work: usize,
    owner_slots: usize,
    arena_work: usize,
    pub(super) nodes: usize,
    pub(super) edges: usize,
    limits: SemanticLimits,
    #[cfg(test)]
    trace: Option<&'a mut Vec<Observation>>,
}

struct Paren {
    start: usize,
    end: usize,
    parent: Option<usize>,
}

impl Driver<'_> {
    pub(super) fn edge_limit(&self) -> usize {
        self.limits.edges
    }
    pub(super) fn tick(&mut self) -> Result<()> {
        if (self.check)() {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }
    pub(super) fn charge(&mut self, bytes: usize) -> Result<()> {
        self.sink
            .charge(bytes, &mut || (self.check)())
            .map_err(Into::into)
    }
    fn release(&mut self, bytes: usize) -> Result<()> {
        self.sink
            .working()
            .checked_sub(bytes)
            .ok_or(Error::Resource)?;
        self.sink.release(bytes);
        Ok(())
    }
    fn drop_text(&mut self, text: String) -> Result<()> {
        let bytes = text.capacity();
        drop(text);
        self.release(bytes)
    }
    pub(super) fn charge_arena(&mut self, bytes: usize) -> Result<()> {
        let next = self.arena_work.checked_add(bytes).ok_or(Error::Resource)?;
        self.charge(bytes)?;
        self.arena_work = next;
        Ok(())
    }
    pub(super) fn arena_text(&mut self, text: &str) -> Result<String> {
        self.charge_arena(text.len())?;
        self.copy_text(text)
    }
    fn arena_scope<T>(&mut self, consume: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        if !self.arena.is_empty() || self.arena_work != 0 {
            return Err(Error::Resource);
        }
        let result = consume(self);
        // No IDs escape these boundaries: header observations own their strings,
        // and assignment IDs remain live together until its complete replay.
        drop(std::mem::take(&mut self.arena));
        let bytes = std::mem::take(&mut self.arena_work);
        self.release(bytes)?;
        result
    }
    fn finish_region(&mut self, before_work: usize, before_persistent: usize) -> Result<()> {
        let desired = before_work
            .checked_sub(before_persistent)
            .and_then(|temporary| temporary.checked_add(self.persistent_work))
            .ok_or(Error::Resource)?;
        let refund = self
            .sink
            .working()
            .checked_sub(desired)
            .ok_or(Error::Resource)?;
        self.release(refund)
    }
    pub(super) fn text(&mut self, text: &str) -> Result<String> {
        self.charge(text.len())?;
        self.copy_text(text)
    }
    fn copy_text(&mut self, text: &str) -> Result<String> {
        let mut out = String::with_capacity(text.len());
        for (i, ch) in text.chars().enumerate() {
            if i % 256 == 0 {
                self.tick()?;
            }
            out.push(ch);
        }
        Ok(out)
    }
    pub(super) fn node(&mut self, depth: usize) -> Result<()> {
        self.tick()?;
        self.nodes += 1;
        self.edges += 1;
        if self.nodes > self.limits.nodes
            || depth > self.limits.depth
            || self.edges > self.limits.edges
        {
            Err(Error::Resource)
        } else {
            Ok(())
        }
    }

    fn consume(&mut self, text: &str, parsed: &Parsed<Mod>) -> Result<()> {
        self.index(text, parsed)?;
        let plan = {
            let sink = &mut self.sink;
            let check = std::cell::RefCell::new(&mut *self.check);
            let mut budget = |bytes| {
                sink.charge(bytes, &mut || (check.borrow_mut())())
                    .map_err(|error| match error {
                        shared::Failure::Cancelled => type_comments::Error::Cancelled,
                        _ => type_comments::Error::WorkingLimit,
                    })
            };
            let mut cancelled = || (check.borrow_mut())();
            if self.limits.nodes == 200_000 && self.limits.depth == 256 {
                type_comments::analyze(text, parsed, &mut budget, &mut cancelled)
            } else {
                type_comments::analyze_with_limits(
                    text,
                    parsed,
                    &mut budget,
                    &mut cancelled,
                    type_comments::Limits {
                        nodes: self.limits.nodes,
                        depth: self.limits.depth,
                    },
                )
            }
        }
        .map_err(|error| match error {
            type_comments::Error::Syntax { line } => Error::Syntax {
                line,
                reason: "type comment placement",
            },
            type_comments::Error::WorkingLimit => Error::Resource,
            type_comments::Error::Cancelled => Error::Cancelled,
            type_comments::Error::UnsupportedCategory(reason) => Error::Unsupported(reason),
        })?;
        debug_assert!(plan.working_charge <= self.sink.working());
        self.comments = Some(plan);
        self.projection.root(&mut work(
            self.sink,
            self.check,
            Some(&mut self.persistent_work),
        ))?;
        let Mod::Module(module) = parsed.syntax() else {
            return Err(Error::Unsupported("module mode"));
        };
        self.node(0)?;
        self.statements(&module.body, 0)
    }

    fn index(&mut self, text: &str, parsed: &Parsed<Mod>) -> Result<()> {
        self.charge((text.len() + 1) * std::mem::size_of::<usize>())?;
        self.lines = Vec::with_capacity(text.len() + 1);
        self.lines.push(0);
        let bytes = text.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            self.tick()?;
            if bytes[i] == b'\r' {
                i += usize::from(bytes.get(i + 1) == Some(&b'\n'));
                self.lines.push(i + 1);
            } else if bytes[i] == b'\n' {
                self.lines.push(i + 1);
            }
            i += 1;
        }
        let mut stack = Vec::new();
        let mut parent = None;
        let mut fstrings = Vec::new();
        let mut temporary = 0usize;
        let mut stack_slots = 0;
        let mut string_slots = 0;
        for token in parsed.tokens() {
            self.tick()?;
            let range = token.range();
            let raw = &text[range.start().to_usize()..range.end().to_usize()];
            match token.kind() {
                TokenKind::Def | TokenKind::Class | TokenKind::Async => {
                    self.charge(4 * std::mem::size_of::<(TokenKind, TextRange)>())?;
                    self.keywords.push((token.kind(), range));
                }
                TokenKind::Name if !raw.is_ascii() => {
                    if !unicode15::valid_identifier(raw, self.check)
                        .map_err(|_| Error::Cancelled)?
                    {
                        return Err(Error::Syntax {
                            line: logical_line(bytes, range.start().to_usize()).0,
                            reason: "invalid Unicode 15 identifier",
                        });
                    }
                }
                TokenKind::Int => {
                    let b = raw.as_bytes();
                    let prefixed = b.len() >= 2
                        && b[0] == b'0'
                        && matches!(b[1], b'x' | b'X' | b'o' | b'O' | b'b' | b'B');
                    if !prefixed {
                        let mut digits = 0usize;
                        let mut nonzero = false;
                        for chunk in b.chunks(256) {
                            self.tick()?;
                            for &c in chunk {
                                digits += usize::from(c.is_ascii_digit());
                                nonzero |= matches!(c, b'1'..=b'9');
                            }
                        }
                        if nonzero && digits > 4300 {
                            return Err(Error::Unsupported("decimal conversion limit"));
                        }
                    }
                }
                TokenKind::FStringStart => {
                    if fstrings.len() == string_slots {
                        let bytes = 4 * std::mem::size_of::<(bool, bool)>();
                        temporary = temporary.checked_add(bytes).ok_or(Error::Resource)?;
                        self.charge(bytes)?;
                        string_slots += 1;
                    }
                    fstrings.push((token.unwrap_string_flags().is_raw_string(), false));
                }
                TokenKind::FStringEnd => {
                    // CPython decodes f-string text after its replacement fields,
                    // locating this failure at the matching closing token. Nested
                    // literal failures therefore take precedence over outer text.
                    // Format-spec exception taxonomy remains Pending G4.
                    if fstrings.pop().is_some_and(|(_, invalid)| invalid) {
                        return Err(Error::Syntax {
                            line: logical_line(bytes, range.start().to_usize()).0,
                            reason: "invalid Unicode 15 named escape",
                        });
                    }
                }
                TokenKind::String | TokenKind::FStringMiddle => {
                    let flags = token.unwrap_string_flags();
                    let raw_string = if token.kind() == TokenKind::FStringMiddle {
                        fstrings.last().is_some_and(|&(raw, _)| raw)
                    } else {
                        flags.is_raw_string()
                    };
                    if !raw_string && !flags.is_byte_string() {
                        let mut i = 0;
                        while i < raw.len() {
                            self.tick()?;
                            if raw.as_bytes()[i] != b'\\' {
                                i += 1;
                                continue;
                            }
                            if raw.as_bytes().get(i + 1) != Some(&b'N') {
                                i += 2;
                                continue;
                            }
                            i += 2;
                            let braced = raw.as_bytes().get(i) == Some(&b'{');
                            i += usize::from(braced);
                            let name_start = i;
                            while i < raw.len() && raw.as_bytes()[i] != b'}' {
                                self.tick()?;
                                i += 1;
                            }
                            if !braced
                                || i == raw.len()
                                || unicode15::lookup_name_15(&raw[name_start..i], self.check)
                                    .map_err(|_| Error::Cancelled)?
                                    .is_none()
                            {
                                if token.kind() == TokenKind::FStringMiddle {
                                    let Some((_, invalid)) = fstrings.last_mut() else {
                                        return Err(Error::Unsupported("f-string index"));
                                    };
                                    *invalid = true;
                                    break;
                                }
                                // Adjacent ordinary literals retain their own
                                // opening-token location, not the group's start.
                                return Err(Error::Syntax {
                                    line: logical_line(bytes, range.start().to_usize()).0,
                                    reason: "invalid Unicode 15 named escape",
                                });
                            }
                            i += 1;
                        }
                    }
                }
                TokenKind::Lpar | TokenKind::Lsqb | TokenKind::Lbrace => {
                    if stack.len() == stack_slots {
                        let bytes = 4 * std::mem::size_of::<(TokenKind, Option<usize>)>();
                        temporary = temporary.checked_add(bytes).ok_or(Error::Resource)?;
                        self.charge(bytes)?;
                        stack_slots += 1;
                    }
                    if token.kind() == TokenKind::Lpar {
                        self.charge(4 * std::mem::size_of::<Paren>())?;
                        let id = self.pairs.len();
                        self.pairs.push(Paren {
                            start: range.start().to_usize(),
                            end: 0,
                            parent,
                        });
                        stack.push((token.kind(), Some(id)));
                        parent = Some(id);
                    } else {
                        stack.push((token.kind(), None));
                    }
                }
                TokenKind::Rpar | TokenKind::Rsqb | TokenKind::Rbrace => {
                    let Some((kind, id)) = stack.pop() else {
                        return Err(Error::Unsupported("delimiter index"));
                    };
                    if !matches!(
                        (kind, token.kind()),
                        (TokenKind::Lpar, TokenKind::Rpar)
                            | (TokenKind::Lsqb, TokenKind::Rsqb)
                            | (TokenKind::Lbrace, TokenKind::Rbrace)
                    ) {
                        return Err(Error::Unsupported("delimiter index"));
                    }
                    if let Some(id) = id {
                        self.pairs[id].end = range.end().to_usize();
                        parent = self.pairs[id].parent;
                    }
                }
                _ => {}
            }
        }
        drop(stack);
        drop(fstrings);
        self.release(temporary)?;
        Ok(())
    }

    pub(super) fn generator_range(&mut self, range: TextRange) -> Result<TextRange> {
        let (start, end) = (range.start().to_usize(), range.end().to_usize());
        let mut current = self
            .pairs
            .partition_point(|pair| pair.start <= start)
            .checked_sub(1);
        while let Some(index) = current {
            self.tick()?;
            let pair = &self.pairs[index];
            if end <= pair.end {
                return Ok(TextRange::new(
                    (pair.start as u32).into(),
                    (pair.end as u32).into(),
                ));
            }
            current = pair.parent;
        }
        Err(Error::Unsupported("generator parentheses"))
    }

    fn span(&self, range: TextRange) -> (usize, usize) {
        (
            self.lines
                .partition_point(|&start| start <= range.start().to_usize()),
            self.lines
                .partition_point(|&start| start <= range.end().to_usize().saturating_sub(1)),
        )
    }

    fn corrected_range(&self, kind: type_comments::NodeKind, range: TextRange) -> TextRange {
        self.comments
            .as_ref()
            .map_or(range, |plan| plan.corrected_range(kind, range))
    }

    fn symbol(&mut self, parameter: Option<&str>) -> Result<String> {
        let size = self.source.module.len()
            + self.owners.iter().map(|(n, _)| n.len() + 1).sum::<usize>()
            + parameter.map_or(0, |n| n.len() + 1);
        self.charge(size)?;
        let mut out = String::with_capacity(size);
        for (part, text) in std::iter::once(self.source.module.as_str())
            .chain(self.owners.iter().map(|(name, _)| name.as_str()))
            .chain(parameter)
            .enumerate()
        {
            if part != 0 {
                out.push('.');
            }
            for (i, ch) in text.chars().enumerate() {
                if i % 256 == 0 && (self.check)() {
                    return Err(Error::Cancelled);
                }
                out.push(ch);
            }
        }
        Ok(out)
    }

    fn emit(
        &mut self,
        kind: &str,
        range: TextRange,
        value: String,
        parameter: Option<&str>,
    ) -> Result<()> {
        let symbol = self.symbol(parameter)?;
        let (line, end) = self.span(range);
        let temporary = symbol.capacity() + value.capacity();
        let result = self.sink.emit(
            &self.source.path,
            kind,
            line,
            end,
            symbol,
            value,
            &mut || (self.check)(),
        );
        self.sink.release(temporary);
        result?;
        #[cfg(test)]
        if let Some(trace) = &mut self.trace {
            trace.push(Observation::Fact {
                fact: self.sink.facts().last().unwrap().clone(),
            });
        }
        Ok(())
    }

    fn emit_lines(
        &mut self,
        kind: &str,
        line: usize,
        end: usize,
        value: String,
        parameter: Option<&str>,
    ) -> Result<()> {
        let symbol = self.symbol(parameter)?;
        let temporary = symbol.capacity() + value.capacity();
        let result = self.sink.emit(
            &self.source.path,
            kind,
            line,
            end,
            symbol,
            value,
            &mut || (self.check)(),
        );
        self.sink.release(temporary);
        result?;
        #[cfg(test)]
        if let Some(trace) = &mut self.trace {
            trace.push(Observation::Fact {
                fact: self.sink.facts().last().unwrap().clone(),
            });
        }
        Ok(())
    }

    fn json_pairs(&mut self, pairs: &[(&str, &str)]) -> Result<String> {
        let mut check = || (self.check)();
        let mut count = AsciiJson::new(shared::CountWriter::new(OUTPUT), OUTPUT, &mut check);
        count.raw(b"{").map_err(shared::Failure::from)?;
        for (i, (key, value)) in pairs.iter().enumerate() {
            if i != 0 {
                count.raw(b",").map_err(shared::Failure::from)?;
            }
            count.text(key).map_err(shared::Failure::from)?;
            count.raw(b":").map_err(shared::Failure::from)?;
            count.text(value).map_err(shared::Failure::from)?;
        }
        count.raw(b"}").map_err(shared::Failure::from)?;
        let size = count.into_inner().count();
        self.charge(size)?;
        let mut check = || (self.check)();
        let mut buffer = AsciiJson::new(Vec::with_capacity(size), size, &mut check);
        buffer.raw(b"{").map_err(shared::Failure::from)?;
        for (i, (key, value)) in pairs.iter().enumerate() {
            if i != 0 {
                buffer.raw(b",").map_err(shared::Failure::from)?;
            }
            buffer.text(key).map_err(shared::Failure::from)?;
            buffer.raw(b":").map_err(shared::Failure::from)?;
            buffer.text(value).map_err(shared::Failure::from)?;
        }
        buffer.raw(b"}").map_err(shared::Failure::from)?;
        Ok(String::from_utf8(buffer.into_inner()).expect("ASCII JSON"))
    }

    fn observation(&mut self, id: usize) -> Result<CallObservation> {
        let Shape::Call { func, .. } = self.arena[id].shape else {
            return Err(Error::Unsupported("call observation"));
        };
        let value = self.render(id)?;
        let callee = self.render(func)?;
        let (line, end) = self.span(self.arena[id].range);
        let preceding = self.include_metadata(id)?;
        Ok(CallObservation {
            line,
            end,
            value,
            callee,
            preceding,
        })
    }

    fn replay_call(&mut self, call: CallObservation) -> Result<()> {
        self.tick()?;
        let (head, tail) = call
            .callee
            .find('.')
            .map_or((call.callee.as_str(), ""), |i| {
                (&call.callee[..i], &call.callee[i..])
            });
        let alias =
            self.projection
                .lookup(self.scope, head, &mut work(self.sink, self.check, None))?;
        let target = alias.map(|id| self.projection.target(id)).unwrap_or(head);
        let size = target.len() + tail.len();
        self.sink.charge(size, &mut || (self.check)())?;
        let mut resolved = String::with_capacity(size);
        for part in [target, tail] {
            for (i, ch) in part.chars().enumerate() {
                if i % 256 == 0 && (self.check)() {
                    return Err(Error::Cancelled);
                }
                resolved.push(ch);
            }
        }
        let terminal = resolved.rsplit('.').next().unwrap_or("");
        if let Some(fact) = call.preceding {
            self.emit_lines(fact.kind, call.line, call.end, fact.value, None)?;
        }
        self.emit_lines("call", call.line, call.end, call.value, None)?;
        if matches!(
            terminal,
            "__import__"
                | "import_module"
                | "iter_modules"
                | "walk_packages"
                | "entry_points"
                | "load_entry_point"
                | "find_spec"
                | "exec"
                | "eval"
        ) {
            let size = resolved.len() + 100;
            self.charge(size)?;
            let mut message = String::with_capacity(size);
            use std::fmt::Write;
            write!(message, "line {}: ", call.line).expect("String writer");
            for (i, ch) in resolved.chars().enumerate() {
                if i % 256 == 0 {
                    self.tick()?;
                }
                message.push(ch);
            }
            message.push_str("; runtime targets are not inferred");
            let capacity = message.capacity();
            let result = self.sink.warn(
                "dynamic_discovery_unresolved",
                &self.source.path,
                message,
                &mut || (self.check)(),
            );
            self.sink.release(capacity);
            result?;
            #[cfg(test)]
            if let Some(trace) = &mut self.trace {
                trace.push(Observation::Warning {
                    warning: self.sink.warnings().last().unwrap().clone(),
                });
            }
        }
        self.drop_text(resolved)?;
        self.drop_text(call.callee)?;
        Ok(())
    }

    fn visits(
        &mut self,
        id: usize,
        mut header: Option<(&mut HeaderSchedule, HeaderGroup)>,
    ) -> Result<()> {
        // Normalization checks semantic depth before recursion. Replay follows
        // only those child edges, so its recursion cannot exceed that bound.
        self.tick()?;
        if matches!(self.arena[id].shape, Shape::Call { .. }) {
            let call = self.observation(id)?;
            if let Some((schedule, group)) = &mut header {
                schedule.capture_visit(
                    *group,
                    call,
                    200_000,
                    &mut work(self.sink, self.check, None),
                )?;
            } else {
                self.replay_call(call)?;
            }
        }
        for i in 0..self.arena[id].children.len() {
            self.tick()?;
            let child = self.arena[id].children[i];
            match &mut header {
                Some((schedule, group)) => self.visits(child, Some((schedule, *group)))?,
                None => self.visits(child, None)?,
            }
        }
        Ok(())
    }

    fn expression(&mut self, expr: &Expr, depth: usize) -> Result<()> {
        self.arena_scope(|driver| {
            let id = driver.normalize(expr, depth + 1)?;
            driver.visits(id, None)
        })
    }

    fn mutation_expressions(
        &mut self,
        targets: &[Expr],
        range: TextRange,
        depth: usize,
    ) -> Result<()> {
        self.arena_scope(|driver| {
            driver.charge_arena(targets.len() * std::mem::size_of::<usize>())?;
            let mut ids = Vec::with_capacity(targets.len());
            for target in targets {
                ids.push(driver.normalize(target, depth + 1)?);
            }
            for &id in &ids {
                driver.mutation(id, range)?;
            }
            for id in ids {
                driver.visits(id, None)?;
            }
            Ok(())
        })
    }

    fn optional(&mut self, expr: Option<&Expr>, depth: usize) -> Result<()> {
        if let Some(expr) = expr {
            self.expression(expr, depth)?;
        }
        Ok(())
    }

    fn statements(&mut self, body: &[Stmt], depth: usize) -> Result<()> {
        for stmt in body {
            self.statement(stmt, depth + 1)?;
        }
        Ok(())
    }

    fn statement(&mut self, stmt: &Stmt, depth: usize) -> Result<()> {
        let before_work = self.sink.working();
        let before_persistent = self.persistent_work;
        self.statement_inner(stmt, depth)?;
        if !self.arena.is_empty() || self.arena_work != 0 {
            return Err(Error::Resource);
        }
        // The inner call has dropped every statement-local buffer. Nested
        // regions preserve live outer headers, but alias replacement may shrink
        // persistent storage, so use absolute totals rather than unsigned deltas.
        self.finish_region(before_work, before_persistent)
    }

    fn statement_inner(&mut self, stmt: &Stmt, depth: usize) -> Result<()> {
        self.node(depth)?;
        match stmt {
            Stmt::FunctionDef(n) => self.function(n, depth)?,
            Stmt::ClassDef(n) => self.class(n, depth)?,
            Stmt::Import(n) => self.imports(stmt.range(), "import", "", 0, &n.names, depth)?,
            Stmt::ImportFrom(n) => self.imports(
                stmt.range(),
                "from_import",
                n.module.as_ref().map_or("", |n| n.as_str()),
                n.level as usize,
                &n.names,
                depth,
            )?,
            Stmt::Assign(n) => {
                let range = self.corrected_range(type_comments::NodeKind::Assign, stmt.range());
                self.assignment(range, &n.targets, None, Some(&n.value), depth)?
            }
            Stmt::AnnAssign(n) => self.assignment(
                stmt.range(),
                std::slice::from_ref(&*n.target),
                Some(&n.annotation),
                n.value.as_deref(),
                depth,
            )?,
            Stmt::AugAssign(n) => {
                self.mutation_expressions(std::slice::from_ref(&*n.target), stmt.range(), depth)?;
                self.expression(&n.value, depth)?;
            }
            Stmt::Delete(n) => {
                self.mutation_expressions(&n.targets, stmt.range(), depth)?;
            }
            Stmt::Expr(n) => self.expression(&n.value, depth)?,
            Stmt::Return(n) => self.optional(n.value.as_deref(), depth)?,
            Stmt::If(n) => {
                self.expression(&n.test, depth)?;
                self.statements(&n.body, depth)?;
                for clause in &n.elif_else_clauses {
                    self.node(depth + 1)?;
                    self.optional(clause.test.as_ref(), depth + 1)?;
                    self.statements(&clause.body, depth + 1)?;
                }
            }
            Stmt::While(n) => {
                self.expression(&n.test, depth)?;
                self.statements(&n.body, depth)?;
                self.statements(&n.orelse, depth)?;
            }
            Stmt::For(n) => {
                self.expression(&n.target, depth)?;
                self.expression(&n.iter, depth)?;
                self.statements(&n.body, depth)?;
                self.statements(&n.orelse, depth)?;
            }
            Stmt::With(n) => {
                for item in &n.items {
                    self.node(depth + 1)?;
                    self.expression(&item.context_expr, depth + 1)?;
                    self.optional(item.optional_vars.as_deref(), depth + 1)?;
                }
                self.statements(&n.body, depth)?;
            }
            Stmt::Try(n) => {
                self.statements(&n.body, depth)?;
                for handler in &n.handlers {
                    let ast::ExceptHandler::ExceptHandler(h) = handler;
                    self.node(depth + 1)?;
                    self.optional(h.type_.as_deref(), depth + 1)?;
                    self.statements(&h.body, depth + 1)?;
                }
                self.statements(&n.orelse, depth)?;
                self.statements(&n.finalbody, depth)?;
            }
            Stmt::Raise(n) => {
                self.optional(n.exc.as_deref(), depth)?;
                self.optional(n.cause.as_deref(), depth)?;
            }
            Stmt::Assert(n) => {
                self.expression(&n.test, depth)?;
                self.optional(n.msg.as_deref(), depth)?;
            }
            Stmt::Match(n) => {
                self.expression(&n.subject, depth)?;
                for case in &n.cases {
                    self.node(depth + 1)?;
                    self.pattern(&case.pattern, depth + 2)?;
                    self.optional(case.guard.as_deref(), depth + 1)?;
                    self.statements(&case.body, depth + 1)?;
                }
            }
            Stmt::TypeAlias(n) => {
                self.expression(&n.name, depth)?;
                self.type_parameters(n.type_params.as_deref(), None, depth + 1)?;
                self.expression(&n.value, depth)?;
            }
            Stmt::Global(n) => {
                for _ in &n.names {
                    self.tick()?;
                }
            }
            Stmt::Nonlocal(n) => {
                for _ in &n.names {
                    self.tick()?;
                }
            }
            Stmt::Pass(_) | Stmt::Break(_) | Stmt::Continue(_) => {}
            Stmt::IpyEscapeCommand(_) => return Err(Error::Unsupported("statement variant")),
        }
        Ok(())
    }

    fn pattern(&mut self, pattern: &ast::Pattern, depth: usize) -> Result<()> {
        self.node(depth)?;
        match pattern {
            ast::Pattern::MatchValue(n) => self.expression(&n.value, depth)?,
            ast::Pattern::MatchSingleton(_) | ast::Pattern::MatchStar(_) => {}
            ast::Pattern::MatchSequence(n) => {
                for child in &n.patterns {
                    self.pattern(child, depth + 1)?;
                }
            }
            ast::Pattern::MatchMapping(n) => {
                // AST fields group all keys before all corresponding patterns.
                for key in &n.keys {
                    self.expression(key, depth)?;
                }
                for child in &n.patterns {
                    self.pattern(child, depth + 1)?;
                }
            }
            ast::Pattern::MatchClass(n) => {
                // The class expression is not a constructor call; keyword attrs
                // and capture names are strings, not expression/binding edges.
                self.expression(&n.cls, depth)?;
                self.node(depth + 1)?;
                for child in &n.arguments.patterns {
                    self.pattern(child, depth + 2)?;
                }
                for keyword in &n.arguments.keywords {
                    self.node(depth + 2)?;
                    self.pattern(&keyword.pattern, depth + 3)?;
                }
            }
            ast::Pattern::MatchAs(n) => {
                if let Some(child) = &n.pattern {
                    self.pattern(child, depth + 1)?;
                }
            }
            ast::Pattern::MatchOr(n) => {
                for child in &n.patterns {
                    self.pattern(child, depth + 1)?;
                }
            }
        }
        Ok(())
    }

    fn type_parameters(
        &mut self,
        parameters: Option<&ast::TypeParams>,
        mut schedule: Option<&mut HeaderSchedule>,
        depth: usize,
    ) -> Result<()> {
        let Some(parameters) = parameters else {
            return Ok(());
        };
        self.node(depth)?;
        // Defensive guard: the frontend rejects defaults through its version channel.
        for parameter in &parameters.type_params {
            self.node(depth + 1)?;
            if parameter.default().is_some() {
                return Err(Error::Unsupported("type parameter default"));
            }
        }
        for parameter in &parameters.type_params {
            self.tick()?;
            match parameter {
                ast::TypeParam::TypeVar(n) => {
                    if let Some(bound) = &n.bound {
                        match &mut schedule {
                            Some(schedule) => self.header_visits(
                                schedule,
                                bound,
                                HeaderGroup::TypeParameters,
                                depth + 1,
                            )?,
                            None => self.expression(bound, depth + 1)?,
                        }
                    }
                }
                ast::TypeParam::TypeVarTuple(_) | ast::TypeParam::ParamSpec(_) => {}
            }
        }
        Ok(())
    }

    fn imports(
        &mut self,
        range: TextRange,
        kind: &str,
        module: &str,
        level: usize,
        names: &[ast::Alias],
        depth: usize,
    ) -> Result<()> {
        self.charge(names.len() * std::mem::size_of::<shared::ImportAlias<'_>>())?;
        let mut aliases = Vec::with_capacity(names.len());
        for alias in names {
            self.node(depth + 1)?;
            aliases.push(shared::ImportAlias {
                name: alias.name.as_str(),
                asname: alias.asname.as_ref().map(|n| n.as_str()),
            });
        }
        let metadata = shared::ImportMetadata {
            module,
            level,
            names: &aliases,
        };
        let count = shared::write_import(
            shared::CountWriter::new(OUTPUT),
            &metadata,
            OUTPUT,
            &mut || (self.check)(),
        )?
        .count();
        self.charge(count)?;
        let value = shared::write_import(Vec::with_capacity(count), &metadata, count, &mut || {
            (self.check)()
        })?;
        self.emit(
            kind,
            range,
            String::from_utf8(value).expect("ASCII JSON"),
            None,
        )?;
        if level == 0 {
            for alias in names {
                self.tick()?;
                let name = alias.name.as_str();
                let local = alias
                    .asname
                    .as_ref()
                    .map_or_else(|| name.split('.').next().unwrap_or(name), |n| n.as_str());
                let target = if kind == "import" && alias.asname.is_none() {
                    local
                } else {
                    name
                };
                let replaced = self.projection.import(
                    self.scope,
                    local,
                    module,
                    target,
                    &mut work(self.sink, self.check, Some(&mut self.persistent_work)),
                )?;
                self.persistent_work = self
                    .persistent_work
                    .checked_sub(replaced)
                    .ok_or(Error::Resource)?;
                self.release(replaced)?;
            }
        }
        Ok(())
    }

    fn mutation(&mut self, id: usize, range: TextRange) -> Result<()> {
        if let Shape::Attribute(_, attr) = &self.arena[id].shape {
            let direct = matches!(attr.as_str(), "router" | "prefix");
            let shape = self.render(id)?;
            if direct || shape.contains(".router.") {
                let value = self.json_pairs(&[("target", &shape)])?;
                self.emit("router_mutation", range, value, None)?;
            }
            self.drop_text(shape)?;
        }
        Ok(())
    }

    fn eligible(&mut self, id: usize) -> Result<bool> {
        if matches!(self.arena[id].shape, Shape::Name(_)) {
            return Ok(true);
        }
        let mut root = id;
        while let Shape::Attribute(parent, _) = self.arena[root].shape {
            self.tick()?;
            root = parent;
        }
        Ok(matches!(&self.arena[id].shape, Shape::Attribute(_, _))
            && matches!(&self.arena[root].shape, Shape::Name(n) if matches!(n.as_str(), "self" | "cls")))
    }

    fn bindings(
        &mut self,
        id: usize,
        destructured: bool,
        source: Option<usize>,
        range: TextRange,
    ) -> Result<()> {
        self.tick()?;
        let eligible = self.eligible(id)?;
        match self.arena[id].shape {
            Shape::Sequence => {
                for i in (0..self.arena[id].children.len()).rev() {
                    let child = self.arena[id].children[i];
                    self.bindings(child, true, source, range)?;
                }
            }
            Shape::Starred(child) => self.bindings(child, true, source, range)?,
            _ if eligible => {
                let target = self.render(id)?;
                let binding = !destructured
                    && source.is_some_and(|s| {
                        matches!(
                            self.arena[s].shape,
                            Shape::Name(_) | Shape::Attribute(_, _) | Shape::Call { .. }
                        )
                    });
                let value = if binding {
                    let source = self.render(source.unwrap())?;
                    let value = self.json_pairs(&[("source", &source), ("target", &target)])?;
                    self.drop_text(source)?;
                    value
                } else {
                    self.json_pairs(&[("target", &target)])?
                };
                self.drop_text(target)?;
                self.emit(
                    if binding {
                        "binding"
                    } else {
                        "binding_invalidation"
                    },
                    range,
                    value,
                    None,
                )?;
            }
            _ => {}
        }
        Ok(())
    }

    fn assignment(
        &mut self,
        range: TextRange,
        targets: &[Expr],
        annotation: Option<&Expr>,
        value: Option<&Expr>,
        depth: usize,
    ) -> Result<()> {
        self.arena_scope(|driver| driver.assignment_inner(range, targets, annotation, value, depth))
    }

    fn assignment_inner(
        &mut self,
        range: TextRange,
        targets: &[Expr],
        annotation: Option<&Expr>,
        value: Option<&Expr>,
        depth: usize,
    ) -> Result<()> {
        self.charge_arena(targets.len() * std::mem::size_of::<usize>())?;
        let mut ids = Vec::with_capacity(targets.len());
        for target in targets {
            ids.push(self.normalize(target, depth + 1)?);
        }
        let annotation = annotation
            .map(|e| self.normalize(e, depth + 1))
            .transpose()?;
        let value = value.map(|e| self.normalize(e, depth + 1)).transpose()?;
        if let Some(anno) = annotation {
            if self.owners.last().is_some_and(|(_, class)| *class) {
                if let Shape::Name(name) = &self.arena[ids[0]].shape {
                    self.sink.charge(name.len(), &mut || (self.check)())?;
                    let name = name.clone();
                    let shape = self.render(anno)?;
                    self.emit("attribute_annotation", range, shape, Some(&name))?;
                    self.drop_text(name)?;
                }
            }
        }
        for &id in &ids {
            self.mutation(id, range)?;
        }
        if let Some(value) = value {
            self.constructor_metadata(&ids, value, range)?;
        }
        if value.is_some() {
            for &id in ids.iter().rev() {
                self.bindings(id, false, value, range)?;
            }
        }
        for id in ids {
            self.visits(id, None)?;
        }
        if let Some(id) = annotation {
            self.visits(id, None)?;
        }
        if let Some(id) = value {
            self.visits(id, None)?;
        }
        Ok(())
    }

    fn enter(&mut self, name: &str, class: bool) -> Result<ScopeId> {
        let previous = self.scope;
        self.scope = self.projection.enter(
            previous,
            name,
            256,
            &mut work(self.sink, self.check, Some(&mut self.persistent_work)),
        )?;
        if self.owners.len() == self.owner_slots {
            let bytes = 4 * std::mem::size_of::<(String, bool)>();
            let next = self
                .persistent_work
                .checked_add(bytes)
                .ok_or(Error::Resource)?;
            self.charge(bytes)?;
            self.persistent_work = next;
            self.owner_slots += 1;
        }
        let name = self.text(name)?;
        self.owners.push((name, class));
        Ok(previous)
    }

    fn header_expression(
        &mut self,
        schedule: &mut HeaderSchedule,
        expr: &Expr,
        group: HeaderGroup,
        depth: usize,
    ) -> Result<projection::Shape> {
        self.arena_scope(|driver| {
            let id = driver.normalize(expr, depth + 1)?;
            let value = driver.render(id)?;
            let (line, end) = driver.span(driver.arena[id].range);
            driver.visits(id, Some((schedule, group)))?;
            Ok(projection::Shape { line, end, value })
        })
    }

    fn header_visits(
        &mut self,
        schedule: &mut HeaderSchedule,
        expr: &Expr,
        group: HeaderGroup,
        depth: usize,
    ) -> Result<()> {
        self.arena_scope(|driver| {
            let id = driver.normalize(expr, depth + 1)?;
            driver.visits(id, Some((schedule, group)))
        })
    }

    fn stage(&mut self, schedule: &mut HeaderSchedule, metadata: HeaderMetadata) -> Result<()> {
        schedule.stage(metadata, 0, 200_000, &mut work(self.sink, self.check, None))?;
        Ok(())
    }

    fn parameter(
        &mut self,
        schedule: &mut HeaderSchedule,
        parameter: &ast::Parameter,
        default: Option<&Expr>,
        groups: (HeaderGroup, HeaderGroup, u8),
        depth: usize,
    ) -> Result<()> {
        let (annotation_group, default_group, order) = groups;
        self.node(depth + 1)?;
        let name = self.text(parameter.name.as_str())?;
        let (line, end) = self.span(TextRange::new(
            parameter.name.range().start(),
            self.corrected_range(type_comments::NodeKind::Parameter, parameter.range())
                .end(),
        ));
        let annotation = parameter
            .annotation
            .as_deref()
            .map(|expr| self.header_expression(schedule, expr, annotation_group, depth))
            .transpose()?
            .map(|s| s.value);
        let default = default
            .map(|expr| self.header_expression(schedule, expr, default_group, depth))
            .transpose()?;
        self.stage(
            schedule,
            HeaderMetadata::Parameter {
                order,
                name,
                line,
                end,
                annotation,
                default,
            },
        )
    }

    fn event(&mut self, event: HeaderEvent) -> Result<()> {
        match event {
            HeaderEvent::Visit(call) => self.replay_call(call),
            HeaderEvent::Metadata(HeaderMetadata::Base(s)) => {
                self.emit_lines("base", s.line, s.end, s.value, None)
            }
            HeaderEvent::Metadata(HeaderMetadata::Return(s)) => {
                self.emit_lines("return_annotation", s.line, s.end, s.value, None)
            }
            HeaderEvent::Metadata(HeaderMetadata::Parameter {
                name,
                line,
                end,
                annotation,
                default,
                ..
            }) => {
                let value = self.text(&name)?;
                self.emit_lines("parameter", line, end, value, Some(&name))?;
                if let Some(value) = annotation {
                    self.emit_lines("parameter_annotation", line, end, value, Some(&name))?;
                }
                if let Some(s) = default {
                    self.emit_lines("parameter_default", s.line, s.end, s.value, Some(&name))?;
                }
                self.drop_text(name)?;
                Ok(())
            }
        }
    }

    fn drain(
        &mut self,
        schedule: &mut HeaderSchedule,
        kind: DefinitionKind,
        after: bool,
    ) -> Result<()> {
        let mut failure = None;
        // Schedule callbacks are sequential. A scoped checked borrow lets both
        // callbacks use the same cancellation latch, including metadata scans
        // that have not yet reached a matching event.
        let result = {
            let driver = std::cell::RefCell::new(&mut *self);
            let mut check = || {
                driver
                    .borrow_mut()
                    .tick()
                    .map_err(|_| projection::Error::Cancelled)
            };
            let mut emit = |event| {
                driver.borrow_mut().event(event).map_err(|error| {
                    failure = Some(error);
                    projection::Error::Unsupported
                })
            };
            if after {
                schedule.after(kind, &mut check, &mut emit)
            } else {
                schedule.before(kind, &mut check, &mut emit)
            }
        };
        if let Some(error) = failure {
            return Err(error);
        }
        result?;
        self.tick()
    }

    fn function(&mut self, n: &ast::StmtFunctionDef, depth: usize) -> Result<()> {
        let previous = self.enter(n.name.as_str(), false)?;
        let range = self.definition_range(n.range(), n.name.range(), false, n.is_async)?;
        let value = self.text(n.name.as_str())?;
        self.emit(
            if n.is_async {
                "async_function"
            } else {
                "function"
            },
            range,
            value,
            None,
        )?;
        let mut schedule = HeaderSchedule::default();
        self.decorators(&n.decorator_list, &mut schedule, true, depth)?;
        let p = &n.parameters;
        self.node(depth + 1)?;
        let mut positional = Vec::new();
        let mut keyword = Vec::new();
        self.charge(
            (p.posonlyargs.len() + p.args.len() + p.kwonlyargs.len())
                * 4
                * std::mem::size_of::<String>(),
        )?;
        for arg in p.posonlyargs.iter().chain(&p.args) {
            positional.push(self.text(arg.parameter.name.as_str())?);
        }
        for arg in &p.kwonlyargs {
            keyword.push(self.text(arg.parameter.name.as_str())?);
        }
        self.charge(2 * 16384 + 17)?;
        let signature = shared::signature(
            shared::Signature {
                positional: &positional,
                positional_only: p.posonlyargs.len(),
                keyword: &keyword,
                vararg: p.vararg.as_ref().map(|a| a.name.as_str()),
                kwarg: p.kwarg.as_ref().map(|a| a.name.as_str()),
            },
            &mut || (self.check)(),
        )?;
        self.sink.release(2 * 16384 + 17 - signature.capacity());
        self.emit("signature", range, signature, None)?;
        use HeaderGroup::*;
        for arg in &p.posonlyargs {
            self.parameter(
                &mut schedule,
                &arg.parameter,
                arg.default.as_deref(),
                (PositionalOnlyAnnotations, PositionalDefaults, 0),
                depth,
            )?;
        }
        for arg in &p.args {
            self.parameter(
                &mut schedule,
                &arg.parameter,
                arg.default.as_deref(),
                (PositionalAnnotations, PositionalDefaults, 0),
                depth,
            )?;
        }
        for arg in &p.kwonlyargs {
            self.parameter(
                &mut schedule,
                &arg.parameter,
                arg.default.as_deref(),
                (KeywordOnlyAnnotations, KeywordDefaults, 0),
                depth,
            )?;
        }
        if let Some(arg) = &p.vararg {
            self.parameter(
                &mut schedule,
                arg,
                None,
                (VarargAnnotation, PositionalDefaults, 1),
                depth,
            )?;
        }
        if let Some(arg) = &p.kwarg {
            self.parameter(
                &mut schedule,
                arg,
                None,
                (KwargAnnotation, PositionalDefaults, 2),
                depth,
            )?;
        }
        if let Some(expr) = &n.returns {
            let shape = self.header_expression(&mut schedule, expr, ReturnAnnotation, depth)?;
            self.stage(&mut schedule, HeaderMetadata::Return(shape))?;
        }
        self.type_parameters(n.type_params.as_deref(), Some(&mut schedule), depth + 1)?;
        self.drain(&mut schedule, DefinitionKind::Function, false)?;
        self.statements(&n.body, depth)?;
        self.drain(&mut schedule, DefinitionKind::Function, true)?;
        self.owners.pop();
        self.scope = previous;
        Ok(())
    }

    fn class(&mut self, n: &ast::StmtClassDef, depth: usize) -> Result<()> {
        let previous = self.enter(n.name.as_str(), true)?;
        let range = self.definition_range(n.range(), n.name.range(), true, false)?;
        let value = self.text(n.name.as_str())?;
        self.emit("class", range, value, None)?;
        let mut schedule = HeaderSchedule::default();
        self.decorators(&n.decorator_list, &mut schedule, false, depth)?;
        if let Some(args) = &n.arguments {
            for expr in &args.args {
                let shape =
                    self.header_expression(&mut schedule, expr, HeaderGroup::Bases, depth)?;
                self.stage(&mut schedule, HeaderMetadata::Base(shape))?;
            }
            for kw in &args.keywords {
                self.node(depth + 1)?;
                let shape =
                    self.header_expression(&mut schedule, &kw.value, HeaderGroup::Keywords, depth)?;
                self.drop_text(shape.value)?;
            }
        }
        self.type_parameters(n.type_params.as_deref(), Some(&mut schedule), depth + 1)?;
        self.drain(&mut schedule, DefinitionKind::Class, false)?;
        self.statements(&n.body, depth)?;
        self.drain(&mut schedule, DefinitionKind::Class, true)?;
        self.owners.pop();
        self.scope = previous;
        Ok(())
    }
}

#[cfg(test)]
#[path = "../python_parity/ruff_facts_batch.rs"]
mod ruff_facts_batch;

#[cfg(test)]
#[path = "../python_parity/ruff_compat_batch.rs"]
mod ruff_compat_batch;
