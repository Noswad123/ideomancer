//! Deliberately small, transactional candidate policy. Not production evidence.
use super::*;
use std::collections::HashSet;
use tree_sitter::Node;

const FACT_POLICY: &str = "python-native-private-facts-1";
const MAX_FACTS: usize = 100_000;
const SIGNATURE_WORK: usize = 2 * 16384 + 17;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(in crate::analysis) struct CandidateFact {
    pub(in crate::analysis) id: String,
    pub(in crate::analysis) kind: String,
    pub(in crate::analysis) path: String,
    pub(in crate::analysis) line: usize,
    #[serde(rename = "endLine")]
    pub(in crate::analysis) end_line: usize,
    pub(in crate::analysis) symbol: String,
    pub(in crate::analysis) value: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub(in crate::analysis) enum CandidateStatus {
    AcceptedFacts,
    UnsupportedSyntax,
    SyntaxError,
    UnsupportedEncoding,
    ResourceLimit,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub(in crate::analysis) enum CandidateDiagnostic {
    UnsupportedSyntax,
    FactLimit,
    OutputLimit,
    WorkingLimit,
    Parse(DiagnosticCode),
}

#[derive(Debug, Serialize)]
pub(in crate::analysis) struct CandidateFile<'a> {
    pub(in crate::analysis) path: &'a str,
    pub(in crate::analysis) module: &'a str,
    pub(in crate::analysis) sha256: String,
    pub(in crate::analysis) status: CandidateStatus,
    pub(in crate::analysis) diagnostic: Option<CandidateDiagnostic>,
    pub(in crate::analysis) nodes_visited: usize,
    pub(in crate::analysis) max_depth: usize,
    pub(in crate::analysis) traversal_finished: bool,
}

#[derive(Debug, Serialize)]
pub(in crate::analysis) struct CandidateReport<'a> {
    pub(in crate::analysis) identity: BatchParserIdentity,
    pub(in crate::analysis) files: Vec<CandidateFile<'a>>,
    pub(in crate::analysis) facts: Vec<CandidateFact>,
}

#[derive(Clone, Copy)]
struct FactLimits {
    count: usize,
    bytes: usize,
    // Separate from encoded output: pending owned text, conservative container
    // capacity, and stage temporaries. Fixed cursor frames remain depth-bounded;
    // this is not an allocator/RSS or native-parser memory sandbox.
    working: usize,
}

impl Default for FactLimits {
    fn default() -> Self {
        Self {
            count: MAX_FACTS,
            bytes: super::super::MAX_OUTPUT,
            working: super::super::MAX_OUTPUT,
        }
    }
}

/// Captured-byte candidates only; the production AST extractor never calls this.
pub(in crate::analysis) fn extract_fact_candidates(
    sources: &[Source],
) -> Result<CandidateReport<'_>, FrontEndError> {
    let start = Instant::now();
    extract(sources, Limits::default(), FactLimits::default(), |_| {
        start.elapsed() >= Duration::from_secs(60)
    })
}

struct CheckedWriter<'a, F> {
    inner: CountWriter,
    cancel: &'a Cancellation<F>,
}
impl<F: FnMut(Checkpoint) -> bool> Write for CheckedWriter<'_, F> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.cancel.at(Checkpoint::Output) {
            return Err(io::Error::other("deadline"));
        }
        self.inner.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn measured<F: FnMut(Checkpoint) -> bool>(
    value: &impl Serialize,
    limit: usize,
    cancel: &Cancellation<F>,
) -> Option<usize> {
    let mut writer = CheckedWriter {
        inner: CountWriter { count: 0, limit },
        cancel,
    };
    serde_json::to_writer(&mut writer, value).ok()?;
    Some(writer.inner.count)
}

fn reserve_candidates<F: FnMut(Checkpoint) -> bool>(
    sources: &[Source],
    limits: Limits,
    fact_limits: FactLimits,
    cancel: &Cancellation<F>,
) -> Result<usize, FrontEndError> {
    #[derive(Serialize)]
    struct Baseline<'a> {
        id: &'static str,
        kind: &'static str,
        path: &'a str,
        line: usize,
        #[serde(rename = "endLine")]
        end_line: usize,
        symbol: &'a str,
        value: &'a str,
    }
    const ZERO_SHA: &str = "0000000000000000000000000000000000000000000000000000000000000000";
    if sources.len() > fact_limits.count {
        return Err(FrontEndError::ReportLimit);
    }
    let mut header = identity_header();
    header.policy = FACT_POLICY;
    header.sha256 = ZERO_SHA.into();
    let empty = CandidateReport {
        identity: header,
        files: Vec::new(),
        facts: Vec::new(),
    };
    let mut summary = json_size(&empty, limits.report)?;
    let mut baselines = 0;
    for (index, source) in sources.iter().enumerate() {
        cancel.at(Checkpoint::Preflight);
        let worst = CandidateFile {
            path: &source.path,
            module: &source.module,
            sha256: ZERO_SHA.into(),
            status: CandidateStatus::UnsupportedEncoding,
            diagnostic: Some(CandidateDiagnostic::Parse(
                DiagnosticCode::EncodingCookieUnsupported,
            )),
            nodes_visited: usize::MAX,
            max_depth: usize::MAX,
            traversal_finished: false,
        };
        summary += json_size(&worst, limits.report)? + usize::from(index != 0);
        if summary > limits.report {
            return Err(FrontEndError::ReportLimit);
        }
        let baseline = Baseline {
            id: concat!(
                "module_",
                "0000000000000000000000000000000000000000000000000000000000000000"
            ),
            kind: "module",
            path: &source.path,
            line: 1,
            end_line: 1,
            symbol: &source.module,
            value: &source.module,
        };
        baselines += json_size(&baseline, fact_limits.bytes)? + usize::from(index != 0);
        if summary + baselines > fact_limits.bytes {
            return Err(FrontEndError::ReportLimit);
        }
    }
    if summary + baselines > fact_limits.bytes {
        return Err(FrontEndError::ReportLimit);
    }
    Ok(summary + baselines)
}

