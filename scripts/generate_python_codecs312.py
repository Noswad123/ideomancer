#!/usr/bin/env python3
"""Offline CPython 3.12.11 codec profile. Run with trusted Python -I -B.

No registry discovery hooks, network access, or arbitrary sys.path inputs. Only
hash-verified installed stdlib modules are eligible for the bounded oracle.
"""

import argparse
import ast
import hashlib
import importlib
import io
import json
from pathlib import Path
import re
import sys
import sysconfig
import tokenize
import unicodedata

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "src/analysis/python_ruff/codecs312"
LIMIT = 2 * 1024 * 1024
TOTAL_LIMIT = 32 * 1024 * 1024
SOURCE_ANCHOR = "1a462346c3491159b3f3863be9ac76465d1af2cf39d2ab7b96fef23d4068218f"
# Full upstream v3.12.11 LICENSE anchor; verification must precede generation.
LICENSE_SHA256 = "3b2f81fe21d181c499c59a256c8e1968455d6689d269aa85373bfb6af41da3bf"
TABLE_WIDTH = 1564
ALIAS_WIDTH = 98


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode("ascii")


def text_bytes(path):
    with path.open("rb") as stream:
        raw = stream.read(LIMIT + 1)
    if len(raw) > LIMIT or b"\0" in raw:
        raise ValueError("bounded UTF-8 text required: " + path.name)
    raw.decode("utf-8")
    return raw


def profile():
    if (sys.implementation.name != "cpython"
            or sys.version_info[:3] != (3, 12, 11)
            or unicodedata.unidata_version != "15.0.0" or not sys.flags.isolated):
        raise ValueError("requires trusted isolated CPython 3.12.11 / Unicode 15.0.0 (-I)")


def read_sources():
    profile()
    base = Path(sysconfig.get_path("stdlib")).resolve()
    import encodings
    if (Path(encodings.__file__).resolve() != base / "encodings/__init__.py"
            or list(encodings.__path__) != [str(base / "encodings")]):
        raise ValueError("unexpected encodings package origin")
    paths = sorted((base / "encodings").glob("*.py")) + [base / "codecs.py"]
    sources = {}
    total = 0
    for path in paths:
        raw = text_bytes(path)
        total += len(raw)
        if total > TOTAL_LIMIT:
            raise ValueError("source aggregate cap")
        sources[str(path.relative_to(base))] = raw
    hashes = {key: digest(raw) for key, raw in sources.items()}
    if len(hashes) != 123 or digest(canonical(hashes)) != SOURCE_ANCHOR:
        raise ValueError("reviewed codec Python source closure mismatch")
    return base, sources, hashes


def assignment(tree, name):
    values = [n.value for n in tree.body if isinstance(n, ast.Assign)
              and any(isinstance(t, ast.Name) and t.id == name for t in n.targets)]
    if len(values) != 1:
        raise ValueError("expected one literal assignment: " + name)
    return ast.literal_eval(values[0])


def audit_table(tree):
    """Check decoder, incremental decoder and registry wiring, not byte samples."""
    expected = ast.parse("""
class Codec(codecs.Codec):
    def decode(self,input,errors='strict'):
        return codecs.charmap_decode(input,errors,decoding_table)
class IncrementalDecoder(codecs.IncrementalDecoder):
    def decode(self,input,final=False):
        return codecs.charmap_decode(input,self.errors,decoding_table)[0]
""")
    for model in expected.body:
        classes = [n for n in tree.body if isinstance(n, ast.ClassDef) and n.name == model.name]
        if len(classes) != 1 or ast.dump(classes[0].bases[0]) != ast.dump(model.bases[0]):
            raise ValueError("decoder base mismatch")
        methods = [n for n in classes[0].body if isinstance(n, ast.FunctionDef) and n.name == "decode"]
        if len(methods) != 1 or ast.dump(methods[0]) != ast.dump(model.body[0]):
            raise ValueError("decoder is not direct stateless charmap_decode")
    functions = [n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == "getregentry"]
    if len(functions) != 1 or len(functions[0].body) != 1:
        raise ValueError("registry shape")
    call = functions[0].body[0].value
    if not isinstance(call, ast.Call) or ast.unparse(call.func) != "codecs.CodecInfo":
        raise ValueError("registry is not CodecInfo")
    fields = {k.arg: ast.unparse(k.value) for k in call.keywords}
    if fields.get("decode") != "Codec().decode" or fields.get("incrementaldecoder") != "IncrementalDecoder":
        raise ValueError("registry decoder wiring")


def trusted_module(base, name):
    # Called only after the complete Python source closure passed its anchor.
    module = importlib.import_module("encodings." + name)
    if Path(module.__file__).resolve() != base / "encodings" / (name + ".py"):
        raise ValueError("codec module origin mismatch")
    return module


