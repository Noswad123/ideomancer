// SPDX-License-Identifier: MIT
//! Experimental raw-byte Module parser. This is not an engine backend or a
//! CPython exception, encoding, type-comment, hard-time or RSS certificate.
//! Parser entry charges conservatively bound node construction; exact boxed AST
//! allocations and allocator residency are not measured. Recursive grammar and
//! reduction chains must both respect the parser depth envelope.

use std::cell::Cell;
use std::rc::Rc;

use ruff_python_ast::{self as ast, ModModule};
use ruff_text_size::{TextRange, TextSize};

use crate::lexer::byte::{ByteLexFailure, ByteLexerLimits, ByteLexerStats, ByteViewError, SourceControl};
use crate::parser::{PResult, Parser};
use crate::token_source::FinishedSource;
use crate::{Mode, ParseError, ParseOptions, Parsed, Tokens, UnsupportedSyntaxError};

/// Bounded token-state evidence only; this never creates or exposes an AST.
#[doc(hidden)]
#[derive(Debug)]
pub struct TokenCheckpointObservation {
    pub before: Vec<crate::Token>,
    pub relexed: Vec<crate::Token>,
    pub restored: Vec<crate::Token>,
    pub before_current: crate::Token,
    pub restored_current: crate::Token,
}

/// Exercise actual recovery re-lex and checkpoint restoration on a caller's
/// bounded synthetic stream. Storage and work use the same byte-provider ledger.
#[doc(hidden)]
pub fn token_checkpoint_observation(
    source: &[u8], bumps: usize, limits: ByteParseLimits, check: &mut dyn FnMut() -> bool,
) -> Result<TokenCheckpointObservation, ByteParseFailure> {
    token_checkpoint_observation_inner(source, bumps, limits, check).map_err(Into::into)
}

fn token_checkpoint_observation_inner(
    source: &[u8], bumps: usize, limits: ByteParseLimits, check: &mut dyn FnMut() -> bool,
) -> PResult<TokenCheckpointObservation> {
    let mut stream = crate::token_source::TokenSource::from_bytes(source, Mode::Module, limits, check)?;
    for _ in 0..bumps {
        let kind = stream.current_kind();
        if kind.is_eof() { return Err(ByteLexFailure::Incomplete.into()); }
        stream.bump(kind)?;
    }
    let before = stream.copy_retained_tokens()?;
    let before_current = crate::Token::new(stream.current_kind(), stream.current_range(), stream.current_flags());
    let checkpoint = stream.checkpoint()?;
    stream.re_lex_logical_token()?;
    let relexed = stream.copy_retained_tokens()?;
    stream.rewind(checkpoint)?;
    let restored = stream.copy_retained_tokens()?;
    let restored_current = crate::Token::new(stream.current_kind(), stream.current_range(), stream.current_flags());
    Ok(TokenCheckpointObservation { before, relexed, restored, before_current, restored_current })
}

#[derive(Clone, Copy, Debug)]
pub struct ByteParseLimits {
    pub lexer: ByteLexerLimits,
    pub parser_work: usize,
    /// Active grammar/reduction depth AND nonrefundable construction entries per
    /// top-level statement. This is not measured AST height: fixed wrapper groups
    /// give a conservative composed-node envelope of `4 * parser_depth + 1`.
    /// Nested suites and speculative alternatives share the statement allowance.
    pub parser_depth: usize,
    pub parser_diagnostics: usize,
    /// Conservative cumulative charges, not live AST residency.
    pub parser_working: usize,
}

