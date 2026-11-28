//! Private F2 parser/source evidence, not CPython admission or production dispatch.
//! Source-backed debug equality is owner/range identity, not semantic equality.
//! Codec selection, contextual type comments and lazy exception precedence remain open.
use std::cell::Cell;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use ruff_python_ast::comparable::ComparableInterpolatedStringElement;
use ruff_python_ast::visitor::{self, Visitor};
use ruff_python_ast::{self as ast, DebugSourceRanges, DebugText, PythonVersion};
use ruff_python_parser::byte::{
    parse_module_checked, AcceptedByteModule, ByteLocation, ByteParseFailure, ByteParseLimits,
    ByteSourceError,
};
use ruff_python_parser::lexer::byte::{ByteLexFailure, ByteLexerLimits};
use ruff_python_parser::{parse_unchecked, Mode, ParseOptions, TokenKind};
use ruff_text_size::{Ranged, TextRange};
use sha2::{Digest, Sha256};

const BAD: [&[u8]; 7] = [
    b"\x80",
    b"\xff",
    b"\xc2",
    b"\xc0\xaf",
    b"\xed\xa0\x80",
    b"\xf4\x90\x80\x80",
    b"\xe2(\xa1",
];

fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(start.try_into().unwrap(), end.try_into().unwrap())
}

fn options() -> ParseOptions {
    ParseOptions::from(Mode::Module)
        .with_target_version(PythonVersion::PY312)
        .with_ast_only(true)
}

fn never_cancel() -> bool {
    false
}

fn accepted<'src, 'check>(
    source: &'src [u8],
    check: &'check mut dyn FnMut() -> bool,
) -> AcceptedByteModule<'src, 'check> {
    assert!(source.len() <= 4096, "bounded synthetic fixture");
    parse_module_checked(source, options(), ByteParseLimits::default(), check).unwrap()
}

#[derive(Default)]
struct Observations {
    debug: Vec<(DebugText, TextRange)>,
    elements: Vec<ast::InterpolatedStringElement>,
    calls: Vec<ast::ExprCall>,
}

impl<'a> Visitor<'a> for Observations {
    fn visit_expr(&mut self, expr: &'a ast::Expr) {
        if let ast::Expr::Call(call) = expr {
            self.calls.push(call.clone());
        }
        visitor::walk_expr(self, expr);
    }

    fn visit_interpolated_string_element(&mut self, element: &'a ast::InterpolatedStringElement) {
        if let ast::InterpolatedStringElement::Interpolation(value) = element {
            if let Some(debug) = &value.debug_text {
                self.debug.push((debug.clone(), value.expression.range()));
                self.elements.push(element.clone());
            }
        }
        visitor::walk_interpolated_string_element(self, element);
    }
}

fn observe(module: &ast::ModModule) -> Observations {
    let mut result = Observations::default();
    result.visit_body(&module.body);
    result
}

fn byte_observations(module: &AcceptedByteModule<'_, '_>) -> Observations {
    let mut result = None;
    module.with_module(|ast, tokens| {
        assert_eq!(tokens, module.tokens());
        result = Some(observe(ast));
    });
    result.unwrap()
}

fn source_ranges(module: &AcceptedByteModule<'_, '_>) -> Vec<DebugSourceRanges> {
    let observations = byte_observations(module);
    assert!(
        !observations.debug.is_empty(),
        "fixture must exercise Some(Source)"
    );
    observations
        .debug
        .into_iter()
        .map(|(debug, expression)| {
            let DebugText::Source(spans) = debug else {
                panic!("byte parser manufactured text gaps");
            };
            assert_eq!(spans.owner(), module.source_id());
            assert_eq!(spans.expression(), expression);
            assert_eq!(spans.leading().end(), expression.start());
            assert_eq!(expression.end(), spans.trailing().start());
            let parts = module.debug_bytes(&spans).unwrap();
            for (span, bytes) in [
                (spans.leading(), parts.leading),
                (expression, parts.expression),
                (spans.trailing(), parts.trailing),
            ] {
                let original =
                    &module.original_bytes()[span.start().to_usize()..span.end().to_usize()];
                assert_eq!(bytes, original);
                assert_eq!(bytes.as_ptr(), original.as_ptr());
            }
            spans
        })
        .collect()
}

