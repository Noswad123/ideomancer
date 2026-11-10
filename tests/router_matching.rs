//! Offline, compiled-CLI contracts for source-backed router-prefix candidates.
#![cfg(unix)]

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;
use tempfile::{tempdir, TempDir};

const SECRET: &str = "PRIVATE_ROUTER_LITERAL_MUST_NOT_LEAK";
fn route(router: &str, prefix: &str, handler: &str) -> String {
    format!(
        r#"
{router} = APIRouter(prefix='{prefix}')
@{router}.post('/search', description='{SECRET}', name='{SECRET}')
def {handler}(request):
    private_default = '{SECRET}'
    return unknown(request)
"#
    )
}

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
        eprintln!("SKIP router matching: install Python >=3.12 or set IDEOMANCER_TEST_PYTHON");
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

struct Fixture {
    _temp: TempDir,
    cwd: PathBuf,
    root: PathBuf,
    marker: PathBuf,
    python: PathBuf,
}

impl Fixture {
    fn new(files: &[(&str, &str)], paths: &[&str]) -> Option<Self> {
        let python = python()?.to_path_buf();
        let temp = tempdir().unwrap();
        let cwd = fs::canonicalize(temp.path()).unwrap();
        let root = cwd.join("source app");
        fs::create_dir(&root).unwrap();
        let marker = cwd.join("SOURCE_MUST_NOT_EXECUTE");
        for (name, source) in files {
            let path = root.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(
                path,
                format!(
                    "from pathlib import Path\nPath({}).write_text('executed')\nraise RuntimeError('never import fixture')\n{source}",
                    serde_json::to_string(marker.to_str().unwrap()).unwrap()
                ),
            )
            .unwrap();
        }
        let mut operations = serde_json::Map::new();
        for path in paths {
            // No operationId: matching must come from source context, not names.
            operations.insert(
                (*path).into(),
                json!({"post":{"responses":{"200":{"description":"OK"}}}}),
            );
        }
        fs::write(
            cwd.join("fixture.json"),
            json!({"openapi":"3.1.0","info":{"title":"Offline","version":"1"},"paths":operations})
                .to_string(),
        )
        .unwrap();
        Some(Self {
            _temp: temp,
            cwd,
            root,
            marker,
            python,
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
        let saved = success(
            self.command()
                .arg("discover")
                .arg(&self.root)
                .env("IDEOMANCER_PYTHON", &self.python)
                .arg("--out")
                .arg(self.cwd.join("catalog output"))
                .args(["--json", "--kind", "api", "--limit", "100"])
                .output()
                .unwrap(),
        );
        // Exercise catalog-native selection with all execution helpers disabled.
        let listed = success(
            self.command()
                .arg("catalog")
                .arg(saved["catalogDirectory"].as_str().unwrap())
                .args(["--json", "--kind", "api", "--limit", "100"])
                .output()
                .unwrap(),
        );
        assert_eq!(listed["page"]["items"], saved["page"]["items"]);
        saved
    }

    fn diagram(&self, saved: &Value, suffix: &str, view: &str) -> Value {
        let entries: Vec<_> = saved["page"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["symbol"].as_str().unwrap().ends_with(suffix))
            .collect();
        assert_eq!(entries.len(), 1, "entry {suffix}: {saved}");
        let report = success(
            self.command()
                .arg("diagram")
                .arg(saved["catalogDirectory"].as_str().unwrap())
                .args([
                    "--entry",
                    entries[0]["id"].as_str().unwrap(),
                    "--view",
                    view,
                    "--no-render",
                    "--focus-hops",
                    "0",
                    "--openapi",
                ])
                .arg(self.cwd.join("fixture.json"))
                .output()
                .unwrap(),
        );
        assert!(
            report["svg"].is_null() && report["error"].is_null(),
            "{report}"
        );
        let data: Value =
            serde_json::from_str(&fs::read_to_string(report["legend"].as_str().unwrap()).unwrap())
                .unwrap();
        assert_eq!(data["limits"]["hops"], 0);
        assert_eq!(data["sourceModel"], "static-candidate");
        assert_eq!(
            data["counts"]["networkGroups"], 0,
            "router factory is not network traffic: {data}"
        );
        data
    }

    fn safe(&self, saved: &Value) {
        assert!(!self.marker.exists(), "fixture source was executed");
        assert!(
            !self.cwd.join(".ideomancer/analyses").exists(),
            "agent workflow dispatched"
        );
        let mut pending = vec![PathBuf::from(saved["catalogDirectory"].as_str().unwrap())];
        while let Some(parent) = pending.pop() {
            for entry in fs::read_dir(parent).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    pending.push(path);
                } else {
                    assert!(
                        !String::from_utf8_lossy(&fs::read(&path).unwrap()).contains(SECRET),
                        "private non-prefix literal leaked in {path:?}"
                    );
                }
            }
        }
    }
}

