#!/usr/bin/env python3
"""Read-only, finite CPython byte-input oracle; not a Ruff parity claim.

Run with the existing trusted CPython 3.12.11: -I -S -B. No source-path
arguments, external fixtures, writes, interpreter discovery or source execution.
There are 190 historical rows, nine ASCII controls and 20 debug rows (219
parses). Expected exceptions/lines, debug values and span anchors are static;
the two pinned hashes freeze measured inputs/metadata and all accepted ASTs.
AST JSON is {"ast": ast.dump(tree, include_attributes=True)} encoded with
canonical(). The accepted fingerprint hashes the name -> AST-JSON-hash map.
Debug constants deliberately omit comments: preserving original bytes requires
byte-capable DebugText or comment-aware AST construction, not source repair.
The profile is local to this study, not a production/facts policy revision.
"""

import ast
from collections import Counter, namedtuple
import hashlib
import json
import sys
import unicodedata

PROFILE = "cpython-3.12.11-ucd15-byte-study-v1"
MAX_CASES = 256
MAX_SOURCE = 8 * 1024
MAX_TOTAL = 1024 * 1024
MAX_NODES = 1024
MAX_AST_JSON = 256 * 1024
MAX_AST_TOTAL = 4 * 1024 * 1024
CLASSES = frozenset(("OK", "SyntaxError", "UnicodeDecodeError"))
Case = namedtuple("Case", "name source expected control constants refs", defaults=(None, None, ()))
EXPECTED_MATRIX_SHA256 = "5554f3dc4742e002b1cddac833cbb6bda12f8d9443c912338193e47b96be0758"
EXPECTED_AST_SHA256 = "bf54e22674a922eebacd7136c99e2ff0fe47c18b4447a934230d36afe552986d"
EXPECTED_COUNTS = {"OK": 107, "SyntaxError@0": 20, "SyntaxError@1": 36,
                   "SyntaxError@2": 13, "SyntaxError@3": 3,
                   "SyntaxError@4": 1, "UnicodeDecodeError@0": 39}


