# Ideomancer

## Overview

Ideomancer is a Rust engine and CLI for developers who want to understand existing software
through readable, source-grounded diagrams in manageable chunks. Discover a Python or JavaScript/TypeScript
application locally, browse API/class/workflow candidates, draw a small source
view, then select one workflow for focused agent interpretation when useful.

This is progressive exploration through explicit commands, not a background job
stream or an all-view generator. Public callables are workflow **candidates**, not
automatically discovered complete business workflows or a whole-program graph.
Python framework-aware work is FastAPI-only; basic Python callable discovery and
views remain supported. Django and Flask support is not promised. The JS/TS slice provides
**offline source navigation**: API/class/callable views, syntax-proven same-file
calls, bounded selected relative named-function call candidates, and named-class UML
relations from saved evidence.
It is not runtime, DI, network, or JS/TS agent analysis, complete framework coverage,
or completion of a whole application. Organizational interpretation belongs to
consumer adapters; public framework/vendor support is separated into adapters,
not hidden company profiles in Core.
Standard FastAPI and generic Python analysis remain supported; custom router
registration, pluralization, and factory conventions are no longer interpreted
by the engine. See [adapter ownership and migration](docs/adapters.md).

## Development Roadmap

See [ROADMAP.md](ROADMAP.md) for ownership boundaries, milestone status, and
acceptance gates. Generic read-only facts and validated derived class relationships
are implemented locally and exercised by independent external adapter conventions.
Native JS/TS Tree-sitter is implemented as a local candidate with the full local
regression suite passing. Hosted platform validation and release review remain pending; the
working tree is uncommitted and unpublished, not a validated release.
Python/Rust Tree-sitter are subsequent milestones; Rust analysis is not implemented.
The next gate is release/history review and approved consumer acceptance—not more
codebase-specific rules in the engine or a claim of complete application understanding.
See the [release/package checklist](docs/releasing.md) for the enforced crate
inventory and the separate historical, asset, dependency, and publication gates.
This is a [source-only fresh baseline](docs/lineage.md), not a continuation of
prior Git ancestry or a claim that previously distributed copies were erased.

## Reuse As An Engine

The package also exposes the `ideomancer_core` library through
`ideomancer_core::engine::v1`. Its initial public surface is deliberately small:
static discovery, verified catalog browsing, and saved-source class diagrams.
Company-specific codebase registries and framework policies belong in consuming
tools, not new special cases in this API. Internal modules remain private.

The additive [facts/claims extension](docs/adapter-contract.md) provides opaque,
snapshot-bound fact and validated-claim handles, structural factory observations,
separate saved claims, and class-model composition. It validates references and
provenance without executing adapters or approving their framework semantics.
See [adapter ownership and migration](docs/adapters.md) for retired profile behavior.

The library facade exposes no agent/model configuration, framework profile,
OpenAPI option, arbitrary command forwarding, or repository execution hooks.
Trusted extraction tools and optional SVG rendering still execute as documented;
source-only class rendering needs neither. The legacy executable retains its
existing, separately authorized agent workflow commands.

See the [engine API contract](docs/engine-api.md) and
[serialization schema](docs/engine-api.schema.json). This library API is currently
local development work; do not assume an older published Git revision contains
it. Consumers can start with an explicit local path, then pin an approved upstream
revision or internal mirror after publication. Ideomancer is available under the
[MIT License](LICENSE); copies and substantial portions must retain its notices.

## Explore In Waves

### 1. Discover Locally

```sh
ideomancer discover /path/to/app
```

Python AST inspection imports or executes no application code and contacts no
agent. It saves the current catalog/evidence snapshot under `.ideomancer/catalogs/APP`
relative to the invocation directory and shows the first 20 candidates. Use the
reported catalog directory as `SAVEDDIR` below. Default source scope selects
`.py` files under `src` when present, otherwise the root, and excludes names including `scripts`,
`shadow`, `tests`, and `migrations`.

For JavaScript/TypeScript, explicitly select the native Tree-sitter backend:

```sh
ideomancer discover /path/to/app --lang js
```

`--language` aliases `--lang`; `js`, `javascript`, `ts`, and `typescript` select
the same backend. Discovery needs no Node runtime or TypeScript compiler. Hidden
obsolete `--node` / `--typescript` flags and `IDEOMANCER_NODE` /
`IDEOMANCER_TYPESCRIPT` settings cause actionable errors for JS/TS discovery;
remove the flags and unset the variables rather than expecting silent fallback.
There is no target `node_modules`, tsconfig/plugin/script loading, module execution,
or type checker/project resolver. Tree-sitter **0.25.10**, JavaScript grammar
**0.25.0**, and TypeScript/TSX grammar **0.23.2** are exactly pinned, compiled native
C with Rust bindings. The language ABI bridge is directly pinned to **0.1.5** to
preserve Rust 1.89-compatible downstream resolutions, not just the local lockfile.
See [JS/TS analysis](docs/javascript-typescript-analysis.md)
for fixed scope, trust boundaries, evidence metadata, and limits.

### 2. Browse A Subset

```sh
ideomancer catalog SAVEDDIR --kind workflow --query submit --limit 20
ideomancer catalog SAVEDDIR --kind api --path src/app --offset 20
ideomancer catalog SAVEDDIR --kind class --json
```

Browsing never rescans source or calls an agent. `--kind api|workflow|class`,
literal case-insensitive text `--query`, root-relative prefix `--path`, and
`--limit 1..100` / `--offset` select a page. Human-readable Markdown is the
default; `--json` exposes page metadata. Inspect warnings and saved evidence
diagnostics: an incomplete scan can still save a catalog but exits 1. A partial
or empty page is not proof that all workflows have been discovered.

### 3. Draw A Source Chunk

```sh
ideomancer diagram SAVEDDIR --kind class --no-render
ideomancer diagram SAVEDDIR --kind class --query Order --limit 12 --no-render
ideomancer diagram SAVEDDIR --kind api --path src/app --limit 12
```

Start with grouped UML at `SAVEDDIR/views/class/index.html`. With no entry, query,
path, or explicit limit/offset, `--kind class` draws **all saved catalog classes**
as actual Mermaid `classDiagram` groups. Every selected class has exactly one home
group, with at most 12 home classes per diagram. Packing considers up to 24 boundary
and 8 context nodes; titles use source folder and stable class ranges, not inferred
runtime architecture. Class boxes show up to 12 methods / 8 fields; full observed
members and modeled relationships remain in secondary details and JSON metadata.
All catalog classes does **not** mean all repository classes: scope and exclusions
(including scripts) are unchanged.

