//! Offline consumer contracts using only synthetic source and trusted test tools.
#![cfg(unix)]

use ideomancer_core::engine::v1::*;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

fn roundtrip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(value: &T) {
    assert_eq!(
        *value,
        serde_json::from_slice::<T>(&serde_json::to_vec(value).unwrap()).unwrap()
    );
    let schema: Value =
        serde_json::from_str(include_str!("../docs/engine-api.schema.json")).unwrap();
    let name = std::any::type_name::<T>().rsplit("::").next().unwrap();
    assert!(
        matches_schema(
            &schema,
            &schema["$defs"][name],
            &serde_json::to_value(value).unwrap()
        ),
        "schema mismatch for {name}"
    );
}

// Validate the subset of JSON Schema used by the checked-in DTO definitions,
// without adding a runtime/schema-generator dependency to the public crate.
fn matches_schema(root: &Value, schema: &Value, value: &Value) -> bool {
    let Some(definition) = schema.as_object() else {
        return false;
    };
    if let Some(reference) = definition.get("$ref") {
        return matches_schema(
            root,
            root.pointer(reference.as_str().unwrap().strip_prefix('#').unwrap())
                .unwrap(),
            value,
        );
    }
    if let Some(variants) = definition.get("oneOf") {
        return variants
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| matches_schema(root, s, value))
            .count()
            == 1;
    }
    if let Some(variants) = definition.get("anyOf") {
        return variants
            .as_array()
            .unwrap()
            .iter()
            .any(|s| matches_schema(root, s, value));
    }
    if definition
        .get("const")
        .is_some_and(|expected| expected != value)
    {
        return false;
    }
    if definition
        .get("enum")
        .is_some_and(|options| !options.as_array().unwrap().contains(value))
    {
        return false;
    }
    if let Some(kind) = definition.get("type") {
        let kinds = kind
            .as_array()
            .cloned()
            .unwrap_or_else(|| vec![kind.clone()]);
        if !kinds.iter().any(|kind| match kind.as_str().unwrap() {
            "object" => value.is_object(),
            "array" => value.is_array(),
            "string" => value.is_string(),
            "boolean" => value.is_boolean(),
            "integer" => value.as_u64().is_some(),
            "null" => value.is_null(),
            _ => panic!("unsupported schema type"),
        }) {
            return false;
        }
    }
    if let Some(fields) = value.as_object() {
        if definition.get("required").is_some_and(|required| {
            required
                .as_array()
                .unwrap()
                .iter()
                .any(|key| !fields.contains_key(key.as_str().unwrap()))
        }) {
            return false;
        }
        for (key, value) in fields {
            if let Some(property) = schema["properties"].get(key) {
                if !matches_schema(root, property, value) {
                    return false;
                }
            } else if schema["additionalProperties"] == false {
                return false;
            }
        }
    }
    if let Some(values) = value.as_array() {
        if let Some(items) = definition.get("items") {
            if !values
                .iter()
                .all(|value| matches_schema(root, items, value))
            {
                return false;
            }
        }
    }
    if let Some(number) = value.as_u64() {
        if definition
            .get("minimum")
            .is_some_and(|min| number < min.as_u64().unwrap())
            || definition
                .get("maximum")
                .is_some_and(|max| number > max.as_u64().unwrap())
        {
            return false;
        }
    }
    if let Some(text) = value.as_str() {
        if definition
            .get("minLength")
            .is_some_and(|min| text.chars().count() < min.as_u64().unwrap() as usize)
        {
            return false;
        }
    }
    true
}

struct Fixture {
    _temp: tempfile::TempDir,
    request: DiscoverRequest,
    tool: PathBuf,
    marker: PathBuf,
}

