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

  The Ruff filter already selects the new ignored
  `analysis::python_ruff::facts::ruff_facts_batch` oracle, not selected by
  `analysis::python_parity`. The three existing native ignored gates remain under
  `analysis::python_native`. Shared ordinary unit tests now live under
  `analysis::python_facts`, `analysis::python_projection` and
  `analysis::python_scalars`; the old native filter does not select them.

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

### Overview

The maintained Ruff 0.12.0 snapshot at
`87f0feb21a1af67195bebf94d811770393f9f447` is a private syntax frontend, not a
production fact backend. It now also has private generic fact candidate policy
`python-ruff-private-facts-2`; facts-1 remains the first cross-family checkpoint,
not the current exclusion list. CPython compatibility remains Pending. Production
`python-static-8-generic-facts`, the trusted Python >=3.12 AST helper, public API,
evidence, CLI and default are unchanged. No new vendor/dependency changes are needed.

### API / Interfaces

`parse_batch` remains classification-only. `extract_fact_candidates` returns bounded
generic facts/IDs/diagnostics and per-file status with a module baseline; no AST,
literal payload or source excerpt is published. `analysis/python_facts.rs` supplies
common Fact/Warning records, IDs, signature/import/warning helpers and a transactional
bounded sink. Projection/scalars are declared once in `analysis.rs`. Tree-sitter
private policy 15 uses common records/IDs/signatures but retains its CST-specific
collector, finalization and import encoder; this is not complete backend unification.

### Data Flow

Captured bytes pass existing input/lexical gates, then one Ruff parse with explicit
PY312 and `with_ast_only(true)`. Both parse and unsupported-version diagnostics must
be empty before a callback consumes the validated borrowed AST. No reparse or AST
escape is allowed. Bounded normalization separates render and visit edges, preserving
descendant calls even through opaque/clipped shapes and helper-order alias effects:
return-annotation calls follow body imports, not source order. Function type-parameter
bound calls replay after body/return annotations; class bounds replay after the body.
Observations stay unresolved until replay, so body imports affect resolution;
constraints use tuple bounds. The observation-only header helper avoids rendering
an unused root shape. Match traversal is subject, pattern, guard, body; mapping
patterns visit all keys before all patterns. Type aliases visit name, type parameters,
value under the same owner; reusing an import alias name does not invalidate it.
No pattern/capture/type-alias/type-parameter facts, bindings, scopes or alias-map
mutations are invented. File emission is
transactional. Source working regions are scoped per statement/expression; persistent
projection maps charge retained aliases, including overwrite-shrink adjustments.
Existing node/edge accounting, including class-pattern wrappers, remains in force.

### Edge Cases

- Admitted: functions/async functions/classes/nested owners, full signatures,
  annotations/defaults/bases/class keywords and non-decorator header calls; relative,
  star and future imports; Assign/AnnAssign/class attributes/chained/destructured
  writes; ordinary control flow, try, async for/with, raise/assert/augmented writes/
  deletion; all Python 3.12 generic expression families, including lambdas,
  comprehensions/generators, await/yield/walrus and f-strings; all eight match pattern
  forms with case guards/bodies, type aliases and function/async-function/class type
  parameters. AST-only top-level return/jumps are allowed when the parser accepts
  them, not executed or compile-validated.
- Excluded: decorators, Python 3.13 type-parameter defaults,
  raw non-ASCII identifiers (even those normalizing to ASCII), all type comments
  including ignores, and active named Unicode escapes. Raw/bytes/escaped-backslash
  text is allowed. Nonzero decimal integers over 4300 digits exclude underscores
  from the count; all-zero, base-prefixed, float and imaginary forms are exempt.
- Constructor/include_router/router-mutation guards reject the whole file as
  unsupported where G3 metadata is missing. Generic redacted shapes need no literal
  payload retention. Effective encoding cookies and invalid UTF-8 remain unsupported;
  UTF-8 BOM source hashes cover original bytes.
- Class patterns such as `Record()`, `FastAPI()` and `APIRouter()` are syntax, not
  constructor calls. MatchValue f-strings are admitted, but a bare f-string mapping
  key accepted by the CPython AST helper produces an ordinary Ruff diagnostic:
  zero AST callbacks, module baseline only, `SyntaxError`, no recovery or diagnostic
  waiver. A separate adjacent plain-string prefix fixture is accepted independently,
  not an automatic source rewrite.
- Ruff's PY312 mode misses version errors for TypeVarTuple/ParamSpec defaults;
  TypeVar has an upstream version error. The fact driver explicitly rejects defaults
  on all three variants without visiting them. Classification-only `parse_batch`
  remains unchanged and Pending, not proof of target-version parity.
- Semantic bounds: 200000 nodes, depth 256, 400000 edges, 100000 facts, 100000
  warnings, 32 MiB working and 32 MiB output, in addition to existing input/lexical
  bounds. Parsing and AST destruction are uninterruptible; cooperative accounting
  is not an RSS/stack sandbox or hard deadline. Full Unicode, encoding, failure,
  resource and full G2–G6 closure remain open; PY312 alone proves no compatibility.

### Example usage

For developer validation, use the existing `--lib analysis::python_ruff -- --ignored`
oracle command above with a trusted Python 3.12 interpreter. Keep the existing active
Rust 1.89 compiler, `RUSTUP_AUTO_INSTALL=0`, two jobs/threads and package-only formatting.
There is no new public CLI/backend selector. The recorded facts-2 focused checkpoint
is 31 Ruff tests, including 77 complete-array/raw-order cases on trusted Python
3.12.11: 75 base fixtures plus two metadata/BOM cases, preserving the old 51 and
adding 26 structured cases. The 18-case acceptance oracle (previously 16) checks
both new CPython/Ruff admission disagreements against the fact guard; the bare
mapping-key gap separately asserts zero callbacks and is not a matching case.
Focused checks are not final acceptance; record actual broader results per batch.
The coordinating owner owns broad gates and ROADMAP/Python-ledger/source-inventory
updates. This batch adds no source paths, manifests, vendor or dependency changes;
the 215-source/85-vendor closure and reviewed two-file AST-only patch are unchanged.

### Related files

See `docs/python-analysis.md` for the completion ledger and historical Tree-sitter/
Ruff probes; `src/analysis/python_ruff/{facts,expressions}.rs` for the candidate;
`src/analysis/python_facts.rs` and `src/analysis/python_native/{projection,scalars}.rs`
for shared primitives. Keep fixtures generic; consumer-private source/paths stay out.

## Finish the feature

Read `.agents/skills/ideomancer-feature-closeout/SKILL.md` from the repository root.
Validation results and cache cleanup
must be recorded before a completed feature is handed off. Never claim passing
checks that were ignored, unavailable or not executed.
