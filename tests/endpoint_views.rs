//! Offline endpoint-view contracts exercised only through the compiled CLI.
#![cfg(unix)]

use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;
use std::time::SystemTime;
use tempfile::{tempdir, TempDir};

const SECRET: &str = "PRIVATE_ENDPOINT_DEFAULT_EXAMPLE_MUST_NOT_LEAK";
const SOURCE: &str = r#"
from unavailable_framework import FastAPI
import requests
import httpx
app = FastAPI()

class Service:
    def run(self):
        requests.get("PRIVATE_ENDPOINT_DEFAULT_EXAMPLE_MUST_NOT_LEAK")
        httpx.post("PRIVATE_ENDPOINT_DEFAULT_EXAMPLE_MUST_NOT_LEAK")
        unknown_client.send()
        return mysterious()

@app.post("/items")
def endpoint(service: Service):
    collaborator = service
    return collaborator.run()

@app.get("/health")
def health():
    return unknown_health()
"#;

#[test]
fn local_import_cannot_borrow_an_http_client_parameter_annotation() {
    let Some(f) = Fixture::new() else {
        return;
    };
    let source = format!(
        r#"from pathlib import Path
Path({}).write_text('executed')
raise RuntimeError('never import source')
from unavailable_framework import FastAPI
from httpx import Client
app = FastAPI()

@app.get('/control')
def control(client: Client):
    return client.get()

@app.get('/x')
def endpoint(client: Client):
    from local_stub import client
    return client.get()
"#,
        serde_json::to_string(f.marker.to_str().unwrap()).unwrap()
    );
    fs::write(f.root.join("app.py"), source).unwrap();
    let saved = f.saved();
    for (suffix, expected) in [(".control", 1), (".endpoint", 0)] {
        let entry = f.entry(&saved, suffix);
        let report = success(f.diagram(&saved, &entry, "network").output().unwrap());
        let data = legend(&report);
        assert_eq!(data["counts"]["networkGroups"], expected, "{data}");
        if expected == 0 {
            assert_eq!(data["counts"]["unclassifiedCalls"], 1);
            assert!(!artifact(&report, "mermaid").contains("HTTP client/import boundary candidate"));
        }
    }
    f.safe(&saved);
}

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
        eprintln!("SKIP endpoint view test: install Python >=3.12 or set IDEOMANCER_TEST_PYTHON to a trusted executable");
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
    assert!(
        !text.contains("No such file"),
        "attempted a missing executable: {text}"
    );
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

struct Fixture {
    _temp: TempDir,
    cwd: PathBuf,
    root: PathBuf,
    python: PathBuf,
    marker: PathBuf,
}

impl Fixture {
    fn new() -> Option<Self> {
        let python = python()?.to_path_buf();
        let temp = tempdir().unwrap();
        let cwd = fs::canonicalize(temp.path()).unwrap();
        let root = cwd.join("source app");
        fs::create_dir(&root).unwrap();
        let marker = cwd.join("SOURCE_MUST_NOT_EXECUTE");
        fs::write(
            root.join("app.py"),
            format!(
                "from pathlib import Path\nPath({}).write_text('executed')\nraise RuntimeError('never import source')\n{SOURCE}",
                serde_json::to_string(marker.to_str().unwrap()).unwrap()
            ),
        ).unwrap();
        Some(Self {
            _temp: temp,
            cwd,
            root,
            python,
            marker,
        })
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_ideomancer"));
        command
            .current_dir(&self.cwd)
            .env("IDEOMANCER_PYTHON", self.cwd.join("missing python"))
            .env("IDEOMANCER_AGENT", self.cwd.join("missing agent"))
            .env("IDEOMANCER_MMDC", self.cwd.join("missing renderer"))
            .env_remove("IDEOMANCER_MODEL");
        command
    }

    fn saved(&self) -> Value {
        success(
            self.command()
                .arg("discover")
                .arg(&self.root)
                .env("IDEOMANCER_PYTHON", &self.python)
                .arg("--out")
                .arg(self.cwd.join("catalog output"))
                .args(["--json", "--kind", "api", "--limit", "2"])
                .output()
                .unwrap(),
        )
    }

    fn entry(&self, saved: &Value, suffix: &str) -> String {
        let items: Vec<_> = saved["page"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i["symbol"].as_str().unwrap().ends_with(suffix))
            .collect();
        assert_eq!(items.len(), 1, "{saved}");
        items[0]["id"].as_str().unwrap().to_owned()
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
                "2",
                "--focus-facts",
                "80",
            ]);
        command
    }

    fn safe(&self, saved: &Value) {
        assert!(!self.marker.exists(), "application source executed");
        for (path, (bytes, _)) in artifacts(Path::new(saved["catalogDirectory"].as_str().unwrap()))
        {
            assert!(
                !String::from_utf8_lossy(&bytes).contains(SECRET),
                "secret leaked in {path:?}"
            );
        }
        assert!(
            !self.cwd.join(".ideomancer/analyses").exists(),
            "local view dispatched workflow"
        );
    }
}

