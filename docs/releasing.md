# Release Review And Git Delivery Boundary

The current engine API is local development work, not a published immutable
dependency. Passing the checks below does not authorize publication or certify
source ownership, security, framework semantics, or historical clearance.

## Overview

The approved personal/company delivery model is Git source, with no registry or
crates.io publication requirement. Root `Cargo.toml` sets `publish = false` and
depends on three local Ruff crates under `vendor/ruff/crates`; their closure is
five source crates. All five, upstream notices and two bounded patches travel
with the Ideomancer checkout. No external fork, submodule, published Ruff version
or separately maintained unpublished registry-version family is required.
The reviewed source closure also carries Unicode-15 raw inputs, derived tables,
manifest, provenance and their permission notice under
`src/analysis/python_ruff/unicode15/`; keep them together in Git delivery.

## API / Interfaces

### Current F1 experimental source closure

The authorized private F1 lexer shares one fallible byte core with legacy text
lexing; it is not a public backend, byte parser or production migration. Frontend-5/
facts-7 names and manifests/dependencies remain unchanged; implementation identity
changes transitively through schema-2 vendor provenance. Actual pre-F1 HEAD was
user-reported `fdf4e58` (facts-6 commit, initially clean/ahead one), not `523cd7b`.
Current F1 changes are uncommitted and unstaged. All earlier HEAD, consumer and
cleanup receipts below are historical.

Current source inventory: **261 paths**, five additions to F0's 256: two root
ordinary test modules, [frozen baseline](python-byte-lexer-baseline.json), the
vendor lexer patch and the authored byte facade. Vendor: **87 files / 66 Rust
sources / 1,972,891 bytes**. The strict schema-2 closure retains the unchanged
65-source pristine map: eight modified upstream files, 57 unchanged, plus one
original MIT `lexer/byte.rs` separately hashed as authored code. Two disjoint
four-file patches reverse independently: unchanged `ast-only-policy.patch` and
`byte-lexer-experiment.patch` (`lexer.rs`, `lexer/{cursor,indentation,interpolated_string}.rs`).

SHA-256 anchors:

- Provenance: `83b701ef82cd549d1b01501520f57b9504e3decac32cdc7d151add639f892800`.
- F1 patch: `4f4d3a244236a3e730ab805716f23f581483fa6e7a38060b1ea10c344fe5aac5`.
- Authored facade: `d8e617b7bb9401e96263997b99151cfa02e3dd0af1254a384c7569e12d2fec94`.

