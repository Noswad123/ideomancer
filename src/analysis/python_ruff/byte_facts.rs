//! Bounded raw-UTF-8 byte admission for the private fact experiment.
use super::*;
use ruff_python_parser::byte::{
    parse_module_checked, ByteParseFailure, ByteParseLimits, ByteSourceError, ParserStats,
};
use ruff_python_parser::lexer::byte::{ByteLexFailure, ByteViewError};
use std::cell::RefCell;

#[derive(Debug)]
pub(in crate::analysis) enum ByteInputFailure {
    Input(ByteLexFailure),
    View {
        range: TextRange,
        error: ByteViewError,
    },
    UnsupportedMode,
    SourceIdExhausted,
}

#[derive(Debug)]
pub(in crate::analysis) enum ByteOutcome {
    NotParsed(&'static str),
    Failed(ByteInputFailure),
    /// Fixed-size counts only: parser-owned diagnostic vectors/strings must not
    /// accumulate across files outside their per-parse allocation ledger.
    Rejected {
        syntax_errors: usize,
        unsupported_errors: usize,
        stats: ParserStats,
    },
    Parsed {
        stats: ParserStats,
    },
}

pub(super) fn run_context<'src, F: FnMut(Checkpoint) -> bool>(
    sources: &'src [Source],
    limits: Limits,
    cancel: &mut Cancellation<F>,
    consumer: &mut ByteFactConsumer<'_>,
    outcomes: &mut Vec<ByteOutcome>,
) -> std::result::Result<BatchReport<'src>, FrontEndError> {
    // Source validation belongs to the caller, before its transactional sink.
    let mut report = BatchReport {
        identity: identity(cancel),
        capabilities: Capabilities::default(),
        files: Vec::new(),
    };
    let mut reserved = json_size(&report, limits.report)?;
    for (index, source) in sources.iter().enumerate() {
        cancel.at(Checkpoint::Admission);
        let mut worst = file_report(source);
        worst.admission = Admission::UnsupportedCompatibility;
        worst.syntax = RuffSyntax::SyntaxAndUnsupportedVersion;
        worst.diagnostic = Some(Diagnostic::EncodingCookieUnsupported);
        worst.syntax_errors = usize::MAX;
        worst.unsupported_version_errors = usize::MAX;
        worst.tokens_checked = usize::MAX;
        worst.max_lexical_depth = usize::MAX;
        // Closed static reasons fit this allowance; raw diagnostics never enter
        // the serialized report. No byte-path exception/line is certified.
        reserved = reserved
            .checked_add(json_size(&worst, limits.report)?)
            .and_then(|count| count.checked_add(512 + usize::from(index != 0)))
            .filter(|&count| count <= limits.report)
            .ok_or(FrontEndError::ReportLimit)?;
    }
    report
        .files
        .try_reserve_exact(sources.len())
        .map_err(|_| FrontEndError::ReportLimit)?;
    outcomes.clear();
    outcomes
        .try_reserve_exact(sources.len())
        .map_err(|_| FrontEndError::ReportLimit)?;
    for (index, source) in sources.iter().enumerate() {
        let mut file = file_report(source);
        // Hash every original byte even after cancellation, including the BOM.
        file.sha256 = digest(&source.bytes, cancel, Checkpoint::Hash);
        let outcome = classify(source, index, &mut file, limits, cancel, consumer);
        cancel.at(Checkpoint::PostDrop);
        report.files.push(file);
        outcomes.push(outcome);
    }
    cancel.at(Checkpoint::Output);
    json_size(&report, limits.report)?;
    if cancel.at(Checkpoint::Final) {
        for (file, source) in report.files.iter_mut().zip(sources) {
            let sha256 = std::mem::take(&mut file.sha256);
            *file = file_report(source);
            file.sha256 = sha256;
        }
    }
    Ok(report)
}

