//! Synthetic, offline contracts for human-authored plans against saved architecture.
#![cfg(unix)]

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;
use std::time::SystemTime;
use tempfile::{tempdir, TempDir};

const SECRET: &str = "SYNTHETIC-CITATION-DO-NOT-READ-81af";
const HOSTILE: &str = "https://example.invalid/missing.py?x=<script>\n![image](file:///missing)\n```\n:::include /missing|";

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
            "SKIP plan workflow test: install Python 3.12 or set \
             IDEOMANCER_TEST_PYTHON to a trusted Python 3.12 executable"
        );
    }
    path.as_deref()
}

fn exit(output: &Output, code: i32) {
    assert_eq!(
        output.status.code(),
        Some(code),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn success(output: &Output) -> Value {
    exit(output, 0);
    serde_json::from_slice(&output.stdout).expect("one JSON report")
}

fn review_report(output: &Output, code: i32, valid: bool, ready: bool) -> Value {
    exit(output, code);
    let report: Value =
        serde_json::from_slice(&output.stdout).expect("structured Review on stdout");
    assert_eq!(report["valid"], valid);
    assert_eq!(report["readyForPlanReview"], ready);
    let mut keys: Vec<_> = report
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "blockers",
            "errors",
            "readyForPlanReview",
            "valid",
            "warnings"
        ]
    );
    for field in ["errors", "blockers", "warnings"] {
        assert!(report[field]
            .as_array()
            .unwrap()
            .iter()
            .all(Value::is_string));
    }
    assert_eq!(report["errors"].as_array().unwrap().is_empty(), valid);
    assert!(!String::from_utf8_lossy(&output.stdout).contains(SECRET));
    assert!(!String::from_utf8_lossy(&output.stderr).contains(SECRET));
    report
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn write_json(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

fn artifacts(directory: &Path) -> BTreeMap<PathBuf, (Vec<u8>, SystemTime)> {
    let mut result = BTreeMap::new();
    let mut pending = vec![directory.to_path_buf()];
    while let Some(parent) = pending.pop() {
        for entry in fs::read_dir(parent).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                result.insert(
                    path.strip_prefix(directory).unwrap().to_path_buf(),
                    (
                        fs::read(&path).unwrap(),
                        fs::metadata(&path).unwrap().modified().unwrap(),
                    ),
                );
            }
        }
    }
    result
}

fn quote(path: &Path) -> String {
    format!("'{}'", path.to_str().unwrap().replace('\'', "'\\''"))
}

struct Fixture {
    _temp: TempDir,
    directory: PathBuf,
    root: PathBuf,
    run: PathBuf,
    plan: PathBuf,
    python: PathBuf,
    agent: PathBuf,
    log: PathBuf,
    python_log: PathBuf,
    marker: PathBuf,
}

