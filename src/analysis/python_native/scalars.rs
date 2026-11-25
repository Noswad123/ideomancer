//! Parser-independent Python text storage and `ensure_ascii=True` JSON spelling.
//! Callers still own lexical validation, input budgets, and Unicode name lookup.
use std::io::{self, Write};

/// Python strings include surrogate code points, unlike Rust strings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CodePoint(u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct InvalidCodePoint;

impl CodePoint {
    #[allow(dead_code)] // Checked seam for a future decoder, not syntax admission.
    pub(super) fn new(value: u32) -> Result<Self, InvalidCodePoint> {
        if value <= 0x10ffff {
            Ok(Self(value))
        } else {
            Err(InvalidCodePoint)
        }
    }
}

impl From<char> for CodePoint {
    fn from(value: char) -> Self {
        Self(value as u32)
    }
}

#[derive(Debug)]
pub(super) enum JsonError {
    Cancelled,
    OutputLimit,
    Io(io::Error),
}

impl std::fmt::Display for JsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => f.write_str("cancelled"),
            Self::OutputLimit => f.write_str("ASCII JSON output limit"),
            Self::Io(error) => std::fmt::Display::fmt(error, f),
        }
    }
}

impl std::error::Error for JsonError {}

/// Streams bounded ASCII bytes to either storage or a hashing adapter. An error
/// may leave a partial sink; callers must discard it, never publish that prefix.
pub(super) struct AsciiJson<W, C> {
    sink: W,
    cancelled: C,
    remaining: usize,
    stopped: bool,
}

impl<W: Write, C: FnMut() -> bool> AsciiJson<W, C> {
    pub(super) fn new(sink: W, limit: usize, cancelled: C) -> Self {
        Self {
            sink,
            cancelled,
            remaining: limit,
            stopped: false,
        }
    }

    /// Only closed, caller-controlled JSON punctuation/numbers belong here.
    pub(super) fn raw(&mut self, bytes: &[u8]) -> Result<(), JsonError> {
        if self.stopped || (self.cancelled)() {
            self.stopped = true;
            return Err(JsonError::Cancelled);
        }
        if bytes.len() > self.remaining {
            return Err(JsonError::OutputLimit);
        }
        self.sink.write_all(bytes).map_err(JsonError::Io)?;
        self.remaining -= bytes.len();
        Ok(())
    }

    pub(super) fn text(&mut self, text: &str) -> Result<(), JsonError> {
        self.points(text.chars().map(CodePoint::from))
    }

    pub(super) fn points(
        &mut self,
        points: impl IntoIterator<Item = CodePoint>,
    ) -> Result<(), JsonError> {
        self.raw(b"\"")?;
        for CodePoint(point) in points {
            match point {
                0x22 => self.raw(b"\\\"")?,
                0x5c => self.raw(b"\\\\")?,
                8 => self.raw(b"\\b")?,
                12 => self.raw(b"\\f")?,
                10 => self.raw(b"\\n")?,
                13 => self.raw(b"\\r")?,
                9 => self.raw(b"\\t")?,
                0x20..=0x7e => self.raw(&[point as u8])?,
                0..=0xffff => self.unit(point as u16)?,
                _ => {
                    let offset = point - 0x10000;
                    self.unit(0xd800 | (offset >> 10) as u16)?;
                    self.unit(0xdc00 | (offset & 0x3ff) as u16)?;
                }
            }
        }
        self.raw(b"\"")
    }

    fn unit(&mut self, unit: u16) -> Result<(), JsonError> {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        self.raw(&[
            b'\\',
            b'u',
            HEX[(unit >> 12) as usize],
            HEX[((unit >> 8) & 15) as usize],
            HEX[((unit >> 4) & 15) as usize],
            HEX[(unit & 15) as usize],
        ])
    }

    pub(super) fn into_inner(self) -> W {
        self.sink
    }
}