fn differential(source: &str) {
    let legacy = parse_unchecked(source, options())
        .try_into_module()
        .unwrap();
    assert!(
        legacy.errors().is_empty(),
        "{source:?}: {:?}",
        legacy.errors()
    );
    assert!(legacy.unsupported_syntax_errors().is_empty());
    let mut check = never_cancel;
    let byte = accepted(source.as_bytes(), &mut check);
    assert!(byte_observations(&byte).debug.is_empty());
    let mut callbacks = 0;
    byte.with_module(|module, tokens| {
        callbacks += 1;
        assert_eq!(module, legacy.syntax(), "{source:?}");
        assert_eq!(tokens, legacy.tokens(), "all token kinds/ranges/flags");
    });
    assert_eq!(callbacks, 1);
    assert_eq!(byte.stats().parse_invocations, 1);
    assert!(byte.stats().parser_work > 0);
    assert!(byte.stats().parser_depth_peak <= ByteParseLimits::default().parser_depth);
}

fn failure(source: &[u8], limits: ByteParseLimits) -> ByteParseFailure {
    let mut check = never_cancel;
    let mut callbacks = 0;
    let error = match parse_module_checked(source, options(), limits, &mut check) {
        Ok(module) => {
            module.with_module(|_, _| callbacks += 1);
            panic!("unexpected acceptance ({callbacks} callback): {source:?}");
        }
        Err(error) => error,
    };
    assert_eq!(callbacks, 0);
    error
}

fn debug_differential(source: &str) {
    let legacy = parse_unchecked(source, options())
        .try_into_module()
        .unwrap();
    assert!(legacy.has_no_syntax_errors(), "{source:?}");
    let old = observe(legacy.syntax());
    let mut check = never_cancel;
    let byte = accepted(source.as_bytes(), &mut check);
    let new = byte_observations(&byte);
    let spans = source_ranges(&byte);
    assert_eq!(
        new.calls, old.calls,
        "call nodes and descendant visitation order"
    );
    assert_eq!(byte.tokens(), legacy.tokens());
    assert_eq!(old.debug.len(), spans.len());
    for ((text, expression), spans) in old.debug.iter().zip(spans) {
        let DebugText::Text { leading, trailing } = text else {
            panic!("legacy debug representation changed");
        };
        assert_eq!(*expression, spans.expression());
        let parts = byte.debug_bytes(&spans).unwrap();
        assert_eq!(leading.as_bytes(), parts.leading);
        assert_eq!(trailing.as_bytes(), parts.trailing);
    }
}

#[test]
fn f2_ascii_modules_match_full_legacy_ast_and_tokens() {
    for source in [
        "", "# comment\n", "value=1\n",
        "from pkg import item as alias\n@decorate\nasync def f(a: T = factory(), /, *, k=other()) -> R:\n    return await alias(a, k=k)\n",
        "n = 0xff_ff + 0o17 + 0b10_01 + 12_345 + .25e-2 + 3.5j\n",
        "if ready:\r\n\tfirst()\r\n\t# comment\r\n\tif nested:\r\n\t\tsecond()\r\nlast()\r",
        "type Alias[T] = list[T]\nmatch subject():\n    case [first, *rest] if guard():\n        use(first)\n",
    ] {
        differential(source);
    }
}

#[test]
fn f2_utf8_modules_match_full_legacy_ast_and_tokens() {
    for source in [
        "\u{212a} = \u{3bb} + caf\u{e9}\n\u{53d8}\u{91cf} = \u{212a}.attr[\u{3bb}]\n",
        "text='\u{e9}\u{4e2d}'\nraw=r'raw\\text'\nvalue='\\N{SPACE}\\ud800'\n",
        "x=f\"{call('key')!r:{width()}.{precision()}f}\"\n",
        "\u{feff}# coding: utf-8\nvalue=(one() # text\n + two()) # eof",
    ] {
        differential(source);
    }
}

#[test]
fn f2_malformed_comments_in_root_suites_eof_and_parentheses() {
    for bad in BAD {
        for (prefix, suffix) in [
            (b"#".as_slice(), b"\nx=1\n".as_slice()),
            (b"if True:\n    #", b"\n    call()\n"),
            (b"x=1 #", b""),
            (b"x=(one() #", b"\n + two())\n"),
            (b"x=f\"{call() #", b"\n}\"\n"),
        ] {
            let source = [prefix, bad, suffix].concat();
            let mut check = never_cancel;
            let module = accepted(&source, &mut check);
            let comments: Vec<_> = module
                .tokens()
                .iter()
                .filter(|token| token.kind() == TokenKind::Comment)
                .collect();
            assert_eq!(comments.len(), 1);
            let start = prefix.len() - 1;
            assert_eq!(comments[0].range(), range(start, prefix.len() + bad.len()));
            assert_eq!(
                module.source_bytes(comments[0].range()).unwrap(),
                [&b"#"[..], bad].concat()
            );
            assert!(std::ptr::eq(module.original_bytes(), source.as_slice()));
            assert_eq!(module.stats().parse_invocations, 1);
        }
    }
}

