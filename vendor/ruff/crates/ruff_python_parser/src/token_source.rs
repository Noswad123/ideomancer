use ruff_python_ast::{DebugSourceId, DebugSourceRanges, DebugText};
use ruff_text_size::{Ranged, TextRange, TextSize};

use crate::byte::{ParserFailure, ByteParseLimits, ParseGuard, ParserBudget};
use crate::error::LexicalError;
use crate::lexer::byte::{ByteLexFailure, ByteLexer, ByteLexerCheckpoint, ByteViewError, SourceControl};
use crate::lexer::{Lexer, LexerCheckpoint};
use crate::parser::PResult;
use crate::token::{Token, TokenFlags, TokenKind, TokenValue};
use crate::Mode;

/// Closed source bank: the byte lane never manufactures a whole-source `str`.
#[derive(Debug)]
enum TokenSourceBank<'src, 'check> {
    Legacy { lexer: Lexer<'src>, source: &'src str },
    Bytes { lexer: ByteLexer<'src, 'check>, owner: DebugSourceId, budget: ParserBudget },
}

enum LexerBankCheckpoint<'src> {
    Legacy(LexerCheckpoint),
    Bytes(ByteLexerCheckpoint<'src>),
}

pub(crate) enum FinishedSource<'src, 'check> {
    Legacy,
    Bytes { control: SourceControl<'src, 'check>, owner: DebugSourceId, budget: ParserBudget },
}

/// The requested additional capacity and its conservative cumulative byte charge.
/// Geometric growth avoids one allocator call per token on the bounded byte lane.
fn reservation<T>(values: &Vec<T>, additional: usize) -> Option<(usize, usize)> {
    let needed = values.len().checked_add(additional)?;
    if needed <= values.capacity() { return Some((0, 0)); }
    let capacity = needed.max(values.capacity().checked_mul(2)?).max(8);
    let charge = capacity.checked_sub(values.capacity())?.checked_mul(std::mem::size_of::<T>())?.checked_mul(2)?;
    Some((capacity.checked_sub(values.len())?, charge))
}

impl FinishedSource<'_, '_> {
    pub(crate) fn stats(&self) -> crate::byte::ParserStats {
        match self {
            Self::Legacy => crate::byte::ParserStats::default(),
            Self::Bytes { control, budget, .. } => crate::byte::ParserStats { lexer: control.stats(), ..budget.stats() },
        }
    }

    pub(crate) fn source_work(&mut self, count: usize) -> PResult<()> {
        if let Self::Bytes { control, .. } = self { control.work(count)?; }
        Ok(())
    }

    pub(crate) fn work(&mut self, count: usize) -> PResult<()> {
        if let Self::Bytes { control, budget, .. } = self {
            control.work(count)?;
            if budget.work(count).is_err() { return control.resource_limit().map_err(Into::into); }
        }
        Ok(())
    }

    pub(crate) fn reserve<T>(&mut self, values: &mut Vec<T>, additional: usize) -> PResult<()> {
        if let Self::Bytes { control, budget, .. } = self {
            control.work(0)?;
            let Some((reserve, charge)) = reservation(values, additional) else {
                return control.resource_limit().map_err(Into::into);
            };
            if budget.allocate(charge).is_err() { return control.resource_limit().map_err(Into::into); }
            control.allocate(charge)?;
            if reserve != 0 && values.try_reserve_exact(reserve).is_err() {
                return control.resource_limit().map_err(Into::into);
            }
        }
        Ok(())
    }

    pub(crate) fn diagnostics(&mut self, count: usize) -> PResult<()> {
        if let Self::Bytes { control, budget, .. } = self {
            control.work(0)?;
            if budget.diagnostics(count).is_err() { return control.resource_limit().map_err(Into::into); }
        }
        Ok(())
    }
}

impl<'src, 'check> TokenSourceBank<'src, 'check> {
    fn is_legacy(&self) -> bool { matches!(self, Self::Legacy { .. }) }

