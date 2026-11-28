//! F3a synthetic byte-to-fact integration; no production dispatch.
use super::*;
use ruff_python_parser::lexer::byte::ByteLexFailure;

fn source(bytes: &[u8]) -> Source {
    Source {
        path: "unit.py".into(),
        module: "unit".into(),
        bytes: bytes.to_vec(),
        hex: bytes.iter().map(|byte| format!("{byte:02x}")).collect(),
    }
}

fn bytes_with_trace(
    sources: &[Source],
) -> (CandidateReport<'_>, Vec<ByteOutcome>, Vec<Observation>) {
    let mut outcomes = Vec::new();
    let mut trace = Vec::new();
    let report = extract_lane(
        sources,
        |_| false,
        SemanticLimits::default(),
        FactLane::Bytes,
        &mut outcomes,
        Some(&mut trace),
    )
    .unwrap();
    (report, outcomes, trace)
}

fn parsed_once(outcome: &ByteOutcome) {
    let ByteOutcome::Parsed { stats } = outcome else {
        panic!("expected one accepted byte parse, got {outcome:?}");
    };
    assert_eq!(stats.parse_invocations, 1);
}

// Independent ASCII projection: no candidate emitter, ID helper, repaired input,
// or text parser supplies these expectations. Rows are in raw observation order.
fn projected_fact(
    path: &str,
    line: usize,
    end_line: usize,
    kind: &str,
    symbol: &str,
    value: &str,
) -> shared::Fact {
    let quoted = |text: &str| {
        assert!(text.is_ascii());
        serde_json::to_string(text).unwrap()
    };
    let id = if kind == "module" {
        format!("module_{:x}", Sha256::digest(path.as_bytes()))
    } else {
        let preimage = format!(
            "[{}, {}, {line}, {end_line}, {}, {}]",
            quoted(path),
            quoted(kind),
            quoted(symbol),
            quoted(value)
        );
        format!("fact_{:x}", Sha256::digest(preimage.as_bytes()))
    };
    shared::Fact {
        id,
        kind: kind.into(),
        path: path.into(),
        line,
        end_line,
        symbol: symbol.into(),
        value: value.into(),
    }
}

fn assert_projection(bytes: &[u8], rows: &[(usize, usize, &str, &str, &str)]) {
    let sources = [source(bytes)];
    let (report, outcomes, trace) = bytes_with_trace(&sources);
    assert_eq!(
        report.files[0].status,
        FactStatus::AcceptedFacts,
        "{report:?}"
    );
    assert_eq!(report.files[0].failure, None);
    assert_eq!(report.files[0].failure_line, None);
    parsed_once(&outcomes[0]);
    let raw: Vec<_> = rows
        .iter()
        .map(|&(line, end, kind, symbol, value)| {
            projected_fact("unit.py", line, end, kind, symbol, value)
        })
        .collect();
    assert_eq!(
        trace,
        raw.iter()
            .cloned()
            .map(|fact| Observation::Fact { fact })
            .collect::<Vec<_>>()
    );
    let mut expected = raw;
    expected.push(projected_fact("unit.py", 1, 1, "module", "unit", "unit"));
    expected.sort_by(|a, b| {
        (a.line, a.end_line, &a.kind, &a.symbol, &a.value, &a.id)
            .cmp(&(b.line, b.end_line, &b.kind, &b.symbol, &b.value, &b.id))
    });
    expected.dedup();
    assert_eq!(report.facts, expected);
    assert!(report.diagnostics.is_empty());
    assert_eq!(
        report.files[0].sha256,
        format!("{:x}", Sha256::digest(bytes))
    );
}

// Shared-implementation differential controls are supplemental, not an oracle
// for malformed bytes (which never enter the text lane).
fn assert_text_control(bytes: &[u8]) {
    let sources = [source(bytes)];
    let (byte, outcomes, byte_trace) = bytes_with_trace(&sources);
    let mut text_trace = Vec::new();
    let text = extract_impl(
        &sources,
        |_| false,
        SemanticLimits::default(),
        Some(&mut text_trace),
    )
    .unwrap();
    assert_eq!(byte.files[0].status, FactStatus::AcceptedFacts, "{byte:?}");
    assert_eq!(text.files[0].status, FactStatus::AcceptedFacts, "{text:?}");
    parsed_once(&outcomes[0]);
    assert_eq!(byte.facts, text.facts);
    assert_eq!(byte.diagnostics, text.diagnostics);
    assert_eq!(byte_trace, text_trace);
    assert_eq!(byte.files[0].sha256, text.files[0].sha256);
    assert_eq!(byte.files[0].type_ignores, text.files[0].type_ignores);
}