#[test]
fn f2_type_ignore_and_cookie_comments_are_only_lexically_opaque() {
    // Clean Ruff ASTs do not establish source-codec or type-comment admission.
    for source in [
        b"x=1 # type: \xff\n".as_slice(),
        b"x=1 # type: ignore[\xff]\n",
        b"# coding: utf8\n#\xff\nx=1\n",
    ] {
        let mut check = never_cancel;
        let module = accepted(source, &mut check);
        assert_eq!(module.original_bytes(), source);
    }
}

#[test]
fn f2_debug_simple_leading_parentheses_and_conversion_match_legacy_gaps() {
    for source in [
        "x=f'{call()=}'\n",
        "x=f'{  call() = }'\n",
        "x=f'{ (call()) =!r}'\n",
        "x=rf'{call() =!s}'\n",
    ] {
        debug_differential(source);
    }
}

#[test]
fn f2_debug_nested_and_format_spec_preserve_descendant_calls() {
    for source in [
        "x=f\"{f'{inner()=}'=}\"\n",
        "x=f'{outer(first(), second()) =!r:{width()}.{precision()}f}'\n",
        "x=f'{call()=:{width()=}}'\n",
        "x=f'{(one() # comment\n + two()) =}'\n",
    ] {
        debug_differential(source);
    }
}

#[test]
fn f2_debug_malformed_bytes_stay_in_exact_source_ranges() {
    for bad in BAD {
        for (prefix, suffix) in [
            (b"x=f\"{call() #".as_slice(), b"\n=}\"\n".as_slice()),
            (b"x=f\"{ (call() #", b"\n) =!r}\"\n"),
            (b"x=rf\"{call() #", b"\n=:{width()}}\"\n"),
            (b"x=f\"{f'{inner() #", b"\n=}'=}\"\n"),
        ] {
            let source = [prefix, bad, suffix].concat();
            let mut check = never_cancel;
            let module = accepted(&source, &mut check);
            let spans = source_ranges(&module);
            assert!(spans.iter().any(|spans| {
                let parts = module.debug_bytes(spans).unwrap();
                [parts.leading, parts.expression, parts.trailing]
                    .iter()
                    .any(|part| part.windows(bad.len()).any(|window| window == bad))
            }));
        }
    }
}

#[test]
fn f2_whole_debug_expression_can_contain_malformed_comment() {
    let source = b"x=f'{(one() #\xff\n + two()) =}'\n";
    let mut check = never_cancel;
    let mut module = accepted(source, &mut check);
    let spans = source_ranges(&module);
    assert_eq!(spans.len(), 1);
    let parts = module.debug_bytes(&spans[0]).unwrap();
    assert_eq!(parts.expression, b"one() #\xff\n + two()");
    assert_eq!(
        module.source_text(spans[0].expression()),
        Err(ByteSourceError::InvalidUtf8)
    );
    let calls = byte_observations(&module).calls;
    assert_eq!(calls.len(), 2);
    assert_eq!(module.source_bytes(calls[0].range()).unwrap(), b"one()");
    assert_eq!(module.source_bytes(calls[1].range()).unwrap(), b"two()");
}

#[test]
fn f2_bom_debug_and_tokens_keep_original_offsets() {
    let source = b"\xef\xbb\xbfx=f'{call() #\xff\r\n=}'\r\n";
    let mut check = never_cancel;
    let module = accepted(source, &mut check);
    assert_eq!(module.tokens().iter().next().unwrap().range(), range(3, 4));
    let spans = source_ranges(&module);
    assert_eq!(spans[0].expression(), range(8, 14));
    assert_eq!(
        module.location(3.into()).unwrap(),
        ByteLocation { line: 1, column: 0 }
    );
    assert_eq!(
        module.location(8.into()).unwrap(),
        ByteLocation { line: 1, column: 5 }
    );
    assert_eq!(
        module.location(20.into()).unwrap(),
        ByteLocation { line: 2, column: 1 }
    );
    assert_eq!(
        module.location(0.into()),
        Err(ByteSourceError::InvalidRange)
    );
}

#[test]
fn f2_locations_use_byte_columns_and_cr_lf_crlf_lines() {
    let source = "\u{e9}=call()\rnext()\nlast()\r\ntail()\n";
    let mut check = never_cancel;
    let module = accepted(source.as_bytes(), &mut check);
    for (offset, line, column) in [(3, 1, 3), (10, 2, 0), (17, 3, 0), (25, 4, 0), (32, 5, 0)] {
        assert_eq!(
            module.location(offset.into()).unwrap(),
            ByteLocation { line, column }
        );
    }
    assert_eq!(
        module.location(33.into()),
        Err(ByteSourceError::InvalidRange)
    );
}

