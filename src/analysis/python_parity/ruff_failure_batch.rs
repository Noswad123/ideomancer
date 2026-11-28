//! Failure certificates are private projections, not emitted helper diagnostics.
use super::ruff_facts_batch::source;
use super::*;

#[test]
fn contextual_locations_do_not_fabricate_exception_certificates() {
    let inputs = [source("pass # type: Any\n")];
    let report = extract_fact_candidates(&inputs).unwrap();
    let file = &report.files[0];
    assert_eq!(file.status, FactStatus::SyntaxError);
    assert_eq!(file.failure_line, Some(1));
    assert_eq!(file.reason, Some("type comment placement"));
    assert_eq!(file.failure, None);
    assert_eq!(file.nodes, 0);
    assert_eq!(report.facts.len(), 1);
    assert!(report.diagnostics.is_empty());
}
use failures::{Exception, Failure};
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug)]
enum Outcome {
    Certified,
    Ambiguous,
    ParserOnly,
}

struct Case {
    input: Source,
    expected: Failure,
    outcome: Outcome,
}

fn rejected() -> Vec<Case> {
    use Exception::{SyntaxError as S, UnicodeDecodeError as U};
    use Outcome::{Ambiguous, Certified, ParserOnly};
    let mut cases = Vec::new();
    // A valid call before every failure must never reach semantic lowering.
    let mut add = |text: String, exception, line, outcome| {
        cases.push(Case {
            input: source(format!("eval()\n{text}\n")),
            expected: Failure {
                exception,
                line: if line == 0 { 0 } else { line + 1 },
            },
            outcome,
        });
    };
    for escape in [
        "\\xzz",
        "\\uZZZZ",
        "\\U00110000",
        "\\N{NO_NAME}",
        "\\N{CUNEIFORM SIGN KALAM}",
    ] {
        for (text, exception, line) in [
            (format!("value = '{escape}'"), S, 1),
            (format!("value = '''first\n{escape}\nlast'''"), S, 1),
            (format!("value = ('first'\n '{escape}')"), S, 2),
            (format!("value = f'''{escape}\n{{call()}}\nlast'''"), S, 3),
            (format!("value = f'''{{call():\n{escape}}}\nlast'''"), U, 0),
            (
                format!("value = f'''{escape}\n{{'''\\xzz'''}}\nlast'''"),
                S,
                2,
            ),
        ] {
            add(text, exception, line, Certified);
        }
    }
    for digits in ["1".repeat(4301), "1_".repeat(4300) + "1"] {
        for (text, line) in decimal_positions(&digits) {
            add(text, S, line, Certified);
        }
    }
    for (text, exception, line) in [
        ("value = b'''first\n\\xzz\nlast'''", S, 1),
        ("value = b'\u{e9}'", S, 1),
        ("value = rb'\u{e9}'", S, 1),
        ("value = rf'{call():\\xzz}'", U, 0),
        ("value = f'{a:{b:{c:{width()}}}}'", S, 1),
        ("value = f'''{a:\n{b:\n{c:\n{width()}}}}'''", S, 4),
        ("\u{1c89} = factory()", S, 1),
        ("def \u{1c89}(): body()", S, 1),
        ("value = f'''first\n{\u{1c89}}\nlast'''", S, 2),
    ] {
        add(text.into(), exception, line, Certified);
    }
    for (text, exception, line) in [
        ("'\\xzz'\n\u{1c89}=1", S, 2),
        ("\u{1c89}=1\n'\\xzz'", S, 1),
        ("f'''\\xzz\n{a:{b:{c:{d}}}}\n'''", S, 2),
        ("f'''{a:\\xzz\n{b:{c:{d}}}}'''", U, 0),
        ("# type: A\n'\\xzz'", S, 1),
        ("'\\xzz'\n# type: A", S, 1),
    ] {
        add(text.into(), exception, line, Ambiguous);
    }
    for text in [
        "def f(:\n'\\xzz'",
        "value = )\n'\\xzz'",
        "type Alias[T = int] = int\n'\\xzz'",
        "'\\xzz'\ndef f(:",
        "f'{value + :\\xzz}'",
    ] {
        add(text.into(), S, 1, ParserOnly);
    }
    for bytes in [
        b"# coding: latin-1\nvalue = '\xe9\\xzz'\n".as_slice(),
        b"\xef\xbb\xbfeval()\r\nvalue = '\\xzz'\r\n".as_slice(),
    ] {
        cases.push(Case {
            input: source(bytes),
            expected: Failure {
                exception: S,
                line: 2,
            },
            outcome: Certified,
        });
    }
    assert_eq!(cases.len(), 64);
    cases
}

