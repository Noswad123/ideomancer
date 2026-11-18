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

- All Rust changes: `RUSTUP_AUTO_INSTALL=0 cargo fmt --all --check`, focused tests,
  and appropriately scoped Clippy with warnings denied. At feature closeout, broaden to the relevant
  integration targets or full suite once when the affected surface warrants it.
- Python extraction/candidate changes: explicitly run the oracle gate; ignored or
  unavailable oracle cases do not establish parity:

  ```sh
  RUSTUP_AUTO_INSTALL=0 IDEOMANCER_TEST_PYTHON="$(command -v python3.12)" \
    cargo test --locked --offline --jobs 2 --lib analysis::python_parity \
    -- --ignored --nocapture --test-threads=2
  ```

- Script changes: `python3.12 -B -m unittest discover -s scripts -p 'test_*.py'`.
- New packaged paths: review and update `docs/package-files.txt`, then run
  `python3.12 -B scripts/check_package.py` before cleanup.
- Manifest/distribution changes: use `docs/releasing.md` as needed. Do not confuse
  path inventory, actual archive integrity, packaged build and consumer acceptance.
- Browser/SVG checks use only existing trusted tools when presentation is affected;
  never provision a browser or invoke a target application to make a test pass.

## Finish the feature

Read `.agents/skills/ideomancer-feature-closeout/SKILL.md` from the repository root.
Validation results and cache cleanup
must be recorded before a completed feature is handed off. Never claim passing
checks that were ignored, unavailable or not executed.
