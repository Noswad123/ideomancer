"""Project-local Cargo cache cleanup, dry-run by default (Python 3.12+).

Only Cargo deletes cache data. Guards are not an OS sandbox: concurrent hostile
changes by the same user are outside the guarantee. Stop builds before applying.
User outputs belong outside target; protected study files are never archived.
Allocated bytes estimate unique-inode blocks, not exact APFS/COW reclamation;
filesystem free-space changes cannot be attributed solely to this cleanup.
"""

import argparse
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import tomllib


PROTECTED = frozenset({
    "catalog.json", "evidence.json", "claims.json", "receipt.json",
    "acceptance-result.json",
})
BUILD_PROCESSES = frozenset({"cargo", "rustc", "rustdoc", "clippy-driver"})


class Refusal(Exception):
    pass


def _identity(info):
    return info.st_dev, info.st_ino, info.st_mode


def _directory(path):
    """Reject symlinks in every existing component, including ancestors."""
    current = Path(path.anchor)
    for part in path.parts[1:]:
        current /= part
        info = current.lstat()
        if not stat.S_ISDIR(info.st_mode):
            raise Refusal("path requires real directories without symlinks")
    return info


def _inspect(target, executable_names=None):
    result = {"entries": 0, "files": 0, "logical_bytes": 0,
              "allocated_bytes": 0, "protected_count": 0,
              "protected_samples": []}
    flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW
    seen_files = set()

    def walk(fd, relative):
        info = os.fstat(fd)
        result["allocated_bytes"] += info.st_blocks * 512
        with os.scandir(fd) as entries:
            for entry in entries:
                name = relative / entry.name
                info = entry.stat(follow_symlinks=False)
                result["entries"] += 1
                if entry.name in PROTECTED:
                    result["protected_count"] += 1
                    if len(result["protected_samples"]) < 20:
                        result["protected_samples"].append(name.as_posix())
                if stat.S_ISDIR(info.st_mode):
                    if info.st_dev != device or os.path.ismount(target / name):
                        raise Refusal("nested mount in target")
                    child = os.open(entry.name, flags, dir_fd=fd)
                    try:
                        if _identity(os.fstat(child)) != _identity(info):
                            raise Refusal("target changed during inspection")
                        walk(child, name)
                    finally:
                        os.close(child)
                elif stat.S_ISREG(info.st_mode):
                    result["files"] += 1
                    result["logical_bytes"] += info.st_size
                    inode = info.st_dev, info.st_ino
                    if inode not in seen_files:
                        seen_files.add(inode)
                        result["allocated_bytes"] += info.st_blocks * 512
                    if executable_names is not None and info.st_mode & 0o111:
                        executable_names.update((entry.name, entry.name[:15]))
                else:
                    raise Refusal("target contains a symlink or special file")

    fd = os.open(target, flags)
    try:
        device = os.fstat(fd).st_dev
        walk(fd, Path())
    finally:
        os.close(fd)
    return result


def _processes(target, executable_names=()):
    # Never request args: they may contain credentials or private inputs.
    result = subprocess.run(["ps", "-A", "-o", "pid=", "-o", "comm="],
                            capture_output=True, text=True, check=False)
    if result.returncode or not result.stdout.strip():
        raise Refusal("process check unavailable")
    count = 0
    names = set()
    for line in result.stdout.splitlines():
        fields = line.strip().split(None, 1)
        if len(fields) != 2 or not fields[0].isdigit() or not fields[1].strip():
            raise Refusal("process check returned invalid data")
        command = fields[1].strip()
        name = Path(command).name
        # Match relative spellings and truncated basenames too. Unrelated
        # executables sharing a cache filename may conservatively block cleanup.
        local_name = name in executable_names
        if (name in BUILD_PROCESSES or command == str(target)
                or command.startswith(str(target) + "/") or local_name):
            count += 1
            names.add(name if name in BUILD_PROCESSES else "target executable")
    return {"status": "blocked" if count else "clear", "blocking_count": count,
            "blocking_names": sorted(names)}


def _disk(root):
    usage = shutil.disk_usage(root)
    return {"total_bytes": usage.total, "used_bytes": usage.used,
            "free_bytes": usage.free}


