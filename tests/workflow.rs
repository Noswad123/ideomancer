//! Black-box workflow contract tests. All source and agent fixtures are synthetic.
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
    Command::new(env!("CARGO_BIN_EXE_ideomancer"))
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

fn failure(output: &Output) {
    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn json_success(output: &Output) -> Value {
    success(output);
    serde_json::from_slice(&output.stdout).expect("one JSON status object on stdout")
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
            "SKIP Python-dependent workflow test: Python 3.12 is unavailable; \
             install python3.12 or set IDEOMANCER_TEST_PYTHON to a trusted Python 3.12 executable"
        );
    }
    path.as_deref()
}

// Invoked directly as an executable, never through a CLI-supplied shell command.
// Environment is scoped to each child process, so tests can run in parallel.
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
assert request_path.parent.resolve() == Path(os.environ['WORKFLOW_RUN']).resolve()
assert request_path.name.startswith('attempt-') and request_path.name.endswith('.request.json')
if os.environ.get('WORKFLOW_MODE') == 'corrupt_convenience_request':
    (Path(os.environ['WORKFLOW_RUN']) / 'request.json').write_text('{not JSON')
request = json.loads(request_path.read_text())
with open(os.environ['WORKFLOW_LOG'], 'a') as log:
    log.write(json.dumps({'request': request, 'cwd': os.getcwd(), 'args': sys.argv[1:]}) + '\n')
