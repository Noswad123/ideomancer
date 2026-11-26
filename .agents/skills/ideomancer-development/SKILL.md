---
name: ideomancer-development
description: Develop Ideomancer with focused validation, bounded resource use and generic source-analysis contracts.
---

# Ideomancer development

## When to use

Read this skill before implementing or debugging this project. This is project
guidance for coding agents, not an interpretation prompt for analyzed applications.

## Start with the contract

1. Inspect `git status --short --branch` and relevant existing work; preserve it.
2. Read `ROADMAP.md` and the affected contract. Use `docs/python-analysis.md` for
   Python candidates, `docs/javascript-typescript-analysis.md` for JS/TS, and
   `docs/engine-api.md` / `docs/adapter-contract.md` for the public seam.
3. Define the smallest observable acceptance case. Use independently synthetic
   fixtures. Never move private source, identifiers or context into this repository.
4. Check `RUSTUP_AUTO_INSTALL=0 rustc --version` using the existing active compiler;
   honor `Cargo.toml`'s MSRV. Use one installed toolchain and the default project
   `target/`. Set `RUSTUP_AUTO_INSTALL=0` for every Rust invocation, including probes,
   Cargo subcommands and delegated commands. Before any explicit `+toolchain`
   selector, inspect `RUSTUP_AUTO_INSTALL=0 rustup toolchain list` (read-only) and
   use only an exact installed name. Never infer an installed alias from the
   reported compiler version or MSRV. A missing toolchain is a blocker: do not
   install or update tools implicitly, even to probe a version.

## Low-disk implementation loop

- Preserve `[profile.dev]` and `[profile.test]`: line-table-only debug information,
  no incremental compilation. Backtraces retain locations; full variable/type
  debugging and incremental speed are deliberately traded for lower disk use.
- Use `--locked --offline --jobs 2`; run test binaries with `--test-threads=2`.
  When changing dependencies, explicitly review/fetch the selected pins first.
- Choose the affected test filter or integration target rather than the entire
  suite on each edit. Example:

  ```sh
  RUSTUP_AUTO_INSTALL=0 cargo test --locked --offline --jobs 2 --lib analysis::python_native -- --test-threads=2
  RUSTUP_AUTO_INSTALL=0 cargo test --locked --offline --jobs 2 --test engine_api -- --test-threads=2
  ```

- Do not run concurrent Cargo commands against the shared target directory. Delegate
  read-only reviews or disjoint edits, but coordinate builds and tests serially.
- Never add per-fixture target directories. Test data uses automatically cleaned
  temporary directories; retained human-review outputs need an explicit location
  and retention decision outside the build cache.
- Do not repeatedly rebuild release binaries or verify packages merely because a
  focused test changed. Do not switch compiler versions during normal iteration.

## Choose the right verification gate

- All Rust changes: `RUSTUP_AUTO_INSTALL=0 cargo fmt --package ideomancer --check`, focused tests,
  and root all-target Clippy with `--no-deps -- -D warnings`. Vendored upstream
  code is hash-checked rather than reformatted or subjected to product Clippy policy;
  compiler diagnostics from that preserved source remain visible. At feature
  closeout, broaden to the relevant
  integration targets or full suite once when the affected surface warrants it.
- Python extraction/candidate changes: explicitly run the oracle gate; ignored or
  unavailable oracle cases do not establish parity:

  ```sh
  RUSTUP_AUTO_INSTALL=0 IDEOMANCER_TEST_PYTHON="$(command -v python3.12)" \
    cargo test --locked --offline --jobs 2 --lib analysis::python_parity \
      -- --ignored --nocapture --test-threads=2
  RUSTUP_AUTO_INSTALL=0 IDEOMANCER_TEST_PYTHON="$(command -v python3.12)" \
    cargo test --locked --offline --jobs 2 --lib analysis::python_native \
      -- --ignored --nocapture --test-threads=2
  RUSTUP_AUTO_INSTALL=0 IDEOMANCER_TEST_PYTHON="$(command -v python3.12)" \
    cargo test --locked --offline --jobs 2 --lib analysis::python_ruff \
      -- --ignored --nocapture --test-threads=2
  ```

- Script changes: `python3.12 -B -m unittest discover -s scripts -p 'test_*.py'`.
- New source paths: review and update `docs/source-files.txt`, then run
  `python3.12 -B scripts/check_checkout.py` before cleanup.
- Vendored Ruff changes: run `python3.12 -B scripts/check_ruff_vendor.py`; review
  immutable pin, checker anchors, provenance, patch reversal, manifests and notices
  together. Never use `cargo fmt --all`: recursive formatting touches path dependencies.
  Third-party source stays pristine except for the reviewed two-file AST-only patch.
- Manifest/distribution changes: use `docs/releasing.md`. Approved personal/company
  delivery is Git source (`publish = false`), with all five local Ruff crates and
  notices in `vendor/ruff`, not registry publication or a separate fork/submodule.
  Run `RUSTUP_AUTO_INSTALL=0 python3.12 -B scripts/check_checkout_consumer.py
  --source-dir . --target-dir target --offline` before cleanup. It exports reviewed
  source, resolves a fresh consumer lockfile, checks local dependency containment,
  performs locked API validation and rechecks hashes. A working-tree snapshot is
  not an attested Git commit; eventual consumers pin a full actual approved revision.
  Legacy `check_package*` helpers and `docs/package-files.txt` remain historical
  archive tooling/tests, not active gates; `cargo package` cannot supply a usable
  registry crate with these unpublished path dependencies.
- Temporary source/test data must use verified disk-backed owned unique runs under
  `${XDG_CACHE_HOME:-$HOME/.cache}/agent-test-runs`, not `/tmp` or `/dev/shm`.
  Check free space, configure `TMPDIR` and establish cleanup before substantial work.
  The checkout consumer defaults to this cache parent and only the trusted project
  target with low-disk profiles. It stops/verifies owned process groups even after
  parent success before deleting temps; on unverified shutdown/cleanup failure it
  retains the owned run and reports the path and cleanup plan on stderr.
- Browser/SVG checks use only existing trusted tools when presentation is affected;
  never provision a browser or invoke a target application to make a test pass.

## Python frontend boundary

The maintained Ruff 0.12.0 snapshot at
`87f0feb21a1af67195bebf94d811770393f9f447` is a private syntax frontend, not a
production fact backend. Captured-byte classification explicitly selects PY312 and
`with_ast_only(true)` and checks both parse/version error channels. No AST, facts,
literals or source excerpts are published. CPython compatibility and fact lowering
remain Pending; the trusted Python >=3.12 AST helper remains the production default.
Keep the Tree-sitter fact candidate as regression coverage until future Ruff lowering.
Preparse byte/token/depth gates and cooperative before/after deadlines are not hard
timeout/RSS guarantees or complete parser-recursion proof. PY312 is not proof of
Unicode 15 compatibility. Syntax acceptance alone establishes no semantic facts.

## Finish the feature

Read `.agents/skills/ideomancer-feature-closeout/SKILL.md` from the repository root.
Validation results and cache cleanup
must be recorded before a completed feature is handed off. Never claim passing
checks that were ignored, unavailable or not executed.
