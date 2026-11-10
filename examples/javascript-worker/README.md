# JavaScript / TypeScript source exploration

## Overview

These small source fixtures exercise syntax-only discovery and offline diagrams.
`batch.ts` contains an aliased named import from `normalize.ts`, which calls its
own local helper, and a class with declared TypeScript visibility. `routes.js`
contains an injected-router factory and a named handler importing a helper from
`cjs-helper.js` through CommonJS destructuring. None is executed or imported by
Ideomancer; linking uses the saved selected-source inventory and syntax proofs.

The route fixture uses standard `router.post('/normalize', handle)` syntax.
It demonstrates module-to-local helper traversal, not request injection or
framework-specific provider resolution.

## API / Interfaces

Provision a trusted Node executable and a self-contained TypeScript compiler
artifact **outside the repository being analyzed**. No compiler is installed or
resolved from the target's dependencies automatically:

```sh
export IDEOMANCER_NODE=/absolute/path/to/trusted/node
export IDEOMANCER_TYPESCRIPT=/absolute/path/to/trusted/typescript/lib/typescript.js
./target/release/ideomancer discover examples/javascript-worker --lang javascript-typescript
./target/release/ideomancer diagram .ideomancer/catalogs/javascript-worker --kind class --no-render
./target/release/ideomancer diagram .ideomancer/catalogs/javascript-worker --kind class --query Formatter
./target/release/ideomancer diagram .ideomancer/catalogs/javascript-worker --kind workflow --entry prepareBatch --view services
./target/release/ideomancer diagram .ideomancer/catalogs/javascript-worker --kind api --query /normalize
```

Run these commands from the Ideomancer repository root after a release build.
`--lang js`, `--lang ts`, and `--lang typescript` select the same combined adapter.
The current viewers are linked from
`.ideomancer/catalogs/javascript-worker/views/index.html`.

Start with `views/class/index.html`: actual grouped UML images show members,
declared-base generalizations, and observed dependencies. Every selected class has
exactly one home group, at most 12 homes per diagram, with boundary/context-aware
packing (24 boundaries / 8 context classes). Titles use source folder and stable
class range, not runtime architecture. Images cap methods/fields at 12/8; full
observed members and modeled relationships remain in secondary details and JSON.
Then focus `Formatter` as above, or use `--entry CLASS_ID`, a path, or explicit
paging. Even `--limit 12` or `--offset 0` requests independent `views/class-focus`
(default 12 entries, maximum 30), without automatically generating full UML or an
inventory. Do not combine entry selection with filters/paging. Catalog browsing
is unchanged.

Discovery uses only supplied source bytes and `createSourceFile`; it does not load
target manifests, TypeScript configuration, plugins, or imports. Saved views do not
invoke the extraction Node runtime or compiler. Rendering SVG uses an independently
trusted Mermaid CLI and its Node/Chromium toolchain; `--no-render` avoids that
renderer dependency.

The source-navigation slice resolves supported same-file lexical functions and
bounded relative named-function imports. Static-5 separately supports named-class
UML relations through explicit relative ESM/CommonJS imports/exports and aliases;
it does not expand function-call resolution to classes. Explicit extensions must match exactly;
extensionless imports need a unique selected `.js`, `.jsx`, `.ts`, or `.tsx` file.
There is no index/package/configuration lookup, extension remapping, interop,
default/namespace/re-export linking, or cyclic-module linking. Complete retained
import/export/binding proofs support candidates, not runtime reachability.
Injected receivers, `this`, prototypes, mixins, inheritance dispatch, and JS network
effects remain explicit boundaries. Class relations are source observations, not
resolved runtime object relationships. API paths are fragments, not deployed URLs.

**Use explicit `--view services` or `--view network` for callable entries.** JS/TS
agent workflows are rejected locally in this MVP; no Python or model fallback
is attempted. Python analysis remains focused on FastAPI rather than expansion to
other Python web frameworks.

## Data Flow

