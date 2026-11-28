//! Private F1 lexical evidence only: no parser admission, AST, encoding selection,
//! type-payload semantics, or Python exception/failure certificates are tested here.
//! The separate frozen pre-refactor hashes remain the independent legacy anchor;
//! shared-core differential tests below supplement, not replace, that anchor.
use std::cell::Cell;

use ruff_python_parser::lexer::byte::{
    legacy_trace, ByteLexFailure, ByteLexer, ByteLexerLimits, ByteLexerStats, ByteTokenValue,
    ByteViewError,
};
use ruff_python_parser::{Mode, TokenKind};
use ruff_text_size::{Ranged, TextRange, TextSize};
use sha2::{Digest, Sha256};

// The seven F0 malformed payloads plus a broken-continuation witness.
const INVALID: [&[u8]; 8] = [
    b"\x80",             // isolated continuation
    b"\xff",             // invalid leading byte
    b"\xc2",             // lone two-byte lead
    b"\xc0\xaf",         // overlong encoding
    b"\xed\xa0\x80",     // surrogate
    b"\xf4\x90\x80\x80", // above Unicode maximum
    b"\xe2(\xa1",        // invalid continuation
    b"\xe2\x82",         // truncated sequence
];

fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(
        TextSize::try_from(start).unwrap(),
        TextSize::try_from(end).unwrap(),
    )
}

fn never_cancel() -> bool {
    false
}

#[test]
fn f1_checked_text_views_charge_work_and_latch_control_failures() {
    let source = format!("{}λ{}", "x".repeat(1023), "y".repeat(3072));
    let all = range(0, source.len());
    let mut check = never_cancel;
    let mut stream = ByteLexer::new(
        source.as_bytes(),
        Mode::Module,
        ByteLexerLimits {
            work: 0,
            ..ByteLexerLimits::default()
        },
        &mut check,
    )
    .unwrap();
    assert_eq!(stream.source_text(all), Err(ByteViewError::ResourceLimit));
    assert_eq!(stream.next_token(), Err(ByteLexFailure::ResourceLimit));

    let mut check = never_cancel;
    let mut stream = ByteLexer::new(
        source.as_bytes(),
        Mode::Module,
        ByteLexerLimits {
            work: source.len() * 3,
            ..ByteLexerLimits::default()
        },
        &mut check,
    )
    .unwrap();
    let checkpoint = stream.checkpoint().unwrap();
    let view = stream.source_text(all).unwrap();
    assert_eq!(view, source);
    assert_eq!(view.as_ptr(), source.as_ptr());
    assert!(stream.stats().work >= source.len());
    assert_eq!(stream.source_text(all), Err(ByteViewError::ResourceLimit));
    assert_eq!(
        stream.rewind(checkpoint),
        Err(ByteLexFailure::ResourceLimit)
    );
    assert_eq!(stream.source_text(all), Err(ByteViewError::ResourceLimit));

    let stop = Cell::new(false);
    let mut check = || stop.get();
    let mut stream = lexer(source.as_bytes(), &mut check);
    let checkpoint = stream.checkpoint().unwrap();
    stop.set(true);
    assert_eq!(stream.source_text(all), Err(ByteViewError::Cancelled));
    assert_eq!(stream.rewind(checkpoint), Err(ByteLexFailure::Cancelled));

    let calls = Cell::new(0);
    let mut check = || {
        calls.set(calls.get() + 1);
        calls.get() > 4
    };
    let mut stream = lexer(source.as_bytes(), &mut check);
    assert_eq!(stream.source_text(all), Err(ByteViewError::Cancelled));
    assert!(
        stream.stats().work > 0,
        "cancel during validation, not just construction"
    );
    assert_eq!(stream.current_token(), Err(ByteLexFailure::Cancelled));
}

fn lexer<'src, 'check>(
    source: &'src [u8],
    check: &'check mut dyn FnMut() -> bool,
) -> ByteLexer<'src, 'check> {
    assert!(source.len() <= 1024 * 1024, "bounded synthetic fixture");
    ByteLexer::new(source, Mode::Module, ByteLexerLimits::default(), check).unwrap()
}