def matrix():
    """Closed, synthetic inputs and independently measured static expectations."""
    rows = []

    def add(name, source, outcome="OK", control=None, constants=None, refs=()):
        rows.append(Case(name, source, outcome, control, constants, refs))

    payloads = (("ff", b"\xff"), ("continuation", b"\x80"), ("lead", b"\xc2"),
                ("truncated", b"\xe2\x82"), ("overlong", b"\xc0\xaf"),
                ("surrogate", b"\xed\xa0\x80"), ("above_max", b"\xf4\x90\x80\x80"),
                ("eacute", b"\xc3\xa9"))
    contexts = (
        ("comment", b"# ", b"\nx=1\n", "OK", "OK", True),
        ("trailing", b"x=1 # ", b"\ny=2\n", "OK", "OK", True),
        ("paren", b"x=(1 # ", b"\n)\n", "OK", "OK", True),
        ("string", b'x="', b'"\n', "SyntaxError@1", "OK", False),
        ("bytes", b'x=b"', b'"\n', "SyntaxError@1", "SyntaxError@1", False),
        ("fouter", b'x=f"', b'{1}"\n', "UnicodeDecodeError@0", "OK", False),
        ("format", b'x=f"{1:', b'}"\n', "UnicodeDecodeError@0", "OK", False),
        ("replacement", b'x=f"{1 # ', b'\n}"\n', "OK", "OK", True),
        ("typeassign", b"x=1 # type: ", b"\n", "UnicodeDecodeError@0", "OK", False),
        ("ignore", b"x=1 # type: ignore[", b"]\n", "UnicodeDecodeError@0", "OK", False),
        ("near_type", b"x=1 # types: ", b"\n", "OK", "OK", True),
        ("standalone_type", b"# type: ", b"\nx=1\n", "SyntaxError@1", "SyntaxError@1", False),
    )
    for pn, payload in payloads:
        for cn, pre, post, bad, good, neutral in contexts:
            add("context/" + pn + "/" + cn, pre + payload + post,
                good if pn == "eacute" else bad,
                "control/" + cn if neutral else None)
    for cn, pre, post, _, _, neutral in contexts:
        if neutral:
            add("control/" + cn, pre + b"ASCII" + post)
    cookies = (
        (b"utf-8", "OK", "SyntaxError@2"), (b"utf8", "SyntaxError@0", "SyntaxError@0"),
        (b"UTF_8", "OK", "SyntaxError@2"), (b"utf-8-extra", "OK", "SyntaxError@2"),
        (b"utf_8_extra", "OK", "SyntaxError@2"), (b"utf-8-sig", "OK", "SyntaxError@2"),
        (b"utf8_sig", "SyntaxError@0", "SyntaxError@0"),
        (b"cp65001", "SyntaxError@0", "SyntaxError@0"),
        (b"u8", "SyntaxError@0", "SyntaxError@0"), (b"latin-1", "OK", "OK"),
    )
    for cookie, comment, string in cookies:
        for role, body, outcome in (("comment", b"# \xff\nx=1\n", comment),
                                    ("string", b'x="\xff"\n', string)):
            add("cookie/" + cookie.decode("ascii") + "/" + role,
                b"# coding: " + cookie + b"\n" + body, outcome)
    eligibility = (
        ("line2_blank", b'\n# coding: latin-1\nx="\xff"\n', "OK"),
        ("line2_comment", b'# note\n# coding: latin-1\nx="\xff"\n', "OK"),
        ("line2_code", b'x=1\n# coding: latin-1\nx="\xff"\n', "SyntaxError@3"),
        ("line3", b'# note\n# note\n# coding: latin-1\nx="\xff"\n', "SyntaxError@4"),
        ("same_line_code", b'x=1 # coding: latin-1\nx="\xff"\n', "SyntaxError@2"),
        ("first_wins", b'# coding: utf-8\n# coding: latin-1\nx="\xff"\n', "SyntaxError@3"),
        ("invalid_before_cookie", b'# \xff\n# coding: latin-1\nx="\xff"\n', "OK"),
        ("cookie_invalid_tail", b'# coding: utf-8 \xff\nx=1\n', "OK"),
        ("alias_invalid_tail", b'# coding: utf8 \xff\nx=1\n', "SyntaxError@0"),
        ("bom_default", b'\xef\xbb\xbf# \xff\nx=1\n', "OK"),
        ("bom_alias", b'\xef\xbb\xbf# coding: utf8\n# \xff\nx=1\n', "SyntaxError@0"),
        ("bom_conflict", b'\xef\xbb\xbf# coding: latin-1\nx=1\n', "SyntaxError@0"),
    )
    for name, source, outcome in eligibility:
        add("eligible/" + name, source, outcome)
    ordering = (
        ("grammar_then_comment", b'x=)\n# \xff\n', "SyntaxError@1"),
        ("grammar_then_string", b'x=)\nx="\xff"\n', "SyntaxError@1"),
        ("grammar_then_bare", b'x=)\n\xff\n', "SyntaxError@1"),
        ("grammar_then_alias_comment", b'# coding: utf8\nx=)\n# \xff\n', "SyntaxError@0"),
        ("grammar_then_canonical_string", b'# coding: utf-8\nx=)\nx="\xff"\n', "SyntaxError@2"),
        ("grammar_then_nul", b'x=)\n# \x00\n', "SyntaxError@0"),
        ("unknown_then_nul", b'# coding: no-such-codec\n# \x00\n', "SyntaxError@0"),
        ("badcookie_then_nul", b'# coding: utf8\n# \xff\x00\n', "SyntaxError@0"),
        ("bomconflict_then_nul", b'\xef\xbb\xbf# coding: latin-1\n\x00\n', "SyntaxError@0"),
        ("nul_in_string", b'x="\x00"\n', "SyntaxError@0"),
        ("nul_in_replacement_comment", b'x=f"{1 # \x00\n}"\n', "SyntaxError@0"),
        ("nul_in_typeignore", b'x=1 # type: ignore[\x00]\n', "SyntaxError@0"),
        ("openparen_then_invalid", b'x=(\n\xff\n', "SyntaxError@2"),
        ("triplestring_invalid", b'x="""\na\n\xff\n"""\n', "SyntaxError@1"),
        ("fouter_bad_then_inner_bad", b'x=f"\xff{\n"\xff"\n}"\n', "UnicodeDecodeError@0"),
        ("format_bad_then_inner_bad", b'x=f"{1:\xff{\n"\xff"\n}}"\n', "UnicodeDecodeError@0"),
        ("type_bad_then_grammar", b'x=1 # type: \xff\ny=)\n', "UnicodeDecodeError@0"),
        ("grammar_then_type_bad", b'x=)\ny=1 # type: \xff\n', "SyntaxError@1"),
        ("ignore_bad_then_grammar", b'x=1 # type: ignore[\xff]\ny=)\n', "SyntaxError@2"),
        ("grammar_then_ignore_bad", b'x=)\ny=1 # type: ignore[\xff]\n', "SyntaxError@1"),
        ("fouter_bad_then_grammar", b'x=f"\xff{)}"\n', "UnicodeDecodeError@0"),
        ("format_bad_then_grammar", b'x=f"{1:\xff{)}}"\n', "UnicodeDecodeError@0"),
        ("plain_bad_then_grammar", b'x="\xff"; y=)\n', "SyntaxError@1"),
        ("comment_unclosed_quotes", b'# "\'\'\' f"{ \xff\nx=1\n', "OK"),
    )
    for name, source, outcome in ordering:
        add("order/" + name, source, outcome)
    for nn, nl in (("LF", b"\n"), ("CR", b"\r"), ("CRLF", b"\r\n")):
        for role, lines, outcome in (
            ("comment", (b"# \xff", b"x=1", b""), "OK"),
            ("string", (b"x=1", b'y="\xff"', b""), "SyntaxError@2"),
            ("replacement", (b'x=f"{1 # \xff', b'}"', b""), "OK"),
            ("line2cookie", (b"# note", b"# coding: latin-1", b'x="\xff"', b""), "OK"),
        ):
            add("newline/" + nn + "/" + role, nl.join(lines), outcome)
    for name, source, outcome in (
        ("name", b'\xc3\xa9=call()\n', "OK"),
        ("string_then_call", b'x="\xc3\xa9"; call()\n', "OK"),
        ("replacement_name", b'x=f"{\xc3\xa9()}"\n', "OK"),
        ("format_defined", b'x=f"{1:\xc3\xa9}"\n', "OK"),
        ("ignore_defined", b'x=1 # type: ignore[\xc3\xa9]\n', "OK"),
        ("type_defined", b'x=1 # type: \xc3\xa9\n', "OK"),
        ("replacement_type_bad", b'x=f"{1 # type: \xff\n}"\n', "SyntaxError@1"),
        ("replacement_ignore_bad", b'x=f"{1 # type: ignore[\xff]\n}"\n', "UnicodeDecodeError@0"),
    ):
        add("unicode/" + name, source, outcome)
    for cookie, outcome in ((b"utf8", "OK"), (b"utf8_sig", "SyntaxError@0"),
                            (b"cp65001", "OK"), (b"u8", "OK")):
        add("supplement/valid_alias/" + cookie.decode("ascii"),
            b"# coding: " + cookie + b'\nx="\xc3\xa9"\n', outcome)
    for name, source, outcome in (
        ("raw_string", b'x=r"\xff"\n', "SyntaxError@1"),
        ("raw_bytes", b'x=br"\xff"\n', "SyntaxError@1"),
        ("raw_fouter", b'x=rf"\xff{1}"\n', "UnicodeDecodeError@0"),
        ("raw_format", b'x=rf"{1:\xff}"\n', "UnicodeDecodeError@0"),
        ("ignore_then_bad_string", b'x=1 # type: ignore[\xff]\ny="\xff"\n', "SyntaxError@2"),
        ("type_then_bad_string", b'x=1 # type: \xff\ny="\xff"\n', "UnicodeDecodeError@0"),
        ("plain_hash", b'x="# \xff"\n', "SyntaxError@1"),
        ("f_format_hash", b'x=f"{1:# \xff}"\n', "UnicodeDecodeError@0"),
        ("f_comment_quote", b'x=f"{call() # \xff " }\n}"\n', "OK"),
        ("later_ignore_on_same_line", b'x=); y=1 # type: ignore[\xff]\n', "SyntaxError@1"),
    ):
        add("supplement/" + name, source, outcome)
    for name, pre, post in (
        ("call_trailing", b"call() # ", b"\nnext_call()\n"),
        ("call_paren", b"x=(call() # ", b"\n)\n"),
        ("call_replacement", b'x=f"{call() # ', b'\n}"\n'),
        ("call_comment", b"# ", b"\ncall()\n"),
    ):
        add("supplement/" + name, pre + b"\xff" + post, control="control/" + name)
        add("control/" + name, pre + b"ASCII" + post)

    # Explicit debug values are independent anchors, not slices of input source.
    debug = (
        ("simple", b'x=f"{call() # \xff\n=}"\n', ("call() \n=",), (1, 5, 1, 11)),
        ("paren", b'x=f"{(call() # \xff\n)=}"\n', ("(call() \n)=",), (1, 6, 1, 12)),
        ("leading", b'x=f"{ # \xff\ncall()=}"\n', (" \ncall()=",), (2, 0, 2, 6)),
        ("nested", b'''x=f"{f'{call() # \xff\n=}'=}"\n''',
         ("f'{call() \n=}'=", "call() \n="), (1, 8, 1, 14)),
        ("nested_plain", b'''x=f"{f'{call() # \xff\n=}'}"\n''',
         ("call() \n=",), (1, 8, 1, 14)),
        ("conversion", b'x=f"{call() # \xff\n=!s}"\n', ("call() \n=",), (1, 5, 1, 11)),
        ("format", b'x=f"{call() # \xff\n=:>8}"\n', ("call() \n=", ">8"), (1, 5, 1, 11)),
        ("crlf", b'x=f"{call() # \xff\r\n=}"\r\n', ("call() \n=",), (1, 5, 1, 11)),
    )
    debug_controls = {
        "simple": b'x=f"{call() # ASCII\n=}"\n',
        "paren": b'x=f"{(call() # ASCII\n)=}"\n',
        "leading": b'x=f"{ # ASCII\ncall()=}"\n',
        "nested": b'''x=f"{f'{call() # ASCII\n=}'=}"\n''',
        "nested_plain": b'''x=f"{f'{call() # ASCII\n=}'}"\n''',
        "conversion": b'x=f"{call() # ASCII\n=!s}"\n',
        "format": b'x=f"{call() # ASCII\n=:>8}"\n',
        "crlf": b'x=f"{call() # ASCII\r\n=}"\r\n',
    }
    debug_spans = {
        "simple": ((1, 5, 2, 1),), "paren": ((1, 5, 2, 2),),
        "leading": ((1, 5, 2, 7),),
        "nested": ((1, 5, 2, 4), (1, 8, 2, 1)),
        "nested_plain": ((1, 8, 2, 1),), "conversion": ((1, 5, 2, 1),),
        "format": ((1, 5, 2, 1), (2, 2, 2, 4)), "crlf": ((1, 5, 2, 1),),
    }
    for name, source, constants, call_span in debug:
        refs = (("Call", *call_span),) + tuple(("Constant", *span) for span in debug_spans[name])
        add("debug/" + name, source, control="debug/" + name + "_ascii",
            constants=constants, refs=refs)
        # A separately parsed comparison input; never used to repair admission.
        add("debug/" + name + "_ascii", debug_controls[name],
            constants=constants, refs=refs)
    add("debug/valid", b'x=f"{call() # \xc3\xa9\n=}"\n', control="debug/simple_ascii",
        constants=("call() \n=",), refs=(("Call", 1, 5, 1, 11), ("Constant", 1, 5, 2, 1)))
    add("debug/later", b'x=f"{call() # \xff\n=}"\ny=)\n', "SyntaxError@3")
    add("debug/type", b'x=f"{call() # type: \xff\n=}"\n', "SyntaxError@1")
    add("debug/ignore", b'x=f"{call() # type: ignore[\xff]\n=}"\n', "UnicodeDecodeError@0")

    anchors = {
        "context/ff/comment": (("Assign", 2, 0, 2, 3),),
        "context/ff/paren": (("Assign", 1, 0, 2, 1),),
        "context/ff/replacement": (("FormattedValue", 1, 4, 2, 1),),
        "unicode/name": (("Name", 1, 0, 1, 2), ("Call", 1, 3, 1, 9)),
        "unicode/string_then_call": (("Constant", 1, 2, 1, 6), ("Call", 1, 8, 1, 14)),
        "unicode/replacement_name": (("Call", 1, 5, 1, 9), ("Name", 1, 5, 1, 7)),
        "unicode/type_defined": (("Assign", 1, 0, 1, 14),),
        "cookie/latin-1/string": (("Constant", 2, 2, 2, 6),),
    }
    return tuple(row._replace(refs=anchors.get(row.name, row.refs)) for row in rows)


