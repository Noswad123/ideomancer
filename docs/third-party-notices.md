# Third-Party Notices: Native Parser and Selected Dependencies

This is a scoped notice collection for the pinned native JavaScript/TypeScript
parser path, the private Python candidate path, and selected other dependencies,
not a complete binary dependency/license review. The Python grammar is not selected
by production discovery; Python continues to use the AST helper.
It covers the tree-sitter runtime and Rust bindings, language bridge, generated
JavaScript, TypeScript, TSX and Python grammars, native scanners, selected parser
dependencies, and unsafe-libyaml's Rust translation of libyaml listed below.
The runtime and grammar/scanner C code is compiled into native artifacts;
reviewing only Rust crate license metadata is insufficient. This collection
does not assert that all dependencies are MIT licensed.

Executable bundles still require a whole dependency audit for the actual target
and enabled features, including transitive, native, build-time and platform
components as applicable. In particular, this collection does not purport to
cover every dependency in Cargo.lock (including the rest of the regex dependency
graph), prove which code is linked, or authorize binary release by itself.
Distributors must retain applicable notices with their artifacts.

## Maintained Vendored Ruff Frontend

The private Python syntax frontend also includes selected Ruff **0.12.0** source
from https://github.com/astral-sh/ruff at exact revision
`87f0feb21a1af67195bebf94d811770393f9f447`. Unlike the registry sources below,
its provenance is recorded in the checkout's
[`vendor/ruff/provenance.json`](../vendor/ruff/provenance.json). The five source
crates, adapted manifests, maintained patches and upstream notices travel with Ideomancer
Git source; there is no separate fork/submodule or registry publication requirement.

- The complete [upstream LICENSE](../vendor/ruff/LICENSE) retains Charles Marsh's
  MIT terms, the RustPython Team 2020 MIT attribution and upstream's other notices.
  Retain the complete file, not only the root MIT heading.
- [`vendor/ruff/notices/`](../vendor/ruff/notices/) preserves baseline
  `unicode_names2 1.3.0` notices; it embeds Unicode 16 name/alias data.
- The actual merged root selection is `unicode-ident 1.0.24` (Unicode 17),
  `unicode_names2 1.3.0` (Unicode 16) and `unicode-normalization 0.1.24`
  (Unicode 16). PY312 parser targeting does **not** establish Unicode 15 identifier
  admission or CPython compatibility. Registry dependency sources are not vendored
  here; preserve applicable notices for actual selected versions in binary bundles.

### Current F3a provenance and license boundary

