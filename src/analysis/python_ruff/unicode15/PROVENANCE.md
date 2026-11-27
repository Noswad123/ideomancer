# Unicode 15 Compatibility Data

The five unmodified UTF-8 inputs in `inputs/` are Unicode Consortium data and
notices. `manifest.json` records their exact URLs, SHA-256 hashes and byte lengths.
The versioned UCD ReadMe identifies Unicode 15.0.0 and its 2022 copyright notice.
`inputs/LICENSE.txt` is the actual available Unicode License V3 permission notice
retrieved from https://www.unicode.org/license.txt on 2026-11-27. It is preserved
byte-for-byte, including its 1991-2026 copyright. The URL is mutable; its checked-in
hash is authoritative for reproduction, not a claim that the URL is immutable.
Do not silently refresh this notice or any pinned input. Unicode data and derived
tables are distributed with this permission notice, not relicensed as product code.

Reproduce with the installed trusted CPython 3.12.11 / Unicode 15.0.0:

```sh
python3.12 -B scripts/generate_python_unicode15.py --fetch
python3.12 -B scripts/check_python_unicode15.py
```

Omit `--fetch` to reproduce entirely offline. The generator's separate hard-coded
official input hashes prevent a rewritten manifest from blessing altered inputs.
Generation verifies every scalar and surrogate's raw Python identifier membership
and every explicit canonical name and alias against the isolated stdlib profile.
It does not use compiler Unicode properties, Unicode 16 lookup, or an age filter.

`start.tsv`, `continue.tsv`, and `cjk.tsv` are sorted disjoint intervals, fixed
14-byte records: six uppercase hex digits, TAB, six digits, LF. Start includes
Python's underscore exception. `names.tsv` is byte-sorted uppercase explicit
canonical names and every official alias, TAB, hex code point, LF. Hangul and CJK
unified names are implemented algorithmically. Other explicit algorithm-shaped
names (including Nushu, Khitan and CJK compatibility names) stay in the name table.
Tangut First/Last ranges have no CPython 3.12 Unicode name lookup entry.

CPython's CJK unified and Hangul algorithmic spellings are case-sensitive. CJK
accepts four or five uppercase hexadecimal digits, including a leading zero when
the total length is five. Explicit names and aliases accept ASCII case folding.
Named sequences are deliberately absent: the API returns exactly one Python code
point, including NUL controls. It does not enumerate `unicodedata.name` as a
substitute for the official aliases file.

This lane admits raw Unicode 15 XID identifiers only. Whole-string NFKC remains
Ruff's lexer responsibility after admission; per-character mappings are not a
replacement for composition across sequences. The ignored Rust oracle additionally
checks admitted identifier normalization using the actual Ruff lexer against
CPython, including combining sequences. This is not a full frontend parity claim.

The static identity input list includes behavior code, scripts, raw inputs, tables,
manifest and this notice. The integrating caller must incorporate every entry in
its implementation fingerprint. Source inventory registration belongs to that
caller. No target application or private source is used to prepare these tables.
