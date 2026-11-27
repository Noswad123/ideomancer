//! Pinned CPython 3.12 / Unicode 15 raw identifiers and single-point names.
//! Whole-string NFKC is deliberately left to Ruff after raw admission.

use crate::analysis::python_scalars::CodePoint;
use std::cmp::Ordering;

const START: &str = include_str!("unicode15/start.tsv");
const CONTINUE: &str = include_str!("unicode15/continue.tsv");
const CJK: &str = include_str!("unicode15/cjk.tsv");
const NAMES: &str = include_str!("unicode15/names.tsv");
const MAX_RECORD: usize = 95;
const L: [&str; 19] = [
    "G", "GG", "N", "D", "DD", "R", "M", "B", "BB", "S", "SS", "", "J", "JJ", "C", "K", "T", "P",
    "H",
];
const V: [&str; 21] = [
    "A", "AE", "YA", "YAE", "EO", "E", "YEO", "YE", "O", "WA", "WAE", "OE", "YO", "U", "WEO", "WE",
    "WI", "YU", "EU", "YI", "I",
];
const T: [&str; 28] = [
    "", "G", "GG", "GS", "N", "NJ", "NH", "D", "L", "LG", "LM", "LB", "LS", "LT", "LP", "LH", "M",
    "B", "BS", "S", "SS", "NG", "J", "C", "K", "T", "P", "H",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Cancelled;

fn contains(table: &str, point: u32) -> bool {
    let mut low = 0;
    let mut high = table.len() / 14;
    while low < high {
        let mid = low + (high - low) / 2;
        let record = &table[mid * 14..(mid + 1) * 14];
        let first = u32::from_str_radix(&record[..6], 16).expect("checked Unicode interval");
        let last = u32::from_str_radix(&record[7..13], 16).expect("checked Unicode interval");
        if point < first {
            high = mid;
        } else if point > last {
            low = mid + 1;
        } else {
            return true;
        }
    }
    false
}

pub(super) fn is_start_15(point: u32) -> bool {
    contains(START, point)
}

pub(super) fn is_continue_15(point: u32) -> bool {
    contains(CONTINUE, point)
}

pub(super) fn valid_identifier(
    raw: &str,
    check: &mut dyn FnMut() -> bool,
) -> Result<bool, Cancelled> {
    if check() {
        return Err(Cancelled);
    }
    let mut points = raw.chars();
    let Some(first) = points.next() else {
        return Ok(false);
    };
    if !is_start_15(first as u32) {
        return Ok(false);
    }
    for point in points {
        if check() {
            return Err(Cancelled);
        }
        if !is_continue_15(point as u32) {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn lookup_name_15(
    name: &str,
    check: &mut dyn FnMut() -> bool,
) -> Result<Option<CodePoint>, Cancelled> {
    if check() {
        return Err(Cancelled);
    }
    if name.is_empty() || name.len() > MAX_RECORD {
        return Ok(None);
    }
    // CPython checks these two algorithmic prefixes before case folding.
    if let Some(hex) = name.strip_prefix("CJK UNIFIED IDEOGRAPH-") {
        if (4..=5).contains(&hex.len())
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'A'..=b'F').contains(&byte))
        {
            let point = u32::from_str_radix(hex, 16).expect("bounded hexadecimal name");
            if contains(CJK, point) {
                return Ok(CodePoint::new(point).ok());
            }
        }
        return Ok(None);
    }
    if let Some(suffix) = name.strip_prefix("HANGUL SYLLABLE ") {
        for (l, initial) in L.iter().enumerate() {
            if check() {
                return Err(Cancelled);
            }
            let Some(rest) = suffix.strip_prefix(*initial) else {
                continue;
            };
            for (v, vowel) in V.iter().enumerate() {
                if let Some(tail) = rest.strip_prefix(*vowel) {
                    if let Some(t) = T.iter().position(|&ending| ending == tail) {
                        let point = 0xac00 + ((l * 21 + v) * 28 + t) as u32;
                        return Ok(CodePoint::new(point).ok());
                    }
                }
            }
        }
        return Ok(None);
    }
    let mut folded = [0u8; MAX_RECORD];
    for (index, byte) in name.bytes().enumerate() {
        if check() {
            return Err(Cancelled);
        }
        if !byte.is_ascii() {
            return Ok(None);
        }
        folded[index] = byte.to_ascii_uppercase();
    }
    let key = &folded[..name.len()];
    let bytes = NAMES.as_bytes();
    let mut low = 0;
    let mut high = bytes.len();
    while low < high {
        if check() {
            return Err(Cancelled);
        }
        let mid = low + (high - low) / 2;
        // Every checked-in record is at most MAX_RECORD bytes. Never scan a
        // whole table or allocate an index just to locate a binary-search row.
        let back = mid.saturating_sub(MAX_RECORD).max(low);
        let start = bytes[back..mid]
            .iter()
            .rposition(|&byte| byte == b'\n')
            .map_or(back, |offset| back + offset + 1);
        let end = start
            + bytes[start..(start + MAX_RECORD).min(bytes.len())]
                .iter()
                .position(|&byte| byte == b'\n')
                .expect("checked bounded Unicode name record");
        let row = &NAMES[start..end];
        let (candidate, point) = row.split_once('\t').expect("checked Unicode name row");
        match candidate.as_bytes().cmp(key) {
            Ordering::Less => low = end + 1,
            Ordering::Greater => high = start,
            Ordering::Equal => {
                let point = u32::from_str_radix(point, 16).expect("checked Unicode name point");
                return Ok(CodePoint::new(point).ok());
            }
        }
    }
    Ok(None)
}

/// The caller hashes names and bytes with framing, alongside its other inputs.
pub(super) fn identity_inputs() -> &'static [(&'static str, &'static [u8])] {
    static INPUTS: &[(&str, &[u8])] = &[
        ("unicode15.rs", include_bytes!("unicode15.rs")),
        ("unicode15/start.tsv", START.as_bytes()),
        ("unicode15/continue.tsv", CONTINUE.as_bytes()),
        ("unicode15/cjk.tsv", CJK.as_bytes()),
        ("unicode15/names.tsv", NAMES.as_bytes()),
        (
            "unicode15/manifest.json",
            include_bytes!("unicode15/manifest.json"),
        ),
        (
            "unicode15/PROVENANCE.md",
            include_bytes!("unicode15/PROVENANCE.md"),
        ),
        (
            "unicode15/inputs/UnicodeData.txt",
            include_bytes!("unicode15/inputs/UnicodeData.txt"),
        ),
        (
            "unicode15/inputs/NameAliases.txt",
            include_bytes!("unicode15/inputs/NameAliases.txt"),
        ),
        (
            "unicode15/inputs/DerivedCoreProperties.txt",
            include_bytes!("unicode15/inputs/DerivedCoreProperties.txt"),
        ),
        (
            "unicode15/inputs/ReadMe.txt",
            include_bytes!("unicode15/inputs/ReadMe.txt"),
        ),
        (
            "unicode15/inputs/LICENSE.txt",
            include_bytes!("unicode15/inputs/LICENSE.txt"),
        ),
        (
            "scripts/generate_python_unicode15.py",
            include_bytes!("../../../scripts/generate_python_unicode15.py"),
        ),
        (
            "scripts/check_python_unicode15.py",
            include_bytes!("../../../scripts/check_python_unicode15.py"),
        ),
        (
            "scripts/test_python_unicode15.py",
            include_bytes!("../../../scripts/test_python_unicode15.py"),
        ),
    ];
    INPUTS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup(name: &str) -> Option<u32> {
        lookup_name_15(name, &mut || false)
            .unwrap()
            .map(CodePoint::value)
    }

    #[test]
    fn raw_identifiers_and_cancellation_are_versioned() {
        for raw in ["_", "name", "\u{e9}", "A\u{301}", "\u{fb03}", "\u{31350}"] {
            assert_eq!(valid_identifier(raw, &mut || false), Ok(true), "{raw}");
        }
        for raw in ["", "0name", "\u{301}A", "\u{200c}", "\u{105c0}", "\u{1c89}"] {
            assert_eq!(valid_identifier(raw, &mut || false), Ok(false), "{raw}");
        }
        for point in [0xd800, 0xdfff, 0x110000, u32::MAX] {
            assert!(!is_start_15(point));
            assert!(!is_continue_15(point));
        }
        assert_eq!(valid_identifier("identifier", &mut || true), Err(Cancelled));
        assert_eq!(lookup_name_15("SPACE", &mut || true), Err(Cancelled));
        let mut checks = 0;
        assert_eq!(
            lookup_name_15("LATIN CAPITAL LETTER A", &mut || {
                checks += 1;
                checks == 5
            }),
            Err(Cancelled)
        );
    }

    #[test]
    fn explicit_names_aliases_and_algorithmic_boundaries() {
        for (name, point) in [
            ("LATIN CAPITAL LETTER A", 65),
            ("latin capital letter a", 65),
            ("null", 0),
            ("BYTE ORDER MARK", 0xfeff),
            ("CJK UNIFIED IDEOGRAPH-04E00", 0x4e00),
            ("CJK UNIFIED IDEOGRAPH-323AF", 0x323af),
            ("HANGUL SYLLABLE GA", 0xac00),
            ("HANGUL SYLLABLE HIH", 0xd7a3),
            ("nushu character-1b170", 0x1b170),
        ] {
            assert_eq!(lookup(name), Some(point), "{name}");
        }
        for name in [
            "",
            "KEYCAP DIGIT ONE",
            "TODHRI LETTER A",
            "TANGUT IDEOGRAPH-17000",
            "CUNEIFORM SIGN KALAM",
            "CJK UNIFIED IDEOGRAPH-004E00",
            "CJK UNIFIED IDEOGRAPH-4e00",
            "cjk unified ideograph-4E00",
            "CJK UNIFIED IDEOGRAPH-323B0",
            "CJK UNIFIED IDEOGRAPH-2EBF0",
            "HANGUL SYLLABLE ga",
            "hangul syllable GA",
            "LATIN CAPITAL LETTER A\0",
            "SPACE ",
        ] {
            assert_eq!(lookup(name), None, "{name}");
        }
        assert_eq!(lookup(&"X".repeat(MAX_RECORD + 1)), None);
    }

    #[test]
    fn all_explicit_rows_and_fixed_interval_invariants() {
        let mut previous = "";
        for row in NAMES.lines() {
            assert!(row.len() < MAX_RECORD);
            let (name, point) = row.split_once('\t').unwrap();
            assert!(previous < name);
            previous = name;
            let expected = Some(u32::from_str_radix(point, 16).unwrap());
            assert_eq!(lookup(name), expected, "{name}");
            assert_eq!(lookup(&name.to_ascii_lowercase()), expected, "{name}");
        }
        for table in [START, CONTINUE, CJK] {
            assert_eq!(table.len() % 14, 0);
            let mut previous = None;
            for row in table.lines() {
                assert_eq!(row.len(), 13);
                let first = u32::from_str_radix(&row[..6], 16).unwrap();
                let last = u32::from_str_radix(&row[7..], 16).unwrap();
                assert!(first <= last && last <= 0x10ffff);
                assert!(previous.is_none_or(|point| point < first));
                previous = Some(last);
            }
        }
    }

    #[cfg(unix)]
    #[test]
    #[ignore = "requires explicit absolute trusted CPython 3.12.11 / Unicode 15"]
    fn shared_data_matches_exhaustive_trusted_stdlib() {
        use sha2::{Digest, Sha256};
        let python = std::path::PathBuf::from(
            std::env::var_os("IDEOMANCER_TEST_PYTHON").expect("select trusted Python 3.12.11"),
        );
        assert!(python.is_absolute() && python.is_file());
        let mut names = Vec::new();
        // Feed the oracle independently sourced official spellings, not just
        // the export table that the implementation under test consumes.
        for text in [
            include_str!("unicode15/inputs/UnicodeData.txt"),
            include_str!("unicode15/inputs/NameAliases.txt"),
        ] {
            for row in text.lines() {
                let row = row.split('#').next().unwrap().trim();
                let mut fields = row.split(';');
                let _point = fields.next();
                if let Some(name) = fields.next().map(str::trim) {
                    if !name.starts_with('<') {
                        names.push(name.to_owned());
                        names.push(name.to_ascii_lowercase());
                    }
                }
            }
        }
        for row in CJK.lines() {
            let first = u32::from_str_radix(&row[..6], 16).unwrap();
            let last = u32::from_str_radix(&row[7..], 16).unwrap();
            for point in first..=last {
                names.push(format!("CJK UNIFIED IDEOGRAPH-{point:X}"));
            }
            for point in [first - 1, first, last, last + 1] {
                for hex in [
                    format!("{point:X}"),
                    format!("{point:05X}"),
                    format!("{point:06X}"),
                    format!("{point:x}"),
                ] {
                    let name = format!("CJK UNIFIED IDEOGRAPH-{hex}");
                    names.push(name.to_ascii_lowercase());
                    names.push(name);
                }
            }
        }
        for initial in L {
            for vowel in V {
                for ending in T {
                    let name = format!("HANGUL SYLLABLE {initial}{vowel}{ending}");
                    names.push(name.to_ascii_lowercase());
                    names.push(name);
                }
            }
        }
        for name in [
            "",
            "NOT A UNICODE NAME",
            "KEYCAP DIGIT ONE",
            "LATIN CAPITAL LETTER A\0",
            " LATIN CAPITAL LETTER A",
            "LATIN CAPITAL LETTER A ",
            "LATIN  CAPITAL LETTER A",
            "TANGUT IDEOGRAPH-17000",
            "TANGUT IDEOGRAPH-18D00",
            "TODHRI LETTER A",
            "CUNEIFORM SIGN KALAM",
            "CJK UNIFIED IDEOGRAPH-2EBF0",
            "CJK UNIFIED IDEOGRAPH-323B0",
            "HANGUL SYLLABLE GG",
            "HANGUL SYLLABLE ga",
            "HANGUL SYLLABLE HIHH",
        ] {
            names.push(name.to_owned());
        }
        let identifiers = [
            "\u{fb03}",
            "\u{212a}",
            "\u{ff21}\u{ff22}",
            "\u{1d400}\u{1d401}",
            "A\u{30a}",
            "A\u{30a}\u{301}",
            "\u{1100}\u{1161}\u{11a8}",
            "a\u{315}\u{300}",
            "\u{3b1}\u{313}\u{301}",
            "\u{212b}\u{301}",
        ];
        let script = r#"
import ast, hashlib, json, sys, unicodedata as u
assert sys.implementation.name == 'cpython' and sys.version_info[:3] == (3, 12, 11), sys.version
assert u.unidata_version == '15.0.0', u.unidata_version
request = json.load(sys.stdin)
flags = hashlib.sha256()
start = continuation = 0
for cp in range(0x110000):
    a, b = chr(cp).isidentifier(), ('A' + chr(cp)).isidentifier()
    start += a
    continuation += b
    flags.update(bytes((a | (b << 1),)))
names = hashlib.sha256()
for name in request['names']:
    try:
        text = u.lookup(name)
        cp = ord(text) if len(text) == 1 else 0xffffffff
    except KeyError:
        cp = 0xffffffff
    names.update(cp.to_bytes(4, 'little'))
normalized = []
for raw in request['identifiers']:
    assert raw.isidentifier()
    actual = ast.parse(raw, mode='eval').body.id
    assert actual == u.normalize('NFKC', raw)
    normalized.append(actual)
json.dump(dict(membership=flags.hexdigest(), start=start, continuation=continuation,
               names=names.hexdigest(), normalized=normalized), sys.stdout, ensure_ascii=True)
"#;
        let mut command = std::process::Command::new(python);
        command
            .args(["-I", "-S", "-c", script])
            .current_dir("/")
            .env_clear()
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let input =
            serde_json::to_vec(&serde_json::json!({"names": names, "identifiers": identifiers}))
                .unwrap();
        let output =
            crate::analysis::run_bounded(command, input, "Unicode 15 shared data").unwrap();
        let oracle: serde_json::Value = serde_json::from_slice(&output).unwrap();
        let mut membership = Sha256::new();
        let mut start = 0usize;
        let mut continuation = 0usize;
        for point in 0..0x110000 {
            let a = is_start_15(point);
            let b = is_continue_15(point);
            start += usize::from(a);
            continuation += usize::from(b);
            membership.update([u8::from(a) | (u8::from(b) << 1)]);
        }
        assert_eq!(oracle["start"], start);
        assert_eq!(oracle["continuation"], continuation);
        assert_eq!(oracle["membership"], format!("{:x}", membership.finalize()));
        let mut actual = Sha256::new();
        for name in &names {
            actual.update(lookup(name).unwrap_or(u32::MAX).to_le_bytes());
        }
        assert_eq!(oracle["names"], format!("{:x}", actual.finalize()));
        for (index, raw) in identifiers.into_iter().enumerate() {
            assert_eq!(valid_identifier(raw, &mut || false), Ok(true));
            // A small sequence gate through the real frontend, not per-char
            // NFKC concatenation and not a large repeated parser workload.
            let parsed = ruff_python_parser::parse_expression(raw).unwrap();
            let ruff_python_ast::Expr::Name(name) = &*parsed.syntax().body else {
                panic!("expected normalized identifier");
            };
            assert_eq!(
                name.id.as_str(),
                oracle["normalized"][index].as_str().unwrap()
            );
        }
    }
}
