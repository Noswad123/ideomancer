"""Small synthetic source-boundary fixtures; no Cargo, Git, network or builds."""

from contextlib import redirect_stdout
import hashlib
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import check_checkout as checkout
from check_checkout_consumer import temporary_parent


def fixture(root):
    contents = {name: b"synthetic text\n" for name in checkout.FIXED}
    contents.update({"src/lib.rs": b"// synthetic library\n",
                     "examples/tiny.py": b"# static text only\n",
                     "tests/tiny.rs": b"// test source\n",
                     "docs/source-files.txt": b"# candidate inventory is not trusted\n",
                     "scripts/tiny.py": b"# source\n",
                     "skills/tiny/SKILL.md": b"# Skill\n",
                     "vendor/ruff/README.md": b"# Synthetic vendor\n"})
    for name, data in contents.items():
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    return set(contents)


class CheckoutTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="checkout-unit-", dir=temporary_parent(os.environ))
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.expected = fixture(self.root)

    def snapshot(self):
        return checkout.snapshot(self.root, self.expected)

    def test_fingerprint_covers_paths_sizes_and_exact_bytes(self):
        before = self.snapshot()
        canonical = json.dumps(before["records"], sort_keys=True, separators=(",", ":")).encode("ascii")
        self.assertEqual(before["source_sha256"], hashlib.sha256(canonical).hexdigest())
        self.assertEqual(before["files"], len(self.expected))
        path = self.root / "src/lib.rs"
        path.write_bytes(path.read_bytes().replace(b"synthetic", b"different"))
        self.assertNotEqual(before["source_sha256"], self.snapshot()["source_sha256"])

    def test_selected_extras_and_missing_reviewed_files_fail(self):
        for name in ("src/extra.rs", "vendor/ruff/extra.txt", "scripts/extra.py", "docs/extra.md",
                     "tests/extra.cjs", "examples/extra.txt", "skills/other/SKILL.md"):
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("extra", encoding="utf-8")
            with self.subTest(name=name), self.assertRaises(ValueError):
                self.snapshot()
            path.unlink()
        (self.root / "src/lib.rs").unlink()
        with self.assertRaises((OSError, ValueError)):
            self.snapshot()

    def test_unselected_private_and_build_trees_are_never_walked(self):
        for name in (".git", ".ideomancer", ".agents", "privatefixtures", "target"):
            (self.root / name).symlink_to(self.root / "absent", target_is_directory=True)
        (self.root / "docs/private.txt").write_text("ignored", encoding="utf-8")
        (self.root / "scripts/__pycache__").mkdir()
        self.assertEqual(self.snapshot()["files"], len(self.expected))

    def test_strict_paths_and_positive_boundary(self):
        for name in ("../src/a.rs", "src//a.rs", "src/./a.rs", "/src/a.rs", "src/a\\b.rs",
                     "src/\u00e9.rs", "src/.git/config", "src/target/file", "src/privatefixtures/a.rs",
                     "Cargo.toml.orig", ".agents/a.md", "docs/private.txt", "scripts/sub/a.py"):
            with self.subTest(name=name), self.assertRaises(ValueError):
                checkout.source_path(name)

    def test_symlink_files_directories_and_source_root_fail(self):
        alias = self.root / "alias"
        alias.symlink_to(self.root, target_is_directory=True)
        with self.assertRaises((OSError, ValueError)):
            checkout.snapshot(alias, self.expected)
        path = self.root / "src/lib.rs"
        path.unlink()
        path.symlink_to(self.root / "README.md")
        with self.assertRaises((OSError, ValueError)):
            self.snapshot()
        path.unlink()
        path.parent.rmdir()
        path.parent.symlink_to(self.root / "examples", target_is_directory=True)
        with self.assertRaises((OSError, ValueError)):
            self.snapshot()

    @unittest.skipUnless(hasattr(os, "mkfifo"), "POSIX FIFO required")
    def test_special_file_does_not_block(self):
        path = self.root / "src/lib.rs"
        path.unlink()
        os.mkfifo(path)
        with self.assertRaises((OSError, ValueError)):
            self.snapshot()

    def test_size_count_total_and_scan_bounds(self):
        for bound in ("MAX_FILE", "MAX_TOTAL", "MAX_FILES", "MAX_ENTRIES"):
            with self.subTest(bound=bound), patch.object(checkout, bound, 1), self.assertRaises(ValueError):
                self.snapshot()

    def test_binary_and_invalid_utf8_rejected(self):
        for data in (b"\0", b"\xff"):
            (self.root / "src/lib.rs").write_bytes(data)
            with self.assertRaises(ValueError):
                self.snapshot()

    def test_candidate_inventory_cannot_approve_itself_and_errors_are_sanitized(self):
        output = io.StringIO()
        with patch.object(checkout, "reviewed_paths", return_value=self.expected), redirect_stdout(output):
            code = checkout.main(["--source-dir", str(self.root / "PRIVATE-missing")])
        self.assertEqual(code, 1)
        self.assertEqual(json.loads(output.getvalue())["status"], "failed")
        self.assertNotIn("PRIVATE", output.getvalue())
        self.assertNotIn(str(self.root), output.getvalue())

    def test_replacement_while_file_is_open_is_detected(self):
        with self.assertRaises(ValueError):
            with checkout.checked_file(self.root, "src/lib.rs") as stream:
                stream.read()
                replacement = self.root / "src/new.rs"
                replacement.write_bytes(b"// replacement\n")
                replacement.replace(self.root / "src/lib.rs")


if __name__ == "__main__":
    unittest.main()
