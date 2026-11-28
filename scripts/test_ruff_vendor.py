"""Bounded integrity tests; TemporaryDirectory honors the caller's TMPDIR."""

import copy
import difflib
import json
import os
from pathlib import Path
import re
import tempfile
import unittest
from unittest.mock import patch

import check_ruff_vendor as checker


VENDOR = Path(__file__).resolve().parents[1] / "vendor" / "ruff"


class RuffInventoryTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="ruff-integrity-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.content = {"LICENSE": b"synthetic license\n", "crates/demo/src/lib.rs": b"pub fn demo() {}\n"}
        self.records = {}
        for name, data in self.content.items():
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
            self.records[name] = {"bytes": len(data), "sha256": checker.digest(data)}

    def test_bounded_synthetic_inventory(self):
        checker.validate_inventory(self.root, self.records)

    def test_changed_source(self):
        (self.root / "LICENSE").write_bytes(b"Synthetic license\n")
        with self.assertRaisesRegex(checker.VendorError, "hash mismatch"):
            checker.validate_inventory(self.root, self.records)

    def test_missing_source(self):
        (self.root / "LICENSE").unlink()
        with self.assertRaisesRegex(checker.VendorError, "missing files"):
            checker.validate_inventory(self.root, self.records)

    def test_extra_file(self):
        (self.root / "extra").write_bytes(b"unexpected")
        with self.assertRaisesRegex(checker.VendorError, "extra file"):
            checker.validate_inventory(self.root, self.records)

    def test_extra_empty_directory(self):
        (self.root / ".git").mkdir()
        with self.assertRaisesRegex(checker.VendorError, "extra directory"):
            checker.validate_inventory(self.root, self.records)

    def test_file_symlink(self):
        path = self.root / "LICENSE"
        path.unlink()
        path.symlink_to("crates/demo/src/lib.rs")
        with self.assertRaisesRegex(checker.VendorError, "symlink"):
            checker.validate_inventory(self.root, self.records)

    def test_directory_symlink(self):
        (self.root / "alias").symlink_to("crates", target_is_directory=True)
        with self.assertRaisesRegex(checker.VendorError, "symlink"):
            checker.validate_inventory(self.root, self.records)

    @unittest.skipUnless(hasattr(os, "mkfifo"), "requires POSIX FIFO")
    def test_special_file(self):
        os.mkfifo(self.root / "fifo")
        with self.assertRaisesRegex(checker.VendorError, "special file"):
            checker.validate_inventory(self.root, self.records)

    def test_unsafe_paths(self):
        for name in ("../escape", "/absolute", "a//b", "a/./b", "a/../b", "a\\b", "", "C:/escape", "a\nb"):
            with self.subTest(name=name), self.assertRaises(checker.VendorError):
                checker.safe_path(name)

    def test_invalid_record_fields_and_bounds(self):
        for item in ({"bytes": True, "sha256": "0" * 64},
                     {"bytes": checker.MAX_FILE_BYTES + 1, "sha256": "0" * 64},
                     {"bytes": 1, "sha256": "invalid"},
                     {"bytes": 1, "sha256": "0" * 64, "unknown": 0}):
            with self.subTest(item=item), self.assertRaises(checker.VendorError):
                checker.validate_records({"file": item})

    def test_record_count_and_total_bounds(self):
        records = {f"file{i}": {"bytes": 0, "sha256": "0" * 64} for i in range(100)}
        checker.validate_records(records)
        with self.assertRaisesRegex(checker.VendorError, "inventory size"):
            checker.validate_records({**records, "extra": records["file0"]})
        for item in records.values():
            item["bytes"] = checker.MAX_FILE_BYTES
        with self.assertRaisesRegex(checker.VendorError, "byte bound"):
            checker.validate_records(records)