def canonical(value):
    return json.dumps(value, ensure_ascii=True, sort_keys=True, separators=(",", ":")).encode("ascii")


def digest(value):
    return hashlib.sha256(canonical(value)).hexdigest()


def manifest(rows):
    return [(r.name, r.source.hex(), r.expected, r.control, r.constants, r.refs) for r in rows]


def require_runtime(implementation=None, version=None, ucd=None, flags=None):
    implementation = sys.implementation.name if implementation is None else implementation
    version = sys.version_info[:3] if version is None else version
    ucd = unicodedata.unidata_version if ucd is None else ucd
    flags = sys.flags if flags is None else flags
    if implementation != "cpython" or version != (3, 12, 11) or ucd != "15.0.0":
        raise ValueError("requires existing trusted CPython 3.12.11 / UCD 15.0.0")
    if not all(getattr(flags, name, False) for name in ("isolated", "no_site", "dont_write_bytecode")):
        raise ValueError("full gate requires -I -S -B")


def validate(rows):
    if type(rows) is not tuple or not 0 < len(rows) <= MAX_CASES:
        raise ValueError("case count/type")
    names = set()
    total = 0
    for row in rows:
        if type(row) is not Case or type(row.source) is not bytes:
            raise ValueError("case/source type")
        if not isinstance(row.name, str) or row.name in names:
            raise ValueError("case name/duplicate")
        names.add(row.name)
        if len(row.source) > MAX_SOURCE:
            raise ValueError("source cap")
        total += len(row.source)
        if total > MAX_TOTAL:
            raise ValueError("aggregate source cap")
        if row.expected != "OK":
            parts = row.expected.split("@") if isinstance(row.expected, str) else []
            if (len(parts) != 2 or parts[0] not in CLASSES - {"OK"}
                    or not parts[1].isascii() or not parts[1].isdigit()
                    or str(int(parts[1])) != parts[1]):
                raise ValueError("expected exception metadata")
        if row.control is not None and row.expected != "OK":
            raise ValueError("failure control")
    by_name = {r.name: r for r in rows}
    for row in rows:
        if row.control is not None and (row.control not in by_name
                                       or by_name[row.control].expected != "OK"
                                       or row.control == row.name):
            raise ValueError("control relationship")
    if rows != matrix():
        raise ValueError("closed matrix changed/unknown/missing/reordered row or metadata")
    if digest(manifest(rows)) != EXPECTED_MATRIX_SHA256:
        raise ValueError("matrix fingerprint")
    return total


