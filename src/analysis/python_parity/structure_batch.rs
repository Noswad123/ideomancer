//! Policy-14 structural composition gates; no target imports or execution.
use super::super::python_native::{extract_fact_candidates, parse_batch, CandidateStatus};
use super::*;

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn structure_candidate_all_eight_match_families_and_composition() {
    let python = oracle();
    // MatchValue, MatchSingleton, MatchSequence, MatchMapping, MatchClass,
    // MatchStar, MatchAs and MatchOr, including AST-neutral grouping.
    for pattern in [
        "constants.VALUE",
        "None",
        "True",
        "False",
        "capture",
        "_",
        "[head, *tail]",
        "[*_]",
        "{'key': value, **rest}",
        "pkg.Record(item, flag=True)",
        "1 | 2",
        "(1 | 2) as chosen",
        "()",
        "[]",
        "{}",
        "head, tail,",
        "*tail,",
        "(*tail,)",
        "(capture)",
        "(capture,)",
        "((capture))",
        "(value as first) as second",
        "Record(key=value as selected)",
        "Record(match=case)",
        "[Record(value=(1 | 2) as selected), {'key': [*rest]}]",
        "{constants.KEY: Record(None, child=[head, *tail]), True: other}",
    ] {
        let text = format!("match subject():\n    case {pattern} if check(): use()\n");
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
                    (1, 1, "call", "unit", "subject()"),
                    (1, 1, "module", "unit", "unit"),
                    (2, 2, "call", "unit", "check()"),
                    (2, 2, "call", "unit", "use()"),
                ]
            )),
            "{pattern}"
        );
    }
    let (_, scan) = capture(&[("unit.py", b"match outer():\n    case [Record(value=(1 | 2) as chosen), *rest]:\n        match inner():\n            case {'key': [head, *_], **remaining} if ready(): done()\n")]);
    assert_declaration_candidates_match_ast(&python, &scan);
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn structure_candidate_pattern_nonwrites_aliases_and_case_visitor_order() {
    let python = oracle();
    let text = concat!(
        "from outer import eval as go\n",
        "match go():\n",
        "    case FastAPI(go, prefix=capture) if go():\n",
        "        from inner import exec as go\n",
        "        go()\n",
        "    case APIRouter(go, tags=[*capture]) if go(): go()\n",
        "go()\n",
    );
    let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
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
                    r#"{"level":0,"module":"outer","names":[{"asname":"go","name":"eval"}]}"#
                ),
                (1, 1, "module", "unit", "unit"),
                (2, 2, "call", "unit", "go()"),
                (3, 3, "call", "unit", "go()"),
                (
                    4,
                    4,
                    "from_import",
                    "unit",
                    r#"{"level":0,"module":"inner","names":[{"asname":"go","name":"exec"}]}"#
                ),
                (5, 5, "call", "unit", "go()"),
                (6, 6, "call", "unit", "go()"),
                (7, 7, "call", "unit", "go()"),
            ]
        ))
    );
    let diagnostics: Vec<_> = [
        (2, "outer.eval"),
        (3, "outer.eval"),
        (5, "inner.exec"),
        (6, "inner.exec"),
        (6, "inner.exec"),
        (7, "inner.exec"),
    ]
    .into_iter()
    .map(|(line, name)| {
        json!({
            "code":"dynamic_discovery_unresolved", "path":"unit.py",
            "message":format!("line {line}: {name}; runtime targets are not inferred")
        })
    })
    .collect();
    assert_eq!(native["diagnostics"], json!(diagnostics));
    for pattern in [
        "go",
        "go as selected",
        "[go, *selected]",
        "{'key': go, **selected}",
        "Record(go=selected)",
        "go | other",
    ] {
        let text = format!("import importlib as go\nmatch subject:\n    case {pattern} if go.import_module('guard'): go.import_module('body')\ngo.import_module('after')\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_warning_candidates_match_ast(&python, &scan);
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn structure_candidate_pattern_literals_signed_complex_and_safe_spellings() {
    let python = oracle();
    let mut patterns: Vec<String> = [
        "0",
        "0x_FF",
        "0b_10",
        "0o_77",
        "1_000",
        "1.25",
        ".5",
        "1.",
        "1e+2",
        "-1",
        "-2.5",
        "3j",
        "-3J",
        "1+2j",
        "-1-2j",
        "1.5+2.5j",
        "'PRIVATE_TEXT'",
        "\"text\"",
        "'''text'''",
        "r'text'",
        "u'text'",
        "b'PRIVATE_BYTES'",
        "br'bytes'",
        "RB'bytes'",
        "'one' 'two'",
        "r'one' u'two'",
        "'escaped\\n'",
        "r'back\\slash'",
        "b'one' br'two'",
        "{'a' 'b': -1+2j}",
        "{-1-2j: value, constants.KEY: other, None: third}",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    patterns.push("9".repeat(512));
    patterns.push("9".repeat(513));
    for pattern in patterns {
        let text = format!("match subject:\n    case {pattern}: pass\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        assert_eq!(
            native["facts"],
            json!(expected("unit.py", &[(1, 1, "module", "unit", "unit"),])),
            "{pattern}"
        );
        assert!(!native.to_string().contains("PRIVATE_"));
    }
    // Malformed escapes and over-limit nonzero decimals still roll back the file.
    for pattern in [
        "'\\xQ0'".to_owned(),
        "'\\u123'".to_owned(),
        "'\\U00110000'".to_owned(),
        "9".repeat(4301),
    ] {
        let text = format!("eval()\nmatch subject:\n    case {pattern}: exec()\n");
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
    for pattern in ["f'{value}'", "f'plain'"] {
        let text = format!("match subject:\n    case {pattern}: pass\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        // The AST builder permits JoinedStr here; compilation does not. Native
        // formatted-pattern traversal remains a documented G2 subset exclusion.
        assert!(extract_captured(&python, &scan.sources).unwrap().complete);
        assert_ne!(
            extract_fact_candidates(&scan.sources).unwrap().files[0].status,
            CandidateStatus::AcceptedFacts
        );
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn structure_candidate_case_owners_subject_roles_jumps_and_end_spans() {
    let python = oracle();
    let text = concat!(
        "def owner():\n",
        "    match (subject := first()), second(),:\n",
        "        case (capture) if guarded := ready():\n",
        "            class Box[T: Bound](Base[T]):\n",
        "                field: T\n",
        "                def run[U](self, value: U) -> U: return value\n",
        "        case head, *tail:\n",
        "            async def nested[V]():\n",
        "                match await work():\n",
        "                    case _: return (result := finish())\n",
        "    after()\n",
        "outside()\n",
    );
    let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
    assert_declaration_candidates_match_ast(&python, &scan);
    let (_, scan) = capture(&[("unit.py", b"class Box:\n    match subject():\n        case _:\n            def run():\n                return work()\n    after()\n")]);
    let native = assert_declaration_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["facts"],
        json!(expected(
            "unit.py",
            &[
                (1, 1, "module", "unit", "unit"),
                (1, 6, "class", "unit.Box", "Box"),
                (2, 2, "call", "unit.Box", "subject()"),
                (4, 5, "function", "unit.Box.run", "run"),
                (
                    4,
                    5,
                    "signature",
                    "unit.Box.run",
                    r#"{"keywordOnly":[],"positional":[],"positionalOnly":0}"#
                ),
                (5, 5, "call", "unit.Box.run", "work()"),
                (6, 6, "call", "unit.Box", "after()"),
            ]
        ))
    );
    for text in [
        "match written := subject():\n    case _ if other := ready(): pass\n",
        "match (written := subject()):\n    case _ if other := ready(): pass\n",
        "match first(), (written := second()):\n    case _ if (outer := (inner := ready())): pass\n",
        "match first(), second(),:\n    case (head,): pass\n",
        "match *items,:\n    case _: pass\n",
        "match first(), *items:\n    case _: pass\n",
        "match (*items,):\n    case _: pass\n",
        "match (\n    first(),\n    second()\n):\n    case (\n        capture\n    ) if (\n        ready()\n    ): use()\n",
        "while ready():\n    match subject():\n        case 1: continue\n        case _: break\n",
        "def owner():\n    match subject:\n        case _: return work()\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
    // Existing function-only return admission is narrower than ast.parse;
    // matching does not manufacture permission or a definition scope.
    let (_, scan) = capture(&[(
        "unit.py",
        b"eval()\nmatch subject:\n    case _: return work()\n",
    )]);
    assert!(extract_captured(&python, &scan.sources).unwrap().complete);
    let rejected = extract_fact_candidates(&scan.sources).unwrap();
    assert_eq!(rejected.files[0].status, CandidateStatus::UnsupportedSyntax);
    assert_eq!(rejected.facts.len(), 1);
    assert!(rejected.diagnostics.is_empty());
    for subject in ["*items", "*factory()"] {
        let text = format!("eval()\nmatch {subject}:\n    case _: exec()\n");
        let (_, scan) = capture(&[
            ("bad.py", text.as_bytes()),
            ("good.py", b"match *items,:\n    case _: okay()\n"),
        ]);
        let ast = extract_captured(&python, &scan.sources).unwrap();
        assert!(!ast.complete);
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_ne!(native.files[0].status, CandidateStatus::AcceptedFacts);
        assert_eq!(native.files[1].status, CandidateStatus::AcceptedFacts);
        assert_eq!(
            native.facts.iter().filter(|f| f.path == "bad.py").count(),
            1
        );
        assert!(native.diagnostics.is_empty());
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn structure_candidate_pattern_bad_roles_and_ast_not_compiler_constraints() {
    let python = oracle();
    for pattern in [
        "",
        ",",
        "(,)",
        "[,]",
        "{,}",
        "Record(,)",
        "key=value",
        "[key=value]",
        "Record(key=value, positional)",
        "Record(*args)",
        "Record(**args)",
        "[*args as name]",
        "**rest",
        "*rest",
        "(*rest)",
        "[**rest]",
        "{*rest}",
        "{**_}",
        "{**first, **second}",
        "{**rest, 'key': value}",
        "{capture: value}",
        "{[1]: value}",
        "{Record(): value}",
        "{(1): value}",
        "value as _",
        "value as first as second",
        "1+2",
        "1j+2j",
        "+1",
        "--1",
        "1+-2j",
        "'a' b'b'",
    ] {
        let text = format!("eval()\nmatch subject:\n    case {pattern}: exec()\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let ast = extract_captured(&python, &scan.sources).unwrap();
        assert!(
            !ast.complete,
            "AST accepted syntax-negative pattern {pattern}"
        );
        assert_eq!(ast.diagnostics[0].code, "parse_failure", "{pattern}");
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_ne!(
            native.files[0].status,
            CandidateStatus::AcceptedFacts,
            "{pattern}"
        );
        assert_eq!(
            serde_json::to_value(&native.facts).unwrap(),
            json!(expected("unit.py", &[(1, 1, "module", "unit", "unit")]))
        );
        assert!(native.diagnostics.is_empty());
    }
    for text in [
        "match subject:\n",
        "match subject:\n    pass\n",
        "match subject: case _: pass\n",
    ] {
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
    // ast.parse constructs these trees; compile-time capture checks are not parity.
    for pattern in [
        "[same, same]",
        "left | right",
        "_ | 1",
        "Record(key=a, key=b)",
        "[*left, *right]",
        "value as value",
        "{1: a, 1: b}",
    ] {
        let text = format!("match subject:\n    case {pattern}: pass\n    case _: pass\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn structure_candidate_star_future_import_metadata_and_absolute_aliases() {
    let python = oracle();
    for (text, metadata) in [
        (
            "from pkg import *\n",
            r#"{"level":0,"module":"pkg","names":[{"asname":null,"name":"*"}]}"#,
        ),
        (
            "from ..pkg import *\n",
            r#"{"level":2,"module":"pkg","names":[{"asname":null,"name":"*"}]}"#,
        ),
        (
            "from . import *\n",
            r#"{"level":1,"module":"","names":[{"asname":null,"name":"*"}]}"#,
        ),
        (
            "from __future__ import annotations\n",
            r#"{"level":0,"module":"__future__","names":[{"asname":null,"name":"annotations"}]}"#,
        ),
        (
            "from __future__ import (annotations as renamed, division,)\n",
            r#"{"level":0,"module":"__future__","names":[{"asname":"renamed","name":"annotations"},{"asname":null,"name":"division"}]}"#,
        ),
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        let native = assert_declaration_candidates_match_ast(&python, &scan);
        assert_eq!(
            native["facts"],
            json!(expected(
                "unit.py",
                &[
                    (1, 1, "from_import", "unit", metadata),
                    (1, 1, "module", "unit", "unit"),
                ]
            ))
        );
    }
    let (_, scan) = capture(&[(
        "unit.py",
        b"from __future__ import eval as go; from tools import *; go()\n",
    )]);
    let native = assert_warning_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["diagnostics"],
        json!([{
            "code":"dynamic_discovery_unresolved", "path":"unit.py",
            "message":"line 1: __future__.eval; runtime targets are not inferred"
        }])
    );
    for text in [
        "from __future__ import annotations\ndef f(x: A | B) -> T[U]: pass\nclass C:\n    field: A | B\n",
        "from outer import eval as go\nfrom .other import safe as go\ngo()\n",
        "from outer import eval as go\nfrom unknown import *\ngo()\n",
        "pass\nfrom __future__ import unknown_feature as feature\nfeature()\n",
        "def f(): from __future__ import annotations; pass\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_warning_candidates_match_ast(&python, &scan);
    }
    // Constructor imports still hit the semantic guard in newly admitted layouts.
    for form in [
        "from __future__ import FastAPI",
        "def f(): from tools import APIRouter",
        "if ready: import pkg.APIRouter as unused",
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
    }
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn structure_candidate_inline_import_lists_simple_suites_and_semicolons() {
    let python = oracle();
    let text = concat!(
        "def owner(): from tools import ( # opening\n",
        "    eval as go, # source order\n",
        "    exec,\n",
        "); go(); exec();\n",
    );
    let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
    let native = assert_warning_candidates_match_ast(&python, &scan);
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
                    "unit.owner",
                    r#"{"level":0,"module":"tools","names":[{"asname":"go","name":"eval"},{"asname":null,"name":"exec"}]}"#
                ),
                (1, 4, "function", "unit.owner", "owner"),
                (
                    1,
                    4,
                    "signature",
                    "unit.owner",
                    r#"{"keywordOnly":[],"positional":[],"positionalOnly":0}"#
                ),
                (4, 4, "call", "unit.owner", "exec()"),
                (4, 4, "call", "unit.owner", "go()"),
            ]
        ))
    );
    for text in [
        "import tools; from tools import item as named; use(named);\n",
        "if ready(): import tools; type Alias[T] = T; use();\nelse: from tools import *; pass;\n",
        "class Box[T]: import tools; type Alias[U] = U; field: T;\n",
        "match subject():\n    case _: import tools; type Alias[T] = T; use();\n",
        "while ready(): import tools; break;\n",
        "with context() as item: from tools import value; use();\n",
        "try: import tools; use();\nexcept Error: from tools import item; pass;\nfinally: finish();\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert_declaration_candidates_match_ast(&python, &scan);
    }
    // Parenthesized with-item lists remain deferred, not silently admitted here.
    for text in [
        "; pass",
        "pass;; pass",
        "pass; ; pass",
        "pass; if x: pass",
        "pass; def f(): pass",
        "pass; class C: pass",
        "if x: while y: pass",
        "match x:\n    case _: def nested(): pass",
        "import tools,",
        "from tools import item,",
        "from tools import (*,)",
        "from tools import * as alias",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert!(
            !extract_captured(&python, &scan.sources).unwrap().complete,
            "{text}"
        );
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_ne!(
            native.files[0].status,
            CandidateStatus::AcceptedFacts,
            "{text}"
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
fn structure_candidate_observed_lf_crlf_continuations_and_invalid_joins() {
    let python = oracle();
    for newline in ["\n", "\r\n"] {
        for text in [
            "from tools \\\n import eval as go; type Alias[T] = T;\ndef \\\n owner[T](\n):\n    match \\\n subject(), \\\n other():\n        case [x, *xs] \\\n if (\n go() # comment\n ) \\\n : pass; go();\n    return \\\n go()\n",
            "def f():\\\npass; import tools; type Alias = T;\n",
            "class \\\n Box[T](\\\n Base[T],\\\n metaclass=Meta\\\n ): pass\n",
            "value \\\n = \\\n make(\\\n item\\\n ); use(\\\n value\\\n )\n",
            "match subject:\n    case 1 | \\\n        2: use()\n",
            "if \\\n ready(): pass\nelse \\\n : pass\n",
            "type \\\n Alias[T] \\\n = \\\n T\n",
            "eval(),\\\n\nhelper()\n",
            "value = source\\\n\n+other\n",
            "def owner():\n    eval(),\\\n\n    helper()\n",
            "def owner():\n    value = source\\\n\n    +other\n",
        ] {
            let text = text.replace('\n', newline);
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            assert_warning_candidates_match_ast(&python, &scan);
        }
        for form in [
            "from tools import \\\n\n item",
            "from tools import \\\n# swallowed\n item",
            "from tools import \\ \n item",
            "value = \\\n\nother",
            "value = \\\n# swallowed\nother",
            "value = \\ \nother",
            "match x:\n    case _ \\\n\n    : pass",
            "type Alias\n= T",
            "def f\n(): pass",
            "match x\n:\n    case _: pass",
            "match x:\n    case _ if\n    ready(): pass",
        ] {
            let text = format!("eval()\n{form}\n").replace('\n', newline);
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            assert!(
                !extract_captured(&python, &scan.sources).unwrap().complete,
                "{form}"
            );
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
    for bytes in [b"eval()\nvalue = \\\0other\n".as_slice(), b"pass\0\n"] {
        let (_, scan) = capture(&[("unit.py", bytes)]);
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
fn structure_candidate_tabs_token_separators_and_mixed_layout_boundary() {
    let python = oracle();
    for indent in ["    ", "\t"] {
        let text = format!("from\ttools\timport\teval\tas\tgo\ndef\towner[T]():\n{indent}match\tsubject():\n{indent}{indent}case\t[head,\t*tail]\tif\tgo():\n{indent}{indent}{indent}type\tAlias[U]\t=\tU;\tgo();\n{indent}after()\n");
        for newline in ["\n", "\r\n"] {
            let text = text.replace('\n', newline);
            let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
            assert_warning_candidates_match_ast(&python, &scan);
        }
    }
    // Even AST-positive mixed layout is G4 UnsupportedSyntax, not an oracle waiver.
    for text in [
        "if ready:\n\t pass\n",
        "if ready:\n    pass\nelse:\n\tpass\n",
        "def first():\n    pass\ndef second():\n\tpass\n",
    ] {
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert!(
            extract_captured(&python, &scan.sources).unwrap().complete,
            "{text}"
        );
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
        assert!(native.diagnostics.is_empty());
    }
    for text in [
        "if ready:\n\tpass\n    pass\n",
        "if ready:\n  pass\n pass\n",
    ] {
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
fn structure_candidate_hidden_calls_metadata_and_header_guards_cross_match() {
    let python = oracle();
    let (_, scan) = capture(&[("unit.py", b"from tools import eval as go\nmatch sink(lambda: go()):\n    case FastAPI(router=prefix) if [go() for item in values]:\n        class Box[T: Bound](Base[T], metaclass=Meta):\n            field: A | B\n            def run[U](self, item: U = (lambda: value)) -> U:\n                match (written := go()):\n                    case APIRouter(prefix=capture): type Alias[V] = go()\n")]);
    assert_warning_candidates_match_ast(&python, &scan);
    for form in [
        "match subject:\n    case _ if FastAPI(): pass",
        "match subject:\n    case _: sink(lambda: APIRouter())",
        "match subject:\n    case _: router.include_router(child)",
        "match subject:\n    case _: self.router = value",
        "match subject:\n    case _:\n        def f(x=hidden()): pass",
        "match subject:\n    case _:\n        def f(x: T[hidden()]): pass",
        "match subject:\n    case _:\n        def f() -> T[hidden()]: pass",
        "match subject:\n    case _:\n        class Box[T: hidden()]: pass",
        "match subject:\n    case _:\n        class Box(Base[hidden()]): pass",
        "match subject:\n    case _:\n        class Box(metaclass=(lambda: hidden())): pass",
        "match subject:\n    case _:\n        @decorate\n        def f(): pass",
        "match subject:\n    case _:\n        type Alias[T: hidden()] = T",
        "match subject:\n    case [head, # type: int\n          *tail]: pass",
    ] {
        let text = format!("eval()\n{form}\n");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        // Type-comment grammar can fail ast.parse; do not conflate it with a guard.
        let ast = extract_captured(&python, &scan.sources).unwrap();
        if !form.contains("# type:") {
            assert!(ast.complete, "{form}");
        }
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_eq!(
            native.files[0].status,
            CandidateStatus::UnsupportedSyntax,
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
fn structure_candidate_deep_patterns_long_names_and_counted_walk() {
    let python = oracle();
    for depth in [12, 24, 48] {
        let name = format!("Type{}", "Name".repeat(180));
        let pattern = format!(
            "{}pkg.{name}(value=capture){}",
            "[".repeat(depth),
            "]".repeat(depth)
        );
        let text = format!("type Alias[{name}: Bound] = T\nmatch subject():\n    case {pattern} if eval(): use()\n");
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
        assert_eq!(
            native["facts"],
            json!(expected(
                "unit.py",
                &[
                    (1, 1, "module", "unit", "unit"),
                    (2, 2, "call", "unit", "subject()"),
                    (3, 3, "call", "unit", "eval()"),
                    (3, 3, "call", "unit", "use()"),
                ]
            ))
        );
    }
    // Patterns do not render facts, but their descendants still consume depth.
    let pattern = format!("{}capture{}", "[".repeat(140), "]".repeat(140));
    let text = format!("eval()\nmatch subject:\n    case {pattern}: exec()\n");
    let (_, scan) = capture(&[("bad.py", text.as_bytes()), ("good.py", b"visible()\n")]);
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
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn structure_candidate_deleted_replay_order_isolation_and_original_digests() {
    let python = oracle();
    let entries: &[(&str, &[u8])] = &[
        ("a.py", b"\xef\xbb\xbf# retained original bytes\r\nfrom tools import eval as go;\r\nmatch subject():\r\n    case FastAPI(value=capture) if go(): type Alias[T] = go();\r\n"),
        ("b.py", b"def owner[T]():\n\tmatch subject():\n\t\tcase [head, *tail]: from tools import item; use();\n"),
        ("c.py", b"eval()\nmatch subject:\n    case _: FastAPI()\n"),
        ("d.py", b"eval()\nmatch subject:\n    case {**_}: pass\n"),
    ];
    let (root, mut scan) = capture(entries);
    let (_good_root, good_scan) = capture(&entries[..2]);
    let good = assert_warning_candidates_match_ast(&python, &good_scan);
    let ast = extract_captured(&python, &scan.sources).unwrap();
    assert!(!ast.complete);
    assert_eq!(
        ast.diagnostics
            .iter()
            .filter(|d| d.code == "parse_failure")
            .map(|d| d.path.as_str())
            .collect::<Vec<_>>(),
        ["d.py"]
    );
    let native = extract_fact_candidates(&scan.sources).unwrap();
    assert_eq!(native.files[2].status, CandidateStatus::UnsupportedSyntax);
    assert_ne!(native.files[3].status, CandidateStatus::AcceptedFacts);
    for (file, (path, bytes)) in native.files.iter().zip(entries) {
        assert_eq!(file.path, *path);
        assert_eq!(file.sha256, hash(bytes));
    }
    for (file, (_, bytes)) in scan.files.iter().zip(entries) {
        assert_eq!(file.sha256, hash(bytes));
    }
    let first = serde_json::to_value(native).unwrap();
    let mut facts = good["facts"].as_array().unwrap().clone();
    facts.extend(expected("c.py", &[(1, 1, "module", "c", "c")]));
    facts.extend(expected("d.py", &[(1, 1, "module", "d", "d")]));
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
