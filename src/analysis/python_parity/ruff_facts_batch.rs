//! Ruff-specific complete-output and raw-observation gates. No target execution.
use super::*;
use serde_json::{json, Value};

fn source(text: impl AsRef<[u8]>) -> Source {
    let bytes = text.as_ref().to_vec();
    let hex = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    Source {
        path: "unit.py".into(),
        module: "unit".into(),
        bytes,
        hex,
    }
}

fn fixtures() -> Vec<String> {
    vec![
        "import package.submodule, other as alias\nfrom package.child import item as renamed, second\nfrom . import relative\nfrom ..parent import *\nfrom __future__ import annotations\n".into(),
        "def f(a: A=default(), /, b: B=other(), *v: V, k: K=keyword(), **w: W) -> result():\n    body()\n".into(),
        "async def f(a: annotation()=default(), *, k: keyword_annotation()=keyword_default()):\n    return await service(a)\n".into(),
        "class C(base(), metaclass=meta(), *bases(), key=key(), **options()):\n    field: annotation() = factory()\n    other: Type\n    def method(self):\n        return self.field()\n".into(),
        "def outer():\n    class Inner(Base):\n        async def method(self):\n            await call()\n    def nested():\n        return child()\n    tail()\nafter()\n".into(),
        "a = b = factory()\na, (b, *c) = factory()\nself.value = cls.inner.value = provider\n".into(),
        "a = receiver()[index()] = rhs()\nreceiver()[index()]: annotation() = rhs()\n".into(),
        "class C:\n    value: Type\n    value: Other = provider\n    self.value: Last = factory()\n".into(),
        "receiver()[index()] += rhs()\ndel receiver()[index()], other()[key()]\n".into(),
        "if first():\n    yes()\nelif second():\n    maybe()\nelse:\n    no()\n".into(),
        "while condition():\n    body()\n    break\nelse:\n    alternative()\nfor target()[index()] in iterable():\n    continue\nelse:\n    done()\n".into(),
        "async def f():\n    async for target()[index()] in iterable():\n        body()\n    else:\n        done()\n    async with context() as value:\n        use(value)\n".into(),
        "with first() as a, second() as target()[index()]:\n    body()\n".into(),
        "try:\n    body()\nexcept first() as exc:\n    handler()\nexcept second():\n    other()\nelse:\n    success()\nfinally:\n    cleanup()\n".into(),
        "try:\n    body()\nexcept* first() as exc:\n    handler()\nexcept* second():\n    other()\nfinally:\n    cleanup()\n".into(),
        "raise error() from cause()\nassert condition(), message()\ndef f():\n    global a, b\n    nonlocal c\n    pass\n    return\n".into(),
        "return result()\nbreak\ncontinue\nawait top()\nyield value()\nyield from values()\n".into(),
        "target(func(), *splat(), key=value(), **mapping())\n(factory())(item())\n(a if choose() else b)(value())\n".into(),
        "target((left() + right()) * divisor(), a @ b, c // d, e ** f, g << h, i >> j, k | l, m ^ n, o & p, q % r, s - t, u / v)\n".into(),
        "target(-negative(), +positive(), ~invert(), not truth(), a() and b() or c(), a() < b() <= c())\n".into(),
        "target([a(), b()], (c(), d()), {e(), f()}, {key1(): value1(), key2(): value2(), **mapping()})\n".into(),
        "target(value()[lower():upper():step()], value()[a(), b()], a if condition() else b())\n".into(),
        "target(lambda a=positional(), *, k=keyword(): body(a, k))\n".into(),
        "target([element() for obj.attr in iterable() if condition()], {element() for x in iterable()}, {key(): value() for x in iterable() if condition()})\n".into(),
        "target(element() for x in iterable() if condition())\nvalue = (\n    element() for x in iterable()\n)\n".into(),
        "async def f():\n    target([await element() async for x in iterable() if condition()])\n    return await target() ** power()\n".into(),
        "target((name := value()), (yield value()), (yield from values()))\n".into(),
        "target(f'{field():{width()}.{precision()}f}', f'{(name := value())}', f'{mapping[\"key\"]}')\n".into(),
        "target('REDACTION_PAYLOAD_71', b'REDACTION_PAYLOAD_71', 123456789, 2.5, 3j, True, False, None, ...)\n".into(),
        "from arbitrary import eval as probe\ndef f(a: probe()=probe(), *v: probe(), k: probe()=probe(), **w: probe()) -> probe():\n    from changed import exec as probe\n    probe()\nprobe()\n".into(),
        "from arbitrary import eval as probe\ndef repeat():\n    from changed import exec as probe\ndef repeat(a=probe()) -> probe():\n    probe()\nclass repeat(base=probe()):\n    probe()\n".into(),
        "from arbitrary import eval as probe\nprobe(); probe()\nprobe() + probe()\n".into(),
        "from arbitrary import eval as probe\ntarget({probe(): probe(), probe(): probe()})\ntarget(lambda a=probe(), *, k=probe(): probe())\na = b = probe()\n".into(),
        "def f(\n    a: annotation() = (\n        element() for x in iterable()\n    ),\n    *\n    varargs: annotation(),\n    **\n    kwargs: annotation()\n) -> result():\n    body()\n    # trailing comment is not the definition end\n".into(),
        "# Unicode comments are harmless: \u{03bb}\r\nclass C:\r\n    field: Type = factory()\r\n    # trailing comment\r\nafter()\r\n".into(),
        "def f():\r    return result()\rnext()\r".into(),
        format!("def many({}): pass\n", (0..129).map(|i| format!("p{i}")).collect::<Vec<_>>().join(",")),
        format!("target({})\n", (0..19).map(|i| format!("arg{i}()")).chain((0..19).map(|i| format!("key{i}=value{i}()"))).collect::<Vec<_>>().join(",")),
        "client.get('/REDACTION_PAYLOAD_71')\nopen('/not/a/real/output', 'w').write('REDACTION_PAYLOAD_71')\n".into(),
        "target(r'\\N{LATIN CAPITAL LETTER A}', b'\\N{LATIN CAPITAL LETTER A}', '\\\\N{LATIN CAPITAL LETTER A}', rf'\\N{{name}}{call()}')\n".into(),
        format!("target({}, hidden())\n", (0..17).map(|i| format!("a{i} + b{i} * c{i}")).collect::<Vec<_>>().join(",")),
        format!("{}.eval()\n", vec!["part"; 30].join(".")),
        format!("{}()\n", "identifier".repeat(80)),
        "before(earlier())\ndef f(arg=(\n    (element()) for x in iterable()\n)):\n    body()\n".into(),
        "def f(arg=(\n    (first(), second()) for x in (iterable())\n)):\n    body()\n".into(),
        "call() # type : int\ncall() #\u{000c}type: int\n".into(),
        "def f(a: A=first(), a: B=second(), *x: V, x: K=third(), **w: W):\n    body()\n".into(),
        "target(key=first(), **mapping(), key=second())\n".into(),
        "class C(key=first(), **mapping(), key=second()):\n    body()\n".into(),
    ]
}

