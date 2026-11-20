//! Bounded AST parity gates, not a native parser or a full Python parity claim.
//! Supported here: lexical ownership/signatures, safe expression shapes, local
//! call guards, bounded imports, direct route syntax, file transactions, and byte decoding.
//! Later native work still needs broader syntax, binding/control-flow, resource,
//! and cross-version coverage before it can propose a production migration.

use super::*;
use serde_json::{json, Value};

const ORACLE_ENV: &str = "IDEOMANCER_TEST_PYTHON";

fn selected_oracle(selection: Option<std::ffi::OsString>) -> Result<PathBuf, String> {
    let path = PathBuf::from(selection.ok_or("explicit trusted Python selection is required")?);
    if !path.is_absolute() || !path.is_file() {
        return Err("oracle must be an absolute path to an installed trusted Python >=3.12".into());
    }
    // The real helper enforces >=3.12, using the same isolation and bounded I/O
    // as production. Missing/old/non-Python executables are hard failures.
    let response = extract_captured(&path, &[])?;
    eprintln!(
        "Python parity oracle: {} ({})",
        response.python_version,
        path.display()
    );
    Ok(path)
}

fn oracle() -> PathBuf {
    selected_oracle(std::env::var_os(ORACLE_ENV)).unwrap_or_else(|e| panic!("{ORACLE_ENV}: {e}"))
}

fn capture(entries: &[(&str, &[u8])]) -> (tempfile::TempDir, Scan) {
    let root = tempfile::tempdir().unwrap();
    for (path, bytes) in entries {
        let path = root.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
    let scan = extract_with_policy(root.path(), Language::Python).unwrap();
    (root, scan)
}

// Normalize only the semantic response, preserving IDs, encoded values, and
// array order. Backend metadata is compared separately, never silently erased
// from production Evidence or its snapshot identity.
fn semantics(response: &Response) -> Value {
    json!({"facts": response.facts, "diagnostics": response.diagnostics,
           "complete": response.complete})
}

fn evidence_semantics(evidence: &Evidence) -> Value {
    json!({"facts": evidence.facts, "diagnostics": evidence.diagnostics,
           "complete": evidence.complete})
}

// Independent Python json.dumps(ensure_ascii=True) spelling for ID preimages:
// default comma/colon spacing, including surrogate pairs rather than UTF-8.
fn python_json(value: &Value) -> String {
    match value {
        Value::Array(items) => format!(
            "[{}]",
            items.iter().map(python_json).collect::<Vec<_>>().join(", ")
        ),
        Value::String(text) => {
            let quoted = serde_json::to_string(text).unwrap();
            let mut result = String::new();
            for ch in quoted.chars() {
                if ch.is_ascii() && ch != '\u{007f}' {
                    result.push(ch);
                } else {
                    for unit in ch.encode_utf16(&mut [0; 2]) {
                        use std::fmt::Write;
                        write!(result, "\\u{unit:04x}").unwrap();
                    }
                }
            }
            result
        }
        Value::Number(_) => value.to_string(),
        _ => panic!("identity preimages contain only arrays, strings, and integers"),
    }
}

type Row<'a> = (usize, usize, &'a str, &'a str, &'a str);

