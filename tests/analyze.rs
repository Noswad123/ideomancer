//! Offline black-box contracts for the single-command analysis workflow.
#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime};

use serde_json::Value;
use tempfile::{tempdir, TempDir};

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

fn success(output: &Output) {
    assert!(
        output.status.success(),
        "status: {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
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
                "import sys; assert sys.version_info[:2] == (3, 12); print(sys.executable)",
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
            "SKIP Python-dependent analyze test: Python 3.12 is unavailable; \
             install python3.12 or set IDEOMANCER_TEST_PYTHON to a trusted Python 3.12 executable"
        );
    }
    path.as_deref()
}

const AGENT: &str = r#"import json
import os
from pathlib import Path
import sys
import time

assert len(sys.argv) == 5, sys.argv
assert sys.argv[1:3] == ['execute', '--request'], sys.argv
assert sys.argv[4] == '--json', sys.argv
request_path = Path(sys.argv[3])
assert request_path.is_absolute()
request = json.loads(request_path.read_text())
root = Path(os.environ['ANALYZE_ROOT']).resolve()
assert not Path.cwd().resolve().is_relative_to(root), 'agent cwd is target source'
assert request['protocolVersion'] == 'agent-execution.v1'
assert isinstance(request['requestId'], str) and request['requestId'].strip()
assert isinstance(request['instructions'], str) and request['instructions'].strip()
assert isinstance(request['responseSchema'], dict) and request['responseSchema']
assert type(request['timeoutMs']) is int and 100 <= request['timeoutMs'] <= 600000
with open(os.environ['ANALYZE_AGENT_LOG'], 'a') as log:
    log.write(json.dumps({'request': request, 'requestPath': str(request_path)}) + '\n')
mode = os.environ.get('ANALYZE_AGENT_MODE', 'valid')
if mode == 'nonzero':
    sys.exit(7)
if mode == 'timeout':
    time.sleep(30)
if mode == 'indeterminate':
    print(json.dumps({
        'protocolVersion': 'agent-execution.v1',
        'requestId': request['requestId'],
        'status': 'failed',
        'error': 'Synthetic remote completion is uncertain',
        'errorKind': 'indeterminate',
    }))
    sys.exit(1)
evidence = request['input']
assert evidence['complete'] is True
fact = next(fact['id'] for fact in evidence['facts'] if fact['kind'] == 'module')
proposal = {
    'schemaVersion': 'ideomancer.architecture.v1',
    'snapshotId': evidence['snapshotId'],
    'name': os.environ.get('ANALYZE_PROPOSAL_NAME', 'Synthetic Architecture'),
    'components': [
        {'id': 'api', 'label': 'API', 'kind': 'component', 'evidenceIds': [fact],
         'rationale': 'The observed module provides an API.'},
        {'id': 'storage', 'label': 'Storage', 'kind': 'component', 'evidenceIds': [fact],
         'rationale': 'A synthetic grouping supported by module evidence.'},
    ],
    'relationships': [
        {'from': 'api', 'to': 'storage', 'label': 'uses', 'evidenceIds': [fact],
         'basis': 'inferred', 'rationale': 'Grouping inferred from the source module.'},
    ],
    'uncertainties': ['This is a synthetic grouping, not a runtime trace.'],
}
if mode == 'invalid':
    proposal['snapshotId'] = 'wrong-snapshot'
print(json.dumps({
    'protocolVersion': 'agent-execution.v1',
    'requestId': request['requestId'],
    'status': 'completed',
    'output': proposal,
    'model': 'actual-stub-model',
}))
"#;

const RENDERER: &str = r#"import json
import os
from pathlib import Path
import sys
import time

args = sys.argv[1:]
source = Path(args[args.index('-i') + 1]).read_text()
output = Path(args[args.index('-o') + 1])
with open(os.environ['ANALYZE_RENDER_LOG'], 'a') as log:
    log.write(json.dumps({'args': args, 'source': source}) + '\n')
if os.environ.get('ANALYZE_RENDER_READY'):
    Path(os.environ['ANALYZE_RENDER_READY']).touch()
    deadline = time.monotonic() + 10
    while not Path(os.environ['ANALYZE_RENDER_RELEASE']).exists():
        if time.monotonic() > deadline: sys.exit(8)
        time.sleep(0.01)
if os.environ.get('ANALYZE_RENDER_MODE') == 'fail':
    print('Synthetic renderer failure', file=sys.stderr)
    sys.exit(9)
output.write_text('<svg xmlns="http://www.w3.org/2000/svg" width="300" height="150"><text>DRAFT</text></svg>')
"#;

struct Fixture {
    _temp: TempDir,
    cwd: PathBuf,
    root: PathBuf,
    base: PathBuf,
    python: PathBuf,
    agent: PathBuf,
    renderer: PathBuf,
    agent_log: PathBuf,
    render_log: PathBuf,
    marker: PathBuf,
}

