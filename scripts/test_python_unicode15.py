"""Small stdlib tests. TemporaryDirectory follows the caller's disk TMPDIR."""

import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import check_python_unicode15 as check
import generate_python_unicode15 as generate


class Unicode15Tests(unittest.TestCase):
    def test_official_hashes_and_exact_generated_membership(self):
        result = check.verify()
        self.assertEqual(result["unicode_version"], "15.0.0")
        manifest = json.loads((generate.DATA / "manifest.json").read_text())
        self.assertEqual(manifest["start_count"], 136323)
        self.assertEqual(manifest["continue_count"], 139463)
        self.assertEqual(manifest["aliases_count"], 473)
        self.assertGreater(result["name_probes"], 180000)

    def test_hash_anchor_is_not_supplied_by_manifest(self):
        with tempfile.TemporaryDirectory(prefix="unicode15-negative-") as run:
            directory = Path(run)
            for name in generate.INPUTS:
                (directory / name).write_bytes((generate.DATA / "inputs" / name).read_bytes())
            (directory / "NameAliases.txt").write_text("0000;WRONG;control\n")
            with self.assertRaisesRegex(ValueError, "official input hash mismatch"):
                generate.read_inputs(directory)

    def test_text_caps_and_negative_names(self):
        for raw in (b"\0", b"x" * (generate.LIMIT + 1), b"\xff"):
            with self.assertRaises((ValueError, UnicodeError)):
                generate.checked_text(raw)
        for name in ("KEYCAP DIGIT ONE", "TODHRI LETTER A", "HANGUL SYLLABLE ga",
                     "CJK UNIFIED IDEOGRAPH-4e00", "TANGUT IDEOGRAPH-17000",
                     "CUNEIFORM SIGN KALAM"):
            self.assertIsNone(check.lookup(name), name)
        self.assertEqual(check.lookup("null"), 0)
        self.assertEqual(check.lookup("CJK UNIFIED IDEOGRAPH-04E00"), 0x4E00)

    def test_interval_encoding_is_sorted_fixed_width(self):
        self.assertEqual(generate.intervals({8, 3, 4, 6}), [[3, 4], [6, 6], [8, 8]])
        raw = generate.range_bytes([[3, 4], [8, 8]])
        self.assertEqual(raw, b"000003\t000004\n000008\t000008\n")
        self.assertEqual(len(raw), 28)

    def test_wrong_unicode_profile_fails(self):
        with patch.object(generate.unicodedata, "unidata_version", "16.0.0"):
            with self.assertRaisesRegex(ValueError, "Unicode 15.0.0"):
                generate.profile()


if __name__ == "__main__":
    unittest.main()