fn source_refs<'a>(value: &'a Value, refs: &mut Vec<&'a Value>) {
    match value {
        Value::Object(object) => {
            if let Some(items) = object.get("sourceRefs").and_then(Value::as_array) {
                refs.extend(items);
            }
            for child in object.values() {
                source_refs(child, refs);
            }
        }
        Value::Array(items) => {
            for child in items {
                source_refs(child, refs);
            }
        }
        _ => {}
    }
}

fn candidate(data: &Value, path: &str) {
    let matched = &data["contractMatch"];
    assert_eq!(matched["status"], "candidate", "{data}");
    let candidates = matched["candidates"].as_array().unwrap();
    assert_eq!(candidates.len(), 1, "{matched}");
    assert_eq!(candidates[0]["path"], path, "{matched}");
    assert!(!matched["basis"].as_str().unwrap().is_empty(), "{matched}");
    assert!(
        !data["routerContext"].is_null(),
        "source-backed context missing: {data}"
    );
    let mut refs = Vec::new();
    source_refs(&data["routerContext"], &mut refs);
    assert!(
        !refs.is_empty(),
        "router context has no proof citations: {data}"
    );
    for reference in refs {
        assert!(reference["factId"].is_string(), "{reference}");
        assert!(
            reference["line"].as_u64().is_some_and(|line| line > 0),
            "{reference}"
        );
        let path = Path::new(reference["path"].as_str().unwrap());
        assert!(
            !path.is_absolute() && path.extension().is_some_and(|ext| ext == "py"),
            "{reference}"
        );
    }
    let text = data["routerContext"].to_string().to_lowercase();
    assert!(
        text.contains("candidate"),
        "context must not assert a runtime full path: {text}"
    );
    assert!(
        text.contains("runtime") || text.contains("unverified") || text.contains("unknown"),
        "runtime uncertainty missing: {text}"
    );
}

