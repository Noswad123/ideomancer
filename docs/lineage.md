# Source-Only Fresh Baseline

The clean lineage is established in the existing checkout with a parentless root
commit containing only the reviewed source inventory. It is not a new working
directory or a merge/squash that retains old commit ancestry.

## Included Source

- The generic Rust engine and CLI, language extraction helpers, public framework
  adapters, independent synthetic tests/examples, documentation and package checks.
- The existing MIT [copyright and permission notice](../LICENSE), unchanged.
- Dependency declarations and lockfile. These do not establish comprehensive
  dependency-license, vulnerability or organizational approval.

The [exact package inventory](package-files.txt) and root-anchored Cargo includes
help enforce file boundaries. Cargo-generated manifest backups and VCS receipts
are package metadata, not baseline source files. File controls are not a secrets
detector or legal/IP clearance; future changes still require content review.

## Local Files And Prior Copies

Private catalogs, generated evidence, root contract exports, target repositories,
credentials, private consumer code and old Git metadata are not root-commit source.
Existing ignored local artifacts are preserved, not published. Unverified legacy
artwork and generated root diagrams remain local/ignored, without README references.
Reintroducing assets requires confirmed redistribution rights and inventory review.

Replacing `main` does not remove other local or remote branches, tags, reflogs,
retained objects, repository-hosted archives, releases, caches or external clones.
Do not describe it as erasing prior public content. Disposition of those copies
and any additional ref deletion requires separate owner authorization.

Consumers must verify the API at a real published immutable revision; a local
path, new root, or updated branch alone is not production/company approval.
See [release gates](releasing.md), [adapter boundaries](adapters.md), and the
[roadmap](../ROADMAP.md).
