// SPDX-License-Identifier: MIT
//! Original Ideomancer F1 facade; retain the notice in Ideomancer's root LICENSE.
//! Unstable F1 lexer experiment. No parser, encoding or admission policy.
//! Limits are cooperative and conservatively cumulative, not a hard deadline or
//! an RSS bound. A source fault is a lexer failure, never a Python exception
//! certificate. Invalid UTF-8 in a checked source view does not change stream
//! state; view resource and cancellation failures latch like lexical operations.
use super::Lexer;
use crate::token::Token;
use crate::Mode;
use ruff_text_size::{TextRange, TextSize};
use crate::error::LexicalError;
use crate::token::TokenKind;
use super::{CoreLexer, LexerCheckpoint};
use std::sync::atomic::{AtomicU64, Ordering};

/// Owned observation view. Conversion moves values without copying payloads.
#[derive(Clone, Debug, PartialEq)]
pub enum ByteTokenValue {
    None,
    Name(ruff_python_ast::name::Name),
    Int(ruff_python_ast::Int),
    Float(f64),
    Complex { real: f64, imag: f64 },
    String(Box<str>),
    InterpolatedStringMiddle(Box<str>),
    IpyEscapeCommand { value: Box<str>, kind: ruff_python_ast::IpyEscapeKind },
}

