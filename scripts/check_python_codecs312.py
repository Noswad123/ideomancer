#!/usr/bin/env python3
"""Independent bounded review anchors for the finite CPython codec profile."""

import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "src/analysis/python_ruff/codecs312"
LIMIT = 2 * 1024 * 1024
TOTAL_LIMIT = 32 * 1024 * 1024
# Independently reviewed literals, not imported from the generator or manifest.
SOURCE_ANCHOR = "1a462346c3491159b3f3863be9ac76465d1af2cf39d2ab7b96fef23d4068218f"
ANCHORS = {
    "tables.tsv": "de9d711ea15bbca0cd3ab1af19d591a275946532c3e4ac5d4c3c31bf22296c43",
    "aliases.tsv": "9c7f6617eea816ee0ec104455c667da8fa0799137b5b596475c7671d3d249e43",
    "manifest.json": "087be41ef40c830f8cacd846c9e3f31be66017fb590cd872241dc28339167298",
    "PROVENANCE.md": "c4cd696190a18ae1a79bbf078a67e26c9a0a82a874078e9fb7a40b01f9a6694e",
    "NOTICES.md": "965d49242d73453c75b19f42faae39ba6f9365f04fb1ce507a11c7812195a447",
}
# Full upstream v3.12.11 LICENSE anchor, independent of the generator/manifest.
LICENSE_SHA256 = "3b2f81fe21d181c499c59a256c8e1968455d6689d269aa85373bfb6af41da3bf"


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def validate(blobs):
    if sum(map(len, blobs.values())) > TOTAL_LIMIT:
        raise ValueError("profile aggregate cap")
    for name, raw in blobs.items():
        if len(raw) > LIMIT or b"\0" in raw:
            raise ValueError("bounded text required: " + name)
        raw.decode("utf-8")
    if LICENSE_SHA256 is None:
        raise ValueError("upstream CPython LICENSE hash verification pending")
    for name, anchor in ANCHORS.items():
        if name not in blobs or digest(blobs[name]) != anchor:
            raise ValueError("independent review anchor mismatch: " + name)
    manifest = json.loads(blobs["manifest.json"])
    source_hashes = manifest["source_sha256"]
    closure = json.dumps(source_hashes, sort_keys=True, separators=(",", ":")).encode("ascii")
    if len(source_hashes) != 123 or digest(closure) != SOURCE_ANCHOR or manifest["source_closure_sha256"] != SOURCE_ANCHOR:
        raise ValueError("Python codec source closure anchor")
    for name in ("tables.tsv", "aliases.tsv", "NOTICES.md"):
        if manifest["outputs"][name] != {"sha256": digest(blobs[name]), "bytes": len(blobs[name])}:
            raise ValueError("manifest output mismatch")
    tables = blobs["tables.tsv"].decode("ascii").splitlines()
    names = []
    identities = set()
    undefined = 0
    expansion = 0
    for index, line in enumerate(tables):
        if len(line) != 1563:
            raise ValueError("table record width")
        name, identity, points = line.split("\t")
        name = name.rstrip(" ")
        values = [int(points[i:i + 6], 16) for i in range(0, len(points), 6)]
        if len(values) != 256 or any(v > 0x10FFFF or 0xD800 <= v <= 0xDFFF for v in values):
            raise ValueError("table scalar domain")
        is_identity = values[:128] == list(range(128))
        if identity != str(int(is_identity)):
            raise ValueError("ASCII identity flag")
        if is_identity:
            identities.add(index)
        names.append(name)
        undefined += 0xFFFE in values
        expansion = max(expansion, *(len(chr(v).encode("utf-8")) for v in values if v != 0xFFFE))
    if len(tables) != 70 or len(identities) != 62 or undefined != 23 or expansion != 3 or names != sorted(set(names)):
        raise ValueError("reviewed table family counts/order")
    notices = blobs["NOTICES.md"].decode("utf-8")
    if notices.count("\n## encodings/") != 71:
        raise ValueError("source notice count")
    for name in [*names, "aliases"]:
        path = "encodings/" + name + ".py"
        if f"## {path}\n\nSource SHA-256: `{source_hashes[path]}`" not in notices:
            raise ValueError("source notice provenance: " + path)
    aliases = blobs["aliases.tsv"].decode("ascii").splitlines()
    keys = []
    origins = 0
    statuses = {}
    for line in aliases:
        if len(line) != 97:
            raise ValueError("alias record width")
        key, target, status, index, origin = line.split("\t")
        key, target = key.rstrip(" "), target.rstrip(" ")
        keys.append(key)
        origins += int(origin)
        statuses[key] = status
        if status not in "ULASTNFR" or origin not in "01":
            raise ValueError("alias classification")
        if status == "S" and (int(index) not in identities or names[int(index)] != target):
            raise ValueError("unsupported table admitted")
    if len(keys) != 445 or origins != 326 or keys != sorted(set(keys)):
        raise ValueError("alias counts/order")
    for key, status in {"utf_8": "U", "utf_8_sig": "U", "latin_1": "L", "ascii": "A", "base64_codec": "N",
                        "rot_13": "N", "undefined": "F", "mbcs": "R", "oem": "R", "cp864": "T", "utf_16": "T"}.items():
        if statuses.get(key) != status:
            raise ValueError("reviewed canonical classification: " + key)
    if digest(blobs.get("LICENSE.txt", b"")) != LICENSE_SHA256:
        raise ValueError("PSF license anchor mismatch")
    if manifest["license"]["sha256"] != LICENSE_SHA256 or manifest["license"]["bytes"] != len(blobs["LICENSE.txt"]):
        raise ValueError("manifest license mismatch")
    return {"tables": len(tables), "ascii_identity": len(identities), "stdlib_aliases": origins,
            "profile_names": len(keys), "license_verified": True}


def read_blobs(directory=DATA):
    result = {}
    total = 0
    for name in (*ANCHORS, "LICENSE.txt"):
        path = directory / name
        if name == "LICENSE.txt" and not path.exists():
            continue
        with path.open("rb") as stream:
            raw = stream.read(LIMIT + 1)
        total += len(raw)
        if len(raw) > LIMIT or total > TOTAL_LIMIT:
            raise ValueError("bounded profile text cap")
        result[name] = raw
    return result


def main():
    argparse.ArgumentParser(description=__doc__).parse_args()
    print(json.dumps(validate(read_blobs()), sort_keys=True))


if __name__ == "__main__":
    main()
