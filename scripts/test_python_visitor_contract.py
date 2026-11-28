#!/usr/bin/env python3
"""Closed-inventory tamper tests plus an explicitly isolated full oracle lane.

Discovery can inspect metadata without -I. Run the full gate with:
    python3.12 -I -S -B scripts/test_python_visitor_contract.py
No test executes witness source or creates on-disk scratch/build artifacts.
"""

import copy
import importlib.util
import json
from pathlib import Path
import sys
from types import SimpleNamespace
import unittest
from unittest import mock

SCRIPT = Path(__file__).resolve().with_name("check_python_visitor_contract.py")
SPEC = importlib.util.spec_from_file_location("check_python_visitor_contract", SCRIPT)
check = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(check)
EXACT = sys.implementation.name == "cpython" and sys.version_info[:3] == (3, 12, 11)
ISOLATED = EXACT and sys.flags.isolated and sys.flags.no_site and sys.flags.dont_write_bytecode


def contract_copy():
    with (check.ROOT / check.FILES[0]).open("rb") as stream:
        raw = stream.read(check.LIMIT + 1)
    if len(raw) > check.LIMIT:
        raise ValueError("contract bounded text")
    return json.loads(raw, object_pairs_hook=check.pairs)


def row(name):
    return next(item for item in check.reviewed_inventory() if item["name"] == name)


def witnesses(source="pass\n"):
    return {"profile": check.PROFILE, "cases": [
        {"id": key, "source": source if index == 0 else ""}
        for index, key in enumerate(check.REQUIRED_IDS)
    ]}


class InventoryTests(unittest.TestCase):
    def setUp(self):
        self.contract = contract_copy()

    def test_closed_inventory_and_counts(self):
        rows = check.validate_inventory(self.contract)
        self.assertEqual(len(rows), 111)
        self.assertEqual(sum(item["domain"] == "Module" for item in rows), 108)
        counts = {}
        for item in rows:
            counts[item["category"]] = counts.get(item["category"], 0) + 1
        self.assertEqual(counts, dict(mod=4, stmt=28, expr=27, expr_context=3,
                                     boolop=2, operator=13, unaryop=4, cmpop=10,
                                     excepthandler=1, pattern=8, type_ignore=1,
                                     type_param=3, support=7))
        self.assertFalse(check.LEGACY & {item["name"] for item in rows})
        self.assertEqual(sum(len(item["ordered_fields"]) for item in rows), 188)
        domain_rules = [rule for item in rows if item["domain"] == "Module" for rule in item["field_rules"]]
        self.assertEqual(len(domain_rules), 184)
        self.assertEqual(sum(rule[2] == "unique-call" for rule in domain_rules), 130)
        self.assertEqual(sum(rule[2] == "structural" for rule in domain_rules), 17)

    @unittest.skipUnless(EXACT, "AST metadata comparison requires exact CPython 3.12.11; no toolchain autoload")
    def test_actual_runtime_metadata_without_isolation_is_not_full_gate(self):
        check.metadata_gate(check.validate_inventory(self.contract))

    def test_missing_class(self):
        self.contract["classes"].pop()
        with self.assertRaisesRegex(ValueError, "missing classes"):
            check.validate_inventory(self.contract)

    def test_duplicate_class(self):
        self.contract["classes"].append(copy.deepcopy(self.contract["classes"][0]))
        with self.assertRaisesRegex(ValueError, "duplicate class"):
            check.validate_inventory(self.contract)

    def test_unexpected_class_including_legacy(self):
        for name in ("Unreviewed", "Num", "Suite", "Index"):
            with self.subTest(name=name):
                data = copy.deepcopy(self.contract)
                data["classes"][0]["name"] = name
                with self.assertRaisesRegex(ValueError, "unexpected class"):
                    check.validate_inventory(data)

    def test_missing_duplicate_unexpected_reordered_fields(self):
        for fields in (["body"], ["body", "body"], ["body", "unexpected"], ["type_ignores", "body"]):
            with self.subTest(fields=fields):
                self.contract["classes"][0]["ordered_fields"] = fields
                with self.assertRaisesRegex(ValueError, "fields"):
                    check.validate_inventory(self.contract)

    def test_invalid_disposition_and_edge_proof_and_type(self):
        for index, value in ((0, "scalar"), (1, "unsupported"), (1, "neutral-scalar"), (2, "structural")):
            with self.subTest(index=index, value=value):
                data = copy.deepcopy(self.contract)
                data["classes"][0]["field_rules"][0][index] = value
                with self.assertRaisesRegex(ValueError, "invalid field type/disposition/proof"):
                    check.validate_inventory(data)

    def test_missing_extra_field_rules(self):
        for rules in ([], [["stmt*", "visit", "unique-call"]] * 3):
            data = copy.deepcopy(self.contract)
            data["classes"][0]["field_rules"] = rules
            with self.assertRaisesRegex(ValueError, "invalid field"):
                check.validate_inventory(data)

    def test_no_fake_domain_exclusions(self):
        self.contract["classes"][0]["domain"] = "outside-Module"
        with self.assertRaisesRegex(ValueError, "class policy"):
            check.validate_inventory(self.contract)

    def test_attributes_are_exact_span_metadata(self):
        data = next(item for item in self.contract["classes"] if item["name"] == "Call")
        self.assertEqual(data["attributes"], check.SPAN)
        data["attributes"].pop()
        with self.assertRaisesRegex(ValueError, "class policy"):
            check.validate_inventory(self.contract)

    def test_profile_narrowing(self):
        self.contract["profile"] = "subset"
        with self.assertRaisesRegex(ValueError, "profile narrowing"):
            check.validate_inventory(self.contract)

    def test_witness_links_cannot_be_narrowed(self):
        self.contract["witness_groups"]["module"]["required_ids"].pop()
        with self.assertRaisesRegex(ValueError, "witness group narrowing"):
            check.validate_inventory(self.contract)

    def test_class_witness_link_cannot_be_removed(self):
        self.contract["classes"][0]["witnesses"] = []
        with self.assertRaisesRegex(ValueError, "class policy"):
            check.validate_inventory(self.contract)

    def test_orders_cannot_be_removed_or_weakened(self):
        self.contract["order_obligations"][0]["groups"].pop()
        with self.assertRaisesRegex(ValueError, "order obligations"):
            check.validate_inventory(self.contract)

    def test_unknown_contract_and_class_keys(self):
        for target in (self.contract, self.contract["classes"][0]):
            with self.subTest(target=list(target)):
                target["surprise"] = True
                with self.assertRaisesRegex(ValueError, "unknown or missing JSON keys"):
                    check.validate_inventory(self.contract)
                del target["surprise"]


