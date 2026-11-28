//! F1-only lexer evidence; never production classification or fact dispatch.
use ruff_python_parser::{lexer::byte::legacy_trace, Mode};
use serde::Deserialize;
use sha2::{Digest, Sha256};

fn cases() -> Vec<(&'static str, Mode, &'static str)> {
    vec![
        ("module_basics", Mode::Module, "from pkg import item as alias\n@decorate\nasync def f(a: T = factory(), /, *, k=other()) -> R:\n    return await alias(a, k=k)\n"),
        ("unicode_names", Mode::Module, "K = λ + café\n变量 = K.attr[λ]\n"),
        ("number_forms", Mode::Module, "n = 0xff_ff + 0o17 + 0b10_01 + 12_345 + .25e-2 + 3.5j\n"),
        ("strings", Mode::Module, "a = r'raw\\text' b'bytes\\x41'\nb = '''triple\nline'''\nc = '\\N{SPACE}\\ud800'\n"),
        ("fstring_context", Mode::Module, "x = f\"outer{{literal}}{call(\"key\")!r:{width()}.{precision()}f}{f'{inner()=}'}\"\n"),
        ("debug_comments", Mode::Module, "x=f\"{(call() # quote \\\" brace }\n)=}\"\n"),
        ("indentation_newlines", Mode::Module, "if ready:\r\n\tfirst()\r\n\t# comment\r\n\tif nested:\r\n\t\tsecond()\r\nlast()\r"),
        ("bom_and_comments", Mode::Module, "\u{feff}# coding: utf-8\nvalue=(one() # text\n + two()) # eof"),
        ("operators", Mode::Module, "a := b\na += b ** c // d << e >> f @ g\nx = a != b <= c >= d == e and not f or g\n"),
        ("syntax_lexical_errors", Mode::Module, "x='unterminated\ny=0xg\nz = \\q\n"),
        ("expression", Mode::Expression, "(lambda a=first(), *, k=second(): a[k])({\"x\": 1})"),
        ("parenthesized", Mode::ParenthesizedExpression, "first +\n second[lower:upper:step]"),
        ("ipython", Mode::Ipython, "%time call()\nresult = !echo value\nname?\n"),
    ]
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Baseline {
    id: String,
    mode: String,
    source_sha256: String,
    trace_sha256: String,
    tokens: usize,
}

#[test]
fn f1_legacy_lexer_matches_independent_pre_refactor_traces() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ruff_python_parser::lexer::Lexer<'static>>();
    // Captured before changing the upstream cursor or lexer implementation.
    // Hashes cover complete ordered token kinds/ranges/flags/values and errors.
    let expected: Vec<Baseline> = serde_json::from_str(include_str!(
        "../../../docs/python-byte-lexer-baseline.json"
    ))
    .unwrap();
    let cases = cases();
    assert_eq!(expected.len(), cases.len());
    for (row, (id, mode, source)) in expected.iter().zip(cases) {
        assert_eq!(row.id, id);
        assert_eq!(row.mode, format!("{mode:?}"));
        assert_eq!(
            row.source_sha256,
            format!("{:x}", Sha256::digest(source.as_bytes()))
        );
        let trace = legacy_trace(source, mode);
        assert_eq!(row.tokens, trace.0.len(), "{id}");
        assert_eq!(
            row.trace_sha256,
            format!("{:x}", Sha256::digest(serde_json::to_vec(&trace).unwrap())),
            "{id}"
        );
    }
}