class RuffProvenanceTests(unittest.TestCase):
    def setUp(self):
        self.raw = (VENDOR / "provenance.json").read_bytes()
        self.value = json.loads(self.raw)

    def test_reviewed_snapshot(self):
        result = checker.check_vendor(VENDOR)
        self.assertEqual(result["sources"], 67)
        self.assertEqual(result["files"], 89)
        self.assertEqual(result["pristine_sources"], 65)
        self.assertEqual(result["local_sources"], 2)
        self.assertEqual(result["modified_upstream_sources"], 14)
        self.assertEqual(result["unmodified_upstream_sources"], 51)
        self.assertEqual(len(self.value["files"]), 88)

    def test_unknown_fields(self):
        self.value["unreviewed"] = True
        with self.assertRaisesRegex(checker.VendorError, "fields"):
            checker.parse_provenance(json.dumps(self.value).encode())

    def test_duplicate_json_keys(self):
        raw = self.raw.replace(b'"schema_version": 3', b'"schema_version": 3, "schema_version": 3')
        self.assertNotEqual(raw, self.raw)
        with self.assertRaisesRegex(checker.VendorError, "duplicate JSON"):
            checker.parse_provenance(raw)

    def test_pin_tag_archive_lock_changes(self):
        for field in ("commit", "tag", "archive_sha256", "archive_url", "lock_sha256", "repository"):
            value = copy.deepcopy(self.value)
            value["upstream"][field] = "unreviewed"
            with self.subTest(field=field), self.assertRaisesRegex(checker.VendorError, "metadata"):
                checker.parse_provenance(json.dumps(value).encode())

    def test_changed_source_and_sidecar_are_not_self_proving(self):
        name = next(iter(self.value["pristine_sources"]))
        self.value["pristine_sources"][name]["sha256"] = "0" * 64
        self.value["files"][name]["sha256"] = "0" * 64
        with self.assertRaisesRegex(checker.VendorError, "pristine source reference"):
            checker.parse_provenance(json.dumps(self.value).encode())

    def test_changed_manifest_hash_is_not_self_proving(self):
        self.value["files"]["crates/ruff_text_size/Cargo.toml"]["sha256"] = "0" * 64
        with self.assertRaisesRegex(checker.VendorError, "provenance digest"):
            checker.parse_provenance(json.dumps(self.value).encode())

    def test_new_policy_source_and_sidecar_are_not_self_proving(self):
        for name in checker.POLICY_FILES[2:]:
            value = copy.deepcopy(self.value)
            value["files"][name]["sha256"] = "0" * 64
            value["byte_parser_experiment"][name]["patched_sha256"] = "0" * 64
            with self.subTest(name=name), self.assertRaisesRegex(checker.VendorError, "provenance digest"):
                checker.parse_provenance(json.dumps(value).encode())

    def test_policy_membership_cannot_expand_or_shrink(self):
        for name in checker.POLICY_FILES:
            value = copy.deepcopy(self.value)
            del value["policy"][name]
            with self.subTest(name=name), self.assertRaisesRegex(checker.VendorError, "policy files"):
                checker.parse_provenance(json.dumps(value).encode())
        self.value["policy"]["extra.rs"] = {}
        with self.assertRaisesRegex(checker.VendorError, "policy files"):
            checker.parse_provenance(json.dumps(self.value).encode())

    def test_missing_provenance_field(self):
        del self.value["policy"]
        with self.assertRaisesRegex(checker.VendorError, "fields"):
            checker.parse_provenance(json.dumps(self.value).encode())

    def test_schema_version_is_exact(self):
        for version in (1, 2, 4, True, 3.0, "3", None):
            self.value["schema_version"] = version
            with self.subTest(version=version), self.assertRaisesRegex(checker.VendorError, "schema version"):
                checker.parse_provenance(json.dumps(self.value).encode())

    def test_f1_membership_cannot_expand_shrink_or_cross_groups(self):
        for name in checker.BYTE_LEXER_FILES:
            for replacement in (None, checker.POLICY_FILES[0], checker.BYTE_SOURCE, "extra.rs"):
                value = copy.deepcopy(self.value)
                record = value["byte_lexer_experiment"].pop(name)
                if replacement is not None:
                    value["byte_lexer_experiment"][replacement] = record
                with self.subTest(name=name, replacement=replacement), self.assertRaisesRegex(
                        checker.VendorError, "unexpected byte_lexer_experiment files"):
                    checker.parse_provenance(json.dumps(value).encode())

    def test_patch_mappings_require_both_exact_hashes(self):
        for group, names in (("policy", checker.POLICY_FILES),
                             ("byte_lexer_experiment", checker.BYTE_LEXER_FILES),
                             ("byte_parser_experiment", checker.BYTE_PARSER_FILES)):
            for name in names:
                for field in ("original_sha256", "patched_sha256", "unknown"):
                    value = copy.deepcopy(self.value)
                    value[group][name][field] = "0" * 64
                    with self.subTest(group=group, name=name, field=field), self.assertRaisesRegex(
                            checker.VendorError, "hash mapping"):
                        checker.parse_provenance(json.dumps(value).encode())
                value = copy.deepcopy(self.value)
                del value["files"][name]
                with self.subTest(name=name), self.assertRaisesRegex(checker.VendorError, "missing .* inventory"):
                    checker.parse_provenance(json.dumps(value).encode())

    def test_f1_source_and_sidecar_are_not_self_proving(self):
        for name in checker.BYTE_LEXER_FILES:
            value = copy.deepcopy(self.value)
            value["files"][name]["sha256"] = checker.digest((VENDOR / name).read_bytes() + b"// extra\n")
            value["byte_lexer_experiment"][name]["patched_sha256"] = value["files"][name]["sha256"]
            with self.subTest(name=name), self.assertRaisesRegex(checker.VendorError, "provenance digest"):
                checker.parse_provenance(json.dumps(value).encode())

    def test_each_patch_has_an_independent_anchor(self):
        for name in ("patches/ast-only-policy.patch", "patches/byte-lexer-experiment.patch",
                     "patches/byte-parser-experiment.patch"):
            value = copy.deepcopy(self.value)
            value["files"][name]["sha256"] = checker.digest((VENDOR / name).read_bytes() + b"\n")
            with self.subTest(name=name), self.assertRaisesRegex(checker.VendorError, "reviewed patch hash"):
                checker.parse_provenance(json.dumps(value).encode())

    def test_local_source_and_sidecars_have_an_independent_anchor(self):
        for name in (checker.BYTE_SOURCE, checker.BYTE_PARSER_SOURCE):
            value = copy.deepcopy(self.value)
            changed_hash = checker.digest((VENDOR / name).read_bytes() + b"// extra\n")
            value["files"][name]["sha256"] = changed_hash
            value["local_sources"][name]["sha256"] = changed_hash
            with self.subTest(name=name), self.assertRaisesRegex(checker.VendorError, "reviewed local source hash"):
                checker.parse_provenance(json.dumps(value).encode())

    def test_local_membership_is_exact(self):
        for local in ({}, {"extra.rs": {}}, {**self.value["local_sources"], "extra.rs": {}},
                      {checker.BYTE_SOURCE: self.value["local_sources"][checker.BYTE_SOURCE]},
                      {checker.BYTE_PARSER_SOURCE: self.value["local_sources"][checker.BYTE_PARSER_SOURCE]}):
            self.value["local_sources"] = local
            with self.subTest(local=local), self.assertRaisesRegex(checker.VendorError, "local source files"):
                checker.parse_provenance(json.dumps(self.value).encode())

    def test_local_metadata_and_record_must_match_inventory(self):
        name = checker.BYTE_SOURCE
        for field, replacement in (("origin", "upstream"), ("license", "Apache-2.0"),
                                   ("bytes", 0), ("sha256", "0" * 64), ("extra", True)):
            value = copy.deepcopy(self.value)
            value["local_sources"][name][field] = replacement
            with self.subTest(field=field), self.assertRaisesRegex(checker.VendorError, "local source metadata"):
                checker.parse_provenance(json.dumps(value).encode())
        for field in self.value["local_sources"][name]:
            value = copy.deepcopy(self.value)
            del value["local_sources"][name][field]
            with self.subTest(missing=field), self.assertRaisesRegex(checker.VendorError, "local source metadata"):
                checker.parse_provenance(json.dumps(value).encode())
        del self.value["files"][name]
        with self.assertRaisesRegex(checker.VendorError, "missing local source inventory"):
            checker.parse_provenance(json.dumps(self.value).encode())

    def test_local_source_cannot_be_claimed_as_pristine(self):
        self.value["pristine_sources"][checker.BYTE_SOURCE] = self.value["files"][checker.BYTE_SOURCE]
        with self.assertRaisesRegex(checker.VendorError, "pristine source membership"):
            checker.parse_provenance(json.dumps(self.value).encode())

    def test_source_set_is_exact_union(self):
        unchanged = next(name for name in self.value["pristine_sources"]
                         if name not in checker.POLICY_FILES + checker.BYTE_LEXER_FILES + checker.BYTE_PARSER_FILES)
        for name in ("crates/ruff_python_parser/src/extra.rs", unchanged):
            value = copy.deepcopy(self.value)
            if name in value["files"]:
                del value["files"][name]
            else:
                value["files"][name] = {"bytes": 0, "sha256": "0" * 64}
            with self.subTest(name=name), self.assertRaisesRegex(checker.VendorError, "source set"):
                checker.parse_provenance(json.dumps(value).encode())

    def test_modified_upstream_set_is_exact(self):
        name = next(name for name in self.value["pristine_sources"]
                    if name not in checker.POLICY_FILES + checker.BYTE_LEXER_FILES + checker.BYTE_PARSER_FILES)
        self.value["files"][name]["sha256"] = "0" * 64
        with self.assertRaisesRegex(checker.VendorError, "modified upstream source set"):
            checker.parse_provenance(json.dumps(self.value).encode())

    def test_provenance_cannot_inventory_itself(self):
        self.value["files"]["provenance.json"] = {"bytes": len(self.raw), "sha256": checker.digest(self.raw)}
        with self.assertRaisesRegex(checker.VendorError, "inventory itself"):
            checker.parse_provenance(json.dumps(self.value).encode())

    def test_reviewed_file_count_is_exact(self):
        self.value["files"]["extra.txt"] = {"bytes": 0, "sha256": "0" * 64}
        with self.assertRaisesRegex(checker.VendorError, "reviewed file count"):
            checker.parse_provenance(json.dumps(self.value).encode())

    def test_provenance_byte_bound(self):
        with self.assertRaisesRegex(checker.VendorError, "provenance exceeds"):
            checker.parse_provenance(b" " * (128 * 1024 + 1))


