"""Consumer harness tests, using stdlib only and never invoking Cargo."""

from contextlib import contextmanager, redirect_stderr, redirect_stdout
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import tomllib
import unittest
from unittest.mock import patch

import check_package_consumer as consumer
from check_package import inventory as parse_inventory


class PackageConsumerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="consumer-unit-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.package = self.root / "ideomancer-0.1.0"
        (self.package / "src").mkdir(parents=True)
        self.original = ('[package]\nname = "ideomancer"\nversion = "0.1.0"\nrust-version = "1.89"\n'
                         'edition = "2021"\nlicense = "MIT"\n[lib]\nname = "ideomancer_core"\n')
        normalized_original = self.original.replace('[lib]',
            'build = false\nautolib = false\nautobins = false\nautoexamples = false\n'
            'autotests = false\nautobenches = false\n[lib]')
        self.normalized = consumer.NORMALIZED_HEADER + '\n' + normalized_original + (
            'path = "src/lib.rs"\n[[bin]]\nname = "ideomancer"\npath = "src/main.rs"\n'
        )
        (self.package / "Cargo.toml").write_text(self.normalized, encoding="utf-8")
        (self.package / "Cargo.toml.orig").write_text(self.original, encoding="utf-8")
        (self.package / "src/lib.rs").write_text("", encoding="utf-8")
        (self.package / "src/main.rs").write_text("fn main() {}\n", encoding="utf-8")
        self.expected = {"Cargo.toml", "Cargo.toml.orig", "src/lib.rs", "src/main.rs"}
        self.archive = self.root / "ideomancer-0.1.0.crate"
        self.write_archive()
        # Only substitute the trusted reviewer's inventory, not archive verification.
        inventory_patch = patch.object(consumer, "inventory", return_value=self.expected)
        self.inventory = inventory_patch.start()
        self.addCleanup(inventory_patch.stop)

    def write_archive(self):
        with tarfile.open(self.archive, "w:gz", format=tarfile.USTAR_FORMAT) as output:
            for name in sorted(self.expected):
                data = (self.package / name).read_bytes()
                member = tarfile.TarInfo("ideomancer-0.1.0/" + name)
                member.size = len(data)
                output.addfile(member, io.BytesIO(data))

    def test_extracted_manifest_and_library_required(self):
        path, manifest = consumer.validate_package(self.package)
        self.assertEqual(path, self.package.resolve())
        self.assertEqual(manifest["lib"]["name"], "ideomancer_core")
        for name in ("Cargo.toml", "Cargo.toml.orig", "src/lib.rs"):
            with self.subTest(name=name):
                file = self.package / name
                data = file.read_bytes()
                file.unlink()
                with self.assertRaises((OSError, ValueError)):
                    consumer.validate_package(self.package)
                file.write_bytes(data)

    def test_checkout_and_unnormalized_or_mismatched_manifests_rejected(self):
        (self.package / ".git").write_text("gitdir: elsewhere", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "checkout"):
            consumer.validate_package(self.package)
        (self.package / ".git").unlink()
        for text in (self.original, self.normalized.replace('version = "0.1.0"', 'version = "9.0.0"'),
                     self.normalized.replace('path = "src/lib.rs"', 'path = "../lib.rs"'),
                     self.normalized + '[dependencies.other]\npath = "../checkout"\n',
                     self.normalized + '[workspace]\n',
                     self.normalized + '[target.\'cfg(unix)\'.dependencies.other]\ngit = "elsewhere"\n'):
            with self.subTest(text=text):
                (self.package / "Cargo.toml").write_text(text, encoding="utf-8")
                with self.assertRaises(ValueError):
                    consumer.validate_package(self.package)

    def test_library_cannot_escape_through_symlink(self):
        outside = self.root / "outside.rs"
        outside.write_text("", encoding="utf-8")
        library = self.package / "src/lib.rs"
        library.unlink()
        library.symlink_to(outside)
        with self.assertRaisesRegex(ValueError, "inside"):
            consumer.validate_package(self.package)

    def assert_package_rejected_without_processes(self):
        output, errors = io.StringIO(), io.StringIO()
        with patch.object(consumer.subprocess, "run") as run, redirect_stdout(output), redirect_stderr(errors):
            code = consumer.main(["--package-dir", str(self.package)])
        self.assertEqual(code, 1)
        receipt = json.loads(output.getvalue())
        self.assertEqual(receipt["status"], "failed")
        self.assertEqual(receipt["failed_stage"], "package_validation")
        self.assertEqual(len(output.getvalue().splitlines()), 1)
        self.assertNotIn(str(self.root), output.getvalue())
        self.assertNotIn("PRIVATE", output.getvalue())
        self.assertEqual(errors.getvalue(), "")
        run.assert_not_called()

    @unittest.skipUnless(hasattr(os, "mkfifo"), "requires POSIX FIFOs")
    def test_fifo_manifests_rejected_without_blocking_or_processes(self):
        for name in ("Cargo.toml", "Cargo.toml.orig"):
            with self.subTest(name=name):
                path = self.package / name
                data = path.read_bytes()
                path.unlink()
                os.mkfifo(path)
                self.assert_package_rejected_without_processes()
                path.unlink()
                path.write_bytes(data)

    def test_oversized_manifests_rejected_before_parsing_or_processes(self):
        for name in ("Cargo.toml", "Cargo.toml.orig"):
            with self.subTest(name=name):
                path = self.package / name
                data = path.read_bytes()
                path.write_bytes(b"PRIVATE" + b"x" * 512)
                with patch.object(consumer.check_package_archive, "MAX_FILE", 512):
                    self.assert_package_rejected_without_processes()
                path.write_bytes(data)

    def test_manifest_read_has_explicit_limit_even_after_file_guard(self):
        reads = []

        class GrowingFile(io.BytesIO):
            def read(self, size=-1):
                reads.append(size)
                return super().read(size)

        @contextmanager
        def checked_file(root, name):
            yield GrowingFile(b"PRIVATE" + b"x" * 512)

        with patch.object(consumer.check_package_archive, "MAX_FILE", 512), patch.object(
            consumer.check_package_archive, "checked_file", side_effect=checked_file
        ):
            self.assert_package_rejected_without_processes()
        self.assertEqual(reads, [513])

    def test_symlink_manifests_rejected_before_processes(self):
        for name in ("Cargo.toml", "Cargo.toml.orig"):
            with self.subTest(name=name):
                path = self.package / name
                data = path.read_bytes()
                outside = self.root / "PRIVATE-manifest"
                outside.write_bytes(data)
                path.unlink()
                path.symlink_to(outside)
                self.assert_package_rejected_without_processes()
                path.unlink()
                path.write_bytes(data)

    def test_malformed_manifest_tables_fail_with_sanitized_receipt(self):
        for name in ("Cargo.toml", "Cargo.toml.orig"):
            path = self.package / name
            data = path.read_bytes()
            for table in ("package", "lib", "target", "dependencies", "dev-dependencies",
                          "build-dependencies", "features"):
                base = data.decode("utf-8")
                if table == "package":
                    base = base[:base.index('[package]')] + base[base.index('[lib]'):]
                elif table == "lib":
                    base = base[:base.index('[lib]')]
                for value in ("1", "[]", '["PRIVATE"]'):
                    with self.subTest(name=name, table=table, value=value):
                        path.write_text(consumer.NORMALIZED_HEADER + f'\n{table} = {value}\n' + base, encoding="utf-8")
                        self.assert_package_rejected_without_processes()
            for fragment in ('[target]\n"cfg(unix)" = 1\n',
                             '[target]\n"cfg(unix)" = []\n',
                             '[target."cfg(unix)"]\ndependencies = []\n',
                             '[target."cfg(unix)"]\ndependencies = 1\n'):
                path.write_text(data.decode("utf-8") + '\n' + fragment, encoding="utf-8")
                self.assert_package_rejected_without_processes()
            for table in ("dependencies", "dev-dependencies", "build-dependencies",
                          'target."cfg(unix)".dependencies',
                          'target."cfg(unix)".dev-dependencies',
                          'target."cfg(unix)".build-dependencies'):
                for value in ('1', '[]', '{version = []}', '{features = 1}', '{optional = []}'):
                    with self.subTest(name=name, table=table, value=value):
                        path.write_text(data.decode("utf-8") + f'\n[{table}]\nPRIVATE = {value}\n',
                                        encoding="utf-8")
                        self.assert_package_rejected_without_processes()
            path.write_bytes(data)

    def test_deeply_nested_or_invalid_manifest_text_is_sanitized(self):
        for name in ("Cargo.toml", "Cargo.toml.orig"):
            path = self.package / name
            data = path.read_bytes()
            for content in (b'PRIVATE = ' + b'[' * 2000 + b'0' + b']' * 2000,
                            b'PRIVATE = "\xff"', b'PRIVATE = ['):
                with self.subTest(name=name, content_length=len(content)):
                    path.write_bytes(content)
                    self.assert_package_rejected_without_processes()
            path.write_bytes(data)

    def test_checksum_is_from_verified_archive_and_extraction(self):
        _, manifest = consumer.validate_package(self.package)
        result = consumer.verify_package_archive(self.package, manifest, self.expected)
        self.assertEqual(result["status"], "passed")
        self.assertTrue(result["matched_extracted"])
        self.assertEqual(result["archive_sha256"], hashlib.sha256(self.archive.read_bytes()).hexdigest())
        self.archive.unlink()
        with self.assertRaises(OSError):
            consumer.verify_package_archive(self.package, manifest, self.expected)

    def test_generated_consumer_is_standalone_without_lock_or_extra_dependencies(self):
        root = self.root / "consumer"
        root.mkdir()
        package = self.package / 'quoted " path'
        consumer.write_consumer(root, package)
        manifest = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
        self.assertEqual(manifest["workspace"], {})
        self.assertEqual(manifest["dependencies"], {"ideomancer": {"path": str(package)}})
        self.assertFalse((root / "Cargo.lock").exists())
        self.assertEqual((root / "tests/public_api.rs").read_text(encoding="utf-8"), consumer.CONSUMER_TEST)

    def test_command_and_cli_defaults(self):
        self.assertEqual(consumer.tool_command("cargo", "1.89.0"), ["cargo", "+1.89.0"])
        self.assertEqual(consumer.tool_command("rustc", None), ["rustc"])
        for value in ("", "+nightly", "--version", "stable\n", "stable tool"):
            with self.subTest(value=value), self.assertRaises(ValueError):
                consumer.tool_command("cargo", value)
        self.assertTrue(consumer.parser().parse_args(["--package-dir", "."]).offline)
        self.assertFalse(consumer.parser().parse_args(["--package-dir", ".", "--no-offline"]).offline)

    def test_fresh_lock_checks_abi_bridge(self):
        for versions, passes in ((["0.1.5"], True), (["0.1.6"], False), ([], False),
                                 (["0.1.5", "0.1.6"], False)):
            text = ''.join(f'[[package]]\nname = "tree-sitter-language"\nversion = "{v}"\n' for v in versions)
            (self.root / "Cargo.lock").write_text(text or 'package = []\n', encoding="utf-8")
            if passes:
                self.assertEqual(consumer.check_resolution(self.root), "0.1.5")
            else:
                with self.assertRaises(ValueError):
                    consumer.check_resolution(self.root)

    def test_main_with_mocked_processes_and_no_engine_lock_copy(self):
        calls = []

        def run(command, **kwargs):
            calls.append((command, kwargs))
            cwd = kwargs["cwd"]
            self.assertFalse(cwd.is_relative_to(self.package))
            self.assertEqual(kwargs["env"]["RUSTUP_AUTO_INSTALL"], "0")
            if "--version" in command:
                return subprocess.CompletedProcess(command, 0, command[0] + " 1.89.0\n", "")
            self.assertFalse((cwd / "Cargo.lock").exists())
            (cwd / "Cargo.lock").write_text(
                '[[package]]\nname = "tree-sitter-language"\nversion = "0.1.5"\n', encoding="utf-8")
            return subprocess.CompletedProcess(command, 0)

        output = io.StringIO()
        target = self.root / "reused target"
        with patch.object(consumer.subprocess, "run", side_effect=run), redirect_stdout(output):
            code = consumer.main(["--package-dir", str(self.package), "--toolchain", "1.89.0",
                                  "--target-dir", str(target)])
        receipt = json.loads(output.getvalue())
        self.assertEqual(code, 0)
        self.assertEqual(receipt["status"], "passed")
        self.assertEqual(receipt["tool_versions"]["rustc"], "rustc 1.89.0")
        self.assertEqual(receipt["archive_checksum"],
                         {"status": "verified", "sha256": hashlib.sha256(self.archive.read_bytes()).hexdigest()})
        self.assertEqual(receipt["archive_integrity"], {"status": "verified", "matched_extracted": True})
        self.inventory.assert_called_once_with(
            (Path(consumer.__file__).resolve().parent.parent / "docs/package-files.txt").read_text(encoding="utf-8"))
        self.assertNotIn(str(self.root), output.getvalue())
        command, kwargs = calls[-1]
        self.assertEqual(command[:3], ["cargo", "+1.89.0", "test"])
        self.assertIn("--offline", command)
        self.assertEqual(command[command.index("--target-dir") + 1], str(target.resolve()))
        self.assertNotIn("--locked", command)
        self.assertFalse(kwargs["cwd"].exists(), "consumer is cleaned up")

    def test_failures_are_json_without_private_exception_contents_or_cargo_calls(self):
        output = io.StringIO()
        with patch.object(consumer.subprocess, "run") as run, redirect_stdout(output):
            code = consumer.main(["--package-dir", str(self.root / "private-missing-path")])
        self.assertEqual(code, 1)
        self.assertEqual(json.loads(output.getvalue())["failed_stage"], "package_validation")
        self.assertNotIn("private-missing-path", output.getvalue())
        run.assert_not_called()

    def test_cargo_failure_is_receipted_and_temporary_target_cleaned(self):
        roots = []

        def run(command, **kwargs):
            roots.append(kwargs["cwd"])
            if "--version" in command:
                return subprocess.CompletedProcess(command, 0, "synthetic 1.89.0\n", "")
            self.assertNotIn("--offline", command)
            self.assertEqual(command[command.index("--target-dir") + 1],
                             str(kwargs["cwd"] / "target"))
            return subprocess.CompletedProcess(command, 101)

        output = io.StringIO()
        with patch.object(consumer.subprocess, "run", side_effect=run), redirect_stdout(output):
            code = consumer.main(["--package-dir", str(self.package), "--no-offline"])
        receipt = json.loads(output.getvalue())
        self.assertEqual(code, 1)
        self.assertEqual(receipt["failed_stage"], "cargo_test")
        self.assertEqual(receipt["cargo_test_exit_code"], 101)
        self.assertEqual(receipt["target"]["mode"], "temporary")
        self.assertTrue(roots)
        self.assertTrue(all(not root.exists() for root in roots))

    def test_target_inside_package_rejected_before_running_tools(self):
        output = io.StringIO()
        with patch.object(consumer.subprocess, "run") as run, redirect_stdout(output):
            code = consumer.main(["--package-dir", str(self.package),
                                  "--target-dir", str(self.package / "target")])
        self.assertEqual(code, 1)
        self.assertEqual(json.loads(output.getvalue())["status"], "failed")
        run.assert_not_called()

    def assert_archive_rejected_without_processes(self):
        output = io.StringIO()
        with patch.object(consumer.subprocess, "run") as run, redirect_stdout(output):
            code = consumer.main(["--package-dir", str(self.package)])
        self.assertEqual(code, 1)
        receipt = json.loads(output.getvalue())
        self.assertEqual(receipt["failed_stage"], "archive_integrity")
        self.assertEqual(receipt["archive_integrity"]["status"], "failed")
        self.assertNotIn(str(self.root), output.getvalue())
        self.assertNotIn("PRIVATE", output.getvalue())
        run.assert_not_called()

    def test_missing_archive_rejected_before_processes(self):
        self.archive.unlink()
        self.assert_archive_rejected_without_processes()

    def test_tampered_archive_rejected_before_processes(self):
        self.archive.write_bytes(b"PRIVATE invalid archive")
        self.assert_archive_rejected_without_processes()

    def test_tampered_extracted_library_rejected_before_processes(self):
        (self.package / "src/lib.rs").write_bytes(b"PRIVATE changed extraction")
        self.assert_archive_rejected_without_processes()

    def test_candidate_inventory_cannot_approve_itself(self):
        docs = self.package / "docs"
        docs.mkdir()
        (docs / "package-files.txt").write_text("\n".join(sorted(self.expected)), encoding="utf-8")
        # Restore the real inventory parser: the trusted repository's full list
        # must reject this small archive, irrespective of its self-approved list.
        self.inventory.side_effect = parse_inventory
        self.assert_archive_rejected_without_processes()

    def test_script_inside_candidate_cannot_supply_trusted_inventory(self):
        with patch.object(consumer, "__file__", str(self.package / "scripts/check_package_consumer.py")):
            self.assert_archive_rejected_without_processes()
        self.inventory.assert_not_called()

    def test_mutations_after_successful_cargo_test_fail_integrity(self):
        for mutation in ("extracted", "archive", "both"):
            with self.subTest(mutation=mutation):
                (self.package / "src/lib.rs").write_bytes(b"")
                self.write_archive()
                before_hash = hashlib.sha256(self.archive.read_bytes()).hexdigest()
                calls = []

                def run(command, **kwargs):
                    calls.append(command)
                    if "--version" in command:
                        return subprocess.CompletedProcess(command, 0, "synthetic 1.89.0\n", "")
                    (kwargs["cwd"] / "Cargo.lock").write_text(
                        '[[package]]\nname = "tree-sitter-language"\nversion = "0.1.5"\n', encoding="utf-8")
                    if mutation in ("extracted", "both"):
                        (self.package / "src/lib.rs").write_bytes(b"// PRIVATE mutation\n")
                    if mutation == "both":
                        self.write_archive()  # Still matches extraction, but is a different archive.
                    elif mutation == "archive":
                        self.archive.write_bytes(b"PRIVATE corrupted after build")
                    return subprocess.CompletedProcess(command, 0)

                output = io.StringIO()
                with patch.object(consumer.subprocess, "run", side_effect=run), redirect_stdout(output):
                    code = consumer.main(["--package-dir", str(self.package)])
                receipt = json.loads(output.getvalue())
                self.assertEqual(code, 1)
                self.assertEqual(receipt["failed_stage"], "archive_integrity_after")
                self.assertEqual(receipt["cargo_test_exit_code"], 0)
                self.assertEqual(receipt["archive_checksum"], {"status": "verified", "sha256": before_hash})
                self.assertEqual(receipt["archive_integrity"]["status"], "failed")
                self.assertEqual(len(calls), 3)
                self.assertNotIn("PRIVATE", output.getvalue())
                self.assertNotIn(str(self.root), output.getvalue())


if __name__ == "__main__":
    unittest.main()