root = Path(os.environ['WORKFLOW_ROOT']).resolve()
assert not Path.cwd().resolve().is_relative_to(root), 'agent cwd is target source'
assert request['protocolVersion'] == 'agent-execution.v1'
assert isinstance(request['requestId'], str) and request['requestId'].strip()
assert isinstance(request['instructions'], str) and request['instructions'].strip()
assert isinstance(request['responseSchema'], dict) and request['responseSchema']
assert type(request['timeoutMs']) is int and 100 <= request['timeoutMs'] <= 600000
evidence = request['input']
assert evidence['complete'] is True
fact = next(fact['id'] for fact in evidence['facts'] if fact['kind'] == 'module')
proposal = {
    'schemaVersion': 'ideomancer.architecture.v1',
    'snapshotId': evidence['snapshotId'],
    'name': 'Synthetic Architecture',
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
mode = os.environ.get('WORKFLOW_MODE', 'valid')
if mode in ('configuration_failure', 'indeterminate_failure', 'unsafe_failure'):
    error = 'structured execution requires OPENAI_API_KEY'
    kind = 'configuration'
    if mode == 'indeterminate_failure':
        error = 'execution deadline expired; remote completion is uncertain'
        kind = 'indeterminate'
    elif mode == 'unsafe_failure':
        error += '\n\r\t\x00\x1b' + 'x' * 8192 + 'UNBOUNDED_DIAGNOSTIC_TAIL'
    print(json.dumps({
        'protocolVersion': 'agent-execution.v1',
        'requestId': request['requestId'],
        'status': 'failed',
        'error': error,
        'errorKind': kind,
    }))
    sys.exit(1)
elif mode == 'noisy_timeout':
    for _ in range(256):
        sys.stderr.write('x' * 8192)
        sys.stdout.write('x' * 8192)
    sys.stderr.flush()
    sys.stdout.flush()
    time.sleep(60)
elif mode == 'timeout':
    time.sleep(60)
elif mode == 'nonzero':
    print('deliberate agent failure', file=sys.stderr)
    sys.exit(7)
elif mode == 'bad_snapshot':
    proposal['snapshotId'] = 'not-the-input-snapshot'
elif mode == 'bad_component_evidence':
    proposal['components'][0]['evidenceIds'] = ['nonexistent-evidence']
elif mode == 'bad_edge_evidence':
    proposal['relationships'][0]['evidenceIds'] = ['nonexistent-evidence']
elif mode == 'bad_edge_ref':
    proposal['relationships'][0]['to'] = 'absent'
elif mode == 'unknown_proposal':
    proposal['surprise'] = True
elif mode == 'unknown_component':
    proposal['components'][0]['surprise'] = True
elif mode == 'unknown_relationship':
    proposal['relationships'][0]['surprise'] = True
elif mode == 'empty_components':
    proposal['components'] = []
    proposal['relationships'] = []
elif mode == 'duplicate_id':
    proposal['components'][1]['id'] = 'api'
elif mode == 'unsafe_id':
    proposal['components'][0]['id'] = 'not a mermaid id'
elif mode == 'empty_component_evidence':
    proposal['components'][0]['evidenceIds'] = []
elif mode == 'empty_edge_evidence':
    proposal['relationships'][0]['evidenceIds'] = []
elif mode == 'blank_component_rationale':
    proposal['components'][0]['rationale'] = '  '
elif mode == 'blank_edge_rationale':
    proposal['relationships'][0]['rationale'] = '  '
elif mode == 'bad_basis':
    proposal['relationships'][0]['basis'] = 'invented'
elif mode == 'bad_schema':
    proposal['schemaVersion'] = 'future-version'
elif mode == 'schema_invalid':
    proposal['components'] = 'not an array'
response = {
    'protocolVersion': 'agent-execution.v1',
    'requestId': request['requestId'],
    'status': 'completed',
    'output': proposal,
    'model': 'actual-stub-model',
}
if mode == 'bad_request_id':
    response['requestId'] = 'wrong-request'
elif mode == 'bad_protocol':
    response['protocolVersion'] = 'unsupported'
elif mode == 'failed_envelope':
    response['status'] = 'failed'
elif mode == 'missing_output':
    del response['output']
elif mode == 'fenced':
    print('```json\n' + json.dumps(response) + '\n```')
    sys.exit(0)
elif mode == 'invalid_json':
    print('{not JSON')
    sys.exit(0)
print(json.dumps(response))
"#;

struct Fixture {
    _temp: TempDir,
    root: PathBuf,
    run: PathBuf,
    agent: PathBuf,
    log: PathBuf,
    marker: PathBuf,
    python: PathBuf,
}

impl Fixture {
    fn new() -> Option<Self> {
        let python = python()?.to_path_buf();
        let temp = tempdir().unwrap();
        let root = temp.path().join("synthetic project");
        fs::create_dir(&root).unwrap();
        let marker = temp.path().join("TARGET_MUST_NOT_EXECUTE");
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
        let script = temp.path().join("agent.py");
        fs::write(&script, AGENT).unwrap();
        // Spaces and shell metacharacters must remain part of the program path.
        let agent = temp.path().join("stub agent ; literal");
        let quote = |path: &Path| format!("'{}'", path.to_str().unwrap().replace('\'', "'\\''"));
        fs::write(
            &agent,
            format!(
                "#!/bin/sh\nexec {} -I -S {} \"$@\"\n",
                quote(&python),
                quote(&script)
            ),
        )
        .unwrap();
        fs::set_permissions(&agent, fs::Permissions::from_mode(0o755)).unwrap();
        Some(Self {
            root,
            run: temp.path().join("run"),
            agent,
            log: temp.path().join("invocations.jsonl"),
            marker,
            python,
            _temp: temp,
        })
    }

    fn inspect(&self) -> Output {
        cli()
            .arg("inspect")
            .arg(&self.root)
            .arg("--out")
            .arg(&self.run)
            .arg("--python")
            .arg(&self.python)
            .output()
            .unwrap()
    }

    fn interpret(&self, mode: &str) -> Command {
        let mut command = cli();
        command
            .arg("interpret")
            .arg(&self.run)
            .arg("--agent")
            .arg(&self.agent)
            .env("WORKFLOW_MODE", mode)
            .env("WORKFLOW_LOG", &self.log)
            .env("WORKFLOW_ROOT", &self.root)
            .env("WORKFLOW_RUN", &self.run);
        command
    }

    fn status(&self, expected: &str) -> Value {
        let status = json_success(&cli().arg("status").arg(&self.run).output().unwrap());
        assert_eq!(status["status"], expected);
        assert!(status["runId"].as_str().is_some_and(|s| !s.is_empty()));
        assert!(status["snapshotId"].as_str().is_some_and(|s| !s.is_empty()));
        assert!(status["evidenceSha256"]
            .as_str()
            .is_some_and(|s| !s.is_empty()));
        assert!(status["complete"].is_boolean());
        assert_eq!(read_json(&self.run.join("state.json")), status);
        status
    }

    fn accept(&self, hash: &str) -> Output {
        cli()
            .arg("accept")
            .arg(&self.run)
            .arg("--proposal-sha256")
            .arg(hash)
            .output()
            .unwrap()
    }

    fn calls(&self) -> Vec<Value> {
        match fs::read_to_string(&self.log) {
            Ok(log) => log
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => panic!("read invocation log: {error}"),
        }
    }

    fn artifacts(&self) -> BTreeMap<String, (Vec<u8>, SystemTime)> {
        fs::read_dir(&self.run)
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                let path = entry.path();
                assert!(path.is_file(), "unexpected artifact directory: {path:?}");
                (
                    entry.file_name().into_string().unwrap(),
                    (
                        fs::read(&path).unwrap(),
                        fs::metadata(path).unwrap().modified().unwrap(),
                    ),
                )
            })
            .collect()
    }

    fn ready(&self) -> String {
        success(&self.inspect());
        let ready = json_success(&self.interpret("valid").output().unwrap());
        assert_eq!(ready["status"], "proposal_ready");
        ready["proposalSha256"]
            .as_str()
            .expect("review fingerprint")
            .to_owned()
    }

    fn unaccepted(&self) {
        assert!(!self.run.join("architecture.json").exists());
        assert!(!self.run.join("diagram.mmd").exists());
        assert!(!self.marker.exists(), "target Python was executed");
    }
}

