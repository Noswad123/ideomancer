# Source Release Readiness — 2026-11-13

**Current status (2026-11-25):** approved delivery is personal/company Git source,
not registry publication; root `publish = false`. The maintained five-crate Ruff
snapshot travels with the checkout. Active gates are the reviewed source inventory,
vendor integrity and independent checkout consumer in [releasing.md](releasing.md).
A dirty working-tree snapshot is not an attested Git revision; current Linux/macOS
× Rust 1.89.0/stable hosted results are not yet known. The archive evidence,
dependency counts and decisions below are historical, not current-closure proof.
Private Ruff syntax classification is not CPython compatibility or fact-backend
completion; production Python remains the trusted >=3.12 AST helper. No new
validation result or release authorization is recorded by this note.

This is a bounded engineering review of the JS/TS P0 source candidate, not legal,
ownership, security, framework, or publication approval. The dated baseline below
is historical; later scoped diagram and consumer acceptance is recorded in the
[roadmap](../ROADMAP.md), not inferred from the earlier hosted run. The immediate deliverable is a reviewed source
dependency; binary distribution is a separate, uncleared deliverable.

## Evidence And Decisions

| Gate | Observed evidence | Remaining action |
| --- | --- | --- |
| P0 implementation/local acceptance | Native JS/TS parser, regression tests, two installed Rust toolchains, optional non-agent checks and synthetic package consumer pass; see the [validation record](javascript-typescript-analysis.md#validation-tooling) | Preserve contracts; no additional parser feature work required for this closeout |
| Hosted baseline | [Run 42911651251](https://github.com/Noswad123/ideomancer/actions/runs/42911651251), 2026-11-10, succeeded on Linux/macOS for `4ca32a7a67b970caccfcfd4446f1022bec82957c` using the then-configured stable selector | It does **not** validate the uncommitted fixes, explicit MSRV lane, archive checks or independent consumer; run the expanded matrix after an authorized checkpoint/push |
| Current source/package boundary | Root-anchored includes, exact reviewed path inventory, actual archive/source/extraction comparison, and independent consumer checks | Recheck the final checkpoint/artifact; a working-tree candidate is not reproducible from current `HEAD` |
| Intended source content | Bounded static review found no concrete credential or hardcoded personal filesystem path in the intended current source | Owner still confirms source/asset provenance; this is not exhaustive secret/IP clearance |
| Git identity metadata | Current baseline commits retain organization-domain author/committer metadata | Owner explicitly approves retention or separately authorizes a remediation plan; no rewrite is implied |
| Legacy lineage | Current `main` has a parentless root; GitHub still lists a legacy backup branch | Owner decides disposition of retained refs and older distributed copies; do not delete or rewrite them implicitly |
| Published version | No tags or GitHub releases were listed at inspection | Publish only after approval and record a real immutable consumer pin |
| Real consumer | Synthetic integration plus a bounded real-source explicit-configuration slice and diagram usefulness accepted | Broader framework semantics, runtime behavior and adoption remain separate; do not export private fixtures or results |
| Binary notices | Scoped parser/selected dependency texts retained, including original libyaml attribution | Complete target/features/toolchain/native-runtime review and a notice-bearing bundle workflow before any binary distribution |
| YAML maintenance | Locked `serde_yaml 0.9.34+deprecated`; upstream says it is no longer maintained | Record owner risk acceptance or plan a separately tested replacement; deprecation alone is not a vulnerability finding |

Remote observations were read through GitHub at review time. Local tracking refs
alone would not establish the current remote state. No refs, repository settings,
visibility, releases, or history were changed. No private consumer source or ignored
local catalogs/artwork were inspected by this review.

## Actual Artifact Integrity

The path inventory is necessary but insufficient: it describes selected paths,
not the bytes in a previously created archive or a later-modified extraction.
The [release commands](releasing.md#review-the-actual-source-package) now check:

1. Cargo's actual path list against the trusted reviewer inventory.
2. The actual `.crate` archive's root, members, bounded sizes, regular-file types,
   normalized/original manifest semantics, and optional VCS metadata.
3. Intended source bytes against archive members, mapping `Cargo.toml.orig` to the
   original manifest and checking Cargo's generated normalization separately.
4. Extracted member bytes against that same archive, rejecting unarchived source
   and configuration files; only a real top-level `target/` build subtree may be extra.
5. Extraction/archive identity before **and after** the independent consumer run,
   retaining the same archive SHA-256. A missing sibling archive fails closed.

The checker uses bounded streaming reads and no extraction. Comparison reads reject
symlinks and nonregular files, including FIFO manifests. It is intentionally specific
to this package candidate and POSIX Linux/macOS; changes to package identity,
normalization, paths or resource limits require review. Receipts are integrity
evidence, not signatures, ownership approval or continuous filesystem isolation.

## Binary Dependency Review Scope

Offline locked graph inspection sampled Apple Darwin and GNU/musl Linux triples
on both architectures. These are audit samples, **not new supported artifact
commitments or cross-target build results**. The normal non-proc-macro graph has
49 dependencies per sampled target, with 50 distinct runtime crates in the union:
macOS selects `errno`, whereas sampled Linux targets select `linux-raw-sys`.
Twelve further crates are build/proc-macro-only in those samples. Host and runtime
feature instances are distinct; merged Cargo metadata is not proof of runtime
feature selection or of what survives linking.

The scoped [notices](third-party-notices.md) are not full graph coverage. Concrete
remaining review categories include:

- CLI/terminal, serialization, hashing, temporary-file and platform dependencies,
  including source-level attributions not present in top-level license files.
- License alternatives and conjunctive data terms: do not summarize everything
  as MIT. For example, `ryu` offers Apache/Boost alternatives, and host-tool
  `unicode-ident` additionally declares Unicode terms.
- Rust standard-library/runtime notices matching the actual build toolchain,
  including Unicode tables; these are not represented in the application lockfile.
- Actual native linkage and startup/unwind/C-runtime material for each chosen
  artifact. GNU, musl and macOS are not interchangeable distribution decisions.
- A bundle-content check ensuring the executable ships with project and applicable
  dependency/toolchain notices. Developer-local `make install` is not that workflow.

One concrete attribution gap was resolved during this review: `unsafe-libyaml`
translates libyaml, but its crate-level MIT text lacks the original copyright
lines. The retained collection now includes both that text and the full original
license at the libyaml revision named by its README. This narrow fix does not
establish whole-binary clearance. No OpenSSL or external SHA assembly dependency
was established by the inspected default hashing graph; do not invent obligations
for unselected optional components.

## Recommended Next Sequence

1. Owner decides identity/history handling and confirms current-source provenance.
2. Review and explicitly authorize checkpointing/pushing the intended closeout.
3. Obtain successful expanded hosted matrix runs for that exact revision and
   approved real-consumer acceptance. Re-run artifact checks on the chosen source.
4. After authorization, establish an immutable source dependency pin.
5. If binaries are desired, separately define exact target triples/features and
   finish the dependency/toolchain/native notice inventory and bundle checks.

Source publication does not erase old public history or approve binary artifacts.
This document authorizes none of these actions by itself.