impl ByteTokenValue {
    fn from_legacy(value: crate::token::TokenValue) -> Self {
        use crate::token::TokenValue;
        match value {
            TokenValue::None => Self::None,
            TokenValue::Name(value) => Self::Name(value),
            TokenValue::Int(value) => Self::Int(value),
            TokenValue::Float(value) => Self::Float(value),
            TokenValue::Complex { real, imag } => Self::Complex { real, imag },
            TokenValue::String(value) => Self::String(value),
            TokenValue::InterpolatedStringMiddle(value) => Self::InterpolatedStringMiddle(value),
            TokenValue::IpyEscapeCommand { value, kind } => Self::IpyEscapeCommand { value, kind },
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ByteLexerLimits {
    pub input_bytes: usize,
    pub work: usize,
    pub tokens: usize,
    pub depth: usize,
    /// Cumulative conservative allocation charges, not live residency or RSS.
    pub working: usize,
    pub checkpoints: usize,
    pub diagnostics: usize,
}

impl Default for ByteLexerLimits {
    fn default() -> Self {
        Self { input_bytes: 2 * 1024 * 1024, work: 32 * 1024 * 1024,
            tokens: 200_000, depth: 256, working: 32 * 1024 * 1024,
            checkpoints: 1024, diagnostics: 10_000 }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceFault {
    pub range: TextRange,
    pub error_len: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ByteLexFailure {
    Source(SourceFault), ResourceLimit, Cancelled, ForeignCheckpoint, InvalidState, Incomplete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ByteViewError { InvalidRange, InvalidUtf8, ResourceLimit, Cancelled }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ByteLexerStats {
    pub work: usize,
    pub tokens: usize,
    pub allocated: usize,
    pub checkpoints: usize,
}

pub(super) struct Control<'check, Check: ?Sized = dyn FnMut() -> bool + 'check> {
    pub(super) limits: Option<ByteLexerLimits>,
    check: Option<&'check mut Check>,
    pub(super) stats: ByteLexerStats,
    pub(super) terminal: Option<ByteLexFailure>,
}

impl<'check, Check: FnMut() -> bool + ?Sized> Control<'check, Check> {
    pub(super) fn unlimited() -> Self {
        Self { limits: None, check: None, stats: ByteLexerStats::default(), terminal: None }
    }

    pub(super) fn fail<T>(&mut self, error: ByteLexFailure) -> Result<T, ByteLexFailure> {
        self.terminal = Some(error.clone());
        Err(error)
    }

    pub(super) fn poll(&mut self) -> Result<(), ByteLexFailure> {
        if let Some(error) = &self.terminal { return Err(error.clone()) }
        if self.check.as_mut().is_some_and(|check| check()) {
            return self.fail(ByteLexFailure::Cancelled);
        }
        Ok(())
    }

    pub(super) fn work(&mut self, count: usize) -> Result<(), ByteLexFailure> {
        if self.limits.is_none() { return Ok(()) }
        self.poll()?;
        let Some(next) = self.stats.work.checked_add(count) else { return self.fail(ByteLexFailure::ResourceLimit) };
        if next > self.limits.unwrap().work { return self.fail(ByteLexFailure::ResourceLimit) }
        self.stats.work = next;
        Ok(())
    }

    /// Validate only the requested fragment, retaining its original byte origin.
    pub(super) fn source_text<'src>(&mut self, bytes: &'src [u8], start: TextSize) -> Result<&'src str, ByteLexFailure> {
        let fault = |offset: usize, error: std::str::Utf8Error| {
            let relative = offset + error.valid_up_to();
            ByteLexFailure::Source(SourceFault {
                range: TextRange::at(start + TextSize::new(relative as u32),
                    TextSize::new(error.error_len().unwrap_or(bytes.len() - relative) as u32)),
                error_len: error.error_len(),
            })
        };
        if self.limits.is_none() {
            return std::str::from_utf8(bytes).map_err(|error| fault(0, error));
        }
        self.poll()?;
        let mut offset = 0;
        while offset < bytes.len() {
            let end = offset + (bytes.len() - offset).min(1024);
            // Charge both this bounded check and the final safe borrowed view.
            // Retried boundary bytes are charged again rather than refunded.
            self.work(2 * (end - offset))?;
            match std::str::from_utf8(&bytes[offset..end]) {
                Ok(_) => offset = end,
                Err(error) if error.error_len().is_none() && end < bytes.len() => {
                    // Retry the incomplete scalar with the next window, not a
                    // standalone continuation byte. At most three bytes overlap.
                    offset += error.valid_up_to();
                }
                Err(error) => return Err(fault(offset, error)),
            }
        }
        self.poll()?;
        // Safe Rust requires this final linear check to construct the borrow.
        // It is precharged above, but is not a hard-interruptible operation.
        std::str::from_utf8(bytes).map_err(|error| fault(0, error))
    }

    pub(super) fn allocate(&mut self, count: usize) -> Result<(), ByteLexFailure> {
        if self.limits.is_none() { return Ok(()) }
        self.poll()?;
        let Some(next) = self.stats.allocated.checked_add(count) else { return self.fail(ByteLexFailure::ResourceLimit) };
        if next > self.limits.unwrap().working { return self.fail(ByteLexFailure::ResourceLimit) }
        self.stats.allocated = next;
        Ok(())
    }

    pub(super) fn allocate_scaled(&mut self, len: usize, factor: usize, extra: usize) -> Result<(), ByteLexFailure> {
        if self.limits.is_none() { return Ok(()) }
        let Some(count) = len.checked_mul(factor).and_then(|n| n.checked_add(extra)) else {
            return self.fail(ByteLexFailure::ResourceLimit);
        };
        self.allocate(count)
    }

    pub(super) fn depth(&mut self, depth: usize) -> Result<(), ByteLexFailure> {
        if self.limits.is_some_and(|limit| depth > limit.depth) {
            return self.fail(ByteLexFailure::ResourceLimit);
        }
        Ok(())
    }
}

/// Experimental token stream over unchanged backing bytes. Malformed-byte
/// admission is promised only for Module-mode ordinary comments, not Python parsing.
pub struct ByteLexer<'src, 'check> {
    core: CoreLexer<'src, 'check>,
    owner: u64,
    initialized: bool,
    source_fault: Option<ByteLexFailure>,
}

pub struct ByteLexerCheckpoint<'src> {
    owner: u64,
    state: LexerCheckpoint,
    initialized: bool,
    source_fault: Option<ByteLexFailure>,
    errors: Vec<LexicalError>,
    source: std::marker::PhantomData<&'src [u8]>,
}

/// Parser-owned continuation of the original lexical control session.
/// Moving the control preserves cumulative charges, terminal failures and the
/// caller's callback borrow; it must not be reconstructed from the limits.
pub(crate) struct SourceControl<'src, 'check> {
    source: &'src [u8],
    control: Control<'check>,
}

impl<'src> SourceControl<'src, '_> {
    pub(crate) fn original_bytes(&self) -> &'src [u8] { self.source }

    pub(crate) fn check_terminal(&self) -> Result<(), ByteViewError> {
        match &self.control.terminal {
            Some(error) => Err(view_error(error.clone())),
            None => Ok(()),
        }
    }

    pub(crate) fn source_bytes(&self, range: TextRange) -> Result<&'src [u8], ByteViewError> {
        self.check_terminal()?;
        self.source.get(range.start().to_usize()..range.end().to_usize())
            .ok_or(ByteViewError::InvalidRange)
    }

    pub(crate) fn source_text(&mut self, range: TextRange) -> Result<&'src str, ByteViewError> {
        self.control.poll().map_err(view_error)?;
        let bytes = self.source_bytes(range)?;
        self.control.source_text(bytes, range.start()).map_err(view_error)
    }

    pub(crate) fn work(&mut self, count: usize) -> Result<(), ByteLexFailure> {
        self.control.work(count)
    }

    pub(crate) fn allocate(&mut self, count: usize) -> Result<(), ByteLexFailure> {
        self.control.allocate(count)
    }

    pub(crate) fn resource_limit<T>(&mut self) -> Result<T, ByteLexFailure> {
        self.control.poll()?;
        self.control.fail(ByteLexFailure::ResourceLimit)
    }

    pub(crate) fn stats(&self) -> ByteLexerStats { self.control.stats }
}

fn view_error(error: ByteLexFailure) -> ByteViewError {
    match error {
        ByteLexFailure::Source(_) => ByteViewError::InvalidUtf8,
        ByteLexFailure::ResourceLimit => ByteViewError::ResourceLimit,
        ByteLexFailure::Cancelled => ByteViewError::Cancelled,
        ByteLexFailure::InvalidState | ByteLexFailure::ForeignCheckpoint | ByteLexFailure::Incomplete => ByteViewError::InvalidRange,
    }
}

impl std::fmt::Debug for ByteLexer<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ByteLexer").field("initialized", &self.initialized).finish_non_exhaustive()
    }
}