#[test]
fn bundled_skill_is_visible_without_python_or_an_agent() {
    let output = cli().arg("skill").output().unwrap();
    success(&output);
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(!text.trim().is_empty());
    assert!(text.to_lowercase().contains("evidence"));
    assert!(text.contains("ideomancer.architecture.v1"));
}

#[test]
fn inspect_interpret_review_accept_and_emit_are_offline_and_idempotent() {
    let Some(f) = Fixture::new() else { return };
    let extracted = json_success(&f.inspect());
    assert_eq!(extracted["status"], "extracted");
    assert_eq!(extracted["complete"], true);
    assert_eq!(f.status("extracted"), extracted);
    for name in ["evidence.json", "modules.json", "modules.mmd", "state.json"] {
        assert!(f.run.join(name).is_file(), "missing {name}");
    }
    let evidence = read_json(&f.run.join("evidence.json"));
    assert_eq!(evidence["snapshotId"], extracted["snapshotId"]);
    assert_eq!(evidence["complete"], true);
    let _: Value = read_json(&f.run.join("modules.json"));
    assert!(!fs::read(f.run.join("modules.mmd")).unwrap().is_empty());
    f.unaccepted();
    let original = f.artifacts();
    assert_eq!(json_success(&f.inspect()), extracted);
    assert_eq!(
        f.artifacts(),
        original,
        "inspect must not rewrite artifacts"
    );

    let ready = json_success(
        &f.interpret("valid")
            .args(["--model", "requested-stub-model"])
            .output()
            .unwrap(),
    );
    assert_eq!(ready["status"], "proposal_ready");
    assert_eq!(f.status("proposal_ready"), ready);
    let hash = ready["proposalSha256"]
        .as_str()
        .expect("proposal fingerprint");
    let calls = f.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0]["request"]["input"], evidence);
    assert_eq!(calls[0]["request"]["model"], "requested-stub-model");
    assert_eq!(calls[0]["request"]["timeoutMs"], 299000);
    assert_eq!(ready["agentTimeoutMs"], 299000);
    let receipt = read_json(&f.run.join("attempt-0001.result.json"));
    assert_eq!(receipt["timeoutMs"], ready["agentTimeoutMs"]);
    assert!(ready["elapsedMs"].as_u64().is_some());
    assert_eq!(receipt["elapsedMs"], ready["elapsedMs"]);
    assert_eq!(read_json(&f.run.join("request.json")), calls[0]["request"]);
    let skill = cli().arg("skill").output().unwrap();
    success(&skill);
    assert_eq!(
        calls[0]["request"]["instructions"].as_str().unwrap().trim(),
        String::from_utf8_lossy(&skill.stdout).trim()
    );
    assert_eq!(
        read_json(&f.run.join("proposal.json"))["snapshotId"],
        evidence["snapshotId"]
    );
    f.unaccepted();
    let reviewed = f.artifacts();
    success(
        &f.interpret("nonzero")
            .args([
                "--model",
                "requested-stub-model",
                "--timeout-seconds",
                "600",
            ])
            .output()
            .unwrap(),
    );
    assert_eq!(f.calls().len(), 1, "ready proposals must be reused");
    assert_eq!(f.artifacts(), reviewed);

    let accepted = json_success(&f.accept(hash));
    assert_eq!(accepted["status"], "accepted");
    assert_eq!(f.status("accepted"), accepted);
    assert!(accepted["acceptedSha256"]
        .as_str()
        .is_some_and(|s| !s.is_empty()));
    let manifest = read_json(&f.run.join("architecture.json"));
    assert_eq!(manifest["schemaVersion"], "0.1.0");
    assert_eq!(manifest["name"], "Synthetic Architecture");
    assert_eq!(manifest["nodes"].as_array().unwrap().len(), 2);
    assert_eq!(manifest["edges"].as_array().unwrap().len(), 1);
    success(
        &cli()
            .arg("validate")
            .arg(f.run.join("architecture.json"))
            .output()
            .unwrap(),
    );
    let emitted = cli()
        .arg("emit")
        .arg(f.run.join("architecture.json"))
        .output()
        .unwrap();
    success(&emitted);
    assert_eq!(fs::read(f.run.join("diagram.mmd")).unwrap(), emitted.stdout);
    let immutable = f.artifacts();
    success(&f.accept(hash));
    success(&f.inspect());
    success(
        &f.interpret("nonzero")
            .args(["--model", "requested-stub-model", "--timeout-seconds", "1"])
            .output()
            .unwrap(),
    );
    assert_eq!(f.status("accepted"), accepted);
    assert_eq!(
        f.artifacts(),
        immutable,
        "accepted artifacts must not be rewritten"
    );
    assert_eq!(f.calls().len(), 1);
    assert!(!f.marker.exists());
}