fn assert_rejected(bytes: &[u8], status: FactStatus) {
    let sources = [source(bytes)];
    let (report, _, trace) = bytes_with_trace(&sources);
    let file = &report.files[0];
    assert_eq!(file.status, status, "{report:?}");
    assert_eq!(file.failure, None);
    assert_eq!(file.failure_line, None);
    assert_eq!(file.nodes, 0);
    assert_eq!(file.edges, 0);
    assert_eq!(
        report.facts,
        [projected_fact("unit.py", 1, 1, "module", "unit", "unit")]
    );
    assert!(report.diagnostics.is_empty());
    assert!(trace.is_empty());
}

#[test]
fn f3a_identity_is_separate_from_unchanged_text_policy() {
    let sources = [source(b"call()\n")];
    let byte = extract_byte_fact_candidates(&sources).unwrap();
    let text = extract_fact_candidates(&sources).unwrap();
    assert_eq!(
        byte.report.identity.policy,
        "python-ruff-private-byte-facts-1"
    );
    assert_eq!(text.identity.policy, "python-ruff-private-facts-7");
    assert_ne!(byte.report.identity.sha256, text.identity.sha256);
    assert_eq!(byte.outcomes.len(), 1);
    parsed_once(&byte.outcomes[0]);
}

#[test]
fn f3a_ordinary_utf8_complete_output_and_replay_controls() {
    for text in [
        "from early import eval as probe\nprobe(); probe()\ndef f(a=probe()) -> probe():\n from late import exec as probe\n probe()\nprobe()\n",
        "class C(Base):\n def method(self, a: T=default()):\n  return self.worker(a)\n",
        "match subject():\n case Record(child=[head, *tail]) if guard(): body()\ntype Alias[T: bound()] = rhs()\n",
        "def caf\u{e9}(\u{3bb}=factory()):\n return service(\u{3bb})\n",
    ] {
        assert_text_control(text.as_bytes());
    }
}

#[test]
fn f3a_metadata_strings_surrogates_method_preimages_and_alias_controls() {
    for text in [
        "import fastapi as framework\nfrom fastapi import APIRouter as Router\nr = Router(prefix='/' 'v1')\napp = framework.FastAPI()\napp.include_router(r, prefix='/mounted')\n",
        "@api.route('/' 'caf\u{e9}/\\ud800', methods=['opt\\u0131ons', 'po\\u017ft', 'po\\ufb05', 'po\\ufb06'])\ndef f(): body()\n",
        "@api.get(r'/raw\\n' u'/\\N{LATIN CAPITAL LETTER A}')\n@api.route(path=unknown, methods=['get', unknown])\ndef f(): body()\n",
    ] {
        assert_text_control(text.as_bytes());
    }
}

#[test]
fn f3a_bom_crlf_offsets_and_original_hash_control() {
    assert_text_control(
        b"\xef\xbb\xbf# coding: utf-8\r\n@api.get('/bom')\r\ndef f():\r\n body()\r\nafter()\r\n",
    );
    assert_projection(
        b"\xef\xbb\xbf#\xff\r\ncall()\r\n",
        &[(2, 2, "call", "unit", "call()")],
    );
}

#[test]
fn f3a_prefix_metadata_hashes_every_original_byte() {
    let mut hashes = Vec::new();
    for bytes in [
        b"r=APIRouter(prefix='/v1') #\xff\n".as_slice(),
        b"r=APIRouter(prefix='/v1') #\xfe\n".as_slice(),
    ] {
        assert_projection(
            bytes,
            &[
                (
                    1,
                    1,
                    "router_prefix",
                    "unit",
                    r#"{"constructor":"APIRouter","prefix":"/v1","supported":true,"target":"r"}"#,
                ),
                (
                    1,
                    1,
                    "binding",
                    "unit",
                    r#"{"source":"APIRouter(prefix=<literal>)","target":"r"}"#,
                ),
                (1, 1, "call", "unit", "APIRouter(prefix=<literal>)"),
            ],
        );
        let sources = [source(bytes)];
        hashes.push(
            extract_byte_fact_candidates(&sources).unwrap().report.files[0]
                .sha256
                .clone(),
        );
    }
    assert_ne!(hashes[0], hashes[1]);
}

