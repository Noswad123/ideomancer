# Reviewed Ruff Source Snapshot

This Git-distributed subset is derived from Ruff tag `0.12.0`, commit
`87f0feb21a1af67195bebf94d811770393f9f447`. It is not a registry publication,
submodule, nested Git repository or independent Cargo workspace.

The F2 closure contains 65 selected upstream Rust source files from five unpublished
crates, plus two original Ideomancer-authored sources, for 67 selected Rust sources
in an 89-file vendor snapshot (88 inventory records plus `provenance.json`).
Fixtures, snapshots, examples, benches, test resources and the standalone
`ruff_python_parser/src/parser/tests.rs` are omitted. Mixed production/test files
remain intact. Adapted manifests disable library tests, doctests, benchmarks and
automatic targets; the upstream `#[cfg(test)] mod tests` declaration is unchanged.
The checked-in AST `src/generated.rs` is a required source input; regeneration is
not part of a build. No local crate needs a build script or external generated input.

## Maintenance And Integrity

`upstream/` preserves the original workspace manifest, lockfile and five crate
manifests as provenance, not ancestor Cargo configuration. `LICENSE` is the complete
upstream license, including RustPython attribution. `provenance.json` records the
immutable pin, retrieval hashes, pristine source hashes and closed file inventory.
`patches/ast-only-policy.patch` retains the byte-unchanged reviewed
four-file AST-only/target-version compatibility patch in parser `options.rs`,
`expression.rs`, `statement.rs` and `pattern.rs`. Adapted manifests are reviewed
local changes, not upstream originals. The separate
`patches/byte-lexer-experiment.patch` modifies exactly `lexer.rs`,
`lexer/cursor.rs`, `lexer/indentation.rs` and `lexer/interpolated_string.rs` in
`ruff_python_parser/src`. These two four-file patch artifacts and the original
65-source pristine map remain byte-identical to the F1 checkpoint `805edb0`.

The third artifact, `patches/byte-parser-experiment.patch`, records exactly nine
upstream files relative to that post-policy checkpoint: AST `src/nodes.rs` and
`src/comparable.rs`, and parser `src/lib.rs`, `src/token_source.rs` and
`src/parser/{mod,expression,statement,pattern,progress}.rs`. The three parser
`expression.rs`, `statement.rs` and `pattern.rs` paths overlap the old policy.
Fourteen distinct upstream sources are modified; 51 remain unmodified.

Schema 3 defines three fixed forward layers: old AST-only policy, F1 lexer, then
F2 parser/AST. Verification reverses F2 first and checks all nine intermediate
hashes. On the three overlaps these must equal the frozen old-policy output hashes,
not the pristine or current-source hashes. The other six inputs are pristine.
Only then does the checker reverse the old policy and apply its unchanged exact
four-file edit model to that intermediate state. The separate F1 four-file reversal
is unchanged. Together they restore all 65 original pristine byte counts and hashes.
No fuzzy application or additional source edits are allowed.

`crates/ruff_python_parser/src/lexer/byte.rs` and `src/byte.rs` in the same crate are
original Ideomancer-authored MIT code, recorded separately in `local_sources`, not
claimed as upstream pristine and not part of any upstream patch. Both current hashes
are independently pinned. The lexer bridge deliberately changes for F2; its metadata
also preserves `previous_sha256`, the authored F1 hash
`d8e617b7bb9401e96263997b99151cfa02e3dd0af1254a384c7569e12d2fec94`.
That historical authored hash is not an upstream pristine reference. Retain
Ideomancer's root `LICENSE` notice alongside the unchanged upstream
`vendor/ruff/LICENSE` and notices.

F1's bounded byte-lexer proof remains separate from F2 parser/source/debug work.
F2 changes private vendored interfaces, including a doc-hidden parser `byte` module
and AST debug metadata; it does not change the root engine v1 public API, production
helper or default backend. Neither provenance integrity nor F1's lexer proof
establishes F2 behavior, CPython admission, complete compatibility or cutover approval.
Frontend-5/facts-7 policy identifiers are unchanged; the reviewed inventory changes
the transitive development fingerprint.

