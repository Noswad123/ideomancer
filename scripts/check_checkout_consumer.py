"""Test a fresh independent consumer of a reviewed Git-source snapshot.

No Cargo package/archive normalization or registry publication is required. The
snapshot includes the reviewed local Ruff crates. Only this engine's synthetic
public API test executes; analyzed applications are never built or imported.
Offline by default; --no-offline permits registry access, never tool installation.
Temporary source/consumer data is cleaned only after owned-group shutdown is
verified. An unverified shutdown retains that run with a cleanup plan on stderr.
The trusted repository target/ is reused for coordinated feature-cache closeout.
Not an OS sandbox.
Compiler diagnostics on stderr can contain paths; JSON stdout never does.
"""

import argparse
from contextlib import contextmanager
import json
import math
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time

import check_checkout as checkout
from check_package_consumer import write_consumer, check_resolution
from check_ruff_vendor import check_vendor, LOCAL


TRUSTED_ROOT = checkout.TRUSTED_ROOT
MAX_OUTPUT = 8 * 1024 * 1024
MIN_FREE = 128 * 1024 * 1024
MAX_PROCESS_OUTPUT = 1024 * 1024
GROUP_TIMEOUT = 3.0
TERM_GRACE = 0.2


class CleanupError(ValueError):
    """Shutdown is unverified or owned data could not be removed; no receipt paths."""


def validate_target(requested, env):
    target = TRUSTED_ROOT / "target"
    given = Path(os.path.abspath(requested)) if requested else target
    if given != target or target.is_symlink() or (target.exists() and not target.is_dir()):
        raise ValueError("only the trusted project target is allowed")
    for key in ("CARGO_TARGET_DIR", "CARGO_BUILD_TARGET_DIR", "CARGO_BUILD_BUILD_DIR"):
        if key in env and Path(os.path.abspath(env[key])) != target:
            raise ValueError("external build directory override")
    if any(k.startswith("CARGO_PROFILE_") or k in {"CARGO_ENCODED_RUSTFLAGS", "RUSTFLAGS",
           "CARGO_INCREMENTAL", "CARGO_BUILD_RUSTFLAGS", "CARGO_BUILD_TARGET"} for k in env):
        raise ValueError("unsupported build configuration override")
    return target


def temporary_parent(env):
    # A caller-supplied TMPDIR is an explicit disk-backed run owned by the caller.
    if env.get("TMPDIR"):
        parent = Path(env["TMPDIR"]).resolve(strict=True)
    else:
        base = Path(env.get("XDG_CACHE_HOME", str(Path.home() / ".cache")))
        parent = base / "agent-test-runs"
        parent.mkdir(parents=True, exist_ok=True)
        parent = parent.resolve(strict=True)
    if (not parent.is_dir() or parent.is_relative_to(Path("/tmp")) or
            parent.is_relative_to(Path("/private/tmp")) or
            parent.is_relative_to(Path("/dev/shm")) or
            parent.is_relative_to(TRUSTED_ROOT)):
        raise ValueError("temporary source requires external disk-backed storage")
    if shutil.disk_usage(parent).free < MIN_FREE:
        raise ValueError("insufficient temporary disk space")
    return parent


@contextmanager
def isolated_run(env):
    def interrupted(signum, frame):
        raise KeyboardInterrupt

    previous = signal.signal(signal.SIGTERM, interrupted)
    try:
        root = Path(tempfile.mkdtemp(prefix="ideomancer-checkout-", dir=temporary_parent(env))).resolve()
        retained = False
        cleanup_failed = False
        try:
            yield root
        except CleanupError:
            retained = True
            raise
        finally:
            if not retained:
                try:
                    shutil.rmtree(root)
                except OSError:
                    retained = True
                    cleanup_failed = True
            if retained:
                print(f"Retained owned consumer run {json.dumps(str(root))}: child shutdown could not "
                      "be verified or temporary cleanup failed. Cleanup plan: verify that no live "
                      "processes reference this run, then remove only this exact directory.", file=sys.stderr)
                # No automatic finalizer may delete data still used by live or
                # unobservable children. Receipts deliberately omit this path.
                if cleanup_failed:
                    raise CleanupError("temporary source cleanup failed")
    finally:
        signal.signal(signal.SIGTERM, previous)


def export_source(source, destination, before):
    destination.mkdir()
    for name, record in before["records"].items():
        data = checkout.read_source(source, name)
        if checkout.file_record(data) != record:
            raise ValueError("source changed before export")
        path = destination / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    if checkout.snapshot(destination, set(before["records"])) != before:
        raise ValueError("export differs from reviewed source")
    if checkout.snapshot(source, set(before["records"])) != before:
        raise ValueError("source changed during export")


