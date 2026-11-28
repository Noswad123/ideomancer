#!/usr/bin/env python3
"""Finite byte-study tamper tests; full lane requires trusted 3.12.11 -I -S -B.

Default discovery skips only runtime-dependent oracle tests when not isolated.
No disk fixtures, subprocesses, target imports or synthetic-source execution.
"""

import contextlib
import importlib.util
import io
from pathlib import Path
import sys
from types import SimpleNamespace
import unittest
from unittest import mock

SPEC = importlib.util.spec_from_file_location(
    "check_python_byte_contract", Path(__file__).resolve().with_name("check_python_byte_contract.py"))
check = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(check)
ISOLATED = (sys.implementation.name == "cpython" and sys.version_info[:3] == (3, 12, 11)
            and check.unicodedata.unidata_version == "15.0.0"
            and sys.flags.isolated and sys.flags.no_site and sys.flags.dont_write_bytecode)
REASON = "full byte oracle requires existing trusted CPython 3.12.11/UCD15 with -I -S -B"


def changed(case_name, **fields):
    return tuple(r._replace(**fields) if r.name == case_name else r for r in check.matrix())


class ContractTests(unittest.TestCase):
    def test_closed_counts_and_fingerprint(self):
        rows = check.matrix()
        self.assertEqual(len(rows), 219)
        self.assertEqual(check.validate(rows), 3769)
        self.assertEqual(max(len(r.source) for r in rows), 40)
        self.assertEqual(sum(r.name.startswith("debug/") for r in rows), 20)
        self.assertEqual(sum(r.name.startswith("control/") for r in rows), 9)
        self.assertEqual(sum(r.control is not None for r in rows), 53)
        self.assertEqual(dict(check.Counter(r.expected for r in rows)), check.EXPECTED_COUNTS)
        self.assertEqual(check.digest(check.manifest(rows)), check.EXPECTED_MATRIX_SHA256)

    def test_matrix_is_fresh_and_immutable(self):
        rows = check.matrix()
        with self.assertRaises(AttributeError):
            rows[0].source = b"pass\n"
        altered = changed(rows[0].name, source=b"pass\n")
        self.assertNotEqual(altered, rows)
        self.assertEqual(rows, check.matrix())

    def test_missing_unknown_duplicate_reordered_rows(self):
        rows = check.matrix()
        for bad in (rows[:-1], rows + (rows[0]._replace(name="unknown"),),
                    rows + (rows[0],), rows[::-1], rows[1:] + (rows[0]._replace(name="unknown"),)):
            with self.subTest(size=len(bad)), self.assertRaises(ValueError):
                check.validate(bad)

    def test_expected_metadata_cannot_be_weakened(self):
        for expected in ("OK", "SyntaxError@2", "Exception@0", "SyntaxError@-1",
                         "SyntaxError@01", "SyntaxError@", "SyntaxError@\u0661", None):
            with self.subTest(expected=expected), self.assertRaises(ValueError):
                check.validate(changed("context/ff/string", expected=expected))

    def test_source_and_debug_metadata_tamper(self):
        for fields in ({"source": b"pass\n"}, {"constants": ("wrong",)},
                       {"refs": (("Call", 1, 6, 1, 12),)},
                       {"control": "debug/paren_ascii"}):
            with self.subTest(fields=tuple(fields)), self.assertRaises(ValueError):
                check.validate(changed("debug/simple", **fields))

    def test_control_relationship_validation(self):
        for name in ("missing", "context/ff/string", "debug/simple"):
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "control relationship"):
                check.validate(changed("debug/simple", control=name))
        with self.assertRaisesRegex(ValueError, "failure control"):
            check.validate(changed("context/ff/string", control="control/comment"))

    def test_case_and_source_types(self):
        rows = check.matrix()
        for bad in ([], (), list(rows), (object(),), rows * 2,
                    changed(rows[0].name, source="pass"), changed(rows[0].name, name=None)):
            with self.subTest(kind=type(bad).__name__), self.assertRaises(ValueError):
                check.validate(bad)

    def test_source_caps_before_parse(self):
        with mock.patch.object(check.ast, "parse", side_effect=AssertionError("must not parse")):
            with self.assertRaisesRegex(ValueError, "source cap"):
                check.validate(changed("debug/simple", source=b"x" * (check.MAX_SOURCE + 1)))
            with mock.patch.object(check, "MAX_TOTAL", 10):
                with self.assertRaisesRegex(ValueError, "aggregate source cap"):
                    check.validate(check.matrix())

    def test_manifest_fingerprint_tamper(self):
        with mock.patch.object(check, "EXPECTED_MATRIX_SHA256", "0" * 64):
            with self.assertRaisesRegex(ValueError, "matrix fingerprint"):
                check.validate(check.matrix())

    def test_runtime_pin(self):
        good = dict(implementation="cpython", version=(3, 12, 11), ucd="15.0.0",
                    flags=SimpleNamespace(isolated=1, no_site=1, dont_write_bytecode=1))
        check.require_runtime(**good)
        for changes in ({"implementation": "pypy"}, {"version": (3, 12, 12)},
                        {"version": (3, 13, 0)}, {"ucd": "16.0.0"}):
            with self.subTest(changes=changes), self.assertRaisesRegex(ValueError, "requires existing"):
                check.require_runtime(**(good | changes))

    def test_each_required_flag(self):
        for name in ("isolated", "no_site", "dont_write_bytecode"):
            flags = dict(isolated=1, no_site=1, dont_write_bytecode=1)
            flags[name] = 0
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "-I -S -B"):
                check.require_runtime("cpython", (3, 12, 11), "15.0.0", SimpleNamespace(**flags))

    def test_gate_checks_runtime_and_inputs_before_parsing(self):
        with mock.patch.object(check, "require_runtime", side_effect=ValueError("runtime")):
            with mock.patch.object(check.ast, "parse") as parser:
                with self.assertRaisesRegex(ValueError, "runtime"):
                    check.run()
                parser.assert_not_called()
        with mock.patch.object(check, "require_runtime"):
            with mock.patch.object(check.ast, "parse") as parser:
                with self.assertRaises(ValueError):
                    check.run(changed("debug/simple", source=b"pass\n"))
                parser.assert_not_called()

    def test_cli_rejects_paths_and_extra_arguments(self):
        for argv in (["anything.py"], ["--source", "anything.py"], ["--json", "extra"]):
            with contextlib.redirect_stderr(io.StringIO()), mock.patch.object(check, "run") as run:
                self.assertEqual(check.main(argv), 2)
                run.assert_not_called()

    def test_compact_output_and_redacted_failures(self):
        report = dict(cases=219, parse_calls=219, debug_cases=20, control_relationships=53,
                      matrix_sha256="a" * 64, accepted_ast_sha256="b" * 64)
        for argv in ([], ["--json"]):
            output = io.StringIO()
            with mock.patch.object(check, "run", return_value=report), contextlib.redirect_stdout(output):
                self.assertEqual(check.main(argv), 0)
            self.assertLess(len(output.getvalue()), 1024)
            self.assertEqual(output.getvalue().count("\n"), 1)
        error = io.StringIO()
        with mock.patch.object(check, "run", side_effect=ValueError("PRIVATE SOURCE")):
            with contextlib.redirect_stderr(error):
                self.assertEqual(check.main([]), 1)
        self.assertNotIn("PRIVATE", error.getvalue())

    def test_concrete_exception_class_and_normalized_line(self):
        row = check.matrix()[0]
        for error, expected in ((UnicodeDecodeError("utf8", b"\xff", 0, 1, "not emitted"),
                                 "UnicodeDecodeError@0"),
                                (SyntaxError("not emitted"), "SyntaxError@0"),
                                (IndentationError("not emitted", ("private", 7, 1, "private")),
                                 "IndentationError@7")):
            with mock.patch.object(check.ast, "parse", side_effect=error):
                self.assertEqual(check.parse_row(row), (expected, None))

    def test_debug_quote_bytes_and_constants_are_explicit(self):
        rows = {r.name: r for r in check.matrix()}
        self.assertEqual(rows["debug/nested"].source,
                         b'x=f"{f\'{call() # \xff\n=}\'=}"\n')
        self.assertNotIn(b"\\u0027", rows["debug/nested"].source)
        self.assertEqual(rows["debug/nested"].constants, ("f'{call() \n=}'=", "call() \n="))
        self.assertEqual(rows["debug/leading"].constants, (" \ncall()=",))


