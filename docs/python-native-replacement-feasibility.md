# Native Python Replacement: Byte Input And Version Feasibility

## Status And Authorization

**Direction selected: full replacement, preserving the existing production
contract.** The user authorized this bounded byte-aware Ruff and version-strategy
investigation after the private G2/G3 implementation checkpoint.

**Current: bounded private F1 lexer goal COMPLETE / PROVED by compiled tests;
all F1 delivery gates PASS, including checkout consumer and guarded cache cleanup.**
The [F1 ledger](python-analysis.md#f1-shared-byte-lexer-experiment)
records the shared fallible byte core, frozen baseline, tests and provenance. F0's
conditional GO/uncompiled finding below is historical and superseded only for
lexical proof, not parser integration or replacement readiness. Unchanged Ruff
`&str` input remains insufficient for malformed comment bytes.

This investigation does not authorize that expanded production patch, a parser
replacement, source repair, tool installation, public profile/API changes,
automatic fallback, a default switch or a narrower production contract. Existing
frontend-5/facts-7 policies are preserved. Production remains
`python-static-8-generic-facts` with the explicitly selected Python >=3.12 helper.
F1's bounded vendor expansion is now authorized; F2 parser/source/debug changes
and version/product policy still require separate approval. The user-reported
actual pre-F1 HEAD is `fdf4e58` (facts-6 commit, initially clean/ahead one), not the
historical `523cd7b` checkpoint. Current F1 changes remain uncommitted and unstaged.
F1 completion does not prove byte parser input, AST/debug construction, CPython
admission, F2 or full G4, and does not authorize further implementation.

## F1 Current Lexical Evidence

### Overview

One `CoreLexer(&[u8], byte cursor)` serves legacy `Lexer(&str)` and the doc-hidden
experimental `pub lexer::byte::ByteLexer`. There is no separate lexer, source
repair/masking/substitution, unchecked UTF-8, parser/AST/debug construction,
codec/type-comment semantic, facts dispatch or `engine::v1` change. Legacy
signatures/defaults and all four modes remain; malformed-byte guarantees cover
Module only. Production still uses the helper; frontend-5/facts-7 names stay fixed,
while implementation identity changes transitively through vendor provenance.

### API / Interfaces

The byte facade is fallible and session-checkpointed. Byte views are range-checked
zero-copy borrows; `source_text(&mut self, range)` meters work and polls cancellation.
Invalid UTF-8 views do not poison lexing; view resource/cancellation failures latch.
Defaults are 2 MiB input, 32M work units, 200000 tokens, depth 256, 32 MiB cumulative
conservative allocation charges, 1024 checkpoints and 10000 diagnostics. These are
cooperative limits, not live RSS or hard deadlines; the final precharged safe UTF-8
check remains linear and uninterruptible (at most 2 MiB under default input limits).

### Data Flow

Scalar, malformed input and EOF are distinct. Only existing `lex_comment` consumes
opaque bytes, including replacement-expression comments. String/outer/format text
faults are structured `SourceFault`s, never Python certificates. Session-owned
checkpoints restore token/value/taken state, cursor/token start/previous opaque
state, diagnostics and indentation/interpolation stacks even non-LIFO. A prior
owned checkpoint can clear a source fault, but never cancellation/resource latches
or cumulative work/token/allocation/checkpoint charges. Re-lex rejects offsets
at/after the cursor and non-newlines. Original pointers, bytes and ranges remain.

### Edge Cases

Raw NUL is a scalar here; root NUL policy remains separate. Lexically opaque
type/ignore payloads are not semantic admission. Debug-shaped field comments do
not prove debug constants, AST admission or lazy failure precedence: those remain
F2, along with byte parser/source access and BOM/coordinate anchors. G4 is open.

The coordinator reports **all 131 private Ruff tests passed** after fixes,
including **23 F1 ordinary** (22 acceptance + one baseline) and **17 existing
opt-ins**. All **364** complete positive facts/IDs/diagnostics/raw replay matched
on the unchanged normal `&str` path. Both facades passed all thirteen independent
pre-core trace hashes (298 tokens including EOF, all four modes); legacy
`Send + Sync` compiled/passed in the baseline. Forty-eight shared-core UTF-8 trace
comparisons remain supplementary. The P2 source-view metering fix and added root
regression passed, including repeated views, cancellation and chunk boundaries.

Product formatting/all-target Clippy, **58 vendor script tests**, schema-2 vendor
checker and **261-path** source check passed. Scripts: **310 discovered, 297 passed,
13 isolation-required skips**; explicit byte/visitor/codec **23 / 43 / 24 passed**,
no skips, together covering all 310 unique tests across lanes. No additional core
changes are planned; provenance anchors are stable. Current preservation passed
**708 library tests (191 ignored), 18 adapter + 8 engine**, and explicit **165
reference + 3 native** tests. With the 17 Ruff opt-ins, **all 185 explicit gates
passed**, including known-limit assertions—not 185 parity matches. The P2 view
accounting item is closed; final read-only review found no material P1/P2/P3
findings and new schema-2 guards passed. Offline consumer passed with a fresh
**105-package** lock, **one native TypeScript public-API test**, five contained
Ruff crates, ABI **0.1.5** and source-hash rechecks. Its tested snapshot is
`2cea3f996bf4a0e0cbbfd242b681331fb2208a3f0e6ac1fba2e4e81472869c6e`,
not a Git revision or final-docs attestation; its owned cache `TMPDIR` was cleaned.
Guarded preflight found zero blockers/protected outputs, normal `debug`/`tmp` only;
cleanup removed **1,360,498,688 estimated allocated bytes (~1.27 GiB)**. Target
absent, follow-up `already_clean`, **no Cargo afterward**. No owned debug/test
artifacts remain; root cache parent empty, `.ideomancer/` JS/global caches
untouched. Final source/Git/helper checks are read-only, without rebuilding.
No commits or pushes were made. The bounded lexical goal and delivery are complete,
not an outstanding source-fault or compilation concern.

### Example usage

`b"# \xff\nx=1\n"` retains opaque comment bytes lexically; a text view of the
comment fails `InvalidUtf8` without poisoning the stream. This is not a CPython
file-admission result. See the acceptance tests rather than a public engine selector.

### Related files

- [Frozen baseline](python-byte-lexer-baseline.json),
  [legacy test](../src/analysis/python_ruff/byte_lexer_tests.rs),
  [acceptance tests](../src/analysis/python_ruff/byte_lexer_acceptance.rs).
- [Schema-2 vendor provenance](../vendor/ruff/provenance.json): unchanged policy
  patch plus disjoint four-file lexer patch; eight modified upstream sources,
  57 unchanged and one separately hashed authored MIT facade. Current closure:
  87 files / 66 Rust sources / 1,972,891 bytes; source inventory 261 paths.
   Exact hashes, completed delivery and future F2 gates are in the [F1 ledger](python-analysis.md#f1-shared-byte-lexer-experiment).

The remaining source audit and F0 validation sections are historical unless marked
as future F2/F3/version requirements; their cache receipts do not describe F1.

## The Two Independent Problems

1. **Byte input.** CPython admits some malformed UTF-8 bytes in comments. At F0,
   Ruff's parser, token source, lexer and cursor required whole-source `&str`.
   F1 changes the lexical seam only; parser integration remains F2.
   Replacing, masking, stripping or lossily decoding those bytes would
   change the parser input; hashing the original would not make that acceptable.
2. **Version semantics.** The helper calls the selected interpreter's
   `ast.parse(original_bytes, type_comments=True)` without a grammar version and
   reports its actual `sys.version` token. A fixed native PY312 implementation
   cannot promise the behavior or actual version token of arbitrary future or
   differently built selected interpreters.

Removing the external-interpreter prerequisite does not, by itself, preserve
the interpreter-defined behavior. Solving byte input does not solve version
selection, codecs, failure precedence, contextual comments or hard resource bounds.

## Evidence And Its Limits

The read-only source audit inspected maintained Ruff 0.12.0 at upstream revision
`87f0feb21a1af67195bebf94d811770393f9f447` and Ideomancer's captured-source seam.
The reproducible [byte contract checker](../scripts/check_python_byte_contract.py)
uses only isolated, trusted **CPython 3.12.11 / Unicode 15.0.0** and synthetic bytes.
It never executes those sources or compares them with a working byte-aware Ruff.

Its frozen **219 named inputs** require 219 direct `ast.parse` calls per gate:

| Outcome | Inputs |
| --- | ---: |
| Accepted | 107 |
| SyntaxError, helper line zero | 20 |
| SyntaxError, lines 1 / 2 / 3 / 4 | 36 / 13 / 3 / 1 |
| UnicodeDecodeError, helper line zero | 39 |

There are **53 complete AST-with-attributes control relationships**, explicit
debug-string constants and UTF-8 coordinate anchors. Input specifications total
3,769 bytes; the longest is 40 bytes. Matrix and accepted-AST fingerprints are
pinned separately from the observed outcomes in the checker. Matching independent
ASCII controls does not equate their original-byte hashes.

Representative CPython observations (`S<n>` is SyntaxError at helper line n;
`U0` is UnicodeDecodeError at helper line zero):

| Original bytes, written as Python byte literals | Outcome |
| --- | --- |
| `b'# \xff\nx=1\n'` | Accepted |
| `b'x="# \xff"\n'` | S1 |
| `b'x=f"{call() # \xff\n}"\n'` | Accepted |
| `b'x=f"{1:# \xff}"\n'` | U0 |
| `b'x=1 # type: \xff\n'` | U0 |
| `b'# type: \xff\nx=1\n'` | S1 |
| `b'# coding: utf8\n# \xff\nx=1\n'` | S0 |
| `b'# coding: utf-8\n# \xff\nx=1\n'` | Accepted |
| `b'x=)\nx="\xff"\n'` | S1 |
| `b'x=1 # type: ignore[\xff]\ny=)\n'` | S2 |
| `b'x=1 # type: \xff\ny=)\n'` | U0 |
| `b'x=)\n# \x00\n'` | S0 |

These disprove whole-file UTF-8 validation and first-bad-byte ordering as general
compatibility algorithms. They do not prove the complete CPython action schedule.
Canonical/default UTF-8 and registry-path UTF-8 must retain distinct selection
provenance. Ignore payload decoding can be deferred past a later grammar failure;
an actual attached type-comment payload can fail earlier.

### Debug F-Strings Are A Required Design Case

CPython also accepts `b'x=f"{call() # \xff\n=}"\n'`. Its synthesized debug
constant is `"call() \n="`: comment text is absent, while the newline remains.
Leading, parenthesized and correctly quoted nested debug witnesses are included.

Ruff's `parser/expression.rs::parse_interpolated_element` currently obtains debug
gaps through `src_text(...).to_string()`, and `ruff_python_ast::DebugText` stores
`String`s. A gap can include malformed comment bytes. Checked identifier/string
token views alone therefore do not solve the byte interface.

The prototype must select an exact representation: range-backed debug spans or
fragments alongside the ordinary text representation are candidates. It must audit
`comparable.rs` and every consumer. It must neither invent empty/lossy strings nor
claim AST equality from range equality alone. Ideomancer needs helper-observable
facts, not a byte-identical CPython AST; nevertheless, storing or accessing debug
spans must be safe and explicit. Constructing a semantic debug constant according
to the reference grammar is distinct from rewriting the input before parsing.

## Options

| Option | Assessment |
| --- | --- |
| General byte-source abstraction across all Ruff utilities | Technically coherent, but unnecessarily broad for the current fact consumer; high fork/rebase cost |
| **Byte-backed lexical input plus narrow checked source access, retaining Ruff's grammar** | **Recommended prototype direction**; substantial but bounded source/lexer/parser integration |
| Independent byte lexer feeding Ruff | Duplicates indentation, f-string context, checkpoints and re-lexing; least attractive maintenance choice |
| Mask invalid bytes or construct a repaired whole-source string | Rejected: violates the selected no-rewrite/full-replacement direction |

There is no existing public token-vector injection seam that avoids the work.
`TokenSource` owns a concrete lexer and depends on lookahead, token values,
checkpoints, rewind and logical-token re-lexing. Chunking calls to the existing
string lexer loses lexical state and is not a byte-input solution.

## Proposed Byte Architecture

This remains a proposed end-to-end design, not an implemented byte parser. F1
implements only its shared lexical seam:

```text
captured immutable bytes + original digest
  -> NUL / BOM / cookie / encoding-selection policy
  -> borrowed canonical-UTF-8 bytes OR legitimately strict-transcoded buffer
  -> shared contextual byte-capable lexer + existing grammar, one parse
  -> both diagnostic channels + compatibility/failure validation
  -> validated borrowed-AST consumer, transactional facts
```

For the canonical/default UTF-8 lane, malformed comment bytes stay unchanged in
the backing buffer. Strict ASCII, registry-path UTF-8 and other codecs do not gain
that tolerance. Existing supported codecs retain legitimate strict transcoding;
that is not permission to repair malformed UTF-8.

### Source And Lexer Requirements

- Borrow immutable backing bytes; request fallible, token/range-local UTF-8 views
  only when text is required. Never reconstruct a whole-source `&str`.
- Reuse Ruff's indentation, string and interpolated-string state. In a replacement
  expression `#` can begin a comment; in outer/format text it is literal text.
- Specify cursor outcomes separately as scalar, malformed sequence and EOF;
  current character-returning peeks and globally valid `rest()` are insufficient.
  Lookahead across a comment boundary must not prematurely commit a decoding
  failure. Rewind must restore pending and deferred failure state, not only offsets.
- Ordinary comment payload may remain opaque through its logical newline. Cookies,
  actual type comments, ignore payloads and debug spans need their own operations.
- Preserve lookahead, checkpoint, rewind and parser-driven re-lex state, including
  validation/failure state. No source-byte omission or replacement sentinel may
  manufacture a successful parse.
- Preserve raw NUL's already-witnessed helper-line-zero rejection before decoding.
- Avoid repeated validation of the remaining suffix at each cursor step. Bound
  scalar decoding, cached validation, debug storage and checkpoint clones before
  allocation; retain the current input/decoded/working limits and cancellation.

### Concrete Touchpoints

Paths are relative to `vendor/ruff/crates/ruff_python_parser/src/` unless noted.

| Area | Source-audit touchpoints |
| --- | --- |
| Entry point | `lib.rs::{parse_unchecked, parse_module}`: add a separate byte entry while preserving existing string signatures/defaults |
| Cursor | `lexer/cursor.rs::Cursor`: byte position, scalar access, skipping and rewind; current `Chars` assumes global validity |
| Lexing | `lexer.rs`: construction, comments, identifiers, numeric/string text, interpolated middle/end, checkpoints and logical re-lex |
| Context | `lexer/interpolated_string.rs::InterpolatedStringContext`: reuse replacement/format/nesting state, not a parallel quote classifier |
| Token provider | `token_source.rs`: concrete lexer ownership, values, lookahead, checkpoints, rewind, re-lex and trivia retention |
| Parser source | `parser/mod.rs::{new, new_starts_at, src_text}`: distinguish arbitrary bytes from checked text |
| Spanning accesses | `parser/expression.rs`: debug gaps and version-specific scans; `parser/progress.rs` and statement error paths must not panic on raw spans |
| AST debug storage | `ruff_python_ast/src/nodes.rs::DebugText` and `comparable.rs`: explicit exact byte/range-capable handling |
| Ideomancer | `source.rs`, `python_ruff.rs::{classify, lexical_gate, ValidatedConsumer}`, `failures.rs::inspect`, `type_comments.rs`, `facts.rs` and literal-part extraction |

Ordinary strings and f-string text currently become `Box<str>` token values and
feed `string.rs::StringParser`. Source-byte validity must be established for those
specific operations; escape-decoding errors are not raw-source UTF-8 errors.

The fact consumer already computes byte-based line starts. It does not need a
generalized `SourceFile` merely for fact lines. Keep unrelated text-only trivia and
source-file APIs text-only unless the byte path actually needs them. In particular,
UTF-16/character-column APIs must not receive malformed bytes through an unchecked
conversion. Document BOM offset origin; decoded-codec ranges use the decoded
buffer, while the file digest always uses the original capture.
The current reference matrix covers BOM admission/conflicts, but does not anchor
BOM-specific columns. F2 must add explicit first-line/BOM, multiline/debug-span and
non-ASCII-column witnesses before claiming coordinate preservation for that path.

### Failure Scheduling Is A Separate Acceptance Gate

Ruff's recovery parse may inspect later input. CPython can stop earlier or defer
payload conversion. Eagerly turning the next bad byte into a fatal file outcome,
or sorting collected diagnostics by offset, is insufficient.

The prototype must expose structured input/context failures and establish their
ordering against grammar and deferred-payload actions. It must not return dummy
text, waive diagnostics or consume a recovery AST. A lexer token-stream success
alone does not close exception class/line/precedence compatibility. Current
certified failures and explicitly unresolved outcomes remain separate until proved.

## Version Strategy: Proposal, Not A Silent Contract Change

The existing wire request `Toolchain::Python { python }` selects an executable.
The helper reports its actual version, which participates in the legacy snapshot
hash. It does not restrict implementation to CPython or pin a patch. An executable
path/version token is also not build or stdlib attestation.

**Proposed destination:** explicit, immutable native compatibility profiles, each
covering grammar/AST, exact reference membership, codecs, Unicode/names/NFKC/case,
integer limits, contextual comments, exceptions, observations and resource policy.
Use an enumerated support list per engine release, expanded only by reviewed
maintenance releases. Do not infer `3.12.*` equivalence or map unknown versions to
the nearest profile. The current 3.12.11/UCD15 reference is private feasibility
evidence, not an approved complete native profile.

The aim would be complete replacement **within an explicitly approved target
contract**, not relabeling today's incomplete candidate as a limited native
capability. That distinction does not remove the need to approve a change from
open-ended interpreter-defined behavior to a finite, maintained version policy.

This proposed finite policy needs **separate product approval before promotion**:
it cannot literally preserve the open-ended behavior of every future selected
Python >=3.12. If that interpretation must remain unlimited, keep the helper and
defer full runtime-free cutover. The approved investigation is not approval to
freeze production, ship a limited native lane or invent an interpreter version.

Examples of the real version boundary include documented Python 3.13 type-parameter
`default_value` fields and Python 3.14 `TemplateStr`/`Interpolation`. The helper's
generic traversal can observe new descendants. These are source/documentation
implications, not locally tested additional interpreter profiles:
[3.13 AST](https://docs.python.org/3.13/library/ast.html#ast.TypeVar),
[3.14 AST](https://docs.python.org/3.14/library/ast.html#ast.TemplateStr).

### Migration Invariants

- Leave legacy executable selection, helper behavior, version tokens, schemas and
  snapshot formulas unchanged while investigating.
- A future native request must select deterministically without Python lookup,
  probing, downloads, helper fallback or nearest-profile substitution.
- Do not put a compatibility target into legacy `pythonVersion`, a profile name
  into an executable field, or upgrade an old save's capabilities.
- Native evidence/snapshot identity needs a distinct approved domain. Existing
  Python evidence-v1 and JS/TS-specific evidence-v2 validation are not shortcuts.
- Switching lanes requires explicit rediscovery. Preserve reading/rendering and
  revalidation semantics of old saves; matching facts do not transfer old approval.
- Public selectors, capability fields, support windows and helper retirement
  remain G5/G6 decisions. No public representation is chosen by this document.

## Finite Implementation And Decision Gates

| Gate | Required result before advancing |
| --- | --- |
| F0: this feasibility investigation | Source-driven design and reproducible reference observations; distinguish conditional feasibility from a native demonstration |
| F1: separately bounded lexer experiment | Original malformed comment bytes retained; contextual tokenization, spans, lookahead/rewind/re-lex and ordinary UTF-8 differential preservation demonstrated without a reconstructed source string |
| F2: parser/source/debug experiment | One parse with exact byte-span/debug representation; no unchecked text access or recovered-AST consumption; reference admission and selected failure-priority witnesses demonstrated |
| F3: captured-byte fact integration | Existing 364 positives plus byte witnesses compare full facts/IDs/diagnostics/raw replay; original hashes, decoded offsets, rollback, redaction, budgets and cancellation preserved |
| V: approved version contract | Explicit reference membership, patch policy, selection, maintenance and identity semantics; unknown-version behavior approved, no silent production narrowing |
| G4 replacement acceptance | Remaining codecs, grammar, contextual errors, failures and resource obligations have no unexplained differences in the approved contract |
| G5/G6 | Separately approved public integration, old-save/capability handling, platform/usefulness/resource acceptance and explicit cutover |

F1/F2 must establish a credible bounded maintenance surface before expanding into
another compatibility-family backlog. Review new source files, all modified vendor
files, exact patch reversal, notices and fingerprints together. Existing standard
library and cached dependencies appear sufficient; this is not a dependency,
toolchain-installation or expanded production-fork approval.

**Stop** if the experiment needs source repair, replacement text, ignored errors,
reparsing for a usable tree, unsafe UTF-8 conversion, or unapproved AST/API changes;
if byte validation/storage has no credible bounds; if failure scheduling is
unexplained; or if replacement depends on silently equating PY312 with the legacy
open-ended version contract. Parsing/destruction remain uninterruptible today;
a byte adapter is not a hard deadline/RSS sandbox.

## Reproduce The Reference Evidence

```sh
python3.12 -I -S -B scripts/check_python_byte_contract.py
python3.12 -I -S -B scripts/test_python_byte_contract.py
```

These are bounded, in-memory reference checks. No native byte implementation,
Rust build, target application, dependency installation or parser adoption is
performed. The existing completed visitor/metadata work and production default
are unchanged.

## Historical F0 Investigation Validation And Handoff

- Reference checker: **219/219 expected CPython outcomes**, **53 control
  relationships**, **20 debug-f-string witnesses**; not native parity results.
- Isolated byte-contract suite: **23 passed, no skips**.
- Script discovery: **286 tests, 273 passed and 13 isolation-required skips**.
  Explicit isolated byte/visitor/codec suites passed **23 / 43 / 24**, covering all
  286 unique script tests across lanes rather than counting skips as passes.
- Independent architecture review found no blocking correction. Cursor
  scalar/error/EOF behavior and BOM-coordinate proof obligations are explicitly
  assigned to the next experiment, not asserted as demonstrated here.
- Reviewed source inventory: **256 paths**; Ruff integrity remains **85 files /
  65 sources / 1,882,473 bytes**, unchanged by this investigation. Git whitespace
  checks passed. These checks do not constitute a new consumer or Rust-build pass.
- No Cargo/Rust build, engine-code change or vendored-source change was made for
  F0. The guarded helper reported `already_clean`; no cache removal was needed.
  Owned validation scratch was cleaned, with no retained test/debug artifacts.
  Earlier implementation results remain historical receipts, not new F0 tests.
- At F0 handoff, changes remained uncommitted with prior implementation work
  preserved and F1 was the proposed next step. F1 is now authorized as recorded
  above; this historical receipt is not its closeout. Version-policy approval
  remains an independent promotion blocker.