def group_live(pgid, temporary, deadline):
    """Return False only for a missing group or a bounded snapshot of dead members.

    killpg(0) alone counts orphaned zombies, which this process cannot portably
    reap. Never capture a process table in an unbounded pipe or include arguments.
    """
    try:
        os.killpg(pgid, 0)
    except ProcessLookupError:
        return False
    except OSError:
        raise CleanupError("owned process group is unobservable") from None
    with tempfile.TemporaryFile(dir=temporary) as output:
        inspector = subprocess.Popen(["ps", "-A", "-o", "pid=,pgid=,stat="],
                                     stdout=output, stderr=subprocess.DEVNULL,
                                     env={**os.environ, "LC_ALL": "C"})
        try:
            limit = min(deadline, time.monotonic() + 0.5)
            while inspector.poll() is None:
                if time.monotonic() >= limit or os.fstat(output.fileno()).st_size > MAX_PROCESS_OUTPUT:
                    raise CleanupError("process inspection exceeded resource bound")
                time.sleep(0.01)
            if inspector.returncode or os.fstat(output.fileno()).st_size > MAX_PROCESS_OUTPUT:
                raise CleanupError("process inspection failed")
            output.seek(0)
            data = output.read(MAX_PROCESS_OUTPUT + 1)
        finally:
            if inspector.poll() is None:
                inspector.kill()
            inspector.wait(timeout=0.5)
    matched = False
    for line in data.splitlines():
        fields = line.split()
        if (len(fields) != 3 or not fields[0].isdigit() or not fields[1].isdigit()
                or len(fields[0]) > 20 or len(fields[1]) > 20
                or not re.fullmatch(rb"[A-Za-z+<>NsLlWw-]+", fields[2])):
            raise CleanupError("process inspection returned unknown state")
        if int(fields[1]) == pgid:
            matched = True
            if fields[2][:1] not in (b"Z", b"X"):
                return True
    if matched:
        return False
    try:
        os.killpg(pgid, 0)
    except ProcessLookupError:
        return False
    raise CleanupError("owned process group is missing from process inspection")


def finalize_group(process, temporary):
    """Stop/verify only the group created by this Popen's start_new_session=True.

    Defer catchable termination while stopping children, so another interrupt
    cannot turn an unverified shutdown into permission to remove their files.
    """
    pending = []
    previous = {}
    try:
        for signum in (signal.SIGINT, signal.SIGTERM):
            previous[signum] = signal.signal(signum, lambda number, frame: pending.append(number))
        deadline = time.monotonic() + GROUP_TIMEOUT
        stopped = False
        for signum, limit in ((signal.SIGTERM, min(deadline, time.monotonic() + TERM_GRACE)),
                              (signal.SIGKILL, deadline)):
            try:
                os.killpg(process.pid, signum)
            except ProcessLookupError:
                stopped = True
            while not stopped and time.monotonic() < limit:
                try:
                    stopped = not group_live(process.pid, temporary, limit)
                except (CleanupError, OSError, subprocess.SubprocessError):
                    # Still attempt KILL after an inconclusive TERM observation.
                    if signum == signal.SIGTERM:
                        break
                if not stopped:
                    time.sleep(min(0.05, max(0, limit - time.monotonic())))
            if stopped:
                process.wait(timeout=max(0.01, deadline - time.monotonic()))
                break
        if not stopped:
            raise CleanupError("owned process group still live or unobservable after shutdown deadline")
    except (OSError, ValueError, subprocess.SubprocessError):
        raise CleanupError("owned process group shutdown could not be verified") from None
    finally:
        for signum, handler in previous.items():
            signal.signal(signum, handler)
    if pending:
        raise KeyboardInterrupt


def run_command(command, *, cwd, env, timeout, capture=False):
    """Finalize the owned group on success, failure, timeout and interruption."""
    with tempfile.TemporaryFile(dir=env["TMPDIR"]) as output:
        process = subprocess.Popen(command, cwd=cwd, env=env, start_new_session=True,
                                   stdout=output if capture else sys.stderr, stderr=sys.stderr)
        try:
            deadline = time.monotonic() + timeout
            while process.poll() is None:
                if time.monotonic() >= deadline or (capture and os.fstat(output.fileno()).st_size > MAX_OUTPUT):
                    raise ValueError("command exceeded resource bound")
                time.sleep(0.05)
            if process.returncode:
                raise ValueError("consumer command failed")
        finally:
            finalize_group(process, env["TMPDIR"])
        if capture:
            if os.fstat(output.fileno()).st_size > MAX_OUTPUT:
                raise ValueError("command output exceeds bound")
            output.seek(0)
            return output.read(MAX_OUTPUT + 1)
        return b""