    fn work(&mut self, count: usize) -> PResult<()> {
        if let Self::Bytes { lexer, budget, .. } = self {
            lexer.parser_work(count)?;
            if budget.work(count).is_err() { return lexer.parser_resource_limit().map_err(Into::into); }
        }
        Ok(())
    }

    fn allocate(&mut self, count: usize) -> PResult<()> {
        if let Self::Bytes { lexer, budget, .. } = self {
            lexer.parser_work(0)?;
            if budget.allocate(count).is_err() { return lexer.parser_resource_limit().map_err(Into::into); }
            lexer.parser_allocate(count)?;
        }
        Ok(())
    }

    fn source_work(&mut self, count: usize) -> PResult<()> {
        self.work(1)?;
        if let Self::Bytes { lexer, .. } = self { lexer.parser_work(count)?; }
        Ok(())
    }

    fn resource_limit<T>(&mut self) -> PResult<T> {
        match self {
            Self::Bytes { lexer, .. } => lexer.parser_resource_limit().map_err(Into::into),
            Self::Legacy { .. } => Err(ByteLexFailure::InvalidState.into()),
        }
    }

    fn reserve<T>(&mut self, values: &mut Vec<T>, additional: usize) -> PResult<()> {
        if !self.is_legacy() {
            let Some((reserve, charge)) = reservation(values, additional) else { return self.resource_limit(); };
            self.allocate(charge)?;
            if reserve != 0 && values.try_reserve_exact(reserve).is_err() { return self.resource_limit(); }
        }
        Ok(())
    }

    fn next_token(&mut self) -> PResult<Token> {
        self.work(1)?;
        match self {
            Self::Legacy { lexer, .. } => {
                let kind = lexer.next_token();
                Ok(Token::new(kind, lexer.current_range(), lexer.current_flags()))
            }
            Self::Bytes { lexer, .. } => {
                lexer.next_token()?;
                Ok(lexer.current_token()?)
            }
        }
    }

    fn take_value(&mut self) -> PResult<TokenValue> {
        self.work(1)?;
        match self {
            Self::Legacy { lexer, .. } => Ok(lexer.take_value()),
            Self::Bytes { lexer, .. } => Ok(lexer.take_value_for_parser()?),
        }
    }

    fn current_flags(&self) -> PResult<TokenFlags> {
        match self {
            Self::Legacy { lexer, .. } => Ok(lexer.current_flags()),
            Self::Bytes { lexer, .. } => Ok(lexer.flags_for_parser()?),
        }
    }

