//! Private captured-byte Ruff frontend, not evidence or a compatibility backend.
//! The separate private fact candidate consumes only validated, borrowed ASTs.

mod expressions;
mod facts;
mod literals;
pub(super) use facts::CandidateReport;

pub(super) fn extract_fact_candidates(
    sources: &[Source],
) -> Result<CandidateReport<'_>, FrontEndError> {
    facts::extract_fact_candidates(sources)
}

use super::Source;
use ruff_python_ast::PythonVersion;
use ruff_python_parser::{lexer::lex, parse_unchecked, Mode, ParseOptions, TokenKind};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::io::{self, Write};
use std::time::Instant;

const COMMIT: &str = "87f0feb21a1af67195bebf94d811770393f9f447";
const POLICY: &str = "python-ruff-private-1-py312-ast-only-duplicate-keywords";
const MAX_METADATA: usize = 4096;
const MAX_REPORT: usize = 1024 * 1024;
const MAX_TOKENS: usize = 200_000;
const MAX_DEPTH: usize = 256;
const MAX_ERRORS: usize = 200_000;
const PROVENANCE: &[u8] = include_bytes!("../../vendor/ruff/provenance.json");

#[derive(Debug, Serialize)]
pub(super) struct ParserIdentity {
    pub(super) upstream_commit: &'static str,
    pub(super) upstream_version: &'static str,
    pub(super) target: &'static str,
    pub(super) policy: &'static str,
    pub(super) provenance_sha256: String,
    pub(super) implementation_sha256: String,
    /// Conservative development fingerprint, including the whole root lockfile.
    pub(super) dependency_lock_sha256: String,
    pub(super) sha256: String,
}

#[derive(Debug, Serialize)]
pub(super) struct BatchReport<'a> {
    pub(super) identity: ParserIdentity,
    pub(super) capabilities: Capabilities,
    pub(super) files: Vec<FileReport<'a>>,
}

#[derive(Debug, Serialize)]
pub(super) struct Capabilities {
    pub(super) cpython_compatibility: CompatibilityStatus,
    pub(super) unicode_15_identifier_admission: CompatibilityStatus,
    pub(super) decimal_4300_admission: CompatibilityStatus,
    pub(super) type_comment_placement: CompatibilityStatus,
    pub(super) encoding_parity: CompatibilityStatus,
    pub(super) literal_and_span_normalization: CompatibilityStatus,
    pub(super) fact_lowering: CompatibilityStatus,
    pub(super) resource_policy: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) enum CompatibilityStatus {
    Pending,
}

