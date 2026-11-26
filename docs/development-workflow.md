# Resource-Bounded Development And Feature Closeout

Use focused validation during implementation, broaden it at a meaningful feature
boundary, then remove the disposable build cache. Reusing a target directory is
not sufficient: compiler variants, test binaries, debug data and incremental state
can accumulate even in one directory.

## Overview

Distribution is approved personal/company Git source, not registry publication.
Root `publish = false`; five local Ruff crates and their notices/policy patch travel
in the checkout. No external fork, submodule or unpublished registry-version family
is needed. Private consumer material still stays outside generic source.

## Repository-local skills

Root `AGENTS.md` directs coding agents to these files, with direct-read fallback
when their harness does not discover project skills:

- `.agents/skills/ideomancer-development/SKILL.md`
- `.agents/skills/ideomancer-feature-closeout/SKILL.md`

They are Git-distributed developer guidance, intentionally excluded from the Cargo
source package. They do not alter the runtime prompts under `skills/`, global
harness configuration or tool permissions. Guidance is not an automatic scheduler;
an agent/operator must invoke the closeout command and report its outcome.

## Small development builds

The root manifest uses line-table-only debug information and disables incremental
compilation in both development and test profiles. This keeps source locations in
backtraces but reduces debugger detail and may slow rebuilds. Release settings are
unchanged. These are root-project profiles, not settings imposed on consumers.

Use one installed toolchain and `target/` per feature by default, with at most two
Cargo jobs/test threads. Do not introduce per-case build caches or arbitrary
`CARGO_TARGET_DIR` overrides. Changing profiles without first cleaning leaves old
artifacts behind; the transition should clean, validate the new settings, and clean
again at closeout. Do not rebuild just to prove that cleanup succeeded.

Set `RUSTUP_AUTO_INSTALL=0` for every Rust invocation, including version probes,
Cargo subcommands and delegated commands. Use the current installed active compiler
without a selector by default, for example `RUSTUP_AUTO_INSTALL=0 rustc --version`.
Before any explicit `+toolchain` selector, inspect the exact installed names with
`RUSTUP_AUTO_INSTALL=0 rustup toolchain list` (read-only). A reported compiler
version or the manifest MSRV does not prove that a matching alias is installed.
Missing toolchains must stop the command, never trigger automatic installation,
even for version probes. Tool installation/removal and global configuration changes
require separate explicit authorization.

| Work | During iteration | At closeout |
| --- | --- | --- |
| Rust behavior | Affected unit filter / integration target | Format, Clippy and broader affected regressions once |
| Python extraction | Focused parser/candidate tests | Explicit trusted Python oracle; ignored cases are not acceptance |
| Scripts/guidance | Focused script tests; review-only for prose changes | All script tests for script behavior changes; applicable policy checks |
| Source paths | Review exact additions in `docs/source-files.txt` | `python3.12 -B scripts/check_checkout.py` |
| Vendored Ruff | Review pin, pristine source, policy diff, manifests and notices together | `python3.12 -B scripts/check_ruff_vendor.py` |
| Manifest/distribution | Focused policy tests | Independent checkout-consumer gate from the release guide, not a crate/archive build |
| Presentation | Focused HTML/model tests | Existing trusted renderer/browser checks where affected |

`make test` is a broad gate, not the edit loop. `make check` checks formatting and
all-target Clippy. Both default to two jobs; tests default to two test threads.
The makefile exports `RUSTUP_AUTO_INSTALL := 0` by default to child commands without
changing the global environment. Neither gate may install tools. Fetch reviewed
dependencies explicitly when offline input is unavailable instead of weakening
the test or silently changing versions.

## API / Interfaces

Format only root product code:
`RUSTUP_AUTO_INSTALL=0 cargo fmt --package ideomancer --check`.
Never use `cargo fmt --all`, which recursively touches local path dependencies.
Scope Clippy to product code with `--all-targets --no-deps -- -D warnings`.
The preserved Ruff sources are verified by hash and patch provenance instead of
product formatting/Clippy policy; upstream compiler warnings remain visible.
Vendored source stays pristine except for the reviewed two-file AST-only policy
patch; adapted manifests disable optional features and upstream tests.

## Data Flow

The active source inventory selects the reviewed export, including `vendor/ruff`,
not private `.ideomancer/` data, `.git`, `.agents` or build outputs. The independent
consumer resolves a fresh lockfile, checks all local dependency paths remain inside
the exported vendor tree, then performs locked public-API validation and compares
source hashes before/after. The receipt is a working-tree content snapshot, not a
Git-commit attestation. Pin a full actual authorized Git revision only once available.

## Example usage

For source/distribution changes, before final cache cleanup:

```sh
python3.12 -B scripts/check_checkout.py --source-dir .
python3.12 -B scripts/check_ruff_vendor.py
RUSTUP_AUTO_INSTALL=0 python3.12 -B scripts/check_checkout_consumer.py --source-dir . --target-dir target --offline
```

Use a verified disk-backed, unique owned run under
`${XDG_CACHE_HOME:-$HOME/.cache}/agent-test-runs` for `TMPDIR`, check free space
before substantial work and establish cleanup first. The helper uses that cache
parent by default, reuses the trusted root target with low-disk profiles and rejects
conflicting build overrides. It verifies owned process-group shutdown even after
parent success before deleting temporary data. If shutdown or cleanup is unverified,
it retains the owned run and reports its path and cleanup plan on stderr. Never
delete another session's data or silently claim cleanup succeeded.

## Edge Cases

`check_package.py`, `check_package_archive.py`, `check_package_consumer.py` and
`docs/package-files.txt` remain historical registry-archive tooling and test coverage,
not active distribution acceptance. Current unpublished path dependencies cannot
produce a usable registry crate via `cargo package`. See [releasing.md](releasing.md).
The Linux/macOS × Rust 1.89.0/stable CI configuration is not a hosted result; current
results remain unknown. No compiler/toolchain installation is implied by any gate.

Private Ruff parsing uses explicit PY312 and AST-only options on captured bytes,
checks both parse/version error channels and publishes classification only. Entire
CPython compatibility and fact lowering remain Pending; Python production remains
the trusted >=3.12 AST helper. Preparse byte/token/depth limits and cooperative
before/after deadlines are not hard timeout/RSS or complete recursion guarantees.

## Cleanup commands

```sh
make cache-status
make cache-clean
```

Equivalent direct commands:

```sh
python3.12 -B scripts/feature_cache.py
python3.12 -B scripts/feature_cache.py --apply
```

The default is dry-run. Apply validates the Git/manifest identity and fixed local
target, checks process activity and filesystem boundaries, then invokes only
`RUSTUP_AUTO_INSTALL=0 cargo --config 'build.build-dir="<project>/target"' clean
--manifest-path <project>/Cargo.toml --target-dir <project>/target`.
The highest-precedence build-directory override prevents newer Cargo versions from
also deleting an independently configured external/shared intermediate directory.
The helper copies the parent environment and pins `RUSTUP_AUTO_INSTALL=0` for the
Cargo subprocess regardless of the parent's value. It preserves other environment
variables without serializing the environment into the JSON receipt.
There is no external-path or force option. Unknown process state, active Rust
processes, target executables, symlinks/special files, mounts and external target
environment overrides fail closed. Conservative process checks can block for a
Rust build in another project; wait or coordinate rather than bypassing them.

Known saved-study filenames such as `catalog.json`, `evidence.json`, `claims.json`
and `receipt.json` stop cleanup. This is not an exhaustive retention scanner.
Review non-Cargo artifacts and requested deliverables before applying. Move or
discard retained data only after an authorized decision; do not fabricate receipts
or alter evidence just to make cleanup pass.

Keep saved studies, acceptance pages and requested release artifacts outside
`target/`. Project-local retained material may use the ignored `.ideomancer/`
directory when authorized. External private reports remain external. Cache cleanup
does not remove these, `bin/`, installed executables, other checkouts, global Cargo
registry caches or compiler toolchains. It also does not make temporary report
directories durable or give them a retention/cleanup schedule.

JSON reports include before/after cache allocation estimates, disk free space,
process guards and the cleanup outcome. Hard-linked files are counted once for
allocated bytes; APFS/COW sharing and unrelated activity prevent exact attribution
of reclaimed space. A successful apply verifies that `target/` is absent or empty.

## Ordering is mandatory

Run all required validation, including checkout-consumer builds, **before** cleanup. At handoff, use only the
Python status helper and Git inspection to verify the result. If a later action
rebuilds artifacts, repeat cleanup. A completed feature must either report a clean
cache or an explicit user retention waiver/blocker; do not silently carry the cache
into the next feature.

Legacy `make clean` also removes the copied `bin/ideomancer` executable and accepts
operator path overrides. Use the guarded cache-only command for routine feature
closeout instead.

## Related files

- [Release review](releasing.md), [third-party notices](third-party-notices.md).
- [Ruff snapshot maintenance](../vendor/ruff/README.md), `scripts/check_ruff_vendor.py`.
- `scripts/check_checkout.py`, `scripts/check_checkout_consumer.py`, `docs/source-files.txt`.
- `AGENTS.md`, `.agents/skills/ideomancer-development/SKILL.md`,
  `.agents/skills/ideomancer-feature-closeout/SKILL.md`, `scripts/feature_cache.py`.
