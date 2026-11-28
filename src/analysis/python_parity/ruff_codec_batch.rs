//! Finite codec-profile gates. These do not claim general registry/UTF-8 parity.
use super::ruff_facts_batch::source as input;
use super::*;
use crate::analysis::python_ruff::codecs312::{self, Codec, Resolution};
use failures::{Exception, Failure};
use serde_json::{json, Value};
use std::borrow::Cow;

const DECODED_FILE: usize = 2 * 1024 * 1024;
const DECODED_RUN: usize = 32 * 1024 * 1024;

fn resolve(name: &str) -> Resolution {
    codecs312::lookup(name.as_bytes(), &mut || false).unwrap()
}

fn admitted() -> Vec<(usize, &'static str)> {
    let names: Vec<_> = (0..70)
        .filter_map(|id| {
            let name = codecs312::name(id).unwrap();
            matches!(resolve(name), Resolution::Supported(_)).then_some((id, name))
        })
        .collect();
    assert_eq!(names.len(), 62);
    assert_eq!(codecs312::name(70), None);
    names
}

fn spellings(name: &str) -> Vec<&'static str> {
    let codec = resolve(name);
    include_str!("../python_ruff/codecs312/aliases.tsv")
        .lines()
        .filter_map(|row| {
            let spelling = row.split('\t').next().unwrap().trim();
            (resolve(spelling) == codec).then_some(spelling)
        })
        .collect()
}

fn encoded(name: &str, body: &[u8]) -> Vec<u8> {
    let mut bytes = format!("# coding: {name}\r\n").into_bytes();
    bytes.extend_from_slice(body);
    bytes
}

fn decode(bytes: &[u8]) -> std::result::Result<source::DecodedSource<'_>, source::Error> {
    source::decode(bytes, DECODED_FILE, &mut |_| Ok(()), &mut || false)
}

fn encoding(name: &str) -> source::Encoding {
    match resolve(name) {
        Resolution::Supported(Codec::Latin1) => source::Encoding::Latin1,
        Resolution::Supported(Codec::Ascii) => source::Encoding::Ascii,
        Resolution::Supported(Codec::Utf8) => source::Encoding::Utf8,
        Resolution::Supported(Codec::SingleByte(id)) => source::Encoding::SingleByte(id),
        other => panic!("not a supported codec: {other:?}"),
    }
}