impl Fixture {
    fn new(interpreted: bool) -> Option<Self> {
        let trusted_python = python()?;
        let temp = tempdir().unwrap();
        let directory = fs::canonicalize(temp.path()).unwrap();
        let root = directory.join("synthetic source");
        fs::create_dir(&root).unwrap();
        let marker = directory.join("SOURCE_MUST_NOT_EXECUTE");
        fs::write(
            root.join("service.py"),
            format!(
                "from pathlib import Path\nPath({}).write_text('executed')\n\
                 def service():\n    return 42\n",
                serde_json::to_string(marker.to_str().unwrap()).unwrap()
            ),
        )
        .unwrap();
        fs::write(directory.join("manual citation.txt"), SECRET).unwrap();
        let python = directory.join("tracked python");
        let python_log = directory.join("python-calls.log");
        fs::write(
            &python,
            format!(
                "#!/bin/sh\nprintf 'called\\n' >> {}\nexec {} \"$@\"\n",
                quote(&python_log),
                quote(trusted_python)
            ),
        )
        .unwrap();
        fs::set_permissions(&python, fs::Permissions::from_mode(0o755)).unwrap();
        let agent = directory.join("stub agent ; literal");
        let script = directory.join("stub.py");
        fs::write(&script, AGENT).unwrap();
        fs::write(
            &agent,
            format!(
                "#!/bin/sh\nexec {} -I -S {} \"$@\"\n",
                quote(trusted_python),
                quote(&script)
            ),
        )
        .unwrap();
        fs::set_permissions(&agent, fs::Permissions::from_mode(0o755)).unwrap();
        let plans = directory.join("authored plans");
        fs::create_dir(&plans).unwrap();
        let f = Self {
            root,
            run: directory.join("saved run"),
            plan: plans.join("plan.json"),
            python,
            agent,
            log: directory.join("agent-calls.jsonl"),
            python_log,
            marker,
            directory,
            _temp: temp,
        };
        let extracted = success(
            &cli()
                .arg("inspect")
                .arg(&f.root)
                .arg("--out")
                .arg(&f.run)
                .arg("--python")
                .arg(&f.python)
                .output()
                .unwrap(),
        );
        assert_eq!(extracted["status"], "extracted");
        if interpreted {
            let ready = success(
                &cli()
                    .arg("interpret")
                    .arg(&f.run)
                    .arg("--agent")
                    .arg(&f.agent)
                    .args(["--model", "offline-plan-stub"])
                    .env("PLAN_AGENT_LOG", &f.log)
                    .output()
                    .unwrap(),
            );
            assert_eq!(ready["status"], "proposal_ready");
            let request = read_json(&f.run.join("request.json"));
            let packet = read_json(&f.run.join("evidence-packet.json"));
            assert_eq!(request["input"], packet);
            let fact = request["input"]["facts"]
                .as_array()
                .unwrap()
                .iter()
                .find(|fact| fact["kind"] == "module" && fact["path"] == "service.py")
                .expect("actual sent module fact");
            let digest = format!(
                "{:x}",
                Sha256::digest(fs::read(f.run.join("proposal.json")).unwrap())
            );
            assert_eq!(ready["proposalSha256"], digest);
            let plan = json!({
                "schemaVersion":"ideomancer.plan.v1", "id":"boundary", "title":"Service boundary",
                "question":"Which boundary should we discuss?",
                "architecture":{"runDirectory":f.run,"snapshotId":packet["snapshotId"],"proposalSha256":digest},
                "scope":{"inScope":["Discuss a boundary"],"outOfScope":["Executing changes"]},
                "references":[
                    {"id":"citation","kind":"test_source","path":f.directory.join("manual citation.txt"),"line":1,"endLine":1,"note":"Unverified manual test citation"},
                    {"id":"hostile","kind":"test_source","path":HOSTILE,"line":1,"endLine":2,"note":"Nonexistent citation, not runtime proof"}
                ],
                "observations":[{"id":"observation","statement":"A module exists in saved source evidence","factIds":[fact["id"]],"referenceIds":["citation","hostile"]}],
                "requirements":[{"id":"requirement","statement":"Describe an explicit boundary","status":"proposed","confirmedBy":null,"observationIds":["observation"]}],
                "questions":[{"id":"question","question":"Is the scope design only?","status":"open","answer":null,"referenceIds":["citation"]}],
                "options":[
                    {"id":"optionA","title":"Explicit boundary","description":"Describe the boundary","pros":["Clarity"],"cons":["Design effort"],"referenceIds":["citation"]},
                    {"id":"optionB","title":"Keep current shape","description":"Document the existing shape","pros":["Less effort"],"cons":["Ambiguity"],"referenceIds":[]}
                ],
                "decisions":[{"id":"decision","question":"Which option?","optionIds":["optionA","optionB"],"selectedOptionId":null,"selectedBy":null,"rationale":null}],
                "recommendation":{"optionId":"optionA","rationale":"Worth discussing, not a selection"},
                "workItems":[{"id":"work","title":"Describe boundary","description":"Propose a design, do not execute","componentIds":["service"],"requirementIds":["requirement"],"decisionIds":["decision"],"dependsOn":[],"verificationIds":["gate"]}],
                "verification":[{"id":"gate","kind":"automated_test","check":"Review a future boundary test","expected":"Boundary is explicit","referenceIds":["citation"],"requirementIds":["requirement"]}]
            });
            write_json(&f.plan, &plan);
        }
        assert!(!f.marker.exists());
        Some(f)
    }