    fn checkpoint(&mut self) -> PResult<LexerBankCheckpoint<'src>> {
        self.work(1)?;
        match self {
            Self::Legacy { lexer, .. } => Ok(LexerBankCheckpoint::Legacy(lexer.checkpoint())),
            Self::Bytes { lexer, .. } => Ok(LexerBankCheckpoint::Bytes(lexer.checkpoint()?)),
        }
    }

    fn rewind(&mut self, checkpoint: LexerBankCheckpoint<'src>) -> PResult<()> {
        // Do not call parser_work/ready here: an owned snapshot must be able to
        // restore a source fault. ByteLexer::rewind still polls terminal control.
        match (self, checkpoint) {
            (Self::Legacy { lexer, .. }, LexerBankCheckpoint::Legacy(checkpoint)) => {
                lexer.rewind(checkpoint);
                Ok(())
            }
            (Self::Bytes { lexer, budget, .. }, LexerBankCheckpoint::Bytes(checkpoint)) => {
                lexer.rewind_for_parser(checkpoint)?;
                if budget.work(1).is_err() { return lexer.parser_resource_limit().map_err(Into::into); }
                Ok(())
            }
            _ => Err(ByteLexFailure::ForeignCheckpoint.into()),
        }
    }

    fn relex(&mut self, start: Option<TextSize>) -> PResult<(bool, Token)> {
        self.work(1)?;
        match self {
            Self::Legacy { lexer, .. } => {
                let changed = lexer.re_lex_logical_token(start);
                Ok((changed, Token::new(lexer.current_kind(), lexer.current_range(), lexer.current_flags())))
            }
            Self::Bytes { lexer, budget, .. } => {
                if budget.relex(false).is_err() { return lexer.parser_resource_limit().map_err(Into::into); }
                let changed = lexer.re_lex_logical_token(start)?;
                if changed && budget.relex(true).is_err() { return lexer.parser_resource_limit().map_err(Into::into); }
                Ok((changed, lexer.current_token()?))
            }
        }
    }

    fn source_bytes(&self, range: TextRange) -> PResult<&'src [u8]> {
        let bytes = match self {
            Self::Legacy { source, .. } => source.as_bytes(),
            Self::Bytes { lexer, .. } => lexer.original_bytes(),
        };
        bytes.get(range.start().to_usize()..range.end().to_usize())
            .ok_or(ParserFailure::View { range, error: ByteViewError::InvalidRange })
    }

    fn source_text(&mut self, range: TextRange) -> PResult<&'src str> {
        self.work(1)?;
        match self {
            Self::Legacy { source, .. } => Ok(&source[range]),
            Self::Bytes { lexer, .. } => lexer.source_text(range).map_err(|error| match error {
                ByteViewError::Cancelled => ParserFailure::Input(ByteLexFailure::Cancelled),
                ByteViewError::ResourceLimit => ParserFailure::Input(ByteLexFailure::ResourceLimit),
                ByteViewError::InvalidRange | ByteViewError::InvalidUtf8 => ParserFailure::View { range, error },
            }),
        }
    }

    fn debug_text(&mut self, leading: TextRange, expression: TextRange, trailing: TextRange) -> PResult<DebugText> {
        self.work(1)?;
        match self {
            Self::Legacy { source, .. } => Ok(DebugText::Text {
                leading: source[leading].to_string(),
                trailing: source[trailing].to_string(),
            }),
            Self::Bytes { lexer, owner, .. } => {
                DebugSourceRanges::new(*owner, leading, expression, trailing,
                    TextSize::new(lexer.original_bytes().len() as u32))
                    .map(DebugText::Source)
                    .ok_or(ParserFailure::View { range: trailing, error: ByteViewError::InvalidRange })
            }
        }
    }

    fn finish(self) -> PResult<(Vec<LexicalError>, FinishedSource<'src, 'check>)> {
        match self {
            Self::Legacy { lexer, .. } => Ok((lexer.finish(), FinishedSource::Legacy)),
            Self::Bytes { lexer, owner, budget } => {
                let (errors, control) = lexer.finish_for_parser()?;
                Ok((errors, FinishedSource::Bytes { control, owner, budget }))
            }
        }
    }
}

/// Trivia-retaining stream with a cache established only by successful advance.
#[derive(Debug)]
pub(crate) struct TokenSource<'src, 'check> {
    bank: TokenSourceBank<'src, 'check>,
    current: Token,
    flags: TokenFlags,
    tokens: Vec<Token>,
}

impl<'src, 'check> TokenSource<'src, 'check> {
    fn from_bank(mut bank: TokenSourceBank<'src, 'check>) -> PResult<Self> {
        let mut tokens = Vec::new();
        let current = loop {
            let token = bank.next_token()?;
            if !token.kind().is_trivia() { break token; }
            bank.reserve(&mut tokens, 1)?;
            tokens.push(token);
        };
        let flags = bank.current_flags()?;
        Ok(Self { bank, current, flags, tokens })
    }

    pub(crate) fn from_source(source: &'src str, mode: Mode, start_offset: TextSize) -> PResult<Self> {
        Self::from_bank(TokenSourceBank::Legacy { lexer: Lexer::new(source, mode, start_offset), source })
    }

    pub(crate) fn from_bytes(source: &'src [u8], mode: Mode, limits: ByteParseLimits,
        check: &'check mut dyn FnMut() -> bool) -> PResult<Self> {
        let owner = DebugSourceId::fresh().ok_or(ParserFailure::SourceIdExhausted)?;
        let mut lexer = ByteLexer::new(source, mode, limits.lexer, check)?;
        // Precharge the small shared quota owner before allocating its Rc.
        if limits.parser_working < 512 { return lexer.parser_resource_limit().map_err(Into::into); }
        lexer.parser_allocate(512)?;
        let budget = ParserBudget::new(limits);
        if budget.allocate(512).is_err() { return lexer.parser_resource_limit().map_err(Into::into); }
        Self::from_bank(TokenSourceBank::Bytes { lexer, owner, budget })
    }