#[test]
fn every_canonical_and_alias_decodes_all_bytes_and_mixed_streams_exactly() {
    let mut undefined_tables = 0;
    for (id, name) in admitted() {
        undefined_tables +=
            usize::from((0..=255).any(|b| codecs312::decode_point(id, b).is_none()));
        let aliases = spellings(name);
        assert!(aliases.contains(&name), "missing canonical {name}");
        for alias in aliases {
            for byte in 0..=255 {
                let bytes = encoded(alias, &[byte]);
                let header = format!("# coding: {alias}\r\n");
                let mut charged = Vec::new();
                let actual = source::decode(
                    &bytes,
                    DECODED_FILE,
                    &mut |size| {
                        charged.push(size);
                        Ok(())
                    },
                    &mut || false,
                );
                match codecs312::decode_point(id, byte) {
                    Some(point) => {
                        let view = actual.unwrap();
                        let expected = format!("{header}{point}");
                        assert_eq!(view.text, expected, "{alias} {byte:02x}");
                        assert_eq!(charged, [expected.len()]);
                        assert_eq!(view.encoding, encoding(alias));
                        assert_eq!(view.text.len(), expected.len());
                        if byte >= 128 {
                            assert!(matches!(view.text, Cow::Owned(_)));
                            assert_eq!(view.allocation_bytes, expected.len());
                        } else {
                            // Borrowing is optional for Latin-1, mandatory for the new family.
                            if view.encoding != source::Encoding::Latin1 {
                                assert!(matches!(view.text, Cow::Borrowed(_)));
                                assert_eq!(view.text.as_ptr(), bytes.as_ptr());
                                assert_eq!(view.allocation_bytes, 0);
                            }
                            assert_eq!(view.text.as_bytes(), bytes);
                        }
                    }
                    None => {
                        assert_eq!(
                            actual.unwrap_err(),
                            source::Error::Syntax {
                                line: 0,
                                reason: source::CODEC_DECODE,
                            }
                        );
                        assert!(charged.is_empty(), "undefined bytes must precede reserve");
                    }
                }
            }
            let body: Vec<_> = (0..=255)
                .chain((0..=255).rev())
                .filter(|&b| codecs312::decode_point(id, b).is_some())
                .collect();
            let bytes = encoded(alias, &body);
            let original = bytes.clone();
            let expected: String = format!("# coding: {alias}\r\n")
                .chars()
                .chain(
                    body.iter()
                        .map(|&b| codecs312::decode_point(id, b).unwrap()),
                )
                .collect();
            let view = decode(&bytes).unwrap();
            assert_eq!(view.text, expected, "mixed stream {alias}");
            assert_eq!(view.allocation_bytes, expected.len());
            assert_eq!(bytes, original);
            assert!(
                view.text.contains('\0'),
                "decoder preserves NUL for callers"
            );
            assert!(view.text.contains("\r\n"), "no newline rewrite");
        }
    }
    assert_eq!(
        undefined_tables, 21,
        "the other two undefined tables are excluded"
    );
    for alias in spellings("ascii") {
        for byte in 0..=255 {
            let bytes = encoded(alias, &[byte]);
            if byte < 128 {
                let view = decode(&bytes).unwrap();
                assert_eq!(view.text.as_bytes(), bytes);
                assert!(matches!(view.text, Cow::Borrowed(_)));
                assert_eq!(view.allocation_bytes, 0);
            } else {
                assert_eq!(
                    decode(&bytes).unwrap_err(),
                    source::Error::Syntax {
                        line: 0,
                        reason: source::CODEC_DECODE,
                    }
                );
            }
        }
    }
}

#[test]
fn three_byte_expansion_reserves_once_and_cancels_before_and_after_charge() {
    let payload = vec![0x80; 2048];
    let bytes = encoded("cp1252", &payload);
    let needed = bytes.len() + 4096;
    let mut charges = Vec::new();
    let view = source::decode(
        &bytes,
        needed,
        &mut |n| {
            charges.push(n);
            Ok(())
        },
        &mut || false,
    )
    .unwrap();
    assert_eq!(charges, [needed]);
    assert_eq!((view.text.len(), view.allocation_bytes), (needed, needed));
    assert!(view.text.ends_with(&"\u{20ac}".repeat(2048)));
    drop(view);
    assert_eq!(
        source::decode(
            &bytes,
            needed - 1,
            &mut |_| panic!("over-limit charge"),
            &mut || false
        )
        .unwrap_err(),
        source::Error::ResourceLimit
    );
    let mut attempts = 0;
    assert_eq!(
        source::decode(
            &bytes,
            needed,
            &mut |_| {
                attempts += 1;
                Err(source::Error::ResourceLimit)
            },
            &mut || false
        )
        .unwrap_err(),
        source::Error::ResourceLimit
    );
    assert_eq!(attempts, 1);
    assert_eq!(
        source::decode(
            &bytes,
            needed,
            &mut |_| panic!("cancelled charge"),
            &mut || true
        )
        .unwrap_err(),
        source::Error::Cancelled
    );
    let charged = std::cell::Cell::new(0);
    for stop in [1, 4] {
        let mut after = 0;
        assert_eq!(
            source::decode(
                &bytes,
                needed,
                &mut |n| {
                    assert_eq!(charged.replace(n), 0);
                    Ok(())
                },
                &mut || {
                    if charged.get() != 0 {
                        after += 1;
                    }
                    after == stop
                }
            )
            .unwrap_err(),
            source::Error::Cancelled
        );
        assert_eq!(
            charged.replace(0),
            needed,
            "caller owns refund after cancellation"
        );
    }
    let mut total = 0;
    for _ in 0..2 {
        let view = source::decode(
            &bytes,
            needed,
            &mut |n| {
                total += n;
                Ok(())
            },
            &mut || false,
        )
        .unwrap();
        assert_eq!(view.text.len(), needed);
    }
    let before = total;
    assert_eq!(
        source::decode(
            &bytes,
            needed,
            &mut |n| {
                if total + n > 2 * needed {
                    return Err(source::Error::ResourceLimit);
                }
                total += n;
                Ok(())
            },
            &mut || false
        )
        .unwrap_err(),
        source::Error::ResourceLimit
    );
    assert_eq!(total, before);
    let payload = vec![0x80; 700_000];
    let oversized = encoded("cp1252", &payload);
    assert!(oversized.len() < 1024 * 1024);
    assert_eq!(
        source::decode(
            &oversized,
            DECODED_FILE,
            &mut |_| panic!("decoded per-file limit must precede charge"),
            &mut || false
        )
        .unwrap_err(),
        source::Error::ResourceLimit
    );
}