class BoundedInputTests(unittest.TestCase):
    def setUp(self):
        self.blobs = {check.FILES[0]: b"{}", check.FILES[1]: b"{}"}

    def test_missing_and_extra_input_files(self):
        for blobs in ({}, dict(self.blobs, unexpected=b"{}")):
            with self.assertRaisesRegex(ValueError, "input files"):
                check.decode_blobs(blobs)

    def test_whitespace_bytes_cap_before_json_parse(self):
        self.blobs[check.FILES[0]] = b" " * (check.LIMIT + 1)
        with mock.patch.object(check.json, "loads", side_effect=AssertionError("must not decode")):
            with self.assertRaisesRegex(ValueError, "bounded text"):
                check.decode_blobs(self.blobs)

    def test_utf8_bytes_not_character_count(self):
        self.blobs[check.FILES[0]] = ('"' + "\u00e9" * (check.LIMIT // 2) + '"').encode()
        with self.assertRaisesRegex(ValueError, "bounded text"):
            check.decode_blobs(self.blobs)

    def test_aggregate_cap(self):
        with mock.patch.object(check, "TOTAL_LIMIT", 3):
            with self.assertRaisesRegex(ValueError, "aggregate cap"):
                check.decode_blobs(self.blobs)

    def test_nul_and_nonbytes_refused(self):
        for raw in (b"\0", "{}"):
            self.blobs[check.FILES[0]] = raw
            with self.assertRaisesRegex(ValueError, "bounded text"):
                check.decode_blobs(self.blobs)

    def test_invalid_utf8(self):
        self.blobs[check.FILES[0]] = b"\xff"
        with self.assertRaises(UnicodeDecodeError):
            check.decode_blobs(self.blobs)

    def test_duplicate_json_keys_and_nonfinite_constants(self):
        for raw in (b'{"profile":1,"profile":2}', b'{"x":NaN}', b'{"x":Infinity}'):
            with self.subTest(raw=raw):
                self.blobs[check.FILES[0]] = raw
                with self.assertRaises(ValueError):
                    check.decode_blobs(self.blobs)

    def test_fixed_paths_read_bounded_and_refuse_symlinks(self):
        with mock.patch.object(Path, "is_symlink", return_value=True):
            with mock.patch.object(Path, "open", side_effect=AssertionError("no read")):
                with self.assertRaisesRegex(ValueError, "symlink"):
                    check.read_blobs()

    def test_missing_witness_file_is_failure_not_success(self):
        with mock.patch.object(Path, "open", side_effect=FileNotFoundError("witness missing")):
            with self.assertRaises(FileNotFoundError):
                check.read_blobs()


@unittest.skipUnless(EXACT, "Synthetic AST coverage requires exact CPython 3.12.11")
class CoverageTests(unittest.TestCase):
    def run_coverage(self, names, data):
        # Small independent unit inputs, not a narrowed production gate. validate
        # always validates the full immutable inventory before calling coverage.
        with mock.patch.object(check, "ORDER_OBLIGATIONS", []):
            return check.coverage([row(name) for name in names], data)

    def test_missing_class_occurrence(self):
        with self.assertRaisesRegex(ValueError, "classes=Return"):
            self.run_coverage(["Return"], witnesses())

    def test_class_occurrence_is_not_edge_proof(self):
        with self.assertRaisesRegex(ValueError, r"Return.value \(unique-call\)"):
            self.run_coverage(["Return"], witnesses("return\n"))

    def test_populated_edge_without_observable_call_rejected(self):
        with self.assertRaisesRegex(ValueError, "Return.value"):
            self.run_coverage(["Return"], witnesses("return 42\n"))

    def test_unique_child_call_proves_edge_input_not_order(self):
        result = self.run_coverage(["Return"], witnesses("return unique_return_tag()\n"))
        self.assertEqual(result["observable_edges"], 1)
        self.assertIn("not certified", result["order_parity"])

    def test_reused_call_tag_is_not_unique_edge_identity(self):
        with self.assertRaisesRegex(ValueError, "Return.value"):
            self.run_coverage(["Return"], witnesses("return reused()\nreused()\n"))

    def test_structural_marker_edge_does_not_need_calls(self):
        result = self.run_coverage(["Name"], witnesses("identifier\n"))
        self.assertEqual(result["structural_edges"], 1)
        self.assertEqual(result["observable_edges"], 0)

    def test_non_ast_scalars_do_not_require_child_witnesses(self):
        result = self.run_coverage(["TypeIgnore"], witnesses("# type: ignore[synthetic]\n"))
        self.assertEqual(result["node_fields"], 0)

    def test_duplicate_ids_even_with_different_source(self):
        data = witnesses()
        data["cases"].append(dict(data["cases"][0], source="other()"))
        with self.assertRaisesRegex(ValueError, "duplicate witness id"):
            self.run_coverage([], data)

    def test_missing_fixed_witness(self):
        data = witnesses()
        data["cases"].pop()
        with self.assertRaisesRegex(ValueError, "missing witness cases"):
            self.run_coverage([], data)

    def test_additional_witnesses_are_allowed_and_linked(self):
        data = witnesses()
        data["cases"].append({"id": "additional_synthetic", "source": "return additional_tag()"})
        self.assertEqual(self.run_coverage(["Return"], data)["cases"], 15)

    def test_unknown_witness_and_case_keys(self):
        for case_key in (False, True):
            data = witnesses()
            (data["cases"][0] if case_key else data)["path"] = "not-an-input-path.py"
            with self.assertRaisesRegex(ValueError, "unknown or missing JSON keys"):
                self.run_coverage([], data)

    def test_wrong_witness_profile(self):
        data = witnesses()
        data["profile"] = "subset"
        with self.assertRaisesRegex(ValueError, "profile narrowing"):
            self.run_coverage([], data)

    def test_witness_source_is_never_executed(self):
        source = "raise RuntimeError('must not execute')\nimport definitely_not_an_installed_module\nopen('/nonexistent/forbidden-write', 'w')\n"
        with mock.patch("builtins.open", side_effect=AssertionError("source performed IO")):
            self.run_coverage([], witnesses(source))

    def test_order_inputs_must_share_one_parent(self):
        order = {"id": "independent", "cases": [check.REQUIRED_IDS[0]], "groups": ["If.body", "If.orelse"]}
        data = witnesses("if x:\n first_body()\nif y:\n pass\nelse:\n second_else()\n")
        with mock.patch.object(check, "ORDER_OBLIGATIONS", [order]):
            with self.assertRaisesRegex(ValueError, "order inputs=independent"):
                check.coverage([], data)

    def test_observable_order_inputs_do_not_certify_output_order(self):
        order = {"id": "independent", "cases": [check.REQUIRED_IDS[0]], "groups": ["If.body", "If.orelse"]}
        data = witnesses("if x:\n first_body()\nelse:\n second_else()\n")
        with mock.patch.object(check, "ORDER_OBLIGATIONS", [order]):
            self.assertIn("not certified", check.coverage([], data)["order_parity"])


class RuntimeTests(unittest.TestCase):
    def test_full_gate_refuses_unisolated_runtime(self):
        flags = SimpleNamespace(isolated=0, no_site=0, dont_write_bytecode=1)
        with mock.patch.object(sys, "version_info", (3, 12, 11)):
            with mock.patch.object(sys, "flags", flags):
                with self.assertRaisesRegex(ValueError, "-I -S -B"):
                    check.trusted_runtime()

    def test_exact_patch_required(self):
        with mock.patch.object(sys, "version_info", (3, 12, 12)):
            with self.assertRaisesRegex(ValueError, "exact CPython 3.12.11"):
                check.trusted_runtime()

    def test_runtime_checked_before_file_decode(self):
        with mock.patch.object(check, "trusted_runtime", side_effect=ValueError("runtime refused")):
            with mock.patch.object(check, "decode_blobs", side_effect=AssertionError("must not decode")):
                with self.assertRaisesRegex(ValueError, "runtime refused"):
                    check.validate({})


@unittest.skipUnless(ISOLATED, "Full visitor oracle requires trusted exact CPython 3.12.11 with -I -S -B; run scripts/test_python_visitor_contract.py explicitly with those flags")
class IsolatedOracleTests(unittest.TestCase):
    def test_full_checked_in_contract_and_witness_coverage(self):
        result = check.validate(check.read_blobs())
        self.assertEqual((result["classes"], result["module_classes"]), (111, 108))


if __name__ == "__main__":
    unittest.main()
