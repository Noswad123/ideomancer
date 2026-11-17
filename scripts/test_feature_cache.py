"""Synthetic temporary checkout tests; never run real Git, ps, or Cargo."""

from contextlib import redirect_stderr, redirect_stdout
import io
import json
import os
from pathlib import Path
import stat
import subprocess
import tempfile
import tomllib
import unittest
from unittest.mock import patch

import feature_cache as cache


class FeatureCacheTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        (self.root / "Cargo.toml").write_text(
            '[package]\nname = "ideomancer"\n[lib]\nname = "ideomancer_core"\n')
        self.target = self.root / "target"
        self.target.mkdir()
        self.artifact = self.target / "artifact"
        self.artifact.write_bytes(b"private fixture contents")
        self.calls = []
        self.ps = "1 init\n42 /usr/bin/python3\n"
        self.ps_code = 0
        self.git_root = str(self.root)
        self.cargo_code = 0
        self.remove_artifact = True
        self.before_cargo = None
        self.ps_hook = None
        self.addCleanup(patch.stopall)
        patch.dict(os.environ, {}, clear=True).start()
        patch.object(cache.shutil, "which", return_value="/tools/cargo").start()
        self.run_mock = patch.object(cache.subprocess, "run", side_effect=self.command).start()

    def command(self, command, **kwargs):
        self.calls.append(command)
        self.assertIsInstance(command, list)
        self.assertNotIn("shell", kwargs)
        if command[0] == "git":
            self.assertEqual(command, ["git", "-C", str(self.root), "rev-parse", "--show-toplevel"])
            return subprocess.CompletedProcess(command, 0, self.git_root + "\n", "")
        if command[0] == "ps":
            self.assertEqual(command, ["ps", "-A", "-o", "pid=", "-o", "comm="])
            if self.ps_hook:
                self.ps_hook()
            return subprocess.CompletedProcess(command, self.ps_code, self.ps, "")
        self.assertEqual(command, ["cargo", "--config",
                                   "build.build-dir=" + json.dumps(str(self.target), ensure_ascii=False),
                                   "clean", "--manifest-path",
                                   str(self.root / "Cargo.toml"), "--target-dir", str(self.target)])
        self.assertEqual(kwargs["cwd"], self.root)
        if self.before_cargo:
            self.before_cargo()
        if self.remove_artifact:
            self.artifact.unlink()
        return subprocess.CompletedProcess(command, self.cargo_code, "private tool output", "private tool error")

    def run_cache(self, apply=False):
        return cache._run(self.root, apply=apply)

    def assert_blocked(self, reason=None):
        receipt = self.run_cache(apply=True)
        self.assertEqual(receipt["status"], "blocked", receipt)
        if reason:
            self.assertIn(reason, receipt["reason"])
        self.assertFalse(receipt["tool"]["invoked"])
        self.assertTrue(all(call[0] != "cargo" for call in self.calls))
        return receipt

    def test_dry_run_counts_and_privacy(self):
        receipt = self.run_cache()
        self.assertEqual(receipt["status"], "dry_run")
        self.assertEqual(receipt["before"]["logical_bytes"], self.artifact.stat().st_size)
        expected = sum(path.stat().st_blocks * 512 for path in (self.target, self.artifact))
        self.assertEqual(receipt["before"]["allocated_bytes"], expected)
        self.assertEqual(receipt["before"]["files"], 1)
        self.assertFalse(receipt["tool"]["invoked"])
        self.assertTrue(self.artifact.exists())
        self.assertIsNotNone(receipt["disk_before"])
        self.assertIsNotNone(receipt["disk_after"])
        encoded = json.dumps(receipt)
        self.assertNotIn(str(self.root), encoded)
        self.assertNotIn("private fixture contents", encoded)

    def test_apply_exact_command_and_empty_verification(self):
        receipt = self.run_cache(apply=True)
        self.assertEqual(receipt["status"], "cleaned", receipt)
        self.assertEqual(receipt["after"]["entries"], 0)
        self.assertEqual([call[0] for call in self.calls], ["git", "ps", "ps", "cargo"])
        self.assertNotIn("private tool", json.dumps(receipt))

    def test_cli_config_overrides_project_ancestor_and_global(self):
        original = self.root
        checkout = original / "parent/checkout"
        checkout.mkdir(parents=True)
        (original / "Cargo.toml").rename(checkout / "Cargo.toml")
        self.target.rename(checkout / "target")
        self.root = checkout
        self.target = checkout / "target"
        self.artifact = self.target / "artifact"
        self.git_root = str(checkout)
        cargo_home = original / "cargo-home"
        for directory in (checkout / ".cargo", checkout.parent / ".cargo", cargo_home):
            directory.mkdir()
            (directory / "config.toml").write_text(
                '[build]\nbuild-dir = "/private-fixture/external-cache"\n')
        with patch.dict(os.environ, {"CARGO_HOME": str(cargo_home)}):
            receipt = self.run_cache(apply=True)
        self.assertEqual(receipt["status"], "cleaned", receipt)
        command = self.calls[-1]
        self.assertEqual(command[:2], ["cargo", "--config"])
        self.assertEqual(tomllib.loads(command[2])["build"]["build-dir"], str(self.target))
        self.assertNotIn("/private-fixture", json.dumps(receipt))
        self.assertNotIn(str(cargo_home), json.dumps(receipt))
        self.assertNotIn("command", receipt["tool"])

    def test_cli_config_toml_quoting_and_receipt_privacy(self):
        checkout = self.root / 'space "quote" \\ slash \u00e9'
        checkout.mkdir()
        (self.root / "Cargo.toml").rename(checkout / "Cargo.toml")
        self.target.rename(checkout / "target")
        self.root = checkout
        self.target = checkout / "target"
        self.artifact = self.target / "artifact"
        self.git_root = str(checkout)
        receipt = self.run_cache(apply=True)
        self.assertEqual(receipt["status"], "cleaned", receipt)
        config = self.calls[-1][2]
        self.assertEqual(tomllib.loads(config)["build"]["build-dir"], str(self.target))
        self.assertIn("\u00e9", config)
        self.assertNotIn(str(checkout), json.dumps(receipt, ensure_ascii=False))
        self.assertNotIn(config, json.dumps(receipt, ensure_ascii=False))

    def test_root_braces_rejected_without_commands(self):
        for name in ("{workspace-root}", "{cargo-cache-home}", "unknown{value}", "closing}"):
            with self.subTest(name=name):
                receipt = cache._run(self.root / name, apply=True)
                self.assertEqual(receipt["status"], "blocked")
                self.assertIn("root braces unsupported", receipt["reason"])
        self.assertEqual(self.calls, [])

    def test_apply_target_absent_after_cargo(self):
        def remove_target():
            self.artifact.unlink()
            self.target.rmdir()
        self.before_cargo = remove_target
        self.remove_artifact = False
        receipt = self.run_cache(apply=True)
        self.assertEqual(receipt["status"], "cleaned")
        self.assertTrue(receipt["after"]["absent"])

    def test_missing_target_noop_even_without_cargo(self):
        self.artifact.unlink()
        self.target.rmdir()
        with patch.object(cache.shutil, "which", return_value=None):
            receipt = self.run_cache(apply=True)
        self.assertEqual(receipt["status"], "already_clean")
        self.assertEqual([call[0] for call in self.calls], ["git"])
        self.assertFalse(self.target.exists())

    def test_manifest_identity(self):
        (self.root / "Cargo.toml").write_text('[package]\nname = "other"\n')
        self.assert_blocked("manifest identity")

    def test_git_mismatch(self):
        self.git_root = str(self.root.parent)
        self.assert_blocked("git root")

    def test_git_command_failure(self):
        self.run_mock.side_effect = lambda command, **kwargs: subprocess.CompletedProcess(
            command, 1, "", "private git error")
        receipt = self.assert_blocked("git root")
        self.assertNotIn("private git error", json.dumps(receipt))

    def test_worktree_git_file_allowed(self):
        (self.root / ".git").write_text("gitdir: fixture metadata\n")
        self.assertEqual(self.run_cache()["status"], "dry_run")

    def test_target_symlink(self):
        self.artifact.unlink()
        self.target.rmdir()
        self.target.symlink_to(self.root, target_is_directory=True)
        self.assert_blocked("without symlinks")

    def test_root_and_ancestor_symlinks(self):
        alias = self.root / "alias"
        alias.symlink_to(self.root, target_is_directory=True)
        for root in (alias, alias / "child"):
            with self.subTest(root=root):
                receipt = cache._run(root, apply=True)
                self.assertEqual(receipt["status"], "blocked")
        self.assertEqual(self.calls, [])

    def test_non_directory_target(self):
        self.artifact.unlink()
        self.target.rmdir()
        self.target.write_text("not a directory")
        self.assert_blocked("real directories")

    def test_non_directory_root(self):
        self.assertEqual(cache._run(self.artifact, apply=True)["status"], "blocked")

    def test_manifest_symlink(self):
        manifest = self.root / "Cargo.toml"
        manifest.rename(self.root / "saved.toml")
        manifest.symlink_to(self.root / "saved.toml")
        self.assert_blocked("manifest must be")

    def test_mounts(self):
        with patch.object(cache.os.path, "ismount", return_value=True):
            self.assert_blocked("mount point")

    def test_nested_mount(self):
        nested = self.target / "nested"
        nested.mkdir()
        with patch.object(cache.os.path, "ismount", side_effect=lambda p: p == nested):
            self.assert_blocked("nested mount")

    def test_environment_overrides(self):
        for variable in ("CARGO_TARGET_DIR", "CARGO_BUILD_TARGET_DIR", "CARGO_BUILD_BUILD_DIR"):
            for value in ("../external", "/external/cache", "", "target/../other"):
                with self.subTest(variable=variable, value=value), patch.dict(os.environ, {variable: value}):
                    self.assert_blocked(variable)

    def test_local_environment_allowed(self):
        for value in ("target", str(self.target)):
            with self.subTest(value=value), patch.dict(os.environ, {"CARGO_TARGET_DIR": value,
                                                                  "CARGO_BUILD_TARGET_DIR": value,
                                                                  "CARGO_BUILD_BUILD_DIR": value}):
                self.assertEqual(self.run_cache()["status"], "dry_run")

    def test_nested_symlinks_never_followed(self):
        for destination in (self.artifact, self.root, self.root / "missing"):
            link = self.target / "link"
            link.symlink_to(destination)
            with self.subTest(destination=destination):
                self.assert_blocked("symlink or special")
            link.unlink()

    def test_special_file(self):
        os.mkfifo(self.target / "pipe", stat.S_IRUSR | stat.S_IWUSR)
        self.assert_blocked("special file")

    def test_protected_names_bounded_and_no_contents(self):
        for index in range(25):
            directory = self.target / str(index)
            directory.mkdir()
            for name in cache.PROTECTED:
                (directory / name).write_text("private saved result")
        receipt = self.assert_blocked("preserve-required")
        self.assertEqual(receipt["before"]["protected_count"], 125)
        self.assertEqual(len(receipt["before"]["protected_samples"]), 20)
        for name in receipt["before"]["protected_samples"]:
            self.assertFalse(Path(name).is_absolute())
        self.assertNotIn("private saved result", json.dumps(receipt))

    def test_name_inside_source_contents_is_not_protected(self):
        directory = self.target / "package/src/tests"
        directory.mkdir(parents=True)
        (directory / "fixture.rs").write_text('let fixture = "receipt.json";')
        self.assertEqual(self.run_cache()["status"], "dry_run")

    def test_hardlink_allocation_counted_once_across_directories(self):
        directory = self.target / "nested"
        directory.mkdir()
        linked = directory / "linked"
        os.link(self.artifact, linked)
        result = cache._inspect(self.target)
        self.assertEqual(result["files"], 2)
        self.assertEqual(result["logical_bytes"], 2 * self.artifact.stat().st_size)
        self.assertEqual(result["allocated_bytes"], sum(
            path.stat().st_blocks * 512 for path in (self.target, directory, self.artifact)))
        self.assertEqual(result["entries"], 3)

    def test_nested_sparse_byte_accounting(self):
        directory = self.target / "nested"
        directory.mkdir()
        sparse = directory / "sparse"
        with sparse.open("wb") as stream:
            stream.truncate(1024 * 1024)
        result = cache._inspect(self.target)
        self.assertEqual(result["logical_bytes"], self.artifact.stat().st_size + sparse.stat().st_size)
        self.assertEqual(result["allocated_bytes"], sum(
            path.stat().st_blocks * 512 for path in (self.target, self.artifact, directory, sparse)))
        self.assertEqual(result["entries"], 3)
        self.assertEqual(result["files"], 2)

    def test_process_guards(self):
        for executable in (*cache.BUILD_PROCESSES, "/tools/cargo", str(self.target / "debug/app")):
            with self.subTest(executable=executable):
                self.ps = f"42 {executable}\n"
                receipt = self.assert_blocked("active build")
                self.assertEqual(receipt["processes"]["blocking_count"], 1)

    def test_process_path_boundary(self):
        self.ps = f"42 {self.target}-other/app\n"
        self.assertEqual(self.run_cache()["status"], "dry_run")

    def test_process_basename_only_and_truncated(self):
        executable = self.target / "long-fixture-executable"
        executable.write_bytes(b"not executed")
        executable.chmod(0o700)
        for name in (executable.name, executable.name[:15]):
            with self.subTest(name=name):
                self.ps = f"42 {name}\n"
                receipt = self.assert_blocked("active build")
                self.assertEqual(receipt["processes"]["blocking_names"], ["target executable"])

    def test_process_relative_paths_and_shared_basenames_block(self):
        executable = self.target / "cache-fixture"
        executable.write_bytes(b"not executed")
        executable.chmod(0o700)
        for command in ("./target/debug/cache-fixture", "target/debug/cache-fixture",
                        "../other/cache-fixture", "/other/cache-fixture"):
            for apply in (False, True):
                with self.subTest(command=command, apply=apply):
                    self.ps = f"42 {command}\n"
                    receipt = self.run_cache(apply=apply)
                    self.assertEqual(receipt["status"], "blocked")
                    self.assertEqual(receipt["processes"]["blocking_names"], ["target executable"])
        self.assertTrue(all(call[0] != "cargo" for call in self.calls))

    def test_relative_process_appears_at_final_check(self):
        executable = self.target / "cache-fixture"
        executable.write_bytes(b"not executed")
        executable.chmod(0o700)
        checks = 0
        def hook():
            nonlocal checks
            checks += 1
            if checks == 2:
                self.ps = "42 ./target/debug/cache-fixture\n"
        self.ps_hook = hook
        self.assert_blocked("active build")
        self.assertEqual(checks, 2)

    def test_ps_failed_empty_invalid_or_unavailable(self):
        for output, code in (("", 0), ("broken", 0), ("42 cargo", 1)):
            with self.subTest(output=output, code=code):
                self.ps, self.ps_code = output, code
                self.assert_blocked("process check")
        self.run_mock.side_effect = FileNotFoundError()
        self.assert_blocked("unavailable")

    def test_ps_executable_unavailable(self):
        original = self.command
        def command(args, **kwargs):
            if args[0] == "ps":
                raise FileNotFoundError()
            return original(args, **kwargs)
        self.run_mock.side_effect = command
        receipt = self.assert_blocked("unavailable")
        self.assertEqual(receipt["processes"]["status"], "unavailable")

    def test_process_appears_at_final_check(self):
        checks = 0
        def hook():
            nonlocal checks
            checks += 1
            if checks == 2:
                self.ps = "42 cargo\n"
        self.ps_hook = hook
        self.assert_blocked("active build")

    def test_cargo_unavailable(self):
        with patch.object(cache.shutil, "which", return_value=None):
            self.assert_blocked("cargo unavailable")

    def test_target_swapped_before_clean(self):
        checks = 0
        def hook():
            nonlocal checks
            checks += 1
            if checks == 2:
                self.target.rename(self.root / "old-target")
                self.target.mkdir()
        self.ps_hook = hook
        self.assert_blocked("identity changed")

    def test_target_symlink_swap_before_clean(self):
        def hook():
            self.target.rename(self.root / "old-target")
            self.target.symlink_to(self.root, target_is_directory=True)
            self.ps_hook = None
        self.ps_hook = hook
        self.assert_blocked("without symlinks")

    def test_cargo_failure_not_quiet_success(self):
        self.cargo_code = 1
        receipt = self.run_cache(apply=True)
        self.assertEqual(receipt["status"], "failed")
        self.assertEqual(receipt["tool"]["returncode"], 1)
        self.assertIn("cargo clean failed", receipt["reason"])
        self.assertIsNotNone(receipt["disk_after"])

    def test_cargo_spawn_failure(self):
        original = self.command
        def command(args, **kwargs):
            if args[0] == "cargo":
                raise OSError("private execution error")
            return original(args, **kwargs)
        self.run_mock.side_effect = command
        receipt = self.run_cache(apply=True)
        self.assertEqual(receipt["status"], "failed")
        self.assertNotIn("private execution error", json.dumps(receipt))

    def test_leftovers_fail(self):
        self.remove_artifact = False
        receipt = self.run_cache(apply=True)
        self.assertEqual(receipt["status"], "failed")
        self.assertIn("left artifacts", receipt["reason"])
        self.assertGreater(receipt["after"]["logical_bytes"], 0)

    def test_cli_infers_root_and_prints_json(self):
        script = self.root / "scripts/feature_cache.py"
        with patch.object(cache, "__file__", str(script)), redirect_stdout(io.StringIO()) as output:
            self.assertEqual(cache.main([]), 0)
        self.assertEqual(json.loads(output.getvalue())["status"], "dry_run")
        self.assertTrue(self.artifact.exists())

    def test_cli_apply_and_blocked_exit(self):
        with patch.object(cache, "__file__", str(self.root / "scripts/feature_cache.py")):
            with redirect_stdout(io.StringIO()) as output:
                self.assertEqual(cache.main(["--apply"]), 0)
            self.assertEqual(json.loads(output.getvalue())["status"], "cleaned")
            self.ps = "42 cargo\n"
            with redirect_stdout(io.StringIO()) as output:
                self.assertEqual(cache.main([]), 1)
            self.assertEqual(json.loads(output.getvalue())["status"], "blocked")

    def test_cli_rejects_path_overrides(self):
        for option in ("--root", "--target", "--target-dir", "--force", "--app"):
            with self.subTest(option=option), redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit) as error:
                    cache.main([option, str(self.root)])
                self.assertEqual(error.exception.code, 2)
        self.assertEqual(self.calls, [])


if __name__ == "__main__":
    unittest.main()
