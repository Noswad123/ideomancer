"""Bounded, read-only Git-source closure checks; no Git or Cargo invocation.

The reviewer's inventory is the trust root, not candidate-supplied policy. Only
selected source roots are inspected. This is a concurrent-change guard, not a
sandbox or an authenticity/commit attestation.
"""

import argparse
from contextlib import contextmanager, ExitStack
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import stat
import sys

from check_package import PolicyError, checked_path, inventory
from check_package_archive import checked_file, identity


TRUSTED_ROOT = Path(__file__).resolve().parents[1]
MAX_FILE = 2 * 1024 * 1024
MAX_TOTAL = 32 * 1024 * 1024
MAX_FILES = 5000
MAX_ENTRIES = 10000
FIXED = frozenset({"Cargo.toml", "Cargo.lock", "LICENSE", "README.md", "ROADMAP.md",
                   "makefile", ".gitignore", ".github/workflows/rust.yml"})
RECURSIVE = ("src", "examples", "vendor/ruff")
FORBIDDEN = frozenset({".git", ".ideomancer", ".agents", "privatefixtures", "target",
                       "node_modules", "__pycache__"})


def source_path(name):
    try:
        checked_path(name)
    except PolicyError:
        raise PolicyError("invalid source path") from None
    parts = PurePosixPath(name).parts
    if FORBIDDEN.intersection(parts) or ":" in name:
        raise PolicyError("forbidden source path")
    selected = (name in FIXED or any(name.startswith(p + "/") for p in RECURSIVE)
                or (len(parts) == 2 and parts[0] == "docs" and
                    (name.endswith((".md", ".json")) or name in
                     {"docs/package-files.txt", "docs/source-files.txt"}))
                or (len(parts) == 2 and parts[0] == "scripts" and name.endswith(".py"))
                or (len(parts) == 2 and parts[0] == "tests" and name.endswith((".rs", ".cjs")))
                or (len(parts) == 3 and parts[0] == "skills" and parts[2] == "SKILL.md"))
    if not selected:
        raise PolicyError("path is outside selected source closure")
    return name


def read_source(root, name):
    source_path(name)
    with checked_file(root, name) as stream:
        if os.fstat(stream.fileno()).st_size > MAX_FILE:
            raise PolicyError("source file exceeds byte bound")
        data = stream.read(MAX_FILE + 1)
        if len(data) > MAX_FILE:
            raise PolicyError("source file exceeds byte bound")
    if b"\0" in data:
        raise PolicyError("source closure requires text files")
    data.decode("utf-8")
    return data


def reviewed_paths():
    names = inventory(read_source(TRUSTED_ROOT, "docs/source-files.txt").decode("utf-8"))
    if len(names) > MAX_FILES:
        raise PolicyError("source inventory exceeds count bound")
    return {source_path(name) for name in names}


@contextmanager
def directory_fd(root, name=""):
    with ExitStack() as stack:
        fd = os.open("/", os.O_RDONLY | os.O_DIRECTORY)
        stack.callback(os.close, fd)
        for part in Path(os.path.abspath(root)).parts[1:] + tuple(name.split("/") if name else ()):
            fd = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=fd)
            stack.callback(os.close, fd)
        yield fd


def selected_files(root):
    """Never descend into unselected trees, even if they are links or enormous."""
    found = set(FIXED)
    entries_seen = 0

    def scan(fd, prefix, recursive=False):
        nonlocal entries_seen
        before = identity(os.fstat(fd))
        with os.scandir(fd) as entries:
            for entry in entries:
                entries_seen += 1
                if entries_seen > MAX_ENTRIES:
                    raise PolicyError("source tree exceeds entry bound")
                name = prefix + "/" + entry.name
                if recursive:
                    source_path(name)
                elif prefix == "skills":
                    # Only skills/*/SKILL.md is selected, not other skill data.
                    checked = "skills/" + entry.name + "/SKILL.md"
                    source_path(checked)
                    with directory_fd(root, name) as child:
                        scan(child, name)
                    continue
                else:
                    try:
                        source_path(name)
                    except PolicyError:
                        continue
                info = entry.stat(follow_symlinks=False)
                if recursive and stat.S_ISDIR(info.st_mode):
                    with directory_fd(root, name) as child:
                        if identity(info) != identity(os.fstat(child)):
                            raise PolicyError("source directory changed")
                        scan(child, name, True)
                    if identity(info) != identity(entry.stat(follow_symlinks=False)):
                        raise PolicyError("source directory changed")
                elif not stat.S_ISREG(info.st_mode):
                    raise PolicyError("source entry must be a regular file")
                else:
                    found.add(name)
                    if len(found) > MAX_FILES:
                        raise PolicyError("source tree exceeds file bound")
        if before != identity(os.fstat(fd)):
            raise PolicyError("source directory changed")

    for name in (*RECURSIVE, "docs", "scripts", "tests", "skills"):
        # An absent root is diagnosed by the exact inventory comparison below.
        try:
            with directory_fd(root, name) as fd:
                scan(fd, name, name in RECURSIVE)
        except FileNotFoundError:
            continue
    return found


def file_record(data):
    return {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}


def snapshot(root, expected=None):
    names = reviewed_paths() if expected is None else {source_path(n) for n in expected}
    if not names or len(names) > MAX_FILES:
        raise PolicyError("invalid source inventory size")
    actual = selected_files(root)
    if actual != names:
        raise PolicyError("source closure differs from reviewed inventory")
    records = {}
    total = 0
    for name in sorted(names):
        data = read_source(root, name)
        total += len(data)
        if total > MAX_TOTAL:
            raise PolicyError("source closure exceeds total byte bound")
        records[name] = file_record(data)
    if selected_files(root) != names:
        raise PolicyError("source closure changed during verification")
    canonical = json.dumps(records, sort_keys=True, separators=(",", ":")).encode("ascii")
    return {"source_sha256": hashlib.sha256(canonical).hexdigest(),
            "files": len(records), "bytes": total, "records": records}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-dir", type=Path, default=TRUSTED_ROOT)
    args = parser.parse_args(argv)
    result = {"schema_version": "ideomancer.checkout.v1", "status": "failed"}
    try:
        result.update({k: v for k, v in snapshot(args.source_dir).items() if k != "records"})
        result["status"] = "passed"
    except (OSError, ValueError, UnicodeError, RecursionError):
        result["error"] = "source boundary validation failed"
    print(json.dumps(result, sort_keys=True))
    return 0 if result["status"] == "passed" else 1


if __name__ == "__main__":
    sys.exit(main())
