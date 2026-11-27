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
| P1 | Native Python backend | Private Ruff frontend-2/facts-4 implements finite Unicode-15, named-escape, contextual type-comment and UTF-8/Latin-1 compatibility; all 75 Ruff tests, broader tests, final formatting/type-comment/Clippy reruns and exported-checkout consumer passed; closeout status in Python ledger; production AST helper retained | Complete the [Python completion gates](docs/python-analysis.md#completion-contract-and-remaining-gates); retain the AST helper until parity, compatibility, integration and cutover acceptance pass |
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

Python completion now follows a finite six-gate program, rather than an open-ended
queue of syntax slices: **G1** generic expressions/ordinary bodies; **G2** remaining
generic traversal; **G3** existing discovery/FastAPI metadata; **G4** grammar,
encoding and failure compatibility; **G5** native identity/product integration;
**G6** existing-engine usefulness, operations and approved cutover. See the
[contract, inventory and blockers](docs/python-analysis.md#completion-contract-and-remaining-gates).
Development's Unicode generation/check and CI oracle use exact CPython 3.12.11 /
Unicode 15.0.0; this oracle pin does not narrow production's trusted Python >=3.12
contract. Test counts do not establish completion.

The current uncommitted integrated batch is `python-ruff-private-facts-4` with
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
malformed UTF-8 comment admission, format-spec exception taxonomy, exact failures
and resource closure remain open. Invalid named escapes now use ordinary token
opening lines or matching f-string closing lines, preserving nested-failure
precedence. G2/G4–G6 are not closed.

Four Ruff source files now carry the maintained patch: default-false AST-only
duplicate-keyword policy, bare f-string mapping keys only in AST-only mode, and
TypeVarTuple/ParamSpec default version checks matching TypeVar (PY312 errors with
either AST-only setting, PY313 accepted). The five local crates and Git delivery
with `publish = false` are unchanged. Current provenance is 85 vendor files / 65
source files / 1,881,065 bytes; the source inventory is 235 paths, including pinned
Unicode inputs, generated data, notices and generation/check tooling.

**Current integrated evidence:** all 75 Ruff tests (including nine opt-in tests)
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
