//! Source-only factory provenance contracts through the compiled CLI.
#![cfg(unix)]

use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;
use std::time::SystemTime;
use tempfile::{tempdir, TempDir};

fn python() -> Option<&'static Path> {
    static PYTHON: OnceLock<Option<PathBuf>> = OnceLock::new();
    let path = PYTHON.get_or_init(|| {
        let output = Command::new(
            std::env::var_os("IDEOMANCER_TEST_PYTHON").unwrap_or_else(|| "python3.12".into()),
        )
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
        eprintln!("SKIP network provenance: install Python >=3.12 or set IDEOMANCER_TEST_PYTHON");
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
    serde_json::from_slice(&output.stdout).expect("CLI JSON report")
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

fn files(root: &Path) -> BTreeMap<PathBuf, (Vec<u8>, SystemTime)> {
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

fn source(annotation: &str, service: bool) -> String {
    let handler = if service {
        "class Service:\n    def run(self, ctx: Context):\n        return ctx.benchling.containers.checkin()\n\n@app.post('/checkin')\ndef endpoint(service: Service, ctx: Context):\n    collaborator = service\n    return collaborator.run(ctx)\n"
    } else {
        "@app.post('/checkin')\ndef endpoint(ctx: Context):\n    return ctx.benchling.containers.checkin()\n"
    };
    format!(
        r#"from unavailable_framework import FastAPI
from benchling_sdk import Benchling
from typing import Any
app = FastAPI()

class Wrapper:
    def __getattr__(self, name: str) -> Any:
        return unavailable(name)

class Factory:
    def create(self, env: str) -> {annotation}:
        return unavailable(env)

class Context:
    def __init__(self, factory: Factory, env: str):
        self.benchling = factory.create(env)

{handler}
@app.get('/health')
def health():
    return unknown_health()
"#
    )
}

struct Fixture {
    _temp: TempDir,
    cwd: PathBuf,
    root: PathBuf,
    python: PathBuf,
    marker: PathBuf,
}

impl Fixture {
    fn new(source: &str) -> Option<Self> {
        let python = python()?.to_path_buf();
        let temp = tempdir().unwrap();
        let cwd = fs::canonicalize(temp.path()).unwrap();
        let root = cwd.join("source app");
        fs::create_dir(&root).unwrap();
        let marker = cwd.join("SOURCE_MUST_NOT_EXECUTE");
        fs::write(
            root.join("app.py"),
            format!(
                "from pathlib import Path\nPath({}).write_text('executed')\nraise RuntimeError('source must not execute')\n{source}",
                serde_json::to_string(marker.to_str().unwrap()).unwrap()
            ),
        )
        .unwrap();
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
            .env("IDEOMANCER_PYTHON", "/usr/bin/false")
            .env("IDEOMANCER_AGENT", "/usr/bin/false")
            .env("IDEOMANCER_MMDC", "/usr/bin/false")
            .env_remove("IDEOMANCER_MODEL");
        command
    }

    fn discover(&self) -> Value {
        success(
            self.command()
                .arg("discover")
                .arg(&self.root)
                .env("IDEOMANCER_PYTHON", &self.python)
                .arg("--out")
                .arg(self.cwd.join("catalog output"))
                .args(["--json", "--kind", "api", "--limit", "20"])
                .output()
                .unwrap(),
        )
    }

    fn entry(&self, saved: &Value, suffix: &str) -> String {
        let items: Vec<_> = saved["page"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["symbol"].as_str().unwrap().ends_with(suffix))
            .collect();
        assert_eq!(items.len(), 1, "{saved}");
        items[0]["id"].as_str().unwrap().to_owned()
    }

    fn diagram(&self, saved: &Value, entry: &str, view: &str, facts: usize) -> Output {
        self.command()
            .arg("diagram")
            .arg(saved["catalogDirectory"].as_str().unwrap())
            .args([
                "--entry",
                entry,
                "--view",
                view,
                "--no-render",
                "--focus-hops",
                "4",
                "--focus-facts",
            ])
            .arg(facts.to_string())
            .output()
            .unwrap()
    }

    fn safe(&self, saved: &Value) {
        assert!(!self.marker.exists(), "application source executed");
        assert!(
            !self.cwd.join(".ideomancer/analyses").exists(),
            "agent workflow dispatched"
        );
        assert!(Path::new(saved["catalogDirectory"].as_str().unwrap()).is_dir());
    }
}

fn sdk_boundaries(data: &Value) -> Vec<&Value> {
    data["boundaries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|boundary| {
            boundary["client"]
                .as_str()
                .is_some_and(|client| client.starts_with("benchling_sdk"))
        })
        .collect()
}

fn assert_unknown(data: &Value) {
    assert!(
        sdk_boundaries(data).is_empty(),
        "unsupported SDK candidate: {data}"
    );
    let call = data["calls"]
        .as_array()
        .unwrap()
        .iter()
        .find(|call| call["label"] == "ctx.benchling.containers.checkin")
        .unwrap_or_else(|| panic!("missing unresolved call: {data}"));
    assert!(
        call["classification"].as_str().unwrap().contains("UNKNOWN"),
        "{call}"
    );
    assert!(call["client"].is_null(), "{call}");
}

fn assert_candidate(report: &Value, annotation: &str) {
    let data = legend(report);
    assert!(data["contract"].is_null(), "{data}");
    assert_eq!(data["contractMatch"]["status"], "source-only");
    let candidates = sdk_boundaries(&data);
    assert_eq!(candidates.len(), 1, "{data}");
    let candidate = candidates[0];
    assert!(candidate["classification"]
        .as_str()
        .unwrap()
        .to_lowercase()
        .contains("candidate"));
    assert_eq!(candidate["operation"], "containers.checkin");
    let refs = candidate["sourceRefs"].as_array().unwrap();
    for kind in ["return_annotation", "import", "class", "call"] {
        assert!(
            refs.iter()
                .any(|r| r["kind"] == kind || kind == "import" && r["kind"] == "from_import"),
            "missing {kind} proof: {candidate}"
        );
    }
    for reference in refs {
        assert!(reference["factId"].is_string(), "{reference}");
        assert!(reference["line"].as_u64().unwrap() > 0, "{reference}");
        assert_eq!(reference["path"], "app.py");
    }
    // The annotation citation and every declared arm must survive serialization;
    // this does not prescribe names for the new branch metadata fields.
    let serialized = candidate.to_string();
    if annotation.contains('|') {
        for member in annotation.split('|').map(str::trim) {
            assert!(
                candidate.to_string().contains(&format!("\"{member}\"")),
                "declared return alternative {member} lost: {candidate}"
            );
        }
    }
    assert!(
        serialized.to_lowercase().contains("runtime"),
        "runtime uncertainty missing: {data}"
    );
    assert!(serialized.contains("UNKNOWN"), "{data}");
    if annotation.contains('|') {
        let alternatives = candidate["branches"].as_array().unwrap();
        let wrapper = alternatives
            .iter()
            .find(|branch| branch["member"] == "Wrapper")
            .expect("local wrapper return alternative");
        assert!(
            wrapper["classification"]
                .as_str()
                .unwrap()
                .contains("UNKNOWN"),
            "wrapper dispatch incorrectly resolved: {wrapper}"
        );
    }
    assert!(
        candidate.to_string().to_lowercase().contains("basis"),
        "candidate basis missing: {candidate}"
    );
    let mermaid = artifact(report, "mermaid");
    assert!(
        mermaid.contains("Benchling"),
        "SDK absent from diagram: {mermaid}"
    );
    for edge in data["edges"].as_array().unwrap().iter().filter(|edge| {
        edge["kind"]
            .as_str()
            .is_some_and(|kind| kind.contains("SDK"))
    }) {
        let from = edge["from"].as_str().unwrap();
        let to = edge["to"].as_str().unwrap();
        assert!(
            mermaid.lines().any(|line| {
                line.trim_start().starts_with(&format!("{from} -.->"))
                    && line.trim_end().ends_with(to)
            }),
            "SDK edge is not dashed: {edge}\n{mermaid}"
        );
    }
    assert!(!mermaid.contains("https://") && !mermaid.contains("http://"));
    assert!(!mermaid.contains("POST containers") && !mermaid.contains("GET containers"));
}

#[test]
fn plain_and_union_factory_returns_are_possible_sdk_boundaries_not_runtime_claims() {
    for annotation in [
        "Benchling",
        "Benchling | Wrapper",
        "Wrapper | Benchling",
        "Benchling | Wrapper | Other | Last",
    ] {
        for service in [false, true] {
            let text = source(annotation, service).replace(
                "class Factory:",
                "class Other:\n    pass\n\nclass Last:\n    pass\n\nclass Factory:",
            );
            let Some(f) = Fixture::new(&text) else { return };
            let saved = f.discover();
            let entry = f.entry(&saved, ".endpoint");
            let original = files(&f.root);
            for view in ["network", "services"] {
                let report = success(f.diagram(&saved, &entry, view, 160));
                assert_candidate(&report, annotation);
            }
            assert_eq!(files(&f.root), original, "source was modified");
            f.safe(&saved);
        }
    }
}

#[test]
fn ambiguous_or_rebound_provenance_never_borrows_an_sdk_annotation() {
    let base = source("Benchling | Wrapper", false);
    let cases = [
        base.replace("class Context:", "class Factory:\n    def create(self, env: str) -> Benchling:\n        return unavailable(env)\n\nclass Context:"),
        base.replace("class Context:", "    def create(self, env: str) -> Benchling:\n        return unavailable(env)\n\nclass Context:"),
        base.replace("        self.benchling =", "        factory = unavailable_factory\n        self.benchling ="),
        base.replace("        self.benchling = factory.create(env)", "        self.benchling = factory.create(env)\n        self.benchling = unavailable_client"),
    ];
    for text in cases {
        let Some(f) = Fixture::new(&text) else { return };
        let saved = f.discover();
        let entry = f.entry(&saved, ".endpoint");
        for view in ["network", "services"] {
            assert_unknown(&legend(&success(f.diagram(&saved, &entry, view, 160))));
        }
        f.safe(&saved);
    }
}

#[test]
fn quoted_generic_and_over_cap_return_annotations_remain_unknown() {
    for annotation in [
        "'Benchling | Wrapper'",
        "list[Benchling]",
        "Benchling | list[Wrapper]",
        "Benchling | Wrapper | A | B | C",
    ] {
        let Some(f) = Fixture::new(&source(annotation, false)) else {
            return;
        };
        let saved = f.discover();
        let entry = f.entry(&saved, ".endpoint");
        for view in ["network", "services"] {
            assert_unknown(&legend(&success(f.diagram(&saved, &entry, view, 160))));
        }
        f.safe(&saved);
    }
}

#[test]
fn insufficient_fact_budget_cannot_emit_an_unsupported_sdk_edge() {
    let Some(f) = Fixture::new(&source("Benchling | Wrapper", false)) else {
        return;
    };
    let saved = f.discover();
    let entry = f.entry(&saved, ".endpoint");
    for view in ["network", "services"] {
        let output = f.diagram(&saved, &entry, view, 1);
        if output.status.success() {
            let report = success(output);
            let data = legend(&report);
            assert!(
                sdk_boundaries(&data).is_empty(),
                "incomplete proof accepted: {data}"
            );
            assert!(data["retainedFactCount"].as_u64().unwrap() <= 1, "{data}");
            assert!(!artifact(&report, "mermaid").contains("SDK boundary"));
        } else {
            let diagnostic = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                diagnostic.to_lowercase().contains("fact")
                    || diagnostic.to_lowercase().contains("limit"),
                "{diagnostic}"
            );
        }
    }
    f.safe(&saved);
}

#[test]
fn saved_source_only_views_are_deterministic_and_keep_only_current_outputs() {
    let Some(f) = Fixture::new(&source("Benchling | Wrapper", true)) else {
        return;
    };
    let saved = f.discover();
    let entry = f.entry(&saved, ".endpoint");
    let health = f.entry(&saved, ".health");
    fs::remove_dir_all(&f.root).unwrap();
    for view in ["network", "services"] {
        let first = success(f.diagram(&saved, &entry, view, 160));
        assert_candidate(&first, "Benchling | Wrapper");
        let directory = Path::new(first["viewDirectory"].as_str().unwrap());
        let before = files(directory);
        assert_eq!(success(f.diagram(&saved, &entry, view, 160)), first);
        assert_eq!(files(directory), before, "repeat rewrote cached outputs");
        let changed = success(f.diagram(&saved, &health, view, 160));
        assert_eq!(changed["viewDirectory"], first["viewDirectory"]);
        assert_eq!(legend(&changed)["item"]["id"], health);
        assert!(sdk_boundaries(&legend(&changed)).is_empty());
        assert!(!artifact(&changed, "mermaid").contains("Benchling"));
        let current = files(directory);
        assert_eq!(
            current.keys().collect::<Vec<_>>(),
            before.keys().collect::<Vec<_>>(),
            "historical outputs retained"
        );
        assert_ne!(current, before);
        assert_eq!(success(f.diagram(&saved, &health, view, 160)), changed);
        assert_eq!(files(directory), current);
    }
    let views = Path::new(saved["catalogDirectory"].as_str().unwrap()).join("views");
    assert_eq!(
        fs::read_dir(views).unwrap().count(),
        3,
        "historical view directories retained"
    );
    f.safe(&saved);
}