class RuffSchemaThreeTests(unittest.TestCase):
    """Exercise layer contracts without blessing a still-moving source snapshot."""

    def setUp(self):
        self.value = json.loads((VENDOR / "provenance.json").read_bytes())
        self.value["schema_version"] = 3
        files = self.value["files"]
        self.value["byte_parser_experiment"] = {}
        for name in checker.BYTE_PARSER_FILES:
            original = checker.POLICY_OUTPUT_SHA256.get(name, self.value["pristine_sources"][name]["sha256"])
            content = f"synthetic F2 {name}\n".encode()
            files[name] = {"bytes": len(content), "sha256": checker.digest(content)}
            self.value["byte_parser_experiment"][name] = {
                "original_sha256": original, "patched_sha256": files[name]["sha256"],
            }
        for name in (checker.BYTE_SOURCE, checker.BYTE_PARSER_SOURCE):
            content = f"synthetic authored {name}\n".encode()
            files[name] = {"bytes": len(content), "sha256": checker.digest(content)}
            self.value["local_sources"][name] = {
                **files[name], "origin": "Ideomancer-authored", "license": "MIT",
            }
        self.value["local_sources"][checker.BYTE_SOURCE]["previous_sha256"] = checker.F1_BYTE_SOURCE_SHA256
        patch_name = "patches/byte-parser-experiment.patch"
        files[patch_name] = {"bytes": 10, "sha256": checker.digest(b"synthetic\n")}
        self.raw = json.dumps(self.value).encode()
        anchors = {
            "PROVENANCE_SHA256": checker.digest(self.raw),
            "BYTE_SOURCE_SHA256": files[checker.BYTE_SOURCE]["sha256"],
            "BYTE_PARSER_SOURCE_SHA256": files[checker.BYTE_PARSER_SOURCE]["sha256"],
            "BYTE_PARSER_PATCH_SHA256": files[patch_name]["sha256"],
        }
        patcher = patch.multiple(checker, **anchors)
        self.addCleanup(patcher.stop)
        patcher.start()

    def reject(self, value, message):
        with self.assertRaisesRegex(checker.VendorError, message):
            checker.parse_provenance(json.dumps(value).encode())

    def test_valid_intermediate_mappings_and_counts(self):
        self.assertEqual(checker.parse_provenance(self.raw), self.value)
        self.assertEqual(len(self.value["files"]), 88)
        self.assertEqual(len(self.value["pristine_sources"]), 65)
        self.assertEqual(len(self.value["local_sources"]), 2)
        modified = set(checker.POLICY_FILES) | set(checker.BYTE_LEXER_FILES) | set(checker.BYTE_PARSER_FILES)
        self.assertEqual(len(modified), 14)
        self.assertEqual(set(checker.POLICY_FILES) & set(checker.BYTE_PARSER_FILES), set(checker.POLICY_FILES[1:]))
        for name in checker.POLICY_FILES[1:]:
            self.assertNotEqual(self.value["policy"][name]["patched_sha256"], self.value["files"][name]["sha256"])

    def test_f2_membership_cannot_omit_expand_or_cross_groups(self):
        for name in checker.BYTE_PARSER_FILES:
            for replacement in (None, checker.POLICY_FILES[0], checker.BYTE_LEXER_FILES[0],
                                checker.BYTE_SOURCE, checker.BYTE_PARSER_SOURCE, "extra.rs"):
                value = copy.deepcopy(self.value)
                record = value["byte_parser_experiment"].pop(name)
                if replacement is not None:
                    value["byte_parser_experiment"][replacement] = record
                with self.subTest(name=name, replacement=replacement):
                    self.reject(value, "unexpected byte_parser_experiment files")

    def test_overlap_inputs_must_be_post_policy_not_pristine_or_current(self):
        for name in checker.POLICY_FILES[1:]:
            for wrong in (self.value["pristine_sources"][name]["sha256"],
                          self.value["files"][name]["sha256"], "0" * 64):
                value = copy.deepcopy(self.value)
                value["byte_parser_experiment"][name]["original_sha256"] = wrong
                with self.subTest(name=name, wrong=wrong):
                    self.reject(value, "byte_parser_experiment hash mapping")

    def test_coordinated_overlap_mapping_changes_are_not_self_proving(self):
        for name in checker.POLICY_FILES[1:]:
            value = copy.deepcopy(self.value)
            value["policy"][name]["patched_sha256"] = "0" * 64
            value["byte_parser_experiment"][name]["original_sha256"] = "0" * 64
            with self.subTest(name=name):
                self.reject(value, "policy hash mapping")

    def test_nonoverlap_input_must_be_pristine(self):
        for name in set(checker.BYTE_PARSER_FILES) - set(checker.POLICY_FILES):
            value = copy.deepcopy(self.value)
            value["byte_parser_experiment"][name]["original_sha256"] = "0" * 64
            with self.subTest(name=name):
                self.reject(value, "byte_parser_experiment hash mapping")

    def test_f2_current_source_and_mapping_are_not_self_proving(self):
        for name in checker.BYTE_PARSER_FILES:
            value = copy.deepcopy(self.value)
            value["files"][name]["sha256"] = "0" * 64
            value["byte_parser_experiment"][name]["patched_sha256"] = "0" * 64
            with self.subTest(name=name):
                self.reject(value, "provenance digest")

    def test_missing_either_authored_source_is_rejected(self):
        for name in (checker.BYTE_SOURCE, checker.BYTE_PARSER_SOURCE):
            for field in ("files", "local_sources"):
                value = copy.deepcopy(self.value)
                del value[field][name]
                with self.subTest(name=name, field=field):
                    self.reject(value, "missing local source inventory|unexpected local source files")

    def test_both_authored_current_hashes_are_independent(self):
        for name in (checker.BYTE_SOURCE, checker.BYTE_PARSER_SOURCE):
            value = copy.deepcopy(self.value)
            value["files"][name]["sha256"] = "0" * 64
            value["local_sources"][name]["sha256"] = "0" * 64
            with self.subTest(name=name):
                self.reject(value, "reviewed local source hash")

    def test_previous_f1_authored_hash_is_exact_and_not_pristine(self):
        value = copy.deepcopy(self.value)
        value["local_sources"][checker.BYTE_SOURCE]["previous_sha256"] = "0" * 64
        self.reject(value, "local source metadata")
        for name in (checker.BYTE_SOURCE, checker.BYTE_PARSER_SOURCE):
            self.assertNotIn(name, self.value["pristine_sources"])
            value = copy.deepcopy(self.value)
            value["pristine_sources"][name] = value["files"][name]
            self.reject(value, "pristine source membership")

    def test_all_three_patch_anchors_are_independent(self):
        for name in ("ast-only-policy", "byte-lexer-experiment", "byte-parser-experiment"):
            value = copy.deepcopy(self.value)
            value["files"][f"patches/{name}.patch"]["sha256"] = "0" * 64
            with self.subTest(name=name):
                self.reject(value, "reviewed patch hash")

    def test_wrong_layer_order_mappings_are_rejected(self):
        value = copy.deepcopy(self.value)
        value["policy"], value["byte_parser_experiment"] = value["byte_parser_experiment"], value["policy"]
        self.reject(value, "unexpected policy files")

    def test_schema_is_exact_and_duplicate_keys_are_rejected(self):
        for version in (1, 2, 4, True, 3.0, "3", None):
            value = copy.deepcopy(self.value)
            value["schema_version"] = version
            with self.subTest(version=version):
                self.reject(value, "schema version")
        raw = self.raw.replace(b'"schema_version": 3', b'"schema_version": 3, "schema_version": 3')
        self.assertNotEqual(raw, self.raw)
        with self.assertRaisesRegex(checker.VendorError, "duplicate JSON"):
            checker.parse_provenance(raw)

    def test_authored_metadata_is_exact_for_both_files(self):
        for name in (checker.BYTE_SOURCE, checker.BYTE_PARSER_SOURCE):
            for field, replacement in (("origin", "upstream"), ("license", "Apache-2.0"),
                                       ("bytes", 0), ("sha256", "0" * 64), ("extra", True)):
                value = copy.deepcopy(self.value)
                value["local_sources"][name][field] = replacement
                with self.subTest(name=name, field=field):
                    self.reject(value, "local source metadata")
            for field in self.value["local_sources"][name]:
                value = copy.deepcopy(self.value)
                del value["local_sources"][name][field]
                with self.subTest(name=name, missing=field):
                    self.reject(value, "local source metadata")


