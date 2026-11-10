# JavaScript / TypeScript Source Analysis

## Overview

The JS/TS slice provides **offline source navigation**: local discovery, a complete
saved-catalog grouped UML class bundle, and bounded native API, focused class, and callable
services/network views from saved evidence. Its evaluated
boundary is syntax-proven same-file calls plus selected relative named-function
call candidates and named-class UML relations, not runtime behavior, complete framework coverage, a type checker,
or a whole-program graph. A network view is available but proves no JS network calls.
Python remains the default language; its framework-aware scope is FastAPI-only,
while basic Python callable support and the existing Python agent path remain intact.
This does not promise Django or Flask support.

The public [JavaScript worker](../examples/javascript-worker/README.md) is the
distributable example. Runtime activation, DI, network proof,
and JS/TS agent interpretation remain deferred; Python/FastAPI behavior is unchanged.

The native parser is an implemented local candidate, still uncommitted/unpublished.
The full local normal suite and applicable optional rendering/browser checks pass
on Rust 1.89. Hosted supported-platform validation remains a separate release gate.

## API / Interfaces

```text
discover ROOT --lang javascript-typescript [--out BASE]
catalog SAVEDDIR [--kind api|workflow|class] [--query TEXT] [--path PREFIX]
diagram SAVEDDIR --kind api|class [--no-render]
diagram SAVEDDIR --entry ID --view services|network [--no-render]
        [--focus-hops 0..5] [--focus-facts 1..2000] [--openapi FILE]
```

- `--language` aliases `--lang`; `js`, `javascript`, `ts`, and `typescript` all
  select `javascript-typescript`. Omitting the language still selects Python.
- No Node runtime or TypeScript compiler is needed for discovery. Tree-sitter
  **0.25.10**, JavaScript grammar **0.25.0**, and TypeScript/TSX grammar **0.23.2**
  are exactly pinned native compiled C dependencies with Rust bindings. Building
  requires a native C compiler, not npm or target dependencies.
- The language ABI bridge `tree-sitter-language` is directly pinned to **0.1.5**
  alongside the declared Rust **1.89** MSRV, protecting downstream dependency
  resolutions rather than relying only on this repository's lockfile.
- Hidden obsolete `--node` / `--typescript` and `IDEOMANCER_NODE` /
  `IDEOMANCER_TYPESCRIPT` are rejected with an actionable error for JS/TS discovery,
  not ignored. Remove the flags and unset both variables.
- Engine callers use `Toolchain::NativeJavaScriptTypeScript {}` with strict wire
  object `{"language":"javascript_typescript_native"}`. The legacy Node/compiler
  variant remains readable but discovery returns `invalid_request` before source
  access. See the [engine contract](engine-api.md).
