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
| Python Unicode data | Review pinned inputs, generator, tables, manifest and notices | `python3.12 -B scripts/check_python_unicode15.py` with exact CPython 3.12.11 / Unicode 15.0.0 |
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
Vendored source stays pristine except for the reviewed four-source-file policy
patch (`options.rs`, `expression.rs`, `pattern.rs`, `statement.rs`). It adds the
default-false duplicate-keyword/AST-only f-string pattern-start policy and completes
the TypeVarTuple/ParamSpec default version guards independently of AST-only mode.
The sequence-separator correction requires an actual comma before a closing-token
lookahead in both modes: it intentionally rejects malformed inputs previously
accepted erroneously in default mode, while preserving valid default-mode forms.
Adapted manifests disable optional features and upstream tests.

## Data Flow

The active source inventory selects the reviewed export, including `vendor/ruff`,
not private `.ideomancer/` data, `.git`, `.agents` or build outputs. The independent
consumer resolves a fresh lockfile, checks all local dependency paths remain inside
the exported vendor tree, then performs locked public-API validation and compares
source hashes before/after. The receipt is a working-tree content snapshot, not a
Git-commit attestation. Pin a full actual authorized Git revision only once available.

The source closure also retains pinned Unicode-15 inputs, generated tables,
manifest and `src/analysis/python_ruff/unicode15/PROVENANCE.md`, including
`inputs/LICENSE.txt`. Keep those permission notices with the data and derived
tables; they are not relicensed as product code. Review their independent hash
anchors together, not by blindly regenerating the inventory or manifest.

## Example usage

For source/distribution changes, before final cache cleanup:

```sh
python3.12 -B scripts/check_checkout.py --source-dir .
python3.12 -B scripts/check_ruff_vendor.py
python3.12 -B scripts/check_python_unicode15.py
RUSTUP_AUTO_INSTALL=0 python3.12 -B scripts/check_checkout_consumer.py --source-dir . --target-dir target --offline
```

The Unicode checker requires installed trusted exact **CPython 3.12.11 / Unicode
15.0.0**, also pinned in CI. Offline table reproduction is
`python3.12 -B scripts/generate_python_unicode15.py` without `--fetch`; do not
silently refresh pinned inputs/notices. This oracle/data gate does not narrow
production's trusted Python >=3.12 prerequisite or authorize tool installation.

Use a verified disk-backed, unique owned run under
`${XDG_CACHE_HOME:-$HOME/.cache}/agent-test-runs` for `TMPDIR`, check free space
before substantial work and establish cleanup first. The helper uses that cache
parent by default, reuses the trusted root target with low-disk profiles and rejects
conflicting build overrides. It verifies owned process-group shutdown even after
parent success before deleting temporary data. If shutdown or cleanup is unverified,
it retains the owned run and reports its path and cleanup plan on stderr. Never
delete another session's data or silently claim cleanup succeeded.

## Edge Cases

### Current direction: completed F0 feasibility, not native implementation