fn drain(lexer: &mut ByteLexer<'_, '_>) -> Result<Vec<String>, ByteLexFailure> {
    let mut trace = Vec::new();
    for _ in 0..20_000 {
        let kind = lexer.next_token()?;
        let token = lexer.current_token()?;
        let value = lexer.take_value()?;
        trace.push(format!("{token:?} {value:?}"));
        if kind.is_eof() {
            return Ok(trace);
        }
    }
    panic!("bounded fixture did not reach EOF");
}

fn monotonic(before: ByteLexerStats, after: ByteLexerStats) {
    assert!(after.work >= before.work);
    assert!(after.tokens >= before.tokens);
    assert!(after.allocated >= before.allocated);
    assert!(after.checkpoints >= before.checkpoints);
}

fn sticky(mut lexer: ByteLexer<'_, '_>, failure: ByteLexFailure) {
    assert_eq!(lexer.current_token(), Err(failure.clone()));
    assert_eq!(lexer.take_value(), Err(failure.clone()));
    assert_eq!(lexer.next_token(), Err(failure.clone()));
    assert_eq!(lexer.re_lex_logical_token(None), Err(failure.clone()));
    assert_eq!(lexer.finish(), Err(failure));
}

fn comment_case(prefix: &[u8], payload: &[u8], ending: &[u8], suffix: &[u8]) {
    let mut source = prefix.to_vec();
    let start = source.len();
    source.extend_from_slice(b"#");
    source.extend_from_slice(payload);
    let end = source.len();
    source.extend_from_slice(ending);
    source.extend_from_slice(suffix);
    let mut check = never_cancel;
    let mut stream = lexer(&source, &mut check);
    assert!(std::ptr::eq(stream.original_bytes(), source.as_slice()));
    let mut comments = 0;
    let mut saw_suffix = suffix.is_empty();
    for _ in 0..256 {
        let kind = stream.next_token().unwrap();
        let token = stream.current_token().unwrap();
        let bytes = stream.source_bytes(token.range()).unwrap();
        assert_eq!(
            bytes.as_ptr(),
            source[token.range().start().to_usize()..].as_ptr()
        );
        if kind == TokenKind::Comment {
            comments += 1;
            assert_eq!(token.range(), range(start, end));
            assert_eq!(bytes, &source[start..end]);
            assert_eq!(stream.take_value().unwrap(), ByteTokenValue::None);
            if std::str::from_utf8(payload).is_err() {
                assert_eq!(
                    stream.source_text(token.range()),
                    Err(ByteViewError::InvalidUtf8)
                );
            } else {
                assert_eq!(stream.source_text(token.range()).unwrap().as_bytes(), bytes);
            }
        }
        if bytes == b"tail" {
            saw_suffix = true;
        }
        if kind.is_eof() {
            assert_eq!(comments, 1);
            assert!(saw_suffix);
            assert!(stream.finish().unwrap().is_empty());
            return;
        }
    }
    panic!("comment fixture did not reach EOF");
}

#[test]
fn f1_comment_bytes_borrow_identity_and_exact_ranges() {
    // 8 malformed payloads x 4 physical endings x 4 adjacency/prefix probes.
    for bad in INVALID {
        for ending in [b"".as_slice(), b"\n", b"\r", b"\r\n"] {
            for prefix in [b"".as_slice(), b"x", b"r", b"rr"] {
                let suffix = if ending.is_empty() {
                    b"".as_slice()
                } else {
                    b"tail=1\n"
                };
                comment_case(prefix, bad, ending, suffix);
            }
        }
    }
}

#[test]
fn f1_valid_utf8_comment_counterparts() {
    for text in [
        "\u{80}",
        "\u{ff}",
        "\u{a2}",
        "/",
        "\u{d7ff}",
        "\u{10ffff}",
        "\u{20ac}(a",
        "\u{20ac}",
    ] {
        for ending in [b"".as_slice(), b"\n", b"\r", b"\r\n"] {
            comment_case(b"x", text.as_bytes(), ending, b"");
        }
    }
}

#[test]
fn f1_type_and_ignore_payloads_are_opaque_not_semantically_admitted() {
    for bad in INVALID {
        for prefix in [b"type: ".as_slice(), b"type: ignore[", b"coding: "] {
            let mut payload = prefix.to_vec();
            payload.extend_from_slice(bad);
            payload.extend_from_slice(b"] ' \" { } #");
            comment_case(b"value=1 ", &payload, b"\n", b"tail=2\n");
        }
    }
}

