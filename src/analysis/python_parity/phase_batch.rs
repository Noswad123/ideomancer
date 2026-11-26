//! Raw helper-order proof. Included under native facts so the opt-in capture does
//! not widen the production API. Module baselines are batch preflight, not visits.
use super::*;
use serde_json::{json, Value};

fn source(text: &str) -> Source {
    Source {
        path: "unit.py".into(),
        module: "unit".into(),
        bytes: text.as_bytes().to_vec(),
        hex: text
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    }
}

fn raw(text: &str) -> (CandidateReport<'static>, Vec<NativeObservation>) {
    // Only the report's file labels borrow input; use static labels for unit data.
    let sources = [source(text)];
    let (report, trace) = extract_with_observations(&sources).unwrap();
    let report = CandidateReport {
        identity: report.identity,
        facts: report.facts,
        diagnostics: report.diagnostics,
        files: report
            .files
            .into_iter()
            .map(|file| CandidateFile {
                path: "unit.py",
                module: "unit",
                ..file
            })
            .collect(),
    };
    (report, trace)
}

#[test]
fn phase_batch_assignment_metadata_precedes_all_child_calls() {
    for (text, expected) in [
        ("a = b = factory()\n", vec!["binding", "binding", "call"]),
        (
            "a, (b, c) = factory()\n",
            vec![
                "binding_invalidation",
                "binding_invalidation",
                "binding_invalidation",
                "call",
            ],
        ),
        (
            "a = receiver()[index()] = rhs()\n",
            vec!["binding", "call", "call", "call"],
        ),
        ("receiver()[index()]: Type\n", vec!["call", "call"]),
        (
            "class C:\n    field: Type = factory()\n",
            vec!["class", "attribute_annotation", "binding", "call"],
        ),
        (
            "class C:\n    field: Type\n",
            vec!["class", "attribute_annotation"],
        ),
    ] {
        let (report, trace) = raw(text);
        assert_eq!(
            report.files[0].status,
            CandidateStatus::AcceptedFacts,
            "{text}"
        );
        let facts: Vec<_> = trace
            .iter()
            .filter_map(|event| match event {
                NativeObservation::Fact { fact } => Some(fact),
                _ => None,
            })
            .collect();
        assert_eq!(
            facts
                .iter()
                .map(|fact| fact.kind.as_str())
                .collect::<Vec<_>>(),
            expected,
            "{text}"
        );
        assert!(facts.iter().all(|fact| fact.end_line != usize::MAX));
        if text.starts_with("a = b") {
            assert_eq!(facts[0].value, r#"{"source":"factory()","target":"b"}"#);
            assert_eq!(facts[1].value, r#"{"source":"factory()","target":"a"}"#);
        }
    }
}

#[test]
fn phase_batch_headers_are_definition_owned_and_parameters_not_signature_clipped() {
    let text = "def f(a: A=d, /, b: B=e, *v: V, k: K=z, **w: W) -> R:\n    body()\n    # not the definition end\n";
    let (report, trace) = raw(text);
    assert_eq!(report.files[0].status, CandidateStatus::AcceptedFacts);
    let facts: Vec<_> = trace
        .iter()
        .filter_map(|event| match event {
            NativeObservation::Fact { fact } => Some(fact),
            _ => None,
        })
        .collect();
    assert_eq!(
        facts
            .iter()
            .map(|fact| fact.kind.as_str())
            .collect::<Vec<_>>(),
        [
            "function",
            "signature",
            "parameter",
            "parameter_annotation",
            "parameter_default",
            "parameter",
            "parameter_annotation",
            "parameter_default",
            "parameter",
            "parameter_annotation",
            "parameter_default",
            "parameter",
            "parameter_annotation",
            "parameter",
            "parameter_annotation",
            "return_annotation",
            "call",
        ]
    );
    assert_eq!(
        facts
            .iter()
            .filter(|fact| fact.kind == "parameter")
            .map(|fact| fact.value.as_str())
            .collect::<Vec<_>>(),
        ["a", "b", "k", "v", "w"]
    );
    assert_eq!(facts[0].end_line, 2);
    let parameters = (0..129)
        .map(|i| format!("p{i}"))
        .collect::<Vec<_>>()
        .join(",");
    let (report, trace) = raw(&format!("def many({parameters}): pass\n"));
    assert_eq!(report.files[0].status, CandidateStatus::AcceptedFacts);
    assert_eq!(trace.iter().filter(|event| matches!(event, NativeObservation::Fact { fact } if fact.kind == "parameter")).count(), 129);
    assert!(report
        .facts
        .iter()
        .any(|fact| fact.kind == "signature" && fact.value == r#"{"truncated":true}"#));
}

#[test]
fn phase_batch_compact_header_storage_does_not_retain_renderer_scratch() {
    for form in [0, 1, 2] {
        let parameters = (0..4096)
            .map(|i| match form {
                0 => format!("p{i}=0"),
                1 => format!("p{i}: T"),
                _ => format!("p{i}: T=0"),
            })
            .collect::<Vec<_>>()
            .join(",");
        let sources = [source(&format!("def f({parameters}): pass\n"))];
        let report = extract(
            &sources,
            Limits::default(),
            FactLimits {
                working: 16 * 1024 * 1024,
                ..FactLimits::default()
            },
            |_| false,
        )
        .unwrap();
        assert_eq!(
            report.files[0].status,
            CandidateStatus::AcceptedFacts,
            "form {form}"
        );
        assert_eq!(
            report
                .facts
                .iter()
                .filter(|fact| fact.kind == "parameter")
                .count(),
            4096
        );
        assert!(report
            .facts
            .iter()
            .any(|fact| fact.kind == "signature" && fact.value == r#"{"truncated":true}"#));
    }
}

#[test]
fn phase_batch_failure_and_cancellation_discard_raw_prefixes() {
    let sources = [source("from arbitrary import eval as probe\ndef f(a: A=d, *v, k: K=z, **w) -> R:\n    a = receiver()[index()] = probe()\n")];
    let mut checkpoints = 0;
    let mut baseline = Vec::new();
    let report = extract_impl(
        &sources,
        Limits::default(),
        FactLimits::default(),
        |phase| {
            checkpoints += usize::from(matches!(
                phase,
                Checkpoint::Collector
                    | Checkpoint::Working
                    | Checkpoint::FactId
                    | Checkpoint::Sort
                    | Checkpoint::FinalOutput
                    | Checkpoint::Final
            ));
            false
        },
        Some(&mut baseline),
    )
    .unwrap();
    assert_eq!(report.files[0].status, CandidateStatus::AcceptedFacts);
    assert!(!baseline.is_empty());
    for stop in 1..=checkpoints {
        let mut seen = 0;
        let mut trace = Vec::new();
        let report = extract_impl(
            &sources,
            Limits::default(),
            FactLimits::default(),
            |phase| {
                seen += usize::from(matches!(
                    phase,
                    Checkpoint::Collector
                        | Checkpoint::Working
                        | Checkpoint::FactId
                        | Checkpoint::Sort
                        | Checkpoint::FinalOutput
                        | Checkpoint::Final
                ));
                seen == stop
            },
            Some(&mut trace),
        )
        .unwrap();
        assert_eq!(report.facts.len(), 1, "checkpoint {stop}");
        assert!(report.diagnostics.is_empty());
        assert!(trace.is_empty(), "checkpoint {stop}");
        assert_eq!(
            report.files[0].diagnostic,
            Some(CandidateDiagnostic::Parse(DiagnosticCode::Deadline))
        );
    }
    for tail in [
        "a = APIRouter()\n",
        "class C:\n    a: hidden()\n",
        "@decorate()\ndef g(): pass\n",
    ] {
        let (report, trace) = raw(&format!("probe = eval()\n{tail}"));
        assert_eq!(report.files[0].status, CandidateStatus::UnsupportedSyntax);
        assert_eq!(report.facts.len(), 1);
        assert!(report.diagnostics.is_empty());
        assert!(trace.is_empty());
    }
    // Sweep every small working limit, covering late capture and drain failures
    // without relying on allocator sizes or exposing partial successful prefixes.
    for working in (0..100_000).step_by(251) {
        let mut trace = Vec::new();
        let report = extract_impl(
            &sources,
            Limits::default(),
            FactLimits {
                working,
                ..FactLimits::default()
            },
            |_| false,
            Some(&mut trace),
        )
        .unwrap();
        if report.files[0].status != CandidateStatus::AcceptedFacts {
            assert_eq!(report.facts.len(), 1);
            assert!(report.diagnostics.is_empty());
            assert!(trace.is_empty());
        }
    }
    for limits in [
        FactLimits {
            count: 4,
            ..FactLimits::default()
        },
        FactLimits {
            warnings: 0,
            ..FactLimits::default()
        },
        FactLimits {
            bytes: 3000,
            ..FactLimits::default()
        },
    ] {
        let mut trace = Vec::new();
        let report = extract_impl(
            &sources,
            Limits::default(),
            limits,
            |_| false,
            Some(&mut trace),
        )
        .unwrap();
        assert_eq!(report.files[0].status, CandidateStatus::ResourceLimit);
        assert_eq!(report.facts.len(), 1);
        assert!(report.diagnostics.is_empty());
        assert!(trace.is_empty());
    }
}

#[test]
fn phase_batch_statement_capacity_is_released_and_multifile_indices_survive_rollback() {
    let sources = [source(&"a = receiver()[index()] = rhs()\n".repeat(100))];
    let report = extract(
        &sources,
        Limits::default(),
        FactLimits {
            working: 64 * 1024,
            ..FactLimits::default()
        },
        |_| false,
    )
    .unwrap();
    assert_eq!(report.files[0].status, CandidateStatus::AcceptedFacts);

    let mut sources = vec![
        source("from arbitrary import eval as probe\na = probe()\n"),
        source("from arbitrary import exec as probe\nb = probe()\ndef bad(a=hidden()): pass\n"),
        source("from other import exec as probe\ndef f():\n    c = probe()\n"),
    ];
    for (source, name) in sources.iter_mut().zip(["a", "b", "c"]) {
        source.path = format!("{name}.py");
        source.module = name.into();
    }
    let (report, trace) = extract_with_observations(&sources).unwrap();
    assert_eq!(
        report
            .files
            .iter()
            .map(|file| file.status)
            .collect::<Vec<_>>(),
        [
            CandidateStatus::AcceptedFacts,
            CandidateStatus::UnsupportedSyntax,
            CandidateStatus::AcceptedFacts,
        ]
    );
    let paths: Vec<_> = trace
        .iter()
        .map(|event| match event {
            NativeObservation::Fact { fact } => {
                assert!(report.facts.iter().any(|stored| stored == fact));
                assert_ne!(fact.end_line, usize::MAX);
                fact.path.as_str()
            }
            NativeObservation::Warning { warning } => {
                assert!(report.diagnostics.iter().any(|stored| stored == warning));
                warning.path.as_str()
            }
        })
        .collect();
    assert!(paths.contains(&"a.py") && paths.contains(&"c.py"));
    assert!(!paths.contains(&"b.py"));
    for stop in [Checkpoint::Sort, Checkpoint::FinalOutput] {
        let mut trace = Vec::new();
        let report = extract_impl(
            &sources,
            Limits::default(),
            FactLimits::default(),
            |phase| phase == stop,
            Some(&mut trace),
        )
        .unwrap();
        assert_eq!(report.facts.len(), 3);
        assert!(report.facts.iter().all(|fact| fact.kind == "module"));
        assert!(report.diagnostics.is_empty() && trace.is_empty());
        assert!(report
            .files
            .iter()
            .all(|file| file.diagnostic
                == Some(CandidateDiagnostic::Parse(DiagnosticCode::Deadline))));
    }
}

#[cfg(unix)]
#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn phase_batch_raw_trace_and_complete_facts_match_trusted_helper() {
    let python = crate::analysis::python_parity::oracle();
    // Trusted-script instrumentation, not execution/import of analyzed source.
    // Keep the real Extractor as oracle; only remove its command-line autocall.
    let helper = include_str!("../../python_extract.py");
    let (helper, autocall) = helper
        .rsplit_once("\nif __name__ == \"__main__\":")
        .unwrap();
    assert_eq!(autocall.trim(), "main()");
    let script = format!(
        "{helper}\n{}",
        r#"
class Diagnostics(list):
    def append(self, value):
        trace.append(dict(warning=value))
        super().append(value)
class Traced(Extractor):
    def fact(self, *args, **kwargs):
        super().fact(*args, **kwargs)
        if self.facts[-1]['kind'] != 'module':
            trace.append(dict(fact=self.facts[-1]))
request = json.load(sys.stdin)
traces = []
for source in request['sources']:
    trace = []
    visitor = Traced(source['path'], source['module'], [], Diagnostics())
    visitor.visit(ast.parse(bytes.fromhex(source['hex']), filename=source['path'], type_comments=True))
    traces.append(trace)
json.dump(traces, sys.stdout, ensure_ascii=True)
"#
    );
    let parameters = (0..129)
        .map(|i| format!("p{i}"))
        .collect::<Vec<_>>()
        .join(",");
    let neutral = tempfile::tempdir().unwrap();
    let marker = neutral.path().join("not_executed");
    let texts = vec![
        "a = b = factory()\na, (b, c) = factory()\nreceiver()[index()] = rhs()\na = receiver()[index()] = rhs()\nreceiver()[index()]: Type\n".to_owned(),
        "class C:\n    field: Type = factory()\n    other: Type\n    receiver()[index()]: Type\n".into(),
        "def f(a: A=d, /, b: B=e, *v: V, k: K=z, **w: W) -> R:\n    body()\n    # trailing comment\nasync def g(a, *v, k=z, **w):\n    return result()\n".into(),
        "class C(First, metaclass=Meta, *Bases, key=value, **options):\n    def f(a: A=(\n        default\n    )) -> R:\n        a = factory()\n        # trailing comment\n".into(),
        "from arbitrary import eval as probe\ndef repeat(a: A=d, *v, k=z, **w) -> R:\n    a = b = probe()\n    from nested import exec as probe\n    def nested():\n        a = probe()\n    a = probe()\ndef repeat():\n    a = probe()\nprobe()\n".into(),
        "a = probe(); plain(); a = other()\nreceiver()[index()] += rhs()\ndel receiver()[index()]\nfor receiver()[index()] in iterable():\n    a = factory()\nwith context() as value:\n    a = factory()\ntry:\n    a = factory()\nexcept error() as exc:\n    a = factory()\ndef f():\n    return\n".into(),
        "from arbitrary import eval as probe\na = probe() + probe()\na = probe(); a = probe()\n".into(),
        format!("def many({parameters}): pass\n"),
        format!("a = factory()\nopen({:?}, 'w').write('redacted')\n", marker.to_str().unwrap()),
    ];
    let sources: Vec<_> = texts.iter().map(|text| source(text)).collect();
    let request = json!({"sources": sources.iter().map(|s| json!({"path":s.path,"module":s.module,"hex":s.hex})).collect::<Vec<_>>()});
    let mut command = std::process::Command::new(&python);
    command
        .args(["-I", "-S", "-c", &script])
        .current_dir(neutral.path())
        .env_clear()
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let output =
        crate::analysis::run_bounded(command, serde_json::to_vec(&request).unwrap(), "Python")
            .unwrap();
    let expected: Vec<Value> = serde_json::from_slice(&output).unwrap();
    for (index, (source, expected)) in sources.iter().zip(expected).enumerate() {
        let captured = std::slice::from_ref(source);
        let (native, trace) = extract_with_observations(captured).unwrap();
        assert_eq!(
            native.files[0].status,
            CandidateStatus::AcceptedFacts,
            "case {index}"
        );
        assert_eq!(
            serde_json::to_value(&trace).unwrap(),
            expected,
            "raw case {index}: {}",
            texts[index]
        );
        let ast = crate::analysis::extract_captured(&python, captured).unwrap();
        assert!(ast.complete);
        assert_eq!(
            serde_json::to_value(&native.facts).unwrap(),
            serde_json::to_value(&ast.facts).unwrap(),
            "facts case {index}"
        );
        assert_eq!(
            serde_json::to_value(&native.diagnostics).unwrap(),
            serde_json::to_value(&ast.diagnostics).unwrap(),
            "warnings case {index}"
        );
        let path = neutral.path().join("unit.py");
        std::fs::write(&path, &source.bytes).unwrap();
        std::fs::remove_file(path).unwrap();
        let (replay, replay_trace) = extract_with_observations(captured).unwrap();
        assert_eq!(native.facts, replay.facts);
        assert_eq!(native.diagnostics, replay.diagnostics);
        assert_eq!(trace, replay_trace);
    }
    assert!(!marker.exists());
}
