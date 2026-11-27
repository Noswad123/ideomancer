//! Integrated G4 profile: sources are parsed by the oracle, never executed.
use super::ruff_facts_batch::source;
use super::*;

fn fixtures() -> Vec<Source> {
    let mut cases: Vec<_> = [
        "\u{03bb} = factory()\n\u{03bb}()\n",
        "\u{53d8}\u{91cf} = service\n\u{53d8}\u{91cf}()\n",
        "a\u{301} = factory()\na\u{301}()\n",
        "K = first\n\u{212a} = second\nK()\n\u{212a}()\n",
        "\u{ff49}\u{ff46} = factory()\n\u{ff49}\u{ff46}()\n",
        "import \u{03bb}.child as \u{03b4}\nfrom package import \u{53d8}\u{91cf} as \u{03b2}\n\u{03b4}.call()\n\u{03b2}()\n",
        "def \u{03bb}(\u{03b1}: annotation()=default(), *, \u{03b2}=other()):\n    return \u{03b1}(\u{03b2})\n",
        "class \u{03a9}(Base):\n    \u{03bb}: Type = factory()\n    def \u{03b4}(self): return self.\u{03bb}()\n",
        "def outer():\n    def \u{03bb}(): return child()\n    class \u{03a9}: pass\n    return \u{03bb}()\n",
        "def \u{03bb}[\u{03a4}: bound(), *\u{03a3}, **\u{03a0}](\u{03b1}: \u{03a4}) -> result():\n    body()\n",
        "type \u{0391}[\u{03a4}: bound()] = rhs()\n",
        "match subject():\n    case [\u{03b1}, *\u{03b2}] if guard(): body()\n    case Record(\u{03b3}, key=\u{03b4}): other()\n",
        "target(\u{212a}=first(), K=second())\n",
        "from early import eval as \u{212a}\ndef f(a=K()) -> \u{212a}():\n    from late import exec as K\n    \u{212a}()\n",
        "eval()\nmatch value:\n    case [\u{212a}]: pass\n",
        "eval()\ntype Alias[\u{212a}] = int\n",
        "value = '\\N{LATIN CAPITAL LETTER A}'\nconsume(value)\n",
        "value = '\\N{latin capital letter a}'\nconsume(value)\n",
        "@api.get('/\\N{NULL}/\\N{NUL}/\\N{SPACE}')\ndef f(): pass\n",
        "@api.get('/\\N{CJK UNIFIED IDEOGRAPH-4E00}/\\N{HANGUL SYLLABLE GA}')\ndef f(): pass\n",
        "@api.get('/\\ud800' '\\N{LATIN CAPITAL LETTER A}' '\\udfff')\ndef f(): pass\n",
        "@api.route('/ok', methods=['\\N{LATIN SMALL LETTER G}et', 'po\\N{LATIN SMALL LETTER LONG S}t'])\ndef f(): pass\n",
        "r = APIRouter(prefix='/\\N{LATIN SMALL LETTER A}')\napp.include_router(r, prefix='/\\N{LATIN SMALL LETTER B}')\n",
        "value = f'\\N{LATIN CAPITAL LETTER A}{call()}'\n",
        "value = rf'{\"\\N{LATIN CAPITAL LETTER A}\"}'\n",
        "value = f'{{literal}}\\N{SPACE}{call():{width()}}'\n",
        "target(r'\\N{NOT A NAME}', b'\\N{NOT A NAME}', '\\\\N{NOT A NAME}')\n",
        "target(rf'\\N{{NOT A NAME}}{call()}')\n",
        "value = factory() # type: REDACTION_PAYLOAD_71\n",
        "a = b = factory() # type: Any\na, b = other() # type: Pair\n",
        "value = factory() \\\n # type: Any\n",
        "class C:\n    value = factory() \\\n # type: Any\n",
        "@task\ndef f():\n    value = factory() \\\n # type: Any\n",
        "@task\nasync def f():\n    value = factory() \\\n # type: Any\n",
        "def f(a, # type: A\n b=default(), # type: B\n *args, # type: C\n c=other(), # type: D\n **kw # type: E\n):\n    body()\n",
        "def f(a: (\n Type\n),\n # type: Any\n b=default()): pass\n",
        "def f(): # type: () -> Any\n    body()\n",
        "def f():\n    # ordinary\n    # type: () -> Any\n    body()\n",
        "async def f(): # type: () -> Any\n    await body()\n",
        "for value in iterable(): # type: Any\n    body()\n",
        "async for value in iterable(): # type: Any\n    body()\n",
        "with context() as value: # type: Any\n    body()\n",
        "async with context() as value: # type: Any\n    body()\n",
        "# type: ignore[code]\npass # type: ignore!\ntype Alias = int # type: ignore\n",
        "value = f'{call() # type: ignore\n}'\n",
        "if ready():\n    value = factory() \\\n # type: Any\n",
        "try:\n    value = factory() # type: A\nexcept Error:\n    value = other() # type: B\nfinally:\n    value = final() # type: C\n",
        "# coding: utf-8\n@api.get('/\\N{LATIN SMALL LETTER A}')\ndef \u{03bb}(a, # type: Any\n): # type: (Any) -> Any\n    r = APIRouter(prefix='/v1') # type: Any\n    app.include_router(r)\n",
        "#!/usr/bin/python\r\n# coding: utf_8\r\ndef \u{03bb}(): pass\r\n",
        "\u{feff}# coding: utf-8\r\nvalue = factory() # type: Any\r\n",
        "value = factory()\n# coding: unknown_codec\n",
        "match subject():\n    case {f'{key()}': capture}: body()\n",
        "match subject():\n    case {f'{key1()}': f'{value1()}', f'{key2()}': capture}: body()\n",
        "target(lambda \u{03b1}=default(), **\u{03b2}: \u{03b1}(\u{03b2}))\nvalue = [\u{03b1}() for \u{03b1} in iterable() if guard()]\n",
        "def outer():\n    \u{03b1} = factory()\n    def inner():\n        nonlocal \u{03b1}\n        global \u{03b2}\n        try: \u{03b1}()\n        except Error as \u{03b3}: handler(\u{03b3})\n",
        "async def \u{03bb}(\u{03b1}=default()):\n    await receiver.\u{03b4}(\u{03b1}=\u{03b1})\n",
        "\u{11f02} = factory()\n\u{11f02}()\n",
    ].into_iter().map(source).collect();
    cases.push(source(b"# coding: latin-1\n\xe9 = factory()\n\xe9()\n"));
    cases.push(source(b"# coding: iso-8859-1\r\n@api.get('/caf\xe9')\r\ndef f():\r\n    r = APIRouter(prefix='/a') # type: Any\r\n"));
    cases.push(source(
        b"# coding: latin_1\nclass Caf\xe9:\n    value = factory() \\\n # type: Any\n",
    ));
    // The helper bounds normalized target names in Python characters, not bytes.
    for character in ['\u{03bb}', '\u{10400}'] {
        for count in [64, 65, 128, 129] {
            let name = character.to_string().repeat(count);
            cases.push(source(format!(
                "{name} = FastAPI()\n{name} = APIRouter(prefix='/v1')\n"
            )));
        }
    }
    for count in [42, 43] {
        let name = "\u{fb03}".repeat(count); // NFKC expands to 126 or 129 ASCII points.
        cases.push(source(format!(
            "{name} = FastAPI()\n{name} = APIRouter(prefix='/v1')\n"
        )));
    }
    let mut latin = b"# coding: latin-1\n".to_vec();
    latin.extend(std::iter::repeat_n(0xe9, 65));
    latin.extend_from_slice(b" = FastAPI()\n");
    cases.push(source(latin));
    cases
}