#[test]
fn f1_replacement_comments_include_nested_and_debug_shaped_contexts() {
    // Quotes/braces after '#' must not escape the comment or close a field.
    for bad in INVALID {
        for (prefix, suffix) in [
            (b"x=f\"{call() ".as_slice(), b"\n}\"\ntail=1\n".as_slice()),
            (b"x=f\"{(call() ", b"\n)=}\"\ntail=1\n"),
            (b"x=f\"{f'{inner() ", b"\n=}'=}\"\ntail=1\n"),
            (b"x=f\"{ {'key': call()} ", b"\n!r}\"\ntail=1\n"),
        ] {
            let mut payload = b" quote ' \" braces } { # ".to_vec();
            payload.extend_from_slice(bad);
            // comment_case supplies the physical newline separately.
            comment_case(prefix, &payload, b"\n", &suffix[1..]);
        }
    }
}

#[test]
fn f1_hash_in_text_is_not_a_comment_and_invalid_text_faults() {
    for (prefix, suffix) in [
        (b"x='#".as_slice(), b"'\n".as_slice()),
        (b"x=b'#", b"'\n"),
        (b"x=r'#", b"'\n"),
        (b"x='''#", b"'''\n"),
        (b"x=f\"#", b"{1}\"\n"),
        (b"x=f\"{1:#", b"}\"\n"),
    ] {
        let mut valid = prefix.to_vec();
        valid.extend_from_slice("\u{e9}".as_bytes());
        valid.extend_from_slice(suffix);
        let mut check = never_cancel;
        let mut stream = lexer(&valid, &mut check);
        loop {
            let kind = stream.next_token().unwrap();
            assert_ne!(kind, TokenKind::Comment);
            if kind.is_eof() {
                break;
            }
        }
        assert!(stream.finish().unwrap().is_empty());
        for bad in INVALID {
            let mut source = prefix.to_vec();
            source.extend_from_slice(bad);
            source.extend_from_slice(suffix);
            let mut check = never_cancel;
            let mut stream = lexer(&source, &mut check);
            loop {
                match stream.next_token() {
                    Ok(kind) => {
                        assert_ne!(kind, TokenKind::Comment);
                        assert!(!kind.is_eof(), "malformed text was silently accepted");
                    }
                    Err(error @ ByteLexFailure::Source(_)) => {
                        sticky(stream, error);
                        break;
                    }
                    Err(error) => panic!("expected Source, got {error:?}"),
                }
            }
        }
    }
}

#[test]
fn f1_source_fault_locations_and_unterminated_fast_paths() {
    for prefix in [b"".as_slice(), b"'", b"'''", b"r'", b"b'", b"f\"text"] {
        for bad in INVALID {
            let mut source = prefix.to_vec();
            source.extend_from_slice(bad);
            let expected = std::str::from_utf8(bad).unwrap_err();
            let mut check = never_cancel;
            let mut stream = lexer(&source, &mut check);
            let failure = loop {
                match stream.next_token() {
                    Ok(kind) => {
                        assert!(!kind.is_eof(), "unterminated fast path hid malformed bytes")
                    }
                    Err(error) => break error,
                }
            };
            let ByteLexFailure::Source(fault) = &failure else {
                panic!("expected typed source fault, got {failure:?}");
            };
            let start = prefix.len() + expected.valid_up_to();
            assert_eq!(
                fault.range,
                range(start, start + expected.error_len().unwrap_or(bad.len()))
            );
            assert_eq!(fault.error_len, expected.error_len());
            sticky(stream, failure);
        }
    }
}

#[test]
fn f1_checked_views_boundary_errors_do_not_poison_stream() {
    let source = b"#\xff\n'\xc3\xa9'\n";
    let mut check = never_cancel;
    let mut stream = lexer(source, &mut check);
    assert_eq!(
        stream.source_text(range(0, 2)),
        Err(ByteViewError::InvalidUtf8)
    );
    assert_eq!(
        stream.source_bytes(range(0, source.len() + 1)),
        Err(ByteViewError::InvalidRange)
    );
    assert_eq!(
        stream.source_text(range(source.len() + 1, source.len() + 1)),
        Err(ByteViewError::InvalidRange)
    );
    assert_eq!(
        stream.source_text(range(5, 6)),
        Err(ByteViewError::InvalidUtf8)
    );
    assert_eq!(
        stream.source_text(range(4, 5)),
        Err(ByteViewError::InvalidUtf8)
    );
    assert_eq!(stream.source_text(range(4, 6)).unwrap(), "\u{e9}");
    assert_eq!(
        stream
            .source_bytes(range(source.len(), source.len()))
            .unwrap(),
        b""
    );
    drain(&mut stream).unwrap();
    assert!(stream.finish().unwrap().is_empty());
}

