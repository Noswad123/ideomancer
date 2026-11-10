# Agent workflow

Ideomancer owns extraction, state transitions, validation, and diagram generation.
The core exploration path is [discovery and diagram chunks](discovery.md):
`discover ROOT`, `catalog SAVEDDIR`, and bounded API/class/network/services diagrams stay local.
Only a Python workflow explicitly selected without `--view` enters the focused agent path below; no
command automatically interprets every workflow. Static source views have no
architecture proposal or acceptance state.

The agent only proposes a responsibility-level grouping from saved evidence. A
human decides whether to accept it; structural validity is not architectural truth.

## Overview

Outputs are current-only, application-named directories, not hash-named snapshot
history. Receipts retain internal identities and versions. Provider access remains
an explicit workflow request without `--view`, or an `analyze`/`interpret` decision,
not part of native source views.
Python framework-aware work remains FastAPI-only, without removing basic Python
callable support or changing its agent path. The first JS/TS slice is local-only:
workflow analysis without `--view`, including `--extract-only`, is rejected before
Python/model dispatch. Use `--view services|network`; no agent fallback is attempted.
See [JS/TS analysis](javascript-typescript-analysis.md) for the separate trust boundary.

## API / Interfaces

```sh
ideomancer diagram CATALOG --entry API_ID --view network --no-render
ideomancer diagram CATALOG --entry API_ID --view services --openapi /path/to/export.yaml --focus-hops 3 --focus-facts 600
ideomancer diagram CATALOG --entry WORKFLOW_ID --view services --no-render
```

Explicit `--view services|network` requires one API or callable workflow entry,
never a class or batch. Functions, async functions, methods, CLI commands, and jobs
need no HTTP endpoint or OpenAPI document. Limits are 0..5 hops
and 1..2000 facts (defaults 3 and 600). They use saved evidence only: no agent,
extraction Node/compiler/Python invocation, rescan, app execution, model, or authentication.
`--no-render` avoids `mmdc` and its separate Node/Chromium renderer toolchain.
`--extract-only` is invalid with an
explicit view; for Python workflow agent preparation, omit `--view`.

Use `--kind workflow --entry SYMBOL` to disambiguate an API-duplicated symbol.
IDs remain exact; explicit native views reject kind/ID mismatches. A workflow
item with role `http_handler` stays in callable mode: select its API ID for enrichment.
Callable views do not even inspect conventional root OpenAPI/Swagger documents;
malformed or multiple files cannot break them. Explicit callable `--openapi` is
rejected as API-only before contract-file access.

API enrichment remains unchanged. Optional OpenAPI 3.0/3.1 or
Swagger 2 JSON/YAML exports must be local regular non-symlink files at most 4 MiB;
no references are fetched and external references are rejected. Exactly one root
`openapi.{json,yaml,yml}` or `swagger.{json,yaml,yml}` is auto-detected. None means
source-only; multiple files require explicit `--openapi FILE`.
For JS/TS API entries, enrichment is limited to literal method/path fragments;
Python FastAPI prefix/converter rules and SDK provenance are not applied. JS/TS
network/DI receivers remain unknown, and `.get` alone never proves an SDK operation.

### Data Flow

Saved callable evidence feeds shared bounded breadth-first expansion and a local
network/services diagram and legend, optionally joined to contract declarations
for API entries only. Contract method/path,
security, request/response, and server declarations are displayed, not proof of
runtime enforcement, activation, deployment, or execution order. Associations are
explicitly candidates. Network edges require provenance-backed known HTTP/SDK
operations; unknowns are explicit in the legend, never guessed network activity.
Services are bounded to 40 call nodes and 20 callables; network has at most 20
network groups. CLI/job metadata and citations count against shared fact and
196608-byte compact-JSON caps. Focus limits and unresolved/omitted details remain visible boundaries.

Callable legends use `ideomancer.callable-view.v1`, report code
`STATIC_CALLABLE_CANDIDATES`, `contract: null`, and `not-applicable` statuses for
`contractMatch` and `routerContext`, with no fabricated HTTP inbound node.
The historically named `src/render/endpoint.rs` serves both modes; no new flags,
catalog kinds, or extractor version are needed.