#[test]
fn f2_single_bad_comment_byte_has_width_one() {
    let mut check = never_cancel;
    let module = accepted(b"#\xff\nx=1\n", &mut check);
    assert_eq!(module.source_bytes(range(1, 2)).unwrap(), b"\xff");
    assert_eq!(
        module.location(2.into()).unwrap(),
        ByteLocation { line: 1, column: 2 }
    );
    assert_eq!(
        module.location(3.into()).unwrap(),
        ByteLocation { line: 2, column: 0 }
    );
}

#[test]
fn f2_debug_range_constructor_checks_adjacency_and_bounds() {
    let owner = ast::DebugSourceId::fresh().unwrap();
    assert!(
        DebugSourceRanges::new(owner, range(0, 1), range(1, 3), range(3, 4), 4.into()).is_some()
    );
    for (leading, expression, trailing, len) in [
        (range(0, 1), range(2, 3), range(3, 4), 4),
        (range(0, 1), range(1, 2), range(3, 4), 4),
        (range(0, 1), range(1, 3), range(3, 5), 4),
    ] {
        assert!(DebugSourceRanges::new(owner, leading, expression, trailing, len.into()).is_none());
    }
}

#[test]
fn f2_debug_views_reject_foreign_owner_and_forged_buffer_bound() {
    let mut a_check = never_cancel;
    let mut b_check = never_cancel;
    let a = accepted(b"x=f'{call()=}'", &mut a_check);
    let b = accepted(b"x=f'{call()=}'", &mut b_check);
    let spans = source_ranges(&a).remove(0);
    assert_eq!(
        b.debug_bytes(&spans).unwrap_err(),
        ByteSourceError::ForeignOwner
    );
    let forged = DebugSourceRanges::new(
        a.source_id(),
        range(0, 0),
        range(0, 100),
        range(100, 100),
        100.into(),
    )
    .unwrap();
    assert_eq!(
        a.debug_bytes(&forged).unwrap_err(),
        ByteSourceError::InvalidRange
    );
}

fn hash(value: &impl Hash) -> u64 {
    let mut state = DefaultHasher::new();
    value.hash(&mut state);
    state.finish()
}

#[test]
fn f2_debug_equality_and_comparable_hash_are_identity_only() {
    let source = b"x=f'{call()=}'";
    let copied = source.to_vec();
    let mut a_check = never_cancel;
    let mut b_check = never_cancel;
    let mut c_check = never_cancel;
    let a = accepted(source, &mut a_check);
    let b = accepted(source, &mut b_check);
    let c = accepted(&copied, &mut c_check);
    let a_obs = byte_observations(&a);
    let a_spans = source_ranges(&a).remove(0);
    for other in [&b, &c] {
        let other_obs = byte_observations(other);
        let other_spans = source_ranges(other).remove(0);
        assert_eq!(a_spans.leading(), other_spans.leading());
        assert_eq!(a_spans.expression(), other_spans.expression());
        assert_eq!(a_spans.trailing(), other_spans.trailing());
        assert_ne!(a_spans, other_spans);
        assert_ne!(a.source_id(), other.source_id());
        assert_ne!(a_obs.debug[0].0, other_obs.debug[0].0);
        assert_ne!(
            ComparableInterpolatedStringElement::from(&a_obs.elements[0]),
            ComparableInterpolatedStringElement::from(&other_obs.elements[0])
        );
    }
    let cloned = a_obs.elements[0].clone();
    let left = ComparableInterpolatedStringElement::from(&a_obs.elements[0]);
    let right = ComparableInterpolatedStringElement::from(&cloned);
    assert_eq!(left, right);
    assert_eq!(hash(&left), hash(&right));
    assert_eq!(hash(&a_obs.debug[0].0), hash(&a_obs.debug[0].0.clone()));
    let text = DebugText::Text {
        leading: String::new(),
        trailing: "=".into(),
    };
    assert_eq!(text, text.clone());
    assert_eq!(hash(&text), hash(&text.clone()));
    assert_ne!(a_obs.debug[0].0, text);
    let legacy = parse_unchecked(std::str::from_utf8(source).unwrap(), options())
        .try_into_module()
        .unwrap();
    let old = observe(legacy.syntax());
    assert_ne!(
        left,
        ComparableInterpolatedStringElement::from(&old.elements[0])
    );
    // Unequal values need not have different hashes. No semantic hash claim here.
}