impl std::fmt::Debug for ByteLexerCheckpoint<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ByteLexerCheckpoint").field("initialized", &self.initialized).finish_non_exhaustive()
    }
}

impl<'src, 'check> ByteLexer<'src, 'check> {
    pub fn new(source: &'src [u8], mode: Mode, limits: ByteLexerLimits,
        check: &'check mut dyn FnMut() -> bool) -> Result<Self, ByteLexFailure> {
        if source.len() > limits.input_bytes || source.len() > (u32::MAX / 2) as usize {
            return Err(ByteLexFailure::ResourceLimit);
        }
        static NEXT_OWNER: AtomicU64 = AtomicU64::new(1);
        let owner = NEXT_OWNER.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .map_err(|_| ByteLexFailure::ResourceLimit)?;
        let mut control = Control { limits: Some(limits), check: Some(check),
            stats: ByteLexerStats::default(), terminal: None };
        control.poll()?;
        control.depth(usize::from(mode == Mode::ParenthesizedExpression))?;
        let core = CoreLexer::new(source, mode, TextSize::new(0), control)?;
        Ok(Self { core, owner, initialized: false, source_fault: None })
    }

    fn ready(&self) -> Result<(), ByteLexFailure> {
        if let Some(error) = &self.core.cursor.control.terminal { return Err(error.clone()) }
        if let Some(error) = &self.source_fault { return Err(error.clone()) }
        Ok(())
    }

    fn current_ready(&self) -> Result<(), ByteLexFailure> {
        self.ready()?;
        if !self.initialized { return Err(ByteLexFailure::InvalidState) }
        Ok(())
    }

    pub fn next_token(&mut self) -> Result<TokenKind, ByteLexFailure> {
        self.ready()?;
        self.initialized = false;
        match self.core.next_token() {
            Ok(kind) => { self.initialized = true; Ok(kind) }
            Err(error) => {
                if matches!(error, ByteLexFailure::Source(_)) { self.source_fault = Some(error.clone()); }
                if matches!(error, ByteLexFailure::ResourceLimit | ByteLexFailure::Cancelled) {
                    self.core.cursor.control.terminal = Some(error.clone());
                }
                Err(error)
            }
        }
    }

