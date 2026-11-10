//! Offline contracts for current discovery catalogs and selected source views.
#![cfg(unix)]

use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;
use std::time::SystemTime;
use tempfile::{tempdir, TempDir};

const SECRET: &str = "PRIVATE_LITERAL_MUST_NOT_LEAVE_SOURCE";

fn cli() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ideomancer"));
    for key in [
        "IDEOMANCER_AGENT",
        "IDEOMANCER_MODEL",
        "IDEOMANCER_PYTHON",
        "IDEOMANCER_MMDC",
    ] {
        command.env_remove(key);
    }
    command
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
        eprintln!(
            "SKIP discovery test: install Python >=3.12 or set IDEOMANCER_TEST_PYTHON to a trusted executable"
        );
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
    serde_json::from_slice(&output.stdout).expect("one JSON report")
}

fn rejected(output: Output, term: &str) {
    assert!(!output.status.success(), "unexpected success: {output:?}");
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(text.to_lowercase().contains(term), "{text}");
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn directory(report: &Value, field: &str) -> PathBuf {
    let path = PathBuf::from(report[field].as_str().unwrap());
    assert!(path.is_absolute(), "{field}: {path:?}");
    assert!(path.is_dir());
    path
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

fn item<'a>(report: &'a Value, kind: &str, suffix: &str) -> &'a Value {
    let matches: Vec<_> = report["page"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["kind"] == kind && i["symbol"].as_str().unwrap().ends_with(suffix))
        .collect();
    assert_eq!(matches.len(), 1, "{kind} {suffix}: {matches:?}");
    matches[0]
}

struct Fixture {
    _temp: TempDir,
    cwd: PathBuf,
    root: PathBuf,
    base: PathBuf,
    python: PathBuf,
    agent: PathBuf,
    renderer: PathBuf,
    calls: PathBuf,
    renders: PathBuf,
    marker: PathBuf,
}

impl Fixture {
    fn new() -> Option<Self> {
        let python = python()?.to_path_buf();
        let temp = tempdir().unwrap();
        let cwd = fs::canonicalize(temp.path()).unwrap();
        let root = cwd.join("source app");
        fs::create_dir(&root).unwrap();
        let marker = cwd.join("SOURCE_MUST_NEVER_EXECUTE");
        let source = format!(
            "from pathlib import Path\nPath({}).write_text('executed')\nraise RuntimeError({SECRET:?})\n{}",
            serde_json::to_string(marker.to_str().unwrap()).unwrap(),
            SOURCE
        );
        fs::write(root.join("app.py"), source).unwrap();
        fs::write(root.join(".env"), format!("TOKEN={SECRET}\n")).unwrap();
        fs::write(
            root.join("private-config.yaml"),
            format!("token: {SECRET}\n"),
        )
        .unwrap();
        let mut unrelated = String::new();
        for n in 0..40 {
            unrelated.push_str(&format!("def unrelated_{n}():\n    return {n}\n"));
        }
        fs::write(root.join("unrelated.py"), unrelated).unwrap();
        let agent = cwd.join("stub agent ; literal");
        let script = cwd.join("agent.py");
        fs::write(&script, AGENT).unwrap();
        let quote = |p: &Path| format!("'{}'", p.to_str().unwrap().replace('\'', "'\\''"));
        fs::write(
            &agent,
            format!(
                "#!/bin/sh\nexec {} -I -S {} \"$@\"\n",
                quote(&python),
                quote(&script)
            ),
        )
        .unwrap();
        let renderer = cwd.join("stub mmdc ; literal");
        fs::write(&renderer, RENDERER).unwrap();
        for program in [&agent, &renderer] {
            fs::set_permissions(program, fs::Permissions::from_mode(0o755)).unwrap();
        }
        Some(Self {
            root,
            base: cwd.join("catalog output"),
            python,
            agent,
            renderer,
            calls: cwd.join("agent calls.jsonl"),
            renders: cwd.join("render calls.txt"),
            marker,
            cwd,
            _temp: temp,
        })
    }

    fn command(&self) -> Command {
        let mut command = cli();
        command
            .current_dir(&self.cwd)
            .env("IDEOMANCER_PYTHON", &self.python)
            .env("IDEOMANCER_AGENT", &self.agent)
            .env("IDEOMANCER_MODEL", "offline-discovery-model")
            .env("IDEOMANCER_MMDC", &self.renderer)
            .env("DISCOVERY_CALLS", &self.calls)
            .env("DISCOVERY_RENDERS", &self.renders);
        command
    }

    fn discover(&self) -> Command {
        let mut command = self.command();
        command
            .arg("discover")
            .arg(&self.root)
            .arg("--out")
            .arg(&self.base);
        command
    }

    fn catalog(&self, catalog: &Path) -> Command {
        let mut command = self.command();
        command.arg("catalog").arg(catalog).arg("--json");
        command
    }

    fn diagram(&self, catalog: &Path) -> Command {
        let mut command = self.command();
        command.arg("diagram").arg(catalog);
        command
    }

    fn saved(&self) -> Value {
        success(
            self.discover()
                .args(["--json", "--limit", "100"])
                .output()
                .unwrap(),
        )
    }