    fn review(&self) -> Command {
        let mut command = cli();
        command
            .args(["plan", "review"])
            .arg(&self.plan)
            .env("IDEOMANCER_PYTHON", &self.python)
            .env("IDEOMANCER_AGENT", &self.agent)
            .env("PLAN_AGENT_LOG", &self.log);
        command
    }
}

const AGENT: &str = r#"import json
import os
from pathlib import Path
import sys
assert len(sys.argv) == 5
assert sys.argv[1:3] == ['execute', '--request'] and sys.argv[4] == '--json'
path = Path(sys.argv[3])
request = json.loads(path.read_text())
assert request['protocolVersion'] == 'agent-execution.v1'
with open(os.environ['PLAN_AGENT_LOG'], 'a') as log:
    log.write(json.dumps(request) + '\n')
evidence = request['input']
assert evidence == json.loads((path.parent / 'evidence-packet.json').read_text())
ref = next(f['id'] for f in evidence['facts'] if f['kind'] == 'module' and f['path'] == 'service.py')
proposal = {
    'schemaVersion': 'ideomancer.architecture.v1', 'snapshotId': evidence['snapshotId'],
    'name': 'Synthetic Service',
    'components': [{'id': 'service', 'label': 'Service', 'kind': 'component',
                    'evidenceIds': [ref], 'rationale': 'Saved module evidence only.'}],
    'relationships': [], 'uncertainties': ['No runtime proof.'],
}
print(json.dumps({'protocolVersion': 'agent-execution.v1', 'requestId': request['requestId'],
                  'status': 'completed', 'output': proposal, 'model': 'offline-plan-stub'}))
"#;

#[test]
fn draft_renders_blockers_without_reading_citations_or_mutating_saved_inputs() {
    let Some(f) = Fixture::new(true) else { return };
    let before = artifacts(&f.directory);
    let output = f.review().output().unwrap();
    exit(&output, 0);
    let markdown = String::from_utf8(output.stdout).unwrap();
    for text in [
        "# DRAFT SOFTWARE PLAN",
        "No implementation is authorized",
        "Review Blockers",
        "PROPOSED",
        "OPEN \\(unselected\\)",
        "UNVERIFIED citation text only",
        "no tests executed",
        "PLANNED, not passed or executed",
        "Saved fact",
        "not accepted",
        "optionA",
        "optionB",
        "&#60;script&#62;",
    ] {
        assert!(markdown.contains(text), "missing {text}: {markdown}");
    }
    for raw in [
        SECRET,
        "<script>",
        "https://",
        "file://",
        "![image]",
        "```",
        ":::include",
    ] {
        assert!(!markdown.contains(raw), "unsafe or read citation: {raw}");
    }
    assert_eq!(
        read_json(&f.run.join("state.json"))["status"],
        "proposal_ready"
    );
    assert!(!f.run.join("architecture.json").exists());
    assert!(!f.marker.exists());
    assert_eq!(artifacts(&f.directory), before);
}