def _run(root, *, apply=False):
    """Private root injection is for synthetic tests; the CLI has no override."""
    root = Path(os.path.abspath(root))
    target = root / "target"
    receipt = {
        "status": "blocked", "apply": apply, "target": "target",
        "reason": None, "manifest": "unchecked", "git": "unchecked",
        "tool": {"name": "cargo", "available": None, "invoked": False},
        "processes": {"status": "not_checked"}, "before": None, "after": None,
        "disk_before": None, "disk_after": None,
        "allocation_note": "allocated_bytes estimates unique-inode blocks, not exact "
                           "APFS/COW reclamation; disk free-space changes are not "
                           "attributable solely to this cleanup",
    }
    invoked = False
    try:
        if "{" in str(root) or "}" in str(root):
            raise Refusal("root braces unsupported: Cargo build-dir may expand templates")
        root_identity = _identity(_directory(root))
        receipt["disk_before"] = _disk(root)
        manifest_path = root / "Cargo.toml"
        if not stat.S_ISREG(manifest_path.lstat().st_mode):
            raise Refusal("manifest must be a regular file, not a symlink")
        with manifest_path.open("rb") as stream:
            manifest = tomllib.load(stream)
        if (manifest.get("package", {}).get("name") != "ideomancer"
                or manifest.get("lib", {}).get("name") != "ideomancer_core"):
            raise Refusal("manifest identity mismatch")
        receipt["manifest"] = "ideomancer/ideomancer_core"
        git = subprocess.run(["git", "-C", str(root), "rev-parse", "--show-toplevel"],
                             capture_output=True, text=True, check=False)
        if git.returncode or git.stdout.strip() != str(root):
            raise Refusal("git root does not match script repository")
        receipt["git"] = "matched"
        for variable in ("CARGO_TARGET_DIR", "CARGO_BUILD_TARGET_DIR", "CARGO_BUILD_BUILD_DIR"):
            value = os.environ.get(variable)
            if value is not None:
                candidate = Path(value)
                if not candidate.is_absolute():
                    candidate = root / candidate
                if not value or os.path.abspath(candidate) != str(target):
                    raise Refusal(variable + " must be unset or the local target")
                _directory(candidate.parent)
        receipt["tool"]["available"] = shutil.which("cargo") is not None
        try:
            target_identity = _identity(_directory(target))
        except FileNotFoundError:
            receipt["status"] = "already_clean"
            return receipt
        if os.path.ismount(target):
            raise Refusal("target is a mount point")
        executable_names = set()
        receipt["before"] = _inspect(target, executable_names)
        if receipt["before"]["protected_count"]:
            raise Refusal("preserve-required: move user outputs outside target before cleanup")
        receipt["processes"] = {"status": "unavailable"}
        receipt["processes"] = _processes(target, executable_names)
        if receipt["processes"]["blocking_count"]:
            raise Refusal("active build process or target executable")
        if not receipt["tool"]["available"]:
            raise Refusal("cargo unavailable")
        if not apply:
            receipt["status"] = "dry_run"
            return receipt

        # Full-directory cargo clean provides no build-lock guarantee. Writers
        # must remain stopped; these checks are coordination, not mutual exclusion.
        receipt["processes"] = {"status": "unavailable"}
        receipt["processes"] = _processes(target, executable_names)
        if receipt["processes"]["blocking_count"]:
            raise Refusal("active build process or target executable")
        if (_identity(_directory(root)) != root_identity
                or _identity(_directory(target)) != target_identity
                or os.path.ismount(target)):
            raise Refusal("root or target identity changed before cleanup")
        # CLI config overrides project, ancestor, global and environment config.
        # Newer Cargo cleans build-dir independently of --target-dir.
        build_dir = "build.build-dir=" + json.dumps(str(target), ensure_ascii=False)
        command = ["cargo", "--config", build_dir, "clean", "--manifest-path", str(manifest_path),
                   "--target-dir", str(target)]
        receipt["tool"]["invoked"] = invoked = True
        result = subprocess.run(command, cwd=root, capture_output=True,
                                text=True, check=False)
        receipt["tool"]["returncode"] = result.returncode
        if result.returncode:
            raise Refusal("cargo clean failed")
        if _identity(_directory(root)) != root_identity:
            raise Refusal("root identity changed after cleanup")
        try:
            _directory(target)
        except FileNotFoundError:
            receipt["after"] = {"absent": True}
        else:
            if os.path.ismount(target):
                raise Refusal("target is a mount point after cleanup")
            receipt["after"] = _inspect(target)
            if receipt["after"]["entries"]:
                raise Refusal("cargo clean left artifacts in target")
        receipt["status"] = "cleaned"
    except (Refusal, OSError, ValueError, AttributeError, RecursionError) as error:
        receipt["status"] = "failed" if invoked else "blocked"
        # Do not echo subprocess output, environment values, or filesystem errors.
        receipt["reason"] = str(error) if isinstance(error, Refusal) else "safety check unavailable or invalid"
    finally:
        if receipt["disk_before"] is not None:
            try:
                receipt["disk_after"] = _disk(root)
            except OSError:
                receipt["status"] = "failed" if invoked else "blocked"
                receipt["reason"] = "disk measurement unavailable"
    return receipt


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--apply", action="store_true", help="run guarded cargo clean")
    args = parser.parse_args(argv)
    # abspath preserves symlink components so validation can reject them.
    root = Path(os.path.abspath(__file__)).parent.parent
    receipt = _run(root, apply=args.apply)
    print(json.dumps(receipt, sort_keys=True))
    return 1 if receipt["status"] in {"blocked", "failed"} else 0


if __name__ == "__main__":
    raise SystemExit(main())
