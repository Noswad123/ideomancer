//! Policy-15 ordinary literal gates. Captured sources are parsed, never executed.
use super::super::python_native::{extract_fact_candidates, parse_batch, CandidateStatus};
use super::*;

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn literal_batch_numeric_categories_roots_and_call_free_headers() {
    let python = oracle();
    for (literal, shape) in [
        ("0", "<literal>"),
        ("00_0", "<literal>"),
        ("1_234", "<literal>"),
        ("0B_101", "<literal>"),
        ("0O_707", "<literal>"),
        ("0X_fF", "<literal>"),
        ("01.25", "<literal>"),
        (".5", "<literal>"),
        ("1.", "<literal>"),
        ("1_2.3_4e-5_6", "<literal>"),
        ("1e999999", "<literal>"),
        ("01j", "<literal>"),
        ("1.2E+3J", "<literal>"),
        ("-42", "<UnaryOp>"),
        ("+0x_FF", "<UnaryOp>"),
        ("-(2J)", "<UnaryOp>"),
    ] {
        let text = format!("root = {literal}\nclass Box({literal}):\n    field: {literal}\ndef owner(value: {literal} = {literal}) -> {literal}:\n    sink({literal})\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        assert_eq!(
            native["identity"]["policy"],
            "python-native-private-facts-15"
        );
        let call = format!("sink({shape})");
        assert_eq!(
            native["facts"],
            json!(expected(
                "unit.py",
                &[
                    (1, 1, "binding_invalidation", "unit", r#"{"target":"root"}"#),
                    (1, 1, "module", "unit", "unit"),
                    (2, 2, "base", "unit.Box", shape),
                    (2, 3, "class", "unit.Box", "Box"),
                    (3, 3, "attribute_annotation", "unit.Box.field", shape),
                    (4, 4, "parameter", "unit.owner.value", "value"),
                    (4, 4, "parameter_annotation", "unit.owner.value", shape),
                    (4, 4, "parameter_default", "unit.owner.value", shape),
                    (4, 4, "return_annotation", "unit.owner", shape),
                    (4, 5, "function", "unit.owner", "owner"),
                    (
                        4,
                        5,
                        "signature",
                        "unit.owner",
                        r#"{"keywordOnly":[],"positional":["value"],"positionalOnly":0}"#
                    ),
                    (5, 5, "call", "unit.owner", &call),
                ]
            ))
        );
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn literal_batch_decimal_conversion_boundary_and_exempt_categories() {
    let python = oracle();
    let mut accepted = vec!["9".repeat(513)];
    for count in [4299, 4300, 4301] {
        if count <= 4300 {
            accepted.push("9".repeat(count));
            accepted.push(vec!["9"; count].join("_"));
        }
        // pegen's zero shortcut is an assertion to challenge with the real AST,
        // not a waiver if the selected interpreter rejects this spelling.
        accepted.push("0".repeat(count));
        accepted.push(vec!["0"; count].join("_"));
        accepted.push(format!("{}.0", "9".repeat(count)));
        accepted.push(format!("{}e999999", "9".repeat(count)));
        accepted.push(format!("{}J", "9".repeat(count)));
        accepted.push(format!("0x_{}", "F".repeat(count)));
        accepted.push(format!("0o_{}", "7".repeat(count)));
        accepted.push(format!("0b_{}", "1".repeat(count)));
    }
    for literal in accepted {
        let text = format!("def owner(value={literal}): pass\nsink({literal})\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        assert_eq!(
            native["facts"],
            json!(expected(
                "unit.py",
                &[
                    (1, 1, "function", "unit.owner", "owner"),
                    (1, 1, "module", "unit", "unit"),
                    (1, 1, "parameter", "unit.owner.value", "value"),
                    (1, 1, "parameter_default", "unit.owner.value", "<literal>"),
                    (
                        1,
                        1,
                        "signature",
                        "unit.owner",
                        r#"{"keywordOnly":[],"positional":["value"],"positionalOnly":0}"#
                    ),
                    (2, 2, "call", "unit", "sink(<literal>)"),
                ]
            ))
        );
    }
    for literal in [
        "9".repeat(4301),
        vec!["9"; 4301].join("_"),
        format!("{}1", "0".repeat(4299)),
        format!("{}1", "0_".repeat(4300)),
    ] {
        let text = format!("eval()\nsink({literal})\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let ast = extract_captured(&python, &scan.sources).unwrap();
        assert!(!ast.complete);
        assert_facts(&ast, "unit.py", &[(1, 1, "module", "unit", "unit")]);
        assert_eq!(ast.diagnostics.len(), 1);
        assert_eq!(ast.diagnostics[0].code, "parse_failure");
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert!(matches!(
            native.files[0].status,
            CandidateStatus::SyntaxError | CandidateStatus::UnsupportedSyntax
        ));
        assert_eq!(
            serde_json::to_value(&native.facts).unwrap(),
            json!(expected("unit.py", &[(1, 1, "module", "unit", "unit")]))
        );
        assert!(native.diagnostics.is_empty());
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn literal_batch_ordinary_escapes_surrogates_and_typed_body_contexts() {
    let python = oracle();
    for literal in [
        r#"'\x00\x7f\xff'"#,
        r#"'\u0000\ud800\udfff\uffff'"#,
        r#"'\U0000d800\U0010ffff'"#,
        r#"'\a\b\f\n\r\t\v\\\"\''"#,
        r#"'\q\8\9\400\777'"#,
        r#"b'\x00\xff\400\777\q'"#,
        r#"b'\uD800\U00110000\N{NOT A NAME}'"#,
        r#"'one' u'\u0061' R'\q'"#,
        r#"b'one' Br'\u1234' RB'two'"#,
    ] {
        let text = format!("root = {literal}\nclass Box:\n    field: {literal}\ndef owner(value: {literal} = {literal}) -> {literal}:\n    local: {literal} = {literal}\n    sink({literal})\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        assert_eq!(
            native["facts"],
            json!(expected(
                "unit.py",
                &[
                    (1, 1, "binding_invalidation", "unit", r#"{"target":"root"}"#),
                    (1, 1, "module", "unit", "unit"),
                    (2, 3, "class", "unit.Box", "Box"),
                    (3, 3, "attribute_annotation", "unit.Box.field", "<literal>"),
                    (4, 4, "parameter", "unit.owner.value", "value"),
                    (
                        4,
                        4,
                        "parameter_annotation",
                        "unit.owner.value",
                        "<literal>"
                    ),
                    (4, 4, "parameter_default", "unit.owner.value", "<literal>"),
                    (4, 4, "return_annotation", "unit.owner", "<literal>"),
                    (4, 6, "function", "unit.owner", "owner"),
                    (
                        4,
                        6,
                        "signature",
                        "unit.owner",
                        r#"{"keywordOnly":[],"positional":["value"],"positionalOnly":0}"#
                    ),
                    (
                        5,
                        5,
                        "binding_invalidation",
                        "unit.owner",
                        r#"{"target":"local"}"#
                    ),
                    (6, 6, "call", "unit.owner", "sink(<literal>)"),
                ]
            ))
        );
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn literal_batch_raw_bytes_triples_joins_and_delimiter_parity() {
    let python = oracle();
    for prefix in [
        "", "r", "R", "u", "U", "b", "B", "br", "bR", "Br", "BR", "rb", "rB", "Rb", "RB",
    ] {
        for quote in ["'", "\"", "'''", "\"\"\""] {
            for slash_count in [0, 2, 4] {
                let literal = format!(
                    "{prefix}{quote}OPAQUE_RAW{}{}",
                    "\\".repeat(slash_count),
                    quote
                );
                let text = format!("sink({literal})\n");
                let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
                let native = assert_declaration_candidates_match_ast(&python, &scan);
                assert_eq!(
                    native["facts"],
                    json!(expected(
                        "unit.py",
                        &[
                            (1, 1, "call", "unit", "sink(<literal>)"),
                            (1, 1, "module", "unit", "unit"),
                        ]
                    ))
                );
                assert!(!native.to_string().contains("OPAQUE_RAW"));
            }
        }
    }
    for literal in [
        r#"r'escaped\'quote'"#,
        r#"br"escaped\"quote""#,
        "'''first\nsecond'''",
        "r'''first\nsecond'''",
        "b'''first\nsecond'''",
        "'first\\\nsecond'",
        "r'first\\\nsecond'",
        "b'first\\\nsecond'",
        "br'first\\\nsecond'",
        "\"\"\"one'\"two\"\"\"",
    ] {
        for newline in ["\n", "\r\n"] {
            let text = format!("sink({literal})\n").replace('\n', newline);
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            assert_declaration_candidates_match_ast(&python, &scan);
        }
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn literal_batch_concatenation_spans_bom_crlf_and_unicode_redaction() {
    let python = oracle();
    for newline in ["\n", "\r\n"] {
        for bom in [false, true] {
            let text = concat!(
                "sink(\n    'OPAQUE_TEXT_\u{3bb}\u{1f642}' # trivia\n    u'\\ud800'\n)\n",
                "sink(b'OPAQUE_BYTES'\n     RB'\\u1234')\n",
                "sink('''OPAQUE_TRIPLE\ncontinuation''')\n",
            )
            .replace('\n', newline);
            let mut bytes = if bom { vec![0xef, 0xbb, 0xbf] } else { vec![] };
            bytes.extend_from_slice(text.as_bytes());
            let (_, scan) = capture(&[("unit.py", &bytes)]);
            let native = assert_declaration_candidates_match_ast(&python, &scan);
            assert_eq!(
                native["facts"],
                json!(expected(
                    "unit.py",
                    &[
                        (1, 1, "module", "unit", "unit"),
                        (1, 4, "call", "unit", "sink(<literal>)"),
                        (5, 6, "call", "unit", "sink(<literal>)"),
                        (7, 8, "call", "unit", "sink(<literal>)"),
                    ]
                ))
            );
            assert_eq!(native["files"][0]["sha256"], hash(&bytes));
            for payload in [
                "OPAQUE_TEXT",
                "OPAQUE_BYTES",
                "OPAQUE_TRIPLE",
                "\u{3bb}",
                "\u{1f642}",
                "\\ud800",
                "\\u1234",
            ] {
                assert!(!native.to_string().contains(payload));
            }
        }
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn literal_batch_match_patterns_share_validators_without_literal_facts() {
    let python = oracle();
    let mut patterns: Vec<String> = [
        "-1",
        "0x_FF",
        "0b_10",
        "0o_77",
        "1e99999",
        "-1.5+2J",
        r#"'\ud800\U0010ffff'"#,
        r#"'\q\777'"#,
        r#"r'\N{ANY}'"#,
        r#"b'\u1234\N{ANY}'"#,
        r#"'one' u'\x32'"#,
        r#"b'one' RB'two'"#,
        r#"{'\x61': value, -1+2j: other}"#,
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    patterns.extend([
        "9".repeat(4300),
        "0".repeat(4301),
        format!("0x{}", "F".repeat(4301)),
        format!("{}J", "9".repeat(4301)),
    ]);
    for pattern in patterns {
        let text = format!("match subject:\n    case {pattern}: sink('OPAQUE_CASE_BODY')\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        assert_eq!(
            native["facts"],
            json!(expected(
                "unit.py",
                &[
                    (1, 1, "module", "unit", "unit"),
                    (2, 2, "call", "unit", "sink(<literal>)"),
                ]
            ))
        );
        assert!(!native.to_string().contains("OPAQUE_CASE_BODY"));
        let parsed = parse_batch(&scan.sources).unwrap();
        assert_eq!(
            native["files"][0]["nodes_visited"],
            json!(parsed.files[0].nodes_visited)
        );
    }
    for pattern in [
        "9".repeat(4301),
        "0b2".to_owned(),
        r#"'\u123'"#.to_owned(),
        r#"'\U00110000'"#.to_owned(),
        "b'\u{e9}'".to_owned(),
        "'text' b'bytes'".to_owned(),
    ] {
        let text = format!("eval()\nmatch subject:\n    case {pattern}: exec()\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let ast = extract_captured(&python, &scan.sources).unwrap();
        assert!(!ast.complete);
        assert_facts(&ast, "unit.py", &[(1, 1, "module", "unit", "unit")]);
        assert_eq!(ast.diagnostics.len(), 1);
        assert_eq!(ast.diagnostics[0].code, "parse_failure");
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert!(matches!(
            native.files[0].status,
            CandidateStatus::SyntaxError | CandidateStatus::UnsupportedSyntax
        ));
        assert_eq!(
            serde_json::to_value(&native.facts).unwrap(),
            json!(expected("unit.py", &[(1, 1, "module", "unit", "unit")]))
        );
        assert!(native.diagnostics.is_empty());
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn literal_batch_malformed_spellings_isolate_files_and_rollback_warnings() {
    let python = oracle();
    for literal in [
        "0b2",
        "0o8",
        "0xG",
        "0x",
        "0b__1",
        "1__0",
        "1_",
        "01",
        "0_1",
        "1e",
        "1e+",
        "1e_2",
        "1._2",
        "1_.0",
        "2jj",
        "1L",
        "0xffL",
        "ur'x'",
        "ru'x'",
        "bu'x'",
        "rr'x'",
        "bb'x'",
        "ubr'x'",
        r#"'\x0'"#,
        r#"'\xgg'"#,
        r#"'\u123'"#,
        r#"'\U0000123'"#,
        r#"'\U00110000'"#,
        r#"b'\x0'"#,
        "b'\u{e9}'",
        "br'\u{3bb}'",
        "'unterminated",
        "'''unterminated",
        r#"r'odd\'"#,
        "'text' b'bytes'",
        "b'bytes' 'text'",
        "'physical\nnewline'",
    ] {
        let text = format!("from tools import eval as go\ngo()\nsink({literal})\n");
        let (_, scan) = capture(&[
            ("bad.py", text.as_bytes()),
            ("good.py", b"visible('OPAQUE_GOOD')\n"),
        ]);
        let ast = extract_captured(&python, &scan.sources).unwrap();
        assert!(!ast.complete);
        let mut facts = expected("bad.py", &[(1, 1, "module", "bad", "bad")]);
        facts.extend(expected(
            "good.py",
            &[
                (1, 1, "call", "good", "visible(<literal>)"),
                (1, 1, "module", "good", "good"),
            ],
        ));
        assert_eq!(serde_json::to_value(&ast.facts).unwrap(), json!(facts));
        assert_eq!(ast.diagnostics.len(), 1);
        assert_eq!(ast.diagnostics[0].path, "bad.py");
        assert_eq!(ast.diagnostics[0].code, "parse_failure");
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert!(matches!(
            native.files[0].status,
            CandidateStatus::SyntaxError | CandidateStatus::UnsupportedSyntax
        ));
        assert_eq!(native.files[1].status, CandidateStatus::AcceptedFacts);
        assert_eq!(serde_json::to_value(&native.facts).unwrap(), json!(facts));
        assert!(native.diagnostics.is_empty());
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn literal_batch_named_escape_table_boundary_is_not_name_validation() {
    let python = oracle();
    for (literal, ast_complete) in [
        (r#"'\N{LATIN CAPITAL LETTER A}'"#, true),
        (r#"u'\N{SNOWMAN}'"#, true),
        (r#"'''\N{LATIN SMALL LETTER A}'''"#, true),
        (r#"'\N{IDEOMANCER UNKNOWN SYNTHETIC NAME}'"#, false),
    ] {
        for statement in [
            format!("sink({literal})\n"),
            format!("def owner(value={literal}): pass\n"),
            format!("match subject:\n    case {literal}: pass\n"),
        ] {
            let text = format!("eval()\n{statement}");
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            let ast = extract_captured(&python, &scan.sources).unwrap();
            assert_eq!(ast.complete, ast_complete);
            if ast_complete {
                assert_eq!(ast.diagnostics.len(), 1);
                assert_eq!(ast.diagnostics[0].code, "dynamic_discovery_unresolved");
            } else {
                assert_facts(&ast, "unit.py", &[(1, 1, "module", "unit", "unit")]);
                assert_eq!(ast.diagnostics.len(), 1);
                assert_eq!(ast.diagnostics[0].code, "parse_failure");
            }
            let native = extract_fact_candidates(&scan.sources).unwrap();
            assert_eq!(native.files[0].status, CandidateStatus::UnsupportedSyntax);
            assert_eq!(
                serde_json::to_value(&native.facts).unwrap(),
                json!(expected("unit.py", &[(1, 1, "module", "unit", "unit")]))
            );
            assert!(native.diagnostics.is_empty());
            assert!(!serde_json::to_string(&native).unwrap().contains("LATIN"));
            assert!(!serde_json::to_string(&native)
                .unwrap()
                .contains("SYNTHETIC NAME"));
        }
    }
    for literal in [
        r#"r'\N{LATIN CAPITAL LETTER A}'"#,
        r#"R'\N{IDEOMANCER UNKNOWN SYNTHETIC NAME}'"#,
        r#"b'\N{LATIN CAPITAL LETTER A}'"#,
        r#"Br'\N{UNKNOWN}'"#,
        r#"r'\N{'"#,
        r#"b'\N{'"#,
        r#"'\\N{UNKNOWN}'"#,
    ] {
        let text = format!("sink({literal})\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        assert_eq!(
            native["facts"],
            json!(expected(
                "unit.py",
                &[
                    (1, 1, "call", "unit", "sink(<literal>)"),
                    (1, 1, "module", "unit", "unit"),
                ]
            ))
        );
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn literal_batch_hidden_payloads_shape_limits_and_metadata_guards() {
    let python = oracle();
    let payload = format!("'OPAQUE_HIDDEN_{}'", "x".repeat(513));
    let mut expressions = vec![payload.clone(), format!("[{payload}]")];
    for depth in [11, 12, 13] {
        expressions.push(format!(
            "{}{payload}{}",
            "T[".repeat(depth),
            "]".repeat(depth)
        ));
    }
    for count in [79, 80, 81] {
        expressions.push(format!("[{}]", vec![payload.as_str(); count].join(",")));
    }
    for length in [511, 512, 513] {
        expressions.push(format!("{}[{payload}]", "A".repeat(length)));
    }
    for expression in expressions {
        for text in [
            format!("sink({expression})\n"),
            format!("def owner(value: {expression} = {expression}) -> {expression}: pass\n"),
            format!("class Box({expression}):\n    field: {expression}\n"),
        ] {
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            let native = assert_declaration_candidates_match_ast(&python, &scan);
            let ast = extract_captured(&python, &scan.sources).unwrap();
            let parsed = parse_batch(&scan.sources).unwrap();
            for report in [
                native.to_string(),
                serde_json::to_string(&semantics(&ast)).unwrap(),
                serde_json::to_string(&parsed).unwrap(),
            ] {
                assert!(!report.contains("OPAQUE_HIDDEN"));
                assert!(!report.contains(&"x".repeat(513)));
            }
        }
    }
    for form in [
        "FastAPI('OPAQUE_METADATA')",
        "APIRouter(prefix='OPAQUE_METADATA')",
        "router.include_router(child, prefix='OPAQUE_METADATA')",
        "def owner(value=hidden('OPAQUE_METADATA')): pass",
    ] {
        let text = format!("eval()\n{form}\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert!(extract_captured(&python, &scan.sources).unwrap().complete);
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_eq!(native.files[0].status, CandidateStatus::UnsupportedSyntax);
        assert_eq!(
            serde_json::to_value(&native.facts).unwrap(),
            json!(expected("unit.py", &[(1, 1, "module", "unit", "unit")]))
        );
        assert!(native.diagnostics.is_empty());
        assert!(!serde_json::to_string(&native)
            .unwrap()
            .contains("OPAQUE_METADATA"));
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn literal_batch_replay_resources_recursion_and_source_nonexecution() {
    let python = oracle();
    let entries: &[(&str, &[u8])] = &[
        ("a.py", b"\xef\xbb\xbfsink('OPAQUE_REPLAY' '\\ud800')\r\n"),
        ("b.py", b"from pathlib import Path\nPath('literal-side-effect').write_text('OPAQUE_NOT_EXECUTED')\n"),
    ];
    let (root, mut scan) = capture(entries);
    let first = assert_declaration_candidates_match_ast(&python, &scan);
    let ast = semantics(&extract_captured(&python, &scan.sources).unwrap());
    assert!(!root.path().join("literal-side-effect").exists());
    for (path, _) in entries {
        fs::write(
            root.path().join(path),
            b"raise RuntimeError('replacement')\n",
        )
        .unwrap();
        fs::remove_file(root.path().join(path)).unwrap();
    }
    assert_eq!(
        first,
        assert_declaration_candidates_match_ast(&python, &scan)
    );
    assert_eq!(
        ast,
        semantics(&extract_captured(&python, &scan.sources).unwrap())
    );
    scan.sources.reverse();
    let reversed = assert_declaration_candidates_match_ast(&python, &scan);
    assert_eq!(first["facts"], reversed["facts"]);
    assert_eq!(first["diagnostics"], reversed["diagnostics"]);
    assert_eq!(
        ast,
        semantics(&extract_captured(&python, &scan.sources).unwrap())
    );
    assert!(!root.path().join("literal-side-effect").exists());
    for (file, (path, bytes)) in first["files"].as_array().unwrap().iter().zip(entries) {
        assert_eq!(file["path"], *path);
        assert_eq!(file["sha256"], hash(bytes));
    }
    let prefix = "sink(0b";
    let suffix = ")\n";
    let text = format!("{prefix}{}{suffix}", "1".repeat(4301));
    let padded = format!("{}{}", " ".repeat(MAX_FILE - text.len() - 1), "\n");
    let text = format!("{padded}{text}");
    assert_eq!(text.len(), MAX_FILE);
    let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
    assert_declaration_candidates_match_ast(&python, &scan);
    let text = format!("{}\n", " ".repeat(MAX_FILE - 1));
    let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
    let native = assert_declaration_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected("unit.py", &[(1, 1, "module", "unit", "unit")]))
    );
    let deep = format!(
        "eval()\nsink({}'OPAQUE_DEPTH'{})\n",
        "nest(".repeat(140),
        ")".repeat(140)
    );
    let (_, scan) = capture(&[("bad.py", deep.as_bytes()), ("good.py", b"visible()\n")]);
    assert!(extract_captured(&python, &scan.sources).unwrap().complete);
    let native = extract_fact_candidates(&scan.sources).unwrap();
    assert_eq!(native.files[0].status, CandidateStatus::ResourceLimit);
    assert_eq!(native.files[1].status, CandidateStatus::AcceptedFacts);
    let mut facts = expected("bad.py", &[(1, 1, "module", "bad", "bad")]);
    facts.extend(expected(
        "good.py",
        &[
            (1, 1, "call", "good", "visible()"),
            (1, 1, "module", "good", "good"),
        ],
    ));
    assert_eq!(serde_json::to_value(&native.facts).unwrap(), json!(facts));
    assert!(native.diagnostics.is_empty());
    assert!(!serde_json::to_string(&native)
        .unwrap()
        .contains("OPAQUE_DEPTH"));
}