fn negatives() -> Vec<Source> {
    let mut cases = Vec::new();
    for alias in spellings("ascii") {
        cases.push(input(encoded(alias, b"eval()\nvalue='\xff'\n")));
    }
    for (id, name) in admitted() {
        if let Some(byte) = (128..=255).find(|&b| codecs312::decode_point(id, b).is_none()) {
            for alias in
                std::iter::once(name).chain(spellings(name).into_iter().find(|&a| a != name))
            {
                let mut body = b"eval()\nvalue='".to_vec();
                body.push(byte);
                body.extend_from_slice(b"'\n");
                cases.push(input(encoded(alias, &body)));
            }
        }
    }
    for name in [
        "base64",
        "base64_codec",
        "rot13",
        "rot_13",
        "hex",
        "hex_codec",
        "undefined",
        "u8",
        "utf8",
        "cp65001",
        "utf__8",
        "utf__8_sig",
    ] {
        cases.push(input(encoded(name, b"eval()\nvalue='\xff'\n")));
    }
    for name in [
        "cp1252",
        "ascii",
        "latin1",
        "utf8",
        "utf__8_sig",
        "undefined",
        "base64",
        "not_a_codec",
    ] {
        let mut bytes = b"\xef\xbb\xbf".to_vec();
        bytes.extend(encoded(name, b"eval()\n"));
        cases.push(input(bytes));
    }
    // Raw NUL wins even when the cookie or a later byte would otherwise fail.
    for name in [
        "ascii",
        "cp1252",
        "undefined",
        "base64",
        "not_a_codec",
        "utf-8",
    ] {
        cases.push(input(encoded(name, b"eval()\n\0\xff")));
        cases.push(input(format!("#\0 coding: {name}\neval()\n").as_bytes()));
    }
    assert_eq!(cases.len(), 84);
    cases
}

#[test]
fn closed_source_failures_have_no_callbacks_or_semantic_records() {
    let expected = Failure {
        exception: Exception::SyntaxError,
        line: 0,
    };
    for case in negatives() {
        let inputs = std::slice::from_ref(&case);
        let mut callbacks = 0;
        let parsed = run_consumer(inputs, Limits::default(), |_| false, &mut |_, _, _, _| {
            callbacks += 1
        })
        .unwrap();
        assert_eq!(callbacks, 0);
        assert_eq!(parsed.files[0].failure, Some(expected), "{:?}", case.bytes);
        let mut trace = Vec::new();
        let report = extract_impl(
            inputs,
            |_| false,
            SemanticLimits::default(),
            Some(&mut trace),
        )
        .unwrap();
        let file = &report.files[0];
        assert_eq!(file.status, FactStatus::SyntaxError, "{:?}", case.bytes);
        assert_eq!(file.failure, Some(expected));
        assert_eq!(file.failure_line, Some(0));
        if case.bytes.contains(&0) {
            assert_eq!(file.reason, Some(source::NULL_BYTE));
        }
        assert_eq!((file.nodes, file.edges), (0, 0));
        assert_eq!(file.sha256, crate::analysis::hash(&case.bytes));
        assert_eq!(parsed.files[0].sha256, file.sha256);
        assert_eq!(report.facts.len(), 1);
        assert_eq!(report.facts[0].kind, "module");
        assert!(trace.is_empty() && report.diagnostics.is_empty());
    }
}