fn classify<F: FnMut(Checkpoint) -> bool>(
    source: &Source,
    index: usize,
    file: &mut FileReport<'_>,
    limits: Limits,
    cancel: &mut Cancellation<F>,
    consumer: &mut ByteFactConsumer<'_>,
) -> ByteOutcome {
    let bytes = source.bytes.as_slice();
    if cancel.at(Checkpoint::Decode) {
        return ByteOutcome::NotParsed("deadline");
    }
    for chunk in bytes.chunks(256) {
        if cancel.at(Checkpoint::Decode) {
            return ByteOutcome::NotParsed("deadline");
        }
        if chunk.contains(&0) {
            file.admission = Admission::InvalidSource;
            file.diagnostic = Some(Diagnostic::SourceSyntax);
            file.failure_reason = Some(source::NULL_BYTE);
            return ByteOutcome::NotParsed(source::NULL_BYTE);
        }
    }
    match source::raw_utf8_domain(bytes, &mut || cancel.at(Checkpoint::Decode)) {
        Ok(true) => {}
        Ok(false) | Err(source::Error::UnsupportedCodec) => {
            file.admission = Admission::UnsupportedEncoding;
            file.diagnostic = Some(Diagnostic::EncodingCookieUnsupported);
            file.failure_reason = Some("outside raw UTF-8 source profile");
            return ByteOutcome::NotParsed("outside raw UTF-8 source profile");
        }
        Err(source::Error::UnresolvedCodec) => {
            file.admission = Admission::UnsupportedEncoding;
            file.diagnostic = Some(Diagnostic::UnresolvedCodec);
            file.failure_reason = Some("unresolved codec membership");
            return ByteOutcome::NotParsed("unresolved codec membership");
        }
        Err(source::Error::Syntax { reason, .. }) => {
            file.admission = Admission::InvalidSource;
            file.diagnostic = Some(Diagnostic::SourceSyntax);
            file.failure_reason = Some(reason);
            return ByteOutcome::NotParsed(reason);
        }
        Err(source::Error::ResourceLimit) => {
            file.diagnostic = Some(Diagnostic::DecodedLimit);
            return ByteOutcome::NotParsed("source resource limit");
        }
        Err(source::Error::Cancelled) => return ByteOutcome::NotParsed("deadline"),
    }
    if cancel.at(Checkpoint::PreParse) {
        return ByteOutcome::NotParsed("deadline");
    }
    // The accepted module retains its parser callback. Interior mutability lets
    // subsequent compatibility/semantic checks share the same cancellation latch.
    let cancel = RefCell::new(cancel);
    let mut parser_check = || cancel.borrow_mut().at(Checkpoint::Token);
    // Preserve every F2 default, including the per-statement construction bound.
    let parsed = parse_module_checked(
        bytes,
        options(),
        ByteParseLimits::default(),
        &mut parser_check,
    );
    cancel.borrow_mut().at(Checkpoint::PostParse);
    let mut accepted = match parsed {
        Ok(accepted) => accepted,
        Err(failure) => {
            file.failure_reason = Some("byte parser compatibility boundary");
            match &failure {
                ByteParseFailure::Rejected {
                    errors,
                    unsupported,
                    stats,
                } => {
                    file.admission = Admission::Admitted;
                    file.diagnostic = None;
                    file.syntax = if errors.is_empty() {
                        RuffSyntax::UnsupportedVersion
                    } else {
                        RuffSyntax::StructuredSyntax
                    };
                    file.failure_reason = Some("parser diagnostics");
                    file.syntax_errors = errors.len().min(MAX_ERRORS);
                    file.unsupported_version_errors = unsupported.len().min(MAX_ERRORS);
                    file.error_counts_capped =
                        errors.len() > MAX_ERRORS || unsupported.len() > MAX_ERRORS;
                    file.tokens_checked = stats.lexer.tokens;
                }
                ByteParseFailure::Input(ByteLexFailure::Source(_)) => {
                    file.admission = Admission::InvalidSource;
                    file.diagnostic = Some(Diagnostic::SourceSyntax);
                    file.failure_reason = Some("byte source fault");
                }
                ByteParseFailure::Input(ByteLexFailure::ResourceLimit)
                | ByteParseFailure::View {
                    error: ByteViewError::ResourceLimit,
                    ..
                } => {
                    file.admission = Admission::ResourceLimit;
                    file.diagnostic = Some(Diagnostic::CompatibilityLimit);
                    file.failure_reason = Some("byte parser resource limit");
                }
                ByteParseFailure::Input(ByteLexFailure::Cancelled)
                | ByteParseFailure::View {
                    error: ByteViewError::Cancelled,
                    ..
                } => {
                    file.admission = Admission::ResourceLimit;
                    file.diagnostic = Some(Diagnostic::Deadline);
                    file.failure_reason = Some("deadline");
                }
                _ => {
                    file.admission = Admission::UnsupportedCompatibility;
                    file.diagnostic = Some(Diagnostic::CompatibilityFailure);
                }
            }
            return match failure {
                ByteParseFailure::Rejected {
                    errors,
                    unsupported,
                    stats,
                } => ByteOutcome::Rejected {
                    syntax_errors: errors.len(),
                    unsupported_errors: unsupported.len(),
                    stats,
                },
                ByteParseFailure::Input(error) => {
                    ByteOutcome::Failed(ByteInputFailure::Input(error))
                }
                ByteParseFailure::View { range, error } => {
                    ByteOutcome::Failed(ByteInputFailure::View { range, error })
                }
                ByteParseFailure::UnsupportedMode => {
                    ByteOutcome::Failed(ByteInputFailure::UnsupportedMode)
                }
                ByteParseFailure::SourceIdExhausted => {
                    ByteOutcome::Failed(ByteInputFailure::SourceIdExhausted)
                }
            };
        }
    };
    file.admission = Admission::Admitted;
    file.syntax = RuffSyntax::AcceptedByRuff;
    file.diagnostic = None;
    let mut check = || cancel.borrow_mut().at(Checkpoint::Compatibility);
    let mut working = 0usize;
    let mut budget = |bytes: usize| {
        working = working
            .checked_add(bytes)
            .filter(|&total| total <= limits.compatibility_working)
            .ok_or(failures::Error::ResourceLimit)?;
        Ok(())
    };
    let admission = (|| {
        let resource = (
            Admission::ResourceLimit,
            Diagnostic::CompatibilityLimit,
            "byte compatibility resource limit",
        );
        let deadline = (Admission::ResourceLimit, Diagnostic::Deadline, "deadline");
        let invalid = (
            Admission::InvalidSource,
            Diagnostic::SourceSyntax,
            "invalid byte token source view",
        );
        match failures::inspect_accepted_bytes(bytes, accepted.tokens(), &mut budget, &mut check) {
            Ok(Some(detected)) => {
                return Err((
                    Admission::UnsupportedCompatibility,
                    Diagnostic::CompatibilityFailure,
                    detected.reason,
                ));
            }
            Ok(None) => {}
            Err(failures::Error::InvalidSource) => return Err(invalid),
            Err(failures::Error::Cancelled) => return Err(deadline),
            Err(failures::Error::ResourceLimit) => return Err(resource),
        }
        match type_comments::check_byte_comment_payloads(bytes, accepted.tokens(), &mut check) {
            Ok(()) => {}
            Err(type_comments::Error::Cancelled) => return Err(deadline),
            Err(type_comments::Error::WorkingLimit) => return Err(resource),
            Err(type_comments::Error::UnsupportedCategory(reason)) => {
                return Err((
                    Admission::UnsupportedCompatibility,
                    Diagnostic::CompatibilityFailure,
                    reason,
                ));
            }
            Err(type_comments::Error::Syntax { .. }) => {
                return Err((
                    Admission::UnsupportedCompatibility,
                    Diagnostic::CompatibilityFailure,
                    "byte type-comment compatibility",
                ));
            }
        }
        let mut count = 0usize;
        for token in accepted.tokens() {
            if check() {
                return Err(deadline);
            }
            if token.kind() == TokenKind::String {
                count = count.checked_add(1).ok_or(resource)?;
            }
        }
        budget(
            count
                .checked_mul(std::mem::size_of::<ValidatedString<'_>>())
                .ok_or(resource)?,
        )
        .map_err(|_| resource)?;
        let mut strings = Vec::new();
        strings.try_reserve_exact(count).map_err(|_| resource)?;
        // Tokens are in raw source order, including BOM offsets. Stage only
        // checked borrowed spellings, not payload copies or cloned AST nodes.
        for index in 0..accepted.tokens().len() {
            if check() {
                return Err(deadline);
            }
            let token = &accepted.tokens()[index];
            if token.kind() != TokenKind::String {
                continue;
            }
            let range = token.range();
            let text = accepted.source_text(range).map_err(|error| match error {
                ByteSourceError::Cancelled => deadline,
                ByteSourceError::ResourceLimit => resource,
                _ => invalid,
            })?;
            strings.push(ValidatedString { range, text });
        }
        if cancel.borrow_mut().at(Checkpoint::Semantic) {
            return Err(deadline);
        }
        accepted.with_module(|module, tokens| {
            consumer(
                index,
                FactSource::Bytes {
                    bytes,
                    strings: &strings,
                },
                module,
                tokens,
                &mut || cancel.borrow_mut().at(Checkpoint::Semantic),
            );
        });
        if cancel.borrow_mut().at(Checkpoint::PostConsume) {
            return Err(deadline);
        }
        Ok(())
    })();
    if let Err((admission, diagnostic, reason)) = admission {
        file.admission = admission;
        file.diagnostic = Some(diagnostic);
        file.failure_reason = Some(reason);
    }
    // Include checked-view work, and sample only after the consumer and control
    // checks. Parsed records parser acceptance, not compatibility/fact admission.
    let stats = accepted.stats();
    file.tokens_checked = stats.lexer.tokens;
    ByteOutcome::Parsed { stats }
}
