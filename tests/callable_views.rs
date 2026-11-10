//! Offline callable-view contracts through the compiled CLI, not renderer internals.
#![cfg(unix)]

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;
use std::time::SystemTime;
use tempfile::{tempdir, TempDir};

const SECRET: &str = "PRIVATE_CALLABLE_LITERAL_MUST_NOT_LEAK";
const SOURCE: &str = r#"
from unavailable_commands import cli
from unavailable_jobs import task, queue
import httpx

class Service:
    def run(self):
        return 42

def public_helper(service: Service):
    constructed = Service()
    constructed.run()
    selected = service
    return selected.run()

def service_entry():
    service = Service()
    return public_helper(service)

async def async_entry():
    return service_entry()

class Worker:
    @classmethod
    def execute(cls):
        return service_entry()

@cli.command("PRIVATE_CALLABLE_LITERAL_MUST_NOT_LEAK")
def _command():
    return service_entry()

@task
async def _job():
    return httpx.get("PRIVATE_CALLABLE_LITERAL_MUST_NOT_LEAK")

@queue.job()
def _queued():
    return public_helper(Service())
"#;

fn python() -> Option<&'static Path> {
    static PYTHON: OnceLock<Option<PathBuf>> = OnceLock::new();
    let path = PYTHON.get_or_init(|| {
        let executable =
            std::env::var_os("IDEOMANCER_TEST_PYTHON").unwrap_or_else(|| "python3.12".into());
        let output = Command::new(executable)
            .args([
                "-I",
                "-c",
                "import sys; assert sys.version_info >= (3, 12); print(sys.executable)",
            ])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let path = PathBuf::from(String::from_utf8(output.stdout).ok()?.trim());
        path.is_absolute().then_some(path)
    });
    if path.is_none() {
        eprintln!("SKIP callable view test: install Python >=3.12 or set IDEOMANCER_TEST_PYTHON to a trusted executable");
    }
    path.as_deref()
}

fn success(output: Output) -> Value {
    assert!(
        output.status.success(),
        "exit {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("one CLI JSON report")
}

fn rejected(output: Output, term: &str) {
    assert!(!output.status.success(), "unexpected success: {output:?}");
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(text.to_lowercase().contains(term), "{text}");
    for unexpected in ["No such file", "SENTINEL EXECUTED", "failed to run"] {
        assert!(!text.contains(unexpected), "not rejected locally: {text}");
    }
}

fn artifacts(root: &Path) -> BTreeMap<PathBuf, (Vec<u8>, SystemTime)> {
    let mut result = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(parent) = pending.pop() {
        for entry in fs::read_dir(parent).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                result.insert(
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    (
                        fs::read(&path).unwrap(),
                        fs::metadata(path).unwrap().modified().unwrap(),
                    ),
                );
            }
        }
    }
    result
}

fn artifact(report: &Value, field: &str) -> String {
    let path = Path::new(report[field].as_str().unwrap());
    assert!(path.is_absolute() && path.is_file(), "{field}: {path:?}");
    assert!(path.starts_with(report["viewDirectory"].as_str().unwrap()));
    fs::read_to_string(path).unwrap()
}

fn legend(report: &Value) -> Value {
    serde_json::from_str(&artifact(report, "legend")).unwrap()
}

struct Fixture {
    _temp: TempDir,
    cwd: PathBuf,
    root: PathBuf,
    python: PathBuf,
    sentinel: PathBuf,
}

impl Fixture {
    fn new() -> Option<Self> {
        let python = python()?.to_path_buf();
        let temp = tempdir().unwrap();
        let cwd = fs::canonicalize(temp.path()).unwrap();
        let root = cwd.join("nonweb source");
        fs::create_dir(&root).unwrap();
        fs::write(
            root.join("app.py"),
            format!(
                "from pathlib import Path\nPath({}).write_text('executed')\nraise RuntimeError('never execute source')\n{SOURCE}",
                serde_json::to_string(&cwd.join("source.executed")).unwrap()
            ),
        )
        .unwrap();
        let sentinel = cwd.join("forbidden tool");
        fs::write(
            &sentinel,
            "#!/bin/sh\n: > \"$CALLABLE_TOOL_MARKER\"\nprintf 'SENTINEL EXECUTED\\n' >&2\nexit 97\n",
        )
        .unwrap();
        fs::set_permissions(&sentinel, fs::Permissions::from_mode(0o755)).unwrap();
        Some(Self {
            _temp: temp,
            cwd,
            root,
            python,
            sentinel,
        })
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_ideomancer"));
        command
            .current_dir(&self.cwd)
            .env("IDEOMANCER_PYTHON", &self.sentinel)
            .env("IDEOMANCER_AGENT", &self.sentinel)
            .env("IDEOMANCER_MMDC", &self.sentinel)
            .env("CALLABLE_TOOL_MARKER", self.cwd.join("tool.executed"))
            .env_remove("IDEOMANCER_MODEL");
        command
    }

