# Reusable engine API v1

The Rust package remains `ideomancer`; its library target is `ideomancer_core`.
Consumers use `ideomancer_core::engine::v1`. Implementation modules remain private.
The existing executable retains its commands through a CLI-only entrypoint.

This initial library surface is intentionally narrower than the CLI: static
discovery, saved catalog browsing, and class diagrams only. It exposes no agent,
model, arbitrary arguments, framework profile, OpenAPI, acceptance, or retry API.
Trusted extraction and SVG tools may run; target applications are never executed.
The library is not an OS sandbox and source metadata must remain private.

## Contract

`API_VERSION` is `ideomancer.engine.v1`. Reports use a camel-case
`schemaVersion` discriminator. Cargo version, extractor versions, and saved
artifact schema versions are separate identities.

```rust,ignore
pub fn capabilities() -> Capabilities;
pub fn discover(request: &DiscoverRequest) -> Result<DiscoveryReport, EngineError>;
pub fn open_catalog(path: impl AsRef<std::path::Path>) -> Result<CatalogRef, EngineError>;
pub fn browse(catalog: &CatalogRef, options: &BrowseOptions) -> Result<BrowseReport, EngineError>;
pub fn class_diagrams(
    catalog: &CatalogRef,
    selection: &ClassSelection,
    render: &RenderMode,
) -> Result<ClassDiagramReport, EngineError>;
```

- `Toolchain::Python { python: PathBuf }` and
  `Toolchain::JavaScriptTypeScript { node: PathBuf, typescript: PathBuf }` keep
  invalid tool/language combinations out of the API.
- `DiscoverRequest { source_root, output_container, toolchain }` launches only
  the explicitly selected trusted extractor. It does not render or contact a provider.
- `DiscoveryReport` carries `schema_version`, `catalog_directory`, `source_root`,
  `snapshot_id`, `complete`, and `warnings`.
- `CatalogRef` is an opaque, non-deserializable handle with read-only methods
  `directory()`, `source_root()`, `snapshot_id()`, and `is_complete()`.
  Every operation reloads verified artifacts. Class publication checks the handle's
  source root and snapshot under the catalog lock before writing anything.
- `BrowseOptions { kind, query, path, limit, offset }` defaults to 20 items;
  its limit is 1..=100. `ItemKind` has `Api`, `Class`, and `Workflow` variants;
  browsing workflows never invokes analysis.
- `BrowseReport` carries `schema_version`, `snapshot_id`, `complete`,
  `total_items`, `matched_items`, `offset`, `limit`, `next_offset`, `counts`,
  `items`, and `warnings`. `CatalogCounts` has `api`, `classes`, and `workflows`.
- `ClassSelection::All`, `Entry { id }`, and
  `Page { query, path, limit, offset }` distinguish complete UML from explicit focus.
  Entries use exact class catalog IDs; page limits are 1..=30.
- `RenderMode::SourceOnly` is the default. `Svg { mmdc: PathBuf }` authorizes an
  explicitly selected trusted renderer, never browser opening or tool installation.
- `ClassDiagramReport` carries `schema_version`, `catalog_directory`, `source_root`,
  `snapshot_id`, `complete`, `selection`, `coverage`, `diagrams`, `viewer`, `legend`,
  `view_directory`, `views_index`, `reused`, and `warnings`.
  Each `DiagramArtifact` has `id`, `title`, `mermaid`, and optional `svg` paths.
  `ClassCoverage` preserves class coverage, modeled/drawn/omitted relationship
  counts, and visible/hidden field-projection counts.
- `EngineError` exposes `code` and `message`; operation-level codes are
  `invalid_request`, `catalog_unavailable`, `discovery_failed`, and
  `class_diagram_failed`. Diagnostics may contain local paths; they are not telemetry.

Request/configuration DTOs reject unknown fields. Neither arbitrary command strings
nor executable hooks are accepted. Consumers own codebase registration, output
ownership, source-root authorization, and company/framework adapters.

For serialized API DTOs, the JavaScript language identifier is
`javascript_typescript` (distinct from the CLI's `javascript-typescript` spelling).
Class selection and rendering use a `kind` tag: `all`, `entry`, `page`, and
`source_only` or `svg`. The serialization schema is authoritative for those wire
spellings. A catalog handle is never deserialized from a response or caller JSON.

## Dependency source

A local path dependency is appropriate while this API is uncommitted/unpublished.
Do not claim that an existing upstream revision contains these changes. After
publication, consumers can pin an immutable upstream revision or use an approved
internal mirror with the same package/API. No remote or visibility changes are
part of adding the library target.

Generic read-only facts and validated class augmentation are available through the
additive [extension contract](adapter-contract.md) and its separate capabilities
DTO. Existing v1 discovery/browse/class operations remain unchanged. Retired CLI
profiles are not extension hooks; see [adapter ownership and migration](adapters.md).
