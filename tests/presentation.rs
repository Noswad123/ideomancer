//! Current-only saved-run presentation contracts. All external programs are stubs.
#![cfg(unix)]
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::SystemTime;

const VIEW: &str = "presentation";
const TEXT: &str = "<script>alert('x')</script> & long original explanatory label";
const AGENT: &str = r#"import json, os, sys
from pathlib import Path
request = json.loads(Path(sys.argv[3]).read_text())
with open(os.environ['AGENT_LOG'], 'a') as log: log.write('call\n')
packet = request['input']
ref = next(f['id'] for f in packet['facts'] if f['kind'] == 'module')
text = os.environ['TITLE']
proposal = {'schemaVersion':'ideomancer.architecture.v1','snapshotId':packet['snapshotId'],
 'name':text, 'components':[{'id':'entry','label':text,'kind':'component','rationale':text,'evidenceIds':[ref]}],
 'relationships':[], 'uncertainties':[text]}
print(json.dumps({'protocolVersion':'agent-execution.v1','requestId':request['requestId'],
 'status':'completed','output':proposal,'model':'stub'}))
"#;
const RENDERER: &str = r#"import os, sys, time
from pathlib import Path
with open(os.environ['RENDER_LOG'], 'a') as log: log.write('call\n')
if os.environ.get('PRESENT_RENDER_READY'):
    Path(os.environ['PRESENT_RENDER_READY']).touch()
    deadline = time.monotonic() + 10
    while not Path(os.environ['PRESENT_RENDER_RELEASE']).exists():
        if time.monotonic() > deadline: sys.exit(8)
        time.sleep(0.01)
if os.environ.get('RENDER_FAIL') == '1': sys.exit(9)
Path(sys.argv[4]).write_text('<svg width="300" height="150"><text>DRAFT</text></svg>')
"#;

fn json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
fn success(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn artifacts(root: &Path) -> BTreeMap<PathBuf, (Vec<u8>, SystemTime)> {
    let mut result = BTreeMap::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            for (child, value) in artifacts(&path) {
                result.insert(PathBuf::from(path.file_name().unwrap()).join(child), value);
            }
        } else {
            result.insert(
                path.file_name().unwrap().into(),
                (
                    fs::read(&path).unwrap(),
                    fs::metadata(&path).unwrap().modified().unwrap(),
                ),
            );
        }
    }
    result
}
struct Fixture {
    _temp: tempfile::TempDir,
    directory: PathBuf,
    root: PathBuf,
    run: PathBuf,
    renderer: PathBuf,
}
impl Fixture {
    fn new() -> Option<Self> {
        let python =
            std::env::var_os("IDEOMANCER_TEST_PYTHON").unwrap_or_else(|| "python3.12".into());
        let output = Command::new(python)
            .args([
                "-I",
                "-c",
                "import sys; assert sys.version_info >= (3,12); print(sys.executable)",
            ])
            .output()
            .ok()?;
        if !output.status.success() {
            eprintln!("SKIP presentation test: Python >=3.12 required");
            return None;
        }
        let python = String::from_utf8(output.stdout).unwrap().trim().to_owned();
        let temp = tempfile::tempdir().unwrap();
        let directory = fs::canonicalize(temp.path()).unwrap();
        let root = directory.join("source");
        fs::create_dir(&root).unwrap();
        fs::write(
            root.join("app.py"),
            "raise RuntimeError('must not execute')\ndef serve():\n    return 42\n",
        )
        .unwrap();
        let program = |name: &str, body: &str| {
            let path = directory.join(name);
            fs::write(&path, format!("#!{python}\n{body}")).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
            path
        };
        let agent = program("agent", AGENT);
        let renderer = program("renderer", RENDERER);
        let f = Self {
            run: directory.join("saved run"),
            directory,
            root,
            renderer,
            _temp: temp,
        };
        success(
            f.cli()
                .arg("inspect")
                .arg(&f.root)
                .arg("--out")
                .arg(&f.run)
                .arg("--python")
                .arg(python)
                .output()
                .unwrap(),
        );
        success(
            f.cli()
                .arg("interpret")
                .arg(&f.run)
                .arg("--agent")
                .arg(agent)
                .args(["--model", "stub"])
                .output()
                .unwrap(),
        );
        Some(f)
    }
    fn cli(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_ideomancer"));
        command
            .current_dir(&self.directory)
            .env("TITLE", TEXT)
            .env("AGENT_LOG", self.directory.join("agent.log"))
            .env("RENDER_LOG", self.directory.join("render.log"));
        command
    }
    fn present(&self) -> Command {
        let mut command = self.cli();
        command
            .arg("present")
            .arg(&self.run)
            .arg("--mmdc")
            .arg(&self.renderer);
        command
    }
    fn calls(&self, name: &str) -> usize {
        fs::read_to_string(self.directory.join(name))
            .unwrap_or_default()
            .lines()
            .count()
    }
}

