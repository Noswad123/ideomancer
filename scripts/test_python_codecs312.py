#!/usr/bin/env python3
"""Bounded in-memory codec data tamper tests and opt-in isolated stdlib oracle."""

import ast
import importlib.util
import json
from pathlib import Path
import sys
import unittest
from unittest import mock

SCRIPTS = Path(__file__).resolve().parent


def load(name):
    # Explicit repository-owned script path, never a sys.path search/import hook.
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / (name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


check = load("check_python_codecs312")
generate = load("generate_python_codecs312")


class CodecProfileTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.blobs = check.read_blobs()

    def test_reviewed_profile(self):
        result = check.validate(self.blobs)
        self.assertEqual(result["tables"], 70)
        self.assertEqual(result["ascii_identity"], 62)
        self.assertEqual(result["stdlib_aliases"], 326)
        self.assertEqual(result["profile_names"], 445)

    def test_independent_anchors_not_generator_constants(self):
        with mock.patch.object(generate, "SOURCE_ANCHOR", "0" * 64):
            self.assertEqual(check.validate(self.blobs)["tables"], 70)

    def test_table_tampering_cannot_be_blessed_by_manifest(self):
        blobs = dict(self.blobs)
        blobs["tables.tsv"] = blobs["tables.tsv"].replace(b"000041", b"000042", 1)
        manifest = json.loads(blobs["manifest.json"])
        manifest["outputs"]["tables.tsv"]["sha256"] = check.digest(blobs["tables.tsv"])
        blobs["manifest.json"] = json.dumps(manifest).encode()
        with self.assertRaisesRegex(ValueError, "independent review anchor"):
            check.validate(blobs)

    def test_each_frozen_input_anchor_rejects_tampering(self):
        for name in check.ANCHORS:
            with self.subTest(name=name):
                blobs = dict(self.blobs)
                blobs[name] += b" "
                with self.assertRaisesRegex(ValueError, "anchor"):
                    check.validate(blobs)

    def test_alias_admission_tamper(self):
        blobs = dict(self.blobs)
        blobs["aliases.tsv"] = blobs["aliases.tsv"].replace(b"\tN\t999", b"\tU\t999", 1)
        with self.assertRaisesRegex(ValueError, "aliases.tsv"):
            check.validate(blobs)

    def test_source_hash_tamper(self):
        blobs = dict(self.blobs)
        manifest = json.loads(blobs["manifest.json"])
        manifest["source_sha256"]["codecs.py"] = "0" * 64
        blobs["manifest.json"] = json.dumps(manifest).encode()
        with self.assertRaisesRegex(ValueError, "manifest.json"):
            check.validate(blobs)

    def test_text_caps(self):
        for raw in (b"\0", b"x" * (check.LIMIT + 1)):
            blobs = dict(self.blobs, extra=raw)
            with self.assertRaisesRegex(ValueError, "bounded text"):
                check.validate(blobs)

    def test_aggregate_cap(self):
        with mock.patch.object(check, "TOTAL_LIMIT", 1):
            with self.assertRaisesRegex(ValueError, "aggregate cap"):
                check.validate(self.blobs)

    def test_invalid_utf8(self):
        with self.assertRaises(UnicodeDecodeError):
            check.validate(dict(self.blobs, extra=b"\xff"))

    def test_unpinned_license_is_not_gate_success(self):
        with mock.patch.object(check, "LICENSE_SHA256", None):
            with self.assertRaisesRegex(ValueError, "LICENSE hash verification pending"):
                check.validate(self.blobs)
        with self.assertRaises(TypeError):
            check.validate(self.blobs, True)

    def test_independent_license_anchor_is_enforced(self):
        with mock.patch.object(check, "LICENSE_SHA256", "0" * 64):
            with self.assertRaisesRegex(ValueError, "PSF license anchor"):
                check.validate(self.blobs)

    def test_license_bytes_are_required_and_immutable(self):
        for replacement in (None, self.blobs["LICENSE.txt"] + b"\n"):
            blobs = dict(self.blobs)
            if replacement is None:
                del blobs["LICENSE.txt"]
            else:
                blobs["LICENSE.txt"] = replacement
            with self.assertRaisesRegex(ValueError, "PSF license anchor"):
                check.validate(blobs)

    def test_generator_license_pin_is_independent(self):
        with mock.patch.object(generate, "LICENSE_SHA256", "0" * 64):
            with self.assertRaisesRegex(ValueError, "CPython LICENSE hash mismatch"):
                generate.verified_license()

    def test_notice_retention_including_legal_mapping_comments(self):
        raw = b'''"""Mapping header and author credit."""
# Copyright (c) Example
decoding_table = (
    'x' # 0x00 -> COPYRIGHT SIGN
    'y' # 0x01 -> LETTER Y
    'z' # Permission: retain this notice
)
class Example:
    """Additional license grant."""
ordinary_payload = 'not-a-source-notice-123'
decoding_map = {0: 'not-a-mapping-notice-456'}
'''
        text = generate.notices({"encodings/example.py": raw}, {"example"}).decode()
        for retained in ("Mapping header and author credit.", "# Copyright (c) Example",
                         "# Permission: retain this notice", "Additional license grant."):
            self.assertIn(retained, text)
        self.assertNotIn("# 0x00 -> COPYRIGHT SIGN", text)
        self.assertNotIn("# 0x01 -> LETTER Y", text)
        self.assertNotIn("decoding_table =", text)
        self.assertNotIn("not-a-source-notice-123", text)
        self.assertNotIn("not-a-mapping-notice-456", text)

    def test_generator_cannot_write_license_or_unreviewed_paths(self):
        for name in ("LICENSE.txt", "../elsewhere"):
            with self.assertRaisesRegex(ValueError, "unreviewed output path"):
                generate.write_outputs({name: b"not an authorized output"})

    def test_all_table_and_alias_notices_are_present(self):
        text = self.blobs["NOTICES.md"].decode("utf-8")
        self.assertEqual(text.count("\n## encodings/"), 71)
        for notice in ("(c) Copyright CNRI, All Rights Reserved. NO WARRANTY.",
                       "(c) Copyright 2000 Guido van Rossum.",
                       "Written by Marc-Andre Lemburg", "Written by Sjoerd Mullender",
                       "(Used with permission)", "Keld Simonsen", "33471-90901"):
            self.assertIn(notice, text)

    def test_structural_audit_rejects_stateful_or_wrong_wiring(self):
        model = """
import codecs
class Codec(codecs.Codec):
    def decode(self,input,errors='strict'):
        return codecs.charmap_decode(input,errors,decoding_table)
class IncrementalDecoder(codecs.IncrementalDecoder):
    def decode(self,input,final=False):
        return codecs.charmap_decode(input,self.errors,decoding_table)[0]
def getregentry():
    return codecs.CodecInfo(decode=Codec().decode, incrementaldecoder=IncrementalDecoder)
"""
        generate.audit_table(ast.parse(model))
        for before, after in (("return codecs.charmap_decode(input,errors,decoding_table)",
                               "self.state = 1\n        return codecs.charmap_decode(input,errors,decoding_table)"),
                              ("decode=Codec().decode", "decode=other"),
                              ("input,self.errors,decoding_table", "input,self.errors,other_table")):
            with self.subTest(after=after), self.assertRaises(ValueError):
                generate.audit_table(ast.parse(model.replace(before, after)))

    def test_oracle_rejects_wrong_defined_and_undefined_points(self):
        class Wrong:
            @staticmethod
            def getregentry():
                class Entry:
                    @staticmethod
                    def decode(raw, errors):
                        return ("x", 1)
                return Entry()
        for table in ("\ufffe" * 256, "a" * 256):
            with self.assertRaisesRegex(ValueError, "strict byte oracle mismatch"):
                generate.byte_oracle(Wrong, table)

    def test_oracle_rejects_unexpected_decode_failure(self):
        class Wrong:
            @staticmethod
            def getregentry():
                class Entry:
                    @staticmethod
                    def decode(raw, errors):
                        raise UnicodeDecodeError("test", raw, 0, 1, "undefined")
                return Entry()
        with self.assertRaisesRegex(ValueError, "unexpected undefined byte"):
            generate.byte_oracle(Wrong, "a" * 256)

    @unittest.skipUnless(sys.flags.isolated, "trusted source oracle requires explicit -I")
    def test_exact_source_closure_structure_aliases_and_every_byte(self):
        outputs = generate.build()
        for name, raw in outputs.items():
            self.assertEqual(raw, self.blobs[name], name)

    @unittest.skipUnless(sys.flags.isolated, "trusted source oracle requires explicit -I")
    def test_generator_source_anchor_cannot_be_manifest_blessed(self):
        with mock.patch.object(generate, "SOURCE_ANCHOR", "0" * 64):
            with self.assertRaisesRegex(ValueError, "source closure mismatch"):
                generate.read_sources()

    @unittest.skipUnless(sys.flags.isolated, "trusted source oracle requires explicit -I")
    def test_exact_alias_entries_and_supported_registry_spellings(self):
        base, _, _ = generate.read_sources()
        import codecs
        import encodings
        rows = [line.split("\t") for line in self.blobs["aliases.tsv"].decode("ascii").splitlines()]
        frozen_aliases = {key.rstrip(): target.rstrip() for key, target, _, _, origin in rows if origin == "1"}
        self.assertEqual(frozen_aliases, encodings.aliases.aliases)
        for key, target, status, _, origin in rows:
            if status not in "ULAS":
                continue  # No registry query for platform/unsupported profiles.
            key, target = key.rstrip(), target.rstrip()
            expected = generate.trusted_module(base, target).getregentry().name
            spellings = [key, key.upper().replace("_", "-")]
            if origin == "1" and "." not in key:
                spellings.append(key.upper().replace("_", "."))
            for spelling in spellings:
                if key != key.lower():
                    # This exact stdlib dictionary typo is unreachable after
                    # registry lowercasing; do not manufacture a lowercase alias.
                    self.assertEqual(key, "csHPRoman8")
                    with self.assertRaises(LookupError):
                        codecs.lookup(spelling)
                else:
                    self.assertEqual(codecs.lookup(spelling).name, expected, spelling)

    @unittest.skipUnless(sys.flags.isolated, "trusted source oracle requires explicit -I")
    def test_profile_rejects_wrong_unicode_version(self):
        with mock.patch.object(generate.unicodedata, "unidata_version", "16.0.0"):
            with self.assertRaisesRegex(ValueError, "requires trusted isolated"):
                generate.profile()

    @unittest.skipUnless(sys.flags.isolated, "trusted source oracle requires explicit -I")
    def test_utf8_sig_captured_source_certificate_twenty_cases(self):
        generate.read_sources()
        import codecs
        names = ("utf-8-sig", "utf_8_sig", "UTF--8_sig", "utf---8---sig", "UTF__8_sig")
        cases = accepted = rejected = 0
        for name in names:
            for prefix in (b"", b"# header\n"):
                for bom in (b"", codecs.BOM_UTF8):
                    cases += 1
                    remaining = prefix + b"# coding: " + name.encode("ascii") + b"\nvalue='\xc3\xa9\xef\xbb\xbf'\n"
                    with self.subTest(name=name, second_line=bool(prefix), bom=bool(bom)):
                        if bom and name not in names[:2]:
                            with self.assertRaises(SyntaxError) as caught:
                                ast.parse(bom + remaining)
                            self.assertEqual(caught.exception.lineno, 0)
                            self.assertIn("encoding problem", caught.exception.msg)
                            rejected += 1
                        else:
                            parsed = ast.parse(bom + remaining)
                            self.assertEqual(parsed.body[0].value.value, "\u00e9\ufeff")
                            self.assertEqual(remaining.decode("utf_8_sig"), remaining.decode("utf_8"))
                            accepted += 1
        self.assertEqual((cases, accepted, rejected), (20, 14, 6))
        self.assertEqual((codecs.BOM_UTF8 + b"x").decode("utf_8_sig"), "x")
        self.assertEqual((codecs.BOM_UTF8 + b"x").decode("utf_8"), "\ufeffx")


if __name__ == "__main__":
    unittest.main(verbosity=2)
