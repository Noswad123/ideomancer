//! Bounded captured-byte parsing and private transactional declaration candidates.
//! No tree, raw source excerpt, or executable target behavior escapes this module.
//! Neither entry point is a production Python backend or trusted Evidence.

use super::Source;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::io::{self, Write};
use std::time::{Duration, Instant};
use tree_sitter::{ParseOptions, Parser};

mod facts;
pub(super) use facts::CandidateReport;
#[allow(unused_imports)] // Private oracle hook; not a production capability.
pub(super) use facts::CandidateStatus;

pub(super) fn extract_fact_candidates(
    sources: &[Source],
) -> Result<CandidateReport<'_>, FrontEndError> {
    facts::extract_fact_candidates(sources)
}

trait Observer {
    fn begin(&mut self) {}
    fn enter<F: FnMut(Checkpoint) -> bool>(
        &mut self,
        _node: tree_sitter::Node<'_>,
        _field: Option<&str>,
        _bytes: &[u8],
        _cancel: &Cancellation<F>,
    ) {
    }
    fn exit<F: FnMut(Checkpoint) -> bool>(
        &mut self,
        _node: tree_sitter::Node<'_>,
        _cancel: &Cancellation<F>,
    ) {
    }
    fn finish(&mut self, _summary: &FileSummary<'_>) {}
}

struct ParseOnly;
impl Observer for ParseOnly {}

const POLICY: &str = "python-native-parse-only-1";
const MAX_METADATA: usize = 4096;
const MAX_REPORT: usize = 1024 * 1024;

#[derive(Debug, Serialize)]
pub(super) struct BatchParserIdentity {
    pub(super) runtime: &'static str,
    pub(super) grammar: &'static str,
    pub(super) language: &'static str,
    pub(super) policy: &'static str,
    pub(super) sha256: String,
}

#[derive(Debug, Serialize)]
pub(super) struct BatchReport<'a> {
    pub(super) identity: BatchParserIdentity,
    pub(super) files: Vec<FileSummary<'a>>,
}

#[derive(Debug, Serialize)]
pub(super) struct FileSummary<'a> {
    pub(super) path: &'a str,
    pub(super) module: &'a str,
    pub(super) sha256: String,
    pub(super) status: ParseStatus,
    pub(super) diagnostic: Option<DiagnosticCode>,
    pub(super) nodes_visited: usize,
    /// Root depth is zero; this is bounded even when the next node is too deep.
    pub(super) max_depth: usize,
    /// Only the CST walk finished, not evidence extraction or semantic checking.
    pub(super) traversal_finished: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) enum ParseStatus {
    SyntaxAccepted,
    SyntaxError,
    UnsupportedEncoding,
    ResourceLimit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) enum DiagnosticCode {
    SyntaxError,
    EncodingCookieUnsupported,
    InvalidUtf8,
    Deadline,
    NodeLimit,
    DepthLimit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FrontEndError {
    FileCount,
    FileBytes,
    TotalBytes,
    InvalidPath,
    DuplicatePath,
    ModuleLength,
    ReportLimit,
    GrammarInitialization,
    ParserFailure,
}

#[derive(Clone, Copy)]
struct Limits {
    files: usize,
    file_bytes: usize,
    total_bytes: usize,
    report: usize,
    nodes: usize,
    depth: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            files: super::MAX_FILES,
            file_bytes: super::MAX_FILE,
            total_bytes: super::MAX_TOTAL,
            report: MAX_REPORT,
            nodes: 200_000,
            depth: 256,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Checkpoint {
    Preflight,
    Identity,
    Hash,
    Preprocess,
    Input,
    Progress,
    PostParse,
    Walk,
    Final,
    Collector,
    FactId,
    Signature,
    Sort,
    Output,
    FinalOutput,
    Baseline,
    Working,
    FunctionFrame,
}

// The latch is independent of Option<Tree>: input cancellation can look like EOF
// and yield a non-None truncated tree. Both parser callbacks share this latch.
struct Cancellation<F> {
    expired: Cell<bool>,
    check: RefCell<F>,
}

impl<F: FnMut(Checkpoint) -> bool> Cancellation<F> {
    fn at(&self, checkpoint: Checkpoint) -> bool {
        if (self.check.borrow_mut())(checkpoint) {
            self.expired.set(true);
        }
        self.expired.get()
    }
}

/// One deadline for validation, identity, hashing, parsing, and traversal.
/// Native parser allocations are not a hard memory sandbox.
pub(super) fn parse_batch(sources: &[Source]) -> Result<BatchReport<'_>, FrontEndError> {
    let start = Instant::now();
    run(sources, Limits::default(), |_| {
        start.elapsed() >= Duration::from_secs(60)
    })
}

struct CountWriter {
    count: usize,
    limit: usize,
}

impl Write for CountWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.count = self
            .count
            .checked_add(bytes.len())
            .filter(|&n| n <= self.limit)
            .ok_or_else(|| io::Error::other("report limit"))?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn json_size(value: &impl Serialize, limit: usize) -> Result<usize, FrontEndError> {
    let mut writer = CountWriter { count: 0, limit };
    serde_json::to_writer(&mut writer, value).map_err(|_| FrontEndError::ReportLimit)?;
    Ok(writer.count)
}

fn digest<F: FnMut(Checkpoint) -> bool>(
    bytes: &[u8],
    cancel: &Cancellation<F>,
    checkpoint: Checkpoint,
) -> String {
    let mut hash = Sha256::new();
    // Even expired entries retain their original-byte digest. Total hashing work
    // is input-bounded, and deadline checks never allow subsequent parsing.
    for chunk in bytes.chunks(4096) {
        cancel.at(checkpoint);
        hash.update(chunk);
    }
    cancel.at(checkpoint);
    format!("{:x}", hash.finalize())
}

fn identity<F: FnMut(Checkpoint) -> bool>(cancel: &Cancellation<F>) -> BatchParserIdentity {
    let mut identity = identity_header();
    // Structured pins and length-framed text prevent ambiguous concatenation.
    let pins = serde_json::to_vec(&identity).expect("fixed identity serialization");
    let mut hash = Sha256::new();
    for part in [
        pins.as_slice(),
        tree_sitter_python::NODE_TYPES.as_bytes(),
        include_bytes!("python_native.rs").as_slice(),
    ] {
        hash.update((part.len() as u64).to_le_bytes());
        for chunk in part.chunks(4096) {
            cancel.at(Checkpoint::Identity);
            hash.update(chunk);
        }
    }
    identity.sha256 = format!("{:x}", hash.finalize());
    identity
}

fn identity_header() -> BatchParserIdentity {
    BatchParserIdentity {
        runtime: "0.25.10",
        grammar: "0.25.0",
        language: "0.1.5",
        policy: POLICY,
        sha256: String::new(),
    }
}

fn valid_path(path: &str) -> bool {
    if path.is_empty() || path.len() > MAX_METADATA || path.contains(['\\', '\0']) {
        return false;
    }
    if matches!(path.as_bytes(), [drive, b':', ..] if drive.is_ascii_alphabetic()) {
        return false;
    }
    path.split('/').all(|p| !matches!(p, "" | "." | ".."))
}

fn run<F: FnMut(Checkpoint) -> bool>(
    sources: &[Source],
    limits: Limits,
    check: F,
) -> Result<BatchReport<'_>, FrontEndError> {
    let cancel = Cancellation {
        expired: Cell::new(false),
        check: RefCell::new(check),
    };
    run_observed(sources, limits, &cancel, &mut ParseOnly)
}

