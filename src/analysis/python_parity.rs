//! Bounded AST parity gates, not a native parser or a full Python parity claim.
//! Supported here: lexical ownership/signatures, safe expression shapes, local
//! call guards, direct route syntax, file transactions, and byte decoding.
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
    use super::python_native::{extract_fact_candidates, CandidateStatus};

    let ast = extract_captured(python, &scan.sources).unwrap();
    assert!(ast.complete && ast.diagnostics.is_empty());
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
    for (file, source) in native.files.iter().zip(&scan.sources) {
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
fn declaration_candidate_supported_whole_files_match_every_ast_fact() {
    let python = oracle();
    for text in [
        "",
        "# a plain comment\npass\n",
        "# Unicode comment: \u{3bb}\u{1f642}\r\npass\r\n",
        "def f(): pass\n",
        "async def f(): pass\n",
        "def f(a, /, b, *rest, k, **opts):\n    pass\n",
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
        "import tools\n",
        "class Hidden:\n    pass\n",
        "hidden()\n",
        "value = 1\n",
        "def f(a=1): pass\n",
        "def f(a=build()): pass\n",
        "def f(a: marker()): pass\n",
        "def f() -> marker(): pass\n",
        "@decorate()\ndef f(): pass\n",
        "def f[T: build()](): pass\n",
        "def f():\n    return\n",
        "def f():\n    'PRIVATE_DECL_LITERAL'\n",
        "def f():\n    f'{hidden()}'\n",
        "def f(): pass; hidden()\n",
        "def f():\n    pass\n    hidden()\n",
        "def f():\n    async def nested():\n        hidden()\n",
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