#[test]
fn f3a_utf8_ignores_and_type_comment_attachment_controls() {
    for bytes in [
        b"# type: ignore[unit]\nx=call() # type: Value\n".as_slice(),
        b"def f(a, # type: A\n b=1 # type: B\n): # type: (A, B) -> R\n body()\n".as_slice(),
        b"for x in items: # type: Item\n body()\nwith manager() as y: # type: Context\n use()\n"
            .as_slice(),
    ] {
        assert_text_control(bytes);
    }
    let sources = [source(b"#\xff\nx=call() # type: ignore[unit]\n")];
    let (report, outcomes, _) = bytes_with_trace(&sources);
    assert_eq!(report.files[0].status, FactStatus::AcceptedFacts);
    assert_eq!(report.files[0].type_ignores, 1);
    parsed_once(&outcomes[0]);
    assert_projection(
        b"#\xff\nx=call() # type: Value\n",
        &[
            (
                2,
                2,
                "binding",
                "unit",
                r#"{"source":"call()","target":"x"}"#,
            ),
            (2, 2, "call", "unit", "call()"),
        ],
    );
}

#[test]
fn f3a_malformed_ordinary_comment_matrix_has_independent_complete_projection() {
    for bad in [
        b"\xff".as_slice(),
        b"\xc0\xaf",
        b"\xe2\x82",
        b"\xed\xa0\x80",
    ] {
        for (prefix, suffix, line, end) in [
            ("#", "\ncall()\n", 2, 2),
            ("if ready:\n #", "\n call()\n", 3, 3),
            ("call() #", "\n", 1, 1),
            ("( #", "\n call()\n)\n", 2, 2),
            ("call( #", "\n)\n", 1, 2),
            ("#\x0ctype: ", "\ncall()\n", 2, 2),
        ] {
            let bytes = [prefix.as_bytes(), bad, suffix.as_bytes()].concat();
            assert_projection(&bytes, &[(line, end, "call", "unit", "call()")]);
        }
    }
}

#[test]
fn f3a_malformed_fstring_replacement_comment_preserves_visit_order() {
    assert_projection(
        b"sink(f'{call() #\xff\n}')\n",
        &[
            (1, 2, "call", "unit", "sink(<JoinedStr>)"),
            (1, 1, "call", "unit", "call()"),
        ],
    );
}

#[test]
fn f3a_malformed_debug_and_nested_fstring_comments_preserve_observables() {
    for bytes in [
        b"sink(f'{call() #\xff\n=}')\n".as_slice(),
        b"sink(f'{f\"{call() #\xff\n=}\"}')\n".as_slice(),
    ] {
        assert_projection(
            bytes,
            &[
                (1, 2, "call", "unit", "sink(<JoinedStr>)"),
                (1, 1, "call", "unit", "call()"),
            ],
        );
    }
}

#[test]
fn f3a_malformed_comments_between_metadata_string_views_are_not_decoded() {
    assert_projection(
        b"r=APIRouter(prefix=('/' #\xff\n 'v1'))\n",
        &[
            (
                1,
                2,
                "router_prefix",
                "unit",
                r#"{"constructor":"APIRouter","prefix":"/v1","supported":true,"target":"r"}"#,
            ),
            (
                1,
                2,
                "binding",
                "unit",
                r#"{"source":"APIRouter(prefix=<literal>)","target":"r"}"#,
            ),
            (1, 2, "call", "unit", "APIRouter(prefix=<literal>)"),
        ],
    );
}

#[test]
fn f3a_non_utf8_codecs_are_excluded_before_parser_construction() {
    for bytes in [
        b"# coding: latin-1\ncall()\n".as_slice(),
        b"# coding: cp1252\ncall()\n".as_slice(),
        b"#!/python\n# coding: shift_jis\ncall()\n".as_slice(),
    ] {
        assert_rejected(bytes, FactStatus::UnsupportedEncoding);
        let sources = [source(bytes)];
        let report = extract_byte_fact_candidates(&sources).unwrap();
        assert!(matches!(report.outcomes[0], ByteOutcome::NotParsed(_)));
    }
    assert_text_control(b"# coding: utf-8\ncall()\n");
    assert_text_control(b"#!/python\n# coding: UTF_8\ncall()\n");
}

