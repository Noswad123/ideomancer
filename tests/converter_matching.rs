//! Offline converter-projection contracts exercised only through the compiled CLI.
#![cfg(unix)]

use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;
use tempfile::{tempdir, TempDir};

const SECRET: &str = "PRIVATE_REGISTERED_CONVERTER_NAME_MUST_NOT_LEAK";

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
        eprintln!("SKIP converter matching: install Python >=3.12 or set IDEOMANCER_TEST_PYTHON");
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

fn artifacts(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut result = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(parent) = pending.pop() {
        for entry in fs::read_dir(parent).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                result.insert(path.clone(), fs::read(path).unwrap());
            }
        }
    }
    result
}

struct Fixture {
    _temp: TempDir,
    cwd: PathBuf,
    saved: Value,
    original: BTreeMap<PathBuf, Vec<u8>>,
}

impl Fixture {
    fn new(source: &str, paths: &[&str], operation_id: Option<&str>) -> Option<Self> {
        let python = python()?;
        let temp = tempdir().unwrap();
        let cwd = fs::canonicalize(temp.path()).unwrap();
        let root = cwd.join("source app");
        fs::create_dir(&root).unwrap();
        fs::write(
            root.join("app.py"),
            format!(
                "from pathlib import Path\nPath({}).write_text('executed')\nraise RuntimeError('never import fixture')\n{source}",
                serde_json::to_string(cwd.join("SOURCE_MUST_NOT_EXECUTE").to_str().unwrap()).unwrap()
            ),
        )
        .unwrap();
        let mut operations = serde_json::Map::new();
        for path in paths {
            let mut operation = json!({"responses":{"200":{"description":"OK"}}});
            if let Some(id) = operation_id {
                operation["operationId"] = json!(id);
            }
            operations.insert((*path).into(), json!({"get":operation}));
        }
        fs::write(
            cwd.join("fixture.json"),
            json!({"openapi":"3.1.0","info":{"title":"Offline","version":"1"},"paths":operations})
                .to_string(),
        )
        .unwrap();
        let mut fixture = Self {
            _temp: temp,
            cwd,
            saved: Value::Null,
            original: BTreeMap::new(),
        };
        fixture.saved = success(
            fixture
                .command()
                .arg("discover")
                .arg(&root)
                .env("IDEOMANCER_PYTHON", python)
                .arg("--out")
                .arg(fixture.cwd.join("catalog output"))
                .args(["--json", "--kind", "api", "--limit", "100"])
                .output()
                .unwrap(),
        );
        fixture.original = artifacts(fixture.catalog());
        // Saved native views must not reopen, import or execute the application.
        fs::remove_dir_all(root).unwrap();
        fixture.safe();
        Some(fixture)
    }