impl Default for Capabilities {
    fn default() -> Self {
        Self {
            cpython_compatibility: CompatibilityStatus::Pending,
            unicode_15_identifier_admission: CompatibilityStatus::Pending,
            decimal_4300_admission: CompatibilityStatus::Pending,
            type_comment_placement: CompatibilityStatus::Pending,
            encoding_parity: CompatibilityStatus::Pending,
            literal_and_span_normalization: CompatibilityStatus::Pending,
            fact_lowering: CompatibilityStatus::Pending,
            resource_policy: "preparse-bytes-and-lexical-tokens; cooperative-before-after-parser; no-hard-timeout-or-rss-bound",
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct FileReport<'a> {
    pub(super) path: &'a str,
    pub(super) module: &'a str,
    /// Hash of the immutable original bytes, including any BOM.
    pub(super) sha256: String,
    pub(super) admission: Admission,
    pub(super) syntax: RuffSyntax,
    pub(super) diagnostic: Option<Diagnostic>,
    pub(super) syntax_errors: usize,
    pub(super) unsupported_version_errors: usize,
    pub(super) error_counts_capped: bool,
    pub(super) syntax_error_line: Option<LogicalLine>,
    pub(super) unsupported_version_line: Option<LogicalLine>,
    pub(super) tokens_checked: usize,
    pub(super) max_lexical_depth: usize,
    pub(super) lexical_gate_finished: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) enum Admission {
    Admitted,
    UnsupportedEncoding,
    ResourceLimit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) enum RuffSyntax {
    NotParsed,
    AcceptedByRuff,
    StructuredSyntax,
    UnsupportedVersion,
    SyntaxAndUnsupportedVersion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) enum Diagnostic {
    EncodingCookieUnsupported,
    InvalidUtf8,
    Deadline,
    TokenLimit,
    DepthLimit,
}

/// One-based physical source line after universal-newline interpretation.
/// No UTF-8 slicing or character-column conversion is needed for byte offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) struct LogicalLine(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FrontEndError {
    FileCount,
    FileBytes,
    TotalBytes,
    InvalidPath,
    DuplicatePath,
    InvalidModule,
    ReportLimit,
}

#[derive(Clone, Copy)]
struct Limits {
    files: usize,
    file_bytes: usize,
    total_bytes: usize,
    report: usize,
    tokens: usize,
    depth: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            files: super::MAX_FILES,
            file_bytes: super::MAX_FILE,
            total_bytes: super::MAX_TOTAL,
            report: MAX_REPORT,
            tokens: MAX_TOKENS,
            depth: MAX_DEPTH,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Checkpoint {
    Admission,
    Identity,
    Hash,
    Decode,
    Token,
    PreParse,
    PostParse,
    Semantic,
    PostConsume,
    PostDrop,
    Output,
    Final,
}

struct Cancellation<F> {
    expired: bool,
    check: F,
}

impl<F: FnMut(Checkpoint) -> bool> Cancellation<F> {
    fn at(&mut self, checkpoint: Checkpoint) -> bool {
        self.expired |= (self.check)(checkpoint);
        self.expired
    }
}

/// No filesystem access, runtime discovery, Python probe, or target execution.
/// Ruff has no parser interrupt hook: its single parse (including allocations and
/// recursive AST destruction) is checked only before/after, not forcibly stopped.
/// The separate lexical pass is not an AST/CST walk or a second parse. Its token
/// and delimiter/indent/string depth caps do not bound every parser recursion form.
pub(super) fn parse_batch(sources: &[Source]) -> Result<BatchReport<'_>, FrontEndError> {
    let start = Instant::now();
    run(sources, Limits::default(), |_| {
        start.elapsed() >= super::DEADLINE
    })
}

fn options() -> ParseOptions {
    ParseOptions::from(Mode::Module)
        .with_target_version(PythonVersion::PY312)
        .with_ast_only(true)
}

fn digest<F: FnMut(Checkpoint) -> bool>(
    bytes: &[u8],
    cancel: &mut Cancellation<F>,
    checkpoint: Checkpoint,
) -> String {
    let mut hash = Sha256::new();
    // Even a cancelled report retains exact original-byte digests. This remaining
    // work is bounded by admission, and cannot re-enable parsing after expiry.
    for chunk in bytes.chunks(4096) {
        cancel.at(checkpoint);
        hash.update(chunk);
    }
    cancel.at(checkpoint);
    format!("{:x}", hash.finalize())
}

fn identity<F: FnMut(Checkpoint) -> bool>(cancel: &mut Cancellation<F>) -> ParserIdentity {
    let mut identity = ParserIdentity {
        upstream_commit: COMMIT,
        upstream_version: "0.12.0",
        target: "PY312",
        policy: POLICY,
        provenance_sha256: digest(PROVENANCE, cancel, Checkpoint::Identity),
        implementation_sha256: digest(
            include_bytes!("python_ruff.rs"),
            cancel,
            Checkpoint::Identity,
        ),
        dependency_lock_sha256: digest(
            include_bytes!("../../Cargo.lock"),
            cancel,
            Checkpoint::Identity,
        ),
        sha256: String::new(),
    };
    let pins = serde_json::to_vec(&identity).expect("fixed identity serialization");
    identity.sha256 = digest(&pins, cancel, Checkpoint::Identity);
    identity
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
            .filter(|&count| count <= self.limit)
            .ok_or_else(|| io::Error::other("private report limit"))?;
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

fn file_report(source: &Source) -> FileReport<'_> {
    FileReport {
        path: &source.path,
        module: &source.module,
        sha256: "0".repeat(64),
        admission: Admission::ResourceLimit,
        syntax: RuffSyntax::NotParsed,
        diagnostic: Some(Diagnostic::Deadline),
        syntax_errors: 0,
        unsupported_version_errors: 0,
        error_counts_capped: false,
        syntax_error_line: None,
        unsupported_version_line: None,
        tokens_checked: 0,
        max_lexical_depth: 0,
        lexical_gate_finished: false,
    }
}

fn run<F: FnMut(Checkpoint) -> bool>(
    sources: &[Source],
    limits: Limits,
    check: F,
) -> Result<BatchReport<'_>, FrontEndError> {
    run_consumer(sources, limits, check, &mut |_, _, _, _| {})
}

type ValidatedConsumer<'a> = dyn FnMut(usize, &str, &ruff_python_parser::Parsed<ruff_python_ast::Mod>, &mut dyn FnMut() -> bool)
    + 'a;

type ValidatedFileConsumer<'a> = dyn FnMut(&str, &ruff_python_parser::Parsed<ruff_python_ast::Mod>, &mut dyn FnMut() -> bool)
    + 'a;