#[test]
fn verified_older_presentation_upgrades_viewer_without_agent_or_source_access() {
    use sha2::{Digest, Sha256};

    let Some(f) = Fixture::new() else { return };
    success(f.present().arg("--no-render").output().unwrap());
    let view = f.run.join(VIEW);
    let old_html = b"<!doctype html><html><body><h1>Saved static presentation</h1><img src=\"diagram.svg\" alt=\"Saved diagram\"></body></html>\n";
    fs::write(view.join("index.html"), old_html).unwrap();
    let mut old_legend = json(&view.join("legend.json"));
    old_legend["presentationVersion"] = "readable-v1".into();
    let old_legend_bytes = serde_json::to_vec_pretty(&old_legend).unwrap();
    fs::write(view.join("legend.json"), &old_legend_bytes).unwrap();
    let mut old_receipt = json(&view.join("receipt.json"));
    old_receipt["presentationVersion"] = "readable-v1".into();
    old_receipt["htmlSha256"] = format!("{:x}", Sha256::digest(old_html)).into();
    old_receipt["legendSha256"] = format!("{:x}", Sha256::digest(&old_legend_bytes)).into();
    fs::write(
        view.join("receipt.json"),
        serde_json::to_vec_pretty(&old_receipt).unwrap(),
    )
    .unwrap();
    let state = fs::read(f.run.join("state.json")).unwrap();
    let proposal = fs::read(f.run.join("proposal.json")).unwrap();
    fs::remove_dir_all(&f.root).unwrap();
    let report = success(
        f.present()
            .arg("--no-render")
            .env("IDEOMANCER_AGENT", f.directory.join("missing agent"))
            .env("IDEOMANCER_PYTHON", f.directory.join("missing python"))
            .output()
            .unwrap(),
    );
    assert_eq!(report["presentationVersion"], "readable-v2");
    let html = fs::read_to_string(view.join("index.html")).unwrap();
    assert!(html.contains("data-viewer-viewport"));
    assert!(html.contains("script-src 'sha256-"));
    assert_eq!(
        json(&view.join("receipt.json"))["presentationVersion"],
        "readable-v2"
    );
    assert_eq!(fs::read(f.run.join("state.json")).unwrap(), state);
    assert_eq!(fs::read(f.run.join("proposal.json")).unwrap(), proposal);
    assert_eq!(f.calls("agent.log"), 1);
    assert_eq!(f.calls("render.log"), 0);
    assert!(!f.run.join("presentations").exists());
    let before = artifacts(&f.run);
    assert_eq!(
        success(f.present().arg("--no-render").output().unwrap()),
        report
    );
    assert_eq!(artifacts(&f.run), before);
}

#[test]
fn current_presentation_is_readable_offline_and_reuses_identical_artifacts() {
    let Some(f) = Fixture::new() else { return };
    let state = fs::read(f.run.join("state.json")).unwrap();
    let proposal = fs::read(f.run.join("proposal.json")).unwrap();
    fs::remove_dir_all(&f.root).unwrap();
    let report = success(f.present().arg("--no-render").output().unwrap());
    let view = f.run.join(VIEW);
    assert_eq!(
        report["draftManifest"],
        view.join("draft.architecture.json").to_str().unwrap()
    );
    assert!(report["draftSvg"].is_null());
    let html = fs::read_to_string(view.join("index.html")).unwrap();
    assert!(html.contains("&lt;script&gt;"));
    assert_eq!(html.matches("<script>").count(), 1);
    assert!(html.contains("script-src 'sha256-"));
    assert!(html.contains("data-viewer-viewport"));
    assert!(html.contains("default-src 'none'"));
    let legend = json(&view.join("legend.json"));
    assert_eq!(legend["title"], TEXT);
    assert_eq!(legend["nodes"][0]["label"], TEXT);
    assert!(!legend["nodes"][0]["evidence"]
        .as_array()
        .unwrap()
        .is_empty());
    let before = artifacts(&f.run);
    assert_eq!(
        success(f.present().arg("--no-render").output().unwrap()),
        report
    );
    assert_eq!(artifacts(&f.run), before);
    assert_eq!(fs::read(f.run.join("state.json")).unwrap(), state);
    assert_eq!(fs::read(f.run.join("proposal.json")).unwrap(), proposal);
    assert_eq!(f.calls("agent.log"), 1);
    assert_eq!(f.calls("render.log"), 0);
    assert!(!f.run.join("presentations").exists());
    assert!(!f.run.join("draft.mmd").exists());
}