#[test]
fn f2_invalid_utf8_views_are_local_nonpoisoning_and_zero_copy() {
    let source = b"#\xff\nname='\xc3\xa9'\n";
    let mut check = never_cancel;
    let mut module = accepted(source, &mut check);
    assert_eq!(
        module.source_text(range(0, 2)),
        Err(ByteSourceError::InvalidUtf8)
    );
    assert_eq!(
        module.source_text(range(9, 10)),
        Err(ByteSourceError::InvalidUtf8)
    );
    assert_eq!(module.source_text(range(9, 11)).unwrap(), "\u{e9}");
    let name = module.source_text(range(3, 7)).unwrap();
    assert_eq!(name, "name");
    assert_eq!(name.as_ptr(), source[3..].as_ptr());
    assert_eq!(
        module.source_bytes(range(0, source.len() + 1)),
        Err(ByteSourceError::InvalidRange)
    );
    assert_eq!(module.source_text(range(3, 7)).unwrap(), "name");
    module.with_module(|module, _| assert_eq!(module.body.len(), 1));
}

#[test]
fn f2_postparse_cancellation_is_retained_without_reset() {
    let stop = Cell::new(false);
    let mut check = || stop.get();
    let mut module = accepted(b"name=1\n", &mut check);
    let before = module.stats();
    stop.set(true);
    assert_eq!(
        module.source_text(range(0, 4)),
        Err(ByteSourceError::Cancelled)
    );
    stop.set(false);
    assert_eq!(
        module.source_text(range(0, 4)),
        Err(ByteSourceError::Cancelled)
    );
    assert_eq!(
        module.source_bytes(range(0, 4)),
        Err(ByteSourceError::Cancelled)
    );
    assert_eq!(module.location(0.into()), Err(ByteSourceError::Cancelled));
    assert!(module.stats().lexer.work >= before.lexer.work);
    assert_eq!(module.stats().parse_invocations, 1);
}

#[test]
fn f2_postparse_views_exhaust_retained_work_and_keep_monotonic_stats() {
    let source = b"name=1\n";
    let mut check = never_cancel;
    let mut module = parse_module_checked(
        source,
        options(),
        ByteParseLimits {
            lexer: ByteLexerLimits {
                work: 4096,
                ..ByteLexerLimits::default()
            },
            ..ByteParseLimits::default()
        },
        &mut check,
    )
    .unwrap();
    let mut before = module.stats();
    let mut exhausted = false;
    for _ in 0..4096 {
        let view = module.source_text(range(0, source.len()));
        let after = module.stats();
        assert!(after.lexer.work >= before.lexer.work);
        assert!(after.lexer.tokens >= before.lexer.tokens);
        assert!(after.lexer.allocated >= before.lexer.allocated);
        assert!(after.lexer.checkpoints >= before.lexer.checkpoints);
        assert_eq!(after.parser_work, before.parser_work);
        assert_eq!(after.parse_invocations, 1);
        before = after;
        if view == Err(ByteSourceError::ResourceLimit) {
            exhausted = true;
            break;
        }
        assert_eq!(view.unwrap().as_bytes(), source);
    }
    assert!(exhausted);
    assert_eq!(
        module.source_text(range(0, 1)),
        Err(ByteSourceError::ResourceLimit)
    );
    assert_eq!(
        module.source_bytes(range(0, 1)),
        Err(ByteSourceError::ResourceLimit)
    );
}

#[test]
fn f2_raw_hash_and_pointer_include_bom_and_malformed_comments() {
    let source = b"\xef\xbb\xbf#\xff\r\nx=1\r\n";
    let expected = Sha256::digest(source);
    let mut check = never_cancel;
    let module = accepted(source, &mut check);
    assert!(std::ptr::eq(module.original_bytes(), source.as_slice()));
    assert_eq!(Sha256::digest(module.original_bytes()), expected);
    assert_ne!(Sha256::digest(&source[3..]), expected);
}

#[test]
fn f2_typed_source_faults_in_names_strings_outer_and_format_text() {
    for bad in BAD {
        for (prefix, suffix) in [
            (b"name".as_slice(), b"=1\n".as_slice()),
            (b"x='", b"'\n"),
            (b"x=b'", b"'\n"),
            (b"x=f'outer", b"{1}'\n"),
            (b"x=f'{1:#", b"}'\n"),
            (b"x='", b""),
            (b"x='''", b""),
            (b"x=f'outer", b""),
        ] {
            let source = [prefix, bad, suffix].concat();
            let error = failure(&source, ByteParseLimits::default());
            let ByteParseFailure::Input(ByteLexFailure::Source(fault)) = error else {
                panic!("expected typed source failure, got {error:?}");
            };
            assert_eq!(fault.range.start().to_usize(), prefix.len());
            assert!(fault.range.end().to_usize() <= prefix.len() + bad.len());
            assert!(!fault.range.is_empty());
        }
    }
}

