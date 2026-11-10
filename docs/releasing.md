# Release Review And Package Boundary

The current engine API is local development work, not a published immutable
dependency. Passing the checks below does not authorize publication or certify
source ownership, security, framework semantics, or historical clearance.

## Review The Actual Source Package

`Cargo.toml` uses a root-anchored positive include policy. Cargo include patterns
can select Git-ignored files, and unanchored names can match nested study files;
`.gitignore` alone is not a package boundary. A regression test requires anchoring.
The exact expected Cargo file list is
reviewed in [package-files.txt](package-files.txt); the check rejects unexpected,
missing, duplicate, noncanonical, and known private/runtime paths. Cargo-generated
`.cargo_vcs_info.json` is optional, and `Cargo.toml.orig` is expected.

```sh
python3.12 -m unittest discover -s scripts -p 'test_*.py'
python3.12 scripts/check_package.py
cargo package --locked --offline --allow-dirty
```

These commands never publish. `--allow-dirty` supports local candidate verification;
it is not a substitute for reviewing/checkpointing the exact intended release.
CI repeats the inventory check and builds the packaged crate. Review additions
before updating the inventory; never regenerate it blindly to make CI pass.

The package intentionally omits local catalogs, root contract exports, unrelated
generated diagrams and unused artwork. Its inventory protects paths, not meaning:
approved paths may still contain inappropriate content. It is not a secrets/IP
scanner. The repository and its history need a separate review; packaging does not
control Git-hosted source archives, commits, prior releases or clones.

This [fresh baseline](lineage.md) excludes legacy artwork and generated root diagrams
from tracked source and packages, including the former README image. Existing local
copies remain ignored and are not deleted. Reintroducing assets requires confirmed
provenance, redistribution permission and inventory review; package policy is not
copyright clearance.

## Validate Reuse From The Package

`cargo package` verifies the normalized package from its extracted source, including
the embedded language helpers and viewer assets. Also exercise an independent
consumer against that extracted package, not just the development checkout. Record
the actual package checksum, tool versions, API/claim tests, and skipped optional
integrations. This local check does not establish an upstream revision pin.

Binary distribution bundles must retain `LICENSE` and applicable third-party
notices. The developer-local `make install` copies only the executable and is not
a complete release-bundle/notice-generation workflow.

## Resolve Historical And Approval Gates

Prior lineages retain retired experimental interpretations. This source-only
baseline does not inherit those ancestors, but does not erase old objects, retained
refs, releases, clones, forks or hosted caches. Their disposition remains a separate
owner-approved task. Do not reconnect old refs as parents or import private history
to make this baseline appear continuous. This guide does not itself authorize any
commit, tag, rewrite, remote change or push.

Before an approved release:

- Review source/asset ownership, relevant history and previously distributed copies.
- Confirm direct/transitive dependency licenses and notices and tool provenance.
- Address or explicitly accept mutable third-party CI action references and the
  deprecated `serde_yaml` dependency; deprecation alone is not a vulnerability finding.
- Run the declared Linux/macOS toolchain matrix. A local macOS run is not that matrix.
- Distinguish static/offline checks from unavailable agent/provider integration.
- Publish only after authorization; then record a real immutable consumer pin.

See the [roadmap](../ROADMAP.md) and [migration/privacy limits](adapters.md).
