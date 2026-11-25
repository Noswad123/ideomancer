//! Unvalidated policy-15 f-string oracle gates. Sources are parsed, never run.
//! Keep G4 CST disagreements visible: no source repair or syntax-error waiver.
use super::super::python_native::{extract_fact_candidates, parse_batch, CandidateStatus};
use super::*;

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn fstring_batch_exact_shapes_roots_prefixes_and_concatenation() {
    let python = oracle();
    for prefix in ["f", "F", "fr", "fR", "Fr", "FR", "rf", "rF", "Rf", "RF"] {
        for body in [
            "",
            "PRIVATE_FSTRING_PAYLOAD",
            "{{PRIVATE_FSTRING_PAYLOAD}}",
            "{field}",
        ] {
            let text = format!("sink({prefix}'{body}')\n");
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
                        (1, 1, "call", "unit", "sink(<JoinedStr>)"),
                        (1, 1, "module", "unit", "unit"),
                    ]
                ))
            );
            assert!(!native.to_string().contains("PRIVATE_FSTRING_PAYLOAD"));
        }
    }
    for value in [
        "'left' f'right'",
        "f'left' 'right'",
        "f'' f''",
        "r'left' F'{field}' u'right'",
        "('left'\n f'{field}'\n 'right')",
    ] {
        let text = format!("sink({value})\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
    let (_, scan) = capture(&[(
        "unit.py",
        b"item = f'{field}'\nother = f'{field}'.member\nthird = f'{field}'()\nsink(f'{field}')\n",
    )]);
    let native = assert_declaration_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (1, 1, "binding_invalidation", "unit", r#"{"target":"item"}"#),
                (1, 1, "module", "unit", "unit"),
                (
                    2,
                    2,
                    "binding",
                    "unit",
                    r#"{"source":"<JoinedStr>.member","target":"other"}"#
                ),
                (
                    3,
                    3,
                    "binding",
                    "unit",
                    r#"{"source":"<JoinedStr>()","target":"third"}"#
                ),
                (3, 3, "call", "unit", "<JoinedStr>()"),
                (4, 4, "call", "unit", "sink(<JoinedStr>)"),
            ]
        ))
    );
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn fstring_batch_nine_warnings_alias_constancy_and_duplicate_diagnostics() {
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
        for callee in [name.to_owned(), format!("arbitrary.{name}")] {
            let text = format!("sink(f'{{{callee}()}}')\n");
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            let native = assert_warning_candidates_match_ast(&python, &scan);
            assert_eq!(
                native["diagnostics"],
                json!([{"code":"dynamic_discovery_unresolved", "path":"unit.py",
                "message":format!("line 1: {callee}; runtime targets are not inferred")}])
            );
        }
        let text = format!("from arbitrary import {name} as probe\nsink(f'{{probe(\"PRIVATE_WARNING_PAYLOAD\")}}{{probe(\"PRIVATE_WARNING_PAYLOAD\")}}')\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_warning_candidates_match_ast(&python, &scan);
        let import = format!(
            r#"{{"level":0,"module":"arbitrary","names":[{{"asname":"probe","name":"{name}"}}]}}"#
        );
        assert_eq!(
            native["facts"],
            json!(expected(
                "unit.py",
                &[
                    (1, 1, "from_import", "unit", &import),
                    (1, 1, "module", "unit", "unit"),
                    (2, 2, "call", "unit", "probe(<literal>)"),
                    (2, 2, "call", "unit", "sink(<JoinedStr>)"),
                ]
            ))
        );
        let warning = json!({"code":"dynamic_discovery_unresolved", "path":"unit.py",
            "message":format!("line 2: arbitrary.{name}; runtime targets are not inferred")});
        assert_eq!(native["diagnostics"], json!([warning.clone(), warning]));
        assert!(!native.to_string().contains("PRIVATE_WARNING_PAYLOAD"));
    }
    // Field, lambda, comprehension, and walrus names do not invent lexical scopes
    // or parameters, nor do they mutate the helper's import-alias table.
    let (_, scan) = capture(&[("unit.py", b"from arbitrary import eval as probe\nsink(f'{probe}{(probe := value)}{(lambda probe: probe())}{[probe() for probe in values]}')\nprobe = f'{probe()}'\nprobe()\n")]);
    assert_warning_candidates_match_ast(&python, &scan);
    let (_, scan) = capture(&[("unit.py", b"sink(f'{eval()}{eval()}')\n")]);
    let native = assert_warning_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (1, 1, "call", "unit", "eval()"),
                (1, 1, "call", "unit", "sink(<JoinedStr>)"),
                (1, 1, "module", "unit", "unit"),
            ]
        ))
    );
    let warning = json!({"code":"dynamic_discovery_unresolved", "path":"unit.py",
        "message":"line 1: eval; runtime targets are not inferred"});
    assert_eq!(native["diagnostics"], json!([warning.clone(), warning]));
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn fstring_batch_nested_format_fields_nested_fstrings_and_call_spans() {
    let python = oracle();
    let (_, scan) = capture(&[(
        "unit.py",
        b"sink(f'{value():{width()}.{precision()}}', f'{f\"{inner()}\"}')\n",
    )]);
    let native = assert_declaration_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (1, 1, "call", "unit", "inner()"),
                (1, 1, "call", "unit", "precision()"),
                (1, 1, "call", "unit", "sink(<JoinedStr>, <JoinedStr>)"),
                (1, 1, "call", "unit", "value()"),
                (1, 1, "call", "unit", "width()"),
                (1, 1, "module", "unit", "unit"),
            ]
        ))
    );
    for value in [
        "f'{x:{{}}}'",
        "f'{x:{{width()}}}'",
        "f'{x:{{{width()}}}}'",
        "f'{{{value()}}}'",
        "f'{x:{f\"{width()}\"}}'",
        "f'{x:()[] + eval()}'",
        "f'{x:!r:=eval()}'",
        "rf'\\{probe()}'",
        "fr'\\N{probe()}'",
    ] {
        let text = format!("sink({value})\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_warning_candidates_match_ast(&python, &scan);
    }
    // Ask ast.parse for each depth. Do not encode the unvalidated native
    // three-field assumption as the authority for the Python grammar.
    for value in [
        "f'{eval()}'",
        "f'{x:{eval()}}'",
        "f'{x:{y:{eval()}}}'",
        "f'{x:{y:{z:{eval()}}}}'",
    ] {
        let text = format!("sink({value})\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let ast = extract_captured(&python, &scan.sources).unwrap();
        if ast.complete {
            assert_warning_candidates_match_ast(&python, &scan);
        } else {
            assert_facts(&ast, "unit.py", &[(1, 1, "module", "unit", "unit")]);
            assert_eq!(ast.diagnostics.len(), 1);
            assert_eq!(ast.diagnostics[0].code, "parse_failure");
            let native = extract_fact_candidates(&scan.sources).unwrap();
            assert_ne!(
                native.files[0].status,
                CandidateStatus::AcceptedFacts,
                "{value}"
            );
            assert_eq!(
                serde_json::to_value(&native.facts).unwrap(),
                json!(expected("unit.py", &[(1, 1, "module", "unit", "unit")]))
            );
            assert!(native.diagnostics.is_empty());
        }
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn fstring_batch_pep701_quotes_comments_newlines_and_bom() {
    let python = oracle();
    for newline in ["\n", "\r\n"] {
        for bom in ["", "\u{feff}"] {
            for comment in ["# ordinary", "# type: ignore", "# type: ignore[attr]"] {
                let text = format!("{bom}sink(f'{{{newline} probe('PRIVATE_QUOTED_PAYLOAD') {comment}{newline}}}', f'{{f'{{inner()}}'}}'){newline}");
                let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
                let native = assert_declaration_candidates_match_ast(&python, &scan);
                assert_eq!(
                    native["facts"],
                    json!(expected(
                        "unit.py",
                        &[
                            (1, 1, "module", "unit", "unit"),
                            (1, 3, "call", "unit", "sink(<JoinedStr>, <JoinedStr>)"),
                            (2, 2, "call", "unit", "probe(<literal>)"),
                            (3, 3, "call", "unit", "inner()"),
                        ]
                    ))
                );
                assert!(!native.to_string().contains("PRIVATE_QUOTED_PAYLOAD"));
                assert_eq!(native["files"][0]["sha256"], hash(text.as_bytes()));
            }
        }
    }
    for value in [
        "f'{\"same\"}'",
        "f\"{\"same\"}\"",
        "f'{\"\\n\".join(parts)}'",
        "f'''line\n{probe()}\ntail'''",
        "rf'{\n probe()\n}'",
    ] {
        let text = format!("sink({value})\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
    for value in ["f'line\nbreak{probe()}'", "rf'line\nbreak{probe()}'"] {
        let text = format!("eval()\nsink({value})\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert!(!extract_captured(&python, &scan.sources).unwrap().complete);
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_ne!(native.files[0].status, CandidateStatus::AcceptedFacts);
        assert_eq!(
            serde_json::to_value(&native.facts).unwrap(),
            json!(expected("unit.py", &[(1, 1, "module", "unit", "unit")]))
        );
        assert!(native.diagnostics.is_empty());
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn fstring_batch_replacement_roles_walrus_format_text_and_no_new_scopes() {
    let python = oracle();
    // The bare := belongs to format text. The pinned CST misclassifies it as
    // NamedExpr and loses field boundaries; retain an explicit G4 incompatibility
    // rather than emit the CST's spurious eval() call or count accepted parity.
    let (_, scan) = capture(&[("unit.py", b"sink(f'{x:=eval()}')\n")]);
    let ast = extract_captured(&python, &scan.sources).unwrap();
    assert!(ast.complete && ast.diagnostics.is_empty());
    assert_eq!(
        serde_json::to_value(&ast.facts).unwrap(),
        json!(expected(
            "unit.py",
            &[
                (1, 1, "call", "unit", "sink(<JoinedStr>)"),
                (1, 1, "module", "unit", "unit"),
            ]
        ))
    );
    let rejected = extract_fact_candidates(&scan.sources).unwrap();
    assert_eq!(rejected.files[0].status, CandidateStatus::UnsupportedSyntax);
    assert_eq!(rejected.facts.len(), 1);
    assert!(rejected.diagnostics.is_empty());
    let (_, scan) = capture(&[("unit.py", b"sink(f'{x:={eval()}}')\n")]);
    let ast = extract_captured(&python, &scan.sources).unwrap();
    assert!(ast.complete && ast.diagnostics.len() == 1);
    let rejected = extract_fact_candidates(&scan.sources).unwrap();
    assert_eq!(rejected.files[0].status, CandidateStatus::UnsupportedSyntax);
    assert_eq!(rejected.facts.len(), 1);
    assert!(rejected.diagnostics.is_empty());
    let (_, scan) = capture(&[("unit.py", b"sink(f'{x: ={eval()}}')\n")]);
    assert_warning_candidates_match_ast(&python, &scan);
    let (_, scan) = capture(&[("unit.py", b"sink(f'{(x:=eval())}')\n")]);
    let native = assert_warning_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (1, 1, "call", "unit", "eval()"),
                (1, 1, "call", "unit", "sink(<JoinedStr>)"),
                (1, 1, "module", "unit", "unit"),
            ]
        ))
    );
    assert_eq!(
        native["diagnostics"],
        json!([{"code":"dynamic_discovery_unresolved", "path":"unit.py", "message":"line 1: eval; runtime targets are not inferred"}])
    );
    // ast.parse, not compilation, decides whether these expression roles exist.
    for field in [
        "yield eval()",
        "yield from eval()",
        "(yield eval())",
        "await eval()",
        "eval(), value",
        "*eval(),",
        "(*eval(), value)",
        "(x := eval())",
        "eval()=",
        "(x := eval())=",
        "(lambda: eval())",
        "(lambda x=eval(): x)",
        "(lambda: eval()), value",
        "value if ready else (lambda: eval())",
        "[lambda: eval()]",
        "call(lambda: eval())",
        "item:{(lambda: eval()), value}",
        "[eval() for item in values]",
        "{eval() for item in values}",
        "{item: eval() for item in values}",
        "(eval() for item in values)",
        "[item for item in eval() if ready()]",
        "left if eval() else right",
        "items[eval():stop:step]",
    ] {
        let text = format!("def owner():\n    return sink(f'{{{field}}}')\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_warning_candidates_match_ast(&python, &scan);
    }
    // Generator syntax is classified by the selected AST helper, not by
    // compilation or by the native parser's potentially broader expression rule.
    let (_, scan) = capture(&[("unit.py", b"sink(f'{eval() for item in values}')\n")]);
    let ast = extract_captured(&python, &scan.sources).unwrap();
    if ast.complete {
        assert_warning_candidates_match_ast(&python, &scan);
    } else {
        assert_facts(&ast, "unit.py", &[(1, 1, "module", "unit", "unit")]);
        assert_eq!(ast.diagnostics.len(), 1);
        assert_eq!(ast.diagnostics[0].code, "parse_failure");
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_ne!(native.files[0].status, CandidateStatus::AcceptedFacts);
        assert_eq!(
            serde_json::to_value(&native.facts).unwrap(),
            json!(expected("unit.py", &[(1, 1, "module", "unit", "unit")]))
        );
        assert!(native.diagnostics.is_empty());
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn fstring_batch_conversions_debug_malformed_grammar_and_rollback() {
    let python = oracle();
    for field in [
        "value!s",
        "value!r",
        "value!a",
        "value=",
        "value = ",
        "value=!s",
        "value=!r:>12",
        "value!a:{width()}",
        "value=:>12",
    ] {
        let text = format!("sink(f'{{{field}}}')\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
    for value in [
        "f'{}'",
        "f'{!r}'",
        "f'{x!q}'",
        "f'{x!S}'",
        "f'{x!R}'",
        "f'{x!A}'",
        "f'{x!rr}'",
        "f'{x!}'",
        "f'{x! r}'",
        "f'{x!r=}'",
        "f'{x!r!s}'",
        "f'{x==}'",
        "f'{lambda: eval()}'",
        "f'{lambda: eval(), value}'",
        "f'{value if ready else lambda: eval()}'",
        "f'{item:{lambda: eval(), value}}'",
        "f'{x'",
        "f'x}'",
        "f'{x:{y}'",
        "t'text'",
        "uF'text'",
        "bf'text'",
        "fb'text'",
        "ff'text'",
        "rff'text'",
        "ur'text'",
        "`text`",
        "f'\\xQ0{x}'",
        "f'\\u123{x}'",
        "f'\\U00110000{x}'",
        "f'\\N{NOT A UNICODE NAME}{x}'",
    ] {
        let text = format!("eval()\nsink({value})\nexec()\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let ast = extract_captured(&python, &scan.sources).unwrap();
        assert!(!ast.complete, "AST unexpectedly accepted {value}");
        assert_facts(&ast, "unit.py", &[(1, 1, "module", "unit", "unit")]);
        assert_eq!(ast.diagnostics.len(), 1);
        assert_eq!(ast.diagnostics[0].code, "parse_failure");
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_ne!(
            native.files[0].status,
            CandidateStatus::AcceptedFacts,
            "{value}"
        );
        assert_eq!(
            serde_json::to_value(&native.facts).unwrap(),
            json!(expected("unit.py", &[(1, 1, "module", "unit", "unit")]))
        );
        assert!(native.diagnostics.is_empty());
    }
    // Known named escapes need unavailable Unicode-name data. This is a native
    // subset exclusion, not a claim that the valid Python source is malformed.
    for value in ["f'\\N{LATIN CAPITAL LETTER A}{x}'", "f'\\N{SNOWMAN}{x}'"] {
        let text = format!("eval()\nsink({value})\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert!(extract_captured(&python, &scan.sources).unwrap().complete);
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_eq!(native.files[0].status, CandidateStatus::UnsupportedSyntax);
        assert_eq!(
            serde_json::to_value(&native.facts).unwrap(),
            json!(expected("unit.py", &[(1, 1, "module", "unit", "unit")]))
        );
        assert!(native.diagnostics.is_empty());
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn fstring_batch_call_free_headers_bounds_and_recursive_call_rejection() {
    let python = oracle();
    for value in [
        "f'plain'",
        "f'{value}'",
        "f'{value:{width}}'",
        "f'{f\"{value}\"}'",
        "f'{(lambda: value)}'",
        "f'{[item for item in values]}'",
        "f'{hidden()}'",
        "f'{value:{hidden()}}'",
        "f'{f\"{hidden()}\"}'",
        "f'{(lambda: hidden())}'",
        "f'{(lambda arg=hidden(): arg)}'",
        "f'{[hidden() for item in values]}'",
        "f'{[item for item in hidden()]}'",
        "f'{[a,a,a,a,a,a,a,a,a,a,a,a,a,a,a,a,hidden()]}'",
        "f'{[[[[[[[[[[[[[[hidden()]]]]]]]]]]]]]]}'",
    ] {
        let has_call = value.contains("hidden()");
        for header in [
            format!("type Alias[T: ({value})] = T"),
            format!("type Alias[A: Bound, T: (Left, ({value})), *Ts, **P] = T"),
            format!("def f[T: ({value})](): pass"),
            format!("async def f[T: ({value})](): pass"),
            format!("class Box[T: ({value})]: pass"),
            format!("def f(arg=({value})): pass"),
            format!("def f(*args, key=({value})): pass"),
            format!("def f(arg: ({value})): pass"),
            format!("def f(*args: ({value})): pass"),
            format!("def f(**opts: ({value})): pass"),
            format!("def f() -> ({value}): pass"),
            format!("field: ({value})"),
            format!("class Box: field: ({value})"),
            format!("class Box(({value})): pass"),
            format!("class Box(*({value})): pass"),
            format!("class Box(metaclass=({value})): pass"),
            format!("class Box(**({value})): pass"),
        ] {
            let text = format!("eval()\n{header}\nexec()\n");
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            if has_call {
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
            } else {
                assert_warning_candidates_match_ast(&python, &scan);
            }
        }
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn fstring_batch_hidden_g3_metadata_guards_past_caps_and_clipping() {
    let python = oracle();
    for call in [
        "FastAPI()".to_owned(),
        "pkg.APIRouter()".to_owned(),
        "app.include_router(child)".to_owned(),
        format!("{}.FastAPI()", "a".repeat(520)),
        format!("{}.include_router(child)", "a".repeat(520)),
    ] {
        for field in [
            call.clone(),
            format!("(lambda: {call})"),
            format!("[{call} for item in values]"),
            format!("f\"{{{call}}}\""),
        ] {
            for value in [
                format!("f'{{{field}}}'"),
                format!("f'{{value:{{{field}}}}}'"),
            ] {
                for expression in [
                    value.clone(),
                    format!(
                        "sink({}, {value})",
                        std::iter::repeat_n("value", 17)
                            .collect::<Vec<_>>()
                            .join(",")
                    ),
                    format!("{}{}{}", "[".repeat(16), value, "]".repeat(16)),
                    format!("sink({}, {value})", "long_name".repeat(70)),
                ] {
                    let text = format!("eval()\nsink({expression})\nexec()\n");
                    let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
                    assert!(extract_captured(&python, &scan.sources).unwrap().complete);
                    let native = extract_fact_candidates(&scan.sources).unwrap();
                    assert_eq!(
                        native.files[0].status,
                        CandidateStatus::UnsupportedSyntax,
                        "{expression}"
                    );
                    assert_eq!(
                        serde_json::to_value(&native.facts).unwrap(),
                        json!(expected("unit.py", &[(1, 1, "module", "unit", "unit")]))
                    );
                    assert!(native.diagnostics.is_empty());
                }
            }
        }
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn fstring_batch_opaque_render_budgets_and_fresh_callee_names() {
    let python = oracle();
    let fields = (0..96)
        .map(|n| format!("{{probe{n}()}}"))
        .collect::<String>();
    let text = format!("sink(f'PRIVATE_OPAQUE_PAYLOAD{fields}{{value:{{eval()}}}}')\n");
    let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
    let native = assert_warning_candidates_match_ast(&python, &scan);
    let calls = (0..96).map(|n| format!("probe{n}()")).collect::<Vec<_>>();
    let mut rows: Vec<Row<'_>> = calls
        .iter()
        .map(|call| (1, 1, "call", "unit", call.as_str()))
        .collect();
    rows.push((1, 1, "call", "unit", "eval()"));
    rows.push((1, 1, "call", "unit", "sink(<JoinedStr>)"));
    rows.push((1, 1, "module", "unit", "unit"));
    rows.sort();
    assert_eq!(native["facts"], json!(expected("unit.py", &rows)));
    assert_eq!(
        native["diagnostics"],
        json!([{"code":"dynamic_discovery_unresolved", "path":"unit.py", "message":"line 1: eval; runtime targets are not inferred"}])
    );
    assert!(!native.to_string().contains("PRIVATE_OPAQUE_PAYLOAD"));
    for value in [
        format!(
            "sink({}, f'{{eval()}}')",
            std::iter::repeat_n("value", 17)
                .collect::<Vec<_>>()
                .join(",")
        ),
        format!("sink({}f'{{eval()}}'{})", "[".repeat(16), "]".repeat(16)),
        format!(
            "sink({}f'{{eval()}}'{})",
            "(left + ".repeat(45),
            ")".repeat(45)
        ),
        format!("sink({}, f'{{eval()}}')", "a".repeat(520)),
        "f'{eval()}'().find_spec()".to_owned(),
        "f'{value:{eval()}}'.method()".to_owned(),
        format!("f'{{eval()}}'.{}()", "a".repeat(520)),
        format!("f'{{value}}'.{}()", "a".repeat(495) + ".eval"),
    ] {
        let text = format!("{value}\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_warning_candidates_match_ast(&python, &scan);
    }
    let (_, scan) = capture(&[("unit.py", b"f'{eval()}'().method()\n")]);
    let native = assert_warning_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (1, 1, "call", "unit", "<JoinedStr>()"),
                (1, 1, "call", "unit", "<JoinedStr>().method()"),
                (1, 1, "call", "unit", "eval()"),
                (1, 1, "module", "unit", "unit"),
            ]
        ))
    );
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn fstring_batch_patterns_reject_joinedstr_and_mixed_bytes_boundaries() {
    let python = oracle();
    for value in [
        "f''",
        "f'plain'",
        "f'{{escaped}}'",
        "f'{value}'",
        "'left' f'right'",
        "f'left' 'right'",
        "rf'plain'",
    ] {
        let text = format!("eval()\nmatch subject:\n    case {value}: exec()\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        // ast.parse can construct these MatchValue(JoinedStr) nodes even though
        // compilation rejects them. Native pattern-expression traversal remains
        // a documented subset gap, not a claimed Python syntax error.
        let ast = extract_captured(&python, &scan.sources).unwrap();
        assert!(ast.complete, "{value}");
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_ne!(
            native.files[0].status,
            CandidateStatus::AcceptedFacts,
            "{value}"
        );
        assert_eq!(
            serde_json::to_value(&native.facts).unwrap(),
            json!(expected("unit.py", &[(1, 1, "module", "unit", "unit")]))
        );
        assert!(native.diagnostics.is_empty());
    }
    for value in [
        "b'left' f'right'",
        "f'left' b'right'",
        "'left' b'right'",
        "b'left' 'right'",
        "b'left' f'' b'right'",
        "f'{eval()}' b'right'",
    ] {
        let text = format!("eval()\nsink({value})\nexec()\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert!(
            !extract_captured(&python, &scan.sources).unwrap().complete,
            "{value}"
        );
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_ne!(native.files[0].status, CandidateStatus::AcceptedFacts);
        assert_eq!(
            serde_json::to_value(&native.facts).unwrap(),
            json!(expected("unit.py", &[(1, 1, "module", "unit", "unit")]))
        );
        assert!(native.diagnostics.is_empty());
    }
    for value in ["'plain' 'text'", "b'plain' br'text'"] {
        let text = format!("match subject:\n    case {value}: pass\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn fstring_batch_depth_delimiters_multiline_spans_and_counted_resources() {
    let python = oracle();
    // Format text is not expression code and must not consume the global
    // 200-delimiter gate. Only actual replacement expressions contribute.
    for text in [
        format!(
            "sink(f'{{value:{}eval(){}}}')\n",
            "([".repeat(220),
            "])".repeat(220)
        ),
        format!(
            "sink(f'{}{{eval()}}{}')\n",
            "([".repeat(220),
            "])".repeat(220)
        ),
        format!("sink(f'{{{}eval(){}}}')\n", "(".repeat(40), ")".repeat(40)),
    ] {
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
    }
    let text = format!(
        "eval()\nsink(f'{{{}value{}}}')\n",
        "(".repeat(201),
        ")".repeat(201)
    );
    let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
    assert!(!extract_captured(&python, &scan.sources).unwrap().complete);
    let native = extract_fact_candidates(&scan.sources).unwrap();
    assert_ne!(native.files[0].status, CandidateStatus::AcceptedFacts);
    assert_eq!(
        serde_json::to_value(&native.facts).unwrap(),
        json!(expected("unit.py", &[(1, 1, "module", "unit", "unit")]))
    );
    assert!(native.diagnostics.is_empty());
    let (_, scan) = capture(&[("unit.py", b"def owner():\n    return sink(f'{\n        probe(\n            value\n        )\n    :{\n        width()\n    }}')\n")]);
    let native = assert_declaration_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (1, 1, "module", "unit", "unit"),
                (1, 8, "function", "unit.owner", "owner"),
                (
                    1,
                    8,
                    "signature",
                    "unit.owner",
                    r#"{"keywordOnly":[],"positional":[],"positionalOnly":0}"#
                ),
                (2, 8, "call", "unit.owner", "sink(<JoinedStr>)"),
                (3, 5, "call", "unit.owner", "probe(value)"),
                (7, 7, "call", "unit.owner", "width()"),
            ]
        ))
    );
    let text = format!(
        "eval()\nsink(f'{{{}value{}}}')\n",
        "nest(".repeat(140),
        ")".repeat(140)
    );
    let (_, scan) = capture(&[
        ("bad.py", text.as_bytes()),
        ("good.py", b"sink(f'{eval()}')\n"),
    ]);
    assert!(extract_captured(&python, &scan.sources).unwrap().complete);
    let native = extract_fact_candidates(&scan.sources).unwrap();
    assert_eq!(native.files[0].status, CandidateStatus::ResourceLimit);
    assert_eq!(native.files[1].status, CandidateStatus::AcceptedFacts);
    let mut facts = expected("bad.py", &[(1, 1, "module", "bad", "bad")]);
    facts.extend(expected(
        "good.py",
        &[
            (1, 1, "call", "good", "eval()"),
            (1, 1, "call", "good", "sink(<JoinedStr>)"),
            (1, 1, "module", "good", "good"),
        ],
    ));
    assert_eq!(serde_json::to_value(&native.facts).unwrap(), json!(facts));
    assert_eq!(
        serde_json::to_value(&native.diagnostics).unwrap(),
        json!([{"code":"dynamic_discovery_unresolved", "path":"good.py", "message":"line 1: eval; runtime targets are not inferred"}])
    );
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn fstring_batch_captured_replay_deleted_source_reverse_and_file_isolation() {
    let python = oracle();
    let entries: &[(&str, &[u8])] = &[
        (
            "a.py",
            b"\xef\xbb\xbfsink(f'{\r\n eval() # type: ignore\r\n :{width()}}')\r\n",
        ),
        (
            "b.py",
            b"def owner():\n    return sink(f'{f\"{probe()}\"}')\n",
        ),
        ("c.py", b"eval()\nsink(f'{value:{FastAPI()}}')\n"),
        ("d.py", b"eval()\nsink(f'{value!q}')\n"),
        (
            "e.py",
            b"eval()\nsink(f'\\N{LATIN CAPITAL LETTER A}{value}')\n",
        ),
    ];
    let (root, mut scan) = capture(entries);
    let (_good_root, good_scan) = capture(&entries[..2]);
    let good = assert_warning_candidates_match_ast(&python, &good_scan);
    let ast = extract_captured(&python, &scan.sources).unwrap();
    assert!(!ast.complete);
    let native = extract_fact_candidates(&scan.sources).unwrap();
    assert_eq!(native.files[2].status, CandidateStatus::UnsupportedSyntax);
    assert_ne!(native.files[3].status, CandidateStatus::AcceptedFacts);
    assert_eq!(native.files[4].status, CandidateStatus::UnsupportedSyntax);
    for (file, (path, bytes)) in native.files.iter().zip(entries) {
        assert_eq!(file.path, *path);
        assert_eq!(file.sha256, hash(bytes));
    }
    let first = serde_json::to_value(native).unwrap();
    let mut facts = good["facts"].as_array().unwrap().clone();
    for module in ["c", "d", "e"] {
        facts.extend(expected(
            &format!("{module}.py"),
            &[(1, 1, "module", module, module)],
        ));
    }
    assert_eq!(first["facts"], json!(facts));
    assert_eq!(first["diagnostics"], good["diagnostics"]);
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
}