fn validate_sources<F: FnMut(Checkpoint) -> bool>(
    sources: &[Source],
    limits: Limits,
    cancel: &mut Cancellation<F>,
) -> Result<(), FrontEndError> {
    cancel.at(Checkpoint::Admission);
    if sources.len() > limits.files {
        return Err(FrontEndError::FileCount);
    }
    let mut paths = BTreeSet::new();
    let mut total = 0usize;
    for source in sources {
        cancel.at(Checkpoint::Admission);
        let path = source.path.as_str();
        if path.is_empty()
            || path.len() > MAX_METADATA
            || path.contains(['\\', '\0'])
            || matches!(path.as_bytes(), [drive, b':', ..] if drive.is_ascii_alphabetic())
            || path.split('/').any(|part| matches!(part, "" | "." | ".."))
        {
            return Err(FrontEndError::InvalidPath);
        }
        if !paths.insert(path) {
            return Err(FrontEndError::DuplicatePath);
        }
        if source.module.len() > MAX_METADATA || source.module.contains('\0') {
            return Err(FrontEndError::InvalidModule);
        }
        if source.bytes.len() > limits.file_bytes {
            return Err(FrontEndError::FileBytes);
        }
        total = total
            .checked_add(source.bytes.len())
            .filter(|&n| n <= limits.total_bytes)
            .ok_or(FrontEndError::TotalBytes)?;
    }
    Ok(())
}

fn run_consumer<'a, F: FnMut(Checkpoint) -> bool>(
    sources: &'a [Source],
    limits: Limits,
    check: F,
    consumer: &mut ValidatedConsumer<'_>,
) -> Result<BatchReport<'a>, FrontEndError> {
    let mut cancel = Cancellation {
        expired: false,
        check,
    };
    run_context(sources, limits, &mut cancel, consumer)
}

fn run_context<'a, F: FnMut(Checkpoint) -> bool>(
    sources: &'a [Source],
    limits: Limits,
    cancel: &mut Cancellation<F>,
    consumer: &mut ValidatedConsumer<'_>,
) -> Result<BatchReport<'a>, FrontEndError> {
    validate_sources(sources, limits, cancel)?;
    let mut report = BatchReport {
        identity: identity(cancel),
        capabilities: Capabilities::default(),
        files: Vec::new(),
    };
    // Reserve escaped metadata and every outcome before parsing, not merely before
    // allocating a serialized output buffer. Longest variants/max counters win.
    let mut reserved = json_size(&report, limits.report)?;
    for (index, source) in sources.iter().enumerate() {
        cancel.at(Checkpoint::Admission);
        let mut worst = file_report(source);
        worst.admission = Admission::UnsupportedEncoding;
        worst.syntax = RuffSyntax::SyntaxAndUnsupportedVersion;
        worst.diagnostic = Some(Diagnostic::EncodingCookieUnsupported);
        worst.syntax_errors = usize::MAX;
        worst.unsupported_version_errors = usize::MAX;
        worst.syntax_error_line = Some(LogicalLine(usize::MAX));
        worst.unsupported_version_line = Some(LogicalLine(usize::MAX));
        worst.tokens_checked = usize::MAX;
        worst.max_lexical_depth = usize::MAX;
        reserved = reserved
            .checked_add(json_size(&worst, limits.report)?)
            .and_then(|count| count.checked_add(usize::from(index != 0)))
            .filter(|&count| count <= limits.report)
            .ok_or(FrontEndError::ReportLimit)?;
    }
    for (index, source) in sources.iter().enumerate() {
        let mut file = file_report(source);
        file.sha256 = digest(&source.bytes, cancel, Checkpoint::Hash);
        if !cancel.at(Checkpoint::Decode) {
            classify(
                &source.bytes,
                &mut file,
                limits,
                cancel,
                &mut |text, parsed, check| {
                    consumer(index, text, parsed, check);
                },
            );
        }
        report.files.push(file);
    }
    cancel.at(Checkpoint::Output);
    json_size(&report, limits.report)?;
    // Expiry anywhere, even during the last output check, invalidates the whole
    // invocation's syntax classifications. No earlier accepted tree survives.
    if cancel.at(Checkpoint::Final) {
        for file in &mut report.files {
            file.admission = Admission::ResourceLimit;
            file.syntax = RuffSyntax::NotParsed;
            file.diagnostic = Some(Diagnostic::Deadline);
            file.syntax_errors = 0;
            file.unsupported_version_errors = 0;
            file.error_counts_capped = false;
            file.syntax_error_line = None;
            file.unsupported_version_line = None;
        }
    }
    // The conservative preparse reservation covers the deadline rewrite too.
    Ok(report)
}

