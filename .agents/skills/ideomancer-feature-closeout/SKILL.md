---
name: ideomancer-feature-closeout
description: Finish an Ideomancer feature by recording validation, preserving deliverables and safely removing the project build cache.
---

# Ideomancer feature closeout

## When to use

Use after a bounded behavior change is validated, before final handoff or starting
another feature. Do not clean after every edit/test, during an active build, or
merely because the user asked an informational question.

## Required closeout sequence

1. Finish the relevant validation from the development skill. Run broad gates once
   when warranted; do not repeat full suites/package builds after tiny fixes when
   a focused final regression is sufficient. Record exact results and limitations.
2. Review `git diff --check`, intended changes, and requested deliverables. Do not
   stage, commit or push unless asked.
3. Resolve artifact retention **before** cleanup. Source, installed executables,
   `.ideomancer/` data, private acceptance reports, other projects and `~/.cargo`
   caches are outside the cleanup authorization. A requested package or rendered
   deliverable must be retained outside `target/` at an approved location.
4. Run the dry-run preflight:

   ```sh
   python3.12 -B scripts/feature_cache.py
   ```

   Inspect the JSON, including protected-file samples, active processes, target
   location and estimated allocated bytes. The known-filename guard is not a
   comprehensive user-data classifier: also review unexpected top-level artifacts
   and the current task's retention requirements. A fresh cache may already be clean.
5. If clear, perform the standing, project-cache-only cleanup:

   ```sh
   python3.12 -B scripts/feature_cache.py --apply
   ```

   This invokes Cargo cleanup for this repository's fixed `target/` only. It never
   calls broad `make clean`, deletes copied/installed binaries, or chooses external
   cache directories. Do not substitute `rm -rf`, force a guard, kill other builds,
   or silently delete a study. Coordinate with other agents before retrying.
6. Verify without rebuilding:

   ```sh
   python3.12 -B scripts/feature_cache.py
   git status --short --branch
   git diff --check
   ```

   Expect `already_clean` or an empty cache. Do not run Cargo tests, Clippy, metadata,
   package checks or builds after cleanup; they belong before it and may recreate
   artifacts. If another verification is necessary, finish it and clean again.
7. Final handoff states: what changed; checks actually run; cleanup status and
   allocation estimate; retained artifact locations; unresolved blockers; and
   whether changes are committed. Note that the next feature will rebuild.

## Blockers and exceptions

An active process, special/symlink path, mount, source/target mismatch, unknown
process state, external target override or retained study must stop cleanup. Report
the blocker rather than claiming closeout complete. An explicit user request to
retain the cache is a waiver: document it and revisit cleanup before the next
feature. Never invent retention permission for an unrelated directory.

The helper is POSIX Linux/macOS tooling, not an OS sandbox against a hostile actor
with the same account. Full-directory Cargo cleanup does not supply a build-lock
guarantee. Stop other writers; process checks are coordination, not mutual exclusion.
The command pins Cargo's separate build directory to the inspected local target,
including on newer Cargo versions. Disk-block figures are estimates;
APFS/COW sharing and other processes can affect actual free-space changes.

## Skill scope

This is repository-local guidance. It runs only when invoked by an agent/operator;
it is not a scheduler, background daemon, Git hook or global cleanup policy. If
the harness does not discover `.agents/skills/`, root `AGENTS.md` routes a direct
file read. The application's bundled interpretation prompts in `skills/` are
separate and unchanged.