fn prefix_proof(f: &Fixture, data: &Value) {
    let mut refs = Vec::new();
    source_refs(&data["routerContext"], &mut refs);
    let cited: String = refs
        .iter()
        .map(|reference| {
            let source =
                fs::read_to_string(f.root.join(reference["path"].as_str().unwrap())).unwrap();
            let line = reference["line"].as_u64().unwrap() as usize;
            let end = reference["endLine"].as_u64().unwrap_or(line as u64) as usize;
            source
                .lines()
                .skip(line - 1)
                .take(end - line + 1)
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect::<Vec<_>>()
        .join("\n");
    for proof in ["APIRouter", "prefix=", "/search"] {
        assert!(
            cited.contains(proof),
            "missing {proof} source proof: {cited}\n{data}"
        );
    }
}

#[test]
fn direct_prefixes_disambiguate_shared_search_in_both_zero_hop_views() {
    let source = format!(
        "from fastapi import APIRouter\n{}\n{}",
        route("first", "/items", "search_items"),
        route("second", "/orders", "search_orders")
    );
    let Some(f) = Fixture::new(&[("app.py", &source)], &["/items/search", "/orders/search"]) else {
        return;
    };
    let saved = f.discover();
    for view in ["services", "network"] {
        for (handler, path) in [
            ("search_items", "/items/search"),
            ("search_orders", "/orders/search"),
        ] {
            let data = f.diagram(&saved, &format!(".{handler}"), view);
            candidate(&data, path);
            prefix_proof(&f, &data);
        }
    }
    f.safe(&saved);
}

#[test]
fn imported_fastapi_alias_supplies_prefix_not_handler_name() {
    let import = "from fastapi import APIRouter as Router";
    let routes = format!(
        "{import}\n{}",
        route("router", "/special-items", "unrelated").replace("APIRouter(", "Router(")
    );
    let Some(f) = Fixture::new(
        &[("pkg/__init__.py", ""), ("pkg/routes.py", &routes)],
        &["/special-items/search", "/items/search", "/orders/search"],
    ) else {
        return;
    };
    let saved = f.discover();
    for view in ["services", "network"] {
        let data = f.diagram(&saved, ".unrelated", view);
        candidate(&data, "/special-items/search");
    }
    f.safe(&saved);
}

#[test]
fn proven_prefix_with_no_contract_route_does_not_revert_to_all_suffix_matches() {
    let source = format!(
        "from fastapi import APIRouter\n{}",
        route("router", "/items", "search")
    );
    let Some(f) = Fixture::new(
        &[("app.py", &source)],
        &["/orders/search", "/unrelated/search"],
    ) else {
        return;
    };
    let saved = f.discover();
    for view in ["services", "network"] {
        let data = f.diagram(&saved, ".search", view);
        assert_ne!(data["contractMatch"]["status"], "candidate", "{data}");
        assert!(
            data["contractMatch"]["candidates"]
                .as_array()
                .unwrap()
                .is_empty(),
            "known-empty mapping became suffix fallback: {data}"
        );
        assert!(
            !data["routerContext"].is_null(),
            "proven source context lost: {data}"
        );
    }
    f.safe(&saved);
}

#[test]
fn direct_fastapi_router_and_include_prefix_compose_source_candidate() {
    let source = r#"
import fastapi
router = fastapi.APIRouter(prefix='/v1')
app = fastapi.FastAPI()
app.include_router(router, prefix='/api')

@router.post('/search')
def search(request):
    return unknown(request)
"#;
    let Some(f) = Fixture::new(
        &[("app.py", source)],
        &["/api/v1/search", "/v1/search", "/other/search"],
    ) else {
        return;
    };
    let saved = f.discover();
    for view in ["services", "network"] {
        candidate(&f.diagram(&saved, ".search", view), "/api/v1/search");
    }
    f.safe(&saved);
}

#[test]
fn foreign_dynamic_and_mutated_router_context_never_forces_match() {
    let base = format!(
        "from fastapi import APIRouter\n{}",
        route("router", "/items", "search")
    );
    let cases = [
        (
            "foreign constructor",
            base.replace(
                "from fastapi import APIRouter",
                "from unrelated import APIRouter",
            ),
        ),
        (
            "dynamic prefix",
            base.replace("prefix='/items'", "prefix=runtime_prefix()"),
        ),
        (
            "dynamic kwargs",
            base.replace("prefix='/items'", "**runtime_options()"),
        ),
        (
            "rebound router",
            base.replace("@router.post", "router = replacement\n@router.post"),
        ),
        (
            "mutated prefix",
            base.replace(
                "@router.post",
                "router.prefix = runtime_prefix()\n@router.post",
            ),
        ),
    ];
    for (label, source) in cases {
        let Some(f) = Fixture::new(&[("app.py", &source)], &["/items/search", "/orders/search"])
        else {
            return;
        };
        let saved = f.discover();
        for view in ["services", "network"] {
            let data = f.diagram(&saved, ".search", view);
            assert_ne!(
                data["contractMatch"]["status"], "candidate",
                "{label} forced a match: {data}"
            );
        }
        f.safe(&saved);
    }
}

#[test]
fn multiple_include_prefixes_remain_ambiguous_instead_of_selecting_one_mount() {
    let source = r#"
import fastapi
router = fastapi.APIRouter(prefix='/v1')
app = fastapi.FastAPI()
app.include_router(router, prefix='/api')
app.include_router(router, prefix='/internal')

@router.post('/search')
def search(request):
    return unknown(request)
"#;
    let Some(f) = Fixture::new(
        &[("app.py", source)],
        &["/api/v1/search", "/internal/v1/search"],
    ) else {
        return;
    };
    let saved = f.discover();
    for view in ["services", "network"] {
        let data = f.diagram(&saved, ".search", view);
        assert_ne!(data["contractMatch"]["status"], "candidate", "{data}");
    }
    f.safe(&saved);
}

#[test]
fn direct_prefix_proof_cannot_be_published_outside_the_fact_budget() {
    let source = format!(
        "from fastapi import APIRouter\n{}",
        route("router", "/items", "search")
    );
    let Some(f) = Fixture::new(&[("app.py", &source)], &["/items/search", "/orders/search"]) else {
        return;
    };
    let saved = f.discover();
    let entry = &saved["page"]["items"][0];
    for view in ["services", "network"] {
        let output = f
            .command()
            .arg("diagram")
            .arg(saved["catalogDirectory"].as_str().unwrap())
            .args([
                "--entry",
                entry["id"].as_str().unwrap(),
                "--view",
                view,
                "--focus-hops",
                "0",
                "--focus-facts",
                "1",
                "--no-render",
                "--openapi",
            ])
            .arg(f.cwd.join("fixture.json"))
            .output()
            .unwrap();
        if output.status.success() {
            let report = success(output);
            let data: Value =
                serde_json::from_slice(&fs::read(report["legend"].as_str().unwrap()).unwrap())
                    .unwrap();
            assert_ne!(data["contractMatch"]["status"], "candidate", "{data}");
            assert_ne!(data["routerContext"]["status"], "supported", "{data}");
        } else {
            let error = String::from_utf8_lossy(&output.stderr);
            assert!(
                error.contains("budget") || error.contains("limit"),
                "{error}"
            );
        }
    }
    f.safe(&saved);
}

#[test]
fn saved_router_views_redact_non_prefix_literals_and_never_execute_source() {
    let source = format!(
        "from fastapi import APIRouter\n{}",
        route("router", "/items", "search")
    );
    let Some(f) = Fixture::new(&[("app.py", &source)], &["/items/search", "/orders/search"]) else {
        return;
    };
    let saved = f.discover();
    f.safe(&saved);
    // Source must not be reopened, imported, or evaluated by either saved view.
    fs::remove_dir_all(&f.root).unwrap();
    for view in ["services", "network"] {
        let data = f.diagram(&saved, ".search", view);
        assert!(!data.to_string().contains(SECRET));
        f.safe(&saved);
    }
}