#[test]
fn ruff_facts_cross_family_candidate_is_not_parse_only() {
    let cases = fixtures();
    assert!(cases.len() >= 30);
    for (index, text) in cases.iter().enumerate() {
        let sources = [source(text)];
        let report = extract_fact_candidates(&sources).unwrap();
        assert_eq!(
            report.files[0].status,
            FactStatus::AcceptedFacts,
            "case {index}: {:?}",
            report.files[0]
        );
        assert!(report.facts.len() > 1, "case {index}");
        assert!(report.files[0].nodes > 0);
        let encoded = serde_json::to_string(&report).unwrap();
        assert!(!encoded.contains("REDACTION_PAYLOAD_71"));
        assert!(!encoded.contains("pythonVersion"));
        assert_eq!(report.identity.policy, FACT_POLICY);
        assert_eq!(report.cpython_compatibility, CompatibilityStatus::Pending);
    }
}

#[test]
fn ruff_facts_whole_file_exclusions_are_transactional() {
    for text in [
        "before()\n@decorator\ndef f(): pass\n",
        "before()\nmatch value:\n    case _: pass\n",
        "before()\ntype Alias = int\n",
        "before()\ndef f[T](): pass\n",
        "before()\nclass C[T]: pass\n",
        "before()\napp = FastAPI()\n",
        "before()\nfrom fastapi import APIRouter as constructor\nr = constructor()\n",
        "before()\nimport fastapi as framework\nr = framework.APIRouter()\n",
        "before()\nreceiver.include_router(router)\n",
        "before()\nreceiver.prefix = value\n",
        "before()\nreceiver.router.child: Type\n",
        "before()\nreceiver.router += value\n",
        "before()\ndel receiver.prefix\n",
        "before()\nvalue = \u{212a}\n",
        "before()\npass # type: ignore\n",
        "before()\npass #type: int\n",
        "before()\npass #\t type: int\n",
        "before()\nvalue = '\\N{LATIN CAPITAL LETTER A}'\n",
        "before()\nvalue = f'\\N{LATIN CAPITAL LETTER A}{call()}'\n",
        "before()\nvalue = rf'{\"\\N{LATIN CAPITAL LETTER A}\"}'\n",
    ] {
        let sources = [source(text)];
        let mut trace = Vec::new();
        let report = extract_impl(
            &sources,
            |_| false,
            SemanticLimits::default(),
            Some(&mut trace),
        )
        .unwrap();
        assert_eq!(
            report.files[0].status,
            FactStatus::UnsupportedSyntax,
            "{text}"
        );
        assert_eq!(report.facts.len(), 1, "{text}");
        assert!(report.diagnostics.is_empty() && trace.is_empty());
    }
}