Private byte-to-facts integration changes authored product code only. The
266-path source closure retains the unchanged F2 vendor snapshot, dependency pins
and all existing MIT, Ruff/RustPython, Unicode and PSF/codec notices. No vendor,
data, manifest or license change accompanies F3a. Current validation/delivery
receipts mark **all bounded F3a implementation/delivery gates PASS**, including
independent consumer and guarded cleanup; see the
[F3a ledger](python-analysis.md#f3a-captured-byte-fact-integration), not the historical
F2 consumer/cache receipt below. The tested snapshot does not attest later receipt
prose. This grants no future implementation authorization and is neither production
cutover nor binary/whole-graph legal clearance.

### Historical F2 provenance and license boundary: bounded delivery PASS

All F2 counts, HEAD/uncommitted observations, tests and cache statements below
describe the F2 handoff only; F2 is now committed at `1ef1f0c`.

The bounded private byte Module parser/source/debug implementation passed final
preservation/oracles, scripts and provenance checks; current consumer/cleanup also
passed. **All bounded F2 delivery gates PASS**, target absent, no Cargo afterward.
This authorizes no further implementation. It changes neither production
`python-static-8-generic-facts`/selected Python >=3.12 helper nor notice terms,
dependencies or manifests. Frontend-5/facts-7 names stay fixed; fingerprints change.
User-reported HEAD `805edb0` contains committed F1, initially clean; F2 is uncommitted
and unstaged, with no commit or push.

The schema-3 live checker passed: **89 vendor files / 67 Rust sources / 2,219,373
bytes** (65 upstream + two authored), **14 modified / 51 unchanged upstream**;
**264 source paths verified**, and **84 vendor tests passed** after the final fix.
The third **nine-file F2 patch layers
after the old policy patch**; old two patches and the 65-source pristine map are
unchanged. Preserve ordered reversal and both authored-source hashes: authored
MIT code is not pristine upstream or a relicensing of Ruff. Retain root MIT,
complete Ruff/RustPython, Unicode and PSF/codec licenses/notices together.
Exact implementation hashes and full validation receipts are recorded in the
primary ledger, including completed current consumer/cleanup delivery receipts.
The tested snapshot is not a Git commit and does not attest final receipt prose;
do not reuse F1 consumer/cache PASS. See the primary
[F2 ledger](python-analysis.md#f2-byte-module-parser-source-and-debug-experiment).
Finite tests are not CPython parity, G4 acceptance or binary/whole-graph license
clearance. No public cutover or native immutable-profile policy is approved.

### Historical F1 provenance and license boundary

All counts, hashes, tests, authorization and cache receipts in the F1 section
below describe that handoff only, not current F2 validation or cache state.

The bounded private F1 experiment adds a shared fallible byte lexer core and
doc-hidden experimental facade, not a public engine backend or byte parser.
Schema-2 [provenance](../vendor/ruff/provenance.json) records **87 vendor files /
66 Rust sources / 1,972,891 bytes**. The **65-source upstream pristine map is
unchanged**: eight upstream files modified by two disjoint four-file patches,
57 unchanged; one additional original `lexer/byte.rs` is separately hashed as
authored Ideomancer code under the [root MIT license](../LICENSE), not pristine
upstream. Keep that license **and** the complete Ruff license/notices. Existing
PSF/codec and Unicode notices and terms are unchanged; there are no dependency or
manifest changes.

`ast-only-policy.patch` is unchanged; `byte-lexer-experiment.patch` covers
`lexer.rs`, `lexer/{cursor,indentation,interpolated_string}.rs`. Current SHA-256:
provenance `83b701ef82cd549d1b01501520f57b9504e3decac32cdc7d151add639f892800`;
F1 patch `4f4d3a244236a3e730ab805716f23f581483fa6e7a38060b1ea10c344fe5aac5`;
authored facade `d8e617b7bb9401e96263997b99151cfa02e3dd0af1254a384c7569e12d2fec94`.
The active source inventory is **261 paths** (+5 from F0's 256). Coordinator reports
**all 131 private Ruff tests passed** after fixes, including **23 F1 ordinary + 17
existing opt-ins**; all **364** complete positive facts/IDs/diagnostics/raw replay
matched on the unchanged normal text path. Both facades passed all thirteen
pre-core hashes (298 tokens/all four modes), including compiled legacy `Send + Sync`.
Product formatting/all-target Clippy, **58 vendor script tests**, schema-2 vendor
checker and the **261-path** source gate passed. Scripts: **310 discovered,
297 passed / 13 isolation skips**; isolated byte/visitor/codec **23 / 43 / 24 passed**,
no skips, covering all 310 unique tests across lanes. Current preservation passed
**708 library tests (191 ignored), 18 adapter + 8 engine** and explicit **165
reference + 3 native** tests. With the 17 Ruff opt-ins, **all 185 explicit gates
passed**, including known-limit assertions—not 185 parity matches. P2 view
accounting is closed and its root regression passed; final read-only review found
no material P1/P2/P3 findings and new schema-2 guards passed. No additional core
changes are planned and provenance anchors are stable. **Bounded F1 lexer goal
COMPLETE / PROVED by compiled tests; all F1 delivery gates PASS**. Offline consumer
passed with fresh **105-package** lock resolution, **one native TypeScript
public-API test**, five contained Ruff crates, ABI **0.1.5** and source-hash rechecks.
Its snapshot (hash in the [F1 ledger](python-analysis.md#f1-shared-byte-lexer-experiment))
is not a Git revision or final-docs attestation; its owned cache `TMPDIR` was cleaned.
Guarded preflight found zero blockers/protected outputs, normal `debug`/`tmp` only;
cleanup removed **1,360,498,688 estimated allocated bytes (~1.27 GiB)**. Target
absent, follow-up `already_clean`, **no Cargo afterward**. No owned debug/test
artifacts remain; root cache parent empty, `.ideomancer/` JS/global caches untouched.
Final source/Git/helper checks are read-only, without rebuilding.
This does not prove byte parser input, AST/debug construction, CPython admission,
F2 or full G4, or itself authorize further implementation. Bounded F2 was later
authorized; version-policy approval remains separate.
These are not binary-release, whole-graph
license or G4 acceptance. See the [F1 ledger](python-analysis.md#f1-shared-byte-lexer-experiment)
for tests/baseline and pending F2 parser/source/debug/admission proof.

Actual pre-F1 HEAD was user-reported `fdf4e58` (facts-6 commit, initially clean/ahead
one), not `523cd7b`; F1 work was then uncommitted and unstaged, with no agent commits or
pushes. The source counts,
HEAD and cleanup statements in the following
visitor/codec receipts describe historical checkpoints, not current-tree status.

### Historical visitor checkpoint

There were 65 selected Rust source files in an 85-file vendor snapshot totaling
1,882,473 bytes. Four upstream source files carry the reviewed policy patch:
duplicate call/class-header keyword validation can be omitted via an explicit
AST-only option that defaults to false, additional f-string mapping/pattern-start
forms are admitted only in that mode, and tuple/parameter-specification defaults
receive the existing type-variable target-version guard. Valid default-mode forms remain unchanged,
including upstream singleton f-string first-element acceptance. An independent
sequence-separator correction now requires an actual comma before closing-token
lookahead in both modes; it deliberately rejects previously erroneously accepted
malformed default-mode sequences. Adapted manifests disable optional features
and upstream tests. The root lock merge adds five local and thirty registry packages
while retaining prior product selections; the upstream lock is provenance, not the
actual product closure. These facts do not constitute a whole-graph license audit.

The visitor checkpoint retained the same four-file patch and 65-source pristine closure.
Its policy-patch SHA-256 (still unchanged by F1) is
`e79a66530f58f63371c0423f5f0469ff82f9b0193f57b6ba8672c3c33aa4e90e`;
provenance anchor is
`b8246ec2c0911719ddd715f80dac504795c97f70ab828bd4375d18538c237ee0`.
The source inventory is 253 paths (+5 visitor contract/check/oracle files).
Dependencies, Unicode/codec data and notice/license terms are unchanged. After both
parser fixes, **108 Ruff tests (17 opt-in)** and **364 complete positive
array/ID/diagnostic/raw-replay cases** passed (344 preserved + 20 new), along with
prior failure/Unicode/codec and 787-outcome contextual oracles. Product formatting,
all-target Clippy (`--no-deps -- -D warnings`), 34 vendor script tests, updated
vendor integrity and the 253-path source check passed. Scripts: **263 discovered,
257 passed, six isolation-required skips**; isolated visitor **43** and codec **24**
passed without skips, covering all **263 unique tests across lanes**, not 263
discovery-lane passes. Preservation passed **685 library tests (191 ignored)**,
**18 adapter + 8 engine** and **165 reference + 3 native** opt-ins. With **17 Ruff
opt-ins**, **185 explicit gates passed**, including known-limit assertions—not all
parity matches. **G2/G3 PRIVATE IMPLEMENTATION COMPLETE/CLOSED for current admitted
Python 3.12 Module-AST lane: closed class/field inventory + full-output/raw-replay
coverage; not complete Python compatibility, G4 bytes/admission/failure/resource/
version equivalence, or production cutover.** G5/G6 remain open. Current local
macOS / Rust 1.89 delivery closeout passed: offline consumer fresh **105-package**
lock and **one native TypeScript public-API test**, five-Ruff-crate containment,
ABI **0.1.5** and source-hash rechecks. Its configured unique owned cache `TMPDIR`
was cleaned with none left. Guarded preflight found **zero blockers / zero protected
outputs**, normal `debug`/`tmp` only; cleanup removed **1,374,564,352 estimated
allocated bytes (~1.28 GiB)**. Target absent, follow-up `already_clean`, no Cargo
afterward. No owned debug/test artifacts remain; `.ideomancer/` JS artifacts/global
caches were untouched. Source **253**, vendor **85/65/1,882,473 bytes** and Git diff
checks passed before final docs. The consumer snapshot (hash in the ledger) is not
a Git revision or later-docs byte attestation. Final source/Git/helper checks are
read-only, no rebuild. Nothing was staged, committed or pushed; HEAD is `523cd7b`.
Local delivery passes do not establish hosted/platform or full production approval.
The G4 STOP still requires a user decision: recommended bounded byte-aware Ruff
feasibility preserving the contract and an explicit fixed-PY312 versus production
>=3.12 version strategy, or a separately limited native capability retaining the
helper. “Continue” authorizes neither narrowing nor replacement. Type-comment
syntax fields remain intentionally uncertified (`failure: None`), including
for/with/class suite cases whose helper exception can be `IndentationError`;
another PEG parser alone cannot close arbitrary failure-location/class parity.
No further code changes are planned; actual differences are narrow parser fixes
and proof files, not a visitor/normalizer rewrite. Pattern controls now pass in
both modes/constructors, preserving pre-existing singleton-first acceptance and
adding only AST-only class-first/later-sequence predicate admission, while
rejecting real missing-separator errors, not weakening tests. The codec validation/
cleanup below is historical, not current-tree or binary-release acceptance. See the
[current visitor ledger](python-analysis.md#ruff-visitor-contract-batch-frontend-5--private-facts-7).

Maintenance gate: `python3.12 -B scripts/check_ruff_vendor.py` verifies anchored
provenance, hashes, manifest/notices policy and reversal to pristine source. See
[snapshot maintenance](../vendor/ruff/README.md) and [Git delivery](releasing.md).
Keep upstream source pristine except for the maintained patch closure above,
tracking both authored sources separately and respecting F2-after-policy layer
order; the two-patch receipt is historical. Format only root product code
with `RUSTUP_AUTO_INSTALL=0 cargo fmt --package ideomancer --check`, never `--all`.

The private frontend supplies captured-byte syntax classification and a separate
bounded generic fact candidate, not public Evidence, an AST or source excerpts.
Production Python remains the trusted >=3.12 AST helper; complete CPython
compatibility and public native integration remain Pending. Source
distribution must retain these notices; executable distribution additionally needs
actual target/feature, transitive, native and toolchain review and a notice-bearing
bundle. `make install` copies only the executable and is not license clearance.

### Unicode 15 Compatibility Data

The shared private frontend and fact candidate use pinned Unicode **15.0.0** source inputs and
derived identifier/name tables in
[`src/analysis/python_ruff/unicode15/`](../src/analysis/python_ruff/unicode15/).
Its [provenance](../src/analysis/python_ruff/unicode15/PROVENANCE.md) and
[manifest](../src/analysis/python_ruff/unicode15/manifest.json) record the original
URLs, exact bytes/hashes and derivation profile. This is distinct from the newer
Unicode tables in Ruff's registry dependencies; it does not replace their notices.

Retain the complete
[Unicode License V3 notice](../src/analysis/python_ruff/unicode15/inputs/LICENSE.txt)
with the data and derived tables, including in binary distributions that embed
them. The pinned notice was retrieved from Unicode's mutable license URL on
2026-11-27; its preserved bytes/hash, not that URL's future contents, are the
reproduction anchor. These data are not relicensed under Ideomancer's MIT license.
`python3.12 -B scripts/check_python_unicode15.py` checks the separate input/table
anchors and the trusted CPython 3.12.11 / Unicode 15.0.0 derivation profile. This
finite admission/name layer does not establish complete CPython compatibility.
The frontend-3/facts-5 failure-certificate work changes neither these data/notices
nor the vendored snapshot, policy patch or dependency selection.
Its offline exported-checkout consumer passed containment and source-hash checks;
this does not extend the scoped notices into binary-release or legal clearance.

## Sources and Pinned Revisions

### CPython 3.12.11 Codec Data

The validation and ~1.28 GiB cleanup receipts in this subsection describe the
historical frontend-4/facts-6 checkpoint only. Historical facts-7 cleanup completed,
removing **1,374,564,352 estimated allocated bytes (~1.28 GiB)** with target absent
and follow-up `already_clean`; no Cargo ran afterward. Retain all license texts
below unchanged.

The frontend-4/facts-6 private source decoder embeds derived tables and aliases in
[`src/analysis/python_ruff/codecs312/`](../src/analysis/python_ruff/codecs312/),
not raw stdlib modules. Preserve its full
[`LICENSE.txt`](../src/analysis/python_ruff/codecs312/LICENSE.txt) and
[`NOTICES.md`](../src/analysis/python_ruff/codecs312/NOTICES.md) together in Git
source and applicable binary distributions. These are not MIT-relicensed data.
Existing Unicode, Ruff and other applicable notices remain required.

The full PSF and historical license stack is the exact raw upstream CPython
v3.12.11 LICENSE: **13,936 bytes**, SHA-256
`3b2f81fe21d181c499c59a256c8e1968455d6689d269aa85373bfb6af41da3bf`,
separately verified against upstream. NOTICES retains headers and legal/provenance
mapping comments from all **71** table/aliases modules, including module-specific
credits and permission/no-warranty statements rather than blanket PSF attribution.
See the [audit/provenance](../src/analysis/python_ruff/codecs312/PROVENANCE.md).

The profile structurally audits 70 literal 256-code-point tables and stateless
decode/stream/incremental/registry wiring against a 123-file installed stdlib
source closure. Only 62 ASCII-identity tables are admitted; eight nonidentity
tables remain excluded. The installed Python-source hash is not native-binary
attestation or independent byte-identical comparison with an upstream commit.
Data/code/scripts/manifest/provenance/notices/license are fingerprinted in the
248-path source closure (11 additions to 237). Dependencies, vendored Ruff patch
and existing Unicode notices are unchanged. The 248-path source and vendor
85-file / 65-source / 1,881,065-byte checks passed. The isolated codec suite passed
24 tests with none skipped; generator check and independent checker passed with
`license_verified: true`, using `-I -S -B`; CI explicitly runs generator check,
independent checker and the isolated codec suite with those flags. Global script discovery was
216 tests: 211 passed, five isolated-profile skips; the combined lanes cover all
216 unique tests, not 216 default passes. All 104 Ruff tests (16 opt-in), product
package formatting and all-target product Clippy passed locally on macOS / Rust
1.89; hosted results remain unknown. Broader gates passed **682 library tests
(190 ignored)**, **18 adapter + 8 engine**, and **165 reference + 3 native**
explicit opt-in tests. With 16 Ruff opt-ins, **184 explicit tests** passed,
including known-limit assertions—not all parity matches. The current offline
consumer passed fresh 105-package lock resolution and one native TypeScript
public-API test, five-crate containment, ABI 0.1.5 and source-hash rechecks;
product formatting passed beforehand. Its owned scratch was cleaned. Guarded
cleanup removed **1,372,758,016 estimated allocated bytes (~1.28 GiB)** with
zero preflight blockers/protected outputs; `target/` was absent and follow-up
reported `already_clean`, with no Cargo afterward. No debug/test artifacts were
retained; the owned cache parent was empty and `.ideomancer/` JS artifacts/global
caches were untouched. Source (248 paths) and Git whitespace checks passed before
final documentation edits; the tested snapshot in the Python ledger is not a Git
revision or later-documentation attestation. Final source/Git/cache verification
is read-only, without rebuilding. Codec-family and prior facts-5 work remains
uncommitted at actual HEAD `523cd7b`; the historical facts-5 ~1.27 GiB cleanup is
separate. Ideomancer's MIT library license does
not replace these codec-data terms or authorize binary release.

### Registry crate revisions

Versions below are the reviewed Cargo.lock versions. Revisions and repository
subdirectories are from each exact crate's `.cargo_vcs_info.json`, not inferred
from tags or a different cached version. Bundled files are copied from those
exact crates. The three missing root MIT files are taken from the corresponding
upstream revision URLs below.

| Component | Upstream repository | Revision | Crate repository subdirectory |
| --- | --- | --- | --- |
| tree-sitter 0.25.10 | https://github.com/tree-sitter/tree-sitter | `da6fe9beb4f7f67beb75914ca8e0d48ae48d6406` | `lib` |
| tree-sitter-language 0.1.5 | https://github.com/tree-sitter/tree-sitter | `8b5c63bffae4c1948106eb0a8d722bf2a9141ce5` | `lib/language` |
| tree-sitter-javascript 0.25.0 | https://github.com/tree-sitter/tree-sitter-javascript | `44c892e0be055ac465d5eeddae6d3e194424e7de` | repository root |
| tree-sitter-typescript 0.23.2 (TypeScript and TSX) | https://github.com/tree-sitter/tree-sitter-typescript | `f975a621f4e7f532fe322e13c4f79495e0a7b2e7` | repository root |
| tree-sitter-python 0.25.0 | https://github.com/tree-sitter/tree-sitter-python | `293fdc02038ee2bf0e2e206711b69c90ac0d413f` | repository root |
| streaming-iterator 0.1.9 | https://github.com/sfackler/streaming-iterator | `69069dc9e46facef6caa79ee48e47420de92633d` | repository root |
| regex 1.13.1 | https://github.com/rust-lang/regex | `2b527599eb9eea0dcc288c704584f242f26a5c61` | repository root |
| regex-automata 0.4.18 | https://github.com/rust-lang/regex | `e6ad938198e68339a593f85d228f2d44cb54718f` | `regex-automata` |
| regex-syntax 0.8.11 | https://github.com/rust-lang/regex | `140167995737fa11dfe11b8af8b9aa143b790b4e` | `regex-syntax` |
| unsafe-libyaml 0.2.11 | https://github.com/dtolnay/unsafe-libyaml | `a7b8d1fbd93aefbca3003dcb5fcc6a9c2297e968` | repository root |

The unsafe-libyaml 0.2.11 README identifies its translated C source as
https://github.com/yaml/libyaml/tree/2c891fc7a770e8ba2fec34fc6b545c672beb37e6.
This libyaml source revision is distinct from the translation crate revision
above; the upstream license is retained separately below.

## Tree-Sitter Runtime and Language Bridge: MIT

The full text below applies to both tree-sitter 0.25.10 and
tree-sitter-language 0.1.5. Their upstream root LICENSE texts are identical:

- Runtime: https://raw.githubusercontent.com/tree-sitter/tree-sitter/da6fe9beb4f7f67beb75914ca8e0d48ae48d6406/LICENSE
- Bridge: https://raw.githubusercontent.com/tree-sitter/tree-sitter/8b5c63bffae4c1948106eb0a8d722bf2a9141ce5/LICENSE

```text
The MIT License (MIT)

Copyright (c) 2018-2024 Max Brunsfeld

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## JavaScript Grammar and Scanner: MIT

Full bundled `tree-sitter-javascript 0.25.0/LICENSE`, also available at
https://raw.githubusercontent.com/tree-sitter/tree-sitter-javascript/44c892e0be055ac465d5eeddae6d3e194424e7de/LICENSE:

```text
The MIT License (MIT)

Copyright (c) 2014 Max Brunsfeld

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## TypeScript and TSX Grammars and Scanners: MIT

Full upstream root LICENSE for tree-sitter-typescript 0.23.2, covering both
TypeScript and TSX:
https://raw.githubusercontent.com/tree-sitter/tree-sitter-typescript/f975a621f4e7f532fe322e13c4f79495e0a7b2e7/LICENSE

```text
The MIT License (MIT)

Copyright (c) 2017 Max Brunsfeld

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Runtime Bundled Unicode License

The entire `tree-sitter 0.25.10/src/unicode/LICENSE` follows verbatim, including
its ICU and third-party sections. Retaining the bundled file in full does not
assert that every ICU component described in it is linked into this parser.
No narrower substitute attribution or component applicability determination is
made here. Non-ASCII characters in these notices are retained from the sources.

```text
COPYRIGHT AND PERMISSION NOTICE (ICU 58 and later)

Copyright © 1991-2019 Unicode, Inc. All rights reserved.
Distributed under the Terms of Use in https://www.unicode.org/copyright.html.

Permission is hereby granted, free of charge, to any person obtaining
a copy of the Unicode data files and any associated documentation
(the "Data Files") or Unicode software and any associated documentation
(the "Software") to deal in the Data Files or Software
without restriction, including without limitation the rights to use,
copy, modify, merge, publish, distribute, and/or sell copies of
the Data Files or Software, and to permit persons to whom the Data Files
or Software are furnished to do so, provided that either
(a) this copyright and permission notice appear with all copies
of the Data Files or Software, or
(b) this copyright and permission notice appear in associated
Documentation.

THE DATA FILES AND SOFTWARE ARE PROVIDED "AS IS", WITHOUT WARRANTY OF
ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE
WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
NONINFRINGEMENT OF THIRD PARTY RIGHTS.
IN NO EVENT SHALL THE COPYRIGHT HOLDER OR HOLDERS INCLUDED IN THIS
NOTICE BE LIABLE FOR ANY CLAIM, OR ANY SPECIAL INDIRECT OR CONSEQUENTIAL
DAMAGES, OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE,
DATA OR PROFITS, WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER
TORTIOUS ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE OR
PERFORMANCE OF THE DATA FILES OR SOFTWARE.

Except as contained in this notice, the name of a copyright holder
shall not be used in advertising or otherwise to promote the sale,
use or other dealings in these Data Files or Software without prior
written authorization of the copyright holder.

---------------------

Third-Party Software Licenses

This section contains third-party software notices and/or additional
terms for licensed third-party software components included within ICU
libraries.

1. ICU License - ICU 1.8.1 to ICU 57.1

COPYRIGHT AND PERMISSION NOTICE

Copyright (c) 1995-2016 International Business Machines Corporation and others
All rights reserved.

Permission is hereby granted, free of charge, to any person obtaining
a copy of this software and associated documentation files (the
"Software"), to deal in the Software without restriction, including
without limitation the rights to use, copy, modify, merge, publish,
distribute, and/or sell copies of the Software, and to permit persons
to whom the Software is furnished to do so, provided that the above
copyright notice(s) and this permission notice appear in all copies of
the Software and that both the above copyright notice(s) and this
permission notice appear in supporting documentation.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT
OF THIRD PARTY RIGHTS. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR
HOLDERS INCLUDED IN THIS NOTICE BE LIABLE FOR ANY CLAIM, OR ANY
SPECIAL INDIRECT OR CONSEQUENTIAL DAMAGES, OR ANY DAMAGES WHATSOEVER
RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION OF
CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

Except as contained in this notice, the name of a copyright holder
shall not be used in advertising or otherwise to promote the sale, use
or other dealings in this Software without prior written authorization
of the copyright holder.

All trademarks and registered trademarks mentioned herein are the
property of their respective owners.

2. Chinese/Japanese Word Break Dictionary Data (cjdict.txt)

 #     The Google Chrome software developed by Google is licensed under
 # the BSD license. Other software included in this distribution is
 # provided under other licenses, as set forth below.
 #
 #  The BSD License
 #  http://opensource.org/licenses/bsd-license.php
 #  Copyright (C) 2006-2008, Google Inc.
 #
 #  All rights reserved.
 #
 #  Redistribution and use in source and binary forms, with or without
 # modification, are permitted provided that the following conditions are met:
 #
 #  Redistributions of source code must retain the above copyright notice,
 # this list of conditions and the following disclaimer.
 #  Redistributions in binary form must reproduce the above
 # copyright notice, this list of conditions and the following
 # disclaimer in the documentation and/or other materials provided with
 # the distribution.
 #  Neither the name of  Google Inc. nor the names of its
 # contributors may be used to endorse or promote products derived from
 # this software without specific prior written permission.
 #
 #
 #  THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND
 # CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES,
 # INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
 # MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
 # DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR CONTRIBUTORS BE
 # LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
 # CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
 # SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR
 # BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF
 # LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
 # NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
 # SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 #
 #
 #  The word list in cjdict.txt are generated by combining three word lists
 # listed below with further processing for compound word breaking. The
 # frequency is generated with an iterative training against Google web
 # corpora.
 #
 #  * Libtabe (Chinese)
 #    - https://sourceforge.net/project/?group_id=1519
 #    - Its license terms and conditions are shown below.
 #
 #  * IPADIC (Japanese)
 #    - http://chasen.aist-nara.ac.jp/chasen/distribution.html
 #    - Its license terms and conditions are shown below.
 #
 #  ---------COPYING.libtabe ---- BEGIN--------------------
 #
 #  /*
 #   * Copyright (c) 1999 TaBE Project.
 #   * Copyright (c) 1999 Pai-Hsiang Hsiao.
 #   * All rights reserved.
 #   *
 #   * Redistribution and use in source and binary forms, with or without
 #   * modification, are permitted provided that the following conditions
 #   * are met:
 #   *
 #   * . Redistributions of source code must retain the above copyright
 #   *   notice, this list of conditions and the following disclaimer.
 #   * . Redistributions in binary form must reproduce the above copyright
 #   *   notice, this list of conditions and the following disclaimer in
 #   *   the documentation and/or other materials provided with the
 #   *   distribution.
 #   * . Neither the name of the TaBE Project nor the names of its
 #   *   contributors may be used to endorse or promote products derived
 #   *   from this software without specific prior written permission.
 #   *
 #   * THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
 #   * "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
 #   * LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS
 #   * FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE
 #   * REGENTS OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT,
 #   * INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
 #   * (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
 #   * SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
 #   * HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
 #   * STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
 #   * ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED
 #   * OF THE POSSIBILITY OF SUCH DAMAGE.
 #   */
 #
 #  /*
 #   * Copyright (c) 1999 Computer Systems and Communication Lab,
 #   *                    Institute of Information Science, Academia
 #       *                    Sinica. All rights reserved.
 #   *
 #   * Redistribution and use in source and binary forms, with or without
 #   * modification, are permitted provided that the following conditions
 #   * are met:
 #   *
 #   * . Redistributions of source code must retain the above copyright
 #   *   notice, this list of conditions and the following disclaimer.
 #   * . Redistributions in binary form must reproduce the above copyright
 #   *   notice, this list of conditions and the following disclaimer in
 #   *   the documentation and/or other materials provided with the
 #   *   distribution.
 #   * . Neither the name of the Computer Systems and Communication Lab
 #   *   nor the names of its contributors may be used to endorse or
 #   *   promote products derived from this software without specific
 #   *   prior written permission.
 #   *
 #   * THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
 #   * "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
 #   * LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS
 #   * FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE
 #   * REGENTS OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT,
 #   * INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
 #   * (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
 #   * SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
 #   * HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
 #   * STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
 #   * ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED
 #   * OF THE POSSIBILITY OF SUCH DAMAGE.
 #   */
 #
 #  Copyright 1996 Chih-Hao Tsai @ Beckman Institute,
 #      University of Illinois
 #  c-tsai4@uiuc.edu  http://casper.beckman.uiuc.edu/~c-tsai4
 #
 #  ---------------COPYING.libtabe-----END--------------------------------
 #
 #
 #  ---------------COPYING.ipadic-----BEGIN-------------------------------
 #
 #  Copyright 2000, 2001, 2002, 2003 Nara Institute of Science
 #  and Technology.  All Rights Reserved.
 #
 #  Use, reproduction, and distribution of this software is permitted.
 #  Any copy of this software, whether in its original form or modified,
 #  must include both the above copyright notice and the following
 #  paragraphs.
 #
 #  Nara Institute of Science and Technology (NAIST),
 #  the copyright holders, disclaims all warranties with regard to this
 #  software, including all implied warranties of merchantability and
 #  fitness, in no event shall NAIST be liable for
 #  any special, indirect or consequential damages or any damages
 #  whatsoever resulting from loss of use, data or profits, whether in an
 #  action of contract, negligence or other tortuous action, arising out
 #  of or in connection with the use or performance of this software.
 #
 #  A large portion of the dictionary entries
 #  originate from ICOT Free Software.  The following conditions for ICOT
 #  Free Software applies to the current dictionary as well.
 #
 #  Each User may also freely distribute the Program, whether in its
 #  original form or modified, to any third party or parties, PROVIDED
 #  that the provisions of Section 3 ("NO WARRANTY") will ALWAYS appear
 #  on, or be attached to, the Program, which is distributed substantially
 #  in the same form as set out herein and that such intended
 #  distribution, if actually made, will neither violate or otherwise
 #  contravene any of the laws and regulations of the countries having
 #  jurisdiction over the User or the intended distribution itself.
 #
 #  NO WARRANTY
 #
 #  The program was produced on an experimental basis in the course of the
 #  research and development conducted during the project and is provided
 #  to users as so produced on an experimental basis.  Accordingly, the
 #  program is provided without any warranty whatsoever, whether express,
 #  implied, statutory or otherwise.  The term "warranty" used herein
 #  includes, but is not limited to, any warranty of the quality,
 #  performance, merchantability and fitness for a particular purpose of
 #  the program and the nonexistence of any infringement or violation of
 #  any right of any third party.
 #
 #  Each user of the program will agree and understand, and be deemed to
 #  have agreed and understood, that there is no warranty whatsoever for
 #  the program and, accordingly, the entire risk arising from or
 #  otherwise connected with the program is assumed by the user.
 #
 #  Therefore, neither ICOT, the copyright holder, or any other
 #  organization that participated in or was otherwise related to the
 #  development of the program and their respective officials, directors,
 #  officers and other employees shall be held liable for any and all
 #  damages, including, without limitation, general, special, incidental
 #  and consequential damages, arising out of or otherwise in connection
 #  with the use or inability to use the program or any product, material
 #  or result produced or otherwise obtained by using the program,
 #  regardless of whether they have been advised of, or otherwise had
 #  knowledge of, the possibility of such damages at any time during the
 #  project or thereafter.  Each user will be deemed to have agreed to the
 #  foregoing by his or her commencement of use of the program.  The term
 #  "use" as used herein includes, but is not limited to, the use,
 #  modification, copying and distribution of the program and the
 #  production of secondary products from the program.
 #
 #  In the case where the program, whether in its original form or
 #  modified, was distributed or delivered to or received by a user from
 #  any person, organization or entity other than ICOT, unless it makes or
 #  grants independently of ICOT any specific warranty to the user in
 #  writing, such person, organization or entity, will also be exempted
 #  from and not be held liable to the user for any such damages as noted
 #  above as far as the program is concerned.
 #
 #  ---------------COPYING.ipadic-----END----------------------------------

3. Lao Word Break Dictionary Data (laodict.txt)

 #  Copyright (c) 2013 International Business Machines Corporation
 #  and others. All Rights Reserved.
 #
 # Project: http://code.google.com/p/lao-dictionary/
 # Dictionary: http://lao-dictionary.googlecode.com/git/Lao-Dictionary.txt
 # License: http://lao-dictionary.googlecode.com/git/Lao-Dictionary-LICENSE.txt
 #              (copied below)
 #
 #  This file is derived from the above dictionary, with slight
 #  modifications.
 #  ----------------------------------------------------------------------
 #  Copyright (C) 2013 Brian Eugene Wilson, Robert Martin Campbell.
 #  All rights reserved.
 #
 #  Redistribution and use in source and binary forms, with or without
 #  modification,
 #  are permitted provided that the following conditions are met:
 #
 #
 # Redistributions of source code must retain the above copyright notice, this
 #  list of conditions and the following disclaimer. Redistributions in
 #  binary form must reproduce the above copyright notice, this list of
 #  conditions and the following disclaimer in the documentation and/or
 #  other materials provided with the distribution.
 #
 #
 # THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
 # "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
 # LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS
 # FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE
 # COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT,
 # INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
 # (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
 # SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
 # HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
 # STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
 # ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED
 # OF THE POSSIBILITY OF SUCH DAMAGE.
 #  --------------------------------------------------------------------------

4. Burmese Word Break Dictionary Data (burmesedict.txt)

 #  Copyright (c) 2014 International Business Machines Corporation
 #  and others. All Rights Reserved.
 #
 #  This list is part of a project hosted at:
 #    github.com/kanyawtech/myanmar-karen-word-lists
 #
 #  --------------------------------------------------------------------------
 #  Copyright (c) 2013, LeRoy Benjamin Sharon
 #  All rights reserved.
 #
 #  Redistribution and use in source and binary forms, with or without
 #  modification, are permitted provided that the following conditions
 #  are met: Redistributions of source code must retain the above
 #  copyright notice, this list of conditions and the following
 #  disclaimer.  Redistributions in binary form must reproduce the
 #  above copyright notice, this list of conditions and the following
 #  disclaimer in the documentation and/or other materials provided
 #  with the distribution.
 #
 #    Neither the name Myanmar Karen Word Lists, nor the names of its
 #    contributors may be used to endorse or promote products derived
 #    from this software without specific prior written permission.
 #
 #  THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND
 #  CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES,
 #  INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
 #  MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
 #  DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS
 #  BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL,
 #  EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
 #  TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
 #  DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON
 #  ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR
 #  TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF
 #  THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF
 #  SUCH DAMAGE.
 #  --------------------------------------------------------------------------

5. Time Zone Database

  ICU uses the public domain data and code derived from Time Zone
Database for its time zone support. The ownership of the TZ database
is explained in BCP 175: Procedure for Maintaining the Time Zone
Database section 7.

 # 7.  Database Ownership
 #
 #    The TZ database itself is not an IETF Contribution or an IETF
 #    document.  Rather it is a pre-existing and regularly updated work
 #    that is in the public domain, and is intended to remain in the
 #    public domain.  Therefore, BCPs 78 [RFC5378] and 79 [RFC3979] do
 #    not apply to the TZ Database or contributions that individuals make
 #    to it.  Should any claims be made and substantiated against the TZ
 #    Database, the organization that is providing the IANA
 #    Considerations defined in this RFC, under the memorandum of
 #    understanding with the IETF, currently ICANN, may act in accordance
 #    with all competent court orders.  No ownership claims will be made
 #    by ICANN or the IETF Trust on the database or the code.  Any person
 #    making a contribution to the database or code waives all rights to
 #    future claims in that contribution or in the TZ Database.

6. Google double-conversion

Copyright 2006-2011, the V8 project authors. All rights reserved.
Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are
met:

    * Redistributions of source code must retain the above copyright
      notice, this list of conditions and the following disclaimer.
    * Redistributions in binary form must reproduce the above
      copyright notice, this list of conditions and the following
      disclaimer in the documentation and/or other materials provided
      with the distribution.
    * Neither the name of Google Inc. nor the names of its
      contributors may be used to endorse or promote products derived
      from this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
"AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

## Runtime Portable Endian Header

The public-domain dedication and jurisdictional fallback notice at the start of
`tree-sitter 0.25.10/src/portable/endian.h` are retained verbatim below. This is
the header author's notice, not a newly supplied BSD/MIT/Apache license text.

```text
// "License": Public Domain
// I, Mathias Panzenböck, place this file hereby into the public domain. Use it at your own risk for whatever you like.
// In case there are jurisdictions that don't support putting things in the public domain you can also consider it to
// be "dual licensed" under the BSD, MIT and Apache licenses, if you want to. This code is trivial anyway. Consider it
// an example on how to get the endian conversion functions on different platforms.

// updates from https://github.com/mikepb/endian.h/issues/4
```

## Streaming Iterator: MIT

Full `streaming-iterator 0.1.9/LICENSE-MIT`. This collection uses the MIT option
of the crate's MIT OR Apache-2.0 licensing.

```text
Copyright (c) 2016 Steven Fackler

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Regex Family: MIT

The full `LICENSE-MIT` text below is identical in regex 1.13.1,
regex-automata 0.4.18, and regex-syntax 0.8.11. This collection uses the MIT
option of their MIT OR Apache-2.0 licensing. These are selected parser
dependencies, not an exhaustive review of the regex dependency graph.

```text
Copyright (c) 2014 The Rust Project Developers

Permission is hereby granted, free of charge, to any
person obtaining a copy of this software and associated
documentation files (the "Software"), to deal in the
Software without restriction, including without
limitation the rights to use, copy, modify, merge,
publish, distribute, sublicense, and/or sell copies of
the Software, and to permit persons to whom the Software
is furnished to do so, subject to the following
conditions:

The above copyright notice and this permission notice
shall be included in all copies or substantial portions
of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
DEALINGS IN THE SOFTWARE.
```

## Regex Syntax Unicode Tables

Full `regex-syntax 0.8.11/src/unicode_tables/LICENSE-UNICODE`:

```text
UNICODE, INC. LICENSE AGREEMENT - DATA FILES AND SOFTWARE

Unicode Data Files include all data files under the directories
http://www.unicode.org/Public/, http://www.unicode.org/reports/,
http://www.unicode.org/cldr/data/, http://source.icu-project.org/repos/icu/, and
http://www.unicode.org/utility/trac/browser/.

Unicode Data Files do not include PDF online code charts under the
directory http://www.unicode.org/Public/.

Software includes any source code published in the Unicode Standard
or under the directories
http://www.unicode.org/Public/, http://www.unicode.org/reports/,
http://www.unicode.org/cldr/data/, http://source.icu-project.org/repos/icu/, and
http://www.unicode.org/utility/trac/browser/.

NOTICE TO USER: Carefully read the following legal agreement.
BY DOWNLOADING, INSTALLING, COPYING OR OTHERWISE USING UNICODE INC.'S
DATA FILES ("DATA FILES"), AND/OR SOFTWARE ("SOFTWARE"),
YOU UNEQUIVOCALLY ACCEPT, AND AGREE TO BE BOUND BY, ALL OF THE
TERMS AND CONDITIONS OF THIS AGREEMENT.
IF YOU DO NOT AGREE, DO NOT DOWNLOAD, INSTALL, COPY, DISTRIBUTE OR USE
THE DATA FILES OR SOFTWARE.

COPYRIGHT AND PERMISSION NOTICE

Copyright © 1991-2018 Unicode, Inc. All rights reserved.
Distributed under the Terms of Use in http://www.unicode.org/copyright.html.

Permission is hereby granted, free of charge, to any person obtaining
a copy of the Unicode data files and any associated documentation
(the "Data Files") or Unicode software and any associated documentation
(the "Software") to deal in the Data Files or Software
without restriction, including without limitation the rights to use,
copy, modify, merge, publish, distribute, and/or sell copies of
the Data Files or Software, and to permit persons to whom the Data Files
or Software are furnished to do so, provided that either
(a) this copyright and permission notice appear with all copies
of the Data Files or Software, or
(b) this copyright and permission notice appear in associated
Documentation.

THE DATA FILES AND SOFTWARE ARE PROVIDED "AS IS", WITHOUT WARRANTY OF
ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE
WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
NONINFRINGEMENT OF THIRD PARTY RIGHTS.
IN NO EVENT SHALL THE COPYRIGHT HOLDER OR HOLDERS INCLUDED IN THIS
NOTICE BE LIABLE FOR ANY CLAIM, OR ANY SPECIAL INDIRECT OR CONSEQUENTIAL
DAMAGES, OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE,
DATA OR PROFITS, WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER
TORTIOUS ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE OR
PERFORMANCE OF THE DATA FILES OR SOFTWARE.

Except as contained in this notice, the name of a copyright holder
shall not be used in advertising or otherwise to promote the sale,
use or other dealings in these Data Files or Software without prior
written authorization of the copyright holder.
```

## Unsafe Libyaml Rust Translation: MIT

Full `unsafe-libyaml 0.2.11/LICENSE-MIT`, from translation revision
`a7b8d1fbd93aefbca3003dcb5fcc6a9c2297e968`. The crate's README describes a
C-to-unsafe-Rust translation of libyaml with c2rust assistance. This bundled
license has no copyright line; it is preserved as supplied, not treated as a
replacement for the original libyaml authors' notice in the next section.

```text
Permission is hereby granted, free of charge, to any
person obtaining a copy of this software and associated
documentation files (the "Software"), to deal in the
Software without restriction, including without
limitation the rights to use, copy, modify, merge,
publish, distribute, sublicense, and/or sell copies of
the Software, and to permit persons to whom the Software
is furnished to do so, subject to the following
conditions:

The above copyright notice and this permission notice
shall be included in all copies or substantial portions
of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
DEALINGS IN THE SOFTWARE.
```

## Translated Libyaml Upstream: MIT

Full upstream `License` at the source revision pinned by the translation
crate's README:
https://raw.githubusercontent.com/yaml/libyaml/2c891fc7a770e8ba2fec34fc6b545c672beb37e6/License

```text
Copyright (c) 2017-2020 Ingy döt Net
Copyright (c) 2006-2016 Kirill Simonov

Permission is hereby granted, free of charge, to any person obtaining a copy of
this software and associated documentation files (the "Software"), to deal in
the Software without restriction, including without limitation the rights to
use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies
of the Software, and to permit persons to whom the Software is furnished to do
so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Python Grammar and Scanner: MIT

Full bundled `tree-sitter-python 0.25.0/LICENSE`, also available at
https://raw.githubusercontent.com/tree-sitter/tree-sitter-python/293fdc02038ee2bf0e2e206711b69c90ac0d413f/LICENSE:

The exact normal dependency pin is `tree-sitter-python = "=0.25.0"`.
The crate's `.cargo_vcs_info.json` records the repository-root revision above.
The crates.io archive checksum (SHA-256), from public registry metadata, is
`6bf85fd39652e740bf60f46f4cda9492c3a9ad75880575bf14960f775cb74a1c`.

The reviewed crate manifest declares MIT licensing. Its `bindings/rust/build.rs`
uses `cc` with `.std("c11")` to compile `src/parser.c` (generated by tree-sitter
v0.25.9) and `src/scanner.c` into native code. The scanner includes the bundled
`src/tree_sitter/array.h` and `parser.h`; `array.h` includes `alloc.h`.
These three headers and the parser/scanner contain no additional copyright or
license notices. Their other includes are standard C headers. The existing
runtime MIT, Unicode and portable endian notices above remain retained.
This review is limited to this crate's manifest, Rust binding/build sources,
license, revision metadata, and native parser/scanner/header notice scope;
it is not a whole-binary or toolchain dependency review.

```text
The MIT License (MIT)

Copyright (c) 2016 Max Brunsfeld

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
