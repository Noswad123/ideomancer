//! Private, source-only text primitives. No AST decoded values or owned raw text.
// The fact producer must wire these only to approved literal contexts.
#![allow(dead_code)]

use crate::analysis::python_scalars::{self, CodePoint, Retention, RetentionError, TextLimit};
use std::iter::Peekable;
use std::str::Chars;

/// Content only, excluding prefixes and quotes. The caller admits only ordinary,
/// u, and raw text tokens (never bytes/f-strings), slices content_range from the
/// same BOM-stripped source as the AST, and retains only approved contexts.
#[derive(Clone, Copy)]
pub(super) struct Part<'a> {
    pub(super) content: &'a str,
    pub(super) raw: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Error {
    InvalidSyntax,
    OutOfRange,
    UnsupportedNamedEscape,
    Cancelled,
}

pub(super) struct Decoder<'a, I, C> {
    parts: I,
    current: Option<(Peekable<Chars<'a>>, bool)>,
    pending: Option<CodePoint>,
    cancelled: C,
    stopped: bool,
}

pub(super) fn decode<'a, I, C>(parts: I, cancelled: C) -> Decoder<'a, I::IntoIter, C>
where
    I: IntoIterator<Item = Part<'a>>,
    C: FnMut() -> bool,
{
    Decoder {
        parts: parts.into_iter(),
        current: None,
        pending: None,
        cancelled,
        stopped: false,
    }
}

fn normalized(chars: &mut Peekable<Chars<'_>>, ch: char) -> char {
    if ch == '\r' {
        if chars.peek() == Some(&'\n') {
            chars.next();
        }
        '\n'
    } else {
        ch
    }
}

fn escaped(chars: &mut Peekable<Chars<'_>>, ch: char) -> Result<CodePoint, Error> {
    let digits = match ch {
        'x' => 2,
        'u' => 4,
        'U' => 8,
        _ => 0,
    };
    let value = if digits != 0 {
        let mut value = 0u32;
        for _ in 0..digits {
            let digit = chars
                .next()
                .and_then(|ch| ch.to_digit(16))
                .ok_or(Error::InvalidSyntax)?;
            value = (value << 4) | digit;
        }
        value
    } else if matches!(ch, '0'..='7') {
        let mut value = ch as u32 - '0' as u32;
        for _ in 0..2 {
            let Some(digit) = chars.peek().and_then(|ch| ch.to_digit(8)) else {
                break;
            };
            chars.next();
            value = value * 8 + digit;
        }
        value
    } else {
        match ch {
            'a' => 7,
            'b' => 8,
            'f' => 12,
            'n' => 10,
            'r' => 13,
            't' => 9,
            'v' => 11,
            '\\' | '\'' | '"' => ch as u32,
            'N' => return Err(Error::UnsupportedNamedEscape),
            _ => unreachable!("unknown escapes are emitted as two events"),
        }
    };
    CodePoint::new(value).map_err(|_| Error::OutOfRange)
}

impl<'a, I, C> Iterator for Decoder<'a, I, C>
where
    I: Iterator<Item = Part<'a>>,
    C: FnMut() -> bool,
{
    type Item = Result<CodePoint, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.stopped {
                return None;
            }
            if (self.cancelled)() {
                self.stopped = true;
                return Some(Err(Error::Cancelled));
            }
            if let Some(point) = self.pending.take() {
                return Some(Ok(point));
            }
            let Some((chars, raw)) = &mut self.current else {
                let Some(part) = self.parts.next() else {
                    self.stopped = true;
                    return None;
                };
                self.current = Some((part.content.chars().peekable(), part.raw));
                continue;
            };
            let Some(ch) = chars.next() else {
                self.current = None;
                continue;
            };
            if *raw || ch != '\\' {
                return Some(Ok(CodePoint::from(normalized(chars, ch))));
            }
            let result = match chars.next() {
                None => Err(Error::InvalidSyntax),
                Some(ch @ ('\n' | '\r')) => {
                    normalized(chars, ch);
                    continue;
                }
                Some(ch)
                    if !matches!(
                        ch,
                        'a' | 'b' | 'f' | 'n' | 'r' | 't' | 'v' | '\\' | '\'' | '"' | '0'
                            ..='7' | 'x' | 'u' | 'U' | 'N'
                    ) =>
                {
                    self.pending = Some(CodePoint::from(ch));
                    Ok(CodePoint::from('\\'))
                }
                Some(ch) => escaped(chars, ch),
            };
            if result.is_err() {
                self.stopped = true;
            }
            return Some(result);
        }
    }
}

