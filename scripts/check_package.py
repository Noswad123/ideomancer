"""Check Cargo's actual file list against a reviewed inventory; never publish."""

from pathlib import Path, PurePosixPath
import subprocess
import sys


GENERATED_OPTIONAL = frozenset({".cargo_vcs_info.json"})
ALLOWED_HIDDEN_PATHS = frozenset({".cargo_vcs_info.json", ".gitignore", ".github/workflows/rust.yml"})
FORBIDDEN_COMPONENTS = frozenset({"node_modules", "target"})


class PolicyError(ValueError):
    pass


def checked_path(name):
    path = PurePosixPath(name)
    if (
        not name
        or not path.parts
        or len(name) > 512
        or not name.isascii()
        or any(ord(char) < 32 or ord(char) == 127 for char in name)
        or "\\" in name
        or path.is_absolute()
        or path.as_posix() != name
        or any(part in {".", ".."} for part in path.parts)
    ):
        raise PolicyError("invalid package path")
    if (
        FORBIDDEN_COMPONENTS.intersection(path.parts)
        or (any(part.startswith(".") for part in path.parts) and name not in ALLOWED_HIDDEN_PATHS)
        or name == "openapi.json"
    ):
        raise PolicyError(f"private/runtime path is forbidden: {name}")
    return name


def inventory(text):
    names = []
    for line in text.splitlines():
        if not line.strip() or line.startswith("#"):
            continue
        names.append(checked_path(line))
    if len(names) != len(set(names)):
        raise PolicyError("duplicate inventory entry")
    if not names:
        raise PolicyError("empty package inventory")
    if GENERATED_OPTIONAL.intersection(names):
        raise PolicyError("optional Cargo-generated metadata is not inventory source")
    return set(names)


def verify(expected, listed):
    actual = [checked_path(name) for name in listed]
    if len(actual) != len(set(actual)):
        raise PolicyError("duplicate Cargo package entry")
    actual = set(actual)
    unexpected = actual - expected - GENERATED_OPTIONAL
    missing = expected - actual
    if unexpected or missing:
        problems = []
        if unexpected:
            problems.append("unreviewed package paths: " + ", ".join(sorted(unexpected)))
        if missing:
            problems.append("missing reviewed paths: " + ", ".join(sorted(missing)))
        raise PolicyError("; ".join(problems))
    return len(actual)


def main():
    root = Path(__file__).resolve().parent.parent
    try:
        expected = inventory((root / "docs/package-files.txt").read_text(encoding="utf-8"))
        result = subprocess.run(
            ["cargo", "package", "--list", "--locked", "--offline", "--allow-dirty"],
            cwd=root,
            text=True,
            stdout=subprocess.PIPE,
            check=True,
        )
        count = verify(expected, result.stdout.splitlines())
    except (OSError, UnicodeError, PolicyError, subprocess.CalledProcessError) as error:
        print(f"Package policy failed: {error}", file=sys.stderr)
        return 1
    print(f"Verified {count} package paths against the reviewed inventory (no publication).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