def byte_oracle(module, table):
    decoder = module.getregentry().decode
    for byte, point in enumerate(table):
        try:
            actual, consumed = decoder(bytes([byte]), "strict")
        except UnicodeDecodeError:
            if point != "\ufffe":
                raise ValueError("unexpected undefined byte") from None
        else:
            if point == "\ufffe" or actual != point or consumed != 1:
                raise ValueError("strict byte oracle mismatch")
    defined = bytes(i for i, point in enumerate(table) if point != "\ufffe")
    expected = "".join(table[i] for i in defined)
    if decoder(defined, "strict") != (expected, len(defined)):
        raise ValueError("whole-stream charmap mismatch")
    incremental = module.IncrementalDecoder("strict")
    if "".join(incremental.decode(bytes([b]), False) for b in defined) + incremental.decode(b"", True) != expected:
        raise ValueError("incremental oracle mismatch")


def notices(sources, names):
    """Retain complete header/string notices and non-mapping comment tokens.

    Mapping assignments contain thousands of Unicode character annotations,
    including COPYRIGHT SIGN, not copyright notices. Still audit every comment
    token there and retain any legal/provenance text rather than dropping it.
    """
    result = ["# CPython 3.12.11 Codec Source Notices\n\n",
              "Derived-data attribution excerpts from the 70 table modules and\n",
              "encodings/aliases.py. Full CPython license terms: LICENSE.txt.\n",
              "Header and standalone string expressions retain their original\n",
              "Python spelling. Comment tokens retain source line numbers.\n",
              "Only mapping-data comments without legal/provenance text are omitted.\n\n"]
    legal = re.compile(r"copyright|permission|licen[cs]e|author|written|warrant|redistribut|rights|based on|generated from|original source", re.I)
    for name in sorted(names):
        path = "encodings/" + name + ".py"
        raw = sources[path]
        text = raw.decode("utf-8")
        tree = ast.parse(raw)
        ranges = [(node.lineno, node.end_lineno) for node in ast.walk(tree)
                  if isinstance(node, ast.Assign) and any(isinstance(t, ast.Name) and t.id in
                     {"decoding_table", "encoding_table", "decoding_map", "encoding_map"} for t in node.targets)]
        strings = sorted((node for node in ast.walk(tree) if isinstance(node, ast.Expr)
                          and isinstance(node.value, ast.Constant) and isinstance(node.value.value, str)),
                         key=lambda node: (node.lineno, node.col_offset))
        comments = []
        for token in tokenize.tokenize(io.BytesIO(raw).readline):
            if token.type != tokenize.COMMENT:
                continue
            in_mapping = any(first <= token.start[0] <= last for first, last in ranges)
            character_annotation = ("COPYRIGHT SIGN" in token.string
                                    and re.fullmatch(r"#\s*(?:0x[0-9a-fA-F]+\s*->\s*)?COPYRIGHT SIGN\s*", token.string))
            if not in_mapping or (legal.search(token.string) and not character_annotation):
                comments.append(f"{token.start[0]}: {token.string}\n")
        result.append(f"## {path}\n\nSource SHA-256: `{digest(raw)}`\n\n")
        for node in strings:
            result.append(f"String notice at line {node.lineno}:\n\n```text\n{ast.get_source_segment(text, node)}\n```\n\n")
        if comments:
            result.append("Comment excerpts:\n\n```text\n" + "".join(comments) + "```\n\n")
    return ("".join(result).rstrip("\n") + "\n").encode("utf-8")


def verified_license():
    if LICENSE_SHA256 is None:
        raise ValueError("upstream CPython LICENSE hash verification pending")
    raw = text_bytes(DATA / "LICENSE.txt")
    if digest(raw) != LICENSE_SHA256:
        raise ValueError("full CPython LICENSE hash mismatch")
    return raw