/// Precharge BEFORE calling retain_text: the fixed cap and a possible shrink
/// copy may coexist. On success release this minus retained_bytes; on failure
/// release all only after the decoder/storage drops (or roll back the file).
pub(super) fn retention_work(limit: TextLimit) -> usize {
    2 * limit.points() * std::mem::size_of::<CodePoint>()
}

pub(super) fn retained_bytes(text: &Retention) -> usize {
    match text {
        Retention::Retained(points) => std::mem::size_of_val(&**points),
        Retention::Oversize => 0,
    }
}

pub(super) fn retain_text<'a>(
    parts: impl IntoIterator<Item = Part<'a>>,
    limit: TextLimit,
    mut cancelled: impl FnMut() -> bool,
) -> Result<Retention, Error> {
    if cancelled() {
        return Err(Error::Cancelled);
    }
    // Cancellation belongs inside the decoder, including loops producing no
    // events (empty adjacent parts and removed physical line continuations).
    python_scalars::retain(decode(parts, cancelled), limit, || false).map_err(|error| match error {
        RetentionError::Invalid(error) => error,
        RetentionError::Cancelled => Error::Cancelled,
    })
}

pub(super) const METHODS: [&str; 8] = [
    "DELETE", "GET", "HEAD", "OPTIONS", "PATCH", "POST", "PUT", "TRACE",
];

// Recorded trusted CPython 3.12.11 / Unicode 15.0.0 exhaustive oracle profile:
// for every code point 0..0x110000 (including surrogates), retain non-ASCII
// points whose upper() is ASCII and a substring of any METHODS word. There are
// no empty uppercase mappings. Since upper() concatenates per-point mappings,
// these four preimages plus ASCII are sufficient for exact whitelist membership.
// Tiny property facts, not copied upstream implementation; Unicode terms are
// already reproduced in docs/third-party-notices.md (Unicode-DFS-2016).
pub(super) const METHOD_PREIMAGES: [(u32, &str); 4] =
    [(305, "I"), (383, "S"), (64261, "ST"), (64262, "ST")];

/// Fixed storage only. Even an impossible candidate consumes all remaining
/// events, so malformed syntax or cancellation can never become a silent None.
pub(super) fn method(
    events: impl IntoIterator<Item = Result<CodePoint, Error>>,
) -> Result<Option<&'static str>, Error> {
    let mut bytes = [0u8; 7];
    let mut len = 0;
    let mut invalid = false;
    for point in events {
        let point = point?.value();
        let ascii;
        let mapped = if point <= 0x7f {
            ascii = [(point as u8).to_ascii_uppercase()];
            &ascii[..]
        } else if let Some((_, text)) = METHOD_PREIMAGES.iter().find(|(value, _)| *value == point) {
            text.as_bytes()
        } else {
            invalid = true;
            continue;
        };
        for &byte in mapped {
            if len == bytes.len() {
                invalid = true;
            } else {
                bytes[len] = byte;
                len += 1;
            }
        }
    }
    if invalid {
        Ok(None)
    } else {
        Ok(METHODS
            .into_iter()
            .find(|word| word.as_bytes() == &bytes[..len]))
    }
}

/// Exact nonempty /segment grammar, one through eight segments. Empty and root
/// prefixes are caller policy, not accepted here. Length is Python code points.
pub(super) fn valid_prefix(points: &[CodePoint]) -> bool {
    if points.is_empty() || points.len() > 128 {
        return false;
    }
    let mut segments = 0;
    let mut content = false;
    for (i, point) in points.iter().enumerate() {
        match point.value() {
            47 => {
                if (i != 0 && !content) || segments == 8 {
                    return false;
                }
                segments += 1;
                content = false;
            }
            97..=122 | 48..=57 | 95 | 123 | 125 | 45 if segments != 0 => content = true,
            _ => return false,
        }
    }
    content
}

#[cfg(test)]
mod tests {
    use super::*;

    fn points(content: &str, raw: bool) -> Result<Vec<u32>, Error> {
        decode([Part { content, raw }], || false)
            .map(|point| point.map(CodePoint::value))
            .collect()
    }

