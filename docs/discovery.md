# Discovery And Diagram Chunks

Understand existing Python or JavaScript/TypeScript software a subset at a time: discover locally, list
candidates, start with all saved catalog classes, focus an API/class page or a selected callable view, then optionally ask an agent about **one**
selected workflow. These are explicit progressive commands, not a background job
stream, automatic batch of all workflows, or all-view generator.

## Overview

Catalogs, views, and analyses use readable application names and keep only current
artifacts, not snapshot history. Hashes and format versions remain in receipts.

## Discover

```sh
ideomancer discover /path/to/app
ideomancer discover /path/to/app --kind workflow --query submit --limit 20 --json
```

`discover ROOT [--out BASE] [--python PROGRAM]` scans source with a trusted Python
>=3.12 AST helper on Unix (Linux/macOS). It imports or executes no application code
and invokes no agent or renderer. The default interpreter is `python3.12`,
overridable by `IDEOMANCER_PYTHON` or the explicit flag. The output container
defaults to `.ideomancer/catalogs` relative to the invocation cwd, not the app.
Parents and the application-named current catalog directory are created automatically.

Python remains the default. For the offline JS/TS source-navigation MVP:

```sh
ideomancer discover /path/to/app --lang js
```

`--language` aliases `--lang`; language aliases are `js`, `javascript`, `ts`, and
`typescript`. Discovery uses native compiled-in Tree-sitter, not Node or an external
TypeScript compiler. Hidden obsolete `--node` / `--typescript` and
`IDEOMANCER_NODE` / `IDEOMANCER_TYPESCRIPT` settings produce actionable errors;
remove/unset them. Nothing is installed or resolved from target dependencies/configuration.
See [JS/TS analysis](javascript-typescript-analysis.md) for syntax-only parsing,
fixed exclusions, bounds, evidence v2, and unsupported framework/runtime behavior.