#[test]
fn f2_syntax_and_ordinary_lexical_errors_reject_without_ast() {
    for source in [
        b"x=)\n".as_slice(),
        b"x='unterminated\n",
        b"x=0xg\n",
        b"x=\\q\n",
        b"(one()\nnext()\n",
    ] {
        let error = failure(source, ByteParseLimits::default());
        let ByteParseFailure::Rejected { errors, .. } = error else {
            panic!("expected diagnostics-only rejection, got {error:?}");
        };
        assert!(!errors.is_empty());
    }
    // Rejections carry bounded diagnostics and counters, never a recovery AST.
}

#[test]
fn f2_version_only_diagnostics_reject_while_legacy_convenience_stays_ok() {
    let source = "type Alias[T = int] = T\n";
    let legacy = parse_unchecked(source, options());
    assert!(legacy.errors().is_empty());
    assert!(!legacy.unsupported_syntax_errors().is_empty());
    assert!(ruff_python_parser::parse(source, options()).is_ok());
    let ByteParseFailure::Rejected {
        errors,
        unsupported,
        ..
    } = failure(source.as_bytes(), ByteParseLimits::default())
    else {
        panic!("version-only errors must reject byte AST access");
    };
    assert!(errors.is_empty());
    assert!(!unsupported.is_empty());
}

#[test]
fn f2_nonmodule_modes_reject_before_cancellation_poll() {
    for mode in [
        Mode::Expression,
        Mode::ParenthesizedExpression,
        Mode::Ipython,
    ] {
        let polls = Cell::new(0);
        let mut check = || {
            polls.set(polls.get() + 1);
            true
        };
        assert!(matches!(
            parse_module_checked(
                b"1",
                ParseOptions::from(mode),
                ByteParseLimits::default(),
                &mut check
            ),
            Err(ByteParseFailure::UnsupportedMode)
        ));
        assert_eq!(polls.get(), 0);
    }
}

#[test]
fn f2_tiny_parser_work_depth_diagnostics_and_working_limits() {
    for (source, limits) in [
        (
            b"x=1\n".as_slice(),
            ByteParseLimits {
                parser_work: 0,
                ..ByteParseLimits::default()
            },
        ),
        (
            b"x=1\n",
            ByteParseLimits {
                parser_depth: 0,
                ..ByteParseLimits::default()
            },
        ),
        (
            b"x=)\n",
            ByteParseLimits {
                parser_diagnostics: 0,
                ..ByteParseLimits::default()
            },
        ),
        (
            b"x=1\n",
            ByteParseLimits {
                parser_working: 0,
                ..ByteParseLimits::default()
            },
        ),
    ] {
        assert_eq!(
            failure(source, limits),
            ByteParseFailure::Input(ByteLexFailure::ResourceLimit)
        );
    }
}

#[test]
fn f2_tiny_lexer_checkpoint_and_token_limits_propagate() {
    for (source, lexer) in [
        (
            b"with (manager()) as value:\n    pass\n".as_slice(),
            ByteLexerLimits {
                checkpoints: 0,
                ..ByteLexerLimits::default()
            },
        ),
        (
            b"match = value\n",
            ByteLexerLimits {
                tokens: 1,
                ..ByteLexerLimits::default()
            },
        ),
        (
            b"x=1\n",
            ByteLexerLimits {
                work: 0,
                ..ByteLexerLimits::default()
            },
        ),
    ] {
        assert_eq!(
            failure(
                source,
                ByteParseLimits {
                    lexer,
                    ..ByteParseLimits::default()
                }
            ),
            ByteParseFailure::Input(ByteLexFailure::ResourceLimit)
        );
    }
}

