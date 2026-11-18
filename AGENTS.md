# Ideomancer Development Rules

## Load the project skills

Before implementation, read `.agents/skills/ideomancer-development/SKILL.md`.
Before feature handoff, read `.agents/skills/ideomancer-feature-closeout/SKILL.md`.
If the harness does not list these skills, read the files directly. They are
repository-local developer guidance, not the application prompts in `skills/`.

## Boundaries

- Keep parsing, facts, proof validation and rendering generic. Consumer-specific
  interpretation, private fixtures, paths and generated evidence stay out of source.
- Never execute a target application's install/build/test/start/migration commands
  or import its modules to inspect it. Native parsing is not runtime verification.
- Preserve unrelated work. Do not stage, commit, push, rewrite history, install
  tools or change global configuration without explicit authorization.
- Set `RUSTUP_AUTO_INSTALL=0` for every Rust invocation, including version probes,
  Cargo subcommands and delegated commands. Use the existing active compiler by
  default. Before any explicit `+toolchain` selector, inspect the installed names
  with `RUSTUP_AUTO_INSTALL=0 rustup toolchain list` (read-only). Never infer an
  installed alias from a reported compiler version or the manifest MSRV. Missing
  toolchains are blockers, not permission to install, even for a version probe.
- Use the existing target directory and one active toolchain per feature by default.
  Do not create per-case build caches, parallel Cargo jobs from multiple agents,
  or global/shared target directories. Limit Cargo jobs and test threads to two.

## Validation and cleanup are one feature lifecycle

- Use focused tests while iterating. Run the broader relevant gates once for a
  completed change, not after every edit. Package builds belong to distribution or
  manifest/package-boundary changes, not every implementation tweak.
- Keep the checked-in low-disk development/test profiles. Full debug information,
  incremental compilation, extra toolchains and alternate targets require a clear
  debugging/compatibility need and an explicit cleanup plan.
- After a feature or bounded implementation slice is validated, clean this
  repository's disposable `target/` before handoff or starting the next feature.
  This routine project-cache cleanup is part of the authorized workflow unless
  the user explicitly requests retention. It is not cleanup after every test/chat turn.
- First preserve requested deliverables and inspect for user outputs. Saved studies
  and acceptance evidence belong outside `target/`; do not move or discard them
  without an authorized retention decision. Other checkouts, installed binaries,
  `.ideomancer/` data, external reports and global Cargo caches are never in scope.
- Run `python3.12 -B scripts/feature_cache.py` for preflight, then
  `python3.12 -B scripts/feature_cache.py --apply` for guarded cache-only cleanup.
  Never bypass a blocked check or kill another process to force cleanup.
- Cleanup is the last build-related step. Verify with the Python helper and Git;
  do not rebuild or run Cargo package checks afterward. Report tests, cache removal,
  retained artifacts and any blocker/explicit retention waiver in the final handoff.

See `docs/development-workflow.md` for the command matrix and cleanup limits.