fn expected(path: &str, rows: &[Row<'_>]) -> Vec<Value> {
    rows.iter().map(|&(line, end, kind, symbol, value)| {
        let key = if kind == "module" {
            path.to_owned()
        } else {
            python_json(&json!([path, kind, line, end, symbol, value]))
        };
        json!({"id": format!("{}_{}", if kind == "module" { "module" } else { "fact" }, hash(key.as_bytes())),
            "path": path, "line": line, "endLine": end, "kind": kind,
            "symbol": symbol, "value": value})
    }).collect()
}

fn assert_facts(response: &Response, path: &str, rows: &[Row<'_>]) {
    assert_eq!(
        serde_json::to_value(&response.facts).unwrap(),
        json!(expected(path, rows))
    );
}

#[test]
fn identity_preimages_preserve_python_spacing_escapes_and_module_special_case() {
    assert_eq!(
        python_json(&json!(["a.py", "call", 2, 2, "a.f", "f(<literal>)"])),
        r#"["a.py", "call", 2, 2, "a.f", "f(<literal>)"]"#
    );
    assert_eq!(
        python_json(&json!([
            "caf\u{e9}.py",
            "call",
            2,
            2,
            "caf\u{e9}.f",
            "f(\u{1f642})"
        ])),
        r#"["caf\u00e9.py", "call", 2, 2, "caf\u00e9.f", "f(\ud83d\ude42)"]"#
    );
    assert_eq!(
        python_json(&json!(["x\n\t\"\\", "\u{007f}"])),
        r#"["x\n\t\"\\", "\u007f"]"#
    );
    assert_eq!(
        python_json(&json!([
            "unit.py",
            "signature",
            1,
            2,
            "unit.f",
            r#"{"keywordOnly":[],"positional":[],"positionalOnly":0}"#
        ])),
        r#"["unit.py", "signature", 1, 2, "unit.f", "{\"keywordOnly\":[],\"positional\":[],\"positionalOnly\":0}"]"#
    );
    let rows = [(1, 1, "module", "first", "first")];
    let a = expected("unit.py", &rows);
    let b = expected("unit.py", &[(9, 10, "module", "other", "other")]);
    assert_eq!(a[0]["id"], b[0]["id"]);
    assert_eq!(a[0]["id"], format!("module_{}", hash(b"unit.py")));
}

#[test]
fn normalizer_does_not_hide_order_duplicates_ids_or_encoded_values() {
    let make = || Response {
        python_version: "backend-a".into(),
        facts: serde_json::from_value(json!(expected(
            "a.py",
            &[(1, 1, "module", "a", "a"), (2, 2, "call", "a", "f()")]
        )))
        .unwrap(),
        diagnostics: vec![],
        complete: true,
    };
    let baseline = semantics(&make());
    let mut changed = make();
    changed.python_version = "backend-b".into();
    assert_eq!(baseline, semantics(&changed));
    assert_ne!(make().python_version, changed.python_version);
    changed.facts.reverse();
    assert_ne!(baseline, semantics(&changed));
    changed = make();
    changed
        .facts
        .push(serde_json::from_value(serde_json::to_value(&changed.facts[0]).unwrap()).unwrap());
    assert_ne!(baseline, semantics(&changed));
    changed = make();
    changed.facts[0].id.push('x');
    assert_ne!(baseline, semantics(&changed));
    changed = make();
    changed.facts[1].value.push(' ');
    assert_ne!(baseline, semantics(&changed));
    changed = make();
    changed.facts.pop();
    assert_ne!(baseline, semantics(&changed));
}

#[test]
fn protocol_limit_boundaries_reuse_production_constants_without_large_allocations() {
    assert_eq!(MAX_INPUT, 40 * 1024 * 1024);
    assert!(check_python_input_size(MAX_INPUT).is_ok());
    assert_eq!(
        check_python_input_size(MAX_INPUT + 1).unwrap_err(),
        "Python extractor input exceeds the 40 MiB protocol limit"
    );
    assert!(check_python_input_size(usize::MAX).is_err());
    assert_eq!(
        (MAX_FILES, MAX_FILE, MAX_TOTAL, MAX_OUTPUT),
        (5000, 1024 * 1024, 16 * 1024 * 1024, 32 * 1024 * 1024)
    );
    assert_eq!(DEADLINE, Duration::from_secs(60));
}

#[test]
fn oracle_selection_fails_closed_without_selection_or_installed_file() {
    assert!(selected_oracle(None)
        .unwrap_err()
        .contains("explicit trusted"));
    assert!(selected_oracle(Some("python3.12".into())).is_err());
    let root = tempfile::tempdir().unwrap();
    assert!(selected_oracle(Some(root.path().join("missing").into_os_string())).is_err());
    assert!(selected_oracle(Some("/usr/bin/false".into())).is_err());
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn definitions_ownership_imports_and_ordered_signature() {
    let python = oracle();
    let (_, scan) = capture(&[("unit.py", b"import pkg as p\nfrom .peer import Item\n@decorate(42)\nclass Box(Base):\n    def run(self, z: T, /, a='PRIVATE_DEFAULT', *rest, k: K=7, **opts) -> R:\n        async def nested():\n            pass\nasync def top():\n    pass\n")]);
    let response = extract_captured(&python, &scan.sources).unwrap();
    assert!(response.complete && response.diagnostics.is_empty());
    assert_facts(
        &response,
        "unit.py",
        &[
            (
                1,
                1,
                "import",
                "unit",
                r#"{"level":0,"module":"","names":[{"asname":"p","name":"pkg"}]}"#,
            ),
            (1, 1, "module", "unit", "unit"),
            (
                2,
                2,
                "from_import",
                "unit",
                r#"{"level":1,"module":"peer","names":[{"asname":null,"name":"Item"}]}"#,
            ),
            (3, 3, "call", "unit.Box", "decorate(<literal>)"),
            (3, 3, "decorator", "unit.Box", "decorate(<literal>)"),
            (4, 4, "base", "unit.Box", "Base"),
            (4, 7, "class", "unit.Box", "Box"),
            (5, 5, "parameter", "unit.Box.run.a", "a"),
            (5, 5, "parameter", "unit.Box.run.k", "k"),
            (5, 5, "parameter", "unit.Box.run.opts", "opts"),
            (5, 5, "parameter", "unit.Box.run.rest", "rest"),
            (5, 5, "parameter", "unit.Box.run.self", "self"),
            (5, 5, "parameter", "unit.Box.run.z", "z"),
            (5, 5, "parameter_annotation", "unit.Box.run.k", "K"),
            (5, 5, "parameter_annotation", "unit.Box.run.z", "T"),
            (5, 5, "parameter_default", "unit.Box.run.a", "<literal>"),
            (5, 5, "parameter_default", "unit.Box.run.k", "<literal>"),
            (5, 5, "return_annotation", "unit.Box.run", "R"),
            (5, 7, "function", "unit.Box.run", "run"),
            (
                5,
                7,
                "signature",
                "unit.Box.run",
                r#"{"keywordOnly":["k"],"kwarg":"opts","positional":["self","z","a"],"positionalOnly":2,"vararg":"rest"}"#,
            ),
            (6, 7, "async_function", "unit.Box.run.nested", "nested"),
            (
                6,
                7,
                "signature",
                "unit.Box.run.nested",
                r#"{"keywordOnly":[],"positional":[],"positionalOnly":0}"#,
            ),
            (8, 9, "async_function", "unit.top", "top"),
            (
                8,
                9,
                "signature",
                "unit.top",
                r#"{"keywordOnly":[],"positional":[],"positionalOnly":0}"#,
            ),
        ],
    );
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn calls_deduplicate_and_literal_invalidation_retains_lexical_guards() {
    let python = oracle();
    let (root, scan) = capture(&[("unit.py", b"def helper():\n    pass\ndef entry(helper):\n    helper(); helper()\n    local = helper\n    local = 'PRIVATE_BINDING'\n    local()\ndef direct():\n    helper()\n")]);
    let response = extract_captured(&python, &scan.sources).unwrap();
    assert!(response.complete && response.diagnostics.is_empty());
    assert_facts(
        &response,
        "unit.py",
        &[
            (1, 1, "module", "unit", "unit"),
            (1, 2, "function", "unit.helper", "helper"),
            (
                1,
                2,
                "signature",
                "unit.helper",
                r#"{"keywordOnly":[],"positional":[],"positionalOnly":0}"#,
            ),
            (3, 3, "parameter", "unit.entry.helper", "helper"),
            (3, 7, "function", "unit.entry", "entry"),
            (
                3,
                7,
                "signature",
                "unit.entry",
                r#"{"keywordOnly":[],"positional":["helper"],"positionalOnly":0}"#,
            ),
            (4, 4, "call", "unit.entry", "helper()"),
            (
                5,
                5,
                "binding",
                "unit.entry",
                r#"{"source":"helper","target":"local"}"#,
            ),
            (
                6,
                6,
                "binding_invalidation",
                "unit.entry",
                r#"{"target":"local"}"#,
            ),
            (7, 7, "call", "unit.entry", "local()"),
            (8, 9, "function", "unit.direct", "direct"),
            (
                8,
                9,
                "signature",
                "unit.direct",
                r#"{"keywordOnly":[],"positional":[],"positionalOnly":0}"#,
            ),
            (9, 9, "call", "unit.direct", "helper()"),
        ],
    );
    let production = extract(root.path(), &python).unwrap();
    assert_eq!(semantics(&response), evidence_semantics(&production));
    for (symbol, links, unresolved) in [("unit.entry", 0, 2), ("unit.direct", 1, 0)] {
        let prepared = crate::focus::prepare(
            &production,
            &crate::focus::FocusOptions {
                symbol: symbol.into(),
                hops: 2,
                max_facts: 100,
            },
        )
        .unwrap();
        assert_eq!(
            prepared.input["focus"]["links"].as_array().unwrap().len(),
            links
        );
        assert_eq!(
            prepared.input["focus"]["unresolved"]
                .as_array()
                .unwrap()
                .len(),
            unresolved
        );
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn same_line_call_deduplication_and_source_order_independence() {
    let python = oracle();
    let (_, mut scan) = capture(&[
        ("other.py", b"# module only\n"),
        (
            "unit.py",
            b"def helper():\n    pass\ndef entry():\n    helper(); helper()\n",
        ),
    ]);
    let response = extract_captured(&python, &scan.sources).unwrap();
    let mut facts = expected("other.py", &[(1, 1, "module", "other", "other")]);
    facts.extend(expected(
        "unit.py",
        &[
            (1, 1, "module", "unit", "unit"),
            (1, 2, "function", "unit.helper", "helper"),
            (
                1,
                2,
                "signature",
                "unit.helper",
                r#"{"keywordOnly":[],"positional":[],"positionalOnly":0}"#,
            ),
            (3, 4, "function", "unit.entry", "entry"),
            (
                3,
                4,
                "signature",
                "unit.entry",
                r#"{"keywordOnly":[],"positional":[],"positionalOnly":0}"#,
            ),
            // Column offsets are not identity: both calls produce this one fact.
            (4, 4, "call", "unit.entry", "helper()"),
        ],
    ));
    assert_eq!(
        semantics(&response),
        json!({
            "facts": facts, "diagnostics": [], "complete": true,
        })
    );
    let repeated = extract_captured(&python, &scan.sources).unwrap();
    assert_eq!(semantics(&response), semantics(&repeated));
    assert_eq!(response.python_version, repeated.python_version);
    assert_eq!(
        scan.sources
            .iter()
            .map(|s| s.path.as_str())
            .collect::<Vec<_>>(),
        ["other.py", "unit.py"]
    );
    scan.sources.reverse();
    let reversed = extract_captured(&python, &scan.sources).unwrap();
    assert_eq!(semantics(&response), semantics(&reversed));
    assert_eq!(response.python_version, reversed.python_version);
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn route_prefix_syntax_dynamic_boundary_and_literal_privacy() {
    let python = oracle();
    let (_, scan) = capture(&[("unit.py", b"from fastapi import APIRouter\nr = APIRouter(prefix='/v1')\n@r.get('/items')\ndef items():\n    client.get('PRIVATE_PAYLOAD')\n@r.post(dynamic_path)\nasync def dynamic():\n    pass\n")]);
    let response = extract_captured(&python, &scan.sources).unwrap();
    assert!(response.complete);
    assert_facts(
        &response,
        "unit.py",
        &[
            (
                1,
                1,
                "from_import",
                "unit",
                r#"{"level":0,"module":"fastapi","names":[{"asname":null,"name":"APIRouter"}]}"#,
            ),
            (1, 1, "module", "unit", "unit"),
            (
                2,
                2,
                "binding",
                "unit",
                r#"{"source":"APIRouter(prefix=<literal>)","target":"r"}"#,
            ),
            (2, 2, "call", "unit", "APIRouter(prefix=<literal>)"),
            (
                2,
                2,
                "router_prefix",
                "unit",
                r#"{"constructor":"APIRouter","prefix":"/v1","supported":true,"target":"r"}"#,
            ),
            (
                3,
                3,
                "api_route",
                "unit.items",
                r#"{"methods":["GET"],"path":"/items","pathIsFragment":true,"receiver":"r"}"#,
            ),
            (3, 3, "call", "unit.items", "r.get(<literal>)"),
            (3, 3, "decorator", "unit.items", "r.get(<literal>)"),
            (4, 5, "function", "unit.items", "items"),
            (
                4,
                5,
                "signature",
                "unit.items",
                r#"{"keywordOnly":[],"positional":[],"positionalOnly":0}"#,
            ),
            (5, 5, "call", "unit.items", "client.get(<literal>)"),
            (
                6,
                6,
                "api_route",
                "unit.dynamic",
                r#"{"methods":["POST"],"path":null,"pathIsFragment":true,"receiver":"r"}"#,
            ),
            (6, 6, "call", "unit.dynamic", "r.post(dynamic_path)"),
            (6, 6, "decorator", "unit.dynamic", "r.post(dynamic_path)"),
            (7, 8, "async_function", "unit.dynamic", "dynamic"),
            (
                7,
                8,
                "signature",
                "unit.dynamic",
                r#"{"keywordOnly":[],"positional":[],"positionalOnly":0}"#,
            ),
        ],
    );
    assert_eq!(
        serde_json::to_value(&response.diagnostics).unwrap(),
        json!([{
            "code":"dynamic_api_route", "path":"unit.py",
            "message":"line 6: route path is not a bounded direct slash-prefixed string; prefix not inferred"
        }])
    );
    assert!(!semantics(&response).to_string().contains("PRIVATE_PAYLOAD"));
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn literal_overwrite_blocks_previously_supported_receiver_edge() {
    let python = oracle();
    for overwritten in [false, true] {
        let source = format!("class Worker:\n    def go(self):\n        pass\ndef entry():\n    obj = Worker()\n    {}\n    obj.go()\n",
            if overwritten { "obj = 'PRIVATE_OVERWRITE'" } else { "pass" });
        let (root, scan) = capture(&[("unit.py", source.as_bytes())]);
        let response = extract_captured(&python, &scan.sources).unwrap();
        assert!(response.complete && response.diagnostics.is_empty());
        let mut rows = vec![
            (1, 1, "module", "unit", "unit"),
            (1, 3, "class", "unit.Worker", "Worker"),
            (2, 2, "parameter", "unit.Worker.go.self", "self"),
            (2, 3, "function", "unit.Worker.go", "go"),
            (
                2,
                3,
                "signature",
                "unit.Worker.go",
                r#"{"keywordOnly":[],"positional":["self"],"positionalOnly":0}"#,
            ),
            (4, 7, "function", "unit.entry", "entry"),
            (
                4,
                7,
                "signature",
                "unit.entry",
                r#"{"keywordOnly":[],"positional":[],"positionalOnly":0}"#,
            ),
            (
                5,
                5,
                "binding",
                "unit.entry",
                r#"{"source":"Worker()","target":"obj"}"#,
            ),
            (5, 5, "call", "unit.entry", "Worker()"),
        ];
        if overwritten {
            rows.push((
                6,
                6,
                "binding_invalidation",
                "unit.entry",
                r#"{"target":"obj"}"#,
            ));
        }
        rows.push((7, 7, "call", "unit.entry", "obj.go()"));
        assert_facts(&response, "unit.py", &rows);
        let production = extract(root.path(), &python).unwrap();
        assert_eq!(semantics(&response), evidence_semantics(&production));
        let prepared = crate::focus::prepare(
            &production,
            &crate::focus::FocusOptions {
                symbol: "unit.entry".into(),
                hops: 2,
                max_facts: 100,
            },
        )
        .unwrap();
        assert_eq!(
            prepared.input["focus"]["links"].as_array().unwrap().len(),
            usize::from(!overwritten)
        );
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn malformed_file_is_module_only_and_diagnostic_is_sanitized() {
    let python = oracle();
    let (root, scan) = capture(&[
        ("a.py", b"def ok():\n    pass\n"),
        ("b.py", b"def discarded():\n    pass\nPRIVATE_PARSE = (\n"),
    ]);
    let response = extract_captured(&python, &scan.sources).unwrap();
    let mut facts = expected(
        "a.py",
        &[
            (1, 1, "module", "a", "a"),
            (1, 2, "function", "a.ok", "ok"),
            (
                1,
                2,
                "signature",
                "a.ok",
                r#"{"keywordOnly":[],"positional":[],"positionalOnly":0}"#,
            ),
        ],
    );
    facts.extend(expected("b.py", &[(1, 1, "module", "b", "b")]));
    assert_eq!(
        semantics(&response),
        json!({"facts":facts,"complete":false,
        "diagnostics":[{"code":"parse_failure","path":"b.py",
        "message":"SyntaxError at line 3; source not analyzed"}]})
    );
    let production = extract(root.path(), &python).unwrap();
    assert_eq!(semantics(&response), evidence_semantics(&production));
    assert_eq!(response.python_version, production.python_version);
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn unicode_crlf_bom_cookie_and_invalid_bytes() {
    let python = oracle();
    for bytes in [
        "def caf\u{e9}():\r\n    emit('\u{1f642}')\r\n"
            .as_bytes()
            .to_vec(),
        "\u{feff}def caf\u{e9}():\n    emit('\u{1f642}')\n"
            .as_bytes()
            .to_vec(),
    ] {
        let (_, scan) = capture(&[("caf\u{e9}\u{1f642}.py", &bytes)]);
        let response = extract_captured(&python, &scan.sources).unwrap();
        assert!(response.complete && response.diagnostics.is_empty());
        assert_facts(
            &response,
            "caf\u{e9}\u{1f642}.py",
            &[
                (1, 1, "module", "caf\u{e9}\u{1f642}", "caf\u{e9}\u{1f642}"),
                (
                    1,
                    2,
                    "function",
                    "caf\u{e9}\u{1f642}.caf\u{e9}",
                    "caf\u{e9}",
                ),
                (
                    1,
                    2,
                    "signature",
                    "caf\u{e9}\u{1f642}.caf\u{e9}",
                    r#"{"keywordOnly":[],"positional":[],"positionalOnly":0}"#,
                ),
                (
                    2,
                    2,
                    "call",
                    "caf\u{e9}\u{1f642}.caf\u{e9}",
                    "emit(<literal>)",
                ),
            ],
        );
    }
    let (_, scan) = capture(&[(
        "cookie.py",
        b"# coding: latin-1\ndef caf\xe9():\n    pass\n",
    )]);
    let response = extract_captured(&python, &scan.sources).unwrap();
    assert!(response.complete && response.diagnostics.is_empty());
    assert_facts(
        &response,
        "cookie.py",
        &[
            (1, 1, "module", "cookie", "cookie"),
            (2, 3, "function", "cookie.caf\u{e9}", "caf\u{e9}"),
            (
                2,
                3,
                "signature",
                "cookie.caf\u{e9}",
                r#"{"keywordOnly":[],"positional":[],"positionalOnly":0}"#,
            ),
        ],
    );
    let (_, scan) = capture(&[("invalid.py", b"def ok():\n    pass\n\xff\n")]);
    let response = extract_captured(&python, &scan.sources).unwrap();
    assert_facts(
        &response,
        "invalid.py",
        &[(1, 1, "module", "invalid", "invalid")],
    );
    assert_eq!(
        semantics(&response)["diagnostics"],
        json!([{
        "code":"parse_failure","path":"invalid.py",
        "message":"SyntaxError at line 3; source not analyzed"}])
    );
    assert!(!response.complete);
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn captured_replay_matches_production_identity_without_second_read_or_execution() {
    let python = oracle();
    let (root, _) = capture(&[("src/pkg/__init__.py", b"")]);
    let marker = root.path().join("executed");
    fs::write(
        root.path().join("src/pkg/unit.py"),
        format!(
            "open({:?}, 'w').write('PRIVATE_MARKER')\ndef entry():\n    pass\n",
            marker.to_str().unwrap()
        ),
    )
    .unwrap();
    let scan = extract_with_policy(root.path(), Language::Python).unwrap();
    let captured = extract_captured(&python, &scan.sources).unwrap();
    let production = extract(root.path(), &python).unwrap();
    assert_eq!(semantics(&captured), evidence_semantics(&production));
    assert_eq!(captured.python_version, production.python_version);
    assert_eq!(production.schema_version, "ideomancer.evidence.v1");
    assert_eq!(
        production.extractor_version,
        "python-static-8-generic-facts"
    );
    assert!(production.language.is_none());
    assert!(serde_json::to_value(&production)
        .unwrap()
        .get("language")
        .is_none());
    assert_eq!(
        scan.sources
            .iter()
            .map(|s| s.module.as_str())
            .collect::<Vec<_>>(),
        ["pkg", "pkg.unit"]
    );
    assert_eq!(
        production
            .files
            .iter()
            .map(|f| (&f.path, &f.sha256))
            .collect::<Vec<_>>(),
        scan.files
            .iter()
            .map(|f| (&f.path, &f.sha256))
            .collect::<Vec<_>>()
    );
    let preimage = serde_json::to_vec(&(
        "python-scope-2-nofollow",
        "python-static-8-generic-facts",
        &captured.python_version,
        "",
        &scan.files,
    ))
    .unwrap();
    assert_eq!(production.snapshot_id, hash(&preimage));
    assert!(!marker.exists());
    let replacement = format!(
        "open({:?}, 'w').write('PRIVATE_MARKER')\n",
        marker.to_str().unwrap()
    );
    fs::write(root.path().join("src/pkg/unit.py"), replacement).unwrap();
    // Remove the tree entirely: replay must not require even its existence.
    fs::remove_dir_all(root.path().join("src")).unwrap();
    let replay = extract_captured(&python, &scan.sources).unwrap();
    assert_eq!(semantics(&captured), semantics(&replay));
    assert_eq!(captured.python_version, replay.python_version);
    assert!(!marker.exists());
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn private_frontend_reads_the_same_captured_utf8_without_claiming_fact_parity() {
    use super::python_native::{parse_batch, ParseStatus};

    let python = oracle();
    for (bytes, ast_complete, native_status) in [
        (
            b"def helper(value):\n    return value\nasync def entry(value):\n    return helper(value)\n".as_slice(),
            true,
            ParseStatus::SyntaxAccepted,
        ),
        (
            b"def broken(:\n    pass\n".as_slice(),
            false,
            ParseStatus::SyntaxError,
        ),
        (
            b"\xef\xbb\xbfvalue = 'PRIVATE_PARSE_LITERAL'\r\n".as_slice(),
            true,
            ParseStatus::SyntaxAccepted,
        ),
    ] {
        let (root, scan) = capture(&[("unit.py", bytes)]);
        let ast = extract_captured(&python, &scan.sources).unwrap();
        let native = parse_batch(&scan.sources).unwrap();
        assert_eq!(ast.complete, ast_complete);
        assert_eq!(native.files[0].status, native_status);
        assert!(native.files[0].traversal_finished);
        assert_eq!(native.files[0].path, scan.sources[0].path);
        assert_eq!(native.files[0].module, scan.sources[0].module);
        assert_eq!(native.files[0].sha256, hash(bytes));
        let native_json = serde_json::to_value(&native).unwrap();
        for absent in ["facts", "complete", "snapshotId", "schemaVersion", "pythonVersion"] {
            assert!(native_json.get(absent).is_none());
        }
        assert!(!native_json.to_string().contains("PRIVATE_PARSE_LITERAL"));
        fs::remove_file(root.path().join("unit.py")).unwrap();
        assert_eq!(semantics(&ast), semantics(&extract_captured(&python, &scan.sources).unwrap()));
        assert_eq!(native_json, serde_json::to_value(parse_batch(&scan.sources).unwrap()).unwrap());
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn private_frontend_exposes_grammar_and_encoding_differences_from_the_ast() {
    use super::python_native::{parse_batch, ParseStatus};

    let python = oracle();
    // The pinned grammar accepts Python 2 print syntax; CPython 3 AST does not.
    // Encoding declarations remain an explicit unsupported native boundary.
    for (bytes, ast_complete, native_status) in [
        (b"print 1\n".as_slice(), false, ParseStatus::SyntaxAccepted),
        (
            b"# coding: utf-8\ndef ready():\n    pass\n".as_slice(),
            true,
            ParseStatus::UnsupportedEncoding,
        ),
        (
            b"# coding: latin-1\ndef caf\xe9():\n    pass\n".as_slice(),
            true,
            ParseStatus::UnsupportedEncoding,
        ),
        (
            b"\r# coding: ascii\nvalue = 1\n".as_slice(),
            true,
            ParseStatus::UnsupportedEncoding,
        ),
        (
            b"\r# coding: latin-1\nvalue = '\xe9'\n".as_slice(),
            true,
            ParseStatus::UnsupportedEncoding,
        ),
    ] {
        let (_, scan) = capture(&[("unit.py", bytes)]);
        let ast = extract_captured(&python, &scan.sources).unwrap();
        let native = parse_batch(&scan.sources).unwrap();
        assert_eq!(ast.complete, ast_complete);
        assert_eq!(native.files[0].status, native_status);
        assert_eq!(native.files[0].sha256, hash(bytes));
        assert_ne!(native.identity.policy, EXTRACTOR_VERSION);
    }
}

fn assert_declaration_candidates_match_ast(python: &Path, scan: &Scan) -> Value {
    let value = assert_warning_candidates_match_ast(python, scan);
    assert_eq!(value["diagnostics"], json!([]));
    value
}

fn assert_warning_candidates_match_ast(python: &Path, scan: &Scan) -> Value {
    use super::python_native::{extract_fact_candidates, CandidateStatus};

    let ast = extract_captured(python, &scan.sources).unwrap();
    assert!(ast.complete);
    let native = extract_fact_candidates(&scan.sources).unwrap();
    assert!(
        native
            .files
            .iter()
            .all(|file| file.status == CandidateStatus::AcceptedFacts),
        "{native:?}"
    );
    // Compare the whole ordered AST vector: never filter it to supported kinds.
    assert_eq!(
        serde_json::to_value(&native.facts).unwrap(),
        serde_json::to_value(&ast.facts).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&native.diagnostics).unwrap(),
        serde_json::to_value(&ast.diagnostics).unwrap()
    );
    for (file, source) in native.files.iter().zip(&scan.sources) {
        assert!(file.diagnostic.is_none(), "{file:?}");
        assert_eq!(file.path, source.path);
        assert_eq!(file.module, source.module);
        assert_eq!(file.sha256, hash(&source.bytes));
    }
    assert_ne!(native.identity.policy, EXTRACTOR_VERSION);
    let value = serde_json::to_value(native).unwrap();
    for absent in ["complete", "schemaVersion", "snapshotId", "pythonVersion"] {
        assert!(value.get(absent).is_none());
    }
    value
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn base_annotation_candidate_bases_are_ordered_normalized_syntax_not_resolution() {
    let python = oracle();
    for text in [
        "class Box(Base): pass\n",
        "class Box(object): pass\n",
        "class Box(pkg.Base, Other,): pass\n",
        "class Box((pkg.Base), ((Other))): pass\n",
        "def outer():\n    class Box(pkg.Base):\n        class Inner(Base): pass\n",
        "from package import Base as B\nclass Box(B): pass\n",
        "class Box: pass\nclass Box(Box): pass\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
    let (_, scan) = capture(&[("unit.py", b"class Box(z.Last, A, A): pass\n")]);
    let native = assert_declaration_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (1, 1, "base", "unit.Box", "A"),
                (1, 1, "base", "unit.Box", "z.Last"),
                (1, 1, "class", "unit.Box", "Box"),
                (1, 1, "module", "unit", "unit"),
            ]
        ))
    );
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn base_annotation_candidate_annotation_only_is_not_a_write_or_type_proof() {
    let python = oracle();
    for text in [
        "value: T\nself.value: T\ncls.value: T\n",
        "def f():\n    value: T\n    self.value: T\n    cls.value: T\n",
        "class Box:\n    field: T\n    self.member: T\n    cls.member: T\n    def run(self):\n        local: T\n        self.field: T\n",
        "def outer():\n    class Box:\n        field: T\n        class Inner:\n            field: pkg.T\n    return\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        assert!(!native["facts"].as_array().unwrap().iter().any(|f| matches!(f["kind"].as_str(), Some("binding" | "binding_invalidation"))));
    }
    let (_, scan) = capture(&[("unit.py", b"class Box:\n    field: T = source\n")]);
    let native = assert_declaration_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (1, 1, "module", "unit", "unit"),
                (1, 2, "class", "unit.Box", "Box"),
                (2, 2, "attribute_annotation", "unit.Box.field", "T"),
                (
                    2,
                    2,
                    "binding",
                    "unit.Box",
                    r#"{"source":"source","target":"field"}"#
                ),
            ]
        ))
    );
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn base_annotation_candidate_rhs_contexts_and_redaction_match_complete_ast() {
    let python = oracle();
    for annotation in [
        "T",
        "pkg.T",
        "list[T]",
        "dict[str, list[T]]",
        "tuple[T, ...]",
        "'PRIVATE_ANNOTATION_LITERAL'",
        "None",
        "...",
    ] {
        for rhs in [
            "item",
            "pkg.item",
            "build(value)",
            "None",
            "-1",
            "'PRIVATE_ANNOTATION_LITERAL'",
        ] {
            let text = format!("module: {annotation} = {rhs}\nclass Box(Base):\n    field: {annotation} = {rhs}\n    self.value: {annotation} = {rhs}\n    def run(self):\n        local: {annotation} = {rhs}\n        self.member: {annotation} = {rhs}\n        cls.member: {annotation} = {rhs}\n");
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            let native = assert_declaration_candidates_match_ast(&python, &scan);
            assert!(!native.to_string().contains("PRIVATE_ANNOTATION_LITERAL"));
            let annotations = native["facts"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|f| f["kind"] == "attribute_annotation")
                .collect::<Vec<_>>();
            assert_eq!(annotations.len(), 1);
            assert_eq!(annotations[0]["symbol"], "unit.Box.field");
        }
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn base_annotation_candidate_whole_statement_and_normalized_call_spans_match_ast() {
    let python = oracle();
    for text in [
        "class Box((pkg.Base), Other,):\n    field: (T)\n    # trailing\n",
        "class Box(Base):\n    field: list[\n        tuple[T, ...]\n    ] = (\n        build(\n            value,\n        )\n    )\n    # trailing\n",
        "class Box(Base):\n    field: (\n        pkg.T # annotation comment\n    ) = (\n        value\n    )\n",
        "class Box(Base):\n    field: T[\n        (A, B)\n    ]\n",
        "class Box(Base):\n    field: T[(A, (B, None))]\n",
        "class Box(Base):\n    field: T[(A, (B, None))] = (\n        build()\n    )\n",
        "class Box(Base):\n    def run(self):\n        self.field: (\n            T\n        ) = (\n            build()\n        )\n",
        "class Box(Base): field: T = value\n",
    ] {
        for newline in ["\n", "\r\n"] {
            let source = format!("\u{feff}{}", text.replace('\n', newline));
            let (_, scan) = capture(&[("unit.py", source.as_bytes())]);
            assert_declaration_candidates_match_ast(&python, &scan);
        }
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn base_annotation_candidate_raw_field_names_and_shape_budgets_match_ast() {
    let python = oracle();
    for size in [511, 512, 513, 4096] {
        let name = "f".repeat(size);
        let text =
            format!("class Box({name}):\n    {name}: pkg.T = value\n    self.{name}: T = None\n");
        let (_, scan) = capture(&[("caf\u{e9}\u{1f642}.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        let facts = native["facts"].as_array().unwrap();
        assert!(facts.iter().any(|f| f["kind"] == "attribute_annotation"
            && f["symbol"].as_str().unwrap().ends_with(&name)));
        let binding = facts.iter().find(|f| f["kind"] == "binding").unwrap();
        let value: Value = serde_json::from_str(binding["value"].as_str().unwrap()).unwrap();
        assert_eq!(value["target"].as_str().unwrap().len(), size.min(512));
    }
    for depth in [12, 13, 24] {
        let annotation = format!("{}Item{}", "T[".repeat(depth), "]".repeat(depth));
        let base = format!("{}Root", "pkg.".repeat(depth));
        let text = format!("class Box({base}):\n    field: {annotation}\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
    for count in [16, 17, 79, 80, 81] {
        let annotation = format!("T[{}]", ["Item"].repeat(count).join(","));
        let bases = (0..count)
            .map(|n| format!("Base{n}"))
            .collect::<Vec<_>>()
            .join(",");
        let text = format!("class Box({bases}):\n    field: {annotation}\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn base_annotation_candidate_writes_do_not_erase_dynamic_alias_warnings() {
    let python = oracle();
    for text in [
        "from arbitrary import eval as load\nload: T\nload()\nload: T = None\nload()\n",
        "class Box(Base):\n    from arbitrary import eval as load\n    load: T = value\n    load()\n    def run(load):\n        load: T\n        load()\n",
        "from arbitrary import eval as load\nclass Box(Base):\n    field: T = outer(load(), load())\n    def run(self):\n        self.member: T = load()\n",
        "from ordinary import safe as eval\nclass Box(Base):\n    field: T = eval()\n",
        "class Same(Base):\n    from arbitrary import eval as load\ndef Same():\n    load: T = None\n    load()\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_warning_candidates_match_ast(&python, &scan);
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn base_annotation_candidate_unreviewed_and_metadata_trigger_forms_roll_back() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};
    let python = oracle();
    let mut forms = vec![
        "class Box(factory()): pass".to_string(),
        "class Box(T[U]): pass".into(),
        "class Box(1): pass".into(),
        "class Box((A, B)): pass".into(),
        "class Box(A | B): pass".into(),
        "class Box(*bases): pass".into(),
        "class Box(metaclass=Meta): pass".into(),
        "class Box[T](Base): pass".into(),
        "class Box(\n Base,\n): pass".into(),
        "@decorate\nclass Box(Base): pass".into(),
        "class Box(Base):\n    field: mark()".into(),
        "class Box(Base):\n    field: T[mark()]".into(),
        "class Box(Base):\n    field: T | U".into(),
        "class Box(Base):\n    field: 1".into(),
        "class Box(Base):\n    field: (T, U)".into(),
        "class Box(Base):\n    field: T[A][B]".into(),
        "class Box(Base):\n    (field): T".into(),
        "(self.field): T = value".into(),
        "other.field: T = value".into(),
        "values[0]: T = value".into(),
        "class Box(Base):\n    field\n    : T".into(),
        "class Box(Base):\n    field:\n        T".into(),
        "class Box(Base):\n    field: T\n    = value".into(),
        "class Box(Base):\n    field: T = FastAPI()".into(),
        "class Box(Base):\n    field: T = host.include_router(child)".into(),
        "self.router: T".into(),
        "self.prefix: T".into(),
        "cls.router.child: T".into(),
        "class Box(Base):\n    self.router: T".into(),
        "class Box(Base):\n    def run(self):\n        self.prefix: T".into(),
    ];
    forms.push(format!(
        "class Box(Base):\n    field: T[{}, hidden()]\n",
        ["Item"; 17].join(",")
    ));
    forms.push(format!(
        "class Box(Base, {}hidden(){}): pass",
        "(".repeat(15),
        ")".repeat(15)
    ));
    for form in forms {
        let text = format!("eval()\nclass Before: pass\n{form}\nclass After: pass\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let _ast = extract_captured(&python, &scan.sources).unwrap();
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_ne!(
            native.files[0].status,
            CandidateStatus::AcceptedFacts,
            "{form}"
        );
        assert_eq!(native.facts.len(), 1);
        assert!(native.diagnostics.is_empty());
    }
    for newline in ["\n", "\r\n"] {
        let text =
            format!("eval(){newline}class Before: pass{newline}class Box( , ): pass{newline}");
        let (_, scan) = capture(&[
            ("a.py", text.as_bytes()),
            ("b.py", b"class Good((pkg.Base),): pass\n"),
        ]);
        let ast = extract_captured(&python, &scan.sources).unwrap();
        assert!(!ast.complete);
        assert!(ast.diagnostics.iter().any(|d| d.path == "a.py"
            && d.code == "parse_failure"
            && d.message.starts_with("SyntaxError at line ")));
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_eq!(native.files[0].status, CandidateStatus::UnsupportedSyntax);
        assert_eq!(native.files[1].status, CandidateStatus::AcceptedFacts);
        assert_eq!(native.facts.iter().filter(|f| f.path == "a.py").count(), 1);
        assert!(native.diagnostics.is_empty());
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn base_annotation_candidate_replay_isolation_and_order_use_only_captured_bytes() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};
    let python = oracle();
    let (root, mut scan) = capture(&[
        ("a.py", b"class Before(Base):\n    field: T = eval()\nclass Hidden(metaclass=Meta): pass\n"),
        ("b.py", b"class Box(Base, pkg.Other):\n    field: list[T] = build()\n    def run(self):\n        self.member: T = None\n"),
        ("c.py", b"from arbitrary import eval as load\nclass Box(Base):\n    field: T = load()\n"),
    ]);
    let native = extract_fact_candidates(&scan.sources).unwrap();
    assert_eq!(native.files[0].status, CandidateStatus::UnsupportedSyntax);
    let ast_good = extract_captured(&python, &scan.sources[1..]).unwrap();
    assert!(ast_good.complete);
    let mut wanted = expected("a.py", &[(1, 1, "module", "a", "a")]);
    wanted.extend(
        serde_json::to_value(&ast_good.facts)
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .cloned(),
    );
    assert_eq!(serde_json::to_value(&native.facts).unwrap(), json!(wanted));
    assert_eq!(
        serde_json::to_value(&native.diagnostics).unwrap(),
        serde_json::to_value(&ast_good.diagnostics).unwrap()
    );
    let first = serde_json::to_value(native).unwrap();
    for path in ["a.py", "b.py", "c.py"] {
        fs::remove_file(root.path().join(path)).unwrap();
    }
    assert_eq!(
        first,
        serde_json::to_value(extract_fact_candidates(&scan.sources).unwrap()).unwrap()
    );
    scan.sources.reverse();
    let reversed = serde_json::to_value(extract_fact_candidates(&scan.sources).unwrap()).unwrap();
    assert_eq!(first["facts"], reversed["facts"]);
    assert_eq!(first["diagnostics"], reversed["diagnostics"]);
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn class_candidate_plain_declarations_and_methods_match_every_ast_fact() {
    let python = oracle();
    for text in [
        "class Box: pass\n", "class Box(): pass\n", "class Box( ): pass\n",
        "class Box:\n    'PRIVATE_CLASS_LITERAL'\n",
        "class Box:\n    def run(self): pass\n",
        "class Box:\n    async def run(cls): pass\n",
        "class Worker:\n    def __init__(self, service):\n        self.service = service\n    def run(self):\n        return self.service.process()\n",
        "class Box:\n    def run(): pass\n    def __init__(cls): pass\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        assert!(!native.to_string().contains("PRIVATE_CLASS_LITERAL"));
        assert!(!native["facts"].as_array().unwrap().iter().any(|f| f["kind"] == "method"));
    }
    let (_, scan) = capture(&[("unit.py", b"class Box:\n    def run(self):\n        pass\n")]);
    let native = assert_declaration_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (1, 1, "module", "unit", "unit"),
                (1, 3, "class", "unit.Box", "Box"),
                (2, 2, "parameter", "unit.Box.run.self", "self"),
                (2, 3, "function", "unit.Box.run", "run"),
                (
                    2,
                    3,
                    "signature",
                    "unit.Box.run",
                    r#"{"keywordOnly":[],"positional":["self"],"positionalOnly":0}"#
                ),
            ]
        ))
    );
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn class_candidate_method_headers_and_long_metadata_preserve_ast_limits() {
    let python = oracle();
    for text in [
        "class Box:\n    def run(self, value: package.Item=None, /, *rest: tuple[str, ...], required: T, optional: R=1, **opts: object) -> list[Result]:\n        return self.work(value)\n",
        "class Box:\n    async def run(\n        self, value: T=1, /,\n        *, flag: bool=False,\n    ) -> Result:\n        pass\n",
        "class Box:\n    def run(self, cls, /, *args, **kwargs):\n        cls.value = self.value\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
    for count in [128, 129] {
        let parameters = (0..count)
            .map(|n| format!("p{n}"))
            .collect::<Vec<_>>()
            .join(",");
        let text = format!("class Box:\n    def run({parameters}): pass\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        let signature = native["facts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["kind"] == "signature")
            .unwrap();
        assert_eq!(signature["value"] == r#"{"truncated":true}"#, count > 128);
    }
    for size in [511, 512, 513, 4096] {
        let name = "C".repeat(size);
        let text = format!("class {name}:\n    def run(self):\n        self.work()\n");
        let (_, mut scan) = capture(&[("caf\u{e9}\u{1f642}.py", text.as_bytes())]);
        scan.sources[0].module = "odd.\u{7f}\n\"\\\u{1f642}".into();
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        assert!(native["facts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["kind"] == "class" && f["value"] == name));
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn class_candidate_mixed_lexical_owners_and_class_writes_match_complete_ast() {
    let python = oracle();
    let text = concat!(
        "def outer():\n",
        "    class Box:\n",
        "        import tools\n",
        "        field = tools.build('PRIVATE_CLASS_LITERAL')\n",
        "        peer = dependency\n",
        "        enabled = None\n",
        "        self.member = dependency\n",
        "        tools.use(field)\n",
        "        class Inner:\n",
        "            local = work()\n",
        "            def run(self):\n",
        "                self.call()\n",
        "                def nested():\n",
        "                    class Local:\n",
        "                        value = prepare()\n",
        "                        def method(self):\n",
        "                            return finish()\n",
        "                    return create()\n",
        "                return nested()\n",
        "        final = complete()\n",
        "    return after()\n",
        "module_work()\n",
    );
    let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
    let native = assert_declaration_candidates_match_ast(&python, &scan);
    assert!(!native.to_string().contains("PRIVATE_CLASS_LITERAL"));
    let facts = native["facts"].as_array().unwrap();
    for (line, owner) in [
        (4, "unit.outer.Box"),
        (8, "unit.outer.Box"),
        (10, "unit.outer.Box.Inner"),
        (12, "unit.outer.Box.Inner.run"),
        (15, "unit.outer.Box.Inner.run.nested.Local"),
        (17, "unit.outer.Box.Inner.run.nested.Local.method"),
        (18, "unit.outer.Box.Inner.run.nested"),
        (20, "unit.outer.Box"),
        (21, "unit.outer"),
        (22, "unit"),
    ] {
        assert!(
            facts
                .iter()
                .any(|f| f["kind"] == "call" && f["line"] == line && f["symbol"] == owner),
            "line {line}"
        );
    }
    assert!(!facts.iter().any(|f| f["kind"] == "attribute_annotation"));
    assert!(!facts
        .iter()
        .any(|f| f["kind"] == "signature" && f["symbol"] == "unit.outer.Box"));
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn class_candidate_statement_ends_comments_and_normalized_spans_match_ast() {
    let python = oracle();
    for text in [
        "class Box(): # header comment\n\n    # before first statement\n    pass\n",
        "class Box: '''PRIVATE_CLASS_LITERAL\nsecond line\nlast line''' # trailing\n# final\n",
        "class Box:\n    pass\n    # trailing\n# outside\n",
        "class Box:\n    'PRIVATE_CLASS_LITERAL'\n    # trailing\n",
        "class Box:\n    value\n    # trailing\n",
        "class Box:\n    class Inner:\n        def run(self):\n            return\n            # trailing method\n        # trailing inner\n    # trailing outer\n",
        "class Box:\n    from .package import (\n        item,\n        # within pair\n    )\n    # trailing\n",
        "class Box:\n    value = (\n        create(\n            'PRIVATE_CLASS_LITERAL',\n        )\n    )\n    # trailing\n",
        "class Box:\n    def run(self): pass\n    final()\nclass Sibling:\n    pass\n",
    ] {
        for newline in ["\n", "\r\n"] {
            let source = format!("\u{feff}{}", text.replace('\n', newline));
            let (_, scan) = capture(&[("unit.py", source.as_bytes())]);
            assert_declaration_candidates_match_ast(&python, &scan);
        }
    }
    let (_, scan) = capture(&[("unit.py", b"def outer():\n    class Box:\n        from .pkg import (\n            item,\n        )\n        # trailing\n# final\n")]);
    let native = assert_declaration_candidates_match_ast(&python, &scan);
    for f in native["facts"].as_array().unwrap().iter().filter(|f| {
        matches!(
            f["kind"].as_str(),
            Some("class" | "function" | "signature" | "from_import")
        )
    }) {
        assert_eq!(f["endLine"], 5);
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn class_candidate_alias_prefixes_repeated_and_cross_kind_scopes_match_ast() {
    let python = oracle();
    for (text, count) in [
        ("class Box:\n    from arbitrary import eval as load\n    def run(self):\n        load()\n", 1),
        ("from arbitrary import eval as load\nclass Box:\n    from ordinary import safe as load\n    class Inner:\n        def run(self):\n            load()\ndef outside():\n    load()\n", 1),
        ("def outer():\n    from arbitrary import eval as load\n    class Box:\n        def run(self):\n            load()\n", 1),
        ("class Box:\n    def first(self):\n        from arbitrary import eval as load\n    def second(self):\n        load()\n", 0),
        ("class Box:\n    from arbitrary import eval as load\nclass Box:\n    def run(load):\n        load = None\n        load()\n", 1),
        ("class Box:\n    from arbitrary import eval as load\ndef Box():\n    load()\n", 1),
        ("def Box():\n    from arbitrary import eval as load\nclass Box:\n    load()\n", 1),
        ("class Box:\n    def run(self):\n        from arbitrary import eval as load\nclass Box:\n    async def run(self):\n        load()\n", 1),
        ("class Box:\n    from arbitrary import eval as load\n    from .ordinary import safe as load\n    load()\nload()\n", 1),
        ("from arbitrary import eval as load\nclass Parent:\n    from ordinary import safe as load\n    class First:\n        from arbitrary import exec as load\n        load()\n    load()\n    def second():\n        load()\n        from arbitrary import eval as load\n        load()\n    load()\n    class Third:\n        load()\nload()\n", 3),
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_warning_candidates_match_ast(&python, &scan);
        assert_eq!(native["diagnostics"].as_array().unwrap().len(), count, "{text}");
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn class_candidate_excluded_headers_and_suite_returns_rollback_whole_files() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};
    let python = oracle();
    for unsupported in [
        "class Box(Base[T]): pass",
        "class Box(object()): pass",
        "class Box(*bases): pass",
        "class Box(metaclass=Meta): pass",
        "class Box(**options): pass",
        "class Box[T]: pass",
        "class Box(factory()): pass",
        "@decorate\nclass Box: pass",
        "class Box:\n    @staticmethod\n    def run(): pass",
        "class Box:\n    @classmethod\n    def run(cls): pass",
        "class Box:\n    @property\n    def value(self): pass",
        "class Box:\n    value: T | U",
        "class Box:\n    value: T | U = build()",
        "class Box:\n    def run(self):\n        self.value: T | U = value",
        "class Box:\n    return work()",
        "def outer():\n    class Box:\n        return work()",
        "class Box:\n    def run(self):\n        class Local:\n            return work()",
        "class Box:\n    app = FastAPI()",
        "class Box:\n    from arbitrary import APIRouter as R",
        "class Box:\n    self.router = value",
        "class Box:\n    def run(self):\n        self.prefix = None",
        "class Box:\n    host.include_router(child)",
        "class Box:\n    if True: pass",
        "class Box:\n    def run(self):\n        await work()",
        "class Box(\n): pass",
        "class\nBox: pass",
        "class \\\nBox: pass",
        "class Box: pass; work()",
        "class Box:\n\tpass",
        "class \u{212a}: pass",
    ] {
        let text = format!("eval()\nclass Before: pass\n{unsupported}\nclass After: pass\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let _ast = extract_captured(&python, &scan.sources).unwrap();
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_ne!(
            native.files[0].status,
            CandidateStatus::AcceptedFacts,
            "{unsupported}"
        );
        assert_eq!(
            serde_json::to_value(&native.facts).unwrap(),
            json!(expected("unit.py", &[(1, 1, "module", "unit", "unit")]))
        );
        assert!(native.diagnostics.is_empty());
    }
    // ast.parse is not compilation validation: class-suite return rejection is
    // the candidate's deliberate function-only policy, not an AST syntax claim.
    let (_, scan) = capture(&[(
        "unit.py",
        b"def outer():\n    class Box:\n        return work()\n",
    )]);
    assert!(extract_captured(&python, &scan.sources).unwrap().complete);
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn class_candidate_replay_transactions_and_source_order_preserve_complete_semantics() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};
    let python = oracle();
    let (root, mut scan) = capture(&[
        ("a.py", b"class Before: pass\neval()\nclass Hidden(metaclass=Meta): pass\n"),
        ("b.py", b"class Worker:\n    def __init__(self, service):\n        self.service = service\n    def run(self):\n        return self.service.process()\n"),
        ("c.py", b"class Box:\n    from arbitrary import eval as load\n    def run(self):\n        load()\n"),
    ]);
    let native = extract_fact_candidates(&scan.sources).unwrap();
    assert_eq!(native.files[0].status, CandidateStatus::UnsupportedSyntax);
    let ast_good = extract_captured(&python, &scan.sources[1..]).unwrap();
    assert!(ast_good.complete);
    let mut wanted = expected("a.py", &[(1, 1, "module", "a", "a")]);
    wanted.extend(
        serde_json::to_value(&ast_good.facts)
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .cloned(),
    );
    assert_eq!(serde_json::to_value(&native.facts).unwrap(), json!(wanted));
    assert_eq!(
        serde_json::to_value(&native.diagnostics).unwrap(),
        serde_json::to_value(&ast_good.diagnostics).unwrap()
    );
    let first = serde_json::to_value(native).unwrap();
    for path in ["a.py", "b.py", "c.py"] {
        fs::remove_file(root.path().join(path)).unwrap();
    }
    assert_eq!(
        first,
        serde_json::to_value(extract_fact_candidates(&scan.sources).unwrap()).unwrap()
    );
    scan.sources.reverse();
    let reversed = serde_json::to_value(extract_fact_candidates(&scan.sources).unwrap()).unwrap();
    assert_eq!(first["facts"], reversed["facts"]);
    assert_eq!(first["diagnostics"], reversed["diagnostics"]);
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn alias_warning_candidate_direct_and_imported_terminals_match_complete_ast() {
    let python = oracle();
    for terminal in [
        "__import__",
        "import_module",
        "iter_modules",
        "walk_packages",
        "entry_points",
        "load_entry_point",
        "find_spec",
        "exec",
        "eval",
    ] {
        for text in [
            format!("{terminal}('PRIVATE_WARNING_LITERAL')\n"),
            format!("value = arbitrary.{terminal}('PRIVATE_WARNING_LITERAL')\n"),
            format!("from arbitrary import {terminal} as load\ndef f():\n    return load('PRIVATE_WARNING_LITERAL')\n"),
            format!("import arbitrary.{terminal} as load\nload('PRIVATE_WARNING_LITERAL')\n"),
            format!("from arbitrary import (\n    ordinary, {terminal} as load,\n)\nload('PRIVATE_WARNING_LITERAL')\n"),
        ] {
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            let native = assert_warning_candidates_match_ast(&python, &scan);
            assert_eq!(native["diagnostics"].as_array().unwrap().len(), 1, "{text}");
            assert!(!native.to_string().contains("PRIVATE_WARNING_LITERAL"));
        }
    }
    let (_, scan) = capture(&[(
        "unit.py",
        b"from importlib import import_module as load\nload('PRIVATE_WARNING_LITERAL')\n",
    )]);
    let native = assert_warning_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["diagnostics"],
        json!([{
            "code": "dynamic_discovery_unresolved", "path": "unit.py",
            "message": "line 2: importlib.import_module; runtime targets are not inferred"
        }])
    );
    assert!(native["files"][0]["diagnostic"].is_null());
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn alias_warning_candidate_overwrite_suppression_and_one_substitution_match_ast() {
    let python = oracle();
    for (text, count) in [
        ("from ordinary import safe as eval\neval()\n", 0),
        ("from ordinary import safe as local\nfrom arbitrary import eval as local\nlocal()\n", 1),
        ("from arbitrary import eval as local\nfrom ordinary import safe as local\nlocal()\n", 0),
        ("from arbitrary import eval as local, safe as local\nlocal()\n", 0),
        ("from arbitrary import safe as local, eval as local\nlocal()\n", 1),
        ("from arbitrary import eval as local\nfrom .ordinary import safe as local\nlocal()\n", 1),
        ("from .arbitrary import eval as local\nlocal()\n", 0),
        ("from arbitrary import eval as ordinary\nimport ordinary as local\nordinary()\nlocal()\n", 1),
        ("import arbitrary.eval\narbitrary()\n", 0),
        ("import arbitrary.eval as local\nlocal()\n", 1),
        ("import eval.child\neval()\n", 1),
        ("load()\nfrom arbitrary import eval as load\nload()\nfrom ordinary import safe as load\nload()\n", 1),
        ("from arbitrary import eval as load\nother = load\nother()\n", 0),
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_warning_candidates_match_ast(&python, &scan);
        assert_eq!(native["diagnostics"].as_array().unwrap().len(), count, "{text}");
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn alias_warning_candidate_scope_persistence_and_nonshadowing_match_ast() {
    let python = oracle();
    for (text, count) in [
        ("from arbitrary import eval as load\ndef f(load, *args, **opts):\n    load = None\n    load()\n    def load(): pass\n    load()\n", 2),
        ("from arbitrary import eval as load\ndef outer():\n    from ordinary import safe as load\n    def inner():\n        load()\n    load()\ndef sibling():\n    load()\n", 1),
        ("def f():\n    from arbitrary import eval as load\ndef f(load):\n    load()\ndef sibling():\n    load()\n", 1),
        ("def outer():\n    def inner():\n        from arbitrary import eval as load\ndef outer():\n    def inner():\n        load()\n    load()\n", 1),
        ("async def f():\n    from arbitrary import eval as load\ndef f():\n    load()\n", 1),
        ("def earlier():\n    load()\nfrom arbitrary import eval as load\ndef later():\n    load()\n", 1),
        ("from arbitrary import eval as load\ndef f():\n    load()\n    from ordinary import safe as load\n    load()\ndef f():\n    load()\n", 1),
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_warning_candidates_match_ast(&python, &scan);
        assert_eq!(native["diagnostics"].as_array().unwrap().len(), count, "{text}");
    }
    for module in ["", "a.b", "a.\"\\\n\u{1f642}", "\u{7f}\u{3bb}"] {
        let (_, mut scan) = capture(&[(
            "caf\u{e9}\u{1f642}.py",
            b"def f():\n    from arbitrary import eval as load\ndef f():\n    load()\n",
        )]);
        scan.sources[0].module = module.into();
        assert_warning_candidates_match_ast(&python, &scan);
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn alias_warning_candidate_callee_clipping_and_full_alias_targets_match_ast() {
    let python = oracle();
    for size in [511, 512, 513, 4096] {
        let name = "a".repeat(size);
        for text in [
            format!("from {name}.arbitrary import eval as load\nload()\n"),
            format!("import {name}.eval as load\nload()\n"),
            format!("from arbitrary import eval as {name}\n{name}()\n"),
            format!("from ordinary import safe as {name}\n{name}.eval()\n"),
        ] {
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            assert_warning_candidates_match_ast(&python, &scan);
        }
    }
    let head = "a".repeat(512);
    for text in [
        format!("from arbitrary import eval as {head}\n{head}Extra()\n"),
        format!("from ordinary import safe as {head}\n{head}Extra()\n"),
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_warning_candidates_match_ast(&python, &scan);
    }
    for terminal in ["eval", "import_module"] {
        let prefix = "a".repeat(512 - 1 - terminal.len());
        let text = format!(
            "from ordinary import safe as {prefix}\nouter({}, {prefix}.{terminal}Extra())\n",
            ["value"; 17].join(",")
        );
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_warning_candidates_match_ast(&python, &scan);
        assert_eq!(native["diagnostics"].as_array().unwrap().len(), 1);
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn alias_warning_candidate_hidden_calls_duplicates_and_lexical_sort_match_ast() {
    let python = oracle();
    let mut expressions = vec![
        "outer(eval(), eval())".to_string(),
        format!(
            "outer({}, eval('PRIVATE_WARNING_LITERAL'))",
            ["value"; 17].join(",")
        ),
        format!(
            "outer({}, late=eval())",
            (0..17)
                .map(|n| format!("k{n}=value"))
                .collect::<Vec<_>>()
                .join(",")
        ),
        format!("{}eval(){}", "nest(".repeat(15), ")".repeat(15)),
    ];
    let exhausted = (0..16)
        .map(|n| format!("g{n}({})", ["value"; 16].join(",")))
        .collect::<Vec<_>>()
        .join(",");
    expressions.push(format!("outer({exhausted}, eval())"));
    for expression in expressions {
        let text = format!("def f():\n    return {expression}\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_warning_candidates_match_ast(&python, &scan);
        assert!(!native["diagnostics"].as_array().unwrap().is_empty());
        assert!(!native.to_string().contains("PRIVATE_WARNING_LITERAL"));
    }
    let text = "pass\neval()\npass\npass\npass\npass\npass\npass\npass\nouter(eval(), eval())\n";
    let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
    let native = assert_warning_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["diagnostics"],
        json!([
            {"code":"dynamic_discovery_unresolved","path":"unit.py","message":"line 10: eval; runtime targets are not inferred"},
            {"code":"dynamic_discovery_unresolved","path":"unit.py","message":"line 10: eval; runtime targets are not inferred"},
            {"code":"dynamic_discovery_unresolved","path":"unit.py","message":"line 2: eval; runtime targets are not inferred"},
        ])
    );
    assert_eq!(
        native["facts"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| f["kind"] == "call" && f["line"] == 10 && f["value"] == "eval()")
            .count(),
        1
    );
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn alias_warning_candidate_rejections_erase_pending_warnings_not_other_files() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};
    let python = oracle();
    for suffix in [
        "class Hidden(metaclass=Meta): pass\n",
        "from arbitrary import FastAPI as factory\n",
        "FastAPI()\n",
        "other.include_router(child)\n",
        "def f(x: eval()): pass\n",
        "from package import *\n",
        "def broken(:\n",
    ] {
        let text = format!("from arbitrary import eval as load\nload()\n{suffix}");
        let (_, scan) = capture(&[("a.py", text.as_bytes()), ("b.py", b"eval()\n")]);
        let ast_good = extract_captured(&python, &scan.sources[1..]).unwrap();
        let _ast_bad = extract_captured(&python, &scan.sources[..1]).unwrap();
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_ne!(native.files[0].status, CandidateStatus::AcceptedFacts);
        assert_eq!(native.files[1].status, CandidateStatus::AcceptedFacts);
        assert_eq!(native.facts.iter().filter(|f| f.path == "a.py").count(), 1);
        assert_eq!(
            serde_json::to_value(&native.diagnostics).unwrap(),
            serde_json::to_value(&ast_good.diagnostics).unwrap()
        );
        assert_eq!(
            serde_json::to_value(
                native
                    .facts
                    .iter()
                    .filter(|f| f.path == "b.py")
                    .collect::<Vec<_>>()
            )
            .unwrap(),
            serde_json::to_value(&ast_good.facts).unwrap()
        );
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn alias_warning_candidate_constructor_guards_remain_conservative() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};
    let python = oracle();
    for terminal in ["FastAPI", "APIRouter"] {
        let prefix = "a".repeat(512 - 1 - terminal.len());
        for text in [
            format!("from arbitrary import {terminal} as unused\n"),
            format!("from ordinary import safe as {terminal}\n{terminal}()\n"),
            format!("{prefix}.{terminal}Extra()\n"),
        ] {
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            assert!(extract_captured(&python, &scan.sources).unwrap().complete);
            let native = extract_fact_candidates(&scan.sources).unwrap();
            assert_eq!(native.files[0].status, CandidateStatus::UnsupportedSyntax);
            assert_eq!(native.facts.len(), 1);
            assert!(native.diagnostics.is_empty());
        }
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn alias_warning_candidate_replay_and_input_order_preserve_full_semantics() {
    use super::python_native::extract_fact_candidates;
    let python = oracle();
    let (root, mut scan) = capture(&[
        ("a.py", b"from arbitrary import eval as load\nload()\ndef f():\n    from ordinary import safe as load\n    load()\n"),
        ("b.py", b"load()\nfrom arbitrary import import_module as load\nouter(load(), load())\n"),
        ("caf\u{e9}\u{1f642}.py", b"from .arbitrary import eval as load\nload()\neval()\n"),
    ]);
    let first = assert_warning_candidates_match_ast(&python, &scan);
    for path in ["a.py", "b.py", "caf\u{e9}\u{1f642}.py"] {
        fs::remove_file(root.path().join(path)).unwrap();
    }
    assert_eq!(first, assert_warning_candidates_match_ast(&python, &scan));
    scan.sources.reverse();
    let reversed = serde_json::to_value(extract_fact_candidates(&scan.sources).unwrap()).unwrap();
    assert_eq!(first["facts"], reversed["facts"]);
    assert_eq!(first["diagnostics"], reversed["diagnostics"]);
    assert_warning_candidates_match_ast(&python, &scan);
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn import_candidate_metadata_and_spelling_match_every_ast_fact() {
    let python = oracle();
    for text in [
        "import alpha\n",
        "import alpha.beta\n",
        "import alpha.beta as ab, gamma, alpha.beta as ab\n",
        "from alpha.beta import Item, run as execute, Item\n",
        "from . import Item\n",
        "from ..alpha import Item as Local\n",
        "from ....alpha.beta import Item, Other\n",
        "import  alpha  .  beta  as  ab  ,  gamma\n",
        "from . . . . alpha . beta import Item as Local\n",
        "from...alpha import Item\n",
        "from.import Item\n",
        "import match as case, type, print\n",
        "\u{feff}import alpha.beta as ab # comment \u{3bb}\r\nfrom ..alpha import Item\r\n",
        "import alpha # trailing comment, not another name\n# final\n",
        "import alpha",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
    for level in [9, 10, 99, 100] {
        for module in ["", "package"] {
            let text = format!("from {}{module} import Item as Local\n", ".".repeat(level));
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            assert_declaration_candidates_match_ast(&python, &scan);
        }
    }
    for count in [4, 5, 16, 17, 128, 129] {
        let names = (0..count)
            .map(|n| format!("item{n} as local{n}"))
            .collect::<Vec<_>>()
            .join(", ");
        let text = format!("from package import {names}\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
    // Independent exact metadata/ID check, not a native-generated expectation.
    let (_, scan) = capture(&[("unit.py", b"import alpha.beta as ab, gamma\n")]);
    assert_facts(
        &extract_captured(&python, &scan.sources).unwrap(),
        "unit.py",
        &[
            (
                1,
                1,
                "import",
                "unit",
                r#"{"level":0,"module":"","names":[{"asname":"ab","name":"alpha.beta"},{"asname":null,"name":"gamma"}]}"#,
            ),
            (1, 1, "module", "unit", "unit"),
        ],
    );
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn import_candidate_scopes_headers_writes_and_calls_keep_exact_shapes() {
    let python = oracle();
    for text in [
        "import tools as t\nt.run('PRIVATE_IMPORT_LITERAL')\nvalue = t.build()\n",
        "from package import Item as Local\ndef f(value: Local=None) -> Local:\n    from .local import helper as run\n    return run(value)\n",
        "def outer():\n    import tools\n    def inner():\n        from ....pkg import item as local\n        # trailing inner\n    from . import sibling\n    # trailing outer\n# final\n",
        "from first import run as local\ndef f(local, *rest, **opts):\n    local = None\n    local()\n    from second import run as local\n    local()\ndef f():\n    local()\n    from .third import other as local\n    return local()\n",
        "from first import run as local, other as local\nlocal()\ndef sibling():\n    from second import run as local\n    local()\ndef other():\n    local()\n",
        "from package import safe as eval\nfrom package import safe as FastAPI\nfrom package import include_router as include\ninclude()\n",
        "from ordinary import safe as a\nfrom a import safe as b\nb()\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        assert!(!native.to_string().contains("PRIVATE_IMPORT_LITERAL"));
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn import_candidate_absolute_constructor_aliases_reject_even_unused_targets() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};

    let python = oracle();
    for terminal in ["FastAPI", "APIRouter"] {
        for statement in [
            format!("from package import safe, {terminal} as hidden, other"),
            format!("import safe, package.{terminal} as hidden, other"),
            format!("import {terminal}.child"),
            format!("def nested(hidden):\n    from package import {terminal} as hidden\n    hidden = None\n    return hidden('PRIVATE_IMPORT_LITERAL')"),
        ] {
            let text = format!("import ordinary\ndef before(): pass\n{statement}\ndef after(): pass\n");
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            assert!(extract_captured(&python, &scan.sources).unwrap().complete);
            let native = extract_fact_candidates(&scan.sources).unwrap();
            assert_eq!(native.files[0].status, CandidateStatus::UnsupportedSyntax, "{text}");
            assert_eq!(serde_json::to_value(&native.facts).unwrap(),
                json!(expected("unit.py", &[(1, 1, "module", "unit", "unit")])));
        }
    }
    // Constructor side facts are still out of scope, including conservative
    // rejection when an ordinary imported target suppresses the raw fallback.
    for (text, extra_kind, diagnostics) in [
        (
            "from fastapi import APIRouter as R\nx = R()\n",
            Some("router_prefix"),
            true,
        ),
        (
            "from fastapi import FastAPI as F\nx = F()\n",
            Some("router_host"),
            true,
        ),
        // Ordinary imported targets suppress helper spelling fallbacks, but the
        // deliberately conservative raw callee guard still rejects these files.
        (
            "from ordinary import safe as FastAPI\nx = FastAPI()\n",
            None,
            true,
        ),
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let ast = extract_captured(&python, &scan.sources).unwrap();
        assert!(ast.complete);
        assert_eq!(ast.diagnostics.is_empty(), diagnostics);
        if let Some(kind) = extra_kind {
            assert!(ast.facts.iter().any(|f| f.kind == kind));
        }
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_eq!(native.files[0].status, CandidateStatus::UnsupportedSyntax);
        assert_eq!(native.facts.len(), 1);
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn import_candidate_relative_and_plain_dotted_alias_targets_stay_neutral() {
    let python = oracle();
    for terminal in [
        "FastAPI",
        "APIRouter",
        "__import__",
        "import_module",
        "iter_modules",
        "walk_packages",
        "entry_points",
        "load_entry_point",
        "find_spec",
        "exec",
        "eval",
    ] {
        for text in [
            format!("from .package import {terminal} as local\nlocal()\n"),
            format!("from ... import {terminal} as local\nlocal()\n"),
            format!("import package.{terminal}\npackage()\n"),
            format!("import package.{terminal}.ordinary as local\nlocal()\n"),
            format!("from ordinary import safe as local\nfrom .package import {terminal} as local\nlocal()\n"),
        ] {
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            assert_declaration_candidates_match_ast(&python, &scan);
        }
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn import_candidate_unclipped_targets_and_clipped_callees_do_not_hide_side_outputs() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};

    let python = oracle();
    for size in [511, 512, 513, 4096] {
        let name = "a".repeat(size);
        for text in [
            format!("import {name}.ordinary as local\nlocal()\n"),
            format!("from {name}.package import ordinary as local\nlocal()\n"),
            format!("from package import ordinary as {name}\n{name}()\n"),
        ] {
            let (_, scan) = capture(&[("caf\u{e9}\u{1f642}.py", text.as_bytes())]);
            assert_declaration_candidates_match_ast(&python, &scan);
        }
        for text in [
            format!("from {name}.package import APIRouter as local\n"),
            format!("import {name}.FastAPI as local\n"),
        ] {
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            assert!(extract_captured(&python, &scan.sources).unwrap().complete);
            let native = extract_fact_candidates(&scan.sources).unwrap();
            assert_eq!(native.files[0].status, CandidateStatus::UnsupportedSyntax);
            assert_eq!(native.facts.len(), 1);
        }
    }
    for terminal in ["FastAPI", "APIRouter"] {
        let prefix = "a".repeat(512 - 1 - terminal.len());
        let text = format!(
            "import {prefix} as harmless\nouter({}, {prefix}.{terminal}Extra())\n",
            ["value"; 17].join(",")
        );
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert!(extract_captured(&python, &scan.sources).unwrap().complete);
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_eq!(native.files[0].status, CandidateStatus::UnsupportedSyntax);
        assert_eq!(native.facts.len(), 1);
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn import_candidate_unreviewed_grammar_rolls_back_before_and_after_facts() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};

    let python = oracle();
    for unsupported in [
        "from package import *",
        "from package import ((item))",
        "from package import (\n    *,\n)",
        "import package,",
        "from package import item,",
        "from package import item.child",
        "from __future__ import annotations",
        "def f(): import package",
        "import package; ordinary()",
        "import\tpackage",
        "import package\u{c}",
        "import \u{212a}",
        "from package import \u{212a}",
        "import package \\\n .child",
        "from package \\\n import item",
        "from package import item # type: ignore",
        "import package as class",
        "from package import True",
        "from package\nimport item",
    ] {
        let text = format!("import safe\ndef before(): pass\n{unsupported}\ndef after(): pass\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let _ast = extract_captured(&python, &scan.sources).unwrap();
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_ne!(
            native.files[0].status,
            CandidateStatus::AcceptedFacts,
            "{text}"
        );
        assert_eq!(native.facts.len(), 1);
    }
    let (_, scan) = capture(&[(
        "unit.py",
        b"import safe\nfrom package import *\ndef broken(:\n",
    )]);
    assert!(!extract_captured(&python, &scan.sources).unwrap().complete);
    let native = extract_fact_candidates(&scan.sources).unwrap();
    assert_eq!(native.files[0].status, CandidateStatus::SyntaxError);
    assert_eq!(native.facts.len(), 1);
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn import_candidate_transactions_order_and_replay_use_only_captured_bytes() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};

    let python = oracle();
    let (root, mut scan) = capture(&[
        ("a.py", b"import safe\ndef before(): pass\nfrom package import FastAPI as local\n"),
        ("b.py", b"from .package import eval as local\nlocal()\ndef f():\n    import tools\n    return tools.run()\n"),
    ]);
    let native = extract_fact_candidates(&scan.sources).unwrap();
    assert_eq!(native.files[0].status, CandidateStatus::UnsupportedSyntax);
    assert_eq!(native.files[1].status, CandidateStatus::AcceptedFacts);
    let ast = extract_captured(&python, &scan.sources[1..]).unwrap();
    assert!(ast.complete && ast.diagnostics.is_empty());
    let mut wanted = expected("a.py", &[(1, 1, "module", "a", "a")]);
    wanted.extend(
        serde_json::to_value(&ast.facts)
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .cloned(),
    );
    assert_eq!(serde_json::to_value(&native.facts).unwrap(), json!(wanted));
    let first = serde_json::to_value(native).unwrap();
    fs::remove_file(root.path().join("a.py")).unwrap();
    fs::remove_file(root.path().join("b.py")).unwrap();
    assert_eq!(
        first,
        serde_json::to_value(extract_fact_candidates(&scan.sources).unwrap()).unwrap()
    );
    scan.sources.reverse();
    assert_eq!(
        first["facts"],
        serde_json::to_value(extract_fact_candidates(&scan.sources).unwrap()).unwrap()["facts"]
    );
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn multiline_import_candidate_spelling_metadata_and_trailing_comma_match_ast() {
    let python = oracle();
    for text in [
        "from package import (item)\n",
        "from package import (item, other)\n",
        "from package import (item,)\n",
        "from alpha.beta import (Item, run as execute, Item,)\n",
        "from . import (Item,)\n",
        "from ..alpha import (Item as Local)\n",
        "from . . . . alpha . beta import ( Item  as  Local , Other , )\n",
        "from...alpha import(Item,)\n",
        "from.import(Item)",
        "from package import (match as case, type, print)\n",
        "from package import (safe as eval, other as FastAPI)\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
    let (_, scan) = capture(&[(
        "unit.py",
        b"from ..alpha.beta import (\n    Item as Local,\n    Other, Item,\n) # closing comment\n",
    )]);
    let native = assert_declaration_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (1, 1, "module", "unit", "unit"),
                (
                    1,
                    4,
                    "from_import",
                    "unit",
                    r#"{"level":2,"module":"alpha.beta","names":[{"asname":"Local","name":"Item"},{"asname":null,"name":"Other"},{"asname":null,"name":"Item"}]}"#
                ),
            ]
        ))
    );
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn multiline_import_candidate_comments_gaps_crlf_bom_and_scoped_spans_match_ast() {
    let python = oracle();
    for text in [
        "from package import (\n\n # punctuation () , . ; \\ * ' \" \t\u{c} \u{3bb}\u{1f642}\n item # name\n as # alias keyword\n local, # separator\n\n other\n # before closing\n) # after closing\n",
        "from package import (item\nas\nlocal\n,\nother\n)\n",
        "from package import (\n item # ordinary type punctuation: not a directive\n)\n",
        "def f(value: T=None) -> R:\n    from .package import (\n        helper as run,\n    )\n    return run(value)\n",
        "from package import (safe as local,)\nlocal()\ndef f(local):\n    from .package import (eval as local,)\n    local = None\n    local()\n",
    ] {
        for newline in ["\n", "\r\n"] {
            let source = format!("\u{feff}{}", text.replace('\n', newline));
            let (_, scan) = capture(&[("unit.py", source.as_bytes())]);
            assert_declaration_candidates_match_ast(&python, &scan);
        }
    }
    // The pinned grammar rejects a CPython-valid dedent inside this nested
    // import. Preserve that difference explicitly; never waive a CST error.
    let (_, scan) = capture(&[(
        "unit.py",
        b"def f():\n    from .package import (\nitem,\n    )\n    item()\n",
    )]);
    let ast = extract_captured(&python, &scan.sources).unwrap();
    assert!(ast.complete && ast.diagnostics.is_empty());
    let native = super::python_native::extract_fact_candidates(&scan.sources).unwrap();
    assert_eq!(
        native.files[0].status,
        super::python_native::CandidateStatus::SyntaxError
    );
    assert_eq!(native.facts.len(), 1);
    let (_, scan) = capture(&[(
        "unit.py",
        b"def outer():\n    def inner():\n        from .pkg import (\n            item, # name\n            # tail within pair\n        ) # closing\n        # trailing inner\n    # trailing outer\n# final\n",
    )]);
    let native = assert_declaration_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (1, 1, "module", "unit", "unit"),
                (1, 6, "function", "unit.outer", "outer"),
                (
                    1,
                    6,
                    "signature",
                    "unit.outer",
                    r#"{"keywordOnly":[],"positional":[],"positionalOnly":0}"#
                ),
                (2, 6, "function", "unit.outer.inner", "inner"),
                (
                    2,
                    6,
                    "signature",
                    "unit.outer.inner",
                    r#"{"keywordOnly":[],"positional":[],"positionalOnly":0}"#
                ),
                (
                    3,
                    6,
                    "from_import",
                    "unit.outer.inner",
                    r#"{"level":1,"module":"pkg","names":[{"asname":null,"name":"item"}]}"#
                ),
            ]
        ))
    );
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn multiline_import_candidate_constructor_targets_rollback_relative_stays_neutral() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};

    let python = oracle();
    let safe = (0..17).map(|n| format!("safe{n},\n")).collect::<String>();
    for terminal in ["FastAPI", "APIRouter"] {
        for alias in ["", " as hidden"] {
            let text = format!(
                "import ordinary\ndef before(): pass\nfrom package import (\n{safe}{terminal}{alias},\n)\ndef after(): pass\n"
            );
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            let ast = extract_captured(&python, &scan.sources).unwrap();
            assert!(ast.complete && ast.diagnostics.is_empty());
            let native = extract_fact_candidates(&scan.sources).unwrap();
            assert_eq!(
                native.files[0].status,
                CandidateStatus::UnsupportedSyntax,
                "{text}"
            );
            assert_eq!(
                serde_json::to_value(&native.facts).unwrap(),
                json!(expected("unit.py", &[(1, 1, "module", "unit", "unit")]))
            );
        }
        for module in [".package", "..."] {
            let scoped_safe = safe
                .lines()
                .map(|line| format!("        {line}\n"))
                .collect::<String>();
            let text = format!(
                "from ordinary import (safe as local,)\ndef f(local):\n    from {module} import (\n{scoped_safe}        {terminal} as local,\n    )\n    local()\n"
            );
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            assert_declaration_candidates_match_ast(&python, &scan);
        }
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn multiline_import_candidate_malformed_and_unmodeled_forms_rollback_whole_files() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};

    let python = oracle();
    for (unsupported, malformed) in [
        ("from\npackage import (item)", true),
        ("from package\nimport (item)", true),
        ("from package import\n(item)", true),
        ("from package import # prefix\n(item)", true),
        ("from package # prefix\nimport (item)", true),
        ("from package import (item other)", true),
        ("from package import (item\nother)", true),
        ("from package import (item as\nlocal other)", true),
        ("from package import (item,,other)", true),
        ("from package import (,item)", true),
        ("from package import (item as)", true),
        ("from package import ()", true),
        ("from package import (\n# empty\n)", true),
        ("from package import ((item))", true),
        ("from package import (*)", true),
        ("from package import (item.child)", true),
        ("from package import (item) ,", true),
        ("from package import (item) (other)", true),
        ("from package import (True)", true),
        ("from package import (item as class)", true),
        ("from __future__ import (annotations,)", false),
        ("from package \\\n import (item)", false),
        ("from package import (item \\\n as local)", false),
        ("def f(): from package import (item)", false),
        ("from package import (item); ordinary()", false),
        ("from\tpackage import (item)", false),
        ("from package import (\n\titem\n)", false),
        ("from package import (item\u{c},)", false),
        ("from package import (\u{212a},)", false),
        ("from package import (item as \u{212a},)", false),
        ("from package import (item, # type: ignore\n)", false),
        ("from package import (item) # type: ignore", false),
        ("from package import (item, # type: int\n)", true),
        ("from package import (item) # type: int", true),
        ("from package import item\nas local", true),
        ("from package import item,\nother", true),
        ("from package import item # gap\nas local", true),
        ("import (package)", true),
    ] {
        let text = format!("import safe\ndef before(): pass\n{unsupported}\ndef after(): pass\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let ast = extract_captured(&python, &scan.sources).unwrap();
        assert_eq!(ast.complete, !malformed, "{unsupported:?}");
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_ne!(
            native.files[0].status,
            CandidateStatus::AcceptedFacts,
            "{unsupported:?}"
        );
        assert_eq!(
            serde_json::to_value(&native.facts).unwrap(),
            json!(expected("unit.py", &[(1, 1, "module", "unit", "unit")]))
        );
    }
    let (_, scan) = capture(&[("unit.py", b"from package import (\n item as\n local,\n)\n")]);
    assert_declaration_candidates_match_ast(&python, &scan);
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn multiline_import_candidate_capacity_levels_and_long_targets_retain_limits() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};

    let python = oracle();
    for count in [4, 5, 16, 17, 128, 129] {
        let names = (0..count)
            .map(|n| format!("item{n} as local{n},\n"))
            .collect::<String>();
        let text = format!("from package import (\n{names})\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
    for level in [9, 10, 99, 100] {
        for module in ["", "package"] {
            let text = format!(
                "from {}{module} import (\nItem as Local,\n)\n",
                ".".repeat(level)
            );
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            assert_declaration_candidates_match_ast(&python, &scan);
        }
    }
    for size in [511, 512, 513] {
        let name = "a".repeat(size);
        for text in [
            format!("from {name}.package import (\nordinary as local,\n)\nlocal()\n"),
            format!("from package import (\n{name} as local,\n)\nlocal()\n"),
            format!("from package import (\nordinary as {name},\n)\n{name}()\n"),
        ] {
            let (_, scan) = capture(&[("caf\u{e9}\u{1f642}.py", text.as_bytes())]);
            assert_declaration_candidates_match_ast(&python, &scan);
        }
        let text = format!("def before(): pass\nfrom {name}.package import (safe, FastAPI as local,)\ndef after(): pass\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let ast = extract_captured(&python, &scan.sources).unwrap();
        assert!(ast.complete && ast.diagnostics.is_empty());
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_eq!(native.files[0].status, CandidateStatus::UnsupportedSyntax);
        assert_eq!(
            serde_json::to_value(&native.facts).unwrap(),
            json!(expected("unit.py", &[(1, 1, "module", "unit", "unit")]))
        );
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn multiline_import_candidate_file_isolation_replay_and_reversed_inputs_match() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};

    let python = oracle();
    let (root, mut scan) = capture(&[
        ("a.py", b"import safe\ndef before(): pass\nfrom package import (\n safe, FastAPI as local,\n)\ndef after(): pass\n"),
        ("b.py", b"def outer():\n    def inner():\n        from .package import (\n            eval as local,\n        )\n    # trailing\n"),
        ("c.py", b"def before(): pass\nfrom package import (item other)\ndef after(): pass\n"),
        ("d.py", b"from . import (\n item as local,\n)\nlocal()\n"),
    ]);
    let ast = extract_captured(&python, &scan.sources).unwrap();
    assert!(!ast.complete);
    assert_eq!(ast.diagnostics.len(), 1);
    assert_eq!(ast.diagnostics[0].path, "c.py");
    let native = extract_fact_candidates(&scan.sources).unwrap();
    for (file, status) in native.files.iter().zip([
        CandidateStatus::UnsupportedSyntax,
        CandidateStatus::AcceptedFacts,
        CandidateStatus::SyntaxError,
        CandidateStatus::AcceptedFacts,
    ]) {
        assert_eq!(file.status, status);
    }
    let first = serde_json::to_value(native).unwrap();
    let mut wanted = Vec::new();
    let mut replay_sources = Vec::new();
    for source in std::mem::take(&mut scan.sources) {
        match source.path.as_str() {
            "a.py" => wanted.extend(expected("a.py", &[(1, 1, "module", "a", "a")])),
            "c.py" => wanted.extend(expected("c.py", &[(1, 1, "module", "c", "c")])),
            _ => {
                scan.sources.push(source);
                let good = assert_declaration_candidates_match_ast(&python, &scan);
                wanted.extend(good["facts"].as_array().unwrap().iter().cloned());
                replay_sources.push(scan.sources.pop().unwrap());
                continue;
            }
        }
        replay_sources.push(source);
    }
    scan.sources = replay_sources;
    assert_eq!(first["facts"], json!(wanted));
    for path in ["a.py", "b.py", "c.py", "d.py"] {
        fs::remove_file(root.path().join(path)).unwrap();
    }
    assert_eq!(
        semantics(&ast),
        semantics(&extract_captured(&python, &scan.sources).unwrap())
    );
    assert_eq!(
        first,
        serde_json::to_value(extract_fact_candidates(&scan.sources).unwrap()).unwrap()
    );
    scan.sources.reverse();
    assert_eq!(
        semantics(&ast),
        semantics(&extract_captured(&python, &scan.sources).unwrap())
    );
    assert_eq!(
        first["facts"],
        serde_json::to_value(extract_fact_candidates(&scan.sources).unwrap()).unwrap()["facts"]
    );
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn declaration_candidate_supported_whole_files_match_every_ast_fact() {
    let python = oracle();
    for text in [
        "",
        "# a plain comment\npass\n",
        "# Unicode comment: \u{3bb}\u{1f642}\r\npass\r\n",
        "def f(): pass\n",
        "async def f(): pass\n",
        "def f(a, /, b, *rest, k, **opts):\n    pass\n",
        "def f(a: int = 1) -> str: pass\n",
        "def f(*rest):\n    pass\n",
        "def f(**opts,): pass\n",
        "def f(a, /,): pass\n",
        "def f(*, required): pass\n",
        "def type(match, case, print, exec): pass\n",
        "def f(\n    a, # first\n    b, /,\n    c, *rest,\n    z, y, **opts,\n):\n    # before\n    pass\n    # trailing\n# final\n",
        "def outer(a):\n    async def inner(b):\n        pass\n        # trailing inner\n    pass\n    # trailing outer\nasync def sibling():\n    def leaf(): pass\n# final\n",
        "def outer(a):\n    def inner(b):\n        pass\n    async def leaf():\n        pass\ndef outer(a):\n    pass\n",
        "\u{feff}def f(a):\r\n    pass\r\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn declaration_candidate_signature_boundaries_preserve_parameters() {
    let python = oracle();
    for count in [128, 129] {
        let names = (0..count).map(|n| format!("p{n}")).collect::<Vec<_>>();
        let text = format!("def f({}, *rest, **opts):\n    pass\n", names.join(", "));
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        let facts = native["facts"].as_array().unwrap();
        assert_eq!(
            facts
                .iter()
                .filter(|fact| fact["kind"] == "parameter")
                .count(),
            count + 2
        );
        let signature = facts
            .iter()
            .find(|fact| fact["kind"] == "signature")
            .unwrap();
        assert_eq!(signature["value"] == r#"{"truncated":true}"#, count > 128);
    }
    let overhead = r#"{"keywordOnly":[],"positional":[""],"positionalOnly":0}"#.len();
    for size in [16_384, 16_385] {
        let name = "a".repeat(size - overhead);
        let text = format!("def f({name}):\n    pass\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        let signature = native["facts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|fact| fact["kind"] == "signature")
            .unwrap();
        if size == 16_384 {
            assert_eq!(signature["value"].as_str().unwrap().len(), size);
        } else {
            assert_eq!(signature["value"], r#"{"truncated":true}"#);
        }
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn declaration_candidate_unicode_metadata_and_empty_module_keep_exact_ids() {
    let python = oracle();
    for module in ["", "pkg.caf\u{e9}\u{1f642}", "pkg.\u{7f}\n\"\\\u{1f642}"] {
        let (_, mut scan) = capture(&[("caf\u{e9}\u{1f642}.py", b"def f(a, *, b):\n    pass\n")]);
        scan.sources[0].module = module.into();
        assert_declaration_candidates_match_ast(&python, &scan);
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn declaration_candidate_unsupported_forms_never_leak_a_declaration_prefix() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};

    let python = oracle();
    for unsupported in [
        "from tools import *\n",
        "class Hidden(metaclass=Meta):\n    pass\n",
        "def f(a=build()): pass\n",
        "def f(a: marker()): pass\n",
        "def f() -> marker(): pass\n",
        "@decorate()\ndef f(): pass\n",
        "def f[T: build()](): pass\n",
        "def f():\n    f'{hidden()}'\n",
        "def f(): pass; hidden()\n",
        "def f():\n    if True:\n        pass\n",
        "def f():\n    global value\n",
        "def f():\n    nonlocal value\n",
        "def f(a, a): pass\n",
        "def f(*, **opts): pass\n",
        "def f(a, /, /): pass\n",
        "def f((a,b)): pass\n",
        "def caf\u{e9}(): pass\n",
        "def \u{212a}(): pass\n",
        "def f(\u{212a}): pass\n",
        "def f():\n\tpass\n",
        "def f():\n  pass\n    pass\n",
        "def \\\nf():\n    pass\n",
        "\u{200b}def f(): pass\n",
        "# type: ignore\n",
        "def f(\n a,\n): pass\n",
        "print 1\n",
        "# \0\n",
    ] {
        let text = format!("def before(): pass\n{unsupported}\ndef after(): pass\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let _ast = extract_captured(&python, &scan.sources).unwrap();
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_ne!(
            native.files[0].status,
            CandidateStatus::AcceptedFacts,
            "{unsupported:?}"
        );
        assert_eq!(
            serde_json::to_value(&native.facts).unwrap(),
            json!(expected("unit.py", &[(1, 1, "module", "unit", "unit")]))
        );
        assert!(!serde_json::to_string(&native)
            .unwrap()
            .contains("PRIVATE_DECL_LITERAL"));
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn declaration_candidate_file_transactions_order_and_replay_use_captured_inputs() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};

    let python = oracle();
    let (root, mut scan) = capture(&[
        ("a.py", b"def before(): pass\ndef hidden(a=build()): pass\n"),
        ("b.py", b"async def ready(a, /, *, k):\n    pass\n"),
    ]);
    let native = extract_fact_candidates(&scan.sources).unwrap();
    assert_eq!(native.files[0].status, CandidateStatus::UnsupportedSyntax);
    assert_eq!(native.files[1].status, CandidateStatus::AcceptedFacts);
    let ast_good = extract_captured(&python, &scan.sources[1..]).unwrap();
    assert!(ast_good.complete && ast_good.diagnostics.is_empty());
    let mut wanted = expected("a.py", &[(1, 1, "module", "a", "a")]);
    wanted.extend(
        serde_json::to_value(&ast_good.facts)
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .cloned(),
    );
    assert_eq!(serde_json::to_value(&native.facts).unwrap(), json!(wanted));
    let first_facts = serde_json::to_value(&native.facts).unwrap();
    let first_report = serde_json::to_value(native).unwrap();
    fs::remove_file(root.path().join("a.py")).unwrap();
    fs::remove_file(root.path().join("b.py")).unwrap();
    assert_eq!(
        first_report,
        serde_json::to_value(extract_fact_candidates(&scan.sources).unwrap()).unwrap()
    );
    scan.sources.reverse();
    let reversed = extract_fact_candidates(&scan.sources).unwrap();
    assert_eq!(first_facts, serde_json::to_value(&reversed.facts).unwrap());
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn header_candidate_parameter_kinds_and_generics_match_the_whole_ast() {
    let python = oracle();
    for text in [
        "def f(x: int): pass\n",
        "def f(x: package.Item) -> package.Result: pass\n",
        "async def f(x: int, /, y: Item = 1, *rest: tuple[str, ...], required: dict[str, list[Item]], optional: list['Item'] = None, **opts: object) -> None:\n    pass\n",
        "def f(x=1, /, y: T=2, *, required: R, optional=False): pass\n",
        "def f(*args: T, **kwargs: R) -> ...: pass\n",
        "def f(x: typing.Optional[int] = None) -> typing.List[str]: pass\n",
        "def outer(x: A=1) -> Result:\n    async def nested(y: B=None):\n        pass\n    pass\n",
        "\u{feff}def f(x: T = 'PRIVATE_HEADER_LITERAL') -> R:\r\n    pass\r\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let result = assert_declaration_candidates_match_ast(&python, &scan);
        assert!(!result.to_string().contains("PRIVATE_HEADER_LITERAL"));
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn header_candidate_grouped_multiline_spans_match_ast_arguments_and_expressions() {
    let python = oracle();
    for text in [
        "def f(\n x: (\n  T\n ) = (\n  1\n ),\n *args: (\n  A\n ),\n **kw: B\n) -> (\n R\n):\n    pass\n",
        "def f(\n x: (\n  T[A]\n ) = (\n  - (\n   2\n  )\n ),\n y=(\n  'PRIVATE_HEADER_LITERAL' # between\n  'SECOND_LITERAL'\n ),\n) -> (\n pkg.R\n):\n    pass\n    # tail\n",
        "def f(\n x: T # annotation comment\n = (\n  2\n ),\n):\n    pass\n",
        "def f(x: (pkg.T), y: T[(A)], *, z: T[(A, B)]) -> (pkg.R): pass\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let result = assert_declaration_candidates_match_ast(&python, &scan);
        assert!(!result.to_string().contains("PRIVATE_HEADER_LITERAL"));
        assert!(!result.to_string().contains("SECOND_LITERAL"));
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn header_candidate_literal_spellings_match_without_retaining_values() {
    let python = oracle();
    for literal in [
        "None",
        "True",
        "False",
        "...",
        "0",
        "00",
        "0_0",
        "12",
        "1_234",
        "0xFF",
        "0X_FF",
        "0o77",
        "0O_7",
        "0b10",
        "0B_1",
        "1.0",
        "1.",
        ".5",
        "01.25",
        "1_0.2_5",
        "1e3",
        "1E+3",
        "1e-3",
        "1_0e2_0",
        "2j",
        "2J",
        "01j",
        ".5j",
        "1.5e+2j",
        "-1",
        "+1",
        "-0xF",
        "+(1.5)",
        "-(2j)",
        "''",
        "'PRIVATE_HEADER_LITERAL'",
        "'\u{3bb}\u{1f642}'",
        "r'PRIVATE_HEADER_LITERAL'",
        "u'PRIVATE_HEADER_LITERAL'",
        "b'PRIVATE_HEADER_LITERAL'",
        "br'PRIVATE_HEADER_LITERAL'",
        "Rb'PRIVATE_HEADER_LITERAL'",
        "'''PRIVATE_HEADER_LITERAL\nSECOND_LITERAL'''",
        "('PRIVATE_HEADER_LITERAL' # between\n 'SECOND_LITERAL')",
        "(b'one'\n br'two')",
    ] {
        let text = format!("def f(value={literal}):\n    pass\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let result = assert_declaration_candidates_match_ast(&python, &scan);
        assert!(!result.to_string().contains("PRIVATE_HEADER_LITERAL"));
        assert!(!result.to_string().contains("SECOND_LITERAL"));
    }
    for annotation in [
        "'PRIVATE_HEADER_LITERAL'",
        "None",
        "...",
        "T[A]",
        "T[A,]",
        "T[A,B]",
        "T[(A)]",
        "T[(A,B)]",
        "dict[str,list['PRIVATE_HEADER_LITERAL']]",
        "tuple[Item,...]",
    ] {
        let text = format!("def f(value: {annotation}) -> {annotation}:\n    pass\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let result = assert_declaration_candidates_match_ast(&python, &scan);
        assert!(!result.to_string().contains("PRIVATE_HEADER_LITERAL"));
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn header_candidate_shape_budgets_match_without_normalizing_away_differences() {
    let python = oracle();
    let mut shapes = vec![];
    for count in [16, 17] {
        shapes.push(format!(
            "T[{}]",
            (0..count)
                .map(|n| format!("A{n}"))
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
    for count in [11, 12, 13, 14] {
        shapes.push(format!("{}A{}", "T[".repeat(count), "]".repeat(count)));
    }
    for size in [511, 512, 513] {
        shapes.push("A".repeat(size));
        shapes.push(format!("{}.B", "A".repeat(size)));
    }
    // Root subscript + base + tuple consume 3 nodes. Eight 9-node generics
    // plus 4/5/6 names exercise 79/80/81 AST-expression visits respectively.
    for extra in [4, 5, 6] {
        let mut arguments = vec!["U[A,A,A,A,A,A]"; 8];
        arguments.extend(std::iter::repeat_n("A", extra));
        shapes.push(format!("T[{}]", arguments.join(",")));
    }
    for shape in shapes {
        let text = format!("def f(value: {shape}) -> {shape}:\n    pass\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn header_candidate_hidden_calls_and_unreviewed_literals_reject_whole_files() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};

    let python = oracle();
    let mut annotations = vec![
        "build()".into(),
        "T[build()]".into(),
        "A | B".into(),
        "Callable[[A],B]".into(),
        "T[A:B]".into(),
        "T[*A]".into(),
        "T[A][B]".into(),
        "f'PRIVATE_HEADER_LITERAL'".into(),
    ];
    annotations.extend([
        format!("T[{},build()]", ["A"; 17].join(",")),
        format!("{}build(){}", "T[".repeat(15), "]".repeat(15)),
        format!("T[{},T[build()]]", ["U[A,A,A,A,A,A]"; 16].join(",")),
        format!("{}.T[build()]", "A".repeat(600)),
    ]);
    let mut cases = annotations
        .into_iter()
        .map(|annotation| format!("def f(x: {annotation}): pass\n"))
        .collect::<Vec<_>>();
    for default in [
        "build()",
        "DEFAULT",
        "math.inf",
        "Ellipsis",
        "[]",
        "{}",
        "()",
        "{1}",
        "1+2j",
        "--1",
        "+-1",
        "-True",
        "f'PRIVATE_HEADER_LITERAL'",
        "f'{build()}'",
        "t'text'",
        "ur'text'",
        "'a' b'b'",
        "b'a' 'b'",
        "r'a\\b'",
        "'a\\n'",
        "b'\u{3bb}'",
        "1L",
        "0xffL",
        "0123",
        "0_1",
        "1_",
        "1__2",
        "0x__1",
        "0b102",
        "1e_2",
        "1._2",
        "1e2_",
    ] {
        cases.push(format!("def f(x={default}): pass\n"));
    }
    cases.extend([
        "def f(x=1, /, y): pass\n".into(),
        "def f(x: A=1, y: B): pass\n".into(),
        "def f(*args: A=1): pass\n".into(),
        "def f(x:A, x:B=2): pass\n".into(),
        "def f(x: A) -> build(): pass\n".into(),
        format!("def f(x={}): pass\n", "1".repeat(513)),
    ]);
    for unsupported in cases {
        let text = format!("def before(): pass\n{unsupported}def after(): pass\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let _ast = extract_captured(&python, &scan.sources).unwrap();
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_ne!(
            native.files[0].status,
            CandidateStatus::AcceptedFacts,
            "{unsupported}"
        );
        assert_eq!(
            serde_json::to_value(&native.facts).unwrap(),
            json!(expected("unit.py", &[(1, 1, "module", "unit", "unit")]))
        );
        assert!(!serde_json::to_string(&native)
            .unwrap()
            .contains("PRIVATE_HEADER_LITERAL"));
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn header_candidate_signature_limits_ordering_and_replay_remain_exact() {
    use super::python_native::extract_fact_candidates;

    let python = oracle();
    for count in [128, 129] {
        let parameters = (0..count)
            .map(|n| format!("p{n}: T = {n}"))
            .collect::<Vec<_>>()
            .join(",");
        let text = format!("def f({parameters}, *args: A, **kw: B) -> R:\n    pass\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
    let (root, mut scan) = capture(&[
        (
            "a.py",
            b"def f(value: T='PRIVATE_HEADER_LITERAL') -> R:\n    pass\n",
        ),
        (
            "b.py",
            b"async def ready(*args: A, key: K=None, **opts: O):\n    pass\n",
        ),
    ]);
    let native = assert_declaration_candidates_match_ast(&python, &scan);
    fs::remove_file(root.path().join("a.py")).unwrap();
    fs::remove_file(root.path().join("b.py")).unwrap();
    assert_eq!(
        native,
        serde_json::to_value(extract_fact_candidates(&scan.sources).unwrap()).unwrap()
    );
    scan.sources.reverse();
    assert_eq!(
        native["facts"],
        serde_json::to_value(extract_fact_candidates(&scan.sources).unwrap()).unwrap()["facts"]
    );
    assert!(!native.to_string().contains("PRIVATE_HEADER_LITERAL"));
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn header_candidate_grammar_only_invalid_newlines_never_become_accepted_facts() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};

    let python = oracle();
    let mut invalid = vec![
        "def f() -> pkg.\nT:\n    pass\n".to_string(),
        "def f() -> pkg. # comment\nT:\n    pass\n".to_string(),
        "def f() -> T\n:\n    pass\n".to_string(),
        "def f() -> T[Item]\n:\n    pass\n".to_string(),
        "def f() -> T\n# comment\n:\n    pass\n".to_string(),
    ];
    for prefix in ["", "r", "b", "br"] {
        for newline in ["\n", "\r\n"] {
            let literal = format!("{prefix}'{newline}PRIVATE_HEADER_LITERAL'");
            invalid.extend([
                format!("def f(x={literal}):\n    pass\n"),
                format!("def f(x:{literal}):\n    pass\n"),
                format!("def f() -> {literal}:\n    pass\n"),
            ]);
        }
    }
    for malformed in invalid {
        let text = format!("def before(): pass\n{malformed}def after(): pass\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let ast = extract_captured(&python, &scan.sources).unwrap();
        assert!(!ast.complete, "AST unexpectedly accepted {malformed:?}");
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_ne!(native.files[0].status, CandidateStatus::AcceptedFacts);
        assert_eq!(
            serde_json::to_value(&native.facts).unwrap(),
            json!(expected("unit.py", &[(1, 1, "module", "unit", "unit")]))
        );
        assert!(!serde_json::to_string(&native)
            .unwrap()
            .contains("PRIVATE_HEADER_LITERAL"));
    }
    for supported in [
        "def f() -> (\n pkg.\n T\n):\n    pass\n",
        "def f() -> Map[\n Key,\n List[Item]\n]:\n    pass\n",
        "def f(\n x: pkg.\n T\n):\n    pass\n",
        "def f(x='''\nPRIVATE_HEADER_LITERAL'''):\n    pass\n",
        "def f(x: '''\nPRIVATE_HEADER_LITERAL''') -> '''\nRETURN_LITERAL''':\n    pass\n",
    ] {
        let (_, scan) = capture(&[("unit.py", supported.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        assert!(!native.to_string().contains("PRIVATE_HEADER_LITERAL"));
        assert!(!native.to_string().contains("RETURN_LITERAL"));
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn body_candidate_calls_writes_and_returns_match_the_whole_ast() {
    let python = oracle();
    for text in [
        "helper()\n",
        "value\n'PRIVATE_BODY_LITERAL'\n",
        "local = helper\nlocal = None\nlocal()\n",
        "def f(): return\n",
        "def f():\n    return\n    helper()\n",
        "def f():\n    return None\n    helper()\n",
        "def f(x: int=3) -> int: return x\n",
        "def f():\n    'PRIVATE_BODY_LITERAL'\n    return -1\n",
        "def f(value):\n    outer(inner(value), key=other('PRIVATE_BODY_LITERAL'))\n    return None\n",
        "def configure(self, provider):\n    self.client = provider\n    self.client = None\n    self.client()\n",
        "def configure(cls, provider):\n    cls.group.client = provider\n    cls.group.client = 1\n    cls.group.client()\n",
        "def f():\n    local = package.member\n    local = package.work(value=3)\n    return local\n",
        "def outer(value: T=1):\n    def nested(item):\n        return leaf(item)\n    nested(value)\n    return 2\n",
        "alias = register_url_convertor\nalias('PRIVATE_BODY_LITERAL', converter())\nregister_url_convertor('PRIVATE_BODY_LITERAL', other())\n",
        "\u{feff}async def f(value: T=None):\r\n    local = value\r\n    local = 0\r\n    return work(value)\r\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        assert!(!native.to_string().contains("PRIVATE_BODY_LITERAL"));
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn body_candidate_grouping_statement_extents_and_target_clipping_match() {
    let python = oracle();
    for text in [
        "def f(item):\n    target = (\n        helper(\n            item\n        )\n    )\n    # trailing\n",
        "def f(item):\n    return (\n        helper(item, option=other())\n    )\n    # trailing\n",
        "def f():\n    (helper)()\n    (\n        helper()\n    )\n",
        "def f():\n    value = (\n        package.item\n    )\n    return (\n        'PRIVATE_BODY_LITERAL'\n    )\n",
        "def f():\n    '''PRIVATE_BODY_LITERAL\nSECOND_BODY_LITERAL'''\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        assert!(!native.to_string().contains("PRIVATE_BODY_LITERAL"));
        assert!(!native.to_string().contains("SECOND_BODY_LITERAL"));
    }
    for size in [511, 512, 513] {
        let name = "a".repeat(size);
        for target in [
            name.clone(),
            format!("self.{name}.child"),
            format!("cls.{name}"),
        ] {
            let text = format!("def f(self, cls):\n    {target} = provider\n    {target} = None\n");
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            assert_declaration_candidates_match_ast(&python, &scan);
        }
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn body_candidate_render_limits_do_not_omit_nested_call_facts() {
    let python = oracle();
    let mut expressions = vec!["outer(leaf(), leaf())".to_string()];
    for count in [16, 17] {
        let positional = (0..count)
            .map(|n| format!("pos{n}()"))
            .collect::<Vec<_>>()
            .join(",");
        let keywords = (0..count)
            .map(|n| format!("k{n}=kw{n}()"))
            .collect::<Vec<_>>()
            .join(",");
        expressions.extend([
            format!("outer({positional})"),
            format!("outer({keywords})"),
            format!("outer({positional},{keywords})"),
        ]);
    }
    for count in [12, 13, 40] {
        expressions.push(format!(
            "{}value{}",
            "nest(".repeat(count),
            ")".repeat(count)
        ));
    }
    let nested = (0..16)
        .map(|n| {
            format!(
                "g{n}({})",
                (0..16)
                    .map(|i| format!("a{i}"))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    expressions.push(format!("outer({nested})"));
    for size in [511, 512, 513] {
        expressions.push(format!("{}(leaf())", "a".repeat(size)));
        expressions.push(format!("{}.run(leaf())", "a".repeat(size)));
    }
    for expression in expressions {
        let text = format!("def f(value):\n    return {expression}\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn body_candidate_side_outputs_and_unmodeled_forms_reject_whole_files() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};

    let python = oracle();
    let mut statements = vec![
        "x = APIRouter()".to_string(),
        "x = FastAPI()".into(),
        "r.include_router(child)".into(),
        "self.router = value".into(),
        "self.prefix = None".into(),
        "cls.router.child = value".into(),
        "other.member = value".into(),
        "x: T | U = value".into(),
        "a = b = work()".into(),
        "a, b = work()".into(),
        "[a] = work()".into(),
        "x[0] = value".into(),
        "x += work()".into(),
        "del x".into(),
        "(x := work())".into(),
        "await work()".into(),
        "yield value".into(),
        "outer(*items)".into(),
        "outer(**options)".into(),
        "outer(x=1, x=2)".into(),
        "factory()()".into(),
        "factory().run()".into(),
        "outer(x for x in source)".into(),
        "outer([x for x in source])".into(),
        "outer(lambda: work())".into(),
        "outer([1])".into(),
        "x = a + b".into(),
    ];
    for callee in ["FastAPI", "APIRouter"] {
        statements.push(format!("{callee}('PRIVATE_BODY_LITERAL')"));
        statements.push(format!("package.{callee}('PRIVATE_BODY_LITERAL')"));
    }
    statements.extend([
        format!("outer({}, *items)", ["value"; 17].join(",")),
        format!(
            "outer({}, FastAPI('PRIVATE_BODY_LITERAL'))",
            ["value"; 17].join(",")
        ),
        format!(
            "{}APIRouter('PRIVATE_BODY_LITERAL'){}",
            "nest(".repeat(15),
            ")".repeat(15)
        ),
    ]);
    for statement in statements {
        let text =
            format!("def before(): pass\ndef f(self, cls):\n    {statement}\ndef after(): pass\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let _ast = extract_captured(&python, &scan.sources).unwrap();
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_ne!(
            native.files[0].status,
            CandidateStatus::AcceptedFacts,
            "{statement}"
        );
        assert_eq!(
            serde_json::to_value(&native.facts).unwrap(),
            json!(expected("unit.py", &[(1, 1, "module", "unit", "unit")]))
        );
        assert!(!serde_json::to_string(&native)
            .unwrap()
            .contains("PRIVATE_BODY_LITERAL"));
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn body_candidate_callee_clipping_and_parser_nesting_never_forge_parity() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};

    let python = oracle();
    for terminal in ["FastAPI", "APIRouter"] {
        let prefix = "a".repeat(512 - 1 - terminal.len());
        let call = format!("{prefix}.{terminal}Extra()");
        for body in [
            format!("x = {call}"),
            format!("outer({}, {call})", ["value"; 17].join(",")),
        ] {
            let text = format!("def before(): pass\ndef f():\n    {body}\ndef after(): pass\n");
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            let ast = extract_captured(&python, &scan.sources).unwrap();
            assert!(ast.complete);
            let native = extract_fact_candidates(&scan.sources).unwrap();
            assert_ne!(native.files[0].status, CandidateStatus::AcceptedFacts);
            assert_eq!(native.facts.len(), 1);
        }
    }
    for levels in [200, 201] {
        let grouped = format!("{}value{}", "(".repeat(levels), ")".repeat(levels));
        for text in [
            format!("x = {grouped}\n"),
            format!("{grouped}\n"),
            format!("def f():\n    return {grouped}\n"),
        ] {
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            if levels == 200 {
                assert_declaration_candidates_match_ast(&python, &scan);
            } else {
                let ast = extract_captured(&python, &scan.sources).unwrap();
                assert!(!ast.complete);
                let native = extract_fact_candidates(&scan.sources).unwrap();
                assert_ne!(native.files[0].status, CandidateStatus::AcceptedFacts);
                assert_eq!(native.facts.len(), 1);
            }
        }
    }
    for levels in [199, 200] {
        let text = format!("outer({}value{})\n", "(".repeat(levels), ")".repeat(levels));
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        if levels == 199 {
            assert_declaration_candidates_match_ast(&python, &scan);
        } else {
            assert!(!extract_captured(&python, &scan.sources).unwrap().complete);
            let native = extract_fact_candidates(&scan.sources).unwrap();
            assert_ne!(native.files[0].status, CandidateStatus::AcceptedFacts);
            assert_eq!(native.facts.len(), 1);
        }
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn body_candidate_focus_fixture_preserves_calls_and_lexical_blockers() {
    use super::python_native::extract_fact_candidates;

    let python = oracle();
    let text = b"def leaf(value: int=0) -> int:\n    return 1\ndef middle(value: int) -> int:\n    leaf(value)\n    return 2\ndef direct(value: int=0):\n    middle(value)\ndef shadowed(middle, /, *, value=0):\n    middle(value)\ndef varargs(*middle):\n    middle()\ndef kwargs(**middle):\n    middle()\ndef overwritten():\n    middle = None\n    middle()\ndef rebound():\n    alias = middle\n    alias()\n    alias = None\n    alias()\n";
    let (root, scan) = capture(&[("unit.py", text)]);
    assert_declaration_candidates_match_ast(&python, &scan);
    let ast = extract(root.path(), &python).unwrap();
    let native = extract_fact_candidates(&scan.sources).unwrap();
    // Test-only resolver fixture after full fact equality; never persist this or
    // treat an AST fixture's identity as authority for native candidate evidence.
    let mut fixture = serde_json::to_value(&ast).unwrap();
    fixture["facts"] = serde_json::to_value(&native.facts).unwrap();
    let candidate_fixture: Evidence = serde_json::from_value(fixture).unwrap();
    for (name, expected_links) in [
        ("direct", Some(2)),
        ("shadowed", Some(0)),
        ("varargs", Some(0)),
        ("kwargs", Some(0)),
        ("overwritten", Some(0)),
        ("rebound", None),
    ] {
        let options = crate::focus::FocusOptions {
            symbol: format!("unit.{name}"),
            hops: 3,
            max_facts: 300,
        };
        let trusted = crate::focus::prepare(&ast, &options).unwrap();
        let candidate = crate::focus::prepare(&candidate_fixture, &options).unwrap();
        assert_eq!(trusted.input, candidate.input);
        if let Some(count) = expected_links {
            assert_eq!(
                candidate.input["focus"]["links"].as_array().unwrap().len(),
                count,
                "{name}"
            );
        }
        if name == "direct" {
            assert!(candidate.input["focus"]["visitedSymbols"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s == "unit.leaf"));
        }
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn body_candidate_replay_transactions_and_source_order_are_deterministic() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};

    let python = oracle();
    let (root, mut scan) = capture(&[
        ("a.py", b"def before(): pass\nx = work()\nfrom hidden import *\n"),
        ("b.py", b"def ready(value: T=None):\n    local = value\n    local = 1\n    return work(local)\n"),
    ]);
    let native = extract_fact_candidates(&scan.sources).unwrap();
    assert_eq!(native.files[0].status, CandidateStatus::UnsupportedSyntax);
    assert_eq!(native.files[1].status, CandidateStatus::AcceptedFacts);
    let ast_good = extract_captured(&python, &scan.sources[1..]).unwrap();
    assert!(ast_good.complete && ast_good.diagnostics.is_empty());
    let mut wanted = expected("a.py", &[(1, 1, "module", "a", "a")]);
    wanted.extend(
        serde_json::to_value(&ast_good.facts)
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .cloned(),
    );
    assert_eq!(serde_json::to_value(&native.facts).unwrap(), json!(wanted));
    let first = serde_json::to_value(native).unwrap();
    fs::remove_file(root.path().join("a.py")).unwrap();
    fs::remove_file(root.path().join("b.py")).unwrap();
    assert_eq!(
        first,
        serde_json::to_value(extract_fact_candidates(&scan.sources).unwrap()).unwrap()
    );
    scan.sources.reverse();
    assert_eq!(
        first["facts"],
        serde_json::to_value(extract_fact_candidates(&scan.sources).unwrap()).unwrap()["facts"]
    );
}
