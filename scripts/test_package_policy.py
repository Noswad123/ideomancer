"""Synthetic policy tests; no private files or target repositories are read."""

import unittest
from pathlib import Path
import tomllib

from check_package import PolicyError, checked_path, inventory, verify


class PackagePolicyTests(unittest.TestCase):
    def test_manifest_includes_are_root_anchored(self):
        root = Path(__file__).resolve().parent.parent
        manifest = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
        patterns = manifest["package"]["include"]
        self.assertTrue(patterns)
        # Cargo include patterns otherwise match same-named nested files even
        # inside ignored study directories (for example an unanchored README.md).
        self.assertTrue(all(pattern.startswith("/") for pattern in patterns))

    def test_exact_inventory_and_optional_cargo_metadata(self):
        expected = inventory("# reviewed\nCargo.toml\nsrc/lib.rs\n\n")
        self.assertEqual(verify(expected, ["src/lib.rs", "Cargo.toml"]), 2)
        self.assertEqual(
            verify(expected, ["Cargo.toml", "src/lib.rs", ".cargo_vcs_info.json"]), 3
        )

    def test_unknown_and_missing_files_fail_independently(self):
        with self.assertRaisesRegex(PolicyError, "unreviewed"):
            verify({"Cargo.toml"}, ["Cargo.toml", "docs/new-study.md"])
        with self.assertRaisesRegex(PolicyError, "missing"):
            verify({"Cargo.toml", "LICENSE"}, ["Cargo.toml"])

    def test_duplicate_empty_and_generated_inventories_fail(self):
        for text in ["", "# nothing\n", "LICENSE\nLICENSE\n", ".cargo_vcs_info.json\n"]:
            with self.subTest(text=text), self.assertRaises(PolicyError):
                inventory(text)
        with self.assertRaisesRegex(PolicyError, "duplicate Cargo"):
            verify({"LICENSE"}, ["LICENSE", "LICENSE"])

    def test_private_paths_cannot_be_approved_by_editing_inventory(self):
        for name in [
            ".ideomancer/study.json", "docs/.private/claims.json", ".git/config",
            "target/cache.json", "examples/node_modules/code.js", "openapi.json",
            ".env", "examples/.env.local",
        ]:
            with self.subTest(name=name), self.assertRaises(PolicyError):
                inventory(name)
            with self.subTest(name=name), self.assertRaises(PolicyError):
                verify({name}, [name])

    def test_paths_must_be_canonical_printable_relative_ascii(self):
        for name in [
            "", ".", "/LICENSE", "../LICENSE", "docs/../LICENSE", "./LICENSE",
            "docs//file.md", "docs/", "docs\\file.md", "docs/\x00file.md",
            "docs/\x7ffile.md", "docs/\tfile.md", "docs/é.md", "x" * 513,
        ]:
            with self.subTest(name=name), self.assertRaises(PolicyError):
                checked_path(name)


if __name__ == "__main__":
    unittest.main()