#[test]
fn json_and_check_ready_distinguish_draft_from_discussion_readiness_not_authority() {
    let Some(f) = Fixture::new(true) else { return };
    let draft = review_report(&f.review().arg("--json").output().unwrap(), 0, true, false);
    assert_eq!(draft["blockers"].as_array().unwrap().len(), 3);
    assert!(!draft["warnings"].as_array().unwrap().is_empty());
    assert_eq!(
        review_report(
            &f.review()
                .args(["--json", "--check-ready"])
                .output()
                .unwrap(),
            1,
            true,
            false
        ),
        draft
    );
    exit(&f.review().arg("--check-ready").output().unwrap(), 1);
    let mut plan = read_json(&f.plan);
    plan["requirements"][0]["status"] = json!("confirmed");
    plan["requirements"][0]["confirmedBy"] = json!("Synthetic product owner");
    plan["questions"][0]["status"] = json!("resolved");
    plan["questions"][0]["answer"] = json!("Design discussion only");
    plan["decisions"][0]["selectedOptionId"] = json!("optionA");
    plan["decisions"][0]["selectedBy"] = json!("Synthetic designer");
    plan["decisions"][0]["rationale"] = json!("Clarity");
    write_json(&f.plan, &plan);
    let before = artifacts(&f.directory);
    let ready = review_report(
        &f.review()
            .args(["--json", "--check-ready"])
            .output()
            .unwrap(),
        0,
        true,
        true,
    );
    assert_eq!(ready["blockers"], json!([]));
    let output = f.review().arg("--check-ready").output().unwrap();
    exit(&output, 0);
    let markdown = String::from_utf8(output.stdout).unwrap();
    assert!(markdown.contains("No implementation is authorized"));
    assert!(markdown.contains("discussion only"));
    assert!(markdown.contains("PLANNED, not passed or executed"));
    assert_eq!(artifacts(&f.directory), before);
}

#[test]
fn stale_pins_unknown_typed_links_and_cycles_are_invalid_and_never_publish() {
    let Some(f) = Fixture::new(true) else { return };
    let base = read_json(&f.plan);
    let out = f.directory.join("invalid.md");
    let saved = artifacts(&f.run);
    for (pointer, replacement) in [
        ("/architecture/snapshotId", json!("stale-snapshot")),
        ("/architecture/proposalSha256", json!("0".repeat(64))),
        ("/observations/0/factIds", json!(["invented-fact"])),
        ("/workItems/0/componentIds", json!(["invented-component"])),
        ("/observations/0/referenceIds", json!(["requirement"])),
        ("/requirements/0/observationIds", json!(["optionA"])),
        ("/decisions/0/optionIds", json!(["requirement"])),
        ("/workItems/0/verificationIds", json!(["work"])),
        ("/workItems/0/dependsOn", json!(["missing-work"])),
        ("/workItems/0/dependsOn", json!(["work"])),
    ] {
        let mut plan = base.clone();
        *plan.pointer_mut(pointer).unwrap() = replacement;
        write_json(&f.plan, &plan);
        review_report(&f.review().arg("--json").output().unwrap(), 1, false, false);
        exit(&f.review().arg("--out").arg(&out).output().unwrap(), 1);
        assert!(!out.exists(), "published invalid {pointer}");
    }
    let mut plan = base;
    let mut second = plan["workItems"][0].clone();
    second["id"] = json!("second");
    second["dependsOn"] = json!(["work"]);
    plan["workItems"][0]["dependsOn"] = json!(["second"]);
    plan["workItems"].as_array_mut().unwrap().push(second);
    write_json(&f.plan, &plan);
    let report = review_report(&f.review().arg("--json").output().unwrap(), 1, false, false);
    assert!(report["errors"].to_string().contains("cycle"));
    exit(&f.review().arg("--out").arg(&out).output().unwrap(), 1);
    assert!(!out.exists());
    assert_eq!(artifacts(&f.run), saved);
}

#[test]
fn relative_architecture_run_is_resolved_from_plan_parent_not_process_cwd() {
    let Some(f) = Fixture::new(true) else { return };
    let mut plan = read_json(&f.plan);
    plan["architecture"]["runDirectory"] = json!("../saved run");
    write_json(&f.plan, &plan);
    let cwd = f.directory.join("unrelated cwd");
    fs::create_dir(&cwd).unwrap();
    let before = artifacts(&f.directory);
    review_report(
        &f.review().current_dir(&cwd).arg("--json").output().unwrap(),
        0,
        true,
        false,
    );
    let output = f.review().current_dir(&cwd).output().unwrap();
    exit(&output, 0);
    assert!(String::from_utf8_lossy(&output.stdout).contains("Saved fact"));
    assert_eq!(artifacts(&f.directory), before);
}

