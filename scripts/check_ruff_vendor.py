#!/usr/bin/env python3
"""Read-only checks for the reviewed Ruff snapshot; never invokes Cargo or Git.

The committed checker constants and inventory are the review trust root. This
detects snapshot drift, not a malicious party able to rewrite the checker too.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import sys
import tomllib


COMMIT = "87f0feb21a1af67195bebf94d811770393f9f447"
ARCHIVE_SHA256 = "e4613ad9fe6f7293510586649aab788e10610ab80fe9dd509ba7ac2364413631"
LOCK_SHA256 = "ccc843838d16f203d18a3410dd120e72e750a89ee42164145ae9cbf181fb6f47"
PROVENANCE_SHA256 = "50bd49e58b189782d00921ed6b2e6fb2229fe244cabfb6de3901cf08105205c4"
PRISTINE_SHA256 = "7e25402b858763700567914d71410161c80128b29605de3cdb29c8f03eafe44b"
MAX_FILE_BYTES = 2 * 1024 * 1024
MAX_TOTAL_BYTES = 4 * 1024 * 1024
SHA256 = re.compile(r"[0-9a-f]{64}\Z")
POLICY_FILES = (
    "crates/ruff_python_parser/src/parser/options.rs",
    "crates/ruff_python_parser/src/parser/expression.rs",
    "crates/ruff_python_parser/src/parser/statement.rs",
    "crates/ruff_python_parser/src/parser/pattern.rs",
)
LOCAL = {
    "ruff_python_parser": ("ruff_python_ast", "ruff_python_trivia", "ruff_text_size"),
    "ruff_python_ast": ("ruff_python_trivia", "ruff_source_file", "ruff_text_size"),
    "ruff_python_trivia": ("ruff_text_size", "ruff_source_file"),
    "ruff_source_file": ("ruff_text_size",),
    "ruff_text_size": (),
}
REGISTRY = {
    "ruff_python_parser": {
        "bitflags": "2.5.0", "bstr": "=1.12.0", "compact_str": "=0.9.0",
        "memchr": "2.7.1", "rustc-hash": "=2.1.1", "static_assertions": "=1.1.0",
        "unicode-ident": "1.0.12", "unicode_names2": "=1.3.0",
        "unicode-normalization": "=0.1.24",
    },
    "ruff_python_ast": {
        "aho-corasick": "1.1.3", "bitflags": "2.5.0", "compact_str": "=0.9.0",
        "is-macro": "=0.3.7", "itertools": "=0.14.0", "memchr": "2.7.1",
        "rustc-hash": "=2.1.1", "thiserror": "2.0.0",
    },
    "ruff_python_trivia": {"itertools": "=0.14.0", "unicode-ident": "1.0.12"},
    "ruff_source_file": {"memchr": "2.7.1"},
    "ruff_text_size": {},
}
EXCLUDED_FEATURES = {
    "ruff_python_ast": ('cfg(feature, values("cache", "salsa", "schemars", "serde"))',),
    "ruff_source_file": ('cfg(feature, values("serde"))',),
    "ruff_text_size": ('cfg(feature, values("schemars", "serde"))',),
}


class VendorError(ValueError):
    """The reviewed snapshot boundary was violated."""


def require(condition, message):
    if not condition:
        raise VendorError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, f"duplicate JSON key: {key}")
        result[key] = value
    return result


def safe_path(name):
    require(isinstance(name, str), "inventory path must be a string")
    p = PurePosixPath(name)
    require(
        bool(name) and not p.is_absolute() and p.as_posix() == name
        and all(part not in (".", "..") for part in p.parts)
        and "\\" not in name and ":" not in name
        and not any(ord(c) < 32 or ord(c) == 127 for c in name),
        f"unsafe inventory path: {name!r}",
    )
    return p


def validate_records(records):
    require(isinstance(records, dict) and 0 < len(records) <= 100, "invalid inventory size")
    total = 0
    for name, item in records.items():
        safe_path(name)
        require(isinstance(item, dict) and set(item) == {"bytes", "sha256"}, f"invalid record: {name}")
        require(type(item["bytes"]) is int and 0 <= item["bytes"] <= MAX_FILE_BYTES, f"invalid size: {name}")
        require(isinstance(item["sha256"], str) and SHA256.fullmatch(item["sha256"]), f"invalid hash: {name}")
        total += item["bytes"]
    require(total <= MAX_TOTAL_BYTES, "inventory exceeds byte bound")


def parse_provenance(data):
    require(len(data) <= 128 * 1024, "provenance exceeds byte bound")
    value = json.loads(data, object_pairs_hook=unique_object)
    require(isinstance(value, dict) and set(value) == {
        "schema_version", "upstream", "pristine_sources", "policy", "files",
    }, "unknown or missing provenance fields")
    require(type(value["schema_version"]) is int and value["schema_version"] == 1, "unknown schema version")
    require(value["upstream"] == {
        "repository": "https://github.com/astral-sh/ruff", "tag": "0.12.0",
        "commit": COMMIT,
        "archive_url": f"https://codeload.github.com/astral-sh/ruff/tar.gz/{COMMIT}",
        "archive_sha256": ARCHIVE_SHA256, "lock_sha256": LOCK_SHA256,
    }, "upstream pin/retrieval metadata changed")
    validate_records(value["pristine_sources"])
    validate_records(value["files"])
    pristine = value["pristine_sources"]
    require(len(pristine) == 65 and all(
        len(PurePosixPath(p).parts) >= 4 and PurePosixPath(p).parts[0] == "crates"
        and PurePosixPath(p).parts[1] in LOCAL and PurePosixPath(p).parts[2] == "src"
        and p.endswith(".rs") for p in pristine
    ), "unexpected pristine source membership")
    canonical = json.dumps(pristine, sort_keys=True, separators=(",", ":")).encode()
    require(digest(canonical) == PRISTINE_SHA256, "pristine source reference changed")
    require(isinstance(value["policy"], dict) and set(value["policy"]) == set(POLICY_FILES), "unexpected policy files")
    for name in POLICY_FILES:
        require(name in value["files"], f"missing policy inventory: {name}")
        require(value["policy"][name] == {
            "original_sha256": pristine[name]["sha256"],
            "patched_sha256": value["files"][name]["sha256"],
        }, f"policy hash mapping changed: {name}")
    # Pin the whole reviewed inventory, not hashes supplied by a mutable sidecar.
    require(digest(data) == PROVENANCE_SHA256, "reviewed provenance digest changed")
    return value


def validate_inventory(root, records):
    """Reject extra/missing files AND directories, links and special entries."""
    validate_records(records)
    require(not root.is_symlink() and root.is_dir(), "vendor root is not a real directory")
    directories = set()
    for name in records:
        directories.update(p.as_posix() for p in PurePosixPath(name).parents if p.as_posix() != ".")
    actual = set()
    pending = [root]
    while pending:
        directory = pending.pop()
        with os.scandir(directory) as entries:
            for entry in entries:
                path = Path(entry.path)
                name = path.relative_to(root).as_posix()
                mode = entry.stat(follow_symlinks=False).st_mode
                require(not stat.S_ISLNK(mode), f"symlink is forbidden: {name}")
                if stat.S_ISDIR(mode):
                    require(name in directories, f"extra directory: {name}")
                    pending.append(path)
                else:
                    require(stat.S_ISREG(mode), f"special file is forbidden: {name}")
                    require(name in records, f"extra file: {name}")
                    expected = records[name]
                    require(entry.stat(follow_symlinks=False).st_size == expected["bytes"], f"size mismatch: {name}")
                    data = path.read_bytes()
                    require(len(data) == expected["bytes"] and digest(data) == expected["sha256"], f"hash mismatch: {name}")
                    actual.add(name)
    require(actual == set(records), f"missing files: {sorted(set(records) - actual)}")


def validate_manifest(data, name, crate, root):
    """Exact reviewed manifest semantics; no workspace/feature/Git escape hatch."""
    require(name in LOCAL, f"unknown local crate: {name}")
    manifest = tomllib.loads(data.decode("utf-8"))
    authors = ["Charlie Marsh <charlie.r.marsh@gmail.com>"]
    if name == "ruff_python_parser":
        authors.append("RustPython Team")
    expected = {
        "package": {
            "name": name, "version": "0.0.0", "publish": False, "authors": authors,
            "edition": "2024", "rust-version": "1.85",
            "homepage": "https://docs.astral.sh/ruff",
            "documentation": "https://docs.astral.sh/ruff",
            "repository": "https://github.com/astral-sh/ruff", "license": "MIT",
            "license-file": "../../LICENSE", "autobins": False, "autoexamples": False,
            "autotests": False, "autobenches": False,
        },
        "lib": {"test": False, "doctest": False, "bench": False},
        "dependencies": {**REGISTRY[name], **{dep: {"path": f"../{dep}"} for dep in LOCAL[name]}},
        "lints": {"rust": {"unexpected_cfgs": {"level": "warn", "check-cfg": [
            "cfg(fuzzing)", "cfg(codspeed)", *EXCLUDED_FEATURES.get(name, ()),
        ]}}},
    }
    require(manifest == expected, f"manifest differs from reviewed dependency/target/feature policy: {name}")
    for dependency in LOCAL[name]:
        path = (crate / manifest["dependencies"][dependency]["path"]).resolve()
        require(path == (root / "crates" / dependency).resolve() and path.is_relative_to(root.resolve()),
                f"local dependency escapes vendor: {name} -> {dependency}")
    require((crate / manifest["package"]["license-file"]).resolve() == (root / "LICENSE").resolve(),
            f"license path escapes vendor: {name}")


def reverse_policy_patch(patch, sources):
    """Reverse exact unified hunks without fuzz or external patch execution."""
    lines = patch.splitlines(keepends=True)
    i = 0
    restored = {}
    while i < len(lines):
        require(lines[i].startswith("--- a/"), "invalid policy patch header")
        name = lines[i][6:].rstrip("\n")
        safe_path(name)
        require(name in sources and name not in restored, "unexpected policy patch path")
        require(i + 1 < len(lines) and lines[i + 1] == f"+++ b/{name}\n", "mismatched policy patch path")
        i += 2
        current = sources[name].decode("utf-8").splitlines(keepends=True)
        output = []
        cursor = 0
        hunks = 0
        while i < len(lines) and lines[i].startswith("@@ "):
            match = re.fullmatch(r"@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@\n", lines[i])
            require(match is not None, "invalid policy hunk")
            old_start, old_count, new_start, new_count = (
                int(value) if value is not None else 1 for value in match.groups()
            )
            # Empty unified ranges identify the preceding line, not the first line.
            start = new_start - (new_count != 0)
            require(cursor <= start <= len(current), "overlapping/out-of-range policy hunk")
            output.extend(current[cursor:start])
            cursor = start
            require(old_start == len(output) + (old_count != 0), "incorrect original policy hunk offset")
            i += 1
            old_used = new_used = 0
            while i < len(lines) and not lines[i].startswith(("@@ ", "--- a/")):
                line = lines[i]
                require(line[:1] in (" ", "+", "-"), "invalid policy hunk line")
                content = line[1:]
                if line[0] in (" ", "+"):
                    require(cursor < len(current) and current[cursor] == content, "policy hunk content mismatch")
                    cursor += 1
                    new_used += 1
                if line[0] in (" ", "-"):
                    output.append(content)
                    old_used += 1
                i += 1
            require((old_used, new_used) == (old_count, new_count), "policy hunk count mismatch")
            hunks += 1
        require(hunks > 0, "policy patch has no hunks")
        output.extend(current[cursor:])
        restored[name] = "".join(output).encode()
    require(set(restored) == set(sources), "missing policy patch file")
    return restored


def apply_policy(options, expression, statement, pattern):
    """Model only the reviewed edits on the four pristine named sources."""
    def replace_once(source, old, new):
        require(source.count(old) == 1, "expected policy edit locus changed")
        return source.replace(old, new, 1)

    options = replace_once(options, b"    pub(crate) target_version: PythonVersion,\n", b"""    pub(crate) target_version: PythonVersion,
    /// Omit only duplicate keyword argument validation; retain all syntax diagnostics.
    pub(crate) ast_only: bool,