class RuffParserLayerTests(unittest.TestCase):
    def setUp(self):
        self.before = {name: f"before {name}\n".encode() for name in checker.BYTE_PARSER_FILES}
        self.after = {name: f"after {name}\n".encode() for name in checker.BYTE_PARSER_FILES}
        self.mappings = {name: {"original_sha256": checker.digest(self.before[name]),
                                "patched_sha256": checker.digest(self.after[name])}
                         for name in checker.BYTE_PARSER_FILES}
        self.sections = ["".join(difflib.unified_diff(
            self.before[name].decode().splitlines(True), self.after[name].decode().splitlines(True),
            fromfile=f"a/{name}", tofile=f"b/{name}", n=0)) for name in checker.BYTE_PARSER_FILES]
        self.patch = "".join(self.sections)

    def test_exact_nine_file_reversal(self):
        self.assertEqual(checker.reverse_parser_patch(self.patch, self.after, self.mappings), self.before)

    def test_omitted_duplicate_and_extra_sections(self):
        extra = self.sections[0].replace(checker.BYTE_PARSER_FILES[0], checker.BYTE_SOURCE)
        changes = [self.patch + extra]
        for index in range(9):
            changes.extend(("".join(self.sections[:index] + self.sections[index + 1:]),
                            self.patch + self.sections[index]))
        for changed in changes:
            with self.subTest(changed=changed[:100]), self.assertRaises(checker.VendorError):
                checker.reverse_parser_patch(changed, self.after, self.mappings)

    def test_source_and_mapping_membership_are_exact(self):
        for name in checker.BYTE_PARSER_FILES:
            for target in ("sources", "mappings"):
                sources, mappings = dict(self.after), dict(self.mappings)
                del (sources if target == "sources" else mappings)[name]
                with self.subTest(name=name, target=target), self.assertRaisesRegex(
                        checker.VendorError, "parser source files"):
                    checker.reverse_parser_patch(self.patch, sources, mappings)
        with self.assertRaisesRegex(checker.VendorError, "parser source files"):
            checker.reverse_parser_patch(self.patch, {**self.after, checker.BYTE_PARSER_SOURCE: b""}, self.mappings)

    def test_output_hash_is_checked_before_reversal(self):
        name = checker.BYTE_PARSER_FILES[0]
        with self.assertRaisesRegex(checker.VendorError, "parser output source mismatch"):
            checker.reverse_parser_patch(self.patch, {**self.after, name: b"wrong\n"}, self.mappings)

    def test_intermediate_hashes_are_checked_after_reversal(self):
        for name in checker.BYTE_PARSER_FILES:
            mappings = copy.deepcopy(self.mappings)
            mappings[name]["original_sha256"] = "0" * 64
            with self.subTest(name=name), self.assertRaisesRegex(checker.VendorError, "parser intermediate source mismatch"):
                checker.reverse_parser_patch(self.patch, self.after, mappings)

    def test_coordinated_outside_hunk_edit_still_fails_intermediate(self):
        for name in checker.BYTE_PARSER_FILES:
            sources = {**self.after, name: self.after[name] + b"outside hunk\n"}
            mappings = copy.deepcopy(self.mappings)
            mappings[name]["patched_sha256"] = checker.digest(sources[name])
            with self.subTest(name=name), self.assertRaisesRegex(checker.VendorError, "parser intermediate source mismatch"):
                checker.reverse_parser_patch(self.patch, sources, mappings)

    def test_wrong_patch_order_is_not_reversible(self):
        name = checker.BYTE_PARSER_FILES[0]
        later = "".join(difflib.unified_diff(
            self.after[name].decode().splitlines(True), ["third state\n"],
            fromfile=f"a/{name}", tofile=f"b/{name}", n=0))
        with self.assertRaisesRegex(checker.VendorError, "hunk content mismatch"):
            checker.reverse_policy_patch(self.sections[0], {name: b"third state\n"})
        intermediate = checker.reverse_policy_patch(later, {name: b"third state\n"})
        self.assertEqual(checker.reverse_policy_patch(self.sections[0], intermediate), {name: self.before[name]})

    def test_tampered_hunks_and_non_utf8_sources(self):
        for changed in (self.patch.replace("@@ -1 +1 @@", "@@ -2 +1 @@", 1),
                        self.patch.replace("@@ -1 +1 @@", "@@ -1 +2 @@", 1),
                        self.patch.replace("@@ -1 +1 @@", "@@ -1,2 +1 @@", 1),
                        self.patch.replace("-before ", "-tampered ", 1),
                        self.patch.replace("+after ", "+tampered ", 1)):
            with self.subTest(changed=changed[:100]), self.assertRaises(checker.VendorError):
                checker.reverse_parser_patch(changed, self.after, self.mappings)
        name = checker.BYTE_PARSER_FILES[0]
        sources = {**self.after, name: b"\xff\n"}
        mappings = copy.deepcopy(self.mappings)
        mappings[name]["patched_sha256"] = checker.digest(sources[name])
        with self.assertRaises(UnicodeDecodeError):
            checker.reverse_parser_patch(self.patch, sources, mappings)

    def test_checker_pipeline_uses_intermediate_policy_and_restores_union(self):
        names = json.loads((VENDOR / "provenance.json").read_bytes())["pristine_sources"]
        pristine = {name: f"pristine {name}\n".encode() for name in names}
        current = dict(pristine)
        for name in checker.POLICY_FILES:
            current[name] = f"policy {name}\n".encode()
        policy_sources = {name: current[name] for name in checker.POLICY_FILES}
        policy_patch = "".join("".join(difflib.unified_diff(
            pristine[name].decode().splitlines(True), current[name].decode().splitlines(True),
            fromfile=f"a/{name}", tofile=f"b/{name}", n=0)) for name in checker.POLICY_FILES)
        for name in checker.BYTE_LEXER_FILES:
            current[name] = f"lexer {name}\n".encode()
        lexer_patch = "".join("".join(difflib.unified_diff(
            pristine[name].decode().splitlines(True), current[name].decode().splitlines(True),
            fromfile=f"a/{name}", tofile=f"b/{name}", n=0)) for name in checker.BYTE_LEXER_FILES)
        intermediate = {name: current[name] for name in checker.BYTE_PARSER_FILES}
        current.update(self.after)
        parser_patch = "".join("".join(difflib.unified_diff(
            intermediate[name].decode().splitlines(True), current[name].decode().splitlines(True),
            fromfile=f"a/{name}", tofile=f"b/{name}", n=0)) for name in checker.BYTE_PARSER_FILES)
        content = {
            **current, "provenance.json": b"{}", "upstream/Cargo.lock": b"lock\n",
            "patches/ast-only-policy.patch": policy_patch.encode(),
            "patches/byte-lexer-experiment.patch": lexer_patch.encode(),
            "patches/byte-parser-experiment.patch": parser_patch.encode(),
            checker.BYTE_SOURCE: b"authored lexer\n", checker.BYTE_PARSER_SOURCE: b"authored parser\n",
            **{f"crates/{name}/Cargo.toml": b"" for name in checker.LOCAL},
        }
        records = {name: {"bytes": len(data), "sha256": checker.digest(data)} for name, data in content.items()}
        provenance = {
            "files": {name: record for name, record in records.items() if name != "provenance.json"},
            "pristine_sources": {name: {"bytes": len(data), "sha256": checker.digest(data)}
                                 for name, data in pristine.items()},
            "local_sources": {checker.BYTE_SOURCE: {}, checker.BYTE_PARSER_SOURCE: {}},
            "byte_parser_experiment": {name: {"original_sha256": checker.digest(intermediate[name]),
                                               "patched_sha256": checker.digest(current[name])}
                                       for name in checker.BYTE_PARSER_FILES},
        }
        read_bytes = Path.read_bytes
        calls = []
        reverse = checker.reverse_policy_patch

        def read(path):
            return content[path.relative_to(VENDOR).as_posix()] if path.is_relative_to(VENDOR) else read_bytes(path)

        def observe(patch_text, sources):
            calls.append(set(sources))
            return reverse(patch_text, sources)

        with patch.object(checker, "parse_provenance", return_value=provenance), \
                patch.object(checker, "validate_inventory"), patch.object(checker, "validate_manifest"), \
                patch.object(checker, "LOCK_SHA256", checker.digest(b"lock\n")), \
                patch.object(checker, "validate_policy") as validate_policy, \
                patch.object(checker, "reverse_policy_patch", observe), patch.object(Path, "read_bytes", read):
            result = checker.check_vendor(VENDOR)
        validate_policy.assert_called_once_with(policy_sources, {name: pristine[name] for name in checker.POLICY_FILES})
        self.assertEqual(calls, [set(checker.BYTE_PARSER_FILES), set(checker.POLICY_FILES), set(checker.BYTE_LEXER_FILES)])
        self.assertEqual((result["sources"], result["pristine_sources"], result["local_sources"],
                          result["modified_upstream_sources"], result["unmodified_upstream_sources"]),
                         (67, 65, 2, 14, 51))


