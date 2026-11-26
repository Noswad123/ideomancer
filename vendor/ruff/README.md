# Reviewed Ruff Source Snapshot

This Git-distributed subset is derived from Ruff tag `0.12.0`, commit
`87f0feb21a1af67195bebf94d811770393f9f447`. It is not a registry publication,
submodule, nested Git repository or independent Cargo workspace.

Only 65 selected Rust source files from five unpublished crates are included.
Fixtures, snapshots, examples, benches, test resources and the standalone
`ruff_python_parser/src/parser/tests.rs` are omitted. Mixed production/test files
remain intact. Adapted manifests disable library tests, doctests, benchmarks and
automatic targets; the upstream `#[cfg(test)] mod tests` declaration is unchanged.
The checked-in AST `src/generated.rs` is a required source input; regeneration is
not part of a build. No local crate needs a build script or external generated input.

## Maintenance And Integrity

`upstream/` preserves the original workspace manifest, lockfile and five crate
manifests as provenance, not ancestor Cargo configuration. `LICENSE` is the complete
upstream license, including RustPython attribution. `provenance.json` records the
immutable pin, retrieval hashes, pristine source hashes and closed file inventory.
`patches/ast-only-policy.patch` is the only source change. Adapted manifests are
reviewed local changes, not upstream originals.

Run `python3.12 -B scripts/check_ruff_vendor.py` from the repository root for
read-only integrity, path, metadata, manifest and policy checks. The verifier
anchors the reviewed inventory and pristine hashes in script constants. It checks
the committed snapshot, not GitHub or archive retrieval again. This is a review
and accidental-tampering guard, not an attacker-controlled-checkout sandbox.
Any intentional update requires review of the source pin, inventories, checker
anchors, manifest assumptions, notices and policy diff together.

## Dependency Policy

The crates are edition 2024, declared MSRV 1.85, unpublished version 0.0.0,
without optional features or dev dependencies. Local edges are sibling paths.
Original upstream registry requirements are retained for dependencies shared with
the product, allowing its existing selections rather than downgrading them.
Unshared dependencies are pinned to the inventoried upstream lock versions.
The root product lockfile determines the actual merged registry closure; the
retained upstream lockfile is provenance, not a claim about that resolution.
In particular newer product proc-macro dependencies must not be forced to the
older probe versions. Different semver compatibility lines may resolve in parallel;
renaming does not guarantee coexistence of incompatible exact pins within one line.

Unused upstream optional feature names are recognized by `unexpected_cfgs`, not
advertised as nonfunctional Cargo features. No Salsa Git dependency, cache/macros
crate, optional serialization/schema feature or upstream dev dependency is enabled.

## Narrow Parser Policy

`ParseOptions::with_ast_only(true)` only omits duplicate call/class-header keyword
validation before its diagnostic is emitted. It defaults to false in both option
constructors. It is not a general Python AST-versus-compiler semantic mode.
Generator, argument-order and other syntax validation remain unchanged.

Consumers must select the intended Python version and require BOTH `errors()` and
`unsupported_syntax_errors()` to be empty before inspecting a tree, equivalently
`has_no_syntax_errors()`. `parse(...).is_ok()` alone does not check the separate
unsupported-version channel. No diagnostic filtering or recovery-tree admission
is authorized by this patch.

## Notices And Compatibility

`LICENSE` retains Charles Marsh's MIT terms and the RustPython Team 2020 MIT
notice, as well as upstream's other attributions. Baseline `unicode_names2 1.3.0`
notices are preserved under `notices/`; this pinned registry crate embeds Unicode
16 name/alias data. PY312 targeting does not make those tables Unicode 15.
`unicode-ident` is deliberately not exact-pinned: preserve the applicable Unicode
notice for the ACTUAL root-selected version when distributing binaries. Registry
sources remain external Cargo dependencies, not copied source in this snapshot.

Declared MSRV/license screening is not proof of lower-MSRV, platform, grammar,
Unicode, decoding or fact compatibility, or legal clearance. This snapshot alone
does not switch the production Python backend or establish replacement parity.
