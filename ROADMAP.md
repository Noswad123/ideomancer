# Ideomancer Roadmap

## Purpose And Ownership

Ideomancer is an MIT-licensed engine and CLI for source-grounded software
understanding. Its job is to collect source safely, describe language-level
structure, validate models and provenance, and generate useful diagrams.

Framework registration rules, organizational conventions, private repository
layouts, and company workflows belong in consuming tools or adapters. The test
of this boundary is practical: a consumer must be able to improve its diagrams
without adding another organization-specific branch to Ideomancer.

This is a milestone plan, not a claim that the current working tree is released
or that a whole application is understood. There are no promised dates.

## Now / Next / Later

| ID | Milestone | Status | Exit gate |
| --- | --- | --- | --- |
| I0 | Generic engine foundation and baseline review | Implemented locally; release review pending | Reviewed source/license/package boundary and a reproducible consumer build |
| I1 | Evidence and derived-relationship contract | Candidate contract/schema implemented locally | External consumer and independent synthetic conventions exercise the same versioned contract |
| I2 | Read-only facts and validated class-model augmentation | Synthetic integration and a bounded real-source explicit-configuration slice accepted | Preserve immutability, identity checks and provenance; broader framework/runtime interpretation remains separate |
| I3 | Harden and release the consumable engine | **Next: release and history review** | An approved immutable release works as an upstream or mirrored dependency |
| P0 | Shared parser backend and native JS/TS Tree-sitter migration | Scoped source/facts, rendered-diagram usefulness and external explicit-configuration boundary accepted; release gates separate | Preserve fact/proof/output contracts; no Node discovery prerequisite; package/consumer checks and supported-platform release validation |
| P1 | Native Python backend | Frontend-5/facts-7 **G2/G3 privately closed** for the admitted Python 3.12 Module-AST lane; **all bounded F2 implementation/delivery gates PASS**, including current consumer/cleanup; production helper retained | [F2 ledger](docs/python-analysis.md#f2-byte-module-parser-source-and-debug-experiment): CPython semantics/admission, debug constants, codecs/type comments, lazy failure priority, resources, F3, full G4, version policy and G5/G6/cutover remain open; no further implementation authorized |
| P2 | Rust Tree-sitter backend and language facts | Next parser milestone; Rust analysis not implemented | Model Rust structs, enums, traits, impls, functions and modules honestly; no build scripts or macro execution |
| I4 | Broader language/model capabilities | Later | Independent use cases justify each extension without weakening the boundary |

P0 scoped acceptance is closed: useful rendered views and one real-source consumer
extension have been reviewed without adding private interpretation to the engine.
This does not approve general framework semantics, runtime wiring or publication.
Release gates remain separate from parser development; no milestone authorizes a
commit, push or release. P1 starts with an executable AST behavior contract, not a
new backend switch or a default replacement. Python and Rust Tree-sitter remain
undelivered, not a claim that all languages are native today. Optional Mermaid SVG rendering and browser checks still use external
Node/Chromium tooling independently of JS/TS discovery.

### Parser Acceptance

**Current F2 checkpoint:** authorized private byte Module parser/source/debug
experiment; one existing Ruff grammar/shared fallible core, closed Legacy/Bytes
provider, accepted AST callbacks only after both diagnostic channels and provider
finish. Source-backed debug owner/ranges are identity, not semantic values or
CPython constants. Bounded implementation validation passed: 743 library tests
(191 ignored), including all 35 F2; preservation/oracles and provenance passed.
Current consumer and guarded cleanup passed: **all bounded F2 delivery gates PASS**;
target absent, no Cargo afterward. Finite tests are not CPython parity and this
closeout authorizes no further implementation. See the primary
[F2 ledger](docs/python-analysis.md#f2-byte-module-parser-source-and-debug-experiment)
for construction/reduction bounds, regression fix, exact evidence/hashes and open
gates. Verified schema-3 closure: 89 vendor files / 67 Rust sources / 2,219,373
bytes; 264 source paths verified before docs freeze. Production helper/Python >=3.12 and
frontend-5/facts-7 names remain unchanged; fingerprints change. User-reported HEAD
`805edb0` is committed F1, initially clean; F2 remains uncommitted and unstaged,
with no commit or push. The consumer snapshot does not attest final receipt prose.

**Historical F1 receipts follow.** Authorization, counts, hashes and cleanup below
describe F1 handoff, not current F2 status.

**Historical F1 checkpoint: bounded lexer goal COMPLETE / PROVED by tests.** One shared
fallible byte core serves legacy `Lexer(&str)`
and a doc-hidden experimental byte facade; no separate lexer, source repair,
unchecked UTF-8, parser/AST/debug construction, semantic admission, facts dispatch
or public API/default change. Coordinator reports **all 131 private Ruff tests
passed**, including **23 F1 ordinary + 17 existing opt-ins**; all **364** complete
positive facts/IDs/diagnostics/raw replay matched on the unchanged normal text
path. Both facades passed all thirteen frozen pre-core hashes (298 tokens/all
four modes), including compiled legacy `Send + Sync`; the 48 shared-core
comparisons remain supplementary. Product formatting/all-target Clippy, **58 vendor
script tests**, schema-2 checker and **261-path source gate** passed. Scripts:
**310 discovered, 297 passed / 13 isolation skips**; isolated byte/visitor/codec
**23 / 43 / 24 passed**, no skips, covering all 310 unique tests across lanes.
Current preservation passed **708 library / 191 ignored**, **18 adapter + 8 engine**
and explicit **165 reference + 3 native** tests. With the 17 Ruff opt-ins, **all 185
explicit gates passed**, including known-limit assertions—not 185 parity matches.
P2 view accounting is closed; final read-only review found no material P1/P2/P3
findings and schema-2 guards passed. **All F1 delivery gates passed**: offline
consumer fresh **105-package** lock, **one native TypeScript public-API test**,
five contained Ruff crates, ABI **0.1.5** and source-hash rechecks. Its snapshot
(hash in the ledger) is not a Git revision or final-docs attestation; its owned
cache `TMPDIR` was cleaned. Guarded cleanup, with zero blockers/protected outputs,
removed **1,360,498,688 estimated allocated bytes (~1.27 GiB)**; target absent,
follow-up `already_clean`, **no Cargo afterward**. No owned debug/test artifacts
remain; root cache parent empty, `.ideomancer/` JS/global caches untouched. Final
source/Git/helper checks are read-only, without rebuilding. No additional core
changes are planned; provenance anchors are stable. F2 byte parser input,
AST/debug/admission/lazy-precedence proof is not established or authorized
by F1, nor is a finite version policy or production cutover.

Current source inventory is **261 paths**; schema-2 vendor closure is **87 files /
66 Rust sources / 1,972,891 bytes**, with two disjoint four-file patches (eight
modified upstream, 57 unchanged) and one separately hashed original MIT byte
facade. Manifests/dependencies and frontend-5/facts-7 names are unchanged; the
implementation fingerprint changes transitively. See the
[F1 ledger](docs/python-analysis.md#f1-shared-byte-lexer-experiment) for tests,
baseline and provenance anchors. Actual pre-F1 HEAD was user-reported `fdf4e58`
(facts-6 commit, initially clean/ahead one), not `523cd7b`; F1 changes were then
uncommitted and unstaged; no agent commits or pushes were made. The detailed F0/visitor
and earlier receipts below are historical, including HEAD and cache assertions.

Python completion now follows a finite six-gate program, rather than an open-ended
queue of syntax slices: **G1** generic expressions/ordinary bodies; **G2** remaining
generic traversal; **G3** existing discovery/FastAPI metadata; **G4** grammar,
encoding and failure compatibility; **G5** native identity/product integration;
**G6** existing-engine usefulness, operations and approved cutover. See the
[contract, inventory and blockers](docs/python-analysis.md#completion-contract-and-remaining-gates).
Development's Unicode generation/check and CI oracle use exact CPython 3.12.11 /
Unicode 15.0.0; this oracle pin does not narrow production's trusted Python >=3.12
contract. Test counts do not establish completion.

The committed facts-4 checkpoint used `python-ruff-private-facts-4` with
frontend policy `python-ruff-private-2-py312-versioned-compatibility`. Raw identifiers
are gated to static Unicode-15 XID data before Ruff whole-name NFKC, not compiler or
latest-Unicode admission. Named escapes include 473 aliases and algorithmic
CJK/Hangul names, preserving surrogates and redaction. Contextual type comments,
including ignores, use a bounded compatibility prepass and span correction;
actual type comments in f-string replacement fields and parameter-internal comments
with lexically active lambda/if/else/for/async/yield prefixes remain unsupported.
The latter uses a constant-space cancellable token scan; closed complex
subexpressions, completed annotations before defaults, trailing comments and
ignores do not trigger that exclusion. Source
decoding admits UTF-8/BOM/eligible cookies and finite Latin-1 aliases, hashes original
bytes, and bounds decoded text at 2 MiB/file and 32 MiB/run in addition to original
1 MiB/file and 16 MiB/run caps. Router targets count 128 normalized Python characters.
Other codecs, strict UTF-8 physical-line versus PEG opening-line differences,
malformed UTF-8 comment admission, complete failures and resource closure remain
open. At that checkpoint invalid named escapes used ordinary token
opening lines or matching f-string closing lines, preserving nested-failure
precedence. G2/G4–G6 are not closed.

At that checkpoint four Ruff source files carried the maintained patch: default-false AST-only
duplicate-keyword policy, bare f-string mapping keys only in AST-only mode, and
TypeVarTuple/ParamSpec default version checks matching TypeVar (PY312 errors with
either AST-only setting, PY313 accepted). The five local crates and Git delivery
with `publish = false` are unchanged. That checkpoint's provenance was 85 vendor files / 65
source files / 1,881,065 bytes; that checkpoint's source inventory was 235 paths, including pinned
Unicode inputs, generated data, notices and generation/check tooling.

**Historical facts-4 integrated evidence:** all 75 Ruff tests (including nine opt-in tests)
passed, with 188 complete positive fact/ID/diagnostic/raw-order comparisons and
32 matching negative failure-line cases (16 new multiline named-escape cases).
The contextual oracle passed 787 asserted outcomes, including finite exclusions;
four strict-UTF-8 differences are asserted separately (two multiline locations,
two comment-byte admission cases). Exhaustive Unicode checks covered 1,114,112
membership points and 190,293 name probes; source and HTTP-method oracles passed.
The library passed 660 tests (183 ignored), adapter/engine targets passed 18/8,
and explicit reference/native gates passed 165/3. Together with Ruff's nine
opt-in tests, 177 explicit tests passed, including known-difference assertions;
this is not an all-parity count. Scripts passed 192 tests; exhaustive data,
85-file/65-source/1,881,065-byte vendor and 235-path source checks, and product
formatting passed. Historical facts-3 results remain separate.

**Final reruns:** after the equivalent type-comment boolean simplification,
product formatting, all eight type-comment tests (including the 787-case oracle)
and all-target product Clippy with `--no-deps -- -D warnings` passed.
The first broad command timed out at 200 seconds; processes were checked stopped
and exact owned temporary data removed. The ten-minute rerun completed all tests
before the now-resolved Clippy finding—there was no test failure. The exported
checkout consumer passed offline fresh 105-package lock resolution and one full
independent native TypeScript public-API test, with five contained Ruff crates,
ABI 0.1.5 and source-hash rechecks. Its configured owned scratch was cleaned.
This was a source-snapshot check, not a Git-commit attestation or validation of
later documentation bytes. See the [Python ledger](docs/python-analysis.md#current-ruff-compatibility-batch-frontend-2--private-facts-4)
for the tested snapshot hash and guarded cache-cleanup record. No production cutover
is claimed.

**Historical prior uncommitted checkpoint:** `python-ruff-private-3-py312-failure-certificates` /
`python-ruff-private-facts-5`, following the user-committed facts-4 baseline at
`523cd7b` (observed clean before this batch). A shared token-only inspector after
one Ruff parse moves raw Unicode-15, decimal-4300 and named-escape guards out of
fact indexing and adds finite exception/line certificates. Admission, not Ruff
syntax alone, controls validated-AST callbacks; neither diagnostic channel is
waived, and recovery AST access, reparsing and source rewriting remain prohibited.
Format-spec decode failures and fourth-level replacement fields now have finite
checks, not a compiler-semantic validator or complete CPython failure coverage.
Contextual type-comment `Error::Syntax` preserves legacy status/line only, with
`failure: None`: its oracles prove placement/lines, not every concrete CPython
exception subclass. Only the shared inspector supplies exception certificates.

Checks passed 356 module-oracle outcomes (312 exact, 39 ambiguous, five
parser-only), 64 actual helper failure cases (53 certified, six ambiguous, five
parser-only), 232 complete positive fact/ID/diagnostic/raw-order cases, 32 prior
negatives, the 787-case contextual oracle and exhaustive Unicode/source/HTTP gates.
The broad run passed **670 library tests / 186 ignored**, **18 adapter + 8 engine**
tests and the corrected policy assertion. The initial 88-test focused run's stale
assertion (87 passed, one failed) is resolved. Explicit reference/native gates passed
165/3; all 12 Ruff opt-in gates passed across initial/final runs (**180 total**,
including known-difference assertions). Scripts passed 192 tests; vendor integrity
passed at 85 files / 65 source files / 1,881,065 bytes and source integrity at 237 paths.
After the conservative contextual-certificate fix, all eight failure-batch tests
(including the added ordinary regression and 64-helper/44-positive oracles), eight
type-comment tests (including 787 outcomes), and final formatting/all-target product
Clippy passed. This is 670 broader library passes plus one added regression passed
focused, not a full 671-test or single 89-test Ruff run. Final read-only review found
no material P0–P3 findings after the precedence fix.

That checkpoint's offline consumer passed fresh 105-package lock resolution and
one native TypeScript public-API test, including five-crate containment, ABI 0.1.5
and source-hash rechecks. Its owned cache scratch was automatically cleaned.
Guarded cleanup removed **1,363,083,264 estimated allocated bytes (~1.27 GiB)**;
`target/` was absent and follow-up reported `already_clean`. No Cargo ran afterward.
Changes remain uncommitted at HEAD `523cd7b`. The tested content snapshot is not a
Git revision and predates these final documentation bytes. The final 237-path
source inventory and Git whitespace checks passed; read-only cache verification
reported `already_clean`, without rebuilding.
Dependencies, vendor and notice data are unchanged. See the
[batch record](docs/python-analysis.md#ruff-failure-certificate-batch-frontend-3--private-facts-5).

**Historical uncommitted codec checkpoint:** frontend
`python-ruff-private-4-py312-single-byte-codecs` / `python-ruff-private-facts-6`.
It preserves the existing uncommitted facts-5 work at actual HEAD `523cd7b`.
The pinned CPython 3.12.11 profile structurally audits 70 literal 256-code-point
tables, admitting only 62 ASCII-identity tables, with strict ASCII and existing
UTF-8/Latin-1 fast paths. Seven EBCDIC tables and cp864 remain unsupported, as do
known multibyte/stateful codecs; unknown names remain unresolved, not certified
syntax errors. The 248-path source closure adds 11 paths, including full CPython
license/per-module notices. No dependency or vendor-patch changes are involved.

All **104 Ruff tests passed, including 16 opt-in tests**, after the initial 17
focused codec passes. Codec oracles cover 15,872 approved table positions + 256 ASCII positions, 63 mixed streams,
112 AST-location comparisons, 112 complete encoded positive fact/raw-order cases,
84 certified helper SyntaxError-at-0 negatives and 10 uncertified controls (eight
helper errors, two helper admissions). All **232 preserved + 112 new = 344**
complete positive fact/raw-order cases and the existing failure/contextual/
Unicode/source/HTTP gates passed. Product package formatting and all-target
Clippy (`--no-deps`, warnings denied) passed, as did 248-path source and vendor
85-file / 65-source / 1,881,065-byte checks. Script discovery: 216 tests, 211
passed and five isolated-profile skips; explicit `-I -S -B` codec suite: 24 passed,
none skipped. Generator check and independent checker (`license_verified: true`)
passed; combined lanes cover all 216 unique script tests, not 216 default passes.
CI now runs generator check, independent checker and the isolated codec suite,
all with `-I -S -B`. Broader gates passed **682 library tests (190 ignored)**,
**18 adapter + 8 engine** tests and **165 reference + 3 native** explicit opt-in
tests. Including 16 Ruff opt-ins, **184 explicit tests** passed; known-limit
assertions are included, not all parity matches. Both full 104-Ruff and 682-library
suites passed after the test fixes, with no engine bug found. Local validation
uses macOS / Rust 1.89; hosted results remain unknown. Product formatting passed
before the current offline consumer, which passed fresh 105-package lock resolution
and one native TypeScript public-API test, five-crate containment, ABI 0.1.5 and
source-hash rechecks. Its owned scratch was cleaned. Guarded cleanup found zero
active blockers/protected outputs and removed **1,372,758,016 estimated allocated
bytes (~1.28 GiB)**; `target/` was absent and follow-up reported `already_clean`.
No Cargo ran afterward; no debug/test artifacts remain, and `.ideomancer/` JS
artifacts/global caches were untouched. The 248-path source check matched the
consumer snapshot before final documentation edits; Git whitespace checks passed.
The snapshot is not a Git revision or later-documentation attestation. Final
source/Git/cache verification is read-only, without rebuilding. Facts-6 and prior
facts-5 work remain uncommitted at actual HEAD `523cd7b`. These finite results
do not close G4 or authorize cutover. See the
[current batch contract](docs/python-analysis.md#ruff-single-byte-codec-batch-frontend-4--private-facts-6).

**Historical uncommitted visitor batch:**
`python-ruff-private-5-py312-visitor-patterns` / `python-ruff-private-facts-7`
preserves facts-5/6 work at HEAD `523cd7b`. The executable
[contract](docs/python-visitor-contract.json) and [witnesses](docs/python-visitor-witnesses.json)
close the finite inventory: 111 modern classes, 108 Module-domain classes, 188
fields and 147 node-bearing fields (130 case-unique observable-input obligations,
17 structural population/neutral witnesses). These are not 130 independent proofs;
the ten ordering obligations require the Rust full-output/raw-replay oracle.
After both parser fixes, **108 Ruff tests passed, including 17 opt-in tests**;
**344 preserved + 20 new = 364** positives matched complete arrays/IDs/diagnostics/
raw replay. All prior failure/Unicode/codec and 787-outcome contextual oracles
passed. Product formatting and all-target Clippy (`--no-deps -- -D warnings`)
passed. Scripts: **263 discovered, 257 passed, six isolation-required skips**;
isolated visitor **43** and codec **24** passed with no skips, covering all **263
unique tests across lanes**. Checker, 34 vendor tests, 253-path source and vendor
integrity passed (85 files / 65 sources / 1,882,473 bytes, same four-file patch
closure). Preservation passed **685 library tests / 191 ignored**, **18 adapter +
8 engine**, and **165 reference + 3 native** opt-ins. With **17 Ruff opt-ins**,
**185 explicit gates passed**, including known-limit assertions, not all parity
matches. **G2/G3 private implementation complete/closed for current admitted Python
3.12 Module-AST lane: closed class/field inventory + full-output/raw-replay coverage;
not complete Python compatibility, G4 bytes/admission/failure/resource/version
equivalence, or production cutover.** Local macOS / Rust 1.89 delivery closeout
passed: offline consumer fresh **105-package** lock, **one native TypeScript
public-API test**, five contained Ruff crates, ABI **0.1.5** and source-hash rechecks.
Its unique owned cache `TMPDIR` was cleaned with none left. No further code changes
are planned; actual changes are narrow parser predicates/separators and proof
files, not a visitor/normalizer rewrite. Data, dependencies and licenses are unchanged.
The pattern-start correction adds AST-only class-first/later-sequence children;
singleton-first elements were already accepted in both modes. The independent mandatory-comma correction
rejects previously erroneously accepted malformed sequences in both modes, including
default mode. No visitor rewrite, dependencies or public behavior/default change
beyond that private parser grammar correction. Singleton-sequence controls now
honor upstream acceptance with false/true AST-only settings in both constructors;
missing-separator controls pass in both modes/constructors, not weakened tests.
Guarded preflight found **zero blockers / zero protected outputs**, normal
`debug`/`tmp` only. Cleanup removed **1,374,564,352 estimated allocated bytes
(~1.28 GiB)**; `target/` absent, follow-up `already_clean`, no Cargo afterward.
No owned debug/test artifacts remain; `.ideomancer/` JS artifacts/global caches
were untouched. Source 253, vendor 85/65/1,882,473 bytes and Git diff checks passed
before final docs. Consumer snapshot (hash in the ledger) is not a Git revision
or later-docs attestation. Final source/Git/helper checks are read-only, no rebuild.
Nothing is staged, committed or pushed; HEAD remains `523cd7b`. Hosted/platform
validation remains unknown. See the [current ledger](docs/python-analysis.md#ruff-visitor-contract-batch-frontend-5--private-facts-7).

**Historical prior checkpoint STOP (subsequently superseded for F0 and bounded F1):** “STOP parity
expansion after this bounded gate.” The user explicitly selected full replacement
and authorized the recommended bounded byte-aware Ruff feasibility/version study.
See the [main-owned ADR](docs/python-native-replacement-feasibility.md): F0 investigation complete, conditional
GO by source audit, **not a thin adapter or compiled pass**. The proposed byte-backed
lexer and narrow checked source views retain Ruff grammar; concrete lexer state,
lookahead/rewind/re-lex, `Chars`/`&str` and debug-string ranges imply an expanded
maintained fork and possible AST/API changes beyond the current four-file policy.

**At F0 handoff: separately bounded F1/F2 proposal, not implementation approval.**
F1 and bounded F2 have since been authorized and compiled as recorded above;
this historical proposal does not describe current authorization.
Proposed immutable full-bundle profiles need exact patch membership, a release
support window and product approval. Production follows the selected interpreter's
grammar/stdlib/version token and snapshot, dynamically >=3.12 and not CPython-only;
no finite profile preserves all future/arbitrary behavior. Legacy `Python { python }`
continues through the helper; no native probe/fallback, fake `pythonVersion` or
old-save capability upgrade. F0 grants no expanded production vendor patch, source
repair, tool installation, public selector, finite-profile policy or cutover approval.
The 219-input reference oracle is not native parity. F0 reference/script gates,
256-path source integrity, unchanged vendor integrity and Git whitespace checks
passed; see the [F0 ledger](docs/python-analysis.md#f0-byte-input-and-version-feasibility).
Independent architecture review found no blocking correction; F1 cursor semantics
and F2 BOM-coordinate anchors remain future proof obligations. No Rust/consumer
rerun or cache removal was needed: helper `already_clean`, owned scratch cleaned
with none left. Facts-7 validation/cleanup remains historical.

**Standing architecture constraint (unchanged text interface):** CPython's admission of malformed non-ASCII UTF-8
comment bytes cannot be reproduced with unchanged Ruff's `&str` input and no source
rewriting. F1 demonstrated the lexical seam; bounded F2 now implements private
parser/source/debug integration. Production compatibility still requires separate
acceptance—not permission to narrow production's
Python >=3.12 contract. Existing codec, type-comment and platform/resource gaps
remain; private G2/G3 implementation is closed, but G4–G6 and public cutover are not.
Contextual type-comment errors can be `IndentationError` (including for/with/class
suite cases); legacy syntax status/line is not an exception certificate and remains
`failure: None`. Another PEG parser alone cannot close arbitrary type-comment
failure locations/classes. This is a G4 obligation, not new visitor work.

- Pin Tree-sitter 0.25.10, JavaScript grammar 0.25.0 and TypeScript/TSX grammar
  0.23.2 exactly, plus language ABI bridge 0.1.5 directly for downstream Rust 1.89
  resolution compatibility; build native C with Rust bindings and review license notices.
- Preserve immutable facts, binding/module/class proof, UTF-16 receipt coordinates,
  redaction, adapter boundaries, and saved output contracts. Use new identity
  `javascript-typescript-tree-sitter-1`; legacy static-1 through static-6 stay
  generically readable without granting new capabilities or upgrading old claims.
- Rediscovery changes snapshot identity and invalidates prior derived claims.
  Syntax `ERROR`/missing nodes, including module-only files, make evidence incomplete.
- Reject obsolete Node/compiler CLI settings and the legacy engine request before
  source access; never silently ignore them or dispatch an old extractor.
- Validate the cooperative 60-second extraction deadline, 200000-node traversal
  budget and depth-256 bound without describing them as a hard memory sandbox.
  Rust 1.89 is the declared MSRV; record actual matrix results separately.
- Local closeout on 2026-11-13 covers grammar-aware JS/JSX fields and arrow methods,
  getter/setter execution boundaries (including split newline/comment and TS
  `override` forms), and static/object methods named `constructor`. The standalone
  package consumer exercises fresh dependency resolution, typed facts, validated
  synthetic claims, stale-claim rejection and source-deleted diagram reuse. See the
  [validation record](docs/javascript-typescript-analysis.md#validation-tooling).
- P1's initial captured-source AST fixtures check exact fact arrays/IDs, redaction,
  binding guards, decoding, per-file rollback and replay. The next private parse-only
  front-end pins Python grammar 0.25.0 with the existing runtime 0.25.10 and ABI
  bridge 0.1.5. Its parse report remains classification-only. A separate private
  candidate entry point started with exact module/function/parameter/signature facts
  only when the entire file fit a declaration-only whitelist; every rejected file
  retains only its module baseline. No candidate becomes Evidence or a trusted
  snapshot. Grammar acceptance is not Python AST acceptance; encoding declarations,
  extended class forms and unreviewed imports remain outside this fact slice. Supported annotations and
  literal defaults use bounded redacted AST-shaped expressions. A body slice now
  retains every admitted call plus simple assignment bindings/target-only
  invalidations; special helper metadata/diagnostic triggers and unreviewed forms
  still reject the whole file. A conservative import expansion retains
  exact metadata and now reproduces alias-aware dynamic-discovery warnings. The
  warning lookup preserves the AST helper's scope-name persistence and one-step
  head substitution, not runtime binding or general alias resolution. Absolute
  constructor imports still reject the file, even when unused. Parenthesized multiline `from` lists preserve comments as
  nonsemantic trivia and extend spans through the closing delimiter; unsupported
  layout/grammar discrepancies remain explicit rejections. Header calls remain
  unsupported. Plain classes now act as lexical containers for methods and admitted
  statements; nested class/function ownership, name-keyed aliases and declaration
  spans remain exact. Named-reference bases and supported annotated assignments now
  retain exact base/attribute facts; annotation-only statements do not invent writes.
  These are syntax facts, not inheritance resolution, decorator/metaclass
  interpretation or inferred class/receiver semantics. No public toolchain variant,
  production evidence identity or extraction default changes. See the
  [Python parity gates](docs/python-analysis.md#native-migration-parity-foundation).
- Python migration must preserve language-specific semantics, not merely accept
  syntax. Rust facts must distinguish structs, traits and impls rather than calling
  everything a class. Neither backend may execute target modules, `build.rs`,
  procedural macros, plugins, or other source side effects.

## I0 — Preserve And Review The Foundation

### Available locally

- Rust library `ideomancer_core::engine::v1` with typed discovery, verified catalog
  browsing, and saved-source class-diagram operations.
- Existing standalone CLI, with implementation modules kept private.
- Opaque catalog handles and source/snapshot identity checks before publication.
- Generic language bindings, signatures, constructor observations, grouped UML,
  provenance, bounded output, and atomic artifact publication.
- MIT license declaration and package metadata.
- Positive package inclusion and an exact file-inventory check in CI; unrelated
  generated diagrams and unused artwork are excluded from crates. This is not
  repository/history, content, asset-ownership, or dependency-license clearance.
- A [source-only fresh baseline](docs/lineage.md) in the existing checkout; old
  ancestry and unverified artwork are not included in the new root commit.

### Remaining release work

- Review and checkpoint the current changes before expanding the API further.
- Audit distributable source, examples, images, documentation, generated files,
  and relevant history for material not appropriate for public distribution.
- Confirm package contents, dependency licenses, supported toolchain versions,
  and which capabilities have actually been validated.
- Follow the [release checklist](docs/releasing.md); prior lineages retain retired
  behavior and need a separate disposition plan. This baseline does not erase them.
- Use the [dated readiness review](docs/release-readiness.md) to distinguish the
  successful hosted baseline from unvalidated closeout changes, actual source
  artifact integrity from path selection, and source approval from binary clearance.
- Distinguish local implementation, published API availability, and release status.
  A path dependency does not pin source bytes or establish production approval.

**Exit:** reviewed baseline and consumer build instructions. Commits, publication,
remote changes, and visibility changes require explicit authorization; this roadmap
does not perform or authorize them. I1 design can proceed alongside this review.

## I1 — Define A Small Extension Contract

**Candidate implemented:** the [facts/claims contract](docs/adapter-contract.md)
and [schema](docs/adapter-contract.schema.json) define additive APIs under
`engine::v1`, without changing the existing capability/report DTOs. Opaque fact and
validated-claim handles, generic factory structure, explicit relationship sites,
claim validation and saved-claim loading are implemented locally. This is not a
published API release or approval of any external framework contract.

The contract must define:

1. **Required structural facts:** stable declaration/binding IDs, lexical ownership,
   imports/exports, function parameters and returns, class factories, assignments,
   references, and source locations. A fact such as “this function returns this
   class” is generic; a framework-specific service classification is not.
2. **Read-only snapshot access:** typed views tied to a verified source root and
   snapshot, not public mutable implementation structs or arbitrary evidence edits.
3. **Derived relationship claims:** source/target IDs, relationship kind, adapter ID
   and version, contract identity, evidence references, and explicit assumptions.
4. **Basis categories:** source syntax, supported framework contract, explicit
   configuration, or hypothesis. These are not numerical confidence percentages
   and must not be silently presented as runtime observations.
5. **Literal/privacy policy:** default redaction, narrowly bounded identifier
   retention where necessary, and how an approved consumer declares required facts.
   Do not expose unrestricted source/default values as an extensibility shortcut.
6. **Capability negotiation and failure:** unsupported versions, missing facts,
   incomplete scope, ambiguity, and invalid references have explicit outcomes.

**Exit:** documented types, versioning, safety rules, and positive/negative synthetic
examples. One real consumer requirement and an alternate synthetic convention must
fit the same generic contract. This is not a requirement to build a plugin ecosystem.

## I2 — Implement The Fact/Relationship Boundary

The consumer can integrate against the candidate implementation before I2 closes;
engine and adapter acceptance are exercised together, not a requirement for each
side to be finished before the other can start.

Implement only the I1 surface needed for the first class-diagram integration:

- Expose the agreed language-level facts without framework-specific names or rules.
- Accept typed derived relationships referencing existing snapshot declarations.
  Validate kinds, source/target IDs, provenance, scope, and completeness requirements.
- Keep original evidence immutable. Store consumer-derived claims separately and
  compose them into the diagram model under an explicit basis/contract.
- Preserve every selected class's home-diagram coverage, source references,
  readable grouping, and explicit member/relationship image omissions.
- Include snapshot, engine/model policy, adapter/contract versions, relevant
  specification identity, and claim-set hash in derived-output cache identity.
- Support deterministic, source-only rendering from saved approved facts/claims
  after target source and extraction tools are unavailable.

### Acceptance gates

- An external adapter changes useful inheritance/collaborator relationships through
  this API without calling legacy CLI profiles or patching the renderer.
- Unknown IDs, incompatible snapshots/roots, conflicting claims, unsupported
  capabilities, and incomplete required evidence fail clearly before publication.
- Equivalent inputs produce deterministic models and byte/mtime-stable reuse;
  changed adapter/specification inputs cannot reuse stale derived diagrams.
- Failed rendering, unsafe paths, unrecognized files, symlinks, or concurrent
  modifications do not overwrite the prior valid artifact bundle.
- Synthetic fixtures preserve redaction, resource bounds, offline behavior, and
  the distinction between framework interpretation and runtime execution.

Organizational service/query and custom router recipes have been removed from new
extraction and interpretation. Public framework/vendor rules are separated into
compiled adapters. See [migration and ownership](docs/adapters.md) for the explicit
retirement of old profile behavior and the limits of class-only replacements.
Two external compiled conventions exercise the same class seam; neither supplies
new engine special cases. Actual private-source acceptance remains consumer-owned.

## I3 — Release A Dependable Engine

- Exercise the contract from a separate consuming repository, not just internal
  unit tests or a path-dependent example in this repository.
- Run the supported platform/toolchain matrix and publish compatibility expectations
  for Rust, API DTOs, facts, model schemas, and saved artifacts separately.
- Record known failing/unavailable optional checks instead of reporting them as
  passing. Agent integration is a separate compatibility lane unless it is included
  in the release's declared capabilities.
- Document upgrades, deprecations, and any unsupported downgrade behavior.
- After review and authorization, publish an immutable revision/release containing
  the API; consumers can pin it directly or use an approved internal mirror.
- Keep consumer fixtures, identifiers, configuration, and generated metadata out
  of public releases. A fork or mirror does not by itself establish approval or
  equivalence; verify it against the same contract tests.

**Exit:** a consumer can replace its development path with a real approved source
pin without changing its adapter behavior. Do not invent an unpublished revision
to make this milestone appear complete.

## I4 — Expand Only After The Boundary Is Proven

Potential later work includes additional language adapters, framework adapters,
call/data-access relationships, and cross-codebase models. Each needs an independent
use case, an explicit semantic contract, and observable improvement in diagrams.
Existing parsers must not be described as general runtime or whole-program analysis.

Do not prioritize a new UI, editor integration, a dynamic plugin host, a general YAML
programming language, more output modes, or additional agent/provider integrations
ahead of demonstrating the external class-relationship integration.

## First Work Session

1. Checkpoint and review the current I1/I2 implementation and compatibility changes.
2. Complete current-package and historical public-distribution review before release;
   working-tree cleanup is not historical clearance and does not authorize a rewrite.
3. Exercise additional approved consumer cases and the supported toolchain matrix,
   then publish a real immutable dependency revision only after authorization.

Changes in priority or acceptance criteria should update this file before expanding
scope. Passing tests and extracting more facts are supporting evidence, not the
definition of a useful software-understanding tool.

## References

- [Current engine API](docs/engine-api.md)
- [Current serialization schema](docs/engine-api.schema.json)
- [Existing discovery and diagram behavior](docs/discovery.md)
- [MIT license](LICENSE)