class RuffManifestTests(unittest.TestCase):
    # Independent, small manifest fixture rather than a copied source/build tree.
    MANIFEST = b'''[package]
name = "ruff_source_file"
version = "0.0.0"
publish = false
authors = ["Charlie Marsh <charlie.r.marsh@gmail.com>"]
edition = "2024"
rust-version = "1.85"
homepage = "https://docs.astral.sh/ruff"
documentation = "https://docs.astral.sh/ruff"
repository = "https://github.com/astral-sh/ruff"
license = "MIT"
license-file = "../../LICENSE"
autobins = false
autoexamples = false
autotests = false
autobenches = false
[lib]
test = false
doctest = false
bench = false
[dependencies]
ruff_text_size = { path = "../ruff_text_size" }
memchr = "2.7.1"
[lints.rust]
unexpected_cfgs = { level = "warn", check-cfg = ['cfg(fuzzing)', 'cfg(codspeed)', 'cfg(feature, values("serde"))'] }
'''

    def validate(self, data):
        root = Path("/synthetic/ruff")
        checker.validate_manifest(data, "ruff_source_file", root / "crates/ruff_source_file", root)

    def test_self_contained_manifest(self):
        self.validate(self.MANIFEST)

    def test_manifest_escaping_and_hidden_features(self):
        replacements = (
            (b'path = "../ruff_text_size"', b'path = "../../../../escape"'),
            (b'path = "../ruff_text_size"', b'git = "https://example.invalid/repo"'),
            (b'path = "../ruff_text_size"', b'workspace = true'),
            (b'memchr = "2.7.1"', b'memchr = { version = "2.7.1", optional = true }'),
            (b'test = false', b'test = true'),
            (b'rust-version = "1.85"', b'rust-version = "1.90"'),
        )
        for old, new in replacements:
            with self.subTest(new=new), self.assertRaises(checker.VendorError):
                self.validate(self.MANIFEST.replace(old, new))
        for extra in (b'\n[features]\nserde = []\n', b'\n[workspace]\n', b'\n[build-dependencies]\ncc = "1"\n'):
            with self.subTest(extra=extra), self.assertRaises(checker.VendorError):
                self.validate(self.MANIFEST + extra)