#[test]
fn integrated_g4_candidates_are_admitted_without_payload_leaks() {
    let cases = fixtures();
    assert_eq!(cases.len(), 71);
    for (index, input) in cases.iter().enumerate() {
        let report = extract_fact_candidates(std::slice::from_ref(input)).unwrap();
        assert_eq!(
            report.files[0].status,
            FactStatus::AcceptedFacts,
            "G4 {index}: {:?}",
            report.files[0]
        );
        assert_eq!(report.files[0].failure_line, None);
        assert_eq!(report.files[0].sha256, crate::analysis::hash(&input.bytes));
        assert_eq!(report.identity.policy, "python-ruff-private-facts-4");
        assert_eq!(report.cpython_compatibility, CompatibilityStatus::Pending);
        assert!(!serde_json::to_string(&report)
            .unwrap()
            .contains("REDACTION_PAYLOAD_71"));
    }
}

fn invalid_cases() -> &'static [(&'static str, usize)] {
    &[
        ("eval()\n\u{1c89} = factory()\n", 2),
        ("eval()\ndef \u{1c89}(): pass\n", 2),
        ("eval()\npass # type: Any\n", 2),
        ("eval()\n# type: Any\nvalue=1\n", 2),
        ("eval()\nvalue: Type = rhs() # type: Any\n", 2),
        ("eval()\ntarget(1, # type: Any\n)\n", 2),
        ("eval()\nclass C: # type: Any\n    pass\n", 2),
        (
            "eval()\ndef f(): # type: Any\n    # type: Other\n    pass\n",
            4,
        ),
        ("eval()\nvalue = '\\N{NOT A UNICODE NAME}'\n", 2),
        ("eval()\nvalue = '\\N{CUNEIFORM SIGN KALAM}'\n", 2),
        ("eval()\nvalue = '\\N{cjk unified ideograph-4E00}'\n", 2),
        ("eval()\nvalue = '\\N{HANGUL SYLLABLE ga}'\n", 2),
        ("eval()\nvalue = '\\N{KEYCAP DIGIT ONE}'\n", 2),
        ("eval()\nimport \u{1c89}\n", 2),
        ("eval()\nreceiver.\u{1c89}()\n", 2),
        ("eval()\ntarget(\u{1c89}=factory())\n", 2),
        // Ordinary literals report their own opening token, including adjacent
        // literals and escaped physical newlines. The alias is valid in Ruff's
        // newer Unicode data, so these cases exercise the compatibility guard.
        ("eval()\nvalue = '''first\n\\N{CUNEIFORM SIGN KALAM}\nlast'''\n", 2),
        ("eval()\nvalue = (\n \"first\"\n '''second\n\\N{CUNEIFORM SIGN KALAM}\nlast'''\n)\n", 4),
        ("eval()\nvalue = (\n '''first\n\\N{CUNEIFORM SIGN KALAM}\nlast'''\n \"second\"\n)\n", 3),
        ("eval()\nvalue = 'first\\\n\\N{CUNEIFORM SIGN KALAM}'\n", 2),
        // F-string text errors report the matching closing token, not the
        // escape's line or the end of an adjacent literal/outer f-string.
        ("eval()\nvalue = f'''first\n\\N{CUNEIFORM SIGN KALAM}\nlast'''\n", 4),
        ("eval()\nvalue = f'''first\n\\N{CUNEIFORM SIGN KALAM}\n{call()}\nend'''\n", 5),
        ("eval()\nvalue = f'''{call()}\n\\N{CUNEIFORM SIGN KALAM}\nlast'''\n", 4),
        ("eval()\nvalue = f'''first\n\\N{CUNEIFORM SIGN KALAM}{{literal}}\nlast'''\n", 4),
        ("eval()\nvalue = (f'''first\n\\N{CUNEIFORM SIGN KALAM}\nlast'''\n \"second\")\n", 4),
        ("eval()\nvalue = (\"first\"\n f'''second\n\\N{CUNEIFORM SIGN KALAM}\nlast''')\n", 5),
        ("eval()\nvalue = f\"\"\"{(\n '''first\n\\N{CUNEIFORM SIGN KALAM}\nlast'''\n)}\"\"\"\n", 3),
        ("eval()\nvalue = f\"\"\"{f'''first\n\\N{CUNEIFORM SIGN KALAM}\nlast'''}\nouter\"\"\"\n", 4),
        // Inner reductions fail before an already-invalid outer text part.
        ("eval()\nvalue = f\"\"\"\\N{CUNEIFORM SIGN KALAM}\n{f'''first\n\\N{CUNEIFORM SIGN KALAM}\nlast'''}\nouter\"\"\"\n", 5),
        ("eval()\nvalue = (f'''first\n\\N{CUNEIFORM SIGN KALAM}\nlast'''\n f\"\"\"second\n\\N{CUNEIFORM SIGN KALAM}\nend\"\"\")\n", 4),
        ("eval()\nvalue = f\"\"\"\\N{CUNEIFORM SIGN KALAM}\n{(\n '''first\n\\N{CUNEIFORM SIGN KALAM}\nlast'''\n)}\"\"\"\n", 4),
        ("eval()\r\nvalue = '''first\r\n\\N{CUNEIFORM SIGN KALAM}\r\nlast'''\r\n", 2),
    ]
}

