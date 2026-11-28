//! Bounded captured-source decoding; no grammar repairs or newline rewriting.
//!
//! The caller hashes the original bytes and uses `text` for ALL subsequent byte
//! ranges (lexer, parser, tokens, literal parts, line indices and Driver source).
//! Decoded offsets must never index the original Latin-1 bytes.
use super::codecs312::{self, Codec, Resolution};
use std::borrow::Cow;

const CHUNK: usize = 256;
pub(super) const INVALID_UTF8: &str = "invalid UTF-8 source";
pub(super) const BOM_CONFLICT: &str = "encoding problem with BOM";
pub(super) const CODEC_DECODE: &str = "source codec decoding failure";
pub(super) const NON_TEXT_CODEC: &str = "non-text source codec";
pub(super) const UNDEFINED_CODEC: &str = "undefined source codec";
pub(super) const NULL_BYTE: &str = "null byte in source";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Encoding {
    Utf8,
    Latin1,
    Ascii,
    SingleByte(usize),
}

impl Encoding {
    fn point(self, byte: u8) -> Option<char> {
        match self {
            Self::Latin1 => Some(char::from(byte)),
            Self::Ascii => byte.is_ascii().then_some(char::from(byte)),
            Self::SingleByte(index) => codecs312::decode_point(index, byte),
            Self::Utf8 => unreachable!("UTF-8 is validated as a stream"),
        }
    }
}

/// Only closed, proved source categories with the helper's normalized zero line.
/// A physical UTF-8 line or an unknown codec is never promoted to a certificate.
pub(super) fn certified_syntax(error: Error) -> bool {
    matches!(error, Error::Syntax { line: 0, reason } if matches!(reason,
        BOM_CONFLICT | CODEC_DECODE | NON_TEXT_CODEC | UNDEFINED_CODEC | NULL_BYTE | INVALID_UTF8))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Error {
    /// Decoder location: the offending physical byte line for strict UTF-8,
    /// or sentinel zero for BOM conflicts and registry-alias decode failures.
    /// This is not generally CPython's PEG diagnostic/literal-opening line.
    /// Reasons are closed static categories, never codec names or source text.
    Syntax {
        line: usize,
        reason: &'static str,
    },
    /// A finite set of known valid codecs outside this decoder's profile.
    UnsupportedCodec,
    /// Registry membership is unknown, NOT a claim of CPython SyntaxError.
    /// The full codec-registry/unknown-codec taxonomy remains Pending.
    UnresolvedCodec,
    ResourceLimit,
    Cancelled,
}

#[derive(Debug)]
pub(super) struct DecodedSource<'a> {
    pub(super) text: Cow<'a, str>,
    pub(super) encoding: Encoding,
    /// Live heap capacity, zero for borrowed UTF-8. Excludes the original bytes.
    pub(super) allocation_bytes: usize,
}