fn run_observed<'a, F: FnMut(Checkpoint) -> bool, O: Observer>(
    sources: &'a [Source],
    limits: Limits,
    cancel: &Cancellation<F>,
    observer: &mut O,
) -> Result<BatchReport<'a>, FrontEndError> {
    admit(sources, limits, cancel)?;
    run_admitted(sources, limits, cancel, observer)
}

fn admit<F: FnMut(Checkpoint) -> bool>(
    sources: &[Source],
    limits: Limits,
    cancel: &Cancellation<F>,
) -> Result<(), FrontEndError> {
    cancel.at(Checkpoint::Preflight);
    if sources.len() > limits.files {
        return Err(FrontEndError::FileCount);
    }
    let mut paths = BTreeSet::new();
    let mut total = 0usize;
    for source in sources {
        cancel.at(Checkpoint::Preflight);
        if !valid_path(&source.path) {
            return Err(FrontEndError::InvalidPath);
        }
        if !paths.insert(source.path.as_str()) {
            return Err(FrontEndError::DuplicatePath);
        }
        // Empty scanner module names are harmless metadata, never identifiers.
        if source.module.len() > MAX_METADATA {
            return Err(FrontEndError::ModuleLength);
        }
        if source.bytes.len() > limits.file_bytes {
            return Err(FrontEndError::FileBytes);
        }
        total = total
            .checked_add(source.bytes.len())
            .filter(|&n| n <= limits.total_bytes)
            .ok_or(FrontEndError::TotalBytes)?;
    }
    let mut header = identity_header();
    header.sha256 = "0".repeat(64);
    let report = BatchReport {
        identity: header,
        files: Vec::new(),
    };
    let mut reserved = json_size(&report, limits.report)?;
    for (index, source) in sources.iter().enumerate() {
        cancel.at(Checkpoint::Preflight);
        // Longest enum spellings, largest counters, false (longer than true),
        // and fixed digest length reserve every possible per-file outcome.
        let worst = FileSummary {
            path: &source.path,
            module: &source.module,
            sha256: "0".repeat(64),
            status: ParseStatus::UnsupportedEncoding,
            diagnostic: Some(DiagnosticCode::EncodingCookieUnsupported),
            nodes_visited: usize::MAX,
            max_depth: usize::MAX,
            traversal_finished: false,
        };
        reserved = reserved
            .checked_add(json_size(&worst, limits.report)?)
            .and_then(|n| n.checked_add(usize::from(index != 0)))
            .filter(|&n| n <= limits.report)
            .ok_or(FrontEndError::ReportLimit)?;
    }
    Ok(())
}