#[test]
fn registry_uncertainty_and_physical_utf8_errors_never_gain_certificates() {
    for (name, reason) in [
        ("base64", source::NON_TEXT_CODEC),
        ("undefined", source::UNDEFINED_CODEC),
    ] {
        assert_eq!(
            decode(&encoded(name, b"eval()\n")).unwrap_err(),
            source::Error::Syntax { line: 0, reason }
        );
    }
    for (names, reason) in [
        (
            &["cp.1252", "utf.8", "latin.1", "base64.codec", "not_a_codec"][..],
            "unresolved codec membership",
        ),
        (
            &["cp037", "cp500", "cp864", "utf-16", "shift_jis"][..],
            "unsupported codec",
        ),
    ] {
        for name in names {
            let inputs = [input(encoded(name, b"eval()\n"))];
            let mut callbacks = 0;
            let root = run_consumer(&inputs, Limits::default(), |_| false, &mut |_, _, _, _| {
                callbacks += 1
            })
            .unwrap();
            let report = extract_fact_candidates(&inputs).unwrap();
            assert_eq!(callbacks, 0);
            assert_eq!(report.files[0].status, FactStatus::UnsupportedEncoding);
            assert_eq!(report.files[0].reason, Some(reason));
            assert_eq!(report.files[0].failure, None);
            assert_eq!(root.files[0].failure, None);
        }
    }
    for bytes in [
        b"value='\xff'\n".as_slice(),
        b"# coding: utf-8\nvalue='\xff'\n",
        b"# coding: utf_8_sig\nvalue='\xff'\n",
        b"# invalid comment \xff\n",
    ] {
        let inputs = [input(bytes)];
        let report = extract_fact_candidates(&inputs).unwrap();
        assert_ne!(report.files[0].status, FactStatus::AcceptedFacts);
        assert_eq!(report.files[0].failure, None);
    }
    for (name, expected) in [
        ("windows.1252", "\u{20ac}"),
        ("us.ascii", "a"),
        ("iso.8859.1", "\u{e9}"),
    ] {
        let byte = match name {
            "windows.1252" => 0x80,
            "iso.8859.1" => 0xe9,
            _ => b'a',
        };
        let bytes = encoded(name, &[byte]);
        assert!(decode(&bytes).unwrap().text.ends_with(expected));
    }
    for name in ["utf_8_sig", "utf-8-sig"] {
        assert_eq!(
            decode(&encoded(name, b"eval()\n")).unwrap().encoding,
            source::Encoding::Utf8
        );
    }
    assert_eq!(
        decode(b"\xef\xbb\xbf# coding: utf__8_sig\n").unwrap_err(),
        source::Error::Syntax {
            line: 0,
            reason: source::BOM_CONFLICT
        }
    );
    assert!(decode(b"\xef\xbb\xbf# coding: utf_8_sig\n").is_ok());
}

fn positives() -> Vec<Source> {
    let mut cases = Vec::new();
    for (id, name) in admitted() {
        let byte = (128..=255)
            .find(|&b| {
                codecs312::decode_point(id, b).is_some_and(|p| !p.is_control() && p.is_alphabetic())
            })
            .unwrap();
        for alias in std::iter::once(name).chain(spellings(name).into_iter().find(|&a| a != name)) {
            let mut body = b"@api.get('/".to_vec();
            body.push(byte);
            body.extend_from_slice(b"')\r\ndef route():\r\n    value='");
            body.push(byte);
            body.extend_from_slice(b"'; eval(value)\r\n");
            // Fixed XID anchors, not host Unicode identifier classification.
            if let Some(letter) = (128..=255).find(|&b| {
                matches!(
                    codecs312::decode_point(id, b),
                    Some('\u{e9}' | '\u{3b1}' | '\u{430}')
                )
            }) {
                body.push(letter);
                body.extend_from_slice(b" = factory()\r\n");
            }
            cases.push(input(encoded(alias, &body)));
        }
    }
    assert_eq!(cases.len(), 112);
    cases
}