#[test]
fn ruff_facts_hidden_framework_guards_and_calls_survive_render_clips() {
    for hidden in ["FastAPI()", "receiver.include_router(router)"] {
        for text in [
            format!(
                "before()\ntarget({}, {hidden})\n",
                vec!["safe()"; 20].join(",")
            ),
            format!("before()\ndef f(a={{'key': [{hidden} for x in values]}}): pass\n"),
            format!("before()\ntarget(lambda: {hidden})\n"),
            format!("before()\ntarget(f'{{value:{{{hidden}}}}}')\n"),
        ] {
            let sources = [source(text)];
            let report = extract_fact_candidates(&sources).unwrap();
            assert_eq!(report.files[0].status, FactStatus::UnsupportedSyntax);
            assert_eq!(report.facts.len(), 1);
        }
    }
    let sources = [source(format!(
        "target({})\n",
        vec!["eval()"; 24].join(",")
    ))];
    let report = extract_fact_candidates(&sources).unwrap();
    assert_eq!(report.diagnostics.len(), 24);
    assert_eq!(report.facts.iter().filter(|f| f.kind == "call").count(), 2);
}

#[test]
fn ruff_facts_decimal_and_byte_admission_is_explicit() {
    for (digits, expected) in [
        ("1".repeat(4300), FactStatus::AcceptedFacts),
        ("1".repeat(4301), FactStatus::UnsupportedSyntax),
        ("0".repeat(4301), FactStatus::AcceptedFacts),
        (format!("0x{}", "f".repeat(4301)), FactStatus::AcceptedFacts),
        (vec!["1"; 4300].join("_"), FactStatus::AcceptedFacts),
        (format!("{}.0", "1".repeat(4301)), FactStatus::AcceptedFacts),
        (format!("{}j", "1".repeat(4301)), FactStatus::AcceptedFacts),
    ] {
        let sources = [source(format!("value = {digits}\n"))];
        assert_eq!(
            extract_fact_candidates(&sources).unwrap().files[0].status,
            expected
        );
    }
    for bytes in [b"# coding: utf-8\ncall()\n".as_slice(), b"call('\xff')\n"] {
        let sources = [source(bytes)];
        assert_eq!(
            extract_fact_candidates(&sources).unwrap().files[0].status,
            FactStatus::UnsupportedEncoding
        );
    }
    let mut input = source(b"\xef\xbb\xbfcall()\r\n");
    input.path = "caf\u{e9}/sample.py".into();
    input.module = "metadata.\u{03bb}".into();
    input.hex = "invalid legacy hex".into();
    let sources = [input];
    let report = extract_fact_candidates(&sources).unwrap();
    assert_eq!(report.files[0].status, FactStatus::AcceptedFacts);
    assert_eq!(
        report.files[0].sha256,
        crate::analysis::hash(&sources[0].bytes)
    );
    assert_eq!(
        report
            .facts
            .iter()
            .find(|f| f.kind == "call")
            .unwrap()
            .symbol,
        "metadata.\u{03bb}"
    );
}