fn classify<F: FnMut(Checkpoint) -> bool>(
    raw: &[u8],
    file: &mut FileReport<'_>,
    limits: Limits,
    cancel: &mut Cancellation<F>,
    consume: &mut ValidatedFileConsumer<'_>,
) {
    let bytes = raw.strip_prefix(b"\xef\xbb\xbf").unwrap_or(raw);
    // Cookie detection precedes UTF-8 validation, including BOM/cookie conflicts.
    if encoding_cookie(bytes) {
        file.admission = Admission::UnsupportedEncoding;
        file.diagnostic = Some(Diagnostic::EncodingCookieUnsupported);
        return;
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        file.admission = Admission::UnsupportedEncoding;
        file.diagnostic = Some(Diagnostic::InvalidUtf8);
        return;
    };
    if let Err(diagnostic) = lexical_gate(text, file, limits, cancel) {
        file.diagnostic = Some(diagnostic);
        return;
    }
    if cancel.at(Checkpoint::PreParse) {
        return;
    }
    // `parse(...).is_ok()` alone ignores unsupported-version diagnostics.
    // Unchecked means retain both channels for inspection, NEVER trust recovery.
    let parsed = parse_unchecked(text, options());
    if cancel.at(Checkpoint::PostParse) {
        return;
    }
    let errors = parsed.errors();
    let unsupported = parsed.unsupported_syntax_errors();
    file.syntax_errors = errors.len().min(MAX_ERRORS);
    file.unsupported_version_errors = unsupported.len().min(MAX_ERRORS);
    file.error_counts_capped = errors.len() > MAX_ERRORS || unsupported.len() > MAX_ERRORS;
    file.syntax_error_line = errors
        .first()
        .map(|error| logical_line(bytes, error.location.start().to_usize()));
    file.unsupported_version_line = unsupported
        .first()
        .map(|error| logical_line(bytes, error.range.start().to_usize()));
    file.admission = Admission::Admitted;
    file.diagnostic = None;
    file.syntax = match (errors.is_empty(), unsupported.is_empty()) {
        (true, true) => RuffSyntax::AcceptedByRuff,
        (false, true) => RuffSyntax::StructuredSyntax,
        (true, false) => RuffSyntax::UnsupportedVersion,
        (false, false) => RuffSyntax::SyntaxAndUnsupportedVersion,
    };
    if errors.is_empty() && unsupported.is_empty() && !cancel.at(Checkpoint::Semantic) {
        consume(text, &parsed, &mut || cancel.at(Checkpoint::Semantic));
        cancel.at(Checkpoint::PostConsume);
    }
    // AST references never escape the callback. Destruction, like parsing, has no
    // interrupt hook; this is cooperative accounting, not a stack/RSS sandbox.
    drop(parsed);
    cancel.at(Checkpoint::PostDrop);
}

fn lexical_gate<F: FnMut(Checkpoint) -> bool>(
    text: &str,
    file: &mut FileReport<'_>,
    limits: Limits,
    cancel: &mut Cancellation<F>,
) -> Result<(), Diagnostic> {
    let mut lexer = lex(text, Mode::Module);
    let mut depth = 0usize;
    loop {
        if cancel.at(Checkpoint::Token) {
            return Err(Diagnostic::Deadline);
        }
        // next_token replaces the previous lexeme value. There is no retained
        // token/value array in this pass; a single lexeme is input-byte-bounded.
        let kind = lexer.next_token();
        if kind == TokenKind::EndOfFile {
            file.lexical_gate_finished = true;
            return Ok(());
        }
        if file.tokens_checked >= limits.tokens {
            return Err(Diagnostic::TokenLimit);
        }
        file.tokens_checked += 1;
        match kind {
            TokenKind::Lpar
            | TokenKind::Lsqb
            | TokenKind::Lbrace
            | TokenKind::Indent
            | TokenKind::FStringStart
            | TokenKind::TStringStart => {
                if depth >= limits.depth {
                    return Err(Diagnostic::DepthLimit);
                }
                depth += 1;
                file.max_lexical_depth = file.max_lexical_depth.max(depth);
            }
            TokenKind::Rpar
            | TokenKind::Rsqb
            | TokenKind::Rbrace
            | TokenKind::Dedent
            | TokenKind::FStringEnd
            | TokenKind::TStringEnd => depth = depth.saturating_sub(1),
            _ => {}
        }
        // Unknown tokens and lexical diagnostics are not waived: the parser
        // below owns structured syntax classification, including NUL errors.
    }
}

fn logical_line(bytes: &[u8], offset: usize) -> LogicalLine {
    let end = offset.min(bytes.len());
    let mut line = 1;
    let mut index = 0;
    while index < end {
        match bytes[index] {
            b'\r' => {
                let crlf = bytes.get(index + 1) == Some(&b'\n');
                if crlf && index + 1 == end {
                    break;
                }
                line += 1;
                index += usize::from(crlf);
            }
            b'\n' => line += 1,
            _ => {}
        }
        index += 1;
    }
    LogicalLine(line)
}