Focus with `--entry CLASS_ID`, `--query`, `--path`, or explicit paging. Even
`--limit 12` or `--offset 0` selects `views/class-focus` (default 12 entries,
maximum 30). Focus is independent: it neither generates a full class bundle nor
an inventory. Without a full bundle, it offers a plain-text command to generate
one, not a dead backlink. Catalog browsing/paging is unchanged. See
[class UML interfaces](docs/discovery.md#class-uml).

Hollow-triangle generalizations point to declared bases; dashed dependencies label
source observations: **stores argument**, **uses factory binding**, or **creates**.
They do not imply DI, composition, or provider identity. Static-4/5 JS/TS evidence
can resolve guarded direct same-file local-class inheritance; static-5 also resolves
explicit relative ESM/CommonJS named-class imports for aliased `extends` and `new`,
including method-local creation. Targets are actual top-level named class declarations,
not filename guesses. A factory-parameter
base stays a scoped unresolved boundary, never a filename-matched provider.
Type annotations are not implementations; JSDoc is not interpreted. Anonymous
classes may use filename aliases explicitly labeled as not declared class names.

Storage-only arguments appear as fields and constructor parameters rather than
invented dependency boxes, unless a class/nonprimitive named declared type or
explicit receiver/creation/factory-binding use supports a dependency candidate.
This projection makes no data/service identity claim. Direct constructor `this.field`
assignments include conditional and literal-RHS observations, not values or definite
initialization. Static/instance slots remain separate. Represented callable signatures
show rest/optional/default-presence flags, generics and canonical declared types;
literal types/default values remain redacted. See [class semantics](docs/javascript-typescript-analysis.md#grouped-class-uml).

These native Mermaid class and API-to-handler flowchart views use only
the saved snapshot: no extraction Node/compiler/Python, rescan, agent, model, or
authentication, and no target-source reads. SVG rendering separately uses trusted
`mmdc` and its toolchain. API pages and focused class pages default to 12 entries,
at most 30, with `--query`, `--path`, and `--offset` for more chunks.
`--no-render` needs no `mmdc`; otherwise it adds SVG. Open the reported `viewer`
alongside its legend. Source views are static candidates, not architecture proposals,
and have no acceptance state. API paths remain fragments, not actual full URLs.
Python native endpoint network/services views can derive cited router-prefix context from
bounded source conventions; outer mounts and runtime activation remain UNKNOWN.
JS/TS API enrichment matches literal method/path fragments only: no FastAPI prefix
or converter projection, SDK inference, or proven JS network edges.

For one callable's local service or network candidates:

```sh
ideomancer diagram SAVEDDIR --entry WORKFLOW_ID --view services --no-render
ideomancer diagram SAVEDDIR --kind workflow --entry run_batch --view network --no-render
```

Explicit `--view services|network` uses saved source calls for one function, async
function, method, CLI command, or job candidate. It needs no HTTP endpoint,
OpenAPI document, Python interpreter, rescan, application execution, model, or agent.
`--no-render` avoids `mmdc`; rendering SVG is the only step that needs it.
Use `--kind workflow --entry SYMBOL` when an API entry duplicates the symbol;
IDs remain exact. Classes and kind/ID mismatches are rejected for explicit views.
`--extract-only` is invalid with `--view` (use `--no-render` instead).

Callable views never inspect conventional root OpenAPI/Swagger documents, so
malformed or multiple documents cannot break them. Explicit `--openapi` is API-only
and is rejected for a workflow entry before contract-file access. Even a workflow
item with role `http_handler` stays in callable mode; select its API ID for enrichment.
The callable legend uses `ideomancer.callable-view.v1`, report code
`STATIC_CALLABLE_CANDIDATES`, `contract: null`, and `not-applicable` statuses for
`contractMatch` and `routerContext`, with no fabricated HTTP inbound node.
CLI/job metadata remains cited within the shared fact/byte budgets and bounded
breadth-first expansion (20 callables, 40 calls, 20 network groups).

JS/TS module navigation accepts direct ESM named imports/aliases to stable named
functions or local export aliases, and top-level CommonJS `const` destructured
requires to named functions in a single plain `module.exports` object. A
factory-export importer can call an imported helper, but the factory is not a
module target. Type-only bindings are not runtime targets. Module links use
`js_module_call_candidate` and `resolutionPolicy: selected-relative-functions-1`;
same-file links remain `js_local_call_candidate`.

Resolution uses selected saved files only: exact explicit `.js`/`.jsx`/`.ts`/`.tsx`
extensions or one unambiguous extension expansion. No index/package/config alias,
extension remap, interop, default/namespace target, or re-export resolution occurs.
Dependency summaries suppress cycles; missing/invalid summaries disable module
promotion. Atomic proof and receipt-bound negative assertions must fit the focus
budgets; unresolved or omitted links are not absence proofs. No target code,
module, or configuration loading is involved. Current extraction is
`javascript-typescript-tree-sitter-1` within unchanged `ideomancer.evidence.v2`;
legacy static-1 through static-6 remain accepted for generic browsing, not silently
upgraded to new fact capabilities. Rediscovery changes identity and invalidates old
snapshot-bound claims. Static-2 and newer support
module links, and
static-1 remains same-file only. Older static-1/2/3 class
snapshots still show members and declared-base boundaries without new relation facts;
static-4/5/6 promote class relations; static-5/6 resolve named-class modules.
Static-5/6 helper summaries use `selected-relative-classes-1`; older static-2/3/4 use
`selected-relative-functions-1`. The function-call resolver remains separate and
function-only. CommonJS class export/import initialization order is checked against
export capture and the independent reference start; ESM uses hoisted live-binding
candidates with conservative cycle suppression. Reached class proof retains guards,
summaries and source/target closures, and records its dependency on the whole verified
selected snapshot. Saved views require
no extractor. See the [MVP acceptance matrix and deferred capabilities](docs/javascript-typescript-analysis.md#bounded-mvp-acceptance-matrix).

Try the standard-library-only [Python worker example](examples/python-worker/README.md):

```sh
ideomancer discover examples/python-worker
ideomancer diagram .ideomancer/catalogs/python-worker --kind workflow --entry run_batch --view services --no-render
ideomancer diagram .ideomancer/catalogs/python-worker --kind workflow --entry run_batch --view network --no-render
```

For public JS/TS worker and module-helper fixtures, see
[JavaScript worker](examples/javascript-worker/README.md). No extraction runtime setup is needed:

```sh
ideomancer discover examples/javascript-worker --lang js
ideomancer diagram .ideomancer/catalogs/javascript-worker --kind workflow --entry prepareBatch --view services --no-render
```

The module example pairs `batch.ts` with `normalize.ts` (exporting `normalize`,
which calls a same-file helper), and factory importer `routes.js` with
`cjs-helper.js` (exporting `helper`, which also calls a same-file helper).

### Generic JS source views

JS services views use saved syntax-backed callable and module candidates. Generic
factory facts describe exported functions, parameters, direct returns, and class
identity, not provider discovery, request injection, or query semantics. Framework
interpretation belongs to consumer adapters under the
[adapter contract](docs/adapter-contract.md); the class extension does not replace
retired operation/query views. The legacy `--js-profile` option is removed and
returns an actionable error rather than selecting a profile or silently falling back.

Optional class relations, constructor fields (`js_constructor_field`), and callable
signatures (`js_callable_signature`) share a 4 MiB / 12000-fact class budget;
none requires source execution.

Default JS services views also use friendlier labels and group repeated resolved
candidate calls into direct owner-to-target edges with source-site counts. Each
unresolved call has one implementation-unresolved node, rather than a guessed target.
This presentation change does not alter Python or network semantics.

For one API endpoint, optional contract enrichment remains unchanged:

```sh
ideomancer diagram SAVEDDIR --entry API_ID --view network --no-render
ideomancer diagram SAVEDDIR --entry API_ID --view services --openapi /path/to/export.yaml --focus-hops 3 --focus-facts 600
```

These API views require one API entry and use saved evidence, never an agent or Python
rescan. `--focus-hops 0..5` and `--focus-facts 1..2000` bound static expansion.
Optional local OpenAPI 3.0/3.1 or Swagger 2 JSON/YAML exports must be at most 4 MiB;
no fetching occurs and external references are rejected. A single conventional
root `openapi.{json,yaml,yml}` or `swagger.{json,yaml,yml}` is auto-detected; none
means source-only, while multiple files require `--openapi FILE`. Contract method,
path, security, request/response, and server declarations are displayed, not runtime
security or activation proofs. Associations are explicitly candidates, not call order.
Network views show only provenance-backed known HTTP/SDK operations; unknowns stay
explicit in the legend, not guessed network edges. Service graphs show at most
40 calls and 20 callables, with omissions and resolution limits recorded.

Endpoint legends expose `routerContext`. Only complete, budgeted source proof can
narrow contract candidates by a composed prefix/fragment suffix; contradictory
supported context reports `context-mismatch`, not a generic fallback. Unsupported
context uses existing fragment rules; insufficient proof budget stays unknown.
Eligible supported FastAPI context also compares `{name:converter}` as `{name}`
for built-in `str`, `path`, `int`, `float`, and `uuid` declarations only. Literal
and projected operation candidates are combined, preserving ambiguity; source
route fragments and composed paths are never rewritten. Unknown/custom or malformed
placeholders reject the entire candidate projection, not literal matching.
Observed or suspected `register_url_convertor` calls anywhere in saved evidence
block projection, including supported alias/re-export chains, because redacted
registration keys cannot rule out built-in overrides. Complete guard proof must
fit the existing endpoint caps. This is neither runtime routing/parameter-type
proof nor a full-path claim; hidden runtime converter changes remain UNKNOWN.
See [router context and limits](docs/discovery.md#endpoint-views).

For an app named `sample-app`, current viewers are
`.ideomancer/catalogs/sample-app/views/{api,class,class-focus,network,services}/index.html`.
Start at `.ideomancer/catalogs/sample-app/views/index.html` for links to all generated
views, source files, legends, and available SVGs. Diagram commands keep it updated.
Callable and API selections share the current `views/services` and `views/network`
paths; the index labels them “Service and call candidates” and “Network call candidates”.
This navigation index remains **script-free**; individual API/class/network/services
and workflow diagram pages share an offline pan/zoom viewer. Diagrams start at
readable 100% with scrolling, not a tiny fit-to-window default. Use `+`, `-`, Fit,
Reset 100%, or Fullscreen when available; left-drag pans, ordinary wheel and touch
use native scrolling, and Ctrl/Meta+wheel zooms at the cursor. Focus the viewport
for `+`/`-`, arrow-key pan, `F` to fit, and `0` to reset. Zoom spans 0.1%–800%;
viewport resizing preserves zoom. No custom touch pinch is promised.

SVG stays an inert `<img>`, never inline SVG, an object, or canvas. Only the
trusted embedded static controller runs, authorized by its exact SHA-256 CSP hash;
there are no unsafe-inline scripts, eval, CDNs, fetches, or viewer network calls.
Styles are inline; source/model/spec content never enters JavaScript. Legends and
source details stay outside the viewport and remain usable without SVG or JavaScript.
With JavaScript disabled, an available SVG retains original-size native scrolling;
controls stay disabled without JavaScript or a valid image. See
[viewer interfaces and fallbacks](docs/discovery.md#offline-diagram-viewer).

Each purpose has one current view; changed selections replace it. Full class UML
and class focus are independent current bundles, not archives or hash/version
folders. Class HTML presents multiple UML images first, each with independent
pan/zoom from one shared CSP-authorized static controller; a class index/details
and JSON link follow. Reports use `diagrams[]` with absolute Mermaid/SVG paths,
not a fictitious singular diagram: top-level `mermaid` and `svg` are null.
Coverage distinguishes home/catalog/selected classes and groups. Modeled relations
equal shown + omitted + represented-as-fields; projected fields report visible/hidden
subsets (for example nine retained fields, eight visible, one hidden by the cap).
All observations remain in metadata; member/relationship caps never omit home classes.
Reduced boundary clutter may change groups without changing the 12-home / 24-boundary /
8-context limits. The legend uses `modelVersion: class-uml-semantics-2`; source and
presentation v2 receipts are unchanged.

Class receipts use `source-views.v2-class-uml`; their canonical `manifestHash`
binds identity, explicit flat artifact hashes, titles, renderer, and diagram order.
This detects un-rehashed changes, not authentication. All groups stage and publish
atomically once, preserving the previous bundle on failure. Limits are 128 groups,
8 MiB combined metadata, 8 MiB per SVG / 64 MiB total SVG, and 180 seconds aggregate
render time. `--no-render` works entirely from saved evidence and never calls `mmdc`.
Valid v1 class bundles can upgrade; v2 without `manifestHash` is rejected.
`class-overview` is recognized safely as legacy only, not a new command flag or
primary view. Changed rediscovery removes stale views; older-binary downgrade is
unsupported. Do not run/import an app to generate its contract;
provide an export explicitly if available. Fully cited factory-call provenance can
retain up to four direct PEP 604 named return alternatives (`A | B`). A known SDK
operation is a dashed **declared SDK return branch candidate**, not classification
of the whole union as SDK: runtime selection, URL, verb, request occurrence, and
runtime security remain UNKNOWN. Local wrappers remain UNKNOWN; `__getattr__` is
not interpreted. This is source-backed provenance, not confirmation that any
network request occurred.

### 4. Select One Workflow

Configure an explicitly selected, updated native Djinn executable and supported
global model/auth before authorizing provider access:

```sh
export IDEOMANCER_AGENT=/absolute/path/to/updated/djinn
ideomancer diagram SAVEDDIR --entry WORKFLOW_ID --no-render
ideomancer diagram SAVEDDIR --kind workflow --entry API_ID --no-render
```

Replace the IDs with exact catalog IDs. `--entry` also accepts an exact symbol or
unique dot-suffix; `--kind` narrows ambiguous symbol lookup. Exact IDs remain exact,
allowing `--kind workflow --entry API_ID` to focus an endpoint's handler. Classes
cannot anchor workflows. Do not combine `--entry` with filters or paging.

For Python, without `--view`, this selected workflow retains the existing focused `analyze` agent path,
including the `--kind workflow --entry API_ID` override shown above.
A live extraction must match the catalog snapshot before model dispatch; stale
selection fails without a silent refocus or agent call. Rediscover to replace the
current catalog, review the new selection, and explicitly request it. Until then,
static views describe the saved snapshot. No command automatically analyzes every workflow.
JS/TS workflow analysis without `--view`, including `--extract-only`, is rejected
before Python or model dispatch. Use `--view services|network`; JS/TS agent analysis
is not supported in this MVP. The existing Python agent path is unchanged.
The default model remains `openai/gpt-6.1-sol`; flags override `IDEOMANCER_*` settings.
`--extract-only` prepares a selected workflow locally; `--no-render` alone does not
disable its agent. Review the resulting DRAFT proposal separately from static views.

See [discovery and diagram chunks](docs/discovery.md) for selection, privacy,
snapshot integrity, output fields, and cache behavior. The hidden manual `plan`
experiment is outside this core exploration workflow.

Catalogs and views still contain sensitive source structure. Extractor
`python-static-8-generic-facts` deliberately retains direct route-path literals
(empty or slash-prefixed, at most 2048 characters), recognized HTTP methods, and
router/include prefixes as route-context
identifier metadata. This is an explicit privacy boundary, not a secret filter;
payload, credential, configuration, description, and legacy-service literals remain redacted. Dynamic route
paths/methods stay unknown with diagnostics. Selected workflow packets can include
this metadata. Keep artifacts private and ignored, and review evidence before
authorizing an agent/provider.

The extractor renders call targets before arguments can exhaust the expression
budget, with no auxiliary call facts, broader literal retention, or increased caps.
Saved `python-static-6-router-context` evidence can still support literal router
associations, but converter eligibility is unknown: rediscover before requesting
converter-aware matching. The CLI and offline viewer UI are unchanged.

## Install

Install with a Rust toolchain and a native C compiler for the pinned Tree-sitter
runtime/grammars. The declared minimum Rust version is **1.89**; full supported
toolchain/platform validation remains a release gate:

```sh
cargo install --path . --locked
```

Rendered `diagram` views, default `analyze`, and standalone `render` need external
Mermaid CLI (`mmdc`). Use `diagram ... --no-render` or `analyze --no-render` for
Mermaid/legend/HTML output without generating SVG:

```sh
npm install -g @mermaid-js/mermaid-cli
```

Python inspection currently supports Unix (Linux/macOS) and requires a trusted
Python >=3.12 interpreter (default `python3.12`); other operating systems fail
explicitly rather than use an unsafe filesystem fallback.
JS/TS inspection also requires Unix, but uses compiled-in Tree-sitter and requires
neither Node nor Python. Parse/traversal limits are cooperative, not a hard memory
sandbox. Python still uses its trusted Python >=3.12 AST helper; Python and Rust
Tree-sitter backends are roadmap work, and Rust source analysis is not implemented.
Optional SVG rendering still needs external `mmdc` and its Node/Chromium toolchain.
`discover`, `catalog`, and explicit source views are local. A workflow selected without `--view` or
explicit `analyze` opts into sending evidence through Djinn or a protocol-compatible
adapter unless `--extract-only` is supplied. `inspect` never contacts an agent or
provider; explicit `interpret` also opts into provider access. The standalone
manifest pipeline needs neither Python nor an agent.

## Advanced Analysis

Use `analyze --focus` as the direct advanced fallback when you already know the
callable, or omit focus for a broad responsibility-level proposal. Prefer the
catalog-first workflow above to choose and understand a bounded subset.

With trusted, updated Djinn and `mmdc` executables on `PATH` and supported global
model/auth configuration (including supported stored OAuth):

```sh
ideomancer analyze /path/app
```

No manual interpretation step is needed. The command automatically extracts,
invokes the agent, validates its proposal, and saves `draft.architecture.json`
plus a current presentation under `RUN/presentation/`:
`diagram.mmd`, `diagram.svg`, `legend.json`, and `index.html`. Open the reported
`draftViewer` first to review the **DRAFT** diagram alongside full labels,
rationales, citations, and uncertainties before acceptance.

To present an existing saved proposal without Python, source extraction, an agent,
model, or authentication access:

```sh
ideomancer present RUN --no-render
ideomancer present RUN --mmdc mmdc
```

`present RUN [--no-render] [--mmdc PROGRAM]` supports `proposal_ready` and `accepted`
runs, not extraction-only (`extracted`) runs. It verifies the saved proposal and
citation evidence, never calls acceptance, and never updates workflow state.
The viewer remains DRAFT even for accepted runs: it projects the saved proposal,
not the current accepted graph. `analyze` automatically selects the current
presentation version. See [presentation details](docs/agent-workflow.md#readable-presentations)
for cache integrity and recovery.

For one operation's workflow rather than a broad application-composition map:

```sh
ideomancer analyze /path/app --focus OrderService.submit
ideomancer analyze /path/app --focus OrderService.submit --focus-hops 2 --focus-facts 400 --extract-only
ideomancer skill --focus
```

`--focus SYMBOL` accepts an exact fully qualified callable symbol, a unique
dot-suffix such as `OrderService.submit`, or a callable fact ID from `evidence.json`.
Only function/async-function declarations (including methods) are anchors, not
classes, `path:line` queries, or HTTP method/path queries. `--focus-hops` accepts
0 through 5 (default 3); `--focus-facts` accepts 1 through 2000 (default 600),
including all selected context. Both limit flags require `--focus`; invalid
ranges or missing focus are usage errors (exit 2). Missing, ambiguous, or
non-callable anchors fail locally before an agent call (exit 1), including with
`--extract-only`; the inventory can still be saved with `packetError`.

Focused expansion follows bounded static candidates, not runtime dispatch, caller
discovery, complete inheritance/MRO, or framework DI. It uses the same agent,
model, timeout, rendering, retry, and explicit acceptance controls as the overview.
Supported inherited `super().__init__` forwarding and nested lexical `self`
handlers use signature facts to resolve static constructor-forwarding candidates,
not general MRO or DI. Depth 0 retains the anchor's body facts and context without expanding callees.
Review unresolved receivers, missing bodies, and depth/fact/byte boundaries rather
than treating the graph as execution order, branch coverage, or rollback proof.
`AugAssign`, `NamedExpr`, `del`, complete control-flow analysis, and registry
resolution are unsupported. Same-line ambiguous receiver writes stay unresolved,
not a fallback to the original local method.
See [focused scope](docs/python-analysis.md#focused-workflow-scope) for resolution
and packet details.

Output defaults to `.ideomancer/analyses` relative to the **invocation working
directory**, not the target app. Parents and a readable application-named current run
subdirectory are created automatically. `--out BASE` changes that container, not
the exact run directory. Keep it private and ignored in the invoking repository.
Choose a different `--out BASE` explicitly to use a separate container.

The JSON report includes `runDirectory`, `status`, `snapshotId`, `proposalSha256`,
`requiresAcceptance`, `complete`, `draftManifest`, `draftMermaid`, `draftSvg`,
`draftLegend`, `draftViewer`, `presentationVersion`, `evidencePacket`,
`coverageReport`, `coverage`, `packetError`, `focus`, and `error`.
`draftManifest` and visual paths point into `presentation/`.
`presentationVersion` is `readable-v2`, an internal receipt version, not a folder name.
The upgrade from `readable-v1` regenerates a verified older bundle in the same
`presentation/` folder without model calls or proposal/acceptance changes.
`focus` is `{"symbol":"OrderService.submit","hops":3,"maxFacts":600}` for
that focused configuration, or null for an overview.
The packet/coverage paths point to local `evidence-packet.json` and
`evidence-coverage.json`; `coverage` includes counts and warnings, not confidence.
Unavailable fields are null; `complete` describes source
extraction, not architectural correctness. A normal draft has
`status: "proposal_ready"` and `requiresAcceptance: true`. The flag is true only
for `proposal_ready` with a proposal fingerprint; it is false for `extracted`,
`failed`, `indeterminate`, and `accepted`. Use its exact run path
and proposal fingerprint when you choose to accept:

```sh
ideomancer accept RUN --proposal-sha256 HASH
```

First acceptance checks source freshness and promotes the exact reviewed proposal
to `architecture.json` and `diagram.mmd`, without another agent call. It does not
rename the draft SVG into an accepted diagram. Draft artifacts remain a DRAFT
proposal view even after acceptance.

Configure tools once in your shell, with per-invocation CLI flags taking precedence:

```sh
export IDEOMANCER_AGENT=/absolute/path/to/updated/djinn
export IDEOMANCER_PYTHON=python3.12
export IDEOMANCER_MMDC=mmdc
# Optional: export IDEOMANCER_MODEL=your-provider-model
ideomancer analyze /path/app --no-render
ideomancer analyze /path/app --extract-only
```

The corresponding flags are `--agent`, `--python`, `--mmdc`, and `--model`.
The default analysis model is `openai/gpt-6.1-sol`; `IDEOMANCER_MODEL` overrides
that default, and an explicit `--model` takes precedence over both.
Ideomancer has no file-based configuration or automatic discovery of a preferred
chat CLI; non-Djinn agents need a protocol-compatible wrapper. Djinn `execute`
reuses supported global model/auth configuration without starting a UI, browser,
login, session, tools, or workspace-config/instruction discovery. Only OpenAI
(including OAuth/Codex), Copilot, and unprefixed OpenAI models are supported, not
every interactive provider. See [Djinn setup](docs/agent-workflow.md#djinn-setup)
for precedence, refresh writes, and deadline details.
If the configured model is unsupported, select a compatible `--model` or
`IDEOMANCER_MODEL`; adding an API key is not a model-compatibility workaround.
Global OpenCode model-ID aliases are honored once after selection from merged
JSON/JSONC maps, preserving provider dispatch without importing provider options,
endpoints, or secrets. No dynamic model discovery or automatic fallback occurs.
Model failures retain route/selected-model context and sanitized identifiers/static
reasons from bounded HTTP-error JSON, never freeform provider bodies.

`--extract-only` invokes neither agent nor renderer. `--no-render` still invokes
the agent when needed and saves the draft manifest, Mermaid source, legend, and
HTML, but does not generate an SVG (a matching current-version SVG is retained). With
`--no-render`, `draftSvg` is null even if a recorded SVG exists. The HTML's diagram
points to the SVG once rendered; its full text/legend is readable without it.
Unchanged runs reuse a validated proposal and any recorded SVG without another
agent/render call. The run key includes the full evidence fingerprint, including
diagnostics and completeness, not just `snapshotId`: repairing an incomplete scan
replaces the current run even if its snapshot ID stays the same. Source, evidence,
interpreter, extractor version, packet policy, bundled skill, agent, or
requested-model key changes replace a validated completed run and reset acceptance. Renderer configuration is
not part of that key: changing `--mmdc` does not rerender a cached SVG. Fresh agent
output can vary; repeatability comes from reusing the saved proposal/artifacts.
After acceptance, a cached repeat reports `requiresAcceptance: false`, while the
draft remains visibly DRAFT.

Focus settings, the focused skill, and focused packet policy participate only for
focused runs. Different anchors or limits replace the single current analysis target,
including earlier views and accepted outputs. For `sample-app`, its viewer is
`.ideomancer/analyses/sample-app/presentation/index.html`. Internal hashes and
versions remain in receipts, not directory names. Failed, running, or indeterminate
work cannot be discarded by changing configuration: reconcile it and explicitly
retry the saved run first. There is no automatic migration of legacy hash storage;
inspect and explicitly clear old legacy artifacts before using the current layout.
Successful replacement retains no old hash folders or snapshot history.

Incomplete extraction, including source parse failures, saves an `extracted`
report with `complete: false` and blocks the agent. If extraction is complete but
no useful packet fits, the full inventory is preserved with `packetError`:
without focus, `--extract-only` succeeds, while normal analysis reports an error and exits 1
before an agent call. Agent or renderer failures
normally return the run's JSON report with `error` and exit 1; failures before a
report can be formed go to stderr. A render failure preserves the proposal and
draft sources: fix/install `mmdc` and use `present RUN`, or rerun the same analysis
without `--retry`. `present` recovers without a source scan or agent retry.
Use `--retry` only after reconciling failed or indeterminate/interrupted agent
work and deliberately authorizing another possible provider call.
Both `analyze` and `interpret` default to a 300-second controller wall timeout;
`--timeout-seconds` accepts 1 through 600. The caller's timeout now reaches updated
Djinn through request `timeoutMs`, with a small margin before the controller's
local hard kill: normally 299000 ms at 300 seconds, or 599000 ms at 600 seconds.
Djinn shares that budget between auth/model resolution and the model call; its
standalone 120-second default no longer caps these requests. Local termination
does not guarantee remote cancellation. No deadline increase or model retry is
automatic, and a timeout report has `requiresAcceptance: false`.

After reconciling a failed or indeterminate attempt, an explicit longer-budget
retry can be authorized with:

```sh
ideomancer analyze /path/app --retry --timeout-seconds 600
```

This new call may incur cost; more time does not guarantee success or reduce the
evidence/context size. Changing the timeout alone does not bypass a saved proposal
or retry uncertain work. Update both Ideomancer and the configured Djinn binary:
legacy Djinn builds reject `timeoutMs` as an unknown request field, with no fallback.
See [deadline details](docs/agent-workflow.md#djinn-setup) for budget precedence.

For an existing authentication-failed run, rebuild/update the configured Djinn
binary, ensure `PATH` or `IDEOMANCER_AGENT` points to it, and rerun the same
`analyze` command with `--retry`. Valid supported stored auth needs no extra API
key; OAuth also needs a configured Codex-compatible model. Updating source alone
does not update an already configured agent binary.
If `PATH` still selects a stale installed Djinn, explicitly select the updated
executable with `--agent` or `IDEOMANCER_AGENT`; there is no automatic executable
or provider fallback, and no global preference change is required.

On Unix, output-container creation checks every path component through anchored
directory descriptors without following symlinks. Relative paths start from the
canonical invocation cwd; only root-level macOS `/var` and `/tmp` system aliases
are specially supported. Manual initial inspection is staged in a sibling
`.ideomancer-init-*` directory and atomically renamed without replacing an existing
run. Managed current-run replacement stages a new run before exchanging directories.
A crash can leave staging behind, but it does not occupy the final run path
and is never automatically deleted on recovery. Later draft/acceptance updates
are not an all-files atomic transaction.

Draft SVG rendering has a 60-second deadline and an 8 MiB SVG limit. Unix cleanup
sends TERM to the renderer's owned process group, allows up to one second for
graceful shutdown, then escalates to KILL and reaps the renderer. This is not a
sandbox against malicious executables or detached helpers; trust `mmdc` and its
browser/network policy.

See [agent workflow](docs/agent-workflow.md) for setup, protocol, recovery, and
freshness semantics, and [Python analysis](docs/python-analysis.md) for scope and limits.

## Pipeline

```sh
ideomancer init --name "My Project" --id my-project --out my-project.yaml
ideomancer validate my-project.yaml
ideomancer emit my-project.yaml --out my-project.mmd
ideomancer render my-project.yaml --out my-project.svg
```

### Initialize

```text
ideomancer init --name NAME [--id ID] [--out PATH] [--format yaml|json]
```

Creates a schema `0.1.0` starter graph with one node, `app`, labeled with the
project name. Without `--out`, YAML goes to stdout by default. With `--out`,
format is inferred from `.json` versus `.yaml`/`.yml`; explicit `--format`
overrides inference. The parent directory must already exist. Output creation
atomically refuses an existing file rather than overwriting it.
If omitted, `--id` is derived from the name as lowercase alphanumeric words
joined by hyphens (including Unicode letters). Names without letters or digits
need an explicit `--id`. Use `--format` for output paths with other extensions.

### Validate

```sh
ideomancer validate examples/ideomancer.idman.yaml
ideomancer validate examples/manifest.json --json
ideomancer validate --json < examples/project.idman.yaml
```

Syntax: `validate [INPUT] [--json]`. Input defaults to `-` (stdin).
Success produces a text confirmation or JSON
`{"valid":true,"errors":[]}`. Invalid manifests produce diagnostics or JSON
`{"valid":false,"errors":["..."]}` and exit 1. I/O errors go to stderr and
exit 1; command-line usage errors from clap exit 2. Successful commands exit 0.

### Emit And Render

```sh
ideomancer emit examples/project.idman.yaml
ideomancer emit examples/manifest.json --format mermaid --out project.mmd
ideomancer emit - < examples/project.idman.yaml
ideomancer render examples/project.idman.yaml --out project.svg
ideomancer render examples/project.idman.yaml --out project.png --mmdc /path/to/mmdc
```

Both commands parse and validate before generating output. `emit INPUT`
defaults to Mermaid on stdout; `--out PATH` overwrites an existing output file.
`render INPUT --out PATH` accepts only `.svg`, `.png`, or `.pdf` output paths.
It invokes `mmdc` from `PATH` unless `--mmdc PATH` overrides it. Temporary
Mermaid input is managed with RAII and cleaned up on success or failure.

## Lower-Level Inspection And Recovery

Use `inspect` and `interpret` separately for advanced analysis when
debugging, recovering a run, or reviewing evidence before authorizing a provider.

Create the output parent yourself; `inspect` creates the run directory, not its
parents. Keep private runs in ignored `.ideomancer/` (check ignore rules when
working in another repository).

```sh
mkdir -p .ideomancer
ideomancer inspect /path/to/example-project --out .ideomancer/example-run --python python3.12
ideomancer status .ideomancer/example-run
ideomancer skill
```

Review `evidence.json`, diagnostics, the prepared `evidence-packet.json` and
`evidence-coverage.json`, and the static module/import projection
`modules.json` / `modules.mmd` before approving provider access and credentials.
Initial inspection (including `analyze --extract-only`) saves the packet and
coverage with hashes before any agent call, when packet preparation succeeds.
No raw source is included in the agent packet by default, but paths, symbols, and
extracted expression shapes remain sensitive proprietary evidence. Incomplete scans block
interpretation. Dynamic-discovery diagnostics alone do not make a scan incomplete.
Full evidence remains local; the extractor includes parameter-shadowing
and unsupported-assignment invalidation facts alongside assignment syntax, not
runtime type or dependency-injection proof. The `parameter`,
`signature`, `binding_invalidation`, `api_route`, and `entrypoint` kinds are support-only in
global bounded selection, never standalone anchors; direct router-context metadata
is likewise support-only. These are fact additions, not public packet-type changes or
mutations of the analyzed application. Larger no-focus inputs use
`architecture-packet.v2-relations`: a deterministic, relation-aware subset at the
unchanged 192 KiB compact-JSON limit, with bounded lexical context and explicit
coverage. Small inputs remain unchanged and also get a coverage report.
Interpretation uses the hash-verified stored packet, not a regenerated selection;
proposal references must occur in the actual sent packet, not just the inventory.
Scan completeness does not mean all graph relationships are resolved, the packet
is exhaustive, or it fits a model's token/context limit.

Legacy state and stored proposals remain readable through `status`, but current
`interpret` refuses a missing/old packet policy before dispatch and requires a new
run. First acceptance of an old-extractor proposal also requires a newly generated
and reviewed run because freshness cannot be checked under old extraction semantics.
Already accepted runs retain their original artifact-reuse behavior.

With a trusted, updated Djinn executable on `PATH` and supported global model/auth
configured:

```sh
ideomancer interpret .ideomancer/example-run --agent djinn
```

For a run created by `analyze --focus ... --extract-only`, `interpret RUN`
automatically uses its persisted focused packet and combined guidance. There is
no `--focus` flag on `interpret`; `skill --focus` prints the exact global plus
focused instructions, while `skill` prints only the global instructions.

This headless `execute` path can reuse supported stored OAuth without interactive
login. Active external OpenCode OAuth can be reused, but only Djinn-owned expired
OAuth auto-refreshes. Expired external auth requires `djinn auth login` or refresh
via its owning harness; Djinn never rotates or copies it. Native Rust headless and
interactive refresh/login writers share a lock. Before refresh dispatch, the
private Djinn store records `_djinn_openai_refresh_inflight_sha256`, a SHA-256
refresh-token hash that blocks unsafe replay after cancellation/provider error.
It is cleared after durable credential commit or a separate explicit login;
reauthenticate/reconcile, do not blindly delete markers/locks. Even failed calls
can modify the native auth store and leave an owner-only lockfile, not app/session
state. See [Djinn setup](docs/agent-workflow.md#djinn-setup) for stored Copilot
precedence, JSON/JSONC model merging in the selected XDG config root, and incremental
SSE completion without waiting for remote EOF.

This returns JSON state with `status: "proposal_ready"` and `proposalSha256`.
Review `proposal.json`, including evidence references, rationales, and uncertainties;
then substitute its exact reported hash below. Acceptance is a separate human decision.

```sh
ideomancer accept .ideomancer/example-run --proposal-sha256 HASH
ideomancer render .ideomancer/example-run/architecture.json --out .ideomancer/example-run/diagram.svg
```

Acceptance writes `architecture.json` and `diagram.mmd`; this optional separate
render produces an accepted view through the manifest pipeline. Repeating
inspection with unchanged inputs reuses the run without rewriting artifacts;
changed source/configuration requires a new manual run path. Managed `analyze`
instead replaces its valid completed current run and resets acceptance. A cached proposal for the same
agent/model makes no new model call.
Failed or uncertain calls require explicit `--retry` after reconciliation, not an
automatic retry. This is not an exactly-once provider guarantee.
Immutable `attempt-NNNN.request.json` files and result receipts preserve attempt
history; `request.json` is only a convenience copy, never adapter execution input.
Interrupted acceptance can reuse identical outputs; conflicting files are refused.

See [agent workflow](docs/agent-workflow.md) for setup, protocol, timeout, and
freshness semantics, and [Python analysis](docs/python-analysis.md) for scope and limits.

## Graph Model

```yaml
schemaVersion: 0.1.0
id: my-project
name: My Project
nodes:
  - id: app
    label: Application
    kind: service
  - id: database
    label: PostgreSQL
    kind: storage
edges:
  - from: app
    to: database
    label: reads and writes
```

- `schemaVersion` must be exactly the string `0.1.0`.
- Graph `id` and `name` must contain non-whitespace text.
- At least one node is required. Node IDs must match `^[A-Za-z_][A-Za-z0-9_]*$`.
- Node `label` and `kind` are optional strings or null. A supplied non-null
  `kind` must contain non-whitespace text; it is descriptive only, not a style
  or behavior selector. Missing/null node labels fall back to the node ID.
- Edges are optional (default `[]`), with required string `from` and `to`
  references and an optional string-or-null `label`.
- Unknown fields are rejected at every object level. Node IDs must be unique
  and edge endpoints must reference existing nodes.

`examples/manifest.schema.json` is a JSON Schema draft 2020-12 for editor and
external tooling use, including YAML tools that support JSON Schema. Its
`default` values are annotations, not inserted data; `nodes` is required and
nonempty. Duplicate IDs and reference integrity are checked by the Rust
semantic validator, not JSON Schema.

Mermaid generation preserves node and edge input order, prefixes generated
identifiers with `n_` to avoid reserved tokens, and escapes labels. The YAML
pipeline example is `examples/ideomancer.idman.yaml`; `examples/manifest.json`
models a web service. `examples/project.idman.yaml` is a compact component map.

## Development

```sh
cargo build --locked
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
```

The core tests use synthetic sources and agent/renderer stubs, need no provider
credentials or `mmdc`, and need no Node/browser installation. Install Python 3.12
for the workflow tests; they skip Python-dependent cases when it is unavailable.
`IDEOMANCER_TEST_PYTHON` can select a trusted Python 3.12 executable.
JS/TS extraction tests use the compiled-in pinned grammars, not an external
Node/compiler installation. CI keeps Python, Rustfmt, Clippy, tests, reviewed
package inventory/package verification, and the release build; it installs no
Node/TypeScript prerequisite. Optional member
rendering validation requires existing `mmdc` and Chromium, with no downloads:

```sh
cargo test js_focus::tests::real_mermaid_accepts_javascript_members -- --ignored
```

JS/TS pages reuse the existing offline browser regression below.
An optional offline browser regression uses an existing absolute Puppeteer module
and its already-installed Chromium; it downloads nothing:

```sh
IDEOMANCER_PUPPETEER_MODULE=/absolute/existing/puppeteer cargo test offline_chromium_regression -- --ignored
```

The optional grouped-UML browser regression also requires trusted Python >=3.12
(selectable with `IDEOMANCER_TEST_PYTHON`), external Node, and an already-installed
Puppeteer/Chromium setup; nothing is downloaded:

```sh
IDEOMANCER_TEST_NODE=/absolute/trusted/node IDEOMANCER_PUPPETEER_MODULE=/absolute/installed/puppeteer cargo test --test class_uml -- --ignored --nocapture
```

`tests/viewer_browser.cjs` also supports smoke checks of generated diagram pages
(not the script-free navigation index):

```sh
IDEOMANCER_PUPPETEER_MODULE=/absolute/existing/puppeteer node tests/viewer_browser.cjs --smoke /path/to/generated/index.html
```

With `mmdc` and its browser runtime installed, optionally run the standalone SVG
escaping smoke test and the draft SVG test (synthetic source/fake agent, real `mmdc`):

```sh
cargo test --locked --test cli -- --ignored
cargo test --locked --test analyze -- --ignored
```

Optional ignored bridge tests use the actual Ideomancer and prebuilt Djinn binaries
against a loopback fake provider, with synthetic credentials and no external calls.
They cover the lower-level workflow, `analyze --no-render`, stored OAuth/Codex SSE,
and focused analysis, including draft generation without automatic acceptance.
The focused fixture checks that request input equals the saved packet and
instructions equal the exact `skill --focus` output, and that cached reuse makes
no extra provider call. These are offline protocol checks, not automatic live
provider loops or live-model quality evidence:

```sh
IDEOMANCER_DJINN_BIN=/absolute/path/to/built/djinn cargo test --locked --test djinn_bridge -- --ignored
```

They require trusted Python >=3.12 and do not build Djinn or exercise a live model.
An additional optional presentation-builder smoke test checks both arrow styles
with real `mmdc` and its browser runtime:

```sh
cargo test --locked --lib render::presentation::tests::local_mmdc_accepts_both_arrow_styles -- --ignored
```

CI sets up Python 3.12 and checks formatting, clippy, tests, and release builds on
Linux and macOS. Tests do not exercise production provider credentials.

`make` (or `make build`) performs a locked release build and copies only the
executable into `bin/`; it does not install it. `make install` explicitly builds
and installs. `make test` runs locked tests, and `make check` checks formatting
and runs clippy with warnings denied. `make clean` cleans Cargo build artifacts
and removes only `bin/ideomancer`, leaving other local binaries alone.
`APP_NAME`, `BIN_DIR`, `INSTALL_DIR` (default `$HOME/.local/bin`), `TARGET_DIR`,
and `CARGO` can be overridden, for example:

```sh
make install INSTALL_DIR="$HOME/.local/bin" BIN_DIR="./local bin"
```

## Migration And Scope

The Go implementation and module files are not part of this baseline; older code
belongs to prior lineages, not this repository's ancestry. Legacy resource/operation manifests (`schemaVersion: 0.2.0`)
and the old graph DSL are not supported or automatically migrated. Rewrite
those inputs as explicit nodes and edges using the current examples. This is
not GUI or event-bus feature parity with the old implementation.

An interactive exploration service, watch mode, an event bus, and auto-layout
controls remain deferred. Generated HTML provides local image pan/zoom, not live
analysis or an interactive exploration service.

## Direction

The core is understanding existing applications in chunks: local discovery,
filtered catalogs, source-grounded API/class/callable diagrams, and explicitly selected
agent-interpreted workflows. This is not automatic reproduction of a curated
architecture diagram or proof of runtime behavior. Framework-aware resolution
remains bounded: Python framework scope is FastAPI-only, while the JS/TS slice
supports offline syntax-based navigation with selected relative named-function
calls and named-class UML relations. Consumer adapters can add explicitly attributed,
validated class relationships; retired operation/query profiles are not replaced
by those claims. General module resolution, runtime/DI, JS network proof,
and JS/TS agent analysis remain deferred; Rust analysis remains future work.
Private application studies belong in ignored `.ideomancer/` directories rather
than distributable examples.

## Related files

- `src/render/class_uml.rs`, `tests/class_uml.rs`, `tests/class_uml_browser.cjs`:
  grouped UML, independent full/focus bundles, and optional browser checks.
- [Discovery](docs/discovery.md): callable/endpoint API / Interfaces, Data Flow, Edge Cases,
  and Example usage for local catalog views.
- [Python analysis](docs/python-analysis.md): extraction, signature-backed candidates, and limits.
- [JavaScript/TypeScript analysis](docs/javascript-typescript-analysis.md): trusted tools,
  fixed scope, language evidence, MVP acceptance matrix, and deferred capabilities.
- [JavaScript worker](examples/javascript-worker/README.md): public worker and module-helper fixtures.
- `src/javascript.rs`, `src/javascript_native.rs`, `src/javascript_native/optional.rs`, `src/js_focus.rs`, `src/js_modules.rs`:
   trusted syntax extraction, atomic lexical/module proofs, and selected-source resolution.
- [Agent workflow](docs/agent-workflow.md): current bundles, acceptance, and recovery.
- [Python worker](examples/python-worker/README.md), `examples/python-worker/worker.py`:
  non-web source-only example with no third-party dependencies.
- `tests/callable_views.rs`: offline callable selection, contract isolation, and bounds.
- `src/cli.rs`, `src/discovery.rs`, `src/router_context.rs`, `src/openapi.rs`, `src/render/endpoint.rs`:
  CLI and local source/contract projections.
- `tests/converter_matching.rs`: offline CLI converter matching and guard regressions.
- `src/analyze.rs`, `src/presentation.rs`, `src/workflow.rs`: current analysis lifecycle.
- `src/render/viewer.{rs,js,css}`, `tests/viewer_browser.cjs`: shared offline viewer and browser checks.

## License

Ideomancer's CLI and reusable engine are licensed under the [MIT License](LICENSE).
Third-party dependencies retain their respective licenses.