#[test]
fn f1_initial_state_and_genuine_eof_are_distinct() {
    for source in [b"".as_slice(), b"name"] {
        let mut check = never_cancel;
        let mut stream = lexer(source, &mut check);
        assert_eq!(stream.current_token(), Err(ByteLexFailure::InvalidState));
        assert_eq!(stream.take_value(), Err(ByteLexFailure::InvalidState));
        assert_eq!(
            stream.re_lex_logical_token(None),
            Err(ByteLexFailure::InvalidState)
        );
        assert_eq!(stream.finish(), Err(ByteLexFailure::Incomplete));
        let mut check = never_cancel;
        let mut stream = lexer(source, &mut check);
        drain(&mut stream).unwrap();
        assert!(stream.current_token().unwrap().kind().is_eof());
        assert!(stream.finish().unwrap().is_empty());
    }
    let mut check = never_cancel;
    let mut stream = lexer(b"name", &mut check);
    assert_eq!(stream.next_token().unwrap(), TokenKind::Name);
    assert_eq!(stream.finish(), Err(ByteLexFailure::Incomplete));
}

#[test]
fn f1_raw_nul_is_a_scalar_not_eof_and_root_policy_is_separate() {
    for source in [b"\0tail\n".as_slice(), b"#\0\ntail\n", b"'\0'\ntail\n"] {
        let mut check = never_cancel;
        let mut stream = lexer(source, &mut check);
        let mut saw_tail = false;
        loop {
            let kind = stream.next_token().unwrap();
            let token = stream.current_token().unwrap();
            if stream.source_bytes(token.range()).unwrap() == b"tail" {
                saw_tail = true;
            }
            if kind.is_eof() {
                assert!(saw_tail, "NUL truncated the lexical stream");
                break;
            }
        }
        stream.finish().unwrap();
    }
}

#[test]
fn f1_full_valid_utf8_traces_match_legacy_in_all_four_modes() {
    let sources = [
        "x#comment\r\nr#adjacent\rrr#last\n",
        "\u{212a} = \u{3bb} + caf\u{e9}\n",
        "0xff_ff + 0o17 + 0b10 + 12_345 + .25e-2 + 3.5j\n",
        "r'raw\\text' b'bytes\\x41' '\u{e9}\u{4e2d}' b'\u{e9}'\n",
        "'''triple\r\nline''' '\\N{SPACE}\\ud800'\n",
        "f\"outer#{{literal}}{call(\"key\")!r:{width()}.{precision()}f}{f'{inner()=}'}\"\n",
        "f\"{(call() # quote \\\" brace }\n)=}\"\n",
        "if ready:\r\n\tfirst()\r\n\tif nested:\n\t\tsecond()\nlast()\r",
        "x='unterminated\ny=0xg\nz = \\q\n",
        "%time call()\nresult = !echo value\nname?\n",
        "\u{feff}# coding: utf-8\nx=(a #comment\n + b) #eof",
        "a := b\na += b ** c // d << e >> f @ g\nx = a != b <= c >= d == e and not f or g\n",
    ];
    for mode in [
        Mode::Module,
        Mode::Expression,
        Mode::ParenthesizedExpression,
        Mode::Ipython,
    ] {
        for source in sources {
            let mut check = never_cancel;
            let mut stream = ByteLexer::new(
                source.as_bytes(),
                mode,
                ByteLexerLimits::default(),
                &mut check,
            )
            .unwrap();
            let tokens = drain(&mut stream).unwrap();
            let actual = (tokens, format!("{:?}", stream.finish().unwrap()));
            assert_eq!(actual, legacy_trace(source, mode), "{mode:?}: {source:?}");
        }
    }
}