#[test]
fn integrated_invalid_compatibility_has_no_semantic_callbacks_or_records() {
    for &(text, line) in invalid_cases() {
        let inputs = [source(text)];
        let mut trace = Vec::new();
        let report = extract_impl(
            &inputs,
            |_| false,
            SemanticLimits::default(),
            Some(&mut trace),
        )
        .unwrap();
        let file = &report.files[0];
        assert_eq!(file.status, FactStatus::SyntaxError, "{text}: {file:?}");
        assert_eq!(file.failure_line, Some(line), "{text}");
        if text.contains("CUNEIFORM SIGN KALAM") {
            assert_eq!(file.reason, Some("invalid Unicode 15 named escape"));
        }
        assert_eq!(file.nodes, 0, "semantic lowering must not begin");
        assert_eq!(report.facts.len(), 1);
        assert!(trace.is_empty() && report.diagnostics.is_empty());
    }
}

// Native physical decoder lines and CPython PEG lines/admission deliberately
// differ here. These are Pending G4 exclusions, never matching-oracle cases.
fn utf8_known_differences() -> &'static [(&'static [u8], usize, Option<usize>)] {
    &[
        (b"eval()\nvalue='''\n\xff\n'''\n", 3, Some(2)),
        (
            b"# coding: utf-8\neval()\nvalue='''\n\xff\n'''\n",
            4,
            Some(3),
        ),
        (b"# invalid comment \xff\neval()\n", 1, None),
        (
            b"# coding: utf-8\n# invalid comment \xff\neval()\n",
            2,
            None,
        ),
    ]
}