fn executable(path: &Path, source: &str) {
    fs::write(path, source).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("synthetic-app");
        fs::create_dir(&root).unwrap();
        fs::write(
            root.join("model.py"),
            "raise RuntimeError('target must never run')\nclass Widget: pass\n",
        )
        .unwrap();
        let tool = temp.path().join("trusted-extractor");
        let marker = temp.path().join("unexpected-tool-launch");
        let response = json!({"pythonVersion":"3.12.synthetic","complete":true,"diagnostics":[],"facts":[
            {"id":"class-widget","kind":"class","path":"model.py","symbol":"model.Widget","line":2,"endLine":2,"value":""}
        ]});
        executable(
            &tool,
            &format!("#!/bin/sh\n/bin/cat >/dev/null\nprintf '%s' '{response}'\n"),
        );
        let request = DiscoverRequest {
            source_root: root,
            output_container: temp.path().join("catalogs"),
            toolchain: Toolchain::Python {
                python: tool.clone(),
            },
        };
        Self {
            _temp: temp,
            request,
            tool,
            marker,
        }
    }

    fn disable_tools(&self) {
        executable(
            &self.tool,
            &format!(
                "#!/bin/sh\nprintf launched > '{}'\nexit 91\n",
                self.marker.display()
            ),
        );
    }
}

fn artifacts(root: &Path) -> BTreeMap<PathBuf, (Vec<u8>, SystemTime)> {
    let mut files = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                files.insert(
                    path.clone(),
                    (
                        fs::read(&path).unwrap(),
                        fs::metadata(path).unwrap().modified().unwrap(),
                    ),
                );
            }
        }
    }
    files
}

#[test]
fn requests_reject_capability_smuggling_and_invalid_tool_combinations() {
    let fixture = Fixture::new();
    roundtrip(&fixture.request);
    for field in [
        "agent",
        "model",
        "args",
        "profile",
        "openapi",
        "retry",
        "configuration",
    ] {
        let mut value = serde_json::to_value(&fixture.request).unwrap();
        value[field] = json!("untrusted");
        assert!(
            serde_json::from_value::<DiscoverRequest>(value).is_err(),
            "{field}"
        );
    }
    for value in [
        json!({"language":"python","python":"/trusted/python","node":"/untrusted/node"}),
        json!({"language":"javascript_typescript","node":"/trusted/node","python":"/untrusted/python"}),
        json!({"language":"javascript_typescript","node":"/trusted/node"}),
    ] {
        assert!(serde_json::from_value::<Toolchain>(value).is_err());
    }
    assert!(serde_json::from_value::<BrowseOptions>(json!({"agent":"bad"})).is_err());
    assert!(serde_json::from_value::<ClassSelection>(json!({"kind":"all","args":[]})).is_err());
    assert!(
        serde_json::from_value::<RenderMode>(json!({"kind":"source_only","mmdc":"bad"})).is_err()
    );
    assert!(serde_json::from_value::<RenderMode>(
        json!({"kind":"svg","mmdc":"tool","install":true})
    )
    .is_err());
    assert_eq!(
        serde_json::from_value::<BrowseOptions>(json!({}))
            .unwrap()
            .limit,
        20
    );
    assert_eq!(RenderMode::default(), RenderMode::SourceOnly);
    let js = Toolchain::JavaScriptTypeScript {
        node: "/trusted/node".into(),
        typescript: "/trusted/typescript.js".into(),
    };
    roundtrip(&js);
    assert_eq!(
        serde_json::to_value(js).unwrap()["language"],
        "javascript_typescript"
    );
    let caps = capabilities();
    roundtrip(&caps);
    assert_eq!(caps.schema_version, API_VERSION);
    assert_eq!(
        caps.languages,
        vec![Language::Python, Language::JavaScriptTypeScript]
    );
    let mut target_tool = fixture.request.clone();
    let target_path = fixture.request.source_root.join("untrusted-python");
    executable(&target_path, "#!/bin/sh\nexit 97\n");
    target_tool.toolchain = Toolchain::Python {
        python: target_path,
    };
    assert_eq!(
        discover(&target_tool).unwrap_err().code,
        ErrorCode::InvalidRequest
    );
}