Run `python3.12 -B scripts/check_ruff_vendor.py` from the repository root for
read-only integrity, path, metadata, manifest and policy checks. The verifier
anchors the reviewed inventory, pristine hashes, all three patches, old-policy
intermediate outputs and both current authored source hashes in script constants.
Schema 3 keeps layer input/output references separate from the current file inventory
and authored-source metadata. It checks
the committed snapshot, not GitHub or archive retrieval again. This is a review
and accidental-tampering guard, not an attacker-controlled-checkout sandbox.
Any intentional update requires review of the source pin, inventories, checker
anchors, manifest assumptions, notices and policy diff together.

## Dependency Policy

The crates are edition 2024, declared MSRV 1.85, unpublished version 0.0.0,
without optional features or dev dependencies. Local edges are sibling paths.
Original upstream registry requirements are retained for dependencies shared with
the product, allowing its existing selections rather than downgrading them.
Unshared dependencies are pinned to the inventoried upstream lock versions.
The root product lockfile determines the actual merged registry closure; the
retained upstream lockfile is provenance, not a claim about that resolution.
In particular newer product proc-macro dependencies must not be forced to the
older probe versions. Different semver compatibility lines may resolve in parallel;
renaming does not guarantee coexistence of incompatible exact pins within one line.

Unused upstream optional feature names are recognized by `unexpected_cfgs`, not
advertised as nonfunctional Cargo features. No Salsa Git dependency, cache/macros
crate, optional serialization/schema feature or upstream dev dependency is enabled.

## Narrow Parser Policy

`ParseOptions::with_ast_only(true)` omits duplicate call/class-header keyword
validation before its diagnostic is emitted and admits bare f-string patterns and
mapping-pattern keys through the existing literal/string parser, matching the CPython AST contract
rather than compiled-code validation. It defaults to false in both option
constructors; default-false f-string pattern, mapping-key and duplicate-keyword
admission behavior is unchanged.
The generic pattern-start predicate covers first/later class arguments, keyword
patterns and sequence items, including grouped forms. Both pattern-start predicates
gate only `FStringStart` on `ast_only`; the shared token sets are unchanged and these
entry paths do not admit t-strings.
It is not a general Python AST-versus-compiler semantic mode. Generator and
argument-order validation remain unchanged.

An independent sequence-pattern grammar correction applies in both AST modes:
the single-element shortcut allows a closing delimiter directly, or a comma
followed by that delimiter, not an arbitrary token before the delimiter. Missing
separators such as `[1 other]`, `[first _]` and `[f'{call()}' other]` must report
syntax errors. This intentionally corrects pre-existing invalid-syntax acceptance
in default mode too; it does not broaden default-false f-string admission. Valid
single-element and comma-separated list/tuple patterns retain their behavior.

TypeVarTuple and ParamSpec defaults now use the same unsupported-version diagnostic
and `=`-through-default range as TypeVar defaults, independently of AST-only policy.
Python 3.12 rejects all three defaults through that channel; Python 3.13 allows valid
defaults. Python 3.12 type parameters without defaults remain allowed.

Consumers must select the intended Python version and require BOTH `errors()` and
`unsupported_syntax_errors()` to be empty before inspecting a tree, equivalently
`has_no_syntax_errors()`. `parse(...).is_ok()` alone does not check the separate
unsupported-version channel. No diagnostic filtering or recovery-tree admission
is authorized by this patch.

## Notices And Compatibility

`LICENSE` retains Charles Marsh's MIT terms and the RustPython Team 2020 MIT
notice, as well as upstream's other attributions. Baseline `unicode_names2 1.3.0`
notices are preserved under `notices/`; this pinned registry crate embeds Unicode
16 name/alias data. PY312 targeting does not make those tables Unicode 15.
`unicode-ident` is deliberately not exact-pinned: preserve the applicable Unicode
notice for the ACTUAL root-selected version when distributing binaries. Registry
sources remain external Cargo dependencies, not copied source in this snapshot.

Declared MSRV/license screening is not proof of lower-MSRV, platform, grammar,
Unicode, decoding or fact compatibility, or legal clearance. This snapshot alone
does not switch the production Python backend or establish replacement parity.