- `diagram SAVEDDIR --kind class` with no entry/query/path or explicit limit/offset
  selects grouped actual UML at `views/class`. Any such selector, including
  `--limit 12` or `--offset 0`, selects the independent, paginated `views/class-focus`
  instead (default 12 entries, maximum 30). Focus never auto-generates full UML or
  an inventory. Catalog browsing/paging is unchanged; see [class UML interfaces](discovery.md#class-uml).

Discovery never loads target `node_modules`, tsconfig, plugins, package scripts,
or imported modules. It does not install tooling or infer the language from target
configuration. Parsing uses Tree-sitter syntax trees only: no type checker,
project construction, compiler module resolver, or source side effects. The
selected-evidence resolver below does not use TypeScript project resolution.

Catalog browsing and native API/class/callable projections work from saved artifacts
without an extraction runtime, compiler, Python, agent, model, or authentication.
With `--no-render`, no Node/browser toolchain is needed. SVG rendering uses a
separately trusted `mmdc`, which normally has its own Node/Chromium requirements.
JS/TS workflow analysis **without `--view`**, including `--extract-only`, is rejected
before Python or model invocation. Use `--view services|network`, not a Python fallback.
`--extract-only` is invalid with an explicit view.

## Data Flow

1. Rust selects, opens, and hashes source bytes through the shared Unix
   descriptor-anchored no-follow scanner. Native parsing receives these bounded
   bytes; it does not reopen target files or load target configuration.
2. The compiled-in JavaScript, TypeScript, or TSX grammar parses the selected file.
   No Node process, compiler artifact, target imports, macros, or source code runs.
3. Tree-sitter byte offsets are converted to the existing UTF-16 code-unit offsets
   used by JS proof receipts. Astral Unicode characters count as two UTF-16 units;
   treating byte offsets as receipt offsets would invalidate guards and ordering.
4. Native syntax traversal yields declarations, call shapes, guarded import/export bindings,
   module dependency summaries, members, source references, and diagnostics. Saved
   evidence and catalog receipts bind artifacts; later views verify saved data,
   not live source freshness.

Evidence keeps the outer schema `ideomancer.evidence.v2`, with extractor
`javascript-typescript-tree-sitter-1` and a `language` metadata object.
Legacy static-1 through static-6 remain accepted for supported generic views.
They are not rewritten or upgraded to new fact capabilities, including static-6
generic factory claims. Rediscover explicitly for native facts; the new extractor
identity invalidates old snapshot-bound claims and derived-output reuse.
Static-2 and newer support module navigation; static-4/5/6 support class relations.
Static-5/6 resolve selected relative
named-class targets. Static-4 local relations still render; static-1/2/3 render observed
members and declared-base boundaries without new relation facts. Rediscover for class-relation
or generic factory facts (or module proofs on static-1). Unknown extractor versions
or inconsistent language/schema metadata are rejected.

| Field | Meaning |
| --- | --- |
| `name` | `javascript-typescript` |
| `parser` / `parserVersion` | `tree-sitter` and `tree-sitter-0.25.10;javascript-0.25.0;typescript-0.23.2` |
| `parserSha256` | Hash binding exact runtime/grammar versions, grammar node-type JSON and all native extractor implementation source |
| `runtime` / `runtimeVersion` | `native` and the Ideomancer package version, not an installed Node version |
| `scanPolicy` | Fixed versioned JS/TS scope policy |

`pythonVersion` is absent, never repurposed for JS/TS. Snapshot identity includes
the native parser policy, extractor, parser/runtime metadata, scope policy, and
selected paths/content hashes. Dirty and untracked selected files count. Legacy
Python evidence v1 with `language: None` still serializes without a language field,
and old Python snapshots retain validation under their existing identity rules.
The native fingerprint includes JavaScript `NODE_TYPES`, TypeScript
`TYPESCRIPT_NODE_TYPES`, TSX `TSX_NODE_TYPES`, and the full text of both
`src/javascript_native.rs` and `src/javascript_native/optional.rs`, alongside exact
runtime/grammar versions and native policy identity. It is not a hash of a local
compiler installation or a claim to authenticate the target source.

The parser migration preserves the fact/proof/output contracts: generic facts,
binding/scope guards, atomic module/class proof, redaction, saved view receipts,
and adapter ownership remain in force. It is not permission to infer more runtime
behavior from a more permissive syntax tree. Tree-sitter `ERROR` or missing nodes,
including failures in files containing only module syntax, mark extraction incomplete;
recovered syntax is not complete evidence or permission to promote module links.

### Declaration and call candidates

Facts retain classes, named functions, async functions, direct variable-bound
arrows/functions, methods, and TypeScript member visibility. Constructors and
private/protected members are not implicit workflow candidates. Interfaces and
bodyless overload signatures do not invent callable bodies. Class boxes show
declarations and bounded members, not runtime object relationships.

#### Grouped class UML

Full and selected class bundles use `ideomancer.class-uml.v1`. Each selected class
has exactly one home group, with at most 12 home classes per diagram. Packing is
boundary/context-aware (24 boundaries, 8 context classes), preserving small file
sets where possible; titles use source folders and stable class ranges, not runtime
architecture. Methods/fields have 12/8 image caps; all observed members and modeled
relationships remain in secondary class details and JSON. Caps never omit home classes.
Wide inheritance families use a left-to-right base/descendant layout instead of
placing all long constructor signatures in one horizontal row; each group records
its presentation-only `direction`.

Static-5 emits `js_class_relation` receipts under policy
`javascript-class-relations-2`, with class/owner IDs, relationship, expression,
binding kind/guard, target, optional member/type, supporting facts, stable-binding
status, an independent `referenceFactId`, ordered `lookupScopeIds`, and explicit
`importFactId` (null for non-import relations). Static-4 retains policy
`javascript-class-relations-1` for local relations.
`js_class_reference` anchors the observation's location and scope;
`js_class_scope` records the uninterrupted parent graph, including scopes without
bindings. Promotion reconstructs that graph and validates the nearest guard, not
merely a matching outer name or a caller-supplied truncated chain. All three fact
kinds, plus optional `js_constructor_field` and `js_callable_signature` receipts,
share a separate 4 MiB / 12000-fact class budget with per-file rollback;
overrun reports `javascript_class_relations_omitted` rather than erasing baseline
facts or promoting partial proofs.
Direct same-file local-class inheritance can resolve; static-5 also supports the
[named-class module policy](#selected-relative-named-class-relations). A factory-parameter base
remains a scoped unresolved boundary, never a link to a provider guessed from its
filename. Constructor assignments of parameters to fields and TS parameter
properties are captured, along with supported captured factory binding uses and `new`.

Storage-only argument observations project as fields and constructor parameters by
default, rather than invented class-dependency boxes. An identified class, supported
nonprimitive named declared type, or source evidence of receiver use, creation, or
factory-binding use can retain a dependency candidate; primitive stores remain fields.
This is a presentation projection, **not** a claim about data/service identity or
the absence of dependencies. All observations and projection proof remain in metadata.

`js_constructor_field` observes direct `this.field = ...` assignments in constructors,
including conditional assignments and literal right-hand sides, without retaining
values or claiming definite initialization. Same-name static and instance fields are
separate slots. `js_callable_signature` enriches represented methods/constructors with
parameter names, rest/optional flags, a `hasDefault` boolean, generic parameter names,
and canonical declared parameter/return types including type arguments. Literal types
and default values are fully redacted; no inferred runtime type is supplied. The image
uses `?` for optional/defaulted parameters. Unsupported computed/string-named method
signatures are not generated unless the callable is already a represented member.
Valid Unicode identifier/guard representations are preserved, not ASCII-normalized.

`extends` draws UML generalization with the hollow triangle at the base. Other
observations draw dashed dependencies labeled **stores argument**, **uses factory
binding**, or **creates**. These do not prove DI, composition, aggregation, provider
identity, or runtime dispatch. Type annotations are not implementations; JSDoc is
not interpreted. Anonymous classes may have filename aliases explicitly labeled
as not declared class names; names do not imply abstractness.

The renderer returns a `Bundle` with `diagrams: Vec<Diagram>`, legend, and HTML.
HTML shows actual multiple UML images first, each inert `<img>` with independent
pan/zoom through one shared CSP-authorized static script. No inline SVG or target
code runs. Full details and a JSON link follow and remain usable without SVG/JS.
Reports use `diagrams[]` with safe `group-NNNN` IDs, titles, absolute Mermaid paths,
and optional SVG paths; top-level `mermaid` / `svg` are null. `page` is null for full
UML or the selected page for focus. Coverage distinguishes catalog/selected/home
classes, group count, and modeled relations, not all repository relationships. The
legend adds `modelVersion: class-uml-semantics-2` without changing its schema or the
source/presentation v2 receipt formats.

To keep complete proofs within the unchanged 8 MiB metadata bound, the semantics-2
legend declares `sourceReferenceFormat.policy: fact-id-source-index-1` and stores
complete citations once in `proofSources`, keyed by fact ID. A `sourceRefs` integer
indexes the same record's `sourceFactIds`; resolve that fact ID through
`proofSources`. Likewise, `projectionSourceRefs` indexes `projectionSupportingFactIds`.
All proof IDs and citation links remain present. Full signature parameters,
`returnType`, and `typeParameters` live once on each member; its `signature` retains
receipt policy and declaration identity. Readers must honor the declared reference
format rather than assume each `sourceRefs` element is an inline citation object.

Relationship accounting is:

```text
modeledRelations = relationsDrawn + relationsOmitted + relationsRepresentedAsFields
relationsRepresentedAsFields = relationsRepresentedAsFieldsVisible + relationsRepresentedAsFieldsHidden
```

Hidden projections are a subset of fields, not extra omitted relations. For example,
nine retained fields can mean eight visible and one hidden by the image cap; full
details retain all nine. Per-entry `fieldCount`, `visibleFieldCount`, and
`omittedFieldCount` report the field cap separately. Less boundary clutter can change
packing/group counts without losing class coverage or changing the 12/24/8 limits.

Focus is independent of full UML. Without a full bundle it offers a plain-text
command to generate one, not a dead link. `--no-render` works entirely from saved
evidence without `mmdc`. All catalog classes is **not all repository classes**:
scripts and other fixed exclusions remain; ORM model factories are not promoted
to class declarations. Empty and incomplete states are explicit.

All groups stage and validate before one atomic publication, protecting the prior
bundle on failure. Limits are 128 groups, 8 MiB aggregate metadata, 8 MiB per SVG /
64 MiB aggregate SVG, and 180 seconds aggregate render time. The receipt version is
`source-views.v2-class-uml`; canonical `manifestHash` covers identity, explicit flat
artifact hashes/names, titles, renderer, and diagram order. It detects un-rehashed
changes, not authentication. Valid v1 class bundles can upgrade; v2 missing the hash
is rejected. `class-overview` is safely recognized as legacy only, not primary
output or a new command flag. Full/focus are separate current purposes, with no
hash/version folders or past archives. Changed rediscovery removes stale views;
older-binary downgrade is unsupported.

`js_declaration`, `js_call`, and member metadata carry explicit declaration owners
and lexical scope IDs. Same-file expansion follows uniquely resolved stable lexical
function bindings. It uses JS scope rules, including `$` identifiers, parameters,
block bindings, `var`, temporal-dead-zone, duplicate-binding, and rebinding guards;
it does not reuse Python `self` rules. Named-handler/local-helper claims must retain
their full proof bundles. Unknown or incomplete proof stays unresolved.

### Selected relative module candidates

Cross-file expansion is a separate `js_module_call_candidate` link with
`resolutionPolicy: selected-relative-functions-1`; same-file links remain
`js_local_call_candidate`. The focus policy is
`javascript-focus.v2-selected-relative-functions-1`. These are static candidates,
not module execution or runtime dispatch proofs.

The helper's module-summary policy is `selected-relative-classes-1` for static-5;
static-2/3/4 summaries retain `selected-relative-functions-1`. This does **not**
broaden the function-call resolver: callable module links remain function-only under
`selected-relative-functions-1`, separate from class UML resolution.

- **ESM:** direct named imports, including `import { work as task }`, can link to
  stable top-level named functions or variable-bound functions/arrows exported
  directly or through local aliases (`export { work as task }`). Type-only imports
  and exports do not create runtime bindings or call targets.
- **CommonJS:** top-level `const { work: task } = require('./helper')` can link to
  named functions from one plain top-level `module.exports = { work }` or
  `{ task: work }` object. Rest/default/computed destructuring, object spreads,
  methods, duplicate names, export mutation, and wrapper aliasing are not supported.
  Variable-backed callable values must be initialized before the export captures
  them; saved guard/export metadata retains and validates this ordering assertion.
  Hoisted function declarations and ESM live bindings are handled separately.
- A factory-exporting CommonJS **importer** may call an imported helper. A
  `module.exports = (...) => { ... }` factory is not a named module export target;
  no factory invocation or DI activation is inferred.
- Only relative `./` or `../` specifiers within the selected saved source inventory
  are considered. An explicit `.js`, `.jsx`, `.ts`, or `.tsx` extension must match
  exactly. An extensionless path must have exactly one selected match among those
  four extensions. A `.js` import is never remapped to `.ts`.
- There is no directory/index fallback, package lookup, tsconfig/path alias,
  default/namespace import target, re-export traversal, dynamic import resolution,
  or ESM/CommonJS interop. Missing, excluded, ambiguous, or unsupported targets
  remain unresolved; the resolver never opens additional source or configuration.

`js_module_summary`, `js_module_import`, and `js_module_export` facts support
module links. Promotion requires unique stable bindings, valid ownership/spans,
matching module formats, safe importer/exporter summaries, and full citations.
Safety is conservative and file-wide for each side: unsupported export shapes can
disable that file's exports, and unsafe import/wrapper syntax can disable its
imports, even when an individual binding looks simple.
Dependency summaries include recognized relative dependencies even when their
import forms cannot produce links. Cyclic components (including self-dependencies)
are suppressed; ambiguous selected dependency matches also participate in cycle
checks. A missing or invalid summary anywhere in the selected inventory disables
**all module promotion**, because the dependency graph could hide a cycle.
Same-file navigation does not require module promotion.

Proof is atomic: owner/declaration closures, call metadata, import/export guards,
summaries, target facts, and file provenance must be retained together within the
budgets below. The full saved evidence is receipt-bound, including dependency and
guard facts used for negative assertions (such as no competing binding, write, or
selected cycle). These assertions are scoped to that verified snapshot, not proof
of absence in excluded source or at runtime. Editing receipt-bound evidence is not
a supported way to enable a link; saved loads verify hashes and rebuild claims.

Focus defaults to 3 hops / 600 facts, accepts at most 5 hops / 2000 facts, and keeps
the exact consolidated compact UTF-8 JSON input within 192 KiB (196608 bytes).
Required facts, files, ownership, and supporting IDs count; atomic proof bundles
are omitted rather than partially promoted when they cannot fit. UI caps remain
20 callables / 40 calls for services and 20 network groups; full/focused class images
use 12-method / 8-field per-class caps with full secondary metadata.
Omission and unresolved boundaries
are explicit, not evidence that behavior is absent.

### Selected relative named-class relations

Static-5 UML resolution uses `selected-relative-classes-1` to link explicit relative
ESM named imports/exports (including import and local export aliases) or CommonJS
top-level `const` destructured requires and one plain `module.exports` object to
actual top-level **named class declarations**. Supported `extends` and `new` observations,
including `new` inside a direct method body, can target those classes. It does not
turn classes into function-call targets, infer receiver dispatch, or guess providers
from filenames. Type-only imports/exports never supply runtime classes.

CommonJS class initialization must precede export capture; importer initialization
must precede the independent `js_class_reference.start`, not merely the enclosing
relation's line/span. ESM imports are hoisted live-binding candidates, with the same
conservative dependency/SCC cycle suppression as function modules, not execution-order
proof. Default/namespace imports, packages, re-exports, interop, index lookup,
extension remapping, type-checker and configuration resolution remain unsupported.

Reached proof retains import/export guards, summaries, source and target ownership/
declaration closures, independent references, and scope-graph proof. The legend's
resolution metadata records `snapshotInventoryDependency: whole saved selected-file
module inventory`: dependency/cycle decisions rely on the **whole receipt-verified
selected snapshot**, not only the reached citations. Missing/invalid summaries disable
module promotion. Incomplete extraction also disables cross-file function/class
promotion: unreadable or rejected selected paths can hide an ambiguous extension
match or reverse dependency, even if the remaining saved files look unique.
Same-file navigation remains available. Proof is not a claim about excluded source
or runtime absence.

### API candidates and contracts

The limited route pattern is a CommonJS factory such as
`module.exports = router => { router.post('/x', req => { ... }); }`.
Supported literal method names are standard HTTP methods such as `get` and `post`,
never methods suffixed `Async`. The pattern requires
an explicit real handler fact ID: a safely resolved named handler (declared before
or after registration when binding rules permit), or an inline callback. Router
shadowing, writes, escapes, unknown bindings, or unsupported handler/path forms
prevent association. Paths are bounded literal fragments, not full mount paths.

These are source registration candidates, **not framework activation or full-mount
proof**. A random `.get` method is neither router registration nor SDK evidence.
API entries can optionally use local OpenAPI/Swagger exports under the shared
4 MiB, no-fetch, no-external-reference rules. JS matching uses literal method/path
fragments only, not handler-name fallback, FastAPI prefixes, converter projection,
or Python SDK provenance. Callable views never inspect conventional contract files;
explicit `--openapi` is API-only.

### Generic facts and consumer adapters

The current extractor records generic exported-function/factory structure,
parameters, direct returns, and referenced class identity. These are syntax facts,
not provider, request-injection, or ORM interpretations. It does not emit retired
service-factory, injected-request, or model-query receipts. Multiple, conditional,
unsupported, or mutated returns/exports do not establish a unique result.

Consumer-specific interpretation belongs in adapters compiled into consuming tools,
using the typed facts/claims boundary in the [adapter contract](adapter-contract.md).
That class-relationship extension is not a replacement for retired route or
operation/query views. The legacy `--js-profile` option returns an actionable
removed-feature error for every value, without profile dispatch or fallback.

JS services presentation uses friendly callable labels and
direct owner-to-target candidate edges, grouped by owner, target, basis, and
resolution policy. Edges retain `siteCount`, call IDs, and citations; an unresolved
call uses one implementation-unresolved node. Grouping is presentation only, not
call order or runtime multiplicity. Python and network semantics are unchanged.

## Edge Cases

### Fixed source scope

Only `.js`, `.jsx`, `.ts`, and `.tsx` files are selected under `ROOT/src` if present,
otherwise `ROOT`; root siblings are not added alongside `src`. `.mjs`, `.cjs`,
`.mts`, and `.cts` are outside the MVP scope. No include/exclude flags exist.

Inherited Python exclusions are `.git`, `.venv`, `venv`, `__pycache__`,
`node_modules`, `.ideomancer`, `caches`, `.cache`, `.pytest_cache`, `.mypy_cache`,
`.ruff_cache`, `build`, `dist`, `scripts`, `migrations`, `shadow`, `tests`,
`alembic`, `env`, `data`, and `db`. JS/TS additionally excludes all dotfolders,
`bower_components`, `assets`, `vendor`, `generated`, `coverage`, `init`, `config`,
`configs`, and `environments`. Names containing `.spec.` or `.test.`, or ending
`.min.js`, `.bundle.js`, or `.d.ts`, are skipped. `--path`, `--query`, and paging
filter display only, not source selection or saved inventory.

Unix (Linux/macOS) is required. The root must be an existing non-symlink directory;
anchored no-follow/nonblocking opens and descriptor type checks avoid substituted
symlinks/FIFOs. Relevant filesystem/parse failures mark evidence incomplete;
incomplete catalogs can be saved but discovery/catalog exit 1. Completeness describes
selected source availability/parsing, not runtime coverage. Excluded entries and
static-resolution limitations are not automatically failed scans.

Source bounds remain 5000 files, 1 MiB/file and 16 MiB total source. Parsing and
traversal share a cooperative **60-second** extraction deadline; tree traversal is
bounded to **200000 nodes** and **depth 256**. Baseline fact/output bounds are
100000 facts and 32 MiB, with separate optional-fact budgets and rollback.
These in-process controls are **not a hard memory sandbox** or an OS-enforced
preemption guarantee; native C parsing can allocate before cancellation is observed.
Legacy helper stdin/stdout and Node heap settings do not describe native limits.
Budget exhaustion must fail closed with explicit incomplete/error reporting, not
partially promoted proof. Python retains its separate AST-helper process controls.

### Unsupported execution and resolution

- Sloppy Annex B block functions without syntactic strict-mode proof suppress
  call/API association and produce a generic diagnostic
  (`javascript_annex_b_binding_unresolved`), rather than guessing runtime hoisting.
- Getter/setter bodies, deferred initializers, static blocks, heritage expressions,
  computed/decorator execution, and default-parameter body calls are omitted with
  execution-boundary diagnostics, not promoted into an enclosing workflow.
- No general `this`/`new` receiver, prototype, `bind`, inheritance dispatch/MRO,
  CommonJS execution, or DI resolution is claimed. Class-relation observations and
   guarded local/imported class relations do not expand callable dispatch. Cross-file
   callable navigation remains limited to selected relative named functions; named
   classes resolve only for supported UML relations.
  There is no JS network proof, even for familiar client names or methods.
- Ordinary literal values are redacted, with deliberate route/import/dependency identifier
  metadata retained. Paths, symbols, fragments, and expression shapes still expose
  private structure. Redaction is **not a secret detector**; keep artifacts ignored
  and private. No target execution or target credential/environment-file reads occur.

### Bounded MVP acceptance matrix

This matrix defines the source-navigation acceptance boundary, not final test
counts, whole-application completion, runtime coverage, or a live-model evaluation.

| Case | MVP result / acceptance boundary |
| --- | --- |
| API, class, and public callable discovery | Saved source candidates with ownership, references, and explicit omissions; no runtime activation |
| Unfiltered class diagram without explicit paging | Actual grouped UML; every catalog class has one home, <=12 homes per diagram; folder/range grouping is not runtime architecture |
| Explicit class entry/query/path/limit/offset | Independent `views/class-focus`; no automatic full/inventory generation |
| Static-4 class relations | Guarded same-file local-class bases and observed argument storage/factory use/construction; parameter bases remain scoped boundaries, not guessed providers |
| Static-5 named-class modules | Guarded relative ESM/CommonJS named class declarations for aliased `extends`/`new`, including method-local creation; no callable-dispatch or DI claim |
| Storage-only constructor argument | Field/parameter projection with full relation metadata; shown + omitted + represented-as-fields accounts for all modeled observations |
| Constructor state and callable signatures | Direct constructor field observations and represented-member signatures; conditional writes are not definite initialization; values/literal types redacted |
| Static-1/2/3 class snapshots | Members and declared-base boundaries without new class-relation promotion |
| Stable same-file helper call | `js_local_call_candidate` with complete lexical binding proof |
| ESM direct named import / import alias / local export alias | `js_module_call_candidate` to a selected stable named function, when summaries and guards permit |
| CommonJS top-level const destructure / alias + single plain export object | Named-function call candidate; static-5 separately permits named-class UML targets; no CommonJS execution |
| Factory-export importer calls a helper | Helper candidate allowed; factory not promoted as an imported target |
| Generic exported factory facts | Parameters, direct returns, and class identity only; no filename-provider or injected-request inference |
| Relative explicit extension / extensionless path | Exact selected path / unique selected extension expansion only |
| Type-only binding, default/namespace import, re-export, package/index/alias/remap/interop | No runtime module target promotion |
| Cycle, missing/invalid summary, unsafe or competing binding/export | No affected module link; incomplete summary inventory disables all module promotion |
| Hop/fact/byte limit | Explicit boundary; no partially retained proof or dangling promoted target |
| Saved browsing and native views after extraction tools/source are unavailable | Verified artifacts only; no extractor or live source read; `--no-render` also avoids `mmdc` |
| Evidence/view tampering | Receipt validation fails, not silent reuse or repair |

**Deferred capabilities:** runtime execution/activation and full mounts; runtime DI;
receiver, prototype and inheritance dispatch; JS network/SDK provenance; JS/TS agent
analysis; whole-program/caller discovery; package/index/config-based resolution,
extension remapping, module interop, default/namespace imports and re-exports;
additional source extensions and configurable scan scope. The MVP does not claim
that any whole private application is completely understood.

## Example usage

With the native candidate built:

```sh
ideomancer discover examples/javascript-worker --language ts
ideomancer catalog .ideomancer/catalogs/javascript-worker --kind workflow
ideomancer diagram .ideomancer/catalogs/javascript-worker --kind class --no-render
ideomancer diagram .ideomancer/catalogs/javascript-worker --kind class --query Formatter --no-render
ideomancer diagram .ideomancer/catalogs/javascript-worker --kind api --no-render
ideomancer diagram .ideomancer/catalogs/javascript-worker --kind workflow --entry prepareBatch --view services --no-render
ideomancer diagram .ideomancer/catalogs/javascript-worker --kind workflow --entry prepareBatch --view network --no-render
ideomancer catalog .ideomancer/catalogs/javascript-worker --kind api --query /normalize
ideomancer diagram .ideomancer/catalogs/javascript-worker --entry API_ID --view services --no-render
```

Replace `API_ID` with the POST `/normalize` API ID from the preceding catalog page.
Alternatively use `--kind api --entry SYMBOL` with its exact handler symbol or a
unique dot-suffix; do not guess a stable anonymous-callback label. Entry selection
cannot be combined with query filters. The public `routes.js` handler calls an
imported helper, which in turn calls its same-file helper.

Open `.ideomancer/catalogs/javascript-worker/views/index.html` for the script-free
index of generated current views. Start with grouped UML at `views/class/index.html`,
then focus a class by ID, query, path, or explicit paging at `views/class-focus`.
Both use multiple independent offline image viewports, with full details secondary.
Neither command generates the other bundle. Saved views remain usable without extractor
tools; rediscovery replaces the current snapshot and stale views, not a snapshot history.

The public module example uses `batch.ts` importing `normalize` from `normalize.ts`,
where `normalize` calls a same-file helper. The CommonJS counterpart uses `routes.js`
as a factory-export importer of `normalize` from `cjs-helper.js`; that function also calls
a same-file helper. These illustrate module-to-local traversal, not factory activation.
The supported import/export shapes are:

```ts
// normalize.ts
function clean(value: string): string { return value.trim(); }
export function normalize(value: string): string { return clean(value).toLowerCase(); }
// batch.ts
import { normalize } from './normalize';
export function prepareBatch(values: string[]): string[] {
    const prepared: string[] = [];
    for (const value of values) prepared.push(normalize(value));
    return prepared;
}
```

Passing a function to `map` instead would not itself prove its invocation; the
example uses a direct `normalize(value)` call.

```js
// cjs-helper.js
function trim(value) { return value.trim(); }
function normalize(value) { return trim(value).toLowerCase(); }
module.exports = { normalize };
// routes.js (factory body can use normalize; factory is not a module target)
const { normalize } = require('./cjs-helper');
module.exports = router => {
    function handle(req) { return normalize(req.body.value); }
    router.post('/normalize', handle);
};
```

`--out` is a container, not the final application-named directory; output paths
are relative to the invocation cwd. Keep private-source artifacts ignored and
private; distribute only public fixtures and sanitized examples.

For named-class UML, the following inline public example is separate from the
existing `Formatter` fixture (which imports no class); no fixture file or output
mode is added:

```ts
// parent.ts
export class Parent {}
// child.ts
import { Parent as Base } from './parent';
export class Child extends Base {
    make(): Base { return new Base(); }
}
```

When both files are selected, static-5 can resolve the aliased base and method-local
creation to `Parent`. A CommonJS variant is `class Parent {}; module.exports =
{ Parent };` in `parent.js`, then `const { Parent: Base } = require('./parent');`
before `class Child extends Base { make() { return new Base(); } }` in `child.js`.
Use either pair independently to keep extensionless paths unambiguous. See the
[worker's inline examples](../examples/javascript-worker/README.md#inline-named-class-examples).

### Validation tooling

Native extraction tests use synthetic fixtures and compiled-in pinned grammars,
without Node/compiler provisioning or tool-availability skips. CI keeps Python
3.12, Rustfmt, Clippy, tests, package-policy/inventory checks, package verification,
and the release build. It does not install Node, TypeScript, Mermaid, or browsers.
The declared MSRV is Rust **1.89**, with direct exact dependency pins protecting
downstream resolution. Record Rust 1.89 and current-stable matrix results separately
before release. The package review must retain applicable runtime/grammar licenses
and notices, including compiled C grammar code in binary distributions.

Local validation on Rust 1.89: the full normal suite has **563 passing tests**
(13 optional tests ignored in that run), including 44 JS/TS view tests, 25 class
UML tests, 13 adapter API tests and 8 engine API tests. All **nine applicable
non-agent optional checks** also pass with existing trusted render/browser tools
and a synthetic saved catalog, plus six package-policy tests. The four optional
agent-bridge tests remain unrun, not reported as fixed. Independent review's six
soundness/resource findings have regression fixes and a clean focused re-review.
These local results are not a claim of hosted matrix success or release approval.

```sh
cargo test --locked --test javascript_views
cargo test js_focus::tests::real_mermaid_accepts_javascript_members -- --ignored
```

The ignored real-Mermaid check requires existing `mmdc` and Chromium; it installs
nothing. The existing [offline browser regression](../README.md#development) is
reused for generated JS/TS diagram pages with an existing Puppeteer/Chromium setup.
Structural/offline checks are not private-app or live-model quality guarantees.

The optional grouped-UML browser regression requires trusted Python >=3.12
(`IDEOMANCER_TEST_PYTHON` can select it), trusted external Node, and an already-installed
Puppeteer module/Chromium. It downloads nothing:

```sh
IDEOMANCER_TEST_NODE=/absolute/trusted/node IDEOMANCER_PUPPETEER_MODULE=/absolute/installed/puppeteer cargo test --test class_uml -- --ignored --nocapture
```

## Related files

- `src/cli.rs`: explicit discovery language selection and retired tool-setting errors.
- `src/analysis.rs`: shared evidence types, language validation, scanner, bounded child I/O.
- `src/javascript.rs`, `src/javascript_native.rs`, `src/javascript_native/optional.rs`: native parser dispatch,
  syntax extraction, lexical guards, and route candidate metadata.
- `src/js_modules.rs`: selected relative paths, summary validation, cycles, and module proof.
- `src/js_focus.rs`, `src/focus.rs`: atomic local/module proof selection and shared focus dispatch.
- `src/catalog.rs`, `src/discovery.rs`: catalog candidates, receipts, saved selection,
  and pre-dispatch rejection of JS/TS agent workflows.
- `src/render/source_view.rs`, `src/render/endpoint.rs`: native member/call/API views.
- `src/render/class_uml.rs`, `tests/class_uml.rs`, `tests/class_uml_browser.cjs`:
  grouped UML, independent full/focus behavior, and optional browser checks.
- `src/render/class_overview.rs`: shared class/member grounding and legacy inventory support.
- `tests/javascript_views.rs`: offline public CLI contracts and negative boundaries.
- [JavaScript worker](../examples/javascript-worker/README.md): public worker and module-helper examples.
- [Discovery](discovery.md), [Python analysis](python-analysis.md),
  [agent workflow](agent-workflow.md): shared interfaces and language-specific limits.