impl Fixture {
    fn new() -> Option<Self> {
        let python = python()?.to_path_buf();
        let temp = tempdir().unwrap();
        let directory = fs::canonicalize(temp.path()).unwrap();
        let cwd = directory.join("invocation directory");
        let root = directory.join("synthetic source project");
        fs::create_dir(&cwd).unwrap();
        fs::create_dir(&root).unwrap();
        let marker = directory.join("SOURCE_MUST_NOT_EXECUTE");
        fs::write(
            root.join("api.py"),
            format!(
                "from pathlib import Path\nPath({}).write_text('executed')\n\
                 import storage\ndef serve():\n    return storage.load()\n",
                serde_json::to_string(marker.to_str().unwrap()).unwrap()
            ),
        )
        .unwrap();
        fs::write(root.join("storage.py"), "def load():\n    return 42\n").unwrap();
        let executable = |name: &str, source: &str| {
            let script = directory.join(format!("{name}.py"));
            fs::write(&script, source).unwrap();
            let program = directory.join(format!("stub {name} ; literal"));
            let quote =
                |path: &Path| format!("'{}'", path.to_str().unwrap().replace('\'', "'\\''"));
            fs::write(
                &program,
                format!(
                    "#!/bin/sh\nexec {} -I -S {} \"$@\"\n",
                    quote(&python),
                    quote(&script)
                ),
            )
            .unwrap();
            fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
            program
        };
        Some(Self {
            agent: executable("agent", AGENT),
            renderer: executable("renderer", RENDERER),
            base: directory.join("new parents").join("analysis container"),
            agent_log: directory.join("agent calls.jsonl"),
            render_log: directory.join("renderer calls.jsonl"),
            _temp: temp,
            cwd,
            root,
            python,
            marker,
        })
    }

    fn command(&self) -> Command {
        let mut command = cli();
        command
            .current_dir(&self.cwd)
            .arg("analyze")
            .arg(&self.root)
            .env("ANALYZE_ROOT", &self.root)
            .env("ANALYZE_AGENT_LOG", &self.agent_log)
            .env("ANALYZE_RENDER_LOG", &self.render_log)
            .env("ANALYZE_AGENT_MODE", "valid")
            .env_remove("ANALYZE_PROPOSAL_NAME")
            .env("ANALYZE_RENDER_MODE", "valid");
        command
    }

    fn configured(&self) -> Command {
        let mut command = self.command();
        command
            .arg("--python")
            .arg(&self.python)
            .arg("--agent")
            .arg(&self.agent)
            .arg("--mmdc")
            .arg(&self.renderer);
        command
    }

    fn analyze(&self) -> Command {
        let mut command = self.configured();
        command.arg("--out").arg(&self.base);
        command
    }

