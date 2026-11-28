use ruff_text_size::{TextRange, TextSize};

use super::byte::{ByteLexFailure, Control, SourceFault};

#[derive(Clone, Copy, Debug)]
pub(super) enum Previous {
    Start,
    Scalar(char),
    Opaque,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct CursorState {
    offset: usize,
    token_start: usize,
    previous: Previous,
}

/// Byte position and bounded scalar decoding shared by both lexer facades.
pub(super) struct Cursor<'src, 'check, Check: ?Sized = dyn FnMut() -> bool + 'check> {
    source: &'src [u8],
    state: CursorState,
    pub(super) control: Control<'check, Check>,
}

impl<Check: ?Sized> std::fmt::Debug for Cursor<'_, '_, Check> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cursor").field("state", &self.state).finish_non_exhaustive()
    }
}

impl<'src, 'check, Check: FnMut() -> bool + ?Sized> Cursor<'src, 'check, Check> {
    pub(super) fn new(source: &'src [u8], control: Control<'check, Check>) -> Self {
        Self {
            source,
            state: CursorState { offset: 0, token_start: 0, previous: Previous::Start },
            control,
        }
    }

    pub(super) fn checkpoint(&self) -> CursorState { self.state }

    pub(super) fn rewind(&mut self, state: CursorState) { self.state = state; }

    /// Reposition without decoding any prefix (which may contain opaque bytes).
    pub(super) fn reposition(&mut self, offset: usize) -> Result<(), ByteLexFailure> {
        if offset > self.source.len() || self.source.get(offset).is_some_and(|b| b & 0xc0 == 0x80) {
            return Err(ByteLexFailure::InvalidState);
        }
        self.state = CursorState { offset, token_start: offset, previous: Previous::Start };
        Ok(())
    }

    #[cfg(debug_assertions)]
    pub(super) fn previous(&self) -> Option<char> {
        match self.state.previous { Previous::Scalar(c) => Some(c), _ => None }
    }

    fn decode(&self, offset: usize) -> Result<Option<char>, ByteLexFailure> {
        let Some(&first) = self.source.get(offset) else { return Ok(None) };
        let width = match first {
            0..=0x7f => return Ok(Some(char::from(first))),
            0xc2..=0xdf => 2,
            0xe0..=0xef => 3,
            0xf0..=0xf4 => 4,
            _ => 1,
        };
        let end = (offset + width).min(self.source.len());
        match std::str::from_utf8(&self.source[offset..end]) {
            Ok(text) => Ok(text.chars().next()),
            Err(error) => Err(ByteLexFailure::Source(SourceFault {
                range: TextRange::at(TextSize::new(offset as u32), TextSize::new(error.error_len().unwrap_or(end - offset) as u32)),
                error_len: error.error_len(),
            })),
        }
    }

    /// Pure lookahead: a fault is returned, never committed to cursor state.
    pub(super) fn first(&self) -> Result<Option<char>, ByteLexFailure> { self.decode(self.state.offset) }

    pub(super) fn second(&self) -> Result<Option<char>, ByteLexFailure> {
        match self.first()? {
            Some(c) => self.decode(self.state.offset + c.len_utf8()),
            None => Ok(None),
        }
    }

    pub(super) fn rest(&self) -> &'src [u8] { &self.source[self.state.offset..] }

    pub(super) fn text_len(&self) -> TextSize { TextSize::new(self.rest().len() as u32) }

    pub(super) fn token_len(&self) -> TextSize {
        TextSize::new((self.state.offset - self.state.token_start) as u32)
    }

    pub(super) fn start_token(&mut self) { self.state.token_start = self.state.offset; }

    pub(super) fn is_eof(&self) -> bool { self.state.offset == self.source.len() }

    pub(super) fn bump(&mut self) -> Result<Option<char>, ByteLexFailure> {
        self.control.work(1)?;
        let c = self.first()?;
        if let Some(c) = c {
            self.state.offset += c.len_utf8();
            self.state.previous = Previous::Scalar(c);
        }
        Ok(c)
    }

    pub(super) fn eat_char(&mut self, c: char) -> Result<bool, ByteLexFailure> {
        // ASCII punctuation probes must not decode an unrelated following byte.
        if c.is_ascii() {
            if self.rest().first().copied() != Some(c as u8) { return Ok(false) }
        } else if self.first()? != Some(c) { return Ok(false) }
        self.bump()?;
        Ok(true)
    }

    pub(super) fn eat_char2(&mut self, c1: char, c2: char) -> Result<bool, ByteLexFailure> {
        debug_assert!(c1.is_ascii() && c2.is_ascii());
        if self.rest().starts_with(&[c1 as u8, c2 as u8]) {
            self.bump()?;
            self.bump()?;
            Ok(true)
        } else { Ok(false) }
    }

    pub(super) fn eat_char3(&mut self, c1: char, c2: char, c3: char) -> Result<bool, ByteLexFailure> {
        debug_assert!(c1.is_ascii() && c2.is_ascii() && c3.is_ascii());
        if self.rest().starts_with(&[c1 as u8, c2 as u8, c3 as u8]) {
            self.bump()?;
            self.bump()?;
            self.bump()?;
            Ok(true)
        } else { Ok(false) }
    }

    pub(super) fn eat_if(&mut self, mut predicate: impl FnMut(char) -> bool) -> Result<Option<char>, ByteLexFailure> {
        if self.first()?.is_some_and(&mut predicate) { self.bump() } else { Ok(None) }
    }

    pub(super) fn eat_while(&mut self, mut predicate: impl FnMut(char) -> bool) -> Result<(), ByteLexFailure> {
        while self.first()?.is_some_and(&mut predicate) { self.bump()?; }
        Ok(())
    }

    /// Validate every consumed byte, including fast and unterminated string paths.
    pub(super) fn skip_bytes(&mut self, count: usize) -> Result<(), ByteLexFailure> {
        let end = self.state.offset.checked_add(count).filter(|&end| end <= self.source.len()).ok_or(ByteLexFailure::InvalidState)?;
        while self.state.offset < end { self.bump()?; }
        if self.state.offset != end { return Err(ByteLexFailure::InvalidState) }
        Ok(())
    }

    pub(super) fn skip_to_end(&mut self) -> Result<(), ByteLexFailure> { self.skip_bytes(self.rest().len()) }

    /// Only the existing comment transition may consume opaque bytes.
    pub(super) fn skip_comment(&mut self) -> Result<(), ByteLexFailure> {
        while let Some(&b) = self.rest().first() {
            if matches!(b, b'\n' | b'\r') { break }
            self.control.work(1)?;
            self.state.offset += 1;
            self.state.previous = Previous::Opaque;
        }
        Ok(())
    }
}