#[test]
fn changed_source_or_root_cannot_clobber_an_existing_run() {
    let Some(mut f) = Fixture::new() else { return };
    success(&f.inspect());
    let original = f.artifacts();
    fs::write(f.root.join("storage.py"), "def load():\n    return 43\n").unwrap();
    failure(&f.inspect());
    assert_eq!(f.artifacts(), original);
    let other = f.root.parent().unwrap().join("other root");
    fs::create_dir(&other).unwrap();
    fs::copy(f.root.join("api.py"), other.join("api.py")).unwrap();
    fs::write(other.join("storage.py"), "def load():\n    return 42\n").unwrap();
    failure(
        &cli()
            .arg("inspect")
            .arg(other)
            .arg("--out")
            .arg(&f.run)
            .arg("--python")
            .arg(&f.python)
            .output()
            .unwrap(),
    );
    assert_eq!(
        f.artifacts(),
        original,
        "root identity is part of the run configuration"
    );
    fs::write(f.root.join("storage.py"), "def load():\n    return 42\n").unwrap();
    let other_python = f.root.parent().unwrap().join("other-python");
    symlink(&f.python, &other_python).unwrap();
    f.python = other_python;
    failure(&f.inspect());
    assert_eq!(f.artifacts(), original, "changed extraction configuration");
    f.unaccepted();
}

#[test]
fn incomplete_parse_or_traversal_persists_evidence_but_blocks_interpretation() {
    for traversal in [false, true] {
        let Some(f) = Fixture::new() else { return };
        if traversal {
            symlink(f.root.join("storage.py"), f.root.join("linked.py")).unwrap();
        } else {
            fs::write(f.root.join("broken.py"), "def broken(:\n").unwrap();
        }
        failure(&f.inspect());
        let state = f.status("extracted");
        assert_eq!(state["complete"], false);
        let evidence = read_json(&f.run.join("evidence.json"));
        assert_eq!(evidence["complete"], false);
        assert!(!evidence["diagnostics"].as_array().unwrap().is_empty());
        failure(&f.interpret("valid").output().unwrap());
        assert!(f.calls().is_empty());
        assert_eq!(f.status("extracted")["complete"], false);
        f.unaccepted();
    }
}

#[test]
fn invalid_proposals_and_envelopes_fail_closed_without_automatic_retry() {
    for mode in [
        "bad_snapshot",
        "bad_component_evidence",
        "bad_edge_evidence",
        "bad_edge_ref",
        "unknown_proposal",
        "unknown_component",
        "unknown_relationship",
        "empty_components",
        "duplicate_id",
        "unsafe_id",
        "empty_component_evidence",
        "empty_edge_evidence",
        "blank_component_rationale",
        "blank_edge_rationale",
        "bad_basis",
        "bad_schema",
        "schema_invalid",
        "bad_request_id",
        "bad_protocol",
        "failed_envelope",
        "missing_output",
        "fenced",
        "invalid_json",
        "nonzero",
    ] {
        let Some(f) = Fixture::new() else { return };
        success(&f.inspect());
        let output = f.interpret(mode).output().unwrap();
        assert!(
            !output.status.success(),
            "accepted invalid agent mode {mode}"
        );
        failure(&output);
        f.status("failed");
        assert_eq!(f.calls().len(), 1, "mode {mode}");
        f.unaccepted();
        failure(&f.interpret("valid").output().unwrap());
        assert_eq!(f.calls().len(), 1, "implicitly retried mode {mode}");
        f.status("failed");
    }
}