#[test]
fn f1_literal_values_preserve_valid_non_ascii_source_bytes() {
    // These are lexical payloads, not evaluated Python bytes/string objects.
    for prefix in ["", "r", "u", "b", "br"] {
        let source = format!("{prefix}'\u{e9}\u{4e2d}'");
        let mut check = never_cancel;
        let mut stream = lexer(source.as_bytes(), &mut check);
        assert_eq!(stream.next_token().unwrap(), TokenKind::String);
        assert_eq!(
            stream.take_value().unwrap(),
            ByteTokenValue::String("\u{e9}\u{4e2d}".into())
        );
        assert_eq!(stream.take_value().unwrap(), ByteTokenValue::None);
        drain(&mut stream).unwrap();
        assert!(stream.finish().unwrap().is_empty());
    }
}

#[test]
fn f1_byte_traces_match_all_thirteen_frozen_pre_refactor_cases() {
    // Only the small input table is repeated because the sibling's cases() is
    // private. Expected hashes remain solely in the unchanged frozen baseline.
    let cases = [
        (Mode::Module, "from pkg import item as alias\n@decorate\nasync def f(a: T = factory(), /, *, k=other()) -> R:\n    return await alias(a, k=k)\n"),
        (Mode::Module, "\u{212a} = \u{3bb} + caf\u{e9}\n\u{53d8}\u{91cf} = \u{212a}.attr[\u{3bb}]\n"),
        (Mode::Module, "n = 0xff_ff + 0o17 + 0b10_01 + 12_345 + .25e-2 + 3.5j\n"),
        (Mode::Module, "a = r'raw\\text' b'bytes\\x41'\nb = '''triple\nline'''\nc = '\\N{SPACE}\\ud800'\n"),
        (Mode::Module, "x = f\"outer{{literal}}{call(\"key\")!r:{width()}.{precision()}f}{f'{inner()=}'}\"\n"),
        (Mode::Module, "x=f\"{(call() # quote \\\" brace }\n)=}\"\n"),
        (Mode::Module, "if ready:\r\n\tfirst()\r\n\t# comment\r\n\tif nested:\r\n\t\tsecond()\r\nlast()\r"),
        (Mode::Module, "\u{feff}# coding: utf-8\nvalue=(one() # text\n + two()) # eof"),
        (Mode::Module, "a := b\na += b ** c // d << e >> f @ g\nx = a != b <= c >= d == e and not f or g\n"),
        (Mode::Module, "x='unterminated\ny=0xg\nz = \\q\n"),
        (Mode::Expression, "(lambda a=first(), *, k=second(): a[k])({\"x\": 1})"),
        (Mode::ParenthesizedExpression, "first +\n second[lower:upper:step]"),
        (Mode::Ipython, "%time call()\nresult = !echo value\nname?\n"),
    ];
    let baseline: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../../docs/python-byte-lexer-baseline.json"
    ))
    .unwrap();
    assert_eq!(baseline.len(), cases.len());
    for (row, (mode, source)) in baseline.iter().zip(cases) {
        assert_eq!(row["mode"], format!("{mode:?}"));
        assert_eq!(
            row["source_sha256"],
            format!("{:x}", Sha256::digest(source.as_bytes()))
        );
        let mut check = never_cancel;
        let mut stream = ByteLexer::new(
            source.as_bytes(),
            mode,
            ByteLexerLimits::default(),
            &mut check,
        )
        .unwrap();
        let tokens = drain(&mut stream).unwrap();
        assert_eq!(
            row["tokens"].as_u64().unwrap(),
            u64::try_from(tokens.len()).unwrap()
        );
        let trace = (tokens, format!("{:?}", stream.finish().unwrap()));
        assert_eq!(trace, legacy_trace(source, mode));
        assert_eq!(
            row["trace_sha256"],
            format!("{:x}", Sha256::digest(serde_json::to_vec(&trace).unwrap())),
            "{}",
            row["id"]
        );
    }
}

#[test]
fn f1_checkpoint_owner_is_lexer_not_backing_buffer() {
    let source = b"name\n";
    let mut check_a = never_cancel;
    let mut check_b = never_cancel;
    let mut a = lexer(source, &mut check_a);
    let mut b = lexer(source, &mut check_b);
    let checkpoint = a.checkpoint().unwrap();
    assert_eq!(b.rewind(checkpoint), Err(ByteLexFailure::ForeignCheckpoint));
    assert_eq!(b.current_token(), Err(ByteLexFailure::InvalidState));
    assert_eq!(drain(&mut a).unwrap(), drain(&mut b).unwrap());
    assert!(a.finish().unwrap().is_empty());
    assert!(b.finish().unwrap().is_empty());
}

