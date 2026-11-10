# Software Planning

## Product Goal

The intended path is **application -> architecture -> design conversation ->
reviewable plan**. Architecture is context for choosing a change, not the end
product. The useful next interaction is with the user: select the desired behavior
contract, identify unknowns, compare alternatives, and record attributed choices.
More infrastructure polish is not a substitute for that conversation.

The minimal workflow is manual plan authoring plus local review. It is not a new
agent engine, automatic LLM plan generation, or an implementation runner. No
`plan accept`, `plan apply`, or `plan run` commands exist. Existing architecture
`accept` records acceptance of an architecture proposal, not permission to change
the application. This workflow makes no application code changes.

## Review Command

```text
ideomancer plan review plan.json [--out review.md] [--json] [--check-ready]
```

`--json` conflicts with `--out`. The input is a UTF-8 JSON file, not stdin.

| Mode | Output | Exit behavior |
| --- | --- | --- |
| Default | Markdown draft on stdout for a valid plan, including blockers and warnings | 0 even when planning choices remain open |
| `--out review.md` | Save Markdown only, instead of printing it | Same validity/readiness rules; output errors are nonzero |
| `--json` | JSON report with `valid`, `readyForPlanReview`, `errors`, `blockers`, `warnings` | 0 for a valid draft unless `--check-ready` is supplied |
| `--check-ready` | Keep the selected output mode | 1 if valid but blocked; 0 if valid and ready |
| Invalid input or context | Diagnostics, or an invalid JSON report with `--json` | Nonzero; no Markdown output file is created |

Invalid schema/types, unresolved or wrong-kind references, mismatched architecture
pins, failed saved-artifact verification, and work dependency cycles are errors,
not discussion blockers. Normal validation/I/O failures exit 1; CLI usage errors
such as combining `--json` and `--out` exit 2.

Input is limited to **1 MiB**. Generated Markdown is limited to **8 MiB**. Input and
reused output must be bounded regular non-symlink files. `--out` must not replace
the input plan. An existing output with different contents is refused; identical
output is reused without rewriting it or changing its mtime. Choose another output
name after revising a plan. The output parent must already exist: review creates
neither a parent directory nor an automatic output folder.

With `--check-ready --out`, a structurally valid blocked draft can still be saved
before the command exits 1. This makes the unresolved choices available for review.
Invalid plans never reach output-file creation.

## Saved Context

The plan pins `architecture.runDirectory`, `architecture.snapshotId`, and
`architecture.proposalSha256`. A relative `architecture.runDirectory` resolves
against the **plan file's parent directory**, not the invocation working directory.
The CLI argument and `--out` paths themselves are relative to the invocation
working directory unless absolute.

Review loads and verifies saved architecture artifacts and their recorded evidence.
The run must be `proposal_ready` or `accepted`. An unaccepted draft is allowed with
a warning; acceptance is not required to discuss a design. Both snapshot and
proposal pins must match. Component IDs come from the immutable saved proposal,
not a mutable accepted graph. Fact IDs must occur in the **actual sent packet**
recorded for that proposal, not merely somewhere in the full evidence inventory.

The command does not rescan the application, establish source freshness, read
application/test sources or manual citation targets, invoke Python, run tests,
access agent authentication, contact a provider/network, or write architecture
workflow state or application files. Its only requested persistent output is the Markdown
review. Saved evidence is static context, not proof of runtime behavior.

Manual citations are **unverified text to this command**. Their paths and notes are
rendered as text, not opened or normalized. A source-reading assistant or human
may have grounded a citation separately; that does not mean `plan review` checked
it. A `test_source` citation describes a test definition, not a passing test, and a
`constraint` citation is an attributed assertion, not automatically established truth.

## Plan Format

The schema version is exactly `ideomancer.plan.v1`. Field names use camelCase;
enum values use snake_case. Unknown fields are rejected at every object level.
Types are strict: arrays are arrays, line numbers are integers, and text is not
coerced from numbers or booleans. All listed non-nullable fields are required,
including arrays; empty arrays allow an incomplete but structurally valid draft.
Nullable fields may be omitted or set to `null`.

| Field | Type and contents |
| --- | --- |
| `schemaVersion` | String `ideomancer.plan.v1` |
| `id`, `title`, `question` | Strings identifying the plan, its title, and the central design question |
| `architecture` | Object with string `runDirectory`, `snapshotId`, `proposalSha256` pins |
| `scope` | Object with string arrays `inScope`, `outOfScope` |
| `references` | Array of manual citation objects: string `id`, `kind`, `path`, `note`; integer `line`, `endLine`. Kind is `source`, `test_source`, or `constraint`; `line >= 1`, `endLine >= line` |
| `observations` | Array with string `id`, `statement`; string arrays `factIds`, `referenceIds`. Each observation needs at least one actual sent fact ID or manual reference ID |
| `requirements` | Array with string `id`, `statement`, `status`; nullable string `confirmedBy`; string array `observationIds`. Status is `proposed` or `confirmed` |
| `questions` | Array with string `id`, `question`, `status`; nullable string `answer`; string array `referenceIds`. Status is `open` or `resolved` |
| `options` | Array of alternatives with string `id`, `title`, `description`; string arrays `pros`, `cons`, `referenceIds` |
| `recommendation` | Nullable object with string `optionId`, `rationale`. Advisory only, never a selection |
| `decisions` | Array with string `id`, `question`; nonempty string array `optionIds`; nullable strings `selectedOptionId`, `selectedBy`, `rationale` |
| `workItems` | Array with string `id`, `title`, `description`; string arrays `componentIds`, `requirementIds`, `decisionIds`, `dependsOn`, `verificationIds` |
| `verification` | Array with string `id`, `kind`, `check`, `expected`; string arrays `referenceIds`, `requirementIds`. Kind is `source_review`, `automated_test`, or `environment_check` |