#[test]
fn integrated_strict_utf8_physical_failures_do_not_lower() {
    for &(bytes, physical, _) in utf8_known_differences() {
        let inputs = [source(bytes)];
        let mut trace = Vec::new();
        let report = extract_impl(
            &inputs,
            |_| false,
            SemanticLimits::default(),
            Some(&mut trace),
        )
        .unwrap();
        assert_eq!(report.cpython_compatibility, CompatibilityStatus::Pending);
        assert_eq!(report.files[0].status, FactStatus::SyntaxError);
        assert_eq!(report.files[0].reason, Some(source::INVALID_UTF8));
        assert_eq!(report.files[0].failure_line, Some(physical));
        assert_eq!(report.files[0].nodes, 0);
        assert_eq!(report.facts.len(), 1);
        assert!(report.diagnostics.is_empty() && trace.is_empty());

        let mut consumes = 0;
        let parsed = run_consumer(&inputs, Limits::default(), |_| false, &mut |_, _, _, _| {
            consumes += 1;
        })
        .unwrap();
        assert_eq!(consumes, 0);
        assert_eq!(parsed.files[0].syntax, RuffSyntax::NotParsed);
    }
}

#[test]
fn integrated_decoder_taxonomy_does_not_invent_codec_syntax_errors() {
    for (bytes, status, line, reason) in [
        (
            b"# coding: unknown_codec\ncall()\n".as_slice(),
            FactStatus::UnsupportedEncoding,
            None,
            "unresolved codec membership",
        ),
        (
            b"# coding: cp1252\ncall()\n".as_slice(),
            FactStatus::UnsupportedEncoding,
            None,
            "unsupported codec",
        ),
        (
            b"\xef\xbb\xbf# coding: latin-1\ncall()\n".as_slice(),
            FactStatus::SyntaxError,
            Some(0),
            source::BOM_CONFLICT,
        ),
        (
            b"call()\nvalue='\xff'\n".as_slice(),
            FactStatus::SyntaxError,
            Some(2),
            source::INVALID_UTF8,
        ),
    ] {
        let inputs = [source(bytes)];
        let report = extract_fact_candidates(&inputs).unwrap();
        assert_eq!(report.files[0].status, status);
        assert_eq!(report.files[0].failure_line, line);
        assert_eq!(report.files[0].reason, Some(reason));
        assert_eq!(report.facts.len(), 1);
        assert!(report.diagnostics.is_empty());
    }
}