fn decimal_positions(digits: &str) -> Vec<(String, usize)> {
    vec![
        (format!("value = {digits}"), 1),
        (format!("value = -{digits}"), 1),
        (format!("def f(arg=+{digits}): body()"), 1),
        (format!("match subject():\n    case -{digits}: body()"), 2),
        (format!("value = f'{{{digits}}}'"), 1),
        (format!("value = f'{{call():{{{digits}}}}}'"), 1),
    ]
}

fn accepted() -> Vec<Source> {
    let mut cases = Vec::new();
    for count in [4299, 4300] {
        cases.extend(
            decimal_positions(&"1".repeat(count))
                .into_iter()
                .map(|(s, _)| source(s)),
        );
        cases.push(source(format!("value = {}1\n", "1_".repeat(count - 1))));
    }
    for number in [
        "0".repeat(4301),
        format!("0x{}", "f".repeat(4301)),
        format!("0o{}", "7".repeat(4301)),
        format!("0b{}", "1".repeat(4301)),
        format!("{}.0", "1".repeat(4301)),
        format!("{}j", "1".repeat(4301)),
    ] {
        cases.push(source(format!("consume({number})\n")));
    }
    cases.extend([
        "consume(r'\\xzz', rb'\\xzz', b'\\uZZZZ\\U00110000\\N{NO_NAME}')\n",
        "consume(r'\\N{NO_NAME}', '\\\\N{NO_NAME}', '\\\\xzz')\n",
        "consume('\\ud800\\udfff\\U0010ffff', '\\N{LATIN CAPITAL LETTER A}')\n",
        "consume(rf'\\xzz{call()}', rf'\\N{NO_NAME}{call()}')\n",
        "consume(f'{call():\\N{LATIN CAPITAL LETTER A}}')\n",
        "consume(rf'{call():\\\\xzz}')\n",
        "consume(f'{a:{b:{c}}}')\n",
        "consume(f'{a:{b:{c:}}}')\n",
        "consume(f'{a:{b:{c!r}}}')\n",
        "consume(f'{a:{f\"{b:{c:{d}}}\"}}')\n",
        "consume(f'{(lambda a: a)({1: 2}):{width()}}')\n",
        "consume(f'{a:{{1:2}}}')\n",
        "value = f'{call() # type: ignore\n}'\n",
        "value = factory() # type: Any\n",
        "def f(a=(lambda: default())(), # type: Any\n b=other()): body()\n",
        // AST-only parsing deliberately does not enforce compiler legality.
        "return result()\nbreak\ncontinue\n",
        "await top()\nyield value()\nyield from values()\n",
        "def f(a=first(), a=second()):\n    nonlocal missing\n    body()\n",
        "target(key=first(), key=second())\n",
        "class C(key=first(), key=second()): body()\n",
        "match subject():\n    case [same, same, *a, *b] if guard(): body()\n",
        "match subject():\n    case Record(key=a, key=b): body()\n    case {1: a, 1: b}: other()\n",
        "type Alias[T: bound(), *Ts, **P] = f'{rhs():{width():{precision()}}}'\n",
        "@api.route(path=unknown, methods=unknown)\nasync def f[T: bound()](a=default()):\n    async with context() as c:\n        return [await item() async for x in stream() if guard()]\n",
    ].into_iter().map(source));
    assert_eq!(cases.len(), 44);
    cases
}

