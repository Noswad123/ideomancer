//! Bounded AST parity gates, not a native parser or a full Python parity claim.
//! Supported here: lexical ownership/signatures, safe expression shapes, local
//! call guards, bounded imports, direct route syntax, file transactions, and byte decoding.
//! Later native work still needs broader syntax, binding/control-flow, resource,
//! and cross-version coverage before it can propose a production migration.

#[path = "python_parity/fstring_batch.rs"]
mod fstring_batch;
#[path = "python_parity/generic_batch.rs"]
mod generic_batch;
#[path = "python_parity/literal_batch.rs"]
mod literal_batch;
#[path = "python_parity/projection_batch.rs"]
mod projection_batch;
#[path = "python_parity/structure_batch.rs"]
mod structure_batch;

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

pub(super) fn oracle() -> PathBuf {
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
        "{native:?}; captured synthetic inputs: {:?}",
        scan.sources
            .iter()
            .map(|source| (
                &source.path,
                String::from_utf8_lossy(&source.bytes[..source.bytes.len().min(512)])
            ))
            .collect::<Vec<_>>()
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
fn suspension_expression_candidate_four_shapes_roots_and_nonwrites() {
    let python = oracle();
    for (value, shape) in [
        ("await hidden()", "<Await>"),
        ("(yield hidden())", "<Yield>"),
        ("(yield from hidden())", "<YieldFrom>"),
        ("(written := hidden())", "<NamedExpr>"),
    ] {
        let text = format!("root = {value}\nsink({value})\nouter = ({value}).field\ncalled = ({value})()\nself.field = {value}\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        assert_eq!(
            native["identity"]["policy"],
            "python-native-private-facts-15"
        );
        let sink = format!("sink({shape})");
        let attribute = format!(r#"{{"source":"{shape}.field","target":"outer"}}"#);
        let call = format!("{shape}()");
        let binding = format!(r#"{{"source":"{call}","target":"called"}}"#);
        assert_eq!(
            native["facts"],
            json!(expected(
                "unit.py",
                &[
                    (1, 1, "binding_invalidation", "unit", r#"{"target":"root"}"#),
                    (1, 1, "call", "unit", "hidden()"),
                    (1, 1, "module", "unit", "unit"),
                    (2, 2, "call", "unit", "hidden()"),
                    (2, 2, "call", "unit", &sink),
                    (3, 3, "binding", "unit", &attribute),
                    (3, 3, "call", "unit", "hidden()"),
                    (4, 4, "binding", "unit", &binding),
                    (4, 4, "call", "unit", &call),
                    (4, 4, "call", "unit", "hidden()"),
                    (
                        5,
                        5,
                        "binding_invalidation",
                        "unit",
                        r#"{"target":"self.field"}"#
                    ),
                    (5, 5, "call", "unit", "hidden()"),
                ]
            )),
            "{value}"
        );
    }
    // NamedExpr is generic traversal, not Assign/AnnAssign or a class attribute.
    let (_, scan) = capture(&[(
        "unit.py",
        b"class Box:\n    (field := value)\n    (router := value)\n    (prefix := value)\n",
    )]);
    let native = assert_declaration_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (1, 1, "module", "unit", "unit"),
                (1, 4, "class", "unit.Box", "Box"),
            ]
        ))
    );
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn suspension_expression_candidate_ast_only_contexts_and_lexical_owners() {
    let python = oracle();
    // ast.parse is the authority, not compile, function kind, or runtime permission.
    for text in [
        "await value\nyield\nyield value\nyield from values\n",
        "def f():\n    await work()\n    yield value\n    yield from values\n",
        "sink(lambda: await work())\nsink(lambda: (yield work()))\nsink(lambda: (yield from work()))\n",
        "def f(a=(yield value), b=await other, c=(name := value)): pass\n",
        "sink([(yield work()) for x in xs])\nsink([await work() for x in xs])\n",
        "sink({await work() for x in xs})\nsink({x:(y := work()) for x in xs})\n",
        "sink(await work() for x in xs)\nsink(x for x in xs if (y := work()))\n",
        "sink([x := x for x in xs])\nsink([x for x in (ys := values)])\n",
        "class Box:\n    sink([x := x for x in xs])\n    await work()\n    yield value\n",
        "def f():\n    global value\n    (x := values)\n    nonlocal other\n    (yield values)\n",
        "try: await work()\nfinally: yield value\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
    let (_, scan) = capture(&[("unit.py", b"class Box:\n    sink(lambda: await hidden())\n    def run():\n        sink([written := hidden() for x in xs])\n")]);
    let native = assert_declaration_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (1, 1, "module", "unit", "unit"),
                (1, 4, "class", "unit.Box", "Box"),
                (2, 2, "call", "unit.Box", "hidden()"),
                (2, 2, "call", "unit.Box", "sink(<Lambda>)"),
                (3, 4, "function", "unit.Box.run", "run"),
                (
                    3,
                    4,
                    "signature",
                    "unit.Box.run",
                    r#"{"keywordOnly":[],"positional":[],"positionalOnly":0}"#
                ),
                (4, 4, "call", "unit.Box.run", "hidden()"),
                (4, 4, "call", "unit.Box.run", "sink(<ListComp>)"),
            ]
        ))
    );
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn suspension_expression_candidate_named_roles_grouping_and_spans() {
    let python = oracle();
    for text in [
        "if x := work(): pass\nelif y := other(): pass\n",
        "while x := work(): break\n",
        "sink(x := work())\nsink([x := work()], (x := work(),), {x := work()})\n",
        "sink([x := work() for item in xs])\nsink({x := work() for item in xs})\n",
        "sink(x := work() for item in xs)\nsink(items[x := work()])\n",
        "item = (x := work())\ndef f():\n    return (x := work())\n",
        "raise (x := work()) from (y := other())\nassert (x := work()), (y := other())\nyield (x := work())\n",
        "def f(a=(x := value)): pass\nsink(lambda: (x := work()))\n",
        "sink(k=(x := work()), **(opts := work()))\nsink(*(args := work()))\n",
        "sink({(x := work()): (y := other())})\nsink({(x := work()): (y := other()) for item in xs})\n",
        "sink([item for item in (xs := work()) if (test := ready())])\n",
        "sink(left if (test := ready()) else right)\nsink((left := work()) if ready else (right := other()))\n",
        "sink((x := work()) + (y := other()), not (z := ready()))\n",
        "sink(items[(start := work()):(stop := other()):(step := stride())])\n",
        "sink((outer := (inner := work())))\n",
        "sink((\n    written\n    :=\n    work()\n))\n",
        "for data[(x := work())] in xs: pass\nwith cm() as (x := work()).field: pass\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
    for newline in ["\n", "\r\n"] {
        let text = "sink(\n    written := hidden(\n        value\n    ),\n    key=(other := neighbor())\n)\n".replace('\n', newline);
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        assert_eq!(
            native["facts"],
            json!(expected(
                "unit.py",
                &[
                    (1, 1, "module", "unit", "unit"),
                    (1, 6, "call", "unit", "sink(<NamedExpr>, key=<NamedExpr>)"),
                    (2, 4, "call", "unit", "hidden(value)"),
                    (5, 5, "call", "unit", "neighbor()"),
                ]
            ))
        );
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn suspension_expression_candidate_named_bad_roles_targets_and_rollback() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};
    let python = oracle();
    // These are syntax failures, not comprehension binding/compiler restrictions.
    for form in [
        "x := value",
        "item = x := value",
        "def f():\n    return x := value",
        "raise x := value",
        "assert x := value",
        "yield x := value",
        "def f(a=x := value): pass",
        "sink(lambda: x := value)",
        "sink(k=x := value)",
        "sink(*x := value)",
        "sink(**x := value)",
        "sink({x := value: other})",
        "sink({key: x := value})",
        "sink({x := value: other for item in xs})",
        "sink({key: x := value for item in xs})",
        "sink([item for item in x := value])",
        "sink([item for item in xs if x := value])",
        "sink(x := value if y := other else z)",
        "sink(left if ready else x := value)",
        "sink(left or x := value)",
        "sink(not x := value)",
        "sink(left < x := value)",
        "sink(items[x := value:stop])",
        "sink(items[start:x := value])",
        "sink(x := y := value)",
        "for item in x := values: pass",
        "sink((obj.field := value))",
        "sink((items[0] := value))",
        "sink(((x) := value))",
        "sink(((x,y) := value))",
        "sink(([x] := value))",
        "sink((*x := value))",
        "sink(((lambda: x) := value))",
        "sink((True := value))",
        "sink((for := value))",
        "sink((x :=))",
        "sink((:= value))",
        "x :=\nvalue",
        "item = x\n:= value",
        "if x :=\nvalue: pass",
        "sink((x : = value))",
        "sink((x :\n= value))",
    ] {
        let text = format!("eval()\nclass Before: pass\n{form}\nexec()\n");
        let (_, scan) = capture(&[
            ("bad.py", text.as_bytes()),
            ("good.py", b"sink((x := eval()))\n"),
        ]);
        let ast = extract_captured(&python, &scan.sources).unwrap();
        assert!(!ast.complete, "AST accepted {form}");
        assert_eq!(ast.diagnostics[0].code, "parse_failure", "{form}");
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_ne!(
            native.files[0].status,
            CandidateStatus::AcceptedFacts,
            "{form}"
        );
        assert_eq!(native.files[1].status, CandidateStatus::AcceptedFacts);
        let mut facts = expected("bad.py", &[(1, 1, "module", "bad", "bad")]);
        facts.extend(expected(
            "good.py",
            &[
                (1, 1, "call", "good", "eval()"),
                (1, 1, "call", "good", "sink(<NamedExpr>)"),
                (1, 1, "module", "good", "good"),
            ],
        ));
        assert_eq!(
            serde_json::to_value(&native.facts).unwrap(),
            json!(facts),
            "{form}"
        );
        assert_eq!(
            serde_json::to_value(&native.diagnostics).unwrap(),
            json!([{
                "code":"dynamic_discovery_unresolved", "path":"good.py",
                "message":"line 1: eval; runtime targets are not inferred"
            }]),
            "{form}"
        );
    }
    // A non-ASCII Name is AST-positive but remains outside the private name slice.
    let text = "sink((\u{212a} := value))\n";
    let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
    assert!(extract_captured(&python, &scan.sources).unwrap().complete);
    let native = extract_fact_candidates(&scan.sources).unwrap();
    assert_eq!(native.files[0].status, CandidateStatus::UnsupportedSyntax);
    assert_eq!(
        serde_json::to_value(&native.facts).unwrap(),
        json!(expected("unit.py", &[(1, 1, "module", "unit", "unit"),]))
    );
    assert!(native.diagnostics.is_empty());
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn suspension_expression_candidate_yield_roles_tuples_stars_and_from() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};
    let python = oracle();
    for text in [
        "yield\nyield value\nyield a,b\nyield *xs, last\nyield *xs,\n",
        "item = yield\nitem: T = yield value\nitem += yield from values\n",
        "yield from (x := values)\nyield from (yield value)\nyield from (a,b)\n",
        "sink((yield), (yield a,b), (yield value,), (yield from values))\n",
        "sink([(yield work())], {(yield work())}, {(yield key()): (yield value())})\n",
        "sink((yield work()) + (yield other()))\nsink(lambda: (yield work()))\n",
        "def f(a=(yield value), b=(yield from values)): pass\n",
        "def f():\n    return (yield work())\n",
        "raise (yield work())\nassert (yield work())\nsink(key=(yield work()))\n",
        "x = [*(yield items)]\nx = (*(yield items),)\nx = {*(yield items)}\nx = {**(yield items)}\n",
        "sink(items[(yield start()):(yield stop())])\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
    let (_, scan) = capture(&[(
        "unit.py",
        b"sink((yield 'from'), (yield from hidden()), (yield), (yield *items, tail))\n",
    )]);
    let native = assert_declaration_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (1, 1, "call", "unit", "hidden()"),
                (
                    1,
                    1,
                    "call",
                    "unit",
                    "sink(<Yield>, <YieldFrom>, <Yield>, <Yield>)"
                ),
                (1, 1, "module", "unit", "unit"),
            ]
        ))
    );
    for text in [
        "sink((yield *eval(), tail))\n",
        "yield from lambda: eval()\n",
        "a = b = yield from eval()\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_warning_candidates_match_ast(&python, &scan);
    }
    let (_, scan) = capture(&[("unit.py", b"sink((yield value,))\nsink(((yield value),))\n")]);
    let native = assert_declaration_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (1, 1, "call", "unit", "sink(<Yield>)"),
                (1, 1, "module", "unit", "unit"),
                (2, 2, "call", "unit", "sink([<Yield>])"),
            ]
        ))
    );
    for form in [
        "yield head, x := eval()",
        "yield from",
        "yield from *xs",
        "yield from x := value",
        "yield from yield value",
        "yield from a,b",
        "sink(yield value)",
        "sink([yield value])",
        "sink({yield value})",
        "sink({key: yield value})",
        "def f():\n    return yield value",
        "def f(a=yield value): pass",
        "sink(lambda: yield value)",
        "sink(left + yield value)",
        "sink(left if yield value else right)",
        "for item in yield values: pass",
        "yield from\nvalues",
    ] {
        let text = format!("eval()\n{form}\nexec()\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let ast = extract_captured(&python, &scan.sources).unwrap();
        assert!(!ast.complete, "AST accepted {form}");
        assert_eq!(ast.diagnostics[0].code, "parse_failure");
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_ne!(
            native.files[0].status,
            CandidateStatus::AcceptedFacts,
            "{form}"
        );
        assert_eq!(
            serde_json::to_value(&native.facts).unwrap(),
            json!(expected("unit.py", &[(1, 1, "module", "unit", "unit"),]))
        );
        assert!(native.diagnostics.is_empty());
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn suspension_expression_candidate_await_primaries_power_association_and_bytes() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};
    let python = oracle();
    for operand in [
        "value",
        "obj.field",
        "items[index()]",
        "factory()",
        "factory()().field[index()]",
        "None",
        "True",
        "1",
        "'PRIVATE_LITERAL'",
        "[]",
        "{}",
        "{value}",
        "(value,)",
        "[x for x in xs]",
        "{x for x in xs}",
        "{x:x for x in xs}",
        "(x for x in xs)",
        "(await work())",
        "(-work())",
        "(left() + right())",
        "(lambda: work())",
        "(yield work())",
        "(written := work())",
    ] {
        let text = format!("sink(await {operand})\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        assert!(!native.to_string().contains("PRIVATE_LITERAL"));
    }
    for (value, shape) in [
        ("await x ** y", "<Await> Pow y"),
        ("await (x ** y)", "<Await>"),
        ("await x ** y ** z", "<Await> Pow y Pow z"),
        ("await x ** await y ** z", "<Await> Pow <Await> Pow z"),
        ("await x ** -y", "<Await> Pow <UnaryOp>"),
        ("await x ** y ** -z", "<Await> Pow y Pow <UnaryOp>"),
        ("(await x) ** y", "<Await> Pow y"),
        ("await x + y", "<Await> Add y"),
        ("-await x ** y", "<UnaryOp>"),
    ] {
        let text = format!("sink({value})\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        let call = format!("sink({shape})");
        assert_eq!(
            native["facts"],
            json!(expected(
                "unit.py",
                &[
                    (1, 1, "call", "unit", &call),
                    (1, 1, "module", "unit", "unit"),
                ]
            )),
            "{value}"
        );
    }
    for text in [
        "sink(*await left() ** right())\n",
        "sink(*(await left() ** right()))\n",
        "(await left() ** await right()).eval()\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_warning_candidates_match_ast(&python, &scan);
    }
    for depth in [10, 11, 12, 13, 14, 30] {
        let chain = (0..depth)
            .map(|n| format!("await v{n}"))
            .collect::<Vec<_>>()
            .join(" ** ");
        let text = format!("sink({chain})\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
    for count in [11, 12, 13] {
        let padding = vec!["[a,b,c,d,e]"; count].join(",");
        let text = format!("sink([{padding}], await left() ** right())\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
    for newline in ["\n", "\r\n"] {
        for bom in ["", "\u{feff}"] {
            let text =
                format!("{bom}sink(\n await left(\n  eval()\n ) ** right(\n  exec()\n )\n)\n")
                    .replace('\n', newline);
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            let native = assert_warning_candidates_match_ast(&python, &scan);
            assert_eq!(
                native["facts"],
                json!(expected(
                    "unit.py",
                    &[
                        (1, 1, "module", "unit", "unit"),
                        (1, 7, "call", "unit", "sink(<Await> Pow right(exec()))"),
                        (2, 4, "call", "unit", "left(eval())"),
                        (3, 3, "call", "unit", "eval()"),
                        (4, 6, "call", "unit", "right(exec())"),
                        (5, 5, "call", "unit", "exec()"),
                    ]
                ))
            );
            assert_eq!(
                native["diagnostics"],
                json!([
                    {"code":"dynamic_discovery_unresolved", "path":"unit.py", "message":"line 3: eval; runtime targets are not inferred"},
                    {"code":"dynamic_discovery_unresolved", "path":"unit.py", "message":"line 5: exec; runtime targets are not inferred"},
                ])
            );
        }
    }
    for operand in [
        "-x ** y",
        "await x ** y",
        "await value",
        "-value",
        "+value",
        "~value",
        "*xs",
        "lambda: value",
        "yield value",
    ] {
        let text = format!("eval()\nsink(await {operand})\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert!(
            !extract_captured(&python, &scan.sources).unwrap().complete,
            "{operand}"
        );
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_ne!(
            native.files[0].status,
            CandidateStatus::AcceptedFacts,
            "{operand}"
        );
        assert_eq!(
            serde_json::to_value(&native.facts).unwrap(),
            json!(expected("unit.py", &[(1, 1, "module", "unit", "unit")]))
        );
        assert!(native.diagnostics.is_empty());
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn suspension_expression_candidate_alias_constancy_dynamic_terms_and_targets() {
    let python = oracle();
    for name in [
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
        for value in [
            "await probe()",
            "(yield probe())",
            "(yield from probe())",
            "(probe := probe())",
        ] {
            let text = format!("from arbitrary import {name} as probe\nsink({value})\nprobe()\n");
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            let native = assert_warning_candidates_match_ast(&python, &scan);
            assert_eq!(
                native["diagnostics"],
                json!([
                    {"code":"dynamic_discovery_unresolved", "path":"unit.py", "message":format!("line 2: arbitrary.{name}; runtime targets are not inferred")},
                    {"code":"dynamic_discovery_unresolved", "path":"unit.py", "message":format!("line 3: arbitrary.{name}; runtime targets are not inferred")},
                ])
            );
        }
        for value in [
            "lambda probe: (probe := probe())",
            "[probe := probe() for probe in xs]",
            "{await probe() for probe in xs}",
            "{probe:(yield probe()) for probe in xs}",
            "((yield from probe()) for probe in xs)",
            "[x for items[(probe := probe())] in xs]",
            "[x for (await probe()).field in xs]",
        ] {
            let text = format!("from arbitrary import {name} as probe\nsink({value})\nprobe()\n");
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            let native = assert_warning_candidates_match_ast(&python, &scan);
            assert_eq!(
                native["diagnostics"].as_array().unwrap().len(),
                2,
                "{name}: {value}"
            );
        }
    }
    for value in [
        "await probe()",
        "(yield probe())",
        "(yield from probe())",
        "(probe := probe())",
    ] {
        for statement in [
            format!("items[{value}] = rhs"),
            format!("({value}).field = rhs"),
            format!("items[{value}] += rhs"),
            format!("del items[{value}]"),
            format!("for items[{value}] in xs: pass"),
            format!("with ctx as ({value}).field: pass"),
            format!("if {value}: pass"),
            format!("while {value}: break"),
            format!("try: pass\nexcept {value}: pass"),
            format!("raise {value} from ({value})"),
            format!("assert {value}, {value}"),
        ] {
            let text = format!("from arbitrary import eval as probe\n{statement}\nprobe()\n");
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            assert_warning_candidates_match_ast(&python, &scan);
        }
    }
    // Rebinding even to a plain literal does not update the helper's import table.
    let (_, scan) = capture(&[(
        "unit.py",
        b"from arbitrary import eval as probe\n(probe := None)\nprobe()\n",
    )]);
    let native = assert_warning_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (
                    1,
                    1,
                    "from_import",
                    "unit",
                    r#"{"level":0,"module":"arbitrary","names":[{"asname":"probe","name":"eval"}]}"#
                ),
                (1, 1, "module", "unit", "unit"),
                (3, 3, "call", "unit", "probe()"),
            ]
        ))
    );
    assert_eq!(
        native["diagnostics"],
        json!([{"code":"dynamic_discovery_unresolved", "path":"unit.py", "message":"line 3: arbitrary.eval; runtime targets are not inferred"}])
    );
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn suspension_expression_candidate_opaque_budgets_and_fresh_hidden_callees() {
    let python = oracle();
    for (value, opaque) in [
        ("await eval()", "<Await>"),
        ("(yield eval())", "<Yield>"),
        ("(yield from eval())", "<YieldFrom>"),
        ("(x := eval())", "<NamedExpr>"),
    ] {
        for extra in [1, 2, 3] {
            let mut items = vec!["U[a,a,a,a,a,a]"; 8];
            items.extend(std::iter::repeat_n("a", extra));
            items.push(value);
            let text = format!("sink(T[{}])\n", items.join(","));
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            let native = assert_warning_candidates_match_ast(&python, &scan);
            let mut rendered = vec!["U[[a, a, a, a, a, a]]"; 8];
            rendered.extend(std::iter::repeat_n("a", extra));
            rendered.push(if extra == 3 { "..." } else { opaque });
            let shape = format!("sink(T[[{}]])", rendered.join(", "));
            assert_eq!(
                native["facts"],
                json!(expected(
                    "unit.py",
                    &[
                        (1, 1, "call", "unit", "eval()"),
                        (1, 1, "call", "unit", &shape),
                        (1, 1, "module", "unit", "unit"),
                    ]
                )),
                "{value}: {extra}"
            );
        }
        for depth in [11, 12, 13] {
            let text = format!("sink({}{value}{})\n", "[".repeat(depth), "]".repeat(depth));
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            assert_warning_candidates_match_ast(&python, &scan);
        }
        for count in [16, 17] {
            let args = vec![value; count].join(",\n");
            let keywords = (0..count)
                .map(|n| format!("k{n}={value}"))
                .collect::<Vec<_>>()
                .join(",\n");
            let text = format!("sink({args},\n{keywords})\n");
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            let native = assert_warning_candidates_match_ast(&python, &scan);
            assert_eq!(native["diagnostics"].as_array().unwrap().len(), count * 2);
            let mut rendered = vec![opaque.to_owned(); 16];
            if count == 17 {
                rendered.push("...".into());
            }
            rendered.extend((0..16).map(|n| format!("k{n}={opaque}")));
            if count == 17 {
                rendered.push("...".into());
            }
            let call = format!("sink({})", rendered.join(", "));
            let call = &call[..call.len().min(512)];
            let mut rows = vec![
                (1, 1, "module", "unit", "unit"),
                (1, count * 2, "call", "unit", call),
            ];
            rows.extend((1..=count * 2).map(|line| (line, line, "call", "unit", "eval()")));
            // The outer Call starts alongside its first argument, not after it.
            rows.sort_by_key(|row| (row.0, row.1, row.2, row.3, row.4));
            assert_eq!(native["facts"], json!(expected("unit.py", &rows)));
        }
        for size in [511, 512, 513] {
            let text = format!("{}({value})\n", "a".repeat(size));
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            assert_warning_candidates_match_ast(&python, &scan);
        }
        let heavy = format!(
            "[{}, ({value}).eval()]",
            ["[a,a,a,a,a,a,a,a,a,a,a,a,a,a,a,a]"; 16].join(",")
        );
        for text in [
            format!("sink({heavy}, ({value}).eval())\n"),
            format!("sink({}, ({value}).eval())\n", ["a"; 17].join(",")),
            format!("{}(({value}).eval())\n", "a".repeat(513)),
        ] {
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            assert_warning_candidates_match_ast(&python, &scan);
        }
    }
    let wide = ["item"; 100].join(",");
    for (value, shape) in [
        (format!("await [{wide}, eval()]"), "<Await>"),
        (format!("(yield [{wide}, eval()])"), "<Yield>"),
        (format!("(yield from [{wide}, eval()])"), "<YieldFrom>"),
        (format!("(x := [{wide}, eval()])"), "<NamedExpr>"),
    ] {
        let text = format!("sink({value}, tail)\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_warning_candidates_match_ast(&python, &scan);
        let call = format!("sink({shape}, tail)");
        assert_eq!(
            native["facts"],
            json!(expected(
                "unit.py",
                &[
                    (1, 1, "call", "unit", "eval()"),
                    (1, 1, "call", "unit", &call),
                    (1, 1, "module", "unit", "unit"),
                ]
            ))
        );
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn suspension_expression_candidate_defaults_headers_and_metadata_guards() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};
    let python = oracle();
    for (value, shape) in [
        ("await value", "<Await>"),
        ("(yield value)", "<Yield>"),
        ("(yield from values)", "<YieldFrom>"),
        ("(written := value)", "<NamedExpr>"),
    ] {
        let text = format!("def f(arg={value}): pass\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        assert_eq!(
            native["facts"],
            json!(expected(
                "unit.py",
                &[
                    (1, 1, "function", "unit.f", "f"),
                    (1, 1, "module", "unit", "unit"),
                    (1, 1, "parameter", "unit.f.arg", "arg"),
                    (1, 1, "parameter_default", "unit.f.arg", shape),
                    (
                        1,
                        1,
                        "signature",
                        "unit.f",
                        r#"{"keywordOnly":[],"positional":["arg"],"positionalOnly":0}"#
                    ),
                ]
            ))
        );
        for header in [
            format!("class Box(({value}, hidden())): pass"),
            format!("def f(arg: ({value}, hidden())): pass"),
            format!("def f() -> ({value}, hidden()): pass"),
            format!("field: ({value}, hidden())"),
            format!("@decorate({value})\ndef f(): pass"),
            format!("self.router = {value}"),
            format!("self.prefix += {value}"),
            format!("del ({value}).router"),
        ] {
            let text = format!("eval()\nclass Before: pass\n{header}\nexec()\n");
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            assert!(
                extract_captured(&python, &scan.sources).unwrap().complete,
                "{header}"
            );
            let native = extract_fact_candidates(&scan.sources).unwrap();
            assert_eq!(
                native.files[0].status,
                CandidateStatus::UnsupportedSyntax,
                "{header}"
            );
            assert_eq!(
                serde_json::to_value(&native.facts).unwrap(),
                json!(expected("unit.py", &[(1, 1, "module", "unit", "unit")]))
            );
            assert!(native.diagnostics.is_empty());
        }
    }
    for value in [
        "lambda: await value",
        "[await value for x in xs]",
        "{(yield key):(x := value) for x in xs}",
        "((yield from values) for x in xs)",
        "(outer := (inner := value))",
    ] {
        let text = format!("def f(arg={value}): pass\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
    for hidden in [
        "eval()",
        "FastAPI()",
        "pkg.APIRouter()",
        "app.include_router(child)",
    ] {
        for value in [
            format!("await {hidden}"),
            format!("(yield {hidden})"),
            format!("(yield from {hidden})"),
            format!("(x := {hidden})"),
        ] {
            let mut contexts = vec![
                format!("def f(arg={value}): pass"),
                format!("def f(arg=lambda: [{value} for item in xs]): pass"),
                format!("def f(arg=[item for items[{value}] in xs]): pass"),
                format!("def f(arg=[item for item in xs if ({value})]): pass"),
                format!("def f(arg=lambda a=({value}): a): pass"),
            ];
            if hidden != "eval()" {
                contexts.extend([
                    format!("sink({value})"),
                    format!("sink(lambda: [{value} for item in xs])"),
                    format!("sink([item for items[{value}] in xs])"),
                    format!("sink({}, {value})", ["a"; 17].join(",")),
                ]);
            }
            for context in contexts {
                let text = format!("eval()\nclass Before: pass\n{context}\nexec()\n");
                let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
                assert!(
                    extract_captured(&python, &scan.sources).unwrap().complete,
                    "{context}"
                );
                let native = extract_fact_candidates(&scan.sources).unwrap();
                assert_eq!(
                    native.files[0].status,
                    CandidateStatus::UnsupportedSyntax,
                    "{context}"
                );
                assert_eq!(
                    serde_json::to_value(&native.facts).unwrap(),
                    json!(expected("unit.py", &[(1, 1, "module", "unit", "unit")]))
                );
                assert!(native.diagnostics.is_empty(), "{context}");
            }
        }
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn suspension_expression_candidate_replay_isolation_resources_and_no_execution() {
    use super::python_native::{extract_fact_candidates, parse_batch, CandidateStatus};
    let python = oracle();
    let (root, _) = capture(&[("placeholder.py", b"")]);
    let marker = root.path().join("executed");
    let first_source = format!("from arbitrary import eval as probe\n(probe := None)\nclass Box(Base):\n    field: T = (written := value)\n    def run(arg=(yield DEFAULT)):\n        try:\n            sink(await probe() ** other(), (yield from probe()), (x := probe()))\n        except (error := E):\n            assert [await probe() for items[(index := probe())] in xs]\n        return (yield probe())\n(await open({:?}, 'w')).write('PRIVATE_MARKER')\n", marker.to_str().unwrap());
    let entries: &[(&str, &[u8])] = &[
        ("a.py", first_source.as_bytes()),
        (
            "b.py",
            b"probe()\nsink((yield from values()), [x := x for x in xs])\n",
        ),
        ("c.py", b"eval()\nsink(await FastAPI())\n"),
        ("d.py", b"eval()\nsink((x := probe()))\nlater = (\n"),
    ];
    let (sources_root, mut scan) = capture(entries);
    let (_good_root, good_scan) = capture(&entries[..2]);
    let good = assert_warning_candidates_match_ast(&python, &good_scan);
    let native = extract_fact_candidates(&scan.sources).unwrap();
    assert_eq!(native.files[2].status, CandidateStatus::UnsupportedSyntax);
    assert_eq!(native.files[3].status, CandidateStatus::SyntaxError);
    let first = serde_json::to_value(native).unwrap();
    let mut facts = good["facts"].as_array().unwrap().clone();
    facts.extend(expected("c.py", &[(1, 1, "module", "c", "c")]));
    facts.extend(expected("d.py", &[(1, 1, "module", "d", "d")]));
    assert_eq!(first["facts"], json!(facts));
    assert_eq!(first["diagnostics"], good["diagnostics"]);
    let ast = extract_captured(&python, &scan.sources).unwrap();
    assert!(!marker.exists());
    for (path, _) in entries {
        fs::remove_file(sources_root.path().join(path)).unwrap();
    }
    assert_eq!(
        first,
        serde_json::to_value(extract_fact_candidates(&scan.sources).unwrap()).unwrap()
    );
    assert_eq!(
        semantics(&ast),
        semantics(&extract_captured(&python, &scan.sources).unwrap())
    );
    scan.sources.reverse();
    let reversed = serde_json::to_value(extract_fact_candidates(&scan.sources).unwrap()).unwrap();
    assert_eq!(first["facts"], reversed["facts"]);
    assert_eq!(first["diagnostics"], reversed["diagnostics"]);
    assert_eq!(
        semantics(&ast),
        semantics(&extract_captured(&python, &scan.sources).unwrap())
    );
    assert!(!marker.exists());
    let text =
        "sink(await left(eval()) ** await right(exec()), (yield from values()), (x := probe()))\n"
            .repeat(80);
    let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
    let native = assert_warning_candidates_match_ast(&python, &scan);
    let parsed = parse_batch(&scan.sources).unwrap();
    assert_eq!(
        native["files"][0]["nodes_visited"],
        json!(parsed.files[0].nodes_visited)
    );
    assert_eq!(
        native["files"][0]["max_depth"],
        json!(parsed.files[0].max_depth)
    );
    assert_eq!(native["files"][0]["traversal_finished"], true);
    assert_eq!(native["diagnostics"].as_array().unwrap().len(), 160);
    for value in [
        format!("await {}eval(){}", "nest(".repeat(180), ")".repeat(180)),
        format!("(yield {}eval(){})", "nest(".repeat(180), ")".repeat(180)),
        format!(
            "(yield from {}eval(){})",
            "nest(".repeat(180),
            ")".repeat(180)
        ),
        format!("(x := {}eval(){})", "nest(".repeat(180), ")".repeat(180)),
    ] {
        let text = format!("eval()\nsink({value})\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert!(extract_captured(&python, &scan.sources).unwrap().complete);
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_eq!(
            native.files[0].status,
            CandidateStatus::ResourceLimit,
            "{value}"
        );
        assert_eq!(
            serde_json::to_value(&native.facts).unwrap(),
            json!(expected("unit.py", &[(1, 1, "module", "unit", "unit")]))
        );
        assert!(native.diagnostics.is_empty());
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn closure_expression_candidate_five_shapes_roots_callees_and_receivers() {
    let python = oracle();
    for (value, shape) in [
        ("lambda: hidden()", "<Lambda>"),
        ("[hidden() for x in xs]", "<ListComp>"),
        ("{hidden() for x in xs}", "<SetComp>"),
        ("{x:hidden() for x in xs}", "<DictComp>"),
        ("(hidden() for x in xs)", "<GeneratorExp>"),
    ] {
        let text = format!("root = {value}\nsink({value})\n({value})()\n({value}).method()\nself.field = {value}\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        assert_eq!(
            native["identity"]["policy"],
            "python-native-private-facts-15"
        );
        let sink = format!("sink({shape})");
        let call = format!("{shape}()");
        let method = format!("{shape}.method()");
        assert_eq!(
            native["facts"],
            json!(expected(
                "unit.py",
                &[
                    (1, 1, "binding_invalidation", "unit", r#"{"target":"root"}"#),
                    (1, 1, "call", "unit", "hidden()"),
                    (1, 1, "module", "unit", "unit"),
                    (2, 2, "call", "unit", "hidden()"),
                    (2, 2, "call", "unit", &sink),
                    (3, 3, "call", "unit", &call),
                    (3, 3, "call", "unit", "hidden()"),
                    (4, 4, "call", "unit", &method),
                    (4, 4, "call", "unit", "hidden()"),
                    (
                        5,
                        5,
                        "binding_invalidation",
                        "unit",
                        r#"{"target":"self.field"}"#
                    ),
                    (5, 5, "call", "unit", "hidden()"),
                ]
            ))
        );
        // All ordinary Body dispatch sites, including Load children of Store targets.
        for statement in [
            format!("assert {value}, {value}"),
            format!("raise {value} from ({value})"),
            format!("if {value}: pass"),
            format!("while {value}: break"),
            format!("for item in {value}: pass"),
            format!("with ({value}) as item: pass"),
            format!("try: pass\nexcept {value}: pass"),
            format!("items[({value})] = rhs"),
            format!("({value}).field = rhs"),
            format!("items[({value})] += rhs"),
            format!("del items[({value})]"),
            format!("for items[({value})] in xs: pass"),
            format!("with ctx as ({value}).field: pass"),
            format!("def f():\n    return {value}"),
            format!("field: T = {value}"),
        ] {
            let (_, scan) = capture(&[("unit.py", statement.as_bytes())]);
            assert_declaration_candidates_match_ast(&python, &scan);
        }
    }
    for text in [
        "sink(flag or (lambda: eval()))\n",
        "sink(flag and (lambda: eval()))\n",
        "sink(not (lambda: eval()))\n",
        "sink(left if (lambda: test()) else right)\n",
        "sink(lambda: flag or eval())\n",
        "sink(left if ready else lambda: eval())\n",
        "def f(value=flag or (lambda: item)): pass\n",
        "try: pass\nexcept flag or (lambda: eval()) as error: pass\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_warning_candidates_match_ast(&python, &scan);
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn closure_expression_candidate_lambda_parameters_are_not_definitions() {
    let python = oracle();
    // Duplicate names are ast.parse-positive even when compile rejects them.
    for parameters in [
        "",
        "a",
        "a,",
        "a,b",
        "a, /",
        "a, /, b=one",
        "a=one, /, b=two",
        "*rest",
        "**opts",
        "*rest, **opts",
        "*, key",
        "*, key=one",
        "a, /, b=one, *rest, key, other=two, **opts",
        "a=one, b=two, *rest, key, **opts",
        "a,a",
        "a,/,a",
        "a,*,a",
        "*a,**a",
        "a=one,*,b",
        "a=lambda: one, *, b=lambda: two",
        "*rest,",
        "**opts,",
        "a,/,",
        "a,*a",
        "a,**a",
        "a=one,/,a=two,*,a=three",
        "a=lambda: one,*,b=lambda x=two: x",
    ] {
        let text = format!("sink(lambda {parameters}: body())\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        assert_eq!(
            native["facts"],
            json!(expected(
                "unit.py",
                &[
                    (1, 1, "call", "unit", "body()"),
                    (1, 1, "call", "unit", "sink(<Lambda>)"),
                    (1, 1, "module", "unit", "unit"),
                ]
            )),
            "parameters: {parameters}"
        );
    }
    for text in [
        "sink(lambda: lambda: body())\n",
        "sink(lambda a=pos(), /, b=other(), *rest, k=kw(), **opts: body())\n",
        "sink(\n    lambda a=one, /,\n        b=two, *, k=three:\n        body()\n)\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn closure_expression_candidate_lambda_defaults_body_aliases_and_warnings() {
    let python = oracle();
    for name in [
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
        let text = format!("from arbitrary import {name} as probe\nsink(lambda probe=probe(), *, key=probe(): probe())\nprobe()\nclass Box:\n    sink(lambda probe=probe(), *, key=probe(): probe())\n    def run():\n        sink(lambda probe=probe(), *, key=probe(): probe())\n        probe()\nprobe()\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_warning_candidates_match_ast(&python, &scan);
        let mut diagnostics = Vec::new();
        for (line, count) in [(2, 3), (3, 1), (5, 3), (7, 3), (8, 1), (9, 1)] {
            for _ in 0..count {
                diagnostics.push(json!({"code":"dynamic_discovery_unresolved", "path":"unit.py",
                    "message":format!("line {line}: arbitrary.{name}; runtime targets are not inferred")}));
            }
        }
        assert_eq!(native["diagnostics"], json!(diagnostics), "{name}");
        assert!(native["facts"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| !f["symbol"].as_str().unwrap().contains("<lambda>")
                && !matches!(
                    f["kind"].as_str().unwrap(),
                    "parameter" | "parameter_default" | "binding" | "binding_invalidation"
                )));
    }
    // Keyword defaults precede positional defaults in AST field order, but no
    // expression can change aliases; globally sorted observable vectors agree.
    let (_, scan) = capture(&[("unit.py", b"sink(lambda a=pos(), *, b=kw(): body())\n")]);
    let native = assert_declaration_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (1, 1, "call", "unit", "body()"),
                (1, 1, "call", "unit", "kw()"),
                (1, 1, "call", "unit", "pos()"),
                (1, 1, "call", "unit", "sink(<Lambda>)"),
                (1, 1, "module", "unit", "unit"),
            ]
        ))
    );
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn closure_expression_candidate_comprehension_clauses_targets_and_owners() {
    let python = oracle();
    let expression = "[result() async for (self.router, [items[index()], *rest]) in source() if first() if second() for factory().prefix in more() if final()]";
    for (prefix, indent, owner, end) in [
        ("", "", "unit", 1),
        ("class Box:\n", "    ", "unit.Box", 2),
        (
            "class Box:\n    def run():\n",
            "        ",
            "unit.Box.run",
            3,
        ),
    ] {
        let text = format!("{prefix}{indent}{expression}\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        let mut rows = vec![(1, 1, "module", "unit", "unit")];
        if end > 1 {
            rows.push((1, end, "class", "unit.Box", "Box"));
        }
        if end == 3 {
            rows.push((2, 3, "function", owner, "run"));
            rows.push((
                2,
                3,
                "signature",
                owner,
                r#"{"keywordOnly":[],"positional":[],"positionalOnly":0}"#,
            ));
        }
        for call in [
            "factory()",
            "final()",
            "first()",
            "index()",
            "more()",
            "result()",
            "second()",
            "source()",
        ] {
            rows.push((end, end, "call", owner, call));
        }
        // At module level same-line call facts precede the module baseline.
        if end == 1 {
            rows.rotate_left(1);
        }
        assert_eq!(native["facts"], json!(expected("unit.py", &rows)));
    }
    for result in [
        "[probe() for probe in xs]",
        "{probe() async for probe in xs}",
        "{probe(): probe() for probe in xs}",
        "(probe() async for probe in xs)",
        "[probe() for self.router in xs for cls.prefix in ys]",
        "[probe() for *probe in xs]",
        "[probe() for a,*probe in xs]",
        "[probe() for (lambda: probe()).field in xs]",
        "[probe() for items[(probe() for x in xs)] in xs]",
        "[probe() for probe in (lambda: probe()) if (lambda: probe())]",
        "[[*xs] for x in xs]",
        "[{**opts} for x in xs]",
        "[x for x in (a,b)]",
        "[x for x in (a if b else c)]",
        "[x for x in xs if (a if b else c)]",
    ] {
        let text = format!("from arbitrary import eval as probe\n{result}\nprobe()\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_warning_candidates_match_ast(&python, &scan);
        assert!(native["facts"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| !matches!(
                f["kind"].as_str().unwrap(),
                "binding" | "binding_invalidation" | "router_mutation"
            )));
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn closure_expression_candidate_generator_call_grouping_and_byte_spans() {
    let python = oracle();
    let text = "sink(\n    result() for x in source()\n)\nsink((result() for x in source()), neighbor(), key=value())\nsink((x for x in xs),)\n";
    for newline in ["\n", "\r\n"] {
        for bom in ["", "\u{feff}"] {
            let text = format!("{bom}{text}").replace('\n', newline);
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            let native = assert_declaration_candidates_match_ast(&python, &scan);
            assert_eq!(
                native["facts"],
                json!(expected(
                    "unit.py",
                    &[
                        (1, 1, "module", "unit", "unit"),
                        (1, 3, "call", "unit", "sink(<GeneratorExp>)"),
                        (2, 2, "call", "unit", "result()"),
                        (2, 2, "call", "unit", "source()"),
                        (4, 4, "call", "unit", "neighbor()"),
                        (4, 4, "call", "unit", "result()"),
                        (
                            4,
                            4,
                            "call",
                            "unit",
                            "sink(<GeneratorExp>, neighbor(), key=value())"
                        ),
                        (4, 4, "call", "unit", "source()"),
                        (4, 4, "call", "unit", "value()"),
                        (5, 5, "call", "unit", "sink(<GeneratorExp>)"),
                    ]
                ))
            );
        }
    }
    for text in [
        "sink(x async for x in xs)\n",
        "sink((x for x in xs), other, k=value)\n",
        "sink(other, (x for x in xs), *args, **opts)\n",
        "sink(inner(x for x in source()) for y in outer())\n",
        "sink(*(x for x in source()), **{k:v for k in keys()})\n",
        "sink(\n    result()\n    async for items[index()] in source()\n    if test()\n)\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
    for count in [16, 17] {
        let args = vec!["(eval() for x in xs)"; count].join(",");
        let keywords = (0..count)
            .map(|n| format!("k{n}={{x:eval() for x in xs}}"))
            .collect::<Vec<_>>()
            .join(",");
        let text = format!("sink({args},{keywords})\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_warning_candidates_match_ast(&python, &scan);
        assert_eq!(native["diagnostics"].as_array().unwrap().len(), 2 * count);
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn closure_expression_candidate_defaults_headers_and_metadata_guards() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};
    let python = oracle();
    for (value, shape) in [
        ("lambda a=one, /, *, b=two: a", "<Lambda>"),
        ("[x for x in xs if flag]", "<ListComp>"),
        ("{x for x in xs}", "<SetComp>"),
        ("{x:y for x in xs}", "<DictComp>"),
        ("(x async for x in xs)", "<GeneratorExp>"),
    ] {
        let text = format!("def f(value={value}): pass\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        assert_eq!(
            native["facts"],
            json!(expected(
                "unit.py",
                &[
                    (1, 1, "function", "unit.f", "f"),
                    (1, 1, "module", "unit", "unit"),
                    (1, 1, "parameter", "unit.f.value", "value"),
                    (1, 1, "parameter_default", "unit.f.value", shape),
                    (
                        1,
                        1,
                        "signature",
                        "unit.f",
                        r#"{"keywordOnly":[],"positional":["value"],"positionalOnly":0}"#
                    ),
                ]
            ))
        );
        for header in [
            format!("class C(({value}, hidden())): pass"),
            format!("def f(x: ({value}, hidden())): pass"),
            format!("def f() -> ({value}, hidden()): pass"),
            format!("x: ({value}, hidden())"),
            format!("@decorate({value})\ndef f(): pass"),
        ] {
            let (_, scan) = capture(&[("unit.py", header.as_bytes())]);
            assert!(
                extract_captured(&python, &scan.sources).unwrap().complete,
                "{header}"
            );
            let native = extract_fact_candidates(&scan.sources).unwrap();
            assert_ne!(
                native.files[0].status,
                CandidateStatus::AcceptedFacts,
                "{header}"
            );
            assert_eq!(native.facts.len(), 1);
            assert!(native.diagnostics.is_empty());
        }
    }
    for hidden in [
        "eval()",
        "FastAPI()",
        "pkg.APIRouter()",
        "app.include_router(child)",
    ] {
        for value in [
            format!("lambda a={hidden}: a"),
            format!("lambda *, a={hidden}: a"),
            format!("lambda: {hidden}"),
            format!("lambda a=lambda: {hidden}: a"),
            format!("[{hidden} for x in xs]"),
            format!("{{{hidden} for x in xs}}"),
            format!("{{{hidden}: x for x in xs}}"),
            format!("{{x: {hidden} for x in xs}}"),
            format!("({hidden} for x in xs)"),
            format!("[x for items[{hidden}] in xs]"),
            format!("[x for ({hidden}).field in xs]"),
            format!("[x for x in {hidden}]"),
            format!("[x for x in xs if {hidden}]"),
        ] {
            // Calls reject recursively in Default; G3 triggers also reject in Body.
            let mut contexts = vec![format!("def f(x={value}): pass")];
            if hidden != "eval()" {
                contexts.push(format!("sink({value})"));
            }
            for context in contexts {
                let text = format!("eval()\nclass Before: pass\n{context}\nexec()\n");
                let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
                assert!(
                    extract_captured(&python, &scan.sources).unwrap().complete,
                    "{context}"
                );
                let native = extract_fact_candidates(&scan.sources).unwrap();
                assert_eq!(
                    native.files[0].status,
                    CandidateStatus::UnsupportedSyntax,
                    "{context}"
                );
                assert_eq!(
                    serde_json::to_value(&native.facts).unwrap(),
                    json!(expected("unit.py", &[(1, 1, "module", "unit", "unit"),]))
                );
                assert!(native.diagnostics.is_empty(), "{context}");
            }
        }
    }
    for value in [
        "lambda a=lambda: one, *, b=(x for x in xs): [a for a in xs]",
        "[x for (lambda: obj).field in xs]",
        "[x for items[(x for x in xs)] in xs if (lambda: flag)]",
        "{(lambda: key):(x for x in xs) for x in (lambda: xs)}",
        "({x for x in xs} async for x in xs)",
    ] {
        let text = format!("def f(value={value}): pass\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn closure_expression_candidate_opaque_render_budgets_and_hidden_call_roles() {
    let python = oracle();
    let heavy = format!(
        "[{},eval()]",
        ["[a,a,a,a,a,a,a,a,a,a,a,a,a,a,a,a]"; 16].join(",")
    );
    for (value, shape) in [
        (format!("lambda a={heavy}, *, b=exec(): body()"), "<Lambda>"),
        (
            format!("[{heavy} for items[index()] in source() if test()]"),
            "<ListComp>",
        ),
        (format!("{{{heavy} for x in xs}}"), "<SetComp>"),
        (format!("{{{heavy}:exec() for x in xs}}"), "<DictComp>"),
        (format!("({heavy} for x in xs)"), "<GeneratorExp>"),
    ] {
        let text = format!("sink({value}, tail)\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_warning_candidates_match_ast(&python, &scan);
        assert!(native["facts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["value"] == format!("sink({shape}, tail)")));
        for count in [16, 17] {
            let text = format!("sink({})\n", vec![value.as_str(); count].join(","));
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            let native = assert_warning_candidates_match_ast(&python, &scan);
            let rendered = format!(
                "sink({}{})",
                vec![shape; 16].join(", "),
                if count == 17 { ", ..." } else { "" }
            );
            assert!(native["facts"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["value"] == rendered));
        }
        for depth in [11, 12, 13] {
            let text = format!("sink({}{value}{})\n", "[".repeat(depth), "]".repeat(depth));
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            assert_warning_candidates_match_ast(&python, &scan);
        }
        for size in [511, 512, 513] {
            let text = format!("{}({value})\n", "a".repeat(size));
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            assert_warning_candidates_match_ast(&python, &scan);
        }
    }
    // Place an opaque node at visits 79/80/81 of a fresh 80-node shape.
    for (value, opaque) in [
        ("lambda: eval()", "<Lambda>"),
        ("[eval() for x in xs]", "<ListComp>"),
        ("{eval() for x in xs}", "<SetComp>"),
        ("{x:eval() for x in xs}", "<DictComp>"),
        ("(eval() for x in xs)", "<GeneratorExp>"),
    ] {
        for extra in [1, 2, 3] {
            let mut items = vec!["U[a,a,a,a,a,a]"; 8];
            items.extend(std::iter::repeat_n("a", extra));
            items.push(value);
            let text = format!("sink(T[{}])\n", items.join(","));
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            let native = assert_warning_candidates_match_ast(&python, &scan);
            let mut rendered = vec!["U[[a, a, a, a, a, a]]"; 8];
            rendered.extend(std::iter::repeat_n("a", extra));
            rendered.push(if extra == 3 { "..." } else { opaque });
            let shape = format!("sink(T[[{}]])", rendered.join(", "));
            assert_eq!(
                native["facts"],
                json!(expected(
                    "unit.py",
                    &[
                        (1, 1, "call", "unit", "eval()"),
                        (1, 1, "call", "unit", &shape),
                        (1, 1, "module", "unit", "unit"),
                    ]
                )),
                "80-node boundary: {value}, extra={extra}"
            );
        }
    }
    for value in [
        "lambda a=eval(), *, b=exec(): body()",
        "lambda: eval()",
        "[eval() for x in xs]",
        "{eval() for x in xs}",
        "{eval():exec() for x in xs}",
        "(eval() for x in xs)",
        "[x for items[eval()] in xs]",
        "[x for eval().field in xs]",
        "[x for x in eval()]",
        "[x for x in xs if eval()]",
    ] {
        for text in [
            format!("sink({heavy}, {value})\n"),
            format!("sink({}, {value})\n", ["a"; 17].join(",")),
            format!("sink({}, {value})\n", "a".repeat(513)),
        ] {
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            assert_warning_candidates_match_ast(&python, &scan);
        }
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn closure_expression_candidate_malformed_grammar_and_future_forms_rollback() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};
    let python = oracle();
    let mut malformed = Vec::new();
    for parameters in [
        "/",
        "*",
        "*, **opts",
        "a, /, /",
        "a, *rest, /",
        "**opts, a",
        "*a, *b",
        "**a, **b",
        "a=one,b",
        "a=one,/,b",
        "a:T",
        "a:T=one",
        "(a,b)",
        "(a,b)=one",
        "*obj.attr",
        "**obj[index]",
        "*rest=one",
        "True",
    ] {
        malformed.push(format!("sink(lambda {parameters}: body())"));
    }
    for value in [
        "flag or lambda: exec()",
        "flag and lambda: eval()",
        "not lambda: eval()",
        "left if lambda: test() else right",
        "[x for x in a,b]",
        "[x for x in xs,]",
        "[x for x in lambda: xs]",
        "[x for x in a if b else c]",
        "[x for x in xs if lambda: flag]",
        "[x for x in xs if a if b else c]",
        "[x for x in flag or lambda: xs]",
        "[x for x in xs if flag or lambda: ready]",
        "[*x for x in xs]",
        "{*x for x in xs}",
        "(*x for x in xs)",
        "{**x for x in xs}",
        "{*x:y for x in xs}",
        "{x:*y for x in xs}",
        "[x for x in *xs]",
        "[x for x in xs if *flag]",
        "[x for call() in xs]",
        "[x for (lambda: y) in xs]",
        "[x if flag for x in xs]",
        "[for x in xs]",
        "[x for in xs]",
        "[x for x in]",
        "lambda: ",
    ] {
        malformed.push(format!("sink({value})"));
    }
    for arguments in [
        "x for x in xs,",
        "x for x in xs,other",
        "x for x in xs,key=v",
        "key=v,x for x in xs",
        "other,x for x in xs",
        "*x for x in xs",
    ] {
        malformed.push(format!("sink({arguments})"));
    }
    malformed.push("def f(value=flag or lambda: item): pass".into());
    malformed.push("try: pass\nexcept flag or lambda: eval() as error: pass".into());
    for (forms, ast_failure) in [
        (malformed, true),
        (
            vec![
                "def f(value=lambda: (yield work())): pass".into(),
                "def f(value=lambda: (yield from work())): pass".into(),
                "def f(value=lambda: await work()): pass".into(),
                "def f(value=lambda: (x := work())): pass".into(),
                "def f(value=[await work() for x in xs]): pass".into(),
                "def f(value={x:(y := work()) for x in xs}): pass".into(),
                "def f(value=(x for x in xs if (y := work()))): pass".into(),
                "match value:\n    case _: FastAPI()".into(),
            ],
            false,
        ),
    ] {
        for form in forms {
            let text = format!("eval()\nclass Before: pass\n{form}\nclass After: pass\nexec()\n");
            let (_, scan) = capture(&[
                ("bad.py", text.as_bytes()),
                ("good.py", b"sink(lambda: eval())\n"),
            ]);
            let ast = extract_captured(&python, &scan.sources).unwrap();
            if ast_failure {
                assert!(!ast.complete, "AST accepted {form}");
                assert_eq!(ast.diagnostics[0].code, "parse_failure", "{form}");
            }
            let native = extract_fact_candidates(&scan.sources).unwrap();
            assert_ne!(
                native.files[0].status,
                CandidateStatus::AcceptedFacts,
                "{form}"
            );
            assert_eq!(native.files[1].status, CandidateStatus::AcceptedFacts);
            let mut facts = expected("bad.py", &[(1, 1, "module", "bad", "bad")]);
            facts.extend(expected(
                "good.py",
                &[
                    (1, 1, "call", "good", "eval()"),
                    (1, 1, "call", "good", "sink(<Lambda>)"),
                    (1, 1, "module", "good", "good"),
                ],
            ));
            assert_eq!(
                serde_json::to_value(&native.facts).unwrap(),
                json!(facts),
                "{form}"
            );
            assert_eq!(
                serde_json::to_value(&native.diagnostics).unwrap(),
                json!([
                    {"code":"dynamic_discovery_unresolved", "path":"good.py",
                     "message":"line 1: eval; runtime targets are not inferred"}
                ]),
                "{form}"
            );
        }
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn closure_expression_candidate_exception_lambda_alias_normalization() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};
    let python = oracle();
    for marker in ["except", "except*"] {
        for value in [
            "lambda: foo()",
            "(lambda: foo())",
            "lambda: lambda: foo()",
            "lambda a=foo(): a",
            "lambda *, a=foo(): a",
            "(lambda a=foo(): a)",
            "(lambda: foo(), E)",
            "lambda: left() if test() else foo()",
            "left() if test() else lambda: foo()",
            "lambda: left() if test() else lambda: foo()",
        ] {
            let text = format!("from arbitrary import eval as err\ntry: pass\n{marker} {value} as err: pass\nerr()\n");
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            let native = assert_warning_candidates_match_ast(&python, &scan);
            let mut rows = vec![
                (
                    1,
                    1,
                    "from_import",
                    "unit",
                    r#"{"level":0,"module":"arbitrary","names":[{"asname":"err","name":"eval"}]}"#,
                ),
                (1, 1, "module", "unit", "unit"),
                (3, 3, "call", "unit", "foo()"),
            ];
            if value.contains("left()") {
                rows.push((3, 3, "call", "unit", "left()"));
                rows.push((3, 3, "call", "unit", "test()"));
            }
            rows.push((4, 4, "call", "unit", "err()"));
            assert_eq!(
                native["facts"],
                json!(expected("unit.py", &rows)),
                "{marker} {value}"
            );
            assert_eq!(
                native["diagnostics"],
                json!([
                    {"code":"dynamic_discovery_unresolved", "path":"unit.py",
                     "message":"line 4: arbitrary.eval; runtime targets are not inferred"}
                ])
            );
        }
        for value in [
            "(lambda: foo() as err)",
            "(lambda: foo() as err, E)",
            "lambda x as err: foo()",
            "lambda x=foo() as err: x",
            "lambda: (foo() as err)",
            "lambda: foo() if test() else (bar() as err)",
            "(lambda: foo(), E as err)",
        ] {
            let text = format!("eval()\ntry: pass\n{marker} {value}: pass\nexec()\n");
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            let ast = extract_captured(&python, &scan.sources).unwrap();
            assert!(!ast.complete, "AST accepted {marker} {value}");
            assert_eq!(ast.diagnostics[0].code, "parse_failure");
            let native = extract_fact_candidates(&scan.sources).unwrap();
            assert_ne!(
                native.files[0].status,
                CandidateStatus::AcceptedFacts,
                "{marker} {value}"
            );
            assert_eq!(
                serde_json::to_value(&native.facts).unwrap(),
                json!(expected("unit.py", &[(1, 1, "module", "unit", "unit"),]))
            );
            assert!(native.diagnostics.is_empty());
        }
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn closure_expression_candidate_replay_isolation_order_and_hidden_resources() {
    use super::python_native::{extract_fact_candidates, parse_batch, CandidateStatus};
    let python = oracle();
    let entries: &[(&str, &[u8])] = &[
        ("a.py", b"from arbitrary import eval as probe\nclass Box(Base):\n    field: T\n    def run(x=lambda: DEFAULT):\n        try:\n            sink(lambda probe=probe(), *, k=probe(): [probe() async for items[probe()] in source() if probe()])\n        except lambda: probe() as probe:\n            assert {probe():probe() for self.router in xs}\n        return (probe() for probe in xs)\nprobe()\n"),
        ("b.py", b"sink(x async for x in xs)\nsink((lambda: eval()), {exec() for x in xs})\n"),
        ("c.py", b"eval()\nsink(lambda: FastAPI())\n"),
        ("d.py", b"sink(lambda: eval())\nlater = (\n"),
    ];
    let (root, mut scan) = capture(entries);
    let (_good_root, good_scan) = capture(&entries[..2]);
    let good = assert_warning_candidates_match_ast(&python, &good_scan);
    let native = extract_fact_candidates(&scan.sources).unwrap();
    assert_eq!(native.files[2].status, CandidateStatus::UnsupportedSyntax);
    assert_eq!(native.files[3].status, CandidateStatus::SyntaxError);
    let first = serde_json::to_value(native).unwrap();
    let mut facts = good["facts"].as_array().unwrap().clone();
    facts.extend(expected("c.py", &[(1, 1, "module", "c", "c")]));
    facts.extend(expected("d.py", &[(1, 1, "module", "d", "d")]));
    assert_eq!(first["facts"], json!(facts));
    assert_eq!(first["diagnostics"], good["diagnostics"]);
    let ast = extract_captured(&python, &scan.sources).unwrap();
    for (path, _) in entries {
        fs::remove_file(root.path().join(path)).unwrap();
    }
    assert_eq!(
        first,
        serde_json::to_value(extract_fact_candidates(&scan.sources).unwrap()).unwrap()
    );
    assert_eq!(
        semantics(&ast),
        semantics(&extract_captured(&python, &scan.sources).unwrap())
    );
    scan.sources.reverse();
    let reversed = serde_json::to_value(extract_fact_candidates(&scan.sources).unwrap()).unwrap();
    assert_eq!(first["facts"], reversed["facts"]);
    assert_eq!(first["diagnostics"], reversed["diagnostics"]);
    assert_eq!(
        semantics(&ast),
        semantics(&extract_captured(&python, &scan.sources).unwrap())
    );
    let text = "sink(lambda a=eval(), *, b=exec(): [body() async for items[index()] in source() if test()])\n".repeat(80);
    let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
    let native = assert_warning_candidates_match_ast(&python, &scan);
    let parsed = parse_batch(&scan.sources).unwrap();
    assert_eq!(
        native["files"][0]["nodes_visited"],
        json!(parsed.files[0].nodes_visited)
    );
    assert_eq!(
        native["files"][0]["max_depth"],
        json!(parsed.files[0].max_depth)
    );
    assert_eq!(native["files"][0]["traversal_finished"], true);
    assert_eq!(native["diagnostics"].as_array().unwrap().len(), 160);
    // Opaque rendering must not hide CST depth from the counted resource walk.
    for value in [
        format!("lambda: {}eval(){}", "nest(".repeat(180), ")".repeat(180)),
        format!(
            "[{}eval(){} for x in xs]",
            "nest(".repeat(180),
            ")".repeat(180)
        ),
    ] {
        let text = format!("eval()\nsink({value})\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert!(extract_captured(&python, &scan.sources).unwrap().complete);
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_eq!(native.files[0].status, CandidateStatus::ResourceLimit);
        assert_eq!(
            serde_json::to_value(&native.facts).unwrap(),
            json!(expected("unit.py", &[(1, 1, "module", "unit", "unit"),]))
        );
        assert!(native.diagnostics.is_empty());
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn generalized_target_candidate_chains_destructuring_and_outer_statement_spans() {
    let python = oracle();
    let text = concat!(
        "first = self.field = (last) = (\n",
        "    make()\n",
        ")\n",
        "scalar = ((single,), [cls.field, *rest], other.field, items[index()]) = rhs\n",
        "(grouped) = source\n",
        "[only] = source\n",
    );
    for newline in ["\n", "\r\n"] {
        let text = text.replace('\n', newline);
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        assert_eq!(
            native["identity"]["policy"],
            "python-native-private-facts-15"
        );
        assert_eq!(
            native["facts"],
            json!(expected(
                "unit.py",
                &[
                    (1, 1, "module", "unit", "unit"),
                    (
                        1,
                        3,
                        "binding",
                        "unit",
                        r#"{"source":"make()","target":"first"}"#
                    ),
                    (
                        1,
                        3,
                        "binding",
                        "unit",
                        r#"{"source":"make()","target":"last"}"#
                    ),
                    (
                        1,
                        3,
                        "binding",
                        "unit",
                        r#"{"source":"make()","target":"self.field"}"#
                    ),
                    (2, 2, "call", "unit", "make()"),
                    (
                        4,
                        4,
                        "binding",
                        "unit",
                        r#"{"source":"rhs","target":"scalar"}"#
                    ),
                    (
                        4,
                        4,
                        "binding_invalidation",
                        "unit",
                        r#"{"target":"cls.field"}"#
                    ),
                    (4, 4, "binding_invalidation", "unit", r#"{"target":"rest"}"#),
                    (
                        4,
                        4,
                        "binding_invalidation",
                        "unit",
                        r#"{"target":"single"}"#
                    ),
                    (4, 4, "call", "unit", "index()"),
                    (
                        5,
                        5,
                        "binding",
                        "unit",
                        r#"{"source":"source","target":"grouped"}"#
                    ),
                    (6, 6, "binding_invalidation", "unit", r#"{"target":"only"}"#),
                ]
            ))
        );
    }
    // ast.parse accepts these Store shapes even though compile can reject them.
    for text in [
        "*x = make()\n",
        "a = (\n b\n) = rhs()\n",
        "(\n a\n) = b = rhs()\n",
        "a,*b,*c = make()\n",
        "a = b = c = None\n",
        "a = [b, (c, *d)] = make()\n",
        "((self).field) = ((cls).field) = make()\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn generalized_target_candidate_ast_roots_noneligible_targets_and_alias_persistence() {
    let python = oracle();
    let (_, scan) = capture(&[("unit.py", b"other.field = items[index()] = factory().field = rhs()\nself.items[index()].field = self.factory().field = rhs()\n(self).field = (cls).inner.field = [value].field\n")]);
    let native = assert_declaration_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (1, 1, "call", "unit", "factory()"),
                (1, 1, "call", "unit", "index()"),
                (1, 1, "call", "unit", "rhs()"),
                (1, 1, "module", "unit", "unit"),
                (2, 2, "call", "unit", "index()"),
                (2, 2, "call", "unit", "rhs()"),
                (2, 2, "call", "unit", "self.factory()"),
                (
                    3,
                    3,
                    "binding",
                    "unit",
                    r#"{"source":"[value].field","target":"cls.inner.field"}"#
                ),
                (
                    3,
                    3,
                    "binding",
                    "unit",
                    r#"{"source":"[value].field","target":"self.field"}"#
                ),
            ]
        ))
    );
    for rhs in [
        "name",
        "factory().field",
        "items[index()].field",
        "[value].field",
        "factory()()",
        "items[index()]()",
        "items[index()]",
        "[value]",
        "(value,)",
        "{**opts}",
        "a+b",
        "a if ready() else b",
        "None",
    ] {
        let text = format!("(name) = ((self).field) = other.field = {rhs}\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
    // The authority is ast.parse, not compilation: unparenthesized starred
    // roots are accepted in star_expressions contexts even when execution or
    // compilation would reject the resulting AST.
    for value in [
        "items",
        "eval()",
        "factory().field",
        "data[eval()]",
        "eval() + other()",
        "factory()().field[index()] + eval()",
    ] {
        for text in [
            format!("*{value}\n"),
            format!("a = *{value}\n"),
            format!("a: T = *{value}\n"),
            format!("a += *{value}\n"),
            format!("def f():\n    return *{value}\n"),
            format!("for target in *{value}: pass\n"),
        ] {
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            let native = assert_warning_candidates_match_ast(&python, &scan);
            assert!(!native["facts"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["kind"] == "binding"));
        }
    }
    // This AST-only starred root exposes a pinned grammar difference. Do not
    // waive the native syntax error or count it as accepted semantic parity.
    let (_, scan) = capture(&[("unit.py", b"*(eval() if ready() else tail())\n")]);
    assert!(extract_captured(&python, &scan.sources).unwrap().complete);
    let rejected = super::python_native::extract_fact_candidates(&scan.sources).unwrap();
    assert_eq!(
        rejected.files[0].status,
        super::python_native::CandidateStatus::SyntaxError
    );
    assert_eq!(rejected.facts.len(), 1);
    assert!(rejected.diagnostics.is_empty());
    for text in [
        "emit(data[*eval()])\n",
        "data[*eval()] = rhs\n",
        "self.data[*eval()].field = rhs\n",
        "del data[*eval()]\n",
        "for data[*eval()] in source(): pass\n",
        "with context() as data[*eval()]: pass\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_warning_candidates_match_ast(&python, &scan);
    }
    for write in [
        "probe = ordinary",
        "a = probe = None",
        "[probe] = rhs",
        "*probe = rhs",
        "(probe): T",
        "(probe): T = rhs",
        "del (probe, other.field)",
        "probe += rhs",
        "for [probe] in xs: pass",
        "with ctx as (probe,): pass",
    ] {
        let text = format!("from arbitrary import eval as probe\n{write}\nprobe()\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_warning_candidates_match_ast(&python, &scan);
        assert_eq!(native["diagnostics"].as_array().unwrap().len(), 1);
        assert!(native["diagnostics"][0]["message"]
            .as_str()
            .unwrap()
            .contains("arbitrary.eval;"));
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn generalized_target_candidate_grouped_annotations_raw_names_and_annotation_only() {
    let python = oracle();
    let (_, scan) = capture(&[("unit.py", b"class Box:\n    (field): T\n    ((valued)): T = make()\n    (self.field): T\n    other.field: T = rhs\n    items[index()]: T\n")]);
    let native = assert_declaration_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (1, 1, "module", "unit", "unit"),
                (1, 6, "class", "unit.Box", "Box"),
                (2, 2, "attribute_annotation", "unit.Box.field", "T"),
                (3, 3, "attribute_annotation", "unit.Box.valued", "T"),
                (
                    3,
                    3,
                    "binding",
                    "unit.Box",
                    r#"{"source":"make()","target":"valued"}"#
                ),
                (3, 3, "call", "unit.Box", "make()"),
                (6, 6, "call", "unit.Box", "index()"),
            ]
        ))
    );
    for size in [511, 512, 513] {
        let name = "n".repeat(size);
        for suffix in ["", " = value", " = None"] {
            let text = format!("class Box:\n    (({name})): T{suffix}\n");
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            let native = assert_declaration_candidates_match_ast(&python, &scan);
            let symbol = format!("unit.Box.{name}");
            let target = &name[..size.min(512)];
            let binding = if suffix == " = None" {
                format!(r#"{{"target":"{target}"}}"#)
            } else {
                format!(r#"{{"source":"value","target":"{target}"}}"#)
            };
            let mut rows = vec![
                (1, 1, "module", "unit", "unit"),
                (1, 2, "class", "unit.Box", "Box"),
                (2, 2, "attribute_annotation", symbol.as_str(), "T"),
            ];
            if !suffix.is_empty() {
                rows.push((
                    2,
                    2,
                    if suffix == " = None" {
                        "binding_invalidation"
                    } else {
                        "binding"
                    },
                    "unit.Box",
                    binding.as_str(),
                ));
            }
            assert_eq!(native["facts"], json!(expected("unit.py", &rows)));
        }
    }
    for text in [
        "(name): T\n",
        "def f():\n    (local): T = rhs\n",
        "class Box:\n    (\n        field\n    ): T = (\n        make()\n    )\n",
        "(factory().field): T\n(items[index()]): T = make()\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn generalized_target_candidate_neutral_loops_with_augassign_and_delete_visitation() {
    let python = oracle();
    let text = concat!(
        "for (self.router, [items[eval()], *rest]) in iterable(exec()): pass\n",
        "with context(exec()) as (self.router, items[eval()]): pass\n",
        "(items[eval()]) += rhs(exec())\n",
        "del items[eval()], [factory().field], (other.field, name)\n",
    );
    let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
    let native = assert_warning_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (1, 1, "call", "unit", "eval()"),
                (1, 1, "call", "unit", "exec()"),
                (1, 1, "call", "unit", "iterable(exec())"),
                (1, 1, "module", "unit", "unit"),
                (2, 2, "call", "unit", "context(exec())"),
                (2, 2, "call", "unit", "eval()"),
                (2, 2, "call", "unit", "exec()"),
                (3, 3, "call", "unit", "eval()"),
                (3, 3, "call", "unit", "exec()"),
                (3, 3, "call", "unit", "rhs(exec())"),
                (4, 4, "call", "unit", "eval()"),
                (4, 4, "call", "unit", "factory()"),
            ]
        ))
    );
    for target in [
        "(name)",
        "a,b",
        "[a, *b]",
        "*a",
        "other.field",
        "factory().router",
        "items[probe()]",
    ] {
        for text in [
            format!("for {target} in probe(): probe()\n"),
            format!("async for {target} in probe(): probe()\n"),
            format!("with probe() as {target}: probe()\n"),
            format!("async with probe() as {target}: probe()\n"),
        ] {
            // Ungrouped commas in with binders denote additional with-items.
            let text = format!("from arbitrary import eval as probe\n{text}probe()\n");
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            assert_warning_candidates_match_ast(&python, &scan);
        }
    }
    for target in ["(name)", "other.field", "factory().field", "items[probe()]"] {
        for op in [
            "+=", "-=", "*=", "/=", "@=", "//=", "%=", "**=", ">>=", "<<=", "&=", "^=", "|=",
        ] {
            let text = format!("from arbitrary import eval as probe\n{target} {op} probe(), probe()\ndel {target}\nprobe()\n");
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            assert_warning_candidates_match_ast(&python, &scan);
        }
    }
    for text in ["del a,b\n", "del (a,b)\n", "del [a,(b,items[index()])]\n"] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn generalized_target_candidate_shallow_metadata_and_structural_callee_guards() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};
    let python = oracle();
    for text in [
        "(self.router, x) = rhs\n",
        "[self.prefix] = rhs\n",
        "del (other.router,x)\n",
        "del [self.prefix]\n",
        "for other.router in xs: pass\n",
        "with ctx as other.prefix: pass\n",
        "for self.router.child in xs: pass\n",
        "with ctx as factory().router: pass\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
    let (_, scan) = capture(&[("unit.py", b"(self.router, x) = rhs\ndel (other.router,x)\n")]);
    let native = assert_declaration_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (
                    1,
                    1,
                    "binding_invalidation",
                    "unit",
                    r#"{"target":"self.router"}"#
                ),
                (1, 1, "binding_invalidation", "unit", r#"{"target":"x"}"#),
                (1, 1, "module", "unit", "unit"),
            ]
        ))
    );
    let mut forms = [
        "factory().router = rhs",
        "factory().prefix: T",
        "other.router.child += rhs",
        "del other.router,x",
        "del (other.prefix)",
        "self.router: T",
        "x = self.router = rhs",
        "(self.router) = rhs",
        "factory().include_router(child)",
        "items[index()].include_router(child)",
        "[value].include_router(child)",
        "(a if ready else b).include_router(child)",
        "factory().FastAPI()",
        "items[index()].APIRouter()",
        "[value].FastAPI()",
        "FastAPI()()",
        "outer(*args, **opts, k=APIRouter())",
        "from arbitrary import APIRouter as R",
        "from fastapi import FastAPI as F",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    forms.push(format!("self.{}.router.child = rhs", "a".repeat(600)));
    // The rendered receiver can itself contain the helper's interior marker.
    forms.push("factory(self.router.child).field = rhs".into());
    for form in forms {
        let text = format!("eval()\nclass Before: pass\n{form}\nclass After: pass\nexec()\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert!(
            extract_captured(&python, &scan.sources).unwrap().complete,
            "{form}"
        );
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_eq!(
            native.files[0].status,
            CandidateStatus::UnsupportedSyntax,
            "{form}"
        );
        assert_eq!(
            serde_json::to_value(&native.facts).unwrap(),
            json!(expected("unit.py", &[(1, 1, "module", "unit", "unit"),]))
        );
        assert!(native.diagnostics.is_empty(), "{form}");
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn generalized_target_candidate_arbitrary_callees_receivers_and_fresh_shape_budgets() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};
    let python = oracle();
    let (_, scan) = capture(&[("unit.py", b"factory()()\nfactory().method()\nitems[index()]()\n[value].field\n(a if ready() else b)()\n")]);
    let native = assert_declaration_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (1, 1, "call", "unit", "factory()"),
                (1, 1, "call", "unit", "factory()()"),
                (1, 1, "module", "unit", "unit"),
                (2, 2, "call", "unit", "factory()"),
                (2, 2, "call", "unit", "factory().method()"),
                (3, 3, "call", "unit", "index()"),
                (3, 3, "call", "unit", "items[index()]()"),
                (5, 5, "call", "unit", "<IfExp>()"),
                (5, 5, "call", "unit", "ready()"),
            ]
        ))
    );
    for size in [511, 512, 513] {
        for terminal in ["eval", "exec", "FastAPI", "APIRouter"] {
            let prefix = "a".repeat(size - "factory().".len() - 1 - terminal.len());
            let callee = format!("factory().{prefix}.{terminal}Extra");
            let text = format!("outer({callee}())\n");
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            if matches!(terminal, "FastAPI" | "APIRouter") && size == 512 {
                assert!(extract_captured(&python, &scan.sources).unwrap().complete);
                let native = extract_fact_candidates(&scan.sources).unwrap();
                assert_eq!(native.files[0].status, CandidateStatus::UnsupportedSyntax);
                assert_eq!(
                    serde_json::to_value(&native.facts).unwrap(),
                    json!(expected("unit.py", &[(1, 1, "module", "unit", "unit"),]))
                );
                assert!(native.diagnostics.is_empty());
            } else {
                let native = assert_warning_candidates_match_ast(&python, &scan);
                assert_eq!(
                    native["diagnostics"].as_array().unwrap().len(),
                    usize::from(size == 512)
                );
            }
        }
    }
    let heavy = format!(
        "[{}, eval()]",
        ["[a,a,a,a,a,a,a,a,a,a,a,a,a,a,a,a]"; 16].join(",")
    );
    for value in [
        format!("{heavy}.field()"),
        "{**exec()}.field()".to_owned(),
        "(a and eval())()".into(),
        "(a if eval() else b).field()".into(),
        format!("{}factory().method(){}", "[".repeat(14), "]".repeat(14)),
        format!("factory({heavy}).field()"),
    ] {
        let (_, scan) = capture(&[("unit.py", value.as_bytes())]);
        assert_warning_candidates_match_ast(&python, &scan);
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn generalized_target_candidate_call_unpacks_ast_group_order_and_independent_caps() {
    let python = oracle();
    let (_, scan) = capture(&[(
        "unit.py",
        b"f(k=x,*ys,**opts,z=v,k=again)\nf(*a,*b,**c,**d,z=e)\nf(k=eval(),*exec(),**opts)\n",
    )]);
    let native = assert_warning_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (1, 1, "call", "unit", "f(*ys, k=x, **=opts, z=v, k=again)"),
                (1, 1, "module", "unit", "unit"),
                (2, 2, "call", "unit", "f(*a, *b, **=c, **=d, z=e)"),
                (3, 3, "call", "unit", "eval()"),
                (3, 3, "call", "unit", "exec()"),
                (3, 3, "call", "unit", "f(*exec(), k=eval(), **=opts)"),
            ]
        ))
    );
    for count in [16, 17] {
        let args = vec!["*a"; count].join(",");
        let keywords = vec!["**b"; count].join(",");
        let shape = format!(
            "f({}{}{}{})",
            vec!["*a"; count.min(16)].join(", "),
            if count == 17 { ", ..., " } else { ", " },
            vec!["**=b"; count.min(16)].join(", "),
            if count == 17 { ", ..." } else { "" }
        );
        let text = format!("f({args},{keywords})\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        assert_eq!(
            native["facts"],
            json!(expected(
                "unit.py",
                &[
                    (1, 1, "call", "unit", shape.as_str()),
                    (1, 1, "module", "unit", "unit"),
                ]
            ))
        );
    }
    // Source keyword order must not consume either positional slots or budget
    // before AST-grouped Starred arguments, including the seventeenth one.
    let keywords = (0..16)
        .map(|n| format!("k{n}=v"))
        .collect::<Vec<_>>()
        .join(",");
    let stars = vec!["*a"; 17].join(",");
    for text in [
        format!("f({keywords},{stars})\n"),
        format!(
            "f({},*eval(),{},**exec())\n",
            vec!["*a"; 17].join(","),
            vec!["**b"; 17].join(",")
        ),
        "f(x=one(),x=two(),**three(),x=four())\n".into(),
        "f(*(a if ready() else b),**(c if check() else d))\n".into(),
        "sink(*a if ready() else tail(), **opts)\n".into(),
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_warning_candidates_match_ast(&python, &scan);
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn generalized_target_candidate_display_unpacks_defaults_and_hidden_header_calls() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};
    let python = oracle();
    let (_, scan) = capture(&[("unit.py", b"sink((*a,),[*b],{*c},{key():value(),**opts()})\ndef f(x=[*xs].field,y=items[index].field,z={**opts}): pass\n")]);
    assert_declaration_candidates_match_ast(&python, &scan);
    for count in [16, 17] {
        for (open, close) in [("[", "]"), ("(", ",)"), ("{", "}")] {
            let text = format!("sink({open}{}{close})\n", vec!["*a"; count].join(","));
            let shape = format!("sink([{}])", vec!["*a"; 16].join(", "));
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            let native = assert_declaration_candidates_match_ast(&python, &scan);
            assert_eq!(
                native["facts"],
                json!(expected(
                    "unit.py",
                    &[
                        (1, 1, "call", "unit", shape.as_str()),
                        (1, 1, "module", "unit", "unit"),
                    ]
                ))
            );
        }
    }
    let padding = vec!["a"; 17].join(",");
    for value in [
        format!("[{padding},*eval()]"),
        format!("({padding},*eval())"),
        format!("{{{padding},*eval()}}"),
        "{key():value(),**eval(),**exec()}".into(),
        "[*(a if eval() else b)]".into(),
        "{**(a if eval() else b)}".into(),
    ] {
        let text = format!("sink({value})\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_warning_candidates_match_ast(&python, &scan);
    }
    let heavy = format!(
        "[{},*eval()]",
        ["[a,a,a,a,a,a,a,a,a,a,a,a,a,a,a,a]"; 16].join(",")
    );
    for default in [
        "[*eval()]".to_owned(),
        "{**eval()}".into(),
        "[eval()].field".into(),
        "items[eval()].field".into(),
        format!("[{padding},*eval()]"),
        format!("{}eval(){}", "[".repeat(14), "]".repeat(14)),
        heavy,
    ] {
        let text = format!("eval()\nclass Before: pass\ndef f(x={default}): pass\nexec()\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert!(extract_captured(&python, &scan.sources).unwrap().complete);
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_eq!(native.files[0].status, CandidateStatus::UnsupportedSyntax);
        assert_eq!(
            serde_json::to_value(&native.facts).unwrap(),
            json!(expected("unit.py", &[(1, 1, "module", "unit", "unit"),]))
        );
        assert!(native.diagnostics.is_empty());
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn generalized_target_candidate_grammar_negatives_and_transactional_rollback() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};
    let python = oracle();
    for form in [
        "a =\nb = rhs()",
        "a = b =\nc = rhs()",
        "a = # comment\nb = rhs()",
        "x = (*items)",
        "f((*items))",
        "assert *items",
        "raise *items",
        "if *items: pass",
        "while *items: pass",
        "with *items: pass",
        "f(k=*items)",
        "def f(x=*items): pass",
        "x = data[**items]",
        "f(k=x,y)",
        "f(**x,*y)",
        "f(*x,y=1,z)",
        "x = [*a if ready else b]",
        "x = {*a or b}",
        "x = {**a if ready else b}",
        "a = b: T = value",
        "a: T = b = value",
        "a = b += value",
        "a += b = value",
        "a += b += value",
        "(a,b): T = value",
        "[a]: T",
        "*a: T",
        "(a,b) += value",
        "[a] += value",
        "*a += value",
        "del [a,*b]",
        "del (*a,)",
        "del call()",
        "for call() in xs: pass",
        "with ctx as call(): pass",
        "{a} = value",
    ] {
        for newline in ["\n", "\r\n"] {
            let text = format!("eval()\nclass Before: pass\n{form}\nclass After: pass\nexec()\n")
                .replace('\n', newline);
            let (_, scan) = capture(&[("bad.py", text.as_bytes()), ("good.py", b"okay()\n")]);
            let ast = extract_captured(&python, &scan.sources).unwrap();
            assert!(!ast.complete, "AST unexpectedly accepted {form}");
            assert_eq!(ast.diagnostics.len(), 1, "{form}");
            assert_eq!(ast.diagnostics[0].code, "parse_failure", "{form}");
            let native = extract_fact_candidates(&scan.sources).unwrap();
            assert_ne!(
                native.files[0].status,
                CandidateStatus::AcceptedFacts,
                "{form}"
            );
            assert_eq!(native.files[1].status, CandidateStatus::AcceptedFacts);
            let mut facts = expected("bad.py", &[(1, 1, "module", "bad", "bad")]);
            facts.extend(expected(
                "good.py",
                &[
                    (1, 1, "call", "good", "okay()"),
                    (1, 1, "module", "good", "good"),
                ],
            ));
            assert_eq!(
                serde_json::to_value(&native.facts).unwrap(),
                json!(facts),
                "{form}"
            );
            assert!(native.diagnostics.is_empty(), "{form}");
        }
    }
    // A later parse error wins even after an earlier G3 exclusion.
    for form in [
        "factory().router = rhs",
        "factory().include_router(child)",
        "a = (b,*rest) = make()",
        "f(k=x,k=y,*args)",
    ] {
        let text = format!("eval()\n{form}\nlater = (\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert!(!extract_captured(&python, &scan.sources).unwrap().complete);
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_eq!(native.files[0].status, CandidateStatus::SyntaxError);
        assert_eq!(
            serde_json::to_value(&native.facts).unwrap(),
            json!(expected("unit.py", &[(1, 1, "module", "unit", "unit"),]))
        );
        assert!(native.diagnostics.is_empty());
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn generalized_target_candidate_cross_family_replay_isolation_and_resources() {
    use super::python_native::{extract_fact_candidates, parse_batch, CandidateStatus};
    let python = oracle();
    let entries: &[(&str, &[u8])] = &[
        ("a.py", b"from arbitrary import eval as probe\nclass Box(Base):\n    (field): T\n    def run(self,x=[*DEFAULT]):\n        try:\n            scalar = (self.field,*rest,items[probe()]) = factory()(*args,**opts)\n            for [probe,other.router] in iterable():\n                with context() as items[probe()]:\n                    assert [*probe()], {**probe()}\n        except type_call(probe()) as probe:\n            items[probe()] += rhs()\n            raise failure(*args) from cause(**opts)\n        finally:\n            del (other.router,items[probe()])\n        return factory().method(*args,**opts)\nprobe()\n"),
        ("b.py", b"probe()\na = (b,*rest) = make(*args,**opts)\n"),
        ("c.py", b"from arbitrary import eval as leak\nleak()\na = (b,*rest) = rhs()\nfactory().router = rhs\n"),
        ("d.py", b"eval()\nfactory().include_router(child)\nf(\n"),
    ];
    let (root, mut scan) = capture(entries);
    let (_good_root, good_scan) = capture(&entries[..2]);
    let good = assert_warning_candidates_match_ast(&python, &good_scan);
    let native = extract_fact_candidates(&scan.sources).unwrap();
    assert_eq!(native.files[2].status, CandidateStatus::UnsupportedSyntax);
    assert_eq!(native.files[3].status, CandidateStatus::SyntaxError);
    let first = serde_json::to_value(native).unwrap();
    let mut facts = good["facts"].as_array().unwrap().clone();
    facts.extend(expected("c.py", &[(1, 1, "module", "c", "c")]));
    facts.extend(expected("d.py", &[(1, 1, "module", "d", "d")]));
    assert_eq!(first["facts"], json!(facts));
    assert_eq!(first["diagnostics"], good["diagnostics"]);
    let ast = extract_captured(&python, &scan.sources).unwrap();
    for (path, _) in entries {
        fs::remove_file(root.path().join(path)).unwrap();
    }
    assert_eq!(
        first,
        serde_json::to_value(extract_fact_candidates(&scan.sources).unwrap()).unwrap()
    );
    assert_eq!(
        semantics(&ast),
        semantics(&extract_captured(&python, &scan.sources).unwrap())
    );
    scan.sources.reverse();
    let reversed = serde_json::to_value(extract_fact_candidates(&scan.sources).unwrap()).unwrap();
    assert_eq!(first["facts"], reversed["facts"]);
    assert_eq!(first["diagnostics"], reversed["diagnostics"]);
    assert_eq!(
        semantics(&ast),
        semantics(&extract_captured(&python, &scan.sources).unwrap())
    );
    scan.sources
        .retain(|source| matches!(source.path.as_str(), "a.py" | "b.py"));
    let reversed_good = assert_warning_candidates_match_ast(&python, &scan);
    assert_eq!(good["facts"], reversed_good["facts"]);
    assert_eq!(good["diagnostics"], reversed_good["diagnostics"]);
    let heavy = format!(
        "[{},*eval()]",
        ["[a,a,a,a,a,a,a,a,a,a,a,a,a,a,a,a]"; 16].join(",")
    );
    let text = format!(
        "{}items[{heavy}] += factory()(*args,**opts)\nsink({{**exec()}})\n",
        "a = (b,*rest,items[index()]) = make(*args,**opts)\n".repeat(80)
    );
    let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
    let native = assert_warning_candidates_match_ast(&python, &scan);
    let parsed = parse_batch(&scan.sources).unwrap();
    assert_eq!(
        native["files"][0]["nodes_visited"],
        json!(parsed.files[0].nodes_visited)
    );
    assert_eq!(
        native["files"][0]["max_depth"],
        json!(parsed.files[0].max_depth)
    );
    assert_eq!(native["files"][0]["traversal_finished"], true);
    assert_eq!(native["diagnostics"].as_array().unwrap().len(), 2);
    let text = format!(
        "eval()\na = (b,*rest) = {}factory()(){}\n",
        "nest(".repeat(180),
        ")".repeat(180)
    );
    let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
    assert!(extract_captured(&python, &scan.sources).unwrap().complete);
    let native = extract_fact_candidates(&scan.sources).unwrap();
    assert_eq!(native.files[0].status, CandidateStatus::ResourceLimit);
    assert_eq!(
        serde_json::to_value(&native.facts).unwrap(),
        json!(expected("unit.py", &[(1, 1, "module", "unit", "unit"),]))
    );
    assert!(native.diagnostics.is_empty());
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn exception_statement_candidate_try_families_and_expression_descendants() {
    let python = oracle();
    let text = concat!(
        "try:\n",
        "    assert check(inner()), message(eval())\n",
        "except error_type(exec()) as problem:\n",
        "    raise failure(eval()) from cause(exec())\n",
        "else:\n",
        "    assert okay()\n",
        "finally:\n",
        "    finish(eval())\n",
    );
    let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
    let native = assert_warning_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (1, 1, "module", "unit", "unit"),
                (2, 2, "call", "unit", "check(inner())"),
                (2, 2, "call", "unit", "eval()"),
                (2, 2, "call", "unit", "inner()"),
                (2, 2, "call", "unit", "message(eval())"),
                (3, 3, "call", "unit", "error_type(exec())"),
                (3, 3, "call", "unit", "exec()"),
                (4, 4, "call", "unit", "cause(exec())"),
                (4, 4, "call", "unit", "eval()"),
                (4, 4, "call", "unit", "exec()"),
                (4, 4, "call", "unit", "failure(eval())"),
                (6, 6, "call", "unit", "okay()"),
                (8, 8, "call", "unit", "eval()"),
                (8, 8, "call", "unit", "finish(eval())"),
            ]
        ))
    );
    let diagnostics = [
        (2, "eval"),
        (3, "exec"),
        (4, "eval"),
        (4, "exec"),
        (8, "eval"),
    ]
    .into_iter()
    .map(|(line, name)| {
        json!({
            "code":"dynamic_discovery_unresolved", "path":"unit.py",
            "message":format!("line {line}: {name}; runtime targets are not inferred")
        })
    })
    .collect::<Vec<_>>();
    assert_eq!(native["diagnostics"], json!(diagnostics));
    // Handler call syntax need not denote an actual exception type at runtime.
    // Direct types and the CST as_pattern route must both retain descendants.
    for text in [
        "try: body()\nexcept: fallback()\nafter()\n",
        "try: body()\nfinally: finish()\nafter()\n",
        "try: body()\nexcept E: first()\nexcept (F, G) as error: second()\nelse: success()\n",
        "try: body()\nexcept type_call(inner()): first()\nexcept other_type() as error: second()\n",
        "try: body()\nexcept* type_call(inner()): first()\nexcept* (F, G) as error: second()\nelse: success()\nfinally: finish()\nafter()\n",
        "try:\n    try: inner()\n    except* group_type(): recover()\nexcept outer_type() as error:\n    try: raise failure()\n    finally: finish()\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
    for value in [
        "[left(), {right(): tail()}]",
        "(left(), right())",
        "{left(), right()}",
        "left() + right()",
        "left() and right()",
        "left() < right()",
        "not ready()",
        "-number()",
        "~number()",
        "yes() if ready() else no()",
        "data[index()]",
        "data[lower():upper():step()]",
        "None",
        "True",
        "42",
        "'PRIVATE_EXCEPTION_LITERAL'",
    ] {
        let text = format!("try:\n    assert {value}, {value}\nexcept {value} as error:\n    raise {value} from {value}\nfinally:\n    value += {value}\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        assert!(!native.to_string().contains("PRIVATE_EXCEPTION_LITERAL"));
    }
    for marker in ["except", "except*"] {
        for value in [
            "load() if first() else load() if second() else tail()",
            "(load()) if ready() else (tail())",
            "load() if ready() else (\n    tail()\n)",
        ] {
            let text = format!("from arbitrary import eval as load\ntry: before()\n{marker} {value} as load:\n    load()\nafter()\nload()\n");
            for newline in ["\n", "\r\n"] {
                let source = text.replace('\n', newline);
                let (_, scan) = capture(&[("unit.py", source.as_bytes())]);
                assert_warning_candidates_match_ast(&python, &scan);
            }
        }
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn exception_statement_candidate_neutral_names_bare_raise_and_assert_lists() {
    let python = oracle();
    for text in [
        "global first, first, second\nnonlocal unresolved, unresolved\nraise\nassert True\nassert (False, None)\n",
        "def f():\n    global value\n    nonlocal value\n    raise\n",
        "class Box:\n    global value\n    nonlocal missing\n    assert True\n    raise\n",
        "try: raise\nexcept: global value\nexcept E: nonlocal missing\nfinally: raise\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        if text.starts_with("global") || text.starts_with("try") {
            assert_eq!(native["facts"], json!(expected("unit.py", &[
                (1, 1, "module", "unit", "unit"),
            ])));
        }
    }
    // The oracle uses ast.parse, not compile: unresolved nonlocal, conflicting
    // declarations and a bare handler before another handler can still parse.
    for text in [
        "def f():\n    value = 1\n    global value\n    nonlocal value\n",
        "assert test()\nassert test(), message()\n",
        "assert (test(), message())\nassert (test(),), (message(),)\n",
        "assert (\n    test()\n), (\n    message()\n)\n",
        "raise (E, F)\nraise (failure()) from (cause())\n",
        "if ready(): assert test(), message()\nif ready(): raise E from cause()\n",
        "if ready(): global x, y\nif ready(): nonlocal missing\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
    let (_, scan) = capture(&[("unit.py", b"assert (test(), message()), (detail(),)\n")]);
    let native = assert_declaration_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (1, 1, "call", "unit", "detail()"),
                (1, 1, "call", "unit", "message()"),
                (1, 1, "call", "unit", "test()"),
                (1, 1, "module", "unit", "unit"),
            ]
        ))
    );
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn exception_statement_candidate_alias_visitation_and_persistent_helper_rules() {
    let python = oracle();
    let text = concat!(
        "from importlib import import_module as probe\n",
        "try:\n",
        "    probe()\n",
        "    from pkgutil import walk_packages as probe\n",
        "except probe() as probe:\n",
        "    probe()\n",
        "    global probe\n",
        "    nonlocal probe\n",
        "    del probe\n",
        "    probe += probe()\n",
        "    from pkgutil import iter_modules as probe\n",
        "except (probe(), Other) as probe:\n",
        "    assert probe(), probe()\n",
        "else:\n",
        "    raise probe() from probe()\n",
        "    from importlib import import_module as probe\n",
        "finally:\n",
        "    probe()\n",
        "    from arbitrary import eval as probe\n",
        "probe()\n",
    );
    let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
    let native = assert_warning_candidates_match_ast(&python, &scan);
    let mut diagnostics = [
        (3, "importlib.import_module"),
        (5, "pkgutil.walk_packages"),
        (6, "pkgutil.walk_packages"),
        (10, "pkgutil.walk_packages"),
        (12, "pkgutil.iter_modules"),
        (13, "pkgutil.iter_modules"),
        (13, "pkgutil.iter_modules"),
        (15, "pkgutil.iter_modules"),
        (15, "pkgutil.iter_modules"),
        (18, "importlib.import_module"),
        (20, "arbitrary.eval"),
    ]
    .into_iter()
    .map(|(line, name)| {
        json!({
            "code":"dynamic_discovery_unresolved", "path":"unit.py",
            "message":format!("line {line}: {name}; runtime targets are not inferred")
        })
    })
    .collect::<Vec<_>>();
    diagnostics.sort_by(|a, b| a["message"].as_str().cmp(&b["message"].as_str()));
    assert_eq!(native["diagnostics"], json!(diagnostics));
    for text in [
        "from arbitrary import eval as probe\ntry: pass\nexcept probe() as probe:\n    from ordinary import safe as probe\nexcept probe(): probe()\nelse: probe()\nfinally: probe()\nprobe()\n",
        "from arbitrary import eval as probe\nfrom ordinary import probe as alias\ntry: assert alias()\nexcept alias() as probe: probe()\n",
        "def same():\n    from importlib import import_module as saved\ndef same():\n    try: pass\n    except saved() as saved: saved()\n",
        "from arbitrary import eval as probe\nclass Box:\n    try: probe()\n    except E:\n        from ordinary import safe as probe\n    finally: probe()\nprobe()\n",
        "from arbitrary import eval as probe\ndef work(probe):\n    try: probe = None\n    except probe() as probe: probe()\n    finally: probe()\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_warning_candidates_match_ast(&python, &scan);
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn exception_statement_candidate_owners_jumps_and_normalized_statement_ends() {
    let python = oracle();
    let text = concat!(
        "class Box:\n",
        "    try:\n",
        "        field: T = make()\n",
        "    except (\n",
        "        type_call() # PRIVATE_HANDLER_COMMENT\n",
        "    ) as problem:\n",
        "        def run(self):\n",
        "            for item in items():\n",
        "                try:\n",
        "                    def local():\n",
        "                        try: return result()\n",
        "                        finally: nonlocal missing\n",
        "                    self.field: T = local()\n",
        "                    continue\n",
        "                except Error:\n",
        "                    break\n",
        "                else:\n",
        "                    return fallback()\n",
        "                finally:\n",
        "                    assert restored()\n",
        "            # trailing method comment\n",
        "    finally:\n",
        "        global final_name\n",
        "    # trailing class comment\n",
        "after()\n",
    );
    for newline in ["\n", "\r\n"] {
        for bom in ["", "\u{feff}"] {
            let text = format!("{bom}{}", text.replace('\n', newline));
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            let native = assert_declaration_candidates_match_ast(&python, &scan);
            assert_eq!(
                native["facts"],
                json!(expected(
                    "unit.py",
                    &[
                        (1, 1, "module", "unit", "unit"),
                        (1, 23, "class", "unit.Box", "Box"),
                        (3, 3, "attribute_annotation", "unit.Box.field", "T"),
                        (
                            3,
                            3,
                            "binding",
                            "unit.Box",
                            r#"{"source":"make()","target":"field"}"#
                        ),
                        (3, 3, "call", "unit.Box", "make()"),
                        (5, 5, "call", "unit.Box", "type_call()"),
                        (7, 7, "parameter", "unit.Box.run.self", "self"),
                        (7, 20, "function", "unit.Box.run", "run"),
                        (
                            7,
                            20,
                            "signature",
                            "unit.Box.run",
                            r#"{"keywordOnly":[],"positional":["self"],"positionalOnly":0}"#
                        ),
                        (8, 8, "call", "unit.Box.run", "items()"),
                        (10, 12, "function", "unit.Box.run.local", "local"),
                        (
                            10,
                            12,
                            "signature",
                            "unit.Box.run.local",
                            r#"{"keywordOnly":[],"positional":[],"positionalOnly":0}"#
                        ),
                        (11, 11, "call", "unit.Box.run.local", "result()"),
                        (
                            13,
                            13,
                            "binding",
                            "unit.Box.run",
                            r#"{"source":"local()","target":"self.field"}"#
                        ),
                        (13, 13, "call", "unit.Box.run", "local()"),
                        (18, 18, "call", "unit.Box.run", "fallback()"),
                        (20, 20, "call", "unit.Box.run", "restored()"),
                        (25, 25, "call", "unit", "after()"),
                    ]
                ))
            );
            assert!(!native.to_string().contains("PRIVATE_HANDLER_COMMENT"));
        }
    }
    let text = concat!(
        "def work():\n",
        "    try:\n",
        "        raise (\n",
        "            failure(\n",
        "                inner()\n",
        "            )\n",
        "        ) from (\n",
        "            cause() # PRIVATE_CAUSE_COMMENT\n",
        "        )\n",
        "    except (\n",
        "        first(), second()\n",
        "    ) as problem:\n",
        "        nonlocal missing\n",
        "        # trailing handler comment\n",
    );
    for newline in ["\n", "\r\n"] {
        let text = text.replace('\n', newline);
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        assert_eq!(
            native["facts"],
            json!(expected(
                "unit.py",
                &[
                    (1, 1, "module", "unit", "unit"),
                    (1, 13, "function", "unit.work", "work"),
                    (
                        1,
                        13,
                        "signature",
                        "unit.work",
                        r#"{"keywordOnly":[],"positional":[],"positionalOnly":0}"#
                    ),
                    (4, 6, "call", "unit.work", "failure(inner())"),
                    (5, 5, "call", "unit.work", "inner()"),
                    (8, 8, "call", "unit.work", "cause()"),
                    (11, 11, "call", "unit.work", "first()"),
                    (11, 11, "call", "unit.work", "second()"),
                ]
            ))
        );
    }
    for text in [
        "while ready():\n    try: continue\n    except E: break\n    else: continue\n    finally: break\n",
        "for item in items():\n    try: pass\n    except* E: continue\n    else: break\n    finally: continue\n",
        "def f():\n    try: return result()\n    except* E: return fallback()\n    finally: raise\n",
        "while outer():\n    while inner(): pass\n    else:\n        try: pass\n        except E: break\n",
    ] {
        // Jumps in except* may parse without being valid to compile.
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn exception_statement_candidate_delete_and_all_augassign_operators_are_nonwrites() {
    let python = oracle();
    for op in [
        "+=", "-=", "*=", "/=", "@=", "//=", "%=", "**=", ">>=", "<<=", "&=", "^=", "|=",
    ] {
        for target in [
            "value",
            "self.value",
            "cls.inner.value",
            "self.prefix.child",
        ] {
            let text = format!("del {target}\n{target} {op} outer(eval())\n");
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            let native = assert_warning_candidates_match_ast(&python, &scan);
            assert_eq!(
                native["facts"],
                json!(expected(
                    "unit.py",
                    &[
                        (1, 1, "module", "unit", "unit"),
                        (2, 2, "call", "unit", "eval()"),
                        (2, 2, "call", "unit", "outer(eval())"),
                    ]
                ))
            );
            assert_eq!(
                native["diagnostics"],
                json!([{
                    "code":"dynamic_discovery_unresolved", "path":"unit.py",
                    "message":"line 2: eval; runtime targets are not inferred"
                }])
            );
        }
    }
    let (_, scan) = capture(&[(
        "unit.py",
        b"from arbitrary import eval as probe\ndel probe\nprobe += probe()\nprobe()\n",
    )]);
    let native = assert_warning_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (
                    1,
                    1,
                    "from_import",
                    "unit",
                    r#"{"level":0,"module":"arbitrary","names":[{"asname":"probe","name":"eval"}]}"#
                ),
                (1, 1, "module", "unit", "unit"),
                (3, 3, "call", "unit", "probe()"),
                (4, 4, "call", "unit", "probe()"),
            ]
        ))
    );
    assert_eq!(
        native["diagnostics"],
        json!([
            {"code":"dynamic_discovery_unresolved", "path":"unit.py", "message":"line 3: arbitrary.eval; runtime targets are not inferred"},
            {"code":"dynamic_discovery_unresolved", "path":"unit.py", "message":"line 4: arbitrary.eval; runtime targets are not inferred"},
        ])
    );
    for text in [
        "del value\nvalue += (left(), right())\n",
        "class Box:\n    del value\n    value += [left(), right()]\n",
        "def f(self, cls):\n    del self.inner.value\n    cls.inner.value += (\n        outer(inner())\n    )\n",
        "if ready(): del self.value\nif ready(): self.value += change()\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn exception_statement_candidate_malformed_and_deferred_forms_rollback() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};
    let python = oracle();
    // Only definite syntax errors claim AST failure. Target/context policy and
    // physical-layout exclusions below must not be confused with compile rules.
    let syntax_errors = [
        "try: pass",
        "try: pass\nelse: pass",
        "try: pass\nelse: pass\nfinally: pass",
        "try: pass\nexcept*: pass",
        "try: pass\nexcept E, F: pass",
        "try: pass\nexcept* E, F: pass",
        "try: pass\nexcept E as x, F: pass",
        "try: pass\nexcept E: pass\nexcept* F: pass",
        "try: pass\nexcept* E: pass\nexcept F: pass",
        "try: pass\nexcept E: pass\nelse: pass\nexcept F: pass",
        "try: pass\nexcept E: pass\nelse: pass\nelse: pass",
        "try: pass\nfinally: pass\nfinally: pass",
        "try: pass\nfinally: pass\nexcept E: pass",
        "try: pass\nexcept E as (x): pass",
        "try: pass\nexcept E as (x,y): pass",
        "try: pass\nexcept E as [x]: pass",
        "try: pass\nexcept E as obj.x: pass",
        "try: pass\nexcept E as x[0]: pass",
        "try: pass\nexcept E as *x: pass",
        "try: pass\nexcept E as True: pass",
        "try: pass\nexcept E as first as second: pass",
        "try: pass\nexcept A if B else C as first as second: pass",
        "try: pass\nexcept* A if B else C as obj.field: pass",
        "try: pass\nexcept A if B else (C as name): pass",
        "try: pass\nexcept (A as name) if B else C: pass",
        "try: pass\nexcept* E as first as second: pass",
        "assert",
        "assert a,b,c",
        "assert a,",
        "assert\na",
        "assert a\n,b",
        "assert a,\nb",
        "raise from x",
        "raise E,F",
        "raise E,F,T",
        "raise E from",
        "raise E\nfrom F",
        "raise E from\nF",
        "raise E, F from G",
        "global",
        "nonlocal",
        "global (x)",
        "nonlocal (x,y)",
        "global x,",
        "nonlocal x,",
        "global\nx",
        "nonlocal\nx",
        "global x,\ny",
        "global x\n,y",
        "nonlocal x.y",
        "raise\nfrom x",
        "try\n: pass\nfinally: pass",
        "try: pass\nexcept\nE: pass",
        "try: pass\nexcept*\nE: pass",
        "try: pass\nexcept E\nas x: pass",
        "try: pass\nexcept E as\nx: pass",
        "try: pass\nexcept E\n: pass",
        "try: pass\nfinally\n: pass",
        "try: pass\nexcept E: pass\nelse\n: pass",
        "del *x",
        "del True",
        "value += y = eval()",
        "value += y += eval()",
        "value +=\neval()",
        "del\nx",
        "self.x\n+= eval()",
    ];
    let deferred = [
        "try: pass\nexcept E as \u{212a}: pass",
        "try: pass\nexcept\tE: FastAPI()",
        "assert\tFastAPI()",
        "global\tvalue; FastAPI()",
        "try: pass\nexcept E: # type: ignore\n    pass",
        "del (self.router)",
        "del other.router,y",
        "del [x for x in xs]",
        "del items[app.include_router(child)]",
        "del obj.prefix",
        "del get().router",
        "del \u{212a}",
        "(self.router) += eval()",
        "x,y += eval()",
        "[x] += eval()",
        "items[0] += await FastAPI()",
        "obj.router += eval()",
        "get().prefix += eval()",
        "\u{212a} += eval()",
        "x += a,(yield APIRouter())",
        "try: break\nfinally: pass",
        "try: pass\nexcept E: continue",
        "while ready():\n    try:\n        def f(): break\n    finally: pass",
        "while ready():\n    try:\n        class C:\n            continue\n    finally: pass",
        "while ready(): pass\nelse:\n    try: break\n    finally: pass",
        "try: return value()\nfinally: pass",
        "class C:\n    try: return\n    finally: pass",
    ];
    for (forms, ast_failure) in [
        (syntax_errors.as_slice(), true),
        (deferred.as_slice(), false),
    ] {
        for form in forms {
            for newline in ["\n", "\r\n"] {
                let text = format!(
                    "\u{feff}eval()\nclass Before: pass\n{form}\nclass After: pass\neval()\n"
                )
                .replace('\n', newline);
                let (_, scan) =
                    capture(&[("bad.py", text.as_bytes()), ("good.py", b"assert eval()\n")]);
                let ast = extract_captured(&python, &scan.sources).unwrap();
                if ast_failure {
                    assert!(!ast.complete, "{form:?}");
                    assert_eq!(ast.diagnostics[0].code, "parse_failure", "{form:?}");
                }
                let native = extract_fact_candidates(&scan.sources).unwrap();
                assert_ne!(
                    native.files[0].status,
                    CandidateStatus::AcceptedFacts,
                    "{form:?}"
                );
                assert_eq!(native.files[1].status, CandidateStatus::AcceptedFacts);
                let mut facts = expected("bad.py", &[(1, 1, "module", "bad", "bad")]);
                facts.extend(expected(
                    "good.py",
                    &[
                        (1, 1, "call", "good", "eval()"),
                        (1, 1, "module", "good", "good"),
                    ],
                ));
                let native = serde_json::to_value(native).unwrap();
                assert_eq!(native["facts"], json!(facts), "{form:?}");
                assert_eq!(
                    native["diagnostics"],
                    json!([{
                        "code":"dynamic_discovery_unresolved", "path":"good.py",
                        "message":"line 1: eval; runtime targets are not inferred"
                    }]),
                    "{form:?}"
                );
            }
        }
    }
    // An early valid-but-deferred target cannot hide a later parser error.
    for form in [
        "del obj.router",
        "obj.prefix += eval()",
        "assert (x := values)",
    ] {
        let text = format!("eval()\n{form}\nclass After: pass\neval()\nlater = (\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert!(!extract_captured(&python, &scan.sources).unwrap().complete);
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_eq!(native.files[0].status, CandidateStatus::SyntaxError);
        assert_eq!(
            serde_json::to_value(&native.facts).unwrap(),
            json!(expected("unit.py", &[(1, 1, "module", "unit", "unit")]))
        );
        assert!(native.diagnostics.is_empty());
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn exception_statement_candidate_side_output_guards_and_hidden_call_budgets() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};
    let python = oracle();
    let mut forms = Vec::new();
    let mut values = [
        "FastAPI()",
        "pkg.APIRouter()",
        "app.include_router(child)",
        "outer(FastAPI())",
        "(lambda: FastAPI())",
        "[APIRouter() for x in xs]",
        "outer(*[app.include_router(x) for x in items])",
        "outer(**{x:FastAPI() for x in items})",
        "await FastAPI()",
        "(yield APIRouter())",
        "(x := app.include_router(child))",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    values.extend([
        format!("[{}, FastAPI()]", ["a"; 17].join(",")),
        format!("{}pkg.APIRouter(){}", "[".repeat(13), "]".repeat(13)),
        format!(
            "{{a:[{}, app.include_router(child)]}}",
            ["[a,a,a,a,a,a,a,a,a,a,a,a,a,a,a,a]"; 16].join(",")
        ),
    ]);
    for value in values {
        forms.extend([
            format!("assert {value}"),
            format!("assert True, {value}"),
            format!("raise {value}"),
            format!("raise E from {value}"),
            format!("try: pass\nexcept {value}: pass"),
            format!("try: pass\nexcept* {value} as error: pass"),
            format!("value += {value}"),
        ]);
    }
    for target in [
        "self.router",
        "cls.prefix",
        "self.router.child",
        "self.inner.router.child",
    ] {
        forms.extend([
            format!("del {target}"),
            format!("{target} += eval()"),
            format!("try: pass\nexcept E: {target}: T"),
        ]);
    }
    forms.extend([
        "try: pass\nexcept E:\n    from arbitrary import APIRouter as R".into(),
        "try: pass\nfinally:\n    from fastapi import FastAPI".into(),
        "try: pass\nexcept E:\n    def f(x=eval()): pass".into(),
        "try: pass\nexcept E:\n    def f(x: T[eval()]): pass".into(),
        "try: pass\nfinally:\n    def f() -> T[eval()]: pass".into(),
        "try: pass\nexcept E:\n    class Box(eval()): pass".into(),
        "try: pass\nexcept E:\n    @decorate(eval())\n    def f(): pass".into(),
        "try: pass\nfinally:\n    with (cm() as x, other() as y): pass".into(),
    ]);
    for form in forms {
        let text = format!("eval()\nclass Before: pass\n{form}\nclass After: pass\neval()\n");
        let (_, scan) = capture(&[
            ("bad.py", text.as_bytes()),
            ("good.py", b"raise eval() from exec()\n"),
        ]);
        let _ast = extract_captured(&python, &scan.sources).unwrap();
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_ne!(
            native.files[0].status,
            CandidateStatus::AcceptedFacts,
            "{form}"
        );
        assert_eq!(native.files[1].status, CandidateStatus::AcceptedFacts);
        let mut facts = expected("bad.py", &[(1, 1, "module", "bad", "bad")]);
        facts.extend(expected(
            "good.py",
            &[
                (1, 1, "call", "good", "eval()"),
                (1, 1, "call", "good", "exec()"),
                (1, 1, "module", "good", "good"),
            ],
        ));
        let native = serde_json::to_value(native).unwrap();
        assert_eq!(native["facts"], json!(facts), "{form}");
        assert_eq!(
            native["diagnostics"],
            json!([
                {"code":"dynamic_discovery_unresolved", "path":"good.py", "message":"line 1: eval; runtime targets are not inferred"},
                {"code":"dynamic_discovery_unresolved", "path":"good.py", "message":"line 1: exec; runtime targets are not inferred"},
            ]),
            "{form}"
        );
    }
    let heavy = format!(
        "[{},eval()]",
        ["[a,a,a,a,a,a,a,a,a,a,a,a,a,a,a,a]"; 16].join(",")
    );
    for value in [
        format!("a and {heavy}"),
        format!("{{a:{heavy}}}"),
        format!("data[:{heavy}]"),
        format!("[{}, eval()]", ["a"; 17].join(",")),
        format!("{}eval(){}", "[".repeat(13), "]".repeat(13)),
    ] {
        for text in [
            format!("assert {value}, detail()\n"),
            format!("assert outer({value}), detail()\n"),
            format!("assert True, {value}\n"),
            format!("raise {value} from cause()\n"),
            format!("raise E from {value}\n"),
            format!("try: pass\nexcept {value} as error: pass\n"),
            format!("try: pass\nexcept* {value}: pass\n"),
            format!("value += {value}\n"),
            format!("if {value}:\n    try: pass\n    finally: assert True\n"),
        ] {
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            let native = assert_warning_candidates_match_ast(&python, &scan);
            assert_eq!(native["diagnostics"].as_array().unwrap().len(), 1);
            assert!(native["facts"]
                .as_array()
                .unwrap()
                .iter()
                .any(|fact| fact["value"] == "eval()"));
        }
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn exception_statement_candidate_replay_deleted_sources_order_and_file_isolation() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};
    let python = oracle();
    let entries: &[(&str, &[u8])] = &[
        ("a.py", b"try: assert eval()\nexcept type_call() as eval: raise eval() from cause()\nelse: del value\nfinally: value += finish()\nafter()\n"),
        ("b.py", b"try: pass\nexcept* group_type(): assert check()\nfinally: nonlocal missing\n"),
        ("c.py", b"eval()\nclass Before: pass\ndel obj.router\nclass After: pass\neval()\n"),
        ("d.py", b"eval()\ntry: pass\nexcept*: pass\n"),
    ];
    let (root, mut scan) = capture(entries);
    let native = extract_fact_candidates(&scan.sources).unwrap();
    assert_eq!(native.files[0].status, CandidateStatus::AcceptedFacts);
    assert_eq!(native.files[1].status, CandidateStatus::AcceptedFacts);
    assert_eq!(native.files[2].status, CandidateStatus::UnsupportedSyntax);
    assert_ne!(native.files[3].status, CandidateStatus::AcceptedFacts);
    let first = serde_json::to_value(native).unwrap();
    let ast = extract_captured(&python, &scan.sources).unwrap();
    assert!(!ast.complete);
    let (good_root, mut good_scan) = capture(&entries[..2]);
    let good = assert_warning_candidates_match_ast(&python, &good_scan);
    let mut facts = good["facts"].as_array().unwrap().clone();
    facts.extend(expected("c.py", &[(1, 1, "module", "c", "c")]));
    facts.extend(expected("d.py", &[(1, 1, "module", "d", "d")]));
    assert_eq!(first["facts"], json!(facts));
    assert_eq!(first["diagnostics"], good["diagnostics"]);
    // Nothing may reread or execute source during replay, including rejected files.
    for path in ["a.py", "b.py", "c.py", "d.py"] {
        fs::remove_file(root.path().join(path)).unwrap();
    }
    for path in ["a.py", "b.py"] {
        fs::remove_file(good_root.path().join(path)).unwrap();
    }
    assert_eq!(
        first,
        serde_json::to_value(extract_fact_candidates(&scan.sources).unwrap()).unwrap()
    );
    assert_eq!(
        semantics(&ast),
        semantics(&extract_captured(&python, &scan.sources).unwrap())
    );
    assert_eq!(
        good,
        assert_warning_candidates_match_ast(&python, &good_scan)
    );
    scan.sources.reverse();
    good_scan.sources.reverse();
    let reversed = serde_json::to_value(extract_fact_candidates(&scan.sources).unwrap()).unwrap();
    assert_eq!(first["facts"], reversed["facts"]);
    assert_eq!(first["diagnostics"], reversed["diagnostics"]);
    assert_eq!(
        semantics(&ast),
        semantics(&extract_captured(&python, &scan.sources).unwrap())
    );
    let reversed_good = assert_warning_candidates_match_ast(&python, &good_scan);
    assert_eq!(good["facts"], reversed_good["facts"]);
    assert_eq!(good["diagnostics"], reversed_good["diagnostics"]);
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn compound_suite_candidate_conditions_branches_and_while_else_spans() {
    let python = oracle();
    let text = concat!(
        "if first(inner()):\n",
        "    yes()\n",
        "elif second():\n",
        "    while test():\n",
        "        body()\n",
        "    else:\n",
        "        exhausted()\n",
        "else:\n",
        "    fallback()\n",
        "if (\n",
        "    grouped() # PRIVATE_SUITE_COMMENT\n",
        "): inline()\n",
        "# trailing\n",
    );
    for newline in ["\n", "\r\n"] {
        for bom in ["", "\u{feff}"] {
            let text = format!("{bom}{}", text.replace('\n', newline));
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            let native = assert_declaration_candidates_match_ast(&python, &scan);
            assert_eq!(
                native["facts"],
                json!(expected(
                    "unit.py",
                    &[
                        (1, 1, "call", "unit", "first(inner())"),
                        (1, 1, "call", "unit", "inner()"),
                        (1, 1, "module", "unit", "unit"),
                        (2, 2, "call", "unit", "yes()"),
                        (3, 3, "call", "unit", "second()"),
                        (4, 4, "call", "unit", "test()"),
                        (5, 5, "call", "unit", "body()"),
                        (7, 7, "call", "unit", "exhausted()"),
                        (9, 9, "call", "unit", "fallback()"),
                        (11, 11, "call", "unit", "grouped()"),
                        (12, 12, "call", "unit", "inline()"),
                    ]
                ))
            );
            assert!(!native.to_string().contains("PRIVATE_SUITE_COMMENT"));
        }
    }
    // Retain all syntactic branches, not just a runtime-selected path. These
    // replace the former declaration/class rejection fixtures with positives.
    for text in [
        "if True:pass\nelse: unreachable()\n",
        "def f():\n    if True:\n        pass\n",
        "class Box:\n    if True: pass\n",
        "if x: pass\n\n# attachment comment\nelif y: pass\nelse: pass\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
    for condition in [
        "[left(), {right(): tail()}]",
        "left() + right()",
        "left() and right() or tail()",
        "left() < right() is not tail()",
        "not ready()",
        "yes() if ready() else no()",
        "data[index()]",
        "data[lower():upper():step()]",
        "{None, True, False}",
        "'PRIVATE_SUITE_LITERAL'",
    ] {
        let text = format!("if {condition}: pass\nelif {condition}: pass\nelse: pass\nwhile {condition}: pass\nelse: pass\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        assert!(!native.to_string().contains("PRIVATE_SUITE_LITERAL"));
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn compound_suite_candidate_for_async_for_neutral_binders_and_loop_jumps() {
    let python = oracle();
    let text = concat!(
        "for item in items(seed()):\n",
        "    if ready(): continue\n",
        "    else: break\n",
        "else: finished()\n",
        "async def consume():\n",
        "    async for value in (\n",
        "        stream(start())\n",
        "    ):\n",
        "        with context() as resource:\n",
        "            use(value)\n",
        "            continue\n",
        "    else:\n",
        "        exhausted()\n",
        "    # trailing\n",
    );
    let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
    let native = assert_declaration_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (1, 1, "call", "unit", "items(seed())"),
                (1, 1, "call", "unit", "seed()"),
                (1, 1, "module", "unit", "unit"),
                (2, 2, "call", "unit", "ready()"),
                (4, 4, "call", "unit", "finished()"),
                (5, 13, "async_function", "unit.consume", "consume"),
                (
                    5,
                    13,
                    "signature",
                    "unit.consume",
                    r#"{"keywordOnly":[],"positional":[],"positionalOnly":0}"#
                ),
                (7, 7, "call", "unit.consume", "start()"),
                (7, 7, "call", "unit.consume", "stream(start())"),
                (9, 9, "call", "unit.consume", "context()"),
                (10, 10, "call", "unit.consume", "use(value)"),
                (13, 13, "call", "unit.consume", "exhausted()"),
            ]
        ))
    );
    for text in [
        "for item in a(), b(): pass\nelse: done()\n",
        "for item in [a(), b()]: pass\n",
        "while outer():\n    while inner(): break\n    else: continue\nelse: done()\n",
        "for item in items():\n    def local():\n        for value in values(): break\n    continue\n",
        "while outer():\n    class Box:\n        while inner(): continue\n    break\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn compound_suite_candidate_with_async_with_context_items_and_trivia() {
    let python = oracle();
    let text = concat!(
        "with context(inner()) as first, other() as second, plain():\n",
        "    use(first, second) # PRIVATE_CONTEXT_COMMENT\n",
        "async def work():\n",
        "    async with context(\n",
        "        nested() # multiline expression, not a with-clause list\n",
        "    ) as resource, peer() as another:\n",
        "        finish(resource, another)\n",
        "    # trailing\n",
        "with (grouped()) as value: after()\n",
    );
    for newline in ["\n", "\r\n"] {
        let text = format!("\u{feff}{}", text.replace('\n', newline));
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        assert_eq!(
            native["facts"],
            json!(expected(
                "unit.py",
                &[
                    (1, 1, "call", "unit", "context(inner())"),
                    (1, 1, "call", "unit", "inner()"),
                    (1, 1, "call", "unit", "other()"),
                    (1, 1, "call", "unit", "plain()"),
                    (1, 1, "module", "unit", "unit"),
                    (2, 2, "call", "unit", "use(first, second)"),
                    (3, 7, "async_function", "unit.work", "work"),
                    (
                        3,
                        7,
                        "signature",
                        "unit.work",
                        r#"{"keywordOnly":[],"positional":[],"positionalOnly":0}"#
                    ),
                    (4, 6, "call", "unit.work", "context(nested())"),
                    (5, 5, "call", "unit.work", "nested()"),
                    (6, 6, "call", "unit.work", "peer()"),
                    (7, 7, "call", "unit.work", "finish(resource, another)"),
                    (9, 9, "call", "unit", "after()"),
                    (9, 9, "call", "unit", "grouped()"),
                ]
            ))
        );
        assert!(!native.to_string().contains("PRIVATE_CONTEXT_COMMENT"));
    }
    for text in [
        "with context((grouped())): pass\n",
        "with context(\n    [left(), right()], # PRIVATE_CONTEXT_COMMENT\n) as value: pass\n",
        "with (\n    grouped() # context-expression grouping\n) as value: pass\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn compound_suite_candidate_mixed_definitions_annotations_and_owner_restoration() {
    let python = oracle();
    let text = concat!(
        "def outer():\n",
        "    if enabled():\n",
        "        class Box(Base):\n",
        "            while class_ready():\n",
        "                with class_context() as resource:\n",
        "                    field: T = make()\n",
        "                break\n",
        "            else:\n",
        "                other: T\n",
        "            def run(self):\n",
        "                for item in items():\n",
        "                    if ready():\n",
        "                        def local(value=DEFAULT):\n",
        "                            while retry():\n",
        "                                value: T = [step()]\n",
        "                                break\n",
        "                            else:\n",
        "                                return final_value()\n",
        "                        self.field: T = local()\n",
        "                    else:\n",
        "                        continue\n",
        "                else:\n",
        "                    return fallback()\n",
        "            # trailing class comment\n",
        "    else:\n",
        "        return done()\n",
        "    # trailing outer comment\n",
        "after()\n",
    );
    let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
    let native = assert_declaration_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (1, 1, "module", "unit", "unit"),
                (1, 26, "function", "unit.outer", "outer"),
                (
                    1,
                    26,
                    "signature",
                    "unit.outer",
                    r#"{"keywordOnly":[],"positional":[],"positionalOnly":0}"#
                ),
                (2, 2, "call", "unit.outer", "enabled()"),
                (3, 3, "base", "unit.outer.Box", "Base"),
                (3, 23, "class", "unit.outer.Box", "Box"),
                (4, 4, "call", "unit.outer.Box", "class_ready()"),
                (5, 5, "call", "unit.outer.Box", "class_context()"),
                (6, 6, "attribute_annotation", "unit.outer.Box.field", "T"),
                (
                    6,
                    6,
                    "binding",
                    "unit.outer.Box",
                    r#"{"source":"make()","target":"field"}"#
                ),
                (6, 6, "call", "unit.outer.Box", "make()"),
                (9, 9, "attribute_annotation", "unit.outer.Box.other", "T"),
                (10, 10, "parameter", "unit.outer.Box.run.self", "self"),
                (10, 23, "function", "unit.outer.Box.run", "run"),
                (
                    10,
                    23,
                    "signature",
                    "unit.outer.Box.run",
                    r#"{"keywordOnly":[],"positional":["self"],"positionalOnly":0}"#
                ),
                (11, 11, "call", "unit.outer.Box.run", "items()"),
                (12, 12, "call", "unit.outer.Box.run", "ready()"),
                (
                    13,
                    13,
                    "parameter",
                    "unit.outer.Box.run.local.value",
                    "value"
                ),
                (
                    13,
                    13,
                    "parameter_default",
                    "unit.outer.Box.run.local.value",
                    "DEFAULT"
                ),
                (13, 18, "function", "unit.outer.Box.run.local", "local"),
                (
                    13,
                    18,
                    "signature",
                    "unit.outer.Box.run.local",
                    r#"{"keywordOnly":[],"positional":["value"],"positionalOnly":0}"#
                ),
                (14, 14, "call", "unit.outer.Box.run.local", "retry()"),
                (
                    15,
                    15,
                    "binding_invalidation",
                    "unit.outer.Box.run.local",
                    r#"{"target":"value"}"#
                ),
                (15, 15, "call", "unit.outer.Box.run.local", "step()"),
                (18, 18, "call", "unit.outer.Box.run.local", "final_value()"),
                (
                    19,
                    19,
                    "binding",
                    "unit.outer.Box.run",
                    r#"{"source":"local()","target":"self.field"}"#
                ),
                (19, 19, "call", "unit.outer.Box.run", "local()"),
                (23, 23, "call", "unit.outer.Box.run", "fallback()"),
                (26, 26, "call", "unit.outer", "done()"),
                (28, 28, "call", "unit", "after()"),
            ]
        ))
    );
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn compound_suite_candidate_alias_visitor_order_binders_and_duplicate_warnings() {
    let python = oracle();
    let text = concat!(
        "from importlib import import_module as load\n",
        "if load():\n",
        "    from pkgutil import walk_packages as load\n",
        "elif load():\n",
        "    load = None\n",
        "else:\n",
        "    from pkgutil import iter_modules as load\n",
        "while load():\n",
        "    for load in load():\n",
        "        emit(load(), load())\n",
        "        break\n",
        "    else:\n",
        "        load()\n",
        "    break\n",
        "else:\n",
        "    with load() as load, load() as other:\n",
        "        load: T\n",
        "        load = None\n",
        "        load()\n",
        "load()\n",
    );
    let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
    let native = assert_warning_candidates_match_ast(&python, &scan);
    let mut diagnostics = [
        (2, "importlib.import_module"),
        (4, "pkgutil.walk_packages"),
        (8, "pkgutil.iter_modules"),
        (9, "pkgutil.iter_modules"),
        (10, "pkgutil.iter_modules"),
        (10, "pkgutil.iter_modules"),
        (13, "pkgutil.iter_modules"),
        (16, "pkgutil.iter_modules"),
        (16, "pkgutil.iter_modules"),
        (19, "pkgutil.iter_modules"),
        (20, "pkgutil.iter_modules"),
    ]
    .into_iter()
    .map(|(line, name)| {
        json!({
            "code": "dynamic_discovery_unresolved", "path": "unit.py",
            "message": format!("line {line}: {name}; runtime targets are not inferred")
        })
    })
    .collect::<Vec<_>>();
    diagnostics.sort_by(|a, b| a["message"].as_str().cmp(&b["message"].as_str()));
    assert_eq!(native["diagnostics"], json!(diagnostics));
    // These complete comparisons also pin duplicate-warning retention while
    // identical same-line call facts deduplicate, without binder write events.
    for text in [
        "from arbitrary import eval as load\nif False:\n    from ordinary import safe as load\nelse:\n    load()\nload()\n",
        "from arbitrary import eval as load\nif True:\n    class Box:\n        if ready():\n            load()\n        else:\n            from ordinary import safe as load\n    load()\nload()\n",
        "from arbitrary import eval as load\nasync def work(load):\n    async for load in load():\n        async with load() as load:\n            load()\n    else:\n        load()\n",
        "if eval(eval()): pass\nelif eval(): pass\nwhile eval(): break\nelse:\n    with context({eval():eval()}) as value: pass\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_warning_candidates_match_ast(&python, &scan);
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn compound_suite_candidate_malformed_deferred_and_invalid_jump_rollback() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};
    let python = oracle();
    for form in [
        "if\nflag: pass",
        "while\nflag: pass",
        "if flag\n: pass",
        "if (flag)\n: pass",
        "if flag # gap\n: pass",
        "if flag:\npass",
        "if flag: pass\n    else: pass",
        "if flag: pass\nelse: pass\nelif other: pass",
        "if flag: pass\nelse: pass\nelse: pass",
        "elif flag: pass",
        "else: pass",
        "while flag: pass\nelif other: pass",
        "with cm(): pass\nelse: pass",
        "if flag:\n    pass\n  wrong()",
        "if flag: FastAPI(); later()",
        "if\tflag: FastAPI()",
        "if flag:\n\tFastAPI()",
        "if (\n\tflag\n): FastAPI()",
        "if flag \\\n: FastAPI()",
        "if flag: # type: ignore\n    pass",
        "for\nitem in items: pass",
        "for item\nin items: pass",
        "for item in\nitems: pass",
        "async\nfor item in items: pass",
        "for (item) in items: await FastAPI()",
        "for a,b in items: (yield APIRouter())",
        "for [a] in items: app.include_router(child)",
        "for *a in items: FastAPI()",
        "for obj.a in items: APIRouter()",
        "for data[0] in items: (x := FastAPI())",
        "for \u{212a} in items: pass",
        "for True in items: pass",
        "with\ncm(): pass",
        "async\nwith cm(): pass",
        "with cm()\nas x: pass",
        "with cm() as\nx: pass",
        "with cm(),\nother(): pass",
        "with cm() as (x): await FastAPI()",
        "with cm() as (x,y): (yield APIRouter())",
        "with cm() as [x]: app.include_router(child)",
        "with cm() as obj.x: FastAPI()",
        "with cm() as data[0]: APIRouter()",
        "with cm() as *x: (y := FastAPI())",
        "with cm() as \u{212a}: pass",
        "with (cm() as x, other() as y): FastAPI()",
        "with (\n    cm() as x,\n    other() as y,\n): FastAPI()",
        "try: await FastAPI()\nfinally: pass",
        "match value:\n    case _: FastAPI()",
        "if flag: raise (yield FastAPI())",
        "if flag: assert (x := APIRouter())",
        "if flag: x += await FastAPI()",
        "if flag: a,b = (yield APIRouter())",
        "if flag: emit(*[FastAPI() for x in values])",
        "if factory()(): app.include_router(child)",
        "if flag:\n    def f(x=hidden()): pass",
        "if flag:\n    def f(x: T[hidden()]): pass",
        "if flag:\n    def f() -> T[hidden()]: pass",
        "if flag:\n    def broken(: pass",
        "if flag: pass\nlater = (",
        "break",
        "continue",
        "if flag: break",
        "with cm(): continue",
        "while flag: pass\nelse: break",
        "for x in xs: pass\nelse: continue",
        "while flag:\n    def f(): break",
        "while flag:\n    class Box:\n        if ready(): continue",
        "while flag:\n    class Box:\n        def f(): break",
        "if flag: return value",
        "class Box:\n    if flag: return value",
        "if FastAPI(): pass",
        "while host.include_router(child): pass",
        "for x in APIRouter(): pass",
        "with pkg.FastAPI(): pass",
        "if flag:\n    from arbitrary import APIRouter as R",
    ] {
        for newline in ["\n", "\r\n"] {
            let text = format!("\u{feff}eval() # PRIVATE_ROLLBACK_COMMENT\nclass Before: pass\n{form}\nclass After: pass\n")
                .replace('\n', newline);
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            // AST parsing is not compile validation. Valid-but-deferred forms
            // and invalid loop contexts need not produce an AST parse failure.
            let _ast = extract_captured(&python, &scan.sources).unwrap();
            let native = extract_fact_candidates(&scan.sources).unwrap();
            assert_ne!(
                native.files[0].status,
                CandidateStatus::AcceptedFacts,
                "{form:?}"
            );
            assert_eq!(
                serde_json::to_value(&native.facts).unwrap(),
                json!(expected("unit.py", &[(1, 1, "module", "unit", "unit")])),
                "{form:?}"
            );
            assert!(native.diagnostics.is_empty(), "{form:?}");
        }
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn compound_suite_candidate_replay_order_and_mixed_file_isolation() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};
    let python = oracle();
    let (root, mut scan) = capture(&[
        ("a.py", b"if eval():\n    before()\nmatch value:\n    case _: FastAPI()\n"),
        ("b.py", b"if ready():\n    yes()\nelse:\n    no()\n"),
        ("c.py", b"\xef\xbb\xbffor eval in items():\r\n    with context() as eval:\r\n        emit(eval(), eval()) # PRIVATE_REPLAY_COMMENT\r\nelse:\r\n    done()\r\n"),
        ("d.py", b"if eval():\n    before()\nif broken\n"),
    ]);
    let mut wanted = expected("a.py", &[(1, 1, "module", "a", "a")]);
    let good_b = expected(
        "b.py",
        &[
            (1, 1, "call", "b", "ready()"),
            (1, 1, "module", "b", "b"),
            (2, 2, "call", "b", "yes()"),
            (4, 4, "call", "b", "no()"),
        ],
    );
    let good_c = expected(
        "c.py",
        &[
            (1, 1, "call", "c", "items()"),
            (1, 1, "module", "c", "c"),
            (2, 2, "call", "c", "context()"),
            (3, 3, "call", "c", "emit(eval(), eval())"),
            (3, 3, "call", "c", "eval()"),
            (5, 5, "call", "c", "done()"),
        ],
    );
    wanted.extend(good_b.iter().cloned());
    wanted.extend(good_c.iter().cloned());
    wanted.extend(expected("d.py", &[(1, 1, "module", "d", "d")]));
    let diagnostics = json!([
        {"code":"dynamic_discovery_unresolved","path":"c.py","message":"line 3: eval; runtime targets are not inferred"},
        {"code":"dynamic_discovery_unresolved","path":"c.py","message":"line 3: eval; runtime targets are not inferred"},
    ]);
    let native = extract_fact_candidates(&scan.sources).unwrap();
    assert_eq!(native.files[0].status, CandidateStatus::UnsupportedSyntax);
    assert_eq!(native.files[1].status, CandidateStatus::AcceptedFacts);
    assert_eq!(native.files[2].status, CandidateStatus::AcceptedFacts);
    assert_ne!(native.files[3].status, CandidateStatus::AcceptedFacts);
    let first = serde_json::to_value(native).unwrap();
    assert_eq!(first["facts"], json!(wanted));
    assert_eq!(first["diagnostics"], diagnostics);
    let ast = extract_captured(&python, &scan.sources).unwrap();
    assert!(!ast.complete);
    let mut ast_facts = expected(
        "a.py",
        &[
            (1, 1, "call", "a", "eval()"),
            (1, 1, "module", "a", "a"),
            (2, 2, "call", "a", "before()"),
            (4, 4, "call", "a", "FastAPI()"),
        ],
    );
    ast_facts.extend(good_b.iter().cloned());
    ast_facts.extend(good_c.iter().cloned());
    ast_facts.extend(expected("d.py", &[(1, 1, "module", "d", "d")]));
    assert_eq!(
        semantics(&ast),
        json!({
            "facts": ast_facts, "complete": false,
            "diagnostics": [
                {"code":"dynamic_discovery_unresolved","path":"a.py","message":"line 1: eval; runtime targets are not inferred"},
                {"code":"dynamic_discovery_unresolved","path":"c.py","message":"line 3: eval; runtime targets are not inferred"},
                {"code":"dynamic_discovery_unresolved","path":"c.py","message":"line 3: eval; runtime targets are not inferred"},
                {"code":"parse_failure","path":"d.py","message":"SyntaxError at line 3; source not analyzed"},
            ]
        })
    );
    for path in ["a.py", "b.py", "c.py", "d.py"] {
        fs::remove_file(root.path().join(path)).unwrap();
    }
    assert_eq!(
        first,
        serde_json::to_value(extract_fact_candidates(&scan.sources).unwrap()).unwrap()
    );
    assert_eq!(
        semantics(&ast),
        semantics(&extract_captured(&python, &scan.sources).unwrap())
    );
    scan.sources.reverse();
    let reversed = serde_json::to_value(extract_fact_candidates(&scan.sources).unwrap()).unwrap();
    assert_eq!(first["facts"], reversed["facts"]);
    assert_eq!(first["diagnostics"], reversed["diagnostics"]);
    assert_eq!(
        semantics(&ast),
        semantics(&extract_captured(&python, &scan.sources).unwrap())
    );
    scan.sources
        .retain(|source| matches!(source.path.as_str(), "b.py" | "c.py"));
    let good = assert_warning_candidates_match_ast(&python, &scan);
    let mut good_facts = good_b;
    good_facts.extend(good_c);
    assert_eq!(good["facts"], json!(good_facts));
    assert_eq!(good["diagnostics"], diagnostics);
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn core_expression_candidate_binary_shapes_precedence_and_invalidation() {
    let python = oracle();
    for (operator, class) in [
        ("+", "Add"),
        ("-", "Sub"),
        ("*", "Mult"),
        ("@", "MatMult"),
        ("/", "Div"),
        ("//", "FloorDiv"),
        ("%", "Mod"),
        ("**", "Pow"),
        ("<<", "LShift"),
        (">>", "RShift"),
        ("|", "BitOr"),
        ("^", "BitXor"),
        ("&", "BitAnd"),
    ] {
        let text = format!("x = a {operator} b\nemit(a {operator} b)\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        let shape = format!("emit(a {class} b)");
        assert_eq!(
            native["facts"],
            json!(expected(
                "unit.py",
                &[
                    (1, 1, "binding_invalidation", "unit", r#"{"target":"x"}"#),
                    (1, 1, "module", "unit", "unit"),
                    (2, 2, "call", "unit", &shape),
                ]
            ))
        );
    }
    // These expectations are independently spelled AST tree traversals, not
    // text substitution: parentheses disappear but traversal order does not.
    for (expression, shape) in [
        ("a + b * c", "a Add b Mult c"),
        ("(a + b) * c", "a Add b Mult c"),
        ("a - (b - c)", "a Sub b Sub c"),
        ("a ** b ** c", "a Pow b Pow c"),
        ("(a ** b) ** c", "a Pow b Pow c"),
        ("-a ** b", "<UnaryOp>"),
        ("(-a) ** b", "<UnaryOp> Pow b"),
        ("a ** -b", "a Pow <UnaryOp>"),
        (
            "a | b ^ c & d << e + f * g ** h",
            "a BitOr b BitXor c BitAnd d LShift e Add f Mult g Pow h",
        ),
    ] {
        let text = format!("emit({expression})\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        assert_eq!(
            native["facts"],
            json!(expected(
                "unit.py",
                &[
                    (1, 1, "call", "unit", &format!("emit({shape})")),
                    (1, 1, "module", "unit", "unit"),
                ]
            ))
        );
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn core_expression_candidate_containers_clip_without_ellipsis_and_dicts_are_opaque() {
    let python = oracle();
    for (expression, shape) in [
        ("[]", "[]"),
        ("()", "[]"),
        ("{}", "<Dict>"),
        ("[1]", "[<literal>]"),
        ("{1}", "[<literal>]"),
        ("(a,)", "[a]"),
        ("[a, (b, {c}), {d: e}]", "[a, [b, [c]], <Dict>]"),
        ("{'PRIVATE_CORE_LITERAL': value}", "<Dict>"),
    ] {
        let text = format!("x = {expression}\nemit({expression})\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        assert_eq!(
            native["facts"],
            json!(expected(
                "unit.py",
                &[
                    (1, 1, "binding_invalidation", "unit", r#"{"target":"x"}"#),
                    (1, 1, "module", "unit", "unit"),
                    (2, 2, "call", "unit", &format!("emit({shape})")),
                ]
            ))
        );
    }
    for count in [16, 17] {
        let items = (0..count)
            .map(|n| format!("v{n}"))
            .collect::<Vec<_>>()
            .join(",");
        let rendered = (0..16)
            .map(|n| format!("v{n}"))
            .collect::<Vec<_>>()
            .join(", ");
        for (open, close) in [("[", "]"), ("{", "}"), ("(", ")")] {
            let text = format!("emit({open}{items}{close})\n");
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            let native = assert_declaration_candidates_match_ast(&python, &scan);
            assert_eq!(
                native["facts"],
                json!(expected(
                    "unit.py",
                    &[
                        (1, 1, "call", "unit", &format!("emit([{rendered}])")),
                        (1, 1, "module", "unit", "unit"),
                    ]
                ))
            );
        }
    }
    for text in [
        "x = a, b\nx = a,\ndef f():\n    return a, b\n",
        "emit({key(): value(), eval(): eval()})\n",
        "emit([], (), {}, [[], (), {}], {None, True, False})\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_warning_candidates_match_ast(&python, &scan);
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn core_expression_candidate_opaque_operators_subscripts_slices_and_trivia() {
    let python = oracle();
    for (expression, shape) in [
        ("a and b or c", "<BoolOp>"),
        ("a < b <= c != d", "<Compare>"),
        ("a == b > c >= d", "<Compare>"),
        ("a is b", "<Compare>"),
        ("a is not b", "<Compare>"),
        ("a not in b", "<Compare>"),
        ("a in b", "<Compare>"),
        ("not a", "<UnaryOp>"),
        ("~a", "<UnaryOp>"),
        ("+-a", "<UnaryOp>"),
        ("a if b else c if d else e", "<IfExp>"),
        ("a if (b if c else d) else e", "<IfExp>"),
        ("a[b]", "a[b]"),
        ("a[b,]", "a[[b]]"),
        ("a[b,c]", "a[[b, c]]"),
        ("a[(b,c)]", "a[[b, c]]"),
        ("a[:][::][b:c:d]", "a[<Slice>][<Slice>][<Slice>]"),
        ("a[b:, :c, ::d, e]", "a[[<Slice>, <Slice>, <Slice>, e]]"),
        ("[a,b][0]", "[a, b][<literal>]"),
        ("(a+b)[c*d]", "a Add b[c Mult d]"),
        ("(a if b else c)[d]", "<IfExp>[d]"),
        ("build()[a]", "build()[a]"),
    ] {
        let text = format!("emit({expression})\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        if expression == "build()[a]" {
            assert_eq!(
                native["facts"],
                json!(expected(
                    "unit.py",
                    &[
                        (1, 1, "call", "unit", "build()"),
                        (1, 1, "call", "unit", "emit(build()[a])"),
                        (1, 1, "module", "unit", "unit"),
                    ]
                ))
            );
        } else {
            assert_eq!(
                native["facts"],
                json!(expected(
                    "unit.py",
                    &[
                        (1, 1, "call", "unit", &format!("emit({shape})")),
                        (1, 1, "module", "unit", "unit"),
                    ]
                ))
            );
        }
    }
    let (_, scan) = capture(&[("unit.py", b"emit(a+b, [c], z=d[e], a={f:g})\n")]);
    let native = assert_declaration_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (1, 1, "call", "unit", "emit(a Add b, [c], z=d[e], a=<Dict>)"),
                (1, 1, "module", "unit", "unit"),
            ]
        ))
    );
    for newline in ["\n", "\r\n"] {
        let text = "emit(\n (a is # gap\n not b),\n (c not # gap\n in d),\n data[\n lower # bound\n : upper : step,\n key,\n ],\n result=(yes if flag else no),\n)\n".replace('\n', newline);
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
    for text in [
        "eval(), helper()\n",
        "eval(),\nhelper()\n",
        "def f():\n    eval(), helper()\n    eval(),\n    helper()\n",
        "class Box: eval(), helper()\nafter()\n",
        "class Box: eval(),\nafter()\n",
        "def f(x=a if (b if c else d) else e):\n    return x\n",
        "x = {key: a if (b if c else d) else e}\n",
    ] {
        for newline in ["\n", "\r\n"] {
            let source = text.replace('\n', newline);
            let (_, scan) = capture(&[("unit.py", source.as_bytes())]);
            assert_warning_candidates_match_ast(&python, &scan);
        }
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn core_expression_candidate_call_free_defaults_and_header_body_spans() {
    let python = oracle();
    for expression in [
        "DEFAULT",
        "math.inf",
        "Ellipsis",
        "[]",
        "{}",
        "()",
        "{1}",
        "[1]",
        "1+2j",
        "--1",
        "+-1",
        "-True",
        "+False",
        "a+b",
        "a-b",
        "a*b",
        "a@b",
        "a/b",
        "a//b",
        "a%b",
        "a**b",
        "a<<b",
        "a>>b",
        "a|b",
        "a^b",
        "a&b",
        "[a, (b,), {c}, {d:e}]",
        "a and b",
        "a or b",
        "a < b is not c not in d",
        "~a",
        "not a",
        "a if b else c",
        "a[b]",
        "a[b,c]",
        "a[:b:c]",
        "[a][0]",
    ] {
        let text = format!("def outer(x: T={expression}, /, *, key: R={expression}) -> Result:\n    def inner(y={expression}):\n        return emit({expression})\n    return x\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
    for newline in ["\n", "\r\n"] {
        let text = "def f(\n value: list[T] = (\n  [a + b, {c: d}, data[lo:hi]]\n ),\n *, flag=(not DEFAULT),\n) -> R:\n    return (\n        emit(\n            value, option=[flag],\n        )\n    )\n    # trailing\n".replace('\n', newline);
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        let facts = native["facts"].as_array().unwrap();
        assert!(facts
            .iter()
            .any(|f| f["kind"] == "parameter_default" && f["line"] == 3 && f["endLine"] == 3));
        assert!(facts
            .iter()
            .any(|f| f["kind"] == "function" && f["endLine"] == 11));
        assert!(facts
            .iter()
            .any(|f| f["kind"] == "call" && f["line"] == 8 && f["endLine"] == 10));
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn core_expression_candidate_hidden_calls_and_shared_render_budgets() {
    let python = oracle();
    let heavy = format!(
        "[{},eval()]",
        ["[a,a,a,a,a,a,a,a,a,a,a,a,a,a,a,a]"; 16].join(",")
    );
    for expression in [
        "[eval(), eval()]",
        "(eval(), eval())",
        "{eval(), eval()}",
        "{eval(): eval()}",
        "eval() + eval()",
        "eval() and eval()",
        "eval() < eval()",
        "-eval()",
        "~eval()",
        "not eval()",
        "eval() if eval() else eval()",
        "eval()[eval()]",
        "data[eval():eval():eval()]",
        "data[eval(), eval():eval()]",
    ] {
        for depth in [0, 11, 12, 13] {
            let text = format!(
                "emit({}{}{})\n",
                "[".repeat(depth),
                expression,
                "]".repeat(depth)
            );
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            let native = assert_warning_candidates_match_ast(&python, &scan);
            assert!(!native["diagnostics"].as_array().unwrap().is_empty());
            assert!(native["facts"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["value"] == "eval()"));
        }
        // Calls remain observable when the enclosing rendering has already
        // exhausted its node budget or filled its entire byte allowance.
        for text in [
            format!("emit({heavy}, {expression})\n"),
            format!("emit({}, {expression})\n", "a".repeat(513)),
        ] {
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            let native = assert_warning_candidates_match_ast(&python, &scan);
            assert!(native["facts"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["value"] == "eval()"));
            assert!(!native["diagnostics"].as_array().unwrap().is_empty());
        }
    }
    for count in [16, 17] {
        for expression in [
            format!("[{}, eval(), eval()]", ["a"].repeat(count).join(",")),
            format!("({}, eval(), eval())", ["a"].repeat(count).join(",")),
            format!("{{{}, eval(), eval()}}", ["a"].repeat(count).join(",")),
        ] {
            let (_, scan) = capture(&[("unit.py", format!("emit({expression})\n").as_bytes())]);
            let native = assert_warning_candidates_match_ast(&python, &scan);
            assert_eq!(native["diagnostics"].as_array().unwrap().len(), 2);
        }
    }
    // Root subscript + receiver + synthetic tuple = 3; each U[...] = 9.
    // The final names cross the shared 80-node budget at 79/80/81 visits.
    for extra in [4, 5, 6] {
        let mut items = vec!["U[a,a,a,a,a,a]"; 8];
        items.extend(std::iter::repeat_n("a", extra));
        let expression = format!("T[{}]", items.join(","));
        let text = format!("def f(x={expression}):\n    emit({expression}, eval())\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_warning_candidates_match_ast(&python, &scan);
        let mut rendered = vec!["U[[a, a, a, a, a, a]]"; 8];
        rendered.extend(std::iter::repeat_n("a", extra.min(5)));
        if extra == 6 {
            rendered.push("...");
        }
        let shape = format!("T[[{}]]", rendered.join(", "));
        assert!(native["facts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["kind"] == "parameter_default" && f["value"] == shape));
    }
    for opaque in [
        format!("{{a:{heavy}}}"),
        format!("a and {heavy}"),
        format!("a < {heavy}"),
        format!("-{heavy}"),
        format!("not {heavy}"),
        format!("{heavy} if a else b"),
        format!("data[:{heavy}]"),
    ] {
        let text = format!("emit({opaque}, tail)\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_warning_candidates_match_ast(&python, &scan);
        assert!(native["facts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["kind"] == "call" && f["value"].as_str().unwrap().ends_with(", tail)")));
    }
    for size in [511, 512, 513] {
        let name = "a".repeat(size);
        let text = format!("{name}({heavy}, eval())\nemit([{name}], {name}[a,b])\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_warning_candidates_match_ast(&python, &scan);
        let prefix = "a".repeat(size.min(512));
        assert!(native["facts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["kind"] == "call" && f["value"].as_str().unwrap().starts_with(&prefix)));
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn core_expression_candidate_malformed_future_forms_and_header_calls_roll_back() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};
    let python = oracle();
    for size in [511, 512] {
        let text = format!(
            "def f(x={}):\n    emit([1], a+b, key=DEFAULT)\n",
            "1".repeat(size)
        );
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
    let mut forms = [
        "x = a if b if c else d else e",
        "def f(x=a if b if c else d else e): pass",
        "x = {key: a if b if c else d else e}",
        "eval(),,helper()",
        "eval()\n,helper()",
        "x = {,}",
        "x = a <> b",
        "x = a + * b",
        "x = [a,,b]",
        "x = a[]",
        "x = a[b:::c]",
        "x = a +\nb",
        "x = a \\\n + FastAPI()",
        "def f(x=[*(yield hidden())]): pass",
        "def f(x=(*(yield hidden()),)): pass",
        "def f(x={*(yield hidden())}): pass",
        "def f(x={**(yield hidden())}): pass",
        "def f(x=[await work() for a in b]): pass",
        "def f(x={await work() for a in b}): pass",
        "def f(x={a:await work() for a in c}): pass",
        "def f(x=(await work() for a in b)): pass",
        "def f(x=lambda: (a := hidden())): pass",
        "def f(x=await work()): pass",
        "def f(x=(yield hidden())): pass",
        "def f(x=(a := hidden())): pass",
        "emit(*[APIRouter() for x in items])",
        "emit(**{x:FastAPI() for x in items})",
        "emit(key=a, b)",
        "emit(key=a, key=(lambda: (yield FastAPI())))",
        "data[0](FastAPI())",
        "factory()()(APIRouter())",
        "factory().include_router(child)",
        "other.router = [a]",
        "class Box(A | B | hidden()): pass",
        "def f(x: A | B | hidden()): pass",
        "x: A | B | hidden() = [a]",
        "def f(x={a: hidden()}): pass",
        "def f(x=not hidden()): pass",
        "def f(x={hidden(): a}): pass",
        "def f(x=a and hidden()): pass",
        "def f(x=a < hidden()): pass",
        "def f(x=-hidden()): pass",
        "def f(x=a+hidden()): pass",
        "def f(x=a if hidden() else b): pass",
        "def f(x=data[:hidden()]): pass",
        "def f(x: T[hidden()]): pass",
        "def f() -> T[hidden()]: pass",
        "class Box(hidden()): pass",
        "def f(x='\\xQ0'): pass",
        "def f(x=513_): pass",
        "def f(x=0b102): pass",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    for (open, close) in [("[", "]"), ("(", ")"), ("{", "}")] {
        forms.push(format!(
            "def f(x={open}{}, hidden(){close}): pass",
            ["a"; 17].join(",")
        ));
    }
    forms.push(format!(
        "def f(x={}hidden(){}): pass",
        "[".repeat(15),
        "]".repeat(15)
    ));
    forms.push(format!("def f(x={}): pass", "1".repeat(4301)));
    for future in [
        "[*[await FastAPI() for x in items]]",
        "{**{x:await FastAPI() for x in items}}",
        "[APIRouter() for a in b]",
        "(lambda: (yield FastAPI()))",
        "(a := FastAPI())",
    ] {
        forms.extend([
            format!("x = {{a: {future}}}"),
            format!("x = a and {future}"),
            format!("x = data[:{future}]"),
            format!("x = [{}, {future}]", ["a"; 17].join(",")),
        ]);
    }
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
        assert_eq!(
            serde_json::to_value(&native.facts).unwrap(),
            json!(expected("unit.py", &[(1, 1, "module", "unit", "unit")]))
        );
        assert!(native.diagnostics.is_empty(), "{form}");
    }
    let (_, scan) = capture(&[("unit.py", b"eval()\nx = a if b if c else d else e\n")]);
    assert!(!extract_captured(&python, &scan.sources).unwrap().complete);
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn core_expression_candidate_source_replay_order_and_file_isolation() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};
    let python = oracle();
    let (root, mut scan) = capture(&[
        ("a.py", b"eval()\nx = [a+b]\ndef f(x={a: hidden()}): pass\n"),
        ("b.py", b"from arbitrary import eval as load\ndef f(x=[a+b]):\n    return emit({load():load()}, data[load():], [load(),load()])\n"),
        ("c.py", b"class Box(Base):\n    field: T = [a+b]\n    def run(self, x=DEFAULT):\n        self.field = data[a,b]\n        return emit(not x, x if y else z, {eval():eval()})\n"),
    ]);
    let native = extract_fact_candidates(&scan.sources).unwrap();
    assert_eq!(native.files[0].status, CandidateStatus::UnsupportedSyntax);
    let first = serde_json::to_value(native).unwrap();
    let unsupported = scan.sources.remove(0);
    let good = assert_warning_candidates_match_ast(&python, &scan);
    let mut facts = expected("a.py", &[(1, 1, "module", "a", "a")]);
    facts.extend(good["facts"].as_array().unwrap().iter().cloned());
    assert_eq!(first["facts"], json!(facts));
    assert_eq!(first["diagnostics"], good["diagnostics"]);
    scan.sources.insert(0, unsupported);
    let ast = extract_captured(&python, &scan.sources).unwrap();
    for path in ["a.py", "b.py", "c.py"] {
        fs::remove_file(root.path().join(path)).unwrap();
    }
    assert_eq!(
        first,
        serde_json::to_value(extract_fact_candidates(&scan.sources).unwrap()).unwrap()
    );
    assert_eq!(
        semantics(&ast),
        semantics(&extract_captured(&python, &scan.sources).unwrap())
    );
    scan.sources.reverse();
    let reversed = serde_json::to_value(extract_fact_candidates(&scan.sources).unwrap()).unwrap();
    assert_eq!(first["facts"], reversed["facts"]);
    assert_eq!(first["diagnostics"], reversed["diagnostics"]);
    assert_eq!(
        semantics(&ast),
        semantics(&extract_captured(&python, &scan.sources).unwrap())
    );
    scan.sources.retain(|source| source.path != "a.py");
    assert_warning_candidates_match_ast(&python, &scan);
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn core_expression_candidate_cross_family_owners_annotations_and_statement_spans() {
    let python = oracle();
    for rhs in [
        "source",
        "pkg.source",
        "build([a+b])",
        "[]",
        "()",
        "{}",
        "{a}",
        "a,b",
        "a+b",
        "a and b",
        "a<b",
        "-a",
        "not a",
        "a if b else c",
        "data[a:b,c]",
    ] {
        let text = format!("from arbitrary import eval as load\ndef outer(x=[DEFAULT, pkg.value]):\n    class Box(Base, pkg.Other):\n        field: list[T] = {rhs}\n        def run(self, cls, x={{a:b}}):\n            local: T = {rhs}\n            self.field: T = {rhs}\n            cls.field = {rhs}\n            load: T\n            return emit({{load():load()}}, [load(),load()])\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_warning_candidates_match_ast(&python, &scan);
        assert_eq!(native["diagnostics"].as_array().unwrap().len(), 4);
        assert!(!native["facts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["kind"] == "binding_invalidation" && f["value"] == r#"{"target":"load"}"#));
    }
    for newline in ["\n", "\r\n"] {
        let text = "class Box(Base):\n    field: T = (\n        [\n            build(a+b),\n        ]\n    )\n    # trailing\n".replace('\n', newline);
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        assert_eq!(
            native["facts"],
            json!(expected(
                "unit.py",
                &[
                    (1, 1, "base", "unit.Box", "Base"),
                    (1, 1, "module", "unit", "unit"),
                    (1, 6, "class", "unit.Box", "Box"),
                    (2, 6, "attribute_annotation", "unit.Box.field", "T"),
                    (
                        2,
                        6,
                        "binding_invalidation",
                        "unit.Box",
                        r#"{"target":"field"}"#
                    ),
                    (4, 4, "call", "unit.Box", "build(a Add b)"),
                ]
            ))
        );
    }
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
        "class Box(T[U, hidden()]): pass".into(),
        "class Box((1, hidden())): pass".into(),
        "class Box((A, B, hidden())): pass".into(),
        "class Box(A | B | hidden()): pass".into(),
        "class Box(*(bases, hidden())): pass".into(),
        "class Box(metaclass=hidden()): pass".into(),
        "class Box[T](Base[hidden()]): pass".into(),
        "class Box(\n Base[hidden()],\n): pass".into(),
        "@decorate\nclass Box(Base): pass".into(),
        "class Box(Base):\n    field: mark()".into(),
        "class Box(Base):\n    field: T[mark()]".into(),
        "class Box(Base):\n    field: T | U | hidden()".into(),
        "class Box(Base):\n    field: (1, hidden())".into(),
        "class Box(Base):\n    field: (T, U, hidden())".into(),
        "class Box(Base):\n    field: T[A][B, hidden()]".into(),
        "class Box(Base):\n    (field): T | U | hidden()".into(),
        "(self.router): T = value".into(),
        "other.prefix: T = value".into(),
        "values[0]: T = FastAPI()".into(),
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
        ("a.py", b"class Before(Base):\n    field: T = eval()\nclass Hidden(metaclass=hidden()): pass\n"),
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
        "class Box(Base[T, hidden()]): pass",
        "class Box(object()): pass",
        "class Box(*(bases, hidden())): pass",
        "class Box(metaclass=hidden()): pass",
        "class Box(**(options, hidden())): pass",
        "class Box[T: hidden()]: pass",
        "class Box(factory()): pass",
        "@decorate\nclass Box: pass",
        "class Box:\n    @staticmethod\n    def run(): pass",
        "class Box:\n    @classmethod\n    def run(cls): pass",
        "class Box:\n    @property\n    def value(self): pass",
        "class Box:\n    value: T | U | hidden()",
        "class Box:\n    value: T | U | hidden() = build()",
        "class Box:\n    def run(self):\n        self.value: T | U | hidden() = value",
        "class Box:\n    return work()",
        "def outer():\n    class Box:\n        return work()",
        "class Box:\n    def run(self):\n        class Local:\n            return work()",
        "class Box:\n    app = FastAPI()",
        "class Box:\n    from arbitrary import APIRouter as R",
        "class Box:\n    self.router = value",
        "class Box:\n    def run(self):\n        self.prefix = None",
        "class Box:\n    host.include_router(child)",
        "class Box:\n    assert (x := FastAPI())",
        "class Box:\n    def run(self):\n        await APIRouter()",
        "class Box(\n): FastAPI()",
        "class\nBox: pass",
        "class \\\nBox: FastAPI()",
        "class Box: FastAPI(); work()",
        "class Box:\n\tFastAPI()",
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
        ("a.py", b"class Before: pass\neval()\nclass Hidden(metaclass=hidden()): pass\n"),
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
        "class Hidden(metaclass=hidden()): pass\n",
        "from arbitrary import FastAPI as factory\n",
        "FastAPI()\n",
        "other.include_router(child)\n",
        "def f(x: eval()): pass\n",
        "from package import APIRouter as unused\n",
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
        "from package import APIRouter as unused",
        "from package import ((item))",
        "from package import (\n    *,\n)",
        "import package,",
        "from package import item,",
        "from package import item.child",
        "from __future__ import FastAPI",
        "def f(): import package.APIRouter as unused",
        "import package.APIRouter as unused; ordinary()",
        "import\tpackage.APIRouter as unused",
        "import package\u{c}",
        "import \u{212a}",
        "from package import \u{212a}",
        "import package \\\n .APIRouter as unused",
        "from package \\\n import APIRouter as unused",
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
        ("from __future__ import (FastAPI,)", false),
        ("from package \\\n import (APIRouter as unused)", false),
        ("from package import (APIRouter \\\n as local)", false),
        ("def f(): from package import (APIRouter as unused)", false),
        (
            "from package import (APIRouter as unused); ordinary()",
            false,
        ),
        ("from\tpackage import (APIRouter as unused)", false),
        ("from package import (\n\tAPIRouter as unused\n)", false),
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
        "from tools import APIRouter as unused\n",
        "class Hidden(metaclass=hidden()):\n    pass\n",
        "def f(a=build()): pass\n",
        "def f(a: marker()): pass\n",
        "def f() -> marker(): pass\n",
        "@decorate()\ndef f(): pass\n",
        "def f[T: build()](): pass\n",
        "def f():\n    f'{FastAPI()}'\n",
        "def f(): pass; FastAPI()\n",
        "def f():\n    try:\n        await FastAPI()\n    finally:\n        pass\n",
        "def f():\n    global value\n    (x := APIRouter())\n",
        "def f():\n    nonlocal value\n    (yield FastAPI())\n",
        "def f(a, a): pass\n",
        "def f(*, **opts): pass\n",
        "def f(a, /, /): pass\n",
        "def f((a,b)): pass\n",
        "def caf\u{e9}(): pass\n",
        "def \u{212a}(): pass\n",
        "def f(\u{212a}): pass\n",
        "def f():\n\tFastAPI()\n",
        "def f():\n  pass\n    pass\n",
        "def \\\nf():\n    FastAPI()\n",
        "\u{200b}def f(): pass\n",
        "# type: ignore\n",
        "def f(\n a,\n): FastAPI()\n",
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
        "r'a\\b'",
        "'a\\n'",
        "'unknown\\q'",
        "'octal\\777'",
        "f'PRIVATE_HEADER_LITERAL'",
        "f'{value}'",
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
    for digits in [513, 4300] {
        let text = format!("def f(value={}): pass\n", "1".repeat(digits));
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
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
        "A | B | hidden()".into(),
        "Callable[[A],B,hidden()]".into(),
        "T[A:B:hidden()]".into(),
        "T[*A,hidden()]".into(),
        "T[A][B,hidden()]".into(),
        "f'PRIVATE_HEADER_LITERAL{hidden()}'".into(),
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
    // Call-free nonliteral/operator defaults have positive core-expression
    // gates. Calls, future forms and malformed literal spellings still reject.
    for default in [
        "build()",
        "[build()]",
        "{'key': build()}",
        "DEFAULT[build()]",
        "lambda: build()",
        "f'PRIVATE_HEADER_LITERAL{hidden()}'",
        "f'{build()}'",
        "t'text'",
        "ur'text'",
        "'a' b'b'",
        "b'a' 'b'",
        "'\\xQ0'",
        "'\\u123'",
        "'\\U00110000'",
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
        format!("def f(x={}): pass\n", "1".repeat(4301)),
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
        "other.router = value".into(),
        "x: T | U | hidden() = value".into(),
        "a = b = FastAPI()".into(),
        "a, b = [await FastAPI() for x in values]".into(),
        "[a] = (lambda: (yield APIRouter()))".into(),
        "x[0] = APIRouter()".into(),
        "x += [APIRouter() for x in values]".into(),
        "del x\n    [await FastAPI() for x in values]".into(),
        "(x := APIRouter())".into(),
        "await FastAPI()".into(),
        "yield APIRouter()".into(),
        "outer(*[FastAPI() for x in items])".into(),
        "outer(**{x:APIRouter() for x in options})".into(),
        "outer(x=1, x=(lambda: (yield FastAPI())))".into(),
        "factory()()(FastAPI())".into(),
        "factory().include_router(child)".into(),
        "outer(await FastAPI() for x in source)".into(),
        "outer([APIRouter() for x in source])".into(),
        "outer(lambda: FastAPI())".into(),
        "outer([*[await FastAPI() for x in items]])".into(),
        "x = a + (lambda: (b := APIRouter()))".into(),
    ];
    for callee in ["FastAPI", "APIRouter"] {
        statements.push(format!("{callee}('PRIVATE_BODY_LITERAL')"));
        statements.push(format!("package.{callee}('PRIVATE_BODY_LITERAL')"));
    }
    statements.extend([
        format!(
            "outer({}, *[FastAPI() for x in items])",
            ["value"; 17].join(",")
        ),
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
        ("a.py", b"def before(): pass\nx = work()\nfrom hidden import APIRouter as unused\n"),
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