#[test]
fn rendering_replaces_source_bundle_without_history_or_agent_retry() {
    let Some(f) = Fixture::new() else { return };
    success(f.present().arg("--no-render").output().unwrap());
    let view = f.run.join(VIEW);
    let sources: Vec<_> = [
        "diagram.mmd",
        "legend.json",
        "index.html",
        "draft.architecture.json",
    ]
    .iter()
    .map(|name| (*name, fs::read(view.join(name)).unwrap()))
    .collect();
    let before = artifacts(&f.run);
    assert!(!f
        .present()
        .env("RENDER_FAIL", "1")
        .output()
        .unwrap()
        .status
        .success());
    assert_eq!(artifacts(&f.run), before);
    let report = success(f.present().output().unwrap());
    assert!(report["draftSvg"].is_string());
    for (name, bytes) in sources {
        assert_eq!(fs::read(view.join(name)).unwrap(), bytes);
    }
    assert_eq!(fs::read_dir(&view).unwrap().count(), 6);
    let before = artifacts(&f.run);
    success(f.present().arg("--no-render").output().unwrap());
    success(f.present().output().unwrap());
    assert_eq!(artifacts(&f.run), before);
    assert_eq!(f.calls("agent.log"), 1);
    assert_eq!(f.calls("render.log"), 2);
}

#[test]
fn first_renderer_failure_publishes_reviewable_source_bundle() {
    let Some(f) = Fixture::new() else { return };
    let output = f.present().env("RENDER_FAIL", "1").output().unwrap();
    assert!(!output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(Path::new(report["draftViewer"].as_str().unwrap()).is_file());
    assert!(report["draftSvg"].is_null());
    assert_eq!(json(&f.run.join("state.json"))["status"], "proposal_ready");
    success(f.present().output().unwrap());
    assert_eq!(f.calls("agent.log"), 1);
}

#[test]
fn corrupt_missing_unknown_and_symlink_artifacts_fail_without_repair_or_calls() {
    for name in [
        "receipt.json",
        "diagram.mmd",
        "legend.json",
        "index.html",
        "diagram.svg",
        "draft.architecture.json",
        "unknown.txt",
    ] {
        let Some(f) = Fixture::new() else { return };
        success(f.present().output().unwrap());
        fs::write(f.run.join(VIEW).join(name), "tampered").unwrap();
        let before = artifacts(&f.run);
        assert!(!f.present().output().unwrap().status.success());
        assert_eq!(artifacts(&f.run), before);
        assert_eq!(f.calls("agent.log"), 1);
        assert_eq!(f.calls("render.log"), 1);
    }
    let Some(f) = Fixture::new() else { return };
    success(f.present().output().unwrap());
    fs::remove_file(f.run.join(VIEW).join("diagram.svg")).unwrap();
    assert!(!f.present().output().unwrap().status.success());
    symlink(
        f.directory.join("renderer"),
        f.run.join(VIEW).join("diagram.svg"),
    )
    .unwrap();
    assert!(!f.present().output().unwrap().status.success());
    assert_eq!(f.calls("render.log"), 1);
}

#[test]
fn notes_added_while_renderer_is_blocked_survive_replacement_failure() {
    let Some(f) = Fixture::new() else { return };
    success(f.present().arg("--no-render").output().unwrap());
    let ready = f.directory.join("renderer.ready");
    let release = f.directory.join("renderer.release");
    let mut child = f
        .present()
        .env("PRESENT_RENDER_READY", &ready)
        .env("PRESENT_RENDER_RELEASE", &release)
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
    let notes = f.run.join(VIEW).join("notes.txt");
    fs::write(&notes, "manual notes added during rendering").unwrap();
    let before = artifacts(&f.run.join(VIEW));
    fs::write(&release, "").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(report["error"].as_str().unwrap().contains("reconciliation"));
    assert!(report["error"]
        .as_str()
        .unwrap()
        .contains("Storage changed"));
    assert_eq!(artifacts(&f.run.join(VIEW)), before);
    assert_eq!(
        fs::read_to_string(notes).unwrap(),
        "manual notes added during rendering"
    );
    let preserved: Vec<_> = fs::read_dir(&f.run)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".presentation-init-")
        })
        .collect();
    assert_eq!(preserved.len(), 1);
    assert!(report["error"]
        .as_str()
        .unwrap()
        .contains(&preserved[0].display().to_string()));
    assert!(preserved[0].join("diagram.svg").is_file());
    assert_eq!(f.calls("agent.log"), 1);
}

#[test]
fn external_workflow_lock_blocks_presentation() {
    let Some(f) = Fixture::new() else { return };
    fs::write(f.directory.join(".saved run.lock"), "").unwrap();
    assert!(!f.present().output().unwrap().status.success());
    assert!(!f.run.join(VIEW).exists());
    assert_eq!(f.calls("agent.log"), 1);
}
