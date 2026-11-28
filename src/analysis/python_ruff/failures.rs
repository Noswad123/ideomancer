//! Finite CPython failure certificates over Ruff tokens, never recovery ASTs.
use super::unicode15;
use ruff_python_ast::Mod;
use ruff_python_parser::{LexicalErrorType, ParseErrorType, Parsed, TokenKind};
use ruff_text_size::Ranged;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[allow(clippy::enum_variant_names)] // Frozen exception-class spellings.
pub(in crate::analysis) enum Exception {
    SyntaxError,
    IndentationError,
    TabError,
    ValueError,
    UnicodeDecodeError,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub(in crate::analysis) struct Failure {
    pub(in crate::analysis) exception: Exception,
    pub(in crate::analysis) line: usize,
}

impl Failure {
    pub(super) fn message(&self) -> String {
        format!(
            "{:?} at line {}; source not analyzed",
            self.exception, self.line
        )
    }
}

pub(super) struct Detected {
    /// None still certifies rejection, but not CPython's first exception/line.
    pub(super) failure: Option<Failure>,
    pub(super) reason: &'static str,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Error {
    Cancelled,
    ResourceLimit,
}

#[derive(Clone, Copy)]
enum Frame {
    String {
        raw: bool,
        invalid: Option<&'static str>,
    },
    Field {
        depth: usize,
        format: bool,
    },
    Delimiter(TokenKind),
}

fn tick(check: &mut dyn FnMut() -> bool) -> Result<(), Error> {
    if check() {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}

fn detected(exception: Exception, line: usize, reason: &'static str) -> Detected {
    Detected {
        failure: Some(Failure { exception, line }),
        reason,
    }
}

/// Only these diagnostics can be certified by a malformed, closed literal token.
/// In particular, this is not a mapping from arbitrary Ruff errors to CPython.
pub(super) fn inspect(
    text: &str,
    parsed: &Parsed<Mod>,
    budget: &mut impl FnMut(usize) -> Result<(), Error>,
    check: &mut dyn FnMut() -> bool,
) -> Result<Option<Detected>, Error> {
    tick(check)?;
    if !parsed.unsupported_syntax_errors().is_empty() {
        return Ok(None);
    }
    for error in parsed.errors() {
        tick(check)?;
        if !matches!(
            error.error,
            ParseErrorType::Lexical(
                LexicalErrorType::UnicodeError
                    | LexicalErrorType::MissingUnicodeLbrace
                    | LexicalErrorType::MissingUnicodeRbrace
                    | LexicalErrorType::InvalidByteLiteral
            )
        ) {
            return Ok(None);
        }
    }
    let tokens = parsed.tokens();
    if tokens.len() > 200_000 || text.len() > 2 * 1024 * 1024 {
        return Err(Error::ResourceLimit);
    }
    budget(256 * std::mem::size_of::<Frame>())?;
    let mut frames = Vec::new();
    frames
        .try_reserve_exact(256)
        .map_err(|_| Error::ResourceLimit)?;
    // Non-overlapping token ranges certify every diagnostic independently, even
    // when CPython's decode order differs from Ruff's diagnostic order.
    let mut malformed = Vec::<(usize, usize)>::new();
    if !parsed.errors().is_empty() {
        budget(tokens.len() * std::mem::size_of::<(usize, usize)>())?;
        malformed
            .try_reserve_exact(tokens.len())
            .map_err(|_| Error::ResourceLimit)?;
    }
    let bytes = text.as_bytes();
    let mut cursor = 0;
    let mut line = 1;
    let mut first = None;
    let mut failure_sites = 0usize;
    let mut literal_only = true;
    let mut actual_type_comment = false;
    for token in tokens {
        tick(check)?;
        let range = token.range();
        let start = range.start().to_usize();
        let end = range.end().to_usize();
        let Some(raw) = text.get(start..end) else {
            return Ok(None);
        };
        if start < cursor {
            return Ok(None);
        }
        while cursor < start {
            if cursor % 256 == 0 {
                tick(check)?;
            }
            match bytes[cursor] {
                b'\r' => line += usize::from(bytes.get(cursor + 1) != Some(&b'\n')),
                b'\n' => line += 1,
                _ => {}
            }
            cursor += 1;
        }
        let mut push = None;
        match token.kind() {
            TokenKind::Comment => {
                let mut tail = &raw.as_bytes()[1..];
                while matches!(tail.first(), Some(b' ' | b'\t')) {
                    tick(check)?;
                    tail = &tail[1..];
                }
                if let Some(mut tail) = tail.strip_prefix(b"type:") {
                    while matches!(tail.first(), Some(b' ' | b'\t')) {
                        tick(check)?;
                        tail = &tail[1..];
                    }
                    let ignore = tail.starts_with(b"ignore")
                        && tail
                            .get(6)
                            .is_none_or(|byte| !byte.is_ascii_alphanumeric() && byte.is_ascii());
                    actual_type_comment |= !ignore;
                }
            }
            TokenKind::Name if !raw.is_ascii() => {
                if !unicode15::valid_identifier(raw, check).map_err(|_| Error::Cancelled)? {
                    failure_sites += 1;
                    literal_only = false;
                    first.get_or_insert_with(|| {
                        detected(
                            Exception::SyntaxError,
                            line,
                            "invalid Unicode 15 identifier",
                        )
                    });
                }
            }
            TokenKind::Int => {
                let b = raw.as_bytes();
                let prefixed = b.len() >= 2
                    && b[0] == b'0'
                    && matches!(b[1], b'x' | b'X' | b'o' | b'O' | b'b' | b'B');
                if !prefixed {
                    let mut digits = 0;
                    let mut nonzero = false;
                    for chunk in b.chunks(256) {
                        tick(check)?;
                        for &ch in chunk {
                            digits += usize::from(ch.is_ascii_digit());
                            nonzero |= matches!(ch, b'1'..=b'9');
                        }
                    }
                    if nonzero && digits > 4300 {
                        failure_sites += 1;
                        literal_only = false;
                        first.get_or_insert_with(|| {
                            detected(Exception::SyntaxError, line, "decimal conversion limit")
                        });
                    }
                }
            }
            TokenKind::FStringStart => {
                push = Some(Frame::String {
                    raw: token.unwrap_string_flags().is_raw_string(),
                    invalid: None,
                });
            }
            TokenKind::FStringEnd => {
                let Some(Frame::String { invalid, .. }) = frames.pop() else {
                    return Ok(None);
                };
                if let Some(reason) = invalid {
                    first.get_or_insert_with(|| detected(Exception::SyntaxError, line, reason));
                }
            }
            TokenKind::String | TokenKind::FStringMiddle => {
                let middle = token.kind() == TokenKind::FStringMiddle;
                let flags = token.unwrap_string_flags();
                // CPython 3.12 decodes format-spec text even in a raw f-string.
                // Raw mode still controls how the lexer splits named braces.
                let format_spec =
                    middle && matches!(frames.last(), Some(Frame::Field { format: true, .. }));
                let raw_string = if middle {
                    let Some(Frame::String { raw, .. }) = frames
                        .iter()
                        .rev()
                        .find(|frame| matches!(frame, Frame::String { .. }))
                    else {
                        return Ok(None);
                    };
                    *raw
                } else {
                    flags.is_raw_string()
                };
                let invalid = if !middle && flags.is_byte_string() && !raw.is_ascii() {
                    Some("non-ASCII byte literal")
                } else if raw_string && !format_spec {
                    None
                } else {
                    invalid_escape(raw, flags.is_byte_string(), check)?
                };
                if let Some(reason) = invalid {
                    failure_sites += 1;
                    if !parsed.errors().is_empty() {
                        malformed.push((start, end));
                    }
                    if middle {
                        if format_spec {
                            first.get_or_insert_with(|| {
                                detected(Exception::UnicodeDecodeError, 0, reason)
                            });
                        } else {
                            let Some(Frame::String { invalid, .. }) = frames.last_mut() else {
                                return Ok(None);
                            };
                            invalid.get_or_insert(reason);
                        }
                    } else {
                        first.get_or_insert_with(|| detected(Exception::SyntaxError, line, reason));
                    }
                }
            }
            TokenKind::Lbrace => {
                let depth = match frames.last() {
                    Some(Frame::String { .. }) => Some(1),
                    Some(Frame::Field {
                        depth,
                        format: true,
                    }) => Some(depth + 1),
                    _ => None,
                };
                push = Some(if let Some(depth) = depth {
                    if depth > 3 {
                        failure_sites += 1;
                        literal_only = false;
                        first.get_or_insert_with(|| {
                            detected(Exception::SyntaxError, line, "f-string format field depth")
                        });
                    }
                    Frame::Field {
                        depth,
                        format: false,
                    }
                } else {
                    Frame::Delimiter(TokenKind::Lbrace)
                });
            }
            TokenKind::Lpar | TokenKind::Lsqb => {
                push = Some(Frame::Delimiter(token.kind()));
            }
            TokenKind::Colon => {
                if let Some(Frame::Field { format, .. }) = frames.last_mut() {
                    *format = true;
                }
            }
            TokenKind::Rbrace | TokenKind::Rpar | TokenKind::Rsqb => {
                if !matches!(
                    (frames.pop(), token.kind()),
                    (Some(Frame::Field { .. }), TokenKind::Rbrace)
                        | (Some(Frame::Delimiter(TokenKind::Lbrace)), TokenKind::Rbrace)
                        | (Some(Frame::Delimiter(TokenKind::Lpar)), TokenKind::Rpar)
                        | (Some(Frame::Delimiter(TokenKind::Lsqb)), TokenKind::Rsqb)
                ) {
                    return Ok(None);
                }
            }
            _ => {}
        }
        if let Some(frame) = push {
            if frames.len() == 256 {
                return Err(Error::ResourceLimit);
            }
            frames.push(frame);
        }
    }
    if !frames.is_empty() {
        return Ok(None);
    }
    for error in parsed.errors() {
        tick(check)?;
        let start = error.location.start().to_usize();
        let end = error.location.end().to_usize();
        let index = malformed.partition_point(|&(opening, _)| opening <= start);
        if index == 0 || end > malformed[index - 1].1 {
            return Ok(None);
        }
    }
    tick(check)?;
    // Literal-only failures follow reductions: ordinary tokens and format text
    // fail immediately; outer f-string text waits for its closing token. Thus
    // inner failures win over pending outer text, while adjacent literals retain
    // reduction order. The first recorded reduction certifies this family only.
    // Identifier/numeric/depth guards and type-comment placement involve other
    // tokenizer/PEG actions; do not infer their precedence from token order.
    if first.is_some() && ((failure_sites > 1 && !literal_only) || actual_type_comment) {
        return Ok(Some(Detected {
            failure: None,
            reason: "competing failure precedence",
        }));
    }
    Ok(first)
}

/// Inspect raw token spelling, not Ruff's decoded value (which loses surrogates
/// and uses a newer UCD). Unknown escapes and surrogate escapes remain admitted.
fn invalid_escape(
    raw: &str,
    bytes_literal: bool,
    check: &mut dyn FnMut() -> bool,
) -> Result<Option<&'static str>, Error> {
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        tick(check)?;
        if bytes[i] != b'\\' {
            i += 1;
            continue;
        }
        let kind = bytes.get(i + 1).copied();
        i += 2;
        let width = match kind {
            Some(b'x') => 2,
            Some(b'u') if !bytes_literal => 4,
            Some(b'U') if !bytes_literal => 8,
            Some(b'N') if !bytes_literal => {
                if bytes.get(i) != Some(&b'{') {
                    return Ok(Some("invalid Unicode 15 named escape"));
                }
                i += 1;
                let start = i;
                while i < bytes.len() && bytes[i] != b'}' {
                    tick(check)?;
                    i += 1;
                }
                if i == bytes.len()
                    || unicode15::lookup_name_15(&raw[start..i], check)
                        .map_err(|_| Error::Cancelled)?
                        .is_none()
                {
                    return Ok(Some("invalid Unicode 15 named escape"));
                }
                i += 1;
                continue;
            }
            _ => continue,
        };
        let mut value = 0u32;
        for _ in 0..width {
            let digit = match bytes.get(i) {
                Some(c @ b'0'..=b'9') => c - b'0',
                Some(c @ b'a'..=b'f') => c - b'a' + 10,
                Some(c @ b'A'..=b'F') => c - b'A' + 10,
                _ => return Ok(Some("invalid hexadecimal escape")),
            };
            value = (value << 4) | u32::from(digit);
            i += 1;
        }
        if value > 0x10ffff {
            return Ok(Some("invalid Unicode escape value"));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruff_python_ast::PythonVersion;
    use ruff_python_parser::{parse_unchecked, Mode, ParseOptions};

    fn parse(text: &str) -> Parsed<Mod> {
        parse_unchecked(
            text,
            ParseOptions::from(Mode::Module)
                .with_target_version(PythonVersion::PY312)
                .with_ast_only(true),
        )
    }

    fn failure(exception: Exception, line: usize) -> Option<Failure> {
        Some(Failure { exception, line })
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Outcome {
        Exact,
        UnresolvedRejection,
        ParserOnly,
    }

    // Oracle expectations are independent of the inspector's classification.
    fn matrix() -> Vec<(String, Option<Failure>, Outcome)> {
        use Exception::{SyntaxError as S, UnicodeDecodeError as U};
        use Outcome::{Exact, ParserOnly, UnresolvedRejection};
        let mut cases = Vec::new();
        for escape in [
            "\\xzz",
            "\\uZZZZ",
            "\\U00110000",
            "\\N{NOT A UNICODE NAME}",
            "\\N{GARAY CAPITAL LETTER A}",
        ] {
            for (text, expected) in [
                (format!("'{escape}'\n"), failure(S, 1)),
                (format!("\n'''\n{escape}\n'''\n"), failure(S, 2)),
                (format!("('first'\n '{escape}')\n"), failure(S, 2)),
                (format!("f'''{escape}\n{{x}}\n'''\n"), failure(S, 3)),
                (format!("f'''{{x:\n{escape}}}\n'''\n"), failure(U, 0)),
                (format!("rf'''{escape}\n{{x}}\n'''\n"), None),
                (format!("rf'{{x:{escape}}}'\n"), failure(U, 0)),
                (format!("r'{escape}'\n"), None),
                (format!("'\\{escape}'\n"), None),
            ] {
                // In raw f-strings, named-escape braces are expression tokens,
                // not part of the literal. Cover that separately below.
                if !(escape.starts_with("\\N") && text.starts_with("rf")) {
                    cases.push((text, expected, Exact));
                }
            }
            cases.push((
                format!("f'''{escape}\n{{'''\\xzz'''}}\n'''\n"),
                failure(S, 2),
                Exact,
            ));
        }
        for (text, expected) in [
            ("b'''\n\\xzz\n'''", failure(S, 1)),
            ("b'\\uZZZZ\\U00110000\\N{NO NAME}'", None),
            ("rb'\\xzz'", None),
            ("b'\u{e9}'", failure(S, 1)),
            ("rb'\u{e9}'", failure(S, 1)),
            ("'\\ud800\\U0010ffff'", None),
            ("'\\N{LATIN CAPITAL LETTER A}\\x41\\u0041'", None),
            ("f'{x:{y:{z}}}'", None),
            ("f'{x:{y:{z:}}}'", None),
            ("f'{x:{y:{z!r}}}'", None),
            ("f'{x:{y:{z:{eval()}}}}'", failure(S, 1)),
            ("f'''{x:\n{y:\n{z:\n{w}}}}'''", failure(S, 4)),
            ("f'{x:{f\"{y:{z:{w}}}\"}}'", None),
            ("f'{(lambda a: a)({1: 2}):{w}}'", None),
            ("f'{x:{{1:2}}}'", None),
            ("f'{x:\\N}'", failure(U, 0)),
            ("f'{x:\\N{}}'", failure(U, 0)),
            ("f'{x:\\N{LATIN CAPITAL LETTER A}}'", None),
            ("rf'{x:\\N{NO_NAME}}'", failure(U, 0)),
            ("rf'\\N{NO_NAME}{x}'", None),
            ("rf'{x:\\\\xzz}'", None),
            ("'\\N'", failure(S, 1)),
            ("'\\N{}'", failure(S, 1)),
            ("'\\N{NO NAME'", failure(S, 1)),
            ("\n\u{1c89} = value", failure(S, 2)),
            ("f'''\n{\u{1c89}}\n'''", failure(S, 2)),
            ("\u{3bb} = 1", None),
        ] {
            cases.push((text.into(), expected, Exact));
        }
        for (text, expected) in [
            (format!("\nx={}", "1".repeat(4301)), failure(S, 2)),
            (format!("x={}", "0".repeat(4301)), None),
            (format!("x={}", "1".repeat(4300)), None),
            (format!("x=0x{}", "f".repeat(4301)), None),
            (format!("x={}j", "1".repeat(4301)), None),
            (format!("x={}.0", "1".repeat(4301)), None),
            (format!("x={}", "1_".repeat(4300) + "1"), failure(S, 1)),
        ] {
            cases.push((text, expected, Exact));
        }
        for (text, expected) in [
            ("f'''{x:\\xzz{'''\n\\uzz\n'''}}'''", failure(U, 0)),
            ("f'''{x:{'''\n\\uzz\n'''}\\xzz}'''", failure(S, 1)),
            ("f'''\\xzz\n{f'''\\uzz\n'''}\n'''", failure(S, 3)),
            ("'\\xzz' '\\uzz'", failure(S, 1)),
            ("f'''\\xzz\n''' '\\uzz'", failure(S, 2)),
            ("('\\xzz' f'{x:\\xzz}')", failure(S, 1)),
            ("(f'{x:\\xzz}' '\\xzz')", failure(U, 0)),
            ("('\\xzz' f'\\xzz')", failure(S, 1)),
            ("(f'\\xzz' '\\xzz')", failure(S, 1)),
            // Preserve the already-proven nested/adjacent Unicode-16-alias
            // regressions from the compatibility batch, without changing it.
            ("eval()\nvalue = f\"\"\"\\N{CUNEIFORM SIGN KALAM}\n{f'''first\n\\N{CUNEIFORM SIGN KALAM}\nlast'''}\nouter\"\"\"\n", failure(S, 5)),
            ("eval()\nvalue = (f'''first\n\\N{CUNEIFORM SIGN KALAM}\nlast'''\n f\"\"\"second\n\\N{CUNEIFORM SIGN KALAM}\nend\"\"\")\n", failure(S, 4)),
            ("eval()\nvalue = f\"\"\"\\N{CUNEIFORM SIGN KALAM}\n{(\n '''first\n\\N{CUNEIFORM SIGN KALAM}\nlast'''\n)}\"\"\"\n", failure(S, 4)),
        ] {
            cases.push((text.into(), expected, Exact));
        }
        let escapes = [
            "\\xzz",
            "\\uZZZZ",
            "\\U00110000",
            "\\N{NO_NAME}",
            "\\N{CUNEIFORM SIGN KALAM}",
        ];
        for a in escapes {
            for b in escapes {
                for (text, expected) in [
                    (format!("'{a}'\n'{b}'"), failure(S, 1)),
                    (format!("('''\n{a}\n'''\n '''\n{b}\n''')"), failure(S, 1)),
                    (format!("'''{a}\n'''\nf'''{b}\n'''"), failure(S, 1)),
                    (format!("f'''{a}\n'''\n'''{b}\n'''"), failure(S, 2)),
                    (format!("(\nf'''{a}\n'''\nf'''{b}\n''')"), failure(S, 3)),
                    (format!("f'''{a}\n{{'''{b}\n'''}}\n'''"), failure(S, 2)),
                    (format!("f'''{a}\n{{f'''{b}\n'''}}\n'''"), failure(S, 3)),
                    (format!("f'''{a}\n{{x:{b}}}\n'''"), failure(U, 0)),
                ] {
                    cases.push((text, expected, Exact));
                }
            }
        }
        for (text, expected) in [
            ("f'''\\xzz\n{x:{y:{z:{w}}}}\n'''", failure(S, 2)),
            ("f'''{x:\\xzz\n{y:{z:{w}}}}'''", failure(U, 0)),
            ("f'''\\xzz\n'''\n\u{1c89}=1", failure(S, 3)),
        ] {
            cases.push((text.into(), expected, UnresolvedRejection));
        }
        let guards = [
            "'\\xzz'".to_owned(),
            "'\\N{NO_NAME}'".to_owned(),
            "1".repeat(4301),
            "\u{1c89}=1".to_owned(),
            "f'{x:{y:{z:{w}}}}'".to_owned(),
            "f'{x:\\xzz}'".to_owned(),
        ];
        for (a, first) in guards.iter().enumerate() {
            for (b, second) in guards.iter().enumerate() {
                if a != b {
                    // Independent CPython 3.12.11 oracle anchors for this finite
                    // matrix only. Production deliberately infers no such order.
                    let expected = match (a, b) {
                        (5, _) => failure(U, 0),
                        (0..=2, 3) => failure(S, 2),
                        _ => failure(S, 1),
                    };
                    let outcome = if matches!(a, 0 | 1 | 5) && matches!(b, 0 | 1 | 5) {
                        Exact
                    } else {
                        UnresolvedRejection
                    };
                    cases.push((format!("{first}\n{second}"), expected, outcome));
                }
            }
            cases.push((
                format!("# type: A\n{first}"),
                failure(S, if a == 3 { 2 } else { 1 }),
                UnresolvedRejection,
            ));
            cases.push((
                format!("{first}\n# type: A"),
                if a == 5 { failure(U, 0) } else { failure(S, 1) },
                UnresolvedRejection,
            ));
        }
        for comment in [
            "# ordinary",
            "# type: ignore",
            "# type: ignore[unit]",
            "# type: ignore_thing",
            "#\x0ctype: A",
            "# Type: A",
        ] {
            cases.push((format!("{comment}\n'\\xzz'"), failure(S, 2), Exact));
            cases.push((format!("'\\xzz'\n{comment}"), failure(S, 1), Exact));
        }
        for text in ["'# type: A'\n'\\xzz'", "f'# type: A'\n'\\xzz'"] {
            cases.push((text.into(), failure(S, 2), Exact));
        }
        for (text, expected) in [
            ("def f(:\n'\\xzz'", failure(S, 1)),
            ("x = )\n'\\xzz'", failure(S, 1)),
            ("type A[T = int] = int\n'\\xzz'", failure(S, 1)),
            ("'\\xzz'\ndef f(:", failure(S, 1)),
            ("f'{x + :\\xzz}'", failure(S, 1)),
        ] {
            cases.push((text.into(), expected, ParserOnly));
        }
        cases
    }

    #[test]
    fn finite_failure_matrix_and_unresolved_diagnostics() {
        for (text, expected, outcome) in matrix() {
            let parsed = parse(&text);
            let actual = inspect(&text, &parsed, &mut |_| Ok(()), &mut || false)
                .unwrap()
                .map(|detected| (detected.failure, detected.reason));
            match outcome {
                Outcome::Exact => assert_eq!(
                    actual.map(|(failure, _)| failure),
                    expected.map(Some),
                    "{text:?}"
                ),
                Outcome::UnresolvedRejection => assert_eq!(
                    actual,
                    Some((None, "competing failure precedence")),
                    "{text:?}"
                ),
                Outcome::ParserOnly => assert!(actual.is_none(), "{text:?}"),
            }
        }
    }

    #[test]
    fn bounded_messages_reasons_cancellation_and_charging() {
        let text = "'\\N{GARAY CAPITAL LETTER A}'";
        let parsed = parse(text);
        let result = inspect(text, &parsed, &mut |_| Ok(()), &mut || false)
            .unwrap()
            .unwrap();
        assert_eq!(result.reason, "invalid Unicode 15 named escape");
        assert_eq!(
            result.failure.unwrap().message(),
            "SyntaxError at line 1; source not analyzed"
        );
        for exception in [
            Exception::SyntaxError,
            Exception::IndentationError,
            Exception::TabError,
            Exception::ValueError,
            Exception::UnicodeDecodeError,
        ] {
            let value = Failure { exception, line: 0 };
            assert!(value.message().ends_with(" at line 0; source not analyzed"));
            assert!(value.message().len() < 80);
            assert_eq!(serde_json::to_value(value).unwrap()["line"], 0);
        }
        assert!(matches!(
            inspect(text, &parsed, &mut |_| Ok(()), &mut || true),
            Err(Error::Cancelled)
        ));
        assert!(matches!(
            inspect(
                text,
                &parsed,
                &mut |_| Err(Error::ResourceLimit),
                &mut || false
            ),
            Err(Error::ResourceLimit)
        ));
        let long = format!("'{}'", "a".repeat(4096));
        let parsed = parse(&long);
        let mut ticks = 0;
        assert!(matches!(
            inspect(&long, &parsed, &mut |_| Ok(()), &mut || {
                ticks += 1;
                ticks > 10
            }),
            Err(Error::Cancelled)
        ));
        let deep = format!("{}1{}", "(".repeat(257), ")".repeat(257));
        let parsed = parse(&deep);
        assert!(matches!(
            inspect(&deep, &parsed, &mut |_| Ok(()), &mut || false),
            Err(Error::ResourceLimit)
        ));
    }

    #[test]
    fn literal_newlines_are_physical_and_adjacent_tokens_keep_opening_lines() {
        for newline in ["\n", "\r", "\r\n"] {
            let text = format!("({newline}'first'{newline}'''{newline}\\xzz{newline}''')");
            let parsed = parse(&text);
            let actual = inspect(&text, &parsed, &mut |_| Ok(()), &mut || false)
                .unwrap()
                .unwrap();
            assert_eq!(actual.failure.unwrap().line, 3);
        }
    }

    #[test]
    fn cancellation_in_long_names_and_diagnostic_validation() {
        let text = format!("'\\N{{{}}}'", "A".repeat(4096));
        let parsed = parse(&text);
        let mut ticks = 0;
        assert!(matches!(
            inspect(&text, &parsed, &mut |_| Ok(()), &mut || {
                ticks += 1;
                ticks == 20
            }),
            Err(Error::Cancelled)
        ));
        assert_eq!(ticks, 20);

        let text = "'\\xzz'\n'\\uzz'";
        let parsed = parse(text);
        assert_eq!(parsed.errors().len(), 2);
        let mut total = 0;
        let result = inspect(text, &parsed, &mut |_| Ok(()), &mut || {
            total += 1;
            false
        })
        .unwrap()
        .unwrap();
        assert_eq!(result.failure, failure(Exception::SyntaxError, 1));
        // The last checkpoint precedes return; the penultimate checkpoint is
        // the second diagnostic's range validation, after all token scanning.
        let mut ticks = 0;
        assert!(matches!(
            inspect(text, &parsed, &mut |_| Ok(()), &mut || {
                ticks += 1;
                ticks == total - 1
            }),
            Err(Error::Cancelled)
        ));
        assert_eq!(ticks, total - 1);
    }

    #[test]
    fn second_reservation_failure_and_type_comment_boundary() {
        let text = "'\\xzz'";
        let parsed = parse(text);
        assert!(!parsed.errors().is_empty());
        let mut reservations = 0;
        assert!(matches!(
            inspect(
                text,
                &parsed,
                &mut |_| {
                    reservations += 1;
                    if reservations == 2 {
                        Err(Error::ResourceLimit)
                    } else {
                        Ok(())
                    }
                },
                &mut || false,
            ),
            Err(Error::ResourceLimit)
        ));
        assert_eq!(reservations, 2);
        for comment in ["# type: A", "# type: ignoreX", "# type: ignore\u{3bb}"] {
            for text in [format!("{comment}\n'\\xzz'"), format!("'\\xzz'\n{comment}")] {
                let parsed = parse(&text);
                let result = inspect(&text, &parsed, &mut |_| Ok(()), &mut || false)
                    .unwrap()
                    .unwrap();
                assert!(result.failure.is_none(), "{text:?}");
                assert_eq!(result.reason, "competing failure precedence");
            }
        }
        let text = "# type: A\nx=1";
        let parsed = parse(text);
        assert!(inspect(text, &parsed, &mut |_| Ok(()), &mut || false)
            .unwrap()
            .is_none());
    }

    #[cfg(unix)]
    #[test]
    #[ignore = "requires explicitly selected trusted exact CPython 3.12.11"]
    fn trusted_failure_projection_oracle() {
        use std::process::{Command, Stdio};
        let cases = matrix();
        let texts: Vec<_> = cases.iter().map(|(text, _, _)| text).collect();
        let script = r#"
import ast, json, sys, unicodedata, warnings
assert sys.version_info[:3] == (3, 12, 11)
assert unicodedata.unidata_version == '15.0.0'
sys.set_int_max_str_digits(4300)
warnings.simplefilter('ignore')
out = []
for text in json.load(sys.stdin):
    try:
        ast.parse(text, type_comments=True)
    except (SyntaxError, ValueError, UnicodeDecodeError) as error:
        line = getattr(error, 'lineno', 0) or 0
        out.append(f'{type(error).__name__} at line {line}; source not analyzed')
    else:
        out.append(None)
json.dump(out, sys.stdout)
"#;
        let mut command = Command::new(crate::analysis::python_parity::oracle());
        command
            .args(["-I", "-S", "-B", "-c", script])
            .env_clear()
            .current_dir("/")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let output = crate::analysis::run_bounded(
            command,
            serde_json::to_vec(&texts).unwrap(),
            "Python Ruff failure oracle",
        )
        .unwrap();
        let observed: Vec<serde_json::Value> = serde_json::from_slice(&output).unwrap();
        assert_eq!(observed.len(), cases.len());
        let mut exact_count = 0;
        let mut unresolved = 0;
        let mut parser_only = 0;
        for ((text, expected, outcome), observed) in cases.into_iter().zip(observed) {
            assert_eq!(
                observed,
                serde_json::to_value(expected.map(|failure| failure.message())).unwrap(),
                "{text:?}"
            );
            let parsed = parse(&text);
            let actual = inspect(&text, &parsed, &mut |_| Ok(()), &mut || false)
                .unwrap()
                .map(|detected| detected.failure);
            match outcome {
                Outcome::Exact => {
                    exact_count += 1;
                    assert_eq!(actual, expected.map(Some), "{text:?}");
                }
                Outcome::UnresolvedRejection => {
                    unresolved += 1;
                    assert_eq!(actual, Some(None), "{text:?}");
                    assert!(expected.is_some());
                }
                Outcome::ParserOnly => {
                    parser_only += 1;
                    assert!(actual.is_none(), "unresolved diagnostics: {text:?}");
                    assert!(expected.is_some());
                }
            }
        }
        eprintln!("failure matrix: {exact_count} exact certificates/admissions; {unresolved} unresolved rejections; {parser_only} parser-only projections");
    }
}