    fn saved(&self) -> Value {
        let saved = success(
            self.command()
                .arg("discover")
                .arg(&self.root)
                .arg("--out")
                .arg(self.cwd.join("catalog output"))
                .args(["--json", "--limit", "100"])
                .env("IDEOMANCER_PYTHON", &self.python)
                .output()
                .unwrap(),
        );
        assert_eq!(saved["page"]["counts"]["api"], 0);
        self.safe(&saved);
        saved
    }

    fn entry(&self, saved: &Value, kind: &str, suffix: &str) -> Value {
        let page = success(
            self.command()
                .arg("catalog")
                .arg(saved["catalogDirectory"].as_str().unwrap())
                .args(["--json", "--kind", kind, "--limit", "100"])
                .output()
                .unwrap(),
        );
        let matches: Vec<_> = page["page"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i["kind"] == kind && i["symbol"].as_str().unwrap().ends_with(suffix))
            .collect();
        assert_eq!(matches.len(), 1, "{kind} {suffix}: {page}");
        matches[0].clone()
    }

    fn diagram(&self, saved: &Value, entry: &str, view: &str) -> Command {
        let mut command = self.command();
        command
            .arg("diagram")
            .arg(saved["catalogDirectory"].as_str().unwrap())
            .args([
                "--entry",
                entry,
                "--view",
                view,
                "--no-render",
                "--focus-hops",
                "3",
                "--focus-facts",
                "80",
            ]);
        command
    }

    fn safe(&self, saved: &Value) {
        for marker in ["source.executed", "tool.executed", ".ideomancer/analyses"] {
            assert!(!self.cwd.join(marker).exists(), "unexpected {marker}");
        }
        let catalog = Path::new(saved["catalogDirectory"].as_str().unwrap());
        let mut pending = vec![catalog.to_path_buf()];
        while let Some(parent) = pending.pop() {
            for entry in fs::read_dir(parent).unwrap() {
                let path = entry.unwrap().path();
                assert_ne!(path.file_name().unwrap(), "attempts", "created {path:?}");
                if path.is_dir() {
                    pending.push(path);
                }
            }
        }
        for (path, (bytes, _)) in artifacts(catalog) {
            assert!(
                !String::from_utf8_lossy(&bytes).contains(SECRET),
                "literal leaked in {path:?}"
            );
        }
    }
}