// Same effective-cookie boundary as the private reference lane: first two
// physical lines, second eligible only after a blank/comment first line.
fn encoding_cookie(bytes: &[u8]) -> bool {
    fn physical_line(bytes: &[u8]) -> (&[u8], &[u8]) {
        let Some(end) = bytes.iter().position(|&byte| matches!(byte, b'\r' | b'\n')) else {
            return (bytes, &[]);
        };
        let width = 1 + usize::from(bytes[end] == b'\r' && bytes.get(end + 1) == Some(&b'\n'));
        (&bytes[..end], &bytes[end + width..])
    }
    fn trim(line: &[u8]) -> &[u8] {
        &line[line
            .iter()
            .take_while(|&&byte| matches!(byte, b' ' | b'\t' | 0x0c))
            .count()..]
    }
    fn cookie(line: &[u8]) -> bool {
        let line = trim(line);
        line.first() == Some(&b'#')
            && line.windows(6).enumerate().any(|(index, window)| {
                if window != b"coding" || !matches!(line.get(index + 6), Some(b':' | b'=')) {
                    return false;
                }
                let rest = &line[index + 7..];
                let skip = rest
                    .iter()
                    .take_while(|&&byte| matches!(byte, b' ' | b'\t'))
                    .count();
                rest.get(skip).is_some_and(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.')
                })
            })
    }
    let (first, rest) = physical_line(bytes);
    let first = trim(first);
    cookie(first)
        || ((first.is_empty() || first.first() == Some(&b'#')) && cookie(physical_line(rest).0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(bytes: impl AsRef<[u8]>) -> Source {
        Source {
            path: "never-opened/sample.py".into(),
            module: "synthetic".into(),
            hex: "deliberately invalid hex".into(),
            bytes: bytes.as_ref().to_vec(),
        }
    }

    fn syntax(text: &str) -> RuffSyntax {
        parse_batch(&[source(text)]).unwrap().files[0].syntax
    }

    #[test]
    fn identity_pins_and_original_byte_digest_policy() {
        let inputs = [source(b"\xef\xbb\xbfx=1\r\n")];
        let report = parse_batch(&inputs).unwrap();
        assert_eq!(report.identity.upstream_commit, COMMIT);
        assert_eq!(report.identity.upstream_version, "0.12.0");
        assert_eq!(report.identity.target, "PY312");
        assert_eq!(report.identity.policy, POLICY);
        assert_eq!(
            report.identity.provenance_sha256,
            super::super::hash(PROVENANCE)
        );
        assert_eq!(
            report.identity.implementation_sha256,
            super::super::hash(include_bytes!("python_ruff.rs"))
        );
        assert_eq!(
            report.identity.dependency_lock_sha256,
            super::super::hash(include_bytes!("../../Cargo.lock"))
        );
        assert_eq!(report.identity.sha256.len(), 64);
        assert_eq!(
            report.identity.sha256,
            parse_batch(&[]).unwrap().identity.sha256
        );
        assert_eq!(report.files[0].sha256, super::super::hash(&inputs[0].bytes));
        assert_ne!(report.files[0].sha256, super::super::hash(b"x=1\r\n"));
        assert_eq!(report.files[0].syntax, RuffSyntax::AcceptedByRuff);
        assert_eq!(
            report.capabilities.cpython_compatibility,
            CompatibilityStatus::Pending
        );
        let provenance: serde_json::Value = serde_json::from_slice(PROVENANCE).unwrap();
        assert_eq!(provenance["upstream"]["commit"], COMMIT);
    }

    #[test]
    fn ast_positive_gap_families_and_fstrings_are_classification_only() {
        for text in [
            "*(a if ready() else b)\n",
            "*(lambda: value)\n",
            "def f(x: T[A][B]) -> T[A][B]: pass\n",
            "type Alias = T[A].B[C]\n",
            "def outer():\n    if ready:\n        from pkg import (\n            # nested layout\n            item,\n        )\n    import other\n",
            "f\"{mapping[\"key\"]}\"\n",
            "f'{value:=10}'\n",
            "f'{(value := call())}'\n",
            "f'{value:{width()}.{precision()}f}'\n",
        ] {
            assert_eq!(syntax(text), RuffSyntax::AcceptedByRuff, "{text}");
        }
    }

    #[test]
    fn ast_only_is_explicit_and_default_false_for_both_constructors() {
        for text in [
            "f(x=1, x=2)\n",
            "f(x=1, **other, x=2)\n",
            "f(**first, x=1, **second, x=2)\n",
            "class C(x=1, **other, x=2): pass\n",
            "f(K=1, \u{212a}=2)\n",
            "class C(K=1, **other, \u{212a}=2): pass\n",
        ] {
            for baseline in [
                ParseOptions::from(Mode::Module),
                ParseOptions::from(ruff_python_ast::PySourceType::Python),
            ] {
                let parsed =
                    parse_unchecked(text, baseline.with_target_version(PythonVersion::PY312));
                assert!(!parsed.errors().is_empty(), "{text}");
            }
            assert_eq!(syntax(text), RuffSyntax::AcceptedByRuff, "{text}");
        }
    }

    #[test]
    fn errors_in_either_channel_reject_without_facts_or_source_diagnostics() {
        for text in [
            "f(x=1, 2)\n",
            "f(x for x in xs, 1)\n",
            "def f(:\n",
            "x=\0\n",
        ] {
            assert_eq!(syntax(text), RuffSyntax::StructuredSyntax, "{text:?}");
        }
        let text = "type Alias[T = int] = T\n";
        let parsed = parse_unchecked(text, options());
        assert!(parsed.errors().is_empty());
        assert!(!parsed.unsupported_syntax_errors().is_empty());
        // Demonstrate why the upstream Result convenience is insufficient.
        assert!(ruff_python_parser::parse(text, options()).is_ok());
        assert_eq!(syntax(text), RuffSyntax::UnsupportedVersion);
        let inputs = [source("secret_call('PRIVATE_PAYLOAD')\ndef f(:\n")];
        let report = parse_batch(&inputs).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        for forbidden in [
            "secret_call",
            "PRIVATE_PAYLOAD",
            "facts",
            "nodes",
            "pythonVersion",
        ] {
            assert!(!json.contains(forbidden));
        }
        assert!(report.files[0].syntax_errors > 0);
        assert!(report.files[0].syntax_error_line.is_some());
    }

    #[test]
    fn known_compatibility_gaps_are_not_silently_parity() {
        for text in [
            format!("x={}\n", "1".repeat(4301)),
            "\u{1c89}=1\n".into(),
            "pass # type: int\n".into(),
            "# type: int\nx=1\n".into(),
        ] {
            let inputs = [source(&text)];
            let report = parse_batch(&inputs).unwrap();
            assert_eq!(report.files[0].syntax, RuffSyntax::AcceptedByRuff);
            assert_eq!(
                report.capabilities.unicode_15_identifier_admission,
                CompatibilityStatus::Pending
            );
            assert_eq!(
                report.capabilities.decimal_4300_admission,
                CompatibilityStatus::Pending
            );
            assert_eq!(
                report.capabilities.type_comment_placement,
                CompatibilityStatus::Pending
            );
        }
    }

    #[test]
    fn cookies_utf8_bom_and_newline_offsets() {
        for bytes in [
            b"# coding: utf-8\nx=1\n".as_slice(),
            b"#!/usr/bin/python\r\n# coding=latin-1\r\nx='\xff'",
            b"\xef\xbb\xbf# coding: utf-8\n",
            b"\r# coding: utf-8\r",
        ] {
            let inputs = [source(bytes)];
            let report = parse_batch(&inputs).unwrap();
            assert_eq!(report.files[0].admission, Admission::UnsupportedEncoding);
            assert_eq!(
                report.files[0].diagnostic,
                Some(Diagnostic::EncodingCookieUnsupported)
            );
            assert_eq!(report.files[0].syntax, RuffSyntax::NotParsed);
        }
        let inputs = [source(b"x='\xff'\n")];
        assert_eq!(
            parse_batch(&inputs).unwrap().files[0].diagnostic,
            Some(Diagnostic::InvalidUtf8)
        );
        assert_eq!(syntax("x=1\n# coding: utf-8\n"), RuffSyntax::AcceptedByRuff);
        let bytes = "\u{03bb}=1\r\nx=2\ry=3\nz=4".as_bytes();
        assert_eq!(logical_line(bytes, 1), LogicalLine(1));
        assert_eq!(logical_line(bytes, 4), LogicalLine(1));
        assert_eq!(logical_line(bytes, 5), LogicalLine(1));
        assert_eq!(logical_line(bytes, 6), LogicalLine(2));
        assert_eq!(logical_line(bytes, bytes.len()), LogicalLine(4));
        let inputs = [source("\u{feff}\u{03bb}=1\r\nx = )\r\n")];
        let report = parse_batch(&inputs).unwrap();
        assert_eq!(report.files[0].syntax_error_line, Some(LogicalLine(2)));
    }

    #[test]
    fn metadata_paths_duplicates_and_preparse_byte_limits() {
        for path in [
            "", "/x.py", "../x.py", "a/./x.py", "a//x.py", "C:/x.py", "a\\x.py", "a\0.py",
        ] {
            let mut input = source("");
            input.path = path.into();
            assert_eq!(
                parse_batch(&[input]).unwrap_err(),
                FrontEndError::InvalidPath
            );
        }
        let mut input = source("");
        input.path = "x".repeat(MAX_METADATA + 1);
        assert_eq!(
            parse_batch(&[input]).unwrap_err(),
            FrontEndError::InvalidPath
        );
        for module in ["x".repeat(MAX_METADATA + 1), "x\0y".into()] {
            let mut input = source("");
            input.module = module;
            assert_eq!(
                parse_batch(&[input]).unwrap_err(),
                FrontEndError::InvalidModule
            );
        }
        assert_eq!(
            parse_batch(&[source(""), source("")]).unwrap_err(),
            FrontEndError::DuplicatePath
        );
        let mut other = source("");
        other.path = "other.py".into();
        assert_eq!(parse_batch(&[source(""), other]).unwrap().files.len(), 2);
        for (limits, expected) in [
            (
                Limits {
                    files: 0,
                    ..Limits::default()
                },
                FrontEndError::FileCount,
            ),
            (
                Limits {
                    file_bytes: 0,
                    ..Limits::default()
                },
                FrontEndError::FileBytes,
            ),
            (
                Limits {
                    total_bytes: 0,
                    ..Limits::default()
                },
                FrontEndError::TotalBytes,
            ),
        ] {
            let mut reached_parser = false;
            assert_eq!(
                run(&[source("x=1")], limits, |at| {
                    reached_parser |= at == Checkpoint::PreParse;
                    false
                })
                .unwrap_err(),
                expected
            );
            assert!(!reached_parser);
        }
        assert_eq!(Limits::default().files, 5000);
        assert_eq!(Limits::default().file_bytes, 1024 * 1024);
        assert_eq!(Limits::default().total_bytes, 16 * 1024 * 1024);
    }

    #[test]
    fn lexical_token_depth_and_output_budgets() {
        for (limits, expected) in [
            (
                Limits {
                    tokens: 2,
                    ..Limits::default()
                },
                Diagnostic::TokenLimit,
            ),
            (
                Limits {
                    depth: 2,
                    ..Limits::default()
                },
                Diagnostic::DepthLimit,
            ),
        ] {
            let inputs = [source("x=(((1)))\n")];
            let report = run(&inputs, limits, |_| false).unwrap();
            assert_eq!(report.files[0].diagnostic, Some(expected));
            assert_eq!(report.files[0].syntax, RuffSyntax::NotParsed);
            assert!(!report.files[0].lexical_gate_finished);
        }
        let inputs = [source("x='[[[{{{((('\n# ((([[[{{{\n")];
        let report = run(
            &inputs,
            Limits {
                depth: 1,
                ..Limits::default()
            },
            |_| false,
        )
        .unwrap();
        assert_eq!(report.files[0].syntax, RuffSyntax::AcceptedByRuff);
        assert_eq!(report.files[0].max_lexical_depth, 0);
        let inputs = [source("x=1")];
        let mut parsed = false;
        assert_eq!(
            run(
                &inputs,
                Limits {
                    report: 1,
                    ..Limits::default()
                },
                |at| {
                    parsed |= at == Checkpoint::PreParse;
                    false
                }
            )
            .unwrap_err(),
            FrontEndError::ReportLimit
        );
        assert!(!parsed);
        let report = parse_batch(&inputs).unwrap();
        assert!(json_size(&report, MAX_REPORT).unwrap() <= MAX_REPORT);
        assert_eq!(
            json_size(&report, 1).unwrap_err(),
            FrontEndError::ReportLimit
        );
        // Escaping, not just raw metadata bytes, must fit before any parse.
        let inputs: Vec<_> = (0..100)
            .map(|index| {
                let mut input = source("");
                input.path = format!("{index}/{}", "\"".repeat(4000));
                input.module = "\"".repeat(MAX_METADATA);
                input
            })
            .collect();
        let mut parsed = false;
        assert_eq!(
            run(&inputs, Limits::default(), |at| {
                parsed |= at == Checkpoint::PreParse;
                false
            })
            .unwrap_err(),
            FrontEndError::ReportLimit
        );
        assert!(!parsed);
    }

    #[test]
    fn cancellation_is_latched_pre_post_parse_token_and_final() {
        for point in [
            Checkpoint::Admission,
            Checkpoint::Identity,
            Checkpoint::Hash,
            Checkpoint::Decode,
            Checkpoint::Token,
            Checkpoint::PreParse,
            Checkpoint::PostParse,
            Checkpoint::Output,
            Checkpoint::Final,
        ] {
            let inputs = [source("x=1\n")];
            let mut fired = false;
            let report = run(&inputs, Limits::default(), |at| {
                let expire = !fired && at == point;
                fired |= expire;
                expire
            })
            .unwrap();
            assert!(fired, "{point:?}");
            let file = &report.files[0];
            assert_eq!(file.admission, Admission::ResourceLimit);
            assert_eq!(file.syntax, RuffSyntax::NotParsed);
            assert_eq!(file.diagnostic, Some(Diagnostic::Deadline));
            assert_eq!(file.sha256, super::super::hash(&inputs[0].bytes));
        }
        let mut second = source("y=2\n");
        second.path = "second.py".into();
        let inputs = [source("x=1\n"), second];
        let mut parses = 0;
        let report = run(&inputs, Limits::default(), |at| {
            if at == Checkpoint::PreParse {
                parses += 1;
            }
            parses == 2
        })
        .unwrap();
        assert_eq!(parses, 2);
        assert!(report
            .files
            .iter()
            .all(|file| file.syntax == RuffSyntax::NotParsed
                && file.diagnostic == Some(Diagnostic::Deadline)));
    }

    #[cfg(unix)]
    #[test]
    #[ignore = "requires explicitly selected trusted Python 3.12; acceptance only, not fact parity"]
    fn trusted_ast_classification_oracle_with_explicit_known_differences() {
        use std::path::PathBuf;
        use std::process::{Command, Stdio};

        let python = PathBuf::from(
            std::env::var_os("IDEOMANCER_TEST_PYTHON")
                .expect("select a trusted Python 3.12 executable explicitly"),
        );
        assert!(
            python.is_absolute(),
            "oracle executable must be an absolute path"
        );
        // Independent synthetic sources are passed to ast.parse only. They are
        // never compiled, evaluated, imported, or executed by the oracle.
        let cases = [
            ("*(a if ready() else b)\n".to_owned(), true, true),
            ("def f(x: T[A][B]) -> T[A][B]: pass\n".into(), true, true),
            (
                "def f():\n    from pkg import (\n        # layout\n        name,\n    )\n".into(),
                true,
                true,
            ),
            ("f(x=1, **other, x=2)\n".into(), true, true),
            (
                "class C(K=1, **other, \u{212a}=2): pass\n".into(),
                true,
                true,
            ),
            ("f\"{mapping[\"key\"]}\"\n".into(), true, true),
            ("f'{value:=10}'\n".into(), true, true),
            ("f'{(value := call())}'\n".into(), true, true),
            ("f(x=1, 2)\n".into(), false, false),
            ("f(x for x in xs, 1)\n".into(), false, false),
            ("def f(:\n".into(), false, false),
            ("type Alias[T = int] = T\n".into(), false, false),
            // The pinned parser omits version errors in these two branches.
            // Fact lowering has an explicit all-variant default exclusion.
            ("def f[*Ts = *tuple[int]](): pass\n".into(), false, true),
            ("class C[**P = [int]]: pass\n".into(), false, true),
            // Explicit, required differences, not unexplained matching cases.
            (format!("x={}\n", "1".repeat(4301)), false, true),
            ("\u{1c89}=1\n".into(), false, true),
            ("pass # type: int\n".into(), false, true),
            ("# type: int\nx=1\n".into(), false, true),
        ];
        let texts: Vec<_> = cases.iter().map(|(text, _, _)| text).collect();
        let script = r#"
import ast, json, sys, unicodedata
assert sys.version_info[:2] == (3, 12)
assert unicodedata.unidata_version == '15.0.0'
sys.set_int_max_str_digits(4300)
results = []
for text in json.load(sys.stdin):
    try:
        ast.parse(text, type_comments=True)
    except SyntaxError:
        results.append(False)
    else:
        results.append(True)
json.dump(results, sys.stdout)
"#;
        let mut command = Command::new(python);
        command
            .args(["-I", "-S", "-c", script])
            .current_dir("/")
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let output = super::super::run_bounded(
            command,
            serde_json::to_vec(&texts).unwrap(),
            "Python Ruff test oracle",
        )
        .unwrap();
        let observed: Vec<bool> = serde_json::from_slice(&output).unwrap();
        assert_eq!(observed.len(), cases.len());
        for ((text, expected_python, expected_ruff), actual_python) in cases.iter().zip(observed) {
            assert_eq!(actual_python, *expected_python);
            let inputs = [source(text)];
            let report = parse_batch(&inputs).unwrap();
            assert_eq!(
                report.files[0].syntax == RuffSyntax::AcceptedByRuff,
                *expected_ruff
            );
            assert_eq!(
                report.capabilities.cpython_compatibility,
                CompatibilityStatus::Pending
            );
            if expected_python != expected_ruff {
                assert_ne!(
                    actual_python,
                    report.files[0].syntax == RuffSyntax::AcceptedByRuff
                );
            }
        }
    }
}