/// Decode without retaining a second source tree or exposing raw diagnostics.
///
/// `reserve(decoded_bytes)` is called at most once, after validation/counting and
/// before any source allocation (also for borrowed UTF-8). It must transactionally
/// charge the invocation's decoded-byte budget; a rejecting callback must not
/// charge anything. A caller can additionally charge working bytes conservatively
/// by this amount, even for borrowed input. The per-file limit is checked first.
///
/// A successful reservation belongs to the CALLER, including on a subsequent
/// allocation failure or cancellation. Record the amount in the callback. On Err,
/// refund working bytes immediately; on Ok, drop the returned view before refunding
/// its working reservation. A cumulative decoded-byte limit may retain its charge
/// until invocation end. No callback is invoked from Drop. This allows a frontend
/// budget before its fact sink exists, with explicit rollback on every exit path.
///
/// Suggested frontend limits: 2 MiB decoded per 1 MiB captured file, 32 MiB total.
/// Cancellation is checked before work and each <=256-byte scan/build chunk.
/// Safe construction of a borrowed str requires a final bounded std UTF-8 check
/// after the interruptible validation pass; allocation itself is uninterruptible.
/// Strict UTF-8 intentionally rejects even invalid bytes in comments that CPython's
/// canonical UTF-8 tokenizer sometimes ignores. This is not full encoding parity.
/// Its physical failure line can also differ from CPython's opening-token line
/// inside multiline literals. No grammar scan is attempted before decoding; these
/// finite location/admission differences remain Pending G4, not PEG parity claims.
pub(super) fn decode<'a>(
    bytes: &'a [u8],
    decoded_limit: usize,
    reserve: &mut impl FnMut(usize) -> Result<(), Error>,
    cancel: &mut impl FnMut() -> bool,
) -> Result<DecodedSource<'a>, Error> {
    checkpoint(cancel)?;
    let bom = bytes.starts_with(b"\xef\xbb\xbf");
    let bytes = if bom { &bytes[3..] } else { bytes };
    let cookie = cookie(bytes, cancel)?;
    let canonical = cookie.map(normal_name);
    if bom && canonical.is_some_and(|name| name != Some(Encoding::Utf8)) {
        return Err(Error::Syntax {
            line: 0,
            reason: BOM_CONFLICT,
        });
    }
    let encoding = match (cookie, canonical.flatten()) {
        (None, _) => Encoding::Utf8,
        (_, Some(encoding)) => encoding,
        (Some(name), None) => {
            match codecs312::lookup(name, cancel).map_err(|_| Error::Cancelled)? {
                Resolution::Supported(Codec::Utf8) => Encoding::Utf8,
                Resolution::Supported(Codec::Latin1) => Encoding::Latin1,
                Resolution::Supported(Codec::Ascii) => Encoding::Ascii,
                Resolution::Supported(Codec::SingleByte(index)) => Encoding::SingleByte(index),
                Resolution::UnsupportedText => return Err(Error::UnsupportedCodec),
                Resolution::Unresolved => return Err(Error::UnresolvedCodec),
                Resolution::NonText => {
                    return Err(Error::Syntax {
                        line: 0,
                        reason: NON_TEXT_CODEC,
                    })
                }
                Resolution::AlwaysFails => {
                    return Err(Error::Syntax {
                        line: 0,
                        reason: UNDEFINED_CODEC,
                    })
                }
            }
        }
    };
    if bytes.len() > decoded_limit {
        return Err(Error::ResourceLimit);
    }
    match encoding {
        Encoding::Utf8 => {
            let mut start = 0;
            while start < bytes.len() {
                checkpoint(cancel)?;
                let end = start.saturating_add(CHUNK).min(bytes.len());
                match std::str::from_utf8(&bytes[start..end]) {
                    Ok(_) => start = end,
                    Err(error) if error.error_len().is_none() && end < bytes.len() => {
                        // Retry the incomplete scalar in the next bounded window.
                        start += error.valid_up_to();
                    }
                    Err(error) => {
                        let offset = start + error.valid_up_to();
                        let line = if cookie.is_some() && canonical != Some(Some(Encoding::Utf8)) {
                            0
                        } else {
                            physical_line_number(bytes, offset, cancel)?
                        };
                        return Err(Error::Syntax {
                            line,
                            reason: INVALID_UTF8,
                        });
                    }
                }
            }
            let text = std::str::from_utf8(bytes).expect("validated UTF-8 windows");
            checkpoint(cancel)?;
            reserve(text.len())?;
            checkpoint(cancel)?;
            Ok(DecodedSource {
                text: Cow::Borrowed(text),
                encoding,
                allocation_bytes: 0,
            })
        }
        Encoding::Latin1 | Encoding::Ascii | Encoding::SingleByte(_) => {
            let mut needed = 0usize;
            let mut ascii = true;
            for chunk in bytes.chunks(CHUNK) {
                checkpoint(cancel)?;
                for &byte in chunk {
                    ascii &= byte.is_ascii();
                    let point = encoding.point(byte).ok_or(Error::Syntax {
                        line: 0,
                        reason: CODEC_DECODE,
                    })?;
                    needed = needed
                        .checked_add(point.len_utf8())
                        .filter(|&length| length <= decoded_limit)
                        .ok_or(Error::ResourceLimit)?;
                }
            }
            checkpoint(cancel)?;
            reserve(needed)?;
            checkpoint(cancel)?;
            if ascii {
                return Ok(DecodedSource {
                    text: Cow::Borrowed(
                        std::str::from_utf8(bytes).expect("ASCII identity profile"),
                    ),
                    encoding,
                    allocation_bytes: 0,
                });
            }
            let mut text = String::new();
            text.try_reserve_exact(needed)
                .map_err(|_| Error::ResourceLimit)?;
            // Do not retain allocator rounding outside the precharged bound.
            if text.capacity() > needed {
                return Err(Error::ResourceLimit);
            }
            for chunk in bytes.chunks(CHUNK) {
                checkpoint(cancel)?;
                for &byte in chunk {
                    text.push(
                        encoding
                            .point(byte)
                            .expect("validated immutable codec table"),
                    );
                }
            }
            checkpoint(cancel)?;
            debug_assert_eq!(text.len(), needed);
            Ok(DecodedSource {
                allocation_bytes: text.capacity(),
                text: Cow::Owned(text),
                encoding,
            })
        }
    }
}