#[test]
fn markdown_output_is_idempotent_no_clobber_and_cannot_replace_plan_or_mix_json() {
    let Some(f) = Fixture::new(true) else { return };
    let out = f.directory.join("review.md");
    let stdout = f.review().output().unwrap();
    exit(&stdout, 0);
    exit(&f.review().arg("--out").arg(&out).output().unwrap(), 0);
    assert_eq!(fs::read(&out).unwrap(), stdout.stdout);
    let before = artifacts(&f.directory);
    exit(&f.review().arg("--out").arg(&out).output().unwrap(), 0);
    assert_eq!(
        artifacts(&f.directory),
        before,
        "identical output rewrote bytes or mtime"
    );
    fs::write(&out, "human-authored existing document\n").unwrap();
    let before = artifacts(&f.directory);
    exit(&f.review().arg("--out").arg(&out).output().unwrap(), 1);
    exit(&f.review().arg("--out").arg(&f.plan).output().unwrap(), 1);
    exit(
        &f.review()
            .arg("--out")
            .arg(f.plan.parent().unwrap().join("./plan.json"))
            .output()
            .unwrap(),
        1,
    );
    exit(
        &f.review()
            .arg("--json")
            .arg("--out")
            .arg(&out)
            .output()
            .unwrap(),
        2,
    );
    let absent = f.directory.join("must-not-create.md");
    exit(
        &f.review()
            .arg("--json")
            .arg("--out")
            .arg(&absent)
            .output()
            .unwrap(),
        2,
    );
    assert!(!absent.exists());
    assert_eq!(artifacts(&f.directory), before);
}

#[test]
fn extracted_context_and_tampered_saved_artifacts_fail_without_repair_or_dispatch() {
    let Some(f) = Fixture::new(true) else { return };
    let Some(extracted) = Fixture::new(false) else {
        return;
    };
    let mut plan = read_json(&f.plan);
    plan["architecture"]["runDirectory"] = json!(extracted.run);
    plan["architecture"]["snapshotId"] =
        read_json(&extracted.run.join("state.json"))["snapshotId"].clone();
    write_json(&extracted.plan, &plan);
    let before = artifacts(&extracted.directory);
    review_report(
        &extracted.review().arg("--json").output().unwrap(),
        1,
        false,
        false,
    );
    let out = extracted.directory.join("invalid.md");
    exit(
        &extracted.review().arg("--out").arg(&out).output().unwrap(),
        1,
    );
    assert!(!out.exists());
    assert_eq!(artifacts(&extracted.directory), before);
    let state = read_json(&f.run.join("state.json"));
    let request_file = state["requestFile"].as_str().unwrap();
    assert_ne!(request_file, "request.json");
    for artifact in [
        "proposal.json",
        "evidence.json",
        "evidence-packet.json",
        "evidence-coverage.json",
        request_file,
    ] {
        let path = f.run.join(artifact);
        let original = fs::read(&path).unwrap();
        let mut changed = read_json(&path);
        changed["tampered"] = json!(true);
        write_json(&path, &changed);
        let before = artifacts(&f.directory);
        review_report(&f.review().arg("--json").output().unwrap(), 1, false, false);
        let out = f.directory.join("invalid.md");
        exit(&f.review().arg("--out").arg(&out).output().unwrap(), 1);
        assert!(!out.exists(), "published despite tampered {artifact}");
        assert_eq!(
            artifacts(&f.directory),
            before,
            "repaired or mutated {artifact}"
        );
        fs::write(path, original).unwrap();
    }
    // The convenience copy is not the immutable request pinned by state.
    fs::write(f.run.join("request.json"), "{not JSON").unwrap();
    let before = artifacts(&f.directory);
    review_report(&f.review().arg("--json").output().unwrap(), 0, true, false);
    assert_eq!(artifacts(&f.directory), before);
}

