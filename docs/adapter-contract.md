# Facts and class-relationship extensions v1

This is the implementation contract for the first external adapter integration.
The existing `engine::v1` discovery/browse/class API remains available. New APIs
are additive; adapters are compiled into consuming tools, not loaded from target
repositories or executed through configuration hooks.

## Ownership

The engine owns language syntax, snapshot integrity, grounding, resource bounds,
claim validation, model composition and publication. Consumers own framework
rules, contract approval, codebase configuration and adapter execution. Core
validation establishes structural consistency, not semantic truth or company
authorization. A framework-contract relationship is not a runtime observation.

## API shape

The extension API lives under `ideomancer_core::engine::v1`:

```rust,ignore
pub fn extension_capabilities() -> ExtensionCapabilities;
pub fn open_facts(catalog: &CatalogRef, request: &FactsRequest)
    -> Result<FactsRef, ExtensionError>;
pub fn validate_claims(facts: &FactsRef, sets: &[ClaimSet])
    -> Result<ValidatedClaimsRef, ExtensionError>;
pub fn class_diagrams_augmented(
    catalog: &CatalogRef, selection: &ClassSelection,
    claims: &ValidatedClaimsRef, render: &RenderMode,
) -> Result<AugmentedClassDiagramReport, ExtensionError>;
pub fn save_claims(claims: &ValidatedClaimsRef, directory: &Path)
    -> Result<(), ExtensionError>;
pub fn load_claims(facts: &FactsRef, directory: &Path)
    -> Result<ValidatedClaimsRef, ExtensionError>;
```

`FactsRef::view()` returns `&FactsView`. Both reference types have private fields,
are non-deserializable and bind verified root, source snapshot and fact-content
identities. Owned DTO copies are data, not trusted evidence. Actions revalidate
identities before publication; source files are not reopened by cached actions.

## Minimum facts

`FactsView` uses schema `ideomancer.facts.v1` and contains:

- `identity`: source-root ID, snapshot ID, extractor version, inventory hash and
  canonical facts hash.
- `complete`, capability coverage, bounded diagnostics and root-relative `files`.
- `classes`: `ClassFact { id, declaration_id, name, symbol, path, line, end_line }`;
  `id` is a catalog class ID, not a declaration/fact ID.
- `factories`: `FactoryFact { id, callable_id, module_path, parameters,
  result_kind, returned_class_id, return_fact_ids, evidence_ids, safe, reason }`.
  Results distinguish `class`, `instance`, `unsupported` and `ambiguous`.
- `parameters`: contained in each factory, with source ID, optional binding ID,
  name, position and optional/rest/default-presence flags; no default values.
- `relationship_sites`: `RelationshipSite { id, source_class_id, owner_id,
  owner_kind, binding_id, binding_name, binding_origin, kind, member_name,
  target_class_id, evidence_ids }`. Sites distinguish inheritance, dependency
  and construction and preserve already-established syntax targets.
- `sources`: fact ID to source citation. Every referenced ID must be grounded in
  this saved snapshot. IDs are stable within a snapshot, not promised across edits.

New generic JavaScript extraction records exported function/factory structure,
direct returns and referenced class identity without interpreting provider naming,
service directories, request injection, routing mounts or ORM calls. Multiple,
conditional, unsupported or mutated exports/returns do not become unique results.
Missing extraction capabilities require explicit rediscovery, not fabricated facts.

The first extension requires no arbitrary literal retention. Source names, member
keys and module specifiers retain their existing structural privacy policy; payload,
default and configuration values remain redacted. Future literal-policy expansion
requires a separate reviewed capability and snapshot identity.

## Claims

`ClaimSet` uses schema `ideomancer.claims.v1`, with `facts` identity, `adapter`
identity (ID/version/implementation), one `contract` identity (ID/version/spec hash),
`configuration_hash`, `claims` and diagnostics. A `ClassClaim` has:

```text
id, site_id, source_class_id, target_class_id, kind,
basis, evidence_ids, rule_id, assumptions
```

Initial authoritative kinds are `extends` and `depends_on`. Basis distinguishes
`source_syntax`, `framework_contract`, `explicit_configuration` and `hypothesis`.
Hypotheses are rejected by authoritative composition in this first version.

Validation rejects wrong roots/snapshots/fact hashes; unknown endpoints/sites or
evidence; incompatible site kinds; absent required capabilities/completeness;
missing adapter/contract/configuration identities; and conflicting resolutions.
A claim cites its source site and target declaration plus its supporting facts.
It cannot override an established syntax target with a different class. Equivalent
claims can share a drawn edge while retaining all provenance. No last-writer-wins.

The consumer is responsible for the interpretation expressed by a contract. The
engine never resolves private providers through global short-name similarity or
file naming. Explicit consumer configuration is labeled as such, not source syntax.

## Composition and cache

Claims compose after generic relationship grounding and before grouping/layout.
They resolve a particular site, not by rewriting Mermaid or saved source evidence.
The original observation, claim basis, adapter/contract/rule and assumptions remain
visible in metadata. Identified targets can replace scoped unresolved boundaries;
selection still determines home/context classes and existing omission accounting.

Derived identity includes facts, adapter implementation/version, contract/spec hash,
configuration hash, canonical claim-set hash and composition policy, even if an
adapter emits an unchanged or empty edge set. Saved claim data is separate from
source evidence and verified on reopening. Source-only rerendering requires neither
live source nor adapter execution. Existing atomic publication, stale-handle checks,
safe paths, private-data policy and engine resource ceilings remain mandatory.

## Transition

New extraction stops producing organization-specific profile receipts. Legacy
catalogs remain generically readable where supported, without reactivating retired
interpreters. The former framework-profile CLI is an explicit removed feature,
not silently ignored. The class extension does not claim to replace old route or
operation/query views; those require distinct adapter/output contracts.
