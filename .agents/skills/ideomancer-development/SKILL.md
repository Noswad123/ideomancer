---
name: ideomancer-development
description: Develop Ideomancer with focused validation, bounded resource use and generic source-analysis contracts.
---

# Ideomancer development

## When to use

Read this skill before implementing or debugging this project. This is project
guidance for coding agents, not an interpretation prompt for analyzed applications.

## Start with the contract

1. Inspect `git status --short --branch` and relevant existing work; preserve it.
2. Read `ROADMAP.md` and the affected contract. Use `docs/python-analysis.md` for
   Python candidates, `docs/javascript-typescript-analysis.md` for JS/TS, and
   `docs/engine-api.md` / `docs/adapter-contract.md` for the public seam.
3. Define the smallest observable acceptance case. Use independently synthetic
   fixtures. Never move private source, identifiers or context into this repository.
4. Check `RUSTUP_AUTO_INSTALL=0 rustc --version` using the existing active compiler;
   honor `Cargo.toml`'s MSRV. Use one installed toolchain and the default project
   `target/`. Set `RUSTUP_AUTO_INSTALL=0` for every Rust invocation, including probes,
   Cargo subcommands and delegated commands. Before any explicit `+toolchain`
   selector, inspect `RUSTUP_AUTO_INSTALL=0 rustup toolchain list` (read-only) and
   use only an exact installed name. Never infer an installed alias from the
   reported compiler version or MSRV. A missing toolchain is a blocker: do not
   install or update tools implicitly, even to probe a version.

## Low-disk implementation loop

- Preserve `[profile.dev]` and `[profile.test]`: line-table-only debug information,
  no incremental compilation. Backtraces retain locations; full variable/type
  debugging and incremental speed are deliberately traded for lower disk use.
- Use `--locked --offline --jobs 2`; run test binaries with `--test-threads=2`.
  When changing dependencies, explicitly review/fetch the selected pins first.
- Choose the affected test filter or integration target rather than the entire
  suite on each edit. Example:

  ```sh
  RUSTUP_AUTO_INSTALL=0 cargo test --locked --offline --jobs 2 --lib analysis::python_native -- --test-threads=2
  RUSTUP_AUTO_INSTALL=0 cargo test --locked --offline --jobs 2 --test engine_api -- --test-threads=2
   ```

- Do not run concurrent Cargo commands against the shared target directory. Delegate
  read-only reviews or disjoint edits, but coordinate builds and tests serially.
- Never add per-fixture target directories. Test data uses automatically cleaned
  temporary directories; retained human-review outputs need an explicit location
  and retention decision outside the build cache.
- Do not repeatedly rebuild release binaries or verify packages merely because a
  focused test changed. Do not switch compiler versions during normal iteration.

## Choose the right verification gate

- All Rust changes: `RUSTUP_AUTO_INSTALL=0 cargo fmt --package ideomancer --check`, focused tests,
  and root all-target Clippy with `--no-deps -- -D warnings`. Vendored upstream
  code is hash-checked rather than reformatted or subjected to product Clippy policy;
  compiler diagnostics from that preserved source remain visible. At feature
  closeout, broaden to the relevant
  integration targets or full suite once when the affected surface warrants it.
- Python extraction/candidate changes: explicitly run the oracle gate; ignored or
  unavailable oracle cases do not establish parity:

  ```sh
  RUSTUP_AUTO_INSTALL=0 IDEOMANCER_TEST_PYTHON="$(command -v python3.12)" \
    cargo test --locked --offline --jobs 2 --lib analysis::python_parity \
      -- --ignored --nocapture --test-threads=2
  RUSTUP_AUTO_INSTALL=0 IDEOMANCER_TEST_PYTHON="$(command -v python3.12)" \
    cargo test --locked --offline --jobs 2 --lib analysis::python_native \
      -- --ignored --nocapture --test-threads=2
  RUSTUP_AUTO_INSTALL=0 IDEOMANCER_TEST_PYTHON="$(command -v python3.12)" \
    cargo test --locked --offline --jobs 2 --lib analysis::python_ruff \
      -- --ignored --nocapture --test-threads=2
   ```

  The Ruff filter selects the ignored
  `analysis::python_ruff::facts::ruff_facts_batch`, acceptance, Unicode/source/
  contextual-type-comment and
  `literals::tests::method_preimages_match_exhaustive_trusted_stdlib` oracles, not selected by
  `analysis::python_parity`. All existing native/Tree-sitter reference gates remain
  required; the three existing native ignored gates remain under
  `analysis::python_native`. Shared ordinary unit tests now live under
  `analysis::python_facts`, `analysis::python_projection` and
  `analysis::python_scalars`; the old native filter does not select them.

- Script changes: `python3.12 -B -m unittest discover -s scripts -p 'test_*.py'`.
- Unicode-data changes: use installed trusted exact CPython 3.12.11 / Unicode
  15.0.0 for `python3.12 -B scripts/generate_python_unicode15.py` (offline without
  `--fetch`) and `python3.12 -B scripts/check_python_unicode15.py`. Review independent
  hard-coded input/table anchors, manifest, notices and fingerprint inputs together.
  Do not refresh/download inputs implicitly. The generator/check and CI interpreter
  pin are oracle requirements, not a narrowing of production's Python >=3.12 contract.
- New source paths: review and update `docs/source-files.txt`, then run
  `python3.12 -B scripts/check_checkout.py` before cleanup.
- Vendored Ruff changes: run `python3.12 -B scripts/check_ruff_vendor.py`; review
  immutable pin, checker anchors, provenance, patch reversal, manifests and notices
  together. Never use `cargo fmt --all`: recursive formatting touches path dependencies.
    F2 schema 3 passed: 89 vendor files / 67 Rust sources / 2,219,373 bytes
    (65 upstream + two authored), 14 modified / 51 unchanged upstream;
    264 source paths verified before docs freeze; 84 vendor tests passed after
    the final fix. The third nine-file parser/source/debug
    patch layers after the old policy patch; old two patches and pristine 65-source
    map remain unchanged. Review ordered reversals and both authored hashes, not
    three disjoint patches. Exact hashes/results are in the F2 ledger;
    completed current consumer/cleanup receipts are recorded there too.
    Historical F1 schema 2 tracked two disjoint four-file patches: unchanged AST-only policy
   (`options.rs`, `expression.rs`, `pattern.rs`, `statement.rs`) and F1 lexer
   (`lexer.rs`, `lexer/{cursor,indentation,interpolated_string}.rs`). Eight modified
   upstream files and 57 unchanged retain the original 65-source pristine map;
   original MIT `lexer/byte.rs` is separately hashed authored code, not pristine
   upstream. Review both exact reversals and the authored hash. Format root product
   code only; do not rewrite upstream through recursive formatting.