fn run_admitted<'a, F: FnMut(Checkpoint) -> bool, O: Observer>(
    sources: &'a [Source],
    limits: Limits,
    cancel: &Cancellation<F>,
    observer: &mut O,
) -> Result<BatchReport<'a>, FrontEndError> {
    let mut report = BatchReport {
        identity: identity(cancel),
        files: Vec::new(),
    };
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .map_err(|_| FrontEndError::GrammarInitialization)?;
    for source in sources {
        observer.begin();
        let mut summary = FileSummary {
            path: &source.path,
            module: &source.module,
            sha256: digest(&source.bytes, cancel, Checkpoint::Hash),
            status: ParseStatus::ResourceLimit,
            diagnostic: Some(DiagnosticCode::Deadline),
            nodes_visited: 0,
            max_depth: 0,
            traversal_finished: false,
        };
        if !cancel.at(Checkpoint::Preprocess) {
            parse_file_observed(
                &mut parser,
                &source.bytes,
                &mut summary,
                limits,
                cancel,
                observer,
            )?;
        }
        if cancel.at(Checkpoint::Final) {
            summary.status = ParseStatus::ResourceLimit;
            summary.diagnostic = Some(DiagnosticCode::Deadline);
        }
        observer.finish(&summary);
        report.files.push(summary);
    }
    // The reservation is conservative; enforce the exact budget as well.
    json_size(&report, limits.report)?;
    // A deadline reached during final serialization must not leave accepted
    // entries behind. No parse is trusted after the invocation deadline.
    if cancel.at(Checkpoint::Final) {
        for summary in &mut report.files {
            summary.status = ParseStatus::ResourceLimit;
            summary.diagnostic = Some(DiagnosticCode::Deadline);
        }
    }
    Ok(report)
}

fn parse_file<F: FnMut(Checkpoint) -> bool>(
    parser: &mut Parser,
    raw: &[u8],
    summary: &mut FileSummary<'_>,
    limits: Limits,
    cancel: &Cancellation<F>,
) -> Result<(), FrontEndError> {
    parse_file_observed(parser, raw, summary, limits, cancel, &mut ParseOnly)
}

fn parse_file_observed<F: FnMut(Checkpoint) -> bool, O: Observer>(
    parser: &mut Parser,
    raw: &[u8],
    summary: &mut FileSummary<'_>,
    limits: Limits,
    cancel: &Cancellation<F>,
    observer: &mut O,
) -> Result<(), FrontEndError> {
    let bytes = raw.strip_prefix(b"\xef\xbb\xbf").unwrap_or(raw);
    // Cookie policy is byte-based and takes precedence over UTF-8 validation:
    // declared non-UTF-8 payloads need the future transcoding gate, not fallback.
    if encoding_cookie(bytes) {
        summary.status = ParseStatus::UnsupportedEncoding;
        summary.diagnostic = Some(DiagnosticCode::EncodingCookieUnsupported);
        return Ok(());
    }
    if std::str::from_utf8(bytes).is_err() {
        summary.status = ParseStatus::UnsupportedEncoding;
        summary.diagnostic = Some(DiagnosticCode::InvalidUtf8);
        return Ok(());
    }
    if cancel.at(Checkpoint::Preprocess) {
        return Ok(());
    }
    parser.reset();
    let mut input = |offset: usize, _| {
        if cancel.at(Checkpoint::Input) {
            &[][..]
        } else {
            // Small borrowed chunks give cancellation a regular input boundary.
            bytes
                .get(offset..offset.saturating_add(4096).min(bytes.len()))
                .unwrap_or(&[])
        }
    };
    let mut progress = |_: &tree_sitter::ParseState| cancel.at(Checkpoint::Progress);
    let tree = parser.parse_with_options(
        &mut input,
        None,
        Some(ParseOptions::new().progress_callback(&mut progress)),
    );
    if cancel.at(Checkpoint::PostParse) {
        return Ok(());
    }
    let tree = tree.ok_or(FrontEndError::ParserFailure)?;
    let mut cursor = tree.walk();
    let mut depth = 0usize;
    let mut syntax_error = false;
    loop {
        if cancel.at(Checkpoint::Walk) {
            break;
        }
        if summary.nodes_visited >= limits.nodes {
            summary.diagnostic = Some(DiagnosticCode::NodeLimit);
            break;
        }
        if depth > limits.depth {
            summary.diagnostic = Some(DiagnosticCode::DepthLimit);
            break;
        }
        summary.nodes_visited += 1;
        summary.max_depth = summary.max_depth.max(depth);
        let node = cursor.node();
        syntax_error |= node.is_error() || node.is_missing();
        observer.enter(node, cursor.field_name(), bytes, cancel);
        if cursor.goto_first_child() {
            depth += 1;
            continue;
        }
        loop {
            observer.exit(cursor.node(), cancel);
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                summary.traversal_finished = true;
                break;
            }
            depth -= 1;
            if cancel.at(Checkpoint::Walk) {
                break;
            }
        }
        if summary.traversal_finished || cancel.expired.get() {
            break;
        }
    }
    if summary.traversal_finished && !cancel.at(Checkpoint::Final) {
        summary.status = if syntax_error {
            ParseStatus::SyntaxError
        } else {
            ParseStatus::SyntaxAccepted
        };
        summary.diagnostic = syntax_error.then_some(DiagnosticCode::SyntaxError);
    }
    // cursor and tree drop here, before the next file is parsed.
    Ok(())
}