#[test]
fn saved_catalog_operations_need_no_source_extractor_renderer_or_agent() {
    let fixture = Fixture::new();
    let discovered = discover(&fixture.request).unwrap();
    roundtrip(&discovered);
    assert_eq!(discovered.schema_version, API_VERSION);
    let handle = open_catalog(&discovered.catalog_directory).unwrap();
    assert_eq!(handle.source_root(), discovered.source_root);
    assert_eq!(handle.snapshot_id(), discovered.snapshot_id);
    assert_eq!(handle.is_complete(), discovered.complete);
    fs::remove_dir_all(&fixture.request.source_root).unwrap();
    fixture.disable_tools();
    let page = browse(&handle, &BrowseOptions::default()).unwrap();
    roundtrip(&page);
    assert_eq!(page.schema_version, API_VERSION);
    assert_eq!(page.counts.classes, 1);
    assert_eq!(page.items[0].kind, ItemKind::Class);
    let workflows = browse(
        &handle,
        &BrowseOptions {
            kind: Some(ItemKind::Workflow),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(workflows.items.is_empty());
    for selection in [
        ClassSelection::All,
        ClassSelection::Entry {
            id: page.items[0].id.clone(),
        },
        ClassSelection::Page {
            query: None,
            path: None,
            limit: 30,
            offset: 0,
        },
    ] {
        roundtrip(&selection);
        let report = class_diagrams(&handle, &selection, &RenderMode::default()).unwrap();
        roundtrip(&report);
        assert_eq!(report.schema_version, API_VERSION);
        assert_eq!(report.selection, selection);
        assert_eq!(report.coverage.home_classes, 1);
        assert!(report
            .diagrams
            .iter()
            .all(|diagram| diagram.svg.is_none() && diagram.mermaid.is_file()));
        let legend: Value = serde_json::from_slice(&fs::read(&report.legend).unwrap()).unwrap();
        assert_eq!(legend["schemaVersion"], "ideomancer.class-uml.v1");
        assert_eq!(
            serde_json::to_value(&report.coverage).unwrap(),
            legend["coverage"]
        );
        let before = artifacts(handle.directory());
        assert!(
            class_diagrams(&handle, &selection, &RenderMode::default())
                .unwrap()
                .reused
        );
        assert_eq!(before, artifacts(handle.directory()));
    }
    assert!(!fixture.marker.exists());
    let before = artifacts(handle.directory());
    let invalid_renderer = RenderMode::Svg {
        mmdc: fixture.tool.with_file_name("missing-renderer"),
    };
    assert!(class_diagrams(&handle, &ClassSelection::All, &invalid_renderer).is_err());
    assert_eq!(before, artifacts(handle.directory()));
    for limit in [0, 101] {
        assert_eq!(
            browse(
                &handle,
                &BrowseOptions {
                    limit,
                    ..Default::default()
                }
            )
            .unwrap_err()
            .code,
            ErrorCode::InvalidRequest
        );
    }
    for limit in [0, 31] {
        let selection = ClassSelection::Page {
            query: None,
            path: None,
            limit,
            offset: 0,
        };
        assert_eq!(
            class_diagrams(&handle, &selection, &RenderMode::default())
                .unwrap_err()
                .code,
            ErrorCode::InvalidRequest
        );
    }
    assert!(class_diagrams(
        &handle,
        &ClassSelection::Entry {
            id: "model.Widget".into()
        },
        &RenderMode::default()
    )
    .is_err());
    assert_eq!(before, artifacts(handle.directory()));
}

#[test]
fn source_only_ignores_cli_tool_environment() {
    let fixture = Fixture::new();
    fixture.disable_tools();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "saved_catalog_operations_need_no_source_extractor_renderer_or_agent",
            "--nocapture",
        ])
        .env("IDEOMANCER_AGENT", &fixture.tool)
        .env("IDEOMANCER_MMDC", &fixture.tool)
        .env("IDEOMANCER_PYTHON", &fixture.tool)
        .env("IDEOMANCER_NODE", &fixture.tool)
        .env("IDEOMANCER_TYPESCRIPT", &fixture.tool)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!fixture.marker.exists());
}