""")
    options = replace_once(options, b"impl ParseOptions {\n", b"""impl ParseOptions {
    /// Omit duplicate call and class-header keyword validation when enabled.
    ///
    /// Defaults to false. This is not a general AST-versus-compiler semantic mode.
    /// Both ordinary and unsupported-version errors must still be checked before AST use.
    #[must_use]
    pub fn with_ast_only(mut self, ast_only: bool) -> Self {
        self.ast_only = ast_only;
        self
    }

""")
    require(options.count(b"            target_version: PythonVersion::default(),\n") == 2,
            "expected policy constructor loci changed")
    options = options.replace(b"            target_version: PythonVersion::default(),\n",
                              b"            target_version: PythonVersion::default(),\n            ast_only: false,\n")
    expression = replace_once(expression, b"if !all_arg_names.insert(arg_name) {",
                              b"if !self.options.ast_only && !all_arg_names.insert(arg_name) {")
    for token, fixture in ((b"Star", b"type_param_type_var_tuple_bound"),
                           (b"DoubleStar", b"type_param_param_spec_bound")):
        locus = b"if self.eat(TokenKind::" + token + b") {\n            let name = self.parse_identifier();\n\n"
        statement = replace_once(statement, locus, locus + b"            let equal_token_start = self.node_start();\n")
        locus = b"            // test_err " + fixture + b"\n"
        statement = replace_once(statement, locus, b"""            if default.is_some() {
                self.add_unsupported_syntax_error(
                    UnsupportedSyntaxErrorKind::TypeParamDefault,
                    self.node_range(equal_token_start),
                );
            }

""" + locus)
    pattern = replace_once(pattern,
                           b"        self.at_ts(MAPPING_PATTERN_START_SET) || self.at_soft_keyword()\n",
                           b"""        self.at_ts(MAPPING_PATTERN_START_SET)
            || self.at_soft_keyword()
            || (self.options.ast_only && self.at(TokenKind::FStringStart))
""")
    return options, expression, statement, pattern


def validate_policy(sources, originals):
    require(set(sources) == set(originals) == set(POLICY_FILES), "unexpected policy source files")
    expected = apply_policy(*(originals[name] for name in POLICY_FILES))
    require(tuple(sources[name] for name in POLICY_FILES) == expected, "source differs from exact reviewed policy edits")
    options, expression = (sources[name] for name in POLICY_FILES[:2])
    require(options.count(b"ast_only: false,") == 2, "both policy constructors must default false")
    require(b"pub(crate) ast_only: bool," in options, "missing narrow policy field")
    require(b"pub fn with_ast_only(mut self, ast_only: bool) -> Self" in options, "missing explicit policy builder")
    require(b"self.ast_only = ast_only;" in options, "policy builder does not select policy")
    require(expression.count(b"if !self.options.ast_only && !all_arg_names.insert(arg_name) {") == 1,
            "duplicate-keyword diagnostic must be guarded before emission")


def check_vendor(root):
    root = Path(root)
    require(not root.is_symlink() and not root.parent.is_symlink(), "symlink vendor boundary")
    path = root / "provenance.json"
    require(stat.S_ISREG(path.lstat().st_mode), "provenance is not a regular file")
    require(path.stat().st_size <= 128 * 1024, "provenance exceeds byte bound")
    data = path.read_bytes()
    provenance = parse_provenance(data)
    files = {**provenance["files"], "provenance.json": {"bytes": len(data), "sha256": digest(data)}}
    validate_inventory(root, files)
    pristine = provenance["pristine_sources"]
    require({name for name in files if "/src/" in name} == set(pristine), "source set changed")
    patched = {name: (root / name).read_bytes() for name in POLICY_FILES}
    originals = reverse_policy_patch((root / "patches/ast-only-policy.patch").read_text(), patched)
    for name, expected in pristine.items():
        content = originals[name] if name in originals else (root / name).read_bytes()
        require(len(content) == expected["bytes"] and digest(content) == expected["sha256"], f"pristine source mismatch: {name}")
    validate_policy(patched, originals)
    for name in LOCAL:
        crate = root / "crates" / name
        validate_manifest((crate / "Cargo.toml").read_bytes(), name, crate, root)
    require(digest((root / "upstream/Cargo.lock").read_bytes()) == LOCK_SHA256, "upstream lock changed")
    return {"files": len(files), "bytes": sum(item["bytes"] for item in files.values()), "sources": len(pristine), "commit": COMMIT}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--vendor-root", type=Path, default=Path(__file__).resolve().parents[1] / "vendor" / "ruff")
    args = parser.parse_args(argv)
    try:
        result = check_vendor(args.vendor_root)
    except (OSError, ValueError, UnicodeError, TypeError, KeyError) as error:
        print(f"Ruff vendor verification failed: {error}", file=sys.stderr)
        return 1
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