    pub(crate) fn is_legacy(&self) -> bool { self.bank.is_legacy() }
    pub(crate) fn copy_retained_tokens(&mut self) -> PResult<Vec<Token>> {
        let mut copy = Vec::new();
        self.bank.reserve(&mut copy, self.tokens.len())?;
        for chunk in self.tokens.chunks(256) {
            self.bank.work(chunk.len())?;
            copy.extend_from_slice(chunk);
        }
        Ok(copy)
    }
    pub(crate) fn current_kind(&self) -> TokenKind { self.current.kind() }
    pub(crate) fn current_range(&self) -> TextRange { self.current.range() }
    pub(crate) fn current_flags(&self) -> TokenFlags { self.flags }
    pub(crate) fn take_value(&mut self) -> PResult<TokenValue> { self.bank.take_value() }
    pub(crate) fn work(&mut self, count: usize) -> PResult<()> { self.bank.work(count) }
    pub(crate) fn source_work(&mut self, count: usize) -> PResult<()> { self.bank.source_work(count) }
    pub(crate) fn allocate(&mut self, count: usize) -> PResult<()> { self.bank.allocate(count) }
    pub(crate) fn reserve<T>(&mut self, values: &mut Vec<T>, additional: usize) -> PResult<()> {
        self.bank.reserve(values, additional)
    }
    pub(crate) fn source_bytes(&self, range: TextRange) -> PResult<&'src [u8]> { self.bank.source_bytes(range) }
    pub(crate) fn source_text(&mut self, range: TextRange) -> PResult<&'src str> { self.bank.source_text(range) }
    pub(crate) fn debug_text(&mut self, leading: TextRange, expression: TextRange, trailing: TextRange) -> PResult<DebugText> {
        self.bank.debug_text(leading, expression, trailing)
    }

    pub(crate) fn begin_parse(&mut self) -> PResult<()> {
        self.work(0)?;
        if let TokenSourceBank::Bytes { budget, .. } = &self.bank {
            if budget.begin_parse().is_err() { return self.bank.resource_limit(); }
        }
        Ok(())
    }

    pub(crate) fn finish_top_level_statement(&mut self) -> PResult<()> {
        self.work(0)?;
        if let TokenSourceBank::Bytes { budget, .. } = &self.bank {
            if budget.finish_top_level_statement().is_err() { return self.bank.resource_limit(); }
        }
        Ok(())
    }

    pub(crate) fn enter_parse(&mut self) -> PResult<ParseGuard> {
        self.work(1)?;
        self.bank.allocate(512)?;
        match &self.bank {
            TokenSourceBank::Legacy { .. } => Ok(ParseGuard::default()),
            TokenSourceBank::Bytes { budget, .. } => match budget.enter() {
                Ok(guard) => Ok(guard),
                Err(_) => self.bank.resource_limit(),
            },
        }
    }

    pub(crate) fn diagnostic(&mut self) -> PResult<()> {
        self.work(1)?;
        if let TokenSourceBank::Bytes { budget, .. } = &self.bank {
            if budget.diagnostics(1).is_err() { return self.bank.resource_limit(); }
        }
        Ok(())
    }

    pub(crate) fn re_lex_logical_token(&mut self) -> PResult<()> {
        let mut start = None;
        for index in (0..self.tokens.len()).rev() {
            self.work(1)?;
            let token = self.tokens[index];
            match token.kind() {
                TokenKind::NonLogicalNewline => start = Some(token.start()),
                TokenKind::Comment => continue,
                _ => break,
            }
        }
        let (changed, current) = self.bank.relex(start)?;
        self.flags = self.bank.current_flags()?;
        self.current = current;
        if changed {
            while self.tokens.last().is_some_and(|last| last.start() >= current.start()) {
                self.work(1)?;
                self.tokens.pop();
            }
        }
        Ok(())
    }