fn extract<F: FnMut(Checkpoint) -> bool>(
    sources: &[Source],
    limits: Limits,
    fact_limits: FactLimits,
    check: F,
) -> Result<CandidateReport<'_>, FrontEndError> {
    let cancel = Cancellation {
        expired: Cell::new(false),
        check: RefCell::new(check),
    };
    // All admission and borrowed metadata accounting precede identity hashing,
    // baseline cloning, and fact-ID work. No parser is involved in admission.
    admit(sources, limits, &cancel)?;
    let reserved = reserve_candidates(sources, limits, fact_limits, &cancel)?;
    let mut candidate_identity = identity(&cancel);
    candidate_identity.policy = FACT_POLICY;
    let mut hash = Sha256::new();
    for part in [
        candidate_identity.sha256.as_bytes(),
        FACT_POLICY.as_bytes(),
        include_bytes!("facts.rs").as_slice(),
    ] {
        hash.update((part.len() as u64).to_le_bytes());
        for chunk in part.chunks(4096) {
            cancel.at(Checkpoint::Identity);
            hash.update(chunk);
        }
    }
    candidate_identity.sha256 = format!("{:x}", hash.finalize());
    let baselines: Vec<_> = sources
        .iter()
        .map(|s| {
            cancel.at(Checkpoint::Baseline);
            CandidateFact {
                id: format!(
                    "module_{}",
                    digest(s.path.as_bytes(), &cancel, Checkpoint::FactId)
                ),
                kind: "module".into(),
                path: s.path.clone(),
                line: 1,
                end_line: 1,
                symbol: s.module.clone(),
                value: s.module.clone(),
            }
        })
        .collect();
    let mut result = CandidateReport {
        identity: candidate_identity,
        files: Vec::new(),
        facts: baselines,
    };
    let mut collector = Collector {
        sources,
        index: 0,
        facts: &mut result.facts,
        files: Vec::new(),
        frames: Vec::new(),
        functions: Vec::new(),
        unsupported: false,
        limited: None,
        terminal_end: 0,
        start: 0,
        used: reserved,
        start_used: reserved,
        limits: fact_limits,
        pending: 0,
    };
    let parsed = run_admitted(sources, limits, &cancel, &mut collector)?;
    result.files = std::mem::take(&mut collector.files);
    drop(collector);
    // Global ID deduplication before the helper's exact tuple ordering.
    let mut ids = HashSet::new();
    result
        .facts
        .retain(|f| cancel.at(Checkpoint::Sort) || ids.insert(f.id.clone()));
    sort_facts(&mut result.facts, &cancel);
    let output_ok = cancel.expired.get() || measured(&result, fact_limits.bytes, &cancel).is_some();
    if cancel.at(Checkpoint::FinalOutput) || cancel.at(Checkpoint::Final) || !output_ok {
        result.facts.retain(|f| f.kind == "module");
        // Only bounded mandatory metadata remains if sorting was interrupted.
        result.facts.sort_unstable_by(|a, b| a.path.cmp(&b.path));
        for file in &mut result.files {
            file.status = CandidateStatus::ResourceLimit;
            file.diagnostic = Some(if cancel.expired.get() {
                CandidateDiagnostic::Parse(DiagnosticCode::Deadline)
            } else {
                CandidateDiagnostic::OutputLimit
            });
        }
        // Only baseline/failure metadata is counted without cancellation.
        // The positive path already passed measured() and its post-write check.
        json_size(&result, fact_limits.bytes)?;
    }
    // Also honor the shared run's final deadline invalidation.
    debug_assert_eq!(parsed.files.len(), result.files.len());
    Ok(result)
}

// An interruptible in-place heapsort preserves every baseline even when it
// stops midway. Unlike changing a std sort comparator on expiry, its ordering
// relation stays total and cancellation does not risk a sorting panic.
fn sort_facts<F: FnMut(Checkpoint) -> bool>(facts: &mut [CandidateFact], cancel: &Cancellation<F>) {
    fn key(f: &CandidateFact) -> (&str, usize, usize, &str, &str, &str, &str) {
        (
            &f.path, f.line, f.end_line, &f.kind, &f.symbol, &f.value, &f.id,
        )
    }
    fn sift<F: FnMut(Checkpoint) -> bool>(
        facts: &mut [CandidateFact],
        mut root: usize,
        end: usize,
        cancel: &Cancellation<F>,
    ) -> bool {
        loop {
            if cancel.at(Checkpoint::Sort) {
                return false;
            }
            let left = root * 2 + 1;
            if left >= end {
                return true;
            }
            let child = if left + 1 < end && key(&facts[left]) < key(&facts[left + 1]) {
                left + 1
            } else {
                left
            };
            if key(&facts[root]) >= key(&facts[child]) {
                return true;
            }
            facts.swap(root, child);
            root = child;
        }
    }
    if cancel.at(Checkpoint::Sort) {
        return;
    }
    for root in (0..facts.len() / 2).rev() {
        if !sift(facts, root, facts.len(), cancel) {
            return;
        }
    }
    for end in (1..facts.len()).rev() {
        if cancel.at(Checkpoint::Sort) {
            return;
        }
        facts.swap(0, end);
        if !sift(facts, 0, end, cancel) {
            return;
        }
    }
}

struct Frame {
    kind: &'static str,
    row: usize,
    column: usize,
    indent: Option<usize>,
    last_end: Option<usize>,
}

#[derive(Default)]
struct Function {
    working: usize,
    name: String,
    symbol: String,
    line: usize,
    end: usize,
    asynchronous: bool,
    names: HashSet<String>,
    positional: Vec<String>,
    keyword: Vec<String>,
    positional_only: usize,
    slash: bool,
    star: bool,
    bare_star: bool,
    vararg: Option<String>,
    kwarg: Option<String>,
    signature: String,
    header_end: usize,
}

struct Collector<'a, 'b> {
    sources: &'a [Source],
    index: usize,
    facts: &'b mut Vec<CandidateFact>,
    files: Vec<CandidateFile<'a>>,
    frames: Vec<Frame>,
    functions: Vec<Function>,
    unsupported: bool,
    limited: Option<CandidateDiagnostic>,
    terminal_end: usize,
    start: usize,
    used: usize,
    start_used: usize,
    limits: FactLimits,
    pending: usize,
}

fn identifier<F: FnMut(Checkpoint) -> bool>(name: &str, cancel: &Cancellation<F>) -> bool {
    let mut bytes = name.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z' | b'A'..=b'Z' | b'_'))
        && bytes.enumerate().all(|(index, b)| {
            (index % 256 != 0 || !cancel.at(Checkpoint::Collector))
                && (b.is_ascii_alphanumeric() || b == b'_')
        })
        && !matches!(
            name,
            "False"
                | "None"
                | "True"
                | "and"
                | "as"
                | "assert"
                | "async"
                | "await"
                | "break"
                | "class"
                | "continue"
                | "def"
                | "del"
                | "elif"
                | "else"
                | "except"
                | "finally"
                | "for"
                | "from"
                | "global"
                | "if"
                | "import"
                | "in"
                | "is"
                | "lambda"
                | "nonlocal"
                | "not"
                | "or"
                | "pass"
                | "raise"
                | "return"
                | "try"
                | "while"
                | "with"
                | "yield"
        )
}

fn join_symbol<F: FnMut(Checkpoint) -> bool>(
    prefix: &str,
    name: &str,
    limit: usize,
    cancel: &Cancellation<F>,
) -> Option<String> {
    let size = prefix.len().checked_add(name.len())?.checked_add(1)?;
    if size > limit {
        return None;
    }
    let mut symbol = String::with_capacity(size);
    for part in [prefix, ".", name] {
        for (index, ch) in part.chars().enumerate() {
            if index % 256 == 0 && cancel.at(Checkpoint::Collector) {
                return None;
            }
            symbol.push(ch);
        }
    }
    Some(symbol)
}

impl Collector<'_, '_> {
    fn reserve_work<F: FnMut(Checkpoint) -> bool>(
        &mut self,
        bytes: usize,
        cancel: &Cancellation<F>,
    ) -> bool {
        if cancel.at(Checkpoint::Working) {
            return false;
        }
        if bytes > self.limits.working.saturating_sub(self.pending) {
            self.limited = Some(CandidateDiagnostic::WorkingLimit);
            return false;
        }
        self.pending += bytes;
        true
    }
    fn add<F: FnMut(Checkpoint) -> bool>(
        &mut self,
        kind: &str,
        line: usize,
        end: usize,
        symbol: String,
        value: String,
        cancel: &Cancellation<F>,
    ) {
        if self.unsupported || self.limited.is_some() || cancel.at(Checkpoint::Collector) {
            return;
        }
        if self.facts.len() >= self.limits.count {
            self.limited = Some(CandidateDiagnostic::FactLimit);
            return;
        }
        let mut fact = CandidateFact {
            id: String::new(),
            kind: kind.into(),
            path: self.sources[self.index].path.clone(),
            line,
            end_line: end,
            symbol,
            value,
        };
        fact.id = fact_id(&fact, cancel);
        let Some(size) = measured(&fact, self.limits.bytes.saturating_sub(self.used), cancel)
        else {
            self.limited = Some(CandidateDiagnostic::OutputLimit);
            return;
        };
        if size + 1 > self.limits.bytes.saturating_sub(self.used) {
            self.limited = Some(CandidateDiagnostic::OutputLimit);
            return;
        }
        self.used += size + 1;
        self.facts.push(fact);
    }