#[test]
fn ruff_facts_file_isolation_resources_and_latched_cancellation() {
    let mut sources = [
        source("from arbitrary import eval as probe\nprobe()\n"),
        source("before()\n@bad\ndef f(): pass\n"),
        source("probe()\n"),
    ];
    for (i, s) in sources.iter_mut().enumerate() {
        s.path = format!("{i}.py");
    }
    let report = extract_fact_candidates(&sources).unwrap();
    assert_eq!(
        report.files.iter().map(|f| f.status).collect::<Vec<_>>(),
        [
            FactStatus::AcceptedFacts,
            FactStatus::UnsupportedSyntax,
            FactStatus::AcceptedFacts
        ]
    );
    assert_eq!(report.diagnostics.len(), 1);
    for point in [
        Checkpoint::Admission,
        Checkpoint::Identity,
        Checkpoint::Hash,
        Checkpoint::Decode,
        Checkpoint::Token,
        Checkpoint::PreParse,
        Checkpoint::PostParse,
        Checkpoint::Semantic,
        Checkpoint::PostConsume,
        Checkpoint::PostDrop,
        Checkpoint::Output,
        Checkpoint::Final,
    ] {
        let mut fired = false;
        let report = extract_with(&sources, |at| {
            let expire = !fired && at == point;
            fired |= expire;
            expire
        })
        .unwrap();
        assert!(fired, "{point:?}");
        assert_eq!(report.facts.len(), 3, "{point:?}");
        assert!(report.diagnostics.is_empty());
        assert!(report
            .files
            .iter()
            .all(|f| f.status == FactStatus::ResourceLimit));
    }
    let mut sources = [
        source("call()\n"),
        source("target(a(b(c(d()))))\n"),
        source("call()\n"),
    ];
    for (i, s) in sources.iter_mut().enumerate() {
        s.path = format!("{i}.py");
    }
    let report = extract_impl(
        &sources,
        |_| false,
        SemanticLimits {
            depth: 4,
            ..SemanticLimits::default()
        },
        None,
    )
    .unwrap();
    assert_eq!(
        report.files.iter().map(|f| f.status).collect::<Vec<_>>(),
        [
            FactStatus::AcceptedFacts,
            FactStatus::ResourceLimit,
            FactStatus::AcceptedFacts
        ]
    );
}

#[test]
fn ruff_facts_validated_callback_has_one_parse_and_no_error_channel_lowering() {
    for (text, expected) in [
        ("call()\n", 1),
        ("def f(:\n", 0),
        ("type Alias[T = int] = T\n", 0),
    ] {
        let sources = [source(text)];
        let mut parses = 0;
        let mut consumes = 0;
        let report = run_consumer(
            &sources,
            Limits::default(),
            |at| {
                parses += usize::from(at == Checkpoint::PreParse);
                false
            },
            &mut |_, _, parsed, _| {
                consumes += 1;
                assert!(parsed.errors().is_empty());
                assert!(parsed.unsupported_syntax_errors().is_empty());
            },
        )
        .unwrap();
        assert_eq!(parses, 1);
        assert_eq!(consumes, expected);
        assert_eq!(
            report.files[0].syntax == RuffSyntax::AcceptedByRuff,
            expected == 1
        );
    }
}