class RuffByteLexerPatchTests(unittest.TestCase):
    def setUp(self):
        self.sources = {name: (VENDOR / name).read_bytes() for name in checker.BYTE_LEXER_FILES}
        self.patch = (VENDOR / "patches/byte-lexer-experiment.patch").read_text()
        self.originals = checker.reverse_policy_patch(self.patch, self.sources)
        self.pristine = json.loads((VENDOR / "provenance.json").read_bytes())["pristine_sources"]

    def test_exact_four_file_pristine_reversal(self):
        self.assertEqual(set(self.originals), set(checker.BYTE_LEXER_FILES))
        for name, data in self.originals.items():
            with self.subTest(name=name):
                self.assertEqual({"bytes": len(data), "sha256": checker.digest(data)}, self.pristine[name])

    def test_canonical_ordered_zero_context_diff(self):
        expected = "".join("".join(difflib.unified_diff(
            self.originals[name].decode().splitlines(keepends=True),
            self.sources[name].decode().splitlines(keepends=True),
            fromfile=f"a/{name}", tofile=f"b/{name}", n=0)) for name in checker.BYTE_LEXER_FILES)
        self.assertEqual(self.patch, expected)
        self.assertEqual(checker.digest(self.patch.encode()), checker.BYTE_PATCH_SHA256)

    def test_each_file_section_is_required_and_unique(self):
        sections = ["--- a/" + section for section in self.patch.split("--- a/")[1:]]
        self.assertEqual(len(sections), 4)
        for index in range(4):
            for changed in ("".join(sections[:index] + sections[index + 1:]), self.patch + sections[index]):
                with self.subTest(index=index), self.assertRaises(checker.VendorError):
                    checker.reverse_policy_patch(changed, self.sources)

    def test_cannot_patch_other_group_or_authored_source(self):
        name = checker.BYTE_LEXER_FILES[0]
        for replacement in (*checker.POLICY_FILES, checker.BYTE_SOURCE, "../escape", "/dev/null"):
            changed = self.patch.replace(f"--- a/{name}\n+++ b/{name}\n",
                                         f"--- a/{replacement}\n+++ b/{replacement}\n", 1)
            with self.subTest(replacement=replacement), self.assertRaises(checker.VendorError):
                checker.reverse_policy_patch(changed, self.sources)
        with self.assertRaisesRegex(checker.VendorError, "source files"):
            checker.validate_policy(self.sources, self.originals)

    def test_source_membership_cannot_shrink_or_expand(self):
        for name in checker.BYTE_LEXER_FILES:
            with self.subTest(name=name), self.assertRaises(checker.VendorError):
                checker.reverse_policy_patch(self.patch, {n: s for n, s in self.sources.items() if n != name})
        with self.assertRaisesRegex(checker.VendorError, "missing policy patch file"):
            checker.reverse_policy_patch(self.patch, {**self.sources, checker.BYTE_SOURCE: b""})

    def test_real_patch_hunk_offsets_counts_and_contents(self):
        match = re.search(r"@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@\n", self.patch)
        self.assertIsNotNone(match)
        old_start, old_count, new_start, new_count = (int(v) if v is not None else 1 for v in match.groups())
        headers = (
            f"@@ -{old_start + 1},{old_count} +{new_start},{new_count} @@\n",
            f"@@ -{old_start},{old_count} +{new_start + 1},{new_count} @@\n",
            f"@@ -{old_start},{old_count + 1} +{new_start},{new_count} @@\n",
            f"@@ -{old_start},{old_count} +{new_start},{new_count + 1} @@\n",
        )
        changes = [self.patch[:match.start()] + header + self.patch[match.end():] for header in headers]
        addition = next(line for line in self.patch.splitlines(True) if line.startswith("+") and not line.startswith("+++"))
        changes.append(self.patch.replace(addition, "+// tampered\n", 1))
        changes.append(self.patch.replace("+++ b/", "+++ b/wrong/", 1))
        for changed in changes:
            with self.subTest(changed=changed[:200]), self.assertRaises(checker.VendorError):
                checker.reverse_policy_patch(changed, self.sources)

    def test_outside_hunk_edits_fail_pristine_guard(self):
        read_bytes = Path.read_bytes
        for name in checker.BYTE_LEXER_FILES:
            changed = self.sources[name] + b"// unreviewed trailing edit\n"
            restored = checker.reverse_policy_patch(self.patch, {**self.sources, name: changed})
            self.assertNotEqual(checker.digest(restored[name]), self.pristine[name]["sha256"])

            def read(path):
                return changed if path == VENDOR / name else read_bytes(path)

            # Isolate the deeper reversal guard; the real inventory also rejects this edit.
            with self.subTest(name=name), patch.object(checker, "validate_inventory"), \
                    patch.object(Path, "read_bytes", read), \
                    self.assertRaisesRegex(checker.VendorError, "pristine source mismatch"):
                checker.check_vendor(VENDOR)

    def test_synthetic_insertions_and_deletions_reverse_exactly(self):
        for old, new in (("", "new\n"), ("old\n", ""),
                         ("same\n", "same\nnew\n"), ("same\nold\n", "same\n")):
            diff = "".join(difflib.unified_diff(old.splitlines(True), new.splitlines(True),
                                                fromfile="a/demo.rs", tofile="b/demo.rs", n=0))
            with self.subTest(old=old, new=new):
                self.assertEqual(checker.reverse_policy_patch(diff, {"demo.rs": new.encode()}),
                                 {"demo.rs": old.encode()})

    def test_synthetic_malformed_hunks(self):
        valid = "--- a/demo.rs\n+++ b/demo.rs\n@@ -1 +1 @@\n-old\n+new\n"
        for changed in ("", valid.replace("@@ -1", "@@ -0"), valid.replace("+1 @@", "+0 @@"),
                        valid.replace("+new\n", "+wrong\n"), valid.replace("-old\n", "?old\n"),
                        valid.replace("@@ -1 +1 @@\n", ""), valid + "@@ -1 +1 @@\n-old\n+new\n",
                        valid.replace("+++ b/demo.rs", "+++ /dev/null")):
            with self.subTest(changed=changed), self.assertRaises(checker.VendorError):
                checker.reverse_policy_patch(changed, {"demo.rs": b"new\n"})