#[test]
fn cookie_grammar_controls_keep_historical_precedence() {
    for header in [
        "# coding: windows.1252\n",
        "#!/synthetic\r\n# coding=windows.1252\r\n",
        " \t\x0c\r\x0c # decoding: windows.1252\r",
        "# coding: ; coding: windows.1252\n",
        "# coding:\n# coding: windows.1252\n",
    ] {
        let mut bytes = header.as_bytes().to_vec();
        bytes.extend_from_slice(b"value='\x80'\n");
        assert_eq!(
            decode(&bytes).unwrap().text,
            format!("{header}value='\u{20ac}'\n")
        );
    }
    for header in [
        "value=1\n# coding: cp1252\n",
        "'# coding: cp1252'\n",
        "# first\n# second\n# coding: cp1252\n",
        "# Coding: cp1252\n",
        "# coding : cp1252\n",
        "# coding:\x0ccp1252\n",
    ] {
        let mut bytes = header.as_bytes().to_vec();
        bytes.extend_from_slice(b"value='\x80'\n");
        assert!(matches!(
            decode(&bytes),
            Err(source::Error::Syntax {
                reason: source::INVALID_UTF8,
                ..
            })
        ));
    }
    assert_eq!(
        decode(b"# coding: ascii coding: cp1252\nvalue='\x80'\n").unwrap_err(),
        source::Error::Syntax {
            line: 0,
            reason: source::CODEC_DECODE
        }
    );
}

#[test]
fn encoded_positives_use_original_hashes_and_decoded_utf8_spans() {
    for case in positives() {
        let mut callbacks = 0;
        let root = run_consumer(
            std::slice::from_ref(&case),
            Limits::default(),
            |_| false,
            &mut |_, text, _, _| {
                callbacks += 1;
                assert_eq!(text, decode(&case.bytes).unwrap().text);
            },
        )
        .unwrap();
        assert_eq!(callbacks, 1, "{:?}", case.bytes);
        let report = extract_fact_candidates(std::slice::from_ref(&case)).unwrap();
        assert_eq!(report.files[0].status, FactStatus::AcceptedFacts);
        assert_eq!(report.files[0].sha256, crate::analysis::hash(&case.bytes));
        assert_eq!(root.files[0].sha256, report.files[0].sha256);
        let declaration = report.facts.iter().find(|f| f.kind == "function").unwrap();
        assert_eq!((declaration.line, declaration.end_line), (3, 4));
        let view = decode(&case.bytes).unwrap();
        assert_ne!(
            crate::analysis::hash(view.text.as_bytes()),
            report.files[0].sha256
        );
    }
}

#[test]
fn failed_siblings_preserve_complete_valid_records_in_both_orders() {
    let mut good = input(encoded(
        "cp1252",
        b"@api.route(path=unknown, methods=unknown)\ndef route():\n    value='\x80'; eval(value)\n",
    ));
    good.path = "accepted.py".into();
    good.module = "accepted".into();
    let mut expected_trace = Vec::new();
    let expected = extract_impl(
        std::slice::from_ref(&good),
        |_| false,
        SemanticLimits::default(),
        Some(&mut expected_trace),
    )
    .unwrap();
    assert!(!expected_trace.is_empty() && !expected.diagnostics.is_empty());
    for bad in negatives() {
        for reverse in [false, true] {
            let mut inputs = vec![input(&good.bytes), input(&bad.bytes)];
            inputs[0].path.clone_from(&good.path);
            inputs[0].module.clone_from(&good.module);
            if reverse {
                inputs.reverse();
            }
            let mut trace = Vec::new();
            let report = extract_impl(
                &inputs,
                |_| false,
                SemanticLimits::default(),
                Some(&mut trace),
            )
            .unwrap();
            let kept: Vec<_> = report
                .facts
                .iter()
                .filter(|f| f.path == good.path)
                .collect();
            assert_eq!(
                serde_json::to_value(kept).unwrap(),
                serde_json::to_value(&expected.facts).unwrap()
            );
            assert_eq!(
                serde_json::to_value(&report.diagnostics).unwrap(),
                serde_json::to_value(&expected.diagnostics).unwrap()
            );
            assert_eq!(trace, expected_trace);
            assert_eq!(
                report.facts.iter().filter(|f| f.path == "unit.py").count(),
                1
            );
            let mut callbacks = Vec::new();
            run_consumer(
                &inputs,
                Limits::default(),
                |_| false,
                &mut |index, _, _, _| callbacks.push(inputs[index].path.as_str()),
            )
            .unwrap();
            assert_eq!(callbacks, ["accepted.py"]);
        }
    }
}

