//! Ruff-specific complete-output and raw-observation gates. No target execution.
use super::*;
use serde_json::{json, Value};

pub(super) fn source(text: impl AsRef<[u8]>) -> Source {
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
        "match subject():\n    case constants.VALUE: body()\n    case None: other()\n    case True: yes()\n    case False: no()\n".into(),
        "match subject():\n    case [head, *tail]: body()\n    case (*_,): other()\n    case []: empty()\n".into(),
        "match subject():\n    case {'key': value, constants.KEY: [head, *tail], **rest}: body()\n".into(),
        "match subject():\n    case pkg.Record(item, flag=True, child=[head, *tail]): body()\n".into(),
        "match subject():\n    case ((1 | 2) as selected): body()\n    case capture: other()\n".into(),
        "match subject():\n    case [same, same, *first, *second]: body()\n    case left | right: other()\n".into(),
        "match subject():\n    case Record(key=first, key=second): body()\n    case {1: first, 1: second}: other()\n    case _: wildcard()\n    case capture: irrefutable()\n".into(),
        "match subject():\n    case f'{value():{width()}.{precision()}f}' if guard(): body()\n".into(),
        "match subject():\n    case {'' f'{key1()}': f'{value1()}', '' f'{key2()}': f'{value2()}'}: body()\n".into(),
        "from early import eval as probe\nmatch probe():\n    case FastAPI(probe, prefix=capture) if probe():\n        from late import exec as probe\n        probe()\n    case APIRouter(probe, tags=[*capture]) if probe(): probe()\nprobe()\n".into(),
        "match subject():\n    case Record(child=[head, *tail]) if guard():\n        match inner():\n            case {'key': (1 | 2) as selected, **rest}: body()\n".into(),
        "before()\ntype Alias = int\nafter()\n".into(),
        "type Alias[T: bound(), U: (left(), right()), *Ts, **P] = rhs()\n".into(),
        "from arbitrary import eval as probe\ntype probe[probe: probe()] = probe()\nprobe()\n".into(),
        "class C:\n    type Field[T: bound()] = rhs()\n    def method[U: method_bound()]():\n        type Local[V: local_bound()] = local_rhs()\n".into(),
        "match subject():\n    case capture:\n        type Alias[T: bound()] = rhs()\n        after()\n".into(),
        "def f[T](): pass\nclass C[T]: pass\n".into(),
        "def f[T: bound(), U: (left(), right()), *Ts, **P](a: annotation()=default()) -> result():\n    body()\n".into(),
        "async def f[T: bound(), *Ts, **P](a: annotation()=default(), *, k=keyword()) -> result():\n    await body()\n".into(),
        "class C[T: bound(), U: (left(), right()), *Ts, **P](base(), metaclass=meta(), key=keyword()):\n    body()\n".into(),
        "from early import eval as probe\ndef f[T: probe()](a=probe()) -> probe():\n    from late import exec as probe\n    probe()\n".into(),
        "from early import eval as probe\nclass C[T: probe()](probe(), metaclass=probe()):\n    from late import exec as probe\n    probe()\n".into(),
        "def outer[T: outer_bound()]():\n    def repeat[U: first_bound()]():\n        body()\n    def repeat[V: second_bound()]() -> result():\n        other()\n    class repeat[W: class_bound()](base()):\n        tail()\n".into(),
        "type Alias[T: (lambda: bound()), *Ts, **P] = [element() for x in iterable() if condition()]\n".into(),
        "type Alias[T: f'{bound():{width()}}'] = f'{rhs()}'\n".into(),
        "\u{feff}# harmless comment\r\nclass C[T: bound()]:\r\n    type Field[U: other()] = rhs()\r\n    # trailing comment\r\nafter()\r\n".into(),
    ]
}