#[test]
fn failure_certificates_precede_callbacks_and_semantic_lowering() {
    for (index, case) in rejected().iter().enumerate() {
        let inputs = std::slice::from_ref(&case.input);
        let mut callbacks = 0;
        let parsed = run_consumer(inputs, Limits::default(), |_| false, &mut |_, _, _, _| {
            callbacks += 1;
        })
        .unwrap();
        assert_eq!(callbacks, 0, "case {index}");
        let root = &parsed.files[0];
        let mut trace = Vec::new();
        let report = extract_impl(
            inputs,
            |_| false,
            SemanticLimits::default(),
            Some(&mut trace),
        )
        .unwrap();
        let file = &report.files[0];
        assert_eq!(file.sha256, crate::analysis::hash(&case.input.bytes));
        assert_eq!(root.sha256, file.sha256);
        assert_eq!((file.nodes, file.edges), (0, 0), "case {index}");
        assert_eq!(report.facts.len(), 1);
        assert_eq!(report.facts[0].kind, "module");
        assert!(trace.is_empty() && report.diagnostics.is_empty());
        assert_eq!(report.identity.policy, "python-ruff-private-facts-7");
        assert_eq!(report.cpython_compatibility, CompatibilityStatus::Pending);
        match case.outcome {
            Outcome::Certified => {
                assert_eq!(root.admission, Admission::InvalidSource, "case {index}");
                assert_eq!(root.diagnostic, Some(Diagnostic::CompatibilityFailure));
                assert_eq!(root.failure, Some(case.expected), "case {index}");
                assert_eq!(file.failure, root.failure);
                assert_eq!(file.failure_line, Some(case.expected.line));
                assert_eq!(
                    file.status,
                    if case.expected.exception == Exception::UnicodeDecodeError {
                        FactStatus::ParseFailure
                    } else {
                        FactStatus::SyntaxError
                    }
                );
                assert_eq!(
                    case.expected.message(),
                    format!(
                        "{:?} at line {}; source not analyzed",
                        case.expected.exception, case.expected.line
                    )
                );
            }
            Outcome::Ambiguous => {
                assert_eq!(
                    root.admission,
                    Admission::UnsupportedCompatibility,
                    "case {index}"
                );
                assert_eq!(root.diagnostic, Some(Diagnostic::CompatibilityFailure));
                assert_eq!(file.status, FactStatus::UnsupportedSyntax);
                assert_eq!(file.failure_line, None);
                assert_eq!(file.reason, Some("competing failure precedence"));
                assert_eq!(root.failure, None);
                assert_eq!(file.failure, None);
            }
            Outcome::ParserOnly => {
                assert!(root.syntax_errors > 0 || root.unsupported_version_errors > 0);
                assert_ne!(root.syntax, RuffSyntax::AcceptedByRuff);
                assert_ne!(file.status, FactStatus::AcceptedFacts);
                assert_eq!(root.failure, None);
                assert_eq!(file.failure, None);
            }
        }
    }
}

#[test]
fn failure_controls_are_admitted_and_hash_original_encoded_bytes() {
    let mut inputs = accepted();
    inputs.push(source(
        b"# coding: latin-1\nvalue = '\xe9'\nconsume(value)\n",
    ));
    inputs.push(source("\u{feff}consume('\\ud800')\r\n"));
    for input in &inputs {
        let mut callbacks = 0;
        let parsed = run_consumer(
            std::slice::from_ref(input),
            Limits::default(),
            |_| false,
            &mut |_, _, _, _| callbacks += 1,
        )
        .unwrap();
        assert_eq!(callbacks, 1, "{:?}", parsed.files[0]);
        let report = extract_fact_candidates(std::slice::from_ref(input)).unwrap();
        assert_eq!(
            report.files[0].status,
            FactStatus::AcceptedFacts,
            "{:?}",
            report.files[0]
        );
        assert_eq!(report.files[0].failure, None);
        assert_eq!(report.files[0].failure_line, None);
        assert_eq!(report.files[0].sha256, crate::analysis::hash(&input.bytes));
        assert_eq!(parsed.files[0].sha256, report.files[0].sha256);
    }
}