#[test]
fn explicit_retry_can_recover_a_failed_invocation() {
    let Some(f) = Fixture::new() else { return };
    success(&f.inspect());
    failure(&f.interpret("nonzero").output().unwrap());
    f.status("failed");
    let recovered = json_success(&f.interpret("valid").arg("--retry").output().unwrap());
    assert_eq!(recovered["status"], "proposal_ready");
    assert_eq!(f.calls().len(), 2);
    let request = &f.calls()[1]["request"];
    assert!(
        request.get("model").is_none(),
        "unspecified model must be omitted"
    );
    f.unaccepted();
}

#[test]
fn structured_agent_failures_preserve_safe_diagnostics_and_require_explicit_retry() {
    for (mode, expected_status, diagnostic) in [
        (
            "configuration_failure",
            "failed",
            "structured execution requires OPENAI_API_KEY",
        ),
        (
            "indeterminate_failure",
            "indeterminate",
            "execution deadline expired; remote completion is uncertain",
        ),
        (
            "unsafe_failure",
            "failed",
            "structured execution requires OPENAI_API_KEY",
        ),
    ] {
        let Some(f) = Fixture::new() else { return };
        success(&f.inspect());
        failure(&f.interpret(mode).output().unwrap());
        let state = f.status(expected_status);
        let error = state["error"].as_str().expect("saved agent diagnostic");
        assert!(error.contains(diagnostic), "mode {mode}: {error}");
        assert!(error.chars().count() <= 2048, "unbounded saved diagnostic");
        assert!(
            !error.chars().any(char::is_control),
            "unsafe saved diagnostic"
        );
        assert!(!error.contains("UNBOUNDED_DIAGNOSTIC_TAIL"));
        assert_eq!(state["attempts"], 1);
        assert_eq!(state["agentTimeoutMs"], 299000);
        let first_request = read_json(&f.run.join("attempt-0001.request.json"));
        let first_result = read_json(&f.run.join("attempt-0001.result.json"));
        assert_eq!(first_request["timeoutMs"], state["agentTimeoutMs"]);
        assert_eq!(first_result["timeoutMs"], state["agentTimeoutMs"]);
        assert!(state["elapsedMs"].as_u64().is_some());
        assert_eq!(first_result["elapsedMs"], state["elapsedMs"]);
        f.unaccepted();
        let before_retry = f.artifacts();
        failure(&f.interpret("valid").output().unwrap());
        failure(
            &f.interpret("valid")
                .args(["--timeout-seconds", "600"])
                .output()
                .unwrap(),
        );
        assert_eq!(f.calls().len(), 1, "implicit retry for {mode}");
        assert_eq!(f.artifacts(), before_retry);
        let recovered = json_success(
            &f.interpret("valid")
                .args(["--retry", "--timeout-seconds", "600"])
                .output()
                .unwrap(),
        );
        assert_eq!(recovered["status"], "proposal_ready");
        assert_eq!(recovered["attempts"], 2);
        assert!(recovered.get("error").is_none_or(Value::is_null));
        assert_eq!(f.calls().len(), 2);
        let second_request = read_json(&f.run.join("attempt-0002.request.json"));
        let second_result = read_json(&f.run.join("attempt-0002.result.json"));
        assert_eq!(second_request["timeoutMs"], 599000);
        assert_eq!(recovered["agentTimeoutMs"], second_request["timeoutMs"]);
        assert_eq!(second_result["timeoutMs"], second_request["timeoutMs"]);
        assert!(recovered["elapsedMs"].as_u64().is_some());
        assert_eq!(second_result["elapsedMs"], recovered["elapsedMs"]);
        assert_ne!(second_request["requestId"], first_request["requestId"]);
        let after_retry = f.artifacts();
        for file in ["attempt-0001.request.json", "attempt-0001.result.json"] {
            assert_eq!(after_retry[file], before_retry[file]);
        }
        f.unaccepted();
    }
}