#[test]
fn schema_enum_io_and_context_errors_produce_safe_structured_invalid_reports() {
    let Some(f) = Fixture::new(true) else { return };
    let base = read_json(&f.plan);
    let out = f.directory.join("invalid.md");
    for pointer in [
        "",
        "/architecture",
        "/references/0",
        "/workItems/0",
        "/verification/0",
    ] {
        let mut plan = base.clone();
        plan.pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("approval".into(), json!(true));
        write_json(&f.plan, &plan);
        review_report(&f.review().arg("--json").output().unwrap(), 1, false, false);
        exit(&f.review().arg("--out").arg(&out).output().unwrap(), 1);
        assert!(!out.exists());
    }
    for pointer in [
        "/references/0/kind",
        "/requirements/0/status",
        "/questions/0/status",
        "/verification/0/kind",
        "/schemaVersion",
        "/architecture/runDirectory",
    ] {
        let mut plan = base.clone();
        *plan.pointer_mut(pointer).unwrap() = json!("nonexistent-or-invalid");
        write_json(&f.plan, &plan);
        review_report(&f.review().arg("--json").output().unwrap(), 1, false, false);
        exit(&f.review().arg("--out").arg(&out).output().unwrap(), 1);
        assert!(!out.exists());
    }
    fs::write(&f.plan, "{broken JSON").unwrap();
    review_report(&f.review().arg("--json").output().unwrap(), 1, false, false);
    fs::remove_file(&f.plan).unwrap();
    let before = artifacts(&f.directory);
    review_report(&f.review().arg("--json").output().unwrap(), 1, false, false);
    exit(&f.review().arg("--out").arg(&out).output().unwrap(), 1);
    assert!(!out.exists());
    assert_eq!(artifacts(&f.directory), before);
}

#[test]
fn accepted_readonly_architecture_reviews_saved_context_without_python_or_rescanning() {
    let Some(f) = Fixture::new(true) else { return };
    let state = read_json(&f.run.join("state.json"));
    let accepted = success(
        &cli()
            .arg("accept")
            .arg(&f.run)
            .arg("--proposal-sha256")
            .arg(state["proposalSha256"].as_str().unwrap())
            .output()
            .unwrap(),
    );
    assert_eq!(accepted["status"], "accepted");
    // A fresh parse must now fail; planning must use only pinned saved context.
    fs::write(f.root.join("service.py"), "def broken(:\n").unwrap();
    let before = artifacts(&f.directory);
    let mut modes = Vec::new();
    let mut pending = vec![f.run.clone()];
    while let Some(path) = pending.pop() {
        let metadata = fs::metadata(&path).unwrap();
        if metadata.is_dir() {
            for entry in fs::read_dir(&path).unwrap() {
                pending.push(entry.unwrap().path());
            }
        }
        modes.push((path.clone(), metadata.permissions()));
        fs::set_permissions(
            &path,
            fs::Permissions::from_mode(if metadata.is_dir() { 0o555 } else { 0o444 }),
        )
        .unwrap();
    }
    let output = f.review().arg("--json").output().unwrap();
    let markdown = f.review().output().unwrap();
    for (path, permissions) in modes.into_iter().rev() {
        fs::set_permissions(path, permissions).unwrap();
    }
    let report = review_report(&output, 0, true, false);
    assert!(!report["warnings"].to_string().contains("not accepted"));
    exit(&markdown, 0);
    assert!(String::from_utf8_lossy(&markdown.stdout).contains("No implementation is authorized"));
    assert_eq!(artifacts(&f.directory), before);
    assert_eq!(fs::read_to_string(&f.log).unwrap().lines().count(), 1);
    assert!(
        !fs::read(&f.python_log).unwrap().is_empty(),
        "tracking wrapper was not exercised"
    );
    assert!(!f.marker.exists());
}