#[test]
fn rejected_siblings_preserve_complete_accepted_records_in_both_orders() {
    let mut good = source("@api.route(path=unknown, methods=unknown)\ndef f():\n    eval()\n    app.include_router()\n");
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
    assert!(!expected.diagnostics.is_empty() && !expected_trace.is_empty());
    for case in rejected() {
        for reverse in [false, true] {
            let mut inputs = vec![source(&good.bytes), source(&case.input.bytes)];
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
fn compatibility_cancellation_invalidates_all_files_and_discards_prior_records() {
    let mut inputs = [
        source("@api.get(path=unknown)\ndef f(): eval()\n"),
        source("value = f'{call():\\xzz}'\n"),
    ];
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
                total += usize::from(at == Checkpoint::Compatibility);
                false
            },
            SemanticLimits::default(),
            None,
        )
        .unwrap();
        assert!(total > 2);
        // Sweep the entire scan, including cancellation after a sibling emitted.
        for stop in 1..=total {
            let mut visits = 0;
            let mut trace = Vec::new();
            let report = extract_impl(
                &inputs,
                |at| {
                    if at == Checkpoint::Compatibility {
                        visits += 1;
                    }
                    at == Checkpoint::Compatibility && visits == stop
                },
                SemanticLimits::default(),
                Some(&mut trace),
            )
            .unwrap();
            assert_eq!(visits, stop);
            assert!(report
                .files
                .iter()
                .all(|f| f.status == FactStatus::ResourceLimit
                    && f.failure.is_none()
                    && f.failure_line.is_none()));
            assert_eq!(report.facts.len(), 2);
            assert!(report.facts.iter().all(|f| f.kind == "module"));
            assert!(trace.is_empty() && report.diagnostics.is_empty());
        }
    }
}

#[test]
fn root_compatibility_working_budget_rejects_before_callback() {
    for text in [
        "consume()\n",
        "value = '\\xzz'\n",
        "value = f'{call():\\xzz}'\n",
    ] {
        let inputs = [source(text)];
        for budget in [0, 1] {
            let mut callbacks = 0;
            let report = run_consumer(
                &inputs,
                Limits {
                    compatibility_working: budget,
                    ..Limits::default()
                },
                |_| false,
                &mut |_, _, _, _| callbacks += 1,
            )
            .unwrap();
            assert_eq!(callbacks, 0);
            assert_eq!(report.files[0].admission, Admission::ResourceLimit);
            assert_eq!(
                report.files[0].diagnostic,
                Some(Diagnostic::CompatibilityLimit)
            );
            assert_eq!(report.files[0].failure, None);
        }
    }
    // Literal diagnostics require a second, token-range reservation. Discover
    // the first reservation independently, then deny that additional storage.
    let text = "value = '\\xzz'\n";
    let parsed = ruff_python_parser::parse_unchecked(
        text,
        ruff_python_parser::ParseOptions::from(ruff_python_parser::Mode::Module)
            .with_target_version(ruff_python_ast::PythonVersion::PY312)
            .with_ast_only(true),
    );
    let mut charges = Vec::new();
    let detected = failures::inspect(
        text,
        &parsed,
        &mut |bytes| {
            charges.push(bytes);
            Ok(())
        },
        &mut || false,
    )
    .unwrap();
    assert!(detected.unwrap().failure.is_some());
    assert_eq!(charges.len(), 2);
    let inputs = [source(text)];
    for (budget, admitted) in [(charges[0], false), (charges.iter().sum(), true)] {
        let mut callbacks = 0;
        let report = run_consumer(
            &inputs,
            Limits {
                compatibility_working: budget,
                ..Limits::default()
            },
            |_| false,
            &mut |_, _, _, _| callbacks += 1,
        )
        .unwrap();
        assert_eq!(callbacks, 0);
        assert_eq!(
            report.files[0].admission,
            if admitted {
                Admission::InvalidSource
            } else {
                Admission::ResourceLimit
            }
        );
        assert_eq!(report.files[0].failure.is_some(), admitted);
    }
}