fn contract() -> Value {
    json!({
        "openapi":"3.1.0", "info":{"title":"Offline inventory","version":"1"},
        "servers":[{"url":"https://{tenant}.example.invalid/api", "variables":{"tenant":{"default":SECRET}}}],
        "security":[{"bearer":[]}],
        "components":{"securitySchemes":{"bearer":{"type":"http","scheme":"bearer"}}},
        "paths":{
            "/items":{"post":{
                "operationId":"endpoint", "summary":"Create <script>alert(1)</script> item",
                "parameters":[{"name":"trace","in":"header","schema":{"type":"string","default":SECRET},"example":SECRET}],
                "requestBody":{"required":true,"content":{"application/json":{
                    "schema":{"type":"object","properties":{"name":{"type":"string","default":SECRET}}},
                    "example":{"name":SECRET}, "examples":{"private":{"value":SECRET}}
                }}},
                "responses":{"201":{"description":"Created","content":{"application/json":{
                    "schema":{"type":"object","properties":{"item_id":{"type":"integer","example":SECRET}}},
                    "example":SECRET
                }}}}
            }},
            "/health":{"get":{"operationId":"health","responses":{"204":{"description":"Healthy"}}}}
        }
    })
}

#[test]
fn openapi3_yaml_and_json_show_inert_redacted_contract_metadata() {
    let Some(f) = Fixture::new() else { return };
    let saved = f.saved();
    let entry = f.entry(&saved, ".endpoint");
    for (name, bytes) in [
        (
            "contract.json",
            serde_json::to_string_pretty(&contract()).unwrap(),
        ),
        ("contract.yaml", serde_yaml::to_string(&contract()).unwrap()),
    ] {
        let spec = f.cwd.join(name);
        fs::write(&spec, bytes).unwrap();
        let report = success(
            f.diagram(&saved, &entry, "services")
                .arg("--openapi")
                .arg(&spec)
                .output()
                .unwrap(),
        );
        assert_eq!(report["kind"], "api");
        assert!(report["svg"].is_null() && report["error"].is_null());
        let data = legend(&report);
        assert_eq!(data["contract"]["version"], "3.1.0");
        assert_eq!(data["contract"]["path"], spec.to_str().unwrap());
        assert_eq!(data["contract"]["sha256"].as_str().unwrap().len(), 64);
        assert_eq!(data["contractMatch"]["status"], "candidate");
        let op = &data["contractMatch"]["candidates"][0];
        assert_eq!(
            op["request"]["content"]["application/json"]["properties"]["name"]["type"],
            "string"
        );
        assert_eq!(
            op["responses"]["201"]["content"]["application/json"]["properties"]["item_id"]["type"],
            "integer"
        );
        assert_eq!(op["security"], json!([{"bearer":[]}]));
        assert_eq!(
            op["servers"][0]["url"],
            "https://{tenant}.example.invalid/api"
        );
        let html = artifact(&report, "viewer");
        for visible in [
            "Request",
            "Responses",
            "Declared status 201",
            "name: string",
            "item_id: integer",
            "trace",
            "bearer",
            "Not Verified",
            "Not Contacted",
            "https://{tenant}.example.invalid/api",
            "&lt;script&gt;",
        ] {
            assert!(html.contains(visible), "missing {visible}: {html}");
        }
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(
            !artifact(&report, "mermaid").contains("example.invalid"),
            "declared server became network destination"
        );
        f.safe(&saved);
    }
}