#[test]
fn integrated_type_comment_fstring_exclusion_is_finite_and_transactional() {
    for text in [
        "eval()\nvalue = f'{call() # type: Any\n}'\n",
        "eval()\nvalue = f'''{(\ncall() # type: Any\n)}'''\n",
        "eval()\nvalue = f'''{[\ncall() # type: Any\n]}'''\n",
    ] {
        let inputs = [source(text)];
        let mut trace = Vec::new();
        let report = extract_impl(
            &inputs,
            |_| false,
            SemanticLimits::default(),
            Some(&mut trace),
        )
        .unwrap();
        assert_eq!(report.files[0].status, FactStatus::UnsupportedSyntax);
        assert_eq!(
            report.files[0].reason,
            Some("actual type comment in f-string replacement field")
        );
        assert_eq!(report.files[0].failure_line, None);
        assert_eq!(report.files[0].nodes, 0);
        assert_eq!(report.facts.len(), 1);
        assert!(report.diagnostics.is_empty() && trace.is_empty());
    }
}

#[test]
fn integrated_compatibility_bounds_and_cancellation_roll_back() {
    let inputs = [source("@api.get('/\\N{SPACE}')\ndef \u{03bb}(a, # type: Any\n):\n    value = factory() # type: Any\n")];
    // A limit below the baseline container's storage cannot admit a report at
    // all. Per-file rollback instead needs enough room for that baseline first.
    assert!(matches!(
        extract_impl(
            &inputs,
            |_| false,
            SemanticLimits {
                working: 64,
                ..SemanticLimits::default()
            },
            None,
        ),
        Err(FrontEndError::ReportLimit)
    ));
    for limits in [
        SemanticLimits {
            nodes: 1,
            ..SemanticLimits::default()
        },
        SemanticLimits {
            depth: 1,
            ..SemanticLimits::default()
        },
        SemanticLimits {
            working: 4 * std::mem::size_of::<shared::Fact>() + 64,
            ..SemanticLimits::default()
        },
    ] {
        let mut trace = Vec::new();
        let report = extract_impl(&inputs, |_| false, limits, Some(&mut trace)).unwrap();
        assert_eq!(report.files[0].status, FactStatus::ResourceLimit);
        assert_eq!(report.facts.len(), 1);
        assert!(trace.is_empty() && report.diagnostics.is_empty());
    }
    for point in [
        Checkpoint::Decode,
        Checkpoint::Semantic,
        Checkpoint::PostConsume,
        Checkpoint::PostDrop,
    ] {
        for after in [0, 1, 8, 32] {
            let mut visits = 0;
            let mut fired = false;
            let mut trace = Vec::new();
            let report = extract_impl(
                &inputs,
                |at| {
                    if at == point {
                        visits += 1;
                        if visits > after {
                            fired = true;
                            return true;
                        }
                    }
                    false
                },
                SemanticLimits::default(),
                Some(&mut trace),
            )
            .unwrap();
            if fired {
                assert_eq!(report.files[0].status, FactStatus::ResourceLimit);
                assert_eq!(report.facts.len(), 1);
                assert!(trace.is_empty() && report.diagnostics.is_empty());
            }
        }
    }
}

#[test]
fn integrated_decoded_budget_precedes_the_only_parse() {
    let input = source(b"# coding: latin-1\nvalue='\xe9'\n");
    let mut file = file_report(&input);
    let mut total = 32 * 1024 * 1024 - input.bytes.len();
    let before = total;
    let mut cancel = Cancellation {
        expired: false,
        check: |_| false,
    };
    let mut consumes = 0;
    classify(
        &input.bytes,
        &mut file,
        Limits::default(),
        &mut total,
        &mut cancel,
        &mut |_, _, _| consumes += 1,
    );
    assert_eq!(file.admission, Admission::ResourceLimit);
    assert_eq!(file.diagnostic, Some(Diagnostic::DecodedLimit));
    assert_eq!(file.syntax, RuffSyntax::NotParsed);
    assert_eq!(total, before, "a rejected reservation does not charge");
    assert_eq!(consumes, 0);

    let mut bytes = b"# coding: latin-1\n# ".to_vec();
    bytes.extend(std::iter::repeat_n(0xe9, 2048));
    bytes.extend_from_slice(b"\nvalue=factory() # type: Any\n");
    let mut inputs = [source("eval()\n"), source(bytes)];
    inputs[1].path = "second.py".into();
    for after in [1, 4, 8, 16] {
        let mut checks = 0;
        let mut trace = Vec::new();
        let report = extract_impl(
            &inputs,
            |at| {
                if at == Checkpoint::Decode {
                    checks += 1;
                }
                at == Checkpoint::Decode && checks == after
            },
            SemanticLimits::default(),
            Some(&mut trace),
        )
        .unwrap();
        assert!(checks >= after);
        assert!(report
            .files
            .iter()
            .all(|file| file.status == FactStatus::ResourceLimit));
        assert_eq!(report.facts.len(), 2);
        assert!(trace.is_empty() && report.diagnostics.is_empty());
    }
}