#[test]
fn f1_rewind_clears_source_fault_and_restores_taken_and_untaken_values() {
    let mut check = never_cancel;
    let mut stream = lexer(b"name #\xff\n'\xff'", &mut check);
    let initial = stream.checkpoint().unwrap();
    assert_eq!(stream.next_token().unwrap(), TokenKind::Name);
    let token = stream.current_token().unwrap();
    let untaken = stream.checkpoint().unwrap();
    let value = stream.take_value().unwrap();
    assert!(matches!(value, ByteTokenValue::Name(_)));
    let taken = stream.checkpoint().unwrap();
    assert!(matches!(drain(&mut stream), Err(ByteLexFailure::Source(_))));
    stream.rewind(taken).unwrap();
    assert_eq!(stream.current_token().unwrap(), token);
    assert_eq!(stream.take_value().unwrap(), ByteTokenValue::None);
    stream.rewind(untaken).unwrap();
    assert_eq!(stream.current_token().unwrap(), token);
    assert_eq!(stream.take_value().unwrap(), value);
    assert_eq!(stream.next_token().unwrap(), TokenKind::Comment);
    assert_eq!(stream.current_token().unwrap().range(), range(5, 7));
    stream.rewind(initial).unwrap();
    assert_eq!(stream.current_token(), Err(ByteLexFailure::InvalidState));
    assert_eq!(stream.next_token().unwrap(), TokenKind::Name);
    assert_eq!(stream.current_token().unwrap(), token);
}

#[test]
fn f1_non_lifo_snapshots_restore_diagnostics_indents_and_nested_fstrings() {
    let source = b"if x:\n    f\"{f'{inner()=}'}\"\n    \\q\n    next_name\nlast\n";
    let mut check = never_cancel;
    let mut stream = lexer(source, &mut check);
    let mut snapshots = Vec::new();
    let mut expected = Vec::new();
    loop {
        let kind = stream.next_token().unwrap();
        let token = stream.current_token().unwrap();
        // Save before taking values. Include live nested string and indent stacks.
        snapshots.push((
            stream.checkpoint().unwrap(),
            format!("{:?}", stream.lexical_errors()),
        ));
        let value = stream.take_value().unwrap();
        expected.push(format!("{token:?} {value:?}"));
        if kind.is_eof() {
            break;
        }
    }
    let errors = format!("{:?}", stream.lexical_errors());
    assert!(!stream.lexical_errors().is_empty());
    let mut snapshots: Vec<_> = snapshots.into_iter().enumerate().collect();
    // Evens then odds restore both backwards and forwards, consuming each owned snapshot.
    snapshots.sort_by_key(|(index, _)| (index % 2, *index));
    for (index, (checkpoint, diagnostics)) in snapshots {
        let before = stream.stats();
        stream.rewind(checkpoint).unwrap();
        monotonic(before, stream.stats());
        assert_eq!(format!("{:?}", stream.lexical_errors()), diagnostics);
        let token = stream.current_token().unwrap();
        let value = stream.take_value().unwrap();
        assert_eq!(format!("{token:?} {value:?}"), expected[index]);
        if !token.kind().is_eof() {
            assert_eq!(drain(&mut stream).unwrap(), expected[index + 1..]);
        }
        assert_eq!(format!("{:?}", stream.lexical_errors()), errors);
    }
    assert_eq!(format!("{:?}", stream.finish().unwrap()), errors);
}

#[test]
fn f1_rewinds_do_not_refund_work_tokens_allocations_or_checkpoints() {
    let limits = ByteLexerLimits {
        tokens: 3,
        ..ByteLexerLimits::default()
    };
    let mut check = never_cancel;
    let mut stream = ByteLexer::new(b"name", Mode::Module, limits, &mut check).unwrap();
    for _ in 0..3 {
        let checkpoint = stream.checkpoint().unwrap();
        stream.next_token().unwrap();
        let before = stream.stats();
        stream.rewind(checkpoint).unwrap();
        let after = stream.stats();
        assert_eq!(after.tokens, before.tokens);
        assert_eq!(after.allocated, before.allocated);
        assert_eq!(after.checkpoints, before.checkpoints);
        monotonic(before, after);
    }
    assert_eq!(stream.stats().checkpoints, 3);
    assert_eq!(stream.next_token(), Err(ByteLexFailure::ResourceLimit));
    sticky(stream, ByteLexFailure::ResourceLimit);
}