// These are also fed, without filtering, through BOTH existing helper gates:
// final arrays/IDs and raw duplicate-preserving fact/warning visit order.
fn g3_fixtures() -> Vec<String> {
    vec![
        "@task\n@task()\n@worker.task()\n@worker.job()\n@worker.scheduled_job()\n@cli.command()\ndef f(): body()\n".into(),
        "@worker.task\n@job\n@command\n@get('/not-a-route')\n@client.getBody('/not-a-route')\ndef f(): client.get('/ordinary')\n".into(),
        "@cli.command()\n@api.get('/class')\nclass C(base()):\n    @task\n    @api.post('/method')\n    def method(self): body()\n".into(),
        "from early import eval as probe\n@probe()\n@api.route(path=path(), methods=methods())\ndef f[T: probe()](a=probe()) -> probe():\n    from late import exec as probe\n    probe()\n".into(),
        "@api.get('/a', extra(), methods=unknown())\n@api.post(path='')\n@api.put('/c')\n@api.patch('/d')\n@api.delete('/e')\n@api.options('/f')\n@api.head('/g')\n@api.trace('/h')\ndef f(): pass\n".into(),
        "@api.route(path='/' 'adjacent', methods=['post', 'GET', 'get'])\n@api.api_route('/tuple', methods=('head', 'trace'))\n@api.route('/set', methods={'put', 'patch'})\n@api.route('', methods=[])\ndef f(): pass\n".into(),
        "@api.route(path=unknown, methods=['get', unknown])\n@api.route('/a', path='/b', methods=['post'], methods=['get'])\n@api.route(methods='get')\n@api.route('/ok')\ndef f(): pass\n".into(),
        "@api.route('/a', methods=[*items])\n@api.route('/b', methods={**mapping})\n@api.route(*args, **options)\n@api.route(path=('/' + 'x'), methods=[f'get'])\n@api.route(path=b'/bytes', methods=[b'get'])\ndef f(): pass\n".into(),
        "@api.route('/unicode', methods=['opt\\u0131ons', 'po\\u017ft', 'po\\ufb05', 'po\\ufb06'])\ndef f(): pass\n".into(),
        "@api.route('/a', methods=['get', 'po\\u0130t'])\n@api.route('/b', methods=['\\u00df'])\n@api.route('/c', methods=['get '])\n@api.route('/d', methods=[''])\ndef f(): pass\n".into(),
        "@api.get('/\\ud800/\\udfff/\\U0010ffff/\\0/\\x01/\\t/\\n/\\r/\\b/\\f/\\v/\\a')\ndef f(): pass\n".into(),
        "@api.get(r'/raw\\n\\N{name}\\u0041')\n@api.get(u'/u\\101\\777\\q\\x42\\u0043\\U0001f600')\ndef f(): pass\n".into(),
        "@api.get(\n    '/first' # separate token\n    r'/raw\\n'\n    '''/triple\nline'''\n)\ndef f(): pass\n".into(),
        "@api.get('/continued\\\ntext')\n@api.get('''/physical\r\nline\rfinal''')\n@api.get('/\u{e9}/\u{1f600}')\ndef f(): pass\n".into(),
        format!("@api.get('/{}')\ndef f(): pass\n", "\\U0001f600".repeat(2047)),
        format!("@api.get('/{}')\ndef f(): pass\n", "\\U0001f600".repeat(2048)),
        format!("@api.get('/{}x')\n@api.get('/{}')\ndef f(): pass\n", "\\ud83d\\ude00".repeat(1023), "\\ud83d\\ude00".repeat(1024)),
        "\u{feff}@(\r\n    api.get('/wrapped')\r\n) # decorator tail\r\nasync \\\r\ndef f():\r\n    @task\r\n    def nested(): pass\r\n    # trailing comment\r\nafter()\r\n".into(),
        "a = b = FastAPI()\nr: Type = APIRouter(prefix='/v1', tags=secret())\nreceiver.field = FastAPI()\na, b = APIRouter(prefix='/ignored')\n".into(),
        "class C:\n    field: annotation() = APIRouter(prefix='/a')\n    self.router: annotation() = FastAPI()\n    cls.router.child = factory()\n".into(),
        "import fastapi as framework\nfrom fastapi import APIRouter as Router, FastAPI as Host\na = framework.FastAPI()\nb = Router(prefix='/b')\nc = Host()\n".into(),
        "from other import APIRouter\na = APIRouter(prefix='/no')\nfrom other import object as fastapi\nb = fastapi.FastAPI()\nc = unrelated.APIRouter(prefix='/yes')\n".into(),
        "import fastapi as first\nimport first as second\na = second.APIRouter(prefix='/no')\nb = first.APIRouter(prefix='/yes')\n".into(),
        "def repeat():\n    from fastapi import APIRouter as Router\ndef repeat():\n    a = Router(prefix='/persistent')\nclass repeat:\n    b = Router(prefix='/new-kind')\n".into(),
        format!("{} = FastAPI()\n{} = APIRouter(prefix='/ok')\n{} = FastAPI()\n{} = APIRouter(prefix='/ok')\n", "a".repeat(128), "b".repeat(128), "c".repeat(129), "d".repeat(129)),
        "a = APIRouter()\nb = APIRouter(prefix='')\nc = APIRouter(prefix='/a', prefix='/b')\nd = APIRouter(prefix=unknown)\ne = APIRouter(prefix=f'/dynamic{call()}')\n".into(),
        "a = APIRouter(prefix='/a', unknown=secret())\nb = APIRouter(prefix='/b', tags={'REDACTION_PAYLOAD_71': secret()})\nc = APIRouter(positional(), prefix='/c')\nd = APIRouter(prefix='/d', **options())\n".into(),
        "a = APIRouter(prefix='/a/b/c/d/e/f/g/h')\nb = APIRouter(prefix='/a/b/c/d/e/f/g/h/i')\nc = APIRouter(prefix='/UPPER')\nd = APIRouter(prefix='/\\u00e9')\ne = APIRouter(prefix='/{literal}_-9')\nf = APIRouter(prefix='/')\n".into(),
        format!("a = APIRouter(prefix='/{}')\nb = APIRouter(prefix='/{}')\nc = APIRouter(prefix='/{}')\n", "x".repeat(126), "x".repeat(127), "x".repeat(128)),
        "app.include_router()\napp.include_router(router)\napp.include_router(first, second)\napp.include_router(holder.router)\napp.include_router(factory())\n".into(),
        "app.include_router(router, prefix='')\napp.include_router(router, prefix='/ok')\napp.include_router(router, prefix='/a', prefix='/b')\napp.include_router(router, prefix=unknown)\napp.include_router(router, prefix='/ok', unknown=value())\napp.include_router(router, **options())\n".into(),
        "from early import eval as probe\n@app.include_router(router, prefix='/decorator')\ndef f[T: app.include_router(router)](a=app.include_router(router, prefix='/default')) -> app.include_router(probe()):\n    from late import exec as probe\n    body()\n".into(),
        "@app.include_router(router)\nclass C[T: app.include_router(router)](app.include_router(router), metaclass=app.include_router(router)):\n    body()\n".into(),
        "receiver().prefix = other().router = rhs()\nreceiver().router.child: annotation() = rhs()\nreceiver().prefix += rhs()\ndel receiver().prefix, other().router.child, tail()[key()]\n".into(),
        "del (\n    receiver().prefix,\n    other().router.child\n)\nself.router, cls.prefix = rhs()\nreceiver[probe()].prefix: Type\n".into(),
        format!("target({}, app.include_router(router, prefix='/hidden'))\n@api.get('/path', tags=[{}])\ndef f(): pass\n", vec!["safe()"; 90].join(","), vec!["child()"; 20].join(",")),
        "type Alias[T: FastAPI()] = receiver.include_router(router)\nmatch subject():\n    case f'{receiver.include_router(router)}' if FastAPI():\n        receiver.prefix = value\n".into(),
        "@api.get(f'/dynamic{unknown_symbol}')\n@api.route('/ok', methods=[unknown_symbol])\ndef f():\n    open('/not/a/real/output', 'w').write('REDACTION_PAYLOAD_71')\n    exec('REDACTION_PAYLOAD_71')\n".into(),
        "@task\nasync def outer():\n    @(cli.command())\n    async def inner(): pass\n    @api.get('/nested')\n    class Nested: pass\n".into(),
        "@api.get('/quotes\\\"/slash\\\\/\\ud83d\\ude00')\n@api.route('/ok', methods=['g' # comment\n'et', r'po' 'st'])\ndef f(): pass\n".into(),
    ]
}