    pub(crate) fn peek(&mut self) -> PResult<TokenKind> {
        let checkpoint = self.bank.checkpoint()?;
        let result = self.next_non_trivia_token();
        let restored = self.bank.rewind(checkpoint);
        // Restoration can clear a source fault, but it must not erase the error
        // observed by this peek. Terminal control errors always survive rewind.
        restored?;
        result.map(|token| token.kind())
    }

    pub(crate) fn peek2(&mut self) -> PResult<(TokenKind, TokenKind)> {
        let checkpoint = self.bank.checkpoint()?;
        let result = (|| {
            let first = self.next_non_trivia_token()?;
            let second = self.next_non_trivia_token()?;
            Ok((first.kind(), second.kind()))
        })();
        let restored = self.bank.rewind(checkpoint);
        restored?;
        result
    }

    pub(crate) fn bump(&mut self, kind: TokenKind) -> PResult<()> {
        self.bank.reserve(&mut self.tokens, 1)?;
        self.tokens.push(Token::new(kind, self.current_range(), self.current_flags()));
        loop {
            let token = self.bank.next_token()?;
            if token.kind().is_trivia() {
                self.bank.reserve(&mut self.tokens, 1)?;
                self.tokens.push(token);
            } else {
                self.flags = self.bank.current_flags()?;
                self.current = token;
                return Ok(());
            }
        }
    }

    fn next_non_trivia_token(&mut self) -> PResult<Token> {
        loop {
            let token = self.bank.next_token()?;
            if !token.kind().is_trivia() { return Ok(token); }
        }
    }

    pub(crate) fn checkpoint(&mut self) -> PResult<TokenSourceCheckpoint<'src>> {
        let lexer = self.bank.checkpoint()?;
        let tokens = if self.bank.is_legacy() {
            None
        } else {
            // Recovery re-lex can remove a prefix that predates this checkpoint.
            // Length-only truncation cannot restore it. Snapshot byte-lane history
            // with cumulative precharges; normal text retains upstream behavior.
            Some(self.copy_retained_tokens()?)
        };
        Ok(TokenSourceCheckpoint { lexer, current: self.current, flags: self.flags, tokens_position: self.tokens.len(), tokens })
    }

    pub(crate) fn rewind(&mut self, checkpoint: TokenSourceCheckpoint<'src>) -> PResult<()> {
        self.bank.rewind(checkpoint.lexer)?;
        self.current = checkpoint.current;
        self.flags = checkpoint.flags;
        if let Some(tokens) = checkpoint.tokens {
            self.tokens = tokens;
        } else {
            self.tokens.truncate(checkpoint.tokens_position);
        }
        Ok(())
    }

    pub(crate) fn in_range(&mut self, range: TextRange) -> PResult<&[Token]> {
        self.work(self.tokens.len().saturating_mul(2))?;
        let start = self.tokens.iter().rposition(|tok| tok.start() == range.start());
        let end = self.tokens.iter().rposition(|tok| tok.end() == range.end());
        let (Some(start), Some(end)) = (start, end) else { return Ok(&self.tokens); };
        Ok(&self.tokens[start..=end])
    }

    pub(crate) fn finish(mut self) -> PResult<(Vec<Token>, Vec<LexicalError>, FinishedSource<'src, 'check>)> {
        if self.current_kind() != TokenKind::EndOfFile { return Err(ByteLexFailure::Incomplete.into()); }
        if let Some(last) = self.tokens.pop() {
            if last.kind() != TokenKind::EndOfFile { return Err(ByteLexFailure::InvalidState.into()); }
        }
        let (errors, finished) = self.bank.finish()?;
        Ok((self.tokens, errors, finished))
    }
}

pub(crate) struct TokenSourceCheckpoint<'src> {
    lexer: LexerBankCheckpoint<'src>,
    current: Token,
    flags: TokenFlags,
    tokens_position: usize,
    tokens: Option<Vec<Token>>,
}
