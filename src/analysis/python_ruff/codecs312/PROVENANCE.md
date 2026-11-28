# Pinned CPython Codec Profile

This is a derived, finite CPython **3.12.11**, Unicode **15.0.0** profile,
not a claim about an extensible process codec registry or the latest host Python.
The full upstream CPython v3.12.11 license, including PSF and historical license
terms, is reproduced in `LICENSE.txt`. Per-file attribution, mapping provenance,
permission statements and other source notices are retained in `NOTICES.md`.
Upstream: <https://github.com/python/cpython/tree/v3.12.11/Lib/encodings>.
No raw stdlib module is embedded in the Rust engine.
Derived from CPython's Python Character Mapping Codecs and `encodings.aliases`.
Copyright Python Software Foundation and contributors, under the Python
Software Foundation License Version 2 and accompanying historical terms.

## Source Closure And Audit

`manifest.json` lists SHA-256 hashes for all 122 installed `encodings/*.py`
files plus `Lib/codecs.py`. The closure hash is SHA-256 of their relative-path
to lowercase-hash dictionary serialized as sorted ASCII JSON with separators
`,` and `:` and no terminal newline. Both generator and independent checker
contain reviewed literal anchors. All inputs and outputs are bounded UTF-8
text: 2 MiB per file, 32 MiB aggregate. The generator verifies this complete
Python source closure before importing any codec module. It requires `-I` and
checks the stdlib package and imported module origins; it never inspects an
analyzed application's imports or search paths and never fetches inputs.

The native `_codecs` primitives and interpreter are the explicitly trusted
installed CPython 3.12.11 oracle, not vendored source inputs. The Python source
closure hash is not an attestation of native binaries or a general Python build.
The source URL identifies the intended upstream release; these installed Python
source inputs have not been independently compared byte-for-byte with upstream.
The full license bytes, separately, match the raw upstream v3.12.11 license.

For each of the 70 literal 256-character `decoding_table` assignments, an AST
audit verifies that `Codec.decode` is exactly a direct stateless
`codecs.charmap_decode(input, errors, decoding_table)`, the incremental decoder
is the matching stateless call, and `getregentry` wires those implementations
into `CodecInfo`. The full-source hashes pin all other assignments and wiring;
256 output samples alone are not the family proof. Each strict byte is also
checked against the trusted module, including every undefined U+FFFE sentinel,
and each defined-byte stream is decoded both together and incrementally.
There are 23 tables with undefined bytes; maximum defined UTF-8 expansion is 3.

Only 62 tables preserve all ASCII bytes. Seven EBCDIC tables and `cp864` are
frozen for provenance but never admitted. Latin-1 has a dedicated fast path;
strict ASCII and existing UTF-8 are separately admitted, not inferred charmap
family members. These data do not waive source-decoder resource limits.

## Source-Only UTF-8-SIG Certificate

This module is a **captured-source** profile, not a general codec API. Its caller
must apply source-cookie historical normalization and the leading-BOM conflict
gate before registry lookup. Only then does this profile admit `utf_8_sig` as
`Codec::Utf8`: a cookie-bearing source buffer reaching lookup cannot begin with
a UTF-8 BOM. The pinned `encodings/utf_8_sig.py` decoder strips exactly an initial
UTF-8 BOM, then delegates to strict UTF-8; on this remaining buffer it therefore
has the same result as UTF-8. An interior U+FEFF must remain unchanged.

The trusted CPython AST oracle checks five cookie spellings (`utf-8-sig`,
`utf_8_sig`, `UTF--8_sig`, `utf---8---sig`, `UTF__8_sig`), each with and without a
leading BOM, in first- and second-line cookies: 20 cases, 14 accepted and six
rejected. Payloads contain non-ASCII text and interior U+FEFF. All five work without
a BOM. Only the first two pass historical normalization with a BOM; the other
three fail with line-zero SyntaxError before this certificate applies. Generic
UTF-8-SIG is **not** universally UTF-8: it strips a leading BOM and its
incremental decoder has signature-detection state. That generic decoder is not
added to the stateless single-byte family or substituted before the source gate.