#[test]
fn f1_invalid_relex_positions_leave_current_and_future_state_unchanged() {
    for position in [0, 2, 4, 100] {
        let source = b"(x\n y)\n";
        let mut check = never_cancel;
        let mut stream = lexer(source, &mut check);
        stream.next_token().unwrap();
        stream.next_token().unwrap();
        let token = stream.current_token().unwrap();
        let checkpoint = stream.checkpoint().unwrap();
        assert_eq!(
            stream.re_lex_logical_token(Some(TextSize::new(position))),
            Err(ByteLexFailure::InvalidState)
        );
        assert_eq!(stream.current_token().unwrap(), token);
        let value = stream.take_value().unwrap();
        let rest = drain(&mut stream).unwrap();
        stream.rewind(checkpoint).unwrap();
        assert_eq!(stream.take_value().unwrap(), value);
        assert_eq!(drain(&mut stream).unwrap(), rest);
        stream.finish().unwrap();
    }
}

#[test]
fn f1_logical_relex_across_opaque_comment_and_triple_fstring_exclusion() {
    let source = b"(x#\xff\nnext\n";
    let mut check = never_cancel;
    let mut stream = lexer(source, &mut check);
    for kind in [
        TokenKind::Lpar,
        TokenKind::Name,
        TokenKind::Comment,
        TokenKind::NonLogicalNewline,
    ] {
        assert_eq!(stream.next_token().unwrap(), kind);
    }
    let newline = stream.current_token().unwrap().range();
    assert_eq!(newline, range(4, 5));
    assert!(stream.re_lex_logical_token(Some(newline.start())).unwrap());
    assert_eq!(
        stream.current_token().unwrap().as_tuple(),
        (TokenKind::Newline, newline)
    );
    assert_eq!(stream.next_token().unwrap(), TokenKind::Name);
    assert_eq!(
        stream
            .source_text(stream.current_token().unwrap().range())
            .unwrap(),
        "next"
    );
    drain(&mut stream).unwrap();
    stream.finish().unwrap();

    let mut check = never_cancel;
    let mut stream = lexer(b"(\nf'''outer\n{value}'''", &mut check);
    assert_eq!(stream.next_token().unwrap(), TokenKind::Lpar);
    assert_eq!(stream.next_token().unwrap(), TokenKind::NonLogicalNewline);
    assert_eq!(stream.next_token().unwrap(), TokenKind::FStringStart);
    assert_eq!(stream.next_token().unwrap(), TokenKind::FStringMiddle);
    let before = stream.current_token().unwrap();
    assert!(!stream.re_lex_logical_token(Some(TextSize::new(1))).unwrap());
    assert_eq!(stream.current_token().unwrap(), before);
}