#[test]
fn executable_path_skips_nonexecutables_without_requiring_executable_compilers() {
    const CHILD: &str = "IDEOMANCER_TEST_EXECUTABLE_PATH_FIXTURE";
    if let Some(base) = std::env::var_os(CHILD) {
        let base = PathBuf::from(base);
        let mut request = DiscoverRequest {
            source_root: base.join("synthetic-app"),
            output_container: base.join("catalogs"),
            toolchain: Toolchain::Python {
                python: "python3.12".into(),
            },
        };
        let python = discover(&request).unwrap();
        let catalog = open_catalog(python.catalog_directory).unwrap();
        assert_eq!(
            browse(&catalog, &BrowseOptions::default())
                .unwrap()
                .counts
                .classes,
            1
        );
        let render = RenderMode::Svg {
            mmdc: "mmdc".into(),
        };
        let report = class_diagrams(&catalog, &ClassSelection::All, &render).unwrap();
        assert!(report
            .diagrams
            .iter()
            .all(|diagram| diagram.svg.as_ref().is_some_and(|path| path.is_file())));

        request.toolchain = Toolchain::JavaScriptTypeScript {
            node: "node".into(),
            typescript: base.join("compiler.js"),
        };
        request.output_container = base.join("js-catalogs");
        assert!(discover(&request).unwrap().complete);

        for (name, toolchain) in [
            (
                "python3.12",
                Toolchain::Python {
                    python: base.join("A/python3.12"),
                },
            ),
            (
                "node",
                Toolchain::JavaScriptTypeScript {
                    node: base.join("A/node"),
                    typescript: base.join("compiler.js"),
                },
            ),
        ] {
            request.toolchain = toolchain;
            let error = discover(&request).unwrap_err();
            assert_eq!(error.code, ErrorCode::InvalidRequest);
            assert!(
                error.message.contains(name) && error.message.contains("executable"),
                "{error}"
            );
        }
        let before = artifacts(catalog.directory());
        for path in [base.join("A/mmdc"), base.join("A")] {
            let error = class_diagrams(
                &catalog,
                &ClassSelection::All,
                &RenderMode::Svg { mmdc: path },
            )
            .unwrap_err();
            assert_eq!(error.code, ErrorCode::InvalidRequest);
            assert!(error.message.contains("executable"), "{error}");
        }
        assert_eq!(before, artifacts(catalog.directory()));
        assert!(!base.join("source-executed").exists());
        return;
    }

    let fixture = Fixture::new();
    let base = fixture._temp.path();
    let first = base.join("A");
    let second = base.join("B");
    fs::create_dir(&first).unwrap();
    fs::create_dir(&second).unwrap();
    for name in ["python3.12", "node", "mmdc"] {
        let path = first.join(name);
        fs::write(&path, "not executable\n").unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o644)).unwrap();
    }
    std::os::unix::fs::symlink(&fixture.tool, second.join("python3.12")).unwrap();
    fs::write(
        fixture.request.source_root.join("model.py"),
        format!(
            "open({:?}, 'w').write('executed')\nclass Widget: pass\n",
            base.join("source-executed")
        ),
    )
    .unwrap();
    fs::write(
        fixture.request.source_root.join("model.js"),
        "throw new Error('source must not execute');\n",
    )
    .unwrap();
    let compiler = base.join("compiler.js");
    let compiler_bytes = b"// trusted synthetic compiler artifact, not an executable\n";
    fs::write(&compiler, compiler_bytes).unwrap();
    fs::set_permissions(&compiler, fs::Permissions::from_mode(0o644)).unwrap();
    let response = json!({
        "protocolVersion":"ideomancer.js-extractor.v1", "nodeVersion":"synthetic-node",
        "typescriptVersion":"synthetic-ts", "typescriptSha256":format!("{:x}", Sha256::digest(compiler_bytes)),
        "facts":[], "diagnostics":[], "complete":true
    });
    executable(
        &second.join("node"),
        &format!("#!/bin/sh\n/bin/cat >/dev/null\nprintf '%s' '{response}'\n"),
    );
    executable(
        &second.join("mmdc"),
        "#!/bin/sh\nprintf '<svg width=\"10\" height=\"10\"></svg>' > \"$4\"\n",
    );
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "executable_path_skips_nonexecutables_without_requiring_executable_compilers",
            "--nocapture",
        ])
        .env(CHILD, base)
        .env("PATH", std::env::join_paths([first, second]).unwrap())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!base.join("source-executed").exists());
}