- Manifest/distribution changes: use `docs/releasing.md`. Approved personal/company
  delivery is Git source (`publish = false`), with all five local Ruff crates and
  notices in `vendor/ruff`, not registry publication or a separate fork/submodule.
  Run `RUSTUP_AUTO_INSTALL=0 python3.12 -B scripts/check_checkout_consumer.py
  --source-dir . --target-dir target --offline` before cleanup. It exports reviewed
  source, resolves a fresh consumer lockfile, checks local dependency containment,
  performs locked API validation and rechecks hashes. A working-tree snapshot is
  not an attested Git commit; eventual consumers pin a full actual approved revision.
  Legacy `check_package*` helpers and `docs/package-files.txt` remain historical
  archive tooling/tests, not active gates; `cargo package` cannot supply a usable
  registry crate with these unpublished path dependencies.
- Temporary source/test data must use verified disk-backed owned unique runs under
  `${XDG_CACHE_HOME:-$HOME/.cache}/agent-test-runs`, not `/tmp` or `/dev/shm`.
  Check free space, configure `TMPDIR` and establish cleanup before substantial work.
  The checkout consumer defaults to this cache parent and only the trusted project
  target with low-disk profiles. It stops/verifies owned process groups even after
  parent success before deleting temps; on unverified shutdown/cleanup failure it
  retains the owned run and reports the path and cleanup plan on stderr.
- Browser/SVG checks use only existing trusted tools when presentation is affected;
  never provision a browser or invoke a target application to make a test pass.

## Python frontend boundary

### Overview

The maintained Ruff 0.12.0 snapshot at
`87f0feb21a1af67195bebf94d811770393f9f447` is a private syntax frontend, not a
production fact backend. It now also has private generic fact candidate policy
`python-ruff-private-facts-7`, with syntax policy
`python-ruff-private-5-py312-visitor-patterns`; facts-1 through facts-6 remain
historical cross-family checkpoints, not the current exclusion list. CPython compatibility remains Pending. Production
`python-static-8-generic-facts`, the trusted Python >=3.12 AST helper, public API,
evidence, CLI and default are unchanged. The same five local crates, dependency
pins and Git delivery with `publish = false` remain; F1 extends the reviewed
vendor patch closure. No public migration is approved.