## Notice Retention

`NOTICES.md` is generated deterministically from all 70 table modules and
`encodings/aliases.py`, with each source path and hash. It retains each complete
header docstring and every standalone string expression, plus all comment tokens
outside mapping-data assignments. Every mapping comment is also scanned for
legal/provenance terms; matching comments are retained except Unicode character
annotations named COPYRIGHT SIGN, which are not copyright notices. No module's
implementation statements or mapping literals are copied into the notice file.

The 71-file audit found CNRI and Guido van Rossum copyright/no-warranty notices
in `mac_latin2` and `ptcp154`, a Marc-Andre Lemburg author credit in both, Sjoerd
Mullender's PalmOS credit, and HP-ROMAN8's Keld Simonsen/HP printer-manual source
attribution and explicit "Used with permission" statement. Generated mapping
paths and other original headers are retained for every module, not replaced
with a blanket PSF attribution. No additional standalone license grant or
separate license-document requirement was found in these 71 pinned Python
inputs. This is a source-notice audit, not an independent relicensing of the
external mapping files named by their headers; those files are not inputs to
generation. Redistribution must retain both `LICENSE.txt` and `NOTICES.md`.

## Formats And Names

`tables.tsv` is sorted by canonical module name. Each 1564-byte ASCII record
contains a 24-byte space-padded name, tab, identity flag, tab, 256 concatenated
six-digit uppercase code points, newline. U+FFFE denotes an undefined byte.
Stable IDs are zero-based record positions across all 70 tables; excluded IDs
must return no scalar, even for an otherwise defined byte.

`aliases.tsv` includes every literal entry of pinned `encodings.aliases` (326),
plus modules declaring `getregentry`. Aliases override canonical module names.
Each 98-byte record has padded 64-byte name, tab, padded 24-byte module, tab,
status, tab, three-digit table ID (`999` if absent), tab, alias-origin flag,
newline. Statuses: U source-certified UTF-8 (including the conditional UTF-8-SIG
certificate above), L Latin-1, A ASCII, S admitted table, T unsupported
text, N explicitly non-text (`_is_text_encoding=False`), F `undefined` (always
raises UnicodeError), R unresolved (including platform-dependent mbcs/oem).
No arbitrary registered hooks or host-dependent availability is asserted.
The exact alias dictionary includes the mixed-case key `csHPRoman8`; registry
lowercasing makes that entry unreachable. It is preserved, not silently repaired
into an admitted `cshproman8` alias. Canonical `hp_roman8` remains supported.

Lookup folds ASCII case and applies the registry punctuation-collapse rule,
preserving dots, not the tokenizer's historical cookie normalization. A second
dot-to-underscore lookup is allowed only for pinned alias entries, matching
`encodings.search_function`. Inputs over 64 bytes or containing non-ASCII or NUL are
conservatively unresolved. Empty and unknown names are unresolved, not syntax
failure certificates. Cancellation is checked before the bounded name work.

## Reproduction

Use the trusted installed interpreter, with no tool installation or downloads:

```sh
python3.12 -I -B scripts/generate_python_codecs312.py --check
python3.12 -I -B scripts/check_python_codecs312.py
python3.12 -I -B scripts/test_python_codecs312.py
```

Omitting `--check` regenerates only `tables.tsv`, `aliases.tsv`, `NOTICES.md`
and `manifest.json`. Generation requires the independently hash-pinned full
CPython license already present; it never rewrites the license. The checker has
separate hardcoded source, data, provenance and license anchors. Tests exercise
tampering without copying arbitrary stdlib or application modules. No temporary
files or build artifacts are needed. Both generator and checker reject missing
or mismatched license bytes; there is no pending-license bypass.