#[test]
fn stale_snapshot_and_source_identity_reject_without_publication() {
    let fixture = Fixture::new();
    let discovered = discover(&fixture.request).unwrap();
    let stale = open_catalog(&discovered.catalog_directory).unwrap();
    class_diagrams(&stale, &ClassSelection::All, &RenderMode::default()).unwrap();
    fs::write(
        fixture.request.source_root.join("extra.py"),
        "# changed snapshot\n",
    )
    .unwrap();
    let updated = discover(&fixture.request).unwrap();
    assert_ne!(stale.snapshot_id(), updated.snapshot_id);
    let current = open_catalog(&updated.catalog_directory).unwrap();
    class_diagrams(&current, &ClassSelection::All, &RenderMode::default()).unwrap();
    let before = artifacts(current.directory());
    let err = class_diagrams(&stale, &ClassSelection::All, &RenderMode::default()).unwrap_err();
    assert_eq!(err.code, ErrorCode::ClassDiagramFailed);
    assert!(err.message.contains("stale"));
    roundtrip(&err);
    assert_eq!(
        browse(&stale, &BrowseOptions::default()).unwrap_err().code,
        ErrorCode::CatalogUnavailable
    );
    assert_eq!(before, artifacts(current.directory()));

    // Keep snapshot/evidence intact but change the verified catalog root. This
    // independently exercises the source identity guard, not only snapshot IDs.
    fs::remove_dir_all(current.directory().join("views")).unwrap();
    let catalog_path = current.directory().join("catalog.json");
    let receipt_path = current.directory().join("receipt.json");
    let mut catalog: Value = serde_json::from_slice(&fs::read(&catalog_path).unwrap()).unwrap();
    catalog["sourceRoot"] = json!(fixture.request.source_root.with_file_name("other-source"));
    let mut bytes = serde_json::to_vec_pretty(&catalog).unwrap();
    bytes.push(b'\n');
    fs::write(catalog_path, &bytes).unwrap();
    let mut receipt: Value = serde_json::from_slice(&fs::read(&receipt_path).unwrap()).unwrap();
    receipt["catalogSha256"] = json!(format!("{:x}", Sha256::digest(bytes)));
    fs::write(receipt_path, serde_json::to_vec(&receipt).unwrap()).unwrap();
    let replacement = open_catalog(current.directory()).unwrap();
    assert_eq!(replacement.snapshot_id(), current.snapshot_id());
    assert_ne!(replacement.source_root(), current.source_root());
    let before = artifacts(current.directory());
    assert!(
        class_diagrams(&current, &ClassSelection::All, &RenderMode::default())
            .unwrap_err()
            .message
            .contains("stale")
    );
    assert!(browse(&current, &BrowseOptions::default())
        .unwrap_err()
        .message
        .contains("stale"));
    assert_eq!(before, artifacts(current.directory()));
}

#[test]
fn schema_declares_the_consumer_contract() {
    let schema: Value =
        serde_json::from_str(include_str!("../docs/engine-api.schema.json")).unwrap();
    assert_eq!(schema["$defs"]["ApiVersion"]["const"], API_VERSION);
    for name in [
        "Capabilities",
        "DiscoverRequest",
        "DiscoveryReport",
        "BrowseOptions",
        "BrowseReport",
        "ClassSelection",
        "RenderMode",
        "ClassDiagramReport",
        "EngineError",
    ] {
        assert!(schema["$defs"][name].is_object(), "missing {name}");
    }
    assert_eq!(
        schema["$defs"]["DiscoverRequest"]["additionalProperties"],
        false
    );
    assert_eq!(
        schema["$defs"]["ClassCoverage"]["properties"]["relationsRepresentedAsFieldsVisible"]
            ["$ref"],
        "#/$defs/Count"
    );
}