#[test]
fn legacy_states_without_optional_timing_fields_load_and_remain_reusable() {
    let Some(f) = Fixture::new() else { return };
    success(&f.inspect());
    for expected in ["extracted", "proposal_ready"] {
        let path = f.run.join("state.json");
        let mut state = read_json(&path);
        let fields = state.as_object_mut().unwrap();
        for field in ["agentTimeoutMs", "elapsedMs"] {
            let removed = fields.remove(field);
            if expected == "proposal_ready" {
                assert!(removed.is_some(), "fixture must remove current {field}");
            }
        }
        fs::write(&path, serde_json::to_vec_pretty(&state).unwrap()).unwrap();
        assert_eq!(f.status(expected), state);
        let before = f.artifacts();
        let ready = json_success(&f.interpret("valid").output().unwrap());
        assert_eq!(ready["status"], "proposal_ready");
        if expected == "proposal_ready" {
            assert_eq!(ready, state);
            assert_eq!(f.artifacts(), before);
        }
        assert_eq!(f.calls().len(), 1);
    }
}

#[test]
fn retry_with_a_different_model_preserves_immutable_attempt_records() {
    let Some(f) = Fixture::new() else { return };
    success(&f.inspect());
    failure(
        &f.interpret("configuration_failure")
            .args(["--model", "first-model"])
            .output()
            .unwrap(),
    );
    let first = f.status("failed");
    assert_eq!(first["attempts"], 1);
    assert_eq!(first["requestFile"], "attempt-0001.request.json");
    assert!(first["requestSha256"]
        .as_str()
        .is_some_and(|s| !s.is_empty()));
    let original = f.artifacts();
    let first_request = read_json(&f.run.join("attempt-0001.request.json"));
    let first_result = read_json(&f.run.join("attempt-0001.result.json"));
    assert_eq!(first_request["requestId"], first["requestId"]);
    assert_eq!(first_request["model"], "first-model");
    assert_eq!(first_result["requestId"], first["requestId"]);
    assert_eq!(first_result["status"], "failed");
    assert_eq!(first_result["error"], first["error"]);

    let second = json_success(
        &f.interpret("valid")
            .args(["--retry", "--model", "second-model"])
            .output()
            .unwrap(),
    );
    assert_eq!(second["status"], "proposal_ready");
    assert_eq!(second["attempts"], 2);
    assert_eq!(second["requestFile"], "attempt-0002.request.json");
    assert_ne!(second["requestId"], first["requestId"]);
    assert_ne!(second["requestSha256"], first["requestSha256"]);
    let current = f.artifacts();
    for file in ["attempt-0001.request.json", "attempt-0001.result.json"] {
        assert_eq!(current[file], original[file], "retry rewrote {file}");
    }
    let second_request = read_json(&f.run.join("attempt-0002.request.json"));
    let second_result = read_json(&f.run.join("attempt-0002.result.json"));
    assert_eq!(second_request["requestId"], second["requestId"]);
    assert_eq!(second_request["model"], "second-model");
    assert_eq!(second_result["requestId"], second["requestId"]);
    assert_eq!(second_result["status"], "proposal_ready");
    assert_eq!(second_result["proposalSha256"], second["proposalSha256"]);
    assert_eq!(read_json(&f.run.join("request.json")), second_request);
    let calls = f.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0]["request"], first_request);
    assert_eq!(calls[1]["request"], second_request);
    assert_eq!(f.status("proposal_ready"), second);
    f.unaccepted();
}

#[test]
fn orphaned_attempt_records_are_skipped_without_reusing_old_configuration() {
    let Some(f) = Fixture::new() else { return };
    success(&f.inspect());
    let preattempt_state = fs::read(f.run.join("state.json")).unwrap();
    assert_eq!(f.status("extracted")["attempts"], 0);
    failure(
        &f.interpret("configuration_failure")
            .args(["--model", "orphaned-model"])
            .output()
            .unwrap(),
    );
    let orphaned_state = f.status("failed");
    assert_eq!(orphaned_state["attempts"], 1);
    let orphaned = f.artifacts();

    // Recreate a precommit interruption: files survived, but no attempt was committed.
    fs::write(f.run.join("state.json"), preattempt_state).unwrap();
    assert_eq!(f.status("extracted")["attempts"], 0);
    let recovered = json_success(
        &f.interpret("valid")
            .args(["--model", "replacement-model"])
            .output()
            .unwrap(),
    );
    assert_eq!(recovered["status"], "proposal_ready");
    assert_eq!(recovered["attempts"], 2);
    assert_eq!(recovered["requestFile"], "attempt-0002.request.json");
    assert_ne!(recovered["requestId"], orphaned_state["requestId"]);
    let current = f.artifacts();
    for file in ["attempt-0001.request.json", "attempt-0001.result.json"] {
        assert_eq!(
            current[file], orphaned[file],
            "recovery rewrote orphan {file}"
        );
    }
    let calls = f.calls();
    assert_eq!(
        calls.len(),
        2,
        "recovery must make exactly one new invocation"
    );
    assert_eq!(calls[0]["request"]["model"], "orphaned-model");
    assert_eq!(calls[1]["request"]["model"], "replacement-model");
    assert_eq!(calls[1]["request"]["requestId"], recovered["requestId"]);
    assert_eq!(
        read_json(&f.run.join("attempt-0002.request.json")),
        calls[1]["request"]
    );
    assert_eq!(f.status("proposal_ready"), recovered);
    f.unaccepted();
}