#[test]
fn f2_bounded_recursive_and_reduction_chains_hit_depth_guard() {
    // Keep fixtures small even if a missing guard regresses. Exercise recursive
    // grammar and iterative reductions that otherwise construct deep boxed ASTs.
    for source in [
        format!("x={}value\n", "-".repeat(64)),
        format!("x={}value\n", "lambda: ".repeat(48)),
        format!("x=value{}\n", ".attr".repeat(64)),
        format!("x=value{}\n", "+value".repeat(64)),
        format!("x={}value{}\n", "call(".repeat(48), ")".repeat(48)),
        format!("x={}value{}\n", "[".repeat(48), "]".repeat(48)),
    ] {
        assert!(source.len() < 1024);
        assert_eq!(
            failure(
                source.as_bytes(),
                ByteParseLimits {
                    parser_depth: 16,
                    ..ByteParseLimits::default()
                }
            ),
            ByteParseFailure::Input(ByteLexFailure::ResourceLimit),
            "{source}"
        );
    }
}

#[test]
fn f2_composed_subtrees_cannot_reset_construction_allowance() {
    let limits = ByteParseLimits {
        parser_depth: 64,
        ..ByteParseLimits::default()
    };
    for source in [
        format!("x=value{}{}\n", ".a".repeat(40), "+value".repeat(32)),
        format!(
            "x=((value{}){}){}\n",
            ".a".repeat(24),
            ".a".repeat(24),
            ".a".repeat(24)
        ),
    ] {
        assert_eq!(
            failure(source.as_bytes(), limits),
            ByteParseFailure::Input(ByteLexFailure::ResourceLimit)
        );
    }
    for source in [
        format!("x=value{}\n", ".a".repeat(10)),
        format!("x=value{}\n", "+value".repeat(2)),
    ] {
        let mut check = never_cancel;
        let module =
            parse_module_checked(source.as_bytes(), options(), limits, &mut check).unwrap();
        assert!(module.stats().construction_entries_peak <= 64);
    }
}

#[test]
fn f2_only_top_level_sibling_completion_resets_construction_allowance() {
    let limits = ByteParseLimits {
        parser_depth: 64,
        ..ByteParseLimits::default()
    };
    let siblings = "value=1\n".repeat(24);
    let mut check = never_cancel;
    let module = parse_module_checked(siblings.as_bytes(), options(), limits, &mut check).unwrap();
    module.with_module(|module, _| assert_eq!(module.body.len(), 24));
    assert!(module.stats().parser_work > 64);
    assert!(module.stats().construction_entries_peak <= 64);
    let nested = format!("if ready:\n{}", "    value=1\n".repeat(24));
    assert_eq!(
        failure(nested.as_bytes(), limits),
        ByteParseFailure::Input(ByteLexFailure::ResourceLimit)
    );
    assert!(matches!(
        failure(b"x=value.a.a+value\ny='\xff'\n", limits),
        ByteParseFailure::Input(ByteLexFailure::Source(_))
    ));
}

#[test]
fn f2_recovery_pattern_conversion_is_bounded_before_folding_or_patterns() {
    let large = format!("match x:\n case ({}1)():pass\n", "1|".repeat(511));
    assert_eq!(
        failure(large.as_bytes(), ByteParseLimits::default()),
        ByteParseFailure::Input(ByteLexFailure::ResourceLimit)
    );
    assert!(matches!(
        failure(
            b"match x:\n case (1|2)():pass\n",
            ByteParseLimits::default()
        ),
        ByteParseFailure::Rejected { .. }
    ));
}

#[test]
fn f2_recovery_relex_is_exercised_and_rejection_never_exposes_ast() {
    let ByteParseFailure::Rejected { stats, errors, .. } =
        failure(b"x=[one()\ndef next():pass\n", ByteParseLimits::default())
    else {
        panic!("expected bounded diagnostics-only rejection");
    };
    assert!(!errors.is_empty());
    assert_eq!(stats.parse_invocations, 1);
    assert!(stats.relex_requests > 0);
    assert!(stats.relex_applied > 0);
}

#[test]
fn f2_checkpoint_restores_token_prefix_removed_by_recovery_relex() {
    use ruff_python_parser::byte::token_checkpoint_observation;
    // The newline/comment precede `next`, but become removable history when
    // re-lex resets the unfinished parenthesized logical line.
    let source = b"(one()\n#\xff\nnext()\n";
    let mut check = never_cancel;
    let observation =
        token_checkpoint_observation(source, 4, ByteParseLimits::default(), &mut check).unwrap();
    assert!(
        observation.before.len() > observation.relexed.len(),
        "fixture must actually remove pre-checkpoint history"
    );
    assert_eq!(observation.before, observation.restored);
    assert_eq!(observation.before_current, observation.restored_current);
    assert!(observation
        .before
        .iter()
        .any(|token| token.kind() == TokenKind::Comment));
}