fn checkpoint(cancel: &mut impl FnMut() -> bool) -> Result<(), Error> {
    if cancel() {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}

fn physical_line<'a>(
    bytes: &'a [u8],
    cancel: &mut impl FnMut() -> bool,
) -> Result<(&'a [u8], &'a [u8]), Error> {
    for (index, &byte) in bytes.iter().enumerate() {
        if index % CHUNK == 0 {
            checkpoint(cancel)?;
        }
        if matches!(byte, b'\r' | b'\n') {
            let width = 1 + usize::from(byte == b'\r' && bytes.get(index + 1) == Some(&b'\n'));
            return Ok((&bytes[..index], &bytes[index + width..]));
        }
    }
    Ok((bytes, &[]))
}

fn physical_line_number(
    bytes: &[u8],
    offset: usize,
    cancel: &mut impl FnMut() -> bool,
) -> Result<usize, Error> {
    let mut line = 1;
    for index in 0..offset {
        if index % CHUNK == 0 {
            checkpoint(cancel)?;
        }
        if bytes[index] == b'\r'
            || (bytes[index] == b'\n' && (index == 0 || bytes[index - 1] != b'\r'))
        {
            line += 1;
        }
    }
    Ok(line)
}

fn cookie<'a>(
    bytes: &'a [u8],
    cancel: &mut impl FnMut() -> bool,
) -> Result<Option<&'a [u8]>, Error> {
    let mut remaining = bytes;
    for _ in 0..2 {
        let (line, rest) = physical_line(remaining, cancel)?;
        remaining = rest;
        let mut start = 0;
        while line
            .get(start)
            .is_some_and(|b| matches!(b, b' ' | b'\t' | 0x0c))
        {
            if start % CHUNK == 0 {
                checkpoint(cancel)?;
            }
            start += 1;
        }
        if start == line.len() {
            continue;
        }
        if line[start] != b'#' {
            return Ok(None);
        }
        let mut index = start;
        while index < line.len() && line[index] != 0 {
            if index % CHUNK == 0 {
                checkpoint(cancel)?;
            }
            if line[index..].starts_with(b"coding")
                && matches!(line.get(index + 6), Some(b':' | b'='))
            {
                let mut begin = index + 7;
                while line.get(begin).is_some_and(|b| matches!(b, b' ' | b'\t')) {
                    if begin % CHUNK == 0 {
                        checkpoint(cancel)?;
                    }
                    begin += 1;
                }
                let mut end = begin;
                while line
                    .get(end)
                    .is_some_and(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
                {
                    if end % CHUNK == 0 {
                        checkpoint(cancel)?;
                    }
                    end += 1;
                }
                if begin != end {
                    return Ok(Some(&line[begin..end]));
                }
            }
            index += 1;
        }
    }
    Ok(None)
}