def parse_row(row):
    """The sole synthetic-source operation: bytes -> AST, never execution."""
    try:
        tree = ast.parse(row.source, filename="<byte-contract>", type_comments=True)
    except (SyntaxError, ValueError, RecursionError, MemoryError, UnicodeError) as error:
        return type(error).__name__ + "@" + str(getattr(error, "lineno", None) or 0), None
    return "OK", tree


def observe(rows):
    """Internal bounded measurement; run() validates the closed inputs first."""
    counts = Counter()
    ast_hashes = {}
    ast_bytes = 0
    node_count = 0
    for row in rows:
        outcome, tree = parse_row(row)
        if outcome != row.expected:
            raise ValueError("outcome mismatch: " + row.name + ": " + outcome)
        counts[outcome] += 1
        if tree is None:
            continue
        nodes = list(ast.walk(tree))
        if len(nodes) > MAX_NODES:
            raise ValueError("AST node cap")
        node_count += len(nodes)
        if row.constants is not None:
            constants = tuple(n.value for n in nodes if isinstance(n, ast.Constant))
            if constants != row.constants:
                raise ValueError("debug constant mismatch: " + row.name)
        refs = {(type(n).__name__, *(getattr(n, a, None) for a in
                 ("lineno", "col_offset", "end_lineno", "end_col_offset"))) for n in nodes}
        if not set(row.refs) <= refs:
            raise ValueError("UTF-8 coordinate mismatch: " + row.name)
        blob = canonical({"ast": ast.dump(tree, include_attributes=True)})
        ast_bytes += len(blob)
        if len(blob) > MAX_AST_JSON or ast_bytes > MAX_AST_TOTAL:
            raise ValueError("AST JSON cap")
        ast_hashes[row.name] = hashlib.sha256(blob).hexdigest()
    relationships = 0
    for row in rows:
        if row.control is not None:
            if ast_hashes[row.name] != ast_hashes[row.control]:
                raise ValueError("AST control mismatch: " + row.name)
            relationships += 1
    return {
        "profile": PROFILE, "cases": len(rows), "parse_calls": len(rows),
        "source_bytes": sum(len(r.source) for r in rows),
        "max_source_bytes": max(len(r.source) for r in rows),
        "debug_cases": sum(r.name.startswith("debug/") for r in rows),
        "counts": dict(sorted(counts.items())), "ast_nodes": node_count,
        "ast_json_bytes": ast_bytes, "control_relationships": relationships,
        "matrix_sha256": digest(manifest(rows)), "accepted_ast_sha256": digest(ast_hashes),
    }


def run(rows=None):
    require_runtime()
    rows = matrix() if rows is None else rows
    validate(rows)
    report = observe(rows)
    if report["counts"] != EXPECTED_COUNTS:
        raise ValueError("outcome count mismatch")
    if report["accepted_ast_sha256"] != EXPECTED_AST_SHA256:
        raise ValueError("accepted AST fingerprint")
    return report


def main(argv=None):
    argv = sys.argv[1:] if argv is None else argv
    if argv not in ([], ["--json"]):
        print("byte contract: only optional --json is accepted", file=sys.stderr)
        return 2
    try:
        report = run()
    except (ValueError, TypeError):
        print("byte contract: gate failed (runtime, bounds or frozen contract)", file=sys.stderr)
        return 1
    if argv:
        print(canonical(report).decode("ascii"))
    else:
        print("byte contract: PASS cases={cases} calls={parse_calls} debug={debug_cases} "
              "controls={control_relationships} matrix={matrix_sha256} ast={accepted_ast_sha256}".format(**report))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
