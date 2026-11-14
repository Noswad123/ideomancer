# Release Review And Package Boundary

The current engine API is local development work, not a published immutable
dependency. Passing the checks below does not authorize publication or certify
source ownership, security, framework semantics, or historical clearance.

## Review The Actual Source Package

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
CI repeats the inventory check, builds the packaged crate, and compares the actual
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

## Validate Reuse From The Package

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
Python/Rust Tree-sitter and Rust source analysis are not delivered by this migration.

Native parser code, schemas, CLI migration errors, UTF-16 receipts, incomplete
module-only parse handling and cooperative resource controls must be validated
together. Local closeout passed the normal suite, packaging, independent consumer
and release build on macOS with Rust 1.89.0 and 1.96.1; see the
[dated validation record](javascript-typescript-analysis.md#validation-tooling).
Hosted CI now defines Linux/macOS × Rust 1.89.0/current-stable lanes, each including
the independent consumer; those runs remain pending. An installed 1.96.1 run is not
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