    fn statement(&mut self, node: Node<'_>, bytes: &[u8]) {
        let Some(suite) = self.frames.last_mut() else {
            self.unsupported = true;
            return;
        };
        let point = node.start_position();
        if !matches!(suite.kind, "module" | "block") {
            self.unsupported = true;
            return;
        }
        if suite.last_end.is_some_and(|row| point.row <= row) {
            self.unsupported = true;
        }
        let inline = suite.kind == "block" && point.row == suite.row;
        if inline {
            if node.kind() != "pass_statement" || suite.last_end.is_some() {
                self.unsupported = true;
            }
        } else {
            let line_start = node.start_byte().saturating_sub(point.column);
            if !bytes[line_start..node.start_byte()]
                .iter()
                .all(|&b| b == b' ')
            {
                self.unsupported = true;
            }
            if suite.kind == "module" {
                if point.column != 0 {
                    self.unsupported = true;
                }
            } else {
                if point.column <= suite.column {
                    self.unsupported = true;
                }
                if suite.indent.is_some_and(|indent| indent != point.column) {
                    self.unsupported = true;
                }
                suite.indent = Some(point.column);
            }
        }
    }
}

impl Observer for Collector<'_, '_> {
    fn begin(&mut self) {
        self.start = self.facts.len();
        self.start_used = self.used;
        self.unsupported = false;
        self.limited = None;
        self.terminal_end = 0;
        self.frames.clear();
        self.functions.clear();
        self.pending = 0;
    }

    fn enter<F: FnMut(Checkpoint) -> bool>(
        &mut self,
        node: Node<'_>,
        field: Option<&str>,
        bytes: &[u8],
        cancel: &Cancellation<F>,
    ) {
        if cancel.at(Checkpoint::Collector) || self.unsupported || self.limited.is_some() {
            return;
        }
        let kind = node.kind();
        let parent = self.frames.last().map(|f| f.kind).unwrap_or("");
        if node.child_count() == 0 && kind != "module" {
            let gap = &bytes[self.terminal_end..node.start_byte()];
            if !trivia(gap, cancel) {
                self.unsupported = true;
            }
            self.terminal_end = node.end_byte();
        }
        match kind {
            "module" if parent.is_empty() => {
                for (chunk_index, chunk) in bytes.chunks(4096).enumerate() {
                    if cancel.at(Checkpoint::Collector) {
                        return;
                    }
                    if chunk.iter().enumerate().any(|(offset, &b)| {
                        b == 0
                            || (b == b'\r'
                                && bytes.get(chunk_index * 4096 + offset + 1) != Some(&b'\n'))
                    }) {
                        self.unsupported = true;
                    }
                }
            }
            "comment" => {
                let text = &bytes[node.start_byte()..node.end_byte()];
                if type_comment(text, cancel) {
                    self.unsupported = true;
                }
            }
            "function_definition" => {
                self.statement(node, bytes);
                let working = 2 * std::mem::size_of::<Function>();
                if !self.reserve_work(working, cancel) {
                    return;
                }
                cancel.at(Checkpoint::FunctionFrame);
                self.functions.push(Function {
                    working,
                    line: node.start_position().row + 1,
                    ..Function::default()
                });
            }
            "block" if parent == "function_definition" && field == Some("body") => {}
            "pass_statement" => self.statement(node, bytes),
            "parameters" if parent == "function_definition" && field == Some("parameters") => {
                if node.start_position().row + 1 != self.functions.last().unwrap().line {
                    self.unsupported = true;
                }
            }
            "identifier" => {
                let name =
                    std::str::from_utf8(&bytes[node.start_byte()..node.end_byte()]).unwrap_or("");
                if !identifier(name, cancel) {
                    self.unsupported = true;
                    return;
                }
                if parent == "function_definition" && field == Some("name") {
                    if node.start_position().row + 1 != self.functions.last().unwrap().line {
                        self.unsupported = true;
                    }
                    let prefix = if self.functions.len() > 1 {
                        &self.functions[self.functions.len() - 2].symbol
                    } else {
                        &self.sources[self.index].module
                    };
                    // Charge retained names and both symbol copies (including
                    // the eventual unwind clone) before allocating either.
                    let working = name.len() + 2 * (prefix.len() + name.len() + 1);
                    if !self.reserve_work(working, cancel) {
                        return;
                    }
                    self.functions.last_mut().unwrap().working += working;
                    let prefix = if self.functions.len() > 1 {
                        &self.functions[self.functions.len() - 2].symbol
                    } else {
                        &self.sources[self.index].module
                    };
                    if let Some(symbol) = join_symbol(
                        prefix,
                        name,
                        self.limits.bytes.saturating_sub(self.used),
                        cancel,
                    ) {
                        let f = self.functions.last_mut().unwrap();
                        f.name = name.into();
                        f.symbol = symbol;
                    } else {
                        self.limited = Some(CandidateDiagnostic::OutputLimit);
                    }
                } else if matches!(
                    parent,
                    "parameters" | "list_splat_pattern" | "dictionary_splat_pattern"
                ) {
                    // Two owned name copies plus conservative per-entry space
                    // for HashSet buckets and geometric Vec growth.
                    let working = 2 * name.len() + 256;
                    if !self.reserve_work(working, cancel) {
                        return;
                    }
                    let Some(f) = self.functions.last_mut() else {
                        self.unsupported = true;
                        return;
                    };
                    f.working += working;
                    if !f.names.insert(name.into()) || f.kwarg.is_some() {
                        self.unsupported = true;
                    }
                    match parent {
                        "list_splat_pattern" => {
                            f.vararg = Some(name.into());
                        }
                        "dictionary_splat_pattern" => {
                            f.kwarg = Some(name.into());
                        }
                        _ if f.star => f.keyword.push(name.into()),
                        _ => f.positional.push(name.into()),
                    }
                    let temporary = f.symbol.len() + 2 * name.len() + 1;
                    if !self.reserve_work(temporary, cancel) {
                        return;
                    }
                    let Some(symbol) = join_symbol(
                        &self.functions.last().unwrap().symbol,
                        name,
                        self.limits.bytes.saturating_sub(self.used),
                        cancel,
                    ) else {
                        self.limited = Some(CandidateDiagnostic::OutputLimit);
                        return;
                    };
                    self.add(
                        "parameter",
                        node.start_position().row + 1,
                        node.end_position().row + 1,
                        symbol,
                        name.into(),
                        cancel,
                    );
                    self.pending -= temporary;
                } else {
                    self.unsupported = true;
                }
            }
            "positional_separator" if parent == "parameters" => {
                let f = self.functions.last_mut().unwrap();
                if f.slash || f.star || f.kwarg.is_some() || f.positional.is_empty() {
                    self.unsupported = true;
                }
                f.slash = true;
                f.positional_only = f.positional.len();
            }
            "keyword_separator" | "list_splat_pattern" if parent == "parameters" => {
                let f = self.functions.last_mut().unwrap();
                if f.star || f.kwarg.is_some() {
                    self.unsupported = true;
                }
                f.star = true;
                f.bare_star = kind == "keyword_separator";
            }
            "dictionary_splat_pattern" if parent == "parameters" => {
                if self.functions.last().unwrap().kwarg.is_some() {
                    self.unsupported = true;
                }
            }
            "async" | "def" if parent == "function_definition" && !node.is_named() => {
                let f = self.functions.last_mut().unwrap();
                if node.start_position().row + 1 != f.line {
                    self.unsupported = true;
                }
                if kind == "async" {
                    f.asynchronous = true;
                }
            }
            ":" if parent == "function_definition" && !node.is_named() => {
                if node.start_position().row != self.functions.last().unwrap().header_end {
                    self.unsupported = true;
                }
            }
            "(" | ")" | "," if parent == "parameters" && !node.is_named() => {}
            "/" if parent == "positional_separator" && !node.is_named() => {}
            "*" if matches!(parent, "keyword_separator" | "list_splat_pattern")
                && !node.is_named() => {}
            "**" if parent == "dictionary_splat_pattern" && !node.is_named() => {}
            "pass" if parent == "pass_statement" && !node.is_named() => {}
            _ => self.unsupported = true,
        }
        let point = node.start_position();
        let (row, column) = if kind == "block" {
            let parent = self.frames.last().unwrap();
            // Inline suites are restricted to single-line headers in this slice.
            (parent.row, parent.column)
        } else {
            (point.row, point.column)
        };
        self.frames.push(Frame {
            kind,
            row,
            column,
            indent: None,
            last_end: None,
        });
    }