#[test]
fn f3a_invalid_noncomment_source_retains_typed_source_fault() {
    for bytes in [b"call()\n\xff\n".as_slice(), b"sink('\xff')\n".as_slice()] {
        assert_rejected(bytes, FactStatus::SyntaxError);
        let sources = [source(bytes)];
        let report = extract_byte_fact_candidates(&sources).unwrap();
        let ByteOutcome::Failed(byte_facts::ByteInputFailure::Input(ByteLexFailure::Source(fault))) =
            &report.outcomes[0]
        else {
            panic!("source fault was not retained: {:?}", report.outcomes);
        };
        let start = fault.range.start().to_usize();
        assert_eq!(bytes[start], 0xff);
        assert_eq!(fault.error_len, Some(1));
    }
}

#[test]
fn f3a_nul_syntax_and_version_failures_never_lower_recovery_ast() {
    for bytes in [b"#\0\ncall()\n".as_slice(), b"sink('\0')\n".as_slice()] {
        assert_rejected(bytes, FactStatus::SyntaxError);
        let sources = [source(bytes)];
        let report = extract_byte_fact_candidates(&sources).unwrap();
        assert!(matches!(report.outcomes[0], ByteOutcome::NotParsed(_)));
    }
    for (bytes, status) in [
        (b"before()\ndef f(:\n".as_slice(), FactStatus::SyntaxError),
        (
            b"type Alias[T = int] = T\n".as_slice(),
            FactStatus::UnsupportedSyntax,
        ),
        (
            b"type Alias[*Ts = *tuple[int]] = int\n".as_slice(),
            FactStatus::UnsupportedSyntax,
        ),
        (
            b"type Alias[**P = [int]] = int\n".as_slice(),
            FactStatus::UnsupportedSyntax,
        ),
    ] {
        assert_rejected(bytes, status);
        let sources = [source(bytes)];
        let report = extract_byte_fact_candidates(&sources).unwrap();
        let ByteOutcome::Rejected { stats, .. } = &report.outcomes[0] else {
            panic!("expected diagnostic rejection: {:?}", report.outcomes);
        };
        assert_eq!(stats.parse_invocations, 1);
    }
}

#[test]
fn f3a_shared_unicode_literal_decimal_and_format_guards_precede_semantics() {
    for text in [
        "before()\n\u{1c89}=1\n".to_owned(),
        "before()\nsink('\\N{NOT A UNICODE NAME}')\n".to_owned(),
        "before()\nsink('\\xzz')\n".to_owned(),
        "before()\nsink('\\u12')\n".to_owned(),
        "before()\nsink(f'{value:\\xzz}')\n".to_owned(),
        "before()\nsink(f'{a:{b:{c:{d}}}}')\n".to_owned(),
        format!("before()\nx={}\n", "1".repeat(4301)),
    ] {
        let sources = [source(text.as_bytes())];
        let (report, _, trace) = bytes_with_trace(&sources);
        assert!(
            matches!(
                report.files[0].status,
                FactStatus::UnsupportedSyntax | FactStatus::SyntaxError
            ),
            "{report:?}"
        );
        assert_eq!(report.files[0].failure, None);
        assert_eq!(report.files[0].failure_line, None);
        assert_eq!(report.files[0].nodes, 0);
        assert_eq!(report.files[0].edges, 0);
        assert_eq!(
            report.facts,
            [projected_fact("unit.py", 1, 1, "module", "unit", "unit")]
        );
        assert!(report.diagnostics.is_empty());
        assert!(trace.is_empty());
    }
}

#[test]
fn f3a_malformed_actual_type_and_ignore_payloads_are_explicitly_unsupported() {
    for bytes in [
        b"x=call() # type: \xff\n".as_slice(),
        b"#\t type: ignore[\xff]\ncall()\n".as_slice(),
        b"# type: ignore\xff\ncall()\n".as_slice(),
        b"sink(f'{call() # type: ignore[\xff]\n}')\n".as_slice(),
    ] {
        assert_rejected(bytes, FactStatus::UnsupportedSyntax);
        let sources = [source(bytes)];
        let report = extract_byte_fact_candidates(&sources).unwrap();
        parsed_once(&report.outcomes[0]);
        assert_eq!(
            report.report.files[0].reason,
            Some("malformed UTF-8 type-comment payload")
        );
    }
}

