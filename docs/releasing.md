# Release Review And Git Delivery Boundary

The current engine API is local development work, not a published immutable
dependency. Passing the checks below does not authorize publication or certify
source ownership, security, framework semantics, or historical clearance.

## Overview

The approved personal/company delivery model is Git source, with no registry or
crates.io publication requirement. Root `Cargo.toml` sets `publish = false` and
depends on three local Ruff crates under `vendor/ruff/crates`; their closure is
five source crates. All five, upstream notices and the narrow policy patch travel
with the Ideomancer checkout. No external fork, submodule, published Ruff version
or separately maintained unpublished registry-version family is required.
The reviewed source closure also carries Unicode-15 raw inputs, derived tables,
manifest, provenance and their permission notice under
`src/analysis/python_ruff/unicode15/`; keep them together in Git delivery.

## API / Interfaces

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
   manifests, notices, exact hashes and four-source-file policy-patch reversal.
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
of production's trusted Python >=3.12 contract. For current reported results,
the tested consumer snapshot hash and guarded cache-cleanup record, see the
[Python ledger](python-analysis.md#current-ruff-compatibility-batch-frontend-2--private-facts-4).

The final product formatting, all eight type-comment tests (including the 787-case
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

Keep third-party Rust source pristine except for the reviewed four-source-file
policy patch in `options.rs`, `expression.rs`, `pattern.rs` and `statement.rs`.
AST-only mode admits duplicate keywords and bare f-string mapping keys; default
mode retains rejection. TypeVarTuple/ParamSpec defaults receive the same version
guard as TypeVar independently of AST-only mode (PY312 errors, PY313 accepted).
Adapted manifests disable optional features and upstream tests;
format only the product with
`RUSTUP_AUTO_INSTALL=0 cargo fmt --package ideomancer --check`, never `--all`.
Ruff syntax acceptance alone proves neither CPython compatibility nor semantic facts;
the separate private facts-4 lowering is implemented, not wholesale pending, but
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