#[test]
fn corrupted_convenience_request_does_not_affect_execution_status_or_acceptance() {
    let Some(f) = Fixture::new() else { return };
    success(&f.inspect());
    let ready = json_success(&f.interpret("corrupt_convenience_request").output().unwrap());
    assert_eq!(ready["status"], "proposal_ready");
    assert_eq!(ready["requestFile"], "attempt-0001.request.json");
    let convenience = f.run.join("request.json");
    assert_eq!(fs::read(&convenience).unwrap(), b"{not JSON");
    assert!(serde_json::from_slice::<Value>(&fs::read(&convenience).unwrap()).is_err());
    let calls = f.calls();
    assert_eq!(calls.len(), 1);
    let authoritative = read_json(&f.run.join("attempt-0001.request.json"));
    assert_eq!(authoritative, calls[0]["request"]);
    assert_eq!(authoritative["requestId"], ready["requestId"]);
    assert_eq!(f.status("proposal_ready"), ready);
    let accepted = json_success(&f.accept(ready["proposalSha256"].as_str().unwrap()));
    assert_eq!(accepted["status"], "accepted");
    assert_eq!(f.status("accepted"), accepted);
    assert_eq!(fs::read(&convenience).unwrap(), b"{not JSON");
    assert_eq!(
        read_json(&f.run.join("attempt-0001.request.json")),
        authoritative
    );
    assert!(f.run.join("architecture.json").is_file());
    assert!(f.run.join("diagram.mmd").is_file());
    assert_eq!(f.calls().len(), 1);
    assert!(!f.marker.exists());
}

#[test]
fn timeout_is_bounded_indeterminate_and_requires_explicit_retry() {
    let Some(f) = Fixture::new() else { return };
    success(&f.inspect());
    let start = Instant::now();
    failure(
        &f.interpret("timeout")
            .args(["--timeout-seconds", "3"])
            .output()
            .unwrap(),
    );
    assert!(
        start.elapsed() < Duration::from_secs(10),
        "agent deadline was not enforced"
    );
    f.status("indeterminate");
    assert_eq!(f.calls().len(), 1);
    failure(&f.interpret("valid").output().unwrap());
    assert_eq!(
        f.calls().len(),
        1,
        "uncertain execution must not automatically retry"
    );
    f.status("indeterminate");
    let recovered = json_success(&f.interpret("valid").arg("--retry").output().unwrap());
    assert_eq!(recovered["status"], "proposal_ready");
    assert_eq!(f.calls().len(), 2);
    f.unaccepted();
}

#[test]
fn noisy_agent_output_cannot_deadlock_the_deadline_or_trigger_automatic_retry() {
    let Some(f) = Fixture::new() else { return };
    success(&f.inspect());
    let start = Instant::now();
    let output = f
        .interpret("noisy_timeout")
        .args(["--timeout-seconds", "3"])
        .output()
        .unwrap();
    failure(&output);
    assert!(start.elapsed() < Duration::from_secs(10));
    let state = read_json(&f.run.join("state.json"));
    // An output limit may fail before the deadline; either outcome forbids retry.
    let status = state["status"].as_str().unwrap();
    assert!(matches!(status, "failed" | "indeterminate"));
    f.status(status);
    failure(&f.interpret("valid").output().unwrap());
    assert_eq!(f.calls().len(), 1);
    f.unaccepted();
}

#[test]
fn a_different_agent_or_model_cannot_replace_a_reviewable_proposal() {
    let Some(mut f) = Fixture::new() else { return };
    f.ready();
    let original = f.artifacts();
    failure(
        &f.interpret("valid")
            .args(["--model", "different-model"])
            .output()
            .unwrap(),
    );
    assert_eq!(f.artifacts(), original);
    let other = f.agent.with_file_name("other agent");
    fs::copy(&f.agent, &other).unwrap();
    f.agent = other;
    failure(&f.interpret("valid").output().unwrap());
    assert_eq!(f.artifacts(), original);
    assert_eq!(f.calls().len(), 1);
    f.status("proposal_ready");
}