**Current direction: authorized private F2 byte Module parser/source/debug
experiment: all bounded implementation/delivery gates PASS.** Coordinator
reports 743 library passes / 191 ignored (including 35 F2 + 23 F1), 18 adapter +
8 engine passes, 185 explicit reference/native/Ruff gates passed (including known
limits, not all parity matches), plus formatting/Clippy, scripts and schema-3/source
checks passed. Ruff's 166 total passes are 149 ordinary + 17 opt-ins across commands,
not one filter invocation. Current independent consumer and guarded cleanup passed;
target absent, follow-up `already_clean`, no Cargo afterward. No owned scratch/debug/
test artifacts remain; `.ideomancer/` JS/global caches/installed binaries untouched.
Use the current F2 receipts, not inherited F1 consumer/cache PASS. The tested source
snapshot does not attest final receipt prose. No Cargo/build checks follow this
docs-only closeout; no further implementation is authorized. User-reported HEAD
`805edb0` is user-committed F1, initially clean; current F2 is uncommitted and
unstaged, with no commit or push. See the primary
[F2 ledger](../../../../docs/python-analysis.md#f2-byte-module-parser-source-and-debug-experiment)
and `docs/python-native-replacement-feasibility.md`.

One existing Ruff grammar/shared fallible `CoreLexer` serves closed Legacy/Bytes
providers; all effectful grammar propagates typed errors. Eight legacy text entries
remain. Experimental `parse_module_checked` allows AST callbacks only after both
diagnostic channels and provider finish; errors expose no AST. Source-backed
`DebugText::Source` owner plus leading/expression/trailing byte ranges preserve
opaque comments; legacy Text strings retain content. Source equality is owner/range
identity, not semantic equality or CPython debug-constant synthesis. Checked text
views are metered zero-copy; invalid UTF-8 is local, control failures latch.
Checkpoints restore token prefix/lexical state, not cumulative quotas; re-lex is live.
Raw BOM offsets remain, first-line columns subtract three; CRLF is one newline.

Lexer defaults: 2 MiB input / 32M work / 32 MiB conservative cumulative allocation /
200000 tokens / depth 256 / 1024 checkpoints / 10000 diagnostics. Parser defaults:
200000 work / depth 256 / 4096 diagnostics / 32 MiB cumulative charges. Depth bounds
both active grammar/reduction depth and nonrefundable construction entries per
complete top-level statement; nested suites/speculation share the quota. The
derived node-bearing layer envelope is `4 * depth + 1` (1025 default), not measured
AST height 256, a hard stack/RSS bound or deadline. Recovery pattern conversion is
precharged before folding; never expose a recovery AST callback.

Later source faults can still win over earlier syntax: explicit unresolved control,
never a fabricated exception certificate. Scope excludes codecs/type-comment payload
semantics, lazy CPython priority, F3/facts dispatch, full G4, version policy and
cutover. Production helper/Python >=3.12 and frontend-5/facts-7 names remain;
fingerprints change. Immutable profiles remain proposed, not approved; no fallback,
Python probe, fake version or old-save upgrade. Tests are finite bounded evidence,
not CPython parity.

**Historical F1 handoff receipts follow.** Counts, hashes, authorization, HEAD and
cache statements describe F1 only, superseded by current bounded F2 authorization.

**Historical direction:** contract-preserving full replacement selected; bounded
private F1 lexer goal **COMPLETE / PROVED by compiled tests; all delivery gates PASS**,
including checkout consumer and guarded cache cleanup.
Read the F1 ledger in `docs/python-analysis.md` and the
main-owned `docs/python-native-replacement-feasibility.md`. One shared fallible
`CoreLexer(&[u8], byte cursor)` serves legacy `Lexer(&str)` and doc-hidden
experimental `pub lexer::byte::ByteLexer`, not `engine::v1`. Legacy signatures,
defaults and four modes remain. No independent lexer, source repair/masking,
unchecked UTF-8, parser/AST/debug construction, source-codec/type-comment semantic,
facts dispatch or public profile change. Frontend-5/facts-7 names stay unchanged;
implementation identity changes transitively through new vendor provenance.

`source_text(&mut self, range)` is metered/cancellable, range-checked and zero-copy.
Invalid UTF-8 views do not poison lexing; resource/cancellation failures latch.
Session-owned checkpoints restore lexical/value/taken/diagnostic/stack state even
non-LIFO, not monotonic work/token/allocation/checkpoint charges or terminal control
failures. A prior owned checkpoint can clear a source fault. EOF, malformed input
and scalars (including NUL) are distinct. Only existing `lex_comment` consumes
opaque bytes; Module-only malformed-byte guarantees are lexical, not semantic
type/ignore/debug admission or Python exception certificates. Re-lex offsets must
be consumed newlines strictly before the current cursor.

Default bounds: 2 MiB input, 32M work units, 200000 tokens, depth 256, 32 MiB
cumulative conservative allocation charges, 1024 checkpoints, 10000 diagnostics.
Legacy control remains unlimited/no callback. Safe final UTF-8 validation is
precharged but linear/uninterruptible (at most 2 MiB under defaults); no hard
deadline or live-RSS claim. At F1 handoff F2 needed approval; bounded parser/source/
debug work is now authorized above. Admission/lazy precedence remains open;
version/product policy and cutover still require separate approval.

Immutable full-bundle profiles, exact patch membership and a release support window
are **proposed**, requiring product approval before finite-policy narrowing or
native wire/default changes. Production follows the selected interpreter's grammar,
stdlib, actual version token and snapshot, dynamically >=3.12 and not CPython-only;
fixed profiles cannot preserve all future/arbitrary behavior. Legacy
`Toolchain::Python { python }` keeps the helper; no native probe/fallback, fake
`pythonVersion`, nearest profile or old-save upgrade. G2/G3 remain privately closed;
G4 byte proof, version policy and other compatibility gates remain open.

**Current results (coordinator-reported): all 131 private Ruff tests passed** after
fixes, including **23 F1 ordinary** (22 acceptance + one independent baseline) and
**17 existing opt-ins**. All **364** complete positive facts/IDs/diagnostics/raw
replay matched on the unchanged normal `&str` path. Both facades passed all thirteen
frozen pre-core hashes (298 tokens including EOF/all four modes); legacy
`Send + Sync` compiled/passed in the baseline. The 48 shared-core UTF-8 differentials
remain supplementary. P2 source-view metering and the added root repeated-view/
cancellation/chunk-boundary regression passed; the 23 F1 count includes that extra
regression beyond the agent's earlier 21 acceptance tests. No additional core
changes are planned; provenance anchors remain stable.

Product formatting/all-target Clippy (`--no-deps -- -D warnings`), 58 vendor script
tests, schema-2 vendor checker (87 files / 66 Rust sources / 1,972,891 bytes;
65 upstream + one local, eight modified / 57 unchanged upstream) and source
inventory (261 paths, +5 from F0) passed. Scripts: **310 discovered, 297 passed /
13 isolation skips**; explicit byte/visitor/codec **23 / 43 / 24 passed**, no skips,
covering all 310 unique tests across lanes. Current preservation passed **708
library tests (191 ignored), 18 adapter + 8 engine** and explicit **165 reference +
3 native** tests. With the 17 Ruff opt-ins, **all 185 explicit gates passed**,
including known-limit assertions—not 185 parity matches. P2 source-view accounting
is closed; final read-only review found no material P1/P2/P3 findings and new
schema-2 guards passed. No compilation or source-fault proof concern remains for
the bounded lexer goal. Offline checkout consumer passed with fresh **105-package**
lock resolution, **one native TypeScript public-API test**, five contained Ruff
crates, ABI **0.1.5** and source-hash rechecks. Its tested snapshot is
`2cea3f996bf4a0e0cbbfd242b681331fb2208a3f0e6ac1fba2e4e81472869c6e`,
not a Git revision or final-docs attestation. Its owned cache `TMPDIR` was cleaned,
none left. Guarded preflight found zero blockers/protected outputs, normal
`debug`/`tmp` only; cleanup removed **1,360,498,688 estimated allocated bytes
(~1.27 GiB)**. Target absent, follow-up **`already_clean`**, **no Cargo afterward**.
No owned debug/test artifacts remain; root cache parent empty, `.ideomancer/` JS
artifacts/global caches untouched. Final source/Git/helper checks are read-only,
without rebuilding. F1 does not prove byte parser
input, AST/debug construction, CPython admission, F2 or full G4; it grants no
approval for further implementation or version/product-policy changes.
Existing CI ordinary root tests
select the new test modules; no new public backend/runtime Python path.
Actual pre-F1 HEAD was user-reported `fdf4e58` (facts-6 commit, initially clean/ahead
one), not `523cd7b`; F1 work was then uncommitted and unstaged, with no agent commits or
pushes. Earlier
HEAD/uncommitted/cache receipts below are historical.

**Historical F0 evidence:** the read-only reference checker has 219 named inputs (107 accepted / 112 failures)
and 53 AST-attribute control relationships, including admitted debug f-strings with
malformed comment bytes and comment-free debug constants. This is not native parity.
F0 validation passed: 219 expected outcomes/AST calls, 20 debug cases; isolated
byte **23 passed**, no skips. Discovery **286 tests: 273 passed / 13 isolation
skips**; isolated byte/visitor/codec **23 / 43 / 24 passed**, no skips, covering all
286 unique tests across lanes. Source **256 paths**, unchanged vendor
**85/65/1,882,473 bytes** and Git whitespace checks passed. Independent architecture
review found no blocking correction; at F0 handoff, F1 typed scalar/error/EOF and
lookahead/rewind were future obligations. F2 BOM-coordinate anchors remain open.
No Rust/consumer rerun was needed for scripts/docs/CI-only F0; no native-engine or
vendor change. Helper **`already_clean`**, no fresh cache removal needed; the unique
owned `TMPDIR` run was cleaned with none left. Final post-doc verification uses only
pure-Python source/helper and Git checks, no rebuild. Facts-7 validation/cleanup
below is historical, including its uncommitted-work/HEAD `523cd7b` observation.
Preserve current work, profiles, code/data and runtime policy.

### API / Interfaces

`parse_batch` remains classification-only. `extract_fact_candidates` returns bounded
generic facts/IDs/diagnostics and per-file status with a module baseline, not an AST
or source-excerpt dump. Literal retention is limited to existing bounded API
path/prefix/method metadata; tags, options, descriptions and all other literal
payloads remain redacted. `analysis/python_facts.rs` supplies
common Fact/Warning records, IDs, signature/import/warning helpers and a transactional
bounded sink. Projection/scalars are declared once in `analysis.rs`. Tree-sitter
private policy 15 uses common records/IDs/signatures but retains its CST-specific
collector, finalization and import encoder; this is not complete backend unification.
Generic `CallObservation` owns an optional preceding fact with an ASCII payload;
shared `Projection` has no framework fields. The old CST collector still supplies
`None` and retains its original admission rules.

Private optional `Failure` certificates carry an exception enum and helper line;
their formatter is exactly `<Class> at line <N>; source not analyzed`.
`CandidateFile.failure` is present only when certified; legacy `failure_line` may
instead be a physical decoder/parser location and is not itself a certificate.
`FactStatus::ParseFailure` covers certified `UnicodeDecodeError`, `SyntaxError`
covers syntax certificates but also legacy uncertified contextual errors.
Contextual type-comment `Error::Syntax` keeps its status/line and `failure: None`:
placement/line oracles do not prove every concrete CPython exception subclass.
The shared inspector and closed source-failure whitelist supply certificates;
arbitrary syntax errors and physical UTF-8 lines do not. `IndentationError`, `TabError` and `ValueError` enum
variants are vocabulary only, not implemented parity. The report's diagnostics
remain semantic warnings; there is no new public `parse_failure` emission.

### Data Flow

Captured bytes pass existing input/lexical gates, then one Ruff parse with explicit
PY312 and `with_ast_only(true)`. After that parse, shared `failures::inspect` uses
tokens/source spellings/diagnostic ranges only, never AST access, including recovery
ASTs. Both diagnostic channels must be empty **and** shared compatibility admission
must succeed before a callback consumes the validated borrowed AST. Classification
must inspect `Admission`, not only `RuffSyntax::AcceptedByRuff`. No diagnostic waiver,
reparse, source rewrite or AST escape is allowed. Hash original bytes (including
BOM/cookies); all lexer/parser/
literal/span operations use the same decoded UTF-8 view. Original limits remain
1 MiB/file and 16 MiB/run; decoded limits are 2 MiB/file and 32 MiB/run. Shared
frontend admission gates raw identifiers against static Unicode-15 XID membership
before using
Ruff's whole-name NFKC, never compiler/latest-Unicode properties. A separately
accounted contextual type-comment token/statement prepass corrects ranges before
fact lines/IDs, without parsing payloads or rewalking expressions. Bounded
normalization separates render and visit edges, preserving
descendant calls even through opaque/clipped shapes and helper-order alias effects:
decorator calls replay after the body, before return annotations/type-parameter
bounds, while decorator metadata emits early, before function signatures/class
bases. Return-annotation calls follow body imports, not source order. Function type-parameter
bound calls replay after body/return annotations; class bounds replay after the body.
Observations stay unresolved until replay, so body imports affect resolution;
constraints use tuple bounds. The observation-only header helper avoids rendering
an unused root shape. Match traversal is subject, pattern, guard, body; mapping
  patterns visit all keys before all patterns. Type aliases visit name, type parameters,
value under the same owner; reusing an import alias name does not invalidate it.
No pattern/capture/type-alias/type-parameter facts, bindings, scopes or alias-map
mutations are invented. File emission is
transactional. Source working regions are scoped per statement/expression; persistent
projection maps charge retained aliases, including overwrite-shrink adjustments.
Existing node/edge accounting, including class-pattern wrappers, remains in force.
An `include_router` preceding fact emits at replay immediately before its call,
using the replay owner; captured scopes/nodes are not retained. A token keyword
index corrects Ruff's decorator-inclusive declaration ranges, including async defs.
The raw Unicode-15, decimal-4300 and named-escape guards now live in the shared
frontend rather than fact indexing. Inspector frame stacks are capped at 256;
temporary storage is precharged under a 32 MiB compatibility cap and cancellation
uses `Checkpoint::Compatibility`. Cumulative original/decoded reservations remain
unchanged. Unproved mixed-family/actual-type-comment precedence rejects as
`UnsupportedSyntax` without a certificate and with zero validated callbacks;
arbitrary Ruff errors and unsupported versions remain uncertified rejections.

### Edge Cases

- Admitted: functions/async functions/classes/nested owners, full signatures,
  annotations/defaults/bases/class keywords and header/decorator calls; relative,
  star and future imports; Assign/AnnAssign/class attributes/chained/destructured
  writes; ordinary control flow, try, async for/with, raise/assert/augmented writes/
  deletion; all Python 3.12 generic expression families, including lambdas,
  comprehensions/generators, await/yield/walrus and f-strings; all eight match pattern
  forms with case guards/bodies, type aliases and function/async-function/class type
  parameters. AST-only top-level return/jumps are allowed when the parser accepts
  them, not executed or compile-validated.
- G3: helper-compatible decorator facts and function/async-function CLI/job/API
  route metadata/diagnostics; class decorators emit facts/calls only, no discovery.
  FastAPI host/APIRouter prefix assignments, `include_router` preludes and
  multi-target mutations are supported. Unknown/dynamic properties emit helper-null,
  `supported: false` and diagnostics as applicable, not whole-file exclusions.
- Admitted compatibility profile: raw Unicode-15 identifiers, active named text
  escapes and contextual type comments including ignores. Static name lookup uses
  canonical names, all 473 official aliases and algorithmic CJK/Hangul names;
  explicit names/aliases fold ASCII case, algorithmic names are case-sensitive.
  Named sequences are not single-code-point escapes. Raw/bytes/escaped-backslash
  text remains distinct from active escapes. Actual type comments inside f-string
  replacement fields remain explicitly unsupported pending exact error locations;
  proven ignore markers there remain admitted. A separate finite category is
  `actual type comment in parameter expression with unresolved complex prefix`:
  internal annotation/default comments with active lambda/if/else/for/async/yield
  prefixes. The constant-space cancellable token scan clears complex state when
  its containing delimiter closes and does not carry completed annotations into
  defaults; opaque f-strings do not contaminate the prefix. Closed complex
  subexpressions, trailing type-comment attachment and ignores are not excluded by
  that rule; misplaced actual type comments still fail contextual syntax checks.
  Nonzero decimal integers over 4300 digits exclude underscores
  from the count; all-zero, base-prefixed, float and imaginary forms are exempt.
  Exceeding this bound now has a proved syntax failure, not an unsupported outcome.
  The 4300 limit follows the bounded CPython 3.12.11 oracle, not a narrowing of
  production's Python >=3.12 contract.
- Source-backed `TextPart` decoding admits ordinary/u/raw/triple/adjacent text,
  escapes and newline normalization, preserving surrogates as Python code points.
  Do not use Ruff's decoded strings: they replace lone surrogates. Routes retain at
  most 2048 Python code points; prefixes at most 128, with the exact nonempty
  `(?:/[a-z0-9_{}-]+){1,8}` grammar (empty/root handling remains caller policy).
  Bytes, f-strings and non-direct constants produce dynamic metadata, not file
  exclusions. `AsciiJson.points` encodes inner JSON; existing ID outer escaping
  remains intact, with no global Fact/String change. Router host/prefix assignment
  targets are limited to 128 normalized Python characters, not UTF-8 bytes.
- HTTP methods use a fixed seven-byte buffer and ASCII plus Unicode 15's four
  preimages: dotless i, long s and the two ST ligatures. Exhaustive trusted CPython
  3.12.11/Unicode 15 checks cover all `0x110000` code points, including no empty
  uppercase mappings; method recognition does not need general uppercase tables.
- The historical frontend-3 profile decoded UTF-8 by default, BOM/eligible cookies and finite Latin-1 aliases;
  code on the first line blocks a second-line cookie, and a BOM/non-UTF-8 cookie
  conflict is syntax failure. Known other codecs remain unsupported, unknown
  registry membership unresolved, not a fabricated syntax error. Strict UTF-8
  reports the offending physical byte line (sentinel zero for BOM conflicts and
  registry-alias decode failures), not generally CPython's PEG/opening-token line.
  Four explicit known differences cover two multiline locations and two malformed
  comment-byte cases CPython admits. Original-byte hashes never use the decoded
  Latin-1 buffer. Full encoding/failure closure remains open.
- Malformed active hex, `\u`, `\U` and named escapes in ordinary strings report
  that token's opening line,
  including adjacent literals. F-string text errors are deferred to the matching
  closing token, so nested ordinary/f-string failures take precedence over outer
  invalid text. Format-spec decoding failures certify `UnicodeDecodeError` at line
  zero even in raw formats. Format-field depth is three (the fourth nested
  replacement rejects); nested f-strings reset depth. These finite checks are not
  a compiler-semantic validator or complete failure parity.
- Class patterns such as `Record()`, `FastAPI()` and `APIRouter()` are syntax, not
  constructor calls. The vendor pattern patch adds bare f-string mapping and
  pattern-start admission only with `ast_only=true`; both constructors still
  default to false. Preserve valid default forms, including upstream singleton
  f-string first-element acceptance. An independent mandatory-comma correction
  rejects malformed sequences previously accepted erroneously in **both modes**,
  including default mode. State this exception honestly; do not claim all default
  behavior is unchanged. This is grammar policy, not recovery, a diagnostic waiver
  or source rewrite. Malformed controls require zero validated callbacks.
- The vendor statement patch adds TypeVarTuple/ParamSpec default version guards
  matching TypeVar: all three produce PY312 version errors independently of
  `ast_only`; PY313 accepts defaults. Classification-only `parse_batch` remains
  Pending, not proof of full target-version parity.
- Semantic bounds: 200000 nodes, depth 256, 400000 edges, 100000 facts, 100000
  warnings, 32 MiB working and 32 MiB output, in addition to existing input/lexical
  bounds. Parsing and AST destruction are uninterruptible; cooperative accounting
  is not an RSS/stack sandbox or hard deadline. Complete Unicode/encoding/failure,
  resource and full G2–G6 closure remain open; PY312 alone proves no compatibility.
- Standing architecture constraint for unchanged text input: malformed non-ASCII UTF-8 comment bytes admitted by CPython
   cannot reach unchanged Ruff's `&str` interface without rewriting. Do not repair
  source to force admission. Bounded F2 now authorizes private parser/source/debug
  experimentation; production expansion still needs separate approval. It does not authorize
  narrowing production. Existing codec, type-comment and platform/resource gaps stay open.

### Example usage

**Current frontend-5/facts-7 finite visitor gate:** review
`docs/python-visitor-contract.json` and `docs/python-visitor-witnesses.json` with
`scripts/check_python_visitor_contract.py`, `scripts/test_python_visitor_contract.py`
and `src/analysis/python_parity/ruff_visitor_batch.rs`. The closed inventory has
111 modern classes (108 Module-domain; three other roots outside scope), 188
fields and 147 in-domain node-bearing fields: 130 obligations with case-unique
observable call inputs and 17 structural population/neutral witnesses, **not 130
independent proofs**. Ten field-order obligations establish distinguishable inputs,
not output order. Require the separate complete helper/Ruff facts/IDs/diagnostics
and raw unsorted replay oracle; sorted facts alone cannot prove order.

Use installed trusted exact CPython 3.12.11, without provisioning:

```sh
python3.12 -I -S -B scripts/check_python_visitor_contract.py
python3.12 -I -S -B scripts/test_python_visitor_contract.py
```

CI adds these isolated gates. The existing explicit Ruff gate selects the visitor
oracle as well as all prior Ruff oracles; retain the reference/native gates too.
Twenty cross-family witnesses cover markers, every argument annotation/default
group, async/class/decorator/type-bound alias replay, multi-generator comprehensions,
interleaved callee/star/keyword groups, optional children, f-strings and labelled
hidden metadata beyond rendering caps. No visitor/normalizer rewrite, framework
semantics, dependency or public API/default change is part of this batch.

After both parser fixes, **108 Ruff tests passed, including 17 opt-in tests**.
**344 prior + 20 new = 364** positives matched complete arrays/IDs/diagnostics/raw
replay; all prior failure/Unicode/codec and **787-outcome contextual** oracles
passed. Checker, all three visitor tests, both-constructor/both-mode pattern
controls and **34 vendor script tests** passed. Product formatting and all-target
Clippy **`--no-deps -- -D warnings`** passed. Scripts: **263 discovered, 257 passed,
six isolation-required skips**; isolated visitor **43** and codec **24** passed
without skips. Combined lanes cover **all 263 unique tests**, not 263
discovery-lane passes. Updated vendor integrity and the **253-path** source check
passed; data, dependencies and license terms are unchanged.

The earlier singleton-sequence control now expects pre-existing singleton-first
acceptance for false/true AST-only settings in both constructors. The predicate
patch adds AST-only class-first/later-sequence admission, not singleton-first
elements. The real missing-separator bug
was fixed and controls pass in both modes/constructors with malformed zero-callback
checks, not by weakening tests. Actual differences are narrow parser predicates/
separators and proof files, **not a visitor/normalizer rewrite**. No further code
changes are planned. Preservation passed **685 library tests / 191 ignored**, **18
adapter + 8 engine**, and **165 reference + 3 native** opt-ins. With **17 Ruff
opt-ins**, **185 explicit gates passed**, including known-limit assertions—not all
parity matches. **G2/G3 PRIVATE IMPLEMENTATION COMPLETE/CLOSED** for the current
admitted Python 3.12 Module-AST lane. Current local macOS / Rust 1.89 delivery
closeout passed: offline consumer fresh **105-package** lock, **one native TypeScript
public-API test**, five contained Ruff crates, ABI **0.1.5** and source-hash rechecks.
Its snapshot is
`0e71afaa639a38f3ca0022726dc24bb24257eee8a01845ac7f36900fe4471f60`,
not an actual Git revision or later-documentation byte attestation. Configured
unique owned cache `TMPDIR` cleaned, none left. Guarded preflight: **zero blockers /
zero protected outputs**, normal `debug`/`tmp` only. Cleanup removed
**1,374,564,352 estimated allocated bytes (~1.28 GiB)**; target absent, follow-up
`already_clean`, no Cargo afterward. No owned debug/test artifacts remain;
`.ideomancer/` JS artifacts/global caches were untouched. Source **253**, vendor
**85/65/1,882,473 bytes** and Git diff checks passed before final docs. Final
source/Git/helper checks are read-only, no rebuild. Nothing staged, committed or
pushed; HEAD is `523cd7b`. Local delivery passes are not hosted/platform or full
production approval; facts-6 ~1.28 GiB cleanup receipts remain historical.

Visitor-checkpoint source inventory: **253 paths** (+5); F0 adds three paths for
**256**, verified by the F0 source gate. Vendor: **85 files / 65 sources /
1,882,473 bytes**, same four patched files and 65-source pristine closure. Patch
SHA-256: `e79a66530f58f63371c0423f5f0469ff82f9b0193f57b6ba8672c3c33aa4e90e`;
provenance anchor: `b8246ec2c0911719ddd715f80dac504795c97f70ab828bd4375d18538c237ee0`.
All facts-5/6 and current work remains uncommitted at actual HEAD `523cd7b`.

The completed preservation checkpoint supports the bounded conclusion: “Private implementation
complete for current admitted Python 3.12 Module-AST lane: closed class/field
inventory + full-output/raw-replay coverage; not complete Python compatibility,
G4 bytes/admission/failure/resource equivalence, or production cutover.” G4 version
parity is also open; G5/G6 public integration/cutover is not approved. Do not label
the named G2 visitor inventory or G3 metadata implementation in progress; local
consumer/cleanup delivery closeout is also complete.

**Historical prior checkpoint STOP (superseded for F0 and bounded F1 only):** “STOP parity
expansion after this bounded gate.” The user explicitly selected full replacement
and authorized the recommended bounded feasibility/version investigation; read the
ADR and current direction above. F1 and bounded F2 are now authorized; production
changes still need separate approval. Contextual for/with/class suite
type-comment errors can be `IndentationError`; legacy `Error::Syntax` fields
intentionally keep `failure: None`. Never invent placeholder exception certificates.
Another PEG parser alone cannot close arbitrary type-comment locations/classes;
this is G4, not new visitor work. Never silently narrow
production or infer G4 implementation/G5/cutover approval from F0. See the visitor and F0 ledgers in
`docs/python-analysis.md` for completed local closeout and open G4–G6 decisions.

**Historical frontend-4/facts-6 contract and validation follow.** These receipts do
not establish current acceptance or cache state. That batch decodes natively without a Python
runtime. The pinned exact CPython 3.12.11 / Unicode 15 profile has 70 literal
256-code-point tables (17,920 positions); 62 ASCII-identity tables, including the
Latin-1 family table, are admitted alongside strict ASCII and existing UTF-8/
Latin-1 fast paths. Seven EBCDIC tables and cp864 are excluded. Of 23 tables with
undefined bytes, 21 are admitted; undefined positions still fail strict decoding.
Maximum defined UTF-8 expansion is three. The 445 combined names preserve all
326 exact stdlib aliases, including the unreachable `csHPRoman8` typo.

Keep historical `normal_name` before the BOM gate before pinned registry lookup;
dot fallback is alias-only. UTF-8-SIG equivalence is source-context-only after
that gate, not general codec identity. Unknown names remain unresolved; known
unsupported multibyte/stateful codecs and eight excluded tables have no exact
failure certificate. Non-text/undefined codecs, ASCII high bytes, undefined table
positions, BOM conflicts and registry-alias UTF-8 decode failures use the closed
SyntaxError-at-0 whitelist. Root raw-NUL rejection precedes decoding/cookies;
generic `source::decode` preserves NUL for data-oracle use.

Validate/count before exact once-only successful reservation and allocation;
ASCII may borrow for any admitted encoding. Test borrowing via `Cow::Borrowed`,
not encoding identity. All ranges use the same unchanged decoded UTF-8 view;
hash original bytes, never rewrite newlines or grammar. Existing original
1 MiB/file / 16 MiB/run and decoded 2 MiB/file / 32 MiB/run caps remain;
three-byte expansion can reach ResourceLimit before parsing. The UTF-8-comment/
`&str` architecture stop, contextual type-comment exclusions and complete failure/
resource gaps remain open. Public production Python >=3.12 is unchanged.

For data maintenance, inspect `codecs312/PROVENANCE.md`, structural stateless
decode/stream/incremental/getregentry audit and 123 installed stdlib source-file
hashes together. These do not attest native binaries or prove byte-identical
upstream source. Use exact trusted installed 3.12.11 / UCD 15 with `-I -S -B` for scripts
`generate_python_codecs312.py --check`, `check_python_codecs312.py` and
`test_python_codecs312.py`; generation is offline, without package installation/
downloads. CI runs the isolated generator check, independent checker and codec
suite with `-I -S -B`. These tool/oracle pins do not narrow production Python >=3.12.
Code/data/scripts/manifest/
provenance/NOTICES/LICENSE are fingerprinted. The Git source closure is 248 paths
(11 additions to 237). Preserve the full PSF/historical license and all 71 module
headers/legal mapping comments in NOTICES, alongside existing Unicode notices;
raw stdlib modules are not embedded and derived data are not MIT-relicensed.

**Historical facts-6 validation:** all **104 Ruff tests passed, including 16 opt-in tests**,
after the initial 17 focused codec passes. Codec oracles cover 15,872 table positions + 256 ASCII,
63 mixed streams, 112 AST-location comparisons, 112 complete encoded positive
fact/raw-order cases, 84 certified helper SyntaxError-at-0 negatives and 10
uncertified controls (eight helper errors, two helper admissions). All **232
preserved + 112 new = 344** complete positive fact/raw-order cases passed, plus
32 prior negatives, 64 actual-helper cases (53 certified, six ambiguous, five
parser-only), 356 module failure outcomes, 787 type-comment outcomes and Unicode/
source/HTTP gates. Product package formatting, all-target Clippy
(`--no-deps -- -D warnings`), source integrity at 248 paths and vendor integrity
at 85 files / 65 source files / 1,881,065 bytes passed.

Global script discovery: **216 tests, 211 passed, five skipped** for isolated
profile requirements. Explicit `-I -S -B` codec suite: **24 passed, none skipped**;
generator check and independent checker (`license_verified: true`) passed with
those flags, also used in CI. Combined lanes cover all 216 unique script tests,
not 216 default-lane passes.

The test `Result` alias compile issue was fixed before the 17 focused passes.
A later root-NUL assertion was corrected from Ruff `StructuredSyntax` to explicit
`InvalidSource` / `NotParsed`, a SyntaxError-at-0 certificate and zero validated
callbacks: this checks intended earlier rejection, not a weakened test. Review
found only a stale facts-5 policy assertion, now facts-6; both full 104-Ruff and
682-library suites passed after corrections with no engine/codec bug or blocker.
Broader gates passed **682 library tests / 190 ignored**, **18 adapter + 8
engine**, and **165 reference + 3 native** explicit opt-in tests. With 16 Ruff
opt-ins, **184 explicit tests** passed, including known-limit assertions—not all
parity matches. Original-hash module baselines, source guards and baseline counts
remain checked. No further code changes are planned.
Local macOS / Rust 1.89 results are not hosted validation, which remains unknown.
Product formatting passed before the current offline consumer, which passed
fresh 105-package lock resolution and one native TypeScript public-API test,
with five Ruff crates contained, ABI 0.1.5 and source hashes rechecked. The tested
content snapshot is
`74e8525de0cb80679435734c0c0ab6f374c2c6818f2008bd67a0fb5064799dc1`,
not an actual Git revision or later-documentation attestation. The 248-path source
check matched that snapshot before final documentation edits; Git whitespace
checks passed.

Its unique owned cache `TMPDIR` was cleaned with no retained scratch. Guarded
preflight found zero active blockers/protected outputs, normal `debug`/`tmp` only;
apply removed **1,372,758,016 estimated allocated bytes (~1.28 GiB)**. `target/`
was absent and follow-up reported `already_clean`; no Cargo ran afterward. The
owned cache parent was empty, no debugging/test artifacts were retained, and
`.ideomancer/` JS artifacts/global caches were untouched. Final source/Git/cache
verification is read-only, without rebuilding. All codec-family and prior facts-5
work remains uncommitted at actual HEAD `523cd7b`; the facts-5 ~1.27 GiB cleanup
is historical only, separate from the current ~1.28 GiB removal.

**Historical checkpoint examples and evidence follow.** Their results and cache
receipts do not establish frontend-4/facts-6 acceptance or current cache state.

For developer validation, use the existing `--lib analysis::python_ruff -- --ignored`
oracle command above with a trusted Python 3.12 interpreter. Keep the existing active
Rust 1.89 compiler, `RUSTUP_AUTO_INSTALL=0`, two jobs/threads and package-only formatting.
There is no new public CLI/backend selector. The historical facts-2 focused checkpoint
was 31 Ruff tests, including 77 complete-array/raw-order cases on trusted Python
3.12.11: 75 base fixtures plus two metadata/BOM cases, preserving the old 51 and
adding 26 structured cases. At that checkpoint the 18-case acceptance oracle
(previously 16) checked both new CPython/Ruff admission disagreements against the
fact guard; the then-open bare mapping-key gap separately asserted zero callbacks
and was not a matching case. Facts-4's vendor patch supersedes that gap.
The historical facts-3 checkpoint passed 117 complete facts/IDs/diagnostics/raw-trace
cases (all previous 77 plus 40 G3 cases). The corrected count fixture explicitly
passed with 603 final deduplicated facts versus 604 raw observations. Literal-decoder
unit checks and the exhaustive HTTP-method oracle passed. Product package formatting
and Clippy `-D warnings` passed after the `StringLiteralFlags` API fix; the 19
unmodified vendor compiler warnings remain outside product `--no-deps` Clippy policy.
These results describe facts-3 only, not the current batch's validation.

**Historical facts-4 evidence:** all **75 Ruff tests**, including **nine opt-in tests**,
passed. All **117 preserved + 71 new = 188** complete positive facts/IDs/diagnostics/
raw-order cases and **32 negative matching-line cases** passed (16 new multiline
named-escape cases). The **787-case contextual oracle** passed its asserted
outcomes, including finite exclusions—not 787 parity matches. Four known UTF-8
differences are tested separately. Exhaustive Unicode (1,114,112 membership points,
190,293 name probes), source and HTTP-method oracles passed.

Broader results: **660** library tests passed (**183 ignored**, not counted as
passes), **18 adapter + 8 engine**, **165 reference + 3 native** explicit tests,
**192 script tests**, exhaustive data checker, vendor/source integrity and product
formatting passed. The **177 explicit tests** (165 + 3 + 9) include known-difference
assertions; do not call them all parity matches. The first broad command timed out
at 200 seconds; processes were checked stopped and exact owned temporary data
removed. Its ten-minute rerun completed all tests before a single Clippy
`nonminimal_bool` finding in the parameter lookahead; no test failed.

**Historical facts-4 final reruns passed:** after the equivalent boolean simplification,
product formatting, all eight type-comment tests (including the 787-case oracle)
and all-target product Clippy with `--no-deps -- -D warnings` passed. The offline
exported-checkout consumer passed fresh 105-package resolution and one full
independent native TypeScript public-API test, including five-crate containment,
ABI 0.1.5 and source-hash rechecks. Its explicitly configured unique owned scratch
was cleaned. The tested snapshot is not an actual Git commit and predates final
documentation edits; see the Python ledger for its hash and guarded cache-cleanup
record. These historical facts-4 results cannot satisfy current-tree gates and
are not production cutover approval.

**Historical prior facts-5 validation:** the broad run passed **670 library tests / 186
ignored**, **18 adapter + 8 engine** tests and the corrected policy assertion.
The initial focused run had 87 passes and one stale facts-4 assertion among 88 tests;
that assertion was corrected to facts-5. Explicit opt-in gates passed
**165 reference + 3 native + 12 Ruff = 180** across initial/final runs, including known-difference
assertions, not 180 parity matches. Scripts passed **192** tests; vendor integrity
passed at **85 files / 65 source files / 1,881,065 bytes** and source integrity at
**237 paths**. Broad product formatting and all-target Clippy passed.

After the broad run, the conservative metadata fix stopped synthesizing syntax
certificates for contextual type-comment errors. Final gates passed all **eight
failure-batch tests** (including the new ordinary
`contextual_locations_do_not_fabricate_exception_certificates` regression and
64-helper/44-positive oracles), all **eight type-comment tests** (including 787
outcomes), product formatting and all-target product Clippy. The broader count
remains 670 plus one added regression passed focused, not a full 671-test run or
a single full 89-test Ruff run. Final read-only review found no material P0–P3
findings after the earlier precedence fix.

Passing oracles cover 356 module outcomes (312 exact admission/certificates, 39
ambiguous, five parser-only), 64 actual-helper failure cases (53 certified, six
ambiguous, five parser-only), and 44 new + 188 preserved = 232 complete positive
facts/IDs/diagnostics/raw-order cases. The 32 prior negatives, 787-case contextual
oracle and exhaustive Unicode/source/HTTP checks passed. Original-hash module
baselines, rollback without failed warnings and siblings in both orders passed.
Do not call ambiguous/parser-only assertions parity. The facts-4 checkpoint was
user-committed at `523cd7b`, observed clean before this batch; its 1.41 GiB cleanup
is historical. That checkpoint's offline consumer passed fresh 105-package lock
resolution and one native TypeScript public-API test, with all five Ruff crates
contained, ABI 0.1.5 and source hashes rechecked. Its unique owned cache `TMPDIR`
was automatically cleaned. Guarded helper preflight found zero active blockers
and zero protected outputs, with only normal `debug`/`tmp` entries. Cleanup removed
**1,363,083,264 estimated allocated bytes (~1.27 GiB)**; `target/` was absent and
follow-up reported `already_clean`. No Cargo ran after cleanup. No debugging/test
artifacts were retained; the owned cache parent was empty, and existing
`.ideomancer/` JS outputs and global caches were untouched.

Changes remain uncommitted at HEAD `523cd7b`. The tested consumer snapshot (hash in
the Python ledger) is not a Git revision and does not attest later documentation
bytes. Final 237-path inventory and Git whitespace checks passed; read-only helper
verification reported `already_clean`, without running Cargo or rebuilding.

That checkpoint's source inventory was **237 paths** (two new failure modules). Vendor
provenance is **85 files / 65 source files / 1,881,065 bytes**, with the four-source-file patch and reversal
anchors. The five local crates, manifests/dependency pins and Git-source delivery
are unchanged. Unicode data/notices, generator/check scripts and manifest belong
to the implementation fingerprint/source closure; never silently regenerate them
using a different Python/Unicode version.

### Related files

- `src/analysis/python_ruff/{codecs312,source}.rs` and
  `codecs312/{PROVENANCE.md,manifest.json,NOTICES.md,LICENSE.txt}`: codec boundary.
- `scripts/{generate,check,test}_python_codecs312.py`,
  `src/analysis/python_parity/ruff_codec_batch.rs`: isolated profile and encoded oracles.

See `docs/python-analysis.md` for the completion ledger and historical Tree-sitter/
Ruff probes; `src/analysis/python_ruff/{failures,facts,expressions,metadata,literals,source,type_comments,unicode15}.rs`
and `python_ruff/unicode15/PROVENANCE.md` for the candidate/data contract;
`src/analysis/python_facts.rs` and `src/analysis/python_native/{projection,scalars}.rs`
for shared primitives. Keep fixtures generic; consumer-private source/paths stay out.
`src/analysis/python_parity/ruff_failure_batch.rs` checks actual-helper failure
records, module baselines and sibling isolation; the separate ignored Ruff gate
above selects it as well as the existing compatibility oracles.

## Finish the feature

Read `.agents/skills/ideomancer-feature-closeout/SKILL.md` from the repository root.
Validation results and cache cleanup
must be recorded before a completed feature is handed off. Never claim passing
checks that were ignored, unavailable or not executed.