fn check_callable(report: &Value, selected: &Value, saved: &Value) -> Value {
    assert_eq!(report["kind"], "workflow");
    assert_eq!(report["snapshotId"], saved["snapshotId"]);
    assert!(report["svg"].is_null() && report["error"].is_null());
    let data = legend(report);
    assert_eq!(data["schemaVersion"], "ideomancer.callable-view.v1");
    assert_eq!(data["sourceModel"], "static-candidate");
    assert_eq!(data["item"], *selected);
    assert_eq!(data["contractMatch"]["status"], "not-applicable");
    assert!(data.get("contract").unwrap().is_null());
    let router = data.get("routerContext").unwrap();
    assert!(
        router.is_null() || router["status"] == "not-applicable",
        "{router}"
    );
    let nodes = data["nodes"].as_array().unwrap();
    assert!(
        !nodes.iter().any(|n| n["id"] == "O"),
        "fabricated inbound node: {data}"
    );
    assert!(!data["edges"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["from"] == "O"));
    let root = nodes.iter().find(|n| n["id"] == "H").unwrap();
    assert_eq!(
        root["label"],
        format!(
            "{}\n{} candidate",
            selected["symbol"].as_str().unwrap(),
            selected["role"].as_str().unwrap()
        )
    );
    assert_eq!(root["role"], selected["role"]);
    assert!(root["kind"].as_str().unwrap().contains("callable"));
    let refs = data["sourceRefs"].as_array().unwrap();
    let ids: BTreeSet<_> = refs.iter().map(|r| r["factId"].as_str().unwrap()).collect();
    assert_eq!(
        ids.len(),
        data["retainedFactCount"].as_u64().unwrap() as usize
    );
    assert!(ids.len() <= 80);
    assert_eq!(data["limits"]["maxFacts"], 80);
    assert!(
        data["limits"]["retainedInputBytes"].as_u64().unwrap()
            <= data["limits"]["maxInputBytes"].as_u64().unwrap()
    );
    assert_eq!(
        data["limits"]["entryMetadataReserveFacts"]
            .as_u64()
            .unwrap() as usize,
        selected["evidenceIds"].as_array().unwrap().len() - 1
    );
    for id in selected["evidenceIds"].as_array().unwrap() {
        assert!(
            ids.contains(id.as_str().unwrap()),
            "selected citation omitted: {data}"
        );
        assert!(root["sourceRefs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["factId"] == *id));
    }
    for node in nodes {
        for reference in node["sourceRefs"].as_array().into_iter().flatten() {
            assert!(ids.contains(reference["factId"].as_str().unwrap()));
        }
    }
    let mermaid = artifact(report, "mermaid");
    assert!(mermaid.starts_with("flowchart"));
    for fabricated in [
        "UNKNOWN DYNAMIC",
        "declared handler candidate",
        "source fragment",
        "contract candidate",
    ] {
        assert!(!mermaid.contains(fabricated), "{mermaid}");
    }
    let html = artifact(report, "viewer");
    for fabricated in [
        "Security (Declared",
        "Servers (Declared",
        "<h4>Request</h4>",
        "<h4>Responses</h4>",
    ] {
        assert!(
            !html.contains(fabricated),
            "fabricated HTTP contract: {html}"
        );
    }
    data
}

#[test]
fn saved_nonweb_functions_async_methods_cli_and_jobs_are_native_callable_views() {
    let Some(f) = Fixture::new() else { return };
    let saved = f.saved();
    let entries: Vec<_> = [
        (".service_entry", "module_function"),
        (".async_entry", "module_function"),
        (".Worker.execute", "class_method"),
        ("._command", "cli_command"),
        ("._job", "job"),
        ("._queued", "job"),
    ]
    .into_iter()
    .map(|(suffix, role)| {
        let selected = f.entry(&saved, "workflow", suffix);
        assert_eq!(selected["role"], role);
        selected
    })
    .collect();
    fs::remove_dir_all(&f.root).unwrap();
    for selected in entries {
        for view in ["services", "network"] {
            let report = success(
                f.diagram(&saved, selected["id"].as_str().unwrap(), view)
                    .output()
                    .unwrap(),
            );
            let data = check_callable(&report, &selected, &saved);
            if matches!(selected["role"].as_str(), Some("cli_command" | "job")) {
                assert!(
                    data["sourceRefs"].as_array().unwrap().iter().any(|r| {
                        r["kind"] == "entrypoint" && r["symbol"] == selected["symbol"]
                    }),
                    "entrypoint metadata lost: {data}"
                );
            }
        }
    }
    f.safe(&saved);
}

#[test]
fn service_view_follows_public_helper_and_typed_service_candidates() {
    let Some(f) = Fixture::new() else { return };
    let saved = f.saved();
    let selected = f.entry(&saved, "workflow", ".service_entry");
    fs::remove_dir_all(&f.root).unwrap();
    let report = success(
        f.diagram(&saved, selected["id"].as_str().unwrap(), "services")
            .output()
            .unwrap(),
    );
    let data = check_callable(&report, &selected, &saved);
    for symbol in ["app.public_helper", "app.Service.run"] {
        assert!(
            data["focus"]["visitedSymbols"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s == symbol),
            "{data}"
        );
        assert!(
            data["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|n| n["label"] == symbol),
            "{data}"
        );
    }
    assert!(data["focus"]["links"]
        .as_array()
        .unwrap()
        .iter()
        .any(|l| l["kind"] == "annotated_receiver_candidate"));
    assert!(data["focus"]["links"]
        .as_array()
        .unwrap()
        .iter()
        .any(|l| l["kind"] == "constructed_receiver_candidate"));
    let mermaid = artifact(&report, "mermaid");
    assert!(mermaid.contains("observed call site") && mermaid.contains("static target candidate"));
    assert_eq!(data["counts"]["networkGroups"], 0);
    f.safe(&saved);
}

#[test]
fn job_network_is_an_import_candidate_not_a_request_or_destination() {
    let Some(f) = Fixture::new() else { return };
    let saved = f.saved();
    let selected = f.entry(&saved, "workflow", "._job");
    fs::remove_dir_all(&f.root).unwrap();
    for view in ["services", "network"] {
        let report = success(
            f.diagram(&saved, selected["id"].as_str().unwrap(), view)
                .output()
                .unwrap(),
        );
        let data = check_callable(&report, &selected, &saved);
        assert_eq!(data["counts"]["networkGroups"], 1);
        let boundary = data["boundaries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|b| b["client"] == "httpx")
            .unwrap();
        assert_eq!(
            boundary["classification"],
            "HTTP client/import boundary candidate"
        );
        assert_eq!(boundary["operation"], "get");
        assert!(boundary["sourceRefs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["kind"] == "import"));
        for field in ["url", "httpVerb", "requestOccurrence", "runtimeSecurity"] {
            assert_eq!(boundary["resolutionTrace"][field], "UNKNOWN");
        }
        let mermaid = artifact(&report, "mermaid");
        assert!(
            mermaid.contains("HTTP client/import boundary candidate")
                && mermaid.contains("Destination UNKNOWN")
        );
    }
    f.safe(&saved);
}

#[test]
fn repeat_is_byte_and_mtime_cached_and_selection_reuses_only_current_view_paths() {
    let Some(f) = Fixture::new() else { return };
    let saved = f.saved();
    let first_entry = f.entry(&saved, "workflow", ".service_entry");
    let second_entry = f.entry(&saved, "workflow", "._job");
    fs::remove_dir_all(&f.root).unwrap();
    let catalog = Path::new(saved["catalogDirectory"].as_str().unwrap());
    for view in ["services", "network"] {
        let first = success(
            f.diagram(&saved, first_entry["id"].as_str().unwrap(), view)
                .output()
                .unwrap(),
        );
        check_callable(&first, &first_entry, &saved);
        assert_eq!(
            Path::new(first["viewDirectory"].as_str().unwrap()),
            catalog.join("views").join(view)
        );
        let before = artifacts(catalog);
        assert_eq!(
            success(
                f.diagram(&saved, first_entry["id"].as_str().unwrap(), view)
                    .output()
                    .unwrap()
            ),
            first
        );
        assert_eq!(artifacts(catalog), before, "cache rewrote bytes or mtime");
        let second = success(
            f.diagram(&saved, second_entry["id"].as_str().unwrap(), view)
                .output()
                .unwrap(),
        );
        assert_eq!(second["viewDirectory"], first["viewDirectory"]);
        check_callable(&second, &second_entry, &saved);
        assert!(!artifact(&second, "mermaid").contains("public_helper"));
        let current = artifacts(catalog);
        assert_eq!(
            success(
                f.diagram(&saved, second_entry["id"].as_str().unwrap(), view)
                    .output()
                    .unwrap()
            ),
            second
        );
        assert_eq!(artifacts(catalog), current);
    }
    let names: BTreeSet<_> = fs::read_dir(catalog.join("views"))
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(
        names,
        ["index.html", "network", "services"]
            .into_iter()
            .map(Into::into)
            .collect()
    );
    f.safe(&saved);
}

#[test]
fn conventional_contracts_are_ignored_even_malformed_multiple_or_symlinked() {
    let Some(f) = Fixture::new() else { return };
    let saved = f.saved();
    let selected = f.entry(&saved, "workflow", ".service_entry");
    for view in ["services", "network"] {
        let baseline = success(
            f.diagram(&saved, selected["id"].as_str().unwrap(), view)
                .output()
                .unwrap(),
        );
        let before = artifacts(Path::new(saved["catalogDirectory"].as_str().unwrap()));
        for name in [
            "openapi.json",
            "openapi.yaml",
            "openapi.yml",
            "swagger.json",
            "swagger.yaml",
            "swagger.yml",
        ] {
            let path = f.root.join(name);
            fs::write(&path, "not a contract: [malformed").unwrap();
            assert_eq!(
                success(
                    f.diagram(&saved, selected["id"].as_str().unwrap(), view)
                        .output()
                        .unwrap()
                ),
                baseline
            );
        }
        for name in [
            "openapi.json",
            "openapi.yaml",
            "openapi.yml",
            "swagger.json",
            "swagger.yaml",
            "swagger.yml",
        ] {
            let path = f.root.join(name);
            fs::remove_file(&path).unwrap();
            symlink(f.cwd.join("absent contract target"), &path).unwrap();
        }
        assert_eq!(
            success(
                f.diagram(&saved, selected["id"].as_str().unwrap(), view)
                    .output()
                    .unwrap()
            ),
            baseline
        );
        assert_eq!(
            artifacts(Path::new(saved["catalogDirectory"].as_str().unwrap())),
            before
        );
        for name in [
            "openapi.json",
            "openapi.yaml",
            "openapi.yml",
            "swagger.json",
            "swagger.yaml",
            "swagger.yml",
        ] {
            fs::remove_file(f.root.join(name)).unwrap();
        }
    }
    f.safe(&saved);
}

#[test]
fn incompatible_options_ids_and_ambiguous_symbols_fail_locally_without_writes() {
    let Some(f) = Fixture::new() else { return };
    fs::write(
        f.root.join("other.py"),
        "def service_entry():\n    return 0\n",
    )
    .unwrap();
    let saved = f.saved();
    let selected = f.entry(&saved, "workflow", "app.service_entry");
    let class = f.entry(&saved, "class", ".Service");
    let catalog = Path::new(saved["catalogDirectory"].as_str().unwrap());
    for view in ["services", "network"] {
        success(
            f.diagram(&saved, selected["id"].as_str().unwrap(), view)
                .output()
                .unwrap(),
        );
    }
    let before = artifacts(catalog);
    for view in ["services", "network"] {
        for spec in [f.cwd.join("nonexistent.json"), f.sentinel.clone()] {
            rejected(
                f.diagram(&saved, selected["id"].as_str().unwrap(), view)
                    .arg("--openapi")
                    .arg(spec)
                    .output()
                    .unwrap(),
                "--openapi requires one api --entry",
            );
        }
        rejected(
            f.diagram(&saved, selected["id"].as_str().unwrap(), view)
                .arg("--extract-only")
                .output()
                .unwrap(),
            "extract-only",
        );
        rejected(
            f.diagram(&saved, class["id"].as_str().unwrap(), view)
                .output()
                .unwrap(),
            "requires one api or callable workflow --entry",
        );
        for kind in ["api", "class"] {
            rejected(
                f.diagram(&saved, selected["id"].as_str().unwrap(), view)
                    .args(["--kind", kind])
                    .output()
                    .unwrap(),
                "kind",
            );
        }
        rejected(
            f.diagram(&saved, "workflow_nonexistent", view)
                .output()
                .unwrap(),
            "no catalog",
        );
        rejected(
            f.diagram(&saved, "service_entry", view).output().unwrap(),
            "ambiguous",
        );
        assert_eq!(
            artifacts(catalog),
            before,
            "invalid input changed catalog/views"
        );
    }
    f.safe(&saved);
}

#[test]
fn http_handler_workflow_selection_has_callable_identity_not_an_http_contract() {
    let Some(f) = Fixture::new() else { return };
    fs::write(
        f.root.join("handler.py"),
        "from unavailable_web import app\n@app.get('/not-a-callable-contract')\ndef handler():\n    return 42\n",
    )
    .unwrap();
    let saved = success(
        f.command()
            .arg("discover")
            .arg(&f.root)
            .arg("--out")
            .arg(f.cwd.join("catalog output"))
            .args(["--json", "--limit", "100"])
            .env("IDEOMANCER_PYTHON", &f.python)
            .output()
            .unwrap(),
    );
    let selected = f.entry(&saved, "workflow", ".handler");
    assert_eq!(selected["role"], "http_handler");
    fs::remove_dir_all(&f.root).unwrap();
    for view in ["services", "network"] {
        let report = success(
            f.diagram(&saved, selected["id"].as_str().unwrap(), view)
                .output()
                .unwrap(),
        );
        let data = check_callable(&report, &selected, &saved);
        assert!(data["sourceRefs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| { r["kind"] == "api_route" && r["symbol"] == selected["symbol"] }));
        assert!(!artifact(&report, "mermaid").contains("/not-a-callable-contract"));
        rejected(
            f.diagram(&saved, selected["id"].as_str().unwrap(), view)
                .args(["--openapi", "nonexistent.json"])
                .output()
                .unwrap(),
            "--openapi requires one api --entry",
        );
    }
    f.safe(&saved);
}

#[test]
fn selected_entrypoint_metadata_counts_against_small_fact_budget_or_rejects() {
    let Some(f) = Fixture::new() else { return };
    let saved = f.saved();
    let catalog = Path::new(saved["catalogDirectory"].as_str().unwrap());
    for suffix in ["._command", "._job", "._queued"] {
        let selected = f.entry(&saved, "workflow", suffix);
        assert!(selected["evidenceIds"].as_array().unwrap().len() >= 2);
        for view in ["services", "network"] {
            let report = success(
                f.diagram(&saved, selected["id"].as_str().unwrap(), view)
                    .output()
                    .unwrap(),
            );
            check_callable(&report, &selected, &saved);
            let before = artifacts(catalog);
            let mut command = f.command();
            command.arg("diagram").arg(catalog).args([
                "--entry",
                selected["id"].as_str().unwrap(),
                "--view",
                view,
                "--no-render",
                "--focus-hops",
                "0",
                "--focus-facts",
                "1",
            ]);
            rejected(command.output().unwrap(), "fact");
            assert_eq!(
                artifacts(catalog),
                before,
                "budget rejection replaced current view"
            );
        }
    }
    f.safe(&saved);
}