    pub fn current_token(&self) -> Result<Token, ByteLexFailure> {
        self.current_ready()?;
        Ok(Token::new(self.core.current_kind, self.core.current_range, self.core.current_flags))
    }

    pub fn take_value(&mut self) -> Result<ByteTokenValue, ByteLexFailure> {
        self.current_ready()?;
        Ok(ByteTokenValue::from_legacy(self.core.take_value()))
    }

    pub(crate) fn take_value_for_parser(&mut self) -> Result<crate::token::TokenValue, ByteLexFailure> {
        self.current_ready()?;
        Ok(self.core.take_value())
    }

    pub(crate) fn flags_for_parser(&self) -> Result<crate::token::TokenFlags, ByteLexFailure> {
        self.current_ready()?;
        Ok(self.core.current_flags)
    }

    pub(crate) fn parser_work(&mut self, count: usize) -> Result<(), ByteLexFailure> {
        self.ready()?;
        self.core.cursor.control.work(count)
    }

    pub(crate) fn parser_allocate(&mut self, count: usize) -> Result<(), ByteLexFailure> {
        self.ready()?;
        self.core.cursor.control.allocate(count)
    }

    pub(crate) fn parser_resource_limit<T>(&mut self) -> Result<T, ByteLexFailure> {
        self.ready()?;
        self.core.cursor.control.poll()?;
        self.core.cursor.control.fail(ByteLexFailure::ResourceLimit)
    }

    pub(crate) fn rewind_for_parser(&mut self, checkpoint: ByteLexerCheckpoint<'src>) -> Result<(), ByteLexFailure> {
        // Unlike ordinary parser work, restoration is allowed after a source
        // fault. It still charges and polls the original monotonic control.
        self.core.cursor.control.work(1)?;
        self.rewind(checkpoint)
    }

    pub fn original_bytes(&self) -> &'src [u8] { self.core.source }