    fn exit<F: FnMut(Checkpoint) -> bool>(&mut self, node: Node<'_>, cancel: &Cancellation<F>) {
        if cancel.at(Checkpoint::Collector) || self.unsupported || self.limited.is_some() {
            return;
        }
        self.frames.pop();
        match node.kind() {
            "module" => {
                // The final gap can include bytes ignored by the grammar.
                let bytes = self.sources[self.index]
                    .bytes
                    .strip_prefix(b"\xef\xbb\xbf")
                    .unwrap_or(&self.sources[self.index].bytes);
                if !trivia(&bytes[self.terminal_end..], cancel) {
                    self.unsupported = true;
                }
            }
            "parameters" => {
                // Reserve the fixed writer and its final compact copy before
                // allocation, including short and truncated signatures.
                if !self.reserve_work(SIGNATURE_WORK, cancel) {
                    return;
                }
                let f = self.functions.last_mut().unwrap();
                f.working += SIGNATURE_WORK;
                if f.bare_star && f.keyword.is_empty() {
                    self.unsupported = true;
                }
                f.signature = signature(f, cancel);
                f.header_end = node.end_position().row;
            }
            "pass_statement" => {
                let end = node.end_position().row + 1;
                if let Some(f) = self.functions.last_mut() {
                    f.end = end;
                }
                if let Some(suite) = self.frames.last_mut() {
                    suite.last_end = Some(end - 1);
                }
            }
            "function_definition" => {
                let f = self.functions.pop().unwrap();
                if f.end == 0 || f.name.is_empty() {
                    self.unsupported = true;
                    return;
                }
                self.add(
                    if f.asynchronous {
                        "async_function"
                    } else {
                        "function"
                    },
                    f.line,
                    f.end,
                    f.symbol.clone(),
                    f.name,
                    cancel,
                );
                self.add("signature", f.line, f.end, f.symbol, f.signature, cancel);
                if let Some(parent) = self.functions.last_mut() {
                    parent.end = f.end;
                }
                if let Some(suite) = self.frames.last_mut() {
                    suite.last_end = Some(f.end - 1);
                }
                self.pending -= f.working;
            }
            _ => {}
        }
    }

    fn finish(&mut self, summary: &FileSummary<'_>) {
        let (status, diagnostic) = match summary.status {
            ParseStatus::ResourceLimit => (
                CandidateStatus::ResourceLimit,
                summary.diagnostic.map(CandidateDiagnostic::Parse),
            ),
            ParseStatus::SyntaxError => (
                CandidateStatus::SyntaxError,
                summary.diagnostic.map(CandidateDiagnostic::Parse),
            ),
            ParseStatus::UnsupportedEncoding => (
                CandidateStatus::UnsupportedEncoding,
                summary.diagnostic.map(CandidateDiagnostic::Parse),
            ),
            ParseStatus::SyntaxAccepted => {
                if let Some(code) = self.limited {
                    (CandidateStatus::ResourceLimit, Some(code))
                } else if self.unsupported {
                    (
                        CandidateStatus::UnsupportedSyntax,
                        Some(CandidateDiagnostic::UnsupportedSyntax),
                    )
                } else {
                    (CandidateStatus::AcceptedFacts, None)
                }
            }
        };
        if status != CandidateStatus::AcceptedFacts {
            self.facts.truncate(self.start);
            self.used = self.start_used;
        }
        self.files.push(CandidateFile {
            path: &self.sources[self.index].path,
            module: &self.sources[self.index].module,
            sha256: summary.sha256.clone(),
            status,
            diagnostic,
            nodes_visited: summary.nodes_visited,
            max_depth: summary.max_depth,
            traversal_finished: summary.traversal_finished,
        });
        self.index += 1;
    }
}

fn trivia<F: FnMut(Checkpoint) -> bool>(bytes: &[u8], cancel: &Cancellation<F>) -> bool {
    for chunk in bytes.chunks(4096) {
        if cancel.at(Checkpoint::Collector) {
            return false;
        }
        if !chunk.iter().all(|b| matches!(b, b' ' | b'\n' | b'\r')) {
            return false;
        }
    }
    // The module event already checked CRLF pairing across all byte boundaries.
    true
}

fn type_comment<F: FnMut(Checkpoint) -> bool>(bytes: &[u8], cancel: &Cancellation<F>) -> bool {
    let rest = bytes.strip_prefix(b"#").unwrap_or(bytes);
    for (index, &b) in rest.iter().enumerate() {
        if index % 256 == 0 && cancel.at(Checkpoint::Collector) {
            return false;
        }
        if !matches!(b, b' ' | b'\t') {
            return rest[index..].starts_with(b"type:");
        }
    }
    false
}

// Python's ensure_ascii=True spelling, streamed directly into the ID hasher.
fn ascii_json<F: FnMut(Checkpoint) -> bool>(
    text: &str,
    hash: &mut Sha256,
    cancel: &Cancellation<F>,
) {
    hash.update(b"\"");
    for (index, ch) in text.chars().enumerate() {
        if index % 256 == 0 && cancel.at(Checkpoint::FactId) {
            return;
        }
        match ch {
            '"' => hash.update(b"\\\""),
            '\\' => hash.update(b"\\\\"),
            '\u{8}' => hash.update(b"\\b"),
            '\u{c}' => hash.update(b"\\f"),
            '\n' => hash.update(b"\\n"),
            '\r' => hash.update(b"\\r"),
            '\t' => hash.update(b"\\t"),
            ' '..='~' => hash.update([ch as u8]),
            _ => {
                let mut buffer = [0; 2];
                for unit in ch.encode_utf16(&mut buffer) {
                    hash.update(format!("\\u{unit:04x}").as_bytes());
                }
            }
        }
    }
    hash.update(b"\"");
}

