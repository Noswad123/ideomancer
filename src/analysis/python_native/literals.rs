//! Borrowed Python 3.12 literal spelling checks, without value conversion.
use super::*;

// Isolated Python 3.12 default, not a token-size/resource policy. CPython's
// Objects/longobject.c long_from_string_base excludes underscores. All-zero
// spellings bypass conversion limits in Parser/pegen.c parsenumber_raw.
const MAX_DECIMAL_DIGITS: usize = 4300;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NumberKind {
    Integer,
    Float,
    Imaginary,
}

pub(super) fn number<F: FnMut(Checkpoint) -> bool>(
    text: &[u8],
    cancel: &Cancellation<F>,
) -> Option<NumberKind> {
    // All digit runs, including exponent and base-prefixed runs, use this
    // cancellable scan. Only spelling metadata is retained.
    fn digits<F: FnMut(Checkpoint) -> bool>(
        text: &[u8],
        at: &mut usize,
        radix: u8,
        cancel: &Cancellation<F>,
    ) -> Option<(usize, bool)> {
        let mut count = 0;
        let mut nonzero = false;
        let mut previous = false;
        while let Some(&b) = text.get(*at) {
            if *at % 256 == 0 && cancel.at(Checkpoint::Collector) {
                return None;
            }
            let valid = match radix {
                2 => matches!(b, b'0'..=b'1'),
                8 => matches!(b, b'0'..=b'7'),
                16 => b.is_ascii_hexdigit(),
                _ => b.is_ascii_digit(),
            };
            if valid {
                count += 1;
                nonzero |= b != b'0';
                previous = true;
            } else if b == b'_' && previous {
                previous = false;
            } else {
                break;
            }
            *at += 1;
        }
        if count != 0 && !previous {
            None
        } else {
            Some((count, nonzero))
        }
    }

    if cancel.at(Checkpoint::Collector) || text.is_empty() {
        return None;
    }
    if text[0] == b'0' && text.len() >= 2 {
        let radix = match text[1] {
            b'b' | b'B' => 2,
            b'o' | b'O' => 8,
            b'x' | b'X' => 16,
            _ => 0,
        };
        if radix != 0 {
            let mut at = 2 + usize::from(text.get(2) == Some(&b'_'));
            let (count, _) = digits(text, &mut at, radix, cancel)?;
            return (count != 0 && at == text.len()).then_some(NumberKind::Integer);
        }
    }
    let mut at = 0;
    let (integer_digits, nonzero) = digits(text, &mut at, 10, cancel)?;
    let dot = text.get(at) == Some(&b'.');
    let mut fraction_digits = 0;
    if dot {
        at += 1;
        (fraction_digits, _) = digits(text, &mut at, 10, cancel)?;
    }
    if integer_digits + fraction_digits == 0 {
        return None;
    }
    let exponent = matches!(text.get(at), Some(b'e' | b'E'));
    if exponent {
        at += 1;
        if matches!(text.get(at), Some(b'+' | b'-')) {
            at += 1;
        }
        let (count, _) = digits(text, &mut at, 10, cancel)?;
        if count == 0 {
            return None;
        }
    }
    let imaginary = matches!(text.get(at), Some(b'j' | b'J'));
    at += usize::from(imaginary);
    if at != text.len() {
        return None;
    }
    if imaginary {
        Some(NumberKind::Imaginary)
    } else if dot || exponent {
        Some(NumberKind::Float)
    } else if !nonzero || (integer_digits <= MAX_DECIMAL_DIGITS && text[0] != b'0') {
        Some(NumberKind::Integer)
    } else {
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Flavor {
    Text,
    Bytes,
    Formatted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Start {
    pub flavor: Flavor,
    pub raw: bool,
    pub quote: u8,
    pub triple: bool,
    pub len: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Error {
    Invalid,
    UnsupportedNamedEscape,
    Cancelled,
}

pub(super) fn start(token: &[u8]) -> Result<Start, Error> {
    if token.is_empty() || token.len() > 5 {
        return Err(Error::Invalid);
    }
    let at = token
        .iter()
        .position(|b| matches!(b, b'\'' | b'"'))
        .ok_or(Error::Invalid)?;
    let mut prefix = [0; 2];
    if at > prefix.len() {
        return Err(Error::Invalid);
    }
    for (out, b) in prefix.iter_mut().zip(&token[..at]) {
        *out = b.to_ascii_lowercase();
    }
    let (flavor, raw) = match &prefix[..at] {
        b"" | b"u" => (Flavor::Text, false),
        b"r" => (Flavor::Text, true),
        b"b" => (Flavor::Bytes, false),
        b"br" | b"rb" => (Flavor::Bytes, true),
        b"f" => (Flavor::Formatted, false),
        b"fr" | b"rf" => (Flavor::Formatted, true),
        _ => return Err(Error::Invalid),
    };
    let quote = token[at];
    let triple = token.len() - at == 3;
    if (!triple && token.len() - at != 1) || token[at..].iter().any(|&b| b != quote) {
        return Err(Error::Invalid);
    }
    Ok(Start {
        flavor,
        raw,
        quote,
        triple,
        len: token.len(),
    })
}

pub(super) fn string<F: FnMut(Checkpoint) -> bool>(
    text: &[u8],
    cancel: &Cancellation<F>,
) -> Result<Flavor, Error> {
    if cancel.at(Checkpoint::Collector) {
        return Err(Error::Cancelled);
    }
    let quote_at = text
        .iter()
        .take(3)
        .position(|b| matches!(b, b'\'' | b'"'))
        .ok_or(Error::Invalid)?;
    let quote = text[quote_at];
    let width = if text.get(quote_at..quote_at + 3) == Some(&[quote, quote, quote]) {
        3
    } else {
        1
    };
    let info = start(&text[..quote_at + width])?;
    if info.flavor == Flavor::Formatted {
        return Err(Error::Invalid);
    }
    let mut at = info.len;
    while at < text.len() {
        if cancel.at(Checkpoint::Collector) {
            return Err(Error::Cancelled);
        }
        if text[at] == b'\\' {
            // Raw strings also escape delimiters lexically; the backslash is
            // simply preserved in their value. Never mistake an escaped end.
            at += 2;
        } else if text.get(at..at + width) == Some(&[quote, quote, quote][..width]) {
            if at + width != text.len() {
                return Err(Error::Invalid);
            }
            segment(&text[info.len..at], info, false, cancel)?;
            return Ok(info.flavor);
        } else {
            at += 1;
        }
    }
    Err(Error::Invalid)
}

/// Validate only literal content. Interpolation grammar and delimiter matching
/// belong to the counted CST consumer, not to this segment checker.
pub(super) fn segment<F: FnMut(Checkpoint) -> bool>(
    text: &[u8],
    info: Start,
    in_format_spec: bool,
    cancel: &Cancellation<F>,
) -> Result<(), Error> {
    let mut at = 0;
    let mut named = false;
    if cancel.at(Checkpoint::Collector) {
        return Err(Error::Cancelled);
    }
    while at < text.len() {
        if cancel.at(Checkpoint::Collector) {
            return Err(Error::Cancelled);
        }
        let b = text[at];
        if b == 0 || (info.flavor == Flavor::Bytes && !b.is_ascii()) {
            return Err(Error::Invalid);
        }
        if matches!(b, b'\n' | b'\r') && !info.triple && !in_format_spec {
            return Err(Error::Invalid);
        }
        at += 1;
        if b != b'\\' {
            continue;
        }
        let Some(&escape) = text.get(at) else {
            // A formatted literal segment may end immediately before `{`.
            if info.flavor == Flavor::Formatted {
                break;
            }
            return Err(Error::Invalid);
        };
        if escape == 0 || (info.flavor == Flavor::Bytes && !escape.is_ascii()) {
            return Err(Error::Invalid);
        }
        at += 1;
        if escape == b'\r' && text.get(at) == Some(&b'\n') {
            at += 1;
        }
        if info.raw {
            continue;
        }
        let width = match escape {
            b'x' => 2,
            b'u' if info.flavor != Flavor::Bytes => 4,
            b'U' if info.flavor != Flavor::Bytes => 8,
            _ => 0,
        };
        if width != 0 {
            let mut value = 0u32;
            for _ in 0..width {
                let b = *text.get(at).ok_or(Error::Invalid)?;
                let digit = match b {
                    b'0'..=b'9' => b - b'0',
                    b'a'..=b'f' => b - b'a' + 10,
                    b'A'..=b'F' => b - b'A' + 10,
                    _ => return Err(Error::Invalid),
                };
                value = (value << 4) | u32::from(digit);
                at += 1;
            }
            if escape == b'U' && value > 0x10ffff {
                return Err(Error::Invalid);
            }
        } else if escape == b'N' && info.flavor != Flavor::Bytes {
            if text.get(at) != Some(&b'{') {
                return Err(Error::Invalid);
            }
            at += 1;
            let name_start = at;
            while text.get(at).is_some_and(|&b| b != b'}') {
                if cancel.at(Checkpoint::Collector) {
                    return Err(Error::Cancelled);
                }
                if matches!(text[at], 0 | b'\n' | b'\r' | b'{') {
                    return Err(Error::Invalid);
                }
                at += 1;
            }
            if at == name_start || text.get(at) != Some(&b'}') {
                return Err(Error::Invalid);
            }
            at += 1;
            named = true;
        } else if matches!(escape, b'0'..=b'7') {
            // Octal overflow and unknown escapes warn, but remain AST-valid.
            for _ in 0..2 {
                if matches!(text.get(at), Some(b'0'..=b'7')) {
                    at += 1;
                } else {
                    break;
                }
            }
        }
    }
    if named {
        Err(Error::UnsupportedNamedEscape)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cancellation() -> Cancellation<impl FnMut(Checkpoint) -> bool> {
        Cancellation {
            expired: Cell::new(false),
            check: RefCell::new(|_| false),
        }
    }

    #[test]
    fn number_categories_and_invalid_spellings() {
        let cancel = cancellation();
        for text in ["0", "00", "0_0", "123_456", "0x_Ff", "0B_10", "0o_77"] {
            assert_eq!(
                number(text.as_bytes(), &cancel),
                Some(NumberKind::Integer),
                "{text}"
            );
        }
        for text in [".1", "1.", "01.0", "1_0.2_3e+4_5", "1e99999", "1e-99999"] {
            assert_eq!(
                number(text.as_bytes(), &cancel),
                Some(NumberKind::Float),
                "{text}"
            );
        }
        for text in ["0j", "01J", ".5j", "1.e2j", "1e99999J"] {
            assert_eq!(
                number(text.as_bytes(), &cancel),
                Some(NumberKind::Imaginary),
                "{text}"
            );
        }
        for text in [
            "", ".", "_1", "1_", "1__0", "0x", "0x_", "0x__f", "0x1j", "0b2", "0o8", "01", "0_1",
            "1L", "1e", "1e_2", "1e+", "1._0", "1_.0", "1jJ", "-1",
        ] {
            assert_eq!(number(text.as_bytes(), &cancel), None, "{text}");
        }
    }

    #[test]
    fn decimal_limit_counts_digits_not_underscores_or_numeric_value() {
        let cancel = cancellation();
        for digit in ["0", "7"] {
            let boundary = digit.repeat(MAX_DECIMAL_DIGITS);
            assert_eq!(
                number(boundary.as_bytes(), &cancel),
                Some(NumberKind::Integer)
            );
            let beyond = if digit == "0" {
                Some(NumberKind::Integer)
            } else {
                None
            };
            assert_eq!(
                number(format!("{boundary}{digit}").as_bytes(), &cancel),
                beyond
            );
            let separated = format!(
                "{digit}{}",
                format!("_{digit}").repeat(MAX_DECIMAL_DIGITS - 1)
            );
            assert_eq!(
                number(separated.as_bytes(), &cancel),
                Some(NumberKind::Integer)
            );
            assert_eq!(
                number(format!("{separated}_{digit}").as_bytes(), &cancel),
                beyond
            );
        }
        let long = "7".repeat(MAX_DECIMAL_DIGITS + 1);
        for text in [
            format!("{long}.0"),
            format!("{long}e0"),
            format!("1e{long}"),
        ] {
            assert_eq!(number(text.as_bytes(), &cancel), Some(NumberKind::Float));
        }
        for text in [
            format!("{long}j"),
            format!("{long}.0J"),
            format!("1e{long}j"),
        ] {
            assert_eq!(
                number(text.as_bytes(), &cancel),
                Some(NumberKind::Imaginary)
            );
        }
        for (prefix, digit) in [("0b", "1"), ("0o", "7"), ("0x", "f")] {
            assert_eq!(
                number(
                    format!("{prefix}{}", digit.repeat(5000)).as_bytes(),
                    &cancel
                ),
                Some(NumberKind::Integer)
            );
        }
    }

    #[test]
    fn complete_starts_validate_prefixes_and_delimiters() {
        for prefix in ["", "r", "u", "b", "br", "rb", "f", "fr", "rf", "BR", "rF"] {
            for quote in ["'", "\"", "'''", "\"\"\""] {
                let token = format!("{prefix}{quote}");
                let info = start(token.as_bytes()).unwrap();
                assert_eq!(info.len, token.len());
                assert_eq!(info.triple, quote.len() == 3);
                assert_eq!(info.raw, prefix.to_ascii_lowercase().contains('r'));
            }
        }
        for token in [
            "", "t'", "`", "rr'", "ur'", "bf'", "fu'", "ff'", "r''", "'\"'", "fr''''", "'''body",
        ] {
            assert_eq!(start(token.as_bytes()), Err(Error::Invalid), "{token}");
        }
    }

    #[test]
    fn whole_strings_validate_escapes_raw_parity_and_delimiters() {
        let cancel = cancellation();
        for text in [
            r"'escaped\n\t\q\400\777'",
            r"'\x41\uD800\U0000DFFF\U0010ffff'",
            r"r'\xNO\uNO\N{literal}'",
            r"'quote\'inside'",
            r"r'quote\'inside'",
            r"r'ends\\'",
            "'''line\nline\r\n'\"'''",
            "'line\\\njoin'",
            "'line\\\r\njoin'",
        ] {
            assert_eq!(string(text.as_bytes(), &cancel), Ok(Flavor::Text), "{text}");
        }
        for text in [
            r"'\x0'",
            r"'\xGG'",
            r"'\u000'",
            r"'\U0000000'",
            r"'\U00110000'",
            r"'escaped\'",
            r"r'escaped\'",
            "'line\nline'",
            "'line\r\nline'",
            "'''unfinished''",
            "'a'junk",
            "'a' 'b'",
            "'nul\0'",
            "f''",
        ] {
            assert_eq!(
                string(text.as_bytes(), &cancel),
                Err(Error::Invalid),
                "{text:?}"
            );
        }
    }

    #[test]
    fn bytes_require_ascii_but_do_not_decode_unicode_escapes() {
        let cancel = cancellation();
        for text in [
            r"b'\uBAD\Ubad\N{missing\q\777\xff'",
            r"br'\xNO\N'",
            "b'''line\nline'''",
            r"RB'quote\'inside'",
        ] {
            assert_eq!(
                string(text.as_bytes(), &cancel),
                Ok(Flavor::Bytes),
                "{text}"
            );
        }
        for text in [
            "b'\u{e9}'",
            "rb'\u{e9}'",
            "b'\\\u{e9}'",
            r"b'\x0'",
            r"b'\xzz'",
        ] {
            assert_eq!(
                string(text.as_bytes(), &cancel),
                Err(Error::Invalid),
                "{text}"
            );
        }
    }

    #[test]
    fn named_escapes_are_explicitly_unsupported_not_assumed_valid() {
        let cancel = cancellation();
        for text in [r"'\N{LATIN CAPITAL LETTER A}'", r"'\N{NOT A KNOWN NAME}'"] {
            assert_eq!(
                string(text.as_bytes(), &cancel),
                Err(Error::UnsupportedNamedEscape)
            );
        }
        for text in [
            r"'\N'",
            r"'\Nname'",
            r"'\N{}'",
            r"'\N{unfinished'",
            r"'\N{a{b}'",
            r"'\N{name}\xZ'",
        ] {
            assert_eq!(
                string(text.as_bytes(), &cancel),
                Err(Error::Invalid),
                "{text}"
            );
        }
    }

    #[test]
    fn formatted_segments_only_validate_literal_content() {
        let cancel = cancellation();
        let info = start(b"f'").unwrap();
        for text in [b"{{literal}}".as_slice(), b"quote'", b"\\", b"\\uD800"] {
            assert_eq!(segment(text, info, false, &cancel), Ok(()));
        }
        assert_eq!(
            segment(b"line\nline", info, false, &cancel),
            Err(Error::Invalid)
        );
        assert_eq!(segment(b"line\nline", info, true, &cancel), Ok(()));
        assert_eq!(segment(b"\\x0", info, true, &cancel), Err(Error::Invalid));
        assert_eq!(
            segment(b"\\N{name}", info, true, &cancel),
            Err(Error::UnsupportedNamedEscape)
        );
        assert_eq!(
            segment(b"\\N{literal}", start(b"rf'").unwrap(), false, &cancel),
            Ok(())
        );
    }

    #[test]
    fn long_borrowed_scans_honor_cancellation_in_every_lane() {
        for text in [
            "7".repeat(5000),
            format!("0x{}", "f".repeat(5000)),
            format!("1.{}", "7".repeat(5000)),
            format!("1e{}", "7".repeat(5000)),
        ] {
            let mut checks = 0;
            let cancel = Cancellation {
                expired: Cell::new(false),
                check: RefCell::new(|_| {
                    checks += 1;
                    checks == 4
                }),
            };
            assert_eq!(number(text.as_bytes(), &cancel), None);
            assert!(cancel.expired.get());
        }
        let body = [b'a'; 4096];
        let mut checks = 0;
        let cancel = Cancellation {
            expired: Cell::new(false),
            check: RefCell::new(|_| {
                checks += 1;
                checks == 4
            }),
        };
        assert_eq!(
            segment(&body, start(b"'").unwrap(), false, &cancel),
            Err(Error::Cancelled)
        );
        assert_eq!(string(b"''", &cancel), Err(Error::Cancelled));
    }
}