Class diagrams reuse verified saved evidence without reading target source or
invoking extraction tools. Full UML and focus are independent; without full UML,
focus offers a plain-text generation command, not a dead link. The legend uses
`ideomancer.class-uml.v1` with `modelVersion: class-uml-semantics-2`; the renderer's
`Bundle` has `diagrams: Vec<Diagram>`. Source/presentation v2 receipts are unchanged.
Reports expose `diagrams[]` with safe `group-NNNN` IDs, titles, absolute Mermaid
paths, and optional SVG paths; top-level `mermaid` and `svg` are null. `page` is
null for full UML or the selected page for focus. Coverage explicitly counts
catalog/selected/home classes and groups. Modeled relations equal shown + omitted +
represented-as-fields (`relationsRepresentedAsFields`), with visible/hidden field
subsets reported separately. For example nine retained fields can mean eight visible
and one hidden; hidden projections are not extra omitted relations.
Caps never omit home classes and do not imply all relationships are drawn.

Static-4/5 `js_class_relation` receipts validate binding guards, an independent
reference, and the nearest lookup-scope graph before promotion. Static-5 uses
`javascript-class-relations-2` with explicit `importFactId`; static-4 local relations
keep rendering under their existing policy. Direct same-file local-class bases can
resolve; factory-parameter bases remain scoped boundaries, not links to guessed
provider files. Hollow triangles point to bases. Dashed dependencies
mean **stores argument**, **uses factory binding**, or **creates**, never DI or
composition. Constructor-assigned parameter fields and TS parameter properties
(such as `Formatter.prefix`) are captured, as are supported captured factory uses
and `new`. Types are annotations, not implementations; JSDoc is not interpreted.
Anonymous classes may use filename aliases explicitly labeled as not declared names.

Storage-only arguments project as fields/constructor parameters instead of invented
dependency boxes, unless an identified class, supported nonprimitive named declared
type, or explicit receiver/creation/factory-binding use supports a candidate.
`Formatter.prefix` is state, not a guessed service. Projection does not establish
data/service identity or absence of dependencies; all observations remain in metadata.
Less boundary clutter may change groups, not class coverage or packing limits.

Optional `js_constructor_field` receipts include direct constructor `this.field`
assignments with conditional or literal right-hand sides, without retaining values
or claiming definite initialization. Same-name static/instance fields are separate.
`js_callable_signature` enriches represented methods/constructors with rest/optional
flags, default-presence booleans, generic names and canonical declared types, including
type arguments/returns. Literal types/default values are fully redacted; unsupported
computed/string-named signatures do not invent members. Valid Unicode guards remain
in their original representation.

Static-5 class module resolution uses `selected-relative-classes-1`, linking only
actual top-level named class declarations for `extends`/`new`, including method-local
creation. CommonJS export capture and import initialization order are checked;
import ordering uses the independent reference start. ESM imports are hoisted
live-binding candidates with conservative SCC cycle suppression, not runtime proof.
Type-only imports/exports are not runtime classes. Reached proof includes guards,
summaries and source/target closures; metadata records its dependency on the whole
receipt-verified selected module inventory. No filename-provider guesses, type
checker, configuration, or additional module fallback is used.

HTML places multiple actual UML images first, each an inert `<img>` with independent
pan/zoom from one shared CSP-authorized static script; secondary full details and a
JSON link follow. No inline SVG or target code runs. Details work without SVG/JS.
`--no-render` uses saved evidence only and never calls `mmdc`.

Ordinary discovery saves generic syntax facts, not injected-service or query
receipts. JS services views group repeated
resolved candidate calls into direct owner-to-target edges with source-site counts
and friendly labels. Each unresolved call uses one implementation-unresolved node.
Python and network semantics are unchanged.

## Edge Cases

- All catalog classes is not all repository classes: scripts and other fixed
  exclusions remain outside discovery scope. ORM factories are not promoted to
  classes. Factory-parameter bases stay unresolved; no discovery scope is expanded.
- Empty and incomplete states are explicit. Incomplete evidence may omit
  classes/members even though every selected catalog class has a home.
- All groups stage and validate before one atomic publication, preserving the prior
  bundle on failure. Limits: 128 groups, 8 MiB aggregate metadata, 8 MiB per SVG /
  64 MiB aggregate SVG, and 180 seconds aggregate rendering.
