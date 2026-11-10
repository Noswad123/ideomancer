# Adapter ownership and migration

## Supported boundary

Language extraction records syntax and binding structure. Framework and vendor
interpretation is separate, and consuming tools own organizational conventions.
The [facts/claims API](adapter-contract.md) is the supported external class-model
extension; its [schema](adapter-contract.schema.json) defines serialized DTOs.

The compiled public adapters currently separate direct FastAPI context/converter
rules and public SDK recognition into `src/adapters`. They do not supply private
codebase registration rules. Their presence does not imply general framework,
network, injection or runtime understanding.

External adapters are ordinary code in the consuming tool. Configuration selects
reviewed built-ins; it cannot load repository programs or mutate engine evidence.
Two independent consumer conventions exercise the class seam: provider-contract
interpretation and explicit declaration/binding configuration.

## Removed experimental behavior

This is an intentional compatibility change, not a silent fallback:

- `--js-profile` is no longer a supported interpretation path. A compatibility
  parser returns an actionable removed-feature error before opening a catalog.
- Ordinary JavaScript discovery no longer creates organizational service-request,
  provider-layout or ORM-query receipts. Nonstandard asynchronous-router method
  suffixes are not interpreted as HTTP declarations by the generic route matcher.
- Python discovery no longer recognizes custom router-registration/pluralization/
  factory recipes. Those naming/storage conventions are not standard FastAPI.
- The class-claim extension does not replace former operation/query diagrams or
  custom router-prefix interpretation. Such outputs require their own reviewed
  consumer adapter and generic output contract.

Generic declarations, imports, ownership, constructor observations, signatures,
source-grounded class relationships, direct FastAPI behavior, local API contracts,
proof budgets and saved-source rendering remain supported.

## Extraction and saved-data compatibility

Native JavaScript extraction uses `javascript-typescript-tree-sitter-1` and
new Python extraction uses `python-static-8-generic-facts`. Evidence envelope
versions are unchanged; extractor generations are not envelope versions.

Legacy static-1 through static-6 catalogs remain readable for supported generic
behavior, without upgrading static-6 claims to new capabilities. Explicit native
rediscovery changes identity and invalidates prior snapshot-bound claims. The
migration preserves fact/proof/output contracts, not old extractor identity.
Retired receipt
kinds are not reinterpreted as approved framework rules. Exported factory facts
require the new JavaScript capability and explicit rediscovery; do not synthesize
them from old profile receipts or edit evidence/receipts to enable an adapter.

Facts are immutable and snapshot-bound. Derived claims are separately validated
and saved. Adapter, contract, implementation, configuration and claim-set identities
participate in reuse even when claims are empty or resulting edges are unchanged.
`SourceSyntax` is accepted only for a target independently established by the
engine; external interpretation must use its actual basis. Hypotheses cannot
enter authoritative composition in this initial extension.

## Release and privacy review

Current source and distributable fixtures have been separated from internal study
material. This does **not** erase historical commits, previously published releases,
forks or caches. Historical review remains a release gate; no history rewrite or
remote publication is authorized by this migration.

Never copy consumer specifications, private fixtures, repository paths or generated
source metadata into public tests. Public tests use independently synthetic facts
or documented public framework/vendor interfaces. Source metadata is not a secret
detector, and a framework-contract acknowledgement is not organizational approval.
