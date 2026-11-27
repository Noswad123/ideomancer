#!/usr/bin/env python3
"""Read-only official-input, generated-data and trusted-oracle Unicode 15 audit."""

import json
import unicodedata

import generate_python_unicode15 as generate

# Review anchors intentionally live separately from the generator and manifest.
OFFICIAL_SHA256 = {
    "UnicodeData.txt": "806e9aed65037197f1ec85e12be6e8cd870fc5608b4de0fffd990f689f376a73",
    "NameAliases.txt": "3e39509e8fae3e5d50ba73759d0b97194501d14a9c63107a6372a46b38be18e8",
    "DerivedCoreProperties.txt": "d367290bc0867e6b484c68370530bdd1a08b6b32404601b8c7accaf83e05628d",
    "ReadMe.txt": "53672c0d0b5185e3cf04c8e970d544c3af81ae7c8eeba0b9cf6d355aa954ae1f",
    "LICENSE.txt": "e7a93b009565cfce55919a381437ac4db883e9da2126fa28b91d12732bc53d96",
}
TABLE_SHA256 = {
    "names.tsv": "e253b72cb380c601852936f118a00857c7cd10d24ce5805c573493da07667ba4",
    "start.tsv": "e4e8fcb73f908e67dee0fc98d72d3affc5d7511def2da20bf84096313b9523cb",
    "continue.tsv": "3450720d38cf19f2227e65c5fa29d9e97a11d01c850935a4e71e5bbaac56051c",
    "cjk.tsv": "5b263c76c24d0d4fe459a70a40dd28fab671524987e05f0bebc0120be028d766",
}


def lookup(name):
    try:
        value = unicodedata.lookup(name)
    except KeyError:
        return None
    return ord(value) if len(value) == 1 else None


def name_probes(inputs):
    """Independent raw official names, plus exhaustive algorithmic spellings."""
    for fields in generate.records(inputs["UnicodeData.txt"]):
        point, name = int(fields[0], 16), fields[1]
        if not name.startswith("<"):
            yield name, point
            yield name.lower(), point
    for fields in generate.records(inputs["NameAliases.txt"]):
        point, name = int(fields[0], 16), fields[1]
        yield name, point
        yield name.lower(), point
    first = None
    for fields in generate.records(inputs["UnicodeData.txt"]):
        point, name = int(fields[0], 16), fields[1]
        if name.startswith("<CJK Ideograph") and name.endswith(", First>"):
            first = point
        elif name.startswith("<CJK Ideograph") and name.endswith(", Last>"):
            for cp in range(first, point + 1):
                yield f"CJK UNIFIED IDEOGRAPH-{cp:X}", cp
            for cp in (first - 1, first, point, point + 1):
                for digits in (f"{cp:X}", f"{cp:05X}", f"{cp:06X}", f"{cp:x}"):
                    candidate = "CJK UNIFIED IDEOGRAPH-" + digits
                    yield candidate, lookup(candidate)
                    yield candidate.lower(), lookup(candidate.lower())
            first = None
    for cp in range(0xAC00, 0xD7A4):
        name = unicodedata.name(chr(cp))
        yield name, cp
        yield name.lower(), None
    for name in (
        "", "NOT A UNICODE NAME", " LATIN CAPITAL LETTER A", "LATIN CAPITAL LETTER A ",
        "LATIN  CAPITAL LETTER A", "LATIN CAPITAL LETTER A\0", "LATIN CAPITAL LETTER \u00c0",
        "KEYCAP DIGIT ONE", "LATIN CAPITAL LETTER A WITH MACRON AND GRAVE",
        "TANGUT IDEOGRAPH-17000", "TANGUT IDEOGRAPH-18D00",
        "CJK UNIFIED IDEOGRAPH-2EBF0", "CJK UNIFIED IDEOGRAPH-323B0",
        "CJK UNIFIED IDEOGRAPH-+4E00", "CJK UNIFIED IDEOGRAPH-0004E00",
        "HANGUL SYLLABLE ", "HANGUL SYLLABLE GG", "HANGUL SYLLABLE ga",
        "HANGUL SYLLABLE HIHH", "EGYPTIAN HIEROGLYPH-13460",
        # Unicode 16 additions and an alias introduced after Unicode 15 for an old cp.
        "TODHRI LETTER A", "KIRAT RAI VOWEL SIGN E", "CUNEIFORM SIGN KALAM",
    ):
        yield name, None


def verify():
    if generate.VERSION != "15.0.0":
        raise ValueError("Unicode version anchor mismatch")
    for name, expected in OFFICIAL_SHA256.items():
        if generate.digest((generate.DATA / "inputs" / name).read_bytes()) != expected:
            raise ValueError("independent official hash anchor mismatch: " + name)
    inputs = generate.read_inputs()
    expected = generate.build(inputs)
    for name, raw in expected.items():
        if (generate.DATA / name).read_bytes() != raw:
            raise ValueError("generated table/manifest mismatch: " + name)
    for name, sha in TABLE_SHA256.items():
        if generate.digest(expected[name]) != sha:
            raise ValueError("independent table anchor mismatch: " + name)
    manifest = json.loads(expected["manifest.json"])
    if (manifest["start_count"], manifest["continue_count"], manifest["names_count"],
            manifest["aliases_count"], manifest["max_name_record_bytes"]) != (136323, 139463, 35296, 473, 95):
        raise ValueError("Unicode 15 membership/count anchor mismatch")
    if manifest["membership_sha256"] != "9f40c2d1d6da73889efd15982c55dc4ede677584a07bf0aa8908c9124c7fdb72":
        raise ValueError("Unicode 15 membership hash anchor mismatch")
    paths = sorted(path for path in generate.DATA.rglob("*") if path.is_file())
    paths += [generate.DATA.with_suffix(".rs")]
    paths += [generate.ROOT / "scripts" / name for name in (
        "generate_python_unicode15.py", "check_python_unicode15.py", "test_python_unicode15.py")]
    total = 0
    for path in paths:
        raw = path.read_bytes()
        generate.checked_text(raw)
        total += len(raw)
    if total > 32 * 1024 * 1024:
        raise ValueError("aggregate source cap exceeded")
    count = 0
    for name, point in name_probes(inputs):
        if lookup(name) != point:
            raise ValueError("independent name oracle mismatch: " + repr(name))
        count += 1
    return {"unicode_version": generate.VERSION, "name_probes": count,
            "owned_files": len(paths), "owned_bytes": total,
            "manifest_sha256": generate.digest(expected["manifest.json"]),
            "files": {str(path.relative_to(generate.ROOT)): {"bytes": path.stat().st_size,
                      "sha256": generate.digest(path.read_bytes())}
                      for path in paths}}


if __name__ == "__main__":
    print(json.dumps(verify(), indent=2, sort_keys=True))