#[test]
fn acceptance_requires_a_review_fingerprint_and_unchanged_source() {
    let Some(f) = Fixture::new() else { return };
    let hash = f.ready();
    let original = f.artifacts();
    let missing = cli().arg("accept").arg(&f.run).output().unwrap();
    assert!(
        !missing.status.success(),
        "accept must require --proposal-sha256"
    );
    let wrong = if hash == "0".repeat(64) {
        "1".repeat(64)
    } else {
        "0".repeat(64)
    };
    failure(&f.accept(&wrong));
    assert_eq!(f.artifacts(), original);
    fs::write(
        f.root.join("storage.py"),
        "def load():\n    return 'changed'\n",
    )
    .unwrap();
    failure(&f.accept(&hash));
    f.unaccepted();
    assert_eq!(
        f.calls().len(),
        1,
        "freshness validation must not invoke the agent"
    );
}

#[test]
fn acceptance_rejects_tampered_proposal_or_evidence_even_when_still_valid_json() {
    for artifact in ["proposal.json", "evidence.json"] {
        let Some(f) = Fixture::new() else { return };
        let hash = f.ready();
        let path = f.run.join(artifact);
        let mut value = read_json(&path);
        if artifact == "proposal.json" {
            value["name"] = "Unreviewed replacement".into();
        } else {
            value["facts"][0]["value"] = "altered evidence".into();
        }
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        let tampered = fs::read(&path).unwrap();
        failure(&f.accept(&hash));
        assert_eq!(
            fs::read(path).unwrap(),
            tampered,
            "must not repair tampering silently"
        );
        f.unaccepted();
        assert_eq!(f.calls().len(), 1);
    }
}

#[test]
fn acceptance_preflights_conflicting_diagram_before_publishing_architecture() {
    let Some(f) = Fixture::new() else { return };
    let hash = f.ready();
    let diagram = f.run.join("diagram.mmd");
    let unrelated = f.run.join("unrelated-review.txt");
    fs::write(&diagram, b"unrelated existing diagram\x00\xff").unwrap();
    fs::write(&unrelated, b"keep these review notes\n").unwrap();
    let original = f.artifacts();
    failure(&f.accept(&hash));
    assert!(!f.run.join("architecture.json").exists());
    assert_eq!(
        f.artifacts(),
        original,
        "failed preflight must not publish or overwrite"
    );
    f.status("proposal_ready");
    fs::remove_file(&diagram).unwrap();
    let accepted = json_success(&f.accept(&hash));
    assert_eq!(accepted["status"], "accepted");
    assert!(f.run.join("architecture.json").is_file());
    assert!(diagram.is_file());
    assert_eq!(
        fs::read(&unrelated).unwrap(),
        original["unrelated-review.txt"].0
    );
    assert_eq!(f.calls().len(), 1);
    assert!(!f.marker.exists());
}

#[test]
fn interrupted_acceptance_reuses_identical_staged_outputs_without_rewriting() {
    let Some(f) = Fixture::new() else { return };
    let hash = f.ready();
    let ready_state = fs::read(f.run.join("state.json")).unwrap();
    let accepted = json_success(&f.accept(&hash));
    assert_eq!(accepted["status"], "accepted");
    let staged = f.artifacts();

    // Simulate a crash after publishing both outputs but before committing state.
    // Restore the actual prior state, not invented acceptance authority or hashes.
    fs::write(f.run.join("state.json"), ready_state).unwrap();
    let pending = read_json(&f.run.join("state.json"));
    assert_eq!(pending["status"], "proposal_ready");
    assert!(pending.get("acceptedSha256").is_none());
    assert!(pending.get("diagramSha256").is_none());
    assert_eq!(json_success(&f.accept(&hash)), accepted);
    assert_eq!(f.status("accepted"), accepted);
    let recovered = f.artifacts();
    for file in ["architecture.json", "diagram.mmd"] {
        assert_eq!(
            recovered[file], staged[file],
            "recovery rewrote staged {file}"
        );
    }
    let immutable = f.artifacts();
    success(&f.accept(&hash));
    assert_eq!(f.artifacts(), immutable);
    assert_eq!(f.calls().len(), 1);
    assert!(!f.marker.exists());
}
