# Python Source Analysis

Start with [local discovery and diagram chunks](discovery.md): `discover ROOT`,
`catalog SAVEDDIR`, and bounded API/class/callable source views require no agent. Select
one workflow without `--view` for focused agent interpretation. These are source-syntax
candidates, not complete business-workflow discovery. Direct `analyze --focus`
remains an advanced fallback.

Agent interpretation is **partially implemented**. `analyze` automatically extracts
static evidence, requests and validates a responsibility-level proposal, and generates a DRAFT
manifest, readable HTML viewer, JSON legend, Mermaid source, and SVG. A human
reviews the draft before explicitly
accepting its fingerprint; generation does not imply acceptance. See the practical
[agent workflow](agent-workflow.md).
The first [JavaScript/TypeScript slice](javascript-typescript-analysis.md) now
supports local syntax-only discovery and saved views; Rust remains future work.

## Overview

Python framework-aware scope is **FastAPI-only**. This is not a promise of Django
or Flask support and does not remove basic non-web Python callable discovery,
local service/network candidate views, or the existing Python agent path.

Explicit `network`/`services` views stay local for one API or callable workflow
entry plus saved evidence. Only API entries can be enriched by a local
OpenAPI/Swagger export; non-web callables need no contract. See
[endpoint interfaces and examples](discovery.md#endpoint-views).

## Evidence And Interpretation

Local catalog/API/class/callable views project saved observed syntax directly, with no
proposal or acceptance state. API paths are fragments, not full URLs; runtime
activation remains unresolved; native endpoint views separately derive bounded
source-prefix context, not full runtime paths. Public module functions and
direct class methods are workflow candidates, not a whole-program graph.

1. **Observed syntax:** modules, classes, functions (including nested/async
   definitions), imports, decorators, bases, parameter annotations/defaults,
   return annotations, parameters, assignment bindings/invalidation, and call
   expression shapes, with source references.
2. **Interpretation:** agent-proposed components and relationships with supporting
   fact IDs, rationales, `observed`/`inferred` basis, and explicit uncertainties.
   Draft views are visibly marked DRAFT in the manifest name and Mermaid
   frontmatter title, including the rendered SVG.
3. **Accepted views:** the exact human-reviewed proposal projected to a `0.1.0`
   graph. Replacing a completed current analysis resets acceptance; old decisions
   and outputs are not retained as snapshot history.

`self.service.method()` is observed syntax, not proof of the receiver's concrete
binding or an executed network request. Imports, containment, dependency injection,
calls, and runtime effects must not be conflated. A module/import graph is evidence,
not the architecture itself. Agent grouping is a proposal, not exact reproduction
of a manually curated responsibility-level diagram.

## Current extraction boundary

```sh
ideomancer analyze /path/app
```

This is the advanced agent path: no separate manual interpretation is needed. Default
output is `.ideomancer/analyses` relative to the invocation cwd; containers and
readable application-named current run directories are created automatically.
`--out BASE` changes the container, not the exact run path. `mmdc` is required by
default; `--no-render` saves the draft manifest, Mermaid source, legend, and HTML
without generating SVG.
`--extract-only` extracts locally with no agent or renderer. Lower-level `inspect`
and `interpret` remain debugging/recovery tools; only `inspect --out RUN` needs an
existing output parent and selects an exact run directory.

The explicitly supplied root must be an
existing non-symlink directory. Filesystem extraction currently supports Unix
(Linux/macOS) only; other operating systems fail explicitly without an insecure
pathname-open fallback. The scanner recursively selects `.py` files under
`ROOT/src` when that directory exists, otherwise under `ROOT`. It does not scan
other root-level Python files alongside an existing `src` tree.

Excluded entry names are `.git`, `.venv`, `venv`, `__pycache__`, `node_modules`,
`.ideomancer`, `caches`, `.cache`, `.pytest_cache`, `.mypy_cache`, `.ruff_cache`,
`build`, `dist`, `scripts`, `migrations`, `shadow`, `tests`, `alembic`, `env`,
`data`, and `db`. Non-Python contents, such as `.env` and database files, are not
read. These are fixed name-based exclusions, not a general secret detector;
operator scripts with other names can still be selected. There are no include/
exclude flags or separate operator-script opt-in mode yet.

Traversal and reads are anchored to directory descriptors, opening each untrusted
path component with `O_NOFOLLOW` and nonblocking flags, then checking the opened
descriptor's file type. This avoids following swapped symlinks or blocking on a
FIFO substituted for a source file. Symlinks are not followed.
Relevant skipped symlinks, non-regular Python entries,
filesystem failures, and parse failures mark evidence `complete: false`.
An excluded symlink can be reported without making the scan incomplete.
`dynamic_discovery_unresolved` reports a static-analysis limitation, not a failed
scan: it does not by itself change completeness. Complete means the selected
source was available and parsed, not that all runtime behavior was resolved.

Limits are 5,000 files, 1 MiB per file, and 16 MiB total source. Hard resource
limits fail extraction rather than return a successful partial snapshot. When
eligible files exist, incomplete extraction can save diagnostic artifacts but
`inspect` exits 1. `analyze` reports `status: "extracted"`, `complete: false`, and
an error and exits 1 without an agent call; interpretation and acceptance reject
incomplete evidence.
No eligible Python files is an error.

## Evidence and projection

`evidence.json` uses `ideomancer.evidence.v1`, separate from the graph schema.
Python retains that schema: legacy `language: None` omits the language field on
serialization, preserving existing serialized evidence and old snapshot validation.
JS/TS instead uses evidence v2 with explicit parser/runtime metadata; its native
Tree-sitter migration does not repurpose `pythonVersion`. Python Tree-sitter is a
roadmap milestone, not a replacement for this Python >=3.12 AST helper today.
See [JS/TS evidence](javascript-typescript-analysis.md#data-flow).
It records extractor/Python versions, file hashes, facts, diagnostics, completeness,
and `snapshotId`. Facts have root-relative paths, line/end-line ranges, symbols,
and stable IDs; file-content hashes live in the file inventory. Rust reads each
file once and hashes the same bytes passed to the parser.

The extractor version is `python-static-8-generic-facts`. Alongside route/entrypoint
metadata described below, it emits `binding` facts for
plain and annotated assignments whose source is a name, attribute reference, or
call, and whose target is a simple name or an attribute chain rooted at `self` or
`cls`. A binding's `value` is a JSON string with `target` and `source` expression
shapes, for example `{"target":"self.repo","source":"repo"}`. Call literals
remain redacted except the route metadata described below. Unsupported targets
such as destructuring and subscripts are not modeled as bindings. These facts
record safe static assignment syntax, not
alias/type inference, guaranteed receiver identity, or proof of DI/runtime wiring.

Call expression rendering visits the callee **before** arguments can exhaust the
shared expression-shape budget. This preserves ordinary call-target evidence for
converter-registration guards without auxiliary call facts or a public fact-type
change. Existing limits remain: 80 expression nodes, depth 12, up to 16 positional
and 16 keyword arguments, and 512 UTF-8 bytes per rendered expression. Ordinary
literals, including converter-registration keys, remain `<literal>`; truncated
argument shapes are not complete call/runtime proof.

`signature` facts preserve bounded parameter order/kinds, without evaluating
defaults, to validate supported constructor argument forwarding. They are
support-only in global selection, not standalone anchors.
`parameter` facts record lexical parameters (typed or untyped) so shadowing does
not falsely resolve a same-named global helper or import. `binding_invalidation`
facts record supported targets assigned unsupported right-hand-side expressions
or written through destructuring, preventing stale receiver bindings from being
reused. In global bounded selection,
these and the new `api_route` / `entrypoint` metadata are support/context facts,
never standalone global anchors. Direct FastAPI context metadata (`router_prefix`,
`router_include`, `router_host`, `router_mutation`) is also support-only, not new
global anchors. Parameter and invalidation facts also inform
the focused resolver's conservative lexical guards.
These additions keep the public evidence/packet types unchanged and do not mutate
analyzed application source. Assignment handling covers `Assign`/`AnnAssign`, not
`AugAssign`, `NamedExpr`, or `del`; it is not complete control-flow analysis.

Snapshot identity includes scan policy/extractor versions, Python version,
package context, and sorted paths/content hashes, not just Git HEAD or timestamps.
Dirty and untracked selected files count. Root location is recorded separately
in run state. Scope policy is currently versioned internally, not a configurable
policy manifest in the evidence.

The `analyze` run key also includes a fingerprint of the full evidence, including
diagnostics and completeness, as well as source root and tool/model selections.
It is not just `snapshotId`: repairing an incomplete scan replaces the current run
even if the snapshot ID is unchanged. Extractor upgrades also change identity for
unchanged source. Valid completed current runs can be replaced, resetting
acceptance; failed/running/indeterminate work must first be reconciled and explicitly
retried, not discarded via changed inputs. Identity hashes are metadata, not folders.

`modules.json` and `modules.mmd` contain one node per module and conservative local
import edges. Exact unique local names, including supported relative imports, can
resolve; external, ambiguous, or unresolved imports produce no guessed target edge.
There is no whole-program call graph or complete DI/framework resolver. Ambiguous
imports are omitted, not currently given a dedicated ambiguity diagnostic.

## API / Interfaces

```sh
ideomancer diagram CATALOG --entry API_ID --view network --no-render
ideomancer diagram CATALOG --entry API_ID --view services --openapi /path/to/export.yaml --focus-hops 3 --focus-facts 600
ideomancer diagram CATALOG --entry WORKFLOW_ID --view services --no-render
```

Explicit views require one API or callable workflow entry, not a class or batch.
Functions, async functions, methods, CLI commands, and jobs are callable candidates.
Use `--kind workflow --entry SYMBOL` when an API entry duplicates the symbol;
exact IDs stay exact, and native views reject kind/ID mismatches. Expansion uses
`--focus-hops 0..5` and `--focus-facts 1..2000`, defaulting to 3 and 600. Unlike
workflow interpretation without `--view`, they never invoke Python, rescan, execute
the application, or contact an agent/model. `--no-render` avoids `mmdc`; only SVG
rendering requires it. `--extract-only` is invalid with an explicit view but remains
valid for workflow agent preparation without `--view`. Without `--view`,
`--kind workflow --entry API_ID` retains its agent override, and `--no-render`
alone does not disable the agent.

Callable legends use `ideomancer.callable-view.v1`, report code
`STATIC_CALLABLE_CANDIDATES`, `contract: null`, and `not-applicable` statuses for
`contractMatch` and `routerContext`. No HTTP inbound node is fabricated. CLI/job
metadata and citations remain within shared fact/byte budgets. Mode follows the
item kind, not role: a workflow `http_handler` is still callable-only; select its
API ID for contract enrichment. This reuses the historically named
`src/render/endpoint.rs`, with no new flags, catalog kinds, or extractor version.

Endpoint legends add `routerContext` with cited candidates, status, warnings, and
unknown boundaries. Catalog items are unchanged: `routePath` remains a fragment
and `pathIsFragment` remains true.

`routerContext.converterProjection` adds `status`, `framework`, `profile`, and
`reason`. Supported-context `contractMatch` adds `converterProjections[]` with
`originalPath`, `projectedPath`, ordered `parameters` (`parameter`, `converter`),
`status`, and `reason`, plus `converterProjectionContext` containing `eligibility`,
`evidenceIds`, and `sourceRefs`. See [endpoint metadata](discovery.md#endpoint-views)
for statuses. Catalog fragments and context `composedPath` remain unchanged.

### Data Flow

Saved evidence feeds shared bounded breadth-first callable expansion and a network
or services projection, producing Mermaid, legend, HTML, and optional SVG.
For API entries only, an optional local OpenAPI
3.0/3.1 or Swagger 2 JSON/YAML file (at most 4 MiB) supplies method/path, security,
request/response, and server declarations. No fetching or target execution occurs;
external references are rejected. Associations remain candidates, not runtime
security enforcement, activation, execution-order, or deployment proofs.

Router context supports a module-level directly imported FastAPI
`APIRouter(prefix=literal)` with at most one same-module `include_router` mount.
Canonical identity and lexical guards apply without executing imports or target
code. Class names do not supply prefixes and generated `operationId` values are
not decoded. Custom registration/factory recipes belong to consumer adapters,
not public Core acceptance.

Only complete, budgeted proof narrows method/composed-path suffix candidates.
`fullPathProven` remains false: outer mounts and dynamic activation are UNKNOWN.
Unsupported context uses old fragment/exact-name rules; supported context that
contradicts the export yields `context-mismatch`, not a generic fallback.
Insufficient budget yields context `budget-unknown` and association `unknown`.

Only eligible supported FastAPI context can additionally project `{name:converter}`
to `{name}` for `str`, `path`, `int`, `float`, and `uuid`. A bounded strict parser
allows at most 2048 UTF-8 bytes and 64 parameters per candidate and preserves names,
order, literal text, and trailing slashes. Literal and projected operation matches
are combined with per-operation deduplication, retaining ambiguity. No method/type
enforcement, activation, runtime routing, or full-path proof is added.

Any observed known or suspected `register_url_convertor` call anywhere in saved
evidence blocks projection globally. Possible-name closure covers import aliases,
re-exports, class-qualified binding aliases, and binding chains without executing
them. Since registration key literals are redacted, the guard cannot know which
built-in converter might be overridden. Alias-closure overflow fails closed;
all blocking guard proof bundles/citations are retained before the existing endpoint
cap, not trimmed to leave an eligible result. Hidden runtime converter changes
remain UNKNOWN even when no registration is observed.

Endpoint factory provenance accepts one complete direct callee-call initializer,
not a chain, subscript, or truncated call shape. With the existing fully cited
factory parameter/class/direct-method/return-annotation proof, a direct PEP 604
union (`A | B`, extracted as `A BitOr B`) supports up to four plain named members
(counted before deduplication). Branches resolve independently and deterministically,
including aliased imports and reversed member order. A branch with a known SDK
operation yields a dashed **declared SDK return branch candidate; runtime selection
UNKNOWN**; the whole union receiver is never classified as SDK. Local wrappers
remain UNKNOWN, with no `__getattr__` interpretation.

Receiver `branches` and `resolutionTrace` retain `branchCoverage`, `stoppingReason`,
citations (`sourceRefs`), and budget diagnostics. Complete coverage describes static
proof retention, not runtime selection. All branch proof and route attachments
count against the fact limit and exact consolidated compact UTF-8 JSON limit of
196608 bytes; incomplete or over-budget proof cannot support a partial accepted SDK
claim. Network grouping separates branch-candidate provenance from ordinary SDK
provenance, while existing legend summaries and diagram labels show unresolved
alternatives. Runtime selection, URL, HTTP verb, request occurrence, and runtime
security remain UNKNOWN.

### Edge Cases

Unknown/custom converters, mixed known/custom placeholders, malformed braces,
repeated colons, invalid ASCII parameter names, and parser limits reject the entire
candidate projection, never a partial rewrite. Literal matching still works.
Supported-context contradictions cannot fall back to handler-name/`operationId`
matching. Complete guard proof must fit the unchanged fact/byte limits or narrowing
fails closed. Saved `python-static-6-router-context` evidence still permits literal
router-context associations, but converter eligibility is unknown because older
call shapes may have lost targets; rediscover with the updated extractor.

Known empty route `''` is distinct from dynamic/null; it can compose with supported
prefix context but never drives an empty-suffix match-all. Dynamic prefixes,
splats, shadowing, alias escapes, and relevant mutations block unsafe prefix claims;
unknown context can leave legacy fragment matching ambiguous.

Callable views never inspect conventional root OpenAPI/Swagger documents, so
malformed or multiple files cannot break them. Explicit callable `--openapi` fails
as API-only before contract-file access. API behavior is unchanged:
a single root `openapi.{json,yaml,yml}` or `swagger.{json,yaml,yml}` is auto-detected;
none yields source-only views, multiple candidates require `--openapi FILE`.
An app's runtime-generated spec is not a local export: do not import/start the app;
use a user-provided export if available. Known HTTP/SDK operations need provenance
before appearing as network edges. Unknowns remain explicit legend entries, not
network guesses. Unsupported generic, `typing.Union`, `Optional`, `Any`, `None`,
quoted forward, variadic, or truncated return annotations, more than four union
members, and incomplete/ambiguous provenance remain UNKNOWN with a `stoppingReason`.
Local relative imports can identify a union alternative's class but cannot prove external
SDK provenance or wrapper delegation. Endpoint selection reserves complete supported
context facts/files and fact count before focus, beyond the existing 16 KiB
provenance, route-attachment/file bytes, and 2 KiB envelope headroom. All retained
references count in the final consolidated input within the unchanged 196608-byte limit;
agent-focused selection retains its existing policy. Services are bounded to
40 call nodes and 20 callables (network: 20 network groups), in addition to focused
hop/fact/byte limits. Unresolved and omitted relationships are known limits, not
proof that no additional behavior exists.

### Example usage

For a non-web, standard-library-only source example, see
[Python worker](../examples/python-worker/README.md):

```sh
ideomancer discover examples/python-worker
ideomancer diagram .ideomancer/catalogs/python-worker --kind workflow --entry run_batch --view services --no-render
ideomancer diagram .ideomancer/catalogs/python-worker --kind workflow --entry run_batch --view network --no-render
```

Callable and API views share current `views/{services,network}/index.html` paths.
The script-free `views/index.html` uses generic network/service call-candidate labels,
not endpoint-only labels. Changed selections replace the current view.

A supported eligible `/items/{name:path}` context can associate with a declared
GET `/api/items/{name}` by suffix while keeping `/items/{name:path}` in context and
`fullPathProven: false`. `/api/items/{other}` or an added trailing slash does not
match. If the export also declares `/api/items/{name:path}`, both operation
candidates remain ambiguous. Use the endpoint commands above; no new flags or UI
controls are required.

For `CATALOG=.ideomancer/catalogs/sample-app`, endpoint viewers are
`.ideomancer/catalogs/sample-app/views/{network,services}/index.html`; API/class
viewers use sibling `api`/`class` directories. Each holds only its current view.
The single current analysis viewer is
`.ideomancer/analyses/sample-app/presentation/index.html`; changing focus replaces
the previous completed analysis. There is no automatic legacy migration: inspect
and explicitly clear legacy hash storage before using the new layout. Successful
replacement retains no old hashes as folders. Manual `inspect --out RUN` remains
exact-path and no-clobber.

## Agent evidence selection

Full `evidence.json` stays local. Without focus, policy `architecture-packet.v2-relations` keeps
the existing 192 KiB limit (196608 bytes), measured from actual compact serialized
UTF-8 JSON including escaping and selection metadata. If the full input fits, it
is supplied unchanged. Larger inputs receive a deterministic subset of original
facts; file inventory, diagnostics, and snapshot identity remain present.

Selection uses package/file/lexical-scope fair ordering and seeds available
relationship fact kinds when they fit before bulk filling. The fact budget has
anchor-byte shares of 20% declarations (modules/classes/functions), 15% imports,
35% wiring (bindings, decorators, bases, annotations/defaults), and 30% calls.
An anchor's bounded lexical context contains at most 10 original facts, potentially
linking enclosing declarations, a receiver assignment, a constructor parameter
annotation, and a representative call. Newly added context is charged to its
anchor category; shared context is paid once. Smaller owner/anchor and anchor-only
fallbacks allow selection when the full neighborhood cannot fit. Unused shares
are redistributed, relationships first. These shares are not guaranteed final
kind percentages or fact-count quotas, and context is not guaranteed for every
selected anchor. All selected facts retain original contents and hash-based IDs;
the policy does not invent edges or infer runtime bindings.

Subset input adds `selection`: `policyVersion`, `totalFacts`, `includedFacts`,
`omittedFacts`, `maxBytes`, `byKind`, `warnings`, and a coverage `note`. Coverage is
also computed for unchanged full inputs. `evidence-coverage.json` records
`policyVersion`, `selectionApplied`, actual `inputBytes`, `maxBytes`, fact totals,
`totalFiles`, `includedFiles`, per-kind total/included/omitted counts, and warnings
for limitations such as partial selection or unavailable relationship evidence.
These counts and warnings describe coverage, not confidence or architectural truth.

Initial `inspect` and `analyze --extract-only` persist `evidence-packet.json` and
`evidence-coverage.json` with state hashes before any agent call, when preparation
succeeds. `interpret` uses that hash-verified stored packet, not a regenerated
selection. Proposal evidence IDs must be in the actual sent input, not merely the
full inventory; this also applies to cached validation, draft generation, and
acceptance. Review the packet, coverage, and immutable attempt request locally.
If metadata or oversized facts leave no useful packet, the full inventory is
preserved with `packetError`. Complete-source no-focus `--extract-only` succeeds with that
diagnostic; normal analysis errors before dispatch. `complete: false` always
blocks the agent and makes analysis fail, even in extraction-only mode.

`complete: true` describes source scan completeness, not packet exhaustiveness or
resolution of all graph relationships. Omitted facts may contain important
relationships. The byte bound excludes the rest of the request and provider
messages and is not a token/context-window fit guarantee. Accepted interpretations
still need human review for omissions.

The version-2 interpretation skill asks labels and rationales to distinguish
provider wiring, construction, injection, delegation, data/view composition, and
association. Imports remain imports, not runtime calls, and binding assignments
are not DI proof. The proposal remains `ideomancer.architecture.v1`, without a
typed relationship-kind field or extra proposal fields. `analyze --focus` changes
the input scope and adds focused guidance, not the proposal schema.

## Focused workflow scope

```sh
ideomancer analyze /path/app --focus OrderService.submit
ideomancer analyze /path/app --focus OrderService.submit --focus-hops 0 --focus-facts 200 --extract-only
ideomancer skill --focus
```

Focus produces an operation-centered workflow graph rather than a broad
application-composition overview. `--focus SYMBOL` resolves an exact fully
qualified callable symbol or fact ID first, otherwise a unique dot-suffix such as
`OrderService.submit`. Only function/async-function facts, including methods, can
anchor a view. There is no path/colon query or HTTP method/path query. Missing,
ambiguous, and non-callable anchors are local preparation errors before an agent
call; diagnostics suggest callable names, locations, and IDs. Inventory can still
be persisted with `packetError`. Invalid focused preparation exits 1 even with
`--extract-only`, unlike the no-focus packet-preparation diagnostic behavior above.

`--focus-hops` accepts 0 through 5, default 3. `--focus-facts` accepts 1 through
2000, default 600, counting every selected fact including lexical context. Both
require `--focus`; missing focus or invalid ranges are usage errors (exit 2).
Depth 0 retains only the anchor's body facts and relevant context, without
expanding candidate callee bodies. Limits can omit body/context facts even there.

Deterministic breadth-first expansion follows unambiguous local helpers and
`self` methods, supported relative/aliased imports, typed constructor field
bindings such as `self.repo = repo`, and locally constructed receivers. Supported
single-base inherited constructor wiring through `super().__init__` is validated
against signature facts and argument mapping. Nested handlers can retain an
unshadowed lexical `self`; these resolve supported nested handlers and service
handoffs, not general inheritance/MRO or runtime DI. These
links are **static candidates**, not runtime receiver/dispatch proof. Direct calls
on typed parameters remain unresolved; annotations alone do not activate DI.
Parameter shadowing and unsupported-RHS invalidation block unsafe name/binding
fallbacks. Same-line ambiguous assignments to `self`, its methods, or a receiver
stay unresolved rather than reusing a stale binding or falling back to the
original local method. Lexical guards distinguish actual declaration instances;
constructor-field context must belong to the same class, not a nested or repeated
class. Ambiguous names, external SDKs, unsupported nested/dynamic receivers, redacted
annotations, and missing local method bodies remain explicit boundaries rather
than guessed targets. There is no complete inheritance/MRO resolution, caller
discovery, registry/plugin evaluation, or framework DI inference.
`AugAssign`, `NamedExpr`, and `del` mutations are unsupported, as is complete
control-flow analysis. All resolved links remain static candidates despite these
lexical guards.

Focused packets use `architecture-focus.v1` even when the inventory is small.
Input `focus` contains `policyVersion`, `requestedSymbol`, resolved `anchor`
(`id`, `symbol`, `path`, `line`, `endLine`), `maxHops`, `maxFacts`,
`visitedSymbols`, candidate `links`, `unresolved`, `truncated`, and `boundaries`.
Links carry `sourceFactId`, `targetFactId`, candidate `kind`, and `evidenceIds`;
all retained link IDs refer to selected facts. These input candidate kinds do not
add typed relationship kinds to the proposal. Unresolved metadata is capped at
100 entries, with an explicit boundary when more entries are omitted.

The maximum remains 192 KiB (196608 bytes) of actual compact UTF-8 JSON, including
focus and selection metadata. Fact/byte trimming can omit bodies, context, and
links; retained declarations are not proof their bodies are complete. Missing
bodies and depth/fact/byte boundaries must be explained in proposal uncertainties.
The full inventory remains local in `evidence.json`; the scoped packet and coverage
use `evidence-packet.json` and `evidence-coverage.json` with persisted hashes.
Coverage totals describe full inventory versus included focused facts/files, not
runtime reachability or confidence. Source `complete` still describes the full scan.

`skill --focus` prints the exact global skill plus focused workflow guidance.
`interpret RUN` automatically uses the persisted focused scope and instructions;
it has no focus flag. Output stays `ideomancer.architecture.v1`, with no extra
proposal fields. Labels/rationales describe supported responsibilities and handoffs,
not proven execution order, branches, atomicity, rollback, or failure policy.
Agent/model/timeout/render controls and the explicit acceptance gate are unchanged;
there is no automatic live retry.

## Parsing and privacy

The Rust controller invokes its bundled standard-library `ast` helper through a
trusted Python >=3.12 interpreter, default `python3.12`. It installs no runtime,
activates no target virtual environment, and imports or executes no target code.
The helper runs with `-I -S`, a neutral temporary cwd, and a cleared environment
except platform/path essentials. It receives source bytes, not target files to
open, through bounded JSON input. Its deadline is 60 seconds, with 40 MiB input
and 32 MiB stdout limits. Isolation is not an operating-system security sandbox.

Literal values are replaced by `<literal>` in expression descriptions; quoted
forward-reference annotations are consequently redacted too. The deliberate
exception in `python-static-8-generic-facts` is whitelisted route/context metadata:
direct empty or slash-prefixed route-path literals of at most 2048 characters,
recognized HTTP methods and router/include
prefix literals. Prefixes use a bounded lowercase slash-segment whitelist (at most
eight segments and 128 characters). This is an explicit identifier-metadata privacy boundary, not a secret
filter. Payload, credential, configuration, description, and legacy-service literals
remain redacted. Dynamic paths/methods remain unknown with diagnostics, never
evaluated. `api_route` and `entrypoint` facts support cataloging and focused context;
they and direct router-context facts are not standalone global packet anchors.
Native endpoint views derive only the supported prefix conventions above, never
runtime activation. Evidence retains
source paths, symbol/import names, expression shapes, and hashes, which can expose
proprietary structure. Redaction does not make evidence public or guarantee the
absence of secrets embedded in identifiers or paths.

`discover`, `catalog`, API/class/network/services source diagrams, `inspect`, and
`analyze --extract-only` are local. A workflow diagram selected without `--view` (unless
`--extract-only`) explicitly authorizes the focused agent path; its selected
callable's route metadata can be included in the packet. Its live extraction must
match the catalog snapshot before any model call; stale catalogs require
rediscovery and explicit reselection, not a silent refocus. Static views can still
show saved evidence without rescanning until rediscovery replaces it. Explicit `analyze` otherwise
opts into passing evidence to the configured agent/provider, as does `interpret`;
raw source is not included by default. Review the evidence policy, provider endpoint,
credentials, and adapter before invoking an agent path. Keep catalog/view/run
outputs private in ignored `.ideomancer/` directories; target repositories need
their own ignore policy.

## Draft review and reuse

The `analyze` JSON report includes `runDirectory`, `status`, `snapshotId`,
`proposalSha256`, `requiresAcceptance`, `complete`, `draftManifest`, `draftMermaid`,
`draftSvg`, `draftLegend`, `draftViewer`, `presentationVersion`, `evidencePacket`,
`coverageReport`, `coverage`, `packetError`, `focus`, and `error`; unavailable fields
are null.
`focus` is `{"symbol":"OrderService.submit","hops":3,"maxFacts":600}` for
that configuration, or null without focus. The packet/coverage fields provide paths
to the saved artifacts and the coverage object itself. Open
`draftViewer` first and review `proposal.json` and evidence before choosing
`ideomancer accept RUN --proposal-sha256 HASH` with that exact run and fingerprint.
First acceptance requires a fresh extraction matching the proposal's snapshot and
writes accepted `architecture.json` / `diagram.mmd`. Draft files remain DRAFT.
`requiresAcceptance` is true only for `proposal_ready` with a proposal fingerprint,
and false for `extracted`, `failed`, `indeterminate`, and `accepted`.

`analyze` automatically selects the current presentation, `readable-v2`.
`draftManifest` points to `presentation/draft.architecture.json`; `draftMermaid`, `draftSvg`,
`draftLegend`, and `draftViewer` refer to `diagram.mmd`, `diagram.svg`, `legend.json`,
and `index.html` under `RUN/presentation/`. The version remains receipt metadata,
not a folder name. There are no legacy root draft files, render blobs, or pending
render journals in the current presentation layout.

```sh
ideomancer present RUN --no-render
ideomancer present RUN --mmdc mmdc
```

`present RUN [--no-render] [--mmdc PROGRAM]` verifies and presents a saved
`proposal_ready` or `accepted` proposal and citation evidence. An `extracted` run
has no reviewable proposal and is not supported. Unlike `analyze`, `present` needs
no Python or source extraction; it uses no agent, model, or authentication, never
accepts, and never updates workflow state. `--no-render` still produces Mermaid,
legend, and HTML and reports `draftSvg: null`, even if a matching current-version
SVG is retained. A version upgrade with `--no-render` replaces the older bundle
with a source-only bundle. The
HTML references the SVG once rendered, but its full text is readable without SVG.

The preferred offline HTML viewer shares pan/zoom controls with API/class/network/
services source views. SVG remains an inert `<img>`, never inline SVG, an object,
or canvas; full labels, rationales, citations, evidence details, and uncertainties
stay outside the viewport. Initial 100% preserves readable native-size scrolling,
not a tiny default fit. Buttons provide `+`, `-`, Fit, Reset 100%, and Fullscreen
when available. Left-drag pans; ordinary wheel and touch use native scrolling
(no custom touch pinch), while Ctrl/Meta+wheel zooms at the cursor. Focus the
viewport for `+`/`-`, arrows, `F` to fit, and `0` to reset. Scale spans 0.1%–800%;
viewport resizing preserves zoom.

The trusted embedded static controller is authorized by its exact SHA-256 CSP hash,
not unsafe-inline scripts or eval. Styles are inline; source/model/spec content
never enters JavaScript. There are no CDNs, fetches, or viewer network calls.
Missing SVG leaves controls disabled and details usable; with JavaScript disabled,
an available SVG keeps original-size native scrolling and controls remain disabled.
The separate user views index remains **script-free**, linking all verified
generated views and artifacts. See [viewer interfaces](discovery.md#offline-diagram-viewer).

Diagram keys such as `E001` identify full relationship entries. Short labels are
excerpts, not semantic paraphrases; solid `observed` and dashed `inferred` arrows
preserve proposal basis rather than asserting runtime proof. Citation paths and
line ranges describe the saved snapshot, not live source. Presentation does not
change the full model, proposal/evidence hashes, or acceptance fingerprints, and
remains DRAFT even after acceptance, separate from accepted-graph authority.

Presentations are staged bundles published to the single current `presentation/`
directory. Receipts bind exact hashes and internal builder versions. Cached
`present` invokes neither agent nor renderer; changing `--mmdc` does not replace
a recorded SVG. A valid source-only bundle can be replaced by a rendered bundle;
tampering, missing recorded files, and unrecorded artifacts fail validation rather
than being restored from hidden blobs. See [readable presentations](agent-workflow.md#readable-presentations).
The internal `VERSION` bump from `readable-v1` to `readable-v2` regenerates a
verified older bundle in the same `presentation/` directory without another model
call or proposal/evidence/acceptance changes. Source-view HTML changes likewise
replace each verified current per-kind bundle, not create history folders.

Unchanged `analyze` calls reuse the validated proposal and any recorded SVG,
avoiding agent/render calls; accepted repeats report `requiresAcceptance: false`.
Source, full evidence (including extractor version), interpreter, packet policy,
bundled skill, agent, or requested-model key changes select
a replacement current run, resetting acceptance. Renderer settings are excluded from
the key, so changing `--mmdc` does not rerender a saved SVG. Deadline budgets also
do not participate in the cache key: changing the timeout reuses a matching proposal
without an agent call or budget consumption. Fresh model output may
vary; cached repeatability does not imply deterministic independent model calls.
To use a separate container, explicitly choose a different `--out BASE`.
Focus options, combined skill fingerprint, and focused policy are conditional in
run identity. Different anchors/hops/fact caps replace previous current views and
accepted models after validation. No-focus global guidance, packet selection, and cached-repeat
behavior remain unchanged for the current configuration, not identical to older
extractor-version run keys across upgrades.

Run state records `packetPolicyVersion`, `packetSha256`, and `coverageSha256`;
these fields are optional on legacy load, so `status` can still read old proposals.
Current `interpret` refuses a missing/old packet policy before dispatch and
requires a new run, without replacing saved proposals. First acceptance of an
old-extractor proposal requires generating and reviewing a new run with the
updated tool because current extraction cannot check old extraction semantics
for freshness. Already accepted runs retain their original artifact-reuse behavior.

Set `IDEOMANCER_AGENT`, `IDEOMANCER_MODEL`, `IDEOMANCER_PYTHON`, and
`IDEOMANCER_MMDC` once for `analyze`, or override with `--agent`, `--model`,
`--python`, and `--mmdc`. The default agent is Djinn. Ideomancer has no file-based
configuration or automatic arbitrary chat-CLI discovery; other agents require a
protocol-compatible wrapper. Updated Djinn reuses supported global model/auth,
including OpenAI OAuth/Codex and Copilot token exchange, without a login/browser,
UI, sessions, tools, or workspace config/instruction discovery. Only OpenAI,
Copilot, and unprefixed OpenAI model routes are supported, not every interactive
provider. OpenAI OAuth needs a configured Codex-compatible model; valid supported
stored auth needs no extra API key. Active external OpenCode OAuth is reused, but
only Djinn-owned expired OAuth auto-refreshes. Expired external auth requires
`djinn auth login` or refresh via its owning harness; Djinn never rotates or copies
it. Native Rust headless and interactive refresh/login writers share a lock.
Before refresh dispatch, the private native store durably records the SHA-256
refresh-token marker `_djinn_openai_refresh_inflight_sha256` to block replay after
cancellation/provider error. It is cleared after durable credential commit or a
separate explicit login. Reauthenticate/reconcile, not blindly delete markers/locks.
Even failed calls can modify the native auth store and leave an owner-only lockfile,
not app/session state; global access is not entirely read-only.
See [Djinn setup](agent-workflow.md#djinn-setup) for stored/inline Copilot precedence
and GitHub OAuth semantics (`refresh`, including `expires: 0`, not OpenAI expiry),
global JSON/JSONC model merging in the selected XDG root without HOME inheritance,
stow support, trusted endpoint overrides, incremental Codex SSE returning on valid
terminal completion before remote EOF, and the shared request-selected auth/model budget.
Model token/context limits still apply.
Global OpenCode `provider.models.<key>.id` aliases resolve by exact key once after
selection across merged JSON/JSONC maps, preserving provider-prefix dispatch and
excluding provider options, endpoints, and secrets. There is no dynamic model
discovery or automatic fallback. An unsupported configured model needs a compatible
`--model` or `IDEOMANCER_MODEL`, not an extra API key. Model errors retain the route
and selected model, with only validated identifiers/static reasons from bounded
HTTP-error JSON, not freeform provider bodies. Codex SSE allows benign
`response.metadata` and explicitly empty terminal output only with an explicit
completed terminal status, actual model identity, and validated nonempty
single-assistant stream text. Limit violations, preceding tools/refusals, multiple
outputs, and conflicting done text are still rejected; remote EOF is not required.

Agent/render failures normally return a JSON report with `error` and exit 1;
failures before a report can be formed instead go to stderr. Render failures keep
the proposal and draft sources: fix the renderer and use `present RUN` without
`--retry` for saved-artifact recovery, or repeat analysis to reuse the agent result.
Reserve `--retry` for reconciled failed or uncertain agent
work, not render recovery. Saved receipts and per-file publication support reuse
and conflict detection, not an all-files atomic transaction.
For an existing authentication-failed run, rebuild/update the configured Djinn
binary, ensure `PATH` or `IDEOMANCER_AGENT` selects it, and rerun the same `analyze`
command with `--retry` after reconciliation. Pre-dispatch auth timeouts are
configuration failures; model transport/deadline failures remain indeterminate.
No automatic provider retry is implied.
If `PATH` selects a stale installed binary, explicitly configure the updated
executable through `--agent` or `IDEOMANCER_AGENT`; there is no automatic
executable/provider fallback and no need to change global model preferences.

Both `analyze` and `interpret` default to a 300-second controller wall timeout,
configurable with `--timeout-seconds` from 1 through 600. New requests forward
`timeoutMs = wallMs - min(1000, wallMs / 10)` to updated Djinn: 299000 ms at 300
seconds, 599000 ms at 600 seconds, or 900 ms at 1 second. Djinn accepts optional
`timeoutMs` from 100 through 600000; explicit Djinn CLI `--timeout-seconds` (1 through
600) overrides it, and only standalone calls without either use 120 seconds.
Auth/model resolution consumes the selected total budget before the model call;
auth network/connect/`gh` subcaps are unchanged. The controller still hard-kills
the local child at its outer limit, without guaranteeing remote cancellation.

Legacy builds could time out at Djinn's inner 120 seconds despite an outer 180-second
limit. Update both binaries: older Djinn rejects `timeoutMs` as an unknown field,
with no fallback. An existing uncertain run is left untouched unless explicitly
retried after reconciliation. Only then, a new potentially billable call can use:

```sh
ideomancer analyze /path/app --retry --timeout-seconds 600
```

Each explicit retry creates a new immutable attempt request carrying `timeoutMs`;
new result receipts record `timeoutMs` and `elapsedMs`. State has optional
`agentTimeoutMs` and `elapsedMs`, so older state remains readable. No deadline
increase or model retry is automatic. Timeout reports have `requiresAcceptance:
false`. More time does not guarantee success or reduce context, and timeout alone
does not establish model latency as the sole cause. Changing the deadline does
not change the 192 KiB limit, extraction budgets, source selection, or bundled skill.

Manual initial inspection is different: its complete artifact set is staged in a sibling
`.ideomancer-init-*` directory and published with an atomic no-replace rename.
Abandoned staging may remain after interruption but does not occupy the final run
path and is never automatically deleted during recovery. Managed current analysis
also stages replacement runs, then exchanges and removes previous contents.
Unix output-container
creation checks every component using anchored directory descriptors without
following symlinks, starting relative paths at the canonical invocation cwd;
only root-level macOS `/var` and `/tmp` system aliases are specially supported.

Draft SVG rendering is bounded to 60 seconds and 8 MiB. Unix cleanup sends TERM
to its owned process group, waits up to one second for graceful shutdown, then
escalates to KILL and reaps the renderer. This is not a sandbox against malicious
renderers or detached helpers, and SVG output is not sanitized.

## Validation and follow-ons

Synthetic extraction/workflow tests cover no target execution, literal redaction,
Python syntax, local imports, exclusions, symlinks, bounds, deterministic snapshots,
incomplete scans, protocol validation, proposal references, freshness, and reuse.
Packet unit and integration tests use synthetic evidence/sources and offline agent
stubs to cover bindings, relation-kind retention, fair selection, lexical context
and smaller fallbacks, actual compact-byte bounds, unchanged small inputs,
coverage, persisted hashes, sent-only references, and legacy-run safety.
CLI tests cover repairing incomplete scans by replacing the current run, including
when `snapshotId` is unchanged, and resetting acceptance after source changes.
Offline CLI tests in `tests/focus_workflow.rs` cover focus flag validation,
combined guidance, local extraction/defaults, zero hops, fact/byte caps, invalid
anchors, acceptance/cache reuse, persisted-scope interpretation, and separation
from global/accepted runs. Focus and lexical-extractor unit tests cover candidate
resolution, imports, shadowing, invalidation, ambiguity, and deterministic bounds.
CI installs Python 3.12; default tests require neither production providers nor
Mermaid CLI. Passing structural validation does not prove an interpretation correct.
Offline presentation tests in `tests/presentation.rs` and the presentation builder
cover saved-run generation/reuse, full-text preservation, escaping, basis styling,
integrity conflicts, and interrupted render recovery with synthetic inputs and
renderer stubs. Optional real-Mermaid smoke checks are listed in the
[README](../README.md#development); they are not live-model quality validation.
The optional offline browser regression is
`IDEOMANCER_PUPPETEER_MODULE=/absolute/existing/puppeteer cargo test offline_chromium_regression -- --ignored`.
It requires an existing Puppeteer module and installed Chromium, downloads nothing,
and checks viewer interaction and fallbacks. `tests/viewer_browser.cjs --smoke HTML ...`
also checks generated diagram pages with the same module setting.
Optional ignored [Djinn bridge tests](../README.md#development) use real binaries
with loopback fake providers, covering the lower-level workflow,
`analyze --no-render`, stored OAuth/Codex SSE, and focused analysis, not an external
provider or live model. The focused fixture checks exact combined skill guidance,
request input equal to the stored focused packet, and cached reuse without another
provider call. These are not automatic live-provider loops. An optional
`cargo test --locked --test analyze -- --ignored` uses synthetic source and a fake
agent with real `mmdc` and its browser runtime to check the visible DRAFT SVG.
No live-provider or private-application quality validation was performed for this
documentation update; private local application checks are separate. Synthetic or bridge
coverage is not a claim of live-model architecture quality.

Still unresolved: general framework-aware route/DI resolution beyond the bounded
source conventions above, richer ambiguity diagnostics,
configurable source roots/operator scope, multiple abstraction levels, editable
curated-view reconciliation, and proposed-change overlays. Current and proposed
architecture must remain distinct. The first JS/TS analyzer shares the evidence/view
boundary while documenting language-specific limits; JS/TS local relative imports
and runtime DI remain bounded as documented. Rust analysis is also future work.
A Rust-native Python parser remains an option if distribution/isolation costs
justify it, subject to grammar and source-range fidelity checks.

## Related files

- [JavaScript/TypeScript analysis](javascript-typescript-analysis.md): separate language
  schema, trusted tool selection, and local-only first-slice behavior.
- `src/analysis.rs`, `src/python_extract.py`: evidence types, extraction, signature facts.
- `src/focus.rs`, `src/packet.rs`: conservative resolution and bounded evidence selection.
- `src/openapi.rs`, `src/render/endpoint.rs`: local contract declarations and endpoint views.
- `tests/callable_views.rs`: native callable selection, contract isolation, and shared bounds.
- [Python worker](../examples/python-worker/README.md), `examples/python-worker/worker.py`:
  non-web source-only example.
- `src/router_context.rs`, `src/adapters/fastapi.rs`: generic context interface and direct FastAPI adapter.
- `tests/converter_matching.rs`: offline CLI converter projection, identity,
  ambiguity, literal matching, no-fallback, registration guards, and redaction.
- Tests in `src/analysis.rs`, `src/router_context.rs`, and `src/render/endpoint.rs`:
  call-target preservation, alias/re-export/class guards, closure overflow,
  legacy eligibility, retained citations, and proof-budget failure.
- `tests/provenance.rs`, `tests/network_provenance.rs`: synthetic offline factory
  union, initializer-shape, citation, and budget regressions, not private application results.
- `src/analyze.rs`, `src/presentation.rs`, `src/workflow.rs`: current runs, receipts, and review state.
- `src/render/viewer.{rs,js,css}`, `tests/viewer_browser.cjs`: shared offline viewer and optional browser checks.
- [Discovery](discovery.md) and [agent workflow](agent-workflow.md): selection and recovery.
