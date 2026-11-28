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

Production remains `python-static-8-generic-facts` with the trusted Python >=3.12
helper. The user selected contract-preserving full replacement and authorized the
bounded private F1 lexer experiment following F0. Start with the
[current F1 ledger](#f1-shared-byte-lexer-experiment) and
[feasibility ADR](python-native-replacement-feasibility.md); the
[frontend-5/facts-7 visitor contract](#ruff-visitor-contract-batch-frontend-5--private-facts-7)
and private G2/G3 closure are unchanged. Earlier results/cleanup receipts remain
historical, not current-tree validation or cache status. The user-reported actual
HEAD before F1 is `fdf4e58` (the facts-6 commit; initially clean and ahead one), not
`523cd7b`. Earlier HEAD/uncommitted-work statements below describe their checkpoints.

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

## Native migration parity foundation

### Completion contract and remaining gates

P1 finishes when a native backend can replace the existing extractor through the
supported product seams, not when another synthetic fixture parses. The contract
is the observable behavior of `python_extract.py`: complete ordered facts and
diagnostics, redaction, identities/spans, supported input bytes, failures and
resource behavior. It is **not** Python execution, type inference or a control-flow
graph. Syntax without a dedicated fact still needs exact descendant traversal.

The development comparison baseline is **Python 3.12**, with the actual trusted
interpreter patch reported by the oracle gate. This is not a change to production's
existing Python >=3.12 prerequisite, nor approval of a narrower production language
or encoding contract. Additional minor-version claims need separate acceptance.

The finite completion ledger is:

| Gate | Outcome and required acceptance | State |
| --- | --- | --- |
| P1-G1 | Generic expression and ordinary-body coverage: containers/operators/indexing, call-free defaults, conditions, loops and context managers; hidden calls/warnings, owner restoration, spans and one counted walk remain exact. | Implemented and locally validated for the bounded G1 subset below |
| P1-G2 | Close the generic visitor inventory: remaining statements, targets/writes/imports, arbitrary callees/splats, comprehensions/lambdas/generators, await/yield/walrus, all headers and literal/identifier forms. Every target-version node has a normalization/traversal rule and cross-family oracle cases. | **PRIVATE IMPLEMENTATION COMPLETE/CLOSED** for current admitted Python 3.12 Module-AST lane: closed class/field inventory + full-output/raw-replay coverage, 364 matching positives and completed preservation checkpoint; not complete Python compatibility or G4–G6/cutover |
| P1-G3 | Preserve every existing discovery/framework side output: decorators, CLI/job/API entry facts, route diagnostics and FastAPI host/prefix/include/mutation metadata, including aliases and literal-retention limits. | **PRIVATE IMPLEMENTATION COMPLETE/CLOSED** for the same admitted lane, including metadata/diagnostic/cap coverage and completed preservation: 108 Ruff (17 opt-in), 685 library (191 ignored), 18 adapter / 8 engine, 165 reference + 3 native; no new framework semantics or G4–G6/cutover approval |
| P1-G4 | Close grammar/version/encoding/failure differences: original-byte digests, decoding/cookies, physical layout and exact observable parse diagnostics. No unexplained valid-file rejection or invalid-file acceptance in the approved compatibility suite. | Open: bounded F1 lexer goal COMPLETE / PROVED by compiled tests; all F1 delivery gates passed, consumer and cleanup complete. Byte parser input, AST/debug construction, CPython admission/F2, approved version policy and remaining failure/resource/codec gates still block replacement; production helper retained |
| P1-G5 | Integrate distinct native identity, evidence/snapshots, CLI/library dispatch, workflow revalidation and capabilities. Preserve old-save reading/rendering; rediscovery invalidates stale claims. Native operation must not probe Python. | Pending; public migration design required |
| P1-G6 | Existing-engine usefulness and operational/cutover acceptance: class/callable/API/network/services views, adapter proofs, saved-source reuse, supported platforms/toolchain/package/resource checks, then explicit replacement approval. | Pending |

G1's named-base/annotation predecessors and its next expression/body work do not
close G2–G6. Partial milestones remain partial; passing oracle counts are not a
coverage percentage. A production incomplete backend is a separate, explicitly
approved product boundary—not a substitute for replacement parity.

#### Generic inventory and implementation strategy

- **Declarations/headers:** all function/class forms, signatures, parameter/default/
  annotation facts, bases, decorators and child visitation order.
- **Expressions:** every special `expression()` branch plus exact opaque AST-node
  names; descendant calls/warnings remain independent from shape rendering.
- **Statements/writes/imports:** exact helper traversal, bindings/invalidation,
  annotation-only nonwrites, router mutation behavior, aliases and neutral syntax.
- **Existing metadata:** all custom helper visitors and diagnostics, without adding
  private conventions or strengthening runtime claims during migration.
- **Input/failure/integration:** bytes and grammar, complete versus incomplete,
  hard extraction limits, identity/capability policy and old-save compatibility.

Keep capture/admission, the pinned front end, transactional emitters, IDs and budgets.
Grow the bounded AST-shaped descriptor layer rather than add fact-specific parsers
for each context. Known fact-neutral wrappers require explicit validation rules;
unknown grammar nodes must not be silently ignored. The one counted CST walk may
build bounded semantic events for later processing, but must not be replaced by
an uncounted traversal or source rescan.

Implementation and handoff use larger, cross-family batches. Within a batch,
disjoint implementation ownership and focused tests remain incremental, but shared
Cargo work is serial and the broader gates/cache cleanup happen at one integrated
checkpoint. Adding one more syntax node is not itself a delivery milestone.

Source order is sufficient only where proven equivalent to helper visitation.
Header/decorator calls require explicit AST-order scheduling before admission:
body imports can affect later-visited return/decorator calls in the helper. Do not
enable these calls merely because their shapes can be rendered.

#### Compatibility and rollout decisions that cannot be skipped

- Resolve the known Python-2 grammar acceptance and valid-Python nested import-layout
  rejection. Collector checks can reject specific invalid forms; they cannot recover
  a tree for every grammar rejection. Maintain a finite correction/exclusion list.
  Reassess the pinned parser before G2/G3 investment if no credible correction path
  exists; no parser dependency replacement is approved by this plan.
  A further AST-only discrepancy is the standalone starred conditional root
  `*(a if ready() else b)`: the Python oracle constructs an AST, while the pinned
  grammar rejects it. That explicit module-only rejection is not counted as parity.
  Ungrouped chained-subscript/member annotation spellings such as
  `def f(x: T[A][B]) -> T[A][B]: pass` are another explicit AST-positive/native
  syntax-error case. Grouped equivalents are separately tested as accepted. These
  differences remain parser-feasibility work, not permission to ignore ERROR nodes.
- Decide encoding coverage explicitly. Current Ruff frontend-5 retains UTF-8/BOM,
  eligible cookies, strict ASCII and 62 pinned ASCII-identity single-byte tables
  with the existing Latin-1 fast path; other codecs remain unsupported or
  unresolved. Historical Tree-sitter/earlier Ruff cookie exclusions below are not
  the current Ruff profile. A narrower production contract needs an explicit
  decision, not a silent migration.
- Scope a production semantic fingerprint to a reviewed closed set of implementation,
  normalization/decoding and dependency inputs. Current private fingerprints include
  test-bearing files and are conservative development identities, not the final policy.
- Design native metadata without inventing `pythonVersion`; review public request
  compatibility and retirement of interpreter settings. Never silently reinterpret
  an AST request or upgrade an old saved snapshot's capabilities.
- Reconcile private module-only `ResourceLimit` reports with production hard-limit
  failures before promotion. Cooperative in-process parsing is not an RSS sandbox.
- Keep representative real-source acceptance consumer-owned and outside project
  source. Never run the analyzed application's commands or import its modules.

Implementation proceeds through G1, G2 and G3; grammar feasibility
is tracked from the start, not postponed until cutover. G4 must close before G5 can
be accepted as a replacement, and G6 is the final stopping gate. New analysis
features or later Python-version lanes are separate work after these gates.

#### Parser-feasibility checkpoint and stop decision

**Historical records:** the following chronological probes and implementation
checkpoints describe their then-current policies, exclusions, inventories and
validation only. They are not current facts-7 limitations or a final integrated
pass. See [the current frontend-5/facts-7 batch](#ruff-visitor-contract-batch-frontend-5--private-facts-7)
for the current finite profile and validation record.

A bounded isolated prototype used the installed Tree-sitter CLI **0.26.11**, native
JavaScript runtime and ABI **15**, cached Python grammar **0.25.0** at revision
`293fdc02038ee2bf0e2e206711b69c90ac0d413f`, and runtime **0.25.10**. Generated C
compiled with the existing C compiler, and language installation/controlled parsing
worked against that pinned runtime. The shipped parser's generator provenance was
0.25.9: regenerated parser-C bytes changed, even though grammar/node-types JSON and
the 20 tested CST structures/acceptance results were unchanged. This is bounded
compatibility evidence, not general generator equivalence or fact parity.
Recorded SHA-256 provenance: upstream grammar
`48bb681fdcc51784077b7d2cbc138a1994309400d32e4ac74fcd304d9fe8ad81`, shipped parser C
`a895f10b3cf7b2608f3283b43cd5cfed70971c7ee4a0136abbaaccbc4a7a25e0`, and regenerated parser C
`543ece58a3968522f2515f6c9a5bd7086cc25fac747e5ed209c3be5a0813f2f9`.

The correction attempt did **not** establish a suitable patched parser:

| Family | Observed prototype outcome |
| --- | --- |
| Nested import layout | Simple variants parsed; an AST-positive comment-containing nested variant rejected. A scanner-only dedent guard produced no improvement. |
| Standalone starred conditional/lambda roots | AST-positive forms still rejected; simple standalone stars and call-argument controls parsed. |
| Ungrouped chained annotations/alias RHS | Chained-subscript/member forms still rejected; grouped/simple/generic declaration controls parsed. |
| Python-2 statements | Python 3 AST rejected while the grammar accepted; collector/failure mapping still needs explicit closure. |

The combined grammar proposal stopped after three successive generation conflicts
(splat/collection, splat/as-pattern, primary/generic-type). It was never compiled or
tested as a corrected parser. The separately compiled scanner-only patch changed
none of the tested results and must not be integrated on this evidence. No registry,
workspace dependency, parser source, global configuration or production default
was changed. All copied/generated sources and test libraries were deleted after
their processes stopped.

**Decision at that checkpoint:** stop unbounded correction-fork investment and do
not begin substantial G3 integration against an assumed-correct frontend. The next
architectural checkpoint was a separately approved native AST-parser evaluation,
now recorded below. Reusable token-local literal validation may proceed; source
rewriting, inserted grouping, recovery reparses and ignored ERROR nodes are not
substitutes for compatible parsing.

Read-only candidate screening identified Ruff **0.12.0**, revision
`87f0feb21a1af67195bebf94d811770393f9f447`, as a possible Rust-1.89-compatible
evaluation lane (declared workspace MSRV 1.85). Its parser is an unpublished
workspace crate. The subsequent isolated evaluation is recorded below; it does not
establish full AST/fact compatibility. Current Ruff main exceeds the project's
compiler baseline.
RustPython Parser **0.4.0** is separately packaged and declares an older MSRV, but
source screening found pending PEP 701 handling and surrogate-value replacement;
it is not an unqualified full-3.12 solution. Its default bigint backend also requires
license review rather than an MIT-only assumption.

The lexical batch performed read-only screening, not candidate builds. Subsequent
explicit approval covered one isolated, exactly pinned Ruff source/dependency
retrieval, license/MSRV/feature review, locked offline build and compatibility
matrix using existing tools. That evaluation did not authorize toolchain
installation, repository adoption, public request/schema changes or a production
switch. No candidate dependency or parser source was added to the product.

#### Isolated Ruff evaluation and next decision

The approved evaluation used upstream tag **0.12.0**, verified at revision
`87f0feb21a1af67195bebf94d811770393f9f447`. Provenance SHA-256 values were:

| Input or inventory | SHA-256 |
| --- | --- |
| Source archive | `c92776b2ecf7f212777a23f165d49f8950fcc40da38b48f770577a8a24788b81` |
| Upstream lockfile | `ccc843838d16f203d18a3410dd120e72e750a89ee42164145ae9cbf181fb6f47` |
| Pruned probe lockfile | `9c9807c1e45cb16fb8626fe59a5b90d516190cb47851be15512649bfcc700fb0` |
| Selected version/feature inventory | `aa9ea3a106f337c043ab95787896cf2c28383a9233d1471e20c46b0a25cb8f26` |

Archive paths and entry types were checked before extraction; one validated
in-tree corpus symlink was omitted. The temporary probe used path dependencies on
`ruff_python_parser`, `ruff_python_ast` and `ruff_text_size`, plus
`serde_json = "=1.0.140"`. Its closure contained **53 upstream packages plus the
probe**. Retained upstream lock entries matched versions, sources and checksums;
selected missing packages were fetched without substituting newer versions. The
Ruff crates had no enabled features; serde_json used default/std features.

The probe built with **Rust 1.89.0 on macOS arm64**, `RUSTUP_AUTO_INSTALL=0`,
locked/offline resolution, two jobs, line-table-only debug information and no
incremental compilation. It used the existing project target directory, not a
second build cache. Ruff declared workspace MSRV **1.85**, but 17 dependency
manifests lacked an MSRV declaration. A successful 1.89 build is not proof of the
declared lower MSRV or of other supported platforms. License metadata review found
no declared copyleft in the retained closure; Unicode notices and upstream
RustPython notices still require preservation/review before redistribution. This
was not full source, binary or legal clearance.

The comparison oracle was **Python 3.12.11**, with Unicode data **15.0.0** and
integer digit limit **4300**. Ruff was explicitly targeted at **PY312**. Candidate
acceptance required both parse success and an empty separate unsupported-version
diagnostic channel: no recovered or errored tree counted as accepted.

Of **114 independently synthetic cases, 105 matched and nine differed** under the
probe's selected comparisons:

| Category | Differences |
| --- | --- |
| Acceptance (3) | Python AST accepts duplicate call keywords (`f(x=1, x=2)`), while Ruff rejects with `DuplicateKeywordArgument`; Python rejects a nonzero 4301-digit decimal integer, while Ruff accepts; Ruff accepts Unicode-16 identifier U+1C89, outside this oracle's Unicode-15 repertoire. |
| Ranges (3) | Ruff's function range includes decorators rather than beginning at `def`; its direct generator-argument range omits the parentheses; its typed-assignment range omits an ending type comment included by the comparison AST. |
| Literal/value API (3) | Escaped lone surrogates `\ud800` and `\udfff` become replacement U+FFFD; the large-hex case exposes raw spelling rather than a decoded big integer. The last difference is an API representation mismatch, not a demonstrated numeric-value error. |

The tested Tree-sitter rejection families and PEP 701/`:=` f-string call
distinctions worked in Ruff. The compared AST inventory had no differences, but
excluded f-string wrapper/constant representation; neither that result nor the
105 matching cases establishes full AST, fact, diagnostic or Python-version
parity. The probe imposed a 1 MiB input gate, not a hard memory/resource sandbox.
Original-byte decoding/cookies and the complete product resource contract remain
separate work.

**Decision: do not adopt unmodified Ruff 0.12.0 as a drop-in replacement.** Its
syntax coverage makes it a credible candidate for one further bounded experiment,
not permission to relax the AST contract or proceed with G3 integration. The
duplicate-keyword check is a post-argument-construction validator; a dedicated
AST-only parser policy or reviewed upstream patch may be a finite correction.
Blindly suppressing errors or consuming recovered trees is not that policy.

The recommended next checkpoint was one isolated AST-only-policy plus
compatibility-adapter experiment at this same pin. It was subsequently approved
and completed as recorded in the next section, with these intended gates:

1. Prove duplicate-keyword AST acceptance without weakening genuine syntax/version
   rejection or accepting recovery output.
2. Prove bounded source-backed literal handling, including lone surrogates and
   large integers, plus Python-3.12 Unicode/digit admission gates.
3. Prove explicit token/range projections for the three observed span differences
   and test the corrections together against the full probe matrix and focused
   adversarial controls.

Stop if those corrections require broad grammar changes, unsound diagnostic
waivers or an unbounded compatibility layer. Even a successful experiment only
justifies a separate parser-adoption design; G2–G6 and explicit production cutover
approval remain necessary. No public backend/default, product manifest or product
lockfile changed in this evaluation.

Temporary source, archive, probe and reports were removed after processes stopped.
The hashes and aggregate findings above are retained as the evaluation record,
not as a checked-in reproducible harness. Downloaded Cargo registry dependencies
remain in the normal reusable cache, outside project-cache cleanup scope.

#### Bounded AST-only-policy and adapter experiment: completed, not adoption-ready

The separately approved experiment retained the same Ruff **0.12.0** commit,
Python **3.12.11** / Unicode **15.0.0** / decimal limit **4300**, and explicit
**PY312** parser target. No product source, dependency, manifest, schema, backend
identity or default changed. Scratch-only code and generated tables were removed
after the probe processes stopped. This record describes selected synthetic
comparisons, not a retained executable harness or complete fact parity.

**Policy result.** Two scratch parser files, `parser/options.rs` and
`parser/expression.rs`, added a default-false `ast_only` option and guarded the
existing duplicate-keyword check at `validate_arguments`:

```rust
if !self.options.ast_only && !all_arg_names.insert(arg_name) {
    self.add_error(
        ParseErrorType::DuplicateKeywordArgumentError(arg_name.to_string()),
        range,
    );
}
```

This selects a validation policy before emitting the diagnostic; it does not
filter diagnostics after parsing. The default policy retained rejection. Both
ordinary parse errors and the separate unsupported-version errors still had to
be empty before any AST traversal. Repeated keywords around unpacking,
normalization-equivalent names and repeated class-header keywords were admitted
under the experimental policy. Malformed arguments, positional-after-keyword,
invalid generator arguments and later-version syntax controls remained rejected.
This establishes only a narrow duplicate-keyword correction, not comprehensive
AST-only semantics or an upstream-supported API.

**Adapter result.** A Python prototype consumed Ruff AST/token ranges; it did not
use Python AST reparsing or `literal_eval` to obtain candidate values. It proved
finite corrections, not an implemented native Rust compatibility layer:

- Source-backed ordinary/raw/bytes/adjacent literal decoding represented strings
  as integer code-point sequences, preserving lone surrogates separately from
  U+FFFD. Tested escapes included octal, hex, Unicode, unknown escapes, physical
  newline normalization and escaped joins. Named escapes used only three entries:
  `LATIN CAPITAL LETTER A`, `LATIN SMALL LETTER SHARP S` and `NULL`.
- Integer reconstruction selected the source radix and accumulated digits. The
  decimal admission gate counted non-underscore digits and rejected nonzero
  decimal integers beyond 4300 digits; all-zero/base-prefixed forms were separate.
  A deliberate 10000-digit reconstruction cap was not Python compatibility.
- Oracle-generated Unicode-15 raw identifier admission tables had 136323 start
  and 139463 continuation code points. Ruff supplied whole-name normalization;
  semantic-name comparisons covered normalization-sensitive cases. An auxiliary
  3630-entry per-code-point NFKC map was not a whole-string normalizer. Generated
  tables are experiment evidence, not an approved distributable data strategy.
- Balanced token projections corrected definition starts past decorators while
  retaining `async`, selected generator parentheses, and extended applicable
  assignment type-comment ends. Controls included multiline syntax, nested
  grouping, comments, semicolons, UTF-8 and CRLF. These AST byte-range comparisons
  are not all observable product differences: the helper emits line/end-line
  evidence, and has no standalone generator-span fact.

The reconstructed matrix was **169 cases**, not a rerun of the deleted 114-case
suite. Counts below compare uncorrected and combined-policy/adapter behavior on
this new matrix only; they are not a general compatibility percentage.

| Category | Cases | Before: matching / differing | Combined: matching / differing |
| --- | --- | --- | --- |
| Keywords and malformed arguments | 20 | 13 / 7 | 20 / 0 |
| Grammar and version controls | 26 | 26 / 0 | 26 / 0 |
| AST-only versus compile controls | 12 | 11 / 1 | 12 / 0 |
| Integer and digit controls | 17 | 9 / 8 | 15 / 2 |
| Identifier controls | 24 | 19 / 5 | 24 / 0 |
| String/bytes literals | 36 | 27 / 9 | 34 / 2 |
| F-strings | 8 | 8 / 0 | 8 / 0 |
| Ranges/type comments | 26 | 4 / 22 | 24 / 2 |
| **Total** | **169** | **117 / 52** | **163 / 6** |

All cases compared acceptance. Further checks were category-specific: decoded
string/integer values, normalized names, selected statement/generator ranges, or
call/name/named-expression inventories. F-string checks did **not** compare
decoded literal segments, debug-text synthesis, wrapper representation or
constant merging. No synthetic source was executed; compile controls only
distinguished AST acceptance from compilation.

The complete six remaining differences in those selected comparisons were:

| Synthetic case | Python oracle | Combined prototype | Interpretation |
| --- | --- | --- | --- |
| `x='\N{SNOWMAN}'` | Accepts and decodes | Unsupported named escape | Deliberate finite-table omission |
| `x='\N{BOM}'` | Accepts name alias | Unsupported named escape | Deliberate finite-table omission |
| `x=0x` followed by 10001 `f` digits | Accepts | Reconstruction limit | Deliberate prototype cap |
| `x=` followed by 10001 zeros | Accepts | Reconstruction limit | Deliberate prototype cap |
| `# type: int` followed by newline and `x=1` | Rejects | Accepts | Genuine acceptance gap |
| `pass # type: int` | Rejects | Accepts | Genuine acceptance gap |

The type-comment comparison is required: production calls
`ast.parse(..., type_comments=True)`, not the default mode. Extending assignment
end ranges does not enforce valid type-comment placement. The first four cases
show incomplete prototype domains, **not inherent Ruff infeasibility**.

**Unproved obligations.** Rust `String`/`char` cannot preserve lone surrogate code
points. A code-point or explicitly specified alternate representation is feasible,
but the Python prototype does not establish its Rust integration, escaping,
equality, limits or identity serialization. Replacing surrogates with U+FFFD is
lossy, and storing literal backslash escape text is not an equivalent semantic
representation. Full named Unicode data/aliases, native f-string values,
original-byte decoding/cookies, complete diagnostics and fact parity remain
unproved. The prototype's 1 MiB check happened after parsing: it is not a preparse
resource gate or hard memory sandbox. Platform, supported-interpreter and product
resource acceptance remain separate.

**Build and provenance.** The existing Rust **1.89.0** compiler built the isolated
53-upstream-package closure plus probe with `RUSTUP_AUTO_INSTALL=0`,
`--locked --offline --jobs 2`, low-disk profiles and the existing project target.
There were no tool installations, dependency downloads, substitutions or unlocked
Cargo invocations. Initial locked attempts rejected incomplete isolation/lock
reconstruction before compilation. Scratch manifests omitted dev/unused optional
dependencies while preserving selected upstream versions/checksums. The upstream
lock hash matched the preceding evaluation. Only macOS arm64/Rust 1.89 was built;
the declared 1.85 MSRV and other platforms were not verified.

The commit-addressed codeload archive had a different hash from the prior archive;
the initial strict comparison failed and that attempt was cleaned. A subsequent
attempt explicitly recorded the new archive hash and verified the upstream tag
commit and lockfile. The old archive was not retained, so no byte-for-byte archive
equivalence is claimed. Paths/types were checked and symlinks omitted. Recorded
SHA-256 values for the completed experiment were:

| Artifact | SHA-256 |
| --- | --- |
| Commit-addressed source archive | `e4613ad9fe6f7293510586649aab788e10610ab80fe9dd509ba7ac2364413631` |
| Reconstructed probe lockfile | `9e2eb5608266cd32bae6eaef87aea0e41e2c01b82d98ed63e0f5a2c6fcb6cd25` |
| Two-file policy diff | `d21e3afc1871fc478ddcebe04b9bf58c62cea38342cad0ee67a7b7ee8f0c2cdc` |
| Generated Unicode table blob | `4c23e0acc9a15d573a67dd6c29067c70c857b8b3f3e8fd50a27949137528cd62` |
| Canonical 169-case matrix tuples | `fe028289d3cc15e4d3c5de45297dd4aaf814c57163316cf769a98adb6b3ed06a` |
| Adapter | `4ceb82aa7beaa0ff60b726e6ff9fc40072edf65ba9c0e86feef22f908bdcd6fd` |
| Final probe | `cb0a2553908992dcfd823a1c8b147a75bb52ecbe722b63b37f03d6fc5da6a9e5` |
| Final comparison script | `582c080ef4e20b137f3ae7016ee9503e5185966611c9a48d31d0b74dc7be090b` |
| License/MSRV inventory | `b4462af77ed45fb31f9c61807d0640243911dbe0f6dd37a3d5c07a447b9edd23` |

Selected manifest licenses again showed no declared copyleft, not redistribution
clearance. Unicode/RustPython notices, generated-data provenance, unpublished
workspace-crate isolation and policy-patch maintenance remain adoption obligations.
Seventeen selected dependency manifests lacked declared MSRV.

**Stop decision:** retain the production Python helper and close this experiment.
The bounded investigation demonstrated localized corrections but did not establish
a complete native adapter or product parity; do not proceed to parser adoption on
this evidence. An independent read-only architecture review agreed with this
scope. This is neither approval for another parser-shopping/probe loop nor proof
that native compatibility is impossible. Reopening native replacement requires an
explicit product/architecture decision about remaining compatibility and
maintenance costs, not another syntax-slice milestone. G2–G6 remain open.

#### Native Rust continuation design and first foundation batch

The subsequent continuation moves from parser probes to a **parser-independent
implementation of observable helper behavior**. It does not adopt Ruff or reopen
the closed experiment. The existing AST helper remains production; the pinned
Tree-sitter candidate remains a conservative private regression lane. At that
checkpoint Ruff was a possible future frontend. The subsequent selection below
chooses it as the implementation direction, not as a compatible production backend.

##### Reconstruct only observable semantics

The experiment compared more value/range detail than the product necessarily
exposes. Keep its failures as recorded evidence, but scope implementation to the
actual helper contract:

| Input | Observable requirement | Unnecessary reconstruction |
| --- | --- | --- |
| Numeric constants | Validate spelling and applicable decimal admission; render `<literal>` | Arbitrary-precision numeric values or a new integer-digit conversion cap |
| Ordinary string/bytes constants | Validate all input; render `<literal>`; retain only direct text required by existing metadata | Unbounded decoded payloads or general constant evaluation |
| F-strings | Correct admission, `<JoinedStr>` rendering, descendant calls/names and visitor order | Evaluated strings, debug-text synthesis or constant merging that cannot affect facts |
| Literal API metadata | Exact code-point lengths, route text, HTTP whitelist behavior and restricted prefixes | An unrestricted public literal-value API |
| AST/token ranges | Exact emitted fact line/end-line and failure line | Every parser byte range when no fact or diagnostic observes it |

Redaction never excuses skipping validation. Unknown named escapes, malformed
strings, misplaced type comments and unsupported versions still affect admission
even when they produce no retained literal value. Generator grouping can affect
a parameter-default span, so range normalization remains driven by the emitting
fact rather than discarded wholesale.

##### Recommended private architecture

Use **captured bytes → decoding/admission → validated frontend → bounded semantic
projection → helper-order visitor → transactional facts**. Keep this a private
Python language implementation, not a general plugin framework or public AST API.

- Reuse captured-byte digests, deadline/cancellation latching, bounded emitters,
  fact IDs, alias primitives and final sorting/deduplication. Extract reusable
  pieces from CST state machines rather than replacing validated semantics.
- Lower once through a counted frontend walk into owned names, expression
  descriptors, literal summaries, logical spans and explicit child references.
  No parser nodes/lifetimes cross that seam. Bound arena capacity and child edges
  before allocation. Charge semantic interpretation to the same invocation budget
  and deadline; a second semantic pass is not an uncounted parser walk or reparse.
- Separate rendering edges from visitation edges: shape truncation and opaque
  `<NodeKind>` rendering must not hide descendant calls or warnings.
- Preserve helper phases, not source order or runtime execution order. Definitions
  push scope and emit declaration/header metadata first. Python-3.12 function
  generic traversal visits arguments, body, decorators, return annotation and type
  parameters; class traversal visits bases, keywords, body, decorators and type
  parameters. Argument traversal has its own AST field order. Body imports can
  therefore affect later decorator/return-call alias resolution even though
  decorator discovery metadata was already emitted.
- Preserve path-keyed alias scopes, including repeated same-named definitions.
  Imports update the helper's alias map; ordinary assignment invalidation facts
  do not erase it. Final sorting cannot repair a wrong resolution decision.

##### Implemented foundation: text retention and ASCII JSON

`src/analysis/python_native/scalars.rs` is a standard-library-only private module,
independent of parser nodes and frontend cancellation enums. It provides:

- Checked code points from zero through U+10FFFF, including lone surrogates.
- Streaming Python `ensure_ascii=True` string spelling with lowercase escapes,
  bounded output, cancellation and explicit I/O failures.
- Bounded retention of fallible decoded-code-point events using existing route
  (2048) and prefix (128) limits. Length counts Python code points, not UTF-8 bytes
  or UTF-16 units. Overflow drops retained payload but continues consuming events
  so a late invalid/unresolved escape event cannot become successful admission.

These are retention/serialization primitives, **not a literal decoder**. Callers
still own lexical validity, source bounds, named-Unicode lookup and method
normalization. The retention seam is not wired to G3 admission in this batch.

The shared writer is integrated into private fact-ID hashing and signature
metadata. Hashing remains streaming with Python's default spaced identity array;
signature metadata remains compact with sorted keys and its 128-parameter/16384
encoded-byte bounds. The byte cap is applied after ASCII escaping. Import metadata
remains on the existing path and needs coordinated ASCII-writer integration before
Unicode import admission. The new module participates in the private implementation
fingerprint. Existing accepted-source semantics and private policy 15 are retained;
the implementation fingerprint changes, not the public backend identity.

No global `Fact.value` or serde representation changes are needed to support
surrogates in metadata: emit them as ASCII escapes inside the inner JSON text,
then escape that text again for the fact-ID preimage. Do not round-trip the inner
metadata through a Unicode-scalar-only JSON value representation. A non-BMP point
and two explicit surrogate points can have identical ASCII JSON spellings but
different Python lengths; retention eligibility must be decided before encoding.

##### Remaining implementation batches and decisions

1. **Helper-order semantic projection.** The admitted-lane call/scope and
   statement/header portions are implemented below; new header-call admission
   remains separate. Use
   bounded descriptors and explicit
   definition phases, render/visit edges, argument order, imports and calls. Prove
   body-import effects, early discovery metadata, nested owner restoration and
   repeated scope-name behavior using synthetic semantic traces. Integrate an
   already-admitted private lane without expanding acceptance on an assumed-correct
   frontend. Include imports' ASCII serialization and contextual metadata sinks;
   do not approximate Python Unicode uppercase with ASCII-only method conversion.
2. **Reviewed frontend and complete visitor/metadata coverage.** Before any parser
   dependency change, decide the exact revision, unpublished-crate distribution
   mechanism, maintained AST-only policy, closure/notices, MSRV and platforms.
   Reuse the probe findings rather than another shopping round. Then lower all
   headers, expressions, statements, patterns and existing G3 side outputs. Empty
   parse and unsupported-version error channels remain mandatory; no recovery-tree
   publication or diagnostic waivers.
3. **Input and failure closure.** Choose reviewed versioned Unicode data for raw
   identifiers, whole-name NFKC, named escapes/aliases/algorithmic names and the
   necessary uppercase behavior. Record immutable inputs, generator provenance
   and notices; neither the three-name prototype nor per-code-point NFKC mappings
   are sufficient. Enforce `type_comments=True` placement, decoding/cookies/BOM,
   original-byte digests and exact observable failures. Enforce byte limits before
   parsing, then bounded decoded storage/descriptors/events/output under one
   deadline. Cooperative limits remain distinct from process/RSS isolation.
4. **Public migration and acceptance.** Design identity, capabilities, saved-state
   compatibility and interpreter-setting retirement only after G2–G4 close.
   Never invent `pythonVersion`, silently reinterpret a request or upgrade old
   capabilities. Complete existing-engine, adapter, platform/package/resource and
   saved-source acceptance before explicit cutover approval.

The first implementation boundary was the private foundation above; the next
admitted-lane projection batch is recorded below. Parser
adoption, distributable Unicode/codec data and a production native language/version
profile remain explicit decisions. Python 3.12.11 / Unicode 15 / decimal limit
4300 describe the development oracle only; production's Python >=3.12 contract is
unchanged. No new toolchain, Unicode-data retrieval or public backend is approved
by this design. Completing these foundations does not close G2–G6.

##### Foundation validation record

The foundation was validated with existing Rust **1.89.0**, two Cargo jobs/test
threads and `RUSTUP_AUTO_INSTALL=0`; synthetic test scratch used owned disk-backed
directories with cleanup. No new dependency or toolchain was installed.

| Gate | Actual result |
| --- | --- |
| Ordinary library tests | 570 passed; 169 opt-in tests ignored by this invocation, not counted as passing |
| Public API regression targets | `adapter_api`: 18 passed; `engine_api`: 8 passed |
| Explicit existing Python oracle gate | All 162 `analysis::python_parity` tests passed with trusted Python 3.12.11 |
| Scalar-focused checks | Four scalar-module tests and two emitter integration-unit tests passed; the `scalar --include-ignored` filter also passed the new oracle and two existing OpenAPI tests |
| New retained-text oracle | Eight synthetic route cases compared metadata and fact IDs against the trusted helper, including lone surrogates, U+FFFD, controls, non-BMP text and the 2048/2049-point boundary |
| Static checks | Formatting check and all-target Clippy with warnings denied passed; independent read-only review found no blocking issue |
| Package inventory | 117 paths verified, including the new scalar module; no publication |

The new oracle is
`analysis::python_native::facts::tests::scalar_bridge_retained_route_metadata_matches_trusted_ast`.
It requires explicit `IDEOMANCER_TEST_PYTHON` and is **not** selected by the sibling
`analysis::python_parity` filter; run it separately or include ignored tests in the
focused scalar filter. It proves the retained-event → metadata → ID seam, not a
native literal decoder or G3 acceptance.

An initial focused invocation correctly failed without explicit oracle selection;
the oracle fixture's initially invalid native-only hex transport was then fixed.
The first broad library attempt hit the command timeout, not a reported assertion
failure. Its stopped processes were checked and exact owned scratch was removed;
the completed rerun above used an adequate timeout. These interrupted/failed
attempts are not counted as passing validation.

#### Helper-order call and scope projection: admitted lane integrated

The next implementation batch adds private `python_native/projection.rs` and
integrates it behind the existing counted CST collector. There is no new parser,
dependency, backend, public capability or syntax admission. Private policy 15
remains; implementation fingerprint inputs now include the projection module.
The production helper and evidence identity are unchanged.

**Actual integrated behavior:**

- `Projection` owns persistent parent/name-keyed scope identities and import
  aliases, separate from parser nodes. The collector's existing definition stack
  retains lexical ownership and supplies explicit scope IDs. Repeated definitions
  reuse path identities even across class/function kinds. Lookup selects the
  nearest prefix, substitutes once and retains the full target without reclipping;
  assignment/invalidation facts still do not mutate import aliases. Fixed hashed
  keys retain original spellings and fail closed on a collision.
- Import metadata is emitted before applying its absolute-alias updates. Calls
  emit their fact before the dynamic-discovery diagnostic, then replay descendant
  calls. Relative imports and conservative constructor/mutation guards retain
  their existing behavior. Import metadata serialization remains ASCII-only on
  its existing path; this is not Unicode import admission.
- Expression completion builds intrusive call chains from validated child
  topology, independently of renderer edges. Calls visit the callee, positional/
  starred arguments, then keyword/unpacked-keyword values. Dictionaries visit all
  keys before values; conditionals visit test before body/alternate; lambda
  keyword defaults precede positional defaults. Comprehension element/key/value
  traversal precedes generator target/iterable/conditions. Opaque nodes, f-string
  fields and children past render caps remain observable. Splat and await/power
  normalization retain each call once. Unknown nonleaf scheduling forms reject
  rather than silently adopting source order.
- Declaration/signature facts are now tentatively emitted before admitted bodies.
  Their end lines and IDs are finalized after the suite's counted walk, excluding
  trailing comments from definition evidence. Provisional records are never
  published on cancellation, unsupported syntax or resource failure.

At that checkpoint this did **not** complete the entire helper event schedule:
parameter/base/header facts still used their existing call-free lowering path,
and assignment-specific metadata still needed its pre-child phase integration.
The subsequent batch below integrates those admitted-lane phases. Header calls
and decorators remain excluded; their generic visit groups are exercised with
synthetic observations, not newly admitted source. No full parser-neutral AST has
been introduced as unused infrastructure.

**Bounds and failure behavior:** call links live inside already charged expression
entries; `ENTRY_WORK` automatically includes the additional storage. Building
chains visits sibling edges a constant number of times and replay follows each
call link once, with cancellation checks. There is no second CST walk, recovery
parse or render-limit-dependent traversal. Scope maps and target copies are
charged before growth through frontend-independent charge/checkpoint callbacks.
Definition finalization reserves 256 bytes of scratch per active definition and
one additional function-owner symbol copy. Provisional maximum-width end lines
reserve output conservatively before finalization releases unused bytes. Tight
private working/output limits can therefore reject earlier than the preceding
implementation; this is not a change to production hard-limit behavior or a claim
of an allocator/RSS sandbox.

**Validation for this batch:**

- The completed ordinary library gate passed **577 tests**, with 173 opt-in tests
  ignored by that invocation. API regressions passed **18 adapter** and **8 engine**
  tests. Ignored tests are not counted as passing ordinary tests.
- The explicitly selected trusted **Python 3.12.11** gate passed all **165**
  `analysis::python_parity` tests, including the three new `projection_batch` cases:
  cross-family facts/warnings and persistent ownership; captured replay and
  nonexecution past rendering caps; unchanged header/constructor rejection with
  full-file rollback.
- Two separate opt-in native-module oracle tests passed: the existing eight-case
  retained-text metadata/ID seam and a new **18-expression raw call-order**
  comparison against stdlib `ast.NodeVisitor`. The latter deliberately compares
  unsorted traces, not final fact arrays. It covers interleaved argument groups,
  dictionary unpacking, lambda parameter separators, conditional expressions,
  comprehension targets, nested f-string specifications, normalized splats,
  await/power, deep calls and calls beyond rendering caps. Python only parses the
  independent synthetic inputs; no target code is executed or imported.
- Unit traces cover alias overwrite, lexical fallback, one-step substitution,
  full long targets, repeated paths and future definition phases. Bounds/collision
  checks run before allocations; per-file resource failures discard facts and
  warnings. Cancellation was injected at **every fact-ID checkpoint** in a nested
  definition fixture, including provisional/final IDs; only the module baseline
  and deadline status survive.
- Formatting and all-target Clippy with warnings denied passed on existing Rust
  1.89.0 with two jobs and `RUSTUP_AUTO_INSTALL=0`. The exact package inventory
  passed at **119 paths**, including both new source files; nothing was published.
  Independent read-only review identified the initially missing inventory entries,
  which were added before this gate. No blocking code issue was found.

The raw-order test is
`analysis::python_native::facts::expressions::tests::projection_semantic_call_trace_matches_trusted_ast`.
Like the scalar seam test, it is outside the `analysis::python_parity` filter and
must be selected separately; the native-module `--ignored` gate selects both.
These checks establish the integrated admitted lane, not complete G2/G3 traversal,
encoding/failure compatibility, a reviewed frontend replacement or cutover.

#### Statement and header phases: admitted lane integrated

The following batch completes the statement-local and definition-local ordering
work for the **already-admitted private lane**, not all Python headers or G3.
No parser dependency, source-admission rule, production identity, public schema or
default changed. Existing private policy 15 and final fact/diagnostic semantics
remain; the implementation fingerprint changes with the implementation.

**Assignment observations.** Assign/AnnAssign retain unresolved call observations
until their outer statement closes. The collector emits class attribute annotation
metadata first, then the helper's LIFO binding/invalidation metadata, then target
and value child calls in AST order. A call still emits its fact before its dynamic
diagnostic. Annotation-only targets retain their calls even without an RHS.
Capturing child visits does not reverse them with the binding-target stack and
does not pre-resolve aliases. AugAssign/Delete, ordinary expressions/returns and
loop/with/exception expressions keep their existing immediate paths; no statement
buffer crosses a compound suite or nested definition. Constructor/mutation guards
remain in force.

**Definition metadata.** Owned header descriptors retain compact shapes and
logical spans during the existing counted walk. Before the body, functions emit
declaration/signature, positional-only and positional parameters, keyword-only
parameters, vararg, kwarg, then return-annotation metadata. Each parameter emits
parameter → annotation → default, preserving the argument's annotation span and
the default's independent span. Classes emit declaration before base metadata;
keyword values do not become bases. Signature truncation still leaves all
parameter facts observable. Declaration/signature end spans and IDs finalize after
the body as before.

**Owned phase driver.** `HeaderSchedule` replaces the iterator-only integration
with bounded metadata and visit groups, consume-once pre-body/post-body drains,
and a poisoned state after failed drains. Metadata emission is separate from
generic child visitation. Function argument visit groups follow AST field order:
posonlyargs, args, vararg, kwonlyargs, kw_defaults, kwarg, defaults. Bases/keywords
are the class pre-body groups; decorators, function return and type parameters are
post-body groups. Post-body draining occurs before the definition owner is popped.
Synthetic call observations exercise the same drain implementation and prove that
body imports affect later resolution without changing earlier resolution or
leaking nested-owner imports.

Current admitted headers are call-free, so their visit groups remain empty.
Source decorators and calls in defaults, annotations, bases and return annotations
are **not** enabled by this batch. Existing ASCII-name and duplicate-parameter
restrictions remain. In particular, the current name-based metadata role
classification must be replaced by syntactic roles before any future relaxation
of the duplicate-parameter guard; current accepted inputs cannot reach that
ambiguity. This is not full AST-only parameter compatibility.

**Resource and transaction discipline.** Call buffers charge retained text,
initial/geometric capacity and growth transients before storage. Rendering scratch
and expression storage remain charged while observations are captured. Completed
expressions and statements drop owned storage before refunding its charge. Header
descriptors retain only compact annotation/default text after rendering; renderer
and temporary parameter scratch do not accumulate across parameters. Header vector
capacity remains charged until owner disposal. Tight private working limits can
reject earlier because of the new bounded scheduling storage; no production
hard-limit behavior or hard RSS guarantee is implied. Partial drains, capture
failures and cancellation still discard the whole file's facts and warnings.

**Raw trace proof.** An opt-in, test-only indexed observation capture records facts
and warning positions before global sorting/deduplication, then resolves indices
after declaration spans/IDs finalize. Ordinary extraction/tests do not retain a
second fact array. Per-file failure truncates its trace prefix; final cancellation
clears the trace with the facts/warnings. Module baselines are deliberately excluded
from this comparison because native preflight reserves them batch-wide rather
than visiting them sequentially.

The new ignored oracle is
`analysis::python_native::facts::phase_batch::phase_batch_raw_trace_and_complete_facts_match_trusted_helper`.
It instruments the repository's trusted `Extractor.fact` and diagnostic append
methods without replacing visitor semantics. Nine independently synthetic source
cases compare **unsorted fact/warning traces, finalized IDs/spans, complete fact
arrays and diagnostics**. Cases cover chained/destructured/annotation-only writes,
class fields, typed and variadic/async headers, multiline defaults, interleaved
class arguments, persistent/repeated owners, control flow and AugAssign/Delete,
duplicate same-line calls, 129 parameters, captured replay and nonexecution. Only
the trusted helper/stdlib AST parses source; analyzed source is never executed or
imported. The oracle has a Unix guard and sits outside the `python_parity` filter.

**Completed behavioral gates:** **585** ordinary library tests passed, with 174
opt-in tests ignored by that invocation; API regressions passed **18 adapter + 8
engine** tests. All **165** explicitly selected `analysis::python_parity` tests
passed with trusted **Python 3.12.11**. The separate native-module ignored gate
passed all **three** oracle tests: raw expression call order, retained-text
metadata/IDs and the new full raw-phase trace. Ignored tests are not counted as
passing ordinary tests.

Additional unit checks prove 4096 defaults, 4096 annotations and 4096 typed defaults
each fit a 16 MiB working budget without clipping parameter facts; 100 sequential
assignments fit 64 KiB without accumulating statement storage. Accepted/rejected/
accepted file traces keep correct finalized indices, and final sort/output
cancellation leaves only module baselines. Cancellation is injected across all
observed collector/working/ID/sort/final checkpoints in the bounded phase fixture;
working/count/output/warning-limit failures discard trace prefixes as well.
Independent review caught the initial retained-renderer overcharge, the new test's
missing Unix guard and missing inventory entry; those were corrected before the
completed gates. No blocking issue remained in the follow-up review.
Formatting and all-target Clippy with warnings denied passed on existing Rust
1.89.0, using two jobs/test threads and `RUSTUP_AUTO_INSTALL=0`. The package
inventory passed at **120 paths**, including `phase_batch.rs`; nothing was
published. Test scratch was restricted to owned disk-backed run directories.
No cross-platform compiler/toolchain matrix was run for this private slice.

That batch reached the frontend adoption/packaging boundary. The subsequent Ruff
selection below resolves parser choice, not remaining language/encoding
compatibility or broader G3 admission. G2–G6 remain open.

#### Ruff selected: initial dependency distribution decision

**Decision:** use Ruff as the native Python frontend direction; do not reopen
parser shopping without a concrete new blocker. The source baseline is Ruff
**0.12.0**, revision `87f0feb21a1af67195bebf94d811770393f9f447`, with the
previously demonstrated default-false AST-only duplicate-keyword policy. This
selection authorizes the adoption direction, not acceptance of recovery trees,
weaker compatibility, publication or a production switch. Both parser diagnostic
channels must remain empty before lowering.

At that checkpoint root integration had a distribution decision to resolve. The selected Ruff
parser/AST workspace crates are unpublished. Ideomancer's current release gates
require a standalone Cargo source package and independently resolved consumer:

- `check_package_archive.py::dependency_tables` supports registry dependency
  manifests and rejects path/git/workspace rewriting.
- `check_package_consumer.py::validate_package` requires normalized, registry-only
  dependencies without a workspace/patch/replace escape hatch.
- CI checks package inventory, builds the package, verifies archive/extracted
  bytes and exercises an independent package consumer.

A private Rust module does not make its Cargo dependencies private to packaging.
Exact-revision Git dependencies would support checkout development, but do not
satisfy this release contract. Adding registry version fields does not make
unpublished crates available. Local path crates likewise do not become usable
dependencies merely by placing their source inside the root package's include
boundary. Optional dependency flags do not remove these distribution obligations.

**Recommended route:** preserve the existing package contract and prepare a minimal,
maintained, separately publishable Ruff-derived crate closure, with selected source
provenance, self-contained manifests, complete applicable notices and an auditable
policy patch. Registry publication, remote fork creation and ownership/naming
decisions need separate authorization. Root dependencies must not reference
invented/unpublished registry versions while that work is pending.

An independent non-publishable development package containing reviewed selected
Ruff sources could bridge the implementation work, but must stay outside the root
dependency graph and source package. It would be a maintained development
integration—not proof of root-package adoption or a replacement for distribution
acceptance. Alternatively, an explicitly approved Git-only distribution contract
could permit pinned fork dependencies, but would require deliberate changes to
the existing release/consumer gates rather than silently disabling them.

No root manifest, dependency lockfile, package gate or production dispatch changed
when recording that initial decision. The subsequent personal/company delivery
decision below removes registry compatibility as an adoption requirement. No
further parser comparison is required.

#### Ruff private frontend and Git-source delivery: implementation boundary

**Historical first-integration checkpoint:** the delivery model and five-crate pin
remain current, but the byte count, two-file patch and encoding/admission profile
recorded here predate frontend-2/facts-4. Use the facts-6 batch section below for
the four-file patch, current inventory and finite compatibility profile.

**Approved delivery decision:** personal/company consumers use pinned Git source
or approved mirrors; registry/crates.io publication is not a requirement. Preserve
reproducible source/consumer builds and notices, not a registry-only restriction.
The root manifest is now `publish = false`. A maintained, selected-source Ruff
snapshot travels inside Ideomancer's checkout via local path dependencies, so no
remote fork, submodule, separately published Ruff crate family or invented registry
version is needed. Source selection remains generic; this decision does not admit
private fixtures, company paths or consumer-specific interpretation into the engine.

**Maintained source:** `vendor/ruff` contains five unpublished Ruff workspace crates
at **0.12.0**, revision `87f0feb21a1af67195bebf94d811770393f9f447`:
parser, AST, trivia, source-file and text-size libraries. The selected production
source comprises **65 files / 1629332 pristine bytes**. The complete maintained
snapshot is **85 files / 1878654 bytes**, including adapted manifests, original
upstream manifests/lockfile, complete upstream license/RustPython attribution,
applicable pinned Unicode-name notices, the separate two-file policy patch and
closed provenance. It does not contain a nested Git repository, unused Ruff crates,
fixtures, parser test resources, registry implementation sources or tool binaries.

The commit-addressed archive SHA-256 is
`e4613ad9fe6f7293510586649aab788e10610ab80fe9dd509ba7ac2364413631`;
upstream lockfile SHA-256 is
`ccc843838d16f203d18a3410dd120e72e750a89ee42164145ae9cbf181fb6f47`.
`check_ruff_vendor.py` anchors the committed provenance digest, checks exact file
membership/types/hashes and self-contained manifest edges, and reverses the narrow
policy patch to verify pristine source hashes. This is reviewed-source integrity,
not a remote signature or a hostile same-user sandbox. See the maintained
[snapshot instructions](../vendor/ruff/README.md).

Root dependencies select parser/AST/text-size; sibling path dependencies supply
the five-crate closure. Optional upstream features, auto targets and vendored
library tests/doctests are disabled. Required generated AST source is retained;
no regeneration or new toolchain was needed. Only the two reviewed parser source
files differ from the selected upstream source. Product formatting and Clippy are
scoped to the product, not used to rewrite third-party code.

**Merged dependency graph:** the product lockfile adds **35 packages**—five local
Ruff crates and 30 registry packages—while retaining every previously locked
product version. Shared compatible requirements use existing product selections;
unshared direct Ruff requirements use reviewed exact pins. The former isolated
probe lock is provenance, not the merged product lock. `cargo fetch --locked
--offline` found the selected inputs already cached; no new tools were installed.
Only existing Rust **1.89.0 on macOS arm64** has been locally exercised for this
adoption slice. The new registry metadata declares no MSRV above 1.73; thirteen
new registry packages omit MSRV, and local Ruff manifests declare 1.85. Successful
1.89 compilation is not a proof of undeclared/lower MSRVs or other platforms.

The actual merged `unicode-ident 1.0.24` uses Unicode **17.0.0**; pinned
`unicode_names2 1.3.0` and `unicode-normalization 0.1.24` use Unicode **16** data.
Setting `PY312` does not turn these into Python 3.12's Unicode 15 baseline. Names,
identifiers and normalization need explicit versioned admission before parity.
Selected manifest/source-notice review found no declared copyleft requirement;
this is scoped screening, not complete source/binary legal clearance. Binary
bundles still need applicable target/feature/toolchain notice review.

**Private frontend:** `src/analysis/python_ruff.rs::parse_batch` consumes captured
`Source.bytes`, ignores legacy hex transport and never reopens source paths or
probes Python. It selects `PY312` and the explicit default-false AST-only policy
override. The override removes only duplicate-keyword validation; it is not a
claim that every compile-time check now matches `ast.parse`.

Both ordinary and unsupported-version diagnostic channels determine classification.
Errored/recovered trees are never consumed. This first integration does not expose
AST nodes at all: reports contain only source identity, structured admission/syntax
classification, capped diagnostic counts/first-error lines and lexical counters.
No source excerpts, decoded literals, raw parser messages, facts or Evidence are
published. `AcceptedByRuff` is deliberately distinct from CPython compatibility;
all CPython/Unicode/digit/type-comment/encoding/normalization/fact capabilities
remain explicitly **Pending**.

Admission enforces 5000 files, 1 MiB/file, 16 MiB total and 4096-byte path/module
metadata bounds. Original-byte hashes include any BOM. Effective cookies and
invalid UTF-8 remain explicit unsupported-encoding outcomes. Before parsing, a
lexical pass caps tokens at 200000 and delimiter/indent/string nesting at 256;
strings/comments are not scanned as punctuation. Escaped worst-case report storage
is checked against 1 MiB before parsing. Cancellation latches across the batch and
final expiry invalidates earlier accepted classifications. Ruff lacks a parser
interrupt hook: the shared 60-second deadline is checked before/after parsing and
before publication. Lexical caps do not prove every parser-recursion bound, and
cooperative checks are not a hard timeout, stack/RSS limit or OS sandbox.

**Active delivery validation:** `docs/source-files.txt` replaces the historical
crate inventory as the reviewed Git-source boundary. `check_checkout.py` checks
bounded no-follow source reads, exact selected paths and a content-based snapshot
hash. The source closure includes the five Ruff crates but excludes `.git`,
`.ideomancer`, `.agents`, build trees and private material. This boundary is not a
secrets scanner or approval of every file/history entry in the Git repository.

`check_checkout_consumer.py` exports only reviewed bytes to owned disk-backed
scratch, resolves a fresh independent consumer lockfile, and performs a locked
public-API build/test with local Ruff dependencies inside that export. It reuses
the trusted project target, low-disk profiles and two jobs/test threads. It verifies
source bytes before/after, requires the ABI bridge pin, and finalizes/verifies owned
process groups even when the parent succeeds. Unverified shutdown preserves only
the owned run and reports its path/reason/manual cleanup plan rather than deleting
live files. This is an exported source-snapshot check, not attestation of an actual
uncommitted Git revision, remote availability or an OS sandbox.

CI now checks checkout/vendor integrity and the independent checkout consumer,
plus explicit trusted-Python oracle lanes. Historical package/archive helpers and
their tests remain available for old artifacts but are not active delivery gates;
they are not weakened to pretend an unpublished dependency is registry-published.
Use `cargo fmt --package ideomancer --check` and root Clippy `--all-targets
--no-deps -- -D warnings`. The preserved vendor source currently emits 19
non-fatal Rust-1.89 lifetime-style compiler warnings; these are not hidden by
rewriting upstream source or relaxing warnings in product code.

The focused Ruff suite passed **nine ordinary tests plus one ignored acceptance
oracle** with trusted Python 3.12.11. Its 16 synthetic oracle cases explicitly
assert four tracked acceptance differences (decimal limit, Unicode admission and
two type-comment placements), not full AST/fact parity. **180 Python script
tests** passed, including vendor tamper/patch/manifest checks, source-boundary
checks and successful-parent/lingering-child cleanup regression coverage.

Completed local adoption gates on existing Rust **1.89.0/macOS arm64** were:

| Gate | Actual result |
| --- | --- |
| Ordinary library suite | 594 passed; 175 opt-in tests ignored by that invocation, not counted as passing |
| API regression targets | 18 adapter and 8 engine tests passed |
| Explicit Python oracle gate | 169 passed: 165 existing captured-source parity cases, three native semantic/scalar oracles and the new Ruff acceptance oracle |
| Product static checks | Scoped formatting and all-target `--no-deps` Clippy with product warnings denied passed; the 19 preserved vendor compiler warnings remain visible |
| Source/vendor integrity | 211 reviewed source paths verified; 85 vendor files and exact policy-patch reversal verified |
| Fresh checkout consumer | Offline fresh lock resolution, locked compile and the independent native JS/TS public-API test passed from an isolated reviewed source export |

The consumer verified the five local Ruff crates stayed inside its exported tree,
the ABI bridge resolved to 0.1.5, and source/provenance were unchanged during its
run. It required neither a published Ruff crate nor a separate Ruff Git checkout.
This is build/distribution and existing public-API acceptance, not Ruff fact parity
or proof that production Python no longer needs its helper.

Fresh consumer resolution intentionally differed from the product lock, including
`clap 4.6.7`, `serde_json 1.0.154` and `unicode-ident 1.0.27`, while the product
kept its existing versions. The private frontend's root-lock hash is therefore a
development fingerprint, not proof of an arbitrary downstream compiled dependency
closure. A closed production semantic fingerprint and reviewed versioned admission
remain G4/G5 work. Consumers should pin their own lockfiles for reproducibility.
The consumer gate attests its tested source snapshot, not a real Git commit;
documentation-only result recording afterward does not mint an immutable release
pin. Hosted matrix results, a complete binary notice audit and production cutover
are not claimed by these local gates.

**Boundary at that checkpoint:** Ruff fact lowering into the helper-order semantic
core was next; the following batch implements its first cross-family lane. The
Tree-sitter fact candidate remains a reference/regression path, not an additional
replacement direction. Literal/name/version/comment/encoding/failure
and resource compatibility still require closure. Production remains the trusted
`python-static-8-generic-facts` helper; public requests, metadata, saved-state
capabilities and defaults are unchanged. G2–G6 and explicit cutover approval remain
open. No remote fork, commit, push, toolchain installation or publication is implied.

#### First Ruff generic fact candidate: cross-family lane

Private `python_ruff::extract_fact_candidates` now consumes Ruff's validated AST
and emits generic facts/IDs/diagnostics for admitted whole files. Its distinct
policy is **`python-ruff-private-facts-1`**. It remains a private candidate with
`cpython_compatibility: Pending`, not a public backend, Evidence producer or
cutover. Production `python-static-8-generic-facts`, interpreter selection,
snapshots, public request types and defaults are unchanged.

The parse-only entry point remains classification-only. Both entry points share
an internal borrowed-AST consumer seam: each eligible file is parsed **once**;
the consumer runs only after both ordinary and unsupported-version channels are
empty. AST references stay inside that callback. A semantic or compatibility
rejection can coexist with `AcceptedByRuff` syntax classification. No recovered
tree is consumed, and the fact entry point does not call the parse-only entry
point followed by another parse. Consume/drop/final checkpoints share the same
latched invocation deadline.

**Shared observable output:** `analysis/python_facts.rs` owns fact/warning record
types, canonical ASCII fact IDs, compact signatures/import metadata, bounded
measurement and a transactional sink. Projection and scalar modules now have one
private owner under `analysis`; both frontends import the same module instances.
The Tree-sitter reference delegates its record types, IDs and signature writer to
the shared core, but retains its CST collector, import serializer and specialized
finalization. It was not rewritten into a universal parser framework.

The shared sink reserves all module baselines before file marks, restores active
file facts/warnings/output accounting on failure, retains container high-water
charges, and bounds committed output separately from pending working storage.
Finalization deduplicates fact IDs only, sorts the helper's full fact tuple, and
sorts diagnostics without removing duplicate observations. Hash preimages retain
Python's spaced `ensure_ascii=True` array spelling and path-only module IDs.
Independent fixed vectors test the shared encoding rather than merely comparing
two native users of the same implementation.

**Admitted together in this lane:**

- Sync/async functions, classes, methods and nested definitions; full ordinary
  signatures, parameter annotations/defaults, return annotations, bases and class
  keyword visitation. Non-decorator header calls are now admitted.
- Plain/from/relative/star/future imports and the helper's persistent path-keyed
  alias lookup, nearest-prefix selection and one-step full-target substitution.
- Calls/arbitrary callees/unpacking, containers, operators/subscripts/slices,
  lambdas, comprehensions/generators, await/yield/walrus and f-string descendants.
- Assign/AnnAssign, chained and destructured targets, class attribute annotations,
  eligible bindings and invalidations. Annotation-only statements do not write.
- Ordinary expression/return/pass/jumps, conditions/loops, sync/async for/with,
  try/try-star, raise/assert, global/nonlocal, AugAssign and Delete. AST-positive
  but non-compilable contexts are not rejected by a new runtime-context validator.

Definitions push their owner before metadata. Parameter metadata follows
positional → keyword-only → vararg → kwarg order, each parameter preceding its
annotation/default. Generic header visits use separate typed `HeaderGroup` slots
in AST field order. Calls remain unresolved until replay, so body imports can
change later return-annotation resolution without affecting earlier argument
visits. Parameter roles come from syntax fields, not comparisons of repeated
parameter names. Class/function reuse of the same path preserves helper aliases.

Expression normalization produces bounded owned redacted descriptors once, with
separate rendering and visitation edges. Dictionaries visit keys before values,
conditionals test before branches, lambda keyword defaults before positional
defaults, and comprehensions their element/key/value before generators. Opaque
shapes and rendering caps never hide calls. Rendering retains the helper's
80-node/depth-12/first-16/per-node-512-byte rules, including independent callee
rendering. UTF-8 clipping stays saturated after truncation rather than filling a
discarded partial code point with later punctuation. No decoded literal payloads
or arbitrary-precision numeric values are reconstructed.

A universal-newline line index maps byte spans without rescanning the entire file
per fact. A token-balanced parentheses index corrects observable generator spans,
including multiline parameter defaults and grouped generator elements. Original
captured-byte digests still include BOM; source paths are never reopened and legacy
hex transport is ignored by native lowering.

**Finite whole-file exclusions, not false completion:** decorators, match/patterns,
type aliases and definition type parameters are not yet lowered. Framework
constructor/include/mutation triggers remain guarded until their G3 side outputs
exist, including triggers hidden in headers, opaque expressions or past clipping.
Alias-dependent constructor checks occur at replay under the current owner.
Ordinary body calls such as `client.get(...)` are not treated as route decorators.

Postparse/pre-lowering checks use Ruff's already-produced token stream, without a
new vendor patch: reject raw non-ASCII identifier spellings, all recognized type
comments including `ignore`, active named Unicode text escapes, and nonzero
decimal integers above 4300 digits excluding underscores. Raw strings, bytes and
escaped-backslash spellings are distinguished from active named escapes; Unicode
comments/redacted strings remain admissible. Zero-only, base-prefixed, float and
imaginary numbers retain their separate admission rules. Effective cookies and
invalid UTF-8 remain explicit unsupported encoding. These exclusions are private
`UnsupportedSyntax`/encoding outcomes, not manufactured CPython parse diagnostics.
They do not change production's Python >=3.12 language/encoding contract.

**Resources and transactions:** semantic bounds are 200000 nodes, depth 256 and
400000 bookkeeping/visitation edges, with 100000 facts/warnings, 32 MiB pending
working storage and 32 MiB complete encoded output. Frontend byte/lexical/report
gates still apply. Normalization arenas drop at expression/assignment/header
boundaries; statement refunds preserve persistent scope/alias and owner-capacity
charges, including alias replacements that shrink storage. Call resolution and
rendered temporaries are refunded after buffers drop. Finalization failure or
invocation cancellation keeps only module baselines and clears warnings; ordinary
file failure does not discard earlier accepted sibling files. Parsing and recursive
AST destruction remain uninterruptible; these are not hard stack/RSS/timeout
guarantees. Tight private resource limits can reject otherwise valid sources.

The new Ruff oracle is
`analysis::python_ruff::facts::ruff_facts_batch::ruff_facts_complete_arrays_and_raw_trace_match_trusted_ast_oracle`.
It is selected by the existing `analysis::python_ruff -- --ignored` gate, not the
Tree-sitter `analysis::python_parity` filter. **51 independently synthetic cases**
compare complete facts, IDs, encoded metadata and diagnostics **and** unsorted raw
fact/warning observations against the trusted helper. They cover header/body alias
effects, duplicate AST-only parameter/keyword forms, variadics/multiline generator
spans, assignments, exceptions/control flow, closures/comprehensions/f-strings,
render clipping, repeated scopes and Unicode source metadata. Recognized type
comments and near-miss comments have separate controls. Only the trusted helper
and stdlib AST parse synthetic inputs; target code is never executed or imported.

Local completed regression gates on existing Rust 1.89.0/macOS arm64 include
**618 ordinary library tests**, **18 adapter + 8 engine** tests and **170 explicitly
selected Python oracle tests** with trusted Python 3.12.11. The latter includes
the existing 165 comparison gates, three native semantic/scalar oracles, the Ruff
acceptance gate and the new 51-case complete-output/raw-order gate. The ordinary
invocation ignored 176 opt-in tests; those are not counted as ordinary passes.
The new oracle count is not a coverage percentage or a claim that every old
Tree-sitter case now runs against Ruff. Eight shared-core unit vectors pass as
part of the library suite. Independent read-only review found no blocking issue.

The candidate also tests one parse/zero callbacks on either error channel,
whole-file rollback for late exclusions, captured replay and input reordering,
redaction, hidden framework triggers, semantic depth limits and every named
cancellation stage. Working-region regressions include 1000 ordinary calls,
2000 calls through a 30-KiB alias both inside/across statements, alias shrinkage
and nested owner/header lifetimes. These protect bounded live state rather than
retaining renderer/normalization charges for the entire file.

Formatting and product all-target `--no-deps` Clippy with warnings denied passed;
the same 19 preserved vendor lifetime-style warnings remain non-fatal. **180**
Python script tests passed, and integrity checks verified **215** reviewed source
paths plus the unchanged **85-file** Ruff snapshot and exact policy patch. The
independent exported-checkout consumer also passed fresh offline resolution,
locked compilation and its native JS/TS public-API test. That gate verifies source
containment/build reuse, not Ruff production dispatch or an actual Git revision.
The vendor snapshot, dependency manifest and product lockfile were not changed by
this lowering batch. No additional toolchain or package build was required.

**Boundary at that checkpoint:** structural patterns, type aliases/type parameters
were next; the following batch implements that lowering. Decorator/discovery/
framework metadata and versioned literal/identifier, comment/encoding/failure
compatibility remain. G2–G6 are still open. Production promotion, public native
identity and an actual downstream compiled dependency fingerprint remain separate
decisions.

#### Ruff structural inventory: private facts-2

Private policy **`python-ruff-private-facts-2`** extends the existing lane with
match statements/all eight pattern variants, type aliases and generic function,
async-function and class declarations. It adds traversal rules, not new capture,
pattern, type-alias or type-parameter fact kinds. Shared output, parser invocation,
vendor source, dependencies, public API and production dispatch are unchanged.
The parse-only policy name and Tree-sitter private policy 15 remain unchanged;
development source fingerprints still advance with their implementation inputs.

**Match semantics:** visit subject, then each case's pattern, optional guard and
body. Mapping patterns visit all keys before patterns; class patterns visit the
class expression, positional patterns and keyword patterns. Sequence, OR, AS,
value, singleton and star forms use their explicit child rules. Capture/rest/star
names and keyword attribute names are strings, not bindings, alias mutations or
new scopes. `Record(...)`, `FastAPI(...)` and `APIRouter(...)` class-pattern syntax
does not invent a constructor call. Actual calls in formatted value patterns,
guards and bodies retain all existing observations and G3 guards. Case-body
imports can affect later cases and later statements, as in the helper.

AST-only duplicate captures, unequal OR captures, multiple stars, duplicate class
keyword attributes/mapping keys and irrefutable alternatives/cases are exercised
without invoking compile-time validation or executing source. No stronger runtime
activation, match exhaustiveness or binding claim is inferred from them.

**Type declaration semantics:** type aliases visit name → type parameters → value
under the existing owner. They bypass assignment handling: no binding,
invalidation, class attribute annotation, import-alias update or annotation scope
is manufactured. Reusing an imported name as an alias/type-parameter name does not
erase the helper's import alias.

For type parameters, TypeVar bounds—including tuple constraints—supply ordinary
expression visits; Python-3.12 TypeVarTuple/ParamSpec names have no bound children.
Function/class bounds are normalized into unresolved observation-only
`HeaderGroup::TypeParameters` visits before the body. They replay after the body
and, for functions, after return-annotation visits, before owner removal. A small
adapter helper captures these visits without rendering/staging an unused root
shape. Body-import and repeated-owner alias effects therefore occur at replay,
not at source capture. Ordinary signature/parameter metadata is unchanged, with
no type-bound annotation facts added.

**Two demonstrated pinned-parser gaps remain explicit:**

1. Bare formatted-string mapping keys such as
   `case {f'{key()}': capture}` are AST-positive in trusted Python 3.12.11 but fail
   Ruff's ordinary syntax channel. The callback count is zero, the fact candidate
   reports `SyntaxError`, and only its module baseline survives. Separate accepted
   adjacent-string-prefix fixtures establish mapping traversal order; live source
   is never rewritten into that spelling and recovery/error channels are not waived.
   This is G4 compatibility work, not proof of full pattern syntax compatibility.
2. The pin emits a version error for a TypeVar default but omits that check in
   TypeVarTuple/ParamSpec branches. Under `PY312`, those later-version defaults can
   reach a callback with both channels empty. The fact walker explicitly rejects
   **every present default for all three variants**, in aliases/functions/classes,
   before visiting any default. This is a private `UnsupportedSyntax` compatibility
   exclusion, not a fabricated parser error or a vendor patch. The acceptance
   oracle now records these two extra differences in its 18-case suite.

Decorators and missing framework side outputs remain guarded. Raw non-ASCII
identifiers, recognized type comments, active named Unicode escapes, effective
encoding cookies and the remaining finite compatibility boundaries are unchanged.
`cpython_compatibility` remains `Pending`; G2–G6 and cutover approval stay open.

Cases, patterns, class-pattern argument/keyword wrappers, type-parameter lists and
items charge semantic nodes/depth/edges even when fact-neutral. Every loop retains
bounded checks, expressions use scoped arenas, and only owned scheduled call
observations survive across a definition body. Low-budget deep/wide neutral-tree
tests discard only the failed file while preserving accepted siblings. Hidden G3
triggers beyond rendering caps, alias-dependent bound guards in both directions,
many aliases/generic bounds and nested live schedules exercise existing rollback
and working-region accounting. Parsing/destruction remain uninterruptible; no hard
stack/RSS/timeout guarantee is claimed.

**Completed local gates for this batch:** all **31 focused Ruff tests** passed;
the complete-output/raw-order oracle preserved its previous 51 cases and added
26 structural cases for **77 matching cases** (75 source fixtures plus two
metadata/BOM cases), with the bare mapping-key gap checked separately and excluded
from the matching count. The broader gates passed **622 ordinary library tests**
(176 opt-in tests ignored by that invocation), **18 adapter + 8 engine** tests,
and **170 explicitly selected Python oracle tests**, using trusted Python 3.12.11.
The latter includes the existing 165 reference comparisons, three native semantic/
scalar gates, Ruff acceptance and Ruff complete-output/raw-order gates. Formatting
and product Clippy with warnings denied passed; the same 19 preserved vendor
lifetime-style warnings remain visible. Existing Rust 1.89.0/macOS arm64 was used;
these counts are not full Python coverage or hosted-platform acceptance.
All **180** Python script tests passed. Source integrity verified the same **215**
reviewed paths, vendor verification confirmed the unchanged **85-file** snapshot,
and the independent exported-checkout consumer passed fresh offline resolution,
locked compilation and its native JS/TS public-API regression. No source paths
were added or renamed; manifests, dependency locks and vendor patches are unchanged.
The consumer attests its reviewed source snapshot, not an immutable Git commit or
Ruff production fact dispatch.

**Boundary at that checkpoint:** decorator/discovery/framework side outputs and
their bounded literal retention were next; the following batch implements them.
Versioned identifier/literal, type-comment/encoding, parser-gap and failure/resource
compatibility still precede public integration. Source selection and Git-based
delivery do not require another parser-selection exercise or registry publication.

#### Historical checkpoint: decorator/discovery/framework outputs, private facts-3

Private policy **`python-ruff-private-facts-3`** adds the helper's existing G3
outputs: decorator facts, CLI/job entrypoint candidates, API routes/diagnostics,
FastAPI hosts, router prefixes, includes and mutations. Their previous whole-file
guards are removed only alongside these implementations. These are the existing
public framework syntax contracts—not new organization-specific inference or
proof of runtime activation. Production helper/identity/defaults, public request
types and saved-state capabilities are unchanged; compatibility stays `Pending`.

**Literal correctness and privacy.** Ordinary/u/raw/triple/adjacent text literals
normalize into source-part ranges/flags, not decoded payloads. Retained values are
decoded from the same BOM-stripped captured source that Ruff parsed. Ruff's decoded
string values are never used for metadata, because they replace lone surrogates.
`python_ruff/literals.rs` streams checked code points, preserving separate surrogate
escapes, unknown escapes, physical newline normalization and part boundaries.
No arbitrary constant evaluation, full bigint conversion or Unicode-name database
is introduced. Active named Unicode escapes remain a whole-file exclusion.

Only the existing bounded contracts retain text: direct route paths at most 2048
Python code points, restricted router prefixes at most 128, and whitelist method
recognition. Invalid/nonconstant/bytes/f-string values yield the helper's dynamic
or unsupported metadata, not invented literal values. Oversize text discards its
payload while continuing decoding/cancellation checks; actual decoder errors fail
closed rather than becoming silently valid metadata. Tags, options, descriptions,
configuration and unrelated string bodies remain redacted.

Closed typed writers emit compact sorted ASCII metadata directly from code points.
Lone surrogates become inner JSON escapes stored in an ordinary Rust `String`,
which the existing fact-ID writer escapes again. No `serde_json::Value` or Rust
Unicode-scalar string round-trip alters retained code points. One non-BMP point
and two explicit surrogate points can encode identically but cross different
Python-length limits; eligibility is decided before encoding.

**Methods and prefixes.** A fixed seven-byte method recognizer implements the
Python-3.12/Unicode-15 uppercase preimages relevant to the eight HTTP words. An
exhaustive trusted stdlib oracle checked every code point through U+10FFFF,
including surrogates: the only relevant non-ASCII mappings are U+0131 → `I`,
U+017F → `S`, and U+FB05/U+FB06 → `ST`; there are no empty uppercase mappings.
ASCII uppercase plus these four property facts exactly determine whitelist
membership without Rust case folding/NFKC or unbounded decoded method strings.
The added ignored oracle independently checks this profile; it does not establish
general Unicode identifier/name compatibility. Applicable Unicode terms remain
in the existing notice collection.

Prefixes use the helper's complete ASCII grammar `(?:/[a-z0-9_{}-]+){1,8}` and
128-point limit. Braces are allowed characters, not a new template parser. Explicit
empty prefixes are invalid; missing include prefixes use the helper's valid empty
default. Missing APIRouter prefixes instead remain unknown/unsupported metadata.

**Metadata and visitation order.** Definitions push their owner and emit the
declaration, decorator facts and function discovery before signature/base metadata.
Class decorators deliberately emit no entrypoint/API discovery. Token-indexed
`def`/`async`/`class` starts correct Ruff's decorator-inclusive declaration ranges,
without regex, reparse or source rewriting. Decorator expressions retain their
own spans excluding `@`; generic calls replay after the body and before return/
type-bound calls. Body imports therefore affect delayed decorator diagnostics but
do not retroactively change early route metadata.

Path selection counts explicit `path=` keywords plus the first positional value;
it requires exactly one bounded empty/slash-prefixed direct text value. Extra
positional arguments after the first do not erase an otherwise valid path. Called
verb decorators use their fixed method; `route`/`api_route` require one literal
list/tuple/set of whitelist strings. Empty containers are valid, methods deduplicate
and sort, and invalid method collections do not retain partial valid subsets.
Path diagnostics precede methods diagnostics, then the route fact. Ordinary body
`client.get(...)` calls remain ordinary calls.

Assign/AnnAssign metadata follows helper order: class annotation, all direct target
mutations, host/prefix metadata, reverse-target bindings, then generic children.
Only direct name targets within the existing limit receive constructor host/prefix
facts. Rendered constructor aliases resolve once at the nearest owner prefix, with
terminal fallback only when no alias exists. Constructor calls elsewhere are
ordinary calls, not newly inferred assignments. Invalid/dynamic prefixes, extra
arguments/keywords and unpacking preserve the helper's null/`supported:false`
metadata instead of excluding the file.

Shared call observations now have one optional generic preceding fact, not a
framework-specific field or an arbitrary event framework. Include metadata is
captured as charged owned ASCII and emitted at replay immediately before its call
under the replay-time owner; no arena ID/AST reference or captured owner survives.
This works inside decorators, defaults, return annotations and type bounds.
The Tree-sitter reference supplies `None` and keeps its conservative admission.

Mutations inspect only direct attribute targets and the helper's clipped rendered
shape. Delete emits **all target mutations before any target child calls**.
A parenthesized tuple is one target and receives no recursively inferred mutation
facts, while its descendant calls remain. Tests distinguish both forms. Same-line
identical include facts deduplicate only at finalization; raw occurrence traces
still preserve each scheduled visit.

**Resource discipline.** Source-part/keyword-index slots, capped literal boxes,
metadata encoders and preceding-fact capacities are charged before growth. The
existing call allowance is not assumed to cover extra metadata. Storage drops
before refunds; live header preludes remain owned across bodies. A 600-call body
with decorator/default/return/type-bound includes exercises bounded repetition,
distinct raw/final duplicate counts, low working limits and cancellation. The
one-parse/both-channel gate, baseline/file transactions and parser/destruction
limitations remain unchanged. This is not a hard stack/RSS/timeout sandbox.

The full oracle retains all 77 previous matching cases and adds 40 G3 cases:
**117 complete fact/ID/diagnostic arrays and 117 duplicate-preserving raw traces**
match the trusted helper. Cases include Unicode casing, literal escapes,
surrogate/length boundaries, dynamic diagnostics, duplicate keywords/unpacks,
multiline/async/BOM declarations, class discovery exclusions, alias fallback,
prefix/include support flags and hidden/clipped descendants. The independent
uppercase-property oracle also passed with trusted Python 3.12.11/Unicode 15.

Completed local regression gates passed **633 ordinary library tests** (177
opt-in tests ignored by that invocation), **18 adapter + 8 engine** tests and
**171 explicitly selected Python oracle tests**. The last count includes the
existing reference/native gates, Ruff acceptance, the 117-case matching gate and
the new exhaustive uppercase-property gate. Product formatting/Clippy with warnings
denied passed; the same 19 preserved vendor lifetime-style warnings remain visible.
Existing Rust 1.89.0/macOS arm64 was used; hosted matrix/production cutover are not
claimed. Review caught an incorrect tuple-delete fixture, and execution caught a
raw-versus-deduplicated include-count expectation; tests now assert both semantics.
All **180** Python script tests passed, source integrity verified **217** reviewed
paths, and vendor verification confirmed the unchanged **85-file** snapshot and
two-file policy patch. The independent exported-checkout consumer passed fresh
offline lock resolution, locked compilation and its native JS/TS public-API test.
That is source/build regression acceptance, not Ruff production dispatch or an
immutable Git revision. No vendor, dependency-manifest or lockfile change was
required. New source paths are the private literal and metadata modules only.

**Remaining at the historical facts-3 checkpoint:** raw Unicode identifier/name admission, active named escapes,
recognized type comments, encoding-cookie support, exact failures/diagnostics and
parser/resource gaps still need G4 closure. Type-parameter defaults retain their
all-variant PY312 guard and bare f-string mapping keys retain their explicit parser
gap. G3 output implementation is no longer merely planned, but full replacement
parity and G2/G4–G6 acceptance remain separate. No public native selector, saved
evidence upgrade or fabricated interpreter version is introduced here.

<a id="current-ruff-compatibility-batch-frontend-2--private-facts-4"></a>

#### Historical Ruff compatibility batch: frontend-2 / private facts-4

##### Overview

This historical checkpoint, subsequently user-committed at `523cd7b`, advanced the private syntax policy to
`python-ruff-private-2-py312-versioned-compatibility` and fact policy to
`python-ruff-private-facts-4`, at the same Ruff 0.12.0 revision
`87f0feb21a1af67195bebf94d811770393f9f447`. Historical facts-3 results above remain
valid for that checkpoint, not for final validation of this batch. CPython
compatibility remains `Pending`; production `python-static-8-generic-facts`, its
trusted Python >=3.12 AST helper, public API, evidence and default are unchanged.

##### API / Interfaces

`parse_batch` remains classification-only. `extract_fact_candidates` returns bounded
generic facts/IDs/diagnostics with per-file status and a module baseline, not public
Evidence or an AST/source dump. Its per-file compatibility counters report prepass
nodes/work/depth and type ignores separately from semantic nodes/edges.
`source::decode` returns one decoded UTF-8 view with explicit reservation/cancellation;
`type_comments::analyze` returns a charged `CommentPlan` with typed range overrides.
`unicode15` supplies static raw XID membership and single-code-point name lookup.
These are private interfaces, not a new CLI/backend selector or literal-value API.

##### Data Flow

1. Validate captured original-byte limits (1 MiB/file, 16 MiB/run) and hash those
   immutable bytes, including BOM/cookies. Decode before lexing/parsing: default or
   declared UTF-8, UTF-8 BOM and a finite UTF-8/Latin-1 cookie/alias profile. Cookies
   are considered on the first two eligible physical lines; code on the first line
   blocks a second-line cookie. A BOM conflicting with a non-UTF-8 cookie is syntax
   failure. Decoded storage is capped at 2 MiB/file, 32 MiB/run and participates in
   cooperative accounting. All subsequent ranges index the same decoded view,
   never original Latin-1 byte offsets; there is no newline/source repair.
2. Retain lexical bounds and one Ruff parse with explicit PY312 / AST-only policy.
   Both ordinary and unsupported-version diagnostic lists must be empty before
   any fact callback. In the fact lane, raw source identifiers must satisfy pinned
   Unicode-15 XID start/continue (including Python's underscore exception), before
   using Ruff's whole-string NFKC names. Compiler/latest-Unicode properties and
   per-character normalization are not substitutes. Classification acceptance
   alone does not establish the fact lane's compatibility checks.
3. Run the bounded contextual type-comment prepass over validated tokens/statements,
   without parsing comment payloads or walking expressions again. It validates
   attachment/placement and ignores, and corrects assignment, parameter and enclosing
   statement/declaration spans before fact lines/IDs are emitted. Its compatibility
   node/depth caps are 200000/256, with working-byte charges and cancellation; this
   is separate accounted normalization, not an uncounted semantic traversal.
4. Preserve the existing helper-order projection, metadata limits and transactional
   file emission. Source-backed literal decoding now resolves active named escapes
   using static Unicode-15 data, not Ruff's surrogate-replacing decoded values.
   Surrogates remain Python code points encoded through ASCII JSON. Only existing
   bounded path/prefix/method metadata retains literal text; all other payloads
   remain redacted. Router host/prefix assignment targets use at most **128 normalized
   Python characters**, not UTF-8 bytes. Route/prefix limits remain 2048/128 points.

##### Edge Cases

- Unicode data includes XID membership, explicit canonical names, all **473 official
  aliases**, and algorithmic CJK unified/Hangul lookup. Explicit names/aliases use
  ASCII case folding; CJK/Hangul algorithmic names are case-sensitive. CJK hex
  spellings follow the pinned four/five-digit rules. Named sequences are absent:
  lookup returns one code point. Raw/bytes/escaped-backslash spellings remain
  distinct from active text `\N{...}` escapes. HTTP-method uppercase behavior keeps
  its finite ASCII/four-preimage Unicode-15 profile, not general case folding.
- Actual type comments inside f-string replacement fields are explicitly
  unsupported pending context-dependent CPython error-location closure; proven
  ignore markers there remain admitted. Text resembling comments in strings is
  not attached as a type comment. Type-comment payloads are not retained as facts.
  Contextual attachment covers assignments, function/async-function signatures and
  parameter slots, and for/async-for/with/async-with headers. Unattached or misplaced
  actual comments reject the file; ignores are counted separately, not emitted as
  inferred typing facts.
- A separate finite exclusion is
  `actual type comment in parameter expression with unresolved complex prefix`.
  It applies to actual comments internal to parameter annotations/defaults when
  lambda/if/else/for/async/yield tokens remain lexically active in the containing
  expression. A constant-space, cancellable token scan tracks delimiter depth and
  the shallowest active complex prefix; closing its containing delimiter clears
  that state. Only the expression containing the first internal comment is scanned:
  a completed annotation does not contaminate its following default, and opaque
  f-strings do not contaminate the prefix. Fully closed complex subexpressions,
  completed annotations before defaults, trailing type comments and ignores do not
  trigger this exclusion. They remain subject to ordinary placement/attachment
  checks; earlier ordinary grammar failures take precedence over a later exclusion.
  The 787-case contextual oracle tests these outcomes and the separate existing
  f-string-field exclusion; it is not a claim that all 787 cases match CPython.
- Invalid named escapes in ordinary strings report the opening line of their own
  token, not the escape's physical line or an adjacent group's start. F-string text
  errors are deferred until the matching closing token; nested ordinary/f-string
  failures therefore take precedence over invalid outer text. The negative oracle
  checks multiline/adjacent/nested/CRLF cases. Format-spec exception taxonomy
  remains Pending G4, not closed by these line corrections.
- The maintained patch now touches **four** vendor source files: `options.rs`,
  `expression.rs`, `pattern.rs`, `statement.rs`. In addition to the original
  default-false duplicate-keyword policy, bare f-string mapping-pattern keys are
  permitted only with `ast_only=true`. TypeVarTuple and ParamSpec defaults now
  receive the same version guard as TypeVar: PY312 reports version errors with
  either AST-only setting; PY313 accepts the defaults. These are pre-diagnostic
  parser policies, not error suppression, recovery acceptance or source rewriting.
- Finite known other codecs report unsupported encoding; unknown registry membership
  stays unresolved, not a fabricated CPython `SyntaxError`. Strict UTF-8 reports
  the offending physical byte line, not generally CPython's PEG/literal-opening
  line; BOM conflicts and registry-alias decode failures use sentinel zero.
  Four known differences are asserted separately: two multiline literal cases
  where physical/opening-token lines differ, and two malformed comment-byte cases
  CPython admits but strict decoding rejects. These are explicit differences, not
  matching negative cases. Full codec, exact failure/diagnostic and resource closure
  remain open. Nonzero decimal integers over 4300 digits (excluding underscores)
  remain excluded; all-zero,
  base-prefixed, float and imaginary forms retain their separate rules.
- Existing semantic limits (200000 nodes, depth 256, 400000 edges, 100000 facts,
  100000 warnings, 32 MiB working/output) and rollback remain. Parsing/allocation/AST
  destruction cannot be forcibly interrupted; cooperative bounds are not hard
  timeout, stack or RSS guarantees. This batch does not close G2/G4–G6.

##### Example usage

Developer-only static data reproduction/checking uses the installed trusted exact
**CPython 3.12.11 / Unicode 15.0.0**, with no target execution:

```sh
python3.12 -B scripts/generate_python_unicode15.py
python3.12 -B scripts/check_python_unicode15.py
```

Generation without `--fetch` is offline. Independent hard-coded input/table hashes,
the manifest and exhaustive trusted-oracle checks guard reproduction. The pinned
official inputs, generated tables and Unicode License V3 notice (retrieved with
its own recorded hash/date) travel with the source; they are not relicensed as
product code. The generator/check gate and CI's exact interpreter pin are oracle
requirements only, **not production Python-version narrowing**. Continue running
the separate ignored Ruff/reference/native oracles from the development skill;
ordinary test success or a `python_parity` filter alone does not select them all.

**Historical facts-4 integrated validation (reported coordinator results):**

| Gate | Actual result |
| --- | --- |
| Focused Ruff suite | All **75 tests**, including **nine opt-in tests**, passed |
| Positive full-fact oracle | All **188** cases (117 preserved + 71 new) matched complete facts, IDs, diagnostics and duplicate-preserving raw order |
| Negative line oracle | All **32** cases matched failure lines, including **16 new multiline named-escape cases** |
| Contextual type-comment oracle | **787 cases** passed their asserted outcomes, including finite exclusions; not 787 parity matches |
| Known strict-UTF-8 differences | **Four** explicit assertions passed: two multiline failure-location differences and two malformed comment-byte admission differences; not matching negative cases |
| Unicode/source/HTTP-method oracles | Passed, including exhaustive Unicode **1,114,112 membership points + 190,293 name probes** |
| Broader ordinary tests | **660 library tests passed / 183 ignored**, plus **18 adapter + 8 engine** tests passed; ignored tests are not counted as passing |
| Explicit oracle/reference gates | **165 reference + 3 native** passed; with Ruff's **nine** opt-in tests above, **177 explicit tests** passed. This total includes known-difference assertion tests, not an all-parity claim |
| Scripts/data/integrity | **192 script tests**, exhaustive Unicode-data checker, **85-file / 65-source / 1,881,065-byte** vendor check and **235-path** source check passed |
| Final Rust reruns | Product `cargo fmt --package ideomancer --check`, all **eight type-comment tests** (including the **787-case oracle**) after the equivalent boolean simplification, and all-target product Clippy with `--no-deps -- -D warnings` passed |
| Exported-checkout consumer | Passed offline fresh **105-package** lock resolution and **one** full independent native TypeScript public-API test; all five Ruff crates contained, ABI **0.1.5** verified and source hashes rechecked |

The first broad command timed out at **200 seconds**. Its processes were checked
stopped and exact owned temporary data removed. The **ten-minute rerun completed
all tests**, then Clippy reported a single `nonminimal_bool` expression in
`type_comments.rs`'s parameter lookahead (then line 371). This was a static-check
failure, not a test failure. The coordinator applied an equivalent boolean
simplification; the final formatting/type-comment/Clippy reruns above passed.

The successful consumer checked `source_sha256`
`720b58542550dc9cd26fa769c72d8f491c4673b449ba8450880a40464ca75665`.
This is the tested content snapshot, **not an actual Git commit**; final
documentation edits necessarily produce different source bytes/hash. The first
consumer invocation inherited the OS `TMPDIR` and automatically cleaned its owned
scratch. The successful rerun explicitly configured a unique owned disk-backed
cache run for `TMPDIR` and cleaned it after validation; no scratch was retained.

**Historical facts-4 local batch closeout:** post-documentation source inventory and vendor checks
passed for **235 paths** and the exact vendor closure above. The guarded helper
preflight found no active build processes, protected outputs or unexpected target
entries. Cache-only cleanup removed **1,510,563,840 estimated allocated bytes**
(about **1.41 GiB**); follow-up reported `already_clean`, with no Cargo invocation
after cleanup. Owned temporary runs were removed; no debugging artifacts were
retained. Existing `.ideomancer/` outputs and global dependency/toolchain caches
were untouched. Changes were uncommitted at that closeout and subsequently
user-committed. The separate facts-5 cleanup is recorded below.
Historical facts-3 passes did not establish facts-4 gates, and these tests do not
close the documented G4 differences or approve production cutover.

Source inventory at that checkpoint: **235 paths**. Ruff provenance: **85 vendor files / 65
source files / 1,881,065 bytes**, with the four-source-file patch and exact reversal
anchors. The five local crates, manifests/dependency pins, Git-source delivery and
`publish = false` remain unchanged; no fork/submodule/registry release is required.

##### Related files

- `src/analysis/python_ruff.rs`, `python_ruff/{facts,expressions,metadata,literals}.rs`
- `src/analysis/python_ruff/{source,type_comments,unicode15}.rs`
- `src/analysis/python_ruff/unicode15/{PROVENANCE.md,manifest.json,inputs/}` and tables
- `scripts/{generate,check,test}_python_unicode15.py`
- `vendor/ruff/{README.md,provenance.json,patches/ast-only-policy.patch}`
- `src/analysis/python_facts.rs`, `python_native/{projection,scalars}.rs`

#### Ruff failure-certificate batch: frontend-3 / private facts-5

##### Overview

This historical prior uncommitted checkpoint used frontend policy
`python-ruff-private-3-py312-failure-certificates` and fact policy
`python-ruff-private-facts-5`. HEAD `523cd7b` was observed clean before this
batch, containing the facts-4 checkpoint above. Ruff remains 0.12.0 at
`87f0feb21a1af67195bebf94d811770393f9f447`; no dependency, vendor patch, Unicode
data or notice-data change accompanied that checkpoint. Its source inventory was
**237 verified paths**, adding `python_ruff/failures.rs` and
`python_parity/ruff_failure_batch.rs`.

The policies, exclusions, validation and cleanup below are historical facts-5
evidence only. That checkpoint remains uncommitted at actual HEAD `523cd7b`;
the current frontend-4/facts-6 batch follows in its own section.

CPython compatibility remains `Pending`. Production `python-static-8-generic-facts`,
the trusted Python >=3.12 helper, public API, evidence, CLI and default are unchanged.
This is finite failure certification, not complete CPython failure parity or
approval to narrow the production language/version contract.

##### API / Interfaces

- `parse_batch` remains classification-only, but classification must consider
  `Admission` as well as `RuffSyntax`: `AcceptedByRuff` can coexist with compatibility
  rejection. Both parse and unsupported-version diagnostic channels still reject
  semantic callbacks, even without a certificate.
- Private `Failure` records contain an exception enum and line. `FileReport.failure`
  and `CandidateFile.failure` are optional and populated only for certified failures.
  `Failure::message()` formats exactly `<Class> at line <N>; source not analyzed`.
  `failure_line` remains a legacy location field: without a certificate it may be
  a physical decoder/parser or contextual type-comment line, not a proved
  exception-class/line pair.
- Fact status `ParseFailure` represents certified `UnicodeDecodeError`; syntax
  certificates use `SyntaxError`. Unproved competing failures remain
  `UnsupportedSyntax` without a certificate. Arbitrary Ruff errors and unsupported
  versions also remain uncertified, and cannot authorize AST consumption.
- Contextual type-comment `Error::Syntax` retains its legacy `SyntaxError` status
  and `failure_line`, but leaves `failure: None`. The contextual oracles establish
  placement and locations, not every concrete CPython exception subclass. Only the
  shared inspector supplies exception certificates; legacy syntax status alone
  does not certify the concrete exception class.
- `CandidateReport.diagnostics` still contains semantic warning records only.
  This batch does **not** emit public helper `parse_failure` diagnostics. The
  exception vocabulary includes `IndentationError`, `TabError` and `ValueError`;
  enum membership is not implemented parity for those exception families.

##### Data Flow

1. Preserve original-byte hashing and cumulative caps: **1 MiB/file, 16 MiB/run**
   original bytes; **2 MiB/file, 32 MiB/run** decoded text. Existing UTF-8/BOM/cookie
   and finite Latin-1 decoding supplies one unchanged decoded view for all ranges.
2. Run lexical gates and **one** Ruff parse with explicit PY312 / AST-only options.
   After that parse, `failures::inspect` reads tokens, source spellings and diagnostic
   ranges only. It never accesses an AST, including recovery output. Raw Unicode-15
   identifiers, decimal-4300 and named-escape guards move from fact indexing into
   this shared admission layer, so both private lanes apply them.
3. Certify only independently checked finite failures and first-failure precedence.
   Unproved mixed families or competing actual type comments reject compatibility
   without a certificate and with **zero validated-AST callbacks**. No diagnostics
   are waived and no reparse or source rewrite occurs.
4. Only admitted files with both diagnostic lists empty reach the borrowed-AST
   callback. Existing fact-only contextual type-comment attachment/span correction,
   helper-order projection and transactional emission remain. Failed files retain
   only their original-hash module baseline, never tentative facts or warnings;
   valid siblings are preserved in either input order.

All temporary inspector frame stacks are bounded to **256**. Storage, including
diagnostic-range scratch, is charged before allocation under a **32 MiB**
compatibility working cap, with cooperative `Checkpoint::Compatibility`
cancellation. Original/decoded cumulative reservations are unchanged. These bounds
do not interrupt Ruff parsing or AST destruction and are not hard RSS, stack or
deadline guarantees.

##### Edge Cases

- Nonzero decimal integers exceeding **4300 digits**, excluding underscores, now
  have a proved `SyntaxError` rejection rather than `UnsupportedSyntax`. All-zero,
  base-prefixed, float and imaginary forms retain their separate rules. This bounded
  policy follows the **CPython 3.12.11** oracle's configured digit limit, not a
  narrowing of production's Python >=3.12 prerequisite.
- Malformed active hex, `\u`, `\U` and named Unicode escapes in ordinary strings
  use that token's opening line, including multiline/adjacent literals. Invalid
  f-string outer text waits for its matching closing line, preserving nested
  literal failure precedence. Format-spec decode errors use
  **`UnicodeDecodeError` at line 0**, even for raw f-string formats; this is not the
  ordinary-string syntax taxonomy. Raw text, bytes, escaped backslashes, unknown
  escapes and surrogate escapes keep their distinct admission rules.
- Format replacement-field depth is **three**; a fourth nested replacement rejects.
  A nested f-string starts its own depth count. This is a token-local compatibility
  check, not a compiler-semantic validator: AST-only accepted constructs are not
  subjected to compilation checks.
- A mixture whose first failure cannot be proved, or a competing actual type
  comment, remains uncertified. This conservative path does not blanket-exclude
  supported type comments in otherwise valid files. Existing f-string-field and
  complex parameter-expression actual-type-comment exclusions remain in force.
- Codec registry coverage, strict UTF-8 physical versus CPython opening-token
  locations, malformed non-ASCII UTF-8 comment-byte admission and platform/resource
  acceptance remain open. Passing known-difference assertions is not parity.

**Historical architecture stop (F0 investigation subsequently authorized; see
[the ADR](python-native-replacement-feasibility.md)):** unchanged Ruff accepts `&str`, so bytes invalid as
UTF-8 cannot reach it unchanged. CPython admits some malformed non-ASCII UTF-8
comment bytes; that behavior cannot be reproduced with this unchanged frontend
and the no-rewriting contract. Do not add comment-byte repair, recovery or a new
parser-shopping loop as a workaround. A future byte-input frontend or an approved
compatibility boundary needs a **separate architecture/product decision**. This
finding is not permission to narrow production. G2/G4–G6 remain open.

##### Example usage

For a private certified format-spec failure, a consumer can format the certificate
as `UnicodeDecodeError at line 0; source not analyzed`. It must not infer this
message from `failure_line` or from `RuffSyntax` alone, nor treat the private warning
array as a public `parse_failure` report. There is no new public backend selector.

**Historical prior-checkpoint validation (facts-5 only):**

| Gate | Actual result |
| --- | --- |
| Initial focused run and correction | **88 tests: 87 passed, one failed** on a stale facts-4 policy assertion; corrected to facts-5 and passed in the broad run |
| Broader ordinary tests | **670 library tests passed / 186 ignored**, plus **18 adapter + 8 engine** passed; ignored tests are not counted as ordinary passes |
| Explicit opt-in gates | **165 reference + 3 native + 12 Ruff = 180** passed across initial/final runs, including known-difference/limit assertions; not 180 parity matches |
| Module oracle | **356 outcomes**: **312 exact admission/certificates**, **39 ambiguous**, **five parser-only**; not 356 parity matches |
| Actual production-helper failure records | **64 cases**: **53 certified**, **six ambiguous**, **five parser-only**; actual helper `parse_failure` messages and full module baselines checked; uncertified cases are rejection assertions, not certificate parity |
| Positive full-fact oracle | **44 new + 188 preserved = 232** exact complete facts/IDs/diagnostics and duplicate-preserving raw-order cases passed |
| Preserved negative/contextual checks | **32 prior negatives** and **787 contextual type-comment outcomes** passed; contextual outcomes include exclusions |
| Unicode/source/HTTP-method oracles | Passed, including exhaustive Unicode and HTTP-method checks |
| Rollback/hash/sibling checks | Original-hash module baseline, no failed-file warnings, and valid siblings in both input orders passed |
| Scripts/integrity | **192 script tests**, vendor check (**85 files / 65 source files / 1,881,065 bytes**) and **237-path** source check passed |
| Broad Rust static checks | Root product formatting and all-target product Clippy with `--no-deps -- -D warnings` passed |
| Final focused gates after metadata fix | All **eight failure-batch tests**, including the added ordinary regression, **64-case actual-helper failure oracle** and **44-case positive oracle**, plus all **eight type-comment tests** including **787 outcomes**, passed |
| Final Rust static reruns | Product formatting and all-target product Clippy passed after the contextual-certificate fix |
| Final read-only review | No material **P0–P3** findings after the earlier precedence fix |
| Exported-checkout consumer | **Passed offline** fresh **105-package** lock resolution and **one** native TypeScript public-API test; all five Ruff crates contained, ABI **0.1.5** verified and source hashes rechecked; unique owned cache `TMPDIR` automatically cleaned |
| Guarded cache cleanup | Preflight: **zero active blockers / zero protected outputs**, only normal `debug`/`tmp` entries. Apply removed **1,363,083,264 estimated allocated bytes (~1.27 GiB)**; `target/` absent, follow-up **`already_clean`**; no Cargo afterward |
| Final documentation verification | **237-path** source inventory and Git whitespace checks passed; read-only helper verification reported `already_clean`, without Cargo or rebuilding |

After the broad run, a conservative metadata correction removed the synthesized
`Failure { exception: SyntaxError, ... }` from contextual type-comment
`Error::Syntax` handling. Location/placement evidence does not prove all concrete
CPython subclasses; `failure` now stays `None` while legacy status/line information
is preserved. The new ordinary regression
`contextual_locations_do_not_fabricate_exception_certificates` passed in the final
focused failure-batch run. The 53 certified / six ambiguous / five parser-only
actual-helper outcomes and 232 positive fact comparisons remain intact.

The broad library count is **670**, followed by **one added ordinary regression
passed focused**—not a full 671-test library rerun. Likewise, the initial 88-test
Ruff run, the corrected assertion in the broad run and final focused checks do not
constitute a single full 89-test Ruff invocation.

That checkpoint's consumer tested `source_sha256`
`28ad32b603c5260e2733532d3ef90154383a270d3cab2f44f2a22a44dda7a2ec`.
This is a tested content snapshot, **not an actual Git revision**; later
documentation edits change the source hash and are not attested by that receipt.
Changes remain uncommitted at HEAD `523cd7b`.

**Historical facts-5 cache closeout:** the guarded cleanup above completed after the
consumer, with `target/` absent and follow-up `already_clean`. No Cargo ran after
cleanup. No debugging/test artifacts were retained; the owned cache parent was
empty. Existing `.ideomancer/` JS outputs and global dependency/toolchain caches
were untouched. The historical facts-4 **1.41 GiB** cleanup remains a separate
record, not this batch's **~1.27 GiB** removal. Final post-documentation source
inventory and Git whitespace checks passed; read-only helper verification reported
`already_clean`. This source-integrity check does not extend the consumer receipt
to documentation bytes changed after its tested snapshot.

##### Related files

- `src/analysis/python_ruff.rs`: shared parse/admission and validated callback seam
- `src/analysis/python_ruff/{failures,facts,literals,source,type_comments,unicode15}.rs`
- `src/analysis/python_parity/ruff_failure_batch.rs`: integrated actual-helper and rollback oracles
- `src/analysis/python_extract.py`: production helper failure-message contract
- `docs/development-workflow.md`, `.agents/skills/ideomancer-development/SKILL.md`

#### Ruff single-byte codec batch: frontend-4 / private facts-6

##### Overview

This historical coherent uncommitted checkpoint used
`python-ruff-private-4-py312-single-byte-codecs` / `python-ruff-private-facts-6`.
It preserves the existing uncommitted facts-5 checkpoint above; actual HEAD is
`523cd7b`, not a facts-5 commit. Production `python-static-8-generic-facts`, the
trusted Python >=3.12 helper, public API, evidence, CLI and default are unchanged.
No dependency or maintained four-file vendor-patch changes accompany this work.
CPython compatibility remains Pending; G2/G4–G6 are not closed.

The finite profile derives 70 literal 256-code-point tables (**17,920 data
points**) from installed trusted **CPython 3.12.11 / Unicode 15.0.0**. Only **62
ASCII-identity tables**, including the Latin-1 family table, are admitted; seven
EBCDIC tables and cp864 are frozen but excluded. Strict ASCII and the existing
UTF-8/Latin-1 fast paths remain separate. Of 23 tables with undefined bytes, 21
are admitted; undefined positions still fail strict decoding. Maximum defined
UTF-8 expansion is **three bytes**. The **445** combined names retain all **326
exact stdlib aliases**, including the unreachable mixed-case typo `csHPRoman8`.

The audit checks direct stateless charmap decode, stream/incremental wiring and
`getregentry` structure, plus a hash-pinned closure of **123 installed stdlib
Python files**. It is not merely a 256-sample proof, an attestation of native
`_codecs` binaries, or a byte-identical comparison of those installed sources with
an upstream commit. The full license bytes were separately verified against raw
upstream v3.12.11. No raw stdlib module is embedded in the Rust engine.

##### API / Interfaces

- `parse_batch` remains private classification; `extract_fact_candidates` remains
  bounded transactional generic facts, not public Evidence. Native Rust decoding
  requires no Python at runtime. `Admission`, both parser diagnostic channels and
  the shared compatibility inspector still control validated-AST access.
- `source::decode` returns `DecodedSource` with `Cow<str>`, encoding and live heap
  capacity. ASCII input may be borrowed for any admitted encoding; zero allocation
  is a property of `Cow::Borrowed`, no longer an iff-UTF-8 identity test.
- Source failure certification is a **closed whitelist**, not every
  `Error::Syntax`: non-text codecs, `undefined`, strict-ASCII high bytes, undefined
  table positions, source-BOM conflicts and registry-alias UTF-8 decode failures
  have finite **SyntaxError at line 0** certificates. General physical UTF-8
  failure lines remain uncertified. Contextual type-comment syntax errors also
  remain uncertified; legacy status/line alone does not establish an exception.
- Unknown names remain unresolved (`UnresolvedCodec` diagnostic), not fabricated
  syntax failures. Known unsupported multibyte/stateful codecs and the eight
  nonidentity tables remain unsupported without an exact failure certificate.

##### Data Flow

1. Hash original bytes; the root frontend rejects **raw NUL before decoding or
   cookie handling**, with a SyntaxError-at-0 certificate. The generic data decoder
   `source::decode` itself preserves NUL for data-oracle use.
2. Detect the source BOM and eligible first/second-line cookie; code on line one
   blocks a second-line cookie. Historical tokenizer `normal_name` runs before
   the BOM conflict gate, and only then does pinned registry lookup run. Registry
   dot fallback is alias-only, not general module-name fallback. `utf_8_sig` is
   equivalent to UTF-8 **only in this source context after the BOM/history gate**,
   not a general codec identity or a stateless-family member.
3. Validate/count decoded bytes, then reserve the exact decoded amount once before
   allocation (a rejected/invalid decode may never reach reservation). Successful
   ASCII identity input can borrow; other admitted single-byte input builds one
   UTF-8 view. All lexer/parser/literal/span ranges index that same unchanged view,
   never original encoded bytes. No newline rewriting or grammar repair occurs.
4. Preserve **1 MiB/file / 16 MiB/run original** and **2 MiB/file / 32 MiB/run
   decoded** caps. Three-byte expansion can hit `ResourceLimit` before parsing;
   family admission does not waive budgets. Existing cancellation, working/output
   accounting, module-baseline rollback and sibling isolation remain in force.

Codec implementation/data, generator/check/test scripts, manifest, provenance,
NOTICES and LICENSE all participate in the private implementation fingerprint.
The Git source closure is **248 paths**, 11 additions to the prior 237. This is
source/fingerprint coverage, not a claim of current broad gate completion.

##### Edge Cases

Malformed non-ASCII UTF-8 comment bytes admitted by CPython still cannot pass
unchanged Ruff's `&str` interface. The existing architecture stop remains: no
source repair, recovery-AST consumption or diagnostic waiver. Actual type comments
in f-string replacement fields and unresolved complex parameter-expression
prefixes remain excluded. Complete registry, failure, platform and general
resource parity remain open; cooperative bounds are not hard RSS/stack/deadline
guarantees. This finite development profile does not narrow production Python >=3.12.

##### Example usage

A private captured source with cookie `# coding: cp1252` and byte `0x80` inside
a quoted literal decodes that byte to U+20AC; ranges refer to its three-byte UTF-8
representation while the module hash still covers the original bytes. A strict
ASCII cookie with that byte instead produces the certified source failure
`SyntaxError at line 0; source not analyzed`. Neither example selects a public
native backend or executes the source.

**Historical facts-6 validation (local macOS / Rust 1.89; hosted results unknown):**

| Gate | Actual result |
| --- | --- |
| Focused codec Rust tests | **17 passed**, including one module oracle and three new batch oracles |
| Full Ruff gate | **104 passed, including 16 opt-in tests**, after the test-assertion corrections below |
| Decoder/location oracle | **15,872 approved table positions + 256 ASCII positions**, **63 mixed streams**, **112 byte/text AST-location comparisons** passed |
| Encoded complete positive oracle | **112** complete fact/ID/diagnostic/raw-order comparisons passed |
| Actual-helper negatives/controls | **84 certified SyntaxError-at-0 negatives** and **10 uncertified controls** passed; controls are **eight helper errors and two helper admissions**, not ten parity matches |
| Positive preservation and combined coverage | **232 preserved + 112 new encoded = 344** complete matching fact/ID/diagnostic/raw-order cases passed |
| Preserved failure/contextual gates | **32 prior negatives**, **64 actual-helper cases** (53 certified, six ambiguous, five parser-only), **356 module failure outcomes** and **787 type-comment outcomes** passed; ambiguous/parser-only/exclusion assertions are not parity matches |
| Unicode/source/HTTP-method oracles | All passed |
| Global script discovery | **216 tests: 211 passed, five skipped** because they require the isolated profile |
| Explicit isolated codec suite | **24 passed, none skipped**, using **`-I -S -B`**; combined script lanes cover all **216 unique tests**, not 216 default-lane passes |
| Codec data gates | Generator `--check` and independent checker passed with **`license_verified: true`**, using **`-I -S -B`**; CI explicitly runs all three gates, including the isolated codec suite, with these flags |
| Product static checks | Package formatting and all-target Clippy with **`--no-deps -- -D warnings`** passed |
| Source/vendor integrity | **248 source paths** and vendor **85 files / 65 source files / 1,881,065 bytes** passed |
| Broader library/API gates | **682 library tests passed / 190 ignored**, plus **18 adapter + 8 engine** tests passed; ignored tests are not ordinary passes |
| Explicit reference/native gates | **165 reference + 3 native** opt-in tests passed; with **16 Ruff opt-ins**, **184 explicit tests** passed, including known-limit assertions—not all parity matches |
| Current exported-checkout consumer | **Passed offline** fresh **105-package** lock resolution and **one native TypeScript public-API test**; all five Ruff crates contained, ABI **0.1.5** verified and source hashes rechecked; product formatting passed before the consumer |
| Consumer scratch | Unique owned cache `TMPDIR` cleaned; no retained scratch |
| Guarded cache closeout | Preflight: **zero active blockers / zero protected outputs**, normal `debug`/`tmp` only. Apply removed **1,372,758,016 estimated allocated bytes (~1.28 GiB)**; `target/` absent, follow-up **`already_clean`**; no Cargo afterward |
| Source/Git checks before final documentation edits | **248-path** source check passed with the same content hash as the consumer; Git whitespace check passed |

The initial test compile issue was a `Result` alias; it was fixed before the 17
focused tests passed. A later root-NUL assertion still expected Ruff
`StructuredSyntax`; it now explicitly requires `InvalidSource` / `NotParsed`, a
SyntaxError-at-0 certificate and zero validated callbacks. This asserts the
intended earlier check, not a weakened test. Review found only a stale facts-5
policy assertion in the old failure batch, now corrected to facts-6. The full
104-test Ruff and 682-test library suites passed after these corrections; no
engine/codec bug or blocker was found. Original-hash module baselines, zero
validated callbacks on guarded failures, source guards and baseline counts remain
checked. The 84 new certified helper SyntaxError-at-0 failures and prior 53
certified helper failures are preserved; the ten uncertified codec controls are
still control outcomes, not matching certificates. No further code changes are
planned. The current consumer tested content snapshot `source_sha256`
`74e8525de0cb80679435734c0c0ab6f374c2c6818f2008bd67a0fb5064799dc1`.
This is **not an actual Git revision** or an attestation of later documentation
bytes. All codec-family and prior facts-5 work remains **uncommitted at actual
HEAD `523cd7b`**.

That facts-6 cleanup completed after the consumer: the owned cache parent was
empty, no debugging/test artifacts were retained, and existing `.ideomancer/` JS
artifacts and global caches were untouched. No Cargo ran after cleanup. The prior
facts-5 **~1.27 GiB** removal remains historical and separate from the current
**~1.28 GiB** removal. Final source/Git/cache verification is read-only, without
rebuilding; the pre-documentation receipt does not self-attest these edited bytes.

##### Related files

- `src/analysis/python_ruff/{codecs312,source,facts}.rs`, `src/analysis/python_ruff.rs`
- `src/analysis/python_ruff/codecs312/{PROVENANCE.md,manifest.json,tables.tsv,aliases.tsv,NOTICES.md,LICENSE.txt}`
- `src/analysis/python_parity/ruff_codec_batch.rs`: encoded helper/fact/location oracles
- `scripts/{generate,check,test}_python_codecs312.py`: offline isolated reproduction and profile gates
- [Development workflow](development-workflow.md), [release boundary](releasing.md), [third-party notices](third-party-notices.md)

#### F1 shared byte lexer experiment

##### Overview

**Bounded F1 lexer goal COMPLETE / PROVED by compiled tests; all delivery gates PASS.**
Checkout-consumer validation and guarded cache cleanup are complete. This does
not prove byte parser input, AST/debug construction,
CPython admission, F2 or full G4, and does not authorize further implementation.
F1 supersedes F0's uncompiled feasibility finding only for lexical proof. One
fallible `CoreLexer` over borrowed bytes and a byte cursor serves both the legacy
`Lexer(&str)` facade and experimental `ruff_python_parser::lexer::byte::ByteLexer`.
There is no independent lexer, source masking/substitution, unchecked UTF-8,
parser/AST/debug-construction change, source-codec/type-comment semantic change or
facts dispatch change. Frontend-5/facts-7 names and production helper/API/defaults
remain unchanged; the implementation fingerprint changes transitively with the
new vendor provenance. F2 and version/product-policy changes still need approval.

##### API / Interfaces

The doc-hidden `pub` vendor module is additive experimental plumbing, **not
`engine::v1` or a public backend selector**. `ByteLexer::new` takes `&[u8]`, `Mode`,
`ByteLexerLimits` and a cancellation callback (`true` cancels). `next_token`,
`current_token`, `take_value`, `checkpoint`, `rewind`, `re_lex_logical_token` and
`finish` are fallible. `finish` requires a genuine consumed EOF. Legacy signatures,
defaults and all four modes remain; legacy control is unlimited and has no
cancellation callback, with source faults unreachable from typed `&str`.

`original_bytes` and range-checked `source_bytes` borrow the unchanged backing
buffer. `source_text(&mut self, range)` returns a zero-copy checked view or
`ByteViewError::{InvalidRange, InvalidUtf8, ResourceLimit, Cancelled}`. Text views
are metered and cancellable, including repeated requests. Invalid UTF-8 views do
not poison the token stream; resource/cancellation failures latch. Validation uses
bounded chunks, then a precharged safe linear UTF-8 check to construct the borrow;
that final check is uninterruptible (at most 2 MiB under default input limits).
This is not a hard deadline or RSS guarantee.

Default limits: **2 MiB input, 32M work units (32 × 1024 × 1024), 200000 tokens,
depth 256, 32 MiB cumulative conservative allocation charges, 1024 checkpoints,
10000 diagnostics**. Allocation accounting is not live residency.

##### Data Flow

The shared cursor distinguishes scalar, malformed input and EOF; raw NUL is
`Scalar(0)`, not EOF. Only the existing `lex_comment` transition consumes opaque
payload bytes, including in the shared f-string replacement-expression context.
Malformed ordinary-string, outer f-string and format-text bytes produce structured
`SourceFault` ranges, not Python exception certificates. Token spans and borrowed
source pointers retain their original byte identity.

Checkpoints belong to a unique atomic lexer-session ID, not merely a buffer.
Restoration includes current token/value and taken state, cursor/token start,
previous opaque state, diagnostics, indentation and interpolation stacks, including
non-LIFO restores. A source fault is sticky until an owned prior checkpoint is
restored. Cancellation/resource failures and monotonic work/token/allocation/
checkpoint counts are never cleared or refunded by rewind. Re-lex rejects a
non-newline or an offset at/after the current cursor, including an unconsumed newline.

##### Edge Cases

- Malformed-byte guarantees are **Module mode only**. Valid UTF-8 differential
  coverage includes Module, Expression, ParenthesizedExpression and Ipython.
- Cookie/type/ignore payloads are lexically opaque comments, not semantic admission.
  Debug-shaped replacement comments tokenize; constructing CPython-compatible debug
  constants remains F2. Root NUL rejection and encoding selection remain separate.
- F2 still needs byte parser/source/debug integration, BOM/coordinate anchors,
  admission and lazy-error precedence proof. F1 is not G4 or replacement parity.

**Current validation reported by the coordinator:** all **131 private Ruff tests
passed**, including **23 ordinary F1 tests** (22 acceptance + one independent
legacy baseline) and **17 existing opt-in tests**, after all fixes. All **364**
current complete positive facts/IDs/diagnostics/raw-replay cases matched on the
unchanged normal `&str` path. Current preservation passed **708 library tests
(191 ignored)** and **18 adapter + 8 engine** tests. Explicit **165 reference +
3 native** tests also passed; with the 17 Ruff opt-ins from the full 131-test run,
**all 185 explicit gates passed**, including known-limit assertions—not 185 parity
matches. These are current feature results; older receipts below remain historical.

Thirteen complete ordered kind/range/flag/value/error traces were frozen **before
core edits**, covering **298 tokens including EOF** across all four modes. Both
legacy and byte facades passed all thirteen hashes using the
[frozen baseline](python-byte-lexer-baseline.json). The legacy `Send + Sync`
assertion compiled and passed inside the baseline test in the full 131-test run.
The additional **48** full valid-UTF-8 trace comparisons share the core and are supplementary,
not independent preservation proof. The acceptance matrix includes **128** malformed
comment ending/adjacency combinations, **32** valid comment counterparts, **24**
opaque type/ignore/cookie payloads, **32** replacement-comment contexts and **48**
negative string/f-string contexts (plus **48** fault-location/unterminated probes).

The initial run had 21 passes/two failures: an unconsumed newline at
`offset == cursor` was incorrectly accepted; rejecting offsets `>= cursor` fixed
the real re-lex API issue without rewriting source. The raw-`0xff` newline
expectation was corrected from `5..6` to `4..5`: `0xff` occupies one byte (test
off-by-one). Review's P2 unmetered-source-view finding is fixed; the passing root
regression covers repeated views, cancellation and UTF-8 chunk boundaries with
bounded `&mut self` control and explicit view-error variants. The **23** F1 count
includes this additional regression beyond the agent's earlier 21 acceptance
tests. All **131** passed after these fixes. No additional core changes are
planned; provenance/patch/authored-source anchors below are stable. The P2
source-view accounting review item is closed. Final read-only review found no
material P1/P2/P3 findings; the new schema-2 guards passed. Structured source-fault
behavior and compiled lexical proof are established, not outstanding concerns.

| Current gate | Coordinator-reported result |
| --- | --- |
| Unfiltered library / API preservation | **708 library passed / 191 ignored; 18 adapter + 8 engine passed** |
| Explicit reference / native / Ruff gates | **165 + 3 + 17 = 185 passed**, including known-limit assertions, not all parity matches |
| Product formatting / all-target Clippy | Passed; product-only formatting and `--no-deps -- -D warnings` |
| Global script discovery | **310 discovered, 297 passed, 13 isolation-required skips** |
| Explicit isolated byte / visitor / codec suites | **23 / 43 / 24 passed**, no skips; combined lanes cover **all 310 unique tests**, not 310 discovery-lane passes |
| Vendor scripts / schema-2 checker | **58 passed**; checker passed at **87 files / 66 Rust sources / 1,972,891 bytes** (65 upstream + one local; eight modified / 57 unchanged upstream) |
| Source inventory | **261 paths passed** |
| Offline checkout consumer | Passed: fresh **105-package** lock, **one native TypeScript public-API test**, five contained Ruff crates, ABI **0.1.5**, source hashes rechecked |
| Guarded cache cleanup | Passed: **1,360,498,688 estimated allocated bytes (~1.27 GiB)** removed; target absent, follow-up **`already_clean`** |

**Completed F1 delivery:** the offline consumer's tested source snapshot is
`2cea3f996bf4a0e0cbbfd242b681331fb2208a3f0e6ac1fba2e4e81472869c6e`.
This is not an actual Git revision or an attestation of final documentation bytes.
Its unique owned cache `TMPDIR` was cleaned, with none left. Guarded preflight
found **zero blockers / zero protected outputs**, normal `debug`/`tmp` only;
cleanup removed **1,360,498,688 estimated allocated bytes (~1.27 GiB)**. Target is
absent and follow-up reported **`already_clean`**; **no Cargo ran after cleanup**.
No owned debug/test artifacts were retained; the root cache parent is empty.
Existing `.ideomancer/` JS artifacts and global caches were untouched. Final
source/Git/helper checks are read-only, without rebuilding.

Current F1 changes remain uncommitted and unstaged, with no commits or pushes,
following the user-committed pre-F1 `fdf4e58` checkpoint observed clean.
Existing CI's ordinary root test
lane selects the F1 tests; no public backend or new runtime Python path is introduced.

##### Example usage

Private lexical example: `b"x#\xff\ntail=1\n"` retains the comment bytes and yields
the following `tail` token. A checked text view over that comment returns
`InvalidUtf8` without stopping lexing. By contrast `b"x='#\xff'\n"` faults in
string text. Neither outcome certifies CPython file admission or exception class.

##### Related files

- [`byte_lexer_acceptance.rs`](../src/analysis/python_ruff/byte_lexer_acceptance.rs),
  [`byte_lexer_tests.rs`](../src/analysis/python_ruff/byte_lexer_tests.rs) and
  [baseline JSON](python-byte-lexer-baseline.json): ordinary root tests and frozen traces.
- [`lexer.rs`](../vendor/ruff/crates/ruff_python_parser/src/lexer.rs) and
  `lexer/{cursor,indentation,interpolated_string,byte}.rs`: shared core/state and facade.
- [`provenance.json`](../vendor/ruff/provenance.json): strict schema 2; two disjoint
  four-file patches (`ast-only-policy.patch` unchanged, plus
  `byte-lexer-experiment.patch`) and separately hashed original MIT `lexer/byte.rs`.
  The **65-source upstream pristine map is unchanged**: eight modified upstream,
  57 unchanged, plus one authored source = **66 Rust sources** in **87 vendor files /
  1,972,891 bytes**. Source inventory: **261 paths**, five additions to F0's 256.
  No manifests, dependencies or tools changed.
- Current SHA-256 anchors: provenance
  `83b701ef82cd549d1b01501520f57b9504e3decac32cdc7d151add639f892800`;
  F1 patch `4f4d3a244236a3e730ab805716f23f581483fa6e7a38060b1ea10c344fe5aac5`;
  authored facade `d8e617b7bb9401e96263997b99151cfa02e3dd0af1254a384c7569e12d2fec94`.
  Retain root MIT and Ruff licenses, plus existing PSF/Unicode notices unchanged.

#### F0 byte-input and version feasibility

**Historical F0 checkpoint:** uncompiled findings and clean-cache receipts below
are superseded only by the bounded F1 lexical evidence above, not by parser proof
or current-tree closeout. HEAD references describe that historical observation.

##### Overview

**Full replacement selected; bounded F0 investigation explicitly authorized.** The
main-owned [feasibility ADR](python-native-replacement-feasibility.md) is the design
record. **F0 investigation is complete.** Its source audit finds a **conditional GO** for proposing separately bounded
F1/F2 prototypes, not an implemented, compiled or demonstrated byte-aware Ruff.
Frontend-5/facts-7 names, private G2/G3 closure, production helper and runtime
compatibility policy remain unchanged; there is no production code or dataset refresh.

##### API / Interfaces

The proposed byte-backed lexical input plus narrow checked source views retains
Ruff's grammar. It is not a thin adapter: `TokenSource` owns a concrete `Lexer`
with lookahead/checkpoint/rewind/re-lex obligations; `Cursor` uses `Chars`/`&str`.
Spanning debug `String` ranges may require AST/API changes. This exceeds the current
four-file vendor policy surface and is not authorized as a production patch.

Version strategy is independently **proposed**: immutable full-bundle profiles,
exact patch members and an enumerated release support window. Production currently
uses the selected interpreter's grammar, stdlib, actual version token and legacy
snapshot identity, dynamically >=3.12 and not CPython-only. A fixed profile cannot
preserve all future/arbitrary interpreter behavior. Product approval is required
before any finite-policy narrowing, native wire representation or default change.
Legacy `Toolchain::Python { python }` continues through the helper; no native probe,
fallback, fake `pythonVersion`, nearest-profile substitution or old-save upgrade.

##### Data Flow

The read-only reference checker parses bounded synthetic original bytes with exact
trusted CPython 3.12.11/UCD15. Its **219 named cases** comprise **107 accepted / 112
failures**: SyntaxError at line 0 = **20**, lines 1/2/3/4 = **36/13/3/1**,
UnicodeDecodeError at line 0 = **39**. There are **53 complete AST-with-attributes
control relationships**. This is reference evidence, not comparison with a native
byte implementation. Debug f-strings admit malformed comment bytes while omitting
comments from synthesized debug constants; checked token text alone is insufficient.
See the ADR for proposed original-byte ownership, checked views and failure scheduling.

##### Edge Cases

F0 authorization does not grant expanded production vendor/AST changes, source
repair, tool installation, public selectors, finite-profile policy or cutover.
F1/F2 require separate bounded approval. G4 is no longer awaiting initial direction,
but byte proof, version policy, codecs, contextual errors, failure precedence and
resource compatibility remain open; G5/G6 are not approved.

**Completed F0 validation:**

- Reference checker: **219/219 expected outcomes, 219 `ast.parse` calls**, 107
  accepted / 112 errors, **53 AST-with-attributes controls** and **20 debug cases**.
- Isolated byte suite: **23 passed, no skips**. Global discovery: **286 tests,
  273 passed, 13 isolation-required skips**. Explicit isolated byte/visitor/codec
  lanes: **23 / 43 / 24 passed, no skips**. Combined lanes cover **all 286 unique
  tests**, not 286 discovery-lane passes.
- Source integrity: **256 paths** (253 + ADR + two scripts). Vendor integrity:
  **85 files / 65 sources / 1,882,473 bytes**, unchanged by F0. Git whitespace
  checks passed.
- Independent architecture review found no blocking correction. F1 must prove
  typed scalar/error/EOF outcomes, noncommitting lookahead and rewind of deferred
  failure state; F2 must add explicit BOM-coordinate anchors. These are future
  obligations, not implemented proof.
- F0 changed only scripts/docs/CI: no Rust/Cargo build, native-engine or vendor
  change, or dataset refresh. No Rust/consumer rerun was needed or claimed.
  The helper reported **`already_clean`**; no fresh build/cache removal was needed.
  The unique owned `TMPDIR` run was cleaned with none left and no retained artifacts.

Prior facts-7 tests and cleanup receipts remain historical; profile names and
existing uncommitted work at HEAD `523cd7b` are unchanged. After documentation
edits, final verification is pure-Python source/helper and Git inspection only,
without a Rust rebuild.

##### Example usage

Reference-only reproduction, not a native parity or cutover gate:

```sh
python3.12 -I -S -B scripts/check_python_byte_contract.py
python3.12 -I -S -B scripts/test_python_byte_contract.py
```

##### Related files

- [Feasibility ADR](python-native-replacement-feasibility.md): source audit, alternatives and staged gates.
- `scripts/check_python_byte_contract.py`, `scripts/test_python_byte_contract.py`: read-only reference/tamper checks.
- `src/python_extract.py`, `src/analysis/python_ruff.rs`: unchanged helper/private frontend.
- `vendor/ruff/crates/ruff_python_parser/src/{token_source.rs,lexer/cursor.rs}`: concrete byte-interface constraints.

#### Ruff visitor contract batch: frontend-5 / private facts-7

**Historical visitor checkpoint:** policy names/closed private G2/G3 scope remain
current; validation, HEAD, uncommitted-work and cache receipts below are historical,
not current F1 closeout.

##### Overview

Current private policies are `python-ruff-private-5-py312-visitor-patterns` and
`python-ruff-private-facts-7`. This bounded batch targets finite **private G2/G3
implementation closure for the currently admitted Python 3.12 Module-AST lane**,
not G4 replacement compatibility. It preserves uncommitted facts-5/facts-6 work at
actual HEAD `523cd7b`. Production `python-static-8-generic-facts`, the trusted
Python >=3.12 helper, public API, CLI, evidence and default are unchanged. There is
no visitor/normalizer rewrite, dependency change or new framework semantics.

##### API / Interfaces

The executable [class/field contract](python-visitor-contract.json) and
[synthetic witnesses](python-visitor-witnesses.json) use profile
`cpython-3.12.11-module-visitor-v1`. The closed inventory has **111 modern CPython
3.12 classes**, **108** in the Module domain; `Interactive`, `Expression` and
`FunctionType` roots are explicitly outside scope. There are **188 fields**
(184 in-domain), including **147 in-domain node-bearing fields**: **130 field
obligations with case-unique observable call inputs** and **17 structural
population/neutral witnesses**. These are not 130 independent proofs. Attributes
are normalized span metadata; complete source/location compatibility stays G4.

##### Data Flow

`scripts/check_python_visitor_contract.py` reads only the two fixed JSON inputs,
checks the independent hand-reviewed field-type/disposition catalog and runtime
`_fields`/`_attributes`, and AST-parses synthetic sources with `type_comments=True`.
It never executes witnesses or application code. The **10 field-order obligations**
check distinguishable input groups, **not output ordering**. That requires the
separate Rust helper/Ruff oracle comparing complete facts, IDs, diagnostics and
unsorted raw replay traces; sorted/deduplicated facts alone are insufficient.

The **20 cross-family fixtures** cover all marker classes, every function argument
annotation/default group, async/class/decorator/type-bound alias replay,
multi-generator comprehensions, interleaved call callee/star/keyword groups,
labelled hidden metadata beyond rendering caps, optional children and f-strings.
They exercise the existing bounded lowering and transactional replay, not a new
semantic framework.

##### Edge Cases

The proof surfaced a real G4 gap: Ruff's `at_pattern_start` lacked AST-only
`FStringStart` admission for class-first and later-sequence children. Singleton-first
sequence elements already accepted in both modes before this patch. The predicate
now uses the same conditional policy as the earlier mapping-key predicate.
AST-only extra admission remains default-false; previously valid default-mode
forms are unchanged. An initial root test wrongly expected default singleton
f-string patterns to reject; it was corrected to honor upstream first-element
acceptance, not to waive a failure.

A later malformed control exposed an **independent upstream sequence separator
bug**: `[1 other]` and `[f'{v()}' other]` could consume an arbitrary token before
the closing delimiter as though it were a comma. `pattern.rs` now requires
`self.at(TokenKind::Comma) && peek-closing` for that path in **both modes**.
This grammar correction intentionally rejects previously erroneously accepted
malformed default-mode inputs; it is the exception to default-behavior preservation.
Malformed inputs must produce zero validated callbacks; no diagnostics are waived.

The same four patched source files and **65-source pristine closure** remain.
Current vendor provenance is **85 files / 65 source files / 1,882,473 bytes**;
policy-patch SHA-256 is
`e79a66530f58f63371c0423f5f0469ff82f9b0193f57b6ba8672c3c33aa4e90e`,
and provenance anchor is
`b8246ec2c0911719ddd715f80dac504795c97f70ab828bd4375d18538c237ee0`.
The source inventory is **253 paths**, five additions to 248: the two JSON files,
checker, script tests and Rust visitor batch. The current source check passed at
253 paths; vendor integrity passed at the updated byte closure above. No data,
dependency or license changes accompany this batch.

**Current validation after both parser fixes:**

| Gate | Actual result |
| --- | --- |
| Full Ruff gate | **108 passed, including 17 opt-in tests** |
| Complete positive preservation | **344 prior + 20 new = 364** matching complete fact arrays/IDs/diagnostics/raw replay |
| Prior compatibility oracles | All prior failure, Unicode, codec and **787-outcome contextual** gates passed; known-difference/exclusion assertions are not parity matches |
| Product static checks | Formatting check and all-target Clippy **`--no-deps -- -D warnings`** passed |
| Global script discovery | **263 discovered: 257 passed, six skipped** for isolation requirements |
| Isolated script lanes | Visitor **43 passed**, codec **24 passed**, no skips; combined lanes cover **all 263 unique tests**, not 263 discovery-lane passes |
| Contract and focused visitor gates | Checker and all **three visitor tests**, including the 20 complete comparisons, passed |
| Parser controls | Both constructors and both AST-only modes passed singleton-sequence acceptance and malformed missing-separator/zero-callback controls |
| Vendor/source gates | **34 vendor script tests** passed; vendor integrity **85 files / 65 sources / 1,882,473 bytes** and source **253 paths** passed |
| Broader library/API preservation | **685 library passed / 191 ignored**, **18 adapter + 8 engine passed**; ignored tests are not passes |
| Explicit reference/native preservation | **165 reference + 3 native passed**; with **17 Ruff opt-ins**, **185 explicit gates passed**, including known-limit assertions—not all parity matches |
| Current offline consumer | **Passed** fresh **105-package** lock resolution and **one native TypeScript public-API test**, five-Ruff-crate containment, ABI **0.1.5** and source-hash rechecks |
| Consumer scratch | Configured unique owned cache `TMPDIR` cleaned; none left |
| Guarded cleanup | Preflight **zero blockers / zero protected outputs**, normal `debug`/`tmp` only; removed **1,374,564,352 estimated allocated bytes (~1.28 GiB)**; target absent, follow-up **`already_clean`**, no Cargo afterward |
| Source/vendor/Git before final documentation edits | **253 paths**, vendor **85 files / 65 sources / 1,882,473 bytes**, and Git diff checks passed |

The earlier singleton-sequence root-unit expectation was corrected to upstream
acceptance with both false/true AST-only settings and both constructors. The real
missing-separator grammar bug is fixed and its controls now pass in both modes and
constructors; these corrections do not weaken tests. CI includes checker/tests
with `-I -S -B`. Actual implementation differences are the narrow parser-start
predicate and separator fix plus proof files, **not rewritten visitor/normalizer
engine code**. No further code changes are planned.

**G2 and G3 PRIVATE IMPLEMENTATION COMPLETE/CLOSED.** The completed preservation
checkpoint supports the qualified conclusion: “Private implementation complete for
current admitted Python 3.12 Module-AST lane:
closed class/field inventory + full-output/raw-replay coverage; not complete Python
compatibility, G4 bytes/admission/failure/resource equivalence, or production cutover.”
G4 version parity is also open; public G5/G6 integration/cutover is not approved.
All 364 complete positives and prior failure/data/reference gates passed. Consumer
validation and guarded cleanup complete the **local macOS / Rust 1.89 delivery
closeout**; this is not hosted/platform validation or full production approval.
The tested consumer content snapshot `source_sha256` is
`0e71afaa639a38f3ca0022726dc24bb24257eee8a01845ac7f36900fe4471f60`,
**not an actual Git revision or an attestation of later documentation bytes**.
Nothing was staged, committed or pushed; actual HEAD remains `523cd7b` and prior
facts-5/6 work is preserved uncommitted. The source/vendor/Git checks above preceded
final documentation edits; final source/Git/helper checks are read-only, with no
rebuild. No Cargo ran after cleanup. No owned debug/test artifacts were retained,
the unique owned cache scratch was cleaned with none left, and `.ideomancer/` JS
artifacts/global caches were untouched. The facts-6 **1,372,758,016-byte** cleanup
is historical and separate from facts-7's **1,374,564,352-byte** removal, despite
both rounding to ~1.28 GiB.

Concrete G4 exception-class gap: contextual type-comment errors in for/with/class
suites can be `IndentationError`, while current `Error::Syntax` status/line fields
intentionally leave `failure: None`. Keep that conservative behavior: no placeholder
certificate or inferred `SyntaxError` subclass. Exact arbitrary type-comment
locations/classes are not closed by selecting another PEG parser alone and do not
reopen the completed visitor inventory.

**Prior checkpoint STOP — superseded for F0 authorization only:**
“STOP parity expansion after this bounded gate. G4 needs a user choice.” The user
has now explicitly selected contract-preserving full replacement and authorized
the recommended bounded byte-aware Ruff feasibility/version investigation. See
the [F0 record](#f0-byte-input-and-version-feasibility); this does not authorize
production expansion or close G4–G6.

##### Example usage

Maintainer gate examples, using installed trusted exact CPython 3.12.11:

```sh
python3.12 -I -S -B scripts/check_python_visitor_contract.py
python3.12 -I -S -B scripts/test_python_visitor_contract.py
RUSTUP_AUTO_INSTALL=0 IDEOMANCER_TEST_PYTHON="$(command -v python3.12)" \
  cargo test --locked --offline --jobs 2 --lib ruff_visitor_batch \
    -- --include-ignored --nocapture --test-threads=2
```

These focused gates do not replace the existing reference/native/Ruff preservation,
metadata, Unicode/codec/failure, API, static, source/vendor and consumer gates.

##### Related files

- [Visitor contract](python-visitor-contract.json), [witnesses](python-visitor-witnesses.json)
- `scripts/check_python_visitor_contract.py`, `scripts/test_python_visitor_contract.py`
- `src/analysis/python_parity/ruff_visitor_batch.rs`, `ruff_facts_batch.rs`
- `src/python_extract.py`, `src/analysis/python_ruff/{facts,expressions,metadata}.rs`
- `src/analysis/python_native/projection.rs`, `vendor/ruff/patches/ast-only-policy.patch`
- [Development gates](development-workflow.md), [source delivery](releasing.md)

#### Historical Tree-sitter reference lane and parity foundation

The following reference-lane contracts/results are not the current Ruff admission
profile. In particular, their ASCII/cookie/named-escape exclusions remain scoped to
those historical policies.

P1 has a **private captured-source AST parity harness**, a native parse-only
front-end and bounded class/function/header/body/import candidates, not a production native Python
backend. The production helper, evidence v1 serialization,
`python-static-8-generic-facts` identity, snapshot formula, scanner/module policy,
redaction and resource limits remain unchanged. There is no new CLI flag, public
toolchain variant or default migration. The private front-end adds the exactly
pinned Python grammar dependency described below.

Production extraction and the harness use the same private `extract_captured`
seam in `src/analysis.rs`. It serializes the bytes already captured by the no-follow
scanner, retains the 40 MiB helper-input gate, and invokes the existing isolated
bounded helper. It does not read target paths again. Fixtures in
`src/analysis/python_parity.rs` are independently synthetic and use automatically
cleaned temporary directories, not private application examples or new build caches.

Four ordinary unit tests check identity preimages, comparison sensitivity, limit
boundaries and fail-closed oracle selection. Eight explicit AST oracle tests pin:

- Full ordered fact tuples and IDs, encoded metadata and Unicode/surrogate escapes.
- Definitions, lexical ownership, imports, decorators and ordered parameter kinds.
- Direct local-call candidates, parameter shadowing and literal-write invalidations.
- Identical same-line call deduplication, repeatability and input-order independence.
- Direct route/prefix syntax versus dynamic boundaries and literal redaction.
- Valid/malformed-file isolation: a failed file retains only its module fact.
- Unicode identifiers/paths, CRLF, UTF-8 BOM, encoding cookies and invalid bytes.
- Captured-byte replay after source replacement/removal, nonexecution markers,
  exact production-response equivalence and unchanged persistence identity.

Two further oracle cases compare private front-end classifications/digests, not
facts, with the AST helper. They expose grammar-versus-AST acceptance differences
and intentionally unsupported encodings rather than hiding them as parity.
Five declaration-candidate oracle cases compare complete, unfiltered ordered AST
fact vectors for the supported subset, pin signature limits and metadata escaping,
and verify whole-file rejection, isolated transactions and captured-byte replay.
Seven header-expression cases add typed/defaulted parameter kinds, multiline spans,
literal spelling/redaction, exact shape budgets, hidden-call rejection, signature
limits/replay and malformed physical-newline rejection against the AST oracle.
Seven body cases compare calls, writes/invalidation, returns, clipping and statement
extents; verify hidden call retention and helper exclusions; test parser nesting;
and compare direct-call/shadowing behavior through an isolated resolver fixture.
Seven import cases add exact metadata/spelling, scope and body combinations,
absolute-alias side-output exclusions, relative-import neutrality, long targets and
clipped callees, unreviewed grammar rollback, and captured-byte replay/isolation.
Six multiline-import cases add parenthesized lists/trailing commas, comment and
implicit-continuation handling, closing-delimiter spans, unchanged alias exclusions,
metadata capacity boundaries and replay. A CPython-valid nested dedent that the
pinned grammar rejects remains an explicit baseline-only classification difference.
Eight alias-warning cases compare full ordered facts **and diagnostics**, including
direct/aliased dynamic calls, suppression/overwrite, persistent scope names,
fresh-callee clipping, full targets, hidden calls, duplicate warnings, lexical sorting,
transaction isolation, retained constructor exclusions and captured-byte replay.
Seven class cases add plain declarations/methods, exact signatures, mixed lexical
ownership, class-suite writes, closing spans, class-inclusive alias lookup, excluded
header/return forms, transactions and replay. Comparisons retain every fact and warning.
Eight base/assignment-annotation cases add ordered/deduplicated base syntax, direct
class attribute annotations, annotation-only nonwrites, supported RHS contexts,
whole-statement spans, unclipped field symbols, shape limits, alias warnings,
metadata exclusions, malformed comma-only base lists, isolation and replay.
Eight core-expression cases add container/operator/index/slice shapes, broad
call-free defaults, opaque-node rendering budgets, hidden descendants, malformed
grammar corrections, cross-family writes and replay. Seven ordinary-suite cases
add conditionals, loops/context managers including async forms, neutral binders,
owner/end restoration, alias visitation order, jumps, rollback and replay.
Eight exception/neutral-statement cases add ordinary/starred handlers, assertions,
raise/cause traversal, global/nonlocal, narrow delete and all augmented operators,
alias order, malformed grammar, hidden calls, metadata guards, isolation and replay.
Ten generalized-target cases add chained/destructuring assignments, AST-root write
eligibility, grouped annotations, neutral target traversal, shallow metadata guards,
arbitrary callees/receivers, argument grouping/unpacks, malformed forms and replay.
Ten closure-expression cases add lambda parameter roles and defaults, list/set/dict
comprehensions, generator expressions and direct generator arguments, neutral
binders/aliases, opaque budgets, hidden descendants, malformed operand roles,
exception aliases, resource isolation and replay.
Ten suspension/named-expression cases add exact Await/Yield/YieldFrom/NamedExpr
shapes, syntax roles versus grouping, AST-only contexts, await/power precedence,
nonwrites and alias neutrality, opaque budgets, defaults, guards and replay.
Twenty-two larger-batch cases compare matching/pattern families and guards, generic
declarations/type aliases, call-free broad headers, star/future imports, semicolons,
continuations, reviewed tabs, combined ownership/aliases, grammar boundaries and replay.
Twenty-two lexical cases compare ordinary/numeric spellings and f-string structure,
PEP 701 fields/specifications, decoded-payload redaction, decimal conversion limits,
pattern boundaries, malformed inputs, hidden calls, resource rollback and replay.

All one hundred and sixty-two oracle cases are **ignored in the ordinary suite**. Run the gate explicitly
with an absolute path to an installed, trusted Python >=3.12:

```sh
RUSTUP_AUTO_INSTALL=0 IDEOMANCER_TEST_PYTHON="$(command -v python3.12)" \
  cargo test --locked --offline --jobs 2 --lib analysis::python_parity \
  -- --ignored --nocapture --test-threads=2
```

An unset, missing, relative, old or non-Python selection fails rather than silently
skipping acceptance. The gate reports the actual interpreter version. Semantic
comparison keeps every fact field, ID, encoded value, diagnostic and array order;
interpreter metadata is checked separately and is never stripped from real evidence.
Fixture expectations are reviewed code, not automatically regenerated goldens.

### Private parse-only front-end

The parse-only entry point in `src/analysis/python_native.rs` operates on borrowed captured bytes. It has
no production caller, discovery switch or evidence-publication path. Dependency
pins are Tree-sitter **0.25.10**, Python grammar **0.25.0**, and language ABI bridge
**0.1.5**. The normal dependency compiles generated C parser/scanner code and adds
build cost; it is not a target Python runtime or an automatically installed tool.
The [retained grammar notice](third-party-notices.md) covers its reviewed MIT text.

Its private parse report contains parser identity, path/module, original-byte SHA-256,
bounded traversal counters and one classification/diagnostic per input. It returns
no tree, source text, fact records, Python runtime version, source-completeness flag
or trusted snapshot identity. `SyntaxAccepted` means the pinned CST has no error or
missing nodes and its visible-node traversal finished within bounds. It does **not**
establish CPython AST acceptance: for example, the grammar accepts Python 2 print
syntax which the Python 3 AST oracle rejects.

The front-end accepts UTF-8, optionally with a leading UTF-8 BOM. Digests always
cover original bytes, including that BOM. Effective encoding declarations on the
first two physical lines (LF, CRLF or bare CR) yield `UnsupportedEncoding`, even
when declaring UTF-8 or containing only ASCII. Cookie detection precedes UTF-8
validation; invalid UTF-8 without an effective cookie is also explicitly unsupported.
There is no silent Latin-1 decoding, transcoding or encoding-parity claim.

Input gates retain 5,000 files, 1 MiB/file and 16 MiB total source, reject invalid or
duplicate relative paths, and bound path/module metadata to 4,096 bytes each. A
1 MiB compact-JSON report allowance is reserved before parsing, including escaped
metadata and worst-case summaries. The file-count maximum does not guarantee that
every metadata set fits that separate report allowance.

One cooperative 60-second invocation budget covers validation, hashing, parser
input/progress callbacks, immediate post-parse checks and traversal. Cancellation
is latched independently of whether a tree was returned: EOF caused by a cancelled
input callback cannot become accepted partial syntax. A cursor walk counts visible
CST nodes, including punctuation/comments/error/missing nodes, with 200,000 nodes
per file and depth 256 (root depth zero). Trees are dropped between files. These
are not hard native-allocation or process-RSS limits: the parser may allocate
before traversal, and bounded digest/report finalization can continue after expiry.

Ordinary Rust tests exercise the parser without an interpreter:

```sh
RUSTUP_AUTO_INSTALL=0 cargo test --locked --offline --jobs 2 --lib analysis::python_native -- --test-threads=2
```

### Private declaration-fact candidates

A separate private candidate entry point uses
`src/analysis/python_native/facts.rs`. It observes the same counted CST enter/exit
events as the front-end: one parse and one walk per file, with no fresh deadline,
node allowance, second scan or second source read. Production extraction never
calls it. Its output has per-file outcomes and candidate facts, not `Evidence`,
source-completeness authority, a Python runtime version or a trusted snapshot.

The accepted file subset is deliberately small:

- Modules containing supported declarations, `pass`, comments and the limited
  expression/assignment and import statements described in the slices below.
- Ordinary `if/elif/else`, `while/else`, `for`/`async for`/`else`, and
  `with`/`async with` suites, now with reviewed general target binders. These do not
  create lexical scopes, binder-write facts or control-flow proof.
- Exception suites, assert/raise/global/nonlocal, and narrowly admitted del/augmented
  statements as described below. These preserve traversal without invented writes.
- Undecorated synchronous/asynchronous functions, including nested declarations,
  with bodies containing supported statements. Async declarations do not imply
  support for `await` or asynchronous execution analysis.
- Plain undecorated classes with single-line `class Name:` or explicitly empty
  `class Name():` headers, plus reviewed multiline/generic call-free headers, ordinary methods,
  and mixed nested class/function declarations. Admitted statements use their
  innermost lexical definition as owner. Supported annotated assignments use
  the bounded annotation grammar without inferring types or new runtime bindings.
- Positional, positional-only, keyword-only, `*args` and `**kwargs` parameters,
  with validated separators/order and no duplicate names. The bounded header
  annotations/defaults described below are optional; signatures still contain
  names/order/kinds only.
- ASCII class/function/parameter names. Unicode paths and supplied module metadata are
  preserved exactly; identifier normalization is not approximated.
- Space-only or pure-tab indentation lanes, LF/CRLF, token-separator tabs, reviewed
  explicit continuations and an optional UTF-8 BOM. Simple semicolon lists and
  multiline declaration headers with inline simple bodies are admitted.

Decorators, call-bearing declaration headers/bounds, type comments, later-version
type-parameter defaults, unreviewed lexical/layout and other syntax forms, and
all other unmodeled forms reject the **whole file**. Calls/writes/returns are
accepted only through the documented body slice; calls in declaration headers
remain unsupported. Calls in admitted statement conditions/iterables/contexts are supported.
So do non-ASCII identifiers,
NUL, bare CR formatting, mixed indentation styles and unsupported
whitespace outside comments. Some rejected forms are valid Python; rejection means
this candidate policy cannot prove whole-file fact parity, not that the AST helper
must reject them. Later syntax/resource errors are not hidden by an earlier
unsupported form.

`AcceptedFacts` is distinct from parse-only `SyntaxAccepted`. Each rejected file
retains only the path-stable module baseline, never an accepted declaration prefix.
Accepted candidates match every AST fact field and global ordering, including
Python's spaced ASCII-escaped JSON identity preimages, surrogate pairs, signature
strings, parameter spans and function ends excluding trailing comments. More than
128 counted parameters or a signature exceeding 16,384 bytes yields the helper's
exact `{"truncated":true}` signature value; parameter facts remain subject to the
ordinary transaction/resource gates. Variadic names are not counted in that 128 limit.

Candidate files retain their original-byte digest and borrowed path/module data.
Parser and collector implementation identities remain separate from production
evidence identity. The candidate identity also fingerprints the collector policy
and implementation; equal declaration facts do not erase differing captured inputs.

Existing input, 1 MiB summary, node/depth and cooperative invocation bounds remain.
Candidate output is separately capped at **100,000 facts including baselines**,
**100,000 warnings** and **32 MiB combined encoded output**, with another
**32 MiB pending working allowance** for owned frame text, parameter/signature
storage, persistent file-local aliases, retained warning storage and temporary construction. Admission
and borrowed baseline reservation precede allocation. A file that exhausts its
allowance rolls back to its module baseline; invocation cancellation also clears
earlier declaration candidates and warnings. These allowances do not bound total process RSS or
native parser allocation. Tests inject small limits and deterministic cancellation
checkpoints rather than relying on wall-clock timing.

This foundation does **not** establish full Python parity or a production native
backend. Broader expression/control-flow and encoding parity, full import/alias semantics, extended class forms,
constructor forwarding, FastAPI semantics, capability/identity design and default
replacement remain later gates. No existing AST expectations were relaxed.

### Bounded header annotations and literal defaults

The private `python_native/expressions.rs` helper normalizes only expressions
admitted by a narrow structural policy. It builds borrowed-span descriptors from
the same counted walk, not a second CST traversal or Python evaluation. Arena
growth, deferred parameter metadata and renderer scratch are charged before
allocation to the existing pending-work budget. Final rendered values are compacted
before entering output facts; literal contents are never retained as owned values.
The header expansion introduced the private candidate policy's second generation;
the body expansion introduced `python-native-private-facts-3`, and the import slice
introduced `python-native-private-facts-4`. Its parenthesized/multiline expansion
introduced `python-native-private-facts-5`; alias-aware warnings advance it to
`python-native-private-facts-6`. Plain classes/methods advance the policy to
`python-native-private-facts-7`; named bases and annotated assignments advance it to
`python-native-private-facts-8`. G1 core expressions and ordinary suites advance it
to `python-native-private-facts-9`; the G2 exception/neutral-statement slice advances
it to `python-native-private-facts-10`. Generalized targets/unpacking/calls advance
it to `python-native-private-facts-11`; lambda/comprehension/generator-expression
traversal advances it to `python-native-private-facts-12`; await/yield/named-expression
traversal advances it to `python-native-private-facts-13`. The larger match/generic/
import-layout batch advances it to `python-native-private-facts-14`; ordinary-literal
and f-string traversal advances it to `python-native-private-facts-15`. Expressions,
patterns and `literals.rs` participate in its fingerprint. Production identity is unchanged.

The initial annotation subset used ASCII names/dotted names, safe quoted text, `None`,
ellipsis, and reference-based generics with nested annotations or tuple slices.
For example, `list[Item]`, `dict[str, list[Item]]` and `tuple[Item, ...]` describe
syntax only, not resolved types or runtime identities. Grouping is normalized:
`T[A,]` renders `T[[A]]`, whereas `T[(A)]` renders `T[A]`. Standalone tuples,
unions, slices, starred arguments and other core expressions were initially
excluded. Policy 14 broadens call-free header/annotation normalization below;
raw grammar discrepancies and lexical restrictions remain explicit.

The original literal-default slice admitted only reviewed constants and one numeric
sign. G1 now admits call-free versions of the core expressions below, including
names such as `DEFAULT`, references, containers, operators and indexing. Constants
still render `<literal>`; unary expressions render `<UnaryOp>`, not their values.
Policy 15 removes the earlier 1,024-byte/512-digit subset limits. Borrowed scans
check Python-compatible bases, underscores and exponents without converting values.
The isolated Python 3.12 baseline's 4,300-digit conversion limit applies only to
nonzero decimal integers; all-zero integers and base-prefixed/float/imaginary forms
take their separately tested lanes. Existing source, traversal and working limits
still apply, and long scans are cancellable.

Policy 15 broadens strings/bytes to reviewed escaped and raw spellings, ordinary
and triple delimiters, physical joins, and f-strings described below. Bytes source
payloads remain ASCII; text/bytes mixing and later-version template strings reject.
Named Unicode escapes still need separately approved versioned name data. No new
literal-retention exception or runtime decoding/evaluation is introduced.

Each shape invocation uses the helper's shared 80-node expression allowance,
depth 12, first 16 tuple elements and 512-byte UTF-8 clipping at every rendered
node. These are rendering limits, not validation limits: every descriptor/CST
descendant is still validated. In declaration headers, calls hidden after a tuple's
16th item, beyond a truncated shape or beneath an exhausted expression budget still
reject the file. Admitted body expressions instead retain those calls independently.

Parameter and `parameter_annotation` facts share the AST argument extent, from
the name through its annotation, including annotation grouping but excluding any
default. Default/return facts use normalized AST-expression extents, excluding
redundant outer grouping. Return header tracking uses the last non-comment syntax
token, and physical line breaks require implicit bracket continuation; literal
payloads cannot spoof bracket depth. Positional defaults remain a suffix across
`/`; required keyword-only parameters may follow defaulted keyword-only parameters.
Variadic defaults and duplicate typed/untyped names remain rejected.

### Private body calls and lexical-write facts

The third private candidate policy adds a deliberately small body/module subset:

- Expression statements containing names/dotted references, reviewed scalar
  literals, supported calls, grouping or the G1 core expressions below. Plain
  docstrings are evidence-neutral.
- The original single-target subset used ASCII names and `self`/`cls` attribute
  chains. Policy 11 generalizes target syntax while keeping binding eligibility
  structural; neither is proof of a receiver type or dependency injection.
- Function-only bare/value returns. No `return` fact or return-dataflow edge is
  invented; contained calls are retained and statement ends extend function spans.
- The original name/dotted call subset is generalized by policy 11 to arbitrary
  admitted callees/receivers and reviewed argument unpacking, described below.

Assignments with normalized name/attribute/call RHS produce `binding` with compact
sorted-key `source`/`target` shapes. Other admitted RHS roots produce a
`binding_invalidation` containing **only** the target. Target shapes, including
plain names, follow the helper's 512-byte clipping. Binding ownership is the
enclosing module/class/function symbol, never a manufactured target declaration.

Calls render the callee first, then at most 16 positional and 16 keyword arguments,
with separate overflow ellipses. Every nested admitted call still gets its own
fact even when omitted/clipped in its parent's shape. Each independent call or
assignment-shape invocation has its own 80-node/depth-12/512-byte allowance; this
does not reset extraction's shared deadline, traversal, output or working limits.
Grouping preserves AST expression spans separately from complete statement spans:
an assignment can end after its nested call's closing parenthesis.

Unreviewed target/augmented/delete and other syntax forms remain unsupported.
Await/yield/named expressions follow the role-sensitive policy below, not a blanket
expression allowance. Calls in annotations/defaults still reject
the file. Whole-file rollback and later syntax/resource-error precedence remain.
Annotated assignments are admitted only through the bounded policy below, not as
an exception to target, expression or special-metadata guards.

Accepted files must also preserve the helper's complete fact/diagnostic contract.
Until dedicated support exists, the candidate rejects possible `FastAPI`/`APIRouter`
constructor calls, `include_router` attribute calls and router/prefix attribute-mutation
targets. Dynamic-discovery calls now produce the warnings described below. Constructor
guards check raw names **and**
fresh standalone callee shapes: clipping can itself create a special terminal name.
Ordinary converter-registration spellings remain ordinary call/binding evidence,
without registration interpretation or extra literal retention.

An additional 200-level structural delimiter gate accounts for enclosing function
parameters and call/generic arguments, not strings/comments or normalized rendering
depth. It prevents grouping accepted by the grammar from exceeding the Python
oracle's parser limit; existing CST/deadline gates can reject earlier. New state,
argument links, scratch, pending statement metadata and ID/JSON construction are
charged before allocation. Capture digests, single-parse/single-walk behavior and
transactional limits remain unchanged.

The oracle tests compare complete ordered AST facts, not selected call/binding
kinds. A **test-only** resolver fixture is constructed only after exact fact
equality, to compare direct-call links and unresolved/shadowed cases. It is never
persisted and supplies no production native-to-Evidence conversion. This preserves
current static resolver semantics, not general function-alias resolution, complete
whole-function local analysis, execution order or control-flow proof.

### Private bounded import facts

The import subset started with single-physical-line ASCII `import` and `from`
statements in module/function suites. The fifth candidate policy also admits
parenthesized `from` lists, on one or more physical lines. Dotted module names, absolute/relative
`from` levels, ordered comma-separated names and optional `as` aliases retain
their exact AST metadata. Each statement emits one `import`/`from_import` fact,
owned by its lexical module/function, with compact sorted-key JSON, explicit
`asname: null`, untruncated names and source-order duplicates. Imports extend
function spans but are not call, runtime-loading or binding facts. Parenthesized
statements end on the closing `)` line, even when the last name appears earlier;
trailing comments do not extend the span or enter metadata.

A parenthesized list must be nonempty, with exactly one enclosing pair and an
optional trailing comma. Spaces, LF/CRLF, blank lines and ordinary comments may
separate tokens inside the pair, including around `as`. Comment payloads are
opaque and nonsemantic, even when they contain punctuation, Unicode or backslashes.
Recognized type comments still reject the file. Before the opening delimiter,
the prefix remains on one physical line with the existing space-only rules;
implicit continuation is not a general whitespace relaxation. The shared walk
tracks the previous observed leaf independently from semantic token descriptors,
so comment contents cannot be reinterpreted as gaps or aliases.

The candidate now reproduces the helper's narrow imported-alias lookup for dynamic
warnings, not general name/type resolution. Ordinary calls/bindings keep unexpanded
shapes. Constructor metadata remains unsupported: every absolute import whose helper-
stored target ends in `FastAPI` or `APIRouter` rejects the whole file, even if unused,
shadowed or overwritten.
This is an exact, case-sensitive terminal test on the full target, not a truncated
shape or a library-identity claim. An unaliased `import a.b` stores only `a`; with
`as`, it stores `a.b`. A `from a import b` stores `a.b`. Relative imports never
populate the helper's alias table, so this extra target exclusion does not apply
to them. Existing raw/fresh-rendered special-callee guards still apply everywhere.

This conservative subset permits supported imports and calls together while
keeping constructor/framework side outputs outside the admitted boundary.
Policy 14 adds star/future imports, inline suites, semicolons and reviewed explicit
continuations. Unparenthesized import trailing commas, non-ASCII identifiers and
other unreviewed forms remain excluded. Token descriptors come from the same counted observer
walk; there is no second parse, source read, module import or source tokenizer.
Descriptor capacity, canonical text, name lists and JSON/ID temporaries are charged
before allocation, with shared cancellation and transactional limits unchanged.
Parser errors are never waived: the pinned grammar can reject CPython-valid
zero-indented list contents within a function's parenthesized import. An oracle
fixture records that difference explicitly as `SyntaxError` with module-only output,
rather than treating every Python-valid formatting choice as supported.

Oracle fixtures compare complete ordered fact and diagnostic vectors; earlier
warning-free cases additionally retain their empty-diagnostics assertions. Native
tests cover resource/cancellation rollback, exact walk counters and per-file
state/budget isolation. Full alias-dependent constructor/framework behavior remains
a later gate, not implied by retaining import syntax.

### Private alias-aware dynamic warnings

The sixth policy adds a separate private report `diagnostics` vector with the
helper's `dynamic_discovery_unresolved` records. This is distinct from each file's
failure classification. A supported dynamic call does not fail the file, execute a
module or infer a runtime target. Its message is exactly
`line N: EXPANDED_CALLEE; runtime targets are not inferred`.

For every admitted call, a **fresh** callee-shape rendering uses the existing
80-node/depth-12/512-byte-per-node limits. The helper's lookup substitutes only the
head before the first dot, once; the full imported target is not clipped again.
Warnings use the resulting terminal name: `__import__`, `import_module`,
`iter_modules`, `walk_packages`, `entry_points`, `load_entry_point`, `find_spec`,
`exec` or `eval`. This is a spelling rule, not proof of a specific library.
A safe import can suppress a raw `eval` warning; a long identifier can clip to a
different imported alias. Ordinary call/binding facts remain unexpanded/redacted.
Nested calls still warn even when their parent's shape omits or clips them.

Aliases are keyed by explicit class/function-name scope components, independent of
supplied module text or declaration identity. Lookup checks the nearest scope,
enclosing prefixes, then module scope. Entries persist until file end, so repeated
same-named definitions reuse earlier aliases, including class/function names that
coincide. Imports update entries in visitor
order; parameters, assignments, literal invalidations and definitions do **not**
invalidate them. Relative imports do not update or erase them. These are deliberate
AST-helper compatibility rules—not Python runtime binding semantics or an upgrade
to the focused resolver's separate lexical proof rules. Calls in headers remain
unsupported, avoiding a claim about their additional AST visitation-order effects.

Warnings sort globally by `(path, code, message)`, retaining duplicates even when
identical call facts deduplicate. Line numbers sort as message text (`10` before
`2`), exactly like the helper. Full targets, interned scopes, alias map capacity,
replacement storage, warning-vector high-water capacity, strings and renderer
scratch are charged before allocation. Hashed alias keys retain their full spelling
and fail closed on a digest collision; long copies/comparisons are cancellable.
Per-file rejection rolls back facts and warnings together. Invocation cancellation
or final output exhaustion clears all warnings and non-module facts. No production
Evidence conversion, completeness authority or public backend switch is introduced.

### Private plain classes and methods

The seventh policy admits undecorated ASCII classes with single-physical-line
`class Name:` or explicitly empty `class Name():` headers. Classes are lexical
containers only. They emit the existing `class` fact, with the raw class name,
full dotted lexical symbol and complete statement-derived source span. Methods,
including `__init__` and async definitions, emit ordinary function/signature/header
facts. There is no new `method` fact, implicit `self`, class signature, inferred
constructor edge or receiver type. Empty class parentheses add no base fact.

Class suites reuse the admitted docstring/pass/expression/import/simple-write and
nested-declaration subset. Ordinary class assignments emit existing bindings or
target-only invalidations owned by the class, not manufactured attribute declarations.
Class, method, local-function and local-class nesting preserve every lexical name
component, without inserting `<locals>`. The shared definition stack restores the
parent owner after each child. Class-inclusive alias prefixes reproduce helper
warnings even where Python runtime class-name lookup differs.

The final body statement determines each declaration's end; nested declarations,
evidence-neutral statements and multiline imports propagate their exact ends to
the parent, while trailing comments do not extend them. Each class frame, retained
symbol and persistent alias scope is charged before allocation. Classes never
allocate the function-signature buffer or bypass the counted traversal/deadline.

The initial class slice excluded bases and annotated assignments; the next slice
below admits only named-reference bases and supported annotations. Metaclass
keywords and type parameters are generalized by policy 14 below. Decorators and
class-header calls still reject the entire
file. Returns require an immediately enclosing function; a class nested in a
function does not inherit return permission. This is the candidate's deliberately
narrow policy, not a claim that `ast.parse` performs compilation validation.
Existing constructor/router exclusions and all expression/control-flow boundaries
remain. Full ordered fact-and-warning parity, resource rollback and captured-byte
replay do not imply complete class semantics or production-native availability.

### Private named bases and annotated assignments

The eighth policy adds `base` facts for ASCII names/dotted references, optionally
grouped, in the existing single-physical-line class headers. Comma-separated bases
and a final comma are supported; an empty list must be exactly `()`, never `(,)`.
Each base fact belongs to the newly declared class and uses the normalized base
expression's span, excluding redundant outer grouping. Identical same-span facts
deduplicate and final ordering follows the helper, not source-list order. Base
names are not resolved through imports or treated as proof of runtime inheritance,
method resolution, constructor forwarding or receiver types.

Annotated assignments admit direct ASCII names and the existing `self`/`cls`
attribute-chain targets, with the bounded call-free header-annotation grammar.
Optional values use the existing body-expression grammar, not the narrower
parameter-default grammar. A direct name under the immediately enclosing class
emits `attribute_annotation`, with the **raw, unclipped** field name appended to the
class symbol. Its span covers the entire annotated statement, including a grouped
annotation and optional multiline RHS. Name annotations in a module/function and
attribute targets such as `self.field` do not emit class-attribute facts.

An annotation without an RHS emits **no binding or invalidation**. It still extends
the enclosing declaration's span, and a direct class name still emits its attribute
annotation. With an RHS, binding/invalidation shapes and complete-statement spans
remain unchanged; binding targets follow their existing 512-byte clipping even
when the attribute symbol retains the full identifier. Annotated RHS calls retain
their own normalized spans and dynamic warnings. Annotations/writes do not erase
the helper-compatible persistent import aliases. Literal values remain redacted.

Before allocation, the collector charges bounded annotation storage, raw-symbol
construction and rendering scratch to the shared working/output allowances. Bases
and assignment annotations use separate implicit-continuation depths; assignment
newlines require actual brackets, not the presence of a colon. Every descendant
is validated even if rendering clips or exhausts its shape budget.

Policy 14 generalizes call-free bases/keywords and annotation forms, including
reviewed multiline headers. Call-valued bases and calls anywhere inside annotations
remain excluded. Constructor,
include-router and router-mutation guards apply to annotated statements too:
annotation-only `self.router: T` or `self.prefix: T` cannot silently lose the helper's
extra metadata. Unsupported syntax, later errors and resource exhaustion roll back
the whole file's facts and warnings, with no production backend or schema change.

### G1 bounded generic expressions and ordinary bodies

Policy 9 separates expression validation/descendant collection from bounded shape
rendering. Body values admit tuple/list/set/dict displays, bare comma tuples, all
ordinary binary operators, boolean/comparison/unary/not expressions, conditionals,
and subscripts/slices/multidimensional indices. Defaults admit their call-free forms;
bases and annotations retain their narrower reviewed policies. No expression is
evaluated, and broader acceptance does not infer types or execution.

Tuple/list/set shapes render the first 16 elements with no overflow ellipsis.
Dictionaries, boolean/comparison/unary/conditional expressions and slices use the
helper's opaque AST-node placeholders without spending their descendants' rendering
budget. Binary operators retain AST operator names and recursive budget accounting.
Every admitted descendant is nevertheless validated, and every body call/warning
is retained independently—even after clipping, depth/budget exhaustion or item 16.
Bindings still depend on the RHS AST root: containers/operators/subscripts invalidate
eligible targets instead of being promoted to bindings by their rendered appearance.

Ordinary `if/elif/else`, `while/else`, `for`/`async for`/`else`, and
`with`/`async with` suites traverse headers and bodies in helper-equivalent order.
Multiple context items are supported. Loop targets and `with as` binders are
initially direct ASCII names only and are generalized by policy 11 below;
parenthesized context-item lists remain excluded.
These constructs create neither definition scopes nor binding/invalidation facts
for their binders. Branches and loop-else bodies all contribute observed syntax;
none is claimed to execute. Imports keep the helper's persistent alias behavior.

`break`/`continue` are evidence-neutral and require an enclosing loop body within
the current definition boundary; a loop's own else suite is not that loop's body.
Compound ends propagate through clauses and nested declarations, excluding trailing
comments. New frame high-water storage is charged before allocation and reset per
file. Physical keyword/colon placement and clause attachment remain checked rather
than assumed from grammar acceptance; implicit continuation requires actual brackets.

G1 alone did not close G2; the sections below track the later target, unpacking,
closure and suspension-expression expansions. Remaining statements/import forms,
full strings/identifiers and declaration-header calls are still open, as are G3
metadata and G4 compatibility. Grammar-only invalid forms such as comma-only dictionaries and ungrouped
conditionals in a conditional's test reject transactionally. Bare tuple statements
are validated as complete value/comma sequences, not by silently ignoring punctuation.

### G2 exception paths and evidence-neutral statements

Policy 10 adds `try` with ordinary or starred handlers, optional `else`/`finally`,
and `try/finally`. Handler families cannot mix; starred handlers require a type,
and `else` requires a handler. Explicit per-frame progress validates clause order,
attachment, indentation and header shape independently of grammar acceptance.
Python 3.14's unparenthesized multi-type handler form is outside the development
3.12 lane; parenthesized exception tuples use the shared core-expression policy.

The helper visits try body, handlers, else and finally in that order. Each handler's
type is visited before its body. The candidate preserves that order, including
imports that affect later alias warnings. Handler names are binding strings in
the AST, not expression descendants or new lexical scopes. They do not invalidate
or redirect the helper's persistent import aliases. A handler's conditional type
may place its alias on the CST's final-else chain; bounded descriptors normalize
that wrapper without visiting the alias as an expression or losing nested calls.

`assert` admits one test and an optional message; `raise` admits a bare statement,
one exception, and an optional `from` cause. Every admitted operand contributes its
descendant calls/warnings, even in opaque shapes or paths that may not execute.
`global`/`nonlocal` admit nonempty direct ASCII identifier lists, including duplicates.
They neither emit facts nor change alias lookup. Bare raise and unresolved/conflicting
scope declarations are not rejected merely for failing compile-time/runtime checks:
the oracle is `ast.parse`, not compilation or execution.

`del` admits one existing direct-name or `self`/`cls` attribute-chain target.
Augmented assignments admit that same subset with any ordinary augmented operator
and a supported body-value RHS. A validation-only `NeutralTarget` path emits **no
binding or invalidation**, and neither statement erases imported aliases. The
helper only adds router-mutation metadata for these statements; affected targets
remain whole-file exclusions until G3 rather than silently losing those facts.
Chained/destructuring/multiple deletion, subscript/arbitrary-receiver targets,
target-side calls and recursive assignment RHS forms remain excluded.

All new statements extend enclosing declaration/suite spans through their actual
last syntax statement, not trailing comments. Exception paths retain existing
function-only return and loop-body jump boundaries. New progress/normalization
state is charged through the counted frame/descriptor arenas; cancellation, fact/
warning/output limits, per-file rollback and later-error precedence remain shared.
This closed one cohesive part of G2; subsequent policies expand targets/callees,
closures and suspension/named expressions. The remaining inventory includes `match`,
imports, declaration headers and literal/identifier compatibility.

### G2 generalized targets, unpacking and call forms

Policy 11 separates valid target syntax, binding eligibility and top-level metadata
guards. Ordinary assignment admits chains and nested tuple/list/starred targets,
grouped scalars, arbitrary attribute receivers and subscript targets. The final RHS
is shared by every target, and every write fact uses the full outer statement span.
Chains do not grant physical line joining after `=`; grouped targets require their
own actual delimiters for continuation.

Only a Name or an Attribute chain reaching the name `self`/`cls` by peeling
**attributes alone** is binding-eligible. Other targets emit no writes but retain
all descendant calls/warnings. Entering Tuple/List/Starred makes the destructured
state sticky: eligible leaves invalidate rather than bind. Scalar parentheses are
transparent. Non-destructured eligible targets bind only for RHS AST roots Name,
Attribute or Call; rendered appearance is not a substitute for this rule.
Class annotation facts recognize normalized grouped Name targets and retain raw
unclipped field symbols; annotation-only statements still invent no write.

For/with binders use general reviewed Store targets without binder facts or alias
invalidation. Augmented assignment accepts scalar Name/Attribute/Subscript targets;
deletion accepts multiple/nested tuple/list targets but no Starred. Their target
descendant calls are visited, with no generic binding/invalidation facts. Exception
handler aliases remain direct names, not general targets.

Router mutation checking follows the helper's **shallow top-level** target list.
An actual top-level attribute requiring that metadata rejects the file until G3,
regardless of binding eligibility. Destructuring leaves are not promoted to
top-level mutation targets, and for/with binders invoke no mutation hook. The
candidate also conservatively rejects raw interior-router chains on top-level
attributes. Existing constructor and structural `include_router` exclusions apply
to arbitrary receiver/callee shapes, not just name/dotted references.

Calls now admit arbitrary supported callees and attribute receivers. Shapes and
warnings continue to use independent fresh rendering budgets. Starred positional
arguments are grouped before all keywords/double-stars, even when source order
interleaves them. Double-star keywords render `**=VALUE`; positional and keyword
groups retain separate 16-item caps/overflow markers. Duplicate explicit keywords
are admitted because the oracle parses ASTs rather than compiles calls. Invalid
plain-positional-after-keyword and iterable-unpack-after-keyword-unpack orders reject.
Tuple/list/set Starred displays and opaque dictionary unpacking retain all descendant
calls, including beyond rendered items. Starred subscript indices form an AST tuple
even for one element without a comma.

Unparenthesized Load-Starred roots follow AST `star_expressions` roles, including
assignment/annotated/augmented RHS, expression statements, returns and for iterables;
they are not generally permitted in conditions, asserts, raises, keyword values or
defaults. Compilation may reject some AST-only forms. Descriptor normalization
preserves nested call spans while lifting CST splats over receivers/calls/operators.
The documented standalone starred-conditional grammar discrepancy remains explicit.

Target records, strings and temporary traversal state are charged before allocation;
argument grouping reuses descriptor links instead of allocating group vectors.
One parse/one counted walk, deadlines, per-file facts/warnings rollback and source-
deleted replay remain unchanged. G2 is still open for the remaining expression,
statement, declaration-header, import and lexical inventory; production is unchanged.

### G2 lambdas, comprehensions and generator expressions

Policy 12 adds `Lambda`, `ListComp`, `SetComp`, `DictComp` and `GeneratorExp`.
Each uses the helper's exact opaque `<ASTClass>` shape and consumes just its own
render node, not its descendants' shape budget. All descendant syntax is still
validated; every call/warning in lambda defaults/body and comprehension result,
key/value, target receiver/index, iterable and filters is collected independently.
An opaque root produces invalidation for an eligible assignment target, while
calling it or selecting an attribute preserves the existing Call/Attribute-root
binding rule. No closure is evaluated or inferred to run.

Lambda arguments have their own parameter-role validator: no annotations or tuple
parameters, direct ASCII names for variadics, valid `/`/`*`/`**` phase ordering and
positional-default suffixes. Duplicate argument names remain AST-parse-compatible
rather than being rejected by a compiler-only check. No function, signature,
parameter/default fact, `<lambda>` owner or alias scope is manufactured. Nested
defaults stay inside the active expression arena instead of entering the enclosing
declaration's parameter collector. A surrounding Default mode remains recursively
call-free, including through every new opaque subtree.

Comprehension targets reuse neutral Store validation and add no binding,
invalidation or router-mutation hook. Multiple generators, filters and `async for`
clauses are admitted without runtime async-scope validation. Lambda parameters
and comprehension binders do not shadow/invalidate the helper's persistent import
aliases. Their calls retain the enclosing module/class/function owner. This
preserves helper behavior, not Python runtime closure/comprehension name resolution.

Raw CST wrappers distinguish grouping before aliases are collapsed: unparenthesized
lambda/conditional iterables or filters, comma-list iterables, bare-star results,
and unpacking dict-comprehension results remain invalid. Bare lambda operands also
require grouping in boolean/not operand roles and conditional tests. Legal lambda
bodies/final-else expressions and explicitly grouped alternatives stay admitted.

A direct generator argument, `sink(item for item in source())`, normalizes to one
positional `<GeneratorExp>` argument without an ordinary argument-list wrapper or
extra allocation. It must be the sole unparenthesized argument, with no trailing
comma. Explicitly grouped generators permit ordinary sibling arguments. Outer
call spans include the closing delimiter; nested calls retain their own AST spans.
Conditional/lambda trailing exception-type aliases are normalized without treating
the handler's name as an expression or visiting aliases inside explicit grouping.

Helper lambda-default/comprehension field order is not runtime order. For the
currently admitted expression families it cannot change import aliases; independent
calls and warnings are globally sorted. Source-order descriptor enumeration is
therefore observationally equivalent here, not approval to enable call-bearing
declaration headers without their separate scheduling work.

Structural parameter/clause state uses the existing precharged descriptor arena;
no per-lambda scope, subtree CST traversal, parser invocation or literal copying is
added. Shared deadlines, rendering/working/output limits, hidden metadata guards,
whole-file rollback and captured-source replay remain. The following slice adds
await/yield/walrus; `match`, remaining headers/imports and lexical coverage still
belong to G2, while G3–G6 are open.

### G2 await, yield and named expressions

Policy 13 adds exact opaque `<Await>`, `<Yield>`, `<YieldFrom>` and `<NamedExpr>`
shapes. Each consumes one rendering node, without rendering its operands, but all
descendant calls/warnings remain independently validated and retained. Opaque RHS
roots invalidate eligible ordinary assignment targets; enclosing Call/Attribute
roots retain their existing binding rule. No generator, asynchronous execution,
resumption, or new runtime responsibility is inferred.

The collector supplies finite syntax roles, and descriptors validate raw CST
wrappers before grouping aliases are resolved. A bare named expression is admitted
in named-expression roles such as if/while tests, positional arguments, display
elements, selected comprehension results and subscript items. Ordinary expression
roles—assignment RHS, return/assert/raise/default/lambda operands, keyword values,
dict pairs, comprehension iterables/filters, operator operands and slice bounds—
require grouping. The walrus target is one unparenthesized ASCII name, never a
general Store target. Its AST traversal adds **no binding, invalidation, attribute,
router-mutation fact, alias update or lexical scope**; even an imported alias
rebound by walrus keeps the helper's later diagnostic behavior.

Bare yield is admitted for expression statements and ordinary/annotated/augmented
assignment RHS. Other expression positions require grouping. Yield has an optional
star-expression value; yield-from requires one ordinary expression, and its `from`
is recognized as syntax rather than literal text. These roles do not impose
compiler/runtime context checks: selected module/synchronous/lambda/comprehension
and call-free default forms can construct ASTs even if they cannot compile or run.
Existing private return/loop-jump restrictions remain unchanged.

Await takes a primary operand; bare await/unary/starred and other wider operands
need grouping where Python requires it. The pinned grammar associates power inside
await, unlike Python. Bounded descriptor rotation normalizes
`await left ** right` as `<Await> Pow right`, while `await (left ** right)` stays
`<Await>`. Right-associative chains, signed exponents, nested await and multiline
call spans retain the oracle's depth/node-budget behavior. Rotation reuses a charged
descriptor without another CST walk, synthetic call duplication or reparse.

Defaults stay recursively call-free through all new opaque nodes; bases and
annotations retain their narrower policies. Hidden constructor/include-router and
ordinary write-mutation guards remain active. Syntax-role errors roll back the
file's facts and warnings; admission, counters, shared deadlines, pending/output
limits, cancellation and captured-source replay remain unchanged. G2 still needs
`match`, remaining declaration/header/import forms and lexical coverage. G3–G6,
including the documented parser/encoding blockers and production migration, are open.

### Integrated G2 matching, declarations, imports and layout batch

Policy 14 integrates several coverage families in one validation/handoff batch.
Structural matching admits all eight pattern AST classes—MatchValue, MatchSingleton,
MatchSequence, MatchMapping, MatchClass, MatchStar, MatchAs and MatchOr—and their
reviewed composition. The separately fingerprinted `python_native/patterns.rs`
validator consumes the same counted events and borrowed spans. Patterns emit no
facts, bindings, alias changes or scopes. A class pattern is not a call. Subject
and guard descendants retain ordinary calls/warnings, and case bodies keep the
current explicit definition owner. Earlier case-body imports affect later guards
in helper visitation order; captures do not shadow those aliases.

Pattern grammar validation is distinct from Store targets and compiler checks.
Open sequences require their comma structure, mapping rest and class keyword
patterns obey their syntactic roles, and signed/complex/string literals retain the
reviewed lexical policy. A starred match subject must form a comma-bearing tuple;
named-expression subjects/guards keep their appropriate raw syntax permissions.
The existing function-only return and definition-bounded loop-jump subset remains.

Python 3.12 `type` aliases and declaration type parameters are now admitted.
TypeAlias emits no declaration, binding, invalidation, class attribute or alias-table
update; its RHS uses ordinary body traversal and may contain admitted calls.
Generic functions/classes retain their usual declaration/signature/header facts,
with no extra type-parameter facts or scopes. TypeVar bounds/constraints and
TypeVarTuple/ParamSpec names are validated separately from subscription slices;
later-version type-parameter defaults remain excluded. Bounds remain recursively
call-free because function/class bound calls require deferred-header scheduling.

Parameter/return/assignment annotations, class bases and class keyword values use
broader normalization over the already admitted expression and literal families,
while rejecting every call descendant. Keyword/metaclass/double-star values emit
no base facts; only positional bases, including Starred, do. Positional class
arguments preserve NamedExpr and argument-specific unpacking permissions without
relaxing keyword values or display unpacking. Multiline headers and inline simple
bodies preserve exact argument, expression and declaration spans. Grouping is not
a repair applied to target source: passing grouped annotation fixtures and native-
rejected ungrouped grammar discrepancies are recorded as different inputs.

Star imports and `__future__` imports emit the existing exact import JSON, including
literal `*` entries and ordinary absolute spelling-based alias entries. They neither
expand imported bindings nor stringify future annotations. Inline simple imports,
parenthesized/continued import lists, simple semicolon lists and a trailing semicolon
are supported; empty/leading/double semicolons and inline compounds still reject.
Observed continuation tokens must be exactly backslash-LF or backslash-CRLF. They
bridge logical syntax rows without extending AST spans or authorizing unrelated
newlines; the grammar's NUL continuation alternative is never admitted.

Token-separator tabs and pure-tab indentation join the existing pure-space lane.
Mixed indentation/style files remain explicit subset rejections: the pinned
scanner's tab handling is not claimed to equal CPython tab-stop/ambiguity behavior.
Non-ASCII identifiers and encoding changes remain outside this batch. F-strings
were not admitted by policy 14 merely because a renderer could describe them;
the following lexical batch adds explicit structural validation and traversal.

Patterns, declaration progress and layout state are charged before allocation,
share cancellation/deadline/node limits, and reset per file. Unknown nodes still
fail closed; later syntax/resource errors retain precedence and failed files lose
facts and warnings together. New oracle modules are independent synthetic source,
not consumer fixtures. Header calls, decorators/G3 side outputs, remaining lexical
coverage, parser/encoding decisions and production integration are still open.

### G2/G4 ordinary literals and f-string traversal

Policy 15 shares a borrowed-token lexical validator between expressions and
pattern literals. Numeric categories are checked without allocating numeric values.
Ordinary text/bytes validate prefixes, quote/backslash parity, raw/triple forms,
hex/Unicode widths, physical joins and implicit concatenation. Escaped surrogate
code points are accepted without converting them to Rust `char`. Unknown escapes
and oversized octal spellings follow AST acceptance rather than being promoted
from warnings to errors. Nonraw text `\N{...}` requiring name lookup remains an
explicit unsupported-data boundary; balanced braces never imply a valid name.
Raw text and bytes have distinct, tested `\N` behavior. No decoded literal bodies
enter facts, warnings, identifiers or owned pending payload storage.

Formatted prefixes are classified from validated starts, so `f''` and plain f-string
text remain `JoinedStr`. Adjacent text/f-string literals promote to `<JoinedStr>`;
bytes mixing rejects. The renderer consumes one opaque node without visiting its
children for shape budget, while counted traversal retains every call/warning in
replacement values, nested f-strings and `format_expression` specification fields.
Conversions admit only `!s`, `!r`, `!a`; debug/conversion/specification progress is
validated independently of the grammar's broader token choices.

PEP 701 quote reuse, comments and multiline replacement expressions are covered
without treating the entire f-string span as an ordinary literal. Short literal
segments retain their newline rules. Format text is not parsed as code, whitespace
or delimiters, and field braces are separate from the global expression delimiter
allowance. Literal segments are validated once through the observed events. A
lambda colon at raw field lexical depth rejects even when hidden under an implicit
tuple/conditional; actual expression delimiters preserve valid grouped lambdas.

Call-free headers/defaults remain recursively call-free across every value and
format specification. JoinedStr RHS roots invalidate eligible ordinary targets;
enclosing Call/Attribute roots preserve their usual binding eligibility. No f-string
scope, alias update or literal-retention permission is introduced. Hidden G3
constructor/include/mutation guards, shared counters/budgets, per-file rollback
and deleted-source replay remain intact.

Two further boundaries stay explicit rather than being counted as accepted parity:

- For `f'{x:=eval()}'`, Python treats `=eval()` as format text, but the pinned CST
  constructs a NamedExpr with a spurious call. `f'{x:={eval()}}'` also loses field
  boundaries. Native rejects these transactionally; it never emits the wrong calls.
  Correctly structured control `f'{x: ={eval()}}'` is separately tested as accepted.
- `ast.parse` can construct formatted-string MatchValue patterns that compilation
  rejects. Native formatted-pattern expression traversal is still a G2 subset gap;
  its rejection is not described as an AST syntax failure.

Unicode identifier/NFKC coverage requires consistent versioned normalization across
names, shapes, scopes, aliases, guards and embedded JSON—not an ASCII-predicate
relaxation. Neither it, Unicode-name tables, source decoding nor existing parser
compatibility blockers are closed by this lexical batch. The production helper,
public requests, schemas, saved evidence and default selection are unchanged.

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