@unittest.skipUnless(ISOLATED, REASON)
class IsolatedOracleTests(unittest.TestCase):
    def test_full_gate_deterministic_and_no_input_mutation(self):
        rows = check.matrix()
        before = check.digest(check.manifest(rows))
        original = check.ast.parse
        with mock.patch.object(check.ast, "parse", wraps=original) as parser:
            report = check.run(rows)
        self.assertEqual(parser.call_count, 219)
        for call in parser.call_args_list:
            self.assertIs(type(call.args[0]), bytes)
            self.assertEqual(call.kwargs, dict(filename="<byte-contract>", type_comments=True))
        self.assertEqual(report["counts"], check.EXPECTED_COUNTS)
        self.assertEqual(report["accepted_ast_sha256"], check.EXPECTED_AST_SHA256)
        self.assertEqual(report["control_relationships"], 53)
        self.assertEqual(before, check.digest(check.manifest(rows)))
        self.assertEqual(report, check.run())

    def test_no_eval_exec_or_source_call_execution(self):
        row = next(r for r in check.matrix() if r.name == "debug/nested")
        with mock.patch("builtins.eval", side_effect=AssertionError("no eval")):
            with mock.patch("builtins.exec", side_effect=AssertionError("no exec")):
                outcome, tree = check.parse_row(row)
        self.assertEqual(outcome, "OK")
        self.assertIsInstance(tree, check.ast.Module)
        self.assertNotIn("call", vars(check))

    def test_ast_hash_tamper(self):
        with mock.patch.object(check, "EXPECTED_AST_SHA256", "0" * 64):
            with self.assertRaisesRegex(ValueError, "accepted AST fingerprint"):
                check.run()

    def test_count_tamper(self):
        with mock.patch.object(check, "EXPECTED_COUNTS", {}):
            with self.assertRaisesRegex(ValueError, "outcome count mismatch"):
                check.run()

    def test_outcome_and_debug_and_coordinate_mismatch(self):
        row = next(r for r in check.matrix() if r.name == "debug/simple")
        for fields, message in (({"expected": "SyntaxError@1"}, "outcome mismatch"),
                                ({"constants": ("wrong",)}, "debug constant mismatch"),
                                ({"refs": (("Call", 9, 0, 9, 1),)}, "UTF-8 coordinate mismatch")):
            with self.subTest(fields=tuple(fields)), self.assertRaisesRegex(ValueError, message):
                check.observe((row._replace(control=None, **fields),))

    def test_control_mismatch(self):
        rows = {r.name: r for r in check.matrix()}
        bad = rows["debug/simple"]._replace(control="debug/paren_ascii")
        with self.assertRaisesRegex(ValueError, "AST control mismatch"):
            check.observe((bad, rows["debug/paren_ascii"]))

    def test_ast_budgets(self):
        row = next(r for r in check.matrix() if r.name == "debug/simple_ascii")
        for name, message in (("MAX_NODES", "AST node cap"), ("MAX_AST_JSON", "AST JSON cap"),
                              ("MAX_AST_TOTAL", "AST JSON cap")):
            with self.subTest(name=name), mock.patch.object(check, name, 1):
                with self.assertRaisesRegex(ValueError, message):
                    check.observe((row,))


if __name__ == "__main__":
    unittest.main()
