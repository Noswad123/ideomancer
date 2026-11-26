# Pinned Registry Notices

The actual MIT, Apache-2.0 and Unicode data notices from `unicode_names2 1.3.0`
are retained unchanged in `unicode_names2-1.3.0/`. Its registry archive checksum
is recorded by the original upstream lockfile. The Unicode notice is the
upstream `data/LICENSE-UNICODE` (Copyright 1991-2023 Unicode, Inc.), required in
addition to the Rust code's MIT-or-Apache-2.0 terms. Its bundled name/alias data
are Unicode 16. The pinned `unicode_names2_generator 1.3.0` uses identical MIT
and Apache license texts. Neither registry crate's implementation is vendored here.

This directory is not a complete license bundle for the product's merged registry
graph. In particular `unicode-ident` shares the product resolution and requires
its actual selected version's Unicode notice in addition to its code license.
Recheck applicable source and binary notices against the root lockfile before
distribution. Upstream Ruff/RustPython notices are in the complete `../LICENSE`.