    #[test]
    fn text_decoding_preserves_python_points_and_token_boundaries() {
        assert_eq!(points("", false), Ok(vec![]));
        assert_eq!(points("\u{1f600}", false), Ok(vec![0x1f600]));
        assert_eq!(points(r"\ud83d\ude00", false), Ok(vec![0xd83d, 0xde00]));
        assert_eq!(points(r"\U0001f600", false), Ok(vec![0x1f600]));
        assert_eq!(points(r"\x41\u0042\U00000043", false), Ok(vec![65, 66, 67]));
        assert_eq!(
            points(r"\0\12\1234\777\8", false),
            Ok(vec![0, 10, 83, 52, 511, 92, 56])
        );
        assert_eq!(
            points(r#"\a\b\f\n\r\t\v\\\'\"\q"#, false),
            Ok(vec![7, 8, 12, 10, 13, 9, 11, 92, 39, 34, 92, 113])
        );
        assert_eq!(
            points("a\r\nb\rc\nd\\\r\ne\\\rf\\\ng\\r", false),
            Ok("a\nb\nc\ndefg\r".chars().map(|ch| ch as u32).collect())
        );
        let raw = "\\\"\\'\\n\\N{name}\\\r\n\r";
        assert_eq!(
            points(raw, true),
            Ok("\\\"\\'\\n\\N{name}\\\n\n"
                .chars()
                .map(|ch| ch as u32)
                .collect())
        );
        // u has the same content semantics as ordinary text; flavor admission
        // and rejecting bytes/f-strings are explicitly the caller's contract.
        let parts = [
            Part {
                content: r"\x41",
                raw: false,
            },
            Part {
                content: r"\n",
                raw: true,
            },
            Part {
                content: r"\u0042",
                raw: false,
            },
        ];
        assert_eq!(
            decode(parts, || false)
                .map(|p| p.unwrap().value())
                .collect::<Vec<_>>(),
            [65, 92, 110, 66]
        );
        let parts = [
            Part {
                content: "\\",
                raw: false,
            },
            Part {
                content: "n",
                raw: false,
            },
        ];
        assert_eq!(
            decode(parts, || false).next(),
            Some(Err(Error::InvalidSyntax))
        );
        let parts = [
            Part {
                content: r"\x4",
                raw: false,
            },
            Part {
                content: "1",
                raw: false,
            },
        ];
        assert_eq!(
            decode(parts, || false).next(),
            Some(Err(Error::InvalidSyntax))
        );
        for content in [r"\x", r"\x0g", r"\u123", r"\U0000000z", "\\"] {
            assert_eq!(
                points(content, false),
                Err(Error::InvalidSyntax),
                "{content}"
            );
        }
        assert_eq!(points(r"\U00110000", false), Err(Error::OutOfRange));
        assert_eq!(points(r"\Uffffffff", false), Err(Error::OutOfRange));
        assert_eq!(
            points(r"\uD800\U0010FFFF", false),
            Ok(vec![0xd800, 0x10ffff])
        );
        assert_eq!(
            points(r"\N{SPACE}", false),
            Err(Error::UnsupportedNamedEscape)
        );
    }

    #[test]
    fn retention_bounds_do_not_hide_late_failures_or_cancellation() {
        for limit in [TextLimit::Route, TextLimit::Prefix] {
            let cap = limit.points();
            assert_eq!(
                retention_work(limit),
                2 * cap * std::mem::size_of::<CodePoint>()
            );
            for count in [0, cap - 1, cap, cap + 1] {
                let content = "\u{1f600}".repeat(count);
                let result = retain_text(
                    [Part {
                        content: &content,
                        raw: false,
                    }],
                    limit,
                    || false,
                )
                .unwrap();
                if count > cap {
                    assert_eq!(result, Retention::Oversize);
                    assert_eq!(retained_bytes(&result), 0);
                } else {
                    assert_eq!(
                        retained_bytes(&result),
                        count * std::mem::size_of::<CodePoint>()
                    );
                }
            }
            for (suffix, error) in [
                (r"\xzz", Error::InvalidSyntax),
                (r"\N{SPACE}", Error::UnsupportedNamedEscape),
                (r"\U00110000", Error::OutOfRange),
            ] {
                let content = format!("{}{suffix}", "x".repeat(cap + 1));
                assert_eq!(
                    retain_text(
                        [Part {
                            content: &content,
                            raw: false
                        }],
                        limit,
                        || false
                    ),
                    Err(error)
                );
            }
            let content = "x".repeat(cap + 100);
            let mut checks = 0;
            assert_eq!(
                retain_text(
                    [Part {
                        content: &content,
                        raw: false
                    }],
                    limit,
                    || {
                        checks += 1;
                        checks > cap + 10
                    }
                ),
                Err(Error::Cancelled)
            );
        }
        let mut checks = 0;
        assert_eq!(
            decode(
                std::iter::repeat(Part {
                    content: "",
                    raw: false
                }),
                || {
                    checks += 1;
                    checks == 10
                }
            )
            .next(),
            Some(Err(Error::Cancelled))
        );
        let mut checks = 0;
        assert_eq!(
            decode(
                [Part {
                    content: "\\\n\\\n\\\n",
                    raw: false
                }],
                || {
                    checks += 1;
                    checks == 4
                }
            )
            .next(),
            Some(Err(Error::Cancelled))
        );
    }

    #[test]
    fn method_uppercase_has_only_the_recorded_relevant_preimages() {
        for word in METHODS {
            let content = word.to_ascii_lowercase();
            assert_eq!(
                method(decode(
                    [Part {
                        content: &content,
                        raw: false
                    }],
                    || false
                )),
                Ok(Some(word))
            );
        }
        for content in ["opt\u{131}ons", "po\u{17f}t", "po\u{fb05}", "po\u{fb06}"] {
            let expected = if content.starts_with("opt") {
                "OPTIONS"
            } else {
                "POST"
            };
            assert_eq!(
                method(decode(
                    [Part {
                        content,
                        raw: false
                    }],
                    || false
                )),
                Ok(Some(expected))
            );
        }
        for content in [
            "",
            "get ",
            "OPTIONSx",
            "po\u{130}t",
            "\u{df}",
            "\u{3c3}",
            "\u{d7ff}",
            "\u{1f600}",
            r"\ud800",
        ] {
            assert_eq!(
                method(decode(
                    [Part {
                        content,
                        raw: false
                    }],
                    || false
                )),
                Ok(None)
            );
        }
        for content in [r"impossible\xzz", r"OPTIONSoverflow\xzz"] {
            assert_eq!(
                method(decode(
                    [Part {
                        content,
                        raw: false
                    }],
                    || false
                )),
                Err(Error::InvalidSyntax)
            );
        }
        let mut checks = 0;
        assert_eq!(
            method(decode(
                [Part {
                    content: "impossible_still_validate",
                    raw: false
                }],
                || {
                    checks += 1;
                    checks == 15
                }
            )),
            Err(Error::Cancelled)
        );
    }

    #[test]
    fn prefix_grammar_is_exact_and_counts_segments_and_points() {
        let valid = |s: &str| valid_prefix(&s.chars().map(CodePoint::from).collect::<Vec<_>>());
        for s in ["/a", "/{id}/x-0_", "/a/b/c/d/e/f/g/h"] {
            assert!(valid(s), "{s}");
        }
        for s in [
            "",
            "/",
            "a",
            "/a/",
            "//a",
            "/a//b",
            "/A",
            "/a b",
            "/\u{e9}",
            "/a/b/c/d/e/f/g/h/i",
        ] {
            assert!(!valid(s), "{s}");
        }
        for len in [127, 128, 129] {
            assert_eq!(valid(&format!("/{}", "a".repeat(len - 1))), len <= 128);
        }
        assert!(!valid_prefix(&[
            CodePoint::from('/'),
            CodePoint::new(0xd800).unwrap()
        ]));
    }

    #[cfg(unix)]
    #[test]
    #[ignore = "requires explicit trusted Python 3.12 with Unicode 15.0.0"]
    fn method_preimages_match_exhaustive_trusted_stdlib() {
        let python = crate::analysis::python_parity::oracle();
        let script = r#"
import json, sys, unicodedata
assert sys.version_info[:2] == (3, 12), sys.version
assert unicodedata.unidata_version == '15.0.0', unicodedata.unidata_version
words = ('DELETE', 'GET', 'HEAD', 'OPTIONS', 'PATCH', 'POST', 'PUT', 'TRACE')
result = []
for point in range(0x110000):
    upper = chr(point).upper()
    assert upper, point
    if point >= 128 and upper.isascii() and any(upper in word for word in words):
        result.append([point, upper])
json.dump(result, sys.stdout, ensure_ascii=True)
"#;
        let mut command = std::process::Command::new(python);
        command
            .args(["-I", "-S", "-c", script])
            .current_dir("/")
            .env_clear()
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let output =
            crate::analysis::run_bounded(command, Vec::new(), "Python method preimages").unwrap();
        let actual: Vec<(u32, String)> = serde_json::from_slice(&output).unwrap();
        assert_eq!(
            actual,
            METHOD_PREIMAGES.map(|(point, text)| (point, text.to_owned()))
        );
    }
}