    pub fn source_bytes(&self, range: TextRange) -> Result<&'src [u8], ByteViewError> {
        self.core.source.get(range.start().to_usize()..range.end().to_usize()).ok_or(ByteViewError::InvalidRange)
    }

    pub fn source_text(&mut self, range: TextRange) -> Result<&'src str, ByteViewError> {
        // Poll before range checking so terminal limits cannot be bypassed with
        // empty or invalid views. A UTF-8 view error never commits a source fault.
        self.core.cursor.control.poll().and_then(|()| self.core.source_text(range))
            .map_err(|error| match error {
                ByteLexFailure::Source(_) => ByteViewError::InvalidUtf8,
                ByteLexFailure::ResourceLimit => ByteViewError::ResourceLimit,
                ByteLexFailure::Cancelled => ByteViewError::Cancelled,
                ByteLexFailure::InvalidState | ByteLexFailure::ForeignCheckpoint | ByteLexFailure::Incomplete => ByteViewError::InvalidRange,
            })
    }

    pub fn checkpoint(&mut self) -> Result<ByteLexerCheckpoint<'src>, ByteLexFailure> {
        self.core.cursor.control.poll()?;
        let control = &mut self.core.cursor.control;
        let Some(next) = control.stats.checkpoints.checked_add(1) else { return control.fail(ByteLexFailure::ResourceLimit) };
        if control.limits.is_some_and(|limit| next > limit.checkpoints) { return control.fail(ByteLexFailure::ResourceLimit) }
        control.stats.checkpoints = next;
        // Precharge cloned values and both stacks before the legacy snapshot clones.
        if let Err(error) = self.core.charge_checkpoint() { return self.core.cursor.control.fail(error) }
        // Preserve diagnostics even if snapshots are restored in non-LIFO order.
        let mut errors = Vec::new();
        let control = &mut self.core.cursor.control;
        let Some(bytes) = self.core.errors.len().checked_mul(256) else { return control.fail(ByteLexFailure::ResourceLimit) };
        control.allocate(bytes)?;
        if errors.try_reserve(self.core.errors.len()).is_err() { return control.fail(ByteLexFailure::ResourceLimit) }
        for error in &self.core.errors {
            control.work(1)?;
            if let crate::error::LexicalErrorType::OtherError(message) = error.error() {
                let Some(bytes) = message.len().checked_mul(2) else { return control.fail(ByteLexFailure::ResourceLimit) };
                control.allocate(bytes)?;
                control.work(message.len())?;
            }
            errors.push(error.clone());
        }
        let state = match self.core.try_checkpoint() {
            Ok(state) => state,
            Err(error) => return self.core.cursor.control.fail(error),
        };
        Ok(ByteLexerCheckpoint { owner: self.owner, state,
            initialized: self.initialized, source_fault: self.source_fault.clone(), errors, source: std::marker::PhantomData })
    }

    pub fn rewind(&mut self, checkpoint: ByteLexerCheckpoint<'src>) -> Result<(), ByteLexFailure> {
        if checkpoint.owner != self.owner { return Err(ByteLexFailure::ForeignCheckpoint) }
        self.core.cursor.control.poll()?;
        self.core.rewind(checkpoint.state);
        self.core.errors = checkpoint.errors;
        self.initialized = checkpoint.initialized;
        self.source_fault = checkpoint.source_fault;
        Ok(())
    }

    pub fn re_lex_logical_token(&mut self, start: Option<TextSize>) -> Result<bool, ByteLexFailure> {
        self.current_ready()?;
        if let Some(start) = start {
            if start >= self.core.offset() || !matches!(self.core.source.get(start.to_usize()), Some(b'\n' | b'\r')) {
                return Err(ByteLexFailure::InvalidState);
            }
        }
        match self.core.re_lex_logical_token(start) {
            Ok(changed) => Ok(changed),
            Err(error) => {
                self.initialized = false;
                if matches!(error, ByteLexFailure::Source(_)) { self.source_fault = Some(error.clone()); }
                if matches!(error, ByteLexFailure::ResourceLimit | ByteLexFailure::Cancelled) {
                    self.core.cursor.control.terminal = Some(error.clone());
                }
                Err(error)
            }
        }
    }

    pub fn lexical_errors(&self) -> &[LexicalError] { &self.core.errors }
    pub fn stats(&self) -> ByteLexerStats { self.core.cursor.control.stats }

    pub fn finish(mut self) -> Result<Vec<LexicalError>, ByteLexFailure> {
        self.ready()?;
        self.core.cursor.control.poll()?;
        if !self.initialized || !self.core.current_kind.is_eof() || !self.core.cursor.is_eof() {
            return Err(ByteLexFailure::Incomplete);
        }
        Ok(self.core.finish())
    }

    pub(crate) fn finish_for_parser(mut self) -> Result<(Vec<LexicalError>, SourceControl<'src, 'check>), ByteLexFailure> {
        self.ready()?;
        self.core.cursor.control.poll()?;
        if !self.initialized || !self.core.current_kind.is_eof() || !self.core.cursor.is_eof() {
            return Err(ByteLexFailure::Incomplete);
        }
        Ok((self.core.errors, SourceControl {
            source: self.core.source,
            control: self.core.cursor.control,
        }))
    }
}

/// Capture full legacy token/flag/value/error observations for bounded fixtures.
/// This probe is deliberately separate from any future byte facade.
#[doc(hidden)]
pub fn legacy_trace(source: &str, mode: Mode) -> (Vec<String>, String) {
    assert!(source.len() <= 32 * 1024, "bounded F1 fixture");
    let mut lexer = Lexer::new(source, mode, 0.into());
    let mut tokens = Vec::new();
    loop {
        assert!(tokens.len() < 200_000, "bounded F1 tokens");
        let kind = lexer.next_token();
        let token = Token::new(kind, lexer.current_range(), lexer.current_flags());
        let value = lexer.take_value();
        tokens.push(format!("{token:?} {value:?}"));
        if kind.is_eof() {
            break;
        }
    }
    (tokens, format!("{:?}", lexer.finish()))
}