#[test]
fn annotated_service_calls_are_candidates_but_unknown_calls_are_not_network_arrows() {
    let Some(f) = Fixture::new() else { return };
    let saved = f.saved();
    let entry = f.entry(&saved, ".endpoint");
    for view in ["network", "services"] {
        let report = success(f.diagram(&saved, &entry, view).output().unwrap());
        let data = legend(&report);
        assert_eq!(data["limits"]["hops"], 2);
        assert!(data["retainedFactCount"].as_u64().unwrap() <= 80);
        assert!(data["focus"]["links"]
            .as_array()
            .unwrap()
            .iter()
            .any(|link| link["kind"] == "annotated_receiver_candidate"));
        let boundaries = data["boundaries"].as_array().unwrap();
        for client in ["requests", "httpx"] {
            let boundary = boundaries
                .iter()
                .find(|b| b["client"] == client)
                .unwrap_or_else(|| panic!("missing {client} HTTP import candidate: {data}"));
            assert!(boundary["classification"]
                .as_str()
                .unwrap()
                .contains("candidate"));
            assert!(boundary["sourceRefs"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["kind"] == "import"));
        }
        for unknown in ["unknown_client.send", "mysterious"] {
            let call = data["calls"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["label"] == unknown)
                .unwrap();
            assert_eq!(call["classification"], "UNKNOWN unresolved destination");
            assert!(call["client"].is_null());
        }
        let mermaid = artifact(&report, "mermaid");
        assert!(mermaid.contains("requests") && mermaid.contains("httpx"));
        assert!(mermaid.contains("UNKNOWN"));
        if view == "network" {
            assert!(!mermaid.contains("unknown_client") && !mermaid.contains("mysterious"));
            let outgoing: Vec<_> = data["edges"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|e| e["from"] == "A")
                .collect();
            assert_eq!(outgoing.len(), 2);
            assert!(outgoing
                .iter()
                .all(|e| e["client"] == "requests" || e["client"] == "httpx"));
        } else {
            assert!(mermaid.contains("Service.run") && mermaid.contains("static target candidate"));
            assert!(mermaid.contains("observed call site"));
        }
    }
    f.safe(&saved);
}

#[test]
fn source_only_saved_views_need_neither_python_model_nor_live_source() {
    let Some(f) = Fixture::new() else { return };
    let saved = f.saved();
    let entry = f.entry(&saved, ".endpoint");
    fs::remove_dir_all(&f.root).unwrap();
    for view in ["network", "services"] {
        let report = success(f.diagram(&saved, &entry, view).output().unwrap());
        assert_eq!(report["snapshotId"], saved["snapshotId"]);
        let data = legend(&report);
        assert!(data["contract"].is_null());
        assert_eq!(data["contractMatch"]["status"], "source-only");
        assert!(artifact(&report, "viewer").contains("Source-only view"));
        let before = artifacts(Path::new(report["viewDirectory"].as_str().unwrap()));
        let repeated = success(f.diagram(&saved, &entry, view).output().unwrap());
        assert_eq!(report, repeated);
        assert_eq!(
            artifacts(Path::new(report["viewDirectory"].as_str().unwrap())),
            before
        );
    }
    f.safe(&saved);
}

#[test]
fn conventional_root_contracts_are_detected_and_ambiguity_requires_explicit_selection() {
    let Some(f) = Fixture::new() else { return };
    let saved = f.saved();
    let entry = f.entry(&saved, ".endpoint");
    for name in [
        "openapi.json",
        "openapi.yaml",
        "swagger.json",
        "swagger.yaml",
    ] {
        let spec = f.root.join(name);
        fs::write(
            &spec,
            if name.ends_with("json") {
                contract().to_string()
            } else {
                serde_yaml::to_string(&contract()).unwrap()
            },
        )
        .unwrap();
        let report = success(f.diagram(&saved, &entry, "network").output().unwrap());
        assert_eq!(legend(&report)["contract"]["path"], spec.to_str().unwrap());
        let second = f.root.join("openapi.yml");
        fs::write(&second, serde_yaml::to_string(&contract()).unwrap()).unwrap();
        let before = artifacts(Path::new(report["viewDirectory"].as_str().unwrap()));
        rejected(
            f.diagram(&saved, &entry, "network").output().unwrap(),
            "multiple",
        );
        assert_eq!(
            artifacts(Path::new(report["viewDirectory"].as_str().unwrap())),
            before
        );
        success(
            f.diagram(&saved, &entry, "network")
                .arg("--openapi")
                .arg(&spec)
                .output()
                .unwrap(),
        );
        fs::remove_file(second).unwrap();
        fs::remove_file(spec).unwrap();
    }
    f.safe(&saved);
}

#[test]
fn external_references_and_remote_spec_urls_are_rejected_without_fetching() {
    let Some(f) = Fixture::new() else { return };
    let saved = f.saved();
    let entry = f.entry(&saved, ".endpoint");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}/remote.json", listener.local_addr().unwrap());
    let catalog = Path::new(saved["catalogDirectory"].as_str().unwrap());
    let before = artifacts(catalog);
    for reference in [format!("{url}#/schemas/Item"), "other.yaml#/Item".into()] {
        let mut spec = contract();
        spec["components"]["schemas"] = json!({"Item":{"$ref":reference}});
        let path = f.cwd.join("external.json");
        fs::write(&path, spec.to_string()).unwrap();
        rejected(
            f.diagram(&saved, &entry, "network")
                .arg("--openapi")
                .arg(path)
                .output()
                .unwrap(),
            "external",
        );
    }
    let output = f
        .diagram(&saved, &entry, "network")
        .args(["--openapi", &url])
        .output()
        .unwrap();
    assert!(!output.status.success(), "remote spec URL accepted");
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock,
        "CLI contacted remote spec/reference"
    );
    assert_eq!(artifacts(catalog), before, "rejected input wrote views");
    f.safe(&saved);
}