#[test]
fn ruff_facts_header_body_alias_order_and_persistent_path_scopes() {
    let sources = [source("from early import eval as probe\ndef repeat(a=probe()) -> probe():\n    from late import exec as probe\n    probe()\nclass repeat(base=probe()):\n    probe()\n")];
    let mut trace = Vec::new();
    let report = extract_impl(
        &sources,
        |_| false,
        SemanticLimits::default(),
        Some(&mut trace),
    )
    .unwrap();
    assert_eq!(report.files[0].status, FactStatus::AcceptedFacts);
    let warnings: Vec<_> = trace
        .iter()
        .filter_map(|e| match e {
            Observation::Warning { warning } => Some(warning.message.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        warnings,
        [
            "line 2: early.eval; runtime targets are not inferred",
            "line 4: late.exec; runtime targets are not inferred",
            "line 2: late.exec; runtime targets are not inferred",
            "line 5: late.exec; runtime targets are not inferred",
            "line 6: late.exec; runtime targets are not inferred"
        ]
    );
    assert!(report
        .facts
        .iter()
        .filter(|f| f.kind == "call")
        .all(|f| f.symbol == "unit.repeat"));
}

#[test]
fn ruff_facts_long_import_targets_are_not_clipped_or_recursively_substituted() {
    let package = (0..180).map(|_| "segment").collect::<Vec<_>>().join(".");
    let sources = [source(format!("from {package} import eval as probe\nprobe()\nfrom safe import read as other\nimport other as probe\nprobe()\n"))];
    let report = extract_fact_candidates(&sources).unwrap();
    assert_eq!(report.files[0].status, FactStatus::AcceptedFacts);
    assert_eq!(report.diagnostics.len(), 1);
    assert_eq!(
        report.diagnostics[0].message,
        format!("line 2: {package}.eval; runtime targets are not inferred")
    );
}

#[test]
fn ruff_facts_captured_replay_order_and_lambda_comprehension_owners() {
    let mut sources = [source("from arbitrary import eval as probe\ndef f():\n    target(lambda: probe(), [probe() for x in values], (probe() for x in values))\n"), source("other()\n")];
    sources[1].path = "other.py".into();
    let first = extract_fact_candidates(&sources).unwrap();
    assert_eq!(first.files[0].status, FactStatus::AcceptedFacts);
    assert_eq!(first.diagnostics.len(), 3);
    assert!(first
        .facts
        .iter()
        .filter(|f| f.kind == "call" && f.path == "unit.py")
        .all(|f| f.symbol == "unit.f"));
    let facts = serde_json::to_value(&first.facts).unwrap();
    let warnings = serde_json::to_value(&first.diagnostics).unwrap();
    sources.reverse();
    for source in &mut sources {
        source.hex = "not decoded or reopened".into();
    }
    let replay = extract_fact_candidates(&sources).unwrap();
    assert_eq!(serde_json::to_value(&replay.facts).unwrap(), facts);
    assert_eq!(serde_json::to_value(&replay.diagnostics).unwrap(), warnings);
}

#[test]
fn ruff_facts_header_schedule_reverses_capture_not_helper_replay() {
    use HeaderGroup::*;
    let groups = [
        PositionalOnlyAnnotations,
        PositionalAnnotations,
        VarargAnnotation,
        KeywordOnlyAnnotations,
        KeywordDefaults,
        KwargAnnotation,
        PositionalDefaults,
        ReturnAnnotation,
    ];
    let mut schedule = HeaderSchedule::default();
    for group in groups.into_iter().rev() {
        schedule
            .capture_visit(
                group,
                CallObservation {
                    line: group.index(),
                    end: group.index(),
                    value: format!("group{}()", group.index()),
                    callee: "unresolved".into(),
                },
                20,
                &mut |_| Ok(()),
            )
            .unwrap();
    }
    let mut observed = Vec::new();
    schedule
        .before(DefinitionKind::Function, &mut || Ok(()), &mut |event| {
            let HeaderEvent::Visit(call) = event else {
                panic!("unexpected metadata");
            };
            observed.push(call.line);
            Ok(())
        })
        .unwrap();
    assert_eq!(
        observed,
        groups[..7].iter().map(|g| g.index()).collect::<Vec<_>>()
    );
    schedule
        .after(DefinitionKind::Function, &mut || Ok(()), &mut |event| {
            let HeaderEvent::Visit(call) = event else {
                panic!("unexpected metadata");
            };
            observed.push(call.line);
            Ok(())
        })
        .unwrap();
    assert_eq!(
        observed,
        groups.iter().map(|g| g.index()).collect::<Vec<_>>()
    );
}

#[test]
fn ruff_facts_alias_dependent_constructor_guards_run_at_header_replay() {
    for (text, expected) in [
        ("from safe import read as probe\ndef f() -> probe():\n    from fastapi import APIRouter as probe\n", FactStatus::UnsupportedSyntax),
        ("from fastapi import APIRouter as probe\ndef f() -> probe():\n    from safe import read as probe\n", FactStatus::AcceptedFacts),
        ("from fastapi import APIRouter as probe\ndef f(a=probe()):\n    from safe import read as probe\n", FactStatus::UnsupportedSyntax),
    ] {
        let sources = [source(text)];
        let report = extract_fact_candidates(&sources).unwrap();
        assert_eq!(report.files[0].status, expected, "{text}");
    }
}

#[test]
fn ruff_facts_work_regions_bound_repeated_statements_and_control_bodies() {
    let calls = "    ordinary()\n".repeat(1000);
    let texts = [
        "ordinary()\n".repeat(1000),
        format!("if ready:\n{calls}"),
        format!("def owner(a=header()) -> result():\n{calls}"),
    ];
    for text in texts {
        let sources = [source(text)];
        let report = extract_impl(
            &sources,
            |_| false,
            SemanticLimits {
                working: 4 * 1024 * 1024,
                ..SemanticLimits::default()
            },
            None,
        )
        .unwrap();
        assert_eq!(
            report.files[0].status,
            FactStatus::AcceptedFacts,
            "{:?}",
            report.files[0]
        );
        assert_eq!(
            report
                .facts
                .iter()
                .filter(|f| f.kind == "call" && f.value == "ordinary()")
                .count(),
            1000
        );
    }
}

#[test]
fn ruff_facts_work_regions_release_long_alias_call_copies_inside_and_across_statements() {
    let module = "segment".repeat(4300);
    for (body, working) in [
        ("probe()\n".repeat(2000), 4 * 1024 * 1024),
        (
            format!("target({})\n", ["probe()"; 2000].join(",")),
            8 * 1024 * 1024,
        ),
    ] {
        // 2,000 transient 30-KiB resolutions exceed even the default budget if
        // retained, despite there being only one live projection target.
        let sources = [source(format!(
            "from {module} import read as probe\n{body}"
        ))];
        let report = extract_impl(
            &sources,
            |_| false,
            SemanticLimits {
                working,
                ..SemanticLimits::default()
            },
            None,
        )
        .unwrap();
        assert_eq!(
            report.files[0].status,
            FactStatus::AcceptedFacts,
            "{:?}",
            report.files[0]
        );
        assert!(report.diagnostics.is_empty());
        assert!(report.files[0].nodes > 4000);
    }
}

#[test]
fn ruff_facts_work_regions_preserve_shrinking_aliases_and_nested_owner_headers() {
    let module = "segment".repeat(4300);
    let text = format!("from {module} import read as probe\nif ready:\n    from tiny import exec as probe\n    probe()\nprobe()\ndef owner(a=probe()) -> probe():\n    from nested import eval as probe\n    def child(a=probe()) -> probe():\n        from leaf import exec as probe\n        probe()\n    probe()\ndef owner(a=probe()) -> probe():\n    probe()\nclass owner(Base, metaclass=probe()):\n    probe()\n");
    let sources = [source(text)];
    let mut trace = Vec::new();
    let report = extract_impl(
        &sources,
        |_| false,
        SemanticLimits {
            working: 4 * 1024 * 1024,
            ..SemanticLimits::default()
        },
        Some(&mut trace),
    )
    .unwrap();
    assert_eq!(
        report.files[0].status,
        FactStatus::AcceptedFacts,
        "{:?}",
        report.files[0]
    );
    let warnings: Vec<_> = trace
        .iter()
        .filter_map(|e| match e {
            Observation::Warning { warning } => Some(warning.message.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        warnings,
        [
            "line 4: tiny.exec; runtime targets are not inferred",
            "line 5: tiny.exec; runtime targets are not inferred",
            "line 6: tiny.exec; runtime targets are not inferred",
            "line 8: nested.eval; runtime targets are not inferred",
            "line 10: leaf.exec; runtime targets are not inferred",
            "line 8: leaf.exec; runtime targets are not inferred",
            "line 11: nested.eval; runtime targets are not inferred",
            "line 6: nested.eval; runtime targets are not inferred",
            "line 12: nested.eval; runtime targets are not inferred",
            "line 13: nested.eval; runtime targets are not inferred",
            "line 12: nested.eval; runtime targets are not inferred",
            "line 14: nested.eval; runtime targets are not inferred",
            "line 15: nested.eval; runtime targets are not inferred",
        ]
    );
}

#[test]
fn ruff_facts_work_regions_keep_only_live_file_state_and_owner_high_water() {
    let definitions = "    def child(a=probe()) -> probe():\n        probe()\n".repeat(80);
    let sources = [source(format!(
        "from package import read as probe\ndef owner(a=probe()) -> probe():\n{definitions}"
    ))];
    let mut sink = shared::FactSink::new(shared::SinkLimits {
        working: 4 * 1024 * 1024,
        ..shared::SinkLimits::default()
    });
    let initial = sink.working();
    let mut consumed = false;
    run_consumer(
        &sources,
        Limits::default(),
        |_| false,
        &mut |_, text, parsed, check| {
            let mut driver = Driver {
                source: &sources[0],
                sink: &mut sink,
                check,
                arena: Vec::new(),
                lines: Vec::new(),
                pairs: Vec::new(),
                projection: Projection::default(),
                scope: ScopeId::default(),
                owners: Vec::new(),
                persistent_work: 0,
                owner_slots: 0,
                arena_work: 0,
                nodes: 0,
                edges: 0,
                limits: SemanticLimits::default(),
                trace: None,
            };
            driver.consume(text, parsed).unwrap();
            assert!(driver.arena.is_empty() && driver.owners.is_empty());
            assert_eq!(driver.arena_work, 0);
            assert_eq!(driver.owner_slots, 2);
            let file_indexes = (text.len() + 1) * std::mem::size_of::<usize>()
                + driver.pairs.len() * 4 * std::mem::size_of::<Paren>();
            assert_eq!(
                driver.sink.working(),
                initial + file_indexes + driver.persistent_work
            );
            drop(driver);
            let pending = sink.working().checked_sub(initial).unwrap();
            sink.release(pending);
            assert_eq!(sink.working(), initial);
            consumed = true;
        },
    )
    .unwrap();
    assert!(consumed);
    assert!(!sink.facts().is_empty());
}

#[cfg(unix)]
#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12; Ruff complete facts and raw observation order"]
fn ruff_facts_complete_arrays_and_raw_trace_match_trusted_ast_oracle() {
    let python = crate::analysis::python_parity::oracle();
    let texts = fixtures();
    let mut sources: Vec<_> = texts.iter().map(source).collect();
    let mut metadata = source("class C:\n    field: Type = factory()\n");
    metadata.path = "caf\u{e9}/unit.py".into();
    metadata.module = "metadata.\u{03bb}".into();
    sources.push(metadata);
    sources.push(source(b"\xef\xbb\xbfcall()\r\n"));
    let helper = include_str!("../../python_extract.py");
    let (helper, autocall) = helper
        .rsplit_once("\nif __name__ == \"__main__\":")
        .unwrap();
    assert_eq!(autocall.trim(), "main()");
    let script = format!(
        "{helper}\n{}",
        r#"
assert sys.version_info >= (3, 12)
sys.set_int_max_str_digits(4300)
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
    let request = json!({"sources": sources.iter().map(|s| json!({"path":s.path,"module":s.module,"hex":s.hex})).collect::<Vec<_>>()});
    let mut command = std::process::Command::new(&python);
    command
        .args(["-I", "-S", "-c", &script])
        .current_dir("/")
        .env_clear()
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let output = crate::analysis::run_bounded(
        command,
        serde_json::to_vec(&request).unwrap(),
        "Python Ruff raw oracle",
    )
    .unwrap();
    let traces: Vec<Value> = serde_json::from_slice(&output).unwrap();
    assert_eq!(traces.len(), sources.len());
    for (index, (source, expected_trace)) in sources.iter().zip(traces).enumerate() {
        let captured = std::slice::from_ref(source);
        let mut trace = Vec::new();
        let native = extract_impl(
            captured,
            |_| false,
            SemanticLimits::default(),
            Some(&mut trace),
        )
        .unwrap();
        assert_eq!(
            native.files[0].status,
            FactStatus::AcceptedFacts,
            "case {index}"
        );
        // Module labels are batch preflight, not AST visits. Everything else,
        // including duplicate fact/warning occurrences, is compared unfiltered.
        assert_eq!(
            serde_json::to_value(&trace).unwrap(),
            expected_trace,
            "RAW_OBSERVATION case {index}"
        );
        let ast = crate::analysis::extract_captured(&python, captured).unwrap();
        assert!(ast.complete, "case {index}");
        assert_eq!(
            serde_json::to_value(&native.facts).unwrap(),
            serde_json::to_value(&ast.facts).unwrap(),
            "complete facts case {index}"
        );
        assert_eq!(
            serde_json::to_value(&native.diagnostics).unwrap(),
            serde_json::to_value(&ast.diagnostics).unwrap(),
            "complete warnings case {index}"
        );
    }
    eprintln!(
        "Ruff oracle: {} complete-array cases; {} raw-order cases",
        sources.len(),
        sources.len()
    );
}