#[test]
fn decode_checkpoint_rolls_back_every_file_and_source_budget_precedes_parse() {
    assert_eq!(Limits::default().compatibility_working, DECODED_RUN);
    assert_eq!(Limits::default().file_bytes, 1024 * 1024);
    assert_eq!(Limits::default().total_bytes, 16 * 1024 * 1024);
    let bytes = encoded("cp1252", b"value='\x80'\n");
    let case = input(&bytes);
    let mut file = file_report(&case);
    let mut total = DECODED_RUN - bytes.len();
    let before = total;
    let mut cancel = Cancellation {
        expired: false,
        check: |_| false,
    };
    let mut callbacks = 0;
    classify(
        &case.bytes,
        &mut file,
        Limits::default(),
        &mut total,
        &mut cancel,
        &mut |_, _, _| callbacks += 1,
    );
    assert_eq!(callbacks, 0);
    assert_eq!(total, before);
    assert_eq!(file.admission, Admission::ResourceLimit);
    assert_eq!(file.diagnostic, Some(Diagnostic::DecodedLimit));
    for budget in [0, 1] {
        let mut callbacks = 0;
        let root = run_consumer(
            std::slice::from_ref(&case),
            Limits {
                compatibility_working: budget,
                ..Limits::default()
            },
            |_| false,
            &mut |_, _, _, _| callbacks += 1,
        )
        .unwrap();
        assert_eq!(callbacks, 0);
        assert_eq!(root.files[0].admission, Admission::ResourceLimit);
        assert_eq!(root.files[0].failure, None);
    }
    let mut comment = b"# ".to_vec();
    comment.extend(std::iter::repeat_n(0x80, 768));
    comment.extend_from_slice(b"\neval()\n");
    let mut inputs = [input("eval()\n"), input(encoded("cp1252", &comment))];
    inputs[1].path = "second.py".into();
    inputs[1].module = "second".into();
    for reverse in [false, true] {
        if reverse {
            inputs.reverse();
        }
        let mut total = 0;
        extract_impl(
            &inputs,
            |at| {
                total += usize::from(at == Checkpoint::Decode);
                false
            },
            SemanticLimits::default(),
            None,
        )
        .unwrap();
        for stop in 1..=total {
            let mut visits = 0;
            let mut trace = Vec::new();
            let report = extract_impl(
                &inputs,
                |at| {
                    visits += usize::from(at == Checkpoint::Decode);
                    at == Checkpoint::Decode && visits == stop
                },
                SemanticLimits::default(),
                Some(&mut trace),
            )
            .unwrap();
            assert!(report
                .files
                .iter()
                .all(|f| f.status == FactStatus::ResourceLimit && f.failure.is_none()));
            assert_eq!(report.facts.len(), 2);
            assert!(report.facts.iter().all(|f| f.kind == "module"));
            assert!(trace.is_empty() && report.diagnostics.is_empty());
        }
    }
}

#[cfg(unix)]
fn trusted(script: &str, requests: &Value) -> Value {
    let python = std::path::PathBuf::from(
        std::env::var_os("IDEOMANCER_TEST_PYTHON").expect("explicit trusted CPython selection"),
    );
    assert!(python.is_absolute() && python.is_file());
    let script = format!("import sys, unicodedata\nassert sys.version_info[:3] == (3, 12, 11)\nassert unicodedata.unidata_version == '15.0.0'\n{script}");
    let request = serde_json::to_vec(requests).unwrap();
    assert!(request.len() <= 512 * 1024);
    let mut command = std::process::Command::new(python);
    command
        .args(["-I", "-S", "-B", "-c", &script])
        .env_clear()
        .current_dir("/")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let output =
        crate::analysis::run_bounded(command, request, "finite codec batch oracle").unwrap();
    serde_json::from_slice(&output).unwrap()
}