/// CPython tokenizer's historical normalization precedes codec-registry lookup.
/// Only its first twelve bytes are examined; underscores become hyphens.
fn normal_name(name: &[u8]) -> Option<Encoding> {
    let mut buffer = [0; 12];
    let count = name.len().min(buffer.len());
    for (out, &byte) in buffer.iter_mut().zip(name) {
        *out = if byte == b'_' {
            b'-'
        } else {
            byte.to_ascii_lowercase()
        };
    }
    let name = &buffer[..count];
    if name == b"utf-8" || name.starts_with(b"utf-8-") {
        Some(Encoding::Utf8)
    } else if [b"latin-1".as_slice(), b"iso-8859-1", b"iso-latin-1"]
        .iter()
        .any(|prefix| {
            name == *prefix || (name.starts_with(prefix) && name.get(prefix.len()) == Some(&b'-'))
        })
    {
        Some(Encoding::Latin1)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn decoded(bytes: &[u8]) -> Result<DecodedSource<'_>, Error> {
        decode(bytes, 2 * 1024 * 1024, &mut |_| Ok(()), &mut || false)
    }

    #[test]
    fn borrowed_view_strips_one_bom_preserving_newlines_nuls_and_original_hash() {
        for bytes in [
            b"".as_slice(),
            b"\xef\xbb\xbf",
            b"\xef\xbb\xbfx=1\r\ny=2\rz=3\n\0",
            b"\xef\xbb\xbf\xef\xbb\xbfx=1\n",
        ] {
            let original = bytes.to_vec();
            let hash = Sha256::digest(bytes);
            let view = decoded(bytes).unwrap();
            let expected = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
            assert!(matches!(&view.text, Cow::Borrowed(_)));
            assert_eq!(view.text.as_bytes(), expected);
            assert_eq!(view.text.as_ptr(), expected.as_ptr());
            assert_eq!(view.encoding, Encoding::Utf8);
            assert_eq!(view.allocation_bytes, 0);
            assert_eq!(bytes, original);
            assert_eq!(Sha256::digest(bytes), hash);
        }
    }

    #[test]
    fn cookie_eligibility_and_marker_grammar() {
        for header in [
            "# coding: latin-1\n",
            "#!/synthetic\r\n# coding=latin-1\r\n",
            " \t\x0c\r\x0c # arbitrary decoding: latin-1\r",
            "# coding: ; coding: latin-1\n",
            "# coding:\n# coding: latin-1\n",
        ] {
            let mut bytes = header.as_bytes().to_vec();
            bytes.extend_from_slice(b"x='\xe9'\n");
            let result = decoded(&bytes).unwrap();
            assert_eq!(result.encoding, Encoding::Latin1, "{header:?}");
            assert!(result.text.ends_with("x='\u{e9}'\n"));
        }
        for source in [
            "x=1\n# coding: latin-1\n",
            "'first'\n# coding: latin-1\n",
            "'''\n# coding: latin-1\n'''\n",
            "'# coding: latin-1'\n",
            "# first\n# second\n# coding: latin-1\n",
            "# Coding: latin-1\n",
            "# coding : latin-1\n",
            "# coding:\n",
            "# coding: ;\n",
            "# coding:\x0clatin-1\n",
            "\x0b# coding: latin-1\n",
            "x=1 # coding: latin-1\n",
        ] {
            assert_eq!(decoded(source.as_bytes()).unwrap().encoding, Encoding::Utf8);
        }
        assert_eq!(
            decoded(b"#\xff\n# coding: latin-1\nx=1").unwrap().encoding,
            Encoding::Latin1
        );
        assert_eq!(
            decoded(b"# coding: ascii coding: latin-1\n")
                .unwrap()
                .encoding,
            Encoding::Ascii
        );
    }

    #[test]
    fn utf8_registry_aliases_and_bom_use_distinct_normalization() {
        for name in [
            "utf-8",
            "UTF_8",
            "utf-8-sig",
            "utf-8-anything",
            "utf_8_suffix",
        ] {
            for bom in ["", "\u{feff}"] {
                assert_eq!(
                    decoded(format!("{bom}# coding: {name}\nx=1\n").as_bytes())
                        .unwrap()
                        .encoding,
                    Encoding::Utf8
                );
            }
        }
        for name in ["utf8", "UTF8", "u8", "cp65001", "utf", "utf__8", "-utf-8-"] {
            assert_eq!(
                decoded(format!("# coding: {name}\nx=1\n").as_bytes())
                    .unwrap()
                    .encoding,
                Encoding::Utf8
            );
            assert_eq!(
                decoded(format!("\u{feff}# coding: {name}\n").as_bytes()).unwrap_err(),
                Error::Syntax {
                    line: 0,
                    reason: BOM_CONFLICT
                }
            );
        }
    }

    #[test]
    fn latin1_aliases_and_all_byte_values_have_exact_utf8_capacity() {
        for name in [
            "latin-1",
            "latin_1_suffix",
            "iso-8859-1-whatever",
            "iso-latin-1-more",
            "8859",
            "cp819",
            "csisolatin1",
            "ibm819",
            "iso8859",
            "iso8859_1",
            "iso_8859_1",
            "iso_8859_1_1987",
            "iso_ir_100",
            "l1",
            "latin",
            "latin1",
            "LATIN__1",
            "---latin1---",
        ] {
            let mut bytes = format!("# coding: {name}\r\n").into_bytes();
            bytes.extend(0..=255u8);
            let original = bytes.clone();
            let needed = bytes.len() + 128;
            let mut reservation = None;
            let view = decode(
                &bytes,
                needed,
                &mut |size| {
                    assert!(reservation.replace(size).is_none());
                    Ok(())
                },
                &mut || false,
            )
            .unwrap();
            assert_eq!(reservation, Some(needed));
            assert_eq!(view.allocation_bytes, needed);
            assert_eq!(view.text.len(), needed);
            assert_eq!(view.encoding, Encoding::Latin1);
            assert!(matches!(&view.text, Cow::Owned(_)));
            assert_eq!(
                view.text
                    .chars()
                    .rev()
                    .take(256)
                    .map(u32::from)
                    .collect::<Vec<_>>(),
                (0..=255u32).rev().collect::<Vec<_>>()
            );
            assert_eq!(bytes, original);
        }
    }

    #[test]
    fn errors_are_closed_and_codec_registry_uncertainty_is_explicit() {
        for name in ["shift_jis", "cp037", "cp864"] {
            assert_eq!(
                decoded(format!("# coding: {name}\n").as_bytes()).unwrap_err(),
                Error::UnsupportedCodec
            );
        }
        for name in [
            "not_a_codec",
            "utf.8",
            "latin..1",
            "utf-8suffix",
            "utf8_sig",
        ] {
            assert_eq!(
                decoded(format!("# coding: {name}\n").as_bytes()).unwrap_err(),
                Error::UnresolvedCodec
            );
            assert_eq!(
                decoded(format!("\u{feff}# coding: {name}\n").as_bytes()).unwrap_err(),
                Error::Syntax {
                    line: 0,
                    reason: BOM_CONFLICT
                }
            );
        }
        for (bytes, line) in [
            (b"x='\xff'\n".as_slice(), 1),
            (b"# coding: utf-8\r\nx=1\rx='\xff'\n", 3),
            (b"# coding: utf8\nx='\xff'\n", 0),
            (b"\xef\xbb\xbfx='\xff'\n", 1),
            (b"# invalid comment \xff\n", 1),
            (b"x='\xc0\x80'\n", 1),
            (b"x='\xed\xa0\x80'\n", 1),
            (b"x='\xf4\x90\x80\x80'\n", 1),
            (b"x='\xe2\x82", 1),
        ] {
            assert_eq!(
                decoded(bytes).unwrap_err(),
                Error::Syntax {
                    line,
                    reason: INVALID_UTF8
                }
            );
        }
    }

    #[test]
    fn utf8_scalars_cross_every_validation_window_boundary() {
        for offset in 250..260 {
            for scalar in ["\u{e9}", "\u{20ac}", "\u{1f600}"] {
                let text = format!("{}{}{}", " ".repeat(offset), scalar, " ".repeat(260));
                assert_eq!(decoded(text.as_bytes()).unwrap().text, text);
            }
        }
    }

    #[test]
    fn limits_and_cancellation_do_not_allocate_before_reservation() {
        let bytes = b"# coding: latin-1\nx='\xff\xff'\n";
        let needed = bytes.len() + 2;
        let mut called = false;
        assert_eq!(
            decode(
                bytes,
                needed - 1,
                &mut |_| {
                    called = true;
                    Ok(())
                },
                &mut || false,
            )
            .unwrap_err(),
            Error::ResourceLimit
        );
        assert!(!called);
        assert_eq!(
            decode(
                bytes,
                needed,
                &mut |_| Err(Error::ResourceLimit),
                &mut || false,
            )
            .unwrap_err(),
            Error::ResourceLimit
        );
        assert_eq!(
            decode(
                bytes,
                needed,
                &mut |_| {
                    called = true;
                    Ok(())
                },
                &mut || true,
            )
            .unwrap_err(),
            Error::Cancelled
        );
        assert!(!called);
        let mut long = b"# coding: latin-1\n".to_vec();
        long.extend(std::iter::repeat_n(0xff, 2048));
        // A shared flag permits cancellation immediately after successful charge.
        let charged = std::cell::Cell::new(0);
        for cancel_after in [1, 4] {
            let mut after_reservation = 0;
            assert_eq!(
                decode(
                    &long,
                    8192,
                    &mut |size| {
                        charged.set(size);
                        Ok(())
                    },
                    &mut || {
                        if charged.get() != 0 {
                            after_reservation += 1;
                        }
                        after_reservation == cancel_after
                    },
                )
                .unwrap_err(),
                Error::Cancelled
            );
            assert_eq!(charged.replace(0), long.len() + 2048);
            // Caller rollback releases the reservation, including mid-build.
        }
        for bytes in [long, vec![b'#'; 4096], vec![b' '; 4096]] {
            let mut checks = 0;
            assert_eq!(
                decode(
                    &bytes,
                    8192,
                    &mut |_| panic!("late reservation"),
                    &mut || {
                        checks += 1;
                        checks == 4
                    },
                )
                .unwrap_err(),
                Error::Cancelled
            );
            assert_eq!(checks, 4);
        }
        assert_eq!(
            decode(
                b"abc",
                2,
                &mut |_| panic!("over-limit reservation"),
                &mut || false,
            )
            .unwrap_err(),
            Error::ResourceLimit
        );
    }

    #[test]
    fn decoded_byte_budget_counts_borrowed_views_across_files() {
        let mut total = 0;
        let mut reserve = |size| {
            let next = total + size;
            if next > 6 {
                return Err(Error::ResourceLimit);
            }
            total = next;
            Ok(())
        };
        for bytes in [b"abc".as_slice(), b"\xef\xbb\xbfdef"] {
            let view = decode(bytes, 3, &mut reserve, &mut || false).unwrap();
            assert_eq!(view.allocation_bytes, 0);
        }
        assert_eq!(
            decode(b"x", 3, &mut reserve, &mut || false).unwrap_err(),
            Error::ResourceLimit
        );
        assert_eq!(total, 6);
    }

    #[cfg(unix)]
    #[test]
    #[ignore = "requires explicitly selected trusted Python 3.12; source decoding only"]
    fn trusted_ast_bytes_oracle() {
        use std::path::PathBuf;
        use std::process::{Command, Stdio};

        let python = PathBuf::from(
            std::env::var_os("IDEOMANCER_TEST_PYTHON")
                .expect("select a trusted Python 3.12 executable explicitly"),
        );
        assert!(python.is_absolute());
        let mut cases: Vec<Vec<u8>> = vec![
            b"".to_vec(),
            b"\xef\xbb\xbf".to_vec(),
            b"#\xff\n# coding: latin-1\nx='\x80\xe9\xff'\n".to_vec(),
            b"\x0c# first\r\n\t# decoding: cp819\rx='\xe9'\n".to_vec(),
            b"x=1\n# coding: latin-1\nx='\xc3\xa9'\n".to_vec(),
            b"'''\n# coding: latin-1\n'''\nx=1\n".to_vec(),
            b"# Coding: not_a_codec\nx=1\n".to_vec(),
            b"# coding: ; coding: latin-1\nx='\xe9'\n".to_vec(),
            b"x='\xff'\n".to_vec(),
            b"# coding: utf-8\r\nx=1\rx='\xff'\n".to_vec(),
            b"# coding: utf8\nx='\xff'\n".to_vec(),
            b"# coding: shift_jis\nx=1\n".to_vec(),
            b"# coding: not_a_codec\nx=1\n".to_vec(),
        ];
        for name in [
            "utf-8",
            "UTF_8",
            "utf8",
            "u8",
            "cp65001",
            "utf",
            "utf-8-sig",
            "utf__8",
            "utf-8-more",
            "latin-1",
            "iso-latin-1-more",
            "cp819",
            "iso_ir_100",
        ] {
            for bom in ["", "\u{feff}"] {
                cases.push(format!("{bom}# coding: {name}\nx='value'\n").into_bytes());
            }
        }
        let requests: Vec<_> = cases
            .iter()
            .map(|bytes| {
                let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
                let text = decoded(bytes).ok().map(|view| view.text.into_owned());
                serde_json::json!({ "hex": hex, "text": text })
            })
            .collect();
        let script = r#"
import ast, json, sys
assert sys.version_info[:2] == (3, 12)
request = sys.stdin.buffer.read(65537)
assert len(request) <= 65536
cases = json.loads(request)
assert len(cases) <= 128
results = []
for case in cases:
    encoded = case['hex']
    assert isinstance(encoded, str) and len(encoded) <= 8192
    source = bytes.fromhex(encoded)
    try:
        tree = ast.parse(source, type_comments=True)
    except SyntaxError as error:
        category = ('bom' if 'with BOM' in error.msg else
                    'unknown' if error.msg.startswith('unknown encoding:') else
                    'utf8' if 'codec can\'t decode' in error.msg else 'syntax')
        results.append(dict(category=category, line=error.lineno))
    else:
        if case['text'] is not None:
            decoded_tree = ast.parse(case['text'], type_comments=True)
            assert ast.dump(tree) == ast.dump(decoded_tree)
        # Only fixed synthetic assignments are inspected, never evaluated.
        names = [node.id for node in ast.walk(tree) if isinstance(node, ast.Name)]
        points = [list(map(ord, node.value)) for node in ast.walk(tree)
                  if isinstance(node, ast.Constant) and isinstance(node.value, str)]
        results.append(dict(category='ok', names=names, points=points))
json.dump(results, sys.stdout, ensure_ascii=True)
"#;
        let mut command = Command::new(python);
        command
            .args(["-I", "-S", "-B", "-c", script])
            .current_dir("/")
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let output = crate::analysis::run_bounded(
            command,
            serde_json::to_vec(&requests).unwrap(),
            "source decoding oracle",
        )
        .unwrap();
        let results: Vec<serde_json::Value> = serde_json::from_slice(&output).unwrap();
        assert_eq!(results.len(), cases.len());
        for (bytes, result) in cases.iter().zip(results) {
            match decoded(bytes) {
                Ok(view) => {
                    assert_eq!(result["category"], "ok");
                    assert!(result["names"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .all(|name| name == "x"));
                    if bytes.windows(3).any(|part| part == b"\x80\xe9\xff") {
                        assert_eq!(result["points"], serde_json::json!([[128, 233, 255]]));
                        assert!(view.text.contains("\u{80}\u{e9}\u{ff}"));
                    }
                }
                Err(Error::Syntax { line, reason }) => {
                    assert_eq!(
                        result["category"],
                        if reason == BOM_CONFLICT {
                            "bom"
                        } else {
                            "utf8"
                        }
                    );
                    assert_eq!(result["line"], line);
                }
                Err(Error::UnsupportedCodec) => assert_eq!(result["category"], "ok"),
                Err(Error::UnresolvedCodec) => assert_eq!(result["category"], "unknown"),
                Err(error) => panic!("unexpected decoder failure: {error:?}"),
            }
        }
    }
}