class RuffByteParserPatchTests(unittest.TestCase):
    def setUp(self):
        self.provenance = json.loads((VENDOR / "provenance.json").read_bytes())
        self.sources = {name: (VENDOR / name).read_bytes() for name in checker.BYTE_PARSER_FILES}
        self.patch = (VENDOR / "patches/byte-parser-experiment.patch").read_bytes().decode("utf-8")
        self.intermediate = checker.reverse_parser_patch(
            self.patch, self.sources, self.provenance["byte_parser_experiment"])

    def test_canonical_ordered_zero_context_diff(self):
        expected = "".join("".join(difflib.unified_diff(
            self.intermediate[name].decode("utf-8").splitlines(True),
            self.sources[name].decode("utf-8").splitlines(True),
            fromfile=f"a/{name}", tofile=f"b/{name}", n=0)) for name in checker.BYTE_PARSER_FILES)
        self.assertEqual(self.patch, expected)
        self.assertEqual(checker.digest(self.patch.encode("utf-8")), checker.BYTE_PARSER_PATCH_SHA256)

    def test_exact_intermediate_and_pristine_inputs(self):
        self.assertEqual(len(self.intermediate), 9)
        for name, content in self.intermediate.items():
            expected = checker.POLICY_OUTPUT_SHA256.get(name, self.provenance["pristine_sources"][name]["sha256"])
            with self.subTest(name=name):
                self.assertEqual(checker.digest(content), expected)
                if name not in checker.POLICY_FILES:
                    self.assertEqual(len(content), self.provenance["pristine_sources"][name]["bytes"])

    def test_old_policy_cannot_reverse_current_f2_sources(self):
        policy_patch = (VENDOR / "patches/ast-only-policy.patch").read_bytes().decode("utf-8")
        with self.assertRaises(checker.VendorError):
            checker.reverse_policy_patch(policy_patch, {name: (VENDOR / name).read_bytes()
                                                       for name in checker.POLICY_FILES})

    def test_checker_reverses_f2_before_policy_then_f1(self):
        reverse = checker.reverse_policy_patch
        calls = []

        def observe(patch_text, sources):
            calls.append(set(sources))
            return reverse(patch_text, sources)

        with patch.object(checker, "reverse_policy_patch", observe):
            result = checker.check_vendor(VENDOR)
        self.assertEqual(calls, [set(checker.BYTE_PARSER_FILES), set(checker.POLICY_FILES),
                                 set(checker.BYTE_LEXER_FILES)])
        self.assertEqual(result["modified_upstream_sources"], 14)