// This retention seam is intentionally not wired to literal parsing or G3.
#[allow(dead_code)]
#[derive(Clone, Copy)]
pub(super) enum TextLimit {
    Route,
    Prefix,
}

impl TextLimit {
    fn points(self) -> usize {
        match self {
            Self::Route => 2048,
            Self::Prefix => 128,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Retention {
    Retained(Box<[CodePoint]>),
    Oversize,
}

#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq)]
pub(super) enum RetentionError<E> {
    Invalid(E),
    Cancelled,
}

/// Retain at most the contextual number of Python code points. Overflow drops
/// all payload but continues validating every remaining event. The caller must
/// bound input work; this is not a decoder or an HTTP method normalizer.
#[allow(dead_code)]
pub(super) fn retain<E>(
    events: impl IntoIterator<Item = Result<CodePoint, E>>,
    limit: TextLimit,
    mut cancelled: impl FnMut() -> bool,
) -> Result<Retention, RetentionError<E>> {
    if cancelled() {
        return Err(RetentionError::Cancelled);
    }
    let limit = limit.points();
    // Fixed boxed storage has no geometric spare capacity beyond the point cap.
    let mut payload = Some(vec![CodePoint(0); limit].into_boxed_slice());
    let mut count = 0;
    for event in events {
        if cancelled() {
            return Err(RetentionError::Cancelled);
        }
        let point = event.map_err(RetentionError::Invalid)?;
        if let Some(points) = &mut payload {
            if count == limit {
                payload = None;
            } else {
                points[count] = point;
                count += 1;
            }
        }
    }
    Ok(match payload {
        Some(points) => {
            let mut points = points.into_vec();
            points.truncate(count);
            Retention::Retained(points.into_boxed_slice())
        }
        None => Retention::Oversize,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(points: impl IntoIterator<Item = CodePoint>) -> String {
        let mut writer = AsciiJson::new(Vec::new(), 4096, || false);
        writer.points(points).unwrap();
        String::from_utf8(writer.into_inner()).unwrap()
    }

    #[test]
    fn ascii_json_exact_text_and_checked_points() {
        assert_eq!(encode([]), "\"\"");
        assert_eq!(
            encode((0..=31).map(|value| CodePoint::new(value).unwrap())),
            r#""\u0000\u0001\u0002\u0003\u0004\u0005\u0006\u0007\b\t\n\u000b\f\r\u000e\u000f\u0010\u0011\u0012\u0013\u0014\u0015\u0016\u0017\u0018\u0019\u001a\u001b\u001c\u001d\u001e\u001f""#
        );
        assert_eq!(
            encode(
                "\0\u{1}\u{8}\u{c}\n\r\t\u{1f} ~\u{7f}\"\\\u{e9}\u{1f600}"
                    .chars()
                    .map(CodePoint::from)
            ),
            "\"\\u0000\\u0001\\b\\f\\n\\r\\t\\u001f ~\\u007f\\\"\\\\\\u00e9\\ud83d\\ude00\""
        );
        assert_eq!(CodePoint::new(0x110000), Err(InvalidCodePoint));
        assert_eq!(encode([CodePoint::new(0xffff).unwrap()]), "\"\\uffff\"");
        assert_eq!(
            encode([CodePoint::new(0x10000).unwrap()]),
            "\"\\ud800\\udc00\""
        );
        assert_eq!(
            encode([CodePoint::new(0x10ffff).unwrap()]),
            "\"\\udbff\\udfff\""
        );
        assert_eq!(encode([CodePoint::new(0xd800).unwrap()]), "\"\\ud800\"");
        assert_eq!(encode([CodePoint::new(0xdfff).unwrap()]), "\"\\udfff\"");
        assert_eq!(
            encode([
                CodePoint::new(0xd83d).unwrap(),
                CodePoint::new(0xde00).unwrap()
            ]),
            encode([CodePoint::from('\u{1f600}')])
        );
    }

    #[test]
    fn ascii_json_limits_cancellation_and_io() {
        for (limit, success) in [(7, false), (8, true)] {
            let mut writer = AsciiJson::new(Vec::new(), limit, || false);
            let result = writer.text("\u{7f}");
            assert_eq!(result.is_ok(), success);
            if !success {
                assert!(matches!(result, Err(JsonError::OutputLimit)));
            }
            assert!(writer.into_inner().len() <= limit);
        }
        let mut calls = 0;
        let mut writer = AsciiJson::new(Vec::new(), 100, || {
            calls += 1;
            calls == 3
        });
        assert!(matches!(writer.text("abc"), Err(JsonError::Cancelled)));
        assert!(matches!(writer.text(""), Err(JsonError::Cancelled)));
        assert_eq!(writer.into_inner(), b"\"a");
        // Use a closed fixed-size slice to exercise WriteZero propagation.
        let mut empty = [];
        let mut failing = AsciiJson::new(&mut empty[..], 100, || false);
        match failing.text("") {
            Err(JsonError::Io(error)) => assert_eq!(error.kind(), io::ErrorKind::WriteZero),
            result => panic!("unexpected result: {result:?}"),
        }
    }

    #[test]
    fn ascii_json_surrogate_metadata_gets_a_second_identity_escape() {
        let mut metadata = AsciiJson::new(Vec::new(), 64, || false);
        metadata.raw(b"{\"text\":").unwrap();
        metadata.points([CodePoint::new(0xd800).unwrap()]).unwrap();
        metadata.raw(b"}").unwrap();
        let metadata = String::from_utf8(metadata.into_inner()).unwrap();
        assert_eq!(metadata, r#"{"text":"\ud800"}"#);
        let mut identity = AsciiJson::new(Vec::new(), 256, || false);
        identity
            .raw(b"[\"sample.py\", \"metadata\", 1, 1, \"sample\", ")
            .unwrap();
        identity.text(&metadata).unwrap();
        identity.raw(b"]").unwrap();
        assert_eq!(
            String::from_utf8(identity.into_inner()).unwrap(),
            r#"["sample.py", "metadata", 1, 1, "sample", "{\"text\":\"\\ud800\"}"]"#
        );
    }

    #[test]
    fn bounded_retention_counts_points_and_checks_late_errors() {
        let point = CodePoint::from('\u{1f600}');
        for (limit, cap) in [(TextLimit::Route, 2048), (TextLimit::Prefix, 128)] {
            for size in [0, cap - 1, cap, cap + 1] {
                let result = retain(std::iter::repeat_n(Ok::<_, ()>(point), size), limit, || {
                    false
                })
                .unwrap();
                match result {
                    Retention::Retained(points) => {
                        assert_eq!(points.len(), size);
                        assert!(std::mem::size_of_val(&*points) <= cap * 4);
                    }
                    Retention::Oversize => assert!(size > cap),
                }
            }
        }
        let pair = [
            CodePoint::new(0xd83d).unwrap(),
            CodePoint::new(0xde00).unwrap(),
        ];
        let events = std::iter::repeat_n(Ok::<_, ()>(point), 127).chain(pair.map(Ok));
        assert_eq!(
            retain(events, TextLimit::Prefix, || false),
            Ok(Retention::Oversize)
        );
        for error in ["malformed", "unresolved-name"] {
            // Synthetic decoder events, not evidence of implemented decoding.
            let events = std::iter::repeat_n(Ok(point), 129).chain([Err(error)]);
            assert_eq!(
                retain(events, TextLimit::Prefix, || false),
                Err(RetentionError::Invalid(error))
            );
        }
        let mut calls = 0;
        let events = std::iter::repeat_n(Ok::<_, ()>(point), 256);
        assert_eq!(
            retain(events, TextLimit::Prefix, || {
                calls += 1;
                calls == 132
            }),
            Err(RetentionError::Cancelled)
        );
    }
}