#[test]
fn f1_tiny_input_work_token_depth_working_and_diagnostic_budgets() {
    let mut check = never_cancel;
    assert!(matches!(
        ByteLexer::new(
            b"ab",
            Mode::Module,
            ByteLexerLimits {
                input_bytes: 1,
                ..ByteLexerLimits::default()
            },
            &mut check
        ),
        Err(ByteLexFailure::ResourceLimit)
    ));
    let long = "a".repeat(3072);
    let sources = [
        format!("#{long}\n"),
        format!("'{long}'"),
        long.clone(),
        format!("'{}'", "\u{e9}".repeat(1536)),
    ];
    for source in &sources {
        let mut check = never_cancel;
        let mut stream = ByteLexer::new(
            source.as_bytes(),
            Mode::Module,
            ByteLexerLimits {
                work: 32,
                ..ByteLexerLimits::default()
            },
            &mut check,
        )
        .unwrap();
        let checkpoint = stream.checkpoint().err();
        // Checkpoint work itself is bounded; test the token loop on a fresh stream.
        assert_eq!(checkpoint, Some(ByteLexFailure::ResourceLimit));
        let mut check = never_cancel;
        let mut stream = ByteLexer::new(
            source.as_bytes(),
            Mode::Module,
            ByteLexerLimits {
                work: 32,
                ..ByteLexerLimits::default()
            },
            &mut check,
        )
        .unwrap();
        assert_eq!(stream.next_token(), Err(ByteLexFailure::ResourceLimit));
        sticky(stream, ByteLexFailure::ResourceLimit);
    }
    // Permit the chunked boundary search, then exhaust work while validating
    // the non-ASCII string body, before a value can be returned.
    let mut check = never_cancel;
    let mut stream = ByteLexer::new(
        sources[3].as_bytes(),
        Mode::Module,
        ByteLexerLimits {
            work: 3200,
            ..ByteLexerLimits::default()
        },
        &mut check,
    )
    .unwrap();
    assert_eq!(stream.next_token(), Err(ByteLexFailure::ResourceLimit));
    assert!(stream.stats().work >= 3072);
    sticky(stream, ByteLexFailure::ResourceLimit);
    for (source, limits) in [
        (
            b"x".as_slice(),
            ByteLexerLimits {
                tokens: 0,
                ..ByteLexerLimits::default()
            },
        ),
        (
            b"(",
            ByteLexerLimits {
                depth: 0,
                ..ByteLexerLimits::default()
            },
        ),
        (
            b"f'{x}'",
            ByteLexerLimits {
                depth: 0,
                ..ByteLexerLimits::default()
            },
        ),
        (
            b"x",
            ByteLexerLimits {
                working: 0,
                ..ByteLexerLimits::default()
            },
        ),
        (
            b"\\q",
            ByteLexerLimits {
                diagnostics: 0,
                ..ByteLexerLimits::default()
            },
        ),
    ] {
        let mut check = never_cancel;
        let mut stream = ByteLexer::new(source, Mode::Module, limits, &mut check).unwrap();
        assert_eq!(drain(&mut stream), Err(ByteLexFailure::ResourceLimit));
        sticky(stream, ByteLexFailure::ResourceLimit);
    }
    // Value storage, not merely the per-token envelope, must be precharged.
    for source in [&sources[1], &sources[2], &sources[3]] {
        let mut check = never_cancel;
        let mut stream = ByteLexer::new(
            source.as_bytes(),
            Mode::Module,
            ByteLexerLimits {
                working: 1024,
                ..ByteLexerLimits::default()
            },
            &mut check,
        )
        .unwrap();
        assert_eq!(drain(&mut stream), Err(ByteLexFailure::ResourceLimit));
        sticky(stream, ByteLexFailure::ResourceLimit);
    }
}

#[test]
fn f1_checkpoint_budgets_and_terminal_failures_cannot_be_rewound() {
    for limits in [
        ByteLexerLimits {
            checkpoints: 1,
            ..ByteLexerLimits::default()
        },
        ByteLexerLimits {
            working: 800,
            ..ByteLexerLimits::default()
        },
        ByteLexerLimits {
            work: 800,
            ..ByteLexerLimits::default()
        },
    ] {
        let mut check = never_cancel;
        let mut stream = ByteLexer::new(b"name", Mode::Module, limits, &mut check).unwrap();
        let checkpoint = stream.checkpoint().unwrap();
        assert!(matches!(
            stream.checkpoint(),
            Err(ByteLexFailure::ResourceLimit)
        ));
        assert_eq!(
            stream.rewind(checkpoint),
            Err(ByteLexFailure::ResourceLimit)
        );
        sticky(stream, ByteLexFailure::ResourceLimit);
    }
}

#[test]
fn f1_cancellation_polls_inside_long_loops_and_is_terminal() {
    let long = "a".repeat(3072);
    for source in [
        format!("#{long}\n"),
        format!("'{long}'"),
        long,
        format!("'{}'", "\u{e9}".repeat(1536)),
    ] {
        let polls = Cell::new(0);
        let armed = Cell::new(false);
        let mut check = || {
            if !armed.get() {
                return false;
            }
            polls.set(polls.get() + 1);
            polls.get() >= 8
        };
        let mut stream = lexer(source.as_bytes(), &mut check);
        let checkpoint = stream.checkpoint().unwrap();
        let before = stream.stats();
        armed.set(true);
        assert_eq!(stream.next_token(), Err(ByteLexFailure::Cancelled));
        assert_eq!(
            polls.get(),
            8,
            "must cancel during work, not just at token entry"
        );
        assert!(stream.stats().work > before.work + 1);
        armed.set(false);
        assert_eq!(stream.rewind(checkpoint), Err(ByteLexFailure::Cancelled));
        sticky(stream, ByteLexFailure::Cancelled);
    }
}