    fn report(&self, output: &Output, ok: bool, status: &str) -> Value {
        if ok {
            success(output);
        } else {
            assert_eq!(
                output.status.code(),
                Some(1),
                "stdout: {}\nstderr: {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let report: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
            panic!(
                "expected ONE JSON report: {error}\nstdout: {}\nstderr: {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
        });
        let run = run_directory(&report);
        assert!(run.is_absolute());
        assert!(!run.starts_with(&self.root));
        let state = read_json(&run.join("state.json"));
        assert_eq!(report["status"], status);
        assert_eq!(report["status"], state["status"]);
        assert_eq!(report["snapshotId"], state["snapshotId"]);
        assert!(report["snapshotId"].as_str().is_some_and(|s| !s.is_empty()));
        assert!(report["requiresAcceptance"].is_boolean());
        let hash = report.get("proposalSha256").expect("proposalSha256 field");
        assert!(hash.is_null() || hash.as_str().is_some_and(|s| s.len() == 64));
        assert_eq!(*hash, state["proposalSha256"]);
        assert_eq!(
            report["requiresAcceptance"],
            status == "proposal_ready" && !hash.is_null()
        );
        for (key, name) in [
            ("draftManifest", "presentation/draft.architecture.json"),
            ("draftMermaid", "presentation/diagram.mmd"),
            ("draftSvg", "presentation/diagram.svg"),
            ("draftLegend", "presentation/legend.json"),
            ("draftViewer", "presentation/index.html"),
        ] {
            let path = report.get(key).unwrap_or_else(|| panic!("missing {key}"));
            if !path.is_null() {
                let path = Path::new(path.as_str().expect("absolute artifact path or null"));
                assert!(path.is_absolute());
                assert_eq!(path, run.join(name));
                assert!(path.is_file());
            }
        }
        assert!(!self.marker.exists(), "source module body was executed");
        report
    }

    fn unaccepted(&self, run: &Path) {
        assert!(!run.join("architecture.json").exists());
        assert!(!run.join("diagram.mmd").exists());
        assert!(!self.marker.exists());
    }

    fn accept(&self, report: &Value) {
        let output = cli()
            .arg("accept")
            .arg(run_directory(report))
            .arg("--proposal-sha256")
            .arg(report["proposalSha256"].as_str().unwrap())
            .output()
            .unwrap();
        success(&output);
        assert_eq!(
            read_json(&run_directory(report).join("state.json"))["status"],
            "accepted"
        );
    }
}

fn run_directory(report: &Value) -> PathBuf {
    PathBuf::from(
        report["runDirectory"]
            .as_str()
            .expect("runDirectory string"),
    )
}

fn calls(path: &Path) -> Vec<Value> {
    match fs::read_to_string(path) {
        Ok(log) => log
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => panic!("read invocation log: {error}"),
    }
}

fn artifacts(run: &Path) -> BTreeMap<PathBuf, (Vec<u8>, SystemTime)> {
    let mut result = BTreeMap::new();
    let mut pending = vec![run.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                result.insert(
                    path.strip_prefix(run).unwrap().to_path_buf(),
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

fn assert_draft(report: &Value) {
    let run = run_directory(report);
    assert_eq!(
        report["draftManifest"],
        run.join("presentation/draft.architecture.json")
            .to_str()
            .unwrap()
    );
    assert_eq!(
        report["draftMermaid"],
        run.join("presentation/diagram.mmd").to_str().unwrap()
    );
    assert_eq!(report["presentationVersion"], "readable-v2");
    for field in ["draftLegend", "draftViewer"] {
        assert!(Path::new(report[field].as_str().unwrap()).is_file());
    }
    let manifest = read_json(&run.join("presentation/draft.architecture.json"));
    assert!(manifest["name"].as_str().unwrap().starts_with("DRAFT"));
    success(
        &cli()
            .arg("validate")
            .arg(run.join("presentation/draft.architecture.json"))
            .output()
            .unwrap(),
    );
    let mermaid = fs::read_to_string(report["draftMermaid"].as_str().unwrap()).unwrap();
    let frontmatter = mermaid
        .strip_prefix("---\n")
        .expect("Mermaid YAML frontmatter");
    let (yaml, body) = frontmatter
        .split_once("\n---\n")
        .expect("closed frontmatter");
    let metadata: Value = serde_yaml::from_str(yaml).unwrap();
    assert!(metadata["title"].as_str().unwrap().contains("DRAFT"));
    assert!(body.contains("flowchart"));
    if report["status"] == "accepted" {
        assert!(!mermaid.contains("UNACCEPTED"));
        assert!(!manifest["name"].as_str().unwrap().contains("UNACCEPTED"));
    }
}

#[test]
fn one_command_renders_reviewable_drafts_and_reuses_them_before_and_after_acceptance() {
    let Some(f) = Fixture::new() else { return };
    assert!(!f.base.exists());
    let ready = f.report(&f.analyze().output().unwrap(), true, "proposal_ready");
    let run = run_directory(&ready);
    assert!(run.starts_with(&f.base));
    assert_ne!(
        run, f.base,
        "--out is a container, not the exact run directory"
    );
    assert_eq!(ready["requiresAcceptance"], true);
    assert!(ready["proposalSha256"].is_string());
    assert_draft(&ready);
    f.unaccepted(&run);
    assert_eq!(
        ready["draftSvg"],
        run.join("presentation/diagram.svg").to_str().unwrap()
    );
    assert!(fs::read_to_string(ready["draftSvg"].as_str().unwrap())
        .unwrap()
        .contains("DRAFT"));
    assert!(!run.join("draft.svg").exists(), "rendered a new legacy SVG");
    let agents = calls(&f.agent_log);
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0]["request"]["timeoutMs"], 299000);
    assert_eq!(agents[0]["request"]["model"], "openai/gpt-6.1-sol");
    assert_eq!(
        agents[0]["request"]["input"],
        read_json(&run.join("evidence.json"))
    );
    assert_eq!(
        Path::new(agents[0]["requestPath"].as_str().unwrap()).parent(),
        Some(run.as_path())
    );
    let renders = calls(&f.render_log);
    assert_eq!(renders.len(), 1);
    assert_eq!(
        renders[0]["source"],
        fs::read_to_string(ready["draftMermaid"].as_str().unwrap()).unwrap()
    );
    let before = artifacts(&run);
    assert_eq!(
        f.report(
            &f.analyze()
                .args(["--timeout-seconds", "600"])
                .output()
                .unwrap(),
            true,
            "proposal_ready"
        ),
        ready
    );
    assert_eq!(
        artifacts(&run),
        before,
        "repeat rewrote artifact bytes or mtimes"
    );
    assert_eq!(calls(&f.agent_log).len(), 1);
    assert_eq!(calls(&f.render_log).len(), 1);

    f.accept(&ready);
    assert!(run.join("architecture.json").is_file());
    assert!(run.join("diagram.mmd").is_file());
    let accepted = artifacts(&run);
    let report = f.report(
        &f.analyze()
            .args(["--timeout-seconds", "1"])
            .output()
            .unwrap(),
        true,
        "accepted",
    );
    assert_eq!(run_directory(&report), run);
    assert_eq!(report["requiresAcceptance"], false);
    assert_eq!(report["proposalSha256"], ready["proposalSha256"]);
    assert_draft(&report);
    assert_eq!(
        artifacts(&run),
        accepted,
        "accepted analysis must not write artifacts"
    );
    assert_eq!(calls(&f.agent_log).len(), 1);
    assert_eq!(calls(&f.render_log).len(), 1);
}

#[test]
fn no_render_drafts_without_a_browser_and_can_render_the_cached_proposal_later() {
    let Some(f) = Fixture::new() else { return };
    let ready = f.report(
        &f.analyze().arg("--no-render").output().unwrap(),
        true,
        "proposal_ready",
    );
    let run = run_directory(&ready);
    assert_draft(&ready);
    assert_eq!(ready["requiresAcceptance"], true);
    assert!(ready["draftSvg"].is_null());
    assert!(!run.join("draft.svg").exists());
    assert!(!run.join("presentation/diagram.svg").exists());
    assert_eq!(calls(&f.agent_log).len(), 1);
    assert!(calls(&f.render_log).is_empty());
    f.unaccepted(&run);
    let before = artifacts(&run);
    assert_eq!(
        f.report(
            &f.analyze().arg("--no-render").output().unwrap(),
            true,
            "proposal_ready"
        ),
        ready
    );
    assert_eq!(artifacts(&run), before);
    assert_eq!(calls(&f.agent_log).len(), 1);
    assert!(calls(&f.render_log).is_empty());
    let rendered = f.report(&f.analyze().output().unwrap(), true, "proposal_ready");
    assert!(rendered["draftSvg"].is_string());
    assert_eq!(calls(&f.agent_log).len(), 1);
    assert_eq!(calls(&f.render_log).len(), 1);
}

#[test]
fn extract_only_never_invokes_agent_or_renderer_and_is_reusable() {
    let Some(f) = Fixture::new() else { return };
    let extracted = f.report(
        &f.analyze().arg("--extract-only").output().unwrap(),
        true,
        "extracted",
    );
    let run = run_directory(&extracted);
    assert!(extracted["proposalSha256"].is_null());
    assert_eq!(extracted["requiresAcceptance"], false);
    for field in ["draftManifest", "draftMermaid", "draftSvg"] {
        assert!(extracted[field].is_null(), "{field}");
    }
    for name in ["evidence.json", "modules.json", "modules.mmd", "state.json"] {
        assert!(run.join(name).is_file(), "missing {name}");
    }
    for name in [
        "proposal.json",
        "draft.architecture.json",
        "draft.mmd",
        "draft.svg",
    ] {
        assert!(!run.join(name).exists(), "unexpected {name}");
    }
    assert!(calls(&f.agent_log).is_empty());
    assert!(calls(&f.render_log).is_empty());
    f.unaccepted(&run);
    let before = artifacts(&run);
    assert_eq!(
        f.report(
            &f.analyze().arg("--extract-only").output().unwrap(),
            true,
            "extracted"
        ),
        extracted
    );
    assert_eq!(artifacts(&run), before);
    assert!(calls(&f.agent_log).is_empty());
    assert!(calls(&f.render_log).is_empty());
    let ready = f.report(
        &f.analyze().arg("--no-render").output().unwrap(),
        true,
        "proposal_ready",
    );
    assert_eq!(run_directory(&ready), run);
    assert_draft(&ready);
    assert_eq!(calls(&f.agent_log).len(), 1);
}

#[test]
fn changed_source_replaces_current_run_and_resets_acceptance() {
    let Some(f) = Fixture::new() else { return };
    let first = f.report(&f.analyze().output().unwrap(), true, "proposal_ready");
    f.accept(&first);
    let old_run = run_directory(&first);
    fs::write(f.root.join("storage.py"), "def load():\n    return 43\n").unwrap();
    let changed = f.report(&f.analyze().output().unwrap(), true, "proposal_ready");
    let new_run = run_directory(&changed);
    assert_eq!(new_run, old_run);
    assert!(new_run.starts_with(&f.base));
    assert_ne!(changed["snapshotId"], first["snapshotId"]);
    assert_eq!(changed["requiresAcceptance"], true);
    assert_draft(&changed);
    f.unaccepted(&new_run);
    assert_eq!(fs::read_dir(&f.base).unwrap().count(), 1);
    assert_eq!(new_run.file_name().unwrap(), "synthetic-source-project");
    assert_eq!(calls(&f.agent_log).len(), 2);
    assert_eq!(calls(&f.render_log).len(), 2);
}

#[test]
fn default_output_is_under_invocation_cwd_not_the_source_root() {
    let Some(f) = Fixture::new() else { return };
    let report = f.report(
        &f.configured().arg("--extract-only").output().unwrap(),
        true,
        "extracted",
    );
    let base = f.cwd.join(".ideomancer/analyses");
    let run = run_directory(&report);
    assert!(run.starts_with(&base));
    assert_ne!(run, base);
    assert!(!f.root.join(".ideomancer").exists());
    assert!(calls(&f.agent_log).is_empty());
    assert!(calls(&f.render_log).is_empty());
}

#[test]
fn analyze_reads_scoped_environment_configuration_without_program_or_model_flags() {
    let Some(f) = Fixture::new() else { return };
    let report = f.report(
        &f.command()
            .arg("--out")
            .arg(&f.base)
            .env("IDEOMANCER_AGENT", &f.agent)
            .env("IDEOMANCER_MODEL", "environment-stub-model")
            .env("IDEOMANCER_PYTHON", &f.python)
            .env("IDEOMANCER_MMDC", &f.renderer)
            .output()
            .unwrap(),
        true,
        "proposal_ready",
    );
    assert_draft(&report);
    assert!(report["draftSvg"].is_string());
    assert_eq!(calls(&f.agent_log).len(), 1);
    assert_eq!(
        calls(&f.agent_log)[0]["request"]["model"],
        "environment-stub-model"
    );
    assert_eq!(calls(&f.render_log).len(), 1);
    f.unaccepted(&run_directory(&report));
}

#[test]
fn explicit_model_overrides_environment_and_default() {
    let Some(f) = Fixture::new() else { return };
    f.report(
        &f.analyze()
            .env("IDEOMANCER_MODEL", "environment-model")
            .args(["--model", "explicit-model", "--no-render"])
            .output()
            .unwrap(),
        true,
        "proposal_ready",
    );
    assert_eq!(calls(&f.agent_log)[0]["request"]["model"], "explicit-model");
}

#[test]
fn explicit_deadlines_reach_the_agent_without_waiting_for_them() {
    for (seconds, budget) in [("600", 599000), ("3", 2700), ("1", 900)] {
        let Some(f) = Fixture::new() else { return };
        let output = f
            .analyze()
            .args(["--timeout-seconds", seconds, "--no-render"])
            .output()
            .unwrap();
        // Process startup can exhaust short deadlines on a loaded test host.
        let completed = output.status.success();
        assert!(completed || seconds != "600");
        let ready = f.report(
            &output,
            completed,
            if completed {
                "proposal_ready"
            } else {
                "indeterminate"
            },
        );
        let calls = calls(&f.agent_log);
        let run = run_directory(&ready);
        let request = read_json(&run.join("attempt-0001.request.json"));
        assert_eq!(request["timeoutMs"], budget);
        if completed {
            assert_eq!(calls.len(), 1);
        } else {
            assert!(calls.len() <= 1);
        }
        for call in calls {
            assert_eq!(call["request"], request);
        }
        let state = read_json(&run.join("state.json"));
        let receipt = read_json(&run.join("attempt-0001.result.json"));
        assert_eq!(state["agentTimeoutMs"], budget);
        assert_eq!(receipt["timeoutMs"], budget);
        assert!(state["elapsedMs"].as_u64().is_some());
        assert_eq!(receipt["elapsedMs"], state["elapsedMs"]);
    }
}

#[test]
fn renderer_failure_keeps_reviewable_sources_and_recovers_without_retrying_the_agent() {
    let Some(f) = Fixture::new() else { return };
    let failed = f.report(
        &f.analyze()
            .env("ANALYZE_RENDER_MODE", "fail")
            .output()
            .unwrap(),
        false,
        "proposal_ready",
    );
    let run = run_directory(&failed);
    assert_draft(&failed);
    assert_eq!(failed["requiresAcceptance"], true);
    assert!(failed["draftSvg"].is_null());
    assert!(!run.join("draft.svg").exists());
    assert!(!run.join("presentation/diagram.svg").exists());
    f.unaccepted(&run);
    let before = artifacts(&run);
    let recovered = f.report(&f.analyze().output().unwrap(), true, "proposal_ready");
    assert_eq!(run_directory(&recovered), run);
    assert_eq!(recovered["proposalSha256"], failed["proposalSha256"]);
    assert!(recovered["draftSvg"].is_string());
    let after = artifacts(&run);
    for name in [
        "proposal.json",
        "presentation/draft.architecture.json",
        "presentation/diagram.mmd",
        "presentation/legend.json",
        "presentation/index.html",
    ] {
        assert_eq!(
            after[Path::new(name)].0,
            before[Path::new(name)].0,
            "recovery changed {name}"
        );
    }
    assert_eq!(calls(&f.agent_log).len(), 1);
    assert_eq!(calls(&f.render_log).len(), 2);
    f.unaccepted(&run);
}

#[test]
fn failed_and_indeterminate_agents_require_explicit_retry() {
    for (mode, status) in [
        ("nonzero", "failed"),
        ("invalid", "failed"),
        ("indeterminate", "indeterminate"),
    ] {
        let Some(f) = Fixture::new() else { return };
        let failed = f.report(
            &f.analyze()
                .env("ANALYZE_AGENT_MODE", mode)
                .output()
                .unwrap(),
            false,
            status,
        );
        let run = run_directory(&failed);
        assert_eq!(failed["requiresAcceptance"], false);
        f.unaccepted(&run);
        for field in [
            "proposalSha256",
            "draftManifest",
            "draftMermaid",
            "draftSvg",
        ] {
            assert!(failed[field].is_null(), "{mode}: unexpected {field}");
        }
        assert_eq!(calls(&f.agent_log).len(), 1);
        assert_eq!(calls(&f.agent_log)[0]["request"]["timeoutMs"], 299000);
        assert!(calls(&f.render_log).is_empty());
        let before = artifacts(&run);
        f.report(&f.analyze().output().unwrap(), false, status);
        f.report(
            &f.analyze()
                .args(["--timeout-seconds", "600"])
                .output()
                .unwrap(),
            false,
            status,
        );
        assert_eq!(calls(&f.agent_log).len(), 1, "implicit retry for {mode}");
        assert_eq!(artifacts(&run), before);
        let recovered = f.report(
            &f.analyze()
                .args(["--retry", "--timeout-seconds", "600"])
                .output()
                .unwrap(),
            true,
            "proposal_ready",
        );
        assert_eq!(run_directory(&recovered), run);
        assert_draft(&recovered);
        assert_eq!(calls(&f.agent_log).len(), 2);
        let agents = calls(&f.agent_log);
        assert_eq!(agents[1]["request"]["timeoutMs"], 599000);
        assert_ne!(
            agents[0]["request"]["requestId"],
            agents[1]["request"]["requestId"]
        );
        let after = artifacts(&run);
        for file in ["attempt-0001.request.json", "attempt-0001.result.json"] {
            assert_eq!(after[Path::new(file)], before[Path::new(file)]);
        }
        assert_eq!(calls(&f.render_log).len(), 1);
        f.unaccepted(&run);
    }
}

#[test]
fn analyze_enforces_the_agent_timeout_without_implicitly_retrying() {
    let Some(f) = Fixture::new() else { return };
    let start = Instant::now();
    let failed = f.report(
        &f.analyze()
            .args(["--timeout-seconds", "3"])
            .env("ANALYZE_AGENT_MODE", "timeout")
            .output()
            .unwrap(),
        false,
        "indeterminate",
    );
    assert_eq!(failed["requiresAcceptance"], false);
    assert!(
        start.elapsed() < Duration::from_secs(10),
        "agent deadline was not enforced"
    );
    f.report(&f.analyze().output().unwrap(), false, "indeterminate");
    assert_eq!(calls(&f.agent_log).len(), 1);
    assert!(calls(&f.render_log).is_empty());
    f.unaccepted(&run_directory(&failed));
}

#[test]
fn removing_a_source_symlink_replaces_current_incomplete_evidence() {
    let Some(f) = Fixture::new() else { return };
    let link = f.root.join("linked.py");
    symlink(f.root.join("storage.py"), &link).unwrap();
    let incomplete = f.report(&f.analyze().output().unwrap(), false, "extracted");
    assert_eq!(incomplete["requiresAcceptance"], false);
    let old_run = run_directory(&incomplete);
    assert!(incomplete["error"].as_str().is_some_and(|s| !s.is_empty()));
    let evidence = read_json(&old_run.join("evidence.json"));
    assert_eq!(evidence["complete"], false);
    assert_eq!(read_json(&old_run.join("state.json"))["complete"], false);
    assert!(!evidence["diagnostics"].as_array().unwrap().is_empty());
    for field in [
        "proposalSha256",
        "draftManifest",
        "draftMermaid",
        "draftSvg",
    ] {
        assert!(incomplete[field].is_null(), "unexpected {field}");
    }
    assert!(calls(&f.agent_log).is_empty());
    assert!(calls(&f.render_log).is_empty());
    f.unaccepted(&old_run);

    fs::remove_file(link).unwrap();
    let complete = f.report(&f.analyze().output().unwrap(), true, "proposal_ready");
    let new_run = run_directory(&complete);
    assert_eq!(complete["snapshotId"], incomplete["snapshotId"]);
    assert_eq!(
        new_run, old_run,
        "full evidence changes replace the current slot"
    );
    assert!(new_run.starts_with(&f.base));
    assert_eq!(read_json(&new_run.join("evidence.json"))["complete"], true);
    assert_eq!(fs::read_dir(&f.base).unwrap().count(), 1);
    assert_draft(&complete);
    f.unaccepted(&new_run);
    assert_eq!(calls(&f.agent_log).len(), 1);
    assert_eq!(calls(&f.render_log).len(), 1);
}

#[test]
fn changed_inputs_never_erase_or_retry_failed_running_or_indeterminate_work() {
    for mode in ["nonzero", "indeterminate", "running"] {
        let Some(f) = Fixture::new() else { return };
        let failed = f.report(
            &f.analyze()
                .env(
                    "ANALYZE_AGENT_MODE",
                    if mode == "running" {
                        "indeterminate"
                    } else {
                        mode
                    },
                )
                .output()
                .unwrap(),
            false,
            if mode == "nonzero" {
                "failed"
            } else {
                "indeterminate"
            },
        );
        let run = run_directory(&failed);
        if mode == "running" {
            let mut state = read_json(&run.join("state.json"));
            state["status"] = serde_json::json!("running");
            fs::write(
                run.join("state.json"),
                serde_json::to_vec_pretty(&state).unwrap(),
            )
            .unwrap();
        }
        let before = artifacts(&run);
        for flags in [
            vec!["--model", "changed"],
            vec!["--focus", "api.serve"],
            vec!["--model", "changed", "--retry"],
            vec!["--model", "changed", "--extract-only"],
        ] {
            let output = f.analyze().args(flags).output().unwrap();
            assert!(!output.status.success());
            assert!(String::from_utf8_lossy(&output.stderr).contains("Reconcile"));
            assert_eq!(artifacts(&run), before);
        }
        fs::write(f.root.join("storage.py"), "def load():\n    return 99\n").unwrap();
        assert!(!f.analyze().output().unwrap().status.success());
        assert_eq!(artifacts(&run), before);
        assert_eq!(calls(&f.agent_log).len(), 1);
    }
}

#[test]
fn analyze_preserves_presentation_notes_added_during_rendering_without_agent_retry() {
    let Some(f) = Fixture::new() else { return };
    let ready = f.report(
        &f.analyze().arg("--no-render").output().unwrap(),
        true,
        "proposal_ready",
    );
    let run = run_directory(&ready);
    let marker = f.cwd.join("renderer.ready");
    let release = f.cwd.join("renderer.release");
    let mut child = f
        .analyze()
        .env("ANALYZE_RENDER_READY", &marker)
        .env("ANALYZE_RENDER_RELEASE", &release)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let start = Instant::now();
    while !marker.exists() {
        assert!(start.elapsed() < Duration::from_secs(10));
        assert!(child.try_wait().unwrap().is_none());
        std::thread::sleep(Duration::from_millis(10));
    }
    let presentation = run.join("presentation");
    fs::write(presentation.join("notes.txt"), "manual analysis notes").unwrap();
    let before = artifacts(&presentation);
    fs::write(release, "").unwrap();
    let failed = f.report(&child.wait_with_output().unwrap(), false, "proposal_ready");
    assert!(failed["error"].as_str().unwrap().contains("reconciliation"));
    assert_eq!(artifacts(&presentation), before);
    assert_eq!(calls(&f.agent_log).len(), 1);
    let preserved: Vec<_> = fs::read_dir(&run)
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
    assert!(failed["error"]
        .as_str()
        .unwrap()
        .contains(&preserved[0].display().to_string()));
    assert!(preserved[0].join("diagram.svg").is_file());
}

#[test]
fn analysis_holds_the_slot_lock_while_python_has_an_older_snapshot() {
    let Some(f) = Fixture::new() else { return };
    let ready = f.cwd.join("python.ready");
    let release = f.cwd.join("python.release");
    let gated = f.cwd.join("gated-python");
    fs::write(&gated, format!("#!{}\nimport subprocess,sys,time\nfrom pathlib import Path\nresponse=subprocess.run([{},*sys.argv[1:]],input=sys.stdin.buffer.read(),capture_output=True)\nPath({}).touch()\ndeadline=time.monotonic()+10\nwhile not Path({}).exists():\n    if time.monotonic()>deadline: sys.exit(8)\n    time.sleep(0.01)\nsys.stdout.buffer.write(response.stdout)\nsys.stderr.buffer.write(response.stderr)\nsys.exit(response.returncode)\n", f.python.display(), serde_json::to_string(&f.python).unwrap(), serde_json::to_string(&ready).unwrap(), serde_json::to_string(&release).unwrap())).unwrap();
    fs::set_permissions(&gated, fs::Permissions::from_mode(0o755)).unwrap();
    let mut child = f
        .command()
        .arg("--out")
        .arg(&f.base)
        .arg("--python")
        .arg(&gated)
        .arg("--agent")
        .arg(&f.agent)
        .arg("--extract-only")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let start = Instant::now();
    while !ready.exists() {
        assert!(start.elapsed() < Duration::from_secs(10));
        assert!(child.try_wait().unwrap().is_none());
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(f.base.join(".synthetic-source-project.lock").is_file());
    fs::write(f.root.join("storage.py"), "def load():\n    return 99\n").unwrap();
    let competing = f.analyze().arg("--extract-only").output().unwrap();
    assert!(!competing.status.success());
    assert!(String::from_utf8_lossy(&competing.stderr).contains("locked"));
    fs::write(&release, "").unwrap();
    let older = f.report(&child.wait_with_output().unwrap(), true, "extracted");
    let newer = f.report(
        &f.analyze().arg("--extract-only").output().unwrap(),
        true,
        "extracted",
    );
    assert_ne!(older["snapshotId"], newer["snapshotId"]);
    assert_eq!(run_directory(&older), run_directory(&newer));
    assert_eq!(
        read_json(&run_directory(&newer).join("state.json"))["snapshotId"],
        newer["snapshotId"]
    );
    assert!(calls(&f.agent_log).is_empty());
}

#[test]
fn external_slot_lock_is_shared_by_analyze_and_direct_workflow_commands() {
    let Some(f) = Fixture::new() else { return };
    let extracted = f.report(
        &f.analyze().arg("--extract-only").output().unwrap(),
        true,
        "extracted",
    );
    let run = run_directory(&extracted);
    let lock = f.base.join(".synthetic-source-project.lock");
    fs::write(&lock, "operator-owned lock").unwrap();
    let before = artifacts(&run);
    assert!(!f.analyze().output().unwrap().status.success());
    for command in ["status", "interpret", "present", "accept"] {
        let mut command_line = cli();
        command_line.arg(command).arg(&run);
        if command == "accept" {
            command_line.args(["--proposal-sha256", &"0".repeat(64)]);
        }
        let output = command_line.output().unwrap();
        assert!(!output.status.success(), "{command}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("locked"),
            "{command}: {output:?}"
        );
    }
    assert_eq!(fs::read_to_string(lock).unwrap(), "operator-owned lock");
    assert_eq!(artifacts(&run), before);
    assert!(calls(&f.agent_log).is_empty());
}

#[test]
fn legacy_hash_storage_requires_explicit_reconciliation_before_new_agent_work() {
    let Some(f) = Fixture::new() else { return };
    fs::create_dir_all(&f.base).unwrap();
    let old = f.base.join("a".repeat(64));
    fs::create_dir(&old).unwrap();
    fs::write(old.join("state.json"), "old uncertain state").unwrap();
    let before = artifacts(&f.base);
    let output = f.analyze().output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("reconcile"));
    assert_eq!(artifacts(&f.base), before);
    assert!(calls(&f.agent_log).is_empty());
}

#[test]
fn unknown_corrupt_and_colliding_analysis_slots_are_not_overwritten() {
    let Some(f) = Fixture::new() else { return };
    let extracted = f.report(
        &f.analyze().arg("--extract-only").output().unwrap(),
        true,
        "extracted",
    );
    let run = run_directory(&extracted);
    fs::write(run.join("notes.txt"), "not managed").unwrap();
    let before = artifacts(&run);
    assert!(!f
        .analyze()
        .args(["--model", "changed"])
        .output()
        .unwrap()
        .status
        .success());
    assert_eq!(artifacts(&run), before);
    fs::remove_file(run.join("notes.txt")).unwrap();
    let other_parent = f.cwd.join("other");
    fs::create_dir(&other_parent).unwrap();
    let other = other_parent.join(f.root.file_name().unwrap());
    fs::create_dir(&other).unwrap();
    fs::write(other.join("app.py"), "def other():\n    pass\n").unwrap();
    let before = artifacts(&run);
    let output = cli()
        .arg("analyze")
        .arg(other)
        .arg("--out")
        .arg(&f.base)
        .arg("--python")
        .arg(&f.python)
        .arg("--extract-only")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("collides"));
    assert_eq!(artifacts(&run), before);
    fs::write(run.join("identity.json"), "{}").unwrap();
    let before = artifacts(&run);
    assert!(!f.analyze().output().unwrap().status.success());
    assert_eq!(artifacts(&run), before);
    assert!(calls(&f.agent_log).is_empty());
}

#[test]
fn symlinked_default_storage_and_explicit_output_ancestors_are_rejected_without_writes() {
    for default_output in [true, false] {
        let Some(f) = Fixture::new() else { return };
        let outside = f.cwd.parent().unwrap().join("outside output storage");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("sentinel.txt"), "must remain untouched\n").unwrap();
        let before = artifacts(&outside);
        let link = f.cwd.join(if default_output {
            ".ideomancer"
        } else {
            "redirected parent"
        });
        symlink(&outside, &link).unwrap();
        let mut command = f.configured();
        if !default_output {
            command.arg("--out").arg(link.join("new child/analyses"));
        }
        let output = command.output().unwrap();
        assert_eq!(
            output.status.code(),
            Some(1),
            "stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stderr).contains("symlink"));
        assert_eq!(artifacts(&outside), before);
        assert_eq!(
            fs::read_dir(&outside).unwrap().count(),
            1,
            "created output directories through a symlink"
        );
        assert!(calls(&f.agent_log).is_empty());
        assert!(calls(&f.render_log).is_empty());
        assert!(!f.marker.exists());
    }
}

#[test]
#[ignore = "requires installed Mermaid CLI and its browser; uses only synthetic source and agent"]
fn real_mermaid_renders_a_visible_draft_and_keeps_unsafe_title_text_out_of_configuration() {
    let Some(mut f) = Fixture::new() else { return };
    f.renderer = PathBuf::from("mmdc");
    let name = "Synthetic \"quoted\" title\n---\nconfig:\n  theme: forest\n---\n%%{init: {'theme': 'dark'}}%%";
    let report = f.report(
        &f.analyze()
            .env("ANALYZE_PROPOSAL_NAME", name)
            .output()
            .unwrap(),
        true,
        "proposal_ready",
    );
    let run = run_directory(&report);
    assert_draft(&report);
    let manifest = read_json(&run.join("presentation/draft.architecture.json"));
    let mermaid = fs::read_to_string(report["draftMermaid"].as_str().unwrap()).unwrap();
    let (yaml, body) = mermaid
        .strip_prefix("---\n")
        .unwrap()
        .split_once("\n---\n")
        .unwrap();
    let metadata: Value = serde_yaml::from_str(yaml).unwrap();
    assert_ne!(metadata["config"]["theme"], "forest");
    assert_ne!(metadata["config"]["theme"], "dark");
    assert_eq!(metadata["config"]["securityLevel"], "strict");
    assert!(metadata["title"].as_str().unwrap().contains("DRAFT"));
    assert!(manifest["name"].as_str().unwrap().contains(name));
    assert!(!body.contains("config:"));
    assert!(!body.contains("%%{init:"));
    let svg = fs::read_to_string(report["draftSvg"].as_str().unwrap()).unwrap();
    assert!(svg.contains("<svg"));
    assert!(svg.contains("DRAFT"));
    assert!(
        svg.split('>')
            .filter_map(|part| part.split_once('<'))
            .any(|(text, _)| text.contains("DRAFT")),
        "DRAFT must appear in visible SVG text"
    );
    assert_eq!(calls(&f.agent_log).len(), 1);
    f.unaccepted(&run);
}
