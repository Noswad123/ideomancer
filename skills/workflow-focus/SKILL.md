# Focused workflow interpretation — version 1

This is the focused form of the Ideomancer workflow. Follow the shared privacy,
evidence-reference, output-schema, validation and human-acceptance rules above.
These more-specific scope instructions replace the broad overview's emphasis on
major application responsibilities. Do not perform another step or call tools.

## Task

The input's `focus` identifies one selected callable and bounded static expansion.
Explain that operation's responsibilities and handoffs, not the whole application.
This is a workflow view, not a generic application assembly overview.
Return the same `ideomancer.architecture.v1` proposal shape and source snapshot ID.
Use a focused name that identifies the anchor operation.

- Organize components around the anchor, its relevant collaborators, and visible
  external/dynamic boundaries. Do not pad the view with unrelated registrations,
  providers, DTO catalogues, infrastructure or broad business domains.
- Use call bodies, imports, annotations and bindings to describe specific handoffs.
  Give labels meaningful verbs such as validates, resolves identifiers, fetches,
  updates, delegates, or parses only where supplied facts support them.
- `focus.links` describe static resolution candidates and supporting evidence,
  not observed runtime dispatch. Locally resolving an annotated receiver does not
  prove dependency-injection activation. Retain inferred basis where appropriate.
- `focus.unresolved`, `focus.boundaries`, and truncation metadata explain where
  traversal stopped. External SDK calls, dynamic receivers, redacted annotations,
  and ambiguous names must not become fabricated local targets.
- No caller discovery or complete route binding is implied. If the anchor is a
  service method, do not invent its HTTP endpoint or upstream callers.
- Separate a source-level handoff from execution order and control flow. Imports,
  provider configuration, and registration metadata are not request execution.
  Do not claim branches, loops, atomicity, rollback, concurrency or failure policy
  that are not represented by the selected syntax.
- Describe missing bodies, depth/fact/byte limits and unresolved boundaries in
  uncertainties. A small focused view is preferable to a complete-looking fiction.

The application still owns sequencing, validation, review and acceptance. You
cannot approve a proposal, mutate source, or request unbounded expansion.
