# Source-only Python worker

This ordinary Python module has no HTTP endpoints, web framework, OpenAPI file,
third-party packages, or network client. It demonstrates local callable diagrams,
not automatic discovery of a complete business workflow.

From the Ideomancer repository root, after `cargo build --release --locked`:

```sh
./target/release/ideomancer discover examples/python-worker --kind workflow --query run_batch
./target/release/ideomancer diagram .ideomancer/catalogs/python-worker --kind workflow --entry run_batch --view services
./target/release/ideomancer diagram .ideomancer/catalogs/python-worker --kind workflow --entry run_batch --view network
```

Open `.ideomancer/catalogs/python-worker/views/index.html` to browse the current
views. `services` follows the selected callable into supported class/method targets;
unresolved string/builtin operations remain explicit evidence boundaries. The
network view should find no provenance-backed network candidates in this sample;
that result is scoped to selected static evidence, not a general absence proof.

Only discovery uses the trusted Python AST helper. It does not import or run the
module. Both explicit `--view` commands use saved evidence and never call a model,
Python interpreter, or application code. Rendering requires an already-installed
Mermaid CLI; add `--no-render` to write Mermaid, legend, and HTML without it.

**Keep `--view` for local diagrams.** A workflow entry selected without `--view`
retains the separate, explicitly documented agent-analysis behavior. `--no-render`
alone does not disable that agent path. Callable views do not inspect conventional
OpenAPI files; explicit `--openapi` requires an API entry instead.