def build():
    license_raw = verified_license()
    base, sources, hashes = read_sources()
    trees = {Path(k).stem: ast.parse(v) for k, v in sources.items() if k.startswith("encodings/")}
    tables = {}
    for name, tree in sorted(trees.items()):
        if not any(isinstance(n, ast.Assign) and any(isinstance(t, ast.Name) and t.id == "decoding_table" for t in n.targets) for n in tree.body):
            continue
        table = assignment(tree, "decoding_table")
        if not isinstance(table, str) or len(table) != 256:
            raise ValueError("unexpected table family")
        audit_table(tree)
        byte_oracle(trusted_module(base, name), table)
        tables[name] = table
    identities = {n for n, t in tables.items() if all(ord(t[b]) == b for b in range(128))}
    undefined = sum("\ufffe" in t for t in tables.values())
    expansion = max(len(c.encode("utf-8")) for t in tables.values() for c in t if c != "\ufffe")
    if (len(tables), len(identities), undefined, expansion) != (70, 62, 23, 3):
        raise ValueError("reviewed table counts changed")
    table_raw = "".join(f"{n:<24}\t{int(n in identities)}\t" + "".join(f"{ord(c):06X}" for c in t) + "\n" for n, t in tables.items()).encode("ascii")
    aliases = assignment(trees["aliases"], "aliases")
    if len(aliases) != 326:
        raise ValueError("alias count")
    # Only modules with an explicit registry entry are canonical codec names.
    modules = {n for n, t in trees.items() if any(isinstance(v, ast.FunctionDef) and v.name == "getregentry" for v in t.body)}
    entries = {n: n for n in modules}
    entries.update(aliases)  # Registry aliases take precedence over module names.
    rows = []
    for key, target in sorted(entries.items()):
        index = list(tables).index(target) if target in tables else 999
        status = "T"
        if target in ("mbcs", "oem"):
            status = "R"  # Platform availability is not inferred from this host.
        elif target == "undefined":
            status = "F"
        elif target in ("utf_8", "utf_8_sig"):
            # Captured source only: BOM/history admission already ran, so the
            # remaining buffer cannot begin with a UTF-8 signature.
            status = "U"
        elif target in ("latin_1", "iso8859_1"):
            status = "L"
        elif target == "ascii":
            status = "A"
        elif target in identities:
            status = "S"
        elif target not in modules:
            status = "R"
        else:
            flags = [n.value for n in ast.walk(trees[target]) if isinstance(n, ast.keyword) and n.arg == "_is_text_encoding"]
            if any(isinstance(n, ast.Constant) and n.value is False for n in flags):
                status = "N"
        if len(key) > 64 or len(target) > 24:
            raise ValueError("fixed record overflow")
        rows.append(f"{key:<64}\t{target:<24}\t{status}\t{index:03}\t{int(key in aliases)}\n")
    alias_raw = "".join(rows).encode("ascii")
    assert len(table_raw) == TABLE_WIDTH * 70
    assert len(alias_raw) == ALIAS_WIDTH * len(entries)
    outputs = {"tables.tsv": table_raw, "aliases.tsv": alias_raw,
               "NOTICES.md": notices(sources, {*tables, "aliases"})}
    manifest = {
        "profile": "cpython-3.12.11-unicode-15.0.0-single-byte-v1",
        "source": "https://github.com/python/cpython/tree/v3.12.11/Lib",
        "source_trust": "hash-pinned installed stdlib Python sources; not byte-for-byte upstream verification or native binary attestation",
        "source_closure_sha256": SOURCE_ANCHOR,
        "source_sha256": hashes,
        "counts": {"tables": 70, "ascii_identity": 62, "with_undefined": 23,
                   "max_defined_utf8_expansion": 3, "stdlib_aliases": 326, "profile_names": len(entries)},
        "records": {"tables.tsv": TABLE_WIDTH, "aliases.tsv": ALIAS_WIDTH},
        "notices": {"path": "NOTICES.md", "source_files": 71,
                    "method": "standalone string expressions and non-mapping comments; legal/provenance mapping comments retained"},
        "source_certificates": {"utf_8_sig": "UTF-8 equivalent only after captured-source BOM/history admission; not a generic codec equivalence"},
        "outputs": {n: {"sha256": digest(v), "bytes": len(v)} for n, v in outputs.items()},
        "license": {"path": "LICENSE.txt", "source": "https://raw.githubusercontent.com/python/cpython/v3.12.11/LICENSE",
                    "sha256": LICENSE_SHA256, "bytes": len(license_raw)},
    }
    outputs["manifest.json"] = (json.dumps(manifest, sort_keys=True, indent=2) + "\n").encode("ascii")
    return outputs


def write_outputs(outputs, check=False):
    if set(outputs) - {"tables.tsv", "aliases.tsv", "NOTICES.md", "manifest.json"}:
        raise ValueError("unreviewed output path")
    if sum(map(len, outputs.values())) > TOTAL_LIMIT:
        raise ValueError("output aggregate cap")
    for raw in outputs.values():
        if len(raw) > LIMIT or b"\0" in raw:
            raise ValueError("bounded output text required")
        raw.decode("utf-8")
    if not DATA.is_dir():
        raise ValueError("reviewed output directory must already exist")
    for name, raw in outputs.items():
        if check:
            if text_bytes(DATA / name) != raw:
                raise ValueError("generated output mismatch: " + name)
        else:
            (DATA / name).write_bytes(raw)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="compare without writing")
    args = parser.parse_args()
    write_outputs(build(), args.check)
    print("CPython codec profile: 70 tables, 62 ASCII identity, 326 aliases; exact byte oracle passed")


if __name__ == "__main__":
    main()