class RuffPolicyTests(unittest.TestCase):
    def setUp(self):
        provenance = json.loads((VENDOR / "provenance.json").read_bytes())
        intermediate = checker.reverse_parser_patch(
            (VENDOR / "patches/byte-parser-experiment.patch").read_bytes().decode("utf-8"),
            {name: (VENDOR / name).read_bytes() for name in checker.BYTE_PARSER_FILES},
            provenance["byte_parser_experiment"])
        self.sources = {name: intermediate[name] if name in intermediate else (VENDOR / name).read_bytes()
                        for name in checker.POLICY_FILES}
        self.originals = checker.reverse_policy_patch(
            (VENDOR / "patches/ast-only-policy.patch").read_text(), self.sources)

    def validate_change(self, index, old, new, count=1):
        sources = dict(self.sources)
        name = checker.POLICY_FILES[index]
        self.assertIn(old, sources[name])
        sources[name] = sources[name].replace(old, new, count)
        with self.assertRaisesRegex(checker.VendorError, "exact reviewed policy edits"):
            checker.validate_policy(sources, self.originals)

    def test_explicit_policy_defaults(self):
        checker.validate_policy(self.sources, self.originals)

    def test_each_constructor_must_default_false(self):
        for replacement in (b"ast_only: true,", b""):
            for count in (1, 2):
                with self.subTest(replacement=replacement, count=count):
                    self.validate_change(0, b"ast_only: false,", replacement, count)

    def test_policy_guard_must_precede_diagnostic(self):
        self.validate_change(1, b"if !self.options.ast_only && !all_arg_names.insert(arg_name) {",
                             b"if !all_arg_names.insert(arg_name) {")

    def test_both_new_default_checks_are_exact(self):
        check = b"""            if default.is_some() {
                self.add_unsupported_syntax_error(
                    UnsupportedSyntaxErrorKind::TypeParamDefault,
                    self.node_range(equal_token_start),
                );
            }

"""
        for fixture in (b"type_param_type_var_tuple_bound", b"type_param_param_spec_bound"):
            locus = check + b"            // test_err " + fixture
            for changed in (b"", check.replace(b"default.is_some()", b"self.options.ast_only"),
                            check.replace(b"TypeParamDefault", b"TypeParams"),
                            check.replace(b"equal_token_start", b"start")):
                with self.subTest(fixture=fixture, changed=changed):
                    self.validate_change(2, locus, changed + b"            // test_err " + fixture)

    def test_pattern_entries_are_ast_only_and_fstring_only(self):
        gate = b"(self.options.ast_only && self.at(TokenKind::FStringStart))"
        for token_set in (b"PATTERN_START_SET", b"MAPPING_PATTERN_START_SET"):
            locus = (b"        self.at_ts(" + token_set + b")\n"
                     b"            || self.at_soft_keyword()\n"
                     b"            || " + gate + b"\n")
            self.assertEqual(self.sources[checker.POLICY_FILES[3]].count(locus), 1)
            for changed in (b"self.at(TokenKind::FStringStart)",
                            gate.replace(b"ast_only", b"fake_version_flag"),
                            gate.replace(b"FStringStart", b"TStringStart"), b"false"):
                with self.subTest(token_set=token_set, changed=changed):
                    self.validate_change(3, locus, locus.replace(gate, changed))
        self.validate_change(3, b"let str = self.parse_strings();", b"let str = self.parse_expression();")

    def test_reviewed_patch_restores_pristine_hashes(self):
        pristine = json.loads((VENDOR / "provenance.json").read_bytes())["pristine_sources"]
        self.assertEqual(len(self.originals), 4)
        for name, source in self.originals.items():
            with self.subTest(name=name):
                self.assertEqual(len(source), pristine[name]["bytes"])
                self.assertEqual(checker.digest(source), pristine[name]["sha256"])

    def test_sequence_separator_check_is_unconditional_and_exact(self):
        locus = (b"            self.at(parentheses.closing_kind())\n"
                 b"                || (self.at(TokenKind::Comma) && self.peek() == parentheses.closing_kind())\n")
        for changed in (
            locus.replace(b"self.at(TokenKind::Comma) && ", b""),
            locus.replace(b"self.at(TokenKind::Comma)", b"self.options.ast_only"),
            locus.replace(b"self.at(TokenKind::Comma)", b"!self.options.ast_only || self.at(TokenKind::Comma)"),
            locus.replace(b"TokenKind::Comma", b"TokenKind::Name"),
            locus.replace(b"self.at(parentheses.closing_kind())", b"false"),
        ):
            with self.subTest(changed=changed):
                self.validate_change(3, locus, changed)

    def test_reviewed_patch_is_exact_zero_context_diff(self):
        expected = "".join(
            "".join(difflib.unified_diff(
                self.originals[name].decode().splitlines(keepends=True),
                self.sources[name].decode().splitlines(keepends=True),
                fromfile=f"a/{name}", tofile=f"b/{name}", n=0))
            for name in checker.POLICY_FILES
        )
        self.assertEqual((VENDOR / "patches/ast-only-policy.patch").read_text(), expected)

    def test_pattern_patch_gate_and_offset_tampering(self):
        patch = (VENDOR / "patches/ast-only-policy.patch").read_text()
        for old, new in (("@@ -57 +57,3 @@", "@@ -57 +58,3 @@"),
                         ("@@ -62 +64,3 @@", "@@ -62 +62,3 @@"),
                         ("@@ -355 +359,2 @@", "@@ -355 +355,2 @@"),
                         ("self.at(TokenKind::Comma) && ", ""),
                         ("self.options.ast_only && ", "")):
            self.assertIn(old, patch)
            with self.subTest(old=old), self.assertRaises(checker.VendorError):
                checker.reverse_policy_patch(patch.replace(old, new, 1), self.sources)

    def test_extra_source_edit_is_not_policy(self):
        for index in range(4):
            with self.subTest(index=index):
                self.validate_change(index, b"\n", b"\n// unreviewed\n")

    def test_policy_membership_is_exact(self):
        for name in checker.POLICY_FILES:
            sources = dict(self.sources)
            del sources[name]
            with self.subTest(name=name), self.assertRaisesRegex(checker.VendorError, "source files"):
                checker.validate_policy(sources, self.originals)

    def test_reviewed_patch_requires_all_four_paths(self):
        patch = (VENDOR / "patches/ast-only-policy.patch").read_text()
        for name in checker.POLICY_FILES:
            with self.subTest(name=name), self.assertRaises(checker.VendorError):
                checker.reverse_policy_patch(patch, {n: s for n, s in self.sources.items() if n != name})
        with self.assertRaisesRegex(checker.VendorError, "missing policy patch file"):
            checker.reverse_policy_patch(patch, {**self.sources, "extra.rs": b""})

    def test_exact_patch_roundtrip(self):
        patch = "--- a/demo.rs\n+++ b/demo.rs\n@@ -1,2 +1,2 @@\n same\n-old\n+new\n"
        self.assertEqual(checker.reverse_policy_patch(patch, {"demo.rs": b"same\nnew\n"}), {"demo.rs": b"same\nold\n"})
        for changed in (patch.replace("+new", "+wrong"), patch.replace("+1,2", "+1,3"), patch.replace("a/demo.rs", "a/../escape")):
            with self.subTest(changed=changed), self.assertRaises(checker.VendorError):
                checker.reverse_policy_patch(changed, {"demo.rs": b"same\nnew\n"})


if __name__ == "__main__":
    unittest.main()
