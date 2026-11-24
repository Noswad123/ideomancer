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
| P1-G2 | Close the generic visitor inventory: remaining statements, targets/writes/imports, arbitrary callees/splats, comprehensions/lambdas/generators, await/yield/walrus, all headers and literal/identifier forms. Every target-version node has a normalization/traversal rule and cross-family oracle cases. | In progress: exceptions, generalized targets/unpacking/calls locally validated; remaining inventory open |
| P1-G3 | Preserve every existing discovery/framework side output: decorators, CLI/job/API entry facts, route diagnostics and FastAPI host/prefix/include/mutation metadata, including aliases and literal-retention limits. | Pending |
| P1-G4 | Close grammar/version/encoding/failure differences: original-byte digests, decoding/cookies, physical layout and exact observable parse diagnostics. No unexplained valid-file rejection or invalid-file acceptance in the approved compatibility suite. | Open compatibility blockers |
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
- Decide encoding coverage explicitly. The private candidate currently rejects all
  effective cookies, even UTF-8, while the production byte parser supports more.
  A narrower production contract needs an explicit decision, not a silent migration.
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

All ninety-eight oracle cases are **ignored in the ordinary suite**. Run the gate explicitly
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
  `class Name():` headers, plus bounded named-reference bases, ordinary methods,
  and mixed nested class/function declarations. Admitted statements use their
  innermost lexical definition as owner. Supported annotated assignments use
  the bounded annotation grammar without inferring types or new runtime bindings.
- Positional, positional-only, keyword-only, `*args` and `**kwargs` parameters,
  with validated separators/order and no duplicate names. The bounded header
  annotations/defaults described below are optional; signatures still contain
  names/order/kinds only.
- ASCII class/function/parameter names. Unicode paths and supplied module metadata are
  preserved exactly; identifier normalization is not approximated.
- Space-based indentation, LF/CRLF and an optional leading UTF-8 BOM. Parameters
  may span lines, but a multiline header with an inline body is not supported.

Decorators, type parameters/comments, unreviewed base expressions, metaclass
keywords, unreviewed annotation/assignment/import and compound-statement forms, and
all other unmodeled forms reject the **whole file**. Calls/writes/returns are
accepted only through the documented body slice; calls in declaration headers
remain unsupported. Calls in admitted statement conditions/iterables/contexts are supported.
So do non-ASCII identifiers,
semicolons, explicit line continuations, NUL, bare CR formatting and unsupported
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
it to `python-native-private-facts-11`, with the expression
implementation included in its fingerprint. Production identity is unchanged.

Supported annotations are ASCII names/dotted names, safe quoted text, `None`,
ellipsis, and reference-based generics with nested annotations or tuple slices.
For example, `list[Item]`, `dict[str, list[Item]]` and `tuple[Item, ...]` describe
syntax only, not resolved types or runtime identities. Grouping is normalized:
`T[A,]` renders `T[[A]]`, whereas `T[(A)]` renders `T[A]`. Standalone tuples,
unions, slices, starred arguments, list/set/dict expressions, subscripted callees
such as `T[A][B]`, and arbitrary expression nodes remain unsupported.

The original literal-default slice admitted only reviewed constants and one numeric
sign. G1 now admits call-free versions of the core expressions below, including
names such as `DEFAULT`, references, containers, operators and indexing. Constants
still render `<literal>`; unary expressions render `<UnaryOp>`, not their values.
Numeric tokens remain limited to 1,024 bytes; decimal spellings are
limited to 512 digits and checked lexically for Python-compatible bases, underscores
and exponents, without converting values.

Strings use no backslashes, including raw strings. Reviewed prefixes are absent,
`r`, `u`, `b`, `br` or `rb`, case-insensitively; bytes bodies must be ASCII.
Adjacent tokens must all be text or all bytes. Ordinary/triple-quoted forms are
supported, but single-delimiter strings cannot contain physical LF/CR. Formatted
strings, template strings, mixed text/bytes and unreviewed escapes/prefixes reject
the whole file. No new literal-retention exception is introduced.

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

Unreviewed target/augmented/delete forms, assignment expressions, `await`,
yield/generators, lambdas and comprehensions remain unsupported. Calls in annotations/defaults still reject
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
Star imports, unparenthesized trailing commas, `__future__` statements,
inline import suites, non-ASCII identifiers, explicit continuations and unsupported
whitespace remain excluded. Token descriptors come from the same counted observer
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
keywords, decorators, type parameters and class-header calls still reject the entire
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

Generic/subscripted, starred, keyword/metaclass, literal and call-valued bases;
multiline class headers; unions/unreviewed annotation
expressions; and calls anywhere inside annotations remain excluded. Constructor,
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
currently direct ASCII names only; parenthesized context-item lists remain excluded.
These constructs create neither definition scopes nor binding/invalidation facts
for their binders. Branches and loop-else bodies all contribute observed syntax;
none is claimed to execute. Imports keep the helper's persistent alias behavior.

`break`/`continue` are evidence-neutral and require an enclosing loop body within
the current definition boundary; a loop's own else suite is not that loop's body.
Compound ends propagate through clauses and nested declarations, excluding trailing
comments. New frame high-water storage is charged before allocation and reset per
file. Physical keyword/colon placement and clause attachment remain checked rather
than assumed from grammar acceptance; implicit continuation requires actual brackets.

G1 does not close G2: starred displays/dictionary unpacking, broader call targets
and splats, complex assignment/loop/context targets, comprehensions/lambdas,
await/yield/walrus, remaining statements/import forms, full strings/identifiers and
declaration-header calls remain. G3 metadata and G4 compatibility blockers are also
unchanged. Grammar-only invalid forms such as comma-only dictionaries and ungrouped
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
This closed one cohesive part of G2; targets/callees/splats are expanded by policy 11
below. The remaining inventory includes `match`, comprehensions/lambdas/generators,
await/yield/walrus, imports, declaration headers and literal/identifier compatibility.

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