#[cfg(unix)]
#[test]
#[ignore = "requires explicit trusted exact CPython 3.12.11; 15872 table positions plus ASCII and AST byte/text locations"]
fn trusted_every_admitted_decoder_position_and_ast_location() {
    let names = admitted();
    let requests: Vec<_> = names
        .iter()
        .map(|(_, name)| json!(name))
        .chain(std::iter::once(json!("ascii")))
        .collect();
    let observed = trusted(
        r#"
import ast, codecs, json
request = json.loads(sys.stdin.buffer.read(524289))
assert len(request) == 63
results = []
for name in request:
    points = []
    defined = bytearray()
    for byte in range(256):
        try:
            text = bytes([byte]).decode(name, 'strict')
        except UnicodeDecodeError:
            points.append(None)
        else:
            assert len(text) == 1
            points.append(ord(text))
            defined.append(byte)
    mixed = bytes(defined) + bytes(reversed(defined))
    decoder = codecs.getincrementaldecoder(name)('strict')
    assert mixed.decode(name) == ''.join(decoder.decode(bytes([b])) for b in mixed) + decoder.decode(b'', final=True)
    results.append(dict(points=points, mixed=list(map(ord, mixed.decode(name)))))
json.dump(results, sys.stdout)
"#,
        &json!(requests),
    );
    let rows = observed.as_array().unwrap();
    assert_eq!(rows.len(), 63);
    for (row, (id, name)) in rows
        .iter()
        .zip(names.iter().copied().chain(std::iter::once((70, "ascii"))))
    {
        for byte in 0..=255 {
            let expected = if id == 70 {
                (byte < 128).then_some(char::from(byte))
            } else {
                codecs312::decode_point(id, byte)
            };
            assert_eq!(
                row["points"][usize::from(byte)],
                json!(expected.map(u32::from)),
                "{name} {byte:02x}"
            );
        }
        let mixed: Vec<_> = (0..=255u8)
            .chain((0..=255).rev())
            .filter(|&b| !row["points"][usize::from(b)].is_null())
            .collect();
        let bytes = encoded(name, &mixed);
        let view = decode(&bytes).unwrap();
        let points: Vec<_> = view.text[format!("# coding: {name}\r\n").len()..]
            .chars()
            .map(u32::from)
            .collect();
        assert_eq!(json!(points), row["mixed"]);
    }
    let cases = positives();
    let requests: Vec<_> = cases
        .iter()
        .map(|s| json!({"hex": s.hex, "text": decode(&s.bytes).unwrap().text}))
        .collect();
    let observed = trusted(
        r#"
import ast, json
request = json.loads(sys.stdin.buffer.read(524289))
assert len(request) <= 124
for case in request:
    raw = bytes.fromhex(case['hex'])
    assert len(raw) <= 4096
    a = ast.parse(raw, type_comments=True)
    b = ast.parse(case['text'], type_comments=True)
    assert ast.dump(a, include_attributes=True) == ast.dump(b, include_attributes=True)
json.dump(len(request), sys.stdout)
"#,
        &json!(requests),
    );
    assert_eq!(observed, json!(cases.len()));
    eprintln!("15872 approved table positions + 256 ASCII positions; 63 mixed/incremental streams; {} byte/text AST location cases", cases.len());
}

#[cfg(unix)]
#[test]
#[ignore = "requires explicit trusted exact CPython 3.12.11; complete encoded facts/IDs/diagnostics and raw helper order"]
fn trusted_all_profile_canonical_and_alias_complete_positive_arrays() {
    // Pin exact oracle version before invoking the shared complete-array harness.
    assert_eq!(
        trusted("import json\njson.dump(True, sys.stdout)", &json!(null)),
        json!(true)
    );
    let cases = positives();
    assert!(cases.len() >= 62 && cases.len() <= 124);
    super::ruff_facts_batch::assert_oracle_sources(cases);
}