The user explicitly selected contract-preserving full replacement and authorized
bounded byte-aware Ruff feasibility/version research. Read the main-owned
[ADR](python-native-replacement-feasibility.md) and [F0 ledger](python-analysis.md#f0-byte-input-and-version-feasibility).
The source audit conditionally supports proposing F1/F2 staged prototypes, which
need separate bounded approval. No byte-aware Ruff has been implemented or compiled.
Byte-backed lexical input plus checked source views retains Ruff grammar, but is
not a thin adapter: concrete lexer lookahead/rewind/re-lex, `Chars`/`&str` and spanning
debug strings imply an expanded maintained fork and possible AST/API changes.
The current four-file production vendor policy is not expanded by F0 authorization.

Proposed immutable full-bundle profiles need exact patch membership, a release
support window and product approval before narrowing policy or native wire/default
changes. Production follows the selected >=3.12 interpreter, not only CPython;
fixed profiles cannot preserve arbitrary future grammar/stdlib/version behavior.
Legacy `Python { python }` keeps the helper; no native probe/fallback, fabricated
`pythonVersion` or old-save upgrade. No source repair, tool installation, public
selector or cutover is authorized. G2/G3 remain privately closed; G4 byte proof,
version policy and other compatibility gates remain open.

F0 passed the **219-input** reference gate (219 AST calls, 107 accepted / 112 errors,
53 AST-attribute controls, 20 debug cases) and **23 isolated byte tests**, no skips.
Discovery: **286 tests, 273 passed / 13 isolation-required skips**; isolated
byte/visitor/codec **23 / 43 / 24 passed**, no skips, together covering all **286
unique tests**. Source **256 paths**, unchanged vendor **85/65/1,882,473 bytes** and
Git whitespace checks passed. Independent review found no blocking correction;
the ADR's F1 scalar/error/EOF/lookahead/rewind and F2 BOM-coordinate requirements
remain future obligations, not proof. See the F0 ledger for details.

This scripts/docs/CI-only slice needed no Rust build or consumer rerun and made no
native-engine/vendor change. Helper **`already_clean`**; no fresh cache removal
needed. The unique owned `TMPDIR` run was cleaned with none left. Prior facts-7
Rust tests and cleanup below remain historical. Post-doc verification is limited
to pure-Python source/helper and Git checks, without rebuilding. Production policy,
profile names, datasets and prior uncommitted implementation remain unchanged.

### Completed visitor gate: frontend-5 / facts-7

Policies are `python-ruff-private-5-py312-visitor-patterns` and
`python-ruff-private-facts-7`. Review the executable
[class/field inventory](python-visitor-contract.json) and
[witnesses](python-visitor-witnesses.json) with the checker, its tamper tests and
`src/analysis/python_parity/ruff_visitor_batch.rs`. The checker establishes inventory
and observable inputs, not output ordering: 111 classes (108 Module-domain), 188
fields, 147 in-domain node fields (130 case-unique call-input obligations and 17
structural population/neutral witnesses), and ten field-order obligations. This is
not 130 independent proofs. The Rust oracle must compare complete facts, IDs,
diagnostics and raw unsorted replay, including hidden metadata beyond caps.

Use installed trusted exact CPython 3.12.11; no installation is implied:

```sh
python3.12 -I -S -B scripts/check_python_visitor_contract.py
python3.12 -I -S -B scripts/test_python_visitor_contract.py
```

CI adds these two isolated gates. The existing explicit Ruff oracle command in the
development skill selects the visitor oracle as well as codec/failure/metadata
preservation; retain reference/native gates too. After both parser fixes, **108 Ruff
tests passed, including 17 opt-in tests**. **344 preserved + 20 new = 364** positives
matched complete arrays/IDs/diagnostics/raw replay; prior failure/Unicode/codec and
787-outcome contextual oracles passed. Checker, three visitor tests, both-mode/
both-constructor pattern controls and **34 vendor script tests** passed. Product
formatting and all-target Clippy (`--no-deps -- -D warnings`) passed, as did source
integrity at **253 paths** and vendor integrity at **85 files / 65 sources /
1,882,473 bytes**. Scripts: **263 discovered, 257 passed, six isolation-required
skips**; isolated visitor **43** and codec **24** passed without skips. Combined
lanes cover **all 263 unique tests**, not 263 discovery-lane passes.

Preservation passed **685 library tests (191 ignored)**, **18 adapter + 8 engine**
and **165 reference + 3 native** opt-ins. With the **17 Ruff opt-ins**, **185
explicit gates passed**, including known-limit assertions—not all parity matches.
**G2/G3 PRIVATE IMPLEMENTATION COMPLETE/CLOSED for current admitted Python 3.12
Module-AST lane: closed class/field inventory + full-output/raw-replay coverage;
not complete Python compatibility, G4 bytes/admission/failure/resource/version
equivalence, or production cutover.** G5/G6 remain open. Local macOS / Rust 1.89
delivery closeout passed: offline consumer fresh **105-package** lock, **one native
TypeScript public-API test**, five-Ruff-crate containment, ABI **0.1.5** and source-hash
rechecks. The configured unique owned cache `TMPDIR` was cleaned with none left.
No further code changes are planned. Actual changes are narrow
parser-start/separator fixes and proof files, not a visitor/normalizer rewrite;
data, dependencies and licenses are unchanged. Singleton-sequence controls honor
pre-existing singleton-first acceptance in both modes/constructors. The AST-only
predicate adds class-first/later-sequence children, not singleton-first admission;
real missing-separator controls
now pass in both modes/constructors, not by weakening tests.
Guarded preflight found **zero blockers / zero protected outputs**, only normal
`debug`/`tmp`. Cleanup removed **1,374,564,352 estimated allocated bytes (~1.28 GiB)**;
`target/` was absent, follow-up `already_clean`, with no Cargo afterward. No owned
debug/test artifacts remain; `.ideomancer/` JS artifacts/global caches were untouched.
Source **253**, vendor **85/65/1,882,473 bytes** and Git diff checks passed before
final documentation edits. The current consumer snapshot (hash in the ledger) is
not an actual Git revision or later-docs byte attestation. Final source/Git/helper
checks are read-only, without rebuilding. Nothing is staged, committed or pushed;
HEAD remains `523cd7b`. Historical facts-6 ~1.28 GiB receipts are separate; these
local results do not establish hosted/platform or full production approval.

**Prior checkpoint STOP (superseded for F0 authorization only):** “STOP parity
expansion.” Full replacement and bounded feasibility/version research are now
explicitly authorized as recorded above and in the ADR; prototype/production
expansion still needs separate approval. No silent narrowing or public cutover.
Contextual for/with/class suite type-comment errors
can be `IndentationError`; legacy syntax fields still intentionally carry
`failure: None`, not placeholder certificates. Another PEG parser alone does not
close arbitrary type-comment failure locations/classes. See the
[current ledger](python-analysis.md#ruff-visitor-contract-batch-frontend-5--private-facts-7).

### Historical codec batch: frontend-4 / facts-6

That checkpoint's policies were `python-ruff-private-4-py312-single-byte-codecs` and
`python-ruff-private-facts-6`. Native Rust decoding needs no Python runtime.
The finite profile admits 62 ASCII-identity tables out of 70 audited literal
tables, plus strict ASCII and existing UTF-8/Latin-1 fast paths. Cookie history
precedes the BOM gate and pinned registry lookup; dot fallback is alias-only.
UTF-8-SIG equivalence applies only after that source-context gate. Raw NUL rejects
at the root before decoding/cookies; generic `source::decode` preserves data NUL.
Source certificates use a closed SyntaxError-at-0 whitelist, not arbitrary syntax
errors or physical UTF-8 failure lines. All ranges use one unchanged decoded UTF-8
view with original hashes/caps unchanged; three-byte expansion can exhaust decoded
limits before parsing.

The Git source inventory is 248 paths (11 additions to 237). Codec implementation,
data, scripts, manifest, provenance, NOTICES and full PSF/historical LICENSE are
fingerprinted; retain those notices, not an MIT replacement. No dependency or
vendor-patch change accompanies this batch.

For codec-data changes, use exact installed trusted CPython 3.12.11 / Unicode
15.0.0 with isolated/no-site mode (`-I -S -B`) and matching installed-source hashes. The scripts
`generate_python_codecs312.py --check`, `check_python_codecs312.py` and
`test_python_codecs312.py` under `scripts/` provide reproduction/profile gates.
Generation is offline; no package installation/download is required or authorized.
CI runs the isolated generator check, independent checker and codec suite, all
with `-I -S -B`. These exact oracle pins do not narrow production Python >=3.12.
The structural audit and
123-file stdlib source closure do not attest native binaries or independently
prove installed sources byte-identical to upstream.

The isolated codec suite is a separate required lane; default script discovery
skips five profile-dependent cases. Use the installed trusted interpreter:

```sh
python3.12 -I -S -B scripts/generate_python_codecs312.py --check
python3.12 -I -S -B scripts/check_python_codecs312.py
python3.12 -I -S -B scripts/test_python_codecs312.py
```

All **104 Ruff tests passed, including 16 opt-in tests**, after the initial 17
focused codec passes. Codec oracles cover 15,872 table positions + 256 ASCII positions, 63 mixed streams,
112 AST-location comparisons, 112 complete encoded positive fact/raw-order cases,
84 certified helper negatives and 10 uncertified controls (eight helper errors,
two helper admissions). The **232 preserved + 112 new = 344** complete positive
fact/raw-order cases passed, alongside 32 prior negatives, 64 actual-helper cases
(53 certified, six ambiguous, five parser-only), 356 module failure outcomes,
787 type-comment outcomes and Unicode/source/HTTP gates. Product package formatting
and all-target Clippy (`--no-deps -- -D warnings`) passed. Source integrity passed
at 248 paths; vendor integrity passed at 85 files / 65 source files / 1,881,065 bytes.
Script discovery reported **216 tests: 211 passed, five skipped**; the explicit
isolated codec suite passed **24, none skipped**. Combined lanes cover all 216
unique script tests, not 216 default passes. Generator check and independent
checker (`license_verified: true`) passed using `-I -S -B`, also used in CI.

The test `Result` alias was fixed before the 17 focused passes. The root-NUL test
now explicitly asserts `InvalidSource` / `NotParsed`, a SyntaxError-at-0
certificate and zero validated callbacks instead of expecting Ruff
`StructuredSyntax`: the intended earlier check, not a weakened assertion. The
stale facts-5 policy assertion is now facts-6; both full 104-Ruff and 682-library
suites passed after corrections, with no engine/codec bug found. Broader results:
**682 library passed / 190 ignored**, **18 adapter + 8 engine** passed, and
**165 reference + 3 native** explicit opt-in tests passed. With 16 Ruff opt-ins,
**184 explicit tests** passed, including known-limit assertions—not all parity
matches. Source guards and original-hash module baselines remain checked.
No further code changes are planned. Validation is local macOS / Rust 1.89, not a
hosted matrix result. Product formatting passed before the current offline
consumer, which passed fresh 105-package lock resolution and one native TypeScript
public-API test, five Ruff crates contained, ABI 0.1.5 and source-hash rechecks.
Its unique owned cache `TMPDIR` was cleaned, with no retained scratch. Guarded
preflight found zero active blockers/protected outputs and only normal
`debug`/`tmp` entries. Apply removed **1,372,758,016 estimated allocated bytes
(~1.28 GiB)**; `target/` was absent and follow-up reported `already_clean`.
No Cargo ran afterward. The owned cache parent was empty, no debugging/test
artifacts were retained, and `.ideomancer/` JS artifacts/global caches were untouched.

The 248-path source check matched the consumer snapshot before final documentation
edits; Git whitespace checks passed. The tested snapshot (hash in the ledger) is
not an actual Git revision or attestation of later documentation bytes. Final
source/Git/cache verification is read-only, without rebuilding. All codec-family
and prior facts-5 work remains uncommitted at actual HEAD `523cd7b`. See the current codec batch in
[the Python ledger](python-analysis.md#ruff-single-byte-codec-batch-frontend-4--private-facts-6).

### Historical prior checkpoint and standing boundaries

The facts-5 policies, results and cleanup receipts below describe the prior
uncommitted checkpoint only, not the current batch or current cache state.

`check_package.py`, `check_package_archive.py`, `check_package_consumer.py` and
`docs/package-files.txt` remain historical registry-archive tooling and test coverage,
not active distribution acceptance. Current unpublished path dependencies cannot
produce a usable registry crate via `cargo package`. See [releasing.md](releasing.md).
The Linux/macOS × Rust 1.89.0/stable CI configuration is not a hosted result; current
results remain unknown. No compiler/toolchain installation is implied by any gate.

Private Ruff parsing uses explicit PY312 and AST-only options on captured bytes
and checks both parse/version error channels. At the prior checkpoint policies were
`python-ruff-private-3-py312-failure-certificates` and
`python-ruff-private-facts-5`. After one Ruff parse, a shared token-only failure
inspector runs before any validated-AST callback. It never accesses recovery ASTs,
waives diagnostics, reparses or rewrites source. Check `Admission`, not only
`RuffSyntax::AcceptedByRuff`: compatibility can reject Ruff-accepted input.
Inspector frames are bounded to 256 and temporary storage is precharged under a
32 MiB cap with `Checkpoint::Compatibility` cancellation; cumulative original and
decoded caps remain unchanged. `parse_batch` is classification-only;
the separate `extract_fact_candidates` implements bounded private generic fact
lowering, helper-order metadata and finite Unicode/type-comment/UTF-8/Latin-1
compatibility. It is not public Evidence or production dispatch. Full CPython
compatibility remains Pending, including finite contextual exclusions, strict-UTF-8
differences and complete failure/resource closure. Finite private exception/line
certificates include format-spec `UnicodeDecodeError` at line zero, not general
CPython failure coverage. Optional `CandidateFile.failure` is authoritative only
when certified; legacy `failure_line` alone is not a certificate. Diagnostics still
contain semantic warnings, not public helper `parse_failure` records.
Contextual type-comment `Error::Syntax` retains legacy status/line but leaves
`failure: None`: placement/line oracles do not prove all concrete CPython exception
subclasses. Only the shared inspector supplies exception certificates.
Passing known-difference assertions is not parity. Python production remains the
trusted >=3.12 AST helper. Preparse byte/token/depth limits and cooperative
before/after deadlines are not hard timeout/RSS or complete recursion guarantees.
See the [current Python validation ledger](python-analysis.md#ruff-failure-certificate-batch-frontend-3--private-facts-5)
for historical facts-5 validation and cache-cleanup evidence only. The broad run passed **670 library
tests / 186 ignored**, **18 adapter + 8 engine**, and the corrected policy assertion.
The initial 88-test focused run had 87 passes and one stale facts-4 assertion,
subsequently fixed to facts-5. All **165 reference + 3 native + 12 Ruff = 180** explicit
opt-in gates passed across initial/final runs, including known-difference/limit
assertions—not an all-parity count. Passing checks include
356 module outcomes (312 exact, 39 ambiguous, five parser-only), 64 actual-helper
failure cases (53 certified, six ambiguous, five parser-only), 232 complete positive
fact/ID/diagnostic/raw-order cases, 32 prior negatives, the 787-case contextual
oracle and exhaustive Unicode/source/HTTP checks. Scripts passed **192 tests**;
vendor integrity passed at **85 files / 65 source files / 1,881,065 bytes** and
source integrity at **237 paths**, including two new failure modules. Broad product
formatting and all-target Clippy passed.

After the conservative contextual-certificate fix, final validation passed **eight
failure-batch tests**, including the new ordinary
`contextual_locations_do_not_fabricate_exception_certificates` regression and
64-helper/44-positive oracles, **eight type-comment tests** including 787 outcomes,
and final product formatting/all-target Clippy. Record 670 broader library passes
plus one added regression passed focused, not a full 671-test library or single
full 89-test Ruff run. Final read-only review found no material P0–P3 findings after
the earlier precedence fix. Dependencies, vendor patches and notice data are unchanged.

That checkpoint's offline consumer passed fresh 105-package lock resolution and
one native TypeScript public-API test, including all five contained Ruff crates,
ABI 0.1.5 and source-hash rechecks. Its unique owned cache `TMPDIR` was automatically
cleaned. Guarded preflight found zero active blockers/protected outputs and only
normal `debug`/`tmp` entries. Cleanup removed **1,363,083,264 estimated allocated
bytes (~1.27 GiB)**; `target/` was absent and follow-up reported `already_clean`.
No Cargo ran afterward. No debugging/test artifacts were retained; the owned cache
parent was empty. Existing `.ideomancer/` JS outputs and global caches were untouched.

Changes remain uncommitted at HEAD `523cd7b`. The consumer receipt identifies a
tested content snapshot, not a Git revision or later documentation bytes; its hash
is in the Python ledger. That checkpoint's final 237-path source and Git whitespace
checks passed, with read-only helper status `already_clean`. These are historical
results, not current frontend-4/facts-6 verification or cache status.

The [historical facts-4 ledger](python-analysis.md#current-ruff-compatibility-batch-frontend-2--private-facts-4)
records completed gates and its **1.41 GiB** cleanup, not current cache status.
At that checkpoint, final product
formatting, all eight type-comment tests (including the 787-case oracle) and
all-target product Clippy passed after the equivalent boolean simplification.
The offline exported-checkout consumer passed fresh 105-package resolution and its
one independent native TypeScript public-API test, including Ruff containment,
ABI 0.1.5 and source-hash rechecks. Its explicitly configured owned cache scratch
was cleaned. These results attest a tested content snapshot, not an actual Git
commit or later documentation bytes; workflow commands themselves are not evidence
of completion.

**Historical architecture stop; standing constraint:** unchanged Ruff requires `&str`; CPython-admitted
malformed non-ASCII UTF-8 comment bytes cannot pass that interface unchanged.
No rewriting workaround is permitted. The [authorized F0 audit](python-native-replacement-feasibility.md)
is not a compiled byte frontend; prototype and production compatibility boundaries
still need separate approval, not
permission to narrow production's Python >=3.12 contract. Codec/type-comment and
platform/resource gaps remain; passing the current validation does not close these
compatibility gates or approve production cutover.

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
- `scripts/check_python_unicode15.py`, `scripts/generate_python_unicode15.py`,
  `src/analysis/python_ruff/unicode15/PROVENANCE.md`: static data, notices and oracle.
- `scripts/check_checkout.py`, `scripts/check_checkout_consumer.py`, `docs/source-files.txt`.
- `AGENTS.md`, `.agents/skills/ideomancer-development/SKILL.md`,
  `.agents/skills/ideomancer-feature-closeout/SKILL.md`, `scripts/feature_cache.py`.