Text must be nonblank and at most 16,384 characters when present. Each array has
at most 128 entries. Plan and local IDs are ASCII identifiers of at most 128
characters matching `[A-Za-z_][A-Za-z0-9_-]*`. IDs across references, observations,
requirements, questions, options, decisions, work items, and verification gates
must be globally unique within the plan.

References are typed: `observationIds` refer to observations, `optionIds` and
`selectedOptionId` to options, `verificationIds` to verification gates, and so on.
`componentIds` must identify saved proposal components. `dependsOn` refers to work
items and must be acyclic, including no self-dependency. Work items are rendered in
authored order; dependencies state prerequisites, not execution permission.

A confirmed requirement needs nonblank `confirmedBy`; a proposed requirement must
not contain it. A resolved question needs an answer. A selected option must belong
to that decision's `optionIds`, with nonblank `selectedBy` and `rationale`.
An unselected decision must not contain selection attribution or rationale.
Attribution records who made a choice; it is not an authenticated approval system.

## Readiness

**Every valid draft is reviewable immediately.** `readyForPlanReview: false` does
not prevent rendering or design discussion. The flag means the recorded planning
choices and required planning sections are complete enough for discussion of the
implementation plan. It does not certify design quality, approve implementation,
or authorize execution, even when true.

Readiness blockers are:

- Any proposed, not yet confirmed and attributed, requirement.
- Any open question.
- Any decision without a selected option. A recommendation cannot satisfy this.
- Any work item missing a requirement, decision, or verification-gate link.
- An empty observations, requirements, options, decisions, work items, or verification section.

Warnings such as unaccepted architecture and unverified manual citations are not
structural errors or automatic readiness blockers. The validator checks recorded
structure and links, not whether all relevant questions were discovered or the
selected design is correct. Verification entries are **planned checks**, never
results: nothing has been run or passed by reviewing the plan.

## Design Conversation

A generic starting question is: **When a workflow partially fails, which completed
steps remain committed, and what should the caller observe?** Do not infer the
desired contract solely from today's source or a static architecture diagram.

1. Record current observations with actual sent fact IDs and/or separately grounded manual citations. Distinguish observed source structure from inferred runtime behavior.
2. Ask the user to choose the contract: stop at the first failure, compensate earlier steps, or continue independent work with an explicit partial result. Record scope and unknowns about side effects, retries, idempotency, and error reporting.
3. Write proposed requirements and open questions. Compare alternatives with pros and cons. Keep any assistant recommendation advisory and decisions unselected until a person chooses.
4. Record confirmed requirements and attributed selections with rationale. Resolve questions with explicit answers rather than silently assuming them.
5. Break the chosen design into component-linked work items, dependencies, and planned verification gates. Render a review for discussion, then use `--check-ready` only when checking planning completeness is useful.

The next conversation should select that behavior contract and surface remaining
unknowns, not add another agent layer or continue installation/authentication
plumbing. Implementation requires a separate explicit user instruction outside
this review command.

## Private Local Plans

Keep actual application plans, citations, architecture pins, and rendered reviews
under ignored `.ideomancer/plans/`. This repository ignores `.ideomancer/`; verify
equivalent ignore rules in any other repository. Do not track private business
names, source paths, saved evidence, or reviews as public examples.

Create the parent explicitly, then manually author an actual complete JSON plan
using the format above. For example, with an existing private architecture run at
`.ideomancer/analyses/RUN`, a plan at `.ideomancer/plans/change.json` would use
`"runDirectory": "../analyses/RUN"` and the run's exact snapshot/proposal pins.
`RUN` is a placeholder, not a runnable example; no synthetic pin or invented fact
ID can replace verified saved context.

The local authoring and review sequence is:

```sh
mkdir -p .ideomancer/plans
# Manually author .ideomancer/plans/change.json with real saved pins and IDs.
ideomancer plan review .ideomancer/plans/change.json
ideomancer plan review .ideomancer/plans/change.json --out .ideomancer/plans/change-review.md
ideomancer plan review .ideomancer/plans/change.json --json --check-ready
```

These commands illustrate local use; they do not generate the input JSON. A draft
with unresolved choices can be saved and discussed successfully; the final command
exits 1 until its recorded readiness blockers are resolved. After edits, use a new
review filename rather than overwriting an earlier review with different content.
