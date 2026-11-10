# Ideomancer Roadmap

## Purpose And Ownership

Ideomancer is a public, MIT-licensed engine and CLI for source-grounded software
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
| I2 | Read-only facts and validated class-model augmentation | Implemented locally; synthetic consumer integration validated | Preserve immutability, identity checks and provenance; real consumer acceptance remains separate |
| I3 | Harden and release the consumable engine | **Next: release and history review** | An approved immutable release works as an upstream or mirrored dependency |
| P0 | Shared parser backend and native JS/TS Tree-sitter migration | Implemented locally; full local regression suite passing on Rust 1.89; hosted matrix pending | Preserve fact/proof/output contracts; no Node discovery prerequisite; package/consumer checks and supported-platform release validation |
| P1 | Python Tree-sitter backend | Next parser milestone; not implemented | Preserve Python facts, FastAPI boundaries and explicit incompleteness; retain current Python >=3.12 AST helper until replacement is validated |
| P2 | Rust Tree-sitter backend and language facts | Next parser milestone; Rust analysis not implemented | Model Rust structs, enums, traits, impls, functions and modules honestly; no build scripts or macro execution |
| I4 | Broader language/model capabilities | Later | Independent use cases justify each extension without weakening the boundary |

P0 is the current parser priority, not postponed behind publication. This working
tree is uncommitted/unpublished; no milestone authorizes a commit, push, or release.
Python and Rust Tree-sitter are roadmap work, not a claim that all languages are
native today. Optional Mermaid SVG rendering and browser checks still use external
Node/Chromium tooling independently of JS/TS discovery.

### Parser Acceptance

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