#[test]
fn native_options_reject_kind_mismatches_and_callable_contracts_before_agent() {
    let Some(f) = Fixture::new() else { return };
    let saved = f.saved();
    let entry = f.entry(&saved, ".endpoint");
    let catalog = Path::new(saved["catalogDirectory"].as_str().unwrap());
    let before = artifacts(catalog);
    for view in ["network", "services"] {
        rejected(
            f.diagram(&saved, &entry, view)
                .args(["--kind", "workflow"])
                .output()
                .unwrap(),
            "api",
        );
        let output = f
            .command()
            .arg("diagram")
            .arg(catalog)
            .args(["--kind", "api", "--view", view, "--no-render"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        rejected(output, "--entry");
    }
    let workflow = success(
        f.command()
            .arg("catalog")
            .arg(catalog)
            .args([
                "--json", "--kind", "workflow", "--query", "endpoint", "--limit", "1",
            ])
            .output()
            .unwrap(),
    );
    let workflow_id = workflow["page"]["items"][0]["id"].as_str().unwrap();
    let output = f
        .command()
        .arg("diagram")
        .arg(catalog)
        .args([
            "--entry",
            &entry,
            "--openapi",
            "missing.json",
            "--no-render",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    rejected(output, "--view");
    assert_eq!(artifacts(catalog), before);
    let report = success(f.diagram(&saved, workflow_id, "services").output().unwrap());
    assert_eq!(report["kind"], "workflow");
    assert_eq!(legend(&report)["contractMatch"]["status"], "not-applicable");
    f.safe(&saved);
}

#[test]
fn each_view_keeps_only_current_endpoint_and_replaces_changed_spec_stably() {
    let Some(f) = Fixture::new() else { return };
    let saved = f.saved();
    let entry = f.entry(&saved, ".endpoint");
    let health = f.entry(&saved, ".health");
    let spec = f.cwd.join("contract.json");
    fs::write(&spec, contract().to_string()).unwrap();
    for view in ["network", "services"] {
        let first = success(
            f.diagram(&saved, &entry, view)
                .arg("--openapi")
                .arg(&spec)
                .output()
                .unwrap(),
        );
        let directory = Path::new(first["viewDirectory"].as_str().unwrap());
        assert_eq!(
            directory,
            Path::new(saved["catalogDirectory"].as_str().unwrap())
                .join("views")
                .join(view)
        );
        let before = artifacts(directory);
        assert_eq!(
            success(
                f.diagram(&saved, &entry, view)
                    .arg("--openapi")
                    .arg(&spec)
                    .output()
                    .unwrap()
            ),
            first
        );
        assert_eq!(artifacts(directory), before, "repeat rewrote cached view");
        let second = success(
            f.diagram(&saved, &health, view)
                .arg("--openapi")
                .arg(&spec)
                .output()
                .unwrap(),
        );
        assert_eq!(second["viewDirectory"], first["viewDirectory"]);
        assert_eq!(legend(&second)["item"]["id"], health);
        assert_ne!(artifacts(directory), before);
        assert!(!artifact(&second, "mermaid").contains("Service.run"));
        let previous_hash = legend(&second)["contract"]["sha256"].clone();
        let mut changed = contract();
        changed["paths"]["/health"]["get"]["summary"] = json!("Changed health contract");
        fs::write(&spec, changed.to_string()).unwrap();
        let third = success(
            f.diagram(&saved, &health, view)
                .arg("--openapi")
                .arg(&spec)
                .output()
                .unwrap(),
        );
        assert_eq!(third["viewDirectory"], first["viewDirectory"]);
        assert_ne!(legend(&third)["contract"]["sha256"], previous_hash);
        assert!(artifact(&third, "viewer").contains("Changed health contract"));
        let current = artifacts(directory);
        assert_eq!(
            success(
                f.diagram(&saved, &health, view)
                    .arg("--openapi")
                    .arg(&spec)
                    .output()
                    .unwrap()
            ),
            third
        );
        assert_eq!(artifacts(directory), current);
        fs::write(&spec, contract().to_string()).unwrap();
    }
    let views = Path::new(saved["catalogDirectory"].as_str().unwrap()).join("views");
    assert_eq!(
        fs::read_dir(&views).unwrap().count(),
        3,
        "historical/staged views retained"
    );
    assert!(views.join("index.html").is_file());
    f.safe(&saved);
}