// Byte equivalent of ^[ \t\f]*#.*?coding[:=][ \t]*([-\w.]+).
// Only the first two physical lines participate; the second is eligible only
// when the first is blank or a comment (including a shebang). Match Python's
// physical LF/CRLF/bare-CR boundaries, without normalizing the captured bytes.
fn encoding_cookie(bytes: &[u8]) -> bool {
    fn physical_line(bytes: &[u8]) -> (&[u8], &[u8]) {
        let Some(end) = bytes.iter().position(|&b| matches!(b, b'\r' | b'\n')) else {
            return (bytes, &[]);
        };
        let newline = if bytes[end] == b'\r' && bytes.get(end + 1) == Some(&b'\n') {
            2
        } else {
            1
        };
        (&bytes[..end], &bytes[end + newline..])
    }

    fn indentation(byte: u8) -> bool {
        matches!(byte, b' ' | b'\t' | 0x0c)
    }
    fn cookie(line: &[u8]) -> bool {
        if line.first() != Some(&b'#') {
            return false;
        }
        line.windows(6).enumerate().any(|(index, window)| {
            if window != b"coding" || !matches!(line.get(index + 6), Some(b':' | b'=')) {
                return false;
            }
            let rest = &line[index + 7..];
            let skip = rest
                .iter()
                .take_while(|&&b| matches!(b, b' ' | b'\t'))
                .count();
            rest.get(skip)
                .is_some_and(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
        })
    }
    let (first, remaining) = physical_line(bytes);
    let first = &first[first.iter().take_while(|&&b| indentation(b)).count()..];
    let valid_comment = |line: &[u8]| {
        let line = &line[line.iter().take_while(|&&b| indentation(b)).count()..];
        line.first() == Some(&b'#') && cookie(line)
    };
    valid_comment(first)
        || ((first.is_empty() || first.first() == Some(&b'#'))
            && valid_comment(physical_line(remaining).0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(path: &str, bytes: impl AsRef<[u8]>) -> Source {
        Source {
            path: path.into(),
            module: "synthetic".into(),
            // Deliberately unusable transport: native parsing must ignore it.
            hex: "not hexadecimal".into(),
            bytes: bytes.as_ref().to_vec(),
        }
    }

    fn status(bytes: impl AsRef<[u8]>) -> (ParseStatus, Option<DiagnosticCode>) {
        let sources = [source("sample.py", bytes)];
        let report = parse_batch(&sources).unwrap();
        (report.files[0].status, report.files[0].diagnostic)
    }

    #[test]
    fn empty_and_valid_batches_have_pinned_identity() {
        let empty = parse_batch(&[]).unwrap();
        assert!(empty.files.is_empty());
        assert_eq!(empty.identity.runtime, "0.25.10");
        assert_eq!(empty.identity.grammar, "0.25.0");
        assert_eq!(empty.identity.language, "0.1.5");
        assert_eq!(empty.identity.policy, POLICY);
        assert_eq!(empty.identity.sha256.len(), 64);
        assert_eq!(
            empty.identity.sha256,
            parse_batch(&[]).unwrap().identity.sha256
        );
        for bytes in ["", "x = 1\n", "def f(x):\n    return x + 1\n"] {
            let sources = [source("sample.py", bytes)];
            let report = parse_batch(&sources).unwrap();
            let file = &report.files[0];
            assert_eq!(file.status, ParseStatus::SyntaxAccepted);
            assert!(file.traversal_finished);
            assert!(file.nodes_visited > 0);
            assert_eq!(file.diagnostic, None);
        }
    }

    #[test]
    fn recovered_errors_and_missing_tokens_are_not_accepted() {
        for bytes in ["def f(:\n    pass\n", "if True\n    pass\n"] {
            let sources = [source("bad.py", bytes)];
            let report = parse_batch(&sources).unwrap();
            assert_eq!(report.files[0].status, ParseStatus::SyntaxError);
            assert_eq!(
                report.files[0].diagnostic,
                Some(DiagnosticCode::SyntaxError)
            );
            assert!(report.files[0].traversal_finished);
        }
        // Verify missing-node recovery separately from explicit ERROR recovery.
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        let fixture = "@wrap(key=1)\nclass C(Base):\n    def f(self, a: int = 1, *, k=None) -> str:\n        if a:\n            return f'{a!r:>8}'\n        return [x for x in (1, 2) if x]\n\ntry:\n    from x import y as z\nexcept ValueError as e:\n    raise e\n";
        let mut missing_count = 0;
        // Deterministic deletion mutations cover recovery with lookahead after
        // the missing token, unlike an EOF-only truncation (usually ERROR).
        for index in 0..fixture.len() {
            let mut bytes = fixture.to_owned();
            bytes.remove(index);
            let tree = parser.parse(&bytes, None).unwrap();
            let mut cursor = tree.walk();
            let mut missing = false;
            'walk: loop {
                missing |= cursor.node().is_missing();
                if cursor.goto_first_child() {
                    continue;
                }
                while !cursor.goto_next_sibling() {
                    if !cursor.goto_parent() {
                        break 'walk;
                    }
                }
            }
            if missing {
                missing_count += 1;
                assert_eq!(status(&bytes).0, ParseStatus::SyntaxError);
            }
        }
        assert!(missing_count > 0);
    }

    #[test]
    fn batch_files_are_isolated_and_metadata_is_borrowed() {
        let mut sources = [source("a.py", "x = (\n"), source("b.py", "pass\n")];
        sources[1].module.clear();
        let report = parse_batch(&sources).unwrap();
        assert_eq!(report.files[0].status, ParseStatus::SyntaxError);
        assert_eq!(report.files[1].status, ParseStatus::SyntaxAccepted);
        assert_eq!(report.files[1].path.as_ptr(), sources[1].path.as_ptr());
        assert_eq!(report.files[1].module, "");
    }

    #[test]
    fn input_limits_fail_with_small_injected_budgets() {
        let sources = [source("a.py", "pass"), source("b.py", "pass")];
        for (limits, expected) in [
            (
                Limits {
                    files: 1,
                    ..Limits::default()
                },
                FrontEndError::FileCount,
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
            assert_eq!(run(&sources, limits, |_| false).unwrap_err(), expected);
        }
        let report = run(
            &sources,
            Limits {
                files: 2,
                file_bytes: 4,
                total_bytes: 8,
                ..Limits::default()
            },
            |_| false,
        )
        .unwrap();
        assert_eq!(report.files.len(), 2);
    }

    #[test]
    fn paths_duplicates_and_module_bounds_are_checked() {
        for path in [
            "",
            "/a.py",
            "a\\b.py",
            "a//b.py",
            "./a.py",
            "a/../b.py",
            "a/./b.py",
            "a/",
            "C:/a.py",
            "a\0.py",
        ] {
            assert_eq!(
                parse_batch(&[source(path, "")]).unwrap_err(),
                FrontEndError::InvalidPath
            );
        }
        assert_eq!(
            parse_batch(&[source(&"p".repeat(MAX_METADATA + 1), "")]).unwrap_err(),
            FrontEndError::InvalidPath
        );
        assert_eq!(
            parse_batch(&[source("a.py", ""), source("a.py", "pass")]).unwrap_err(),
            FrontEndError::DuplicatePath
        );
        let mut sources = [source("a.py", "")];
        sources[0].module = "m".repeat(MAX_METADATA + 1);
        assert_eq!(
            parse_batch(&sources).unwrap_err(),
            FrontEndError::ModuleLength
        );
        sources[0].module.pop();
        assert!(parse_batch(&sources).is_ok());
    }

    #[test]
    fn report_reservation_counts_escaped_metadata_before_parsing() {
        let plain = [source(&"p".repeat(80), "pass")];
        let escaped = [source(&"\u{0001}".repeat(80), "pass")];
        let limits = Limits {
            report: 800,
            ..Limits::default()
        };
        assert!(run(&plain, limits, |_| false).is_ok());
        let mut entered_parser = false;
        let result = run(&escaped, limits, |phase| {
            entered_parser |= phase == Checkpoint::Input;
            false
        });
        assert_eq!(result.unwrap_err(), FrontEndError::ReportLimit);
        assert!(!entered_parser);
        assert_eq!(
            run(
                &[],
                Limits {
                    report: 1,
                    ..limits
                },
                |_| false
            )
            .unwrap_err(),
            FrontEndError::ReportLimit
        );
        let report = parse_batch(&plain).unwrap();
        let exact = serde_json::to_vec(&report).unwrap().len();
        assert_eq!(json_size(&report, exact).unwrap(), exact);
        assert_eq!(
            json_size(&report, exact - 1),
            Err(FrontEndError::ReportLimit)
        );
        let flood: Vec<_> = (0..5)
            .map(|n| source(&format!("{n}{}", "\u{0001}".repeat(80)), ""))
            .collect();
        assert_eq!(
            run(
                &flood,
                Limits {
                    report: 2000,
                    ..limits
                },
                |_| false
            )
            .unwrap_err(),
            FrontEndError::ReportLimit
        );
    }

    #[test]
    fn bom_crlf_unicode_and_original_byte_digest() {
        let text = "name = '\u{03bb}'\r\n";
        let mut bom = b"\xef\xbb\xbf".to_vec();
        bom.extend_from_slice(text.as_bytes());
        let sources = [source("plain.py", text), source("bom.py", &bom)];
        let report = parse_batch(&sources).unwrap();
        for (file, source) in report.files.iter().zip(&sources) {
            assert_eq!(file.status, ParseStatus::SyntaxAccepted);
            assert_eq!(file.sha256, super::super::hash(&source.bytes));
        }
        assert_eq!(report.files[0].nodes_visited, report.files[1].nodes_visited);
        assert_ne!(report.files[0].sha256, report.files[1].sha256);
        assert_eq!(
            status(b"x = '\xff'\n"),
            (
                ParseStatus::UnsupportedEncoding,
                Some(DiagnosticCode::InvalidUtf8)
            )
        );
    }

    #[test]
    fn effective_cookies_are_explicitly_unsupported_even_utf8_or_ascii() {
        for bytes in [
            "# coding: utf-8\nx = 1\n",
            "# coding=latin-1\nx = 1\n",
            "#!/usr/bin/python\r\n# coding: utf8\r\n",
            "\n# coding: ascii\n",
            "\r#coding:utf8\n",
            " \t\u{c}\r\n\t# coding=latin-1\n",
            "# plain\n# coding: UTF-8\n",
            "# decoding: utf8\n",
            "# coding: ; coding=ascii\n",
            "\u{feff}# coding: utf-8\n",
        ] {
            assert_eq!(
                status(bytes),
                (
                    ParseStatus::UnsupportedEncoding,
                    Some(DiagnosticCode::EncodingCookieUnsupported)
                ),
                "{bytes:?}"
            );
        }
    }

    #[test]
    fn ineffective_cookie_lookalikes_do_not_select_encoding() {
        for bytes in [
            "x = '# coding: latin-1'\n",
            "x = 1 # coding: latin-1\n",
            "x = 1\n# coding: latin-1\n",
            "\n\n# coding: latin-1\n",
            "# Coding: latin-1\n",
            "# coding : latin-1\n",
            "# coding:\n",
            "# coding: /utf8\n",
        ] {
            assert!(!encoding_cookie(bytes.as_bytes()), "{bytes:?}");
            assert_eq!(status(bytes).0, ParseStatus::SyntaxAccepted, "{bytes:?}");
        }
        for bytes in [
            b"\x0b# coding: utf8\n".as_slice(),
            b"code\n# coding: utf8\n",
        ] {
            assert!(!encoding_cookie(bytes));
        }
    }

    #[test]
    fn cookie_positions_follow_physical_lines_with_mixed_newlines() {
        for first_newline in ["\n", "\r\n", "\r"] {
            for second_newline in ["\n", "\r\n", "\r"] {
                let first = format!("# coding: ascii{first_newline}value=1{second_newline}");
                assert!(encoding_cookie(first.as_bytes()));
                assert_eq!(
                    status(&first).1,
                    Some(DiagnosticCode::EncodingCookieUnsupported)
                );
                for prefix in ["", " \t\u{c}", "# comment", "#!/usr/bin/python"] {
                    let second =
                        format!("{prefix}{first_newline}# coding: ascii{second_newline}value=1\n");
                    assert!(encoding_cookie(second.as_bytes()));
                    assert_eq!(
                        status(&second).1,
                        Some(DiagnosticCode::EncodingCookieUnsupported)
                    );
                    let third = format!(
                        "{prefix}{first_newline}# comment{second_newline}# coding: ascii\n"
                    );
                    assert!(!encoding_cookie(third.as_bytes()));
                }
                let after_code = format!("value=1{first_newline}# coding: ascii{second_newline}");
                assert!(!encoding_cookie(after_code.as_bytes()));
            }
        }
    }

    #[test]
    fn bare_cr_second_line_cookie_is_rejected_before_parser_input() {
        let sources = [source("cookie.py", b"\r# coding: ascii\nvalue=1\n")];
        let mut entered_parser = false;
        let report = run(&sources, Limits::default(), |phase| {
            // Make any accidental parser entry deterministic and observable.
            entered_parser |= phase == Checkpoint::Input;
            phase == Checkpoint::Input
        })
        .unwrap();
        assert!(!entered_parser);
        assert_eq!(report.files[0].status, ParseStatus::UnsupportedEncoding);
        assert_eq!(
            report.files[0].diagnostic,
            Some(DiagnosticCode::EncodingCookieUnsupported)
        );
        assert_eq!(report.files[0].nodes_visited, 0);
        assert!(!report.files[0].traversal_finished);
    }

    #[test]
    fn effective_cookie_policy_precedes_payload_utf8_validation() {
        for bytes in [
            b"# coding: latin-1\nvalue='\xe9'\n".as_slice(),
            b"\r# coding: latin-1\rvalue='\xe9'\r",
            b"# comment\r\n# coding: latin-1\nvalue='\xe9'\r",
        ] {
            assert_eq!(
                status(bytes),
                (
                    ParseStatus::UnsupportedEncoding,
                    Some(DiagnosticCode::EncodingCookieUnsupported)
                )
            );
        }
        for bytes in [
            b"value='\xe9'\n".as_slice(),
            b"\r\r# coding: latin-1\nvalue='\xe9'\n",
            b"value=1\r# coding: latin-1\nvalue='\xe9'\n",
        ] {
            assert_eq!(
                status(bytes),
                (
                    ParseStatus::UnsupportedEncoding,
                    Some(DiagnosticCode::InvalidUtf8)
                )
            );
        }
    }

    #[test]
    fn all_nodes_including_punctuation_and_comments_count() {
        // module, comment, expression_statement, assignment, identifier, =,
        // list, [, integer, comma, integer, ]: twelve total CST nodes.
        let sources = [source("a.py", "# note\nx = [1, 2]\n")];
        let report = parse_batch(&sources).unwrap();
        assert_eq!(report.files[0].nodes_visited, 12);
        let exact = run(
            &sources,
            Limits {
                nodes: 12,
                ..Limits::default()
            },
            |_| false,
        )
        .unwrap();
        assert_eq!(exact.files[0].status, ParseStatus::SyntaxAccepted);
        let limited = run(
            &sources,
            Limits {
                nodes: 11,
                ..Limits::default()
            },
            |_| false,
        )
        .unwrap();
        assert_eq!(limited.files[0].status, ParseStatus::ResourceLimit);
        assert_eq!(limited.files[0].diagnostic, Some(DiagnosticCode::NodeLimit));
        assert_eq!(limited.files[0].nodes_visited, 11);
        assert!(!limited.files[0].traversal_finished);
    }

    #[test]
    fn wide_and_deep_trees_obey_walk_budgets() {
        let wide = format!("items = [{}]\n", "0,".repeat(500));
        let sources = [source("wide.py", wide)];
        let report = run(
            &sources,
            Limits {
                nodes: 100,
                ..Limits::default()
            },
            |_| false,
        )
        .unwrap();
        assert_eq!(report.files[0].diagnostic, Some(DiagnosticCode::NodeLimit));
        assert_eq!(report.files[0].nodes_visited, 100);
        let deep = format!("x = {}0{}\n", "(".repeat(40), ")".repeat(40));
        let sources = [source("deep.py", deep)];
        let report = run(
            &sources,
            Limits {
                depth: 8,
                ..Limits::default()
            },
            |_| false,
        )
        .unwrap();
        assert_eq!(report.files[0].diagnostic, Some(DiagnosticCode::DepthLimit));
        assert_eq!(report.files[0].max_depth, 8);
        let root_only = [source("empty.py", "")];
        let report = run(
            &root_only,
            Limits {
                depth: 0,
                nodes: 1,
                ..Limits::default()
            },
            |_| false,
        )
        .unwrap();
        assert_eq!(report.files[0].max_depth, 0);
        assert_eq!(report.files[0].status, ParseStatus::SyntaxAccepted);
    }

    #[test]
    fn resource_limits_override_an_earlier_syntax_error() {
        let sources = [source("bad.py", format!("!\n{}", "x = 1\n".repeat(30)))];
        let report = run(
            &sources,
            Limits {
                nodes: 20,
                ..Limits::default()
            },
            |_| false,
        )
        .unwrap();
        assert_eq!(report.files[0].status, ParseStatus::ResourceLimit);
        assert_eq!(report.files[0].diagnostic, Some(DiagnosticCode::NodeLimit));
        assert!(!report.files[0].traversal_finished);
    }

    #[test]
    fn deterministic_deadline_checks_latch_across_all_stages() {
        for stop in [
            Checkpoint::Preflight,
            Checkpoint::Identity,
            Checkpoint::Hash,
            Checkpoint::Preprocess,
            Checkpoint::Input,
            Checkpoint::PostParse,
            Checkpoint::Walk,
            Checkpoint::Final,
        ] {
            let sources = [
                source("first.py", "x = [1, 2]\n"),
                source("second.py", "pass\n"),
            ];
            let mut fired = false;
            let mut second_inputs = 0;
            let report = run(&sources, Limits::default(), |phase| {
                if fired && phase == Checkpoint::Input {
                    second_inputs += 1;
                }
                let expire = !fired && phase == stop;
                fired |= expire;
                expire
            })
            .unwrap();
            assert!(fired, "{stop:?}");
            for file in &report.files {
                assert_eq!(file.status, ParseStatus::ResourceLimit, "{stop:?}");
                assert_eq!(file.diagnostic, Some(DiagnosticCode::Deadline), "{stop:?}");
            }
            // Input callback may be retried within the cancelled first parse,
            // but the remaining entry never starts traversal or parsing.
            assert_eq!(report.files[1].nodes_visited, 0);
            if stop != Checkpoint::Input {
                assert_eq!(second_inputs, 0);
            }
            if stop == Checkpoint::PostParse {
                assert_eq!(report.files[0].nodes_visited, 0);
            }
        }
    }

    #[test]
    fn input_eof_can_return_a_tree_but_cancellation_is_still_a_limit() {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse_with_options(&mut |_, _| b"".as_slice(), None, None);
        assert!(tree.is_some());
        assert!(!tree.unwrap().root_node().has_error());
        let sources = [source("a.py", "def f(:\n")];
        let report = run(&sources, Limits::default(), |phase| {
            phase == Checkpoint::Input
        })
        .unwrap();
        assert_eq!(report.files[0].status, ParseStatus::ResourceLimit);
        assert_eq!(report.files[0].nodes_visited, 0);
        assert!(!report.files[0].traversal_finished);
    }

    #[test]
    fn progress_callback_can_cancel_independently_of_input() {
        let sources = [source("large.py", "x = 1\n".repeat(4000))];
        let mut progress_seen = false;
        let report = run(&sources, Limits::default(), |phase| {
            if phase == Checkpoint::Progress {
                progress_seen = true;
                true
            } else {
                false
            }
        })
        .unwrap();
        assert!(progress_seen);
        assert_eq!(report.files[0].diagnostic, Some(DiagnosticCode::Deadline));
        assert_eq!(report.files[0].nodes_visited, 0);
    }

    #[test]
    fn cancellation_after_a_valid_input_prefix_never_accepts_a_partial_tree() {
        let sources = [source("a.py", "pass\n".repeat(2000))];
        let mut inputs = 0;
        let mut post_parse_seen = false;
        let report = run(&sources, Limits::default(), |phase| {
            post_parse_seen |= phase == Checkpoint::PostParse;
            if phase == Checkpoint::Input {
                inputs += 1;
                return inputs == 2;
            }
            false
        })
        .unwrap();
        assert!(inputs >= 2);
        assert!(post_parse_seen);
        assert_eq!(report.files[0].status, ParseStatus::ResourceLimit);
        assert_eq!(report.files[0].diagnostic, Some(DiagnosticCode::Deadline));
        assert!(!report.files[0].traversal_finished);
    }

    #[test]
    fn deadline_during_a_walk_preserves_bounded_partial_counters() {
        let sources = [source("a.py", "x = [1, 2, 3]\n"), source("b.py", "pass\n")];
        let mut walks = 0;
        let report = run(&sources, Limits::default(), |phase| {
            if phase == Checkpoint::Walk {
                walks += 1;
                return walks == 5;
            }
            false
        })
        .unwrap();
        assert_eq!(report.files[0].nodes_visited, 4);
        assert_eq!(report.files[0].diagnostic, Some(DiagnosticCode::Deadline));
        assert!(!report.files[0].traversal_finished);
        assert_eq!(report.files[1].nodes_visited, 0);
    }

    #[test]
    fn missing_tree_without_cancellation_is_a_front_end_failure() {
        let sources = [source("a.py", "pass\n")];
        let mut report = parse_batch(&sources).unwrap();
        let cancel = Cancellation {
            expired: Cell::new(false),
            check: RefCell::new(|_| false),
        };
        // No language configured: exercise None without inventing syntax or a
        // deadline. The normal entry point always initializes the pinned grammar.
        let result = parse_file(
            &mut Parser::new(),
            &sources[0].bytes,
            &mut report.files[0],
            Limits::default(),
            &cancel,
        );
        assert_eq!(result, Err(FrontEndError::ParserFailure));
        assert!(!cancel.expired.get());
    }

    #[test]
    fn replay_uses_capture_not_paths_hex_or_executable_code() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("captured.py");
        let marker = root.path().join("executed");
        let bytes = format!("from pathlib import Path\nPath({:?}).touch()\n", marker);
        std::fs::write(&path, &bytes).unwrap();
        let sources = [source("captured.py", std::fs::read(&path).unwrap())];
        std::fs::write(&path, "def broken(:\n").unwrap();
        let first = parse_batch(&sources).unwrap();
        std::fs::remove_file(&path).unwrap();
        let second = parse_batch(&sources).unwrap();
        assert_eq!(first.files[0].status, ParseStatus::SyntaxAccepted);
        assert_eq!(
            serde_json::to_vec(&first).unwrap(),
            serde_json::to_vec(&second).unwrap()
        );
        assert!(!marker.exists());
        assert!(!serde_json::to_string(&second).unwrap().contains("touch"));
    }
}