def check_graph(metadata, consumer, exported):
    packages = metadata["packages"]
    if not isinstance(packages, list) or len(packages) > 5000:
        raise ValueError("invalid dependency graph")
    expected = {"ideomancer": exported / "Cargo.toml",
                "ideomancer-package-consumer": consumer / "Cargo.toml",
                **{name: exported / "vendor/ruff/crates" / name / "Cargo.toml" for name in LOCAL}}
    local = {}
    versions = []
    for package in packages:
        name, version = package["name"], package["version"]
        if (not isinstance(name, str) or not re.fullmatch(r"[A-Za-z0-9_-]{1,128}", name)
                or not isinstance(version, str) or not re.fullmatch(r"[0-9][A-Za-z0-9.+-]{0,127}", version)):
            raise ValueError("invalid dependency identity")
        source = package.get("source")
        if source is None:
            if name not in expected or name in local:
                raise ValueError("unexpected local dependency")
            if Path(package["manifest_path"]).resolve() != expected[name]:
                raise ValueError("dependency escaped exported source")
            if name in LOCAL and version != "0.0.0":
                raise ValueError("unexpected local Ruff version")
            local[name] = version
        elif name in expected or not isinstance(source, str) or not source.startswith("registry+"):
            raise ValueError("unexpected non-registry dependency source")
        versions.append({"name": name, "version": version})
    if set(local) != set(expected):
        raise ValueError("missing exported local dependencies")
    if [p["version"] for p in packages if p["name"] == "tree-sitter-language"] != ["0.1.5"]:
        raise ValueError("dependency ABI mismatch")
    return sorted(versions, key=lambda p: (p["name"], p["version"]))


def parser():
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--source-dir", type=Path, required=True)
    result.add_argument("--target-dir", type=Path)
    result.add_argument("--offline", action=argparse.BooleanOptionalAction, default=True)
    result.add_argument("--timeout", type=float, default=600, help="seconds per command (0 < n <= 3600)")
    return result


def main(argv=None):
    args = parser().parse_args(argv)
    receipt = {"schema_version": "ideomancer.checkout-consumer.v1", "status": "failed",
               "offline": args.offline, "resolution": "fresh_consumer_lockfile",
               "target": "trusted_project_target", "identity": "source_snapshot_not_git_commit"}
    stage = "source_validation"
    try:
        if not math.isfinite(args.timeout) or not 0 < args.timeout <= 3600:
            raise ValueError("invalid timeout")
        source = Path(os.path.abspath(args.source_dir))
        before = checkout.snapshot(source)
        receipt["source_sha256"] = before["source_sha256"]
        check_vendor(source / "vendor/ruff")
        stage = "consumer_setup"
        env = os.environ.copy()
        target = validate_target(args.target_dir, env)
        with isolated_run(env) as root:
            exported, consumer = root / "engine", root / "consumer"
            export_source(source, exported, before)
            check_vendor(exported / "vendor/ruff")
            consumer.mkdir()
            write_consumer(consumer, exported)
            with (consumer / "Cargo.toml").open("a", encoding="utf-8") as manifest:
                for profile in ("dev", "test"):
                    manifest.write(f'\n[profile.{profile}]\ndebug = "line-tables-only"\nincremental = false\n')
            temporary = root / "tmp"
            temporary.mkdir()
            env.update(RUSTUP_AUTO_INSTALL="0", CARGO_TARGET_DIR=str(target),
                       CARGO_BUILD_BUILD_DIR=str(target), CARGO_BUILD_JOBS="2", TMPDIR=str(temporary))
            for key in list(env):
                if key.startswith("IDEOMANCER_") or key in ("NODE_OPTIONS", "NODE_PATH"):
                    env.pop(key)
            # Highest-precedence flags defeat user Cargo config target/build-dir.
            cargo = ["cargo", "--config", f"build.target-dir={json.dumps(str(target))}",
                     "--config", f"build.build-dir={json.dumps(str(target))}"]
            options = {"cwd": consumer, "env": env, "timeout": args.timeout}
            offline = ["--offline"] if args.offline else []
            stage = "fresh_resolution"
            run_command(cargo + ["generate-lockfile", *offline], **options)
            receipt["tree_sitter_language"] = check_resolution(consumer)
            stage = "dependency_graph"
            metadata = run_command(cargo + ["metadata", "--locked", "--format-version", "1", *offline],
                                   capture=True, **options)
            receipt["dependencies"] = check_graph(json.loads(metadata), consumer, exported)
            stage = "cargo_test"
            run_command(cargo + ["test", "--locked", *offline, "--jobs", "2", "--test", "public_api",
                                 "--", "--test-threads=2"], **options)
            stage = "source_validation_after"
            if (checkout.snapshot(source, set(before["records"])) != before or
                    checkout.snapshot(exported, set(before["records"])) != before):
                raise ValueError("source changed during consumer validation")
            check_vendor(exported / "vendor/ruff")
        receipt["status"] = "passed"
    except CleanupError:
        receipt["failed_stage"] = "temporary_cleanup"
    except (OSError, ValueError, UnicodeError, TypeError, KeyError, RecursionError, subprocess.SubprocessError):
        receipt["failed_stage"] = stage
    print(json.dumps(receipt, sort_keys=True))
    return 0 if receipt["status"] == "passed" else 1


if __name__ == "__main__":
    sys.exit(main())