#[test]
fn ruff_facts_g3_metadata_candidates_are_admitted_and_redacted() {
    let cases = g3_fixtures();
    assert_eq!(cases.len(), 40);
    for (index, text) in cases.iter().enumerate() {
        let sources = [source(text)];
        let report = extract_fact_candidates(&sources).unwrap();
        assert_eq!(
            report.files[0].status,
            FactStatus::AcceptedFacts,
            "G3 case {index}: {:?}",
            report.files[0]
        );
        assert!(!serde_json::to_string(&report)
            .unwrap()
            .contains("REDACTION_PAYLOAD_71"));
    }
}

#[test]
fn ruff_facts_g3_scalar_json_and_diagnostic_order_are_exact() {
    let sources = [source("@api.get('/\\ud800')\n@api.route(path=unknown, methods=['get', unknown])\ndef f(): body()\n")];
    let mut trace = Vec::new();
    let report = extract_impl(
        &sources,
        |_| false,
        SemanticLimits::default(),
        Some(&mut trace),
    )
    .unwrap();
    assert_eq!(report.files[0].status, FactStatus::AcceptedFacts);
    let routes: Vec<_> = trace
        .iter()
        .filter_map(|event| match event {
            Observation::Fact { fact } if fact.kind == "api_route" => Some(fact.value.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        routes,
        [
            r#"{"methods":["GET"],"path":"/\ud800","pathIsFragment":true,"receiver":"api"}"#,
            r#"{"methods":[],"path":null,"pathIsFragment":true,"receiver":"api"}"#,
        ]
    );
    let warnings: Vec<_> = trace
        .iter()
        .filter_map(|event| match event {
            Observation::Warning { warning } => Some(warning.message.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        warnings,
        [
            "line 2: route path is not a bounded direct slash-prefixed string; prefix not inferred",
            "line 2: route methods are not a literal HTTP whitelist; methods unknown",
        ]
    );
    assert!(report.diagnostics[0].message.contains("methods"));
    let declaration = report.facts.iter().find(|f| f.kind == "function").unwrap();
    assert_eq!((declaration.line, declaration.end_line), (3, 3));
}

#[test]
fn ruff_facts_g3_header_preludes_replay_before_calls_under_the_live_owner() {
    let sources = [source("from early import eval as probe\n@app.include_router(probe(), prefix='/decorator')\ndef f[T: app.include_router(router)](a=app.include_router(router, prefix='/default')) -> app.include_router(router, prefix='/return'):\n    from late import exec as probe\n    body()\n")];
    let mut trace = Vec::new();
    let report = extract_impl(
        &sources,
        |_| false,
        SemanticLimits::default(),
        Some(&mut trace),
    )
    .unwrap();
    assert_eq!(report.files[0].status, FactStatus::AcceptedFacts);
    let includes: Vec<_> = trace
        .iter()
        .enumerate()
        .filter_map(|(i, event)| match event {
            Observation::Fact { fact } if fact.kind == "router_include" => Some((i, fact)),
            _ => None,
        })
        .collect();
    assert_eq!(includes.len(), 4);
    for &(index, include) in &includes {
        let Observation::Fact { fact: call } = &trace[index + 1] else {
            panic!("missing call after prelude");
        };
        assert_eq!(call.kind, "call");
        assert_eq!(
            (&include.symbol, include.line, include.end_line),
            (&call.symbol, call.line, call.end_line)
        );
        assert_eq!(include.symbol, "unit.f");
    }
    assert!(includes[0].1.value.contains("/default"));
    assert!(includes[1].1.value.contains("/decorator"));
    assert!(includes[2].1.value.contains("/return"));
    assert!(includes[3].1.value.contains(r#""prefix":"""#));
    let body = trace.iter().position(|event| matches!(event, Observation::Fact { fact } if fact.kind == "call" && fact.value == "body()")).unwrap();
    assert!(includes[0].0 < body && body < includes[1].0);
    assert_eq!(report.diagnostics.len(), 1);
    assert_eq!(
        report.diagnostics[0].message,
        "line 2: late.exec; runtime targets are not inferred"
    );
}

#[test]
fn ruff_facts_g3_delete_emits_all_mutations_before_any_target_children() {
    let sources = [source(
        "del receiver().prefix, \\\n    other().router.child\n",
    )];
    let mut trace = Vec::new();
    let report = extract_impl(
        &sources,
        |_| false,
        SemanticLimits::default(),
        Some(&mut trace),
    )
    .unwrap();
    assert_eq!(report.files[0].status, FactStatus::AcceptedFacts);
    let facts: Vec<_> = trace
        .iter()
        .filter_map(|event| match event {
            Observation::Fact { fact } => Some(fact),
            _ => None,
        })
        .collect();
    assert_eq!(
        facts.iter().map(|f| f.kind.as_str()).collect::<Vec<_>>(),
        ["router_mutation", "router_mutation", "call", "call"]
    );
    assert_eq!((facts[0].line, facts[0].end_line), (1, 2));
    assert_eq!((facts[1].line, facts[1].end_line), (1, 2));
    assert_eq!(facts[0].value, r#"{"target":"receiver().prefix"}"#);
    assert_eq!(facts[1].value, r#"{"target":"other().router.child"}"#);
    // Parentheses instead make one tuple target. The helper does not recursively
    // infer mutation metadata for that target, but it still visits both calls.
    let sources = [source(
        "del (\n    receiver().prefix,\n    other().router.child\n)\n",
    )];
    let mut trace = Vec::new();
    let report = extract_impl(
        &sources,
        |_| false,
        SemanticLimits::default(),
        Some(&mut trace),
    )
    .unwrap();
    assert_eq!(report.files[0].status, FactStatus::AcceptedFacts);
    let kinds: Vec<_> = trace
        .iter()
        .filter_map(|event| match event {
            Observation::Fact { fact } => Some(fact.kind.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(kinds, ["call", "call"]);
}

#[test]
fn ruff_facts_g3_assignment_metadata_precedes_lifo_bindings_and_generic_calls() {
    let sources = [source("a = receiver().router = b = FastAPI()\nclass C:\n    field: annotation() = APIRouter(prefix='/a')\n")];
    let mut trace = Vec::new();
    let report = extract_impl(
        &sources,
        |_| false,
        SemanticLimits::default(),
        Some(&mut trace),
    )
    .unwrap();
    assert_eq!(report.files[0].status, FactStatus::AcceptedFacts);
    let facts: Vec<_> = trace
        .iter()
        .filter_map(|event| match event {
            Observation::Fact { fact } => Some(fact),
            _ => None,
        })
        .collect();
    assert_eq!(
        facts.iter().map(|f| f.kind.as_str()).collect::<Vec<_>>(),
        [
            "router_mutation",
            "router_host",
            "router_host",
            "binding",
            "binding",
            "call",
            "call",
            "class",
            "attribute_annotation",
            "router_prefix",
            "binding",
            "call",
            "call",
        ]
    );
    assert_eq!(facts[1].value, r#"{"constructor":"FastAPI","target":"a"}"#);
    assert_eq!(facts[2].value, r#"{"constructor":"FastAPI","target":"b"}"#);
    assert_eq!(facts[3].value, r#"{"source":"FastAPI()","target":"b"}"#);
    assert_eq!(facts[4].value, r#"{"source":"FastAPI()","target":"a"}"#);
}

#[test]
fn ruff_facts_g3_metadata_regions_preserve_live_headers_and_bound_repetition() {
    assert_eq!(SemanticLimits::default().working, 32 * 1024 * 1024);
    assert_eq!(OUTPUT, 32 * 1024 * 1024);
    let body = "    app.include_router(router, prefix='/body')\n".repeat(600);
    let sources = [source(format!("@app.include_router(router, prefix='/decorator')\ndef owner[T: app.include_router(router)](a=app.include_router(router, prefix='/default')) -> app.include_router(router):\n{body}"))];
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
    assert_eq!(
        report
            .facts
            .iter()
            .filter(|f| f.kind == "router_include")
            .count(),
        603
    );
    // Return and type-bound includes occupy the same line and produce the same
    // final ID, but both observations must survive until final fact deduplication.
    assert_eq!(
        trace
            .iter()
            .filter(|event| matches!(event,
        Observation::Fact { fact } if fact.kind == "router_include"))
            .count(),
        604
    );
    let limited = extract_impl(
        &sources,
        |_| false,
        SemanticLimits {
            working: 2048,
            ..SemanticLimits::default()
        },
        None,
    )
    .unwrap();
    assert_eq!(limited.files[0].status, FactStatus::ResourceLimit);
    assert_eq!(limited.facts.len(), 1);
    assert!(limited.diagnostics.is_empty());
    let mut checks = 0;
    let cancelled = extract_with(&sources, |at| {
        if at == Checkpoint::Semantic {
            checks += 1;
        }
        checks == 100
    })
    .unwrap();
    assert_eq!(checks, 100);
    assert_eq!(cancelled.facts.len(), 1);
    assert!(cancelled.diagnostics.is_empty());
    assert_eq!(cancelled.files[0].status, FactStatus::ResourceLimit);
}

#[test]
fn ruff_facts_cross_family_candidate_is_not_parse_only() {
    let cases = fixtures();
    assert_eq!(cases.len(), 75);
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
        "eval()\nmatch value:\n    case _: pass # type: int\n",
        "before()\npass #type: int\n",
        "before()\npass #\t type: int\n",
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
        assert_eq!(report.files[0].status, FactStatus::SyntaxError, "{text}");
        assert_eq!(report.facts.len(), 1, "{text}");
        assert!(report.diagnostics.is_empty() && trace.is_empty());
    }
}

#[test]
fn ruff_facts_hidden_framework_metadata_and_calls_survive_render_clips() {
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
            assert_eq!(report.files[0].status, FactStatus::AcceptedFacts);
            assert!(report
                .facts
                .iter()
                .any(|f| f.kind == "call" && f.value == hidden));
            assert_eq!(
                report
                    .facts
                    .iter()
                    .filter(|f| f.kind == "router_include")
                    .count(),
                usize::from(hidden.contains("include_router"))
            );
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
        ("1".repeat(4301), FactStatus::SyntaxError),
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
    for (bytes, expected) in [
        (
            b"# coding: utf-8\ncall()\n".as_slice(),
            FactStatus::AcceptedFacts,
        ),
        (b"call('\xff')\n".as_slice(), FactStatus::SyntaxError),
    ] {
        let sources = [source(bytes)];
        assert_eq!(
            extract_fact_candidates(&sources).unwrap().files[0].status,
            expected
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
        source("before()\npass # type: int\n"),
        source("def f[T: bound()]() -> result():\n    match subject():\n        case Record(child=[head, *tail]):\n            type Alias[U: other()] = rhs()\n"),
    ];
    for (i, s) in sources.iter_mut().enumerate() {
        s.path = format!("{i}.py");
    }
    let report = extract_fact_candidates(&sources).unwrap();
    assert_eq!(
        report.files.iter().map(|f| f.status).collect::<Vec<_>>(),
        [
            FactStatus::AcceptedFacts,
            FactStatus::SyntaxError,
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
        ("type Alias[*Ts = *tuple[int]] = int\n", 0),
        ("type Alias[**P = [int]] = int\n", 0),
        (
            "match subject():\n    case [capture]: body()\ntype Alias[T: bound()] =\n",
            0,
        ),
        ("type Alias[T: bound()] = rhs()\ndef f[U: other()](:\n", 0),
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
fn ruff_facts_type_parameter_defaults_are_explicit_py312_exclusions() {
    for parameter in ["T = int", "*Ts = *tuple[int]", "**P = [int]"] {
        for declaration in [
            format!("type Alias[{parameter}] = rhs()\n"),
            format!("def f[{parameter}](): body()\n"),
            format!("async def f[{parameter}](): body()\n"),
            format!("class C[{parameter}]: body()\n"),
        ] {
            let sources = [source(format!("eval()\n{declaration}"))];
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
                "{declaration}"
            );
            assert_eq!(report.facts.len(), 1);
            assert!(report.diagnostics.is_empty() && trace.is_empty());
            {
                assert_eq!(report.files[0].reason, Some("parser diagnostics"));
                let mut consumed = false;
                run_consumer(
                    &sources,
                    Limits::default(),
                    |_| false,
                    &mut |_, _, parsed, _| {
                        assert!(parsed.errors().is_empty());
                        assert!(parsed.unsupported_syntax_errors().is_empty());
                        consumed = true;
                    },
                )
                .unwrap();
                assert!(!consumed);
            }
        }
    }
}

#[test]
fn ruff_facts_bare_fstring_mapping_key_is_ast_only_without_recovery() {
    let sources = [source(
        "eval()\nmatch value:\n    case {f'{key()}': capture}: body()\n",
    )];
    let mut consumes = 0;
    let parsed = run_consumer(&sources, Limits::default(), |_| false, &mut |_, _, _, _| {
        consumes += 1;
    })
    .unwrap();
    assert_eq!(consumes, 1);
    assert_eq!(parsed.files[0].syntax, RuffSyntax::AcceptedByRuff);
    let mut trace = Vec::new();
    let report = extract_impl(
        &sources,
        |_| false,
        SemanticLimits::default(),
        Some(&mut trace),
    )
    .unwrap();
    assert_eq!(report.files[0].status, FactStatus::AcceptedFacts);
    assert!(report.facts.iter().any(|fact| fact.value == "key()"));
    assert!(!trace.is_empty());
    let text = std::str::from_utf8(&sources[0].bytes).unwrap();
    assert!(!parse_unchecked(text, ParseOptions::from(Mode::Module))
        .errors()
        .is_empty());
}

#[test]
fn ruff_facts_structural_neutral_nodes_and_depth_are_bounded_per_file() {
    for (text, limits) in [
        (
            "match value:\n    case [[[[capture]]]]: pass\n".to_string(),
            SemanticLimits {
                depth: 6,
                ..SemanticLimits::default()
            },
        ),
        (
            format!(
                "match value:\n    case [{}]: pass\n",
                vec!["_"; 40].join(",")
            ),
            SemanticLimits {
                nodes: 20,
                ..SemanticLimits::default()
            },
        ),
        (
            format!(
                "match value:\n    case Record({}): pass\n",
                (0..30)
                    .map(|i| format!("k{i}=_"))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            SemanticLimits {
                edges: 20,
                ..SemanticLimits::default()
            },
        ),
        (
            format!(
                "type Alias[{}] = int\n",
                (0..40)
                    .map(|i| format!("T{i}"))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            SemanticLimits {
                nodes: 20,
                ..SemanticLimits::default()
            },
        ),
        (
            "def f[T: bound() ](): pass\n".to_string(),
            SemanticLimits {
                depth: 3,
                ..SemanticLimits::default()
            },
        ),
    ] {
        let mut sources = [
            source("eval()\n"),
            source(format!("eval()\n{text}")),
            source("eval()\n"),
        ];
        for (i, source) in sources.iter_mut().enumerate() {
            source.path = format!("{i}.py");
        }
        let mut trace = Vec::new();
        let report = extract_impl(&sources, |_| false, limits, Some(&mut trace)).unwrap();
        assert_eq!(
            report.files.iter().map(|f| f.status).collect::<Vec<_>>(),
            [
                FactStatus::AcceptedFacts,
                FactStatus::ResourceLimit,
                FactStatus::AcceptedFacts
            ],
            "{text}"
        );
        assert_eq!(report.files[1].reason, Some("semantic resource limit"));
        assert_eq!(report.facts.iter().filter(|f| f.path == "1.py").count(), 1);
        assert_eq!(report.diagnostics.len(), 2);
        assert!(trace.iter().all(|event| match event {
            Observation::Fact { fact } => fact.path != "1.py",
            Observation::Warning { warning } => warning.path != "1.py",
        }));
    }
}

#[test]
fn ruff_facts_structural_hidden_metadata_survives_shape_caps() {
    let wide = vec!["safe()"; 90].join(",");
    let long = "identifier".repeat(80);
    for hidden in ["FastAPI()", "receiver.include_router(router)"] {
        for text in [
            format!("match subject:\n    case f'{{target({wide}, {hidden})}}': pass\n"),
            format!("type Alias[T: target({wide}, {hidden})] = int\n"),
            format!("def f[T: target({long}, {hidden})](): pass\n"),
            format!("class C[T: target({wide}, {hidden})]: pass\n"),
            format!("type Alias = target({wide}, {hidden})\n"),
        ] {
            let sources = [source(format!("eval()\n{text}"))];
            let mut trace = Vec::new();
            let report = extract_impl(
                &sources,
                |_| false,
                SemanticLimits::default(),
                Some(&mut trace),
            )
            .unwrap();
            assert_eq!(report.files[0].status, FactStatus::AcceptedFacts, "{text}");
            assert!(report
                .facts
                .iter()
                .any(|f| f.kind == "call" && f.value == hidden));
            assert_eq!(
                report
                    .facts
                    .iter()
                    .filter(|f| f.kind == "router_include")
                    .count(),
                usize::from(hidden.contains("include_router"))
            );
            assert!(!trace.is_empty());
        }
    }
    for text in [
        format!("match value:\n    case {}: pass\n", "1".repeat(4301)),
        format!("type Alias[T: {}] = int\n", "1".repeat(4301)),
    ] {
        let sources = [source(text)];
        let report = extract_fact_candidates(&sources).unwrap();
        assert_eq!(report.files[0].status, FactStatus::SyntaxError);
        assert_eq!(report.facts.len(), 1);
        assert!(report.diagnostics.is_empty());
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
        TypeParameters,
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
                    preceding: None,
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
fn ruff_facts_ordinary_header_constructors_do_not_invent_assignment_metadata() {
    for (text, expected) in [
        ("from safe import read as probe\ndef f() -> probe():\n    from fastapi import APIRouter as probe\n", FactStatus::AcceptedFacts),
        ("from fastapi import APIRouter as probe\ndef f() -> probe():\n    from safe import read as probe\n", FactStatus::AcceptedFacts),
        ("from fastapi import APIRouter as probe\ndef f(a=probe()):\n    from safe import read as probe\n", FactStatus::AcceptedFacts),
        ("from safe import read as probe\ndef f[T: probe()]():\n    from fastapi import APIRouter as probe\n", FactStatus::AcceptedFacts),
        ("from fastapi import APIRouter as probe\ndef f[T: probe()]():\n    from safe import read as probe\n", FactStatus::AcceptedFacts),
        ("from safe import read as probe\nclass C[T: probe()]:\n    from fastapi import APIRouter as probe\n", FactStatus::AcceptedFacts),
        ("from fastapi import APIRouter as probe\nclass C[T: probe()]:\n    from safe import read as probe\n", FactStatus::AcceptedFacts),
    ] {
        let sources = [source(text)];
        let report = extract_fact_candidates(&sources).unwrap();
        assert_eq!(report.files[0].status, expected, "{text}");
        assert!(!report.facts.iter().any(|f| matches!(f.kind.as_str(), "router_prefix" | "router_host")));
    }
}

#[test]
fn ruff_facts_work_regions_bound_repeated_statements_and_control_bodies() {
    let calls = "    ordinary()\n".repeat(1000);
    let texts = [
        "ordinary()\n".repeat(1000),
        format!("if ready:\n{calls}"),
        format!("def owner(a=header()) -> result():\n{calls}"),
        "type Alias[T: bound()] = ordinary()\n".repeat(1000),
        "def owner[T: bound()](): ordinary()\n".repeat(1000),
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
    let definitions = "    def child[T: probe()](a=probe()) -> probe():\n        type Alias[U: probe()] = probe()\n        probe()\n".repeat(80);
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
                source_text: text,
                sink: &mut sink,
                check,
                arena: Vec::new(),
                lines: Vec::new(),
                pairs: Vec::new(),
                keywords: Vec::new(),
                comments: None,
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
                + driver.pairs.len() * 4 * std::mem::size_of::<Paren>()
                + driver.keywords.len() * 4 * std::mem::size_of::<(TokenKind, TextRange)>()
                + driver
                    .comments
                    .as_ref()
                    .map_or(0, |plan| plan.working_charge);
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
    let mut texts = fixtures();
    texts.extend(g3_fixtures());
    let mut sources: Vec<_> = texts.iter().map(source).collect();
    let mut metadata = source("class C:\n    field: Type = factory()\n");
    metadata.path = "caf\u{e9}/unit.py".into();
    metadata.module = "metadata.\u{03bb}".into();
    sources.push(metadata);
    sources.push(source(b"\xef\xbb\xbfcall()\r\n"));
    assert_eq!(sources.len(), 117);
    assert_oracle_sources(sources);
}

#[cfg(unix)]
pub(super) fn assert_oracle_sources(sources: Vec<Source>) {
    let python = crate::analysis::python_parity::oracle();
    let helper = include_str!("../../python_extract.py");
    let (helper, autocall) = helper
        .rsplit_once("\nif __name__ == \"__main__\":")
        .unwrap();
    assert_eq!(autocall.trim(), "main()");
    let script = format!(
        "{helper}\n{}",
        r#"
import unicodedata
assert sys.version_info[:2] == (3, 12)
assert unicodedata.unidata_version == '15.0.0'
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