#[cfg(unix)]
#[test]
#[ignore = "requires explicitly selected trusted exact CPython 3.12.11; actual production helper failures"]
fn trusted_helper_failure_records_and_module_baselines_are_exact() {
    let cases = rejected();
    let helper = include_str!("../../python_extract.py");
    let (helper, autocall) = helper
        .rsplit_once("\nif __name__ == \"__main__\":")
        .unwrap();
    assert_eq!(autocall.trim(), "main()");
    let script = format!(
        "{helper}\n{}",
        r#"
import contextlib, io, unicodedata, warnings
assert sys.version_info[:3] == (3, 12, 11)
assert unicodedata.unidata_version == '15.0.0'
sys.set_int_max_str_digits(4300)
warnings.simplefilter('ignore')
requests = json.load(sys.stdin)
original_stdin = sys.stdin
outputs = []
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
    let requests: Vec<_> = cases.iter().map(|case| json!({"sources": [{"path": case.input.path, "module": case.input.module, "hex": case.input.hex}]})).collect();
    let mut command = std::process::Command::new(crate::analysis::python_parity::oracle());
    command
        .args(["-I", "-S", "-B", "-c", &script])
        .env_clear()
        .current_dir("/")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let output = crate::analysis::run_bounded(
        command,
        serde_json::to_vec(&requests).unwrap(),
        "Python Ruff production failure records",
    )
    .unwrap();
    let observed: Vec<Value> = serde_json::from_slice(&output).unwrap();
    assert_eq!(observed.len(), cases.len());
    let mut counts = [0; 3];
    for (index, (case, actual)) in cases.iter().zip(observed).enumerate() {
        let native = extract_fact_candidates(std::slice::from_ref(&case.input)).unwrap();
        assert_eq!(actual["complete"], false, "case {index}");
        assert_eq!(
            actual["facts"],
            serde_json::to_value(&native.facts).unwrap(),
            "full module-only facts case {index}"
        );
        assert_eq!(actual["facts"].as_array().unwrap().len(), 1);
        assert_eq!(actual["facts"][0]["kind"], "module");
        assert_eq!(
            actual["diagnostics"],
            json!([{"code": "parse_failure", "path": case.input.path, "message": case.expected.message()}]),
            "actual helper record case {index}"
        );
        assert!(
            native.diagnostics.is_empty(),
            "private projection must not emit helper diagnostics"
        );
        match case.outcome {
            Outcome::Certified => {
                counts[0] += 1;
                let failure = native.files[0].failure.unwrap();
                assert_eq!(failure, case.expected);
                assert_eq!(actual["diagnostics"][0]["message"], failure.message());
            }
            Outcome::Ambiguous | Outcome::ParserOnly => {
                counts[if matches!(case.outcome, Outcome::Ambiguous) {
                    1
                } else {
                    2
                }] += 1;
                assert_eq!(native.files[0].failure, None);
                assert_ne!(native.files[0].status, FactStatus::AcceptedFacts);
            }
        }
    }
    assert_eq!(counts, [53, 6, 5]);
    eprintln!("production helper: 53 certified failures; 6 unresolved rejections; 5 parser-only rejections; full module baselines and actual parse_failure records");
}

#[cfg(unix)]
#[test]
#[ignore = "requires explicit trusted CPython 3.12 / Unicode 15; complete accepted facts and raw observations"]
fn trusted_ast_only_controls_and_nested_formats_preserve_complete_facts() {
    super::ruff_facts_batch::assert_oracle_sources(accepted());
}