Native endpoint legends emit `routerContext`, derived from saved source only;
catalog `routePath` fragments and `pathIsFragment: true` are unchanged. Supported
direct FastAPI router/include prefixes can
narrow method/composed-path suffix candidates only with complete cited proof.
This is not a runtime full path: outer mounts and dynamic activation remain UNKNOWN.
No class-name prefix inference or generated `operationId` decoding occurs. See
[direct router context and guards](python-analysis.md#data-flow).

Eligible supported FastAPI context can also compare `{name:converter}` as `{name}`
for exactly `str`, `path`, `int`, `float`, and `uuid`, under the same declared-method
and composed-suffix rules. The strict parser is bounded to 2048 UTF-8 bytes and 64
parameters; names/order, literals, and trailing slashes are not normalized. Literal
and projected operation candidates are combined and deduplicated per operation,
preserving ambiguity. Source `routePath`, context `composedPath`, and
`fullPathProven: false` are unchanged; neither runtime routing, method/type
enforcement, activation, nor hidden runtime converter changes are proven.

Legends expose `routerContext.converterProjection` (`status`, `framework`,
`profile`, `reason`). Supported-context `contractMatch.converterProjections[]`
records `originalPath`, `projectedPath`, ordered `parameters` (`parameter`,
`converter`), `status`, and `reason`; `converterProjectionContext` supplies
`eligibility`, `evidenceIds`, and `sourceRefs`. These are local JSON metadata, not
new agent protocol fields or UI controls. See [endpoint details](discovery.md#endpoint-views).

Context facts/files and fact count, including all converter-registration guard
proof bundles and citations, are reserved before focus, beyond the existing
16 KiB provenance, actual route-attachment/file bytes, and 2 KiB envelope margin.
All retained references count in the consolidated input; the final 196608-byte cap
and agent-focused packet policy are unchanged.

### Edge Cases

Unsupported context retains old fragment/exact-name matching; supported context
contradicting an export reports `context-mismatch`, not a generic fallback.
Incomplete budgeted proof reports context `budget-unknown` and association `unknown`
without narrower claims. Known empty routes are not dynamic/null or match-all suffixes.

Unknown/custom or mixed known/custom converters, malformed placeholders, and parser
limits reject the entire projection while retaining literal matching. Any observed
known or suspected `register_url_convertor` call in saved evidence blocks projection
globally, including import aliases, re-exports, class-qualified binding aliases,
and binding chains. Registration literals stay redacted, so a built-in override
cannot be excluded by key. Alias-closure overflow and proof-budget failure fail
closed rather than dropping blocking evidence. Hidden runtime changes remain UNKNOWN.

The current Python extractor renders callees before arguments can
exhaust the existing expression budget, without auxiliary call facts, widened
literal retention, or increased caps. Legacy `python-static-6-router-context`
evidence can retain literal router associations but has unknown converter
eligibility; rediscover to use current guarded projection. This feature does not
authorize a rescan, agent call, runtime spec generation, or new audit command.

Supported signature-backed inherited `super().__init__` forwarding and nested lexical
`self` resolve static constructor wiring, not full MRO or DI. Do not run/import an
app to obtain a runtime-generated spec; a user can supply an export.
A factory union `SDKClient | LocalClient` can retain a
declared SDK return branch candidate plus an UNKNOWN wrapper alternative. The
whole receiver, runtime branch selection, and network destination remain unknown;
neither union annotations nor dynamic wrappers prove that a request occurred.

### Example usage

The standard-library-only [Python worker](../examples/python-worker/README.md)
needs neither a web framework nor a contract:

```sh
ideomancer discover examples/python-worker
ideomancer diagram .ideomancer/catalogs/python-worker --kind workflow --entry run_batch --view services --no-render
ideomancer diagram .ideomancer/catalogs/python-worker --kind workflow --entry run_batch --view network --no-render
```

Callable and API selections share current `views/{services,network}` bundles.
The script-free root `views/index.html` links them with generic network/service
call-candidate labels, not endpoint-only labels.

For `CATALOG=.ideomancer/catalogs/sample-app`, viewers live at
`.ideomancer/catalogs/sample-app/views/{api,class,network,services}/index.html`.
Each kind has one current view. Agent analysis has one current target at
`.ideomancer/analyses/sample-app/presentation/index.html`; changing focus replaces
the previous valid completed analysis and resets acceptance.

## Select A Workflow

This agent workflow section applies to Python. JS/TS native saved views require
neither extraction Node/compiler nor Python/agent (`--no-render` also avoids the
independent SVG renderer); selecting a JS/TS workflow without `--view`
fails locally even with `--extract-only` or `--no-render`.

Browse a saved catalog, then authorize one selected workflow using an updated
native Djinn executable and supported global model/auth:

```sh
export IDEOMANCER_AGENT=/absolute/path/to/updated/djinn
ideomancer catalog SAVEDDIR --kind workflow --query submit
ideomancer diagram SAVEDDIR --entry WORKFLOW_ID --no-render
ideomancer diagram SAVEDDIR --kind workflow --entry API_ID --no-render
```

Use actual catalog IDs. Exact symbols or unique dot-suffixes also work; `--kind`
narrows ambiguous symbol lookup, but exact IDs remain exact. The explicit workflow
override lets an API ID focus its endpoint handler **without `--view`**. This
retains the agent path, unlike an explicit native view; `--no-render` alone does
not disable the agent. Classes cannot anchor workflows.
`--entry` must not be combined with filters or paging overrides. Workflow selection
uses existing focused analysis, including a live snapshot check **before model
dispatch**. Stale snapshots fail with rediscovery guidance, never a silent refocus
or agent call. Saved catalogs/views remain browseable until rediscovery replaces
the current catalog and removes stale views; no old snapshot history is retained.

Without `--view`, `--extract-only` prepares this selected workflow without agent or renderer;
`--no-render` skips SVG only and still authorizes its agent. The report is the
analysis report described below plus `catalogDirectory` and `entryId`. Focused
limits, cache reuse, explicit retries, DRAFT review, and human acceptance are the
same as direct focused analysis. Static API/class/callable views instead return the
[source-view report](discovery.md#source-views) and use current per-kind view bundles.
Changed view inputs replace the validated bundle, not create hash/version folders.

## Advanced Analysis

On Unix (Linux/macOS), install trusted Python >=3.12, Djinn, and Mermaid CLI
(`mmdc`), and build/install Ideomancer as described in the [README](../README.md).
Configure supported global Djinn model/auth (including supported stored OAuth), then run:

```sh
ideomancer analyze /path/app
```

This is an explicit network opt-in: extraction, agent invocation, proposal
validation, and draft manifest/Mermaid/legend/HTML/SVG generation run automatically.
There is no manual `interpret` step and no automatic acceptance. Approve the adapter,
provider endpoint, credentials, and evidence policy before invoking it; use
`--extract-only` to review local evidence first without an agent or renderer.
[Python analysis](python-analysis.md) describes source selection and redaction limits.
Extraction uses descriptor-anchored no-follow/nonblocking filesystem access;
other operating systems fail explicitly rather than use a weaker fallback.

The default container is `.ideomancer/analyses` relative to the invocation cwd,
not the app root. Parents and a readable application-named current run directory are
created automatically. `--out BASE` selects a container, not an exact run path.
Keep these outputs private and ignored.
Select a different `--out BASE` explicitly to use a separate container; renderer
configuration alone does not create one.

On Unix, container creation walks every path component through anchored directory
descriptors with no-follow checks. Relative paths are rooted at the canonical
invocation cwd. Only the known root-level macOS `/var` and `/tmp` aliases receive
special handling; arbitrary symlink ancestors are rejected.

Syntax: `analyze ROOT [--out BASE] [--python PATH] [--agent PATH] [--model M]
[--mmdc PATH] [--timeout-seconds SECONDS] [--retry] [--extract-only] [--no-render]
[--focus SYMBOL [--focus-hops HOPS] [--focus-facts FACTS]]`.
Both `analyze` and `interpret` default to a 300-second controller wall timeout
and accept 1 through 600 via `--timeout-seconds`. Each new request carries a
slightly smaller `timeoutMs` budget for the agent; see [Djinn setup](#djinn-setup).
`--no-render` saves the draft manifest, Mermaid source, legend, and HTML without
invoking `mmdc`; otherwise `mmdc` is required to produce the SVG. `--extract-only` runs neither
agent nor renderer. Incomplete evidence, including source parse failures, blocks
the agent, reports `status: "extracted"`, `complete: false`, and an error, and exits 1.

Set tool/model preferences once through the environment; CLI flags override them:

```sh
export IDEOMANCER_AGENT=/absolute/path/to/updated/djinn
export IDEOMANCER_PYTHON=python3.12
export IDEOMANCER_MMDC=mmdc
# Optional: export IDEOMANCER_MODEL=your-provider-model
```

Defaults are `djinn`, `python3.12`, `mmdc`, and model `openai/gpt-6.1-sol`.
Model precedence for `analyze` is explicit `--model`, then `IDEOMANCER_MODEL`,
then this built-in default; it sends that selection to the agent.
These `IDEOMANCER_*` settings apply to `analyze` and workflow `diagram` without `--view`;
`discover` honors `IDEOMANCER_PYTHON`, and static `diagram` uses `IDEOMANCER_MMDC`.
JS/TS `discover --lang javascript-typescript` instead honors `IDEOMANCER_NODE`
(default `node`) and mandatory `IDEOMANCER_TYPESCRIPT` (absolute external compiler),
overridden by `--node` / `--typescript`. These tools are not agent configuration.
`present` also honors
`IDEOMANCER_MMDC` (explicit `--mmdc` wins, default `mmdc`). Use explicit flags for
the other lower-level commands. There is no file-based config or automatic selection of an
arbitrary preferred chat CLI. Other agents need the protocol wrapper described below.

The JSON report has `runDirectory`, `status`, `snapshotId`, `proposalSha256`,
`requiresAcceptance`, `complete`, `draftManifest`, `draftMermaid`, `draftSvg`,
`draftLegend`, `draftViewer`, `presentationVersion`, `evidencePacket`,
`coverageReport`, `coverage`, `packetError`, `focus`, and `error`.
`focus` is an options object with `symbol`, `hops`, and `maxFacts`, or null for
the broad overview.
`evidencePacket` and `coverageReport` are paths to the saved packet and coverage;
`coverage` contains the coverage object. Fields are null when unavailable.
`complete` means source extraction completeness, not proposal correctness. A successful unaccepted
draft has `status: "proposal_ready"` and `requiresAcceptance: true`.
`requiresAcceptance` is true only for `proposal_ready` with a proposal fingerprint;
it is false for `extracted`, `failed`, `indeterminate`, and `accepted`.
Agent/render failures normally preserve a report with `error` and exit 1;
early extraction/storage failures or failures loading damaged state can instead
exit 1 with stderr before a report is available.

The run contains `state.json`, `evidence.json`, `modules.json`, and `modules.mmd`.
When packet preparation succeeds, initial inspection also persists hashed
`evidence-packet.json` and `evidence-coverage.json` before any agent invocation,
including for `inspect` and `analyze --extract-only`. If complete source evidence
cannot yield a useful bounded packet, the inventory remains available with
`packetError`. No-focus extraction-only analysis succeeds with that diagnostic; normal
analysis reports `error` and exits 1 before dispatch. Incomplete source evidence
still blocks the agent and exits 1, regardless of packet preparation.
The module files are only a static module/import projection. Open the report's
`draftViewer` before acceptance; it is the preferred readable DRAFT review view.
`draftManifest` points to `presentation/draft.architecture.json`. All draft visual
artifacts share that current bundle; there are no legacy root draft artifacts,
hash-addressed render blobs, or pending-render journals in the current layout.
`ideomancer skill` prints the exact bundled skill whose fingerprint is bound to the run.

### Readable presentations

```sh
ideomancer present RUN --no-render
ideomancer present RUN --mmdc mmdc
```

Syntax: `present RUN [--no-render] [--mmdc PROGRAM]`. This command verifies the
saved proposal and its citation evidence, including sent-packet references. It
uses no Python, live source extraction, agent, model, or authentication. It supports
`proposal_ready` and `accepted`, not `extracted` runs. It returns the same report
shape as `analyze`, never calls `accept`, and does not update workflow state.
`analyze` automatically generates/reuses the current presentation too, but still
extracts source when invoked.

The current internal `VERSION` is `readable-v2`, stored in receipts, not folder names. Under `RUN/presentation/`,
`draftMermaid` points to `diagram.mmd`, `draftSvg` to `diagram.svg` when available,
`draftLegend` to `legend.json`, and `draftViewer` to `index.html`.
`presentationVersion` identifies that version. `--no-render` creates Mermaid,
legend, and HTML even without SVG, reports `draftSvg: null`, and retains a matching
current-version SVG. A version upgrade with `--no-render` publishes a source-only
replacement instead of retaining the older bundle's SVG. The viewer always references `diagram.svg`; its text and legend
remain readable before rendering adds the image.

Open `draftViewer` first. Its offline HTML uses the shared API/class/network/services/
workflow pan/zoom viewer. SVG stays an inert `<img>`, never inline active SVG,
`<object>`, or canvas. Full component/relationship labels, rationales, citations,
evidence details, and model uncertainties remain outside the viewport.

The diagram starts at readable 100% with scrollable overflow, not a tiny automatic
fit. Buttons provide `+`, `-`, Fit, Reset 100%, and Fullscreen when available.
Left mouse drag pans; ordinary wheel and touch use native scrolling, not custom
touch pinch. Ctrl/Meta+wheel zooms at the cursor. Focus the viewport for `+`/`-`,
arrow-key pan, `F` to fit, and `0` to reset. Scale is bounded to 0.1%–800%; viewport
resizing and fullscreen transitions preserve zoom.

Only trusted embedded static JS runs, authorized by its exact SHA-256 CSP hash.
Styles are inline; scripts do not use unsafe-inline or eval. There are no CDNs,
fetches, or viewer network calls, and source/model/spec content never enters JS.
Without SVG or JavaScript, details remain usable and controls stay disabled;
JavaScript-disabled pages retain native original-size SVG scrolling when available.
Fullscreen can be unavailable or denied. The separate user `views/index.html`
navigation page remains **script-free**, linking all verified generated current
views, sources, legends, and available SVGs. See [shared viewer details](discovery.md#offline-diagram-viewer).

Source paths and line ranges
refer to the saved snapshot, not live files or freshness checks. Diagram edge keys
`E001`, `E002`, and so on map to full relationship details. Short diagram labels
are excerpts, not semantic paraphrases. Solid arrows preserve `observed` basis;
dashed arrows preserve `inferred` basis. These describe source support versus
interpretation, not runtime proof; presentation does not reclassify relationships.

Presentation changes leave the full model, proposal, evidence, and acceptance
fingerprints unchanged. Accepted authority remains in the accepted artifacts,
separate from this DRAFT projection of the proposal, even after acceptance.
Builder/style versions are internal identities; a changed presentation replaces
the validated current bundle, rather than preserving versioned folders.
The `readable-v1` → `readable-v2` upgrade first verifies the older bundle, then
regenerates it in the same `presentation/` folder without a model call or changes
to the proposal, evidence, workflow state, or acceptance. This is a presentation
metadata/version upgrade, not a run/proposal identity change. Generated source-view
HTML changes similarly replace the verified current per-kind bundle.
Cached `present` preserves source bytes/mtimes and makes no agent or renderer call.
Changing `--mmdc` does not replace a recorded SVG for the same view.

Publication stages the manifest, Mermaid, legend, HTML, optional SVG, and receipt
as a bundle, then publishes it to the current `presentation/` directory. The
receipt binds exact document hashes to proposal, snapshot, and builder version.
A valid source-only bundle can be replaced by a rendered bundle. There is no
render blob or pending journal and no restoration of missing recorded SVGs from
hidden storage. Tampered, conflicting, or unrecorded artifacts fail validation,
not silent repair. A render failure preserves the proposal and available source
bundle; fix `mmdc` and repeat `present RUN`, without `--retry` or an agent call.
This presentation boundary does not make agent or acceptance updates an all-files
atomic transaction. No run-cleanup/prune CLI exists.

### Focused workflows

```sh
ideomancer analyze /path/app --focus OrderService.submit --focus-hops 3 --focus-facts 600 --no-render
ideomancer analyze /path/app --focus OrderService.submit --extract-only
ideomancer skill --focus
```

Use focus for an operation's responsibilities and collaborator handoffs rather
than broad provider wiring/application composition. Anchor lookup accepts an exact
fully qualified callable symbol, a unique dot-suffix, or a callable fact ID.
Functions and async functions, including methods, are supported; classes are not
anchors. Path/colon and HTTP method/path queries are not supported. Missing,
ambiguous, or non-callable anchors fail locally before dispatch, with candidate
locations/IDs in diagnostics. The full inventory can be persisted with `packetError`;
invalid focused preparation exits 1 even with `--extract-only`.

`--focus-hops` is 0 through 5 (default 3), and `--focus-facts` is 1 through 2000
(default 600, including context). Both require `--focus`; invalid CLI limits or
limits without focus exit 2. Depth 0 includes anchor body facts/context without
callee expansion. Breadth-first expansion follows only unambiguous local static
candidates: helpers, `self` methods, relative/aliased imports, typed constructor
field bindings, supported signature-validated inherited constructor forwarding,
nested lexical `self` handlers, and locally constructed receivers. Direct typed-parameter receiver
calls remain unresolved. These are not runtime dispatch guarantees, caller
discovery, complete inheritance/MRO, or framework DI inference.
Lexical name shadowing and receiver context are conservative: constructor-field
context must belong to the same class, not a nested or repeated declaration.
Ambiguous same-line assignments to `self` or its methods remain unresolved rather
than falling back to the original local method. Assignment support does not cover
`AugAssign`, `NamedExpr`, or `del`; complete control-flow analysis and registry
resolution are unsupported. The extractor's `parameter` and `binding_invalidation`
facts, along with `signature`, `api_route`, `entrypoint`, and direct router-context
metadata, are support-only in global bounded
selection, never standalone anchors.
They do not change public packet types or mutate analyzed application source.

Policy `architecture-focus.v1` saves the focused packet and coverage in the same
`evidence-packet.json` and `evidence-coverage.json` artifacts. Input `focus` records
`policyVersion`, `requestedSymbol`, resolved `anchor` location/ID, `maxHops`,
`maxFacts`, `visitedSymbols`, `links`, `unresolved`, `truncated`, and `boundaries`.
Every retained link's source, target, and supporting evidence IDs occur in selected
facts. The 192 KiB compact-JSON limit includes focus/selection metadata; context
counts toward the fact cap. Coverage compares full inventory totals against the
selected focused scope, not confidence or whole-program workflow completeness.
Review missing bodies and unresolved/truncated boundaries as uncertainties.

Focus uses the same agent/model, timeout, render/extract-only, explicit retry, and
human acceptance controls. `skill --focus` prints the exact global instructions
plus focused guidance. The proposal schema remains `ideomancer.architecture.v1`:
no typed relationship-kind or extra proposal fields. Labels should explain
supported flow/handoffs, not claim execution order, branches, atomicity, or rollback.
No automatic live-model retry or quality guarantee is implied.

Draft rendering has a 60-second deadline, an 8 MiB SVG limit, and 64 KiB limits
for each captured stdout/stderr log. On Unix, cleanup sends TERM to the renderer's
owned process group, gives graceful shutdown up to one second, then escalates to
KILL and reaps the main child. These controls are not a sandbox for malicious
renderers or helpers that detach into another process group/session. SVG output
is not sanitized; use a trusted renderer and browser/network policy.

Full evidence remains in local `evidence.json`. Without focus, small inputs are supplied unchanged;
larger inputs use a deterministic subset bounded to 192 KiB of compact JSON,
using `architecture-packet.v2-relations`, not declaration-first truncation. The
limit is unchanged and measured in actual serialized compact UTF-8 JSON bytes,
including escaping and selection metadata. Selection interleaves packages, files,
and lexical scopes, seeds available relationship fact kinds when they fit, and
allocates anchor-byte shares of 20% declarations, 15% imports, 35% wiring facts,
and 30% calls. New context is charged to its anchor's category, shared context is
charged once, and unused shares are redistributed with relationships first.
These are not guaranteed percentages of final fact kinds or counts.

Each anchor can bring a bounded lexical neighborhood of at most 10 original facts,
such as an enclosing declaration, receiver binding, constructor annotation, and
representative call. Smaller owner/anchor or anchor-only fallbacks are used when
the context cannot fit. Every selected fact retains its original hash-based ID
and contents; selection invents no relationship edge or runtime resolution.
Subsets add `selection` with `policyVersion`, `totalFacts`, `includedFacts`,
`omittedFacts`, `maxBytes`, `byKind`, `warnings`, and a coverage `note`.

Coverage is computed for both full and selected packets. The separate report has
`policyVersion`, `selectionApplied`, `inputBytes`, `maxBytes`, fact totals,
`totalFiles`, `includedFiles`, per-kind total/included/omitted counts in `byKind`,
and `warnings`. Counts and warnings describe coverage, not confidence. Inspect
the saved packet and immutable attempt request to see what the agent received.
`interpret` verifies and loads the stored packet rather than regenerating it.
Scan `complete` does not imply exhaustive coverage or resolved graph relationships.
This byte bound excludes request/schema/message overhead and does not guarantee
a fit within a model's token/context window. Oversized metadata or facts that
leave no useful packet fail preparation before dispatch; omitted facts can hide
relationships that need local review.

Review `proposal.json`: component groupings, every relationship's evidence and
`observed`/`inferred` basis, rationales, and uncertainties. Verify against the source
locally as needed. Substitute the exact reported fingerprint for `HASH`:

```sh
ideomancer accept RUN --proposal-sha256 HASH
```

Use the exact `runDirectory` and `proposalSha256` from the report. First acceptance
checks source freshness and writes the `0.1.0` graph `architecture.json` and
deterministic `diagram.mmd` from that exact proposal, without another model call.
The draft files remain a visibly DRAFT proposal view even after acceptance; they
are not promoted in place. To render a separate accepted view, use
`ideomancer render RUN/architecture.json --out /path/to/accepted.svg` with `mmdc`.
Keep this optional output outside a managed current run's generated inventory.
Evidence, rationales, and uncertainties remain in the sidecars rather than the
compact graph schema.
Do not edit fingerprinted run artifacts in place; there is no proposal-edit command.

## Reuse and recovery

Repeating `analyze` with the same full evidence and configuration reuses the saved
proposal and recorded SVG without another agent/render call. A fresh agent call
can be nondeterministic; cached repeats are deterministic reuse, not a claim that
independent model calls produce identical proposals. Source root/snapshot, the
full evidence fingerprint (including diagnostics and completeness), interpreter
selection, bundled skill, agent path, and requested model form the run key.
Extractor version is part of evidence identity, and packet policy is also in the
analysis key. Identity changes replace the validated current run, rather than
creating another subdirectory or preserving old proposals. Focused state includes
its options; combined skill and `architecture-focus.v1` policy are conditional on
focus. Different anchors, hop limits, or fact caps replace previous current views
and accepted outputs and reset acceptance. `python-static-8-generic-facts` changes
extractor/snapshot/evidence identity even for unchanged source. Repairing an
incomplete scan also replaces the current run even when `snapshotId` is unchanged.
Replacement requires validation of the existing run; failed, running, or
indeterminate agent work cannot be discarded via changed source/configuration.
Reconcile it and explicitly retry the saved run first. Internal hashes and versions
remain receipt/identity metadata, never history folder names.
Selections are compared, not executable bytes or all environment/credential
settings. Deadline budgets are excluded from the run/proposal cache key: changing
the timeout reuses a matching saved proposal without an agent call or budget
consumption. Renderer configuration is excluded: changing `--mmdc` does not invalidate
an existing SVG for the same proposal. After acceptance, a cached repeat reports
`status: "accepted"` and `requiresAcceptance: false`, retaining the DRAFT view.
If SVG rendering was previously skipped, a repeat without `--no-render` can add it.

A render failure keeps the validated proposal and draft sources. Fix/install
`mmdc` and use `present RUN` for saved-artifact recovery without Python or a source
scan, or repeat `analyze` with the same inputs to use the cached agent result
without `--retry`. `--retry` authorizes another agent attempt only after failed,
indeterminate, or interrupted `running` work is reconciled; it is not a render
retry or a force-refresh switch for successful proposals. Conflicting draft files
are refused rather than overwritten. Inspect errors before moving aside damaged
or unrecorded artifacts; do not edit fingerprinted files in place.

### Lower-level commands

`inspect` and `interpret` remain available for debugging, recovery, or a deliberate
local evidence review before provider access, not as required steps for `analyze`:

```sh
mkdir -p .ideomancer
ideomancer inspect /path/app --out .ideomancer/example-run --python python3.12
ideomancer status .ideomancer/example-run
ideomancer interpret .ideomancer/example-run --agent djinn --timeout-seconds 300
```

`inspect` never invokes an agent/provider or renderer. Unlike `analyze --out`, its
`--out` is the exact run directory and its parent must already exist. Syntax for
interpretation is `interpret RUN [--agent PATH] [--model M] [--retry]
[--timeout-seconds SECONDS]`. Its JSON output is workflow state, not the analyze
report, and it does not generate draft previews automatically.
For a run prepared with `analyze --focus ... --extract-only`, `interpret RUN`
automatically uses the persisted focus settings, hash-verified packet, and combined
skill. There is no focus flag on `interpret`, and `inspect` remains an overview scan.

`inspect` with the same output, snapshot, and recorded configuration verifies and
reuses artifacts without rewriting them. Changed source, interpreter selection,
evidence, or bundled skill refuses an existing run; choose a new `--out`. Do not
delete reviewed decisions merely to rerun a changed configuration. Manual
`inspect --out RUN` remains no-clobber, unlike managed `analyze` current-slot replacement.

There is no automatic migration of legacy hash-named storage or old presentation
layouts; the verified `readable-v1` bundle upgrade within `presentation/` is separate.
Stop old processes, inspect/reconcile prior work, then explicitly clear
legacy output or select a new `--out`. Do not blindly discard uncertain provider
work. Successful current replacement retains no old hash folders or snapshot history.

`interpret` on `proposal_ready` or `accepted` reuses the saved proposal without a
provider call when agent path and requested model match. Changed agent/model is
rejected in these states. Reuse compares recorded selections, not executable bytes
or all credential/environment settings.

`status RUN` verifies saved artifact fingerprints and reports JSON state. It does
not rescan live source. Before first acceptance, `accept` re-extracts source and
requires complete evidence matching the saved snapshot. Repeating acceptance of an
already accepted run verifies/reuses its saved artifacts without a live rescan.
Accepted output describes that snapshot, not a claim that today's source is unchanged.

State fields `packetPolicyVersion`, `packetSha256`, and `coverageSha256` are optional
when loading legacy runs. `status` can still read old stored proposals. Current
`interpret` refuses missing/old packet policies before dispatch and asks for a new
run; changed bundled skills likewise require a new run. First acceptance of an
old-extractor proposal fails with guidance to generate and review a new run using
the updated tool: current extraction cannot check freshness under old semantics.
Already accepted runs retain their original saved-artifact reuse without a rescan.

Acceptance parses the exact proposal bytes covered by the reviewed fingerprint.
Before publishing, it preflights both output destinations: conflicting or unsafe
files are refused, and identical outputs left by interrupted acceptance are reused.
Each output is staged and published without clobbering, then state is updated.
This supports resuming acceptance with the same hash, not an all-files atomic
transaction; first acceptance still requires the source freshness check.

Failed, `indeterminate`, or interrupted `running` work requires reconciliation and
explicit `--retry` before another call. A controller timeout kills the local child
but remote completion can be unknown. Request IDs and caching do not provide
exactly-once execution or billing guarantees. Commands lock a slot using a sibling
`.APP.lock` (or the manual run's directory name), not a legacy `.analyze.lock`.
After a crash, verify the owning process is stopped before removing its lock.
Do not implicitly retry uncertain remote work.

An existing uncertain run is not retried or rewritten merely because the deadline
changes. Only after reconciling failed or indeterminate work, deliberately authorize
a new call (which may incur cost), for example:

```sh
ideomancer analyze /path/app --retry --timeout-seconds 600
```

No deadline is automatically increased and no model retry is automatic. A longer
deadline does not guarantee success, reduce context, or establish that model
latency was the only cause. A timeout report has `requiresAcceptance: false`.

Manual initial inspection stages its artifact set, including packet and coverage when
available, in a sibling `.ideomancer-init-*`
directory before publishing the complete inspection with an atomic no-replace
rename. Managed analysis also stages replacements before exchanging the current
directory and removing the previous contents. An interrupted initialization can leave abandoned staging directories,
but does not poison the deterministic final run path. Recovery never automatically
deletes those directories. This initial-publication boundary does not make later
agent, draft, or acceptance updates an all-files atomic transaction.

Each dispatched attempt uses an immutable `attempt-NNNN.request.json`, passed
directly to the adapter by absolute path. `state.json` records `requestFile`,
`requestId`, `requestSha256`, and `attempts` for the latest allocation, alongside
agent/model/error fields and optional `agentTimeoutMs` and `elapsedMs`. Old state
without these timing fields remains readable. `request.json` is a replaceable
convenience copy only.
New allocations skip slots containing either a request or result, preserving
orphaned artifacts after interruption; numbering need not be contiguous.
`attempt-NNNN.result.json` receipts retain request ID and outcome, plus proposal
hash/model on success or error on failure, with `timeoutMs` and `elapsedMs` for new
attempts. An explicit retry allocates a new immutable request with its selected
`timeoutMs`, preserving prior attempts and receipts. A crash can leave a request without a
receipt. `status` verifies the current state-referenced request, not a comprehensive
audit of every historical receipt; retained attempts are not proof of remote execution.

## Djinn setup

`djinn execute` is a standalone, tool-free boundary, not its interactive UI or a
stored session. For a sibling development checkout, build from the Djinn repository:

```sh
cargo build -p djinn-cli --bin djinn
```

Then, from the Ideomancer repository:

```sh
export IDEOMANCER_AGENT="$PWD/../djinn/target/debug/djinn"
ideomancer diagram SAVEDDIR --entry WORKFLOW_ID --no-render
```

An optional `cargo build --release -p djinn-cli --bin djinn` in Djinn produces
`../djinn/target/release/djinn` instead. No UI build is needed.

Rebuild/update the executable actually selected by `--agent`, `IDEOMANCER_AGENT`,
or `PATH`; an existing configured binary does not gain support just because the
Djinn checkout changed. If an installed binary on `PATH` is stale, explicitly
select the updated executable with `--agent` or `IDEOMANCER_AGENT`; no automatic
executable/provider fallback or global preference change is implied.
For an existing authentication-failed analysis, reconcile
the failure and rerun the same command with `--retry`, for example:

```sh
export IDEOMANCER_AGENT=/absolute/path/to/updated/djinn
ideomancer analyze /path/app --retry
```

Valid supported stored auth needs no extra API key. This is headless reuse of
normal configured auth, not automatic login. No browser, UI, sessions, tools,
workspace config, or repository instructions are started or loaded. Only global
model/auth fields are read from normal trusted `HOME`/XDG locations, not all
interactive settings. Global native config and OpenCode config permit stow-style
symlinks (on Unix, final symlink and target must belong to the current user);
OpenCode JSONC comments/trailing commas are supported. Model-relevant global fields
are merged from `config.json`, `.opencode.json`, `.opencode.jsonc`, `opencode.json`,
then `opencode.jsonc`; later values override earlier ones, including JSONC over
JSON. The selected OpenCode config root is `XDG_CONFIG_HOME` when set, otherwise
`HOME/.config`; the XDG root does not inherit the HOME root. Credential stores reject
symlinks. Native provider endpoints and other non-model/auth settings are ignored.

Model selection is the requested model (`analyze --model` or `IDEOMANCER_MODEL`),
including `analyze`'s built-in `openai/gpt-6.1-sol` when neither is set,
then the global native default agent/profile model, then `DJINN_AGENT_MODEL`,
`DJINN_COPILOT_MODEL`, `DJINN_OPENAI_MODEL`, then the global OpenCode selected-agent/
default model (first available nonblank). API-key auth alone can fall back to
`gpt-4o-mini`. OAuth without a selected model fails with guidance to configure a
Codex-compatible model, set `DJINN_AGENT_MODEL`, or pass `--model`.

After selection, exact global OpenCode `provider.models.<key>.id` aliases are
resolved once across the merged JSON/JSONC maps, even for explicit/native/env
selections. Unprefixed names use the `openai` map. Provider-prefix dispatch stays
unchanged; aliases do not chain or import provider options, endpoints, or secrets.
There is no dynamic model discovery or automatic fallback. If a configured model
is unsupported, choose an explicitly compatible `--model` or `IDEOMANCER_MODEL`,
not an API-key workaround. Changing the requested model selects a separate run.

Supported model routes are `openai/*` and unprefixed OpenAI names, plus `copilot/*`
and `github-copilot/*`. Other provider prefixes are rejected, even if configured
for interactive Djinn. OpenAI credential precedence is `OPENAI_API_KEY`, global
native `providers.openai.auth`, Djinn auth store, inline `OPENCODE_AUTH_CONTENT`,
then OpenCode auth store. Native auth accepts a literal API key or `env:NAME`;
`auto` continues discovery. Unset/empty named variables error; legacy `opencode:`
specifications require migration to `env:`.

Current stored/inline OpenAI OAuth works, including active external OpenCode auth.
Only expired Djinn-owned OAuth can auto-refresh. Expired external OpenCode auth
is refused: reauthenticate with `djinn auth login` or refresh via the owning
harness. External credentials are never rotated or copied by Djinn. Expired
inline OAuth cannot refresh; reauthenticate separately.

Native Rust headless and interactive auth refresh/login writers share a store
lock. Bounded refresh atomically updates the private Djinn store with owner-only
permissions, preserving other providers and refusing a concurrently replaced
OpenAI credential. Before dispatch it durably stores a SHA-256 refresh-token hash
in `_djinn_openai_refresh_inflight_sha256`. Cancellation or provider error leaves
this marker to block resending a possibly consumed token. The marker is removed
only after durable credential commit or a separate explicit login committing new
credentials. Reauthenticate or reconcile the uncertain refresh, not blindly delete
markers/locks. Even failed calls can modify the native auth store with a marker
and leave an owner-only lockfile; global access is not entirely read-only. No app
config or session state is written.

Copilot tries direct `DJINN_COPILOT_TOKEN`, `GITHUB_COPILOT_TOKEN`, `COPILOT_TOKEN`
in order. Otherwise it exchanges OAuth from `DJINN_COPILOT_OAUTH_TOKEN`, then
`GITHUB_COPILOT_OAUTH_TOKEN`, Djinn auth store, inline `OPENCODE_AUTH_CONTENT`,
OpenCode auth store, local `github-copilot/hosts.json` / `apps.json`, then
noninteractive `gh auth token`. Stored/inline `github-copilot` (preferred) or
`copilot` entries support `type: "api"` with `key`, or `type: "oauth"` with
`refresh` holding the GitHub OAuth token for exchange. `expires: 0` is supported
for Copilot OAuth; OpenAI expiry rules do not apply. No automatic login, provider
retry, or backend fallback occurs.

Trusted operator URL overrides receive the selected credentials:
`DJINN_CODEX_API_ENDPOINT`, `DJINN_OPENAI_OAUTH_TOKEN_URL`,
`GITHUB_COPILOT_TOKEN_URL`, `GITHUB_COPILOT_CHAT_COMPLETIONS_URL`, and
`OPENAI_BASE_URL`. Approve endpoints as well as credentials. Redirects are disabled.
API-key OpenAI and Copilot use strict chat completions. OpenAI OAuth uses Codex
Responses with bounded JSON/SSE parsing, requiring a completed terminal response,
one completed assistant message, and actual model identity. Incremental SSE parsing
returns at a valid completed terminal event without waiting for remote EOF. Deltas
or `[DONE]` alone are not completion. Benign `response.metadata` is supported, not
treated as completion or a safety bypass. Explicitly empty terminal `output` can
use independently validated nonempty single-assistant stream text only with an
explicit `response.completed`, completed status, and actual model identity. Bounds,
preceding tools/refusals, multiple outputs, conflicting done text, incomplete
output, and unknown/unsupported events remain checked and rejected as appropriate.
Errors retain the route and selected model. Bounded HTTP-error JSON exposes only
validated `error.code`/`type`/`param` identifiers and static reasons such as
`unsupported_model`, `invalid_model`, or `unsupported_parameter`, not freeform
provider messages/bodies, secrets, or raw model output.

Djinn reports the actual provider model separately from the requested selection.
Updated `djinn execute` accepts optional request `timeoutMs`, an integer from 100
through 600000. Deadline precedence is explicit Djinn `--timeout-seconds` (1 through
600), then request `timeoutMs`, then the standalone 120-second default. Ideomancer
passes its caller-selected budget in the request, not as a Djinn CLI flag.

For a controller wall limit of `wallMs = seconds * 1000`, Ideomancer sends
`timeoutMs = wallMs - min(1000, wallMs / 10)`: 299000 ms at the default 300 seconds,
599000 ms at 600 seconds, and 900 ms at 1 second (a 100 ms margin). The margin lets
Djinn return a failure envelope before the controller's outer wall timeout, but is
not a guarantee. The controller still hard-kills the local child at its outer limit;
that does not guarantee remote cancellation or prevent remote completion/cost.

Auth/model resolution and the model call share this selected total budget, rather
than a hardcoded 120 seconds; the model receives only the remaining time. Auth
subcaps are unchanged: HTTP requests have a 20-second timeout and 5-second connect
timeout; noninteractive `gh` is bounded to 5 seconds. Pre-model-dispatch auth timeout
is `configuration`, not `indeterminate`; model transport/deadline failures are
`indeterminate` and must not be retried automatically.

Legacy builds could hit Djinn's inner 120-second deadline even with an outer
180-second controller limit. Update both binaries, including the actual executables
selected by `PATH` or agent configuration: legacy Djinn rejects `timeoutMs` because
unknown request fields are rejected. There is no fallback that drops the field or
retries the call. Updating does not reconcile or retry an existing uncertain run.
Tests use synthetic adapters, not production credentials.

Optional ignored bridge tests run actual binaries against loopback fake
chat-completions and Codex providers, with synthetic credentials and no external calls:

```sh
IDEOMANCER_DJINN_BIN=/absolute/path/to/built/djinn cargo test --locked --test djinn_bridge -- --ignored
```

They cover the lower-level workflow, `analyze --no-render`, stored OAuth/Codex SSE,
and focused analysis, with drafts remaining unaccepted until explicit acceptance.
The focused fixture verifies request input against the saved focused packet and
instructions against the exact combined `skill --focus` output; a cached repeat
makes no extra provider call. No automatic live-provider loop is involved.
Build Djinn first; the tests do not build it. They require trusted Python >=3.12
(`IDEOMANCER_TEST_PYTHON` can select it). This checks the executable/protocol bridge,
not production authentication or live-model architecture quality.

An optional draft SVG test uses synthetic source and a fake agent with real
Mermaid CLI and its browser runtime, checking visible DRAFT output and title
escaping without a live provider:

```sh
cargo test --locked --test analyze -- --ignored
```

An optional offline viewer regression uses an existing absolute Puppeteer module
and its already-installed Chromium, with no downloads or provider access:

```sh
IDEOMANCER_PUPPETEER_MODULE=/absolute/existing/puppeteer cargo test offline_chromium_regression -- --ignored
IDEOMANCER_PUPPETEER_MODULE=/absolute/existing/puppeteer node tests/viewer_browser.cjs --smoke /path/to/generated/index.html
```

`tests/viewer_browser.cjs` covers controls, cursor anchoring, native-size scrolling,
keyboard focus, resize, capability-dependent fullscreen, missing images,
JavaScript-disabled fallback, CSP, and hostile content. Smoke mode targets generated
diagram pages, not the script-free navigation index.

## Adapter contract

`--agent` selects a trusted executable implementing this protocol, not an arbitrary
chat CLI. Other agents need a wrapper that implements:

```text
AGENT execute --request ABSOLUTE_REQUEST_FILE --json
```

Ideomancer invokes it directly without a shell, with closed stdin and a neutral
temporary cwd, not the source root. Agent credentials/environment are inherited.
The adapter must enforce no source-file/tool access; neither a neutral cwd nor
skill instructions sandbox an arbitrary executable. Djinn's structured boundary
does not register tools or discover repository/session context.

The UTF-8 JSON request uses `protocolVersion: "agent-execution.v1"`, `requestId`,
`instructions` (the bundled skill), `input` (`ideomancer.evidence.v1` evidence,
possibly a selected subset with `selection` metadata),
`responseSchema` (draft-07), optional nonblank `model`, and optional integer
`timeoutMs` (100 through 600000). Ideomancer includes `timeoutMs` on each new attempt
as described above. Treat input as untrusted
data, not instructions. Only supplied evidence should inform the proposal.

Success must exit zero and emit exactly one strict JSON object on stdout:

```json
{"protocolVersion":"agent-execution.v1","requestId":"same-as-request","status":"completed","output":{},"model":"actual-provider-model"}
```

Here `output` is a placeholder for a schema-valid proposal, not an empty object:
`schemaVersion: "ideomancer.architecture.v1"`, matching `snapshotId`, `name`,
`components`, `relationships`, and `uncertainties`. Components require `id`, `label`,
`kind`, `evidenceIds`, and `rationale`; relationships require `from`, `to`, `label`,
`evidenceIds`, `basis` (`observed` or `inferred`), and `rationale`. Unknown fields,
invalid IDs/endpoints, and missing/duplicate/nonexistent evidence references are
rejected. Bounds include 1-40 components, at most 160 relationships and 128
uncertainties. The request schema supplies the detailed constraints.
References must occur in the actual delivered packet, not merely full
`evidence.json`; cached proposal reuse, draft generation, and acceptance also
validate against the sent input. The version-2 skill asks labels/rationales to
distinguish wiring, construction, injection, delegation, composition, and mere
association. The proposal schema is unchanged: there is no typed relationship-kind
field or extra proposal fields. `analyze --focus` changes input scope and guidance,
not the output schema. Assignment bindings are not DI proof, and imports
are not runtime calls.

No Markdown fences, progress text, or tool calls belong on stdout. Failure envelopes
use `protocolVersion`, matching `requestId`, `status: "failed"`, a safe `error`,
and optional `errorKind`. Ideomancer preserves a bounded, control-character-filtered
error from a valid matching envelope; `errorKind: "indeterminate"` propagates the
uncertain outcome rather than downgrading it to an ordinary failure. Adapters must
not put credentials, source, raw provider bodies, or other sensitive text in errors;
filtering is not secret redaction. A nonzero exit without a valid failure envelope
produces a generic failure. No failure envelope is treated as a proposal.
Requests are bounded to 16 MiB;
the controller bounds stdout to 8 MiB and stderr to 64 KiB. Provider restrictions
and limits may be tighter. No automatic repair or retry is implied.

No raw source is sent by default, but evidence and all catalog/view/run outputs
still reveal potentially proprietary paths, symbols, route fragments, and structure.
Extractor `python-static-8-generic-facts` retains direct empty or slash-prefixed
route-path literals of at most 2048 characters, recognized HTTP methods, and
bounded router prefixes/include prefixes as
route-context identifier metadata. This is an explicit privacy boundary, not a
secret filter. Payload, credential, configuration, description, and legacy-service
literals remain redacted; dynamic paths/methods stay unknown with diagnostics.
Route/context metadata is not proof of full URLs or runtime activation.
The catalog stays local unless a workflow agent is explicitly selected; that
focused packet can include its callable's route metadata. Explicit broad
`analyze`/`interpret` likewise authorize evidence transfer under their packet policy.
Review the packet and approve adapter/provider/credentials before any agent path;
keep artifacts private and ignored. See [parsing and privacy](python-analysis.md#parsing-and-privacy).

## Related files

- [JavaScript/TypeScript analysis](javascript-typescript-analysis.md): local-only
  discovery, evidence v2, trusted tooling, and rejected agent paths.
- [JavaScript worker](../examples/javascript-worker/README.md): public local-view example.
- `src/main.rs`, `src/discovery.rs`: CLI dispatch and local endpoint selection.
- `src/openapi.rs`, `src/render/endpoint.rs`: contract declarations and candidate projections.
- `tests/callable_views.rs`: native callable selection, contract isolation, and bounds.
- [Python worker](../examples/python-worker/README.md), `examples/python-worker/worker.py`:
  standard-library-only local diagram example.
- `src/router_context.rs`, `src/adapters/fastapi.rs`: generic context interface and direct FastAPI adapter; no agent access.
- `tests/converter_matching.rs` and guard tests in `src/analysis.rs`,
  `src/router_context.rs`, `src/render/endpoint.rs`: offline matching, redaction,
  alias/callee preservation, legacy eligibility, and complete-proof budget regressions.
- `src/analyze.rs`, `src/workflow.rs`: current-run identity, replacement guards, attempts, acceptance.
- `src/presentation.rs`, `src/render/presentation.rs`: staged current bundle and readable documents.
- `src/render/viewer.{rs,js,css}`, `tests/viewer_browser.cjs`: shared offline viewer and browser checks.
- [Discovery](discovery.md), [Python analysis](python-analysis.md): source boundaries and local usage.
