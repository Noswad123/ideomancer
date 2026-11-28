//! Finite CPython 3.12.11 captured-source profile, never host properties.
//! Fixed ASCII records permit direct scalar access and bounded binary search.
//! The caller MUST complete the source BOM/history gate before lookup. UTF-8-SIG
//! is UTF-8 equivalent only on the remaining signature-free source buffer.

const TABLES: &str = include_str!("codecs312/tables.tsv");
const ALIASES: &[u8] = include_bytes!("codecs312/aliases.tsv");
const TABLE_WIDTH: usize = 1564;
const ALIAS_WIDTH: usize = 98;
const NAME_LIMIT: usize = 64;
pub(super) const TABLE_COUNT: usize = 70;
pub(super) const MAX_UTF8_EXPANSION: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Codec {
    Utf8,
    Latin1,
    Ascii,
    SingleByte(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Resolution {
    Supported(Codec),
    UnsupportedText,
    NonText,
    AlwaysFails,
    Unresolved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Cancelled;

fn record(key: &[u8], alias_only: bool) -> Option<&'static [u8]> {
    let mut low = 0;
    let mut high = ALIASES.len() / ALIAS_WIDTH;
    while low < high {
        let mid = low + (high - low) / 2;
        let row = &ALIASES[mid * ALIAS_WIDTH..(mid + 1) * ALIAS_WIDTH];
        let length = row[..NAME_LIMIT]
            .iter()
            .position(|&byte| byte == b' ')
            .unwrap_or(NAME_LIMIT);
        match key.cmp(&row[..length]) {
            std::cmp::Ordering::Less => high = mid,
            std::cmp::Ordering::Greater => low = mid + 1,
            std::cmp::Ordering::Equal => {
                return (!alias_only || row[96] == b'1').then_some(row);
            }
        }
    }
    None
}

/// Registry spelling normalization, NOT the tokenizer's cookie normal_name.
/// Names beyond this finite profile remain unresolved, never fake syntax errors.
pub(super) fn lookup(
    name: &[u8],
    check: &mut dyn FnMut() -> bool,
) -> Result<Resolution, Cancelled> {
    if check() {
        return Err(Cancelled);
    }
    if name.len() > NAME_LIMIT || !name.is_ascii() || name.contains(&0) {
        return Ok(Resolution::Unresolved);
    }
    let mut normalized = [0; NAME_LIMIT];
    let mut length = 0;
    let mut punctuation = false;
    for &byte in name {
        if byte.is_ascii_alphanumeric() || byte == b'.' {
            if punctuation && length != 0 {
                normalized[length] = b'_';
                length += 1;
            }
            normalized[length] = byte.to_ascii_lowercase();
            length += 1;
            punctuation = false;
        } else {
            punctuation = true;
        }
    }
    let mut found = record(&normalized[..length], false);
    if found.is_none() && normalized[..length].contains(&b'.') {
        for byte in &mut normalized[..length] {
            if *byte == b'.' {
                *byte = b'_';
            }
        }
        // CPython's dot fallback is an alias lookup, not a module lookup.
        found = record(&normalized[..length], true);
    }
    let Some(row) = found else {
        return Ok(Resolution::Unresolved);
    };
    Ok(match row[90] {
        b'U' => Resolution::Supported(Codec::Utf8),
        b'L' => Resolution::Supported(Codec::Latin1),
        b'A' => Resolution::Supported(Codec::Ascii),
        b'S' => {
            let index = row[92..95]
                .iter()
                .fold(0, |value, byte| value * 10 + usize::from(byte - b'0'));
            Resolution::Supported(Codec::SingleByte(index))
        }
        b'T' => Resolution::UnsupportedText,
        b'N' => Resolution::NonText,
        b'F' => Resolution::AlwaysFails,
        _ => Resolution::Unresolved,
    })
}

/// Stable IDs are all-table row positions; excluded tables return None always.
pub(super) fn decode_point(index: usize, byte: u8) -> Option<char> {
    if index >= TABLE_COUNT {
        return None;
    }
    let row = &TABLES[index * TABLE_WIDTH..(index + 1) * TABLE_WIDTH];
    if row.as_bytes()[25] != b'1' {
        return None;
    }
    let offset = 27 + usize::from(byte) * 6;
    let point = u32::from_str_radix(&row[offset..offset + 6], 16).ok()?;
    if point == 0xFFFE {
        return None;
    }
    let decoded = char::from_u32(point)?;
    debug_assert!(decoded.len_utf8() <= MAX_UTF8_EXPANSION);
    Some(decoded)
}

#[cfg(test)]
pub(super) fn name(index: usize) -> Option<&'static str> {
    if index >= TABLE_COUNT {
        return None;
    }
    Some(TABLES[index * TABLE_WIDTH..index * TABLE_WIDTH + 24].trim_end())
}

pub(super) fn identity_inputs() -> &'static [(&'static str, &'static [u8])] {
    &[
        ("codecs312.rs", include_bytes!("codecs312.rs")),
        (
            "codecs312/tables.tsv",
            include_bytes!("codecs312/tables.tsv"),
        ),
        (
            "codecs312/aliases.tsv",
            include_bytes!("codecs312/aliases.tsv"),
        ),
        (
            "codecs312/manifest.json",
            include_bytes!("codecs312/manifest.json"),
        ),
        (
            "codecs312/PROVENANCE.md",
            include_bytes!("codecs312/PROVENANCE.md"),
        ),
        (
            "codecs312/NOTICES.md",
            include_bytes!("codecs312/NOTICES.md"),
        ),
        (
            "codecs312/LICENSE.txt",
            include_bytes!("codecs312/LICENSE.txt"),
        ),
        (
            "generate_python_codecs312.py",
            include_bytes!("../../../scripts/generate_python_codecs312.py"),
        ),
        (
            "check_python_codecs312.py",
            include_bytes!("../../../scripts/check_python_codecs312.py"),
        ),
        (
            "test_python_codecs312.py",
            include_bytes!("../../../scripts/test_python_codecs312.py"),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(name: &[u8]) -> Resolution {
        lookup(name, &mut || false).unwrap()
    }

    #[test]
    fn finite_registry_normalization_and_classification() {
        for name in [
            b"UTF-8".as_slice(),
            b"  utf@@8  ",
            b"u8",
            b"utf_8_sig",
            b"UTF--8_sig",
            b"UTF__8_sig",
            b"utf---8---sig",
        ] {
            assert_eq!(resolve(name), Resolution::Supported(Codec::Utf8));
        }
        assert_eq!(resolve(b"latin1"), Resolution::Supported(Codec::Latin1));
        assert_eq!(resolve(b"us-ascii"), Resolution::Supported(Codec::Ascii));
        assert_eq!(resolve(b"base64"), Resolution::NonText);
        assert_eq!(resolve(b"rot13"), Resolution::NonText);
        assert_eq!(resolve(b"undefined"), Resolution::AlwaysFails);
        for name in [b"cp864".as_slice(), b"cp037", b"utf-16"] {
            assert_eq!(resolve(name), Resolution::UnsupportedText);
        }
        for name in [
            b"".as_slice(),
            b"unknown",
            b"mbcs",
            b"oem",
            &[b'a'; 65],
            b"cp.1252",
            b"utf.8",
            b"cshproman8",
            b"ascii\0",
            b"\xff",
        ] {
            assert_eq!(resolve(name), Resolution::Unresolved);
        }
        assert_eq!(lookup(b"cp1252", &mut || true), Err(Cancelled));
    }

    #[test]
    fn every_profile_name_and_alias_spelling() {
        for row in ALIASES.chunks_exact(ALIAS_WIDTH) {
            let end = row[..64]
                .iter()
                .position(|&byte| byte == b' ')
                .unwrap_or(64);
            let key = &row[..end];
            if key.iter().any(u8::is_ascii_uppercase) {
                // Preserve the pinned stdlib typo without inventing an alias.
                assert_eq!(key, b"csHPRoman8");
                assert_eq!(resolve(key), Resolution::Unresolved);
                assert_eq!(resolve(b"cshproman8"), Resolution::Unresolved);
                continue;
            }
            let index = std::str::from_utf8(&row[92..95])
                .unwrap()
                .parse::<usize>()
                .unwrap();
            let expected = match row[90] {
                b'U' => Resolution::Supported(Codec::Utf8),
                b'L' => Resolution::Supported(Codec::Latin1),
                b'A' => Resolution::Supported(Codec::Ascii),
                b'S' => Resolution::Supported(Codec::SingleByte(index)),
                b'T' => Resolution::UnsupportedText,
                b'N' => Resolution::NonText,
                b'F' => Resolution::AlwaysFails,
                b'R' => Resolution::Unresolved,
                _ => panic!("invalid frozen status"),
            };
            assert_eq!(resolve(key), expected);
            let mut spelling = key.to_ascii_uppercase();
            for byte in &mut spelling {
                if *byte == b'_' {
                    *byte = b'-';
                }
            }
            assert_eq!(resolve(&spelling), expected);
            if row[96] == b'1' && !key.contains(&b'.') {
                for byte in &mut spelling {
                    if *byte == b'-' {
                        *byte = b'.';
                    }
                }
                assert_eq!(resolve(&spelling), expected);
            }
        }
    }

    #[test]
    fn exhaustive_static_table_domain_and_admission() {
        assert_eq!(TABLES.len(), TABLE_COUNT * TABLE_WIDTH);
        assert_eq!(ALIASES.len() % ALIAS_WIDTH, 0);
        let mut identities = 0;
        for index in 0..TABLE_COUNT {
            let admitted = TABLES.as_bytes()[index * TABLE_WIDTH + 25] == b'1';
            identities += usize::from(admitted);
            for byte in 0..=255 {
                let point = decode_point(index, byte);
                let offset = index * TABLE_WIDTH + 27 + usize::from(byte) * 6;
                let raw = u32::from_str_radix(&TABLES[offset..offset + 6], 16).unwrap();
                let expected = if admitted && raw != 0xFFFE {
                    char::from_u32(raw)
                } else {
                    None
                };
                assert_eq!(point, expected);
                if !admitted {
                    assert_eq!(point, None);
                } else if byte < 128 {
                    assert_eq!(point, Some(char::from(byte)));
                }
                if let Some(point) = point {
                    assert!(point.len_utf8() <= MAX_UTF8_EXPANSION);
                }
            }
            assert!(name(index).is_some());
        }
        assert_eq!(identities, 62);
        assert_eq!(decode_point(usize::MAX, 0), None);
        assert_eq!(name(TABLE_COUNT), None);
        let Resolution::Supported(Codec::SingleByte(index)) = resolve(b"cp1252") else {
            panic!("cp1252 must be admitted");
        };
        assert_eq!(decode_point(index, 0x80), Some('\u{20ac}'));
        assert_eq!(decode_point(index, 0x81), None);
    }

    #[test]
    #[ignore = "requires explicit absolute trusted CPython 3.12.11 / Unicode 15"]
    fn trusted_profile_source_audit_and_exhaustive_byte_oracle() {
        let python = std::path::PathBuf::from(
            std::env::var_os("IDEOMANCER_TEST_PYTHON").expect("select trusted CPython explicitly"),
        );
        assert!(python.is_absolute());
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        for script in ["generate_python_codecs312.py", "test_python_codecs312.py"] {
            let mut command = std::process::Command::new(&python);
            command
                .args(["-I", "-B"])
                .arg(root.join("scripts").join(script));
            if script.starts_with("generate") {
                command.arg("--check");
            }
            let output = command.output().expect("trusted Python oracle");
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}
