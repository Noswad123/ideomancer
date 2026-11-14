"""Package policy and retained notice tests; no network or Cargo cache required."""

import hashlib
import re
import unittest
from pathlib import Path
import tomllib

from check_package import PolicyError, checked_path, inventory, verify


class PackagePolicyTests(unittest.TestCase):
    def test_selected_dependency_notices_are_in_reviewed_package_scope(self):
        root = Path(__file__).resolve().parent.parent
        name = "docs/third-party-notices.md"
        expected = inventory((root / "docs/package-files.txt").read_text(encoding="utf-8"))
        self.assertIn(name, expected)
        manifest = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
        self.assertIn("/docs/*.md", manifest["package"]["include"])
        notices = (root / name).read_text(encoding="utf-8")
        locked = tomllib.loads((root / "Cargo.lock").read_text(encoding="utf-8"))
        versions = {package["name"]: package["version"] for package in locked["package"]}
        for component, version, revision in [
            ("tree-sitter", "0.25.10", "da6fe9beb4f7f67beb75914ca8e0d48ae48d6406"),
            ("tree-sitter-language", "0.1.5", "8b5c63bffae4c1948106eb0a8d722bf2a9141ce5"),
            ("tree-sitter-javascript", "0.25.0", "44c892e0be055ac465d5eeddae6d3e194424e7de"),
            ("tree-sitter-typescript", "0.23.2", "f975a621f4e7f532fe322e13c4f79495e0a7b2e7"),
            ("streaming-iterator", "0.1.9", "69069dc9e46facef6caa79ee48e47420de92633d"),
            ("regex", "1.13.1", "2b527599eb9eea0dcc288c704584f242f26a5c61"),
            ("regex-automata", "0.4.18", "e6ad938198e68339a593f85d228f2d44cb54718f"),
            ("regex-syntax", "0.8.11", "140167995737fa11dfe11b8af8b9aa143b790b4e"),
            ("unsafe-libyaml", "0.2.11", "a7b8d1fbd93aefbca3003dcb5fcc6a9c2297e968"),
        ]:
            with self.subTest(component=component):
                self.assertEqual(versions[component], version, "re-review notices on upgrade")
                row = next(line for line in notices.splitlines()
                           if line.startswith(f"| {component} {version} "))
                self.assertIn(revision, row)
        self.assertIn("TypeScript and TSX", notices)
        self.assertIn(
            "https://raw.githubusercontent.com/yaml/libyaml/"
            "2c891fc7a770e8ba2fec34fc6b545c672beb37e6/License", notices
        )
        self.assertIn("not a complete binary dependency/license review", notices)
        self.assertIn("Executable bundles still require a whole dependency audit", notices)
        self.assertIn("does not assert that all dependencies are MIT licensed", notices)

    def test_full_selected_dependency_license_texts_are_retained_verbatim(self):
        root = Path(__file__).resolve().parent.parent
        notices = (root / "docs/third-party-notices.md").read_bytes()
        blocks = re.findall(rb"^```text\n(.*?)^```$", notices, re.MULTILINE | re.DOTALL)
        # SHA-256 of the exact pinned source files documented in the notices.
        # Keep full texts, including the entire 414-line runtime Unicode file;
        # no network/cache lookup or abbreviated clause matching in this test.
        expected = [
            ("runtime/bridge MIT", "5f9cf9fb6acb1972b35ae29119ce563bb60ec097656bc4b69b9bac2d04c7a147"),
            ("JavaScript MIT", "2e0110e07abef7c2548b26ec9d6969775617ca539a0dc8dbeeb14d6452c711d1"),
            ("TypeScript/TSX MIT", "49bf33cf78ef5897e4e161ce1517df7de1ae5042a65b6bcfd44401e0fc606559"),
            ("runtime Unicode", "6a18c5fac70d7860b57f5b72b4e2c9a1ba6b3d2741eef7ff9767c5379364f10d"),
            ("endian header notice", None),
            ("streaming-iterator MIT", "34cb64162194d52a8412e96d8693ebf43a67d019c6b5bd76da3bb377ba7507aa"),
            ("regex family MIT", "6485b8ed310d3f0340bf1ad1f47645069ce4069dcc6bb46c7d5c6faf41de1fdb"),
            ("regex-syntax Unicode", "74db5baf44a41b1000312c673544b3374e4198af5605c7f9080a402cec42cfa3"),
            ("unsafe-libyaml translation MIT", "23f18e03dc49df91622fe2a76176497404e46ced8a715d9d2b67a7446571cca3"),
            ("translated libyaml upstream MIT", "c40112449f254b9753045925248313e9270efa36d226b22d82d4cc6c43c57f29"),
        ]
        self.assertEqual(len(blocks), len(expected))
        for block, (label, digest) in zip(blocks, expected):
            if digest is not None:
                with self.subTest(notice=label):
                    self.assertEqual(hashlib.sha256(block).hexdigest(), digest)
        # The header is code, not a standalone license file: retain its complete
        # opening notice, including the public-domain jurisdictional fallback.
        self.assertEqual(blocks[4].decode("utf-8"), (
            '// "License": Public Domain\n'
            '// I, Mathias Panzenb\u00f6ck, place this file hereby into the public domain. Use it at your own risk for whatever you like.\n'
            "// In case there are jurisdictions that don't support putting things in the public domain you can also consider it to\n"
            '// be "dual licensed" under the BSD, MIT and Apache licenses, if you want to. This code is trivial anyway. Consider it\n'
            '// an example on how to get the endian conversion functions on different platforms.\n'
            '\n'
            '// updates from https://github.com/mikepb/endian.h/issues/4\n'
        ))

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
