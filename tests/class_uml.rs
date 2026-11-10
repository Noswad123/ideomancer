//! Offline public CLI contracts for complete and independently selected class UML.
#![cfg(unix)]

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;
use std::time::SystemTime;
use tempfile::{tempdir, TempDir};

fn python() -> Option<&'static Path> {
    static PYTHON: OnceLock<Option<PathBuf>> = OnceLock::new();
    PYTHON
        .get_or_init(|| {
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
            Some(PathBuf::from(String::from_utf8(output.stdout).ok()?.trim()))
        })
        .as_deref()
}

fn report(output: Output) -> Value {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn json_file(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
fn text(r: &Value, key: &str) -> String {
    fs::read_to_string(r[key].as_str().unwrap()).unwrap()
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

// Snapshot links themselves, never their targets, for nondestructive rejection checks.
fn artifacts(root: &Path) -> BTreeMap<PathBuf, (Vec<u8>, SystemTime)> {
    let mut out = BTreeMap::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        let meta = fs::symlink_metadata(&path).unwrap();
        if meta.is_dir() {
            for (name, value) in artifacts(&path) {
                out.insert(path.strip_prefix(root).unwrap().join(name), value);
            }
        } else {
            let bytes = if meta.file_type().is_symlink() {
                fs::read_link(&path)
                    .unwrap()
                    .as_os_str()
                    .as_encoded_bytes()
                    .to_vec()
            } else {
                fs::read(&path).unwrap()
            };
            out.insert(
                path.strip_prefix(root).unwrap().to_owned(),
                (bytes, meta.modified().unwrap()),
            );
        }
    }
    out
}

struct Fixture {
    _temp: TempDir,
    cwd: PathBuf,
    root: PathBuf,
    forbidden: PathBuf,
    renderer: PathBuf,
}
impl Fixture {
    fn new() -> Option<Self> {
        if python().is_none() {
            eprintln!("SKIP class_uml: trusted Python >=3.12 required");
            return None;
        }
        let temp = tempdir().unwrap();
        let cwd = fs::canonicalize(temp.path()).unwrap();
        let root = cwd.join("synthetic app <img onerror=alert(1)>");
        fs::create_dir(&root).unwrap();
        let forbidden = cwd.join("forbidden tool ; literal");
        fs::write(&forbidden, "#!/bin/sh\n: > \"$UML_TOOL_MARKER\"\nexit 97\n").unwrap();
        let renderer = cwd.join("stub renderer ; literal");
        fs::write(&renderer, r##"#!/bin/sh
set -eu
output=
while [ "$#" -gt 0 ]; do
    if [ "$1" = '-o' ]; then shift; output=$1; fi
    shift
done
printf 'render\n' >> "$UML_RENDERS"
count=$(wc -l < "$UML_RENDERS" | tr -d ' ')
if [ "$count" = "${UML_FAIL_AT:-0}" ]; then exit 41; fi
printf '<svg xmlns="http://www.w3.org/2000/svg" width="300" height="150"><rect width="300" height="150" fill="blue"/></svg>' > "$output"
if [ "${UML_LARGE_SVG:-0}" = 1 ]; then dd if=/dev/zero bs=1048576 count=9 >> "$output" 2>/dev/null; fi
if [ "${UML_TOTAL_SVG:-0}" = 1 ]; then dd if=/dev/zero bs=1048576 count=7 >> "$output" 2>/dev/null; fi
"##).unwrap();
        for path in [&forbidden, &renderer] {
            fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let f = Self {
            _temp: temp,
            cwd,
            root,
            forbidden,
            renderer,
        };
        f.write("app.py", &format!("from pathlib import Path\nPath({}).write_text('executed')\nraise RuntimeError('must never run')\n", json!(f.cwd.join("source.executed"))));
        // Retain the original 88-class crowded/hostile-folder fixture.
        for n in 0..88 {
            let folder = if n < 31 {
                "crowded".to_owned()
            } else if (n - 31) % 30 == 0 {
                "folder <img src=x onerror=alert(1)> & \"quote\"".to_owned()
            } else {
                format!("folder{:02}", (n - 31) % 30)
            };
            let mut source = format!(
                "class Class{n:03}{}:\n",
                if n == 0 { "(ExternalBoundary)" } else { "" }
            );
            for m in 0..if n == 0 { 35 } else { 2 } {
                source.push_str(&format!(
                    "    def method{m:02}(self):\n        self.field{m:02} = None\n"
                ));
            }
            f.write(&format!("{folder}/class{n:03}.py"), &source);
        }
        f.write("openapi.json", "not JSON");
        f.write("swagger.yaml", "malformed: [");
        Some(f)
    }
    fn write(&self, name: &str, source: &str) {
        let path = self.root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, source).unwrap();
    }
    fn command(&self) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_ideomancer"));
        c.current_dir(&self.cwd)
            .env("IDEOMANCER_PYTHON", &self.forbidden)
            .env("IDEOMANCER_NODE", &self.forbidden)
            .env("IDEOMANCER_TYPESCRIPT", &self.forbidden)
            .env("IDEOMANCER_AGENT", &self.forbidden)
            .env("IDEOMANCER_MMDC", &self.forbidden)
            .env("UML_TOOL_MARKER", self.cwd.join("tool.executed"))
            .env("UML_RENDERS", self.cwd.join("renders.txt"))
            .env_remove("IDEOMANCER_MODEL")
            .env_remove("NODE_OPTIONS")
            .env_remove("NODE_PATH");
        c
    }
    fn discover(&self) -> Output {
        self.command()
            .arg("discover")
            .arg(&self.root)
            .arg("--out")
            .arg(self.cwd.join("catalog output"))
            .arg("--python")
            .arg(python().unwrap())
            .args(["--json", "--kind", "class", "--limit", "100"])
            .output()
            .unwrap()
    }
    fn saved(&self) -> PathBuf {
        let r = report(self.discover());
        assert_eq!(r["page"]["counts"]["class"], 88);
        self.safe();
        PathBuf::from(r["catalogDirectory"].as_str().unwrap())
    }
    fn diagram(&self, catalog: &Path, args: &[&str]) -> Command {
        let mut c = self.command();
        c.arg("diagram")
            .arg(catalog)
            .args(["--kind", "class"])
            .args(args);
        c
    }
    fn complete(&self, catalog: &Path) -> Value {
        report(self.diagram(catalog, &["--no-render"]).output().unwrap())
    }
    fn safe(&self) {
        for name in ["source.executed", "tool.executed", ".ideomancer/analyses"] {
            assert!(!self.cwd.join(name).exists(), "unexpected {name}");
        }
    }
}

fn classes(catalog: &Path) -> Vec<Value> {
    json_file(&catalog.join("catalog.json"))["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["kind"] == "class")
        .cloned()
        .collect()
}

fn javascript_fixture(files: &[(&str, &str)]) -> Option<(Fixture, PathBuf)> {
    let (Some(node), Some(compiler)) = (
        std::env::var_os("IDEOMANCER_TEST_NODE"),
        std::env::var_os("IDEOMANCER_TEST_TYPESCRIPT"),
    ) else {
        eprintln!("SKIP class_uml JavaScript: explicit trusted Node and TypeScript required");
        return None;
    };
    for tool in [&node, &compiler] {
        assert!(Path::new(tool).is_absolute() && Path::new(tool).is_file());
    }
    let f = Fixture::new()?;
    fs::remove_dir_all(&f.root).unwrap();
    fs::create_dir(&f.root).unwrap();
    for (path, source) in files {
        f.write(path, source);
    }
    let saved = report(
        f.command()
            .arg("discover")
            .arg(&f.root)
            .args([
                "--lang", "js", "--json", "--kind", "class", "--limit", "100",
            ])
            .arg("--out")
            .arg(f.cwd.join("catalog output"))
            .arg("--node")
            .arg(node)
            .arg("--typescript")
            .arg(compiler)
            .output()
            .unwrap(),
    );
    let catalog = PathBuf::from(saved["catalogDirectory"].as_str().unwrap());
    let evidence = json_file(&catalog.join("evidence.json"));
    assert_eq!(
        evidence["extractorVersion"],
        "javascript-typescript-static-6-generic-facts"
    );
    assert_eq!(evidence["complete"], true);
    assert!(
        evidence["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["code"] == "javascript_unsupported_execution_boundary"),
        "{}",
        evidence["diagnostics"]
    );
    fs::remove_dir_all(&f.root).unwrap();
    f.safe();
    Some((f, catalog))
}

fn class_entry<'a>(legend: &'a Value, name: &str) -> &'a Value {
    let suffix = format!(".{name}");
    let matches: Vec<_> = legend["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["symbol"].as_str().unwrap().ends_with(&suffix))
        .collect();
    assert_eq!(matches.len(), 1, "{name}: {legend}");
    matches[0]
}

fn class_relation<'a>(legend: &'a Value, name: &str, kind: &str) -> &'a Value {
    let entry = class_entry(legend, name);
    let matches: Vec<_> = legend["relationships"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["classId"] == entry["id"] && r["relationship"] == kind)
        .collect();
    assert_eq!(matches.len(), 1, "{name} {kind}: {legend}");
    matches[0]
}

fn all_mermaid(report: &Value) -> String {
    report["diagrams"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| text(d, "mermaid"))
        .collect()
}

fn check_bundle(r: &Value, expected: &[Value]) -> Value {
    assert!(r["mermaid"].is_null() && r["svg"].is_null() && r["error"].is_null());
    let view = Path::new(r["viewDirectory"].as_str().unwrap());
    let legend = json_file(&view.join("legend.json"));
    let receipt = json_file(&view.join("receipt.json"));
    assert_eq!(receipt["schemaVersion"], "source-views.v2-class-uml");
    assert_eq!(receipt["policyVersion"], legend["schemaVersion"]);
    assert_eq!(
        receipt["legendHash"],
        hash(&fs::read(view.join("legend.json")).unwrap())
    );
    assert_eq!(
        receipt["htmlHash"],
        hash(&fs::read(view.join("index.html")).unwrap())
    );
    let diagrams = r["diagrams"].as_array().unwrap();
    assert!(!diagrams.is_empty() && diagrams.len() <= 128);
    assert_eq!(diagrams.len(), legend["groups"].as_array().unwrap().len());
    let entries = legend["entries"].as_array().unwrap();
    let expected: BTreeSet<_> = expected.iter().map(|e| e["id"].as_str().unwrap()).collect();
    assert_eq!(
        entries
            .iter()
            .map(|e| e["id"].as_str().unwrap())
            .collect::<BTreeSet<_>>(),
        expected
    );
    let mut homes = BTreeSet::new();
    for (n, ((d, g), saved)) in diagrams
        .iter()
        .zip(legend["groups"].as_array().unwrap())
        .zip(receipt["diagrams"].as_array().unwrap())
        .enumerate()
    {
        let id = format!("group-{:04}", n + 1);
        assert_eq!(d["id"], id);
        assert_eq!(g["id"], id);
        assert_eq!(saved["id"], id);
        assert_eq!(
            Path::new(d["mermaid"].as_str().unwrap()),
            view.join(format!("{id}.mmd"))
        );
        let mmd = text(d, "mermaid");
        assert!(mmd.starts_with("classDiagram\n") && !mmd.contains("flowchart"));
        assert_eq!(saved["mmdSha"], hash(mmd.as_bytes()));
        let ids = g["homeClassIds"].as_array().unwrap();
        assert!(ids.len() <= 12);
        assert!(g["contextClassIds"].as_array().unwrap().len() <= 8);
        assert!(g["boundaryIds"].as_array().unwrap().len() <= 24);
        for id in ids {
            assert!(homes.insert(id.as_str().unwrap()), "duplicate home");
            let e = entries.iter().find(|e| e["id"] == *id).unwrap();
            assert!(mmd.contains(&format!("class {}[", e["diagramId"].as_str().unwrap())));
        }
        if let Some(svg) = d["svg"].as_str() {
            assert_eq!(saved["svgSha"], hash(&fs::read(svg).unwrap()));
        }
    }
    assert_eq!(homes, expected);
    let actual: BTreeSet<_> = fs::read_dir(view)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    let manifest = receipt["artifacts"].as_array().unwrap();
    assert_eq!(manifest.len(), actual.len());
    assert_eq!(
        manifest
            .iter()
            .map(|s| s.as_str().unwrap().to_owned())
            .collect::<BTreeSet<_>>(),
        actual
    );
    legend
}

#[test]
fn default_has_all_88_actual_class_boxes_and_uncapped_member_metadata_offline() {
    let Some(f) = Fixture::new() else { return };
    let catalog = f.saved();
    let expected = classes(&catalog);
    fs::remove_dir_all(&f.root).unwrap();
    let r = f.complete(&catalog);
    assert_eq!(r["view"], "class");
    assert!(r["page"].is_null());
    assert_eq!(
        Path::new(r["viewDirectory"].as_str().unwrap()),
        catalog.join("views/class")
    );
    let legend = check_bundle(&r, &expected);
    assert!(r["diagrams"].as_array().unwrap().len() >= 8);
    assert_eq!(legend["coverage"]["homeClasses"], 88);
    assert_eq!(legend["coverage"]["allSelectedClassesRepresented"], true);
    let html = text(&r, "viewer");
    assert_eq!(html.matches("<script>").count(), 1);
    assert!(html.contains("Content-Security-Policy") && html.contains("sha256-"));
    assert!(!html.contains("<img src=x") && !html.contains("class-overview"));
    assert_eq!(html.matches("data-class-id=").count(), 88);
    for e in legend["entries"].as_array().unwrap() {
        let count = if e["symbol"].as_str().unwrap().ends_with(".Class000") {
            35
        } else {
            2
        };
        for (key, prefix) in [("methods", "method"), ("attributes", "field")] {
            let members = e[key].as_array().unwrap();
            assert_eq!(members.len(), count);
            assert_eq!(
                members
                    .iter()
                    .map(|m| m["name"].as_str().unwrap().to_owned())
                    .collect::<BTreeSet<_>>(),
                (0..count).map(|n| format!("{prefix}{n:02}")).collect()
            );
            for m in members {
                assert!(!m["sourceRefs"].as_array().unwrap().is_empty());
                assert!(html.contains(m["name"].as_str().unwrap()));
            }
        }
    }
    let all_mmd = r["diagrams"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| text(d, "mermaid"))
        .collect::<String>();
    assert!(all_mmd.contains("23 more methods / 27 more fields in details"));
    assert!(!all_mmd.contains("method34()") && !all_mmd.contains("    field34"));
    assert!(html.contains("method34") && html.contains("field34"));
    assert!(!catalog.join("views/class-overview").exists());
    let before = artifacts(&catalog);
    f.complete(&catalog);
    assert_eq!(artifacts(&catalog), before);
    f.safe();
}

#[test]
fn explicit_selectors_only_publish_focus_and_match_catalog_pagination() {
    for mode in ["entry", "query", "path", "limit", "offset"] {
        let Some(f) = Fixture::new() else { return };
        let catalog = f.saved();
        let items = classes(&catalog);
        let selected = items
            .iter()
            .find(|i| i["symbol"].as_str().unwrap().ends_with(".Class087"))
            .unwrap();
        let args = match mode {
            "entry" => vec!["--entry", selected["id"].as_str().unwrap(), "--no-render"],
            "query" => vec!["--query", "Class087", "--no-render"],
            "path" => vec!["--path", selected["path"].as_str().unwrap(), "--no-render"],
            "limit" => vec!["--limit", "12", "--no-render"],
            _ => vec!["--offset", "0", "--no-render"],
        };
        fs::remove_dir_all(&f.root).unwrap();
        let r = report(f.diagram(&catalog, &args).output().unwrap());
        assert_eq!(r["view"], "class-focus");
        assert_eq!(
            Path::new(r["viewDirectory"].as_str().unwrap()),
            catalog.join("views/class-focus")
        );
        let page = r["page"]["items"].as_array().unwrap();
        check_bundle(&r, page);
        if matches!(mode, "entry" | "query" | "path") {
            assert_eq!(page.len(), 1);
            assert_eq!(page[0]["id"], selected["id"]);
        } else {
            let expected = report(
                f.command()
                    .arg("catalog")
                    .arg(&catalog)
                    .args([
                        "--kind", "class", "--json", "--limit", "12", "--offset", "0",
                    ])
                    .output()
                    .unwrap(),
            );
            assert_eq!(r["page"], expected["page"]);
        }
        assert!(!catalog.join("views/class").exists());
        assert!(!catalog.join("views/class-overview").exists());
        f.safe();
    }
}

#[test]
fn focus_navigation_does_not_link_to_a_nonexistent_full_bundle() {
    let Some(f) = Fixture::new() else { return };
    let catalog = f.saved();
    let r = report(
        f.diagram(&catalog, &["--query", "Class087", "--no-render"])
            .output()
            .unwrap(),
    );
    assert!(!catalog.join("views/class/index.html").exists());
    assert!(
        !text(&r, "viewer").contains("href=\"../class/index.html\""),
        "focus advertises a nonexistent full bundle"
    );
}

#[test]
fn complete_and_focus_preserve_each_others_bytes_mtimes_and_only_real_index_links() {
    let Some(f) = Fixture::new() else { return };
    let catalog = f.saved();
    let first = report(
        f.diagram(&catalog, &["--query", "Class087", "--no-render"])
            .output()
            .unwrap(),
    );
    let focus = artifacts(Path::new(first["viewDirectory"].as_str().unwrap()));
    f.complete(&catalog);
    assert_eq!(artifacts(&catalog.join("views/class-focus")), focus);
    let full = artifacts(&catalog.join("views/class"));
    for args in [
        vec!["--query", "Class086"],
        vec!["--limit", "12"],
        vec!["--offset", "0"],
    ] {
        let mut args = args;
        args.push("--no-render");
        report(f.diagram(&catalog, &args).output().unwrap());
        assert_eq!(artifacts(&catalog.join("views/class")), full);
    }
    let focus = artifacts(&catalog.join("views/class-focus"));
    f.complete(&catalog);
    assert_eq!(artifacts(&catalog.join("views/class-focus")), focus);
    let index = fs::read_to_string(catalog.join("views/index.html")).unwrap();
    for link in index.split("href=\"").skip(1) {
        let target = link.split('"').next().unwrap();
        assert!(catalog.join("views").join(target).is_file(), "{target}");
    }
    assert!(!index.contains("class-overview"));
    f.safe();
}

#[test]
fn python_same_file_inheritance_uses_hollow_triangles_to_actual_base_boxes() {
    let Some(f) = Fixture::new() else { return };
    f.write(
        "crowded/class001.py",
        "class Class001:\n    pass\nclass LocalChild(Class001):\n    pass\n",
    );
    let saved = report(f.discover());
    let catalog = Path::new(saved["catalogDirectory"].as_str().unwrap());
    fs::remove_dir_all(&f.root).unwrap();
    let r = f.complete(catalog);
    let legend = check_bundle(&r, &classes(catalog));
    let entries = legend["entries"].as_array().unwrap();
    let child = entries
        .iter()
        .find(|e| e["symbol"].as_str().unwrap().ends_with(".LocalChild"))
        .unwrap();
    let base = entries
        .iter()
        .find(|e| e["symbol"].as_str().unwrap().ends_with(".Class001"))
        .unwrap();
    let rel = legend["relationships"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["classId"] == child["id"])
        .unwrap();
    assert_eq!(rel["targetClassId"], base["id"]);
    assert!(!rel["sourceRefs"].as_array().unwrap().is_empty());
    let mmd = r["diagrams"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| text(d, "mermaid"))
        .collect::<String>();
    assert!(mmd.contains(&format!(
        "{} <|-- {} : declared extends",
        base["diagramId"].as_str().unwrap(),
        child["diagramId"].as_str().unwrap()
    )));
    f.safe();
}

#[test]
fn strict_v2_receipts_reject_tampering_unknowns_links_and_traversal_without_writes() {
    for mode in [
        "corrupt",
        "symlink",
        "directory-symlink",
        "unknown",
        "extra-svg",
        "traversal",
        "duplicate",
        "unknown-field",
        "129-diagrams",
        "metadata-cap",
    ] {
        let Some(f) = Fixture::new() else { return };
        let catalog = f.saved();
        f.complete(&catalog);
        let view = catalog.join("views/class");
        let receipt_path = view.join("receipt.json");
        let mut receipt = json_file(&receipt_path);
        match mode {
            "corrupt" => fs::write(view.join("group-0002.mmd"), "corrupt").unwrap(),
            "symlink" => {
                let p = view.join("group-0001.mmd");
                fs::remove_file(&p).unwrap();
                symlink(f.root.join("app.py"), p).unwrap();
            }
            "directory-symlink" => {
                let preserved = catalog.join("preserved-class");
                fs::rename(&view, &preserved).unwrap();
                symlink(preserved, &view).unwrap();
            }
            "unknown" => fs::write(view.join("notes.txt"), "user content").unwrap(),
            "extra-svg" => fs::write(view.join("group-0001.svg"), "<svg/>").unwrap(),
            "metadata-cap" => {
                let bytes = vec![b' '; 8 * 1024 * 1024];
                fs::write(view.join("index.html"), &bytes).unwrap();
                receipt["htmlHash"] = json!(hash(&bytes));
            }
            "traversal" => {
                receipt["diagrams"][0]["id"] = json!("../outside");
                receipt["artifacts"][3] = json!("../outside.mmd");
            }
            "duplicate" => {
                let d = receipt["diagrams"][0].clone();
                receipt["diagrams"].as_array_mut().unwrap().push(d);
            }
            "129-diagrams" => {
                let mut diagrams = Vec::new();
                let mut manifest = vec![
                    json!("receipt.json"),
                    json!("legend.json"),
                    json!("index.html"),
                ];
                for n in 1..=129 {
                    let id = format!("group-{n:04}");
                    let name = format!("{id}.mmd");
                    let mmd = b"classDiagram\n";
                    fs::write(view.join(&name), mmd).unwrap();
                    diagrams.push(
                        json!({"id":id,"title":"Bounded group","mmdSha":hash(mmd),"svgSha":null}),
                    );
                    manifest.push(json!(name));
                }
                receipt["diagrams"] = json!(diagrams);
                receipt["artifacts"] = json!(manifest);
            }
            _ => receipt["unexpected"] = json!(true),
        }
        if matches!(
            mode,
            "traversal" | "duplicate" | "unknown-field" | "129-diagrams" | "metadata-cap"
        ) {
            fs::write(&receipt_path, serde_json::to_vec(&receipt).unwrap()).unwrap();
        }
        let before = artifacts(&f.cwd);
        let output = f.diagram(&catalog, &["--no-render"]).output().unwrap();
        assert!(!output.status.success(), "accepted {mode}");
        assert_eq!(artifacts(&f.cwd), before, "rejection modified {mode}");
        f.safe();
    }
}

#[test]
fn later_group_render_failure_and_svg_cap_leave_previous_bundle_intact() {
    for mode in ["second-group", "svg-cap", "total-svg-cap"] {
        let Some(f) = Fixture::new() else { return };
        let catalog = f.saved();
        f.complete(&catalog);
        let before = artifacts(&catalog);
        let mut command = f.diagram(&catalog, &[]);
        command.env("IDEOMANCER_MMDC", &f.renderer);
        if mode == "second-group" {
            command.env("UML_FAIL_AT", "2");
        } else if mode == "svg-cap" {
            command.env("UML_LARGE_SVG", "1");
        } else {
            command.env("UML_TOTAL_SVG", "1");
        }
        let output = command.output().unwrap();
        assert!(!output.status.success(), "accepted {mode}");
        let count = fs::read_to_string(f.cwd.join("renders.txt"))
            .unwrap()
            .lines()
            .count();
        assert_eq!(
            count,
            match mode {
                "second-group" => 2,
                "svg-cap" => 1,
                _ => 10,
            }
        );
        assert_eq!(artifacts(&catalog), before, "partial publication: {mode}");
        f.safe();
    }
}

#[test]
fn rendered_cache_preserves_all_group_bytes_and_mtimes() {
    let Some(f) = Fixture::new() else { return };
    let catalog = f.saved();
    let r = report(
        f.diagram(&catalog, &[])
            .env("IDEOMANCER_MMDC", &f.renderer)
            .output()
            .unwrap(),
    );
    check_bundle(&r, &classes(&catalog));
    let before = artifacts(&catalog);
    report(
        f.diagram(&catalog, &[])
            .env("IDEOMANCER_MMDC", &f.renderer)
            .output()
            .unwrap(),
    );
    f.complete(&catalog);
    assert_eq!(artifacts(&catalog), before);
    assert_eq!(
        fs::read_to_string(f.cwd.join("renders.txt"))
            .unwrap()
            .lines()
            .count(),
        r["diagrams"].as_array().unwrap().len()
    );
    f.safe();
}

#[test]
fn legitimate_v1_class_slot_upgrades_but_unknown_old_files_are_never_cleaned() {
    for unknown in [false, true] {
        let Some(f) = Fixture::new() else { return };
        let catalog = f.saved();
        let view = catalog.join("views/class");
        fs::create_dir_all(&view).unwrap();
        for (name, bytes) in [
            ("diagram.mmd", "classDiagram\n"),
            ("legend.json", "{}\n"),
            ("index.html", "<!doctype html><p>Old class view</p>\n"),
        ] {
            fs::write(view.join(name), bytes).unwrap();
        }
        let receipt = json!({"schemaVersion":"source-views.v1","snapshotId":json_file(&catalog.join("catalog.json"))["snapshotId"],"mermaidSha256":hash(&fs::read(view.join("diagram.mmd")).unwrap()),"legendSha256":hash(&fs::read(view.join("legend.json")).unwrap()),"htmlSha256":hash(&fs::read(view.join("index.html")).unwrap()),"svgSha256":null,"renderer":null});
        fs::write(
            view.join("receipt.json"),
            serde_json::to_vec(&receipt).unwrap(),
        )
        .unwrap();
        if unknown {
            fs::write(view.join("my-notes.txt"), "preserve me").unwrap();
            let before = artifacts(&catalog);
            assert!(!f
                .diagram(&catalog, &["--no-render"])
                .output()
                .unwrap()
                .status
                .success());
            assert_eq!(artifacts(&catalog), before);
        } else {
            let r = f.complete(&catalog);
            check_bundle(&r, &classes(&catalog));
            assert!(!view.join("diagram.mmd").exists());
        }
        f.safe();
    }
}

#[test]
fn cached_svg_size_cap_is_enforced_even_with_matching_hash() {
    let Some(f) = Fixture::new() else { return };
    let catalog = f.saved();
    let r = report(
        f.diagram(&catalog, &[])
            .env("IDEOMANCER_MMDC", &f.renderer)
            .output()
            .unwrap(),
    );
    let svg = Path::new(r["diagrams"][0]["svg"].as_str().unwrap());
    let bytes = vec![b' '; 8 * 1024 * 1024 + 1];
    fs::write(svg, &bytes).unwrap();
    let receipt_path = catalog.join("views/class/receipt.json");
    let mut receipt = json_file(&receipt_path);
    receipt["diagrams"][0]["svgSha"] = json!(hash(&bytes));
    fs::write(receipt_path, serde_json::to_vec(&receipt).unwrap()).unwrap();
    let before = artifacts(&catalog);
    assert!(!f
        .diagram(&catalog, &["--no-render"])
        .output()
        .unwrap()
        .status
        .success());
    assert_eq!(artifacts(&catalog), before);
    f.safe();
}

#[test]
fn rediscovery_invalidates_both_slots_and_empty_incomplete_snapshots_stay_honest() {
    let Some(f) = Fixture::new() else { return };
    let catalog = f.saved();
    f.complete(&catalog);
    report(
        f.diagram(&catalog, &["--limit", "12", "--no-render"])
            .output()
            .unwrap(),
    );
    let before = artifacts(&catalog);
    assert_eq!(f.saved(), catalog);
    assert_eq!(artifacts(&catalog), before);
    fs::remove_dir_all(&f.root).unwrap();
    fs::create_dir(&f.root).unwrap();
    f.write("empty.py", "def no_classes():\n    pass\n");
    report(f.discover());
    assert!(!catalog.join("views").exists());
    let empty = f.complete(&catalog);
    check_bundle(&empty, &[]);
    assert_eq!(empty["coverage"]["homeClasses"], 0);
    assert!(text(&empty, "viewer").contains("No classes in the saved catalog"));
    f.write(
        "valid.py",
        "class Retained:\n    def observed(self):\n        self.value = None\n",
    );
    f.write("broken.py", "class Broken(:\n");
    let output = f.discover();
    assert!(!output.status.success());
    let saved: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(saved["complete"], false);
    assert!(!catalog.join("views").exists());
    fs::remove_dir_all(&f.root).unwrap();
    let incomplete = f.complete(&catalog);
    check_bundle(&incomplete, &classes(&catalog));
    let html = text(&incomplete, "viewer");
    assert!(html.contains("INCOMPLETE SNAPSHOT") && html.contains("role=\"alert\""));
    assert!(html.contains("Retained") && html.contains("observed") && html.contains("value"));
    f.safe();
}

#[test]
fn trusted_javascript_hierarchy_and_scoped_dependencies_survive_deleted_source() {
    let (Some(node), Some(compiler)) = (
        std::env::var_os("IDEOMANCER_TEST_NODE"),
        std::env::var_os("IDEOMANCER_TEST_TYPESCRIPT"),
    ) else {
        eprintln!("SKIP class_uml JavaScript: explicit trusted Node and TypeScript required");
        return;
    };
    let node = PathBuf::from(node);
    let compiler = PathBuf::from(compiler);
    assert!(node.is_absolute() && node.is_file() && compiler.is_absolute() && compiler.is_file());
    let Some(f) = Fixture::new() else { return };
    fs::remove_dir_all(&f.root).unwrap();
    fs::create_dir(&f.root).unwrap();
    let mut errors = String::from("class RootError extends Error { constructor(message) { super(message); this.message = message; } }\n");
    for n in 0..8 {
        errors.push_str(&format!(
            "class Error{n} extends RootError {{ describe() {{ return this.message; }} }}\n"
        ));
    }
    f.write("errors.ts", &errors);
    f.write("factory.ts", "class Parent {}\nclass Child extends Parent {}\nexport function factory(Base, Factory) { return class Product extends Base { constructor(dep: DeclaredService) { super(); this.dep = dep; } run() { Factory(); return new Factory(); } }; }\n");
    f.write("openapi.json", "invalid [");
    f.write("swagger.yaml", "malformed: [");
    let saved = report(
        f.command()
            .arg("discover")
            .arg(&f.root)
            .args([
                "--lang", "js", "--json", "--kind", "class", "--limit", "100",
            ])
            .arg("--out")
            .arg(f.cwd.join("catalog output"))
            .arg("--node")
            .arg(node)
            .arg("--typescript")
            .arg(compiler)
            .output()
            .unwrap(),
    );
    assert_eq!(saved["page"]["counts"]["class"], 12);
    let catalog = PathBuf::from(saved["catalogDirectory"].as_str().unwrap());
    fs::remove_dir_all(&f.root).unwrap();
    let r = f.complete(&catalog);
    let legend = check_bundle(&r, &classes(&catalog));
    let entries = legend["entries"].as_array().unwrap();
    let error_ids: BTreeSet<_> = entries
        .iter()
        .filter(|e| e["path"] == "errors.ts")
        .map(|e| e["id"].as_str().unwrap())
        .collect();
    assert_eq!(error_ids.len(), 9);
    assert!(legend["groups"]
        .as_array()
        .unwrap()
        .iter()
        .any(|g| error_ids.is_subset(
            &g["homeClassIds"]
                .as_array()
                .unwrap()
                .iter()
                .map(|id| id.as_str().unwrap())
                .collect::<BTreeSet<_>>()
        )));
    let relations = legend["relationships"].as_array().unwrap();
    assert_eq!(
        relations
            .iter()
            .filter(|r| r["relationship"] == "extends")
            .count(),
        11
    );
    assert_eq!(
        relations
            .iter()
            .filter(|r| r["relationship"] == "extends" && !r["targetClassId"].is_null())
            .count(),
        9
    );
    for (kind, count) in [
        ("stores_parameter", 2),
        ("uses_factory_binding", 1),
        ("creates", 1),
    ] {
        assert_eq!(
            relations
                .iter()
                .filter(|r| r["relationship"] == kind)
                .count(),
            count,
            "{kind}: {relations:?}"
        );
    }
    let product = entries
        .iter()
        .find(|e| e["symbol"].as_str().unwrap().ends_with(".Product"))
        .unwrap();
    let field = product["attributes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["name"] == "dep")
        .unwrap();
    assert_eq!(field["type"], "DeclaredService");
    for rel in relations {
        assert!(!rel["sourceRefs"].as_array().unwrap().is_empty());
        if rel["relationship"] == "stores_parameter" {
            assert_eq!(rel["representedAsField"], rel["memberName"] == "message");
        }
    }
    let mmd = r["diagrams"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| text(d, "mermaid"))
        .collect::<String>();
    for required in [
        "<|--",
        "..>",
        "stores argument",
        "creates",
        "dep : DeclaredService",
        "supplied parameter: Base",
    ] {
        assert!(mmd.contains(required), "{required}: {mmd}");
    }
    assert!(!mmd.contains("*--") && !mmd.contains("o--"));
    assert!(!mmd.contains("supplied parameter: message"));
    f.safe();
}

#[test]
fn javascript_esm_named_aliases_resolve_extends_and_new_with_full_saved_proof() {
    let Some((f, catalog)) = javascript_fixture(&[
        ("base.ts", "class Base {} export { Base as Published };"),
        ("child.ts", "import { Published as Parent } from './base'; class Child extends Parent { make() { return new Parent(); } }"),
    ]) else { return };
    let r = f.complete(&catalog);
    let legend = check_bundle(&r, &classes(&catalog));
    let evidence = json_file(&catalog.join("evidence.json"));
    let facts = evidence["facts"].as_array().unwrap();
    for kind in ["extends", "creates"] {
        let rel = class_relation(&legend, "Child", kind);
        assert_eq!(rel["targetClassId"], class_entry(&legend, "Base")["id"]);
        assert_eq!(rel["policy"], "javascript-class-relations-2");
        assert_eq!(rel["bindingKind"], "import");
        assert!(rel["importFactId"].is_string());
        assert_eq!(rel["resolution"]["policy"], "selected-relative-classes-1");
        assert_eq!(rel["resolution"]["status"], "resolved");
        let proof: Vec<_> = rel["sourceFactIds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| {
                facts
                    .iter()
                    .find(|fact| fact["id"] == *id)
                    .expect("saved proof fact")
            })
            .collect();
        for path in ["base.ts", "child.ts"] {
            assert!(proof.iter().any(|fact| fact["path"] == path));
            assert!(proof
                .iter()
                .any(|fact| fact["path"] == path && fact["kind"] == "js_module_summary"));
            assert!(proof
                .iter()
                .any(|fact| fact["path"] == path && fact["kind"] == "js_binding_guard"));
        }
        for kind in [
            "js_module_import",
            "js_module_export",
            "js_binding_guard",
            "class",
        ] {
            assert!(
                proof.iter().any(|fact| fact["kind"] == kind),
                "missing {kind}: {proof:?}"
            );
        }
        assert_eq!(rel["sourceRefs"].as_array().unwrap().len(), proof.len());
    }
    assert!(all_mermaid(&r).contains("<|--"));
    assert!(all_mermaid(&r).contains("creates"));
    f.safe();
}

#[test]
fn javascript_commonjs_named_aliases_resolve_class_exports_and_creation() {
    let Some((f, catalog)) = javascript_fixture(&[
            ("base.js", "class Base {} module.exports = { Published: Base };"),
            ("child.js", "const { Published: Parent } = require('./base'); class Child extends Parent { make() { return new Parent(); } }"),
        ]) else { return };
    let r = f.complete(&catalog);
    let legend = check_bundle(&r, &classes(&catalog));
    for kind in ["extends", "creates"] {
        let rel = class_relation(&legend, "Child", kind);
        assert_eq!(rel["targetClassId"], class_entry(&legend, "Base")["id"]);
        assert_eq!(rel["resolution"]["status"], "resolved");
    }
    f.safe();
}

#[test]
fn javascript_local_new_class_resolves_without_import_or_injection() {
    let Some((f, catalog)) = javascript_fixture(&[
        (
            "local.ts",
            "class Local {} class Consumer { make() { return new Local(); } }",
        ),
        ("unrelated.ts", "export class Other {}"),
    ]) else {
        return;
    };
    let r = f.complete(&catalog);
    let legend = check_bundle(&r, &classes(&catalog));
    let rel = class_relation(&legend, "Consumer", "creates");
    assert_eq!(rel["targetClassId"], class_entry(&legend, "Local")["id"]);
    assert_eq!(rel["bindingKind"], "local_class");
    assert!(rel["importFactId"].is_null());
    assert_eq!(rel["resolution"]["status"], "resolved");
    assert!(!all_mermaid(&r).contains("injected"));
    f.safe();
}

#[test]
fn javascript_commonjs_exports_before_class_initialization_do_not_resolve() {
    for base in [
        "module.exports = { Base }; class Base {}",
        "exports.Base = Base; class Base {}",
        "module.exports = { Base }; const Base = class Base {};",
    ] {
        let Some((f, catalog)) = javascript_fixture(&[
            ("base.js", base),
            ("child.js", "const { Base } = require('./base'); class Child extends Base { make() { return new Base(); } }"),
        ]) else { return };
        let r = f.complete(&catalog);
        let legend = check_bundle(&r, &classes(&catalog));
        for kind in ["extends", "creates"] {
            let rel = class_relation(&legend, "Child", kind);
            assert!(rel["targetClassId"].is_null(), "{base}: {rel}");
            assert_eq!(rel["resolution"]["status"], "unresolved");
        }
        f.safe();
    }
}

#[test]
fn javascript_commonjs_use_before_require_is_an_unresolved_boundary() {
    let Some((f, catalog)) = javascript_fixture(&[
        ("base.js", "class Base {} module.exports = { Base };"),
        ("child.js", "class Child extends Base { make() { return new Base(); } } const { Base } = require('./base');"),
    ]) else { return };
    let r = f.complete(&catalog);
    let legend = check_bundle(&r, &classes(&catalog));
    let rel = class_relation(&legend, "Child", "extends");
    assert!(rel["targetClassId"].is_null());
    assert_eq!(rel["resolution"]["status"], "unresolved");
    f.safe();
}

#[test]
fn javascript_ambiguous_relative_paths_and_cycles_never_choose_a_class() {
    for mode in ["ambiguous", "cycle"] {
        let mut files = vec![
            ("base.ts", "export class Base {}"),
            ("child.ts", "import { Base } from './base'; export class Child extends Base { make() { return new Base(); } }"),
        ];
        if mode == "ambiguous" {
            files.push(("base.js", "export class Base {}"));
        } else {
            files[0].1 = "import { Child } from './child'; export class Base {}";
        }
        let Some((f, catalog)) = javascript_fixture(&files) else {
            return;
        };
        let r = f.complete(&catalog);
        let legend = check_bundle(&r, &classes(&catalog));
        for kind in ["extends", "creates"] {
            let rel = class_relation(&legend, "Child", kind);
            assert!(rel["targetClassId"].is_null(), "{mode}: {rel}");
            assert_eq!(rel["resolution"]["status"], "unresolved");
            assert!(rel["resolution"]["reason"]
                .as_str()
                .is_some_and(|s| !s.is_empty()));
        }
        f.safe();
    }
}

#[test]
fn javascript_out_of_scope_interop_and_nonclass_exports_never_resolve_as_classes() {
    for (base, child) in [
        ("export class Base {}", "import { Base } from '../base'; class Child extends Base { make() { return new Base(); } }"),
        ("class Base {} module.exports = { Base };", "import { Base } from './base'; class Child extends Base { make() { return new Base(); } }"),
        ("export function Base() {}", "import { Base } from './base'; class Child extends Base { make() { return new Base(); } }"),
        ("export const Base = 1;", "import { Base } from './base'; class Child extends Base { make() { return new Base(); } }"),
        ("export default class Base {}", "import Base from './base'; class Child extends Base { make() { return new Base(); } }"),
    ] {
        let Some((f, catalog)) = javascript_fixture(&[("base.ts", base), ("child.ts", child)]) else { return };
        let r = f.complete(&catalog);
        let legend = check_bundle(&r, &classes(&catalog));
        class_relation(&legend, "Child", "extends");
        let relations = legend["relationships"].as_array().unwrap();
        if !child.contains("import Base from") {
            class_relation(&legend, "Child", "creates");
        }
        for rel in relations {
            assert!(rel["targetClassId"].is_null(), "{base} {child}: {rel}");
            assert_eq!(rel["resolution"]["status"], "unresolved");
        }
        f.safe();
    }
}

#[test]
fn javascript_imported_class_called_as_function_is_not_class_creation_or_service_target() {
    let Some((f, catalog)) = javascript_fixture(&[
        ("base.ts", "export class Base {}"),
        ("child.ts", "import { Base as Imported } from './base'; class Child { run() { return Imported(); } }"),
    ]) else { return };
    let r = f.complete(&catalog);
    let legend = check_bundle(&r, &classes(&catalog));
    assert!(legend["relationships"].as_array().unwrap().is_empty());
    let method = json_file(&catalog.join("catalog.json"))["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["symbol"].as_str().unwrap().ends_with(".Child.run"))
        .unwrap()
        .clone();
    let services = report(
        f.command()
            .arg("diagram")
            .arg(&catalog)
            .args([
                "--entry",
                method["id"].as_str().unwrap(),
                "--view",
                "services",
                "--no-render",
            ])
            .output()
            .unwrap(),
    );
    let services = json_file(Path::new(services["legend"].as_str().unwrap()));
    let calls = services["calls"].as_array().unwrap();
    assert!(!calls.is_empty());
    assert!(calls.iter().all(|c| c["targetCandidate"].is_null()));
    f.safe();
}

#[test]
fn javascript_unicode_class_and_named_alias_preserve_exact_identifier_resolution() {
    let Some((f, catalog)) = javascript_fixture(&[
        ("base.ts", "class Cafe\u{301} {} export { Cafe\u{301} as Publie\u{301} };"),
        ("child.ts", "import { Publie\u{301} as Alias\u{301} } from './base'; class Child extends Alias\u{301} { make() { return new Alias\u{301}(); } }"),
    ]) else { return };
    let r = f.complete(&catalog);
    let legend = check_bundle(&r, &classes(&catalog));
    for kind in ["extends", "creates"] {
        let rel = class_relation(&legend, "Child", kind);
        assert_eq!(
            rel["targetClassId"],
            class_entry(&legend, "Cafe\u{301}")["id"]
        );
        assert_eq!(rel["expression"], "Alias\u{301}");
        assert_eq!(rel["resolution"]["status"], "resolved");
    }
    f.safe();
}

#[test]
fn javascript_constructor_state_is_cited_and_values_are_redacted() {
    let secret = "PRIVATE_CONSTRUCTOR_STATE_NEVER_RETAIN";
    let source = format!("class State {{ constructor(error) {{ this.errorType = '{secret}'; this.stackTrace = error.stack; this.cache = new Map([['{secret}', '{secret}']]); }} }}");
    let Some((f, catalog)) =
        javascript_fixture(&[("state.ts", &source), ("other.ts", "export class Other {}")])
    else {
        return;
    };
    let r = f.complete(&catalog);
    let legend = check_bundle(&r, &classes(&catalog));
    let fields = class_entry(&legend, "State")["attributes"]
        .as_array()
        .unwrap();
    assert_eq!(
        fields
            .iter()
            .map(|a| a["name"].as_str().unwrap())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["cache", "errorType", "stackTrace"])
    );
    let evidence = json_file(&catalog.join("evidence.json"));
    for field in fields {
        assert_eq!(field["static"], false);
        assert!(field["sourceFactIds"]
            .as_array()
            .unwrap()
            .iter()
            .any(|id| evidence["facts"]
                .as_array()
                .unwrap()
                .iter()
                .any(|fact| fact["id"] == *id && fact["kind"] == "js_constructor_field")));
        assert!(text(&r, "viewer").contains(field["name"].as_str().unwrap()));
    }
    assert!(legend["relationships"].as_array().unwrap().is_empty());
    for (path, (bytes, _)) in artifacts(&catalog) {
        assert!(
            !String::from_utf8_lossy(&bytes).contains(secret),
            "payload in {path:?}"
        );
    }
    f.safe();
}

#[test]
fn javascript_signatures_keep_declared_types_modifiers_and_no_default_dependencies() {
    let Some((f, catalog)) = javascript_fixture(&[
        ("signature.ts", "class Signature { constructor(message: string = 'PRIVATE_DEFAULT_NEVER_RETAIN') { this.message = message; } static build<T>(value: T, optional?: string, count: number = 73, ...rest: Array<T>): Promise<T> { return value; } }"),
        ("other.ts", "export class Other {}"),
    ]) else { return };
    let r = f.complete(&catalog);
    let legend = check_bundle(&r, &classes(&catalog));
    let entry = class_entry(&legend, "Signature");
    let constructor = entry["methods"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["name"] == "constructor")
        .unwrap();
    assert_eq!(
        constructor["parameters"],
        json!([
            {"name":"message","pattern":false,"optional":false,"rest":false,"hasDefault":true,"type":"string"}
        ])
    );
    assert!(constructor["returnType"].is_null());
    let method = entry["methods"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["name"] == "build")
        .unwrap();
    assert_eq!(method["static"], true);
    assert_eq!(method["typeParameters"], json!(["T"]));
    assert_eq!(method["returnType"], "Promise<T>");
    assert_eq!(
        method["parameters"],
        json!([
            {"name":"value","pattern":false,"optional":false,"rest":false,"hasDefault":false,"type":"T"},
            {"name":"optional","pattern":false,"optional":true,"rest":false,"hasDefault":false,"type":"string"},
            {"name":"count","pattern":false,"optional":false,"rest":false,"hasDefault":true,"type":"number"},
            {"name":"rest","pattern":false,"optional":false,"rest":true,"hasDefault":false,"type":"Array<T>"}
        ])
    );
    let mmd = all_mermaid(&r);
    for term in [
        "build",
        // Source punctuation is entity-escaped in Mermaid, then decoded by
        // the renderer; the full unescaped signature is checked above.
        "optional#63;",
        "count#63;",
        "...rest",
        "Promise",
        "$",
        "message",
    ] {
        assert!(mmd.contains(term), "{term}: {mmd}");
    }
    assert!(legend["relationships"]
        .as_array()
        .unwrap()
        .iter()
        .all(|r| r["relationship"] == "stores_parameter" && r["representedAsField"] == true));
    assert!(!mmd.contains("supplied parameter"));
    for (path, (bytes, _)) in artifacts(&catalog) {
        assert!(
            !String::from_utf8_lossy(&bytes).contains("PRIVATE_DEFAULT_NEVER_RETAIN"),
            "default in {path:?}"
        );
    }
    f.safe();
}

#[test]
fn javascript_v4_saved_same_file_relations_remain_renderable_without_member_extensions() {
    let Some((f, catalog)) = javascript_fixture(&[
        (
            "local.ts",
            "class Base {} class Child extends Base { make() { return new Base(); } }",
        ),
        ("other.ts", "export function unrelated() {}"),
    ]) else {
        return;
    };
    let mut saved = json_file(&catalog.join("catalog.json"));
    let mut evidence = json_file(&catalog.join("evidence.json"));
    let mut receipt = json_file(&catalog.join("receipt.json"));
    for document in [&mut saved, &mut evidence] {
        document["extractorVersion"] = json!("javascript-typescript-static-4-class-relations");
    }
    let facts = evidence["facts"].as_array_mut().unwrap();
    facts.retain(|f| {
        !matches!(
            f["kind"].as_str(),
            Some("js_callable_signature" | "js_constructor_field")
        )
    });
    for fact in facts {
        let Some(kind) = fact["kind"].as_str() else {
            continue;
        };
        if matches!(
            kind,
            "js_class_scope" | "js_class_reference" | "js_class_relation" | "js_module_summary"
        ) {
            let mut value: Value = serde_json::from_str(fact["value"].as_str().unwrap()).unwrap();
            value["policy"] = json!(if kind == "js_module_summary" {
                "selected-relative-functions-1"
            } else {
                "javascript-class-relations-1"
            });
            if kind == "js_class_relation" {
                value.as_object_mut().unwrap().remove("importFactId");
            }
            fact["value"] = json!(serde_json::to_string(&value).unwrap());
        }
    }
    let catalog_bytes = serde_json::to_vec(&saved).unwrap();
    let evidence_bytes = serde_json::to_vec(&evidence).unwrap();
    receipt["catalogSha256"] = json!(hash(&catalog_bytes));
    receipt["evidenceSha256"] = json!(hash(&evidence_bytes));
    fs::write(catalog.join("catalog.json"), catalog_bytes).unwrap();
    fs::write(catalog.join("evidence.json"), evidence_bytes).unwrap();
    fs::write(
        catalog.join("receipt.json"),
        serde_json::to_vec(&receipt).unwrap(),
    )
    .unwrap();
    let r = f.complete(&catalog);
    let legend = check_bundle(&r, &classes(&catalog));
    for kind in ["extends", "creates"] {
        let rel = class_relation(&legend, "Child", kind);
        assert_eq!(rel["policy"], "javascript-class-relations-1");
        assert_eq!(rel["targetClassId"], class_entry(&legend, "Base")["id"]);
    }
    f.safe();
}

#[test]
fn javascript_storage_only_fields_hide_parameter_boxes_but_receiver_use_keeps_dependency() {
    let Some((f, catalog)) = javascript_fixture(&[
        ("storage.ts", "class Storage { constructor(message, registryId, worker) { this.message = message; this.registryId = registryId; this.worker = worker; } run() { return this.worker.run(); } }"),
        ("other.ts", "export class Other {}"),
    ]) else { return };
    let r = f.complete(&catalog);
    let legend = check_bundle(&r, &classes(&catalog));
    let fields = class_entry(&legend, "Storage")["attributes"]
        .as_array()
        .unwrap();
    assert_eq!(fields.len(), 3);
    let relations = legend["relationships"].as_array().unwrap();
    assert_eq!(relations.len(), 3);
    for name in ["message", "registryId", "worker"] {
        assert!(fields.iter().any(|f| f["name"] == name));
        let rel = relations.iter().find(|r| r["memberName"] == name).unwrap();
        assert_eq!(rel["relationship"], "stores_parameter");
        assert_eq!(rel["representedAsField"], name != "worker");
        if name == "worker" {
            assert!(!rel["projectionSupportingFactIds"]
                .as_array()
                .unwrap()
                .is_empty());
        }
    }
    let mmd = all_mermaid(&r);
    assert!(
        !mmd.contains("supplied parameter: message")
            && !mmd.contains("supplied parameter: registryId")
    );
    assert!(mmd.contains("worker") && mmd.contains("stores argument"));
    f.safe();
}

#[test]
#[ignore = "requires explicit trusted Node, installed Puppeteer and Chromium; no downloads"]
fn chromium_multi_uml_controls_and_no_javascript_fallback() {
    let node =
        PathBuf::from(std::env::var_os("IDEOMANCER_TEST_NODE").expect("trusted Node required"));
    let module = PathBuf::from(
        std::env::var_os("IDEOMANCER_PUPPETEER_MODULE").expect("installed Puppeteer required"),
    );
    assert!(node.is_absolute() && node.is_file() && module.is_absolute() && module.exists());
    let f = Fixture::new().expect("trusted Python required");
    let catalog = f.saved();
    let r = report(
        f.diagram(&catalog, &[])
            .env("IDEOMANCER_MMDC", &f.renderer)
            .output()
            .unwrap(),
    );
    let output = Command::new(node)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/class_uml_browser.cjs"))
        .arg(r["viewer"].as_str().unwrap())
        .arg(r["legend"].as_str().unwrap())
        .env("IDEOMANCER_PUPPETEER_MODULE", module)
        .env_remove("NODE_OPTIONS")
        .env_remove("NODE_PATH")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    f.safe();
}