- `source-views.v2-class-uml` receipts have canonical `manifestHash` covering
  identity, explicit flat artifact hashes/names, titles, renderer, and diagram order.
  This detects un-rehashed changes, not authentication. Valid v1 class bundles can
  upgrade; v2 missing the hash is rejected. `class-overview` is safely recognized
  as legacy only, not primary output or a new command flag. Changed rediscovery
  removes stale views. Full/focus are separate current purposes, not archives or
  hash/version output folders; older-binary downgrade is unsupported.
- The legacy `--js-profile` option is removed and returns an actionable error,
  without operation-profile dispatch or fallback. Consumer adapters own framework
  interpretation; the class extension does not replace retired operation views.
- Current evidence is `javascript-typescript-static-6-generic-facts`; legacy static-1
  through static-5 remain accepted for generic browsing. Static-2 and newer support module links;
  static-1 supports same-file navigation. Static-1/2/3 still show members and
  declared-base boundaries without new class-relation facts. Static-4/5 promote
  class relations; static-5/6 resolve named-class modules. Static-5/6 helper
  summaries use `selected-relative-classes-1`; older static-2/3/4 summaries use
  `selected-relative-functions-1`. Function-call links stay function-only under the
  latter policy. Rediscover for missing generic factory or class-relation facts.
- Route/import/dependency identifiers are a privacy boundary, not a secret filter.
  Arbitrary predicate values remain redacted.
- Constructor-field/signature receipts share the optional class-relation budget of
  4 MiB / 12000 facts. Class-batch overrun
  rolls back that file's optional facts with `javascript_class_relations_omitted`;
  no target source execution is involved.

## Example usage

After configuring the trusted tools above, run from the repository root:

```sh
ideomancer discover examples/javascript-worker --lang js
ideomancer diagram .ideomancer/catalogs/javascript-worker --kind class --no-render
ideomancer diagram .ideomancer/catalogs/javascript-worker --kind class --query Formatter --no-render
ideomancer catalog .ideomancer/catalogs/javascript-worker --kind api --query /normalize
ideomancer diagram .ideomancer/catalogs/javascript-worker --entry API_ID --view services --no-render
```

Replace `API_ID` with the POST `/normalize` API ID returned by the catalog command.
Alternatively use `--kind api --entry SYMBOL` with that handler's exact symbol or
unique dot-suffix. Do not guess a stable anonymous callback label, or combine
`--entry` with `--query`/paging. Use the installed `ideomancer` or substitute
`./target/release/ideomancer` consistently. Open the reported viewer for inspection
locations and unknowns; expand its technical legend for retained proof.

### Inline named-class examples

These are illustrative public source snippets, **not additional fixture files**.
The existing `Formatter` fixture imports a function, not a class. With these files
in a selected source root, ordinary discovery and the same `--kind class` command
can resolve the named-class relations; no new output mode is required:

```ts
// parent.ts
export class Parent {}
// child.ts
import { Parent as Base } from './parent';
export class Child extends Base {
    make(): Base { return new Base(); }
}
```

The CommonJS equivalent uses a named export object and initialized destructure:

```js
// parent.js
class Parent {}
module.exports = { Parent };
// child.js
const { Parent: Base } = require('./parent');
class Child extends Base {
    make() { return new Base(); }
}
module.exports = { Child };
```

Use either pair independently: selecting both `parent.ts` and `parent.js` makes an
extensionless import ambiguous. Neither snippet proves DI, registry resolution,
runtime execution, or provider identity.

## Related files

- `../../src/render/class_uml.rs`, `../../tests/class_uml.rs`,
  `../../tests/class_uml_browser.cjs`: grouped UML, independent full/focus contracts,
  and optional browser checks (see [validation tooling](../../docs/javascript-typescript-analysis.md#validation-tooling)).
- `batch.ts`, `normalize.ts`, `routes.js`, `cjs-helper.js`: module-to-local navigation.
- [JS/TS analysis](../../docs/javascript-typescript-analysis.md),
  [discovery](../../docs/discovery.md): interfaces, budgets, privacy, and limitations.