impl Default for ByteParseLimits {
    fn default() -> Self {
        Self {
            lexer: ByteLexerLimits::default(),
            parser_work: 200_000,
            parser_depth: 256,
            parser_diagnostics: 4096,
            parser_working: 32 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ByteParseFailure {
    Input(ByteLexFailure),
    View { range: TextRange, error: ByteViewError },
    Rejected { errors: Vec<ParseError>, unsupported: Vec<UnsupportedSyntaxError>, stats: ParserStats },
    UnsupportedMode,
    SourceIdExhausted,
}

/// Keep the recursive grammar's error carrier small. Diagnostic arrays and the
/// final statistics report exist only at the outer checked boundary; propagating
/// that large variant through every grammar frame regresses the legacy stack.
#[derive(Debug)]
pub(crate) enum ParserFailure {
    Input(ByteLexFailure),
    View { range: TextRange, error: ByteViewError },
    SourceIdExhausted,
}

impl From<ByteLexFailure> for ParserFailure {
    fn from(error: ByteLexFailure) -> Self { Self::Input(error) }
}

impl From<ParserFailure> for ByteParseFailure {
    fn from(error: ParserFailure) -> Self {
        match error {
            ParserFailure::Input(error) => Self::Input(error),
            ParserFailure::View { range, error } => Self::View { range, error },
            ParserFailure::SourceIdExhausted => Self::SourceIdExhausted,
        }
    }
}

impl From<ByteLexFailure> for ByteParseFailure {
    fn from(error: ByteLexFailure) -> Self { Self::Input(error) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ByteSourceError {
    ForeignOwner,
    InvalidRange,
    InvalidUtf8,
    ResourceLimit,
    Cancelled,
}

impl From<ByteViewError> for ByteSourceError {
    fn from(error: ByteViewError) -> Self {
        match error {
            ByteViewError::InvalidRange => Self::InvalidRange,
            ByteViewError::InvalidUtf8 => Self::InvalidUtf8,
            ByteViewError::ResourceLimit => Self::ResourceLimit,
            ByteViewError::Cancelled => Self::Cancelled,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ByteLocation {
    /// One-based logical line; CRLF counts as one newline.
    pub line: usize,
    /// Zero-based UTF-8 byte column, excluding an initial BOM on line one.
    pub column: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct DebugByteParts<'src> {
    pub leading: &'src [u8],
    pub expression: &'src [u8],
    pub trailing: &'src [u8],
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ParserStats {
    pub parse_invocations: usize,
    pub parser_work: usize,
    pub parser_depth_peak: usize,
    pub construction_entries_peak: usize,
    pub parser_diagnostics: usize,
    pub parser_working: usize,
    pub relex_requests: usize,
    pub relex_applied: usize,
    pub lexer: ByteLexerStats,
}

#[derive(Clone, Copy, Debug, Default)]
struct BudgetState {
    stats: ParserStats,
    depth: usize,
    construction_entries: usize,
    failed: bool,
}

#[derive(Debug)]
struct Budget {
    limits: ByteParseLimits,
    state: Cell<BudgetState>,
}

#[derive(Clone, Debug)]
pub(crate) struct ParserBudget(Rc<Budget>);

impl ParserBudget {
    pub(crate) fn new(limits: ByteParseLimits) -> Self {
        Self(Rc::new(Budget { limits, state: Cell::new(BudgetState::default()) }))
    }

    fn update(&self, update: impl FnOnce(&mut BudgetState, ByteParseLimits) -> Option<()>) -> PResult<()> {
        let mut state = self.0.state.get();
        if state.failed { return Err(ByteLexFailure::ResourceLimit.into()); }
        if update(&mut state, self.0.limits).is_none() {
            state.failed = true;
            self.0.state.set(state);
            return Err(ByteLexFailure::ResourceLimit.into());
        }
        self.0.state.set(state);
        Ok(())
    }

    pub(crate) fn work(&self, count: usize) -> PResult<()> {
        self.update(|state, limits| {
            let next = state.stats.parser_work.checked_add(count)?;
            if next > limits.parser_work { return None; }
            state.stats.parser_work = next;
            Some(())
        })
    }

    pub(crate) fn allocate(&self, count: usize) -> PResult<()> {
        self.update(|state, limits| {
            let next = state.stats.parser_working.checked_add(count)?;
            if next > limits.parser_working { return None; }
            state.stats.parser_working = next;
            Some(())
        })
    }

    pub(crate) fn diagnostics(&self, count: usize) -> PResult<()> {
        self.update(|state, limits| {
            let next = state.stats.parser_diagnostics.checked_add(count)?;
            if next > limits.parser_diagnostics { return None; }
            state.stats.parser_diagnostics = next;
            Some(())
        })
    }

    pub(crate) fn begin_parse(&self) -> PResult<()> {
        self.update(|state, _| {
            state.stats.parse_invocations = state.stats.parse_invocations.checked_add(1)?;
            Some(())
        })
    }

    pub(crate) fn relex(&self, applied: bool) -> PResult<()> {
        self.update(|state, _| {
            let counter = if applied { &mut state.stats.relex_applied } else { &mut state.stats.relex_requests };
            *counter = counter.checked_add(1)?;
            Some(())
        })
    }

    pub(crate) fn enter(&self) -> PResult<ParseGuard> {
        self.update(|state, limits| {
            let depth = state.depth.checked_add(1)?;
            let construction_entries = state.construction_entries.checked_add(1)?;
            if depth > limits.parser_depth || construction_entries > limits.parser_depth { return None; }
            state.depth = depth;
            state.construction_entries = construction_entries;
            state.stats.parser_depth_peak = state.stats.parser_depth_peak.max(depth);
            state.stats.construction_entries_peak = state.stats.construction_entries_peak.max(construction_entries);
            Some(())
        })?;
        Ok(ParseGuard { budget: Some(self.clone()), entries: 1 })
    }

    /// Only module-level sibling completion may release construction allowance.
    /// Preserve the still-active module frames and every cumulative quota/stat.
    pub(crate) fn finish_top_level_statement(&self) -> PResult<()> {
        self.update(|state, _| {
            state.construction_entries = state.depth;
            Some(())
        })
    }

    pub(crate) fn stats(&self) -> ParserStats { self.0.state.get().stats }
}

/// Owns its depth charge, so it never borrows the parser across grammar calls.
#[derive(Debug, Default)]
pub(crate) struct ParseGuard {
    budget: Option<ParserBudget>,
    entries: usize,
}

impl ParseGuard {
    /// Retain another reduction's depth without allocating a vector of guards.
    pub(crate) fn absorb(&mut self, mut next: Self) -> PResult<()> {
        match (&self.budget, &next.budget) {
            (None, None) => Ok(()),
            (Some(current), Some(other)) if Rc::ptr_eq(&current.0, &other.0) => {
                // Both charges are already included in this owner's depth.
                self.entries += next.entries;
                next.budget = None;
                Ok(())
            }
            _ => Err(ByteLexFailure::InvalidState.into()),
        }
    }
}

impl Drop for ParseGuard {
    fn drop(&mut self) {
        if let Some(budget) = &self.budget {
            let mut state = budget.0.state.get();
            state.depth -= self.entries;
            budget.0.state.set(state);
        }
    }
}

/// The only byte AST access is a callback after both diagnostic channels pass.
/// The callback can borrow or clone nodes but cannot extract an unchecked Parsed.
pub struct AcceptedByteModule<'src, 'check> {
    parsed: Parsed<ModModule>,
    control: SourceControl<'src, 'check>,
    owner: ast::DebugSourceId,
    budget: ParserBudget,
    line_starts: Vec<TextSize>,
}

impl std::fmt::Debug for AcceptedByteModule<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AcceptedByteModule").field("source_id", &self.owner)
            .field("stats", &self.stats()).finish_non_exhaustive()
    }
}

impl<'src> AcceptedByteModule<'src, '_> {
    pub fn original_bytes(&self) -> &'src [u8] { self.control.original_bytes() }

    pub fn source_bytes(&self, range: TextRange) -> Result<&'src [u8], ByteSourceError> {
        self.control.source_bytes(range).map_err(Into::into)
    }

    pub fn source_text(&mut self, range: TextRange) -> Result<&'src str, ByteSourceError> {
        self.control.source_text(range).map_err(Into::into)
    }

    pub fn source_id(&self) -> ast::DebugSourceId { self.owner }

    pub fn debug_bytes(&self, ranges: &ast::DebugSourceRanges) -> Result<DebugByteParts<'src>, ByteSourceError> {
        if ranges.owner() != self.owner { return Err(ByteSourceError::ForeignOwner); }
        if ranges.leading().end() != ranges.expression().start()
            || ranges.expression().end() != ranges.trailing().start() {
            return Err(ByteSourceError::InvalidRange);
        }
        Ok(DebugByteParts {
            leading: self.source_bytes(ranges.leading())?,
            expression: self.source_bytes(ranges.expression())?,
            trailing: self.source_bytes(ranges.trailing())?,
        })
    }

    pub fn with_module(&self, visit: impl FnOnce(&ModModule, &Tokens)) {
        visit(self.parsed.syntax(), self.parsed.tokens());
    }

    pub fn tokens(&self) -> &Tokens { self.parsed.tokens() }

    pub fn stats(&self) -> ParserStats {
        ParserStats { lexer: self.control.stats(), ..self.budget.stats() }
    }

    pub fn location(&self, offset: TextSize) -> Result<ByteLocation, ByteSourceError> {
        self.control.check_terminal()?;
        let offset_usize = offset.to_usize();
        let bytes = self.original_bytes();
        if offset_usize > bytes.len() { return Err(ByteSourceError::InvalidRange); }
        let bom = bytes.starts_with(b"\xef\xbb\xbf");
        if bom && offset_usize < 3 { return Err(ByteSourceError::InvalidRange); }
        let line = self.line_starts.partition_point(|start| *start <= offset) - 1;
        let mut column = offset_usize - self.line_starts[line].to_usize();
        if bom && line == 0 { column -= 3; }
        Ok(ByteLocation { line: line + 1, column })
    }
}

pub fn parse_module_checked<'src, 'check>(
    source: &'src [u8],
    options: ParseOptions,
    limits: ByteParseLimits,
    check: &'check mut dyn FnMut() -> bool,
) -> Result<AcceptedByteModule<'src, 'check>, ByteParseFailure> {
    if options.mode != Mode::Module { return Err(ByteParseFailure::UnsupportedMode); }
    let parser = Parser::new_bytes(source, options, limits, check)?;
    let (parsed, mut finished) = parser.parse_inner()?;
    // Reject before any AST callback, including unsupported-version-only parses.
    if !parsed.has_no_syntax_errors() {
        let Parsed { syntax, errors, unsupported_syntax_errors, .. } = parsed;
        drop(syntax);
        return Err(ByteParseFailure::Rejected { errors, unsupported: unsupported_syntax_errors, stats: finished.stats() });
    }

    let mut line_starts = Vec::new();
    finished.reserve(&mut line_starts, 1)?;
    line_starts.push(TextSize::new(0));
    let mut previous_cr = false;
    for (offset, byte) in source.iter().copied().enumerate() {
        if offset % 1024 == 0 { finished.source_work((source.len() - offset).min(1024))?; }
        if byte == b'\n' && previous_cr {
            // The preceding CR's provisional start belongs after the whole CRLF.
            if let Some(last) = line_starts.last_mut() { *last = TextSize::new((offset + 1) as u32); }
        } else if matches!(byte, b'\r' | b'\n') {
            finished.reserve(&mut line_starts, 1)?;
            line_starts.push(TextSize::new((offset + 1) as u32));
        }
        previous_cr = byte == b'\r';
    }
    finished.work(0)?;
    match (parsed.try_into_module(), finished) {
        (Some(parsed), FinishedSource::Bytes { control, owner, budget }) => {
            Ok(AcceptedByteModule { parsed, control, owner, budget, line_starts })
        }
        _ => Err(ByteLexFailure::InvalidState.into()),
    }
}
