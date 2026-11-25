//! Python 3.12 generic declarations and broad, call-free header roles.
use super::*;

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn generic_batch_type_alias_rhs_roots_calls_and_neutral_names() {
    let python = oracle();
    for rhs in [
        "Name",
        "pkg.Name",
        "None",
        "'redacted'",
        "b'redacted'",
        "...",
        "[hidden()]",
        "(hidden(),)",
        "{hidden()}",
        "{key: hidden()}",
        "{**hidden()}",
        "left | hidden()",
        "-hidden()",
        "not hidden()",
        "left and hidden()",
        "left < hidden()",
        "left if hidden() else right",
        "items[hidden()]",
        "items[start:hidden():step]",
        "[*hidden()]",
        "hidden(arg, *args, key=value, **opts)",
        "(lambda: hidden())()",
        "lambda a=hidden(): a",
        "[hidden() for x in xs if ready]",
        "{hidden() for x in xs}",
        "{x: hidden() for x in xs}",
        "(hidden() for x in xs)",
        "await hidden()",
        "(yield hidden())",
        "(yield from hidden())",
        "(written := hidden())",
        "f'{T}'",
        "f'{hidden()}'",
    ] {
        let text = format!("type Alias[T: (Left, Right), *Ts, **P] = {rhs}\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_warning_candidates_match_ast(&python, &scan);
    }
    let (_, scan) = capture(&[("unit.py", b"type Alias[T] = f'{hidden()}'\n")]);
    let native = assert_warning_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (1, 1, "call", "unit", "hidden()"),
                (1, 1, "module", "unit", "unit"),
            ]
        ))
    );
    let (_, scan) = capture(&[(
        "unit.py",
        b"type Alias[T] = hidden()\nclass Box:\n    type Field[T: Bound] = nested()\n    def run[U]():\n        type Local = inner()\n        after()\ntail()\n",
    )]);
    let native = assert_warning_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["identity"]["policy"],
        "python-native-private-facts-15"
    );
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (1, 1, "call", "unit", "hidden()"),
                (1, 1, "module", "unit", "unit"),
                (2, 6, "class", "unit.Box", "Box"),
                (3, 3, "call", "unit.Box", "nested()"),
                (4, 6, "function", "unit.Box.run", "run"),
                (
                    4,
                    6,
                    "signature",
                    "unit.Box.run",
                    r#"{"keywordOnly":[],"positional":[],"positionalOnly":0}"#
                ),
                (5, 5, "call", "unit.Box.run", "inner()"),
                (6, 6, "call", "unit.Box.run", "after()"),
                (7, 7, "call", "unit", "tail()"),
            ]
        ))
    );
    // Neither a TypeAlias name nor a type parameter updates the import alias map.
    let (_, scan) = capture(&[(
        "unit.py",
        b"from arbitrary import eval as probe\ntype probe[probe: Bound] = probe()\nprobe()\n",
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
                (2, 2, "call", "unit", "probe()"),
                (3, 3, "call", "unit", "probe()"),
            ]
        ))
    );
    assert_eq!(native["diagnostics"].as_array().unwrap().len(), 2);
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn generic_batch_function_async_signatures_bounds_and_alias_constancy() {
    let python = oracle();
    let (_, scan) = capture(&[("unit.py", b"def f[T: (Left, Right), *Ts, **P](a: T, /, b=default, *args: Ts, key: P, **opts: P) -> Result[T]:\n    body()\nasync def g[U: pkg.Bound]():\n    await work()\n")]);
    let native = assert_warning_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (1, 1, "module", "unit", "unit"),
                (1, 1, "parameter", "unit.f.a", "a"),
                (1, 1, "parameter", "unit.f.args", "args"),
                (1, 1, "parameter", "unit.f.b", "b"),
                (1, 1, "parameter", "unit.f.key", "key"),
                (1, 1, "parameter", "unit.f.opts", "opts"),
                (1, 1, "parameter_annotation", "unit.f.a", "T"),
                (1, 1, "parameter_annotation", "unit.f.args", "Ts"),
                (1, 1, "parameter_annotation", "unit.f.key", "P"),
                (1, 1, "parameter_annotation", "unit.f.opts", "P"),
                (1, 1, "parameter_default", "unit.f.b", "default"),
                (1, 1, "return_annotation", "unit.f", "Result[T]"),
                (1, 2, "function", "unit.f", "f"),
                (
                    1,
                    2,
                    "signature",
                    "unit.f",
                    r#"{"keywordOnly":["key"],"kwarg":"opts","positional":["a","b"],"positionalOnly":1,"vararg":"args"}"#
                ),
                (2, 2, "call", "unit.f", "body()"),
                (3, 4, "async_function", "unit.g", "g"),
                (
                    3,
                    4,
                    "signature",
                    "unit.g",
                    r#"{"keywordOnly":[],"positional":[],"positionalOnly":0}"#
                ),
                (4, 4, "call", "unit.g", "work()"),
            ]
        ))
    );
    for definition in ["def", "async def"] {
        let text = format!("from arbitrary import eval as probe\n{definition} f[probe: (Left, Right)](arg: probe) -> probe:\n    probe()\n{definition} f[probe: Other]():\n    probe()\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_warning_candidates_match_ast(&python, &scan);
        assert_eq!(native["diagnostics"].as_array().unwrap().len(), 2);
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn generic_batch_classes_mixed_nesting_bases_and_attributes() {
    let python = oracle();
    let (_, scan) = capture(&[("unit.py", b"class Box[T: (Left, Right), *Ts, **P](Base[T] | Other):\n    field: list[T]\n    type Alias[U] = make()\n    def run[V](self):\n        class Inner[W](Base[V]):\n            value: W\n        tail()\nclass Last[X]: pass\n")]);
    let native = assert_warning_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (1, 1, "base", "unit.Box", "Base[T] BitOr Other"),
                (1, 1, "module", "unit", "unit"),
                (1, 7, "class", "unit.Box", "Box"),
                (2, 2, "attribute_annotation", "unit.Box.field", "list[T]"),
                (3, 3, "call", "unit.Box", "make()"),
                (4, 4, "parameter", "unit.Box.run.self", "self"),
                (4, 7, "function", "unit.Box.run", "run"),
                (
                    4,
                    7,
                    "signature",
                    "unit.Box.run",
                    r#"{"keywordOnly":[],"positional":["self"],"positionalOnly":0}"#
                ),
                (5, 5, "base", "unit.Box.run.Inner", "Base[V]"),
                (5, 6, "class", "unit.Box.run.Inner", "Inner"),
                (
                    6,
                    6,
                    "attribute_annotation",
                    "unit.Box.run.Inner.value",
                    "W"
                ),
                (7, 7, "call", "unit.Box.run", "tail()"),
                (8, 8, "class", "unit.Last", "Last"),
            ]
        ))
    );
    for base in [
        "Base[T, *Ts]",
        "(lambda: Base)",
        "[Base, Other]",
        "{Base}",
        "{key: Base}",
        "Base if ready else Other",
        "Base or Other",
        "-Base",
        "Base < Other",
        "items[start:stop:step]",
        "(bound := Base)",
        "[Base for item in values]",
        "(Base for item in values)",
        "await Base",
        "(yield Base)",
        "(yield from Bases)",
    ] {
        let text = format!(
            "def outer[T]():\n    class Box[U]({base}):\n        field: ({base})\n    after()\n"
        );
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_warning_candidates_match_ast(&python, &scan);
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn generic_batch_type_parameter_grammar_ast_only_and_future_defaults() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};
    let python = oracle();
    for parameters in [
        "T",
        "T,",
        "T: Bound",
        "T: (Left, Right)",
        "T: (Bound,)",
        "T: ()",
        "T: tuple[Left, Right]",
        "T: (lambda: Bound)",
        "T: [Bound for item in values]",
        "T: (bound := Bound)",
        "T: await Bound",
        "T: (yield Bound)",
        "T: (yield from Bounds)",
        "*Ts",
        "**P",
        "T, *Ts, **P",
        "type, match, case",
    ] {
        for form in [
            format!("type Alias[{parameters}] = T\n"),
            format!("def f[{parameters}](): pass\n"),
            format!("async def f[{parameters}](): pass\n"),
            format!("class Box[{parameters}]: pass\n"),
        ] {
            let (_, scan) = capture(&[("unit.py", form.as_bytes())]);
            assert_warning_candidates_match_ast(&python, &scan);
        }
    }
    // Ask ast.parse about duplicates/order, never substitute compile's policy.
    for parameters in [
        "T, T",
        "*Ts, *Us",
        "**P, **Q",
        "**P, T",
        "*Ts, T",
        "T, *T, **T",
    ] {
        for form in [
            format!("type Alias[{parameters}] = T\n"),
            format!("def f[{parameters}](): pass\n"),
            format!("class Box[{parameters}]: pass\n"),
        ] {
            let (_, scan) = capture(&[("unit.py", form.as_bytes())]);
            let ast = extract_captured(&python, &scan.sources).unwrap();
            if ast.complete {
                assert_warning_candidates_match_ast(&python, &scan);
            } else {
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
                assert!(native.diagnostics.is_empty());
            }
        }
    }
    for form in [
        "type Alias[] = T",
        "def f[](): pass",
        "class Box[]: pass",
        "type Alias[T: ] = T",
        "type Alias[T.attr] = T",
        "type Alias[(T)] = T",
        "type Alias[*Ts: Bound] = T",
        "type Alias[**P: Bound] = T",
        "type Alias[T = int] = T",
        "def f[T = int](): pass",
        "class Box[*Ts = *tuple[int]]: pass",
        "type Alias[**P = [int]] = T",
    ] {
        let (_, scan) = capture(&[("unit.py", form.as_bytes())]);
        let ast = extract_captured(&python, &scan.sources).unwrap();
        // A newer selected oracle may parse defaults, but this is a 3.12 policy.
        let future_default =
            form.contains("T = int") || form.contains("Ts = *") || form.contains("P = [");
        if !future_default || ast.python_version.starts_with("3.12.") {
            assert!(!ast.complete, "AST accepted {form}");
        }
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
        assert!(native.diagnostics.is_empty());
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn generic_batch_broad_annotations_shapes_roots_and_argument_spans() {
    let python = oracle();
    for (value, shape) in [
        ("T", "T"),
        ("pkg.T", "pkg.T"),
        ("'hidden'", "<literal>"),
        ("f'{T}'", "<JoinedStr>"),
        ("list[T]", "list[T]"),
        ("T[A][B]", "T[A][B]"),
        (
            "Box[A | B][start:stop].member",
            "Box[A BitOr B][<Slice>].member",
        ),
        ("T | U", "T BitOr U"),
        ("(T, U)", "[T, U]"),
        ("[T, *Ts]", "[T, *Ts]"),
        ("{T, U}", "[T, U]"),
        ("{key: T}", "<Dict>"),
        ("lambda: T", "<Lambda>"),
        ("[T for x in xs]", "<ListComp>"),
        ("{T for x in xs}", "<SetComp>"),
        ("{x: T for x in xs}", "<DictComp>"),
        ("(T for x in xs)", "<GeneratorExp>"),
        ("T if ready else U", "<IfExp>"),
        ("T and U", "<BoolOp>"),
        ("not T", "<UnaryOp>"),
        ("T < U", "<Compare>"),
        ("items[start:stop:step]", "items[<Slice>]"),
        ("await T", "<Await>"),
        ("(yield T)", "<Yield>"),
        ("(yield from Ts)", "<YieldFrom>"),
        ("(named := T)", "<NamedExpr>"),
    ] {
        let text = format!("def f[T](arg: ({value})) -> ({value}): pass\nclass Box[T]:\n    field: ({value})\nfield: ({value})\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_warning_candidates_match_ast(&python, &scan);
        assert_eq!(
            native["facts"],
            json!(expected(
                "unit.py",
                &[
                    (1, 1, "function", "unit.f", "f"),
                    (1, 1, "module", "unit", "unit"),
                    (1, 1, "parameter", "unit.f.arg", "arg"),
                    (1, 1, "parameter_annotation", "unit.f.arg", shape),
                    (1, 1, "return_annotation", "unit.f", shape),
                    (
                        1,
                        1,
                        "signature",
                        "unit.f",
                        r#"{"keywordOnly":[],"positional":["arg"],"positionalOnly":0}"#
                    ),
                    (2, 3, "class", "unit.Box", "Box"),
                    (3, 3, "attribute_annotation", "unit.Box.field", shape),
                ]
            )),
            "{value}"
        );
    }
    let (_, scan) = capture(&[("unit.py", b"def f[T](\n    arg: (\n        list[T]\n    )\n) -> (\n    T | None\n): pass\nclass Box[T]:\n    field: (\n        list[T]\n    )\n")]);
    let native = assert_warning_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (1, 1, "module", "unit", "unit"),
                (1, 7, "function", "unit.f", "f"),
                (
                    1,
                    7,
                    "signature",
                    "unit.f",
                    r#"{"keywordOnly":[],"positional":["arg"],"positionalOnly":0}"#
                ),
                (2, 4, "parameter", "unit.f.arg", "arg"),
                (2, 4, "parameter_annotation", "unit.f.arg", "list[T]"),
                (6, 6, "return_annotation", "unit.f", "T BitOr <literal>"),
                (8, 11, "class", "unit.Box", "Box"),
                (9, 11, "attribute_annotation", "unit.Box.field", "list[T]"),
            ]
        ))
    );
    // These exact ungrouped annotation spellings are AST-positive, but the
    // pinned grammar rejects them. Keep the G4 difference separate from the
    // passing grouped forms instead of waiving parser ERROR nodes.
    for text in [
        "def f(x: T[A][B]) -> T[A][B]: pass\n",
        "class C(Box[A | B][start:stop].member):\n field: Box[A | B][start:stop].member\ndef f(x: Box[A | B][start:stop].member) -> Box[A | B][start:stop].member: pass\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let ast = extract_captured(&python, &scan.sources).unwrap();
        assert!(ast.complete && ast.diagnostics.is_empty());
        let native = super::super::python_native::extract_fact_candidates(&scan.sources).unwrap();
        assert_eq!(native.files[0].status, super::super::python_native::CandidateStatus::SyntaxError);
        assert_eq!(native.facts.len(), 1);
        assert!(native.diagnostics.is_empty());
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn generic_batch_multiline_inline_headers_lf_crlf_bom_and_comments() {
    let python = oracle();
    let source = "def f[\n    T: (Left, Right), # constraints\n    *Ts,\n    **P,\n](\n    arg: T,\n) -> T: body(); tail() # inline suite\nclass Box[\n    T, # parameter\n](\n    Base[T],\n    metaclass=Meta,\n): body(); field: T\ntype Alias[\n    T: Bound,\n] = (\n    hidden()\n)\n";
    for newline in ["\n", "\r\n"] {
        for bom in ["", "\u{feff}"] {
            let text = format!("{bom}{}", source.replace('\n', newline));
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            let native = assert_warning_candidates_match_ast(&python, &scan);
            assert_eq!(
                native["facts"],
                json!(expected(
                    "unit.py",
                    &[
                        (1, 1, "module", "unit", "unit"),
                        (1, 7, "function", "unit.f", "f"),
                        (
                            1,
                            7,
                            "signature",
                            "unit.f",
                            r#"{"keywordOnly":[],"positional":["arg"],"positionalOnly":0}"#
                        ),
                        (6, 6, "parameter", "unit.f.arg", "arg"),
                        (6, 6, "parameter_annotation", "unit.f.arg", "T"),
                        (7, 7, "call", "unit.f", "body()"),
                        (7, 7, "call", "unit.f", "tail()"),
                        (7, 7, "return_annotation", "unit.f", "T"),
                        (8, 13, "class", "unit.Box", "Box"),
                        (11, 11, "base", "unit.Box", "Base[T]"),
                        (13, 13, "attribute_annotation", "unit.Box.field", "T"),
                        (13, 13, "call", "unit.Box", "body()"),
                        (17, 17, "call", "unit", "hidden()"),
                    ]
                ))
            );
        }
    }
    for text in [
        "def f[T](arg: \\\n    T) -> T: import pkg; body()\n",
        "class Box[T](\\\n    Base[T]): import pkg; field: T\n",
        "def outer[T]():\n\tclass Box[U](Base[U]): body()\n\tdef inner[V](): import pkg; body()\n",
        "type Alias[T] = \\\n    hidden()\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_warning_candidates_match_ast(&python, &scan);
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn generic_batch_class_arguments_starred_keywords_grouping_and_order() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};
    let python = oracle();
    for (bases, shape) in [
        ("chosen := Base", "<NamedExpr>"),
        ("(chosen := Base)", "<NamedExpr>"),
        ("*left if ready else right", "*<IfExp>"),
        ("*(left if ready else right)", "*<IfExp>"),
    ] {
        let text = format!("class C({bases}): pass\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        assert_eq!(
            native["facts"],
            json!(expected(
                "unit.py",
                &[
                    (1, 1, "base", "unit.C", shape),
                    (1, 1, "class", "unit.C", "C"),
                    (1, 1, "module", "unit", "unit"),
                ]
            ))
        );
    }
    let (_, scan) = capture(&[("unit.py", b"class Box[T]((Base[T]), *bases, metaclass=(Meta), **options): pass\nclass Other[T](flag=Value, *more, flag=Other, **opts, final=End): pass\n")]);
    let native = assert_warning_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (1, 1, "base", "unit.Box", "*bases"),
                (1, 1, "base", "unit.Box", "Base[T]"),
                (1, 1, "class", "unit.Box", "Box"),
                (1, 1, "module", "unit", "unit"),
                (2, 2, "base", "unit.Other", "*more"),
                (2, 2, "class", "unit.Other", "Other"),
            ]
        ))
    );
    for arguments in [
        "",
        "Base,",
        "*bases",
        "*(Base, Other)",
        "**options",
        "metaclass=Meta",
        "metaclass=Meta, metaclass=Other",
        "**opts, flag=Value, **more",
        "flag=Value, *bases",
        "Base, *bases, metaclass=(lambda: Meta), **{key: value}",
        "Base[T], option=[T for x in xs], **(opts if ready else other)",
    ] {
        let text = format!("class Box[T]({arguments}): pass\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_warning_candidates_match_ast(&python, &scan);
    }
    for arguments in [
        "flag=Value, Base",
        "flag=chosen := Base",
        "**opts, Base",
        "**opts, *bases",
        "Base,,Other",
        ",",
        "*",
        "**",
    ] {
        let text = format!("eval()\nclass Box[T]({arguments}): pass\nexec()\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let ast = extract_captured(&python, &scan.sources).unwrap();
        assert!(!ast.complete, "AST accepted {arguments}");
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_ne!(
            native.files[0].status,
            CandidateStatus::AcceptedFacts,
            "{arguments}"
        );
        assert_eq!(
            serde_json::to_value(&native.facts).unwrap(),
            json!(expected("unit.py", &[(1, 1, "module", "unit", "unit")]))
        );
        assert!(native.diagnostics.is_empty());
    }
    for text in [
        "class C(chosen := hidden()): pass\n",
        "class C(*left if hidden() else right): pass\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert!(extract_captured(&python, &scan.sources).unwrap().complete);
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_eq!(native.files[0].status, CandidateStatus::UnsupportedSyntax);
        assert_eq!(native.facts.len(), 1);
        assert!(native.diagnostics.is_empty());
    }
    let (_, scan) = capture(&[("unit.py", b"value = [*left if ready else right]\n")]);
    assert!(!extract_captured(&python, &scan.sources).unwrap().complete);
    assert_ne!(
        extract_fact_candidates(&scan.sources).unwrap().files[0].status,
        CandidateStatus::AcceptedFacts
    );
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn generic_batch_hidden_header_calls_and_g3_metadata_guards() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};
    let python = oracle();
    for hidden in [
        "hidden()",
        "lambda: hidden()",
        "lambda a=hidden(): a",
        "[hidden() for x in xs]",
        "[x for x in hidden()]",
        "[x for x in xs if hidden()]",
        "[x for items[hidden()] in xs]",
        "{hidden(): value for x in xs}",
        "(hidden() for x in xs)",
        "await hidden()",
        "(yield hidden())",
        "(yield from hidden())",
        "(written := hidden())",
        "f'{hidden()}'",
    ] {
        for header in [
            format!("type Alias[T: ({hidden})] = value"),
            format!("type Alias[A: Bound, T: (Left, ({hidden})), *Ts, **P] = value"),
            format!("def f[T: ({hidden})](): pass"),
            format!("async def f[T: ({hidden})](): pass"),
            format!("class Box[T: ({hidden})]: pass"),
            format!("def f[T](arg=({hidden})): pass"),
            format!("def f[T](*args, key=({hidden})): pass"),
            format!("def f[T](arg: ({hidden})): pass"),
            format!("def f[T](*args: ({hidden}), **opts): pass"),
            format!("def f[T](**opts: ({hidden})): pass"),
            format!("def f[T]() -> ({hidden}): pass"),
            format!("field: ({hidden})"),
            format!("class Box[T]: field: ({hidden})"),
            format!("class Box[T](({hidden})): pass"),
            format!("class Box[T](*({hidden})): pass"),
            format!("class Box[T](metaclass=({hidden})): pass"),
            format!("class Box[T](**({hidden})): pass"),
        ] {
            let text = format!("eval()\n{header}\nexec()\n");
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
        // RHS calls have ordinary body ownership, not deferred-header scheduling.
        let text = format!("def f[T]():\n    type Alias[U] = ({hidden})\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_warning_candidates_match_ast(&python, &scan);
    }
    for rhs in [
        "f'{FastAPI()}'",
        "FastAPI()",
        "pkg.APIRouter()",
        "app.include_router(child)",
        "lambda: FastAPI()",
        "[app.include_router(child) for x in xs]",
    ] {
        let text = format!("eval()\ntype Alias[T] = {rhs}\nexec()\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert!(extract_captured(&python, &scan.sources).unwrap().complete);
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_eq!(
            native.files[0].status,
            CandidateStatus::UnsupportedSyntax,
            "{rhs}"
        );
        assert_eq!(
            serde_json::to_value(&native.facts).unwrap(),
            json!(expected("unit.py", &[(1, 1, "module", "unit", "unit")]))
        );
        assert!(native.diagnostics.is_empty());
    }
    for decorated in [
        "class Box[T]: field: f'{hidden()}'",
        "@decorate\ndef f[T](): pass",
        "@decorate()\nclass Box[T]: pass",
        "@app.get('/path')\nasync def f[T](): pass",
    ] {
        let (_, scan) = capture(&[("unit.py", decorated.as_bytes())]);
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
fn generic_batch_match_inline_imports_semicolons_and_alias_ownership() {
    let python = oracle();
    for text in [
        "from arbitrary import eval as probe\nmatch subject:\n    case [first, *rest] if ready:\n        type probe[T] = probe(); import pkg; probe()\n    case {'key': value, **rest}:\n        def f[probe: Bound](): import other; probe()\n    case _:\n        class Box[T](Base[T]): field: T; type Alias = probe()\nprobe()\n",
        "def outer[T]():\n    match subject:\n        case Item(value) | Other(value):\n            from arbitrary import eval as probe; type Alias[probe] = probe()\n            async def inner[U](): import pkg; probe()\n        case _:\n            class Box[V]: type Alias = probe(); field: V\n    type Tail[W] = probe()\n",
        "class Box[T]:\n    import pkg as p; type Alias[U: p.Bound] = p.make()\n    match subject:\n        case _:\n            type Nested[V] = [p.make() for x in values]\n    def run[W](self): from arbitrary import eval as probe; type Local = probe()\n",
        "from arbitrary import eval as probe\ndef f[probe](): type Alias = probe()\ndef f[probe](): type Alias = probe()\nclass Box[probe]: type Alias = probe()\nclass Box[probe]: type Alias = probe()\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_warning_candidates_match_ast(&python, &scan);
    }
    let (_, scan) = capture(&[("unit.py", b"match subject:\n    case _:\n        def f[T](): type Alias = hidden(); after()\ntail()\n")]);
    let native = assert_warning_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (1, 1, "module", "unit", "unit"),
                (3, 3, "call", "unit.f", "after()"),
                (3, 3, "call", "unit.f", "hidden()"),
                (3, 3, "function", "unit.f", "f"),
                (
                    3,
                    3,
                    "signature",
                    "unit.f",
                    r#"{"keywordOnly":[],"positional":[],"positionalOnly":0}"#
                ),
                (4, 4, "call", "unit", "tail()"),
            ]
        ))
    );
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn generic_batch_replay_deletion_order_isolation_and_hidden_bound_resources() {
    use super::python_native::{extract_fact_candidates, parse_batch, CandidateStatus};
    let python = oracle();
    let (marker_root, _) = capture(&[("placeholder.py", b"")]);
    let marker = marker_root.path().join("not_executed");
    let source = format!("from arbitrary import eval as probe\ndef f[T: (Left, Right)](arg: list[T]):\n    type Alias[U] = probe()\nclass Box[V](Base[V], metaclass=Meta):\n    field: list[V]\n    type Alias = hidden()\ntype Marker = open({:?}, 'w').write('redacted')\n", marker.to_str().unwrap());
    let entries: &[(&str, &[u8])] = &[
        ("a.py", source.as_bytes()),
        ("b.py", b"type Alias[T] = eval()\n"),
        ("c.py", b"eval()\ndef f[T: (lambda: hidden())](): pass\n"),
        ("d.py", b"eval()\ntype Alias[T] = (\n"),
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
    assert!(!ast.complete);
    assert!(ast
        .diagnostics
        .iter()
        .any(|d| d.path == "d.py" && d.code == "parse_failure"));
    assert!(!marker.exists());
    for (path, _) in entries {
        fs::write(root.path().join(path), b"replacement = (\n").unwrap();
    }
    assert_eq!(
        first,
        serde_json::to_value(extract_fact_candidates(&scan.sources).unwrap()).unwrap()
    );
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
    assert!(!marker.exists());
    let text = "type Alias[T: (Left, Right)] = [eval() for x in xs]\n".repeat(80);
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
    assert_eq!(native["diagnostics"].as_array().unwrap().len(), 80);
    // The counted walk must discover deep descendants even in neutral bounds.
    for bound in [
        format!("{}Bound{}", "list[".repeat(180), "]".repeat(180)),
        format!("{}hidden(){}", "list[".repeat(180), "]".repeat(180)),
    ] {
        let text = format!("eval()\ntype Alias[T: {bound}] = value\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert!(extract_captured(&python, &scan.sources).unwrap().complete);
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_eq!(native.files[0].status, CandidateStatus::ResourceLimit);
        assert_eq!(
            serde_json::to_value(&native.facts).unwrap(),
            json!(expected("unit.py", &[(1, 1, "module", "unit", "unit")]))
        );
        assert!(native.diagnostics.is_empty());
    }
}