#[test]
fn f3a_contextual_type_placement_rolls_back_and_isolates_siblings_in_both_orders() {
    for (bytes, status, line) in [
        (
            b"#\xff\n@api.get(unknown)\ndef f(): body()\npass # type: A\n".as_slice(),
            FactStatus::SyntaxError,
            Some(4),
        ),
        (
            b"before()\ndef f(:\n".as_slice(),
            FactStatus::SyntaxError,
            None,
        ),
        (
            b"before()\n\xff\n".as_slice(),
            FactStatus::SyntaxError,
            None,
        ),
        (
            b"before() # type: \xff\n".as_slice(),
            FactStatus::UnsupportedSyntax,
            None,
        ),
        (
            b"before()\nsink('\\xzz')\n".as_slice(),
            FactStatus::SyntaxError,
            None,
        ),
    ] {
        for reverse in [false, true] {
            let mut good = source(b"visible()\n");
            good.path = "sibling.py".into();
            good.module = "sibling".into();
            let mut bad = source(bytes);
            bad.path = "rejected.py".into();
            bad.module = "rejected".into();
            let mut sources = [good, bad];
            if reverse {
                sources.reverse();
            }
            let (report, outcomes, trace) = bytes_with_trace(&sources);
            parsed_once(&outcomes[usize::from(reverse)]);
            let bad = report
                .files
                .iter()
                .find(|file| file.path == "rejected.py")
                .unwrap();
            assert_eq!(bad.status, status);
            assert_eq!(bad.failure, None);
            assert_eq!(bad.failure_line, line);
            assert_eq!(
                report
                    .files
                    .iter()
                    .find(|file| file.path == "sibling.py")
                    .unwrap()
                    .status,
                FactStatus::AcceptedFacts
            );
            let sibling_call = projected_fact("sibling.py", 1, 1, "call", "sibling", "visible()");
            assert_eq!(
                trace,
                [Observation::Fact {
                    fact: sibling_call.clone()
                }]
            );
            assert_eq!(
                report.facts,
                [
                    projected_fact("rejected.py", 1, 1, "module", "rejected", "rejected"),
                    sibling_call,
                    projected_fact("sibling.py", 1, 1, "module", "sibling", "sibling"),
                ]
            );
            assert!(report.diagnostics.is_empty());
        }
    }
}

#[test]
fn f3a_cancellation_at_each_stage_invalidates_prior_facts_warnings_and_trace() {
    for point in [
        Checkpoint::Token,
        Checkpoint::Compatibility,
        Checkpoint::Semantic,
        Checkpoint::PostConsume,
        Checkpoint::PostDrop,
        Checkpoint::Final,
    ] {
        let mut sources = [
            source(b"@api.get(unknown)\ndef first(): body()\n"),
            source(b"after()\n"),
        ];
        sources[1].path = "second.py".into();
        sources[1].module = "second".into();
        let (control, _, control_trace) = bytes_with_trace(&sources);
        assert!(!control_trace.is_empty());
        assert!(!control.diagnostics.is_empty());
        let mut drops = 0;
        let mut fired = false;
        let mut outcomes = Vec::new();
        let mut trace = Vec::new();
        let report = extract_lane(
            &sources,
            |at| {
                // The first file has completed lowering before cancellation fires.
                let after_first = drops >= 1;
                if at == Checkpoint::PostDrop {
                    drops += 1;
                }
                let expire = !fired && after_first && at == point;
                fired |= expire;
                expire
            },
            SemanticLimits::default(),
            FactLane::Bytes,
            &mut outcomes,
            Some(&mut trace),
        )
        .unwrap();
        assert!(fired, "{point:?}");
        assert_eq!(outcomes.len(), 2);
        assert_eq!(report.facts.len(), 2, "{point:?}");
        assert!(report.facts.iter().all(|fact| fact.kind == "module"));
        assert!(report.diagnostics.is_empty());
        assert!(trace.is_empty());
        for file in &report.files {
            assert_eq!(file.status, FactStatus::ResourceLimit, "{point:?}");
            assert_eq!(file.failure, None);
            assert_eq!(file.failure_line, None);
        }
    }
}

#[test]
fn f3a_tiny_semantic_working_and_node_budgets_rollback_complete_observables() {
    let sources = [source(b"before()\n@api.get(unknown)\ndef f(): body()\n")];
    for limits in [
        SemanticLimits {
            working: 1024,
            ..SemanticLimits::default()
        },
        SemanticLimits {
            nodes: 2,
            ..SemanticLimits::default()
        },
    ] {
        let mut outcomes = Vec::new();
        let mut trace = Vec::new();
        let report = extract_lane(
            &sources,
            |_| false,
            limits,
            FactLane::Bytes,
            &mut outcomes,
            Some(&mut trace),
        )
        .unwrap();
        parsed_once(&outcomes[0]);
        assert_eq!(report.files[0].status, FactStatus::ResourceLimit);
        assert_eq!(report.files[0].failure, None);
        assert_eq!(report.files[0].failure_line, None);
        assert_eq!(
            report.facts,
            [projected_fact("unit.py", 1, 1, "module", "unit", "unit")]
        );
        assert!(report.diagnostics.is_empty());
        assert!(trace.is_empty());
    }
}