Python source scope selects `.py` files under `ROOT/src` when present, otherwise `ROOT`;
it does not add root-level scripts alongside `src`. Fixed exclusions include `scripts`, `shadow`,
`tests`, `migrations`, virtual environments, caches, and generated/output directories.
There are no include/exclude flags. See [the extraction boundary](python-analysis.md#current-extraction-boundary)
for the full list, filesystem safeguards, and resource limits. Completeness is
about that selected source scope, not all software in the repository.

The Python catalog contains API-handler candidates from recognized decorator syntax,
workflow candidates from recognized HTTP/CLI/job decorators and public module
functions/direct class methods, and class declarations. Public callables are
**workflow candidates**, not discovered complete business workflows. No runtime
activation or complete call graph is inferred. Catalog paths remain fragments;
native endpoint views separately derive only supported source-prefix conventions.
Dynamic route paths/methods remain unknown with diagnostics.
Python framework-aware scope is FastAPI-only, not Django/Flask support; ordinary
Python callable candidates remain available. JS/TS catalogs retain classes, public
callables, and a limited injected CommonJS router registration pattern with explicit
handler IDs. They do not prove framework activation or full mounts.

The JS/TS slice covers API/class/callable navigation, syntax-proven same-file calls,
bounded cross-file named-function call candidates, and named-class UML relations
from selected saved evidence.
It is not JS runtime, DI, network, or agent analysis, nor completion of a whole
private application. Python/FastAPI behavior is unchanged. See the
[bounded acceptance matrix](javascript-typescript-analysis.md#bounded-mvp-acceptance-matrix)
for supported and deferred capabilities.
The native parser is an implemented local candidate with the full local regression
suite passing, not a release. Hosted platform validation remains pending; see
[parser milestones](../ROADMAP.md#parser-acceptance).

The first page defaults to 20 entries. `discover` accepts the same filters as
`catalog` below; filters change the displayed page, not the scan scope or saved
catalog. Use its reported `catalogDirectory` as `SAVEDDIR` in later commands.

## Browse

```sh
ideomancer catalog SAVEDDIR --kind workflow --query submit --limit 20
ideomancer catalog SAVEDDIR --kind api --path src/app --offset 20
ideomancer catalog SAVEDDIR --kind class --limit 10 --json
```

`catalog SAVEDDIR` verifies saved artifacts and browses stored evidence without
Node, a compiler, Python, a live source rescan, an agent, or authentication.

- `--kind api|workflow|class` restricts the candidate kind.
- `--query TEXT` is a literal case-insensitive substring search over label, symbol, path, HTTP method, and route fragment, not a regex or semantic search.
- `--path PREFIX` is a root-relative source-path prefix, not an absolute path or HTTP route query. Parent traversal is rejected; path matching is case-sensitive.
- `--limit` accepts 1 through 100, default 20; `--offset` defaults to 0.
- Markdown is the human-readable default; `--json` returns `catalogDirectory`, `snapshotId`, `complete`, and `page`.

Page metadata includes `totalItems`, `matchedItems`, per-kind `counts`, `offset`,
`limit`, `nextOffset`, `items`, `snapshotId`, `complete`, and `warnings`. Counts are
inventory metadata, not business-workflow coverage. Entries carry stable IDs,
kind/role, symbol, source path and line range, evidence references, and API metadata
where applicable. Inspect diagnostics in saved `evidence.json` as well as warnings.

An incomplete extraction can still publish a diagnostic catalog; `discover` and
`catalog` return its page but exit 1 when `complete: false`. Fatal extraction/storage
errors can prevent publication. A partial or empty page means only what the saved
scope and filters support, not that there are no other workflows. Dynamic-discovery
limitations alone do not make the source scan incomplete.

## Source Views

```sh
ideomancer diagram SAVEDDIR --kind class --no-render
ideomancer diagram SAVEDDIR --kind api --query orders --limit 12 --no-render
ideomancer diagram SAVEDDIR --kind class --path src/app --limit 12
ideomancer diagram SAVEDDIR --kind class --offset 12 --no-render
```

API/class projections use only the saved snapshot, with no extraction Node/compiler/Python,
rescan, agent, model, or auth access. SVG rendering independently requires trusted
`mmdc` and its renderer toolchain; `--no-render` avoids those. `--query` and `--path`
use catalog semantics; `--limit` is
1 through 30, default 12, and `--offset` defaults to 0 for API and focused class pages.
Unfiltered class requests without explicit paging instead draw all-class UML below.
Until rediscovery replaces it, the saved snapshot can be viewed after
live source changes; snapshot IDs and references describe saved source, not freshness.

Full and focused class views use grouped native Mermaid `classDiagram`, with
observed members, declared-base generalizations, and supported source dependencies.
They are not exhaustive attribute/type models, runtime DI, or composition models.
See the class-specific report, proof boundaries, and caps below.

API views use a native Mermaid flowchart from route candidates to handlers, not
a runtime request topology. Paths are **fragments**, not actual full URLs;
activation, deployment hosts, and dynamic registrations are unresolved.
Unknown methods/paths stay unknown rather than being guessed.

`--no-render` writes Mermaid, a JSON legend, and an offline HTML diagram viewer without
requiring Mermaid CLI. Otherwise trusted `mmdc` generates SVG; configure
`IDEOMANCER_MMDC` or `--mmdc PROGRAM`. The saved viewer supports local image
pan/zoom, not live analysis or a web service. Source-view JSON reports contain:

```text
catalogDirectory, viewDirectory, viewsIndex, kind, snapshotId, mermaid, svg,
viewer, legend, page, error
```

For singular API/callable views, `svg` is null with `--no-render`; class reports
instead use `diagrams[]` as described below. `error` is null on success. Static candidates have
no architecture proposal, acceptance fingerprint, or acceptance state. They are
separate from the agent's DRAFT proposal/report below.

### Class UML

#### Overview

Start at `SAVEDDIR/views/class/index.html` for **all saved catalog classes** in
actual UML class diagrams, with members, inheritance, and observed dependencies.
Images come first; the class index, full details, and JSON metadata are secondary.
Grouping is source organization, not runtime architecture or an inventory-first view.

#### API / Interfaces

`diagram SAVEDDIR --kind class [--no-render]` selects all-class UML only with no
`--entry`, `--query`, `--path`, or explicit `--limit`/`--offset`. Explicit defaults
such as `--limit 12` or `--offset 0` select paginated `views/class-focus` instead
(default 12 entries, maximum 30).
Catalog browsing and its paging semantics are unchanged.

The legend schema is `ideomancer.class-uml.v1`, with
`modelVersion: class-uml-semantics-2`, `entries`, `relationships`,
`groups`, `layout`, warnings, and `coverage`. Reports have `view: "class"` and
`selection: "all-catalog-classes"`, or `view: "class-focus"` and
`selection: "selected-classes"`. `page` is null for full UML or the selected page
for focus. `diagrams[]` carries each safe `group-NNNN` ID, title, and absolute
`mermaid` / optional `svg` path. Top-level `mermaid` and `svg` are null: the bundle
does not pretend to be a singular diagram.

Coverage exposes `catalogClasses`, `selectedClasses`, `homeClasses`,
`allSelectedClassesRepresented`, `diagramCount`, `modeledRelations`, `relationsDrawn`,
and `relationsOmitted`, plus `relationsRepresentedAsFields` and its
`relationsRepresentedAsFieldsVisible` / `relationsRepresentedAsFieldsHidden` subsets.
The equation is **modeled = shown + omitted + represented-as-fields**; hidden field
projections are not extra omitted relations. Groups retain home/context classes, boundary identities,
drawn arrows, relationship indices, and shown/omitted counts. This is explicit
modeled-source coverage, not a claim that all repository relationships are drawn.

#### Data Flow

Verified saved catalog/evidence produces a `Bundle` containing `diagrams: Vec<Diagram>`,
legend, and HTML; no target source, extraction runtime/compiler, agent, or provider
is accessed. Every selected class has exactly one home group. Packing preserves
small file sets where possible, considers boundary/context density, and splits by
source folder and stable class range. Each diagram has at most 12 home classes,
8 context classes, and 24 boundaries. A context copy points to its unique home.
Class boxes cap visible methods at 12 and fields at 8; full observed members and
modeled relations remain in details and JSON. Caps never drop home classes.
Per-entry `fieldCount`, `visibleFieldCount`, and `omittedFieldCount` make field caps
explicit: nine retained fields can mean eight visible and one hidden. Less boundary
clutter may change grouping without changing the 12-home / 24-boundary / 8-context
limits or complete selected-class coverage.

Declared `extends` uses UML generalization with the hollow triangle at the base.
Dashed dependencies say `stores argument`, `uses factory binding`, or `creates`:
source observations, never DI, composition, aggregation, or guessed providers.
Static-4/5 JS/TS `js_class_relation` receipts require complete ownership, binding
guards, an independent reference, and nearest lookup-scope graph proof. Static-5
uses `javascript-class-relations-2` with explicit `importFactId`; static-4 keeps its
local `javascript-class-relations-1` behavior. Direct same-file local-class bases can
resolve; static-5 also resolves explicit relative ESM/CommonJS named imports/exports
and aliases to actual top-level named class declarations for `extends` and `new`,
including method-local creation. Factory-parameter bases stay scoped unresolved boundaries, not links to
similarly named provider files. Constructor-assigned parameter fields and TS
parameter properties are captured, as are supported captured factory uses and `new`.
Types are annotations, not implementations; JSDoc is not interpreted. Anonymous
class filename aliases are explicitly not declared class names.

Storage-only argument observations project as fields/constructor parameters, not
invented dependency boxes, unless an identified class, supported nonprimitive named
declared type, or explicit receiver/creation/factory-binding use supports a candidate.
Primitive stores remain fields. This is projection, not data/service identity or a
claim that dependencies are absent; all observations remain in metadata.

Optional `js_constructor_field` receipts observe direct constructor `this.field`
assignments, including conditional/literal-RHS writes, without values or definite
initialization claims. Same-name static/instance fields remain separate. Optional
`js_callable_signature` receipts enrich represented members with rest/optional flags,
default-presence booleans, generics and canonical declared parameter/return types
including type arguments. Literal types/default values are fully redacted; unsupported
computed/string-named signatures do not invent members. Valid Unicode guard
representations are preserved. These facts share the 4 MiB / 12000-fact optional class
budget.

Class module policy is `selected-relative-classes-1`; function-call resolution stays
separate and function-only under `selected-relative-functions-1`. CommonJS class
export capture and import initialization ordering are checked, the latter against
the independent reference start. ESM imports are hoisted live-binding candidates
with conservative SCC cycle suppression. Type-only bindings are not runtime classes;
no default/namespace, package, re-export, interop, index/extension-remap, type-checker,
or configuration resolution is added. Reached proofs retain import/export guards,
summaries and source/target closures; resolution metadata records the dependency on
the whole receipt-verified selected module inventory, not just reached citations.

Full UML and focus publish independently: focus never automatically generates
full UML or an inventory. A focus without a full bundle offers only a plain-text
command to generate all-class UML, not a dead link. Matching bundles can be reused;
`--no-render` uses only saved evidence and invokes no `mmdc`. Each group image has
independent pan/zoom via one shared static CSP-authorized controller. SVG remains
an inert `<img>`; there is no inline SVG or target code in JavaScript.

#### Edge Cases

- Empty catalogs have an explicit empty state; incomplete snapshots warn that
  classes/members may be missing even though every selected catalog class has a home.
- All catalog classes is **not all repository classes**. The visible scope warning
  includes scripts and other excluded directories; no discovery expansion or
  render-time exclusion of existing catalog classes occurs.
- Relationship caps can omit arrows; shown/omitted counts and full metadata remain
  explicit. No class omissions or claim of all relationships is substituted.
- Limits are 128 groups, 8 MiB aggregate metadata (HTML, legend, Mermaid, receipt),
  8 MiB per SVG, 64 MiB aggregate SVG, and a 180-second aggregate render budget.
  All groups stage and validate before one atomic publication; failure protects
  the previous bundle rather than publishing a partial set.
- Receipts use `source-views.v2-class-uml`. Canonical `manifestHash` covers identity,
  explicit flat artifact names/hashes, titles, renderer, and diagram order. This
  detects un-rehashed changes, not authentication by an adversary who can rehash.
  New v2 receipts missing the hash are rejected; valid v1 class bundles can upgrade.
- Older JS static-1/2/3 evidence still renders members and declared-base boundaries
  without new relation facts; static-4 local relations still render, while static-5
  can additionally resolve selected relative named classes.
- `class-overview` is safely recognized as legacy only, not the primary output or
  a new command flag. Changed rediscovery removes stale views. Full and focus are
  separate current purposes, not hash/version folders or past archives; older-binary
  downgrade is unsupported.

#### Example usage

```sh
ideomancer diagram SAVEDDIR --kind class --no-render
ideomancer diagram SAVEDDIR --kind class --entry CLASS_ID --no-render
ideomancer diagram SAVEDDIR --kind class --limit 12 --offset 0 --no-render
```

Replace `CLASS_ID` with a catalog class ID; do not combine entry selection with
filters or paging. Generate the full class bundle only when wanted; focus is independent.

#### Related files

- `src/render/class_uml.rs`: grouped UML, relation proofs, packing, and secondary details.
- `src/javascript_native.rs`, `src/javascript.rs`, `src/js_modules.rs`: versioned
  class/member receipts and separate selected-relative class/function policies.
- `src/render/class_overview.rs`: shared saved class/member grounding and legacy inventory.
- `src/render/source_view.rs`, `src/catalog.rs`: shared members/bases and catalog types.
- `src/main.rs`, `src/discovery.rs`: explicit paging, dispatch, reuse, and publication.
- `tests/class_uml.rs`, `tests/class_uml_browser.cjs`: offline contracts
  and the optional [browser regression](../README.md#development).

Open `SAVEDDIR/views/index.html` (reported as `viewsIndex`) for links to every
generated current view, its Mermaid source and legend, and its SVG when available.
Each successful local diagram command refreshes this script-free navigation page;
unchanged links leave its bytes and modification time untouched. Only existing,
verified view bundles are linked. Rediscovery with changed evidence removes the
old views and their index together.
Network/services index labels are generic “Network call candidates” and
“Service and call candidates”, not endpoint-only labels.

## Callable Views

### Overview

Explicit `--view services|network` draws local saved-source call candidates for
one workflow entry: a function, async function, method, CLI command, or job.
No HTTP endpoint or OpenAPI document is required. There is no Python invocation,
rescan, application execution, agent, model, or authentication access.

### API / Interfaces

```text
diagram CATALOG --entry WORKFLOW_ID --view services|network [--no-render]
        [--focus-hops 0..5] [--focus-facts 1..2000]
```

Defaults are 3 hops and 600 facts, including context. Use `--kind workflow --entry
SYMBOL` to disambiguate a symbol also cataloged as an API; exact IDs always resolve
exactly. Explicit native views reject classes and kind/ID mismatches. Entry
selection cannot be combined with filters or paging. `--extract-only` is invalid
with an explicit view; `--no-render` writes Mermaid, legend, and HTML without
`mmdc`, which is needed only for SVG rendering.

### Data Flow

Verified saved catalog/evidence feeds the shared bounded breadth-first callable
expansion and services/network projection. CLI/job metadata and citations are
retained and charged to the shared fact and 196608-byte compact-JSON caps.
Services show at most 20 callables and 40 calls; network shows at most 20 network
groups. Unknown operations and omitted relationships remain explicit, not guessed
network activity or proof of execution order.

The legend uses schema `ideomancer.callable-view.v1` and report code
`STATIC_CALLABLE_CANDIDATES`. `contract` is null; `contractMatch.status` and
`routerContext.status` are `not-applicable`. The entry is a callable candidate,
not a fabricated HTTP inbound node. The source-view report and current paths
`SAVEDDIR/views/{services,network}` are shared with API views; a changed selection
replaces that view, not a separate callable history. The existing renderer module
retains its historical name, `src/render/endpoint.rs`; module navigation adds no
new CLI flags or catalog kinds.

For JS/TS, `js_local_call_candidate` links retain same-file lexical proofs.
`js_module_call_candidate` links carry
`resolutionPolicy: selected-relative-functions-1` and expand only direct ESM named
imports/aliases to named functions or local export aliases, or top-level CommonJS
`const` destructured requires to named functions in one plain `module.exports`
object. A factory-export importer may call an imported helper, but the factory is
not an imported target. Type-only bindings do not create runtime targets.

Relative paths resolve only against selected saved files: exact explicit
`.js`/`.jsx`/`.ts`/`.tsx`, or exactly one extension expansion. No index/package
lookup, config aliases, extension remap, interop, default/namespace target, or
re-export traversal is performed. Dependency summaries suppress cyclic components;
missing or invalid summaries anywhere in the selected inventory disable all module
promotion. Complete import/export/binding/ownership proof must fit atomically within
the same fact/byte budgets. Negative assertions about competing bindings, writes,
or cycles are bound to full receipt-verified evidence, not runtime absence claims.
No target code, modules, or configuration are loaded to resolve these candidates.

Default JS services diagrams use friendly labels and direct owner-to-target edges
for resolved candidates. Repeated source calls with the same owner, target, basis,
and policy share an edge with `siteCount`, call IDs, and source citations. Each
unresolved call has one implementation-unresolved node, not a guessed target.
This changes presentation only; Python and network semantics are unchanged.

### Edge Cases

Conventional root OpenAPI/Swagger files are not even inspected in callable mode;
malformed or multiple files cannot break these views. Explicit `--openapi` is
rejected as API-only before contract-file access. A workflow item whose role is
`http_handler` still uses callable mode: select the API ID for contract enrichment.
API contract discovery and matching remain separate from callable mode.

For Python, without `--view`, selecting a workflow still uses the focused agent path in
[One Workflow](#one-workflow), including `--kind workflow --entry API_ID`.
`--no-render` alone does not disable that agent. `--extract-only` remains valid
on that non-native agent-preparation path.
JS/TS workflow selection without `--view` is rejected, even with `--extract-only`,
before Python or model dispatch. Select `--view services|network` instead.

### Example usage

The standard-library-only [Python worker](../examples/python-worker/README.md)
has no HTTP endpoints, third-party packages, or network client:

```sh
ideomancer discover examples/python-worker
ideomancer diagram .ideomancer/catalogs/python-worker --kind workflow --entry run_batch --view services --no-render
ideomancer diagram .ideomancer/catalogs/python-worker --kind workflow --entry run_batch --view network --no-render
```

No network candidates in this sample is a scoped static result, not an absence proof.

With the native JS/TS candidate built as described under [Discover](#discover):

```sh
ideomancer discover examples/javascript-worker --lang ts
ideomancer diagram .ideomancer/catalogs/javascript-worker --kind workflow --entry prepareBatch --view services --no-render
```

The public module example pairs `batch.ts` with `normalize.ts` and the CommonJS
factory importer `routes.js` with `cjs-helper.js`. Each imported helper can continue
to its syntax-proven same-file helper. Later catalog/native-view commands need no
extractor, target source, or compiler; SVG rendering remains a separate tool step.

## Generic JS Facts

Discovery records syntax-backed exported factories, parameters, direct returns,
and referenced class identity. It does not infer providers from service filenames,
request injection, or ORM query semantics. The retired `--js-profile` option fails
with an actionable removed-feature error; no value selects an operation profile.

Consumers own framework interpretation through the typed facts/claims boundary in
the [adapter contract](adapter-contract.md). The class-relationship extension does
not replace retired route or operation/query views. Legacy catalogs remain usable
for supported generic browsing without reactivating retired interpreters.

## Offline Diagram Viewer

### Overview

API, full/focused class, network, services, and agent workflow diagram pages share the same
offline SVG pan/zoom HTML shell. The separate `views/index.html` navigation index
remains **script-free**, linking all verified generated current views and their
source, legends, and available SVGs; it does not load the controller.
Class bundles place multiple UML viewports first, with independent controls and
one shared controller script; full class details follow the images.

### API / Interfaces

- Buttons: `+`, `-`, Fit, Reset 100%, and Fullscreen when the browser supports it.
- Initial zoom is 100% with scrollable overflow, not automatic fit. Fit is explicit;
  Reset returns to 100% and the top-left. Scale is clamped to 0.1%–800%.
- Left mouse drag pans. Ordinary wheel and touch retain native scrolling;
  Ctrl/Meta+wheel zooms anchored at the cursor. There is no custom touch-pinch zoom.
- With the viewport focused, `+`/`-` zoom, arrows pan, `F` fits, and `0` resets.
  These shortcuts do not apply when toolbar buttons or outside content have focus.
- Responsive viewport resizing and fullscreen transitions preserve the chosen zoom.

### Data Flow

Renderers escape source/model/spec text into HTML details and supply a fixed local
`diagram.svg` image (or `group-NNNN.svg` images for classes). SVG remains an inert `<img>`, never inline SVG, `<object>`,
or canvas. Legends, citations, source text, and contract details stay outside the
pan/zoom viewport. Only the trusted embedded static JavaScript controller executes,
authorized by a CSP SHA-256 hash of its exact bytes. Styles are inline; script
`unsafe-inline` and eval are not allowed. There are no CDNs, fetches, or viewer
network calls (`connect-src 'none'`); source/model/spec content never enters JS.

### Edge Cases

Without SVG (including `--no-render`) or with an invalid image, controls remain
disabled and full text/details remain usable. With JavaScript disabled, controls
also remain disabled, but an available SVG has original-size native scrolling.
Fullscreen is capability-dependent and can be denied without losing the scrollable
view. This image boundary does not sanitize SVG files or sandbox the renderer;
continue to trust `mmdc` and its browser/network policy.

### Example usage

Open the reported `viewer` or workflow `draftViewer`, focus the diagram, and use
`F` for an overview, then `0` for readable original-size scrolling. Browser checks
are optional and use only an existing Puppeteer module and installed Chromium,
with no downloads:

```sh
IDEOMANCER_PUPPETEER_MODULE=/absolute/existing/puppeteer cargo test offline_chromium_regression -- --ignored
IDEOMANCER_PUPPETEER_MODULE=/absolute/existing/puppeteer node tests/viewer_browser.cjs --smoke /path/to/generated/index.html
```

The smoke command targets diagram pages, not the script-free navigation index.

## Endpoint Views

### Overview

Native API network/services views join saved source candidates to an optional local
contract. Supported FastAPI router context can additionally project known built-in
converter declarations for matching; neither projection nor prefix composition
proves runtime routing or a full URL. Commands and viewer controls are unchanged.
Those prefix/converter/SDK rules are Python-specific. JS/TS API entries may use an
optional local OpenAPI export for literal method/path-fragment matching only;
handler-name fallback, FastAPI prefix/converter projection, and JS network proof
are not inferred. A method named `.get` alone is not SDK evidence.

### API / Interfaces

```text
diagram CATALOG --entry API_ID --view network|services [--openapi FILE] [--no-render]
        [--focus-hops 0..5] [--focus-facts 1..2000]
```

The endpoint mode requires one API entry; workflow entries use [callable mode](#callable-views).
Classes are rejected. Do not combine entry selection with filters or paging.
`--openapi` requires an API entry and `--view`; `--extract-only` is invalid with `--view`.
Focus defaults are 3 hops and 600 facts including context. These limits bound local
static expansion, not an agent request. `--no-render` skips Mermaid CLI only.

The endpoint legend emits `routerContext`: `status`, `candidates` (`prefix`,
`composedPath`, `fullPathProven`), `evidenceIds`, `sourceRefs`, `basis`, `warnings`,
and `unknownStatus`. The catalog item schema is unchanged: `routePath` stays the
decorator fragment and `pathIsFragment` stays true. Context is derived only for
native network/services endpoint views, not written back as a full catalog path.

`routerContext.converterProjection` contains `status`, `framework`, `profile`, and
`reason`. For supported context, eligibility is `eligible`, `blocked`, or `unknown`;
the recognized identity/profile are `fastapi.APIRouter` and
`fastapi-starlette-builtins-v1`. Supported-context contract matches additionally emit:

- `contractMatch.converterProjections[]`: `originalPath`, `projectedPath`,
  `parameters` (an ordered inventory of `{parameter, converter}`, with null for an
  untyped placeholder), `status`, and `reason`. Status is `projected`, `unchanged`,
  `blocked`, or `rejected`; blocked/rejected entries have null projected paths and
  empty inventories.
- `contractMatch.converterProjectionContext`: `eligibility` plus `evidenceIds` and
  `sourceRefs` citing the retained context and guard proof.

These are separate JSON metadata, not rewrites of the original item's `routePath`
or any candidate's `composedPath`. `fullPathProven` remains false. Unsupported or
budget-unknown context does not imply converter eligibility.

### Data Flow

Saved catalog/evidence feeds bounded handler expansion and the selected view;
there is no Python rescan, agent, model, or authentication. An optional local
OpenAPI 3.0/3.1 or Swagger 2 JSON/YAML export adds contract declarations. The loader
accepts regular non-symlink files at most 4 MiB, fetches nothing, and rejects
external references. Local references remain explicitly unexpanded or unresolved.
Contract method/path, security requirements/schemes, parameters, request/response
shapes, and server declarations are displayed with provenance. Source-to-contract
associations are **candidates**, not proof of runtime security, activation,
request order, or deployed URLs.

Source-backed context supports a module-level directly imported FastAPI
`APIRouter(prefix=literal)` and at most one same-module `include_router` mount.
Custom registration/factory recipes are not public Core acceptance. No class-name
prefix inference or generated `operationId` decoding occurs.

Complete cited context narrows by method and composed-path **suffix**, not runtime
full path (`fullPathProven: false`); outer mounts and dynamic module activation
remain UNKNOWN. Unsupported context falls back to the existing fragment/exact-name
rules. Supported context contradicting the export yields `context-mismatch`, never
a generic handler-name fallback. Budget failure yields context `budget-unknown`
and contract association `unknown`, with no narrower claim.

With eligible supported FastAPI context and complete retained proof, matching also
projects `{name:converter}` to `{name}` for exactly `str`, `path`, `int`, `float`,
and `uuid`. The strict parser accepts at most 2048 UTF-8 bytes and 64 parameters
per composed candidate; names must be ASCII identifiers. It preserves parameter
names/order, literal text, and trailing slashes. It does not normalize route
identity, percent encodings, or parameter types. Literal and projected paths are
compared under the same declared-method/composed-suffix rules; operation candidates
are combined and deduplicated per operation, not ranked in favor of projection.
Distinct matching operations remain `ambiguous`.

Any observed known or suspected `register_url_convertor` call in the saved evidence
blocks projection globally, including import aliases, re-exports, class-qualified
bindings, and binding-alias chains. This is a conservative possible-name guard,
not runtime call resolution. Registration literals remain redacted, so the guard
cannot know whether a built-in converter key was overridden or constrain its scope.
Alias-closure overflow fails closed with blocking citations. Hidden runtime
converter modifications remain UNKNOWN even when eligibility is `eligible`.

Fully cited factory-call provenance supports direct PEP 604 return unions (`A | B`,
stored by the extractor as `A BitOr B`) with at most four plain named members,
counted before deduplication. Each branch is resolved independently: a known SDK
operation becomes a dashed **declared SDK return branch candidate; runtime selection
UNKNOWN**. The whole union receiver is never classified as SDK. Local wrapper
alternatives remain UNKNOWN without interpreting `__getattr__`. Aliased imports
and reversed member order resolve deterministically. Network groups keep this
branch basis separate from ordinary SDK provenance; existing legend summaries and
diagram labels keep unresolved alternatives visible.

Receiver records expose `branches` and `resolutionTrace`, including `branchCoverage`,
`stoppingReason`, citations (`sourceRefs`), and budget diagnostics. Complete branch
coverage means retained static provenance, not runtime branch coverage. All SDK and
alternative proof, including route attachments, must fit the same fact budget and
196608-byte limit measured from exact consolidated compact UTF-8 JSON. If complete
proof cannot fit, the operation remains UNKNOWN rather than retaining a partial SDK
claim. Runtime selection, URL, HTTP verb, request occurrence, and runtime security
remain UNKNOWN.

Native endpoint selection reserves complete supported context facts/files and their
fact count, including every converter-registration guard proof bundle, **before**
focused expansion, in addition to 16 KiB for provenance,
the actual route attachment/file bytes, and a 2 KiB envelope margin.
The legend records this headroom and the smaller focus cap; truncation stays
explicit. All retained references are accounted for in the consolidated facts/files;
the final compact UTF-8 JSON input still must fit 196608 bytes. Agent-focused
analysis keeps its existing full byte budget and policy.

### Edge Cases

- An unknown/custom converter anywhere in a candidate (including mixed built-in
  and custom placeholders), malformed/nested/unbalanced braces, repeated colons,
  invalid parameter names, or parser limits rejects the **entire** projection.
  Literal matching remains available; no partial converter removal occurs.
- A literal operation and a projected operation can both match and stay ambiguous.
  A supported-context contradiction still reports `context-mismatch`, never an
  `operationId`/handler-name fallback, even if projection is blocked or rejected.
- All guard facts/files and citations must fit the existing fact and 196608-byte
  consolidated-input caps. Budget failure disables narrowing rather than dropping
  blocking proof. Projection establishes neither runtime behavior, method/type
  enforcement, registration/activation, nor a full path.
- Saved `python-static-6-router-context` evidence may retain literal router-context
  associations, but converter eligibility is `unknown`: run `discover` again with
  the updated extractor to enable the guarded projection check.
- Known empty route `''` differs from dynamic/null: a supported prefix can compose
  with it, but an empty suffix never becomes an `endsWith('')` match-all.
- Dynamic prefixes, splats, shadowing, alias escapes, and relevant mutations
  prevent unsupported narrowing and can leave association ambiguous.
- For API entries only, without `--openapi`, exactly one root-level `openapi.json`, `openapi.yaml`,
  `openapi.yml`, `swagger.json`, `swagger.yaml`, or `swagger.yml` is auto-detected.
  None means source-only; multiple candidates require explicit `--openapi FILE`.
- A runtime-generated spec may not be available as a local export.
  Do not run/import the application or fetch a live spec; the user can provide an export.
- Network views include only provenance-backed known HTTP/SDK operations. Unknown
  receivers/operations are explicitly listed in the legend, never guessed network
  edges. The network view is capped at 20 network nodes.
- Services show bounded static callable/call candidates, at most 40 call nodes and
  20 callable nodes. Hop/fact/byte limits, omitted nodes, and unresolved receivers
  are explicit boundaries, not evidence that other behavior is absent.
- Signature-backed inherited `super().__init__` forwarding and nested lexical
  `self` handlers resolve supported constructor wiring, not general MRO or DI.
  Relative imports
  for union alternatives require a unique local class and module; they do not
  establish external SDK provenance or wrapper delegation.
- Factory initializers must be one complete direct callee call; chains, subscripts,
  and truncated call shapes remain UNKNOWN. Unsupported generic, `typing.Union`,
  `Optional`, `Any`, `None`, quoted forward, variadic, or truncated return annotations,
  too many members, and incomplete/ambiguous proof stop with a `stoppingReason`,
  not a partial accepted network classification.

### Example usage

```sh
ideomancer diagram .ideomancer/catalogs/sample-app --entry API_ID --view network --no-render
ideomancer diagram .ideomancer/catalogs/sample-app --entry API_ID --view services --openapi /path/to/export.yaml --focus-hops 2 --focus-facts 400
```

Replace `API_ID` with an API ID from that catalog. Current viewers are
`.ideomancer/catalogs/sample-app/views/{api,class,class-focus,network,services}/index.html`;
each purpose holds one current selection, with full class and focus independent,
not a history of chunks.

For example, supported eligible context composing `/items/by-name/{name:path}`
can match a declared GET `/api/items/by-name/{name}` by projected suffix. The
source fragment `/by-name/{name:path}` and composed path stay unchanged, and the
outer `/api` mount is not proven. A different parameter name or trailing slash
does not match; a second literal operation `/api/items/by-name/{name:path}` keeps
the association ambiguous.

## One Workflow

First browse workflow candidates and choose a stable catalog ID, exact symbol, or
unique dot-suffix. Select an updated native Djinn executable explicitly rather than
assuming an installed `djinn` on `PATH` is current:

```sh
export IDEOMANCER_AGENT=/absolute/path/to/updated/djinn
ideomancer diagram SAVEDDIR --entry WORKFLOW_ID --no-render
ideomancer diagram SAVEDDIR --kind workflow --entry OrderService.submit --no-render
ideomancer diagram SAVEDDIR --kind workflow --entry API_ID --no-render
```

Replace ID placeholders with exact IDs from the catalog. A handler can have both
API and workflow entries: `--kind` narrows ambiguous symbol lookup, while an exact
ID always resolves exactly. Thus `--kind workflow --entry API_ID` selects that
endpoint's callable for workflow interpretation. Without that override an API ID
selects a local API source view. A class cannot anchor a workflow. Missing or
ambiguous selections fail locally; choose a precise ID instead of guessing.
Do not combine `--entry` with `--query`, `--path`, or paging overrides.

For Python, without `--view`, workflow kind requires **one** selected entry, never an automatic batch. It uses
the existing focused `analyze` path and re-extracts live source with the selected
interpreter. Before any model call, the live snapshot must match the saved catalog
snapshot; a mismatch fails with rediscovery guidance, without silently refocusing
or dispatching the agent. Run `discover` again and explicitly review/select the new
candidate. Incomplete extraction also blocks the agent.
For JS/TS this no-view workflow path, including `--extract-only`, fails locally
before Python/model invocation; explicit saved-source `--view services|network`
is required. Python agent behavior remains unchanged.

Only this explicit workflow request opts into sending a focused evidence packet
to the configured agent/provider. Supported global model/auth must already be
configured. Default model is `openai/gpt-6.1-sol`; `--model` overrides
`IDEOMANCER_MODEL`. `--agent`, `--python`, and `--mmdc` similarly override their
`IDEOMANCER_*` settings. Updating Djinn source does not update a configured binary.
See [Djinn setup](agent-workflow.md#djinn-setup) for native build and auth details.

Without `--view`, `--extract-only` prepares the selected workflow locally without agent or renderer;
it is not an API/class option. `--no-render` skips SVG, **not the workflow agent**.
Focused controls are `--focus-hops 0..5` (default 3), `--focus-facts 1..2000`
(default 600), `--timeout-seconds 1..600` (default 300), and explicit `--retry` only
after reconciling failed/uncertain work. A matching saved proposal can be reused;
selection does not promise a fresh model call. No automatic model retry occurs.

The workflow report is the existing analysis report plus `catalogDirectory` and
`entryId`, with `runDirectory`, DRAFT artifact paths, coverage, and acceptance
fields. Runs live under `.ideomancer/analyses` relative to the invocation cwd.
Review the reported `draftViewer` and uncertainties. Acceptance remains a separate
human action, never required for static catalog browsing or API/class/callable views.
Direct `analyze ROOT --focus SYMBOL` remains an advanced fallback, not a substitute
for the catalog freshness guard. See [focused scope](python-analysis.md#focused-workflow-scope)
for conservative resolution and packet boundaries.

## Privacy And Reuse

Extractor `python-static-8-generic-facts` retains direct empty or slash-prefixed
route-path literals of at most 2048 characters, recognized HTTP methods, and
bounded router prefixes/include prefixes as route-context identifier
metadata. This expanded whitelist is an explicit privacy boundary, not a secret
filter. Payload, credential, configuration, description, and legacy-service
literals remain redacted in expression descriptions. Dynamic paths/methods are
unknown with diagnostics, not evaluated. Identifiers, paths, route fragments, and
expression shapes still expose proprietary structure; redaction is not a secret
detector. `api_route` and `entrypoint` facts are support-only in global packet
selection, not new standalone global anchors; all direct router-context facts
are likewise support-only.

Call expression rendering now visits the callee before argument traversal can
exhaust its shared budget. It adds no auxiliary call facts and keeps ordinary
literals, including converter-registration keys, redacted under the same caps.

The route catalog and static views stay local unless you explicitly select a
workflow agent path; its focused packet can include the selected callable's route
metadata. Broad `analyze`/`interpret` also explicitly authorize evidence transfer
under their packet policies. Review the packet, adapter, provider, and credentials
before dispatch. Keep all catalog/run/view outputs private and ignored, including
when the invocation repository differs from the source repository.

Catalog publication stages `catalog.json`, `evidence.json`, and `receipt.json`.
SHA-256 receipts bind saved catalog/evidence; loads verify hashes and rebuild
claims from stored evidence, never live source. Do not edit fingerprinted files.
Identical discovery reuses the verified current directory; changed source/evidence
replaces it and removes stale views. Old snapshots are not retained.
JS/TS evidence uses `ideomancer.evidence.v2` with a `language` metadata object
(`name`, `parser`, `parserVersion`, `parserSha256`, `runtime`, `runtimeVersion`,
`scanPolicy`); JS/TS never occupies `pythonVersion`. Snapshot identity binds
the native parser policy, extractor, parser/runtime metadata, scope policy, and file hashes.
The native JS/TS extractor is `javascript-typescript-tree-sitter-1`;
the outer evidence v2 schema is unchanged and legacy static-1 through static-6 remain
accepted for supported generic views. Static-2 and newer support module links,
static-4/5/6 support class relations, and static-5/6 resolve named-class modules.
Static-5/6 helper summaries use
`selected-relative-classes-1`; static-2/3/4 summaries use
`selected-relative-functions-1`. Static-1 supports same-file navigation.
Rediscover for missing generic factory or class-relation facts,
or for module proofs on static-1.
Legacy static-6 is not upgraded to new generic-fact capabilities. Explicit
rediscovery changes identity and invalidates prior snapshot-bound claims. Native
byte positions convert to existing UTF-16 receipt coordinates. Tree-sitter `ERROR`
or missing nodes, including module-only syntax failures, make extraction incomplete.
Cooperative parse/traversal limits are not a hard memory sandbox. See
[native migration details](javascript-typescript-analysis.md#data-flow).
Legacy Python v1 evidence with `language: None` serializes without a new language
field; old snapshots remain validated under their existing identity semantics.

Source views stage documents, optional SVG, and a receipt, then publish the current
bundle at `SAVEDDIR/views/api`, `class`, `class-focus`, `network`, or `services`. Changed selection,
contract, rendering inputs, or generated viewer HTML replace that kind's validated current bundle.
Matching requests reuse verified artifacts; source-only requests can reuse a matching
rendered view. Tampering and unrecorded SVGs fail validation, not silent repair.
Changing source-view renderer can regenerate the current bundle; changing `--mmdc`
for an agent presentation does not replace its recorded SVG.

Analysis similarly has one current application-named target, for example
`.ideomancer/analyses/sample-app/presentation/index.html`. Changed focus or other
identity inputs replace a valid completed run and reset acceptance. Failed,
running, or indeterminate work cannot be discarded via changed configuration;
reconcile and explicitly retry the saved run first. Manual `inspect --out RUN`
remains exact-path and no-clobber. Legacy hash storage is not automatically migrated:
inspect and explicitly clear it before using the new layout. No old hash folders
are retained by successful current replacement.

No catalog prune command, hosted web application, or live-model quality validation is
claimed here. These scope and integrity rules describe implemented behavior, not
proof that a generated interpretation captures the business correctly.

## Related files

- [JavaScript/TypeScript analysis](javascript-typescript-analysis.md): scope, tools,
  evidence, module resolution guards, bounded MVP acceptance, and deferred capabilities.
- [JavaScript worker](../examples/javascript-worker/README.md): public source fixtures.
- `src/javascript.rs`, `src/javascript_native.rs`, `src/js_focus.rs`, `src/js_modules.rs`,
  `tests/javascript_views.rs`: JS/TS extraction and saved-view contracts.
- `src/main.rs`: CLI interfaces and limit validation.
- `src/discovery.rs`, `src/catalog.rs`: saved catalogs, selection, current view publication.
- `src/openapi.rs`, `src/render/endpoint.rs`: local contract summaries and endpoint candidates.
- `tests/callable_views.rs`: native callable views, selection, contract isolation, and bounds.
- [Python worker](../examples/python-worker/README.md), `examples/python-worker/worker.py`:
  standard-library-only non-web example.
- `src/router_context.rs`, `src/adapters/fastapi.rs`: generic context interface and direct FastAPI adapter.
- `tests/converter_matching.rs`: offline CLI matching in both endpoint views,
  identity preservation, ambiguity, no fallback, registration guards, and redaction.
- `src/analysis.rs`, `src/router_context.rs`, `src/render/endpoint.rs` tests:
  callee preservation, re-export/class-alias guards, closure overflow, citations,
  legacy evidence, and complete-proof budget regressions.
- `src/render/viewer.{rs,js,css}`: shared offline document shell, controller, and styles.
- `tests/viewer_browser.cjs`: optional offline browser regression and generated-page smoke checks.
- `src/focus.rs`, `src/python_extract.py`: bounded resolution and signature facts.
- `tests/provenance.rs`, `tests/network_provenance.rs`: synthetic offline factory
  union, initializer-shape, citation, and budget regressions.
- [Python analysis](python-analysis.md) and [agent workflow](agent-workflow.md):
  extraction limits, current analysis replacement, and explicit provider access.
