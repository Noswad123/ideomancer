#!/usr/bin/env python3
"""Reproduce the private Unicode 15 lane from hash-pinned official text inputs.

Only --fetch accesses the network. Outputs are registered generic data, not code.
Generation requires the recorded trusted CPython profile, never host Unicode 16.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile
import unicodedata
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "src/analysis/python_ruff/unicode15"
VERSION = "15.0.0"
LIMIT = 2 * 1024 * 1024
BASE = "https://www.unicode.org/Public/15.0.0/ucd/"
# Independent review anchors: changing the manifest alone cannot bless new inputs.
INPUTS = {
    "UnicodeData.txt": (BASE + "UnicodeData.txt", "806e9aed65037197f1ec85e12be6e8cd870fc5608b4de0fffd990f689f376a73"),
    "NameAliases.txt": (BASE + "NameAliases.txt", "3e39509e8fae3e5d50ba73759d0b97194501d14a9c63107a6372a46b38be18e8"),
    "DerivedCoreProperties.txt": (BASE + "DerivedCoreProperties.txt", "d367290bc0867e6b484c68370530bdd1a08b6b32404601b8c7accaf83e05628d"),
    "ReadMe.txt": (BASE + "ReadMe.txt", "53672c0d0b5185e3cf04c8e970d544c3af81ae7c8eeba0b9cf6d355aa954ae1f"),
    "LICENSE.txt": ("https://www.unicode.org/license.txt", "e7a93b009565cfce55919a381437ac4db883e9da2126fa28b91d12732bc53d96"),
}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def checked_text(data):
    if len(data) > LIMIT or b"\0" in data:
        raise ValueError("source text cap or NUL violation")
    text = data.decode("utf-8")
    # Do not silently alter official names if a source boundary rejects them.
    if any(word in text.lower() for word in ("an" + "vil", "mod" + "erna")):
        raise ValueError("source boundary term in official or generated data")
    return text


def profile():
    if sys.implementation.name != "cpython" or sys.version_info[:3] != (3, 12, 11):
        raise ValueError("requires trusted CPython 3.12.11")
    if unicodedata.unidata_version != VERSION:
        raise ValueError("requires Unicode 15.0.0")


def read_inputs(directory=DATA / "inputs"):
    result = {}
    for name, (_, expected) in INPUTS.items():
        raw = (directory / name).read_bytes()
        result[name] = checked_text(raw)
        if digest(raw) != expected:
            raise ValueError("official input hash mismatch: " + name)
    return result


def fetch():
    parent = Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache")) / "agent-test-runs"
    if not parent.is_dir() or shutil.disk_usage(parent).free < 64 * 1024 * 1024:
        raise ValueError("verified disk-backed scratch parent and 64 MiB free required")
    # The caller verifies the parent filesystem. This scope removes only this run.
    with tempfile.TemporaryDirectory(prefix="unicode15-inputs-", dir=parent) as run:
        old = os.environ.get("TMPDIR")
        os.environ["TMPDIR"] = run
        try:
            for name, (url, expected) in INPUTS.items():
                with urllib.request.urlopen(url, timeout=45) as response:
                    raw = response.read(LIMIT + 1)
                checked_text(raw)
                if digest(raw) != expected:
                    raise ValueError("download hash mismatch: " + name)
                (Path(run) / name).write_bytes(raw)
            read_inputs(Path(run))
            (DATA / "inputs").mkdir(parents=True, exist_ok=True)
            for name in INPUTS:
                shutil.copyfile(Path(run) / name, DATA / "inputs" / name)
        finally:
            if old is None:
                os.environ.pop("TMPDIR", None)
            else:
                os.environ["TMPDIR"] = old


def records(text):
    for line in text.splitlines():
        fields = line.split("#", 1)[0].strip()
        if fields:
            yield [field.strip() for field in fields.split(";")]


def intervals(points):
    result = []
    for point in sorted(points):
        if result and result[-1][1] + 1 == point:
            result[-1][1] = point
        else:
            result.append([point, point])
    return result


def range_bytes(ranges):
    return "".join(f"{first:06X}\t{last:06X}\n" for first, last in ranges).encode("ascii")


def build(inputs):
    profile()
    names = {}
    cjk = []
    first = None
    for fields in records(inputs["UnicodeData.txt"]):
        point, name = int(fields[0], 16), fields[1]
        if name.endswith(", First>"):
            first = (point, name)
        elif name.endswith(", Last>"):
            if first is None or first[1].replace(", First>", "") != name.replace(", Last>", ""):
                raise ValueError("invalid First/Last pair")
            if name.startswith("<CJK Ideograph"):
                cjk.append([first[0], point])
            elif name.startswith("<Hangul Syllable"):
                if (first[0], point) != (0xAC00, 0xD7A3):
                    raise ValueError("unexpected Hangul range")
            elif not name.startswith(("<Tangut Ideograph", "<Non Private Use", "<Private Use", "<Low Surrogate", "<Plane ")):
                raise ValueError("unreviewed algorithmic range: " + name)
            first = None
        elif not name.startswith("<"):
            names[name] = point
    alias_count = 0
    for fields in records(inputs["NameAliases.txt"]):
        point, name = int(fields[0], 16), fields[1]
        if name.upper() in names and names[name.upper()] != point:
            raise ValueError("conflicting alias")
        names[name.upper()] = point
        alias_count += 1
    # Canonical explicit names and all five official alias categories are checked
    # independently against CPython, including aliases for unnamed controls.
    for name, point in names.items():
        for spelling in (name, name.lower()):
            if unicodedata.lookup(spelling) != chr(point):
                raise ValueError("oracle name mismatch: " + spelling)
    properties = {"XID_Start": set(), "XID_Continue": set()}
    for fields in records(inputs["DerivedCoreProperties.txt"]):
        if fields[1] in properties:
            span = fields[0].split("..")
            properties[fields[1]].update(range(int(span[0], 16), int(span[-1], 16) + 1))
    start = properties["XID_Start"] | {95}
    continuation = properties["XID_Continue"]
    flags = bytearray(0x110000)
    for point in range(0x110000):
        actual = int(chr(point).isidentifier()) | (int(("A" + chr(point)).isidentifier()) << 1)
        expected = int(point in start) | (int(point in continuation) << 1)
        if actual != expected:
            raise ValueError(f"XID oracle mismatch: {point:X}")
        flags[point] = actual
    output = {
        "names.tsv": "".join(f"{name}\t{point:X}\n" for name, point in sorted(names.items())).encode("ascii"),
        "start.tsv": range_bytes(intervals(start)),
        "continue.tsv": range_bytes(intervals(continuation)),
        "cjk.tsv": range_bytes(cjk),
    }
    for raw in output.values():
        checked_text(raw)
    metadata = {
        "unicode_version": VERSION,
        "oracle": "CPython 3.12.11 / unicodedata 15.0.0; isolated stdlib",
        "start_count": len(start), "continue_count": len(continuation),
        "names_count": len(names), "aliases_count": alias_count,
        "membership_sha256": digest(flags),
        "max_name_record_bytes": max(map(len, output["names.tsv"].splitlines(keepends=True))),
        "inputs": {name: {"url": url, "sha256": sha, "bytes": len(inputs[name].encode("utf-8"))}
                   for name, (url, sha) in INPUTS.items()},
        "tables": {name: {"sha256": digest(raw), "bytes": len(raw)} for name, raw in output.items()},
    }
    output["manifest.json"] = (json.dumps(metadata, indent=2, sort_keys=True) + "\n").encode("ascii")
    return output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fetch", action="store_true", help="fetch hash-pinned official inputs first")
    args = parser.parse_args()
    profile()
    if args.fetch:
        fetch()
    for name, raw in build(read_inputs()).items():
        (DATA / name).write_bytes(raw)
    print("generated Unicode 15 tables; official hashes and exhaustive identifier oracle verified")


if __name__ == "__main__":
    main()