#[test]
fn f3a_default_wide_statement_is_typed_resource_limit_not_unsupported_syntax() {
    let text = format!("target({})\n", vec!["child()"; 300].join(","));
    assert_rejected(text.as_bytes(), FactStatus::ResourceLimit);
    let sources = [source(text.as_bytes())];
    let report = extract_byte_fact_candidates(&sources).unwrap();
    assert!(matches!(
        report.outcomes[0],
        ByteOutcome::Failed(byte_facts::ByteInputFailure::Input(
            ByteLexFailure::ResourceLimit
        ))
    ));
}

#[test]
fn f3a_staged_literal_views_deny_comments_and_misaligned_utf8_boundaries() {
    use ruff_text_size::TextSize;
    let bytes = b"#\xff\n'\xc3\xa9'";
    let range = |start, end| TextRange::new(TextSize::new(start), TextSize::new(end));
    let strings = [ValidatedString {
        range: range(3, 7),
        text: "'\u{e9}'",
    }];
    let source = FactSource::Bytes {
        bytes,
        strings: &strings,
    };
    assert_eq!(source.text(range(3, 7)).unwrap(), "'\u{e9}'");
    assert_eq!(source.text(range(4, 6)).unwrap(), "\u{e9}");
    for denied in [
        range(0, 2),
        range(0, 7),
        range(3, 5),
        range(5, 6),
        range(3, 8),
    ] {
        assert!(matches!(
            source.text(denied),
            Err(Error::Unsupported("literal source view"))
        ));
    }
}

#[test]
fn f3a_rejected_batch_retains_fixed_size_summaries_not_diagnostic_vectors() {
    let sources: Vec<_> = (0..12)
        .map(|index| {
            let mut item = source(b"x=)\ny=)\n");
            item.path = format!("unit{index}.py");
            item.module = format!("unit{index}");
            item
        })
        .collect();
    let (report, outcomes, trace) = bytes_with_trace(&sources);
    assert_eq!(outcomes.len(), sources.len());
    for outcome in outcomes {
        let ByteOutcome::Rejected {
            syntax_errors,
            unsupported_errors,
            stats,
        } = outcome
        else {
            panic!("fixed-size rejection summary required");
        };
        assert!(syntax_errors > 0);
        assert_eq!(unsupported_errors, 0);
        assert_eq!(stats.parse_invocations, 1);
    }
    assert!(trace.is_empty() && report.diagnostics.is_empty());
    assert!(report.facts.iter().all(|fact| fact.kind == "module"));
    assert!(report.files.iter().all(|file| file.failure.is_none()));
}

#[cfg(unix)]
#[test]
#[ignore = "requires explicitly selected trusted Python 3.12; original malformed-byte complete facts and raw replay"]
fn f3a_original_malformed_comment_facts_match_trusted_helper() {
    let bad: [&[u8]; 7] = [
        b"\x80",
        b"\xff",
        b"\xc2",
        b"\xc0\xaf",
        b"\xed\xa0\x80",
        b"\xf4\x90\x80\x80",
        b"\xe2(\xa1",
    ];
    let mut cases = Vec::new();
    for bad in bad {
        for (prefix, suffix) in [
            (b"#".as_slice(), b"\nvalue=call()\n".as_slice()),
            (b"if ready:\n    #", b"\n    call()\n"),
            (b"value=call() #", b""),
            (b"x=(one() #", b"\n + two())\n"),
            (b"x=f\"{call() #", b"\n}\"\n"),
            (b"x=f\"{call() #", b"\n=}\"\n"),
            (b"x=f\"{f'{inner() #", b"\n=}'=}\"\n"),
            (b"@api.get('/one' #", b"\n '/two')\ndef route(): body()\n"),
            (
                b"\xef\xbb\xbf#",
                b"\r\n@api.get('/ok')\r\ndef route(): body()\r\n",
            ),
        ] {
            cases.push(source(&[prefix, bad, suffix].concat()));
        }
    }
    assert_eq!(cases.len(), 63);
    super::ruff_facts_batch::assert_byte_oracle_sources(cases);
}