fn fact_id<F: FnMut(Checkpoint) -> bool>(fact: &CandidateFact, cancel: &Cancellation<F>) -> String {
    let mut hash = Sha256::new();
    hash.update(b"[");
    ascii_json(&fact.path, &mut hash, cancel);
    hash.update(b", ");
    ascii_json(&fact.kind, &mut hash, cancel);
    hash.update(format!(", {}, {}, ", fact.line, fact.end_line).as_bytes());
    ascii_json(&fact.symbol, &mut hash, cancel);
    hash.update(b", ");
    ascii_json(&fact.value, &mut hash, cancel);
    hash.update(b"]");
    format!("fact_{:x}", hash.finalize())
}

fn signature<F: FnMut(Checkpoint) -> bool>(f: &Function, cancel: &Cancellation<F>) -> String {
    #[derive(Serialize)]
    struct Signature<'a> {
        #[serde(rename = "keywordOnly")]
        keyword_only: &'a [String],
        #[serde(skip_serializing_if = "Option::is_none")]
        kwarg: &'a Option<String>,
        positional: &'a [String],
        #[serde(rename = "positionalOnly")]
        positional_only: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        vararg: &'a Option<String>,
    }
    struct Bounded<'a, F> {
        bytes: Vec<u8>,
        cancel: &'a Cancellation<F>,
    }
    impl<F: FnMut(Checkpoint) -> bool> Write for Bounded<'_, F> {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.cancel.at(Checkpoint::Signature) || bytes.len() > 16384 - self.bytes.len() {
                return Err(io::Error::other("signature bound"));
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    cancel.at(Checkpoint::Signature);
    if f.positional.len() + f.keyword.len() > 128 {
        return "{\"truncated\":true}".into();
    }
    let value = Signature {
        keyword_only: &f.keyword,
        kwarg: &f.kwarg,
        positional: &f.positional,
        positional_only: f.positional_only,
        vararg: &f.vararg,
    };
    let mut writer = Bounded {
        bytes: Vec::with_capacity(16384),
        cancel,
    };
    if serde_json::to_writer(&mut writer, &value).is_err() {
        "{\"truncated\":true}".into()
    } else {
        // Do not transfer 16 KiB of spare capacity per short signature into the
        // output array, whose separate budget measures encoded bytes.
        String::from_utf8(writer.bytes.into_boxed_slice().into_vec())
            .expect("ASCII parameter signature")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(path: &str, module: &str, text: impl AsRef<[u8]>) -> Source {
        Source {
            path: path.into(),
            module: module.into(),
            bytes: text.as_ref().to_vec(),
            hex: "invalid transport".into(),
        }
    }

    fn candidates(text: impl AsRef<[u8]>) -> CandidateReport<'static> {
        let sources = [source("sample.py", "sample", text)];
        let result = super::super::extract_fact_candidates(&sources).unwrap();
        CandidateReport {
            identity: result.identity,
            facts: result.facts,
            files: result
                .files
                .into_iter()
                .map(|f| CandidateFile {
                    path: "sample.py",
                    module: "sample",
                    ..f
                })
                .collect(),
        }
    }

    fn accepted(text: impl AsRef<[u8]>) -> CandidateReport<'static> {
        let result = candidates(text);
        assert_eq!(
            result.files[0].status,
            CandidateStatus::AcceptedFacts,
            "{result:?}"
        );
        result
    }

    #[test]
    fn empty_comments_pass_and_simple_definitions() {
        for text in [
            "",
            " \n\r\n",
            "# comment\n",
            "pass\n",
            "pass # note\n\n# tail\n",
        ] {
            let result = accepted(text);
            assert_eq!(result.facts.len(), 1);
        }
        for text in [
            "def f(): pass\n",
            "async def f(): pass\n",
            "def f():\n    pass\n",
        ] {
            let result = accepted(text);
            assert_eq!(result.facts.len(), 3);
            let value = &result
                .facts
                .iter()
                .find(|f| f.kind == "signature")
                .unwrap()
                .value;
            assert_eq!(value.capacity(), value.len());
            assert!(result.facts.iter().any(|f| f.kind == "signature"
                && f.value == "{\"keywordOnly\":[],\"positional\":[],\"positionalOnly\":0}"));
        }
    }

    #[test]
    fn nested_ranges_ignore_trailing_comments() {
        let result = accepted("def outer(a):\n    # before\n    async def inner(b):\n        pass\n        # inner tail\n    # outer tail\n# module tail\n");
        for fact in result
            .facts
            .iter()
            .filter(|f| matches!(f.kind.as_str(), "function" | "async_function" | "signature"))
        {
            assert_eq!(fact.end_line, 4);
            assert_eq!(fact.line, if fact.symbol == "sample.outer" { 1 } else { 3 });
        }
        assert!(result
            .facts
            .iter()
            .any(|f| f.symbol == "sample.outer.inner.b"));
    }

    #[test]
    fn parameter_kinds_multiline_comments_and_order() {
        let result =
            accepted("def f(\n a, # a\n b, /,\n c, *args,\n z, y, **kwargs,\n):\n    pass\n");
        let signature = result.facts.iter().find(|f| f.kind == "signature").unwrap();
        assert_eq!(signature.value, "{\"keywordOnly\":[\"z\",\"y\"],\"kwarg\":\"kwargs\",\"positional\":[\"a\",\"b\",\"c\"],\"positionalOnly\":2,\"vararg\":\"args\"}");
        assert_eq!(
            result
                .facts
                .iter()
                .filter(|f| f.kind == "parameter")
                .count(),
            7
        );
        for (text, expected) in [
            (
                "def f(*, x,): pass",
                "{\"keywordOnly\":[\"x\"],\"positional\":[],\"positionalOnly\":0}",
            ),
            (
                "def f(**kw): pass",
                "{\"keywordOnly\":[],\"kwarg\":\"kw\",\"positional\":[],\"positionalOnly\":0}",
            ),
            (
                "def f(a, /,): pass",
                "{\"keywordOnly\":[],\"positional\":[\"a\"],\"positionalOnly\":1}",
            ),
        ] {
            let result = accepted(text);
            assert_eq!(
                result
                    .facts
                    .iter()
                    .find(|f| f.kind == "signature")
                    .unwrap()
                    .value,
                expected
            );
        }
    }

    #[test]
    fn bom_crlf_and_unicode_metadata_with_empty_module() {
        let sources = [source(
            "\u{03bb}/\u{1f600}.py",
            "",
            b"\xef\xbb\xbfdef f(x):\r\n    pass # Unicode: \xce\xbb\r\n",
        )];
        let result = extract_fact_candidates(&sources).unwrap();
        assert_eq!(result.files[0].status, CandidateStatus::AcceptedFacts);
        let module = result.facts.iter().find(|f| f.kind == "module").unwrap();
        assert_eq!(
            module.id,
            format!(
                "module_{}",
                super::super::super::hash(sources[0].path.as_bytes())
            )
        );
        assert_eq!(module.symbol, "");
        assert!(result.facts.iter().any(|f| f.symbol == ".f.x"));
        let sources = [source(
            "other.py",
            "\u{03bb}.\u{1f600}\u{7f}\n",
            "def f(): pass",
        )];
        let result = extract_fact_candidates(&sources).unwrap();
        assert_eq!(result.files[0].status, CandidateStatus::AcceptedFacts);
        assert!(result
            .facts
            .iter()
            .any(|f| f.symbol == "\u{03bb}.\u{1f600}\u{7f}\n.f"));
    }

    #[test]
    fn unsupported_forms_rollback_the_entire_file() {
        for text in [
            "def f(x=1): pass",
            "def f(x: int): pass",
            "def f() -> int: pass",
            "@wrap\ndef f(): pass",
            "class C: pass",
            "import sys",
            "from x import y",
            "def f():\n    pass\n    call()",
            "def f():\n    'doc'\n    pass",
            "'doc'",
            "def \u{212a}(): pass",
            "def f(\u{212a}): pass",
            "def f(x, x): pass",
            "def f(*x, **x): pass",
            "def f(/): pass",
            "def f(*): pass",
            "def f(*, **kw): pass",
            "def f(x, /, /): pass",
            "def f(*x, y, /): pass",
            "def f(**kw, x): pass",
            "def f(*x, *y): pass",
            "def f(**x, **y): pass",
            "def f(True): pass",
            "def f[T](): pass",
            "print 'old'",
            "pass\0",
            "def f():\\\n    pass",
            "\u{200b}pass",
            "pass; pass",
            "pass\t",
            "\u{c}pass",
            "pass\n\u{feff}",
            "# type: ignore\npass",
            "def f(): # type: () -> None\n    pass",
            "def f():\n    global x",
            "def f():\n    nonlocal x",
            "def f():\n    return",
            "def f():\n    x=1",
            "def f():\n    if True: pass",
            "pass\rpass",
            "# hidden\rimport x",
        ] {
            let text = format!("def prefix(): pass\n{text}\n");
            let result = candidates(&text);
            assert_ne!(
                result.files[0].status,
                CandidateStatus::AcceptedFacts,
                "{text:?}"
            );
            assert_eq!(result.facts.len(), 1, "{text:?}");
            assert_eq!(result.facts[0].kind, "module");
        }
    }

    #[test]
    fn indentation_and_ignored_extras_are_not_python_validation() {
        for text in [
            " pass",
            "def f():\npass",
            "def f():\n    pass\n  pass",
            "def f():\n    pass\n     pass",
            "def f():\n\tpass",
            "def f():\n    pass\n        pass",
            "def f(): def g(): pass",
            "def f(\n): pass",
            "\0",
            "\u{200b}",
            "\t",
            "\u{c}",
            "def\nf(): pass",
            "async\ndef f(): pass",
            "def f\n(): pass",
            "def f()\n: pass",
        ] {
            let result = candidates(text);
            assert_ne!(
                result.files[0].status,
                CandidateStatus::AcceptedFacts,
                "{text:?}"
            );
            assert_eq!(result.facts.len(), 1);
        }
        accepted("def f():\n  pass\n  def g():\n      pass\n  pass\npass\n");
    }

    #[test]
    fn signature_count_and_byte_boundaries_keep_parameter_facts() {
        for count in [128, 129] {
            let params = (0..count)
                .map(|i| format!("p{i}"))
                .collect::<Vec<_>>()
                .join(",");
            let result = accepted(format!("def f({params}): pass"));
            let signature = &result
                .facts
                .iter()
                .find(|f| f.kind == "signature")
                .unwrap()
                .value;
            assert_eq!(signature == "{\"truncated\":true}", count == 129);
            assert_eq!(
                result
                    .facts
                    .iter()
                    .filter(|f| f.kind == "parameter")
                    .count(),
                count
            );
        }
        for length in [16000, 16400, 70000] {
            let name = "x".repeat(length);
            let result = accepted(format!("def f({name}): pass"));
            let signature = &result
                .facts
                .iter()
                .find(|f| f.kind == "signature")
                .unwrap()
                .value;
            assert_eq!(signature == "{\"truncated\":true}", length > 16384);
            assert_eq!(
                result
                    .facts
                    .iter()
                    .filter(|f| f.kind == "parameter")
                    .count(),
                1
            );
        }
        let overhead = "{\"keywordOnly\":[],\"positional\":[\"\"],\"positionalOnly\":0}".len();
        for bytes in [16384, 16385] {
            let result = accepted(format!("def f({}): pass", "x".repeat(bytes - overhead)));
            let value = &result
                .facts
                .iter()
                .find(|f| f.kind == "signature")
                .unwrap()
                .value;
            assert_eq!(value == "{\"truncated\":true}", bytes > 16384);
            if bytes == 16384 {
                assert_eq!(value.len(), bytes);
            }
        }
    }

    #[test]
    fn exact_ascii_json_preimages_and_seven_field_facts() {
        let cancel = Cancellation {
            expired: Cell::new(false),
            check: RefCell::new(|_| false),
        };
        let mut hash = Sha256::new();
        ascii_json(
            "\"\\\u{8}\u{c}\n\r\t\0\u{1f}\u{7f}\u{03bb}\u{1f600}",
            &mut hash,
            &cancel,
        );
        assert_eq!(
            format!("{:x}", hash.finalize()),
            super::super::super::hash(br#""\"\\\b\f\n\r\t\u0000\u001f\u007f\u03bb\ud83d\ude00""#)
        );
        let result = accepted("def f(a): pass\n");
        let fact = result.facts.iter().find(|f| f.kind == "function").unwrap();
        assert_eq!(
            fact.id,
            format!(
                "fact_{}",
                super::super::super::hash(br#"["sample.py", "function", 1, 1, "sample.f", "f"]"#)
            )
        );
        let signature = "{\"keywordOnly\":[],\"positional\":[\"a\"],\"positionalOnly\":0}";
        let expected = [
            ("function", "sample.f", "f", br#"["sample.py", "function", 1, 1, "sample.f", "f"]"#.as_slice()),
            ("module", "sample", "sample", b"sample.py".as_slice()),
            ("parameter", "sample.f.a", "a", br#"["sample.py", "parameter", 1, 1, "sample.f.a", "a"]"#.as_slice()),
            ("signature", "sample.f", signature, br#"["sample.py", "signature", 1, 1, "sample.f", "{\"keywordOnly\":[],\"positional\":[\"a\"],\"positionalOnly\":0}"]"#.as_slice()),
        ].into_iter().map(|(kind, symbol, value, preimage)| CandidateFact {
            id: format!("{}_{}", if kind == "module" { "module" } else { "fact" }, super::super::super::hash(preimage)),
            kind: kind.into(), path: "sample.py".into(), line: 1, end_line: 1, symbol: symbol.into(), value: value.into(),
        }).collect::<Vec<_>>();
        assert_eq!(result.facts, expected);
        let value = serde_json::to_value(fact).unwrap();
        let keys: BTreeSet<_> = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            BTreeSet::from(["id", "kind", "path", "line", "endLine", "symbol", "value"])
        );
    }

    #[test]
    fn isolation_global_dedup_order_and_determinism() {
        let sources = [
            source("z.py", "z", "def f(): pass\nclass C: pass"),
            source("a.py", "a", "def f(z, y): pass\n"),
        ];
        let result = extract_fact_candidates(&sources).unwrap();
        assert_eq!(result.files[0].status, CandidateStatus::UnsupportedSyntax);
        assert_eq!(result.files[1].status, CandidateStatus::AcceptedFacts);
        assert_eq!(result.facts.iter().filter(|f| f.path == "z.py").count(), 1);
        let mut sorted = result.facts.clone();
        sorted.sort_by_key(|f| {
            (
                f.path.clone(),
                f.line,
                f.end_line,
                f.kind.clone(),
                f.symbol.clone(),
                f.value.clone(),
                f.id.clone(),
            )
        });
        assert_eq!(result.facts, sorted);
        assert_eq!(
            serde_json::to_vec(&result).unwrap(),
            serde_json::to_vec(&extract_fact_candidates(&sources).unwrap()).unwrap()
        );
        assert_ne!(result.identity.policy, POLICY);
        assert_ne!(
            result.identity.sha256,
            parse_batch(&sources).unwrap().identity.sha256
        );
    }

    #[test]
    fn shares_exact_walk_counters_and_single_parse_per_file() {
        let sources = [source("a.py", "a", "def f(a, /, *b, c, **d):\n    pass\n")];
        let mut parses = 0;
        let result = extract(
            &sources,
            Limits::default(),
            FactLimits::default(),
            |phase| {
                parses += usize::from(phase == Checkpoint::PostParse);
                false
            },
        )
        .unwrap();
        assert_eq!(parses, 1);
        let parsed = parse_batch(&sources).unwrap();
        assert_eq!(result.files[0].nodes_visited, parsed.files[0].nodes_visited);
        assert_eq!(result.files[0].max_depth, parsed.files[0].max_depth);
        let result = extract(
            &sources,
            Limits {
                nodes: parsed.files[0].nodes_visited - 1,
                ..Limits::default()
            },
            FactLimits::default(),
            |_| false,
        )
        .unwrap();
        assert_eq!(result.files[0].status, CandidateStatus::ResourceLimit);
        assert_eq!(result.facts.len(), 1);
    }

    #[test]
    fn fact_count_and_encoded_bytes_are_transactional() {
        let sources = [
            source("a.py", "a", "def f(x): pass"),
            source("b.py", "b", "pass"),
        ];
        let result = extract(
            &sources,
            Limits::default(),
            FactLimits {
                count: 3,
                ..FactLimits::default()
            },
            |_| false,
        )
        .unwrap();
        assert_eq!(result.files[0].status, CandidateStatus::ResourceLimit);
        assert_eq!(
            result.files[0].diagnostic,
            Some(CandidateDiagnostic::FactLimit)
        );
        assert_eq!(result.files[1].status, CandidateStatus::AcceptedFacts);
        assert_eq!(result.facts.len(), 2);
        let full = extract_fact_candidates(&sources).unwrap();
        let byte_limit = serde_json::to_vec(&full).unwrap().len() - 1;
        let result = extract(
            &sources,
            Limits::default(),
            FactLimits {
                bytes: byte_limit,
                ..FactLimits::default()
            },
            |_| false,
        )
        .unwrap();
        assert_eq!(result.files[0].status, CandidateStatus::ResourceLimit);
        assert_eq!(
            result.files[0].diagnostic,
            Some(CandidateDiagnostic::OutputLimit)
        );
        assert_eq!(result.facts.len(), 2);
        assert!(serde_json::to_vec(&result).unwrap().len() <= byte_limit);
        assert_eq!(
            extract(
                &sources,
                Limits::default(),
                FactLimits {
                    bytes: 10,
                    ..FactLimits::default()
                },
                |_| false
            )
            .unwrap_err(),
            FrontEndError::ReportLimit
        );
    }

    #[test]
    fn cancellation_at_every_collector_stage_clears_prior_files() {
        let sources = [
            source("a.py", "a", "def f(x): pass"),
            source("b.py", "b", "def g(y): pass"),
        ];
        for stop in [
            Checkpoint::Collector,
            Checkpoint::FactId,
            Checkpoint::Signature,
            Checkpoint::Sort,
            Checkpoint::Output,
            Checkpoint::Final,
        ] {
            let mut hit = false;
            let mut parsed = 0;
            let result = extract(
                &sources,
                Limits::default(),
                FactLimits::default(),
                |phase| {
                    parsed += usize::from(phase == Checkpoint::PostParse);
                    let expire = phase == stop && parsed >= 2;
                    hit |= expire;
                    expire
                },
            )
            .unwrap();
            assert!(hit, "{stop:?}");
            assert_eq!(result.facts.len(), 2, "{stop:?}");
            assert!(
                result
                    .files
                    .iter()
                    .all(|f| f.status == CandidateStatus::ResourceLimit),
                "{stop:?}"
            );
            assert!(result.files.iter().all(
                |f| f.diagnostic == Some(CandidateDiagnostic::Parse(DiagnosticCode::Deadline))
            ));
        }
    }

    #[test]
    fn cancellation_mid_sort_and_during_final_output_keeps_sorted_baselines() {
        let params = (0..40)
            .map(|i| format!("p{i}"))
            .collect::<Vec<_>>()
            .join(",");
        let sources = [
            source("z.py", "z", format!("def f({params}): pass")),
            source("a.py", "a", "def g(): pass"),
        ];
        let count = extract_fact_candidates(&sources).unwrap().facts.len();
        for during_output in [false, true] {
            let mut sorts = 0;
            let result = extract(
                &sources,
                Limits::default(),
                FactLimits::default(),
                |phase| {
                    sorts += usize::from(phase == Checkpoint::Sort);
                    if during_output {
                        phase == Checkpoint::Output && sorts > 0
                    } else {
                        sorts == count + 10
                    }
                },
            )
            .unwrap();
            assert!(sorts >= if during_output { count } else { count + 10 });
            assert_eq!(result.facts.len(), 2);
            assert_eq!(result.facts[0].path, "a.py");
            assert_eq!(result.facts[1].path, "z.py");
            assert!(result
                .files
                .iter()
                .all(|f| f.status == CandidateStatus::ResourceLimit));
        }
    }

    #[test]
    fn expiry_after_last_output_write_rolls_back_every_file() {
        let sources = [
            source("z.py", "z", "def f(x): pass"),
            source("a.py", "a", "def g(y): pass"),
        ];
        let mut parses = 0;
        let mut sorting = false;
        let mut final_writes = 0;
        let mut final_checks = 0;
        let result = extract(
            &sources,
            Limits::default(),
            FactLimits::default(),
            |phase| {
                parses += usize::from(phase == Checkpoint::PostParse);
                sorting |= phase == Checkpoint::Sort;
                final_writes += usize::from(sorting && phase == Checkpoint::Output);
                if phase == Checkpoint::FinalOutput {
                    assert_eq!(parses, 2);
                    assert!(final_writes > 0);
                    final_checks += 1;
                    return true;
                }
                false
            },
        )
        .unwrap();
        assert_eq!(final_checks, 1);
        assert_eq!(result.facts.len(), sources.len());
        assert!(result.facts.iter().all(|fact| fact.kind == "module"));
        assert_eq!(result.facts[0].path, "a.py");
        assert_eq!(result.facts[1].path, "z.py");
        for file in &result.files {
            assert!(file.traversal_finished);
            assert_eq!(file.status, CandidateStatus::ResourceLimit);
            assert_eq!(
                file.diagnostic,
                Some(CandidateDiagnostic::Parse(DiagnosticCode::Deadline))
            );
        }
    }

    #[test]
    fn later_syntax_and_resource_errors_override_unsupported() {
        let sources = [source("a.py", "a", "class C: pass\ndef broken(:\n")];
        let result = extract_fact_candidates(&sources).unwrap();
        assert_eq!(result.files[0].status, CandidateStatus::SyntaxError);
        let result = extract(
            &sources,
            Limits {
                nodes: 5,
                ..Limits::default()
            },
            FactLimits::default(),
            |_| false,
        )
        .unwrap();
        assert_eq!(result.files[0].status, CandidateStatus::ResourceLimit);
        let sources = [source("a.py", "a", b"# coding: latin-1\n\xff")];
        assert_eq!(
            extract_fact_candidates(&sources).unwrap().files[0].status,
            CandidateStatus::UnsupportedEncoding
        );
    }

    #[test]
    fn admission_rejects_before_identity_baseline_or_fact_id_work() {
        let sources = [
            source("a.py", "sample", "pass"),
            source("b.py", "sample", "pass"),
        ];
        for (limits, expected) in [
            (
                Limits {
                    report: 1,
                    ..Limits::default()
                },
                FrontEndError::ReportLimit,
            ),
            (
                Limits {
                    file_bytes: 3,
                    ..Limits::default()
                },
                FrontEndError::FileBytes,
            ),
            (
                Limits {
                    total_bytes: 7,
                    ..Limits::default()
                },
                FrontEndError::TotalBytes,
            ),
        ] {
            let mut entered = false;
            let result = extract(&sources, limits, FactLimits::default(), |phase| {
                entered |= matches!(
                    phase,
                    Checkpoint::Identity
                        | Checkpoint::Baseline
                        | Checkpoint::FactId
                        | Checkpoint::Input
                );
                false
            });
            assert_eq!(result.unwrap_err(), expected);
            assert!(!entered);
        }
        let duplicates = [
            source("a.py", "sample", "pass"),
            source("a.py", "sample", "pass"),
        ];
        let mut entered = false;
        assert_eq!(
            extract(
                &duplicates,
                Limits::default(),
                FactLimits::default(),
                |phase| {
                    entered |= matches!(
                        phase,
                        Checkpoint::Identity | Checkpoint::Baseline | Checkpoint::FactId
                    );
                    false
                }
            )
            .unwrap_err(),
            FrontEndError::DuplicatePath
        );
        assert!(!entered);

        let mut worst = parse_batch(&sources).unwrap();
        for file in &mut worst.files {
            file.status = ParseStatus::UnsupportedEncoding;
            file.diagnostic = Some(DiagnosticCode::EncodingCookieUnsupported);
            file.nodes_visited = usize::MAX;
            file.max_depth = usize::MAX;
            file.traversal_finished = false;
        }
        let limits = Limits {
            report: json_size(&worst, usize::MAX).unwrap(),
            ..Limits::default()
        };
        let cancel = Cancellation {
            expired: Cell::new(false),
            check: RefCell::new(|_| false),
        };
        admit(&sources, limits, &cancel).unwrap();
        for (limits, facts) in [
            (limits, FactLimits::default()),
            (
                Limits::default(),
                FactLimits {
                    bytes: 10,
                    ..FactLimits::default()
                },
            ),
        ] {
            let mut entered = false;
            assert_eq!(
                extract(&sources, limits, facts, |phase| {
                    entered |= matches!(
                        phase,
                        Checkpoint::Identity | Checkpoint::Baseline | Checkpoint::FactId
                    );
                    false
                })
                .unwrap_err(),
                FrontEndError::ReportLimit
            );
            assert!(!entered);
        }
    }

    #[test]
    fn pending_symbols_hit_aggregate_work_guard_before_all_frames_exist() {
        let depth = 30;
        let mut text = format!("def {}():\n", "x".repeat(9000));
        for level in 1..depth {
            text.push_str(&format!("{}def f():\n", " ".repeat(level)));
        }
        text.push_str(&format!("{}pass\n", " ".repeat(depth)));
        let sources = [source("a.py", "sample", &text)];
        let mut frames = 0;
        let mut parses = 0;
        let result = extract(
            &sources,
            Limits::default(),
            FactLimits {
                working: 90_000,
                ..FactLimits::default()
            },
            |phase| {
                frames += usize::from(phase == Checkpoint::FunctionFrame);
                parses += usize::from(phase == Checkpoint::PostParse);
                false
            },
        )
        .unwrap();
        assert_eq!(parses, 1);
        assert!(frames > 1 && frames < depth, "{frames}");
        assert!(result.files[0].traversal_finished);
        assert_eq!(
            result.files[0].diagnostic,
            Some(CandidateDiagnostic::WorkingLimit)
        );
        assert_eq!(result.facts.len(), 1);
        assert_eq!(result.facts[0].kind, "module");
        assert_eq!(
            result.files[0].nodes_visited,
            parse_batch(&sources).unwrap().files[0].nodes_visited
        );
    }

    #[test]
    fn parameter_and_signature_storage_are_charged_before_allocation() {
        for (text, working) in [
            (format!("def f({}): pass", "x".repeat(15000)), 20_000),
            ("def f(): pass".into(), 4096),
        ] {
            let sources = [source("a.py", "sample", text)];
            let mut parsed = false;
            let mut declaration_ids = 0;
            let mut signatures = 0;
            let result = extract(
                &sources,
                Limits::default(),
                FactLimits {
                    working,
                    ..FactLimits::default()
                },
                |phase| {
                    parsed |= phase == Checkpoint::PostParse;
                    declaration_ids += usize::from(parsed && phase == Checkpoint::FactId);
                    signatures += usize::from(phase == Checkpoint::Signature);
                    false
                },
            )
            .unwrap();
            assert_eq!(declaration_ids, 0);
            assert_eq!(signatures, 0);
            assert_eq!(result.files[0].status, CandidateStatus::ResourceLimit);
            assert_eq!(
                result.files[0].diagnostic,
                Some(CandidateDiagnostic::WorkingLimit)
            );
            assert_eq!(result.facts.len(), 1);
        }
    }

    #[test]
    fn file_metadata_retains_raw_capture_digest_even_when_cancelled() {
        let first = [source("a.py", "sample", "# first\n")];
        let second = [source("a.py", "sample", "# changed\n")];
        let a = extract_fact_candidates(&first).unwrap();
        let b = extract_fact_candidates(&second).unwrap();
        assert_eq!(a.facts, b.facts);
        assert_ne!(a.files[0].sha256, b.files[0].sha256);
        assert_eq!(a.files[0].module.as_ptr(), first[0].module.as_ptr());
        assert_eq!(a.files[0].path.as_ptr(), first[0].path.as_ptr());
        for bytes in [
            b"\xef\xbb\xbf# comment\r\n".as_slice(),
            b"# coding: latin-1\n\xff",
            b"\xff",
        ] {
            let sources = [source("a.py", "sample", bytes)];
            for cancelled in [false, true] {
                let result = extract(
                    &sources,
                    Limits::default(),
                    FactLimits::default(),
                    |phase| cancelled && phase == Checkpoint::Preflight,
                )
                .unwrap();
                assert_eq!(result.files[0].sha256, super::super::super::hash(bytes));
                assert_eq!(result.files[0].module, "sample");
                if cancelled {
                    assert_eq!(result.files[0].status, CandidateStatus::ResourceLimit);
                }
            }
        }
    }
}