#[cfg(unix)]
#[test]
#[ignore = "requires explicit trusted CPython 3.12 / Unicode 15; integrated G4 complete facts and raw order"]
fn integrated_g4_complete_arrays_and_raw_trace_match_trusted_ast_oracle() {
    super::ruff_facts_batch::assert_oracle_sources(fixtures());
}

#[cfg(unix)]
#[test]
#[ignore = "requires explicit trusted CPython 3.12 / Unicode 15; integrated negative classifications and lines"]
fn integrated_g4_negative_lines_match_trusted_ast_oracle() {
    let inputs: Vec<_> = invalid_cases()
        .iter()
        .map(|(text, _)| source(text))
        .collect();
    let lines = oracle_failure_lines(&inputs);
    for ((input, expected), line) in inputs.iter().zip(invalid_cases()).zip(lines) {
        assert_eq!(line, Some(expected.1));
        let native = extract_fact_candidates(std::slice::from_ref(input)).unwrap();
        assert_eq!(native.files[0].status, FactStatus::SyntaxError);
        assert_eq!(native.files[0].failure_line, line);
    }
}

#[cfg(unix)]
#[test]
#[ignore = "requires explicit trusted CPython 3.12 / Unicode 15; known Pending G4 decoder differences"]
fn integrated_strict_utf8_known_differences_remain_pending_g4() {
    let inputs: Vec<_> = utf8_known_differences()
        .iter()
        .map(|(bytes, _, _)| source(bytes))
        .collect();
    let lines = oracle_failure_lines(&inputs);
    for ((input, &(_, physical, expected)), line) in
        inputs.iter().zip(utf8_known_differences()).zip(lines)
    {
        assert_eq!(line, expected);
        assert_ne!(
            Some(physical),
            line,
            "known difference must remain explicit"
        );
        let native = extract_fact_candidates(std::slice::from_ref(input)).unwrap();
        assert_eq!(native.cpython_compatibility, CompatibilityStatus::Pending);
        assert_eq!(native.files[0].status, FactStatus::SyntaxError);
        assert_eq!(native.files[0].failure_line, Some(physical));
        assert_eq!(native.facts.len(), 1);
        assert!(native.diagnostics.is_empty());
    }
}

#[cfg(unix)]
fn oracle_failure_lines(inputs: &[Source]) -> Vec<Option<usize>> {
    let python = crate::analysis::python_parity::oracle();
    let script = r#"
import ast, json, sys, unicodedata
assert sys.version_info[:2] == (3, 12)
assert unicodedata.unidata_version == '15.0.0'
lines = []
for text in json.load(sys.stdin):
    try:
        ast.parse(bytes.fromhex(text), type_comments=True)
    except SyntaxError as error:
        lines.append(error.lineno)
    else:
        lines.append(None)
json.dump(lines, sys.stdout)
"#;
    let mut command = std::process::Command::new(python);
    command
        .args(["-I", "-S", "-c", script])
        .current_dir("/")
        .env_clear()
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let output = crate::analysis::run_bounded(
        command,
        serde_json::to_vec(&inputs.iter().map(|input| &input.hex).collect::<Vec<_>>()).unwrap(),
        "Python Ruff compatibility negative oracle",
    )
    .unwrap();
    let lines: Vec<Option<usize>> = serde_json::from_slice(&output).unwrap();
    assert_eq!(lines.len(), inputs.len());
    lines
}