    fn catalog(&self) -> &Path {
        Path::new(self.saved["catalogDirectory"].as_str().unwrap())
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

    fn diagram(&self, symbol: &str, literal: &str, view: &str) -> Value {
        let entries: Vec<_> = self.saved["page"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|entry| entry["symbol"].as_str().unwrap().ends_with(symbol))
            .collect();
        assert_eq!(entries.len(), 1, "{symbol}: {}", self.saved);
        assert_eq!(entries[0]["routePath"], literal);
        let report = success(
            self.command()
                .arg("diagram")
                .arg(self.catalog())
                .args([
                    "--entry",
                    entries[0]["id"].as_str().unwrap(),
                    "--view",
                    view,
                    "--no-render",
                    "--focus-hops",
                    "0",
                    "--focus-facts",
                    "600",
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
            serde_json::from_slice(&fs::read(report["legend"].as_str().unwrap()).unwrap()).unwrap();
        assert_eq!(data["item"]["routePath"], literal, "{data}");
        assert_eq!(data["sourceModel"], "static-candidate");
        assert_eq!(data["limits"]["hops"], 0);
        self.safe();
        data
    }

    fn safe(&self) {
        assert!(!self.cwd.join("SOURCE_MUST_NOT_EXECUTE").exists());
        assert!(!self.cwd.join(".ideomancer/analyses").exists());
        for (path, bytes) in &self.original {
            assert_eq!(
                &fs::read(path).unwrap(),
                bytes,
                "saved evidence changed: {path:?}"
            );
        }
        for (path, bytes) in artifacts(self.catalog()) {
            assert!(
                !String::from_utf8_lossy(&bytes).contains(SECRET),
                "registration literal leaked: {path:?}"
            );
        }
    }
}

fn source(route: &str, prelude: &str) -> String {
    format!(
        "import fastapi\n{prelude}\nrouter = fastapi.APIRouter(prefix='/items')\n\n@router.get('{route}')\ndef endpoint():\n    return unknown()\n"
    )
}

fn paths(data: &Value) -> Vec<&str> {
    data["contractMatch"]["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|candidate| candidate["path"].as_str().unwrap())
        .collect()
}

fn context(data: &Value, composed: &str) {
    assert_eq!(data["routerContext"]["status"], "supported", "{data}");
    let candidates = data["routerContext"]["candidates"].as_array().unwrap();
    assert_eq!(candidates.len(), 1, "{data}");
    assert_eq!(candidates[0]["composedPath"], composed, "{data}");
    assert_eq!(candidates[0]["fullPathProven"], false, "{data}");
    assert!(!data["routerContext"]["sourceRefs"]
        .as_array()
        .unwrap()
        .is_empty());
}

fn has_projection_metadata(value: &Value) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(key, child)| {
            (key.to_lowercase().contains("project") && !child.is_null())
                || has_projection_metadata(child)
        }),
        Value::Array(items) => items.iter().any(has_projection_metadata),
        _ => false,
    }
}

fn projected(data: &Value, composed: &str, exported: &str) {
    context(data, composed);
    assert_eq!(data["contractMatch"]["status"], "candidate", "{data}");
    assert_eq!(paths(data), [exported], "{data}");
    let association = &data["contractMatch"];
    assert!(
        association["basis"]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains("project"),
        "{data}"
    );
    // Projection is separate metadata, never a rewrite of the catalog or context.
    assert!(
        has_projection_metadata(association),
        "projection metadata absent: {data}"
    );
    assert!(
        association.to_string().contains(composed),
        "original projection evidence absent: {data}"
    );
}

#[test]
fn builtin_converters_project_without_rewriting_literal_routes_in_both_views() {
    let routes = [
        ("/by-name/{name:path}", "/items/by-name/{name}"),
        ("/by-name/{name:str}", "/items/by-name/{name}"),
        ("/by-id/{item_id:int}", "/items/by-id/{item_id}"),
        ("/by-score/{score:float}", "/items/by-score/{score}"),
        ("/by-uuid/{item_uuid:uuid}", "/items/by-uuid/{item_uuid}"),
        (
            "/{name:str}/{tail:path}/{count:int}/{score:float}/{key:uuid}",
            "/items/{name}/{tail}/{count}/{score}/{key}",
        ),
    ];
    for (literal, exported) in routes {
        let Some(f) = Fixture::new(&source(literal, ""), &[exported], None) else {
            return;
        };
        for view in ["network", "services"] {
            projected(
                &f.diagram(".endpoint", literal, view),
                &format!("/items{literal}"),
                exported,
            );
        }
    }
}

#[test]
fn outer_prefix_is_only_a_suffix_candidate_not_a_verified_full_path() {
    let literal = "/by-name/{name:path}";
    let Some(f) = Fixture::new(&source(literal, ""), &["/api/items/by-name/{name}"], None) else {
        return;
    };
    for view in ["network", "services"] {
        let data = f.diagram(".endpoint", literal, view);
        projected(
            &data,
            "/items/by-name/{name:path}",
            "/api/items/by-name/{name}",
        );
        let boundary = format!(
            "{} {}",
            data["contractMatch"]["basis"], data["contractMatch"]["boundary"]
        )
        .to_lowercase();
        assert!(boundary.contains("suffix"), "{data}");
        assert!(
            boundary.contains("unknown")
                || boundary.contains("unverified")
                || boundary.contains("not runtime"),
            "{data}"
        );
    }
}

#[test]
fn custom_mixed_and_malformed_converters_never_partially_project() {
    for (literal, exported) in [
        ("/by-name/{name:slug}", "/items/by-name/{name}"),
        ("/{count:int}/{name:slug}", "/items/{count}/{name:slug}"),
        ("/{count:int}/{name:slug}", "/items/{count}/{name}"),
        ("/by-name/{name:path", "/items/by-name/{name}"),
        ("/by-name/{{name:path}}", "/items/by-name/{name}"),
        ("/by-name/{name:path}}", "/items/by-name/{name}"),
        ("/by-name/{name:path:extra}", "/items/by-name/{name}"),
    ] {
        let Some(f) = Fixture::new(&source(literal, ""), &[exported], None) else {
            return;
        };
        for view in ["network", "services"] {
            let data = f.diagram(".endpoint", literal, view);
            assert!(
                paths(&data).is_empty(),
                "unsupported route projected: {data}"
            );
        }
    }
}

#[test]
fn absent_or_fake_router_context_cannot_force_a_normalized_match() {
    let literal = "/by-name/{name:path}";
    for setup in [
        "router = unknown_router()",
        "from foreign_framework import APIRouter\nrouter = APIRouter(prefix='/items')",
        "class APIRouter:\n    pass\nrouter = APIRouter(prefix='/items')",
    ] {
        let source =
            format!("{setup}\n@router.get('{literal}')\ndef endpoint():\n    return unknown()\n");
        let Some(f) = Fixture::new(&source, &["/items/by-name/{name}"], None) else {
            return;
        };
        for view in ["network", "services"] {
            let data = f.diagram(".endpoint", literal, view);
            assert_ne!(data["routerContext"]["status"], "supported", "{data}");
            assert!(
                paths(&data).is_empty(),
                "unsupported context forced projection: {data}"
            );
        }
    }
}

#[test]
fn trailing_slashes_and_parameter_names_are_not_normalized() {
    for (literal, exported) in [
        ("/by-name/{name:path}", "/items/by-name/{name}/"),
        ("/by-name/{name:path}/", "/items/by-name/{name}"),
        ("/by-name/{name:path}", "/items/by-name/{other}"),
    ] {
        let Some(f) = Fixture::new(&source(literal, ""), &[exported], None) else {
            return;
        };
        for view in ["network", "services"] {
            let data = f.diagram(".endpoint", literal, view);
            context(&data, &format!("/items{literal}"));
            assert!(paths(&data).is_empty(), "route identity normalized: {data}");
        }
    }
}

#[test]
fn literal_and_projected_operations_are_both_kept_as_ambiguous() {
    let literal = "/by-name/{name:path}";
    let operations = ["/items/by-name/{name:path}", "/items/by-name/{name}"];
    let Some(f) = Fixture::new(&source(literal, ""), &operations, None) else {
        return;
    };
    for view in ["network", "services"] {
        let data = f.diagram(".endpoint", literal, view);
        context(&data, operations[0]);
        assert_eq!(data["contractMatch"]["status"], "ambiguous", "{data}");
        let mut actual = paths(&data);
        actual.sort_unstable();
        let mut expected = operations;
        expected.sort_unstable();
        assert_eq!(actual, expected, "{data}");
        assert!(
            data["contractMatch"]["basis"]
                .as_str()
                .unwrap()
                .to_lowercase()
                .contains("project"),
            "{data}"
        );
        assert!(has_projection_metadata(&data["contractMatch"]), "{data}");
    }
}

#[test]
fn supported_context_contradiction_cannot_fall_back_to_operation_id() {
    let literal = "/by-name/{name:path}";
    let Some(f) = Fixture::new(
        &source(literal, ""),
        &["/elsewhere/by-name/{name}"],
        Some("endpoint"),
    ) else {
        return;
    };
    for view in ["network", "services"] {
        let data = f.diagram(".endpoint", literal, view);
        context(&data, "/items/by-name/{name:path}");
        assert_eq!(
            data["contractMatch"]["status"], "context-mismatch",
            "{data}"
        );
        assert!(
            paths(&data).is_empty(),
            "operationId fallback defeated context: {data}"
        );
    }
}

#[test]
fn observed_converter_registration_blocks_projection_but_allows_literal_match() {
    let guards = [
        "from starlette.convertors import register_url_convertor as reg\nreg('path', object())".to_owned(),
        "import starlette.convertors as converters\nconverters.register_url_convertor('path', object())".to_owned(),
        "from starlette.convertors import register_url_convertor as reg\nregistration = reg('path', object())".to_owned(),
        "from starlette.convertors import register_url_convertor as reg\ninstall = reg\nregistration = install('path', object())".to_owned(),
        format!("from starlette.convertors import register_url_convertor as reg\nreg('{SECRET}', object())"),
    ];
    let literal = "/by-name/{name:path}";
    for guard in guards {
        for literal_contract in [false, true] {
            let exported = if literal_contract {
                "/items/by-name/{name:path}"
            } else {
                "/items/by-name/{name}"
            };
            let Some(f) = Fixture::new(&source(literal, &guard), &[exported], None) else {
                return;
            };
            for view in ["network", "services"] {
                let data = f.diagram(".endpoint", literal, view);
                if literal_contract {
                    context(&data, exported);
                    assert_eq!(
                        data["contractMatch"]["status"], "candidate",
                        "{guard}: {data}"
                    );
                    assert_eq!(paths(&data), [exported], "{guard}: {data}");
                } else {
                    assert!(
                        paths(&data).is_empty(),
                        "registration ignored: {guard}: {data}"
                    );
                }
                assert!(
                    !data.to_string().contains(SECRET),
                    "private registration name leaked"
                );
            }
        }
    }
}
