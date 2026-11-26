"""Consumer orchestration and trusted synthetic Python process lifecycle tests."""

from contextlib import redirect_stderr, redirect_stdout
import io
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import tomllib
import unittest
from unittest.mock import Mock, patch

import check_checkout as checkout
import check_checkout_consumer as consumer
from test_checkout import fixture


def graph(root, exported):
    paths = {"ideomancer": exported / "Cargo.toml",
             "ideomancer-package-consumer": root / "Cargo.toml",
             **{name: exported / "vendor/ruff/crates" / name / "Cargo.toml" for name in consumer.LOCAL}}
    return {"packages": [
        *({"name": name, "version": "0.0.0", "source": None, "manifest_path": str(path)}
          for name, path in paths.items()),
        {"name": "tree-sitter-language", "version": "0.1.5", "source": "registry+synthetic"},
    ]}


class CheckoutConsumerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="checkout-consumer-unit-",
                                                     dir=consumer.temporary_parent(os.environ))
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.source = self.root / "source"
        self.source.mkdir()
        self.expected = fixture(self.source)
        self.trusted = self.root / "trusted"
        self.trusted.mkdir()
        self.calls = []
        self.mutation = None

    def fake_run(self, command, **kwargs):
        self.calls.append((command, kwargs))
        cwd = kwargs["cwd"]
        exported = cwd.parent / "engine"
        env = kwargs["env"]
        self.assertEqual(env["RUSTUP_AUTO_INSTALL"], "0")
        self.assertEqual(env["CARGO_TARGET_DIR"], str(self.trusted / "target"))
        self.assertEqual(env["CARGO_BUILD_BUILD_DIR"], str(self.trusted / "target"))
        self.assertEqual(env["CARGO_BUILD_JOBS"], "2")
        self.assertNotIn("IDEOMANCER_PRIVATE", env)
        self.assertNotIn("NODE_OPTIONS", env)
        self.assertTrue(Path(env["TMPDIR"]).is_relative_to(cwd.parent))
        self.assertFalse((exported / ".git").exists())
        self.assertFalse(any(arg.startswith("+") for arg in command))
        manifest = tomllib.loads((cwd / "Cargo.toml").read_text(encoding="utf-8"))
        self.assertEqual(manifest["workspace"], {})
        self.assertEqual(manifest["dependencies"], {"ideomancer": {"path": str(exported)}})
        for profile in ("dev", "test"):
            self.assertEqual(manifest["profile"][profile], {"debug": "line-tables-only", "incremental": False})
        if "generate-lockfile" in command:
            self.assertFalse((cwd / "Cargo.lock").exists())
            (cwd / "Cargo.lock").write_text(
                '[[package]]\nname = "tree-sitter-language"\nversion = "0.1.5"\n', encoding="utf-8")
        if "metadata" in command:
            return json.dumps(graph(cwd, exported)).encode()
        if "test" in command:
            self.assertEqual(command[-6:], ["--jobs", "2", "--test", "public_api", "--", "--test-threads=2"])
            self.assertIn("--locked", command)
            if self.mutation == "failure":
                raise ValueError("PRIVATE compiler failure")
            if self.mutation in ("source", "export"):
                changed = self.source if self.mutation == "source" else exported
                (changed / "src/lib.rs").write_bytes(b"// PRIVATE mutation\n")
        return b""

    def invoke(self, *args, runner=None):
        output = io.StringIO()
        with patch.object(checkout, "reviewed_paths", return_value=self.expected), \
                patch.object(consumer, "TRUSTED_ROOT", self.trusted), \
                patch.object(consumer, "check_vendor") as vendor, \
                patch.object(consumer, "run_command", side_effect=runner or self.fake_run), \
                patch.dict(os.environ, {"TMPDIR": str(self.root), "RUSTUP_AUTO_INSTALL": "1",
                                       "IDEOMANCER_PRIVATE": "PRIVATE", "NODE_OPTIONS": "PRIVATE"},
                           clear=True), redirect_stdout(output):
            code = consumer.main(["--source-dir", str(self.source), *args])
        text = output.getvalue()
        self.assertEqual(len(text.splitlines()), 1)
        self.assertNotIn(str(self.root), text)
        self.assertNotIn("PRIVATE", text)
        return code, json.loads(text), vendor

    def test_fresh_lock_then_locked_graph_then_locked_offline_test_and_cleanup(self):
        code, receipt, vendor = self.invoke()
        self.assertEqual(code, 0)
        self.assertEqual(receipt["status"], "passed")
        self.assertEqual(receipt["tree_sitter_language"], "0.1.5")
        self.assertEqual(receipt["source_sha256"], checkout.snapshot(self.source, self.expected)["source_sha256"])
        self.assertEqual(vendor.call_count, 3)
        self.assertEqual(len(self.calls), 3)
        for command, kwargs in self.calls:
            self.assertIn("--offline", command)
            self.assertFalse(kwargs["cwd"].exists(), "framework removes isolated run")
        self.assertIn("generate-lockfile", self.calls[0][0])
        self.assertNotIn("--locked", self.calls[0][0])
        self.assertIn("metadata", self.calls[1][0])
        self.assertIn("--locked", self.calls[1][0])
        self.assertFalse((self.source / "target").exists())

    def test_online_permission_does_not_change_toolchain_or_locked_test(self):
        code, _, _ = self.invoke("--no-offline")
        self.assertEqual(code, 0)
        for command, _ in self.calls:
            self.assertNotIn("--offline", command)

    def test_failure_and_source_or_export_mutation_are_cleaned(self):
        for mutation in ("failure", "source", "export"):
            with self.subTest(mutation=mutation):
                fixture(self.source)
                self.mutation = mutation
                code, receipt, _ = self.invoke()
                self.assertEqual(code, 1)
                self.assertEqual(receipt["failed_stage"], "cargo_test" if mutation == "failure"
                                 else "source_validation_after")
                self.assertFalse(self.calls[-1][1]["cwd"].exists())

    def test_boundary_and_target_failure_start_no_commands(self):
        for target in (self.source / "target", self.root / "external"):
            code, receipt, _ = self.invoke("--target-dir", str(target))
            self.assertEqual(code, 1)
            self.assertEqual(receipt["failed_stage"], "consumer_setup")
        (self.source / "src/extra.rs").write_bytes(b"// PRIVATE\n")
        code, receipt, _ = self.invoke()
        self.assertEqual(code, 1)
        self.assertEqual(receipt["failed_stage"], "source_validation")
        self.assertFalse(self.calls)

    def test_target_overrides_and_symlinks_rejected(self):
        with patch.object(consumer, "TRUSTED_ROOT", self.trusted):
            self.assertEqual(consumer.validate_target(None, {}), self.trusted / "target")
            for env in ({"CARGO_TARGET_DIR": str(self.root)}, {"CARGO_BUILD_BUILD_DIR": str(self.root)},
                        {"CARGO_PROFILE_DEV_DEBUG": "2"}, {"RUSTFLAGS": "--cfg custom"}):
                with self.subTest(env=env), self.assertRaises(ValueError):
                    consumer.validate_target(None, env)
            (self.trusted / "target").symlink_to(self.root / "elsewhere", target_is_directory=True)
            with self.assertRaises(ValueError):
                consumer.validate_target(None, {})

    def test_export_detects_mutation_and_never_copies_private_trees(self):
        (self.source / ".git").mkdir()
        before = checkout.snapshot(self.source, self.expected)
        destination = self.root / "export"
        consumer.export_source(self.source, destination, before)
        self.assertFalse((destination / ".git").exists())
        (self.source / "src/lib.rs").write_bytes(b"// different\n")
        with self.assertRaises(ValueError):
            consumer.export_source(self.source, self.root / "bad-export", before)

    def test_graph_rejects_external_git_wrong_ruff_and_wrong_abi(self):
        exported, root = self.root / "engine", self.root / "consumer"
        valid = graph(root, exported)
        self.assertTrue(consumer.check_graph(valid, root, exported))
        for index, key, value in ((0, "manifest_path", str(self.root / "outside/Cargo.toml")),
                                  (0, "source", "git+https://example.invalid/repo"),
                                  (2, "version", "9.0.0"), (-1, "version", "0.1.6"),
                                  (-1, "source", "git+https://example.invalid/repo")):
            candidate = graph(root, exported)
            candidate["packages"][index][key] = value
            with self.subTest(index=index, key=key), self.assertRaises(ValueError):
                consumer.check_graph(candidate, root, exported)

    def test_temp_storage_and_timeout_guards(self):
        for location in ("/tmp", "/dev/shm"):
            if Path(location).exists():
                with self.assertRaises(ValueError):
                    consumer.temporary_parent({"TMPDIR": location})
        for timeout in ("0", "-1", "nan", "inf", "3601"):
            code, _, _ = self.invoke("--timeout", timeout)
            self.assertEqual(code, 1)
        self.assertFalse(self.calls)
        with patch.object(consumer.shutil, "disk_usage", return_value=Mock(free=0)), self.assertRaises(ValueError):
            consumer.temporary_parent({"TMPDIR": str(self.root)})

    def test_subprocess_timeout_stops_owned_group_before_return(self):
        process = Mock(pid=12345)
        process.poll.return_value = None
        with patch.object(consumer.subprocess, "Popen", return_value=process) as popen, \
                patch.object(consumer, "finalize_group") as finalize, \
                self.assertRaises(ValueError):
            consumer.run_command(["cargo", "test"], cwd=self.root, env={"TMPDIR": str(self.root)}, timeout=0)
        self.assertTrue(popen.call_args.kwargs["start_new_session"])
        finalize.assert_called_once_with(process, str(self.root))

    def test_temporary_cleanup_failure_is_explicit(self):
        errors = io.StringIO()
        owned = None
        try:
            with patch.object(consumer, "temporary_parent", return_value=self.root), \
                    patch.object(consumer.shutil, "rmtree", side_effect=OSError("PRIVATE cleanup failure")), \
                    redirect_stderr(errors), self.assertRaises(consumer.CleanupError):
                with consumer.isolated_run({}) as owned:
                    (owned / "kept.txt").write_bytes(b"bounded synthetic evidence")
            self.assertTrue(owned.is_dir())
            self.assertIn(str(owned), errors.getvalue())
            self.assertIn("Cleanup plan:", errors.getvalue())
            self.assertNotIn("PRIVATE", errors.getvalue())
        finally:
            if owned is not None and owned.exists():
                consumer.shutil.rmtree(owned)

    def test_captured_metadata_is_bounded_without_real_processes(self):
        process = Mock(pid=12345, returncode=0)
        process.poll.return_value = 0

        def start(command, **kwargs):
            kwargs["stdout"].write(b"x" * 17)
            kwargs["stdout"].flush()
            return process

        with patch.object(consumer.subprocess, "Popen", side_effect=start), \
                patch.object(consumer, "MAX_OUTPUT", 16), patch.object(consumer, "finalize_group") as finalize, \
                self.assertRaises(ValueError):
            consumer.run_command(["cargo", "metadata"], cwd=self.root, env={"TMPDIR": str(self.root)},
                                 timeout=1, capture=True)
        finalize.assert_called_once_with(process, str(self.root))

    def test_success_failure_and_interrupt_always_finalize_owned_group(self):
        for outcome in ("success", "failure", "interrupt"):
            process = Mock(pid=12345, returncode=1 if outcome == "failure" else 0)
            process.poll.side_effect = KeyboardInterrupt if outcome == "interrupt" else None
            process.poll.return_value = 0
            with self.subTest(outcome=outcome), \
                    patch.object(consumer.subprocess, "Popen", return_value=process), \
                    patch.object(consumer, "finalize_group") as finalize:
                if outcome == "success":
                    self.assertEqual(consumer.run_command(["synthetic"], cwd=self.root,
                                     env={"TMPDIR": str(self.root)}, timeout=1), b"")
                else:
                    with self.assertRaises(KeyboardInterrupt if outcome == "interrupt" else ValueError):
                        consumer.run_command(["synthetic"], cwd=self.root,
                                             env={"TMPDIR": str(self.root)}, timeout=1)
                finalize.assert_called_once_with(process, str(self.root))

    def test_finalizer_escalates_only_owned_group_and_reaps_parent(self):
        process = Mock(pid=12345)
        with patch.object(consumer.os, "killpg") as kill, \
                patch.object(consumer, "group_live", side_effect=[True, False]), \
                patch.object(consumer, "TERM_GRACE", 0), patch.object(consumer.time, "sleep"):
            consumer.finalize_group(process, self.root)
        self.assertEqual(kill.call_args_list[0].args, (12345, signal.SIGTERM))
        self.assertEqual(kill.call_args_list[1].args, (12345, signal.SIGKILL))
        self.assertEqual(kill.call_count, 2)
        process.wait.assert_called_once()
        self.assertGreater(process.wait.call_args.kwargs["timeout"], 0)

    def test_finalizer_unknown_or_live_group_blocks_cleanup(self):
        for unknown in (True, False):
            process = Mock(pid=12345)
            live = Mock(side_effect=consumer.CleanupError("unknown")) if unknown else Mock(return_value=True)
            with self.subTest(unknown=unknown), patch.object(consumer.os, "killpg"), \
                    patch.object(consumer, "group_live", live), patch.object(consumer, "GROUP_TIMEOUT", 0.02), \
                    patch.object(consumer, "TERM_GRACE", 0), self.assertRaises(consumer.CleanupError):
                consumer.finalize_group(process, self.root)

    def test_liveness_ignores_zombies_but_not_live_or_unknown_states(self):
        for state, expected in ((b"Z", False), (b"Z+", False), (b"S", True), (b"R+", True), (b"?", None)):
            inspector = Mock(returncode=0)
            inspector.poll.return_value = 0

            def start(command, **kwargs):
                self.assertEqual(command, ["ps", "-A", "-o", "pid=,pgid=,stat="])
                kwargs["stdout"].write(b"12346 12345 " + state + b"\n")
                kwargs["stdout"].flush()
                return inspector

            with self.subTest(state=state), patch.object(consumer.os, "killpg"), \
                    patch.object(consumer.subprocess, "Popen", side_effect=start):
                if expected is None:
                    with self.assertRaises(consumer.CleanupError):
                        consumer.group_live(12345, self.root, time.monotonic() + 1)
                else:
                    self.assertEqual(consumer.group_live(12345, self.root, time.monotonic() + 1), expected)
                inspector.wait.assert_called_once()

    def test_process_inspection_output_and_runtime_are_bounded(self):
        for oversized in (True, False):
            inspector = Mock(returncode=0)
            inspector.poll.return_value = 0 if oversized else None

            def start(command, **kwargs):
                kwargs["stdout"].write(b"x" * 17 if oversized else b"")
                kwargs["stdout"].flush()
                return inspector

            with self.subTest(oversized=oversized), patch.object(consumer.os, "killpg"), \
                    patch.object(consumer.subprocess, "Popen", side_effect=start), \
                    patch.object(consumer, "MAX_PROCESS_OUTPUT", 16), self.assertRaises(consumer.CleanupError):
                consumer.group_live(12345, self.root, time.monotonic() - 1)
            inspector.wait.assert_called_once_with(timeout=0.5)
            self.assertEqual(inspector.kill.call_count, 0 if oversized else 1)

    def test_signal_during_finalization_is_deferred_until_children_stop_then_files_removed(self):
        owned = None
        process = Mock(pid=12345, returncode=0)
        process.poll.return_value = 0

        def stopped(*args):
            signal.getsignal(signal.SIGTERM)(signal.SIGTERM, None)
            return False

        with patch.object(consumer, "temporary_parent", return_value=self.root), \
                patch.object(consumer.subprocess, "Popen", return_value=process), \
                patch.object(consumer.os, "killpg"), patch.object(consumer, "group_live", side_effect=stopped), \
                self.assertRaises(KeyboardInterrupt):
            with consumer.isolated_run({}) as owned:
                consumer.run_command(["synthetic"], cwd=owned, env={"TMPDIR": str(owned)}, timeout=1)
        process.wait.assert_called_once()
        self.assertFalse(owned.exists())

    def test_unverified_shutdown_retains_only_owned_run_and_reports_cleanup_plan(self):
        errors = io.StringIO()
        owned = None
        try:
            with patch.object(consumer, "temporary_parent", return_value=self.root), \
                    redirect_stderr(errors), self.assertRaises(consumer.CleanupError):
                with consumer.isolated_run({}) as owned:
                    (owned / "kept.txt").write_bytes(b"bounded synthetic evidence")
                    process = Mock(pid=12345, returncode=0)
                    process.poll.return_value = 0
                    with patch.object(consumer.subprocess, "Popen", return_value=process), \
                            patch.object(consumer, "finalize_group", side_effect=consumer.CleanupError("unknown")):
                        consumer.run_command(["synthetic"], cwd=owned,
                                             env={"TMPDIR": str(owned)}, timeout=1)
            self.assertEqual((owned / "kept.txt").read_bytes(), b"bounded synthetic evidence")
            self.assertIn(str(owned), errors.getvalue())
            self.assertIn("Cleanup plan:", errors.getvalue())
            self.assertIn("verify", errors.getvalue())
        finally:
            # There were no real children in this mocked unprovable-shutdown case.
            if owned is not None and owned.exists():
                consumer.shutil.rmtree(owned)

    def test_unverified_shutdown_receipt_reports_failure_without_retained_paths(self):
        errors = io.StringIO()
        owned = []

        def unverified(command, **kwargs):
            owned.append(kwargs["cwd"].parent)
            raise consumer.CleanupError("PRIVATE unobservable children")

        try:
            with redirect_stderr(errors):
                code, receipt, _ = self.invoke(runner=unverified)
            self.assertEqual(code, 1)
            self.assertEqual(receipt["failed_stage"], "temporary_cleanup")
            self.assertTrue(owned[0].is_dir())
            self.assertIn(str(owned[0]), errors.getvalue())
            self.assertIn("Cleanup plan:", errors.getvalue())
            self.assertNotIn("PRIVATE", errors.getvalue())
            self.assertNotIn(str(owned[0]), json.dumps(receipt))
        finally:
            # All build calls were mocked; there are no actual children to await.
            for root in owned:
                if root.exists():
                    consumer.shutil.rmtree(root)

    @unittest.skipUnless(os.name == "posix", "POSIX process groups required")
    def test_successful_parent_cannot_leave_sigterm_ignoring_child_running(self):
        child = self.root / "child.py"
        parent = self.root / "parent.py"
        ready = self.root / "ready.txt"
        child.write_text(
            "import os, signal, sys, time\nfrom pathlib import Path\n"
            "signal.signal(signal.SIGTERM, signal.SIG_IGN)\n"
            "Path(sys.argv[1]).write_text(f'{os.getpid()} {os.getpgrp()}')\n"
            "time.sleep(30)\n", encoding="utf-8")
        parent.write_text(
            "import subprocess, sys, time\nfrom pathlib import Path\n"
            "subprocess.Popen([sys.executable, sys.argv[1], sys.argv[2]])\n"
            "ready = Path(sys.argv[2])\ndeadline = time.monotonic() + 5\n"
            "while not ready.exists() or not ready.read_text():\n"
            "    if time.monotonic() >= deadline: raise RuntimeError('child not ready')\n"
            "    time.sleep(0.01)\n"
            "print(ready.read_text(), flush=True)\n", encoding="utf-8")
        real_popen = subprocess.Popen
        owned_processes = []

        def start(command, **kwargs):
            process = real_popen(command, **kwargs)
            if command[0] == sys.executable:
                owned_processes.append(process)
            return process

        try:
            with patch.object(consumer.subprocess, "Popen", side_effect=start):
                output = consumer.run_command([sys.executable, str(parent), str(child), str(ready)],
                    cwd=self.root, env={**os.environ, "TMPDIR": str(self.root)}, timeout=8, capture=True)
            self.assertLess(len(output), 64)
            child_pid, pgid = map(int, output.split())
            self.assertGreater(child_pid, 1)
            self.assertEqual(pgid, owned_processes[0].pid)
            self.assertEqual(owned_processes[0].returncode, 0)
            self.assertFalse(consumer.group_live(pgid, self.root, time.monotonic() + 1),
                             "child must be stopped before run_command returns (zombies are not live)")
        finally:
            # Always stop only the group this test created, even if an assertion
            # or the finalizer under test fails. Reap only our actual child.
            for process in owned_processes:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.wait(timeout=2)
                deadline = time.monotonic() + 2
                while consumer.group_live(process.pid, self.root, deadline):
                    if time.monotonic() >= deadline:
                        self.fail("synthetic child did not stop after unconditional test teardown KILL")
                    time.sleep(0.01)


if __name__ == "__main__":
    unittest.main()