Retain root MIT and Ruff licenses and existing PSF/Unicode notices unchanged.
Coordinator-reported current validation: **all 131 private Ruff tests passed**
after fixes, including **23 F1 ordinary + 17 existing opt-ins**. All **364** complete
positive facts/IDs/diagnostics/raw replay matched on the unchanged normal text
path. Both facades passed all thirteen frozen pre-core hashes (298 tokens/all four
modes), and legacy `Send + Sync` compiled/passed in the baseline test. Product
formatting/all-target Clippy (`--no-deps -- -D warnings`), **58 vendor script tests**,
schema-2 checker and **261-path** source gate passed. Scripts: **310 discovered,
297 passed / 13 isolation skips**; isolated byte/visitor/codec **23 / 43 / 24 passed**,
no skips, together covering all 310 unique tests across lanes. Existing
CI ordinary root tests select the new modules without a public backend selector.
Current preservation passed **708 library tests (191 ignored), 18 adapter + 8
engine** and explicit **165 reference + 3 native** tests. With the 17 Ruff opt-ins,
**all 185 explicit gates passed**, including known-limit assertions—not 185 parity
matches. P2 text-view accounting is closed and its root regression passed; final
read-only review found no material P1/P2/P3 findings and new schema-2 guards passed.
No additional core changes are planned and provenance anchors are stable.
**Bounded F1 lexer goal COMPLETE / PROVED by compiled tests; all delivery gates PASS.**
The offline checkout consumer passed fresh **105-package** lock resolution,
**one native TypeScript public-API test**, five-Ruff-crate containment, ABI **0.1.5**
and source-hash rechecks. Its tested source snapshot is
`2cea3f996bf4a0e0cbbfd242b681331fb2208a3f0e6ac1fba2e4e81472869c6e`,
not an actual Git revision or final-docs attestation. Its owned cache `TMPDIR`
was cleaned, with none left. Guarded preflight found zero blockers/protected
outputs, normal `debug`/`tmp` only. Cleanup removed **1,360,498,688 estimated
allocated bytes (~1.27 GiB)**; target absent, follow-up `already_clean`, **no Cargo
afterward**. No owned debug/test artifacts remain; root cache parent empty,
`.ideomancer/` JS artifacts/global caches untouched. Final source/Git/helper checks
are read-only, without rebuilding. No commits or pushes were made. This is not release
approval or proof of byte parser input, AST/debug construction, CPython admission,
F2 or full G4; it does not authorize further implementation. F2 parser/source/
debug/admission/lazy-error precedence and version/product policy still need
separate approval. See the
[F1 ledger](python-analysis.md#f1-shared-byte-lexer-experiment) and
[ADR](python-native-replacement-feasibility.md).

### Historical visitor source closure

Frontend `python-ruff-private-5-py312-visitor-patterns` / facts policy
`python-ruff-private-facts-7` has a **253-path** Git source inventory, five additions
to the codec checkpoint's 248: [contract](python-visitor-contract.json),
[witnesses](python-visitor-witnesses.json), `scripts/check_python_visitor_contract.py`,
`scripts/test_python_visitor_contract.py` and
`src/analysis/python_parity/ruff_visitor_batch.rs`. Review the inventory and complete
helper/Ruff output/raw-replay oracle together; the script does not prove ordering.
CI adds checker/tests with `-I -S -B` on trusted exact CPython 3.12.11.

At the visitor checkpoint the maintained patch touched four source files, with the
same 65-source pristine closure; vendor totals were **85 files / 65 sources / 1,882,473
bytes**. Patch SHA-256:
`e79a66530f58f63371c0423f5f0469ff82f9b0193f57b6ba8672c3c33aa4e90e`;
provenance anchor:
`b8246ec2c0911719ddd715f80dac504795c97f70ab828bd4375d18538c237ee0`.
No dependency or notice/license changes accompany the visitor batch. Existing
codec/Unicode/Ruff notices remain required.

After both parser fixes, **108 Ruff tests passed, including 17 opt-in tests**;
**344 preserved + 20 new = 364** positives matched complete arrays/IDs/diagnostics/
raw replay. Prior failure/Unicode/codec and 787-outcome contextual oracles passed.
Product formatting and all-target Clippy (`--no-deps -- -D warnings`) passed.
Scripts: **263 discovered, 257 passed, six isolation-required skips**; isolated
visitor **43** and codec **24** passed without skips, covering **all 263 unique
tests across lanes**, not 263 discovery-lane passes. Checker, three visitor tests,
both-mode/both-constructor pattern controls, **34 vendor script tests**, updated
vendor integrity and **253-path source integrity** passed.

Preservation passed **685 library tests (191 ignored)**, **18 adapter + 8 engine**
and **165 reference + 3 native** opt-ins. With **17 Ruff opt-ins**, **185 explicit
gates passed**, including known-limit assertions—not all parity matches.
**G2/G3 PRIVATE IMPLEMENTATION COMPLETE/CLOSED for current admitted Python 3.12
Module-AST lane: closed class/field inventory + full-output/raw-replay coverage;
not complete Python compatibility, G4 bytes/admission/failure/resource/version
equivalence, or production cutover.** G5/G6 remain open. **Current local macOS /
Rust 1.89 delivery closeout passed**: offline consumer fresh **105-package** lock,
**one native TypeScript public-API test**, five contained Ruff crates, ABI **0.1.5**
and source-hash rechecks. Its tested content snapshot is
`0e71afaa639a38f3ca0022726dc24bb24257eee8a01845ac7f36900fe4471f60`,
not an actual Git revision or later-documentation byte attestation.
No further code changes are planned. Actual differences are narrow
parser predicates/separators and proof files, not a visitor/normalizer rewrite.
Singleton-sequence controls honor pre-existing singleton-first acceptance in both
modes/constructors; AST-only predicate admission adds class-first and later-sequence
children, not singleton-first elements. The independent
real missing-separator controls now pass in both modes/constructors, not weakened
tests. The configured unique owned cache `TMPDIR` was cleaned, none left. Guarded
preflight found **zero blockers / zero protected outputs**, normal `debug`/`tmp`
only; cleanup removed **1,374,564,352 estimated allocated bytes (~1.28 GiB)**.
`target/` was absent, follow-up `already_clean`, with no Cargo afterward. No owned
debug/test artifacts remain; `.ideomancer/` JS artifacts/global caches were untouched.
Source **253**, vendor **85/65/1,882,473 bytes** and Git diff checks passed before
final documentation edits. Final source/Git/helper checks are read-only, no rebuild.
The facts-6 ~1.28 GiB receipt below is historical, as are prior consumer receipts.
No staging, commit or push was performed; all work remains uncommitted at actual
HEAD `523cd7b`. These are local passes, not hosted/platform or full production approval.

**Historical STOP at visitor handoff (superseded for F0 and bounded F1 only).**
At that checkpoint G4 required a user choice:
preserve the full replacement contract and explicitly authorize the recommended
bounded byte-aware Ruff feasibility/version-strategy investigation (not a rewrite), or approve a separate limited native
capability retaining the helper. “Continue” chooses neither. Unchanged Ruff `&str`
cannot cover CPython-admitted invalid UTF-8 comment bytes, and native PY312 versus
production >=3.12 needs a decision: how will a fixed native target preserve that
production version contract? Contextual for/with/class suite type-comment errors
can be `IndentationError`; existing syntax status/line fields intentionally keep
`failure: None`, not placeholder certificates. Another PEG parser alone cannot
close arbitrary type-comment failure-location/class parity. Production helper/API/default remain unchanged;
neither private G2/G3 closure nor source distribution is G4 or cutover approval.
See the [current ledger](python-analysis.md#ruff-visitor-contract-batch-frontend-5--private-facts-7).

### Historical codec source closure

Frontend-4/facts-6 has a 248-path Git source inventory (11 additions to the prior
237). Codec data, scripts, manifest, provenance, NOTICES and LICENSE are private
fingerprint inputs. No dependency or vendor-patch change accompanies this batch.
Generation/checking requires exact trusted installed CPython 3.12.11 / Unicode
15.0.0, isolated/no-site mode (`-I -S -B`) and matching stdlib source hashes. The scripts
`generate_python_codecs312.py --check`, `check_python_codecs312.py` and
`test_python_codecs312.py` under `scripts/` provide offline reproduction/profile
gates; CI explicitly runs all three, including the isolated codec suite, with
`-I -S -B`. These exact oracle pins do not narrow production Python >=3.12. No package installation
or download is required. Native Rust decoding needs no Python runtime.

Review the structural stateless charmap/wiring audit and 123-file installed stdlib
source closure together; these do not attest native binaries or independently
prove byte-identical upstream source. Retain both codec
[`NOTICES.md`](../src/analysis/python_ruff/codecs312/NOTICES.md) and full
[`LICENSE.txt`](../src/analysis/python_ruff/codecs312/LICENSE.txt) in Git and
applicable binary distributions, alongside existing Unicode and Ruff notices.
The 13,936-byte license matches raw upstream v3.12.11 SHA-256
`3b2f81fe21d181c499c59a256c8e1968455d6689d269aa85373bfb6af41da3bf`.
It contains the full PSF and historical stack, not an MIT relicensing. NOTICES
retains all 71 codec/aliases module headers and legal mapping comments; raw stdlib
modules are not embedded. This scoped retention is not whole-binary legal clearance.

All **104 Ruff tests passed, including 16 opt-in tests**, with **232 preserved +
112 new = 344** complete positive fact/raw-order cases and preserved failure/
contextual/Unicode/source/HTTP gates passing. Product package formatting and
all-target Clippy (`--no-deps -- -D warnings`) passed, as did 248-path source and
vendor 85-file / 65-source / 1,881,065-byte checks. Global script discovery:
**216 tests, 211 passed, five skipped**; explicit isolated codec suite: **24 passed,
none skipped**. These lanes cover all 216 unique script tests, not 216 default
passes. Generator check and independent checker (`license_verified: true`) passed
with `-I -S -B`; CI uses the same flags. Local macOS / Rust 1.89 validation does
not establish hosted results, which remain unknown. Broader results passed:
**682 library tests / 190 ignored**, **18 adapter + 8 engine**, and **165
reference + 3 native** explicit opt-in tests. With 16 Ruff opt-ins, **184 explicit
tests** passed, including known-limit assertions—not all parity matches. Both
full 104-Ruff and 682-library suites passed after the test fixes, with no engine
bug found. Product formatting passed before the current offline consumer, which
passed fresh 105-package lock resolution and one native TypeScript public-API
test, five Ruff crates contained, ABI 0.1.5 and source-hash rechecks. Its tested
content snapshot is
`74e8525de0cb80679435734c0c0ab6f374c2c6818f2008bd67a0fb5064799dc1`,
not an actual Git revision or attestation of later documentation bytes. The
248-path source check matched that snapshot before final documentation edits;
Git whitespace checks passed.

The consumer's unique owned cache `TMPDIR` was cleaned. Guarded preflight found
zero active blockers/protected outputs and normal `debug`/`tmp` entries; apply
removed **1,372,758,016 estimated allocated bytes (~1.28 GiB)**. `target/` was
absent, follow-up reported `already_clean`, and no Cargo ran afterward. The owned
cache parent was empty, with no retained debug/test artifacts; `.ideomancer/` JS
artifacts and global caches were untouched. Final source/Git/cache verification
is read-only, without rebuilding.
All codec-family and prior facts-5 work remains uncommitted at actual HEAD
`523cd7b`; historical facts-5 consumer/cleanup receipts remain separate. See the
[current ledger](python-analysis.md#ruff-single-byte-codec-batch-frontend-4--private-facts-6).

### Consumer seam

Consumers use the existing `ideomancer_core::engine::v1` facade. Start with a local
path; after an explicitly authorized commit is available, pin its **full actual
Git revision** from the approved upstream or internal mirror. A working-tree
snapshot receipt does not attest an actual Git commit or prove remote availability.

The active source contract is [source-files.txt](source-files.txt), not the legacy
Cargo include list. `check_checkout.py` checks the exact reviewed source inventory
and hashes its contents. Its export excludes `.git`, `.ideomancer`, `.agents`,
`target/` and private material; this is not a secrets/IP scanner or clearance of
everything that may be tracked in Git. Review the actual authorized revision too.

## Data Flow

1. Review source additions and inventory changes rather than regenerating blindly.
2. Check the source closure and `check_ruff_vendor.py`'s anchored provenance,
   manifests, notices, exact hashes, both disjoint four-file patch reversals and
   the separate authored-source hash.
   Run `check_python_unicode15.py` to verify independent official-input/table
   hashes, reproducibility and exhaustive Unicode oracle checks; review data
   provenance and permission notices with the source inventory.
3. `check_checkout_consumer.py` exports only reviewed source into an owned external
   temporary run. It resolves a fresh independent consumer lockfile, then runs the
   locked public-API consumer tests with all local dependencies inside the export's
   `vendor/ruff`. Root product dependency selections are not replaced by the
   retained upstream provenance lockfile.
4. It rechecks original/exported content hashes after validation. This establishes
   bounded snapshot integrity, not authenticity or continuous filesystem isolation.
5. Record actual results, tool versions, limitations and the eventual approved pin.
   Hosted Linux/macOS × Rust 1.89.0/current-stable results remain unknown for this
   change; historical package acceptance is not proof of the new Git closure.

## Example usage

These are gate commands for the coordinating maintainer, not automatic release
approval. Record actual completed and pending results in the Python ledger:

```sh
python3.12 -B -m unittest discover -s scripts -p 'test_*.py'
python3.12 -B scripts/check_checkout.py --source-dir .
python3.12 -B scripts/check_ruff_vendor.py
python3.12 -B scripts/check_python_unicode15.py
RUSTUP_AUTO_INSTALL=0 python3.12 -B scripts/check_checkout_consumer.py \
  --source-dir . --target-dir target --offline
```

The Unicode checker requires the installed trusted exact **CPython 3.12.11 /
Unicode 15.0.0** profile used by CI. Reproduce tables offline with
`python3.12 -B scripts/generate_python_unicode15.py` (no `--fetch`); do not refresh
inputs or notices implicitly. This is an oracle/data requirement, not a narrowing
of production's trusted Python >=3.12 contract. For historical facts-4 results,
the tested consumer snapshot hash and guarded cache-cleanup record, see the
[Python ledger](python-analysis.md#current-ruff-compatibility-batch-frontend-2--private-facts-4).

At the historical facts-4 checkpoint, final product formatting, all eight type-comment tests (including the 787-case
oracle) and all-target product Clippy reruns passed after the equivalent boolean
simplification. The exported-checkout consumer passed offline fresh 105-package
lock resolution and one full independent native TypeScript public-API test, with
all five Ruff crates contained, ABI 0.1.5 verified and source hashes rechecked.
The explicitly configured unique owned cache scratch was cleaned. This validates
the recorded source snapshot, not an actual Git commit or the later documentation
edits, and does not constitute release/cutover approval.

Configure a verified disk-backed `TMPDIR` under an owned unique run in
`${XDG_CACHE_HOME:-$HOME/.cache}/agent-test-runs` before substantial work, check
space and establish cleanup. The consumer helper defaults to that cache parent
when `TMPDIR` is unset, reuses only the trusted root `target/`, enforces low-disk
profiles and two jobs/test threads, and rejects conflicting build overrides.
It stops/verifies owned process groups even after a successful parent exits before
removing temporary files. Unverified shutdown or cleanup failure retains the owned
run and reports its path and cleanup plan on stderr; diagnostics can contain paths.
No compiler/toolchain installation is authorized by these commands. Use the existing
active compiler (local baseline 1.89); do not infer an installed toolchain alias.

Example consumer manifest entries (choose one; replace placeholders, not a known pin):

```toml
[dependencies]
ideomancer_core = { package = "ideomancer", path = "../ideomancer" }
# After authorization and availability of the actual commit, replace the path with:
# ideomancer_core = { package = "ideomancer", git = "<approved-upstream-or-mirror-url>", rev = "<full-actual-commit-revision>" }
```

## Edge Cases

`cargo package` cannot provide a usable registry crate with the current unpublished
path dependencies. The archive helpers and inventory below remain historical tools
and unit-test coverage, **not active delivery gates**. Do not run archive builds as
a substitute for checkout-consumer acceptance, or claim CI still runs them.

Keep upstream Rust source pristine except for the two reviewed disjoint four-file
patches above; the original byte facade is separately authored, not upstream source.
The unchanged policy patch covers `options.rs`, `expression.rs`, `pattern.rs` and `statement.rs`.
AST-only mode admits duplicate keywords and adds f-string mapping/pattern-start
admission; the option defaults to false in both constructors. Valid default-mode
forms remain unchanged, including upstream singleton f-string first-element
acceptance. The independent sequence-separator correction requires a comma before
closing-token lookahead in both modes: previously erroneously accepted malformed
default-mode sequences now reject. TypeVarTuple/ParamSpec defaults receive the same version
guard as TypeVar independently of AST-only mode (PY312 errors, PY313 accepted).
Adapted manifests disable optional features and upstream tests;
format only the product with
`RUSTUP_AUTO_INSTALL=0 cargo fmt --package ideomancer --check`, never `--all`.
Ruff syntax acceptance alone proves neither CPython compatibility nor semantic facts;
the separate private facts-7 lowering is implemented, not wholesale pending, but
full CPython compatibility, public integration and production cutover remain open.
Known-difference assertion passes are not parity matches. Production Python still
uses the trusted >=3.12 AST helper.

Personal/company scope does not permit private fixtures, identifiers or company
paths in generic source. Existing history, ownership, source privacy, license,
security and supported-platform gates below still apply. Binary bundles must retain
`LICENSE`, `vendor/ruff/LICENSE`, vendored notices, the Unicode data/derived-table
permission notice at `src/analysis/python_ruff/unicode15/inputs/LICENSE.txt`, and
applicable dependency/toolchain notices after actual target/feature review. Preserve
the Unicode manifest/provenance with source data and do not silently refresh its
hashed notice from a mutable URL. `make install` copies only the executable,
not a license-cleared release bundle. Remote commits, pushes, releases and any
publication still require explicit authorization.

## Related files

- [Development workflow](development-workflow.md): gate selection and cleanup last.
- [Third-party notices](third-party-notices.md), [Ruff maintenance](../vendor/ruff/README.md).
- `scripts/check_checkout.py`, `scripts/check_checkout_consumer.py`,
  `scripts/check_ruff_vendor.py`, `scripts/check_python_unicode15.py`, and
  `docs/source-files.txt`: active source/data gates.
- `src/analysis/python_ruff/unicode15/PROVENANCE.md`: data inputs, notice and generation profile.
- [Readiness review](release-readiness.md): dated historical evidence and open approvals.

## Historical Registry-Package Archive Procedure (Inactive)

The following archive procedure and JS/TS validation record are retained for
historical context and legacy helper tests. They do not describe current CI or
establish a deliverable for the current vendored path-dependency manifest.

### Review The Actual Source Package

`Cargo.toml` uses a root-anchored positive include policy. Cargo include patterns
can select Git-ignored files, and unanchored names can match nested study files;
`.gitignore` alone is not a package boundary. A regression test requires anchoring.
The exact expected Cargo file list is
reviewed in [package-files.txt](package-files.txt); the check rejects unexpected,
missing, duplicate, noncanonical, and known private/runtime paths. Cargo-generated
`.cargo_vcs_info.json` is optional, and `Cargo.toml.orig` is expected.

```sh
python3.12 -m unittest discover -s scripts -p 'test_*.py'
python3.12 scripts/check_package.py
cargo package --locked --offline --allow-dirty
python3.12 scripts/check_package_archive.py \
  --archive target/package/ideomancer-0.1.0.crate --source-dir . \
  --extracted-dir target/package/ideomancer-0.1.0
```

These commands never publish. `--allow-dirty` supports local candidate verification;
it is not a substitute for reviewing/checkpointing the exact intended release.
The historical CI procedure repeated the inventory check, built the packaged crate, and compared the actual
archive with intended source and extracted files. Review additions before updating
the inventory; never regenerate it blindly to make CI pass.

The archive checker reads the trusted inventory from the reviewing checkout, not
from the candidate archive. It rejects duplicate/unreviewed/missing paths, links,
special files, malformed metadata and oversized compressed/expanded contents.
It compares source bytes, checks this candidate's manifest normalization semantics
and compares extraction bytes; unarchived files outside a real `target/` subtree
are rejected. It never extracts. JSON stdout records the actual archive SHA-256,
file count, comparison flags and available VCS metadata, including the dirty bit.
Its POSIX Linux/macOS snapshot checks are not authenticity or ownership approval,
nor continuous isolation against a malicious concurrent filesystem.

The package intentionally omits local catalogs, root contract exports, unrelated
generated diagrams and unused artwork. Its inventory protects paths, not meaning:
approved paths may still contain inappropriate content. It is not a secrets/IP
scanner. The repository and its history need a separate review; packaging does not
control Git-hosted source archives, commits, prior releases or clones.

This [fresh baseline](lineage.md) excludes legacy artwork and generated root diagrams
from tracked source and packages, including the former README image. Existing local
copies remain ignored and are not deleted. Reintroducing assets requires confirmed
provenance, redistribution permission and inventory review; package policy is not
copyright clearance.

### Validate Reuse From The Package (Historical)

`cargo package` verifies the normalized package from its extracted source, including
the embedded language helpers and viewer assets. Also exercise an independent
consumer against that extracted package, not just the development checkout. Record
the actual package checksum, tool versions, API/claim tests, and skipped optional
integrations. This local check does not establish an upstream revision pin.

After `cargo package` succeeds, run the standalone synthetic native JS/TS consumer:

```sh
python3.12 scripts/check_package_consumer.py \
  --package-dir target/package/ideomancer-0.1.0 --offline --target-dir target
# Repeat with an explicitly installed second toolchain, for example:
python3.12 scripts/check_package_consumer.py \
  --package-dir target/package/ideomancer-0.1.0 --toolchain 1.96.1 --offline --target-dir target
```

The script creates a temporary consumer outside this repository, points its sole
direct dependency at the normalized extracted package, and resolves a **fresh**
consumer lockfile rather than copying this package's lock. It checks the exact ABI
bridge resolution, native discovery, typed facts, a synthetic configured relationship,
stale-claim rejection, saved claims, and source-deleted source-only diagram/cache
reuse. It provisions no runtime, renderer, agent, or Rust toolchain. `--offline` is
the default; `--no-offline` explicitly permits registry resolution (used in CI).
The optional target directory reuses build artifacts; omit it for disposable builds.

Run the script from the reviewing checkout, outside the candidate extraction. The
sibling `.crate` is now **required**, and the trusted inventory plus extracted-byte
comparison must pass before any tool runs. Immediately after successful consumer
tests it repeats archive/extraction checks and requires the same archive SHA-256.
JSON stdout records tool versions, outcome, verified checksum and extraction-match
status. Missing, changed or mismatched artifacts fail closed. These snapshot guards
are not signatures, ownership approval or continuous isolation. Cargo diagnostics
go to stderr and can contain local paths. Synthetic acceptance does not replace
approved real-consumer cases or establish a release pin.

Binary distribution bundles must retain `LICENSE` and applicable third-party
notices. The developer-local `make install` copies only the executable and is not
a complete release-bundle/notice-generation workflow.
The packaged [third-party notice collection](third-party-notices.md) retains the
pinned parser/runtime/grammar and selected transitive texts, including the complete
bundled Unicode/ICU and regex Unicode notices. Offline policy tests protect those
texts and their package inclusion. This is scoped retention, not a complete binary
dependency audit; do not distribute an executable-only bundle as a cleared release.

The native JS/TS migration pins Tree-sitter 0.25.10, JavaScript grammar 0.25.0,
and TypeScript/TSX grammar 0.23.2, plus a direct language ABI bridge pin at 0.1.5
to protect downstream Rust 1.89 resolution compatibility. Review the runtime,
Rust bindings, generated C parsers/scanners and transitive dependency licenses and retain their applicable
notices in binary distributions. Compiled-in parsing removes the discovery
Node/compiler prerequisite, not license obligations or the optional Mermaid
Node/Chromium renderer. Builds need a native C compiler. The declared MSRV is
Rust **1.89**; record actual validation for that baseline and current stable,
since current stable alone is insufficient. Python remains a trusted Python >=3.12
AST-helper path;
production native Python fact extraction and Rust source analysis were not
delivered by that migration. The current private Ruff frontend is described above.

Native parser code, schemas, CLI migration errors, UTF-16 receipts, incomplete
module-only parse handling and cooperative resource controls must be validated
together. Local closeout passed the normal suite, packaging, independent consumer
and release build on macOS with Rust 1.89.0 and 1.96.1; see the
[dated validation record](javascript-typescript-analysis.md#validation-tooling).
The current CI matrix retains Linux/macOS × Rust 1.89.0/current-stable lanes but uses
the checkout gates above, not these archive steps; those results remain unknown. An installed 1.96.1 run is not
proof of the current hosted stable lane. Local docs/CI edits are not proof of release
completion. The reviewed inventory includes `src/javascript_native.rs`
and `src/javascript_native/optional.rs`; the deleted legacy JavaScript helper is
no longer packaged. Recheck the exact inventory after any further module changes.
The earlier committed baseline did pass Linux/macOS stable CI on 2026-11-10;
that result does not validate these uncommitted closeout or workflow changes. See
the [bounded readiness review](release-readiness.md) for the exact run and outstanding
source, identity/history, consumer and binary-distribution decisions.

## Resolve Historical And Approval Gates

Prior lineages retain retired experimental interpretations. This source-only
baseline does not inherit those ancestors, but does not erase old objects, retained
refs, releases, clones, forks or hosted caches. Their disposition remains a separate
owner-approved task. Do not reconnect old refs as parents or import private history
to make this baseline appear continuous. This guide does not itself authorize any
commit, tag, rewrite, remote change or push.

Before an approved release:

- Review source/asset ownership, relevant history and previously distributed copies.
- Confirm direct/transitive dependency licenses and notices and tool provenance.
- Review updates to the pinned third-party CI action revisions and address or
  explicitly accept the deprecated `serde_yaml` dependency; deprecation alone is
  not a vulnerability finding. Floating toolchain/platform selectors intentionally
  exercise current stable and supported hosted images; record actual versions.
- Run the declared Linux/macOS toolchain matrix. A local macOS run is not that matrix.
- Distinguish static/offline checks from unavailable agent/provider integration.
- Publish only after authorization; then record a real immutable consumer pin.

See the [roadmap](../ROADMAP.md) and [migration/privacy limits](adapters.md).
