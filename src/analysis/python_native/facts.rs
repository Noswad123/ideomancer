//! Deliberately small, transactional candidate policy. Not production evidence.
use super::*;
use std::collections::{HashMap, HashSet};
use tree_sitter::Node;

#[path = "expressions.rs"]
mod expressions;
use expressions::{Expression, Mode};

const FACT_POLICY: &str = "python-native-private-facts-9";
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
    WarningLimit,
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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(in crate::analysis) struct CandidateWarning {
    pub(in crate::analysis) code: &'static str,
    pub(in crate::analysis) path: String,
    pub(in crate::analysis) message: String,
}

#[derive(Debug, Serialize)]
pub(in crate::analysis) struct CandidateReport<'a> {
    pub(in crate::analysis) identity: BatchParserIdentity,
    pub(in crate::analysis) files: Vec<CandidateFile<'a>>,
    pub(in crate::analysis) facts: Vec<CandidateFact>,
    pub(in crate::analysis) diagnostics: Vec<CandidateWarning>,
}

#[derive(Clone, Copy)]
struct FactLimits {
    count: usize,
    warnings: usize,
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
            warnings: MAX_FACTS,
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
        diagnostics: Vec::new(),
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
        include_bytes!("expressions.rs").as_slice(),
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
        diagnostics: Vec::new(),
    };
    let mut collector = Collector {
        sources,
        index: 0,
        facts: &mut result.facts,
        diagnostics: &mut result.diagnostics,
        warning_start: 0,
        warning_text: 0,
        start_warning_text: 0,
        warning_slots: 0,
        scopes: Vec::new(),
        files: Vec::new(),
        frames: Vec::new(),
        frame_slots: 0,
        definitions: Vec::new(),
        definition_frames: 0,
        unsupported: false,
        limited: None,
        terminal_end: 0,
        start: 0,
        used: reserved,
        start_used: reserved,
        limits: fact_limits,
        pending: 0,
        expression: None,
        parameter: None,
        body: None,
        import: None,
        terminal_line: 0,
        delimiter_depth: 0,
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
    interruptible_sort(&mut result.diagnostics, &cancel, |a, b| {
        for (a, b) in [
            (a.path.as_str(), b.path.as_str()),
            (a.code, b.code),
            (a.message.as_str(), b.message.as_str()),
        ] {
            let order = compare_text(a, b, &cancel)?;
            if !order.is_eq() {
                return Some(order);
            }
        }
        Some(std::cmp::Ordering::Equal)
    });
    let output_ok = cancel.expired.get() || measured(&result, fact_limits.bytes, &cancel).is_some();
    if cancel.at(Checkpoint::FinalOutput) || cancel.at(Checkpoint::Final) || !output_ok {
        result.facts.retain(|f| f.kind == "module");
        result.diagnostics.clear();
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
    interruptible_sort(facts, cancel, |a, b| Some(key(a).cmp(&key(b))));
}

fn compare_text<F: FnMut(Checkpoint) -> bool>(
    a: &str,
    b: &str,
    cancel: &Cancellation<F>,
) -> Option<std::cmp::Ordering> {
    for (a, b) in a.as_bytes().chunks(256).zip(b.as_bytes().chunks(256)) {
        if cancel.at(Checkpoint::Sort) {
            return None;
        }
        let order = a.cmp(b);
        if !order.is_eq() {
            return Some(order);
        }
    }
    Some(a.len().cmp(&b.len()))
}

fn interruptible_sort<T, F: FnMut(Checkpoint) -> bool>(
    facts: &mut [T],
    cancel: &Cancellation<F>,
    compare: impl Fn(&T, &T) -> Option<std::cmp::Ordering>,
) {
    fn sift<T, F: FnMut(Checkpoint) -> bool>(
        facts: &mut [T],
        mut root: usize,
        end: usize,
        cancel: &Cancellation<F>,
        compare: &impl Fn(&T, &T) -> Option<std::cmp::Ordering>,
    ) -> bool {
        loop {
            if cancel.at(Checkpoint::Sort) {
                return false;
            }
            let left = root * 2 + 1;
            if left >= end {
                return true;
            }
            let mut child = left;
            if left + 1 < end {
                let Some(order) = compare(&facts[left], &facts[left + 1]) else {
                    return false;
                };
                if order.is_lt() {
                    child += 1;
                }
            }
            let Some(order) = compare(&facts[root], &facts[child]) else {
                return false;
            };
            if !order.is_lt() {
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
        if !sift(facts, root, facts.len(), cancel, &compare) {
            return;
        }
    }
    for end in (1..facts.len()).rev() {
        if cancel.at(Checkpoint::Sort) {
            return;
        }
        facts.swap(0, end);
        if !sift(facts, 0, end, cancel, &compare) {
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
    last_child: &'static str,
}

fn ordinary_compound(kind: &str) -> bool {
    matches!(
        kind,
        "if_statement" | "while_statement" | "for_statement" | "with_statement"
    )
}

fn ordinary_clause(kind: &str) -> bool {
    matches!(kind, "elif_clause" | "else_clause")
}

fn suite_parent(parent: &str, field: Option<&str>) -> bool {
    matches!(
        (parent, field),
        (
            "function_definition"
                | "class_definition"
                | "for_statement"
                | "while_statement"
                | "with_statement"
                | "else_clause",
            Some("body")
        ) | ("if_statement" | "elif_clause", Some("consequence"))
    )
}

fn body_header_value(parent: &str, field: Option<&str>, kind: &str) -> bool {
    matches!(
        (parent, field),
        (
            "if_statement" | "elif_clause" | "while_statement",
            Some("condition")
        ) | ("for_statement", Some("right"))
    ) || (parent == "with_item" && field == Some("value") && kind != "as_pattern")
        || (parent == "as_pattern" && field != Some("alias") && kind != "comment" && kind != "as")
}

fn ordinary_header_token(anchor: &Frame, node: Node<'_>, previous_line: usize) -> bool {
    let kind = node.kind();
    let placed = if kind == ":" {
        node.start_position().row + 1 == previous_line
    } else {
        node.start_position().row == anchor.row
    };
    placed
        && matches!(
            (anchor.kind, kind),
            ("if_statement", "if" | ":")
                | ("elif_clause", "elif" | ":")
                | ("else_clause", "else" | ":")
                | ("while_statement", "while" | ":")
                | ("for_statement", "async" | "for" | "in" | ":")
                | ("with_statement", "async" | "with" | ":")
        )
}

#[derive(Default)]
struct Definition {
    scope_id: usize,
    working: usize,
    name: String,
    symbol: String,
    line: usize,
    end: usize,
    asynchronous: bool,
    class: bool,
    // Classes share ownership/scopes, but never allocate signature state.
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
    positional_default: bool,
}

struct Parameter {
    name: String,
    line: usize,
    end: usize,
    annotation: Option<String>,
    working: usize,
}

#[derive(Clone, Copy)]
enum BodySequence {
    Empty,
    Value,
    Comma,
    Assignment,
}

struct BodyStatement {
    line: usize,
    target: Option<String>,
    source: Option<String>,
    binding: bool,
    has_value: bool,
    annotation: Option<String>,
    // Borrowed source spelling, independent of the clipped binding descriptor.
    field_name: Option<std::ops::Range<usize>>,
    sequence: BodySequence,
    working: usize,
}

struct ImportToken {
    kind: &'static str,
    span: std::ops::Range<usize>,
}

struct ImportStatement {
    start: usize,
    line: usize,
    from: bool,
    tokens: Vec<ImportToken>,
    working: usize,
    parenthesized: bool,
    closed: bool,
}

#[derive(Serialize)]
struct ImportName {
    asname: Option<String>,
    name: String,
}

#[derive(Serialize)]
struct ImportMetadata {
    level: usize,
    module: String,
    names: Vec<ImportName>,
}

fn constructor_terminal(name: &[u8]) -> bool {
    matches!(name, b"FastAPI" | b"APIRouter")
}

fn dynamic_terminal(name: &[u8]) -> bool {
    matches!(
        name,
        b"__import__"
            | b"import_module"
            | b"iter_modules"
            | b"walk_packages"
            | b"entry_points"
            | b"load_entry_point"
            | b"find_spec"
            | b"exec"
            | b"eval"
    )
}

impl ImportStatement {
    // Consume only descriptors from the observer, never tokenize source again.
    fn name<F: FnMut(Checkpoint) -> bool>(
        &self,
        at: &mut usize,
        dotted: bool,
        bytes: &[u8],
        cancel: &Cancellation<F>,
    ) -> Option<String> {
        let start = *at;
        if self.tokens.get(*at)?.kind != "identifier" {
            return None;
        }
        *at += 1;
        while dotted && self.tokens.get(*at).is_some_and(|t| t.kind == ".") {
            if cancel.at(Checkpoint::Collector) {
                return None;
            }
            *at += 1;
            if self.tokens.get(*at)?.kind != "identifier" {
                return None;
            }
            *at += 1;
        }
        let size = self.tokens[start..*at].iter().try_fold(0usize, |n, t| {
            if cancel.at(Checkpoint::Collector) {
                None
            } else {
                n.checked_add(t.span.len())
            }
        })?;
        let mut result = String::with_capacity(size);
        for token in &self.tokens[start..*at] {
            for chunk in bytes[token.span.clone()].chunks(256) {
                if cancel.at(Checkpoint::Collector) {
                    return None;
                }
                result.push_str(std::str::from_utf8(chunk).ok()?);
            }
        }
        Some(result)
    }
}

// Fixed-size hashed keys keep map hashing/comparison bounded. Retain the full
// spelling and fail closed on a collision rather than conflating scope/alias keys.
// Every new entry is charged for four buckets plus allocator/control overhead.
#[derive(Default)]
struct AliasScope {
    parent: Option<usize>,
    children: HashMap<[u8; 32], (String, usize)>,
    aliases: HashMap<[u8; 32], (String, String)>,
}

fn alias_key<F: FnMut(Checkpoint) -> bool>(
    text: &str,
    cancel: &Cancellation<F>,
) -> Option<[u8; 32]> {
    let mut hash = Sha256::new();
    for chunk in text.as_bytes().chunks(256) {
        if cancel.at(Checkpoint::Collector) {
            return None;
        }
        hash.update(chunk);
    }
    Some(hash.finalize().into())
}

fn copy_text<F: FnMut(Checkpoint) -> bool>(text: &str, cancel: &Cancellation<F>) -> Option<String> {
    let mut result = String::with_capacity(text.len());
    for (i, ch) in text.chars().enumerate() {
        if i % 256 == 0 && cancel.at(Checkpoint::Collector) {
            return None;
        }
        result.push(ch);
    }
    Some(result)
}

fn same_text<F: FnMut(Checkpoint) -> bool>(a: &str, b: &str, cancel: &Cancellation<F>) -> bool {
    a.len() == b.len()
        && a.as_bytes()
            .chunks(256)
            .zip(b.as_bytes().chunks(256))
            .all(|(a, b)| !cancel.at(Checkpoint::Collector) && a == b)
}

fn terminal_name<'a, F: FnMut(Checkpoint) -> bool>(
    text: &'a str,
    cancel: &Cancellation<F>,
) -> Option<&'a str> {
    for (i, b) in text.bytes().rev().enumerate() {
        if i % 256 == 0 && cancel.at(Checkpoint::Collector) {
            return None;
        }
        if b == b'.' {
            return Some(&text[text.len() - i..]);
        }
    }
    Some(text)
}

struct Collector<'a, 'b> {
    sources: &'a [Source],
    index: usize,
    facts: &'b mut Vec<CandidateFact>,
    diagnostics: &'b mut Vec<CandidateWarning>,
    warning_start: usize,
    warning_text: usize,
    start_warning_text: usize,
    warning_slots: usize,
    scopes: Vec<AliasScope>,
    files: Vec<CandidateFile<'a>>,
    frames: Vec<Frame>,
    frame_slots: usize,
    definitions: Vec<Definition>,
    definition_frames: usize,
    unsupported: bool,
    limited: Option<CandidateDiagnostic>,
    terminal_end: usize,
    start: usize,
    used: usize,
    start_used: usize,
    limits: FactLimits,
    pending: usize,
    expression: Option<Expression>,
    parameter: Option<Parameter>,
    body: Option<BodyStatement>,
    import: Option<ImportStatement>,
    terminal_line: usize,
    delimiter_depth: usize,
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
    fn scope(&self) -> usize {
        self.definitions.last().map_or(0, |f| f.scope_id)
    }

    fn enter_scope<F: FnMut(Checkpoint) -> bool>(&mut self, name: &str, cancel: &Cancellation<F>) {
        let parent = self
            .definitions
            .iter()
            .rev()
            .nth(1)
            .map_or(0, |f| f.scope_id);
        let Some(key) = alias_key(name, cancel) else {
            return;
        };
        if let Some((stored, id)) = self.scopes[parent].children.get(&key) {
            if !same_text(stored, name, cancel) {
                self.unsupported = true;
                return;
            }
            self.definitions.last_mut().unwrap().scope_id = *id;
            return;
        }
        let charge = 4
            * (std::mem::size_of::<AliasScope>()
                + std::mem::size_of::<([u8; 32], (String, usize))>()
                + 64)
            + name.len();
        if !self.reserve_work(charge, cancel) {
            return;
        }
        let Some(name) = copy_text(name, cancel) else {
            return;
        };
        let id = self.scopes.len();
        self.scopes.push(AliasScope {
            parent: Some(parent),
            ..AliasScope::default()
        });
        self.scopes[parent].children.insert(key, (name, id));
        self.definitions.last_mut().unwrap().scope_id = id;
    }

    fn import_alias<F: FnMut(Checkpoint) -> bool>(
        &mut self,
        local: &str,
        prefix: &str,
        name: &str,
        cancel: &Cancellation<F>,
    ) {
        let Some(key) = alias_key(local, cancel) else {
            return;
        };
        let scope = self.scope();
        let old_size = if let Some((stored, target)) = self.scopes[scope].aliases.get(&key) {
            if !same_text(stored, local, cancel) {
                self.unsupported = true;
                return;
            }
            Some(target.len())
        } else {
            None
        };
        let target_size = prefix.len() + usize::from(!prefix.is_empty()) + name.len();
        let entry = if old_size.is_none() {
            local.len() + 4 * (std::mem::size_of::<([u8; 32], (String, String))>() + 64)
        } else {
            0
        };
        if !self.reserve_work(target_size + entry, cancel) {
            return;
        }
        let target = if prefix.is_empty() {
            copy_text(name, cancel)
        } else {
            join_symbol(prefix, name, target_size, cancel)
        };
        let Some(target) = target else {
            return;
        };
        if let Some(old_size) = old_size {
            self.scopes[scope].aliases.get_mut(&key).unwrap().1 = target;
            self.pending -= old_size;
        } else {
            let Some(local) = copy_text(local, cancel) else {
                return;
            };
            self.scopes[scope].aliases.insert(key, (local, target));
        }
    }

    fn warn_call<F: FnMut(Checkpoint) -> bool>(
        &mut self,
        callee: &str,
        line: usize,
        cancel: &Cancellation<F>,
    ) {
        let head_end = callee.find('.').unwrap_or(callee.len());
        let head = &callee[..head_end];
        let Some(key) = alias_key(head, cancel) else {
            return;
        };
        let mut scope = self.scope();
        let mut target = None;
        loop {
            if cancel.at(Checkpoint::Collector) {
                return;
            }
            if let Some((stored, value)) = self.scopes[scope].aliases.get(&key) {
                if !same_text(stored, head, cancel) {
                    self.unsupported = true;
                    return;
                }
                target = Some(value.as_str());
                break;
            }
            let Some(parent) = self.scopes[scope].parent else {
                break;
            };
            scope = parent;
        }
        let prefix = target.unwrap_or(head);
        let tail = &callee[head_end..];
        // The tail is already bounded by expression(func); only a bare head can
        // acquire a different terminal. Never reclip the full imported target.
        let Some(terminal) = terminal_name(if tail.is_empty() { prefix } else { tail }, cancel)
        else {
            return;
        };
        if !dynamic_terminal(terminal.as_bytes()) {
            return;
        }
        let expanded_len = prefix.len() + tail.len();
        if self.diagnostics.len() >= self.limits.warnings {
            self.limited = Some(CandidateDiagnostic::WarningLimit);
            return;
        }
        let path_len = self.sources[self.index].path.len();
        let message_len = "line : ; runtime targets are not inferred".len()
            + line.max(1).ilog10() as usize
            + 1
            + expanded_len;
        // Bound the message before expansion. Descriptors are ASCII; paths may
        // require JSON escaping, so preview the exact record before retaining it.
        if message_len > self.limits.bytes.saturating_sub(self.used) {
            self.limited = Some(CandidateDiagnostic::OutputLimit);
            return;
        }
        let slot_charge = 4 * std::mem::size_of::<CandidateWarning>();
        // Logical-slot high water covers initial/geometric Vec capacity and
        // survives rollback. Text charges roll back independently with warnings.
        let grow = usize::from(self.diagnostics.len() == self.warning_slots) * slot_charge;
        let text_charge = message_len + path_len;
        let scope_target = target.is_some();
        // End the borrowed arena reference before reserving; retrieve by fixed key.
        if !self.reserve_work(grow + text_charge, cancel) {
            return;
        }
        let prefix = if scope_target {
            self.scopes[scope].aliases.get(&key).unwrap().1.as_str()
        } else {
            head
        };
        let mut message = String::with_capacity(message_len);
        use std::fmt::Write as _;
        write!(&mut message, "line {line}: ").unwrap();
        for part in [prefix, tail] {
            for chunk in part.as_bytes().chunks(256) {
                if cancel.at(Checkpoint::Collector) {
                    return;
                }
                message.push_str(std::str::from_utf8(chunk).unwrap());
            }
        }
        message.push_str("; runtime targets are not inferred");
        debug_assert_eq!(message.len(), message_len);
        #[derive(Serialize)]
        struct Preview<'a> {
            code: &'static str,
            path: &'a str,
            message: &'a str,
        }
        let code = "dynamic_discovery_unresolved";
        let preview = Preview {
            code,
            path: &self.sources[self.index].path,
            message: &message,
        };
        let Some(size) = measured(
            &preview,
            self.limits
                .bytes
                .saturating_sub(self.used)
                .saturating_sub(1),
            cancel,
        ) else {
            self.limited = Some(CandidateDiagnostic::OutputLimit);
            return;
        };
        let Some(path) = copy_text(&self.sources[self.index].path, cancel) else {
            return;
        };
        self.used += size + 1;
        self.warning_text += text_charge;
        if grow != 0 {
            self.warning_slots += 1;
        }
        self.diagnostics.push(CandidateWarning {
            code,
            path,
            message,
        });
    }

    fn finish_import<F: FnMut(Checkpoint) -> bool>(&mut self, cancel: &Cancellation<F>) {
        let import = self.import.take().unwrap();
        let bytes = self.sources[self.index]
            .bytes
            .strip_prefix(b"\xef\xbb\xbf")
            .unwrap_or(&self.sources[self.index].bytes);
        // All canonical names, aliases and module text together are shorter
        // than the source span. Charge that storage before any string allocation.
        let text_work = self.terminal_end - import.start;
        let mut text_size = 0;
        let mut names = 1;
        let mut aliases = 0;
        for token in &import.tokens {
            if cancel.at(Checkpoint::Collector) {
                return;
            }
            if matches!(token.kind, "identifier" | ".") {
                text_size += token.span.len();
            }
            names += usize::from(token.kind == ",");
            aliases += usize::from(token.kind == "as");
        }
        // A trailing comma inside the single admitted pair adds no alias.
        if import.parenthesized
            && import.tokens.len() >= 2
            && import.tokens[import.tokens.len() - 2].kind == ","
        {
            names -= 1;
        }
        let mut level = 0usize;
        if import.from {
            for token in import.tokens.iter().skip(1) {
                if cancel.at(Checkpoint::Collector) {
                    return;
                }
                if !matches!(token.kind, "." | "...") {
                    break;
                }
                level += token.span.len();
                if token.kind == "." {
                    text_size -= 1;
                }
            }
        }
        // Exact compact metadata size for valid ASCII descriptors, including
        // explicit nulls. Invalid descriptor sequences still fail closed below.
        let metadata_size = "{\"level\":0,\"module\":\"\",\"names\":[]}".len()
            + level.max(1).ilog10() as usize
            + text_size
            + names * "{\"asname\":null,\"name\":\"\"}".len()
            + names
            - 1;
        let Some(metadata_size) = metadata_size.checked_sub(2 * aliases) else {
            self.unsupported = true;
            return;
        };
        if metadata_size > self.limits.bytes.saturating_sub(self.used) {
            self.limited = Some(CandidateDiagnostic::OutputLimit);
            return;
        }
        if !self.reserve_work(text_work, cancel) {
            return;
        }
        let mut metadata = ImportMetadata {
            level: 0,
            module: String::new(),
            names: Vec::new(),
        };
        let mut at = 1;
        if import.from {
            while import
                .tokens
                .get(at)
                .is_some_and(|t| matches!(t.kind, "." | "..."))
            {
                if cancel.at(Checkpoint::Collector) {
                    return;
                }
                metadata.level += import.tokens[at].span.len();
                at += 1;
            }
            if import
                .tokens
                .get(at)
                .is_some_and(|t| t.kind == "identifier")
            {
                let Some(module) = import.name(&mut at, true, bytes, cancel) else {
                    self.unsupported = true;
                    return;
                };
                metadata.module = module;
            } else if metadata.level == 0 {
                self.unsupported = true;
                return;
            }
            if import.tokens.get(at).is_none_or(|t| t.kind != "import") {
                self.unsupported = true;
                return;
            }
            at += 1;
        }
        let names_end = if import.parenthesized {
            if !import.closed
                || !import.from
                || import.tokens.get(at).is_none_or(|t| t.kind != "(")
                || import.tokens.last().is_none_or(|t| t.kind != ")")
            {
                self.unsupported = true;
                return;
            }
            at += 1;
            import.tokens.len() - 1
        } else {
            import.tokens.len()
        };
        let mut names_work = 0;
        loop {
            if cancel.at(Checkpoint::Collector) {
                return;
            }
            let name_start = at;
            let Some(name) = import.name(&mut at, !import.from, bytes, cancel) else {
                self.unsupported = true;
                return;
            };
            let name_end = at - 1;
            let asname = if import.tokens.get(at).is_some_and(|t| t.kind == "as") {
                at += 1;
                let Some(alias) = import.name(&mut at, false, bytes, cancel) else {
                    self.unsupported = true;
                    return;
                };
                Some(alias)
            } else {
                None
            };
            // Constructor metadata remains outside this slice, even for an
            // unused/overwritten alias. Dynamic imports now populate the table.
            let terminal = &import.tokens[if !import.from && asname.is_none() {
                name_start
            } else {
                name_end
            }];
            if metadata.level == 0 && constructor_terminal(&bytes[terminal.span.clone()]) {
                self.unsupported = true;
                return;
            }
            let charge = 4 * std::mem::size_of::<ImportName>();
            if !self.reserve_work(charge, cancel) {
                return;
            }
            names_work += charge;
            metadata.names.push(ImportName { asname, name });
            if at == names_end {
                break;
            }
            if import.tokens[at].kind != "," {
                self.unsupported = true;
                return;
            }
            at += 1;
            if import.parenthesized && at == names_end {
                break;
            }
        }
        if metadata.level == 0 {
            for item in &metadata.names {
                if cancel.at(Checkpoint::Collector) {
                    return;
                }
                let local = item
                    .asname
                    .as_deref()
                    .unwrap_or_else(|| item.name.split('.').next().unwrap());
                let target = if !import.from && item.asname.is_none() {
                    local
                } else {
                    &item.name
                };
                self.import_alias(local, &metadata.module, target, cancel);
                if self.unsupported || self.limited.is_some() {
                    return;
                }
            }
        }
        let Some(size) = measured(
            &metadata,
            self.limits.bytes.saturating_sub(self.used),
            cancel,
        ) else {
            self.limited = Some(CandidateDiagnostic::OutputLimit);
            return;
        };
        debug_assert_eq!(size, metadata_size);
        let owner = self
            .definitions
            .last()
            .map_or(&self.sources[self.index].module, |f| &f.symbol);
        // Metadata buffer, fact ownership, serialization/ID and dedup copies.
        let temporary = 4 * size + 4 * owner.len() + 4 * self.sources[self.index].path.len() + 1024;
        if !self.reserve_work(temporary, cancel) {
            return;
        }
        struct Writer<'a, F> {
            bytes: Vec<u8>,
            cancel: &'a Cancellation<F>,
        }
        impl<F: FnMut(Checkpoint) -> bool> Write for Writer<'_, F> {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                for chunk in bytes.chunks(256) {
                    if self.cancel.at(Checkpoint::Collector) {
                        return Err(io::Error::other("deadline"));
                    }
                    self.bytes.extend_from_slice(chunk);
                }
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut writer = Writer {
            bytes: Vec::with_capacity(size),
            cancel,
        };
        if serde_json::to_writer(&mut writer, &metadata).is_err() {
            return;
        }
        let value = String::from_utf8(writer.bytes).unwrap();
        #[derive(Serialize)]
        struct BorrowedFact<'a> {
            id: &'a str,
            kind: &'a str,
            path: &'a str,
            line: usize,
            #[serde(rename = "endLine")]
            end_line: usize,
            symbol: &'a str,
            value: &'a str,
        }
        let kind = if import.from { "from_import" } else { "import" };
        let preview = BorrowedFact {
            id: concat!(
                "fact_",
                "0000000000000000000000000000000000000000000000000000000000000000"
            ),
            kind,
            path: &self.sources[self.index].path,
            line: import.line,
            end_line: self.terminal_line,
            symbol: self
                .definitions
                .last()
                .map_or(&self.sources[self.index].module, |f| &f.symbol),
            value: &value,
        };
        if measured(
            &preview,
            self.limits
                .bytes
                .saturating_sub(self.used)
                .saturating_sub(1),
            cancel,
        )
        .is_none()
        {
            self.limited = Some(CandidateDiagnostic::OutputLimit);
            return;
        }
        let owner = self
            .definitions
            .last()
            .map_or(&self.sources[self.index].module, |f| &f.symbol)
            .clone();
        self.add(kind, import.line, self.terminal_line, owner, value, cancel);
        self.pending -= import.working + text_work + names_work + temporary;
    }

    fn finish_expression<F: FnMut(Checkpoint) -> bool>(&mut self, cancel: &Cancellation<F>) {
        let expression = self.expression.take().unwrap();
        if matches!(expression.mode, Mode::Base | Mode::AssignmentAnnotation) {
            let owner_len = self.definitions.last().map_or(0, |f| f.symbol.len());
            let temporary = Expression::RENDER_WORK + owner_len;
            if !self.reserve_work(temporary, cancel) {
                return;
            }
            let bytes = self.sources[self.index]
                .bytes
                .strip_prefix(b"\xef\xbb\xbf")
                .unwrap_or(&self.sources[self.index].bytes);
            let value = expression
                .render(bytes, cancel)
                .into_boxed_str()
                .into_string();
            if expression.mode == Mode::Base {
                let (line, end) = expression.span();
                let symbol = self.definitions.last().unwrap().symbol.clone();
                self.add("base", line, end, symbol, value, cancel);
            } else if self.body.as_ref().unwrap().field_name.is_some() {
                // Keep only a bounded redacted shape until the statement closes.
                if !self.reserve_work(512, cancel) {
                    return;
                }
                let body = self.body.as_mut().unwrap();
                body.annotation = Some(value);
                body.working += 512;
            }
            self.pending -= expression.working + temporary;
            return;
        }
        if matches!(expression.mode, Mode::BodyValue | Mode::AssignmentTarget) {
            self.finish_body_expression(expression, cancel);
            return;
        }
        let function = self.definitions.last().unwrap();
        let temporary = Expression::RENDER_WORK
            + 2 * (function.symbol.len() + self.parameter.as_ref().map_or(0, |p| p.name.len() + 1));
        if !self.reserve_work(temporary, cancel) {
            return;
        }
        let bytes = self.sources[self.index]
            .bytes
            .strip_prefix(b"\xef\xbb\xbf")
            .unwrap_or(&self.sources[self.index].bytes);
        // Compact only the final shape, not the bounded recursive scratch buffers.
        let value = expression
            .render(bytes, cancel)
            .into_boxed_str()
            .into_string();
        let (line, end) = expression.span();
        match expression.mode {
            Mode::Annotation => {
                let p = self.parameter.as_mut().unwrap();
                p.annotation = Some(value);
                p.end = expression.terminal_end_line;
                p.working += temporary;
            }
            Mode::Return => {
                let f = self.definitions.last_mut().unwrap();
                f.header_end = expression.terminal_end_line - 1;
                let symbol = f.symbol.clone();
                self.add("return_annotation", line, end, symbol, value, cancel);
                self.pending -= temporary;
            }
            Mode::Default => {
                let symbol = join_symbol(
                    &self.definitions.last().unwrap().symbol,
                    &self.parameter.as_ref().unwrap().name,
                    self.limits.bytes.saturating_sub(self.used),
                    cancel,
                );
                if let Some(symbol) = symbol {
                    self.add("parameter_default", line, end, symbol, value, cancel);
                } else {
                    self.limited = Some(CandidateDiagnostic::OutputLimit);
                }
                self.pending -= temporary;
            }
            Mode::Base | Mode::AssignmentAnnotation | Mode::BodyValue | Mode::AssignmentTarget => {
                unreachable!()
            }
        }
        self.pending -= expression.working;
    }

    fn finish_body_expression<F: FnMut(Checkpoint) -> bool>(
        &mut self,
        expression: Expression,
        cancel: &Cancellation<F>,
    ) {
        let owner_len = self
            .definitions
            .last()
            .map_or(self.sources[self.index].module.len(), |f| f.symbol.len());
        let retained = 512;
        let temporary = Expression::RENDER_WORK + owner_len;
        if !self.reserve_work(temporary + retained, cancel) {
            return;
        }
        let bytes = self.sources[self.index]
            .bytes
            .strip_prefix(b"\xef\xbb\xbf")
            .unwrap_or(&self.sources[self.index].bytes);
        if expression.mode == Mode::AssignmentTarget {
            let target = expression
                .render(bytes, cancel)
                .into_boxed_str()
                .into_string();
            let body = self.body.as_mut().unwrap();
            body.target = Some(target);
            body.working += retained;
        } else {
            for index in expression.calls(cancel) {
                if cancel.at(Checkpoint::Collector) || self.limited.is_some() {
                    break;
                }
                let bytes = self.sources[self.index]
                    .bytes
                    .strip_prefix(b"\xef\xbb\xbf")
                    .unwrap_or(&self.sources[self.index].bytes);
                let value = expression.render_call(index, bytes, cancel);
                let callee = expression.render_callee(index, bytes, cancel);
                let (line, end) = expression.call_span(index);
                self.warn_call(&callee, line, cancel);
                let owner = self
                    .definitions
                    .last()
                    .map_or(&self.sources[self.index].module, |f| &f.symbol)
                    .clone();
                self.add("call", line, end, owner, value, cancel);
            }
            let bytes = self.sources[self.index]
                .bytes
                .strip_prefix(b"\xef\xbb\xbf")
                .unwrap_or(&self.sources[self.index].bytes);
            if let Some(body) = self.body.as_mut().filter(|b| b.target.is_some()) {
                body.binding = expression.binding();
                if body.binding {
                    body.source = Some(
                        expression
                            .render(bytes, cancel)
                            .into_boxed_str()
                            .into_string(),
                    );
                }
                body.working += retained;
            } else {
                self.pending -= retained;
            }
        }
        self.pending -= expression.working + temporary;
    }

    fn finish_body_statement<F: FnMut(Checkpoint) -> bool>(&mut self, cancel: &Cancellation<F>) {
        let body = self.body.take().unwrap();
        if let (Some(name), Some(annotation)) = (body.field_name.as_ref(), body.annotation.as_ref())
        {
            let size = self.definitions.last().unwrap().symbol.len() + name.len() + 1;
            if !self.reserve_work(size + 512, cancel) {
                return;
            }
            let bytes = self.sources[self.index]
                .bytes
                .strip_prefix(b"\xef\xbb\xbf")
                .unwrap_or(&self.sources[self.index].bytes);
            let Some(symbol) = join_symbol(
                &self.definitions.last().unwrap().symbol,
                std::str::from_utf8(&bytes[name.clone()]).unwrap(),
                self.limits.bytes.saturating_sub(self.used),
                cancel,
            ) else {
                self.limited = Some(CandidateDiagnostic::OutputLimit);
                return;
            };
            self.add(
                "attribute_annotation",
                body.line,
                self.terminal_line,
                symbol,
                annotation.clone(),
                cancel,
            );
            self.pending -= size + 512;
        }
        if let Some(target) = body.target.as_deref().filter(|_| body.has_value) {
            #[derive(Serialize)]
            struct Binding<'a> {
                #[serde(skip_serializing_if = "Option::is_none")]
                source: Option<&'a str>,
                target: &'a str,
            }
            let metadata = Binding {
                source: body.source.as_deref(),
                target,
            };
            let Some(size) = measured(
                &metadata,
                self.limits.bytes.saturating_sub(self.used),
                cancel,
            ) else {
                self.limited = Some(CandidateDiagnostic::OutputLimit);
                return;
            };
            let owner = self
                .definitions
                .last()
                .map_or(&self.sources[self.index].module, |f| &f.symbol);
            let temporary = 2 * size + owner.len();
            if !self.reserve_work(temporary, cancel) {
                return;
            }
            let mut value = Vec::with_capacity(size);
            if serde_json::to_writer(&mut value, &metadata).is_err() {
                unreachable!("string metadata");
            }
            let value = String::from_utf8(value).unwrap();
            let owner = self
                .definitions
                .last()
                .map_or(&self.sources[self.index].module, |f| &f.symbol)
                .clone();
            self.add(
                if body.binding {
                    "binding"
                } else {
                    "binding_invalidation"
                },
                body.line,
                self.terminal_line,
                owner,
                value,
                cancel,
            );
            self.pending -= temporary;
        }
        self.pending -= body.working;
    }

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
            if !matches!(
                node.kind(),
                "pass_statement"
                    | "expression_statement"
                    | "return_statement"
                    | "break_statement"
                    | "continue_statement"
            ) || suite.last_end.is_some()
            {
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
        self.warning_start = self.diagnostics.len();
        self.start_warning_text = self.warning_text;
        self.start_used = self.used;
        self.unsupported = false;
        self.limited = None;
        self.terminal_end = 0;
        self.frames = Vec::new();
        self.frame_slots = 0;
        self.definitions = Vec::new();
        self.definition_frames = 0;
        self.pending =
            self.warning_text + self.warning_slots * 4 * std::mem::size_of::<CandidateWarning>();
        self.scopes = Vec::new();
        self.expression = None;
        self.parameter = None;
        self.body = None;
        self.import = None;
        self.terminal_line = 0;
        self.delimiter_depth = 0;
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
        // CPython's delimiter limit spans enclosing headers and arguments too.
        // Only punctuation events count, never string/comment payload bytes.
        if !node.is_named() {
            match kind {
                "(" | "[" | "{" => {
                    self.delimiter_depth += 1;
                    if self.delimiter_depth > 200 {
                        self.unsupported = true;
                        return;
                    }
                }
                ")" | "]" | "}" => {
                    let Some(depth) = self.delimiter_depth.checked_sub(1) else {
                        self.unsupported = true;
                        return;
                    };
                    self.delimiter_depth = depth;
                }
                _ => {}
            }
        }
        let parent = self.frames.last().map(|f| f.kind).unwrap_or("");
        if parent == "expression_statement" && kind != "comment" {
            // Bare tuple expression statements have separate direct CST roots,
            // unlike expression_list RHS/return values. Validate the sequence
            // without suppressing root validation or permitting line joining.
            let body = self.body.as_mut().unwrap();
            let same_line = node.start_position().row + 1 == self.terminal_line;
            body.sequence = match (body.sequence, kind, node.is_named()) {
                (BodySequence::Empty, "assignment", true) => BodySequence::Assignment,
                (BodySequence::Value, ",", false) if same_line => BodySequence::Comma,
                (BodySequence::Empty, _, true) if node.start_position().row + 1 == body.line => {
                    BodySequence::Value
                }
                (BodySequence::Comma, _, true) if same_line && kind != "assignment" => {
                    BodySequence::Value
                }
                _ => {
                    self.unsupported = true;
                    return;
                }
            };
        }
        let header_value = body_header_value(parent, field, kind);
        let mode = match (parent, field) {
            _ if header_value && node.is_named() => Some(Mode::BodyValue),
            ("argument_list", _)
                if self.expression.is_none()
                    && self.body.is_none()
                    && self.definitions.last().is_some_and(|f| f.class)
                    && node.is_named()
                    && kind != "comment" =>
            {
                Some(Mode::Base)
            }
            ("typed_parameter" | "typed_default_parameter", Some("type")) => Some(Mode::Annotation),
            ("default_parameter" | "typed_default_parameter", Some("value")) => Some(Mode::Default),
            ("function_definition", Some("return_type")) => Some(Mode::Return),
            ("assignment", Some("left")) => Some(Mode::AssignmentTarget),
            ("assignment", Some("right")) => Some(Mode::BodyValue),
            ("assignment", Some("type")) => Some(Mode::AssignmentAnnotation),
            ("expression_statement", _)
                if node.is_named() && kind != "assignment" && kind != "comment" =>
            {
                Some(Mode::BodyValue)
            }
            ("return_statement", _) if node.is_named() && kind != "comment" => {
                Some(Mode::BodyValue)
            }
            _ => None,
        };
        if let Some(mode) = mode {
            if self.expression.is_some() {
                self.unsupported = true;
                return;
            }
            if mode == Mode::Return
                && node.start_position().row != self.definitions.last().unwrap().header_end
            {
                self.unsupported = true;
                return;
            }
            // A root expression does not see the gap before its own start.
            // Enclosing expression delimiters, not CST acceptance, authorize
            // implicit continuation after this first physical header token.
            if header_value
                && (self.body.is_some() || node.start_position().row + 1 != self.terminal_line)
            {
                self.unsupported = true;
                return;
            }
            if ((mode == Mode::BodyValue && matches!(parent, "assignment" | "return_statement"))
                || mode == Mode::AssignmentAnnotation)
                && node.start_position().row + 1 != self.terminal_line
            {
                self.unsupported = true;
                return;
            }
            self.expression = Some(Expression::new(mode, node.start_byte()));
        }
        let previous_terminal_end = self.terminal_end;
        let previous_terminal_line = self.terminal_line;
        if node.child_count() == 0 && kind != "module" {
            let gap = &bytes[self.terminal_end..node.start_byte()];
            if !trivia(gap, cancel)
                || self.expression.as_mut().is_some_and(|e| {
                    !e.terminal(
                        node,
                        &bytes[self.terminal_end.max(e.start)..node.start_byte()],
                        cancel,
                    )
                })
            {
                self.unsupported = true;
            }
            self.terminal_end = node.end_byte();
            if kind != "comment" {
                self.terminal_line = node.end_position().row + 1;
            }
        }
        if self.import.is_some() {
            if node.child_count() == 0 {
                let import = self.import.as_ref().unwrap();
                // Comments are observed leaves but never semantic descriptors.
                // Use the previous leaf, not the previous semantic token, so
                // a comment's payload cannot be mistaken for inter-token trivia.
                let previous = previous_terminal_end.max(import.start);
                let in_list = import.parenthesized && !import.closed;
                let word = |kind| matches!(kind, "identifier" | "import" | "from" | "as");
                // Only the single enclosing from-import pair grants implicit
                // continuation; prefix tokens and '(' stay on the first line.
                if import.closed
                    || (!in_list
                        && (!bytes[previous..node.start_byte()].chunks(256).all(|chunk| {
                            !cancel.at(Checkpoint::Collector) && chunk.iter().all(|&b| b == b' ')
                        }) || node.start_position().row + 1 != import.line
                            || node.end_position().row + 1 != import.line))
                    || (previous == node.start_byte()
                        && word(kind)
                        && import.tokens.last().is_some_and(|t| word(t.kind)))
                    || !matches!(
                        kind,
                        "identifier"
                            | "import"
                            | "from"
                            | "as"
                            | "."
                            | "..."
                            | ","
                            | "("
                            | ")"
                            | "comment"
                    )
                    || (kind == "identifier"
                        && !identifier(
                            std::str::from_utf8(&bytes[node.start_byte()..node.end_byte()])
                                .unwrap_or(""),
                            cancel,
                        ))
                {
                    self.unsupported = true;
                    return;
                }
                match kind {
                    "comment" => {
                        if !in_list
                            || type_comment(&bytes[node.start_byte()..node.end_byte()], cancel)
                        {
                            self.unsupported = true;
                        }
                    }
                    "(" => {
                        if import.parenthesized
                            || !import.from
                            || import.tokens.last().is_none_or(|t| t.kind != "import")
                        {
                            self.unsupported = true;
                            return;
                        }
                        self.import.as_mut().unwrap().parenthesized = true;
                    }
                    ")" => {
                        if !in_list {
                            self.unsupported = true;
                            return;
                        }
                        self.import.as_mut().unwrap().closed = true;
                    }
                    _ => {}
                }
                if kind != "comment" {
                    let charge = 4 * std::mem::size_of::<ImportToken>();
                    if !self.reserve_work(charge, cancel) {
                        return;
                    }
                    let import = self.import.as_mut().unwrap();
                    import.working += charge;
                    import.tokens.push(ImportToken {
                        kind,
                        span: node.start_byte()..node.end_byte(),
                    });
                }
            } else if !matches!(
                kind,
                "dotted_name" | "aliased_import" | "relative_import" | "import_prefix"
            ) {
                self.unsupported = true;
                return;
            }
        } else if self.expression.is_some() {
            if !self.reserve_work(Expression::ENTRY_WORK, cancel) {
                return;
            }
            if !self.expression.as_mut().unwrap().enter(node, bytes, cancel) {
                self.unsupported = true;
                return;
            }
        } else {
            match kind {
                "module" if parent.is_empty() => {
                    if !self.reserve_work(4 * std::mem::size_of::<AliasScope>(), cancel) {
                        return;
                    }
                    self.scopes.push(AliasScope::default());
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
                "function_definition" | "class_definition" => {
                    // Known header exclusions precede owned declaration state.
                    if kind == "class_definition"
                        && (node.child_by_field_name("type_parameters").is_some()
                            || node
                                .child_by_field_name("superclasses")
                                .is_some_and(|bases| {
                                    bases.start_position().row != node.start_position().row
                                        || bases.end_position().row != node.start_position().row
                                        || bases.named_child(0).is_some_and(|base| {
                                            !matches!(
                                                base.kind(),
                                                "identifier"
                                                    | "attribute"
                                                    | "parenthesized_expression"
                                            )
                                        })
                                }))
                    {
                        self.unsupported = true;
                        return;
                    }
                    self.statement(node, bytes);
                    // Keep the vector's high-water capacity charged even after
                    // nested definitions unwind. Drop that capacity between files.
                    if self.definitions.len() == self.definition_frames {
                        if !self.reserve_work(4 * std::mem::size_of::<Definition>(), cancel) {
                            return;
                        }
                        self.definition_frames += 1;
                    }
                    cancel.at(Checkpoint::FunctionFrame);
                    self.definitions.push(Definition {
                        line: node.start_position().row + 1,
                        class: kind == "class_definition",
                        ..Definition::default()
                    });
                }
                _ if ordinary_compound(kind) => self.statement(node, bytes),
                _ if ordinary_clause(kind) => {
                    let anchor = self.frames.last().unwrap();
                    let point = node.start_position();
                    let line_start = node.start_byte().saturating_sub(point.column);
                    if field != Some("alternative")
                        || !matches!(
                            (parent, kind),
                            ("if_statement", "elif_clause" | "else_clause")
                                | ("for_statement" | "while_statement", "else_clause")
                        )
                        || anchor.last_end.is_none_or(|end| point.row <= end)
                        || anchor.last_child == "else_clause"
                        || point.column != anchor.column
                        || !bytes[line_start..node.start_byte()]
                            .iter()
                            .all(|&b| b == b' ')
                    {
                        self.unsupported = true;
                    }
                }
                "block" if suite_parent(parent, field) => {}
                "with_clause" if parent == "with_statement" => {
                    if node.start_position().row != self.frames.last().unwrap().row {
                        self.unsupported = true;
                    }
                }
                "with_item" if parent == "with_clause" => {
                    if node.start_position().row + 1 != previous_terminal_line {
                        self.unsupported = true;
                    }
                }
                "as_pattern" if parent == "with_item" && field == Some("value") => {}
                "as_pattern_target" if parent == "as_pattern" && field == Some("alias") => {
                    // The grammar wraps even a direct name in this alias node.
                    // Admit that exact wrapper, not grouped, starred, attribute
                    // or destructuring expressions. The child validates ASCII.
                    if node.child_count() != 1
                        || node
                            .child(0)
                            .is_none_or(|child| child.kind() != "identifier")
                        || node.start_position().row + 1 != previous_terminal_line
                    {
                        self.unsupported = true;
                    }
                }
                "as" if parent == "as_pattern" && !node.is_named() => {
                    if node.start_position().row + 1 != previous_terminal_line {
                        self.unsupported = true;
                    }
                }
                "," if parent == "with_clause" && !node.is_named() => {
                    if node.start_position().row + 1 != previous_terminal_line {
                        self.unsupported = true;
                    }
                }
                _ if (ordinary_compound(parent) || ordinary_clause(parent)) && !node.is_named() => {
                    if !ordinary_header_token(
                        self.frames.last().unwrap(),
                        node,
                        previous_terminal_line,
                    ) {
                        self.unsupported = true;
                    }
                }
                "argument_list" if parent == "class_definition" => {
                    let line = self.definitions.last().unwrap().line;
                    // The pinned grammar admits `(,)` as an argument list.
                    // A class list with no named base must be exactly `()`;
                    // named-child and child counts are cached CST metadata.
                    if (node.named_child_count() == 0 && node.child_count() != 2)
                        || node.start_position().row + 1 != line
                        || node.end_position().row + 1 != line
                    {
                        self.unsupported = true;
                    }
                }
                "class" | ":" if parent == "class_definition" && !node.is_named() => {
                    if node.start_position().row + 1 != self.definitions.last().unwrap().line {
                        self.unsupported = true;
                    }
                }
                "(" | ")" | "," if parent == "argument_list" && !node.is_named() => {}
                "pass_statement" => self.statement(node, bytes),
                "break_statement" | "continue_statement" => {
                    self.statement(node, bytes);
                    // A loop's else suite is outside that loop's jump context;
                    // an outer loop can still supply one. Definitions cut it off.
                    let in_loop = self
                        .frames
                        .iter()
                        .enumerate()
                        .rev()
                        .take_while(|(_, f)| {
                            !matches!(f.kind, "function_definition" | "class_definition")
                        })
                        .any(|(i, f)| {
                            matches!(f.kind, "for_statement" | "while_statement")
                                && self.frames.get(i + 1).is_some_and(|f| f.kind == "block")
                        });
                    if !in_loop {
                        self.unsupported = true;
                    }
                }
                "import_statement" | "import_from_statement" => {
                    self.statement(node, bytes);
                    if self.facts.len() >= self.limits.count {
                        self.limited = Some(CandidateDiagnostic::FactLimit);
                        return;
                    }
                    self.import = Some(ImportStatement {
                        start: node.start_byte(),
                        line: node.start_position().row + 1,
                        from: kind == "import_from_statement",
                        tokens: Vec::new(),
                        working: 0,
                        parenthesized: false,
                        closed: false,
                    });
                }
                "expression_statement" | "return_statement" => {
                    self.statement(node, bytes);
                    if self.body.is_some()
                        || (kind == "return_statement"
                            && self.definitions.last().is_none_or(|f| f.class))
                    {
                        self.unsupported = true;
                        return;
                    }
                    let working = 2 * std::mem::size_of::<BodyStatement>();
                    if !self.reserve_work(working, cancel) {
                        return;
                    }
                    self.body = Some(BodyStatement {
                        line: node.start_position().row + 1,
                        target: None,
                        source: None,
                        binding: false,
                        has_value: false,
                        annotation: None,
                        field_name: None,
                        sequence: BodySequence::Empty,
                        working,
                    });
                }
                "assignment" if parent == "expression_statement" => {
                    let body = self.body.as_mut().unwrap();
                    body.has_value = node.child_by_field_name("right").is_some();
                    if node.child_by_field_name("type").is_some()
                        && self.definitions.last().is_some_and(|f| f.class)
                    {
                        body.field_name = node
                            .child_by_field_name("left")
                            .filter(|left| left.kind() == "identifier")
                            .map(|left| left.start_byte()..left.end_byte());
                    }
                }
                "," if parent == "expression_statement" && !node.is_named() => {}
                ":" | "=" if parent == "assignment" && !node.is_named() => {
                    if node.start_position().row + 1 != previous_terminal_line {
                        self.unsupported = true;
                    }
                }
                "return" if parent == "return_statement" && !node.is_named() => {}
                "parameters" if parent == "function_definition" && field == Some("parameters") => {
                    if node.start_position().row + 1 != self.definitions.last().unwrap().line {
                        self.unsupported = true;
                    }
                }
                "typed_parameter" | "typed_default_parameter" | "default_parameter"
                    if parent == "parameters" =>
                {
                    let working = 2 * std::mem::size_of::<Parameter>();
                    if !self.reserve_work(working, cancel) {
                        return;
                    }
                    self.parameter = Some(Parameter {
                        name: String::new(),
                        line: 0,
                        end: 0,
                        annotation: None,
                        working,
                    });
                }
                "identifier" => {
                    let name = std::str::from_utf8(&bytes[node.start_byte()..node.end_byte()])
                        .unwrap_or("");
                    if !identifier(name, cancel) {
                        self.unsupported = true;
                        return;
                    }
                    if (parent == "for_statement" && field == Some("left"))
                        || parent == "as_pattern_target"
                    {
                        if node.start_position().row != self.frames.last().unwrap().row {
                            self.unsupported = true;
                        }
                    } else if matches!(parent, "function_definition" | "class_definition")
                        && field == Some("name")
                    {
                        if node.start_position().row + 1 != self.definitions.last().unwrap().line {
                            self.unsupported = true;
                        }
                        self.enter_scope(name, cancel);
                        if self.unsupported || self.limited.is_some() || cancel.expired.get() {
                            return;
                        }
                        let prefix = if self.definitions.len() > 1 {
                            &self.definitions[self.definitions.len() - 2].symbol
                        } else {
                            &self.sources[self.index].module
                        };
                        // Charge retained names and both symbol copies (including
                        // the eventual unwind clone) before allocating either.
                        let working = name.len() + 2 * (prefix.len() + name.len() + 1);
                        if !self.reserve_work(working, cancel) {
                            return;
                        }
                        self.definitions.last_mut().unwrap().working += working;
                        let prefix = if self.definitions.len() > 1 {
                            &self.definitions[self.definitions.len() - 2].symbol
                        } else {
                            &self.sources[self.index].module
                        };
                        if let Some(symbol) = join_symbol(
                            prefix,
                            name,
                            self.limits.bytes.saturating_sub(self.used),
                            cancel,
                        ) {
                            let f = self.definitions.last_mut().unwrap();
                            f.name = name.into();
                            f.symbol = symbol;
                        } else {
                            self.limited = Some(CandidateDiagnostic::OutputLimit);
                        }
                    } else if matches!(
                        parent,
                        "parameters"
                            | "list_splat_pattern"
                            | "dictionary_splat_pattern"
                            | "typed_parameter"
                            | "typed_default_parameter"
                            | "default_parameter"
                    ) {
                        // Signature/set names and the deferred parameter name,
                        // plus HashSet buckets and geometric Vec growth.
                        let working = 3 * name.len() + 256;
                        if !self.reserve_work(working, cancel) {
                            return;
                        }
                        let Some(f) = self.definitions.last_mut() else {
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
                            _ => {
                                let default = matches!(
                                    parent,
                                    "default_parameter" | "typed_default_parameter"
                                );
                                if f.positional_default && !default {
                                    self.unsupported = true;
                                }
                                f.positional_default |= default;
                                f.positional.push(name.into());
                            }
                        }
                        if let Some(parameter) = self.parameter.as_mut() {
                            parameter.name = name.into();
                            parameter.line = node.start_position().row + 1;
                            parameter.end = node.end_position().row + 1;
                        } else {
                            let temporary = f.symbol.len() + 2 * name.len() + 1;
                            if !self.reserve_work(temporary, cancel) {
                                return;
                            }
                            let Some(symbol) = join_symbol(
                                &self.definitions.last().unwrap().symbol,
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
                        }
                    } else {
                        self.unsupported = true;
                    }
                }
                "positional_separator" if parent == "parameters" => {
                    let f = self.definitions.last_mut().unwrap();
                    if f.slash || f.star || f.kwarg.is_some() || f.positional.is_empty() {
                        self.unsupported = true;
                    }
                    f.slash = true;
                    f.positional_only = f.positional.len();
                }
                "keyword_separator" | "list_splat_pattern"
                    if matches!(parent, "parameters" | "typed_parameter") =>
                {
                    let f = self.definitions.last_mut().unwrap();
                    if f.star || f.kwarg.is_some() {
                        self.unsupported = true;
                    }
                    f.star = true;
                    f.bare_star = kind == "keyword_separator";
                }
                "dictionary_splat_pattern"
                    if matches!(parent, "parameters" | "typed_parameter") =>
                {
                    if self.definitions.last().unwrap().kwarg.is_some() {
                        self.unsupported = true;
                    }
                }
                "async" | "def" if parent == "function_definition" && !node.is_named() => {
                    let f = self.definitions.last_mut().unwrap();
                    if node.start_position().row + 1 != f.line {
                        self.unsupported = true;
                    }
                    if kind == "async" {
                        f.asynchronous = true;
                    }
                }
                ":" if parent == "function_definition" && !node.is_named() => {
                    if node.start_position().row != self.definitions.last().unwrap().header_end {
                        self.unsupported = true;
                    }
                }
                "(" | ")" | "," if parent == "parameters" && !node.is_named() => {}
                ":" | "="
                    if matches!(
                        parent,
                        "typed_parameter" | "typed_default_parameter" | "default_parameter"
                    ) && !node.is_named() => {}
                "->" if parent == "function_definition" && !node.is_named() => {
                    if node.start_position().row != self.definitions.last().unwrap().header_end {
                        self.unsupported = true;
                    }
                }
                "/" if parent == "positional_separator" && !node.is_named() => {}
                "*" if matches!(parent, "keyword_separator" | "list_splat_pattern")
                    && !node.is_named() => {}
                "**" if parent == "dictionary_splat_pattern" && !node.is_named() => {}
                "pass" if parent == "pass_statement" && !node.is_named() => {}
                "break" if parent == "break_statement" && !node.is_named() => {}
                "continue" if parent == "continue_statement" && !node.is_named() => {}
                _ => self.unsupported = true,
            }
        }
        let point = node.start_position();
        let (row, column) = if kind == "block" {
            let parent = self.frames.last().unwrap();
            // Compound indentation belongs to its keyword; inline suite rows
            // belong to the colon, including a grouped multiline header.
            let row = if ordinary_compound(parent.kind) || ordinary_clause(parent.kind) {
                self.terminal_line - 1
            } else {
                parent.row
            };
            (row, parent.column)
        } else {
            (point.row, point.column)
        };
        // Charge the cursor-frame high water before Vec growth, retaining that
        // charge when suites unwind. The shared walk bounds its depth.
        if self.frames.len() == self.frame_slots {
            if !self.reserve_work(4 * std::mem::size_of::<Frame>(), cancel) {
                return;
            }
            self.frame_slots += 1;
        }
        self.frames.push(Frame {
            kind,
            row,
            column,
            indent: None,
            last_end: None,
            last_child: "",
        });
    }

    fn exit<F: FnMut(Checkpoint) -> bool>(&mut self, node: Node<'_>, cancel: &Cancellation<F>) {
        if cancel.at(Checkpoint::Collector) || self.unsupported || self.limited.is_some() {
            return;
        }
        let frame = self.frames.pop().unwrap();
        if node.kind() != "comment" {
            if let Some(parent) = self.frames.last_mut() {
                parent.last_child = node.kind();
            }
        }
        if self.import.is_some() {
            if matches!(node.kind(), "import_statement" | "import_from_statement") {
                let end = self.terminal_line;
                self.finish_import(cancel);
                if let Some(f) = self.definitions.last_mut() {
                    f.end = end;
                }
                if let Some(suite) = self.frames.last_mut() {
                    suite.last_end = Some(end - 1);
                }
            }
            return;
        }
        if self.expression.is_some() {
            let scratch = if node.kind() == "call" {
                Expression::RENDER_WORK
            } else {
                0
            };
            if scratch != 0 && !self.reserve_work(scratch, cancel) {
                return;
            }
            let bytes = self.sources[self.index]
                .bytes
                .strip_prefix(b"\xef\xbb\xbf")
                .unwrap_or(&self.sources[self.index].bytes);
            let result = self.expression.as_mut().unwrap().exit(bytes, cancel);
            self.pending -= scratch;
            match result {
                Ok(false) => return,
                Err(()) => {
                    self.unsupported = true;
                    return;
                }
                Ok(true) => {}
            }
            self.finish_expression(cancel);
            return;
        }
        match node.kind() {
            "with_clause" => {
                // Parenthesized with-item lists are deferred in G1. Ordinary
                // parenthesized context expressions still use BodyValue. The
                // grammar also accepts an invalid unparenthesized trailing comma.
                if frame.last_child != "with_item" {
                    self.unsupported = true;
                }
            }
            kind if kind == "block" || ordinary_compound(kind) || ordinary_clause(kind) => {
                let Some(end) = frame.last_end else {
                    self.unsupported = true;
                    return;
                };
                if let Some(parent) = self.frames.last_mut() {
                    parent.last_end = Some(end);
                }
                if let Some(owner) = self.definitions.last_mut() {
                    owner.end = end + 1;
                }
            }
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
                let f = self.definitions.last_mut().unwrap();
                f.working += SIGNATURE_WORK;
                if f.bare_star && f.keyword.is_empty() {
                    self.unsupported = true;
                }
                f.signature = signature(f, cancel);
                f.header_end = node.end_position().row;
            }
            "typed_parameter" | "typed_default_parameter" | "default_parameter" => {
                let parameter = self.parameter.take().unwrap();
                let temporary =
                    self.definitions.last().unwrap().symbol.len() + parameter.name.len() + 1;
                if !self.reserve_work(temporary, cancel) {
                    return;
                }
                let symbol = join_symbol(
                    &self.definitions.last().unwrap().symbol,
                    &parameter.name,
                    self.limits.bytes.saturating_sub(self.used),
                    cancel,
                );
                let Some(symbol) = symbol else {
                    self.limited = Some(CandidateDiagnostic::OutputLimit);
                    return;
                };
                if let Some(annotation) = parameter.annotation {
                    // The second symbol copy is covered by the expression's
                    // retained temporary reservation until this parameter exits.
                    self.add(
                        "parameter_annotation",
                        parameter.line,
                        parameter.end,
                        symbol.clone(),
                        annotation,
                        cancel,
                    );
                }
                self.add(
                    "parameter",
                    parameter.line,
                    parameter.end,
                    symbol,
                    parameter.name,
                    cancel,
                );
                self.pending -= parameter.working + temporary;
            }
            "pass_statement"
            | "expression_statement"
            | "return_statement"
            | "break_statement"
            | "continue_statement" => {
                let end = self.terminal_line;
                if node.kind() == "expression_statement"
                    && matches!(self.body.as_ref().unwrap().sequence, BodySequence::Empty)
                {
                    self.unsupported = true;
                    return;
                }
                if matches!(node.kind(), "expression_statement" | "return_statement") {
                    self.finish_body_statement(cancel);
                }
                if let Some(f) = self.definitions.last_mut() {
                    f.end = end;
                }
                if let Some(suite) = self.frames.last_mut() {
                    suite.last_end = Some(end - 1);
                }
            }
            "function_definition" | "class_definition" => {
                let f = self.definitions.pop().unwrap();
                if f.end == 0 || f.name.is_empty() {
                    self.unsupported = true;
                    return;
                }
                self.add(
                    if f.class {
                        "class"
                    } else if f.asynchronous {
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
                if !f.class {
                    self.add("signature", f.line, f.end, f.symbol, f.signature, cancel);
                }
                if let Some(parent) = self.definitions.last_mut() {
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
            self.diagnostics.truncate(self.warning_start);
            self.warning_text = self.start_warning_text;
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

fn signature<F: FnMut(Checkpoint) -> bool>(f: &Definition, cancel: &Cancellation<F>) -> String {
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
            diagnostics: result.diagnostics,
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
    fn policy9_suites_if_elif_else_and_while_else_headers() {
        accepted("def f():\n    if True: pass\n");
        let result = accepted(concat!(
            "if first(inner()):\n    a()\n",
            "elif second():\n    while test():\n        body()\n    else:\n        done()\n",
            "else:\n    final_call()\n",
            "if (\n    grouped() # neutral\n): inline()\n",
        ));
        let calls: Vec<_> = result
            .facts
            .iter()
            .filter(|f| f.kind == "call")
            .map(|f| (f.line, f.end_line, f.symbol.as_str(), f.value.as_str()))
            .collect();
        assert_eq!(
            calls,
            [
                (1, 1, "sample", "first(inner())"),
                (1, 1, "sample", "inner()"),
                (2, 2, "sample", "a()"),
                (3, 3, "sample", "second()"),
                (4, 4, "sample", "test()"),
                (5, 5, "sample", "body()"),
                (7, 7, "sample", "done()"),
                (9, 9, "sample", "final_call()"),
                (11, 11, "sample", "grouped()"),
                (12, 12, "sample", "inline()"),
            ]
        );
        assert!(result
            .facts
            .iter()
            .all(|f| matches!(f.kind.as_str(), "module" | "call")));
        for header in [
            "if {eval(): inner()}",
            "while a and eval()",
            "for value in [eval()]",
            "with context({key: eval()}) as value",
        ] {
            let result = accepted(format!("{header}: pass\n"));
            assert_eq!(result.diagnostics.len(), 1, "{header}");
            assert!(result
                .facts
                .iter()
                .any(|f| f.kind == "call" && f.value == "eval()"));
        }
    }

    #[test]
    fn policy9_suites_for_async_for_else_and_neutral_binders() {
        let result = accepted(concat!(
            "for item in items(inner()):\n    use(item)\nelse: finished()\n",
            "async def consume():\n",
            "    async for value in (\n        stream(seed())\n    ):\n",
            "        consume_one(value)\n    else:\n        exhausted()\n",
            "for item in [a(), b()]: pass\n",
        ));
        assert_eq!(result.facts.iter().filter(|f| f.kind == "call").count(), 10);
        assert!(!result.facts.iter().any(|f| f.kind.starts_with("binding")));
        for name in [
            "stream(seed())",
            "seed()",
            "consume_one(value)",
            "exhausted()",
        ] {
            assert!(result
                .facts
                .iter()
                .any(|f| f.kind == "call" && f.symbol == "sample.consume" && f.value == name));
        }
        assert!(result
            .facts
            .iter()
            .any(|f| f.kind == "async_function" && f.line == 4 && f.end_line == 10));
        accepted("for item in a(), b(): pass\n");
    }

    #[test]
    fn policy9_suites_with_async_with_and_multiple_contexts() {
        let result = accepted(concat!(
            "with open_context(inner()) as handle, other() as second, plain():\n",
            "    use(handle, second)\n",
            "async def work():\n",
            "    async with context(\n        nested()\n    ) as resource:\n",
            "        finish(resource)\n",
            "with (grouped()) as value: pass\n",
        ));
        assert_eq!(result.facts.iter().filter(|f| f.kind == "call").count(), 9);
        assert!(!result.facts.iter().any(|f| f.kind.starts_with("binding")));
        assert!(result.facts.iter().any(|f| f.kind == "call"
            && f.line == 4
            && f.end_line == 6
            && f.symbol == "sample.work"
            && f.value == "context(nested())"));
        assert!(result.facts.iter().any(|f| f.kind == "call"
            && f.line == 8
            && f.symbol == "sample"
            && f.value == "grouped()"));
        accepted("with context((grouped())): pass\n");
    }

    #[test]
    fn policy9_suites_nested_definitions_annotation_owners_and_exact_ends() {
        let result = accepted(concat!(
            "def outer():\n",
            "    if enabled():\n",
            "        class Plain:\n",
            "            if class_test():\n",
            "                field: T = make()\n",
            "            else:\n",
            "                def method(self):\n",
            "                    for value in items():\n",
            "                        with context() as resource:\n",
            "                            if ready(): return result()\n",
            "                    else:\n",
            "                        return fallback()\n",
            "                    # trailing method comment\n",
            "            # trailing class comment\n",
            "    else:\n",
            "        while retry():\n",
            "            step()\n",
            "        else:\n",
            "            return done()\n",
            "    # trailing outer comment\n",
            "after()\n",
        ));
        for (kind, symbol, line, end) in [
            ("function", "sample.outer", 1, 19),
            ("class", "sample.outer.Plain", 3, 12),
            ("function", "sample.outer.Plain.method", 7, 12),
            ("attribute_annotation", "sample.outer.Plain.field", 5, 5),
        ] {
            assert!(
                result.facts.iter().any(|f| f.kind == kind
                    && f.symbol == symbol
                    && f.line == line
                    && f.end_line == end),
                "{kind}: {result:?}"
            );
        }
        for (value, owner) in [
            ("enabled()", "sample.outer"),
            ("class_test()", "sample.outer.Plain"),
            ("make()", "sample.outer.Plain"),
            ("items()", "sample.outer.Plain.method"),
            ("context()", "sample.outer.Plain.method"),
            ("ready()", "sample.outer.Plain.method"),
            ("result()", "sample.outer.Plain.method"),
            ("fallback()", "sample.outer.Plain.method"),
            ("retry()", "sample.outer"),
            ("done()", "sample.outer"),
            ("after()", "sample"),
        ] {
            assert!(result
                .facts
                .iter()
                .any(|f| f.kind == "call" && f.value == value && f.symbol == owner));
        }
        for text in [
            "if True: return x",
            "class C:\n    if True: return x",
            "def f():\n    class C:\n        if True: return x",
        ] {
            assert_ne!(
                candidates(text).files[0].status,
                CandidateStatus::AcceptedFacts
            );
        }
    }

    #[test]
    fn policy9_suites_alias_order_and_body_state_without_binder_writes() {
        let result = accepted(concat!(
            "from importlib import import_module as load\n",
            "if load():\n    from pkgutil import walk_packages as load\n",
            "elif load():\n    x = value()\n",
            "else:\n    from pkgutil import iter_modules as load\n",
            "while load():\n    for load in load():\n        load()\n",
            "else:\n    with load() as load:\n        load()\n",
            "x = final_value()\nload()\n",
        ));
        let mut expected: Vec<_> = [
            (2, "importlib.import_module"),
            (4, "pkgutil.walk_packages"),
            (8, "pkgutil.iter_modules"),
            (9, "pkgutil.iter_modules"),
            (10, "pkgutil.iter_modules"),
            (12, "pkgutil.iter_modules"),
            (13, "pkgutil.iter_modules"),
            (15, "pkgutil.iter_modules"),
        ]
        .into_iter()
        .map(|(line, name)| warning(line, name))
        .collect();
        expected.sort();
        assert_eq!(
            warning_messages(&result),
            expected.iter().map(String::as_str).collect::<Vec<_>>()
        );
        let bindings: Vec<_> = result
            .facts
            .iter()
            .filter(|f| f.kind.starts_with("binding"))
            .map(|f| (f.line, f.value.as_str()))
            .collect();
        assert_eq!(
            bindings,
            [
                (5, "{\"source\":\"value()\",\"target\":\"x\"}"),
                (14, "{\"source\":\"final_value()\",\"target\":\"x\"}"),
            ]
        );
    }

    #[test]
    fn policy9_suites_break_continue_stop_at_definition_boundaries() {
        for text in [
            "while test():\n    if ready(): break\n    else: continue\nelse: done()",
            "for item in items():\n    with context(): continue",
            "while outer():\n    while inner(): break\n    else: continue",
            "while outer():\n    def f():\n        while inner(): break\n    break",
            "while outer():\n    class C:\n        while inner(): continue\n    break",
        ] {
            let result = accepted(text);
            assert!(result
                .facts
                .iter()
                .all(|f| !matches!(f.kind.as_str(), "break" | "continue")));
        }
        for text in [
            "break",
            "continue",
            "if True: break",
            "while test(): pass\nelse: break",
            "for item in items(): pass\nelse: continue",
            "while test():\n    def f(): break",
            "while test():\n    class C:\n        if True: continue",
            "while test():\n    class C:\n        def f(): break",
        ] {
            let result = candidates(format!("eval()\n{text}\n"));
            assert_ne!(
                result.files[0].status,
                CandidateStatus::AcceptedFacts,
                "{text}"
            );
            assert_eq!(result.facts.len(), 1);
            assert!(result.diagnostics.is_empty());
        }
    }

    #[test]
    fn policy9_suites_physical_headers_clause_attachments_and_binder_boundary() {
        for newline in ["\n", "\r\n"] {
            for text in [
                "if\nx: pass",
                "while\nx: pass",
                "if x\n: pass",
                "if (x)\n: pass",
                "if x # neutral\n: pass",
                "if x:\npass",
                "if x: pass\n    else: pass",
                "if x: pass\nelse: pass\nelif y: pass",
                "if x: pass\nelse: pass\nelse: pass",
                "else: pass",
                "elif x: pass",
                "while x: pass\nelif y: pass",
                "with x: pass\nelse: pass",
                "if x: pass; pass",
                "if\tx: pass",
                "if x:\n\tpass",
                concat!("if x:", "\\", "\n", "    pass"),
                "if x: # type: ignore\n    pass",
                concat!("if x", "\\", "\n", ": pass"),
                "if (\n\tx\n): pass",
                "for\nx in xs: pass",
                "for x\nin xs: pass",
                "for x in\nxs: pass",
                "for x in (xs)\n: pass",
                "async\nfor x in xs: pass",
                "for (x) in xs: pass",
                "for x,y in xs: pass",
                "for [x] in xs: pass",
                "for *x in xs: pass",
                "for obj.x in xs: pass",
                "for xs[0] in xs: pass",
                "for \u{212a} in xs: pass",
                "for True in xs: pass",
                "with\nx: pass",
                "async\nwith x: pass",
                "with x\nas y: pass",
                "with x as\ny: pass",
                "with x,\ny: pass",
                "with x, : pass",
                "with x\n: pass",
                "with x as (y): pass",
                "with x as (y,z): pass",
                "with x as [y]: pass",
                "with x as obj.y: pass",
                "with x as y[0]: pass",
                "with x as *y: pass",
                "with x as \u{212a}: pass",
                // Parenthesized with-item lists, even valid ones, are deferred.
                "with (x as y, z as w): pass",
                "with (\n x as y,\n z as w,\n): pass",
                "try: pass\nfinally: pass",
                "match x:\n    case _: pass",
                "if x: assert y",
                "if x: raise E",
                "if x: global y",
                "if FastAPI(): pass",
                "while obj.include_router(x): pass",
                "for x in APIRouter(): pass",
                "with pkg.FastAPI(): pass",
                "if x:\n    from fastapi import FastAPI",
            ] {
                let text = format!("eval()\n{text}\n").replace('\n', newline);
                let result = candidates(&text);
                assert_ne!(
                    result.files[0].status,
                    CandidateStatus::AcceptedFacts,
                    "{text:?}"
                );
                assert_eq!(result.facts.len(), 1, "{text:?}");
                assert!(result.diagnostics.is_empty(), "{text:?}");
            }
            accepted(
                "if (\n test()\n):\n    pass\n\n# neutral\nelif other(): pass\nelse: pass\n"
                    .replace('\n', newline),
            );
        }
    }

    #[test]
    fn policy9_suites_resources_cancellation_counters_isolation_and_replay() {
        let sources = [source(
            "a.py",
            "a",
            concat!(
                "if eval():\n    for item in items():\n        with context() as value:\n",
                "            x = value()\n    else: done()\nelse: fallback()\n",
            ),
        )];
        let mut parses = 0;
        let phases = [Checkpoint::Collector, Checkpoint::Working];
        let mut checkpoints = [0; 2];
        let full = extract(
            &sources,
            Limits::default(),
            FactLimits::default(),
            |phase| {
                parses += usize::from(phase == Checkpoint::PostParse);
                for (i, point) in phases.iter().enumerate() {
                    checkpoints[i] += usize::from(phase == *point);
                }
                false
            },
        )
        .unwrap();
        assert_eq!(full.files[0].status, CandidateStatus::AcceptedFacts);
        assert_eq!(parses, 1);
        let parsed = parse_batch(&sources).unwrap();
        assert_eq!(full.files[0].nodes_visited, parsed.files[0].nodes_visited);
        assert_eq!(full.files[0].max_depth, parsed.files[0].max_depth);
        assert!(full.files[0].traversal_finished);
        for (i, point) in phases.iter().enumerate() {
            for stop in [1, checkpoints[i] / 2, checkpoints[i]] {
                let mut seen = 0;
                let result = extract(
                    &sources,
                    Limits::default(),
                    FactLimits::default(),
                    |phase| {
                        seen += usize::from(phase == *point);
                        seen == stop
                    },
                )
                .unwrap();
                assert_eq!(result.files[0].status, CandidateStatus::ResourceLimit);
                assert_eq!(result.facts.len(), 1);
                assert!(result.diagnostics.is_empty());
            }
        }
        for limits in [
            FactLimits {
                count: full.facts.len() - 1,
                ..FactLimits::default()
            },
            FactLimits {
                warnings: 0,
                ..FactLimits::default()
            },
            FactLimits {
                working: 1024,
                ..FactLimits::default()
            },
            FactLimits {
                bytes: serde_json::to_vec(&full).unwrap().len() - 1,
                ..FactLimits::default()
            },
        ] {
            let result = extract(&sources, Limits::default(), limits, |_| false).unwrap();
            assert_eq!(result.files[0].status, CandidateStatus::ResourceLimit);
            assert_eq!(result.facts.len(), 1);
            assert!(result.diagnostics.is_empty());
        }
        for limits in [
            Limits {
                nodes: parsed.files[0].nodes_visited - 1,
                ..Limits::default()
            },
            Limits {
                depth: parsed.files[0].max_depth - 1,
                ..Limits::default()
            },
        ] {
            let result = extract(&sources, limits, FactLimits::default(), |_| false).unwrap();
            assert_eq!(result.files[0].status, CandidateStatus::ResourceLimit);
            assert_eq!(result.facts.len(), 1);
            assert!(result.diagnostics.is_empty());
        }
        let batch = [
            source(
                "bad.py",
                "bad",
                "if eval():\n    from importlib import import_module as leak\nelse: assert x\n",
            ),
            source(
                "good.py",
                "good",
                "if leak():\n    with eval() as value: use(value)\n",
            ),
        ];
        let result = extract_fact_candidates(&batch).unwrap();
        assert_eq!(result.files[0].status, CandidateStatus::UnsupportedSyntax);
        assert_eq!(result.files[1].status, CandidateStatus::AcceptedFacts);
        assert_eq!(
            result.facts.iter().filter(|f| f.path == "bad.py").count(),
            1
        );
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].path, "good.py");
        assert_eq!(result.diagnostics[0].message, warning(2, "eval"));
        let replay = extract_fact_candidates(&batch).unwrap();
        assert_eq!(
            serde_json::to_vec(&result).unwrap(),
            serde_json::to_vec(&replay).unwrap()
        );
    }

    #[test]
    fn policy9_conditional_tests_require_grouping_and_rollback() {
        let invalid = "a if b if c else d else e";
        for statement in [
            format!("x = {invalid}\n"),
            format!("sink({invalid})\n"),
            format!("x = {{'key': {invalid}}}\n"),
            format!("def f(x={invalid}): pass\n"),
            format!("def f(x={{'key': {invalid}}}): pass\n"),
        ] {
            let sources = [
                source(
                    "bad.py",
                    "bad",
                    format!("eval()\nkept=helper()\n{statement}after=helper()\neval()\n"),
                ),
                source("good.py", "good", "eval()\nx=helper()\n"),
            ];
            let result = extract_fact_candidates(&sources).unwrap();
            assert_eq!(
                result.files[0].status,
                CandidateStatus::UnsupportedSyntax,
                "{statement}: {result:?}"
            );
            assert_eq!(result.files[1].status, CandidateStatus::AcceptedFacts);
            assert_eq!(
                result.facts.iter().filter(|f| f.path == "bad.py").count(),
                1
            );
            assert!(result
                .facts
                .iter()
                .any(|f| f.path == "good.py" && f.kind == "binding"));
            assert_eq!(result.diagnostics.len(), 1);
            assert_eq!(result.diagnostics[0].path, "good.py");
        }
        for expression in [
            "a if (b if c else d) else e",
            "a if ((b if c else d)) else e",
            "a if b else c if d else e",
        ] {
            let result = accepted(format!(
                "x={expression}\nsink({expression})\nx={{'key': {expression}}}\ndef f(x={expression}, y={{'key': {expression}}}): pass\n"
            ));
            assert!(result
                .facts
                .iter()
                .any(|f| f.kind == "call" && f.value == "sink(<IfExp>)"));
            assert!(result
                .facts
                .iter()
                .any(|f| f.kind == "parameter_default" && f.value == "<IfExp>"));
            assert!(result
                .facts
                .iter()
                .any(|f| f.kind == "parameter_default" && f.value == "<Dict>"));
        }
        let result = accepted("sink(a if (eval() if helper() else c) else done())");
        for call in ["sink(<IfExp>)", "eval()", "helper()", "done()"] {
            assert!(result
                .facts
                .iter()
                .any(|f| f.kind == "call" && f.value == call));
        }
        assert_eq!(result.diagnostics.len(), 1);
    }

    #[test]
    fn policy9_bare_tuple_statements_validate_sequences_and_physical_lines() {
        for newline in ["\n", "\r\n"] {
            for (text, expected, owner_end) in [
                (
                    "eval(), helper()\neval(),\nhelper()\nafter()\n",
                    vec![
                        (1, "sample", "eval()"),
                        (1, "sample", "helper()"),
                        (2, "sample", "eval()"),
                        (3, "sample", "helper()"),
                        (4, "sample", "after()"),
                    ],
                    None,
                ),
                (
                    "def f(): eval(), helper()\nafter()\n",
                    vec![
                        (1, "sample.f", "eval()"),
                        (1, "sample.f", "helper()"),
                        (2, "sample", "after()"),
                    ],
                    Some(("function", 1)),
                ),
                (
                    "class Plain: eval(), helper()\nafter()\n",
                    vec![
                        (1, "sample.Plain", "eval()"),
                        (1, "sample.Plain", "helper()"),
                        (2, "sample", "after()"),
                    ],
                    Some(("class", 1)),
                ),
                (
                    "def f(): eval(),\nhelper()\n",
                    vec![(1, "sample.f", "eval()"), (2, "sample", "helper()")],
                    Some(("function", 1)),
                ),
                (
                    "class Plain: eval(),\nhelper()\n",
                    vec![(1, "sample.Plain", "eval()"), (2, "sample", "helper()")],
                    Some(("class", 1)),
                ),
                (
                    "def f():\n    eval(),\n    helper()\nafter()\n",
                    vec![
                        (2, "sample.f", "eval()"),
                        (3, "sample.f", "helper()"),
                        (4, "sample", "after()"),
                    ],
                    Some(("function", 3)),
                ),
                (
                    "class Plain:\n    eval(),\n    helper()\nafter()\n",
                    vec![
                        (2, "sample.Plain", "eval()"),
                        (3, "sample.Plain", "helper()"),
                        (4, "sample", "after()"),
                    ],
                    Some(("class", 3)),
                ),
            ] {
                let result = accepted(text.replace('\n', newline));
                let calls = result
                    .facts
                    .iter()
                    .filter(|f| f.kind == "call")
                    .map(|f| {
                        assert_eq!(f.line, f.end_line);
                        (f.line, f.symbol.as_str(), f.value.as_str())
                    })
                    .collect::<Vec<_>>();
                assert_eq!(calls, expected, "{text}: {result:?}");
                assert_eq!(
                    result.diagnostics.len(),
                    if owner_end.is_none() { 2 } else { 1 }
                );
                assert!(!result
                    .facts
                    .iter()
                    .any(|f| matches!(f.kind.as_str(), "binding" | "binding_invalidation")));
                if let Some((kind, end)) = owner_end {
                    assert!(result
                        .facts
                        .iter()
                        .any(|f| f.kind == kind && f.line == 1 && f.end_line == end));
                }
            }
            accepted("(eval(),\n helper())\n(eval()\n), helper()\n".replace('\n', newline));
            for statement in [
                "eval()\n, helper()",
                "eval(),,helper()",
                ", eval()",
                "eval() helper()",
                "eval(), x=helper()",
                "eval(), x: T = helper()",
                "x: T, helper()",
                "eval(),\\\nhelper()",
            ] {
                let result =
                    candidates(format!("eval()\nx=helper()\n{statement}\n").replace('\n', newline));
                assert_ne!(
                    result.files[0].status,
                    CandidateStatus::AcceptedFacts,
                    "{statement}: {result:?}"
                );
                assert_eq!(result.facts.len(), 1);
                assert!(result.diagnostics.is_empty());
            }
            accepted("x: T\nx=eval(),helper()\n".replace('\n', newline));
        }
    }

    #[test]
    fn policy9_core_shapes_and_exact_root_binding_invalidation() {
        for (expression, rendered) in [
            ("[]", "[]"),
            ("()", "[]"),
            ("{}", "<Dict>"),
            ("[x]", "[x]"),
            ("(x,)", "[x]"),
            ("x[0]", "x[<literal>]"),
            ("a+b", "a Add b"),
            ("[a, b,]", "[a, b]"),
            ("(a, b,)", "[a, b]"),
            ("{a, b,}", "[a, b]"),
            ("{'private': a, b: c,}", "<Dict>"),
            ("a+b*c", "a Add b Mult c"),
            ("a and b or c", "<BoolOp>"),
            ("a < b <= c", "<Compare>"),
            ("not a", "<UnaryOp>"),
            ("~a", "<UnaryOp>"),
            ("a if b else c", "<IfExp>"),
            ("xs[a]", "xs[a]"),
        ] {
            let result = accepted(format!("x={expression}\nsink({expression})\n"));
            assert!(
                result
                    .facts
                    .iter()
                    .any(|f| f.kind == "call" && f.value == format!("sink({rendered})")),
                "{expression}: {result:?}"
            );
            assert!(
                result
                    .facts
                    .iter()
                    .any(|f| f.kind == "binding_invalidation" && f.value == "{\"target\":\"x\"}"),
                "{expression}: {result:?}"
            );
            assert!(!result.facts.iter().any(|f| f.kind == "binding"));
        }
        let result = accepted("x=a,b,\ndef f():\n    return a,b\n");
        assert_eq!(
            result
                .facts
                .iter()
                .filter(|f| f.kind == "binding_invalidation")
                .count(),
            1
        );
        for rhs in ["name", "pkg.name", "helper()", "((helper()))"] {
            assert!(accepted(format!("x={rhs}"))
                .facts
                .iter()
                .any(|f| f.kind == "binding"));
        }
    }

    #[test]
    fn policy9_all_binary_operators_precedence_and_grouping() {
        for (operator, name) in [
            ("+", "Add"),
            ("-", "Sub"),
            ("*", "Mult"),
            ("@", "MatMult"),
            ("/", "Div"),
            ("//", "FloorDiv"),
            ("%", "Mod"),
            ("**", "Pow"),
            ("<<", "LShift"),
            (">>", "RShift"),
            ("|", "BitOr"),
            ("^", "BitXor"),
            ("&", "BitAnd"),
        ] {
            let result = accepted(format!("sink(a {operator} b)"));
            assert!(result
                .facts
                .iter()
                .any(|f| f.value == format!("sink(a {name} b)")));
        }
        for (text, expected) in [
            ("a+b*c**d", "a Add b Mult c Pow d"),
            ("(a+b)*c", "a Add b Mult c"),
            ("a**b**c", "a Pow b Pow c"),
            ("a<<b|c^d&e", "a LShift b BitOr c BitXor d BitAnd e"),
            ("-a**b", "<UnaryOp>"),
            ("(-a)**b", "<UnaryOp> Pow b"),
        ] {
            let result = accepted(format!("sink({text})"));
            assert!(
                result
                    .facts
                    .iter()
                    .any(|f| f.value == format!("sink({expected})")),
                "{result:?}"
            );
        }
        let plain = accepted("sink([a+b, c[d], a if b else c])");
        let grouped = accepted(format!(
            "sink({}([a+b, c[d], a if b else c]){})",
            "(".repeat(30),
            ")".repeat(30)
        ));
        assert_eq!(plain.facts, grouped.facts);
    }

    #[test]
    fn policy9_opaque_nodes_do_not_spend_descendant_render_budget() {
        let opaque = "[{'private': [a,b,c]}, a and (b or c), a < b < c, -(a+b), a if b else c]";
        let mut args = vec![opaque.to_owned()];
        args.extend(std::iter::repeat_n("[a,b,c,d,e]".to_owned(), 12));
        args.push("[a,b,c,d]".to_owned());
        args.push("tail".to_owned());
        // Call + callee + (list + five opaque nodes) + twelve six-node
        // lists exhaust exactly 80 nodes; later arguments remain ellipses.
        let result = accepted(format!("sink({})", args.join(",")));
        let expected = format!(
            "sink([<Dict>, <BoolOp>, <Compare>, <UnaryOp>, <IfExp>], {}, ..., ...)",
            std::iter::repeat_n("[a, b, c, d, e]", 12)
                .collect::<Vec<_>>()
                .join(", ")
        );
        assert!(
            result
                .facts
                .iter()
                .any(|f| f.kind == "call" && f.value == expected),
            "{result:?}"
        );
    }

    #[test]
    fn policy9_subscripts_slices_and_synthetic_tuple_indices() {
        for (expression, expected) in [
            ("xs[:]", "xs[<Slice>]"),
            ("xs[::]", "xs[<Slice>]"),
            ("xs[a:b:c]", "xs[<Slice>]"),
            ("xs[a:,]", "xs[[<Slice>]]"),
            ("xs[a,b,]", "xs[[a, b]]"),
            ("xs[(a,b)]", "xs[[a, b]]"),
            ("xs[a:b,c,::d]", "xs[[<Slice>, c, <Slice>]]"),
            ("[a,b][i+j][k]", "[a, b][i Add j][k]"),
            ("helper()[index()]", "helper()[index()]"),
        ] {
            let result = accepted(format!("sink({expression})"));
            assert!(
                result
                    .facts
                    .iter()
                    .any(|f| f.value == format!("sink({expected})")),
                "{result:?}"
            );
        }
        let indices = std::iter::repeat_n("a", 17).collect::<Vec<_>>().join(",");
        let result = accepted(format!("sink(xs[{indices}], xs[({indices})])"));
        let tuple = format!(
            "[{}]",
            std::iter::repeat_n("a", 16).collect::<Vec<_>>().join(", ")
        );
        assert!(result
            .facts
            .iter()
            .any(|f| f.value == format!("sink(xs[{tuple}], xs[{tuple}])")));
        let synthetic = std::iter::repeat_n("xs[a,b]", 16)
            .collect::<Vec<_>>()
            .join(",");
        let explicit = std::iter::repeat_n("xs[(a,b)]", 16)
            .collect::<Vec<_>>()
            .join(",");
        let synthetic = accepted(format!("sink({synthetic})"));
        let explicit = accepted(format!("sink({explicit})"));
        assert_eq!(synthetic.facts, explicit.facts);
        assert!(synthetic
            .facts
            .iter()
            .any(|f| f.kind == "call" && f.value.ends_with("xs[[..., ...]])")));
    }

    #[test]
    fn policy9_hidden_calls_and_all_dynamic_terminals_survive_render_limits() {
        for terminal in [
            "__import__",
            "import_module",
            "iter_modules",
            "walk_packages",
            "entry_points",
            "load_entry_point",
            "find_spec",
            "exec",
            "eval",
        ] {
            let padding = std::iter::repeat_n("a", 17).collect::<Vec<_>>().join(",");
            for expression in [
                format!("[{padding},{terminal}()]"),
                format!("{{{terminal}(): inner()}}"),
                format!("a and {terminal}()"),
                format!("a < {terminal}()"),
                format!("-{terminal}()"),
                format!("a if {terminal}() else inner()"),
                format!("xs[inner():{terminal}():inner()]"),
                format!("xs[a,{terminal}()]"),
                format!("a+{terminal}()"),
                format!("{}[{terminal}()]{}", "[".repeat(15), "]".repeat(15)),
                format!("{}([{terminal}()])", "a".repeat(600)),
                format!(
                    "[{}, {terminal}()]",
                    std::iter::repeat_n("[a,b,c,d,e]", 14)
                        .collect::<Vec<_>>()
                        .join(",")
                ),
            ] {
                for text in [
                    expression.clone(),
                    format!("x={expression}"),
                    format!("def f():\n    return {expression}\n"),
                ] {
                    let result = accepted(&text);
                    assert!(
                        result
                            .facts
                            .iter()
                            .any(|f| f.kind == "call" && f.value == format!("{terminal}()")),
                        "{text}: {result:?}"
                    );
                    assert_eq!(result.diagnostics.len(), 1, "{text}: {result:?}");
                    assert!(result.diagnostics[0].message.contains(terminal));
                }
            }
        }
    }

    #[test]
    fn policy9_call_free_defaults_expand_but_headers_stay_call_free() {
        for expression in [
            "DEFAULT",
            "math.inf",
            "Ellipsis",
            "[]",
            "{}",
            "()",
            "{1}",
            "1+2j",
            "--1",
            "+-1",
            "-True",
            "+False",
            "T[A]",
            "[a,{b:c}]",
            "a and b",
            "a not in b",
            "a is not b",
            "a if b else c",
            "xs[a:b,c]",
        ] {
            let result = accepted(format!("def f(x={expression}): pass"));
            assert_eq!(
                result
                    .facts
                    .iter()
                    .filter(|f| f.kind == "parameter_default")
                    .count(),
                1
            );
        }
        for expression in [
            "[eval()]",
            "{a: eval()}",
            "a and eval()",
            "a < eval()",
            "-eval()",
            "a if b else eval()",
            "xs[:eval()]",
            "xs[a,eval()]",
        ] {
            for text in [
                format!("def f(x={expression}): pass"),
                format!("def f(x: {expression}): pass"),
                format!("def f() -> {expression}: pass"),
                format!("class Plain({expression}): pass"),
            ] {
                let result = candidates(format!("eval()\n{text}"));
                assert_ne!(
                    result.files[0].status,
                    CandidateStatus::AcceptedFacts,
                    "{text}"
                );
                assert_eq!(result.facts.len(), 1);
                assert!(result.diagnostics.is_empty());
            }
        }
        for expression in ["A|B", "[A]", "{A}", "A+B", "T[A:B]"] {
            assert_ne!(
                candidates(format!("def f(x: {expression}): pass")).files[0].status,
                CandidateStatus::AcceptedFacts
            );
        }
        let padding = std::iter::repeat_n("a", 81).collect::<Vec<_>>().join(",");
        for expression in [
            format!("[{padding}, eval()]"),
            format!("{}eval(){}", "[".repeat(15), "]".repeat(15)),
            format!("{{a: {}(eval())}}", "a".repeat(600)),
        ] {
            let result = candidates(format!("eval()\ndef f(x={expression}): pass"));
            assert_eq!(result.files[0].status, CandidateStatus::UnsupportedSyntax);
            assert_eq!(result.facts.len(), 1);
            assert!(result.diagnostics.is_empty());
        }
    }

    #[test]
    fn policy9_invalid_grammar_and_star_contexts_remain_transactions() {
        for expression in [
            "{,}",
            "a<>b",
            "[,]",
            "[a,,b]",
            "(,)",
            "sink(,)",
            "xs[]",
            "xs[a:b:c:d]",
            "[*xs]",
            "(*xs,)",
            "{*xs}",
            "{**xs}",
            "*xs,",
            "sink(*xs)",
            "sink(**xs)",
            "[*xs][0]",
            "[x for x in xs]",
            "(lambda: x)",
            "(x:=y)",
            "(x for x in xs)",
            "(yield x)",
            "await x",
            "[a].field",
            "xs[a].field",
            "xs[a]()",
            "{a: FastAPI()}",
            "[obj.include_router(x)]",
            "a if pkg.APIRouter() else b",
        ] {
            for rhs in [expression.to_owned(), format!("{{a: {expression}}}")] {
                let result = candidates(format!("eval()\nx={rhs}\n"));
                assert_ne!(
                    result.files[0].status,
                    CandidateStatus::AcceptedFacts,
                    "{rhs}"
                );
                assert_eq!(result.facts.len(), 1);
                assert!(result.diagnostics.is_empty());
            }
        }
    }

    #[test]
    fn policy9_comparison_tokens_comments_and_physical_newline_gates() {
        for operator in [
            "<", "<=", "==", "!=", ">=", ">", "in", "not in", "is", "is not",
        ] {
            accepted(format!("sink(a {operator} b)"));
        }
        for newline in ["\n", "\r\n"] {
            for text in [
                "sink(a not\n in b)",
                "sink(a is\n not b)",
                "sink(a # neutral\n + b)",
                "sink({ # neutral\n a: b,\n})",
            ] {
                accepted(text.replace('\n', newline));
            }
            for text in [
                "x=a not\n in b",
                "x=a is\n not b",
                "x=a+\nb",
                "x=a\\\n+b",
                "x=[a,\tb]",
                "x=[a] # type: ignore",
            ] {
                assert_ne!(
                    candidates(text.replace('\n', newline)).files[0].status,
                    CandidateStatus::AcceptedFacts,
                    "{text}"
                );
            }
        }
    }

    #[test]
    fn policy9_core_resources_cancellation_and_single_counted_walk() {
        let sources = [source(
            "a.py",
            "a",
            "x=[{a:eval()}, xs[inner():b,c], a+(b*c), a if b else c]\n",
        )];
        let mut parses = 0;
        let mut checkpoints = 0;
        let full = extract(
            &sources,
            Limits::default(),
            FactLimits::default(),
            |phase| {
                parses += usize::from(phase == Checkpoint::PostParse);
                checkpoints += usize::from(phase == Checkpoint::Collector);
                false
            },
        )
        .unwrap();
        assert_eq!(full.files[0].status, CandidateStatus::AcceptedFacts);
        assert_eq!(parses, 1);
        let parsed = parse_batch(&sources).unwrap();
        assert_eq!(full.files[0].nodes_visited, parsed.files[0].nodes_visited);
        assert_eq!(full.files[0].max_depth, parsed.files[0].max_depth);
        assert!(full.files[0].traversal_finished);
        for stop in [1, checkpoints / 2, checkpoints] {
            let mut seen = 0;
            let result = extract(
                &sources,
                Limits::default(),
                FactLimits::default(),
                |phase| {
                    seen += usize::from(phase == Checkpoint::Collector);
                    seen == stop
                },
            )
            .unwrap();
            assert_eq!(result.files[0].status, CandidateStatus::ResourceLimit);
            assert_eq!(result.facts.len(), 1);
            assert!(result.diagnostics.is_empty());
        }
        for limits in [
            FactLimits {
                count: full.facts.len() - 1,
                ..FactLimits::default()
            },
            FactLimits {
                warnings: 0,
                ..FactLimits::default()
            },
            FactLimits {
                working: 1024,
                ..FactLimits::default()
            },
            FactLimits {
                bytes: serde_json::to_vec(&full).unwrap().len() - 1,
                ..FactLimits::default()
            },
        ] {
            let result = extract(&sources, Limits::default(), limits, |_| false).unwrap();
            assert_eq!(result.files[0].status, CandidateStatus::ResourceLimit);
            assert_eq!(result.facts.len(), 1);
            assert!(result.diagnostics.is_empty());
        }
        for limits in [
            Limits {
                nodes: parsed.files[0].nodes_visited - 1,
                ..Limits::default()
            },
            Limits {
                depth: parsed.files[0].max_depth - 1,
                ..Limits::default()
            },
        ] {
            let result = extract(&sources, limits, FactLimits::default(), |_| false).unwrap();
            assert_eq!(result.files[0].status, CandidateStatus::ResourceLimit);
            assert_eq!(result.facts.len(), 1);
            assert!(result.diagnostics.is_empty());
        }
    }

    #[test]
    fn policy9_core_failure_isolation_and_replay() {
        let sources = [
            source("bad.py", "bad", "eval()\nx={a:[lambda: b]}\n"),
            source("good.py", "good", "x=[a+b, xs[:], {a:eval()}]\n"),
        ];
        let result = extract_fact_candidates(&sources).unwrap();
        assert_eq!(result.files[0].status, CandidateStatus::UnsupportedSyntax);
        assert_eq!(result.files[1].status, CandidateStatus::AcceptedFacts);
        assert_eq!(
            result.facts.iter().filter(|f| f.path == "bad.py").count(),
            1
        );
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].path, "good.py");
        let replay = extract_fact_candidates(&sources).unwrap();
        assert_eq!(result.facts, replay.facts);
        assert_eq!(result.diagnostics, replay.diagnostics);
        assert_eq!(result.identity.sha256, replay.identity.sha256);
    }

    const DYNAMIC_NAMES: [&str; 9] = [
        "__import__",
        "import_module",
        "iter_modules",
        "walk_packages",
        "entry_points",
        "load_entry_point",
        "find_spec",
        "exec",
        "eval",
    ];

    fn warning_messages<'a>(result: &'a CandidateReport<'_>) -> Vec<&'a str> {
        assert!(result
            .diagnostics
            .iter()
            .all(|d| d.code == "dynamic_discovery_unresolved"));
        result
            .diagnostics
            .iter()
            .map(|d| d.message.as_str())
            .collect()
    }

    fn warning(line: usize, callee: &str) -> String {
        format!("line {line}: {callee}; runtime targets are not inferred")
    }

    #[test]
    fn base_lists_reject_comma_without_a_base_and_roll_back_warnings() {
        for header in [
            "class C(,): pass",
            "class C( , ): pass",
            "class C(, ): pass",
        ] {
            for newline in ["\n", "\r\n"] {
                let text = format!("eval(){newline}class Before: pass{newline}{header}{newline}");
                let sources = [
                    source("a.py", "a", text),
                    source("b.py", "b", "class Good((pkg.Base),): pass\n"),
                ];
                let parsed = parse_batch(&sources).unwrap();
                // The grammar admits this shape, so the collector must reject
                // it rather than relying on tree.has_error().
                assert_eq!(parsed.files[0].status, ParseStatus::SyntaxAccepted);
                let result = extract_fact_candidates(&sources).unwrap();
                assert_eq!(result.files[0].status, CandidateStatus::UnsupportedSyntax);
                assert_eq!(result.files[1].status, CandidateStatus::AcceptedFacts);
                assert_eq!(result.facts.iter().filter(|f| f.path == "a.py").count(), 1);
                assert!(result.diagnostics.is_empty());
            }
        }
        accepted("class Empty(): pass\nclass One(Base,): pass\n");
    }

    #[test]
    fn bounded_bases_are_exact_deduplicated_ordered_lexical_syntax() {
        let text = "class Plain(Zed, ((pkg.Base)), Zed, object,): pass\ndef outer():\n    class Inner((Other),): pass\n";
        let result = accepted(text);
        let bases: Vec<_> = result.facts.iter().filter(|f| f.kind == "base").collect();
        assert_eq!(bases.len(), 4);
        for (fact, (line, symbol, value)) in bases.iter().zip([
            (1, "sample.Plain", "Zed"),
            (1, "sample.Plain", "object"),
            (1, "sample.Plain", "pkg.Base"),
            (3, "sample.outer.Inner", "Other"),
        ]) {
            assert_eq!((fact.line, fact.end_line), (line, line));
            assert_eq!((&*fact.symbol, &*fact.value), (symbol, value));
            let preimage =
                format!("[\"sample.py\", \"base\", {line}, {line}, \"{symbol}\", \"{value}\"]");
            assert_eq!(
                fact.id,
                format!("fact_{}", super::super::super::hash(preimage.as_bytes()))
            );
        }
        assert!(!result
            .facts
            .iter()
            .any(|f| f.kind == "signature" && f.symbol != "sample.outer"));
        assert_eq!(result.facts, accepted(text.replace('\n', "\r\n")).facts);
        let mut sorted = result.facts.clone();
        sorted.sort_by(|a, b| {
            (
                &a.path, a.line, a.end_line, &a.kind, &a.symbol, &a.value, &a.id,
            )
                .cmp(&(
                    &b.path, b.line, b.end_line, &b.kind, &b.symbol, &b.value, &b.id,
                ))
        });
        assert_eq!(result.facts, sorted);
        // Neither duplicate bases nor unresolved names imply inheritance resolution.
        accepted("class Plain(Missing, Missing): pass\n");
        let sources = [source("sample.py", "sample", "class Plain(A, A): pass\n")];
        let limited = extract(
            &sources,
            Limits::default(),
            FactLimits {
                count: 3,
                ..FactLimits::default()
            },
            |_| false,
        )
        .unwrap();
        // Both base attempts count before global fact-ID deduplication.
        assert_eq!(
            limited.files[0].diagnostic,
            Some(CandidateDiagnostic::FactLimit)
        );
    }

    #[test]
    fn annotated_assignments_keep_context_and_annotation_only_has_no_write() {
        let result = accepted(concat!(
            "module: Kind\n",
            "module: Kind = helper()\n",
            "class Plain(Base):\n",
            "    field: pkg.Kind\n",
            "    scalar: None = -1\n",
            "    self.member: Kind\n",
            "    cls.member: Kind = value\n",
            "    def method(self):\n",
            "        local: Kind\n",
            "        local: Kind = value\n",
            "        self.member: Kind = helper()\n",
            "        class Nested:\n",
            "            field: 'sensitive annotation'\n",
            "def f():\n",
            "    local: Kind\n",
        ));
        let annotations: Vec<_> = result
            .facts
            .iter()
            .filter(|f| f.kind == "attribute_annotation")
            .map(|f| (f.symbol.as_str(), f.value.as_str(), f.line, f.end_line))
            .collect();
        assert_eq!(
            annotations,
            [
                ("sample.Plain.field", "pkg.Kind", 4, 4),
                ("sample.Plain.scalar", "<literal>", 5, 5),
                ("sample.Plain.method.Nested.field", "<literal>", 13, 13),
            ]
        );
        let writes: Vec<_> = result
            .facts
            .iter()
            .filter(|f| matches!(f.kind.as_str(), "binding" | "binding_invalidation"))
            .map(|f| (f.line, f.symbol.as_str(), f.kind.as_str()))
            .collect();
        assert_eq!(
            writes,
            [
                (2, "sample", "binding"),
                (5, "sample.Plain", "binding_invalidation"),
                (7, "sample.Plain", "binding"),
                (10, "sample.Plain.method", "binding"),
                (11, "sample.Plain.method", "binding"),
            ]
        );
        assert!(result
            .facts
            .iter()
            .any(|f| f.kind == "function" && f.symbol == "sample.f" && f.end_line == 15));
        assert!(result
            .facts
            .iter()
            .any(|f| f.kind == "class" && f.symbol.ends_with("Nested") && f.end_line == 13));
        assert!(!serde_json::to_string(&result)
            .unwrap()
            .contains("sensitive annotation"));
        for annotation in [
            "Kind",
            "pkg.Kind",
            "'private'",
            "None",
            "...",
            "T[A,]",
            "T[()]",
            "T[(A,B)]",
            "T[U[V], None, ...]",
        ] {
            accepted(format!(
                "x: {annotation}\nself.member: {annotation} = helper()\n"
            ));
        }
        for value in [
            "item",
            "pkg.item",
            "helper()",
            "'private'",
            "True",
            "None",
            "...",
            "12",
            "-2",
            "1.5",
        ] {
            accepted(format!("x: Kind = {value}\n"));
        }
    }

    #[test]
    fn class_annotation_symbols_keep_raw_names_while_binding_targets_clip() {
        for length in [511, 512, 513, 4096] {
            let name = "x".repeat(length);
            let result = accepted(format!("class Plain:\n    {name}: 'private' = value\n"));
            let annotation = result
                .facts
                .iter()
                .find(|f| f.kind == "attribute_annotation")
                .unwrap();
            assert_eq!(annotation.symbol, format!("sample.Plain.{name}"));
            assert_eq!(annotation.value, "<literal>");
            assert_eq!((annotation.line, annotation.end_line), (2, 2));
            let binding = result.facts.iter().find(|f| f.kind == "binding").unwrap();
            assert_eq!(
                binding.value,
                format!(
                    "{{\"source\":\"value\",\"target\":\"{}\"}}",
                    "x".repeat(length.min(512))
                )
            );
            let only = accepted(format!("class Plain:\n    {name}: Kind\n"));
            assert!(only
                .facts
                .iter()
                .any(|f| f.symbol == format!("sample.Plain.{name}")));
            assert!(!only.facts.iter().any(|f| f.kind.starts_with("binding")));
        }
    }

    #[test]
    fn annotated_statement_spans_include_groups_but_calls_are_normalized() {
        let text = concat!(
            "class Plain((pkg.Base),):\n", // 1
            "    field: (\n",              // 2
            "        T[\n",                // 3
            "            A, U[B],\n",      // 4
            "        ]\n",                 // 5
            "    ) = (\n",                 // 6
            "        helper(\n",           // 7
            "            item\n",          // 8
            "        )\n",                 // 9
            "    ) # trailing\n",          // 10
            "    only: (\n",               // 11
            "        'private'\n",         // 12
            "    )\n",                     // 13
            "    # not part of class\n",
        );
        let result = accepted(text);
        for (kind, value, line, end) in [
            ("base", "pkg.Base", 1, 1),
            ("attribute_annotation", "T[[A, U[B]]]", 2, 10),
            (
                "binding",
                r#"{"source":"helper(item)","target":"field"}"#,
                2,
                10,
            ),
            ("call", "helper(item)", 7, 9),
            ("attribute_annotation", "<literal>", 11, 13),
            ("class", "Plain", 1, 13),
        ] {
            assert!(
                result.facts.iter().any(|f| f.kind == kind
                    && f.value == value
                    && (f.line, f.end_line) == (line, end)),
                "{kind}: {result:?}"
            );
        }
        assert_eq!(result.facts, accepted(text.replace('\n', "\r\n")).facts);
        accepted("x: T[\n A, # inner\n U[B]\n] = (\n value\n)\n");
        accepted("x: T[(A, (B, None))]\n");
    }

    #[test]
    fn annotated_writes_preserve_alias_warnings_and_persistent_mixed_owners() {
        let result = accepted(concat!(
            "from pkg import eval as go\n",        // 1
            "go: Kind\n",                          // 2
            "go: Kind = None\n",                   // 3
            "go()\n",                              // 4
            "class Plain(Base):\n",                // 5
            "    from tools import exec as run\n", // 6
            "    run: Kind = ordinary\n",          // 7
            "    def method(run):\n",              // 8
            "        run: Kind\n",                 // 9
            "        run: Kind = helper()\n",      // 10
            "        run()\n",                     // 11
            "def Plain():\n",                      // 12
            "    run: Kind = None\n",              // 13
            "    run()\n",                         // 14
            "go: Kind = outer(eval(), eval())\n",  // 15
        ));
        assert_eq!(
            warning_messages(&result),
            [
                warning(11, "tools.exec"),
                warning(14, "tools.exec"),
                warning(15, "eval"),
                warning(15, "eval"),
                warning(4, "pkg.eval"),
            ]
        );
        assert_eq!(
            result
                .facts
                .iter()
                .filter(|f| f.kind == "call" && f.line == 15)
                .count(),
            2
        );
        assert!(!result
            .facts
            .iter()
            .any(|f| f.kind == "attribute_annotation" && f.line >= 8));
    }

    #[test]
    fn base_and_assignment_annotations_reject_unreviewed_and_hidden_forms() {
        for base in [
            "factory()",
            "T[A]",
            "'literal'",
            "None",
            "...",
            "1",
            "(A,B)",
            "A+B",
            "*bases",
            "**options",
            "metaclass=Meta",
            "A, metaclass=Meta",
            "A, *bases",
            "A, factory()",
        ] {
            let result = candidates(format!("eval()\nclass Plain({base}): pass\n"));
            assert_ne!(
                result.files[0].status,
                CandidateStatus::AcceptedFacts,
                "{base}"
            );
            assert_eq!(result.facts.len(), 1, "{base}");
            assert!(result.diagnostics.is_empty(), "{base}");
        }
        for annotation in [
            "factory()",
            "A | B",
            "1",
            "True",
            "b'text'",
            "-1",
            "A+B",
            "(A,B)",
            "T[A][B]",
            "T[factory()]",
            "T[A, factory()]",
            "T[A | B]",
            "lambda: A",
        ] {
            let result = candidates(format!(
                "eval()\nclass Plain:\n    field: {annotation} = helper()\n"
            ));
            assert_ne!(
                result.files[0].status,
                CandidateStatus::AcceptedFacts,
                "{annotation}"
            );
            assert_eq!(result.facts.len(), 1, "{annotation}");
            assert!(result.diagnostics.is_empty(), "{annotation}");
        }
        for target in [
            "(x)",
            "(self.member)",
            "other.member",
            "x[0]",
            "x,y",
            "self.router",
            "self.prefix",
            "cls.router.child",
            "self.field.router.child",
        ] {
            for suffix in [": T", ": T = helper()"] {
                let result = candidates(format!("eval()\n{target}{suffix}\n"));
                assert_ne!(
                    result.files[0].status,
                    CandidateStatus::AcceptedFacts,
                    "{target}{suffix}"
                );
                assert_eq!(result.facts.len(), 1);
                assert!(result.diagnostics.is_empty());
            }
        }
        for statement in [
            "x: T = y = z",
            "x = y: T",
            "x\n: T",
            "x:\n T",
            "x: T\n = value",
            "x: T =\n value",
            "class Plain(A,\n B): pass",
            "class Plain((\n A)): pass",
            "class Plain(A, # comment\n B): pass",
        ] {
            assert_ne!(
                candidates(statement).files[0].status,
                CandidateStatus::AcceptedFacts,
                "{statement}"
            );
        }
        for depth in [13, 30] {
            let grouped = format!("{}factory(){}", "(".repeat(depth), ")".repeat(depth));
            let annotation = format!(
                "T[{},factory()]",
                std::iter::repeat_n("A", 81).collect::<Vec<_>>().join(",")
            );
            for text in [
                format!("class Plain({grouped}): pass"),
                format!("x: {annotation}"),
                format!("x: {}factory(){}", "T[".repeat(depth), "]".repeat(depth)),
            ] {
                assert_eq!(candidates(text).facts.len(), 1);
            }
        }
        for value in [
            "FastAPI()",
            "pkg.APIRouter()",
            "obj.include_router(x)",
            "outer(FastAPI())",
        ] {
            assert_eq!(
                candidates(format!("x: T = {value}")).files[0].status,
                CandidateStatus::UnsupportedSyntax
            );
        }
        let long_target = format!("self.{}.router.child", "a".repeat(600));
        assert_eq!(
            candidates(format!("{long_target}: T\n")).files[0].status,
            CandidateStatus::UnsupportedSyntax
        );
        for value in ["pkg.APIRouter()", "obj.include_router(x)"] {
            let values = std::iter::repeat_n("item", 81)
                .collect::<Vec<_>>()
                .join(",");
            assert_eq!(
                candidates(format!("x: T = outer({values}, {value})\n")).files[0].status,
                CandidateStatus::UnsupportedSyntax
            );
        }
        for import in [
            "from fastapi import APIRouter as Router",
            "from pkg import FastAPI as Host",
        ] {
            assert_eq!(
                candidates(format!("{import}\nx: T = helper()\n")).files[0].status,
                CandidateStatus::UnsupportedSyntax
            );
        }
        let callee = format!("{}.APIRouter{}", "a".repeat(502), "z".repeat(100));
        assert_eq!(
            candidates(format!("x: T = {callee}()\n")).files[0].status,
            CandidateStatus::UnsupportedSyntax
        );
    }

    #[test]
    fn policy8_resources_cancellation_and_walk_counters_remain_shared() {
        let sources = [source("a.py", "a", "class Plain(Base, (pkg.Other),):\n    field: T[A] = outer(eval(), inner())\n    only: None\n")];
        let mut parses = 0;
        let mut collections = 0;
        let full = extract(
            &sources,
            Limits::default(),
            FactLimits::default(),
            |phase| {
                parses += usize::from(phase == Checkpoint::PostParse);
                collections += usize::from(phase == Checkpoint::Collector);
                false
            },
        )
        .unwrap();
        assert_eq!(parses, 1);
        assert_eq!(full.files[0].status, CandidateStatus::AcceptedFacts);
        let parsed = parse_batch(&sources).unwrap();
        assert_eq!(full.files[0].nodes_visited, parsed.files[0].nodes_visited);
        assert_eq!(full.files[0].max_depth, parsed.files[0].max_depth);
        assert!(full.files[0].traversal_finished);
        for stop in [1, collections / 2, collections] {
            let mut visits = 0;
            let result = extract(
                &sources,
                Limits::default(),
                FactLimits::default(),
                |phase| {
                    visits += usize::from(phase == Checkpoint::Collector);
                    visits == stop
                },
            )
            .unwrap();
            assert_eq!(result.files[0].status, CandidateStatus::ResourceLimit);
            assert_eq!(result.facts.len(), 1);
            assert!(result.diagnostics.is_empty());
        }
        for (limits, diagnostic) in [
            (
                FactLimits {
                    count: full.facts.len() - 1,
                    ..FactLimits::default()
                },
                CandidateDiagnostic::FactLimit,
            ),
            (
                FactLimits {
                    warnings: 0,
                    ..FactLimits::default()
                },
                CandidateDiagnostic::WarningLimit,
            ),
            (
                FactLimits {
                    working: 1024,
                    ..FactLimits::default()
                },
                CandidateDiagnostic::WorkingLimit,
            ),
            (
                FactLimits {
                    bytes: serde_json::to_vec(&full).unwrap().len() - 1,
                    ..FactLimits::default()
                },
                CandidateDiagnostic::OutputLimit,
            ),
        ] {
            let result = extract(&sources, Limits::default(), limits, |_| false).unwrap();
            assert_eq!(result.files[0].diagnostic, Some(diagnostic));
            assert_eq!(result.facts.len(), 1);
            assert!(result.diagnostics.is_empty());
        }
        for limits in [
            Limits {
                nodes: parsed.files[0].nodes_visited - 1,
                ..Limits::default()
            },
            Limits {
                depth: parsed.files[0].max_depth - 1,
                ..Limits::default()
            },
        ] {
            let result = extract(&sources, limits, FactLimits::default(), |_| false).unwrap();
            assert_eq!(result.files[0].status, CandidateStatus::ResourceLimit);
            assert_eq!(result.facts.len(), 1);
            assert!(result.diagnostics.is_empty());
        }
    }

    #[test]
    fn policy8_long_names_limits_replay_and_failed_files_stay_independent() {
        let name = "a".repeat(6000);
        let text = format!("class {name}(({name}),):\n    {name}: T['private'] = value\n");
        let result = accepted(&text);
        assert_eq!(
            result.files[0].sha256,
            super::super::super::hash(text.as_bytes())
        );
        assert!(result
            .facts
            .iter()
            .any(|f| f.kind == "base" && f.value == "a".repeat(512)));
        assert!(result.facts.iter().any(
            |f| f.kind == "attribute_annotation" && f.symbol == format!("sample.{name}.{name}")
        ));
        assert_eq!(result.facts, accepted(text.as_bytes()).facts);
        let sources = [
            source("a.py", "a", &text),
            source("b.py", "b", "class Plain(Base):\n    field: T\n"),
        ];
        let limited = extract(
            &sources,
            Limits::default(),
            FactLimits {
                working: 32_000,
                ..FactLimits::default()
            },
            |_| false,
        )
        .unwrap();
        assert_eq!(
            limited.files[0].diagnostic,
            Some(CandidateDiagnostic::WorkingLimit)
        );
        assert_eq!(limited.files[1].status, CandidateStatus::AcceptedFacts);
        for bad in [
            "eval()\nclass Bad(factory()): pass\n",
            "eval()\nx: T[factory()]\n",
            "eval()\nx: T\ndef broken(:\n",
        ] {
            let sources = [
                source("a.py", "a", bad),
                source("b.py", "b", "class Plain(Base):\n    field: T\n"),
            ];
            let result = extract_fact_candidates(&sources).unwrap();
            assert_ne!(result.files[0].status, CandidateStatus::AcceptedFacts);
            assert_eq!(result.files[1].status, CandidateStatus::AcceptedFacts);
            assert_eq!(result.facts.iter().filter(|f| f.path == "a.py").count(), 1);
            assert!(result.diagnostics.is_empty());
        }
        let sources = [source("a.py", "a", "x: T[factory()]\ndef broken(:\n")];
        let result = extract_fact_candidates(&sources).unwrap();
        assert_eq!(result.files[0].status, CandidateStatus::SyntaxError);
        let result = extract(
            &sources,
            Limits {
                nodes: 3,
                ..Limits::default()
            },
            FactLimits::default(),
            |_| false,
        )
        .unwrap();
        assert_eq!(result.files[0].status, CandidateStatus::ResourceLimit);
    }

    #[test]
    fn plain_classes_have_raw_names_exact_ids_and_narrow_inline_suites() {
        for header in ["class Plain:", "class Plain():", "class Plain (  ) :"] {
            for suite in [
                "pass",
                "'documentation'",
                "ordinary()",
                "value = item",
                "value = 1",
            ] {
                let text = format!("{header} {suite}\n# trailing\n");
                let result = accepted(&text);
                let class = result.facts.iter().find(|f| f.kind == "class").unwrap();
                assert_eq!((class.line, class.end_line), (1, 1));
                assert_eq!(class.symbol, "sample.Plain");
                assert_eq!(class.value, "Plain");
                assert_eq!(
                    class.id,
                    format!(
                        "fact_{}",
                        super::super::super::hash(
                            br#"["sample.py", "class", 1, 1, "sample.Plain", "Plain"]"#
                        )
                    )
                );
                assert!(result.facts.iter().all(|f| matches!(
                    f.kind.as_str(),
                    "module" | "class" | "call" | "binding" | "binding_invalidation"
                )));
                assert_eq!(
                    result.facts,
                    accepted(format!("\u{feff}{}", text.replace('\n', "\r\n"))).facts
                );
            }
        }
    }

    #[test]
    fn class_methods_reuse_function_signatures_and_plain_assignment_facts() {
        let result = accepted(concat!(
            "class Plain:\n",
            "    value = item\n",
            "    def __init__(self, x: Kind = None):\n",
            "        self.value = x\n",
            "    async def run(self, /, *args, flag=True, **kwargs) -> Kind:\n",
            "        return send(self.value)\n",
            "    def no_receiver(): return ordinary()\n",
        ));
        for (symbol, kind, line, end) in [
            ("sample.Plain", "class", 1, 7),
            ("sample.Plain.__init__", "function", 3, 4),
            ("sample.Plain.run", "async_function", 5, 6),
            ("sample.Plain.no_receiver", "function", 7, 7),
        ] {
            let fact = result
                .facts
                .iter()
                .find(|f| f.kind == kind && f.symbol == symbol)
                .unwrap();
            assert_eq!((fact.line, fact.end_line), (line, end));
        }
        let signatures: Vec<_> = result
            .facts
            .iter()
            .filter(|f| f.kind == "signature")
            .collect();
        assert_eq!(signatures.len(), 3);
        for (name, expected) in [
            (
                "__init__",
                serde_json::json!({"keywordOnly": [], "positional": ["self", "x"], "positionalOnly": 0}),
            ),
            (
                "run",
                serde_json::json!({"keywordOnly": ["flag"], "kwarg": "kwargs", "positional": ["self"], "positionalOnly": 1, "vararg": "args"}),
            ),
            (
                "no_receiver",
                serde_json::json!({"keywordOnly": [], "positional": [], "positionalOnly": 0}),
            ),
        ] {
            let signature = signatures
                .iter()
                .find(|f| f.symbol == format!("sample.Plain.{name}"))
                .unwrap();
            let metadata: serde_json::Value = serde_json::from_str(&signature.value).unwrap();
            assert_eq!(metadata, expected);
            let declaration = result
                .facts
                .iter()
                .find(|f| {
                    f.symbol == signature.symbol
                        && matches!(f.kind.as_str(), "function" | "async_function")
                })
                .unwrap();
            assert_eq!(
                (signature.line, signature.end_line),
                (declaration.line, declaration.end_line)
            );
        }
        assert!(result.facts.iter().any(|f| f.kind == "binding"
            && f.symbol == "sample.Plain"
            && f.value == r#"{"source":"item","target":"value"}"#));
        assert!(result.facts.iter().any(|f| f.kind == "binding"
            && f.symbol == "sample.Plain.__init__"
            && f.value == r#"{"source":"x","target":"self.value"}"#));
        assert!(result
            .facts
            .iter()
            .any(|f| f.kind == "parameter_annotation" && f.symbol == "sample.Plain.__init__.x"));
        assert!(result
            .facts
            .iter()
            .any(|f| f.kind == "return_annotation" && f.symbol == "sample.Plain.run"));
        assert!(result.facts.iter().all(|f| !matches!(
            f.kind.as_str(),
            "method" | "attribute" | "constructor" | "class_signature"
        )));
    }

    #[test]
    fn mixed_definitions_own_calls_bindings_imports_headers_and_spans() {
        let result = accepted(concat!(
            "def outer():\n",                                           // 1
            "    class Plain:\n",                                       // 2
            "        value = make()\n",                                 // 3
            "        def method(self, x=1):\n",                         // 4
            "            class Inner:\n",                               // 5
            "                ordinary()\n",                             // 6
            "                class Leaf:\n",                            // 7
            "                    async def work(arg: Kind) -> Kind:\n", // 8
            "                        def nested():\n",                  // 9
            "                            from pkg import (\n",          // 10
            "                                item,\n",                  // 11
            "                            )\n",                          // 12
            "                            # trailing nested\n",
            "# trailing module\n",
        ));
        let names = [
            ("outer", "function", 1),
            ("outer.Plain", "class", 2),
            ("outer.Plain.method", "function", 4),
            ("outer.Plain.method.Inner", "class", 5),
            ("outer.Plain.method.Inner.Leaf", "class", 7),
            ("outer.Plain.method.Inner.Leaf.work", "async_function", 8),
            ("outer.Plain.method.Inner.Leaf.work.nested", "function", 9),
        ];
        for (name, kind, line) in names {
            let fact = result
                .facts
                .iter()
                .find(|f| f.kind == kind && f.symbol == format!("sample.{name}"))
                .unwrap();
            assert_eq!((fact.line, fact.end_line), (line, 12), "{name}");
            assert_eq!(fact.value, name.rsplit('.').next().unwrap());
        }
        for (kind, line, end, symbol) in [
            ("call", 3, 3, "sample.outer.Plain"),
            ("binding", 3, 3, "sample.outer.Plain"),
            ("call", 6, 6, "sample.outer.Plain.method.Inner"),
            ("parameter_default", 4, 4, "sample.outer.Plain.method.x"),
            (
                "parameter_annotation",
                8,
                8,
                "sample.outer.Plain.method.Inner.Leaf.work.arg",
            ),
            (
                "from_import",
                10,
                12,
                "sample.outer.Plain.method.Inner.Leaf.work.nested",
            ),
        ] {
            assert!(
                result.facts.iter().any(|f| f.kind == kind
                    && f.symbol == symbol
                    && f.line == line
                    && f.end_line == end),
                "{kind}: {result:?}"
            );
        }
        assert_eq!(
            result
                .facts
                .iter()
                .filter(|f| f.kind == "signature")
                .count(),
            4
        );
    }

    #[test]
    fn class_spans_end_at_evidence_neutral_statements_and_nested_declarations() {
        for (tail, end) in [
            ("    pass\n", 4),
            ("    '''first\nlast'''\n", 5),
            ("    import pkg\n", 4),
            ("    from pkg import (\n        item,\n    )\n", 6),
            ("    class Nested: pass\n", 4),
            ("    def method(): pass\n", 4),
        ] {
            let result = accepted(format!(
                "def outer():\n    class Plain:\n        pass\n{}    # trailing\n# final\n",
                tail.lines()
                    .map(|line| format!("    {line}\n"))
                    .collect::<String>()
            ));
            for symbol in ["sample.outer", "sample.outer.Plain"] {
                let fact = result
                    .facts
                    .iter()
                    .find(|f| f.symbol == symbol && matches!(f.kind.as_str(), "function" | "class"))
                    .unwrap();
                assert_eq!(fact.end_line, end, "{tail}: {result:?}");
            }
        }
    }

    #[test]
    fn class_alias_scopes_are_persistent_kind_agnostic_lexical_names() {
        let text = concat!(
            "from root import eval as go\n",          // 1
            "class Same:\n",                          // 2
            "    from local import exec as go\n",     // 3
            "    class Child:\n",                     // 4
            "        from deep import eval as go\n",  // 5
            "def Same(go):\n",                        // 6
            "    go = None\n",                        // 7
            "    go()\n",                             // 8
            "    def Child():\n",                     // 9
            "        go()\n",                         // 10
            "class Same:\n",                          // 11
            "    from .safe import ordinary as go\n", // 12
            "    def method(go):\n",                  // 13
            "        go()\n",                         // 14
            "go()\n",                                 // 15
        );
        for module in ["sample", "", "odd.Same.Child", "odd..Same."] {
            let sources = [
                source("a.py", module, text),
                source("b.py", module, "class Same:\n    def Child(): go()\n"),
            ];
            let result = extract_fact_candidates(&sources).unwrap();
            assert!(
                result
                    .files
                    .iter()
                    .all(|f| f.status == CandidateStatus::AcceptedFacts),
                "{result:?}"
            );
            assert_eq!(
                warning_messages(&result),
                [
                    warning(10, "deep.eval"),
                    warning(14, "local.exec"),
                    warning(15, "root.eval"),
                    warning(8, "local.exec"),
                ]
            );
            assert!(result.diagnostics.iter().all(|d| d.path == "a.py"));
        }
    }

    #[test]
    fn class_policy_rejects_header_extensions_returns_and_special_metadata() {
        for text in [
            "class Hidden(Base()): pass",
            "class Hidden(object[T]): pass",
            "class Hidden(metaclass=Meta): pass",
            "class Hidden(*bases): pass",
            "class Hidden(**options): pass",
            "class Hidden(factory()): pass",
            "class Hidden[T]: pass",
            "@decorate\nclass Hidden: pass",
            "class Hidden(\n): pass",
            "class Hidden( # comment\n): pass",
            "class Hidden(\r\n): pass",
            "class Hidden \\\n(): pass",
            "class Hidden:\n    @decorate\n    def method(): pass",
            "class Hidden:\n    value: Kind | Other = item",
            "class Hidden:\n    value: Kind()",
            "class Hidden: import pkg",
            "class Hidden: pass; ordinary()",
            "class Hidden: def method(): pass",
            "class Hidden:\n\tpass",
            "class Hidden:\n    from pkg import FastAPI as unused",
            "class Hidden:\n    import APIRouter as unused",
            "class Hidden:\n    FastAPI()",
            "class Hidden:\n    pkg.APIRouter()",
            "class Hidden:\n    obj.include_router()",
            "class Hidden:\n    obj.include_router(router)",
            "class Hidden:\n    router.prefix = '/path'",
            "class Hidden:\n    def method(self):\n        self.router = item",
        ] {
            let result = candidates(format!("eval()\n{text}\n"));
            assert_ne!(
                result.files[0].status,
                CandidateStatus::AcceptedFacts,
                "{text}"
            );
            assert_eq!(result.facts.len(), 1, "{text}");
            assert!(result.diagnostics.is_empty(), "{text}");
        }
        for text in [
            "class Hidden: return",
            "class Hidden: return ordinary()",
            "def outer():\n    class Hidden:\n        return ordinary()",
            "async def outer():\n    class Hidden:\n        return",
            "class Hidden:\n    def method(): pass\n    return",
        ] {
            let result = candidates(format!("eval()\n{text}\n"));
            assert_eq!(
                result.files[0].status,
                CandidateStatus::UnsupportedSyntax,
                "{text}"
            );
            assert_eq!(result.facts.len(), 1, "{text}");
            assert!(result.diagnostics.is_empty(), "{text}");
        }
    }

    #[test]
    fn class_only_depth_boundary_has_no_signatures_and_is_file_local() {
        let mut text = String::new();
        for depth in 0..16 {
            text.push_str(&format!("{}class C{depth}:\n", " ".repeat(depth)));
        }
        text.push_str(&format!("{}pass\n", " ".repeat(16)));
        let sources = [
            source("a.py", "a", &text),
            source("b.py", "b", "class Plain: pass\n"),
        ];
        let parsed = parse_batch(&sources).unwrap();
        let depth = parsed.files[0].max_depth;
        assert!(depth > parsed.files[1].max_depth);
        for (limit, accepted) in [(depth - 1, false), (depth, true)] {
            let mut signatures = 0;
            let result = extract(
                &sources,
                Limits {
                    depth: limit,
                    ..Limits::default()
                },
                FactLimits::default(),
                |phase| {
                    signatures += usize::from(phase == Checkpoint::Signature);
                    false
                },
            )
            .unwrap();
            assert_eq!(signatures, 0);
            assert_eq!(
                result.files[0].status == CandidateStatus::AcceptedFacts,
                accepted
            );
            assert_eq!(result.files[1].status, CandidateStatus::AcceptedFacts);
            assert_eq!(
                result
                    .facts
                    .iter()
                    .filter(|f| f.path == "a.py" && f.kind == "class")
                    .count(),
                if accepted { 16 } else { 0 }
            );
            assert!(result.diagnostics.is_empty());
        }
    }

    #[test]
    fn class_resources_charge_frames_symbols_and_persistent_scopes_without_signatures() {
        let sources = [source("a.py", "a", "class Plain: pass\n")];
        let mut signatures = 0;
        let result = extract(
            &sources,
            Limits::default(),
            FactLimits {
                working: 16_000,
                ..FactLimits::default()
            },
            |phase| {
                signatures += usize::from(phase == Checkpoint::Signature);
                false
            },
        )
        .unwrap();
        assert_eq!(result.files[0].status, CandidateStatus::AcceptedFacts);
        assert_eq!(signatures, 0);

        let target = format!("{}.ordinary", "p".repeat(2000));
        for distinct in [false, true] {
            let text = (0..100)
                .map(|i| {
                    format!(
                        "class C{}:\n    import {target} as go\n",
                        if distinct { i } else { 0 }
                    )
                })
                .collect::<String>();
            let sources = [
                source("a.py", "a", text),
                source("b.py", "b", "class C0: go()\n"),
            ];
            let result = extract(
                &sources,
                Limits::default(),
                FactLimits {
                    working: 80_000,
                    ..FactLimits::default()
                },
                |_| false,
            )
            .unwrap();
            assert_eq!(
                result.files[0].status,
                if distinct {
                    CandidateStatus::ResourceLimit
                } else {
                    CandidateStatus::AcceptedFacts
                },
                "{result:?}"
            );
            if distinct {
                assert_eq!(
                    result.files[0].diagnostic,
                    Some(CandidateDiagnostic::WorkingLimit)
                );
            }
            assert_eq!(result.files[1].status, CandidateStatus::AcceptedFacts);
            assert!(result.diagnostics.is_empty());
        }

        let depth = 30;
        let mut text = format!("class {}:\n", "x".repeat(9000));
        for level in 1..depth {
            text.push_str(&format!(
                "{}{}\n",
                " ".repeat(level),
                if level % 2 == 0 {
                    "class Nested:"
                } else {
                    "def nested():"
                }
            ));
        }
        text.push_str(&format!("{}pass\n", " ".repeat(depth)));
        let sources = [
            source("a.py", "a", text),
            source("b.py", "b", "class Plain: pass\n"),
        ];
        let mut frames = 0;
        let result = extract(
            &sources,
            Limits::default(),
            FactLimits {
                working: 90_000,
                ..FactLimits::default()
            },
            |phase| {
                frames += usize::from(phase == Checkpoint::FunctionFrame);
                false
            },
        )
        .unwrap();
        assert_eq!(
            result.files[0].diagnostic,
            Some(CandidateDiagnostic::WorkingLimit)
        );
        assert!(frames > 2 && frames < depth);
        assert_eq!(result.files[1].status, CandidateStatus::AcceptedFacts);

        let sources = [source(
            "a.py",
            "a",
            "class Plain:\n    eval()\n    exec()\n",
        )];
        let full = extract_fact_candidates(&sources).unwrap();
        for (limits, diagnostic) in [
            (
                FactLimits {
                    count: 2,
                    ..FactLimits::default()
                },
                CandidateDiagnostic::FactLimit,
            ),
            (
                FactLimits {
                    warnings: 1,
                    ..FactLimits::default()
                },
                CandidateDiagnostic::WarningLimit,
            ),
            (
                FactLimits {
                    working: 1,
                    ..FactLimits::default()
                },
                CandidateDiagnostic::WorkingLimit,
            ),
            (
                FactLimits {
                    bytes: serde_json::to_vec(&full).unwrap().len() - 1,
                    ..FactLimits::default()
                },
                CandidateDiagnostic::OutputLimit,
            ),
        ] {
            let result = extract(&sources, Limits::default(), limits, |_| false).unwrap();
            assert_eq!(result.files[0].diagnostic, Some(diagnostic));
            assert_eq!(result.facts.len(), 1);
            assert!(result.diagnostics.is_empty());
        }
    }

    #[test]
    fn class_candidates_share_single_parse_walk_cancellation_and_file_isolation() {
        let sources = [
            source("a.py", "a", "class Plain:\n    from pkg import eval as go\n    def method(self):\n        class Nested:\n            go()\n"),
            source("b.py", "b", "class Plain:\n    def method(): go()\n"),
        ];
        let checkpoints = [
            Checkpoint::Collector,
            Checkpoint::Working,
            Checkpoint::FunctionFrame,
        ];
        let mut totals = [0; 3];
        let mut parses = 0;
        let full = extract(
            &sources,
            Limits::default(),
            FactLimits::default(),
            |phase| {
                parses += usize::from(phase == Checkpoint::PostParse);
                for (i, point) in checkpoints.iter().enumerate() {
                    totals[i] += usize::from(phase == *point);
                }
                false
            },
        )
        .unwrap();
        assert_eq!(parses, sources.len());
        assert!(full
            .files
            .iter()
            .all(|f| f.status == CandidateStatus::AcceptedFacts));
        assert_eq!(warning_messages(&full), [warning(5, "pkg.eval")]);
        let parsed = parse_batch(&sources).unwrap();
        for (file, parsed) in full.files.iter().zip(&parsed.files) {
            assert!(file.traversal_finished);
            assert_eq!(file.nodes_visited, parsed.nodes_visited);
            assert_eq!(file.max_depth, parsed.max_depth);
        }
        for (i, point) in checkpoints.iter().enumerate() {
            for stop in 1..=totals[i] {
                let mut visits = 0;
                let result = extract(
                    &sources,
                    Limits::default(),
                    FactLimits::default(),
                    |phase| {
                        visits += usize::from(phase == *point);
                        visits == stop
                    },
                )
                .unwrap();
                assert!(result.diagnostics.is_empty(), "{point:?} {stop}");
                assert_eq!(result.facts.len(), sources.len(), "{point:?} {stop}");
                assert!(result.facts.iter().all(|f| f.kind == "module"));
                assert!(result
                    .files
                    .iter()
                    .all(|f| f.diagnostic
                        == Some(CandidateDiagnostic::Parse(DiagnosticCode::Deadline))));
            }
        }
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
        assert_eq!(result.files[1].status, CandidateStatus::AcceptedFacts);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn dynamic_warnings_keep_duplicates_and_sort_globally_as_text() {
        let sources = [
            source("z.py", "z", "eval()\n"),
            source(
                "a.py",
                "a",
                "pass\nouter(eval(), eval())\npass\npass\npass\npass\npass\npass\npass\nexec()\n",
            ),
        ];
        let result = extract_fact_candidates(&sources).unwrap();
        assert!(result
            .files
            .iter()
            .all(|f| f.status == CandidateStatus::AcceptedFacts));
        assert_eq!(
            serde_json::to_value(&result.diagnostics).unwrap(),
            serde_json::json!([
                {"code":"dynamic_discovery_unresolved", "path":"a.py", "message":warning(10, "exec")},
                {"code":"dynamic_discovery_unresolved", "path":"a.py", "message":warning(2, "eval")},
                {"code":"dynamic_discovery_unresolved", "path":"a.py", "message":warning(2, "eval")},
                {"code":"dynamic_discovery_unresolved", "path":"z.py", "message":warning(1, "eval")},
            ])
        );
        assert_eq!(
            result
                .facts
                .iter()
                .filter(|f| f.path == "a.py" && f.value == "eval()")
                .count(),
            1
        );
        assert_eq!(result.identity.policy, "python-native-private-facts-9");
    }

    #[test]
    fn dynamic_imports_move_from_exclusions_to_supported_aliases() {
        for terminal in DYNAMIC_NAMES {
            for import in [
                format!("from pkg import (\n safe, # ordinary\n {terminal} as unused,\n)"),
                format!("import safe, pkg.{terminal} as unused, other"),
                format!("import safe, {terminal}.child, other"),
                format!("from pkg import safe, {terminal} as unused, other"),
                format!("import {}.{terminal} as unused", "a".repeat(900)),
                format!("def scoped():\n    from pkg import {terminal} as unused\n    unused=1"),
            ] {
                let result = accepted(format!("import safe\ndef before(): pass\n{import}\n"));
                assert!(result.diagnostics.is_empty(), "{import}");
            }
            for import in [
                format!("import pkg.{terminal}"),
                format!("from .pkg import {terminal} as alias\nalias()"),
                format!("from ... import {terminal} as alias\nalias()"),
                format!("from .pkg import (\n {terminal} as alias,\n)\nalias()"),
                format!("import pkg.{terminal}.ordinary as alias\nalias()"),
            ] {
                assert!(accepted(import).diagnostics.is_empty());
            }
            let result = accepted(format!("from pkg import {terminal} as go\ngo()\n"));
            assert_eq!(
                warning_messages(&result),
                [warning(2, &format!("pkg.{terminal}"))]
            );
        }
        assert_eq!(
            warning_messages(&accepted("import tools\nobj.import_module()")),
            [warning(2, "obj.import_module")]
        );
    }

    #[test]
    fn dynamic_aliases_overwrite_suppress_do_not_recurse_or_invalidate() {
        let result = accepted(concat!(
            "from pkg import eval as go\n",        // 1
            "go()\n",                              // 2
            "from safe import ordinary as go\n",   // 3
            "go()\n",                              // 4
            "from safe import ordinary as eval\n", // 5
            "eval()\n",                            // 6
            "import eval as go\n",                 // 7: no recursive expansion through eval
            "go()\n",                              // 8
            "from .safe import ordinary as go\n",  // 9: no invalidation
            "go=None\n",                           // 10
            "def go(): pass\n",                    // 11: no invalidation
            "go()\n",                              // 12
            "import exec.child\n",                 // 13: target is exec, not exec.child
            "exec()\n",                            // 14
            "import pkg.eval\n",                   // 15: target is pkg
            "pkg()\n",                             // 16
            "pkg.eval()\n",                        // 17
        ));
        assert_eq!(
            warning_messages(&result),
            [
                warning(12, "eval"),
                warning(14, "exec"),
                warning(17, "pkg.eval"),
                warning(2, "pkg.eval"),
                warning(8, "eval")
            ]
        );
        assert_eq!(
            warning_messages(&accepted("from pkg import eval as go, exec as go\ngo()")),
            [warning(2, "pkg.exec")]
        );
        for text in [
            "import ordinary as FastAPI\nFastAPI()",
            "import ordinary as APIRouter\nAPIRouter()",
            "from pkg import FastAPI as x\nimport ordinary as x\nx()",
            "from pkg import APIRouter as x\nimport ordinary as x\nx()",
        ] {
            assert_eq!(
                candidates(text).files[0].status,
                CandidateStatus::UnsupportedSyntax
            );
        }
    }

    #[test]
    fn dynamic_scope_keys_persist_by_name_not_module_or_declaration() {
        let text = concat!(
            "from pkg import eval as go\n", // root
            "def outer(go):\n",
            "    go=None\n",
            "    go()\n", // parameters/writes do not shadow imports
            "    def inner():\n",
            "        from nested import exec as go\n",
            "        go()\n",
            "    go()\n", // nearest parent, not exited child
            "def outer():\n",
            "    def inner():\n",
            "        go()\n", // reuses outer/inner aliases
            "    from safe import ordinary as go\n",
            "    go()\n",
            "def outer():\n",
            "    go()\n", // reuses outer safe alias
            "go()\n",     // root unchanged
        );
        let expected = [
            warning(11, "nested.exec"),
            warning(16, "pkg.eval"),
            warning(4, "pkg.eval"),
            warning(7, "nested.exec"),
            warning(8, "pkg.eval"),
        ];
        for module in ["sample", "", "odd.outer.inner", "odd..outer."] {
            let sources = [source("a.py", module, text)];
            let result = extract_fact_candidates(&sources).unwrap();
            assert_eq!(
                result.files[0].status,
                CandidateStatus::AcceptedFacts,
                "{result:?}"
            );
            assert_eq!(warning_messages(&result), expected);
        }
    }

    #[test]
    fn dynamic_callees_clip_fresh_roots_and_expand_full_targets_once() {
        for length in [511, 512, 513] {
            let local = "n".repeat(length.min(512));
            let spelling = "n".repeat(length);
            let target = format!("{}.eval", "p".repeat(900));
            let result = accepted(format!("import {target} as {local}\n{spelling}()\n"));
            assert_eq!(warning_messages(&result), [warning(2, &target)]);
            let fact = result.facts.iter().find(|f| f.kind == "call").unwrap();
            assert_eq!(
                fact.value,
                format!(
                    "{}{}",
                    &spelling[..length.min(512)],
                    if length < 512 { "(" } else { "" }
                )
            );
        }
        // Raw alias length 513 is not a match for the rendered 512-byte head.
        let local = "n".repeat(513);
        assert!(
            accepted(format!("from pkg import eval as {local}\n{local}()\n"))
                .diagnostics
                .is_empty()
        );
        let target = format!("{}.ordinary", "p".repeat(900));
        assert_eq!(
            warning_messages(&accepted(format!("import {target} as go\ngo.eval()"))),
            [warning(2, &format!("{target}.eval"))]
        );
        for terminal in DYNAMIC_NAMES {
            let n = 512 - terminal.len() - 1;
            for length in [n - 1, n, n + 1] {
                let callee = format!("{}.{}Extra", "a".repeat(length), terminal);
                let result = accepted(format!("{callee}()"));
                let expected = if length == n {
                    vec![warning(1, &callee[..512])]
                } else {
                    vec![]
                };
                assert_eq!(warning_messages(&result), expected);
            }
        }
    }

    #[test]
    fn dynamic_nested_calls_warn_even_when_parent_shape_hides_them() {
        let padding = std::iter::repeat_n("ok()", 17)
            .collect::<Vec<_>>()
            .join(",");
        let exhausted = (0..16)
            .map(|i| format!("k{i}=f(a,b,c,d,e,f)"))
            .collect::<Vec<_>>()
            .join(",");
        for terminal in DYNAMIC_NAMES {
            // These were all previously negative fixtures, including literal args.
            let call = format!("{terminal}('x')");
            let clipped = format!(
                "{}.{}Extra()",
                "a".repeat(512 - terminal.len() - 1),
                terminal
            );
            for (call, callee) in [
                (call, terminal.to_string()),
                (clipped.clone(), clipped[..512].to_string()),
                (format!("pkg.{terminal}()"), format!("pkg.{terminal}")),
            ] {
                for rhs in [
                    call.clone(),
                    format!("outer({padding},{call})"),
                    format!("outer({exhausted},bad={call})"),
                    format!("{}{}{}", "f(".repeat(15), call, ")".repeat(15)),
                    format!("{}(x={call})", "a".repeat(600)),
                ] {
                    let result =
                        accepted(format!("def before(): pass\nx={rhs}\ndef after(): pass\n"));
                    assert_eq!(warning_messages(&result), [warning(2, &callee)], "{rhs}");
                }
            }
        }
    }

    #[test]
    fn dynamic_warning_limits_rollback_and_alias_storage_is_bounded() {
        let sources = [
            source("a.py", "a", "outer(eval(), eval(), eval())"),
            source("b.py", "b", "eval()"),
        ];
        let result = extract(
            &sources,
            Limits::default(),
            FactLimits {
                warnings: 2,
                ..FactLimits::default()
            },
            |_| false,
        )
        .unwrap();
        assert_eq!(
            result.files[0].diagnostic,
            Some(CandidateDiagnostic::WarningLimit)
        );
        assert_eq!(result.files[1].status, CandidateStatus::AcceptedFacts);
        assert_eq!(warning_messages(&result), [warning(1, "eval")]);
        assert_eq!(result.diagnostics[0].path, "b.py");
        let target = format!("{}.eval", "p".repeat(2000));
        // Replacing the same alias and reusing the same function scope must not
        // accumulate old target/name/container charges indefinitely.
        let sources = [source(
            "a.py",
            "a",
            format!("def f():\n    import {target} as go\n").repeat(100),
        )];
        let result = extract(
            &sources,
            Limits::default(),
            FactLimits {
                working: 80_000,
                ..FactLimits::default()
            },
            |_| false,
        )
        .unwrap();
        assert_eq!(
            result.files[0].status,
            CandidateStatus::AcceptedFacts,
            "{result:?}"
        );
        let sources = [source(
            "a.py",
            "a",
            (0..100)
                .map(|i| format!("def f{i}():\n    import {target} as go\n"))
                .collect::<String>(),
        )];
        let result = extract(
            &sources,
            Limits::default(),
            FactLimits {
                working: 80_000,
                ..FactLimits::default()
            },
            |_| false,
        )
        .unwrap();
        assert_eq!(
            result.files[0].diagnostic,
            Some(CandidateDiagnostic::WorkingLimit)
        );
        assert!(result.diagnostics.is_empty());
        let sources = [source(
            "a.py",
            "a",
            format!("import {target} as go\n{}", "go()\n".repeat(20)),
        )];
        let full = extract_fact_candidates(&sources).unwrap();
        for limits in [
            FactLimits {
                working: 10_000,
                ..FactLimits::default()
            },
            FactLimits {
                bytes: serde_json::to_vec(&full).unwrap().len() - 1,
                ..FactLimits::default()
            },
        ] {
            let result = extract(&sources, Limits::default(), limits, |_| false).unwrap();
            assert_eq!(result.files[0].status, CandidateStatus::ResourceLimit);
            assert!(result.diagnostics.is_empty());
            assert_eq!(result.facts.len(), 1);
        }
        let sources = [
            source("a.py", "a", "eval()\n".repeat(100)),
            source("b.py", "b", "eval()\n".repeat(100)),
        ];
        let result = extract(
            &sources,
            Limits::default(),
            FactLimits {
                working: 60_000,
                ..FactLimits::default()
            },
            |_| false,
        )
        .unwrap();
        assert_eq!(result.files[0].status, CandidateStatus::AcceptedFacts);
        assert_eq!(
            result.files[1].diagnostic,
            Some(CandidateDiagnostic::WorkingLimit)
        );
        assert_eq!(result.diagnostics.len(), 100);
        assert!(result.diagnostics.iter().all(|d| d.path == "a.py"));
    }

    #[test]
    fn dynamic_warning_capacity_reuses_after_rollback_and_alias_arenas_reset() {
        fn minimum_working(sources: &[Source]) -> usize {
            let mut low = 0;
            let mut high = 64 * 1024;
            let accepts = |working| {
                extract(
                    sources,
                    Limits::default(),
                    FactLimits {
                        working,
                        ..FactLimits::default()
                    },
                    |_| false,
                )
                .unwrap()
                .files
                .iter()
                .all(|f| f.status == CandidateStatus::AcceptedFacts)
            };
            assert!(accepts(high));
            while low < high {
                let middle = low + (high - low) / 2;
                if accepts(middle) {
                    high = middle;
                } else {
                    low = middle + 1;
                }
            }
            low
        }

        let count = 128;
        let calls = "eval()\n".repeat(count);
        let warning_work = minimum_working(&[source("a.py", "a", &calls)]);
        let function_work = minimum_working(&[source("b.py", "b", "def f(): pass\n")]);
        let working = warning_work.max(function_work);
        let retained_slots = count * 4 * std::mem::size_of::<CandidateWarning>();
        assert!(retained_slots > working - function_work);
        let discarded_text: usize = (1..=count)
            .map(|line| warning(line, "eval").len() + "a.py".len())
            .sum();
        assert!(discarded_text > working - warning_work);
        // Measure the single-file peaks rather than pinning allocator/layout
        // constants. The same warnings fit again only if discarded text is freed
        // and existing vector slots are reused without being charged twice.
        let sources = [
            source(
                "a.py",
                "a",
                format!("{calls}class Unsupported(metaclass=Meta): pass\n"),
            ),
            source("b.py", "b", &calls),
        ];
        let result = extract(
            &sources,
            Limits::default(),
            FactLimits {
                working,
                ..FactLimits::default()
            },
            |_| false,
        )
        .unwrap();
        assert_eq!(result.files[0].status, CandidateStatus::UnsupportedSyntax);
        assert_eq!(result.files[1].status, CandidateStatus::AcceptedFacts);
        assert_eq!(result.diagnostics.len(), count);
        assert!(result.diagnostics.iter().all(|d| d.path == "b.py"));
        assert_eq!(result.facts.iter().filter(|f| f.path == "a.py").count(), 1);

        // A function fits by itself, but not alongside the discarded file's
        // retained warning-vector capacity. Capacity must stay charged even
        // though no warning text or records remain after rollback.
        let sources = [
            source(
                "a.py",
                "a",
                format!("{calls}class Unsupported(metaclass=Meta): pass\n"),
            ),
            source("b.py", "b", "def f(): pass\n"),
        ];
        let result = extract(
            &sources,
            Limits::default(),
            FactLimits {
                working,
                ..FactLimits::default()
            },
            |_| false,
        )
        .unwrap();
        assert_eq!(result.files[0].status, CandidateStatus::UnsupportedSyntax);
        assert_eq!(
            result.files[1].diagnostic,
            Some(CandidateDiagnostic::WorkingLimit)
        );
        assert!(result.diagnostics.is_empty());
        assert!(result.diagnostics.capacity() >= count);

        let target = format!("{}.ordinary", "p".repeat(2000));
        let mut text = "from pkg import eval as go\n".to_string();
        for i in 0..24 {
            text.push_str(&format!("def f{i}():\n    import {target} as local\n"));
        }
        let sources = [
            source("a.py", "a", text),
            source("b.py", "b", "go()\ndef f(): pass\n"),
        ];
        // As in the existing 80k retained-alias test, distinct persistent scopes
        // exhaust this budget. Neither their charges nor aliases may leak files.
        let result = extract(
            &sources,
            Limits::default(),
            FactLimits {
                working: 80_000,
                ..FactLimits::default()
            },
            |_| false,
        )
        .unwrap();
        assert_eq!(
            result.files[0].diagnostic,
            Some(CandidateDiagnostic::WorkingLimit)
        );
        assert_eq!(result.files[1].status, CandidateStatus::AcceptedFacts);
        assert!(result.diagnostics.is_empty());
        assert!(result
            .facts
            .iter()
            .any(|f| f.path == "b.py" && f.value == "go()"));
    }

    #[test]
    fn dynamic_alias_size_changes_warning_output_and_long_comparison_are_bounded() {
        let target = format!("{}.eval", "p".repeat(2000));
        let rounds = 40;
        let working = 64 * 1024;
        assert!(rounds * target.len() > working);
        let mut text =
            format!("import {target} as go\nfrom pkg import exec as go\n").repeat(rounds);
        text.push_str("go()\n");
        let sources = [source("a.py", "a", text)];
        // 64 KiB admits one import's target/metadata/scratch, not cumulative
        // copies of all forty long targets. Alternation exercises both shrink
        // and grow replacements rather than only equal-size replacement.
        let result = extract(
            &sources,
            Limits::default(),
            FactLimits {
                working,
                ..FactLimits::default()
            },
            |_| false,
        )
        .unwrap();
        assert_eq!(result.files[0].status, CandidateStatus::AcceptedFacts);
        assert_eq!(
            warning_messages(&result),
            [warning(2 * rounds + 1, "pkg.exec")]
        );

        let path = "escaped\"\t\u{e9}.py";
        let import = format!("import {target} as go\n");
        let sources = [
            source(path, "a", format!("{import}go()\ngo()\n")),
            source("good.py", "good", "ordinary()\n"),
        ];
        let full = extract_fact_candidates(&sources).unwrap();
        assert!(full
            .files
            .iter()
            .all(|f| f.status == CandidateStatus::AcceptedFacts));
        assert_eq!(
            warning_messages(&full),
            [warning(2, &target), warning(3, &target)]
        );
        let encoded = serde_json::to_string(&full.diagnostics[0]).unwrap();
        assert!(encoded.contains("\\\"") && encoded.contains("\\t") && encoded.contains('\u{e9}'));
        let cancel = Cancellation {
            expired: Cell::new(false),
            check: RefCell::new(|_| false),
        };
        let reserved =
            reserve_candidates(&sources, Limits::default(), FactLimits::default(), &cancel)
                .unwrap();
        let import_fact = full
            .facts
            .iter()
            .find(|f| f.path == path && f.kind == "import")
            .unwrap();
        let first_call = full
            .facts
            .iter()
            .find(|f| f.path == path && f.kind == "call" && f.line == 2)
            .unwrap();
        // Admit the import, first warning and first call, then leave the second
        // warning's exact record size but not its required comma byte. Include
        // escaped UTF-8 path serialization in every record's measured size.
        let bytes = reserved
            + serde_json::to_vec(import_fact).unwrap().len()
            + 1
            + encoded.len()
            + 1
            + serde_json::to_vec(first_call).unwrap().len()
            + 1
            + serde_json::to_vec(&full.diagnostics[1]).unwrap().len();
        let prefix = [
            source(path, "a", format!("{import}go()\n")),
            source("good.py", "good", "ordinary()\n"),
        ];
        let admitted_prefix = extract(
            &prefix,
            Limits::default(),
            FactLimits {
                bytes,
                ..FactLimits::default()
            },
            |_| false,
        )
        .unwrap();
        assert!(admitted_prefix
            .files
            .iter()
            .all(|f| f.status == CandidateStatus::AcceptedFacts));
        assert!(admitted_prefix
            .facts
            .iter()
            .any(|f| f.path == path && f.kind == "import"));
        assert_eq!(warning_messages(&admitted_prefix), [warning(2, &target)]);
        let result = extract(
            &sources,
            Limits::default(),
            FactLimits {
                bytes,
                ..FactLimits::default()
            },
            |_| false,
        )
        .unwrap();
        assert_eq!(
            result.files[0].diagnostic,
            Some(CandidateDiagnostic::OutputLimit)
        );
        assert_eq!(result.files[1].status, CandidateStatus::AcceptedFacts);
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.facts.iter().filter(|f| f.path == path).count(), 1);
        assert!(result
            .facts
            .iter()
            .any(|f| f.path == "good.py" && f.value == "ordinary()"));
        assert!(serde_json::to_vec(&result).unwrap().len() <= bytes);

        let shared = "p".repeat(4096);
        let mut warnings: Vec<_> = ["exec", "eval"]
            .into_iter()
            .map(|terminal| CandidateWarning {
                code: "dynamic_discovery_unresolved",
                path: "a.py".into(),
                message: warning(1, &format!("{shared}.{terminal}")),
            })
            .collect();
        let before = warnings.clone();
        let in_comparison = Cell::new(false);
        let comparison_hits = Cell::new(0);
        let interrupted = Cell::new(false);
        let cancel = Cancellation {
            expired: Cell::new(false),
            check: RefCell::new(|phase| {
                if phase == Checkpoint::Sort && in_comparison.get() {
                    comparison_hits.set(comparison_hits.get() + 1);
                    return comparison_hits.get() == 3;
                }
                false
            }),
        };
        interruptible_sort(&mut warnings, &cancel, |a, b| {
            in_comparison.set(true);
            let order = compare_text(&a.message, &b.message, &cancel);
            in_comparison.set(false);
            interrupted.set(order.is_none());
            order
        });
        assert!(interrupted.get());
        assert_eq!(comparison_hits.get(), 3);
        assert!(cancel.expired.get());
        assert!(cancel.at(Checkpoint::Sort));
        assert_eq!(
            compare_text(&warnings[0].message, &warnings[1].message, &cancel),
            None
        );
        assert_eq!(warnings, before);
    }

    #[test]
    fn dynamic_warnings_share_cancellation_transactions_and_single_walk() {
        let sources = [
            source(
                "a.py",
                "a",
                "from pkg import eval as go\ndef f():\n    go()\n",
            ),
            source("b.py", "b", "go()\nexec()\neval()\n"),
        ];
        let mut parses = 0;
        let mut totals = [0; 3];
        let checkpoints = [Checkpoint::Collector, Checkpoint::Working, Checkpoint::Sort];
        let full = extract(
            &sources,
            Limits::default(),
            FactLimits::default(),
            |phase| {
                parses += usize::from(phase == Checkpoint::PostParse);
                for (i, point) in checkpoints.iter().enumerate() {
                    totals[i] += usize::from(phase == *point);
                }
                false
            },
        )
        .unwrap();
        assert_eq!(parses, 2);
        assert_eq!(full.diagnostics.len(), 3); // no alias leak into b.py
        let parsed = parse_batch(&sources).unwrap();
        for (file, parsed) in full.files.iter().zip(&parsed.files) {
            assert_eq!(file.nodes_visited, parsed.nodes_visited);
            assert_eq!(file.max_depth, parsed.max_depth);
        }
        for (i, point) in checkpoints.iter().enumerate() {
            for stop in 1..=totals[i] {
                let mut visits = 0;
                let result = extract(
                    &sources,
                    Limits::default(),
                    FactLimits::default(),
                    |phase| {
                        visits += usize::from(phase == *point);
                        visits == stop
                    },
                )
                .unwrap();
                assert!(result.diagnostics.is_empty(), "{point:?} {stop}");
                assert!(result.facts.iter().all(|f| f.kind == "module"));
                assert!(result
                    .files
                    .iter()
                    .all(|f| f.status == CandidateStatus::ResourceLimit));
            }
        }
        for point in [
            Checkpoint::Output,
            Checkpoint::FinalOutput,
            Checkpoint::Final,
        ] {
            let mut visits = 0;
            extract(
                &sources,
                Limits::default(),
                FactLimits::default(),
                |phase| {
                    visits += usize::from(phase == point);
                    false
                },
            )
            .unwrap();
            let mut seen = 0;
            let result = extract(
                &sources,
                Limits::default(),
                FactLimits::default(),
                |phase| {
                    seen += usize::from(phase == point);
                    seen == visits
                },
            )
            .unwrap();
            assert!(result.diagnostics.is_empty());
            assert!(result.facts.iter().all(|f| f.kind == "module"));
        }
        for suffix in [
            "class Unsupported(Base()): pass",
            "def broken(: pass",
            "from pkg import FastAPI",
        ] {
            let sources = [
                source("a.py", "a", format!("eval()\n{suffix}\n")),
                source("b.py", "b", "eval()"),
            ];
            let result = extract_fact_candidates(&sources).unwrap();
            assert_ne!(result.files[0].status, CandidateStatus::AcceptedFacts);
            assert_eq!(result.diagnostics.len(), 1);
            assert_eq!(result.diagnostics[0].path, "b.py");
        }
        let sources = [
            source("a.py", "a", "eval()"),
            source("b.py", "b", b"# coding: latin-1\neval()\n\xff"),
            source("c.py", "c", "exec()"),
        ];
        let result = extract_fact_candidates(&sources).unwrap();
        assert_eq!(result.files[1].status, CandidateStatus::UnsupportedEncoding);
        assert_eq!(
            result
                .diagnostics
                .iter()
                .map(|d| d.path.as_str())
                .collect::<Vec<_>>(),
            ["a.py", "c.py"]
        );
    }

    #[test]
    fn imports_preserve_metadata_order_ownership_and_statement_extents() {
        let text = "import a . b as c, a.b, a.b # tail\nfrom a . b import x as y, x, z\ndef outer():\n    import tools\n    def inner():\n        from ....pkg . sub import thing as t\n        # tail\n    from . import local\n# tail\n";
        let result = accepted(text);
        let imports: Vec<_> = result
            .facts
            .iter()
            .filter(|f| matches!(f.kind.as_str(), "import" | "from_import"))
            .map(|f| {
                (
                    f.kind.as_str(),
                    f.line,
                    f.end_line,
                    f.symbol.as_str(),
                    f.value.as_str(),
                )
            })
            .collect();
        assert_eq!(imports, vec![
            ("import", 1, 1, "sample", "{\"level\":0,\"module\":\"\",\"names\":[{\"asname\":\"c\",\"name\":\"a.b\"},{\"asname\":null,\"name\":\"a.b\"},{\"asname\":null,\"name\":\"a.b\"}]}"),
            ("from_import", 2, 2, "sample", "{\"level\":0,\"module\":\"a.b\",\"names\":[{\"asname\":\"y\",\"name\":\"x\"},{\"asname\":null,\"name\":\"x\"},{\"asname\":null,\"name\":\"z\"}]}"),
            ("import", 4, 4, "sample.outer", "{\"level\":0,\"module\":\"\",\"names\":[{\"asname\":null,\"name\":\"tools\"}]}"),
            ("from_import", 6, 6, "sample.outer.inner", "{\"level\":4,\"module\":\"pkg.sub\",\"names\":[{\"asname\":\"t\",\"name\":\"thing\"}]}"),
            ("from_import", 8, 8, "sample.outer", "{\"level\":1,\"module\":\"\",\"names\":[{\"asname\":null,\"name\":\"local\"}]}"),
        ]);
        for fact in result
            .facts
            .iter()
            .filter(|f| matches!(f.kind.as_str(), "function" | "signature"))
        {
            assert_eq!(
                fact.end_line,
                if fact.symbol == "sample.outer" { 8 } else { 6 }
            );
        }
        for text in [
            "import  a  .  b  as  c  ,  d\n",
            "from . . . . pkg . sub import x as y\n",
            "from...pkg import x\n",
            "from.import x\n",
            "import tools\ntools.run()\ndef f(x: tools.T) -> tools.R:\n    from pkg import run as r\n    return r(x)\n",
            "\u{feff}import tools\r\nfrom ..pkg import item # tail\r\n",
        ] { accepted(text); }
    }

    #[test]
    fn multiline_import_metadata_and_closing_spans_preserve_ownership() {
        let text = "from pkg import (x,)\ndef outer():\n    def inner():\n        from ..pkg import ( # opening\n            x # before as\n            as # before alias\n            local,\n            x, # duplicate\n            # before close ) ( , \\\n        ) # trailing\n        # not part of extent\n# final\n";
        let result = accepted(text);
        let import = result.facts.iter().find(|f| f.line == 4).unwrap();
        assert_eq!(import.kind, "from_import");
        assert_eq!(import.end_line, 10);
        assert_eq!(import.symbol, "sample.outer.inner");
        assert_eq!(
            import.value,
            r#"{"level":2,"module":"pkg","names":[{"asname":"local","name":"x"},{"asname":null,"name":"x"}]}"#
        );
        for f in result
            .facts
            .iter()
            .filter(|f| matches!(f.kind.as_str(), "function" | "signature"))
        {
            assert_eq!(f.end_line, 10);
        }
        for text in [
            "from pkg import(x)",
            "from pkg import (x, y as z,)",
            "from.import(x,)",
            "from...pkg import(\nx\nas\ny,\n)",
            "\u{feff}from ..pkg import (\r\n # comment \u{3bb}\t\u{c}\r\n x,\r\n) # tail\r\n",
        ] {
            accepted(text);
        }
    }

    #[test]
    fn multiline_import_continuation_and_grammar_remain_fail_closed() {
        for text in [
            "from\npkg import (x)",
            "from pkg\nimport (x)",
            "from pkg import\n(x)",
            "from pkg import # comment\n(x)",
            "from pkg import ()",
            "from pkg import (,)",
            "from pkg import (x,,y)",
            "from pkg import (x y)",
            "from pkg import (x.y)",
            "from pkg import (x as)",
            "from pkg import ((x))",
            "from pkg import (x), y",
            "from pkg import (x) (y)",
            "from pkg import (x,) as y",
            "import (pkg)",
            "from pkg import (x\t)",
            "from pkg import (\n\tx\n)",
            "from pkg import (x\u{c})",
            "from pkg import (\u{212a})",
            "from pkg import (x as \\\n y)",
            "from pkg import (x, # type: ignore\n y)",
            "from pkg import ( #\t type: list[T]\n x)",
            "from __future__ import (annotations,)",
            "from pkg import (x); pass",
            "def f(): from pkg import (x)",
        ] {
            let result = candidates(format!(
                "import safe\ndef before(): pass\n{text}\ndef after(): pass\n"
            ));
            assert_ne!(
                result.files[0].status,
                CandidateStatus::AcceptedFacts,
                "{text:?}: {result:?}"
            );
            assert_eq!(result.facts.len(), 1);
        }
    }

    #[test]
    fn multiline_import_comments_are_borrowed_and_state_is_file_local() {
        let marker = "PRIVATE_COMMENT_PAYLOAD";
        let text = format!("from pkg import (\n # {}\n x,\n)\n", marker.repeat(5000));
        let result = accepted(&text);
        assert!(!serde_json::to_string(&result).unwrap().contains(marker));
        let sources = [
            source("a.py", "a", "from pkg import (\n x, # type: ignore\n)\n"),
            source("b.py", "b", &text),
            source("c.py", "c", "from pkg import item\nitem()\n"),
        ];
        let result = super::super::extract_fact_candidates(&sources).unwrap();
        assert_eq!(result.files[0].status, CandidateStatus::UnsupportedSyntax);
        assert_eq!(result.files[1].status, CandidateStatus::AcceptedFacts);
        assert_eq!(result.files[2].status, CandidateStatus::AcceptedFacts);
        assert_eq!(result.facts.iter().filter(|f| f.path == "a.py").count(), 1);
        // Unsupported syntax does not hide a later parser error.
        let result = candidates("from pkg import (\n x, # type: ignore\n)\ndef broken(:\n");
        assert_eq!(result.files[0].status, CandidateStatus::SyntaxError);
        assert_eq!(result.facts.len(), 1);
    }

    #[test]
    fn import_grammar_rejections_rollback_every_candidate() {
        for text in [
            "from pkg import *",
            "from pkg import ((x))",
            "from pkg import (*)",
            "import a,",
            "from pkg import a,",
            "from pkg import a.b",
            "from __future__ import annotations",
            "import a; import b",
            "import a\t",
            "import\ta",
            "from pkg\timport a",
            "import a\u{c}",
            "import \u{212a}",
            "from \u{212a} import x",
            "import a as \u{212a}",
            "import a \\\n .b",
            "from pkg \\\n import x",
            "from pkg import \\\n x",
            "import a.\nb",
            "from pkg\nimport x",
            "from pkg import x # type: ignore",
            "def f(): import tools",
            "def f(): from pkg import item",
            "import a as class",
            "from pkg import True",
            "from pkgimport x",
        ] {
            let result = candidates(format!(
                "import safe\ndef before(): pass\n{text}\ndef after(): pass\n"
            ));
            assert_ne!(
                result.files[0].status,
                CandidateStatus::AcceptedFacts,
                "{text:?}: {result:?}"
            );
            assert_eq!(result.facts.len(), 1, "{text:?}");
        }
    }

    #[test]
    fn imports_still_exclude_full_absolute_constructor_targets() {
        for terminal in ["FastAPI", "APIRouter"] {
            for import in [
                format!("from pkg import (\n safe, # ordinary\n {terminal} as unused,\n)"),
                format!("import safe, pkg.{terminal} as unused, other"),
                format!("import safe, {terminal}.child, other"),
                format!("from pkg import safe, {terminal} as unused, other"),
                format!("import {}.{terminal} as unused", "a".repeat(900)),
                format!("def scoped():\n    from pkg import {terminal} as unused\n    unused=1"),
            ] {
                let result = candidates(format!("import safe\ndef before(): pass\n{import}\n"));
                assert_eq!(
                    result.files[0].status,
                    CandidateStatus::UnsupportedSyntax,
                    "{import}: {result:?}"
                );
                assert_eq!(result.facts.len(), 1);
            }
            // A plain dotted import binds only its first component in the helper.
            accepted(format!("import pkg.{terminal}\n"));
            // Relative imports never populate the helper alias map.
            accepted(format!("from .pkg import {terminal} as alias\nalias()\n"));
            accepted(format!("from ... import {terminal} as alias\nalias()\n"));
            accepted(format!(
                "from .pkg import (\n {terminal} as alias,\n)\nalias()\n"
            ));
            accepted(format!(
                "import pkg.{terminal}.ordinary as alias\nalias()\n"
            ));
        }
        accepted(
            "import pkg.fastapi as alias\nalias()\nfrom pkg import include_router as r\nr()\n",
        );
        for text in [
            "import tools\nFastAPI()",
            "from .pkg import FastAPI\nFastAPI()",
            "import tools\nobj.include_router(x)",
        ] {
            let result = candidates(text);
            assert_eq!(result.files[0].status, CandidateStatus::UnsupportedSyntax);
            assert_eq!(result.facts.len(), 1);
        }
        let long = "a".repeat(900);
        let result = accepted(format!(
            "from {long}.pkg import ordinary as alias\nalias()\n"
        ));
        assert!(result
            .facts
            .iter()
            .any(|f| f.kind == "from_import" && f.value.contains(&long)));
    }

    #[test]
    fn import_limits_cancellation_and_single_counted_walk() {
        let sources = [source(
            "a.py",
            "a",
            "import safe\nfrom ....pkg . sub import (\n x as y, # ordinary\n x, z,\n)\ny()\n",
        )];
        let parsed = parse_batch(&sources).unwrap();
        let mut parses = 0;
        let full = extract(
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
        assert_eq!(full.files[0].status, CandidateStatus::AcceptedFacts);
        assert_eq!(full.files[0].nodes_visited, parsed.files[0].nodes_visited);
        assert_eq!(full.files[0].max_depth, parsed.files[0].max_depth);
        for limits in [
            FactLimits {
                working: 1,
                ..FactLimits::default()
            },
            FactLimits {
                count: 2,
                ..FactLimits::default()
            },
            FactLimits {
                bytes: serde_json::to_vec(&full).unwrap().len() - 1,
                ..FactLimits::default()
            },
        ] {
            let result = extract(&sources, Limits::default(), limits, |_| false).unwrap();
            assert_eq!(
                result.files[0].status,
                CandidateStatus::ResourceLimit,
                "{result:?}"
            );
            assert_eq!(result.facts.len(), 1);
            assert_eq!(result.files[0].nodes_visited, parsed.files[0].nodes_visited);
        }
        for checkpoint in [
            Checkpoint::Collector,
            Checkpoint::Working,
            Checkpoint::Output,
            Checkpoint::FactId,
        ] {
            let mut total = 0;
            extract(
                &sources,
                Limits::default(),
                FactLimits::default(),
                |phase| {
                    total += usize::from(phase == checkpoint);
                    false
                },
            )
            .unwrap();
            for stop in 1..=total {
                let mut visits = 0;
                let result = extract(
                    &sources,
                    Limits::default(),
                    FactLimits::default(),
                    |phase| {
                        visits += usize::from(phase == checkpoint);
                        visits == stop
                    },
                )
                .unwrap();
                assert_eq!(
                    result.files[0].status,
                    CandidateStatus::ResourceLimit,
                    "{checkpoint:?} {stop}: {result:?}"
                );
                assert_eq!(result.facts.len(), 1);
            }
        }
    }

    #[test]
    fn import_state_and_budget_are_isolated_between_files() {
        let sources = [
            source(
                "a.py",
                "a",
                "import safe\nfrom pkg import FastAPI as alias\n",
            ),
            source("b.py", "b", "from .pkg import FastAPI as alias\nalias()\n"),
            source("c.py", "c", "import tools\ntools.run()\n"),
        ];
        let result = extract_fact_candidates(&sources).unwrap();
        assert_eq!(result.files[0].status, CandidateStatus::UnsupportedSyntax);
        assert_eq!(result.files[1].status, CandidateStatus::AcceptedFacts);
        assert_eq!(result.files[2].status, CandidateStatus::AcceptedFacts);
        assert_eq!(result.facts.iter().filter(|f| f.path == "a.py").count(), 1);
        assert!(result
            .facts
            .iter()
            .any(|f| f.path == "b.py" && f.kind == "call" && f.value == "alias()"));
        assert!(result
            .facts
            .iter()
            .any(|f| f.path == "c.py" && f.kind == "call" && f.value == "tools.run()"));
        let sources = [
            source(
                "a.py",
                "a",
                format!("import safe\nimport {}\n", "a".repeat(20_000)),
            ),
            source("b.py", "b", "import tools\n".repeat(100)),
        ];
        let result = extract(
            &sources,
            Limits::default(),
            FactLimits {
                working: 4096,
                ..FactLimits::default()
            },
            |_| false,
        )
        .unwrap();
        assert_eq!(result.files[0].status, CandidateStatus::ResourceLimit);
        assert_eq!(
            result.files[0].diagnostic,
            Some(CandidateDiagnostic::WorkingLimit)
        );
        assert_eq!(result.files[1].status, CandidateStatus::AcceptedFacts);
        assert_eq!(result.facts.iter().filter(|f| f.path == "a.py").count(), 1);
        assert_eq!(
            result
                .facts
                .iter()
                .filter(|f| f.path == "b.py" && f.kind == "import")
                .count(),
            100
        );
    }

    #[test]
    fn annotated_parameter_kinds_and_literal_defaults() {
        let result = accepted("async def f(a: pkg.T = 1, /, b: T[A,] = -2, *args: T[(A, B)], c: 'Forward' = None, d: None, **kw: T[U[V]]) -> (pkg.R):\n    pass # tail\n");
        let signature = result.facts.iter().find(|f| f.kind == "signature").unwrap();
        assert_eq!(signature.value, "{\"keywordOnly\":[\"c\",\"d\"],\"kwarg\":\"kw\",\"positional\":[\"a\",\"b\"],\"positionalOnly\":1,\"vararg\":\"args\"}");
        for (symbol, expected) in [
            ("a", "pkg.T"),
            ("b", "T[[A]]"),
            ("args", "T[[A, B]]"),
            ("c", "<literal>"),
            ("d", "<literal>"),
            ("kw", "T[U[V]]"),
        ] {
            assert!(
                result.facts.iter().any(|f| f.kind == "parameter_annotation"
                    && f.symbol == format!("sample.f.{symbol}")
                    && f.value == expected),
                "{symbol}: {result:?}"
            );
        }
        assert!(result.facts.iter().any(|f| f.kind == "return_annotation"
            && f.symbol == "sample.f"
            && f.value == "pkg.R"));
        assert_eq!(
            result
                .facts
                .iter()
                .filter(|f| f.kind == "parameter")
                .count(),
            6
        );
        assert_eq!(
            result
                .facts
                .iter()
                .filter(|f| f.kind == "parameter_default")
                .count(),
            3
        );
        for annotation in [
            "A",
            "pkg.A",
            "None",
            "...",
            "'Forward'",
            "r'Forward'",
            "u'Forward'",
            "'''Forward'''",
            "'For' 'ward'",
            "(T[(A)])",
            "T[A]",
            "T[A,]",
            "T[A,B]",
            "T[(A,B)]",
            "T[()]",
            "pkg.T[U[V], None, ...]",
        ] {
            accepted(format!("def f(x: {annotation}) -> {annotation}: pass"));
        }
        for default in [
            "None",
            "True",
            "False",
            "...",
            "0",
            "0_0",
            "12_345",
            "0x_Ff",
            "0O_17",
            "0b_101",
            "1.",
            ".5",
            "01.2",
            "1_0.2_3e+4_5",
            "1e-3",
            "00e2",
            "1j",
            "01j",
            "1.2e3J",
            "+(2)",
            "-(0xF)",
            "+1j",
            "'text'",
            "R'text'",
            "U'text'",
            "'''text'''",
            "b'abc'",
            "Br'abc'",
            "rB'abc'",
            "b'a' b'b'",
            "'a' r'b'",
        ] {
            let result = accepted(format!("def f(x={default}): pass"));
            let fact = result
                .facts
                .iter()
                .find(|f| f.kind == "parameter_default")
                .unwrap();
            assert_eq!(
                fact.value,
                if default.starts_with(['+', '-']) {
                    "<UnaryOp>"
                } else {
                    "<literal>"
                },
                "{default}"
            );
        }
    }

    #[test]
    fn grouped_expression_spans_follow_ast_not_cst_wrappers() {
        let text = "def f(\n x: (\n  T[A]\n ) = (\n  - (\n   2\n  )\n ),\n y=(\n  'hidden' # between\n  'payload'\n ),\n) -> (\n pkg.R\n):\n    pass\n    # tail\n";
        let result = accepted(text);
        for (kind, symbol, line, end) in [
            ("parameter", "sample.f.x", 2, 4),
            ("parameter_annotation", "sample.f.x", 2, 4),
            ("parameter_default", "sample.f.x", 5, 7),
            ("parameter", "sample.f.y", 9, 9),
            ("parameter_default", "sample.f.y", 10, 11),
            ("return_annotation", "sample.f", 14, 14),
            ("function", "sample.f", 1, 16),
            ("signature", "sample.f", 1, 16),
        ] {
            let fact = result
                .facts
                .iter()
                .find(|f| f.kind == kind && f.symbol == symbol)
                .unwrap();
            assert_eq!(
                (fact.line, fact.end_line),
                (line, end),
                "{kind} {symbol}: {result:?}"
            );
        }
        let report = serde_json::to_string(&result).unwrap();
        for secret in ["hidden", "payload", "between", "tail"] {
            assert!(!report.contains(secret));
        }
        accepted("def f(x: T = 1):\r\n    pass # end\r\n");
    }

    #[test]
    fn expression_line_breaks_require_implicit_continuation() {
        for newline in ["\n", "\r\n"] {
            for header in [
                "def f() -> pkg.\nT:\n    pass\n",
                "def f() -> pkg\n.T:\n    pass\n",
                "def f() -> pkg. # continued?\nT:\n    pass\n",
                "def f() -> T\n[A]:\n    pass\n",
                "def f() -> (T) # comment\n:\n    pass\n",
                "def f() -> (\n T\n)\n# extra\n:\n    pass\n",
                "def f() -> T[A]\n# extra\n:\n    pass\n",
                "def f() -> 'forward'\n'continuation':\n    pass\n",
            ] {
                let text = format!("def before(): pass\n{header}def after(): pass\n")
                    .replace('\n', newline);
                let result = candidates(&text);
                assert_ne!(
                    result.files[0].status,
                    CandidateStatus::AcceptedFacts,
                    "{text:?}: {result:?}"
                );
                assert_eq!(result.facts.len(), 1, "{text:?}");
            }
            for header in [
                "def f() -> (pkg.\n T):\n    pass\n",
                "def f() -> pkg.T[\n U[V],\n W,\n]:\n    pass\n",
                "def f() -> (\n pkg. # ][()\n T[\n U,\n V\n ]\n):\n    pass\n",
                "def f(x: pkg.\n T = (\n -(\n 2\n )\n)) -> pkg.T:\n    pass\n",
                "def f(x: # annotation\n pkg.\n T,\n y= # default\n -\n 1,\n):\n    pass\n",
                "def f(x: (T # inner\n ) = 1, y: T # before default\n = 1, z = 2\n):\n    pass\n",
            ] {
                let text = format!("def before(): pass\n{header}def after(): pass\n")
                    .replace('\n', newline);
                let result = candidates(&text);
                assert_eq!(
                    result.files[0].status,
                    CandidateStatus::AcceptedFacts,
                    "{text:?}: {result:?}"
                );
                let sources = [source("sample.py", "sample", &text)];
                let parsed = parse_batch(&sources).unwrap();
                assert_eq!(result.files[0].nodes_visited, parsed.files[0].nodes_visited);
                assert_eq!(result.files[0].max_depth, parsed.files[0].max_depth);
            }
        }
        let result = candidates("def f() -> pkg.\nT:\n    pass\n");
        assert_eq!(result.files[0].status, CandidateStatus::UnsupportedSyntax);
    }

    #[test]
    fn only_triple_quoted_literals_allow_physical_body_newlines() {
        for newline in ["\n", "\r\n"] {
            for prefix in ["", "r", "u", "b", "br", "rb", "R", "BR"] {
                for quote in ["'", "\""] {
                    let literal = format!("{prefix}{quote}{newline}value{quote}");
                    let mut headers = vec![format!("def f(x={literal}):\n    pass\n")];
                    if !prefix.to_ascii_lowercase().contains('b') {
                        headers.push(format!("def f(x: {literal}):\n    pass\n"));
                        headers.push(format!("def f() -> {literal}:\n    pass\n"));
                    }
                    for header in headers {
                        let text = format!("def before(): pass\n{header}def after(): pass\n");
                        let result = candidates(&text);
                        assert_ne!(
                            result.files[0].status,
                            CandidateStatus::AcceptedFacts,
                            "{text:?}: {result:?}"
                        );
                        assert_eq!(result.facts.len(), 1, "{text:?}");
                    }
                    let literal = format!("{prefix}{quote}{quote}{quote}{newline}[value](){newline}{quote}{quote}{quote}");
                    accepted(format!("def f(x={literal}):\n    pass\n"));
                    if !prefix.to_ascii_lowercase().contains('b') {
                        accepted(format!("def f(x: {literal}):\n    pass\n"));
                        accepted(format!("def f() -> {literal}:\n    pass\n"));
                    }
                }
            }
        }
    }

    #[test]
    fn retained_expression_values_have_compact_capacity() {
        let result = accepted("def f(x: 'Forward' = -(1), y: T[A] = 'hidden') -> None: pass");
        let mut count = 0;
        for fact in &result.facts {
            if matches!(
                fact.kind.as_str(),
                "parameter_annotation" | "parameter_default" | "return_annotation"
            ) {
                assert_eq!(fact.value.capacity(), fact.value.len(), "{fact:?}");
                count += 1;
            }
        }
        assert_eq!(count, 5);
    }

    #[test]
    fn expression_rejections_are_whole_file_transactions() {
        for annotation in [
            "(A,B)",
            "[A]",
            "{A}",
            "{'a': A}",
            "T[A:B]",
            "T[*A]",
            "A | B",
            "Callable[[A],B]",
            "T[A][B]",
            "T[call()]",
            "call()",
            "f'text'",
            "b'text'",
            "True",
            "123",
            "-1",
            "T[1]",
            "T[(A, (B,C))]",
            "T[A for A in B]",
            "T[A].B",
        ] {
            let result = candidates(format!("def prefix(): pass\ndef f(x: {annotation}): pass"));
            assert_ne!(
                result.files[0].status,
                CandidateStatus::AcceptedFacts,
                "{annotation}: {result:?}"
            );
            assert_eq!(result.facts.len(), 1, "{annotation}");
        }
        for default in [
            "lambda: DEFAULT",
            "[x for x in xs]",
            "{x for x in xs}",
            "{x: x for x in xs}",
            "(x for x in xs)",
            "(x:=value)",
            "[*xs]",
            "{**xs}",
            "await value",
            "(yield value)",
            "value <> other",
            "value.method()",
            "call()",
            "f'text'",
            "f'{call()}'",
            "t'text'",
            "ur'text'",
            "'a' b'b'",
            "b'a' 'b'",
            "r'a\\b'",
            "'a\\n'",
            "b'\u{03bb}'",
            "1L",
            "0xffL",
            "0123",
            "0_1",
            "1_",
            "1__2",
            "0x__1",
            "0b102",
            "1e_2",
            "1._2",
            "1e2_",
            "T[call()]",
        ] {
            let result = candidates(format!("def prefix(): pass\ndef f(x={default}): pass"));
            assert_ne!(
                result.files[0].status,
                CandidateStatus::AcceptedFacts,
                "{default}: {result:?}"
            );
            assert_eq!(result.facts.len(), 1, "{default}");
        }
        for text in [
            "def f(x=1, /, y): pass",
            "def f(x: A=1, y: B): pass",
            "def f(x:A, x:B=2): pass",
            "def f(*x:A, **x:B): pass",
            "def f(**x:A, y:B): pass",
            "def f(*x:A, *y:B): pass",
            "def f(*x:A=1): pass",
            "def f(**x:A=1): pass",
            "def f(x:\tA): pass",
            "def f(x:A): # type: ignore\n pass",
            "def f(x:A\n): pass",
            "def f() -> (\n A\n): pass",
        ] {
            let result = candidates(text);
            assert_ne!(
                result.files[0].status,
                CandidateStatus::AcceptedFacts,
                "{text}: {result:?}"
            );
            assert_eq!(result.facts.len(), 1);
        }
        accepted("def f(x=1, /, y=2, *, required, z: T): pass");
        accepted("def f(x: T, /, y: T=2, *args: T, required: T, **kw: T): pass");
        assert_ne!(
            candidates(format!("def f(x={}): pass", "1".repeat(513))).files[0].status,
            CandidateStatus::AcceptedFacts
        );
    }

    #[test]
    fn shape_limits_do_not_hide_unsupported_descendants() {
        let tuple = std::iter::repeat_n("A", 17).collect::<Vec<_>>().join(",");
        let result = accepted(format!("def f(x:T[{tuple}]): pass"));
        assert_eq!(
            result
                .facts
                .iter()
                .find(|f| f.kind == "parameter_annotation")
                .unwrap()
                .value,
            format!(
                "T[[{}]]",
                std::iter::repeat_n("A", 16).collect::<Vec<_>>().join(", ")
            )
        );
        for shape in [
            format!("T[{tuple}, call()]"),
            format!("{}call(){}", "T[".repeat(15), "]".repeat(15)),
            format!(
                "T[{}, T[{}]]",
                std::iter::repeat_n("T[A,A,A,A,A,A]", 15)
                    .collect::<Vec<_>>()
                    .join(","),
                "call()"
            ),
            format!("{}.T[call()]", "A".repeat(600)),
        ] {
            let result = candidates(format!("def f(x:{shape}): pass"));
            assert_eq!(
                result.files[0].status,
                CandidateStatus::UnsupportedSyntax,
                "{shape}: {result:?}"
            );
            assert_eq!(result.facts.len(), 1);
        }
        for (shape, expected) in [
            (
                format!("{}A{}", "T[".repeat(14), "]".repeat(14)),
                format!("{}...[...]{}", "T[".repeat(12), "]".repeat(12)),
            ),
            (format!("{}.B", "A".repeat(600)), "A".repeat(512)),
        ] {
            let result = accepted(format!("def f(x:{shape}): pass"));
            assert_eq!(
                result
                    .facts
                    .iter()
                    .find(|f| f.kind == "parameter_annotation")
                    .unwrap()
                    .value,
                expected
            );
        }
        let shape = std::iter::repeat_n("T[A,A,A,A,A,A]", 16)
            .collect::<Vec<_>>()
            .join(",");
        let result = accepted(format!("def f(x:T[{shape}]): pass"));
        let expected = format!(
            "T[[{}, T[[A, A, ..., ..., ..., ...]], {}]]",
            std::iter::repeat_n("T[[A, A, A, A, A, A]]", 8)
                .collect::<Vec<_>>()
                .join(", "),
            std::iter::repeat_n("...", 7).collect::<Vec<_>>().join(", ")
        );
        assert_eq!(
            result
                .facts
                .iter()
                .find(|f| f.kind == "parameter_annotation")
                .unwrap()
                .value,
            expected
        );
    }

    #[test]
    fn typed_signatures_keep_exact_count_and_byte_boundaries() {
        for count in [128, 129] {
            let params = (0..count)
                .map(|i| format!("p{i}: T[A] = 1"))
                .collect::<Vec<_>>()
                .join(",");
            let result = accepted(format!("def f({params}): pass"));
            assert_eq!(
                result
                    .facts
                    .iter()
                    .find(|f| f.kind == "signature")
                    .unwrap()
                    .value
                    == "{\"truncated\":true}",
                count == 129
            );
            assert_eq!(
                result
                    .facts
                    .iter()
                    .filter(|f| f.kind == "parameter_annotation")
                    .count(),
                count
            );
            assert_eq!(
                result
                    .facts
                    .iter()
                    .filter(|f| f.kind == "parameter_default")
                    .count(),
                count
            );
        }
        let overhead = "{\"keywordOnly\":[],\"positional\":[\"\"],\"positionalOnly\":0}".len();
        for length in [16384, 16385] {
            let result = accepted(format!(
                "def f({}: T[A] = None): pass",
                "x".repeat(length - overhead)
            ));
            let value = &result
                .facts
                .iter()
                .find(|f| f.kind == "signature")
                .unwrap()
                .value;
            assert_eq!(value == "{\"truncated\":true}", length > 16384);
            if length == 16384 {
                assert_eq!(value.len(), length);
            }
        }
    }

    #[test]
    fn large_literal_bodies_are_borrowed_and_report_stays_redacted() {
        let payload = "private_payload_".repeat(20_000);
        // Fixed alias-arena/render-scratch overhead still fits far below the
        // near-1-MiB literal data. This is only an injected test budget.
        let working = 64 * 1024;
        let sources = [source(
            "a.py",
            "sample",
            format!("def f(x: '{payload}' = r'''{payload}''') -> '{payload}':\n pass"),
        )];
        let result = extract_fact_candidates(&sources).unwrap();
        assert_eq!(result.files[0].status, CandidateStatus::AcceptedFacts);
        assert!(serde_json::to_vec(&result).unwrap().len() < 4096);
        assert!(!serde_json::to_string(&result)
            .unwrap()
            .contains("private_payload_"));
        let result = extract(
            &sources,
            Limits::default(),
            FactLimits {
                working,
                ..FactLimits::default()
            },
            |_| false,
        )
        .unwrap();
        // The near-megabyte source's literal bodies never enter pending storage.
        assert_eq!(
            result.files[0].status,
            CandidateStatus::AcceptedFacts,
            "{result:?}"
        );
        let body_sources = [source(
            "a.py",
            "sample",
            format!("def f():\n x = r'''{payload}'''\n sink(b'''{payload}''')\n return '''{payload}'''\n"),
        )];
        let result = extract(
            &body_sources,
            Limits::default(),
            FactLimits {
                working,
                ..FactLimits::default()
            },
            |_| false,
        )
        .unwrap();
        assert_eq!(
            result.files[0].status,
            CandidateStatus::AcceptedFacts,
            "{result:?}"
        );
        assert!(serde_json::to_vec(&result).unwrap().len() < 4096);
        assert!(!serde_json::to_string(&result.facts)
            .unwrap()
            .contains("private_payload_"));
    }

    #[test]
    fn expression_limits_cancellation_and_single_counted_parse() {
        let sources = [source(
            "a.py",
            "a",
            "def f(x: T[U[A], B] = -(2)) -> T[A,]:\n pass",
        )];
        let parsed = parse_batch(&sources).unwrap();
        let mut parses = 0;
        let mut working = 0;
        let mut collecting = 0;
        let full = extract(
            &sources,
            Limits::default(),
            FactLimits::default(),
            |phase| {
                parses += usize::from(phase == Checkpoint::PostParse);
                if parses > 0 {
                    working += usize::from(phase == Checkpoint::Working);
                    collecting += usize::from(phase == Checkpoint::Collector);
                }
                false
            },
        )
        .unwrap();
        assert_eq!(full.files[0].status, CandidateStatus::AcceptedFacts);
        assert_eq!(parses, 1);
        assert_eq!(full.files[0].nodes_visited, parsed.files[0].nodes_visited);
        assert_eq!(full.files[0].max_depth, parsed.files[0].max_depth);
        for stop in 1..=working {
            let mut visits = 0;
            let result = extract(
                &sources,
                Limits::default(),
                FactLimits::default(),
                |phase| {
                    visits += usize::from(phase == Checkpoint::Working);
                    visits == stop
                },
            )
            .unwrap();
            assert_eq!(result.files[0].status, CandidateStatus::ResourceLimit);
            assert_eq!(result.facts.len(), 1);
        }
        // Every collector checkpoint includes descriptor normalization, lexical
        // validation, and every bounded renderer node, not only CST entry.
        for stop in 1..=collecting {
            let mut visits = 0;
            let result = extract(
                &sources,
                Limits::default(),
                FactLimits::default(),
                |phase| {
                    visits += usize::from(phase == Checkpoint::Collector);
                    visits == stop
                },
            )
            .unwrap();
            assert_eq!(result.files[0].status, CandidateStatus::ResourceLimit);
            assert_eq!(result.facts.len(), 1);
        }
        for limits in [
            FactLimits {
                working: 2048,
                ..FactLimits::default()
            },
            FactLimits {
                working: 12000,
                ..FactLimits::default()
            },
            FactLimits {
                count: 3,
                ..FactLimits::default()
            },
            FactLimits {
                bytes: serde_json::to_vec(&full).unwrap().len() - 1,
                ..FactLimits::default()
            },
        ] {
            let result = extract(&sources, Limits::default(), limits, |_| false).unwrap();
            assert_eq!(
                result.files[0].status,
                CandidateStatus::ResourceLimit,
                "{result:?}"
            );
            assert_eq!(result.facts.len(), 1);
            assert_eq!(result.files[0].nodes_visited, parsed.files[0].nodes_visited);
        }
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
            "def f(x=call()): pass",
            "def f(x: call()): pass",
            "def f() -> call(): pass",
            "@wrap\ndef f(): pass",
            "class C(Base()): pass",
            "import sys,",
            "from x import *",
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
            "def f():\n    try: pass\n    finally: pass",
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
    fn body_facts_calls_bindings_and_literal_invalidations() {
        let text = "'private doc'\nx = (pkg.value)\ny = -2\nself.field = builder(x, option=inner('private'))\ncls.field = None\nregister_url_convertor('private', x)\nalias = register_url_convertor\ndef f(x: T=1) -> T:\n    'private doc'\n    self.field = x\n    cls.field = -1\n    return pkg.run(x, key=helper())\n";
        let result = accepted(text);
        let facts: Vec<_> = result
            .facts
            .iter()
            .filter(|f| matches!(f.kind.as_str(), "call" | "binding" | "binding_invalidation"))
            .map(|f| {
                (
                    f.kind.as_str(),
                    f.line,
                    f.end_line,
                    f.symbol.as_str(),
                    f.value.as_str(),
                )
            })
            .collect();
        assert_eq!(facts, vec![
            ("binding", 2, 2, "sample", "{\"source\":\"pkg.value\",\"target\":\"x\"}"),
            ("binding_invalidation", 3, 3, "sample", "{\"target\":\"y\"}"),
            ("binding", 4, 4, "sample", "{\"source\":\"builder(x, option=inner(<literal>))\",\"target\":\"self.field\"}"),
            ("call", 4, 4, "sample", "builder(x, option=inner(<literal>))"),
            ("call", 4, 4, "sample", "inner(<literal>)"),
            ("binding_invalidation", 5, 5, "sample", "{\"target\":\"cls.field\"}"),
            ("call", 6, 6, "sample", "register_url_convertor(<literal>, x)"),
            ("binding", 7, 7, "sample", "{\"source\":\"register_url_convertor\",\"target\":\"alias\"}"),
            ("binding", 10, 10, "sample.f", "{\"source\":\"x\",\"target\":\"self.field\"}"),
            ("binding_invalidation", 11, 11, "sample.f", "{\"target\":\"cls.field\"}"),
            ("call", 12, 12, "sample.f", "helper()"),
            ("call", 12, 12, "sample.f", "pkg.run(x, key=helper())"),
        ]);
        assert!(!serde_json::to_string(&result)
            .unwrap()
            .contains("private doc"));
        assert!(!serde_json::to_string(&result.facts)
            .unwrap()
            .contains("private"));
        assert!(!result.facts.iter().any(|f| f.kind == "return"));
        for text in [
            "def f(): return",
            "def f(): return x",
            "def f(): helper()",
            "def f(): x=build()",
            "def f(): 'doc'",
            "def f():\n    pass\n    call()",
            "def f():\n    'doc'\n    pass",
            "'doc'",
            "def f():\n    return",
            "def f():\n    x=1",
        ] {
            accepted(text);
        }
    }

    #[test]
    fn body_statement_ranges_are_distinct_from_normalized_expressions() {
        let text = "def outer():\n    def inner():\n        x = (\n            ((helper))(\n                (nested())\n            )\n        ) # trailing\n        return (\n            x\n        )\n        # tail\n    return (\n        inner()\n    ) # final\n# tail\n";
        for text in [
            text.to_owned(),
            format!("\u{feff}{}", text.replace('\n', "\r\n")),
        ] {
            let result = accepted(&text);
            for (kind, symbol, value, line, end) in [
                ("function", "sample.outer", "outer", 1, 14),
                ("function", "sample.outer.inner", "inner", 2, 10),
                (
                    "binding",
                    "sample.outer.inner",
                    "{\"source\":\"helper(nested())\",\"target\":\"x\"}",
                    3,
                    7,
                ),
                ("call", "sample.outer.inner", "helper(nested())", 4, 6),
                ("call", "sample.outer.inner", "nested()", 5, 5),
                ("call", "sample.outer", "inner()", 13, 13),
            ] {
                let fact = result
                    .facts
                    .iter()
                    .find(|f| f.kind == kind && f.symbol == symbol && f.value == value)
                    .unwrap();
                assert_eq!((fact.line, fact.end_line), (line, end), "{fact:?}");
            }
            for fact in result.facts.iter().filter(|f| f.kind == "signature") {
                assert_eq!(
                    fact.end_line,
                    if fact.symbol == "sample.outer" {
                        14
                    } else {
                        10
                    }
                );
            }
        }
        let result = accepted("sink(same(), same())\n");
        assert_eq!(result.facts.iter().filter(|f| f.kind == "call").count(), 2);
    }

    #[test]
    fn every_nested_call_is_emitted_beyond_shape_limits() {
        for count in [16, 17] {
            let positional = (0..count)
                .map(|i| format!("p{i}()"))
                .collect::<Vec<_>>()
                .join(",");
            let keywords = (0..count)
                .map(|i| format!("k{i}=v{i}()"))
                .collect::<Vec<_>>()
                .join(",");
            let result = accepted(format!("outer({positional},{keywords})"));
            let outer = result
                .facts
                .iter()
                .find(|f| f.kind == "call" && f.value.starts_with("outer("))
                .unwrap();
            let mut args = (0..16).map(|i| format!("p{i}()")).collect::<Vec<_>>();
            if count == 17 {
                args.push("...".into());
            }
            args.extend((0..16).map(|i| format!("k{i}=v{i}()")));
            if count == 17 {
                args.push("...".into());
            }
            assert_eq!(outer.value, format!("outer({})", args.join(", ")));
            assert_eq!(
                result.facts.iter().filter(|f| f.kind == "call").count(),
                1 + 2 * count
            );
        }
        let result = accepted(format!(
            "{}leaf(){}",
            (0..15).map(|i| format!("f{i}(")).collect::<String>(),
            ")".repeat(15)
        ));
        assert!(result
            .facts
            .iter()
            .any(|f| f.kind == "call" && f.value == "leaf()"));
        assert!(result
            .facts
            .iter()
            .any(|f| f.kind == "call" && f.value.contains("...")));
        assert_eq!(result.facts.iter().filter(|f| f.kind == "call").count(), 16);
        let expected = format!(
            "{}...(...){}",
            (0..12).map(|i| format!("f{i}(")).collect::<String>(),
            ")".repeat(12)
        );
        assert!(
            result
                .facts
                .iter()
                .any(|f| f.kind == "call" && f.value == expected),
            "{result:?}"
        );
        let result = accepted(format!("{}(hidden())", "a".repeat(600)));
        assert!(result
            .facts
            .iter()
            .any(|f| f.kind == "call" && f.value == "a".repeat(512)));
        assert!(result
            .facts
            .iter()
            .any(|f| f.kind == "call" && f.value == "hidden()"));
        let args = (0..16)
            .map(|i| format!("k{i}=f{i}(a,b,c,d,e,f)"))
            .collect::<Vec<_>>()
            .join(",");
        let result = accepted(format!("callee({args})"));
        let outer = result
            .facts
            .iter()
            .find(|f| f.kind == "call" && f.value.starts_with("callee("))
            .unwrap();
        let expected = format!(
            "callee({}, k9=f9(a, b, c, d, ..., ...), {})",
            (0..9)
                .map(|i| format!("k{i}=f{i}(a, b, c, d, e, f)"))
                .collect::<Vec<_>>()
                .join(", "),
            (10..16)
                .map(|i| format!("k{i}=..."))
                .collect::<Vec<_>>()
                .join(", ")
        );
        assert_eq!(outer.value, expected);
        for fact in result.facts.iter().filter(|f| f.kind == "call") {
            assert_eq!(fact.value.capacity(), fact.value.len());
        }
    }

    #[test]
    fn body_hidden_unsupported_forms_and_helper_side_outputs_rollback() {
        let padding = std::iter::repeat_n("ok()", 17)
            .collect::<Vec<_>>()
            .join(",");
        for expression in [
            "factory()()",
            "factory().run()",
            "f(*xs)",
            "f(**xs)",
            "f(x for x in xs)",
            "f(k=x, y)",
            "f(k=x,k=y)",
            "await f()",
            "lambda: x",
            "[x for x in xs]",
            "(x for x in xs)",
            "x[lambda: y]",
            "a+(lambda: b)",
            "(x:=f())",
            "FastAPI()",
            "pkg.APIRouter()",
            "obj.include_router(x)",
        ] {
            for value in [
                expression.to_owned(),
                format!("outer({padding},{expression})"),
                format!("{}{}{}", "f(".repeat(15), expression, ")".repeat(15)),
                format!("{}(x={expression})", "a".repeat(600)),
            ] {
                let result = candidates(format!(
                    "def before(): pass\nx={value}\ndef after(): pass\n"
                ));
                assert_ne!(
                    result.files[0].status,
                    CandidateStatus::AcceptedFacts,
                    "{value}: {result:?}"
                );
                assert_eq!(result.facts.len(), 1);
            }
        }
        for target in [
            "other.field",
            "self.router",
            "cls.prefix",
            "self.router.child",
            "self.field.router.child",
            "x[0]",
            "x,y",
            "*x",
            "x=y",
        ] {
            let result = candidates(format!("def before(): pass\n{target}=helper()\n"));
            assert_ne!(
                result.files[0].status,
                CandidateStatus::AcceptedFacts,
                "{target}"
            );
            assert_eq!(result.facts.len(), 1);
        }
        let target = format!("self.{}.router.child", "a".repeat(600));
        assert_eq!(
            candidates(format!("{target}=helper()\n")).files[0].status,
            CandidateStatus::UnsupportedSyntax
        );
        for text in [
            "return",
            "return x",
            "x: T | U = helper()",
            "x += helper()",
            "del x",
            "x=y=z",
            "x=(y:=z)",
            "pass; helper()",
        ] {
            let result = candidates(text);
            assert_ne!(
                result.files[0].status,
                CandidateStatus::AcceptedFacts,
                "{text}"
            );
            assert_eq!(result.facts.len(), 1);
        }
    }

    #[test]
    fn body_newline_gating_and_reviewed_literals() {
        for newline in ["\n", "\r\n"] {
            for text in [
                "x =\nhelper()",
                "x = # comment\nhelper()",
                "pkg.\nname",
                "pkg. # comment\nname",
                "def f():\n    return pkg. # comment\n    name",
            ] {
                let result = candidates(text.replace('\n', newline));
                assert_ne!(
                    result.files[0].status,
                    CandidateStatus::AcceptedFacts,
                    "{text}: {result:?}"
                );
                assert_eq!(result.facts.len(), 1);
            }
            for text in [
                "x = (\nhelper()\n)",
                "def f():\n    return # comment\n    helper()",
                "def f():\n    return (\n        helper()\n    )",
                "(pkg. # comment\n name)(\n x,\n key=helper(),\n)",
            ] {
                accepted(text.replace('\n', newline));
            }
            // These are two AST expression statements, not a continued call.
            let result = accepted(format!("f{newline}(x)"));
            assert_eq!(result.facts.len(), 1);
        }
        for expression in [
            "None",
            "True",
            "False",
            "...",
            "+(2)",
            "-(0xF)",
            "1.2e3J",
            "b'a' b'b'",
            "'a' r'b'",
            "r'''private\npayload'''",
            "pkg.name",
            "((name))",
        ] {
            accepted(format!(
                "{expression}\nx={expression}\ndef f():\n    return {expression}\n"
            ));
        }
    }

    #[test]
    fn assignment_target_shapes_clip_names_and_attributes_for_both_binding_kinds() {
        for length in [511, 512, 513] {
            for target in [
                "n".repeat(length),
                format!("self.{}", "n".repeat(length - 5)),
            ] {
                for (rhs, kind, source) in [
                    ("helper()", "binding", "\"source\":\"helper()\","),
                    ("None", "binding_invalidation", ""),
                ] {
                    let result = accepted(format!("{target}={rhs}\n"));
                    let fact = result.facts.iter().find(|f| f.kind == kind).unwrap();
                    assert_eq!(
                        fact.value,
                        format!("{{{source}\"target\":\"{}\"}}", &target[..length.min(512)])
                    );
                    assert_eq!(fact.value.capacity(), fact.value.len());
                }
            }
        }
        let name = "n".repeat(20_000);
        let sources = [source("sample.py", "sample", format!("{name}=helper()\n"))];
        let result = extract(
            &sources,
            Limits::default(),
            FactLimits {
                working: 20_000,
                ..FactLimits::default()
            },
            |_| false,
        )
        .unwrap();
        assert_eq!(result.files[0].status, CandidateStatus::AcceptedFacts);
        let binding = result.facts.iter().find(|f| f.kind == "binding").unwrap();
        assert_eq!(
            binding.value,
            format!(
                "{{\"source\":\"helper()\",\"target\":\"{}\"}}",
                &name[..512]
            )
        );
        for working in [2048, 8192] {
            let result = extract(
                &sources,
                Limits::default(),
                FactLimits {
                    working,
                    ..FactLimits::default()
                },
                |_| false,
            )
            .unwrap();
            assert_eq!(result.files[0].status, CandidateStatus::ResourceLimit);
            assert_eq!(
                result.files[0].diagnostic,
                Some(CandidateDiagnostic::WorkingLimit)
            );
            assert_eq!(result.facts.len(), 1);
        }
    }

    #[test]
    fn standalone_callee_clipping_cannot_hide_helper_side_outputs() {
        let padding = std::iter::repeat_n("ok()", 17)
            .collect::<Vec<_>>()
            .join(",");
        let exhausted = (0..16)
            .map(|i| format!("k{i}=f(a,b,c,d,e,f)"))
            .collect::<Vec<_>>()
            .join(",");
        for name in ["APIRouter", "FastAPI"] {
            let prefix_length = 512 - name.len() - 1;
            let call = format!("{}.{}Extra()", "a".repeat(prefix_length), name);
            for rhs in [
                call.clone(),
                format!("outer({padding},{call})"),
                format!("outer({exhausted},bad={call})"),
                format!("{}{}{}", "f(".repeat(15), call, ")".repeat(15)),
            ] {
                let result =
                    candidates(format!("def before(): pass\nx={rhs}\ndef after(): pass\n"));
                assert_eq!(
                    result.files[0].status,
                    CandidateStatus::UnsupportedSyntax,
                    "{name}: {result:?}"
                );
                assert_eq!(result.facts.len(), 1);
            }
            // Adjacent clipping boundaries do not create the special name.
            for length in [prefix_length - 1, prefix_length + 1] {
                accepted(format!("x={}.{}Extra()\n", "a".repeat(length), name));
            }
        }
    }

    #[test]
    fn standalone_callee_scratch_is_reserved_before_the_exit_renderer() {
        let sources = [source("sample.py", "sample", "probe()\n")];
        let frame_slots = parse_batch(&sources).unwrap().files[0].max_depth + 1;
        let mut reservations = 0;
        let result = extract(
            &sources,
            Limits::default(),
            FactLimits {
                working: 4 * std::mem::size_of::<AliasScope>()
                    + frame_slots * 4 * std::mem::size_of::<Frame>()
                    + 2 * std::mem::size_of::<BodyStatement>()
                    + 5 * Expression::ENTRY_WORK
                    + Expression::RENDER_WORK
                    - 1,
                ..FactLimits::default()
            },
            |phase| {
                reservations += usize::from(phase == Checkpoint::Working);
                false
            },
        )
        .unwrap();
        // Root scope, statement, five arena entries and the counted frame
        // high-water reservations precede the call-exit scratch allocation.
        assert_eq!(reservations, 8 + frame_slots);
        assert_eq!(result.files[0].status, CandidateStatus::ResourceLimit);
        assert_eq!(
            result.files[0].diagnostic,
            Some(CandidateDiagnostic::WorkingLimit)
        );
        assert_eq!(result.facts.len(), 1);
    }

    #[test]
    fn delimiter_nesting_uses_global_anonymous_punctuation_depth() {
        for depth in [200, 201] {
            let text = format!(
                "def before(): pass\nx={}value{}\ndef after(): pass\n",
                "(".repeat(depth),
                ")".repeat(depth)
            );
            let sources = [source("sample.py", "sample", &text)];
            let parsed = parse_batch(&sources).unwrap();
            assert_eq!(parsed.files[0].status, ParseStatus::SyntaxAccepted);
            let result = candidates(&text);
            assert_eq!(result.files[0].nodes_visited, parsed.files[0].nodes_visited);
            assert_eq!(result.files[0].max_depth, parsed.files[0].max_depth);
            assert!(result.files[0].traversal_finished);
            assert_eq!(
                result.files[0].status,
                if depth == 200 {
                    CandidateStatus::AcceptedFacts
                } else {
                    CandidateStatus::UnsupportedSyntax
                }
            );
            if depth == 201 {
                assert_eq!(result.facts.len(), 1);
                let malformed = candidates(format!("{text}def broken(:\n pass\n"));
                assert_eq!(malformed.files[0].status, CandidateStatus::SyntaxError);
                assert_eq!(malformed.facts.len(), 1);
                let limited = extract(
                    &sources,
                    Limits {
                        nodes: parsed.files[0].nodes_visited - 1,
                        ..Limits::default()
                    },
                    FactLimits::default(),
                    |_| false,
                )
                .unwrap();
                assert_eq!(limited.files[0].status, CandidateStatus::ResourceLimit);
                assert_eq!(
                    limited.files[0].diagnostic,
                    Some(CandidateDiagnostic::Parse(DiagnosticCode::NodeLimit))
                );
                assert_eq!(limited.facts.len(), 1);
            }
        }
        for inner in [199, 200] {
            let groups = format!("{}value{}", "(".repeat(inner), ")".repeat(inner));
            for text in [
                format!("x=helper({groups})\n"),
                format!("def f(x: {groups}): pass\n"),
                format!(
                    "def f(x={}1{}): pass\n",
                    "(".repeat(inner),
                    ")".repeat(inner)
                ),
            ] {
                let result = candidates(format!("before=helper()\n{text}"));
                assert_eq!(
                    result.files[0].status,
                    if inner == 199 {
                        CandidateStatus::AcceptedFacts
                    } else {
                        CandidateStatus::UnsupportedSyntax
                    },
                    "{text}: {result:?}"
                );
                if inner == 200 {
                    assert_eq!(result.facts.len(), 1);
                }
            }
        }
        for inner in [198, 199] {
            let result = candidates(format!(
                "def f(x: T[{}A{}]): pass\n",
                "(".repeat(inner),
                ")".repeat(inner)
            ));
            assert_eq!(
                result.files[0].status,
                if inner == 198 {
                    CandidateStatus::AcceptedFacts
                } else {
                    CandidateStatus::UnsupportedSyntax
                }
            );
            if inner == 199 {
                assert_eq!(result.facts.len(), 1);
            }
        }
        accepted(format!(
            "# {}\nx='{}'\ny=helper()\n",
            "(".repeat(201),
            ")".repeat(201)
        ));
    }

    #[test]
    fn body_limits_share_single_parse_walk_and_all_rollback_guards() {
        let sources = [source("sample.py", "sample", "def f(x: T = 1):\n    self.field=outer(x, key=inner())\n    return (\n        other()\n    )\n")];
        let parsed = parse_batch(&sources).unwrap();
        let mut parses = 0;
        let full = extract(
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
        assert_eq!(full.files[0].status, CandidateStatus::AcceptedFacts);
        assert_eq!(full.files[0].nodes_visited, parsed.files[0].nodes_visited);
        assert_eq!(full.files[0].max_depth, parsed.files[0].max_depth);
        for limits in [
            FactLimits {
                working: 2048,
                ..FactLimits::default()
            },
            FactLimits {
                count: 5,
                ..FactLimits::default()
            },
            FactLimits {
                bytes: serde_json::to_vec(&full).unwrap().len() - 1,
                ..FactLimits::default()
            },
        ] {
            let result = extract(&sources, Limits::default(), limits, |_| false).unwrap();
            assert_eq!(result.files[0].status, CandidateStatus::ResourceLimit);
            assert_eq!(result.facts.len(), 1);
        }
        for checkpoint in [
            Checkpoint::Working,
            Checkpoint::Collector,
            Checkpoint::FactId,
            Checkpoint::Output,
            Checkpoint::Sort,
            Checkpoint::FinalOutput,
        ] {
            let mut total = 0;
            extract(
                &sources,
                Limits::default(),
                FactLimits::default(),
                |phase| {
                    total += usize::from(phase == checkpoint);
                    false
                },
            )
            .unwrap();
            for stop in 1..=total {
                let mut visits = 0;
                let result = extract(
                    &sources,
                    Limits::default(),
                    FactLimits::default(),
                    |phase| {
                        visits += usize::from(phase == checkpoint);
                        visits == stop
                    },
                )
                .unwrap();
                assert_eq!(
                    result.files[0].status,
                    CandidateStatus::ResourceLimit,
                    "{checkpoint:?} {stop}"
                );
                assert_eq!(result.facts.len(), 1);
            }
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
            source("z.py", "z", "def f(): pass\nclass C(Base()): pass"),
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
        let sources = [source("a.py", "a", "class C(Base()): pass\ndef broken(:\n")];
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