#[test]
fn f2_lookahead_and_parenthesized_with_fallback_match_legacy() {
    for source in [
        "match = value\n",
        "match(one(), two())\n",
        "with (manager()) as value:\n    use(value)\n",
        "with (manager()).attribute:\n    pass\n",
        "with (first(), second()):\n    pass\n",
        "with (manager() # opaque\n) as value:\n    pass\n",
    ] {
        differential(source);
    }
    let mut check = never_cancel;
    let module = accepted(b"with (manager()) as value:\n    pass\n", &mut check);
    assert!(module.stats().lexer.checkpoints > 0);
}

#[test]
fn f2_cancellation_threshold_sweep_cannot_be_lost_by_peek_or_rewind() {
    for source in [
        b"match(one(), two())\n".as_slice(),
        b"with (manager()) as value:\n    pass\n",
    ] {
        let polls = Cell::new(0);
        let mut count = || {
            polls.set(polls.get() + 1);
            false
        };
        let module = accepted(source, &mut count);
        assert_eq!(module.stats().parse_invocations, 1);
        let total = polls.get();
        drop(module);
        assert!(total > 1);
        for threshold in 1..=total {
            let calls = Cell::new(0);
            // One-shot cancellation: a rewind must retain it even when later
            // polls would return false. This sweeps the actual peek schedule.
            let mut check = || {
                calls.set(calls.get() + 1);
                calls.get() == threshold
            };
            assert!(
                matches!(
                    parse_module_checked(source, options(), ByteParseLimits::default(), &mut check),
                    Err(ByteParseFailure::Input(ByteLexFailure::Cancelled))
                ),
                "threshold {threshold}/{total}"
            );
        }
    }
}

#[test]
fn f2_source_fault_after_ambiguous_prefix_is_not_ok_none() {
    for source in [
        b"match(\xff)\n".as_slice(),
        b"match = '\xff'\n",
        b"with (manager()) as value:\n    use('\xff')\n",
        b"with (manager()).attribute:\n    use('\xff')\n",
    ] {
        assert!(matches!(
            failure(source, ByteParseLimits::default()),
            ByteParseFailure::Input(ByteLexFailure::Source(_))
        ));
    }
}

#[test]
fn f2_later_source_fault_can_win_over_earlier_syntax_unresolved_control() {
    // CPython can stop at line-one syntax instead. This explicitly records the
    // experimental recovery schedule, NOT exception-class/line parity.
    assert!(matches!(
        failure(b"x=)\nx='\xff'\n", ByteParseLimits::default()),
        ByteParseFailure::Input(ByteLexFailure::Source(_))
    ));
}

#[test]
fn f2_all_eight_legacy_text_entry_points_keep_shapes_modes_and_ranges() {
    use ruff_python_parser::{
        parse, parse_expression, parse_expression_range, parse_module,
        parse_parenthesized_expression_range, parse_string_annotation, parse_unchecked_source,
    };
    let module = parse_module("call()\n").unwrap();
    assert!(matches!(&module.syntax().body[0], ast::Stmt::Expr(_)));
    assert_eq!(
        parse_expression("call()").unwrap().syntax().body.range(),
        range(0, 6)
    );
    assert_eq!(
        parse_expression_range("skip call() tail", range(5, 11))
            .unwrap()
            .syntax()
            .body
            .range(),
        range(5, 11)
    );
    assert_eq!(
        parse_parenthesized_expression_range("skip a +\n b tail", range(5, 11))
            .unwrap()
            .syntax()
            .body
            .range(),
        range(5, 11)
    );
    let literal = ast::StringLiteral {
        value: "call()".into(),
        flags: ast::StringLiteralFlags::empty(),
        range: range(0, 8),
        node_index: ast::AtomicNodeIndex::dummy(),
    };
    assert_eq!(
        parse_string_annotation("'call()'", &literal)
            .unwrap()
            .syntax()
            .body
            .range(),
        range(1, 7)
    );
    assert!(parse_unchecked_source("call()\n", ast::PySourceType::Python).has_no_syntax_errors());
    for (mode, source) in [
        (Mode::Module, "call()\n"),
        (Mode::Expression, "call()"),
        (Mode::ParenthesizedExpression, "a +\n b"),
        (Mode::Ipython, "%time call()\n"),
    ] {
        let parsed = parse(source, ParseOptions::from(mode)).unwrap();
        assert_eq!(parsed, parse_unchecked(source, ParseOptions::from(mode)));
        assert_eq!(
            matches!(parsed.syntax(), ast::Mod::Module(_)),
            matches!(mode, Mode::Module | Mode::Ipython)
        );
    }
    assert!(!parse_unchecked("x=)", options()).errors().is_empty());
    assert!(parse("x=)", options()).is_err());
}