    fn calls(&self) -> Vec<Value> {
        fs::read_to_string(&self.calls)
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    fn safe(&self) {
        assert!(!self.marker.exists(), "application code was executed");
    }
}

const SOURCE: &str = r#"
from unavailable_framework import FastAPI, APIRouter
app = FastAPI()
router = APIRouter(prefix="PRIVATE_LITERAL_MUST_NOT_LEAVE_SOURCE")
DYNAMIC_PATH = "PRIVATE_LITERAL_MUST_NOT_LEAVE_SOURCE"
class Base:
    pass
class Service(Base):
    def run(self):
        return helper()
    def _private(self):
        return "PRIVATE_LITERAL_MUST_NOT_LEAVE_SOURCE"
def helper():
    return 42
@router.get("/items/{item_id}", description="PRIVATE_LITERAL_MUST_NOT_LEAVE_SOURCE")
@router.post("/items/{item_id}")
def endpoint(item_id):
    payload = "PRIVATE_LITERAL_MUST_NOT_LEAVE_SOURCE"
    return Service().run()
@router.get(DYNAMIC_PATH)
def _dynamic():
    return helper()
def factory():
    @app.delete("/nested")
    def _nested():
        return helper()
    def nested_helper():
        return "PRIVATE_LITERAL_MUST_NOT_LEAVE_SOURCE"
    return _nested
@cli.command("PRIVATE_LITERAL_MUST_NOT_LEAVE_SOURCE")
def _cli():
    return helper()
@queue.task(name="PRIVATE_LITERAL_MUST_NOT_LEAVE_SOURCE")
def _job():
    return helper()
def _private():
    return "PRIVATE_LITERAL_MUST_NOT_LEAVE_SOURCE"
"#;

const AGENT: &str = r#"import json
import os
from pathlib import Path
import sys

assert sys.argv[1:3] == ['execute', '--request'] and sys.argv[4:] == ['--json']
request = json.loads(Path(sys.argv[3]).read_text())
with open(os.environ['DISCOVERY_CALLS'], 'a') as log:
    log.write(json.dumps(request) + '\n')
packet = request['input']
assert 'PRIVATE_LITERAL_MUST_NOT_LEAVE_SOURCE' not in json.dumps(request)
focus = packet['focus']
ref = focus['anchor']['id']
assert ref in {f['id'] for f in packet['facts']}
count = len(packet['facts'])
proposal = {
    'schemaVersion': 'ideomancer.architecture.v1',
    'snapshotId': packet['snapshotId'],
    'name': 'Selected facts: ' + str(count),
    'components': [
        {'id': 'entry', 'label': 'Selected entry', 'kind': 'component',
         'evidenceIds': [ref], 'rationale': 'Selected callable source syntax.'},
        {'id': 'dependency', 'label': 'Candidate dependency', 'kind': 'component',
         'evidenceIds': [ref], 'rationale': 'Synthetic local grouping.'},
    ],
    'relationships': [
        {'from': 'entry', 'to': 'dependency', 'label': 'may delegate',
         'basis': 'inferred', 'evidenceIds': [ref], 'rationale': 'Not runtime proof.'},
    ],
    'uncertainties': ['Static focused candidates only.'],
}
print(json.dumps({'protocolVersion': 'agent-execution.v1', 'requestId': request['requestId'],
                  'status': 'completed', 'output': proposal, 'model': 'offline-discovery-model'}))
"#;

const RENDERER: &str = r#"#!/bin/sh
set -eu
printf 'render\n' >> "$DISCOVERY_RENDERS"
output=
while [ "$#" -gt 0 ]; do
    if [ "$1" = '-o' ]; then
        shift
        output=$1
    fi
    shift
done
test -n "$output"
printf '<svg width="300" height="150"/>' > "$output"
"#;

#[test]
fn discovery_counts_pages_filters_and_candidate_fields() {
    let Some(f) = Fixture::new() else { return };
    let source = artifacts(&f.root);
    let first = success(
        f.discover()
            .args(["--json", "--limit", "2"])
            .output()
            .unwrap(),
    );
    let catalog = directory(&first, "catalogDirectory");
    let page = &first["page"];
    assert_eq!(page["snapshotId"], first["snapshotId"]);
    assert_eq!(page["complete"], first["complete"]);
    assert_eq!(page["complete"], true);
    assert_eq!(page["offset"], 0);
    assert_eq!(page["limit"], 2);
    assert_eq!(page["nextOffset"], 2);
    assert_eq!(page["items"].as_array().unwrap().len(), 2);
    assert_eq!(page["counts"]["api"], 4);
    assert_eq!(page["counts"]["class"], 2);
    assert_eq!(page["counts"]["workflow"], 48);
    assert_eq!(page["totalItems"], 54);
    assert_eq!(page["matchedItems"], 54);
    assert!(!page["warnings"].as_array().unwrap().is_empty());
    let all = success(
        f.catalog(&catalog)
            .args(["--limit", "100"])
            .output()
            .unwrap(),
    );
    let items = all["page"]["items"].as_array().unwrap();
    let mut ids = BTreeSet::new();
    for candidate in items {
        let kind = candidate["kind"].as_str().unwrap();
        let id = candidate["id"].as_str().unwrap();
        assert!(id.starts_with(&format!("{kind}_")));
        assert!(ids.insert(id));
        for field in ["role", "label", "symbol", "path", "factId"] {
            assert!(!candidate[field].as_str().unwrap().is_empty(), "{field}");
        }
        assert!(candidate["line"].as_u64().unwrap() > 0);
        assert!(candidate["endLine"].as_u64().unwrap() >= candidate["line"].as_u64().unwrap());
        assert!(candidate["evidenceIds"]
            .as_array()
            .unwrap()
            .contains(&candidate["factId"]));
        assert!(candidate["pathIsFragment"].is_boolean());
    }
    for role in [
        "http_handler",
        "cli_command",
        "job",
        "module_function",
        "class_method",
    ] {
        assert!(items
            .iter()
            .any(|i| i["kind"] == "workflow" && i["role"] == role));
    }
    assert!(items
        .iter()
        .any(|i| i["symbol"].as_str().unwrap().ends_with("factory._nested")));
    assert!(!items.iter().any(|i| {
        let symbol = i["symbol"].as_str().unwrap();
        symbol.ends_with("._private") || symbol.ends_with(".nested_helper")
    }));
    let filtered = success(
        f.catalog(&catalog)
            .args([
                "--kind", "api", "--query", "pOsT", "--path", "app", "--limit", "1",
            ])
            .output()
            .unwrap(),
    );
    assert_eq!(filtered["page"]["matchedItems"], 1);
    assert_eq!(filtered["page"]["counts"], page["counts"]);
    assert_eq!(filtered["page"]["items"][0]["method"], "POST");
    assert_eq!(
        filtered["page"]["items"][0]["routePath"],
        "/items/{item_id}"
    );
    assert_eq!(filtered["page"]["items"][0]["pathIsFragment"], true);
    let empty = success(
        f.catalog(&catalog)
            .args(["--query", ".*"])
            .output()
            .unwrap(),
    );
    assert_eq!(
        empty["page"]["matchedItems"], 0,
        "query is literal, not regex"
    );
    assert_eq!(artifacts(&f.root), source);
    assert!(f.calls().is_empty() && !f.renders.exists());
    f.safe();
}

#[test]
fn saved_pages_are_stable_without_python_or_live_source() {
    let Some(f) = Fixture::new() else { return };
    let saved = f.saved();
    let catalog = directory(&saved, "catalogDirectory");
    let before = artifacts(&catalog);
    fs::remove_dir_all(&f.root).unwrap();
    let all = saved["page"]["items"].as_array().unwrap();
    let mut collected = Vec::new();
    let mut offset = 0;
    loop {
        let report = success(
            f.catalog(&catalog)
                .env("IDEOMANCER_PYTHON", f.cwd.join("missing python"))
                .args(["--limit", "7", "--offset", &offset.to_string()])
                .output()
                .unwrap(),
        );
        assert_eq!(report["page"]["snapshotId"], saved["snapshotId"]);
        assert_eq!(report["page"]["counts"], saved["page"]["counts"]);
        collected.extend(report["page"]["items"].as_array().unwrap().iter().cloned());
        match report["page"]["nextOffset"].as_u64() {
            Some(next) => {
                assert!(next > offset);
                offset = next;
            }
            None => break,
        }
    }
    assert_eq!(&collected, all);
    let beyond = success(
        f.catalog(&catalog)
            .args(["--offset", "999999"])
            .output()
            .unwrap(),
    );
    assert_eq!(beyond["page"]["items"], json!([]));
    assert!(beyond["page"]["nextOffset"].is_null());
    assert_eq!(artifacts(&catalog), before);
    assert!(f.calls().is_empty() && !f.renders.exists());
    f.safe();
}

#[test]
fn discovery_defaults_human_output_and_new_snapshots_replace_current_catalog() {
    let Some(f) = Fixture::new() else { return };
    let default = success(
        f.command()
            .arg("discover")
            .arg(&f.root)
            .arg("--json")
            .output()
            .unwrap(),
    );
    let catalog = directory(&default, "catalogDirectory");
    assert!(catalog.starts_with(f.cwd.join(".ideomancer/catalogs")));
    assert_eq!(default["page"]["limit"], 20);
    assert_eq!(default["page"]["nextOffset"], 20);
    let old = artifacts(&catalog);
    for (command, target) in [("discover", &f.root), ("catalog", &catalog)] {
        let human = f
            .command()
            .arg(command)
            .arg(target)
            .args(["--limit", "1"])
            .output()
            .unwrap();
        assert!(human.status.success());
        let text = String::from_utf8(human.stdout).unwrap().to_lowercase();
        assert!(text.contains("candidate") && text.contains("catalog"));
        assert!(text.contains("nextoffset") || text.contains("next offset"));
        assert!(
            text.replace('\\', "")
                .contains(catalog.file_name().unwrap().to_str().unwrap()),
            "human output lost saved catalog path: {text}"
        );
    }
    assert_eq!(
        artifacts(&catalog),
        old,
        "rediscovery rewrote immutable catalog"
    );
    fs::write(f.root.join("new.py"), "def newly_added():\n    pass\n").unwrap();
    let changed = success(
        f.command()
            .arg("discover")
            .arg(&f.root)
            .arg("--json")
            .output()
            .unwrap(),
    );
    assert_ne!(changed["snapshotId"], default["snapshotId"]);
    assert_eq!(changed["catalogDirectory"], default["catalogDirectory"]);
    assert_ne!(artifacts(&catalog), old);
    let reread = success(f.catalog(&catalog).output().unwrap());
    assert_eq!(reread["page"], changed["page"]);
    assert_eq!(catalog.file_name().unwrap(), "source-app");
    assert_eq!(fs::read_dir(catalog.parent().unwrap()).unwrap().count(), 1);
    assert!(f.calls().is_empty() && !f.renders.exists());
    f.safe();
}

#[test]
fn route_literal_exception_is_local_and_dynamic_paths_remain_unknown() {
    let Some(f) = Fixture::new() else { return };
    let saved = f.saved();
    let catalog = directory(&saved, "catalogDirectory");
    assert!(!saved.to_string().contains(SECRET));
    let dynamic = item(&saved, "api", "._dynamic");
    assert!(dynamic["routePath"].is_null());
    assert_eq!(dynamic["pathIsFragment"], true);
    assert!(saved["page"]["warnings"]
        .to_string()
        .to_lowercase()
        .contains("dynamic"));
    let evidence = read_json(&catalog.join("evidence.json"));
    let facts = evidence["facts"].as_array().unwrap();
    let route = facts
        .iter()
        .find(|fact| {
            fact["kind"] == "api_route"
                && fact["value"].as_str().unwrap().contains("/items/{item_id}")
        })
        .expect("route literal retained as allowlisted metadata");
    let metadata: Value = serde_json::from_str(route["value"].as_str().unwrap()).unwrap();
    assert_eq!(metadata["path"], "/items/{item_id}");
    assert_eq!(metadata["pathIsFragment"], true);
    for (path, (bytes, _)) in artifacts(&catalog) {
        assert!(
            !String::from_utf8_lossy(&bytes).contains(SECRET),
            "leaked in {path:?}"
        );
    }
    assert!(f.calls().is_empty());
    f.safe();
}

#[test]
fn api_and_class_views_render_saved_source_without_agent_or_rescan() {
    let Some(f) = Fixture::new() else { return };
    let saved = f.saved();
    let catalog = directory(&saved, "catalogDirectory");
    fs::write(f.root.join("app.py"), "invalid Python !!!\n").unwrap();
    for (kind, syntax) in [("api", "flowchart"), ("class", "classDiagram")] {
        let report = success(
            f.diagram(&catalog)
                .env("IDEOMANCER_PYTHON", f.cwd.join("missing python"))
                .args(["--kind", kind, "--limit", "1", "--offset", "1"])
                .output()
                .unwrap(),
        );
        assert_eq!(report["snapshotId"], saved["snapshotId"]);
        assert_eq!(report["kind"], kind);
        assert_eq!(directory(&report, "catalogDirectory"), catalog);
        let view = directory(&report, "viewDirectory");
        let diagram = if kind == "class" {
            assert!(report["mermaid"].is_null() && report["svg"].is_null());
            assert_eq!(report["diagrams"].as_array().unwrap().len(), 1);
            &report["diagrams"][0]
        } else {
            &report
        };
        for field in ["mermaid", "svg", "viewer", "legend"] {
            let artifact = if matches!(field, "mermaid" | "svg") {
                diagram
            } else {
                &report
            };
            let path = Path::new(artifact[field].as_str().unwrap());
            assert!(path.is_absolute() && path.starts_with(&view) && path.is_file());
        }
        assert!(report["error"].is_null());
        assert!(fs::read_to_string(diagram["mermaid"].as_str().unwrap())
            .unwrap()
            .contains(syntax));
        assert_eq!(
            fs::read(diagram["svg"].as_str().unwrap()).unwrap(),
            b"<svg width=\"300\" height=\"150\"/>"
        );
        let legend = fs::read_to_string(report["legend"].as_str().unwrap())
            .unwrap()
            .to_lowercase();
        assert!(legend.contains("source") && legend.contains("candidate"));
        if kind == "api" {
            assert!(legend.contains("fragment") && legend.contains("prefix"));
        }
        assert!(!legend.contains(&SECRET.to_lowercase()));
    }
    // API and selected class focus render independently, without an automatic full bundle.
    assert_eq!(fs::read_to_string(&f.renders).unwrap().lines().count(), 2);
    fs::remove_dir_all(&f.root).unwrap();
    success(
        f.diagram(&catalog)
            .args(["--kind", "class", "--no-render"])
            .output()
            .unwrap(),
    );
    assert!(f.calls().is_empty());
    f.safe();
}

#[test]
fn no_render_views_and_selected_api_class_entries_reuse_artifact_bytes() {
    let Some(f) = Fixture::new() else { return };
    let saved = f.saved();
    let catalog = directory(&saved, "catalogDirectory");
    for (kind, suffix) in [("api", "._dynamic"), ("class", ".Service")] {
        let selected = item(&saved, kind, suffix);
        let id = selected["id"].as_str().unwrap();
        let report = success(
            f.diagram(&catalog)
                .args(["--entry", id, "--no-render"])
                .output()
                .unwrap(),
        );
        assert_eq!(report["kind"], kind, "entry kind should be the default");
        assert!(report["svg"].is_null() && report["error"].is_null());
        let view = directory(&report, "viewDirectory");
        for field in ["mermaid", "legend", "viewer"] {
            let artifact = if kind == "class" && field == "mermaid" {
                &report["diagrams"][0]
            } else {
                &report
            };
            assert!(!fs::read(artifact[field].as_str().unwrap())
                .unwrap()
                .is_empty());
        }
        let legend = read_json(Path::new(report["legend"].as_str().unwrap()));
        assert_eq!(legend["entries"].as_array().unwrap().len(), 1);
        assert!(legend.to_string().contains(id));
        let before = artifacts(&view);
        let mut repeated = success(
            f.diagram(&catalog)
                .args(["--entry", id, "--no-render"])
                .output()
                .unwrap(),
        );
        if kind == "class" {
            assert_eq!(report["reused"], false);
            assert_eq!(repeated["reused"], true);
            repeated["reused"] = report["reused"].clone();
        }
        assert_eq!(report, repeated);
        assert_eq!(artifacts(&view), before, "cached source view was rewritten");
    }
    let filtered = success(
        f.diagram(&catalog)
            .args([
                "--kind",
                "api",
                "--query",
                "post",
                "--path",
                "app",
                "--no-render",
            ])
            .output()
            .unwrap(),
    );
    let legend = read_json(Path::new(filtered["legend"].as_str().unwrap()));
    assert_eq!(legend["entries"].as_array().unwrap().len(), 1);
    assert!(legend.to_string().contains("POST"));
    assert!(f.calls().is_empty() && !f.renders.exists());
    f.safe();
}

#[test]
fn no_render_rejects_unrecorded_svg_files_and_symlinks_without_writes() {
    let Some(f) = Fixture::new() else { return };
    let saved = f.saved();
    let catalog = directory(&saved, "catalogDirectory");
    let outside = f.cwd.join("untrusted.svg");
    fs::write(&outside, "<svg><script>untrusted()</script></svg>").unwrap();
    for kind in ["api", "class"] {
        let report = success(
            f.diagram(&catalog)
                .args(["--kind", kind, "--no-render"])
                .output()
                .unwrap(),
        );
        assert!(report["svg"].is_null());
        let view = directory(&report, "viewDirectory");
        let svg = view.join(if kind == "class" {
            "group-0001.svg"
        } else {
            "diagram.svg"
        });
        assert!(!svg.exists());
        for linked in [false, true] {
            if linked {
                symlink(&outside, &svg).unwrap();
            } else {
                fs::write(&svg, "<svg>unrecorded</svg>").unwrap();
            }
            let before = artifacts(&f.cwd);
            rejected(
                f.diagram(&catalog)
                    .args(["--kind", kind, "--no-render"])
                    .output()
                    .unwrap(),
                if linked || kind == "class" {
                    "unsafe"
                } else {
                    "unrecorded"
                },
            );
            assert_eq!(artifacts(&f.cwd), before, "rejection wrote artifacts");
            assert_eq!(
                fs::symlink_metadata(&svg).unwrap().file_type().is_symlink(),
                linked
            );
            if linked {
                assert_eq!(fs::read_link(&svg).unwrap(), outside);
            }
            assert!(f.calls().is_empty() && !f.renders.exists());
            fs::remove_file(&svg).unwrap();
        }
    }
    f.safe();
}

#[test]
fn workflow_entry_dispatches_one_focused_agent_and_caches_repeat() {
    let Some(f) = Fixture::new() else { return };
    let saved = f.saved();
    let catalog = directory(&saved, "catalogDirectory");
    let selected = item(&saved, "workflow", ".endpoint");
    let id = selected["id"].as_str().unwrap();
    let report = success(
        f.diagram(&catalog)
            .args(["--entry", id, "--no-render"])
            .output()
            .unwrap(),
    );
    assert_eq!(report["status"], "proposal_ready");
    assert_eq!(report["snapshotId"], saved["snapshotId"]);
    assert_eq!(report["focus"]["symbol"], selected["factId"]);
    assert_eq!(report["entrySymbol"], selected["symbol"]);
    assert_eq!(report["entryId"], selected["id"]);
    assert_eq!(report["focus"]["hops"], 3);
    assert_eq!(report["focus"]["maxFacts"], 600);
    let run = directory(&report, "runDirectory");
    let calls = f.calls();
    assert_eq!(calls.len(), 1, "workflow must never auto-batch candidates");
    assert_eq!(calls[0]["model"], "offline-discovery-model");
    let packet = &calls[0]["input"];
    assert_eq!(packet["focus"]["anchor"]["id"], selected["factId"]);
    assert_eq!(packet, &read_json(&run.join("evidence-packet.json")));
    let facts = packet["facts"].as_array().unwrap();
    assert!(
        facts.len()
            < read_json(&catalog.join("evidence.json"))["facts"]
                .as_array()
                .unwrap()
                .len()
    );
    assert!(!facts.iter().any(|fact| fact["path"] == "unrelated.py"));
    assert!(!calls[0].to_string().contains(SECRET));
    let routes: Vec<_> = facts
        .iter()
        .filter(|fact| fact["kind"] == "api_route")
        .collect();
    assert_eq!(
        routes.len(),
        2,
        "selected GET and POST metadata must be retained"
    );
    let mut methods = BTreeSet::new();
    for route in routes {
        assert_eq!(route["symbol"], selected["symbol"]);
        assert_eq!(route["path"], selected["path"]);
        assert!(packet["focus"]["visitedSymbols"]
            .as_array()
            .unwrap()
            .contains(&route["symbol"]));
        assert!(
            facts.iter().any(|owner| {
                matches!(owner["kind"].as_str(), Some("function" | "async_function"))
                    && owner["symbol"] == route["symbol"]
                    && owner["path"] == route["path"]
                    && owner["id"] == selected["factId"]
                    && route["endLine"].as_u64().unwrap() < owner["line"].as_u64().unwrap()
            }),
            "route metadata must belong to the selected reachable callable"
        );
        let metadata: Value = serde_json::from_str(route["value"].as_str().unwrap()).unwrap();
        assert_eq!(metadata["path"], "/items/{item_id}");
        assert_eq!(metadata["pathIsFragment"], true);
        for method in metadata["methods"].as_array().unwrap() {
            methods.insert(method.as_str().unwrap().to_owned());
        }
    }
    assert_eq!(
        methods,
        BTreeSet::from(["GET".to_owned(), "POST".to_owned()])
    );
    assert!(
        !calls[0].to_string().contains("/nested"),
        "unrelated route fragment reached agent"
    );
    let before = artifacts(&run);
    assert_eq!(
        success(
            f.diagram(&catalog)
                .args(["--entry", id, "--no-render"])
                .output()
                .unwrap()
        ),
        report
    );
    assert_eq!(f.calls().len(), 1);
    assert_eq!(artifacts(&run), before);
    assert!(!f.renders.exists());
    f.safe();
}

#[test]
fn api_can_focus_workflow_and_extract_only_keeps_dispatch_local() {
    let Some(f) = Fixture::new() else { return };
    let saved = f.saved();
    let catalog = directory(&saved, "catalogDirectory");
    let selected = item(&saved, "api", "._dynamic");
    let id = selected["id"].as_str().unwrap();
    let report = success(
        f.diagram(&catalog)
            .args([
                "--kind",
                "workflow",
                "--entry",
                id,
                "--focus-hops",
                "0",
                "--focus-facts",
                "20",
                "--extract-only",
            ])
            .output()
            .unwrap(),
    );
    assert_eq!(report["status"], "extracted");
    assert_eq!(report["focus"]["symbol"], selected["factId"]);
    assert_eq!(report["entrySymbol"], selected["symbol"]);
    assert_eq!(report["entryId"], selected["id"]);
    assert_eq!(report["focus"]["hops"], 0);
    assert_eq!(report["focus"]["maxFacts"], 20);
    let run = directory(&report, "runDirectory");
    let packet = read_json(&run.join("evidence-packet.json"));
    assert_eq!(packet["focus"]["anchor"]["id"], selected["factId"]);
    assert!(packet["facts"].as_array().unwrap().len() <= 20);
    assert!(f.calls().is_empty() && !f.renders.exists());
    let ready = success(
        f.diagram(&catalog)
            .args(["--kind", "workflow", "--entry", id, "--no-render"])
            .output()
            .unwrap(),
    );
    assert_eq!(ready["status"], "proposal_ready");
    assert_eq!(ready["focus"]["symbol"], selected["factId"]);
    assert_eq!(ready["entrySymbol"], selected["symbol"]);
    assert_eq!(ready["entryId"], selected["id"]);
    assert_eq!(f.calls().len(), 1);
    assert_eq!(
        f.calls()[0]["input"]["focus"]["anchor"]["id"],
        selected["factId"]
    );
    f.safe();
}

#[test]
fn stale_snapshot_is_rejected_before_agent_even_after_cached_workflow() {
    let Some(f) = Fixture::new() else { return };
    let saved = f.saved();
    let catalog = directory(&saved, "catalogDirectory");
    let selected = item(&saved, "workflow", ".endpoint");
    let id = selected["id"].as_str().unwrap();
    success(
        f.diagram(&catalog)
            .args(["--entry", id, "--no-render"])
            .output()
            .unwrap(),
    );
    assert_eq!(f.calls().len(), 1);
    fs::write(f.root.join("new.py"), "def changed():\n    return 99\n").unwrap();
    let before = artifacts(&catalog);
    for flags in [
        vec!["--no-render"],
        vec!["--no-render", "--retry"],
        vec!["--extract-only"],
    ] {
        rejected(
            f.diagram(&catalog)
                .args(["--entry", id])
                .args(flags)
                .output()
                .unwrap(),
            "stale",
        );
        assert_eq!(f.calls().len(), 1, "stale catalog dispatched agent");
    }
    let uncached = item(&saved, "workflow", ".helper");
    rejected(
        f.diagram(&catalog)
            .args(["--entry", uncached["id"].as_str().unwrap(), "--no-render"])
            .output()
            .unwrap(),
        "stale",
    );
    assert_eq!(f.calls().len(), 1, "uncached stale entry dispatched agent");
    assert_eq!(artifacts(&catalog), before);
    success(
        f.diagram(&catalog)
            .args(["--kind", "api", "--no-render"])
            .output()
            .unwrap(),
    );
    f.safe();
}

#[test]
fn unknown_ambiguous_and_incompatible_entries_fail_without_agent() {
    let Some(f) = Fixture::new() else { return };
    let saved = f.saved();
    let catalog = directory(&saved, "catalogDirectory");
    rejected(
        f.diagram(&catalog)
            .args(["--entry", "workflow_nonexistent", "--no-render"])
            .output()
            .unwrap(),
        "no catalog",
    );
    rejected(
        f.diagram(&catalog)
            .args(["--entry", "endpoint", "--no-render"])
            .output()
            .unwrap(),
        "ambiguous",
    );
    let narrowed = success(
        f.diagram(&catalog)
            .args([
                "--kind",
                "workflow",
                "--entry",
                "endpoint",
                "--extract-only",
            ])
            .output()
            .unwrap(),
    );
    assert_eq!(narrowed["status"], "extracted");
    assert_eq!(
        narrowed["focus"]["symbol"],
        item(&saved, "workflow", ".endpoint")["factId"]
    );
    assert_eq!(
        narrowed["entrySymbol"],
        item(&saved, "workflow", ".endpoint")["symbol"]
    );
    let narrowed = success(
        f.diagram(&catalog)
            .args(["--kind", "api", "--entry", "_dynamic", "--no-render"])
            .output()
            .unwrap(),
    );
    assert_eq!(narrowed["kind"], "api");
    let legend = read_json(Path::new(narrowed["legend"].as_str().unwrap()));
    assert_eq!(legend["entries"].as_array().unwrap().len(), 1);
    assert!(legend
        .to_string()
        .contains(item(&saved, "api", "._dynamic")["id"].as_str().unwrap()));
    let class = item(&saved, "class", ".Service");
    let output = f
        .diagram(&catalog)
        .args([
            "--kind",
            "workflow",
            "--entry",
            class["id"].as_str().unwrap(),
            "--no-render",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success(), "class cannot anchor a workflow");
    let output = f
        .diagram(&catalog)
        .args(["--kind", "workflow", "--no-render"])
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "workflow requires an entry, not automatic batching"
    );
    assert!(f.calls().is_empty() && !f.renders.exists());
    f.safe();
}

#[test]
fn repeated_workflow_declarations_select_exact_facts_in_current_run() {
    let Some(f) = Fixture::new() else { return };
    fs::write(
        f.root.join("repeated.py"),
        "def repeated():\n    return 1\n\ndef repeated():\n    return 2\n",
    )
    .unwrap();
    let saved = f.saved();
    let catalog = directory(&saved, "catalogDirectory");
    let entries: Vec<_> = saved["page"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|candidate| {
            candidate["kind"] == "workflow" && candidate["symbol"] == "repeated.repeated"
        })
        .collect();
    assert_eq!(entries.len(), 2);
    assert_ne!(entries[0]["id"], entries[1]["id"]);
    assert_ne!(entries[0]["factId"], entries[1]["factId"]);
    let mut runs = BTreeSet::new();
    for selected in entries {
        let report = success(
            f.diagram(&catalog)
                .args([
                    "--entry",
                    selected["id"].as_str().unwrap(),
                    "--extract-only",
                ])
                .output()
                .unwrap(),
        );
        assert_eq!(report["status"], "extracted");
        assert_eq!(report["entryId"], selected["id"]);
        assert_eq!(report["entrySymbol"], selected["symbol"]);
        assert_eq!(report["focus"]["symbol"], selected["factId"]);
        let run = directory(&report, "runDirectory");
        let packet = read_json(&run.join("evidence-packet.json"));
        for (anchor_field, item_field) in [
            ("id", "factId"),
            ("symbol", "symbol"),
            ("path", "path"),
            ("line", "line"),
            ("endLine", "endLine"),
        ] {
            assert_eq!(
                packet["focus"]["anchor"][anchor_field],
                selected[item_field]
            );
        }
        runs.insert(run);
    }
    assert_eq!(runs.len(), 1);
    for symbol in ["repeated", "repeated.repeated"] {
        for narrowed in [false, true] {
            let mut command = f.diagram(&catalog);
            command.args(["--entry", symbol, "--extract-only"]);
            if narrowed {
                command.args(["--kind", "workflow"]);
            }
            rejected(command.output().unwrap(), "ambiguous");
        }
    }
    assert!(f.calls().is_empty() && !f.renders.exists());
    f.safe();
}

#[test]
fn discovery_holds_the_slot_lock_while_python_has_an_older_snapshot() {
    let Some(f) = Fixture::new() else { return };
    let ready = f.cwd.join("python.ready");
    let release = f.cwd.join("python.release");
    let gated = f.cwd.join("gated-python");
    fs::write(&gated, format!("#!{}\nimport subprocess,sys,time\nfrom pathlib import Path\nresponse=subprocess.run([{},*sys.argv[1:]],input=sys.stdin.buffer.read(),capture_output=True)\nPath({}).touch()\ndeadline=time.monotonic()+10\nwhile not Path({}).exists():\n    if time.monotonic()>deadline: sys.exit(8)\n    time.sleep(0.01)\nsys.stdout.buffer.write(response.stdout)\nsys.stderr.buffer.write(response.stderr)\nsys.exit(response.returncode)\n", f.python.display(), serde_json::to_string(&f.python).unwrap(), serde_json::to_string(&ready).unwrap(), serde_json::to_string(&release).unwrap())).unwrap();
    fs::set_permissions(&gated, fs::Permissions::from_mode(0o755)).unwrap();
    let mut child = f
        .discover()
        .arg("--python")
        .arg(&gated)
        .arg("--json")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let start = std::time::Instant::now();
    while !ready.exists() {
        assert!(start.elapsed() < std::time::Duration::from_secs(10));
        assert!(child.try_wait().unwrap().is_none());
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(f.base.join(".source-app.lock").is_file());
    fs::write(f.root.join("v2.py"), "def newer_source():\n    pass\n").unwrap();
    let competing = f.discover().arg("--json").output().unwrap();
    assert!(!competing.status.success());
    assert!(String::from_utf8_lossy(&competing.stderr).contains("locked"));
    fs::write(&release, "").unwrap();
    let older = success(child.wait_with_output().unwrap());
    let newer = f.saved();
    assert_ne!(older["snapshotId"], newer["snapshotId"]);
    assert_eq!(older["catalogDirectory"], newer["catalogDirectory"]);
    let current = directory(&newer, "catalogDirectory");
    assert_eq!(
        read_json(&current.join("catalog.json"))["snapshotId"],
        newer["snapshotId"]
    );
    assert!(f.calls().is_empty());
}

#[test]
fn source_view_preserves_notes_added_during_a_blocked_render() {
    let Some(f) = Fixture::new() else { return };
    let saved = f.saved();
    let catalog = directory(&saved, "catalogDirectory");
    let report = success(
        f.diagram(&catalog)
            .args(["--kind", "api", "--no-render"])
            .output()
            .unwrap(),
    );
    let view = directory(&report, "viewDirectory");
    let ready = f.cwd.join("renderer.ready");
    let release = f.cwd.join("renderer.release");
    fs::write(&f.renderer, format!("#!{}\nimport sys,time\nfrom pathlib import Path\nPath({}).touch()\ndeadline=time.monotonic()+10\nwhile not Path({}).exists():\n    if time.monotonic()>deadline: sys.exit(8)\n    time.sleep(0.01)\nPath(sys.argv[4]).write_text('<svg width=\"300\" height=\"150\"/>')\n", f.python.display(), serde_json::to_string(&ready).unwrap(), serde_json::to_string(&release).unwrap())).unwrap();
    let mut child = f
        .diagram(&catalog)
        .args(["--kind", "api"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let start = std::time::Instant::now();
    while !ready.exists() {
        assert!(start.elapsed() < std::time::Duration::from_secs(10));
        assert!(child.try_wait().unwrap().is_none());
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    fs::write(view.join("notes.txt"), "manual source-view notes").unwrap();
    let before = artifacts(&view);
    fs::write(&release, "").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("Storage changed"), "{error}");
    assert!(error.contains("reconciliation"), "{error}");
    assert_eq!(artifacts(&view), before);
    let preserved: Vec<_> = fs::read_dir(catalog.join("views"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".source-view-init-")
        })
        .collect();
    assert_eq!(preserved.len(), 1);
    assert!(error.contains(&preserved[0].display().to_string()));
    assert!(preserved[0].join("diagram.svg").is_file());
    assert!(f.calls().is_empty());
}

#[test]
fn views_index_links_only_current_artifacts_and_reuses_identical_bytes() {
    let Some(f) = Fixture::new() else { return };
    let saved = f.saved();
    let catalog = directory(&saved, "catalogDirectory");
    let api_id = saved["page"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["kind"] == "api")
        .unwrap()["id"]
        .as_str()
        .unwrap();
    let first = success(
        f.diagram(&catalog)
            .args(["--kind", "api", "--no-render"])
            .env("IDEOMANCER_PYTHON", f.cwd.join("missing python"))
            .env("IDEOMANCER_AGENT", f.cwd.join("missing agent"))
            .output()
            .unwrap(),
    );
    let index = catalog.join("views/index.html");
    assert_eq!(first["viewsIndex"], index.to_str().unwrap());
    let html = fs::read_to_string(&index).unwrap();
    assert!(html.contains("href=\"api/index.html\""));
    assert!(!html.contains("href=\"class/"));
    assert!(!html.contains("diagram.svg"));
    for args in [
        vec!["--kind", "class", "--no-render"],
        vec!["--entry", api_id, "--view", "network", "--no-render"],
        vec!["--entry", api_id, "--view", "services", "--no-render"],
    ] {
        success(
            f.diagram(&catalog)
                .args(args)
                .env("IDEOMANCER_PYTHON", f.cwd.join("missing python"))
                .env("IDEOMANCER_AGENT", f.cwd.join("missing agent"))
                .output()
                .unwrap(),
        );
    }
    let html = fs::read_to_string(&index).unwrap();
    for view in ["api", "class", "network", "services"] {
        assert_eq!(
            html.matches(&format!("href=\"{view}/index.html\"")).count(),
            1
        );
        let mmd = if view == "class" {
            "group-0001.mmd"
        } else {
            "diagram.mmd"
        };
        assert!(html.contains(&format!("href=\"{view}/{mmd}\"")));
        assert!(html.contains(&format!("href=\"{view}/legend.json\"")));
    }
    assert!(!html.contains("class-overview"));
    assert!(!html.contains("<script"));
    for link in html.split("href=\"").skip(1) {
        let target = link.split('"').next().unwrap();
        assert!(catalog.join("views").join(target).is_file(), "{target}");
    }
    assert!(f.calls().is_empty());
    assert!(!f.renders.exists());
    let before = artifacts(&catalog);
    success(
        f.diagram(&catalog)
            .args(["--kind", "class", "--no-render"])
            .output()
            .unwrap(),
    );
    assert_eq!(
        artifacts(&catalog),
        before,
        "cached index changed bytes/mtime"
    );

    success(
        f.diagram(&catalog)
            .args(["--kind", "class"])
            .output()
            .unwrap(),
    );
    let html = fs::read_to_string(&index).unwrap();
    assert!(html.contains("href=\"class/group-0001.svg\""));
    assert!(!html.contains("href=\"api/diagram.svg\""));
    rejected(
        f.diagram(&catalog)
            .args(["--kind", "api", "--mmdc"])
            .arg(f.cwd.join("missing renderer"))
            .output()
            .unwrap(),
        "mmdc",
    );
    assert_eq!(fs::read_to_string(&index).unwrap(), html);
    f.safe();
}

#[test]
fn views_index_escapes_application_names_and_rejects_symlinks() {
    let Some(f) = Fixture::new() else { return };
    let root = f.cwd.join("app <script>alert & \"test\"");
    fs::rename(&f.root, &root).unwrap();
    let saved = success(
        f.command()
            .arg("discover")
            .arg(&root)
            .arg("--out")
            .arg(&f.base)
            .arg("--json")
            .output()
            .unwrap(),
    );
    let catalog = directory(&saved, "catalogDirectory");
    success(
        f.diagram(&catalog)
            .args(["--kind", "class", "--no-render"])
            .output()
            .unwrap(),
    );
    let index = catalog.join("views/index.html");
    let html = fs::read_to_string(&index).unwrap();
    assert!(html.contains("app &lt;script&gt;alert &amp; &quot;test&quot;"));
    assert!(!html.contains("<script>"));
    let outside = f.cwd.join("outside.html");
    fs::write(&outside, "must not be overwritten").unwrap();
    fs::remove_file(&index).unwrap();
    symlink(&outside, &index).unwrap();
    rejected(
        f.diagram(&catalog)
            .args(["--kind", "api", "--no-render"])
            .output()
            .unwrap(),
        "unsafe",
    );
    assert_eq!(
        fs::read_to_string(&outside).unwrap(),
        "must not be overwritten"
    );
    assert!(fs::symlink_metadata(&index)
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(f.calls().is_empty());
    f.safe();
}

#[test]
fn new_catalog_discards_views_and_each_kind_keeps_only_its_current_page() {
    let Some(f) = Fixture::new() else { return };
    let saved = f.saved();
    let catalog = directory(&saved, "catalogDirectory");
    let first = success(
        f.diagram(&catalog)
            .args(["--kind", "api", "--limit", "1", "--no-render"])
            .output()
            .unwrap(),
    );
    let view = directory(&first, "viewDirectory");
    assert_eq!(view, catalog.join("views/api"));
    let second = success(
        f.diagram(&catalog)
            .args([
                "--kind",
                "api",
                "--limit",
                "1",
                "--offset",
                "1",
                "--no-render",
            ])
            .output()
            .unwrap(),
    );
    assert_eq!(directory(&second, "viewDirectory"), view);
    assert_ne!(first["page"], second["page"]);
    assert_eq!(fs::read_dir(catalog.join("views")).unwrap().count(), 2);
    assert!(catalog.join("views/index.html").is_file());
    fs::write(f.root.join("new.py"), "def newly_added():\n    pass\n").unwrap();
    let changed = f.saved();
    assert_eq!(directory(&changed, "catalogDirectory"), catalog);
    assert_ne!(changed["snapshotId"], saved["snapshotId"]);
    assert!(!catalog.join("views").exists());
    assert_eq!(fs::read_dir(&f.base).unwrap().count(), 1);
    assert!(f.calls().is_empty());
}

#[test]
fn tampered_catalog_evidence_and_receipt_are_rejected_and_ranges_are_usage_errors() {
    let temp = tempdir().unwrap();
    for (subcommand, flags) in [
        ("discover", vec!["--limit", "0"]),
        ("discover", vec!["--limit", "101"]),
        ("discover", vec!["--offset", "-1"]),
        ("catalog", vec!["--limit", "0"]),
        ("catalog", vec!["--limit", "101"]),
        ("catalog", vec!["--offset", "-1"]),
        ("diagram", vec!["--kind", "api", "--limit", "0"]),
        ("diagram", vec!["--kind", "class", "--limit", "31"]),
        ("diagram", vec!["--kind", "api", "--offset", "-1"]),
        (
            "diagram",
            vec![
                "--kind",
                "workflow",
                "--entry",
                "workflow_fake",
                "--focus-hops",
                "6",
            ],
        ),
        (
            "diagram",
            vec![
                "--kind",
                "workflow",
                "--entry",
                "workflow_fake",
                "--focus-facts",
                "0",
            ],
        ),
    ] {
        let output = cli()
            .current_dir(temp.path())
            .arg(subcommand)
            .arg(temp.path())
            .args(&flags)
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(2),
            "{subcommand} {flags:?}: {output:?}"
        );
    }
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
    let Some(f) = Fixture::new() else { return };
    let saved = f.saved();
    let catalog = directory(&saved, "catalogDirectory");
    let receipt = fs::read_dir(&catalog)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.file_name().unwrap().to_string_lossy().contains("receipt"))
        .expect("saved catalog must have an integrity receipt");
    for path in [
        catalog.join("catalog.json"),
        catalog.join("evidence.json"),
        receipt,
    ] {
        let original = fs::read(&path).unwrap();
        let mut value: Value = serde_json::from_slice(&original).unwrap();
        if path.file_name().unwrap() == "catalog.json" {
            value["items"][0]["label"] = json!("tampered label");
        } else if path.file_name().unwrap() == "evidence.json" {
            value["facts"][0]["value"] = json!("tampered evidence");
        } else {
            value = json!({"tampered": true});
        }
        fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
        let before = artifacts(&catalog);
        for mut command in [f.catalog(&catalog), {
            let mut command = f.diagram(&catalog);
            command.args(["--kind", "api", "--no-render"]);
            command
        }] {
            let output = command.output().unwrap();
            assert!(
                !output.status.success(),
                "accepted tampered {path:?}: {output:?}"
            );
            assert_eq!(
                artifacts(&catalog),
                before,
                "reader repaired tampered catalog"
            );
        }
        fs::write(&path, original).unwrap();
    }
    assert!(f.calls().is_empty() && !f.renders.exists());
    f.safe();
}