#[cfg(unix)]
#[test]
#[ignore = "requires explicit trusted exact CPython 3.12.11; actual production helper source failures"]
fn trusted_production_helper_source_failure_records_and_module_ids() {
    let cases = negatives();
    let helper = include_str!("../../python_extract.py");
    let (helper, autocall) = helper
        .rsplit_once("\nif __name__ == \"__main__\":")
        .unwrap();
    assert_eq!(autocall.trim(), "main()");
    let script = format!(
        "{helper}\n{}",
        r#"
import contextlib, io, warnings
warnings.simplefilter('ignore')
requests = json.loads(sys.stdin.buffer.read(524289))
assert len(requests) <= 256
outputs = []
original_stdin = sys.stdin
for request in requests:
    capture = io.StringIO()
    sys.stdin = io.StringIO(json.dumps(request))
    with contextlib.redirect_stdout(capture):
        main()
    outputs.append(json.loads(capture.getvalue()))
sys.stdin = original_stdin
json.dump(outputs, sys.stdout, ensure_ascii=True)
"#
    );
    let requests: Vec<_> = cases
        .iter()
        .map(|s| json!({"sources": [{"path": s.path, "module": s.module, "hex": s.hex}]}))
        .collect();
    let observed = trusted(&script, &json!(requests));
    let observed = observed.as_array().unwrap();
    assert_eq!(observed.len(), cases.len());
    let failure = Failure {
        exception: Exception::SyntaxError,
        line: 0,
    };
    for (case, actual) in cases.iter().zip(observed) {
        let native = extract_fact_candidates(std::slice::from_ref(case)).unwrap();
        assert_eq!(actual["complete"], false, "{:?}", case.bytes);
        assert_eq!(
            actual["facts"],
            serde_json::to_value(&native.facts).unwrap()
        );
        assert_eq!(actual["facts"].as_array().unwrap().len(), 1);
        assert_eq!(actual["facts"][0]["kind"], "module");
        assert_eq!(
            actual["diagnostics"],
            json!([{"code": "parse_failure", "path": case.path, "message": failure.message()}]),
            "{:?}",
            case.bytes
        );
        assert_eq!(native.files[0].failure, Some(failure));
        assert!(native.diagnostics.is_empty());
    }
    eprintln!("{} certified SyntaxError-at-line-zero actual helper records and complete original-hash module baselines", cases.len());
    let controls: Vec<_> = [
        ("cp.1252", Some(0)),
        ("utf.8", Some(0)),
        ("latin.1", Some(0)),
        ("base64.codec", Some(0)),
        ("not_a_codec", Some(0)),
        ("cp037", Some(1)),
        ("cp500", Some(1)),
        ("cp864", None),
        ("utf-16", Some(1)),
        ("shift_jis", None),
    ]
    .into_iter()
    .map(|(name, line)| (input(encoded(name, b"eval()\n")), line))
    .collect();
    let requests: Vec<_> = controls
        .iter()
        .map(|(s, _)| json!({"sources": [{"path": s.path, "module": s.module, "hex": s.hex}]}))
        .collect();
    let observed = trusted(&script, &json!(requests));
    let observed = observed.as_array().unwrap();
    assert_eq!(observed.len(), controls.len());
    for ((case, line), actual) in controls.iter().zip(observed) {
        let native = extract_fact_candidates(std::slice::from_ref(case)).unwrap();
        assert_eq!(native.files[0].status, FactStatus::UnsupportedEncoding);
        assert_eq!(native.files[0].failure, None);
        assert_eq!(actual["complete"], json!(line.is_none()));
        if let Some(line) = line {
            let failure = Failure {
                exception: Exception::SyntaxError,
                line: *line,
            };
            assert_eq!(
                actual["diagnostics"],
                json!([{"code": "parse_failure", "path": case.path, "message": failure.message()}])
            );
        }
    }
    eprintln!("10 unsupported/unresolved helper controls, explicitly not certified parity matches (8 helper failures, 2 helper admissions)");
}
