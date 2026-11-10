//! Offline focused-workflow contracts. No application imports or live providers.
#![cfg(unix)]

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;
use std::time::SystemTime;
use tempfile::{tempdir, TempDir};

const POLICY: &str = "architecture-focus.v1";
const GLOBAL_SKILL: &str = include_str!("../skills/architecture-interpretation/SKILL.md");
const MAX_BYTES: usize = 192 * 1024;

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
            "SKIP focus workflow test: install Python 3.12 or set \
             IDEOMANCER_TEST_PYTHON to a trusted Python 3.12 executable"
        );
    }
    path.as_deref()
}

fn success(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "exit {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("one JSON report")
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn run(report: &Value) -> PathBuf {
    let path = PathBuf::from(report["runDirectory"].as_str().unwrap());
    assert!(path.is_absolute());
    path
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
                        fs::metadata(path).unwrap().modified().unwrap(),
                    ),
                );
            }
        }
    }
    result
}

struct Fixture {
    _temp: TempDir,
    root: PathBuf,
    base: PathBuf,
    python: PathBuf,
    agent: PathBuf,
    log: PathBuf,
    marker: PathBuf,
}

impl Fixture {
    fn new() -> Option<Self> {
        let python = python()?.to_path_buf();
        let temp = tempdir().unwrap();
        let directory = fs::canonicalize(temp.path()).unwrap();
        let root = directory.join("synthetic application");
        fs::create_dir(&root).unwrap();
        let marker = directory.join("TARGET_MUST_NOT_EXECUTE");
        fs::write(
            root.join("app.py"),
            format!(
                "from pathlib import Path\nPath({}).write_text('executed')\n\
                 from workflow import Workflow\n\
                 def helper():\n    return 42\n\
                 def endpoint(workflow: Workflow):\n\
                 \x20   helper()\n    selected = workflow\n    return selected.run()\n\
                 def alternate():\n    return helper()\n",
                serde_json::to_string(marker.to_str().unwrap()).unwrap()
            ),
        )
        .unwrap();
        fs::write(
            root.join("workflow.py"),
            "from dependencies import Repository\n\
             class Workflow:\n\
             \x20   def __init__(self, repo: Repository):\n\
             \x20       self.repo = repo\n\
             \x20   def run(self):\n\
             \x20       return self.repo.execute()\n",
        )
        .unwrap();
        fs::write(
            root.join("dependencies.py"),
            "class Repository:\n    def execute(self):\n        return external_sdk.send()\n",
        )
        .unwrap();
        fs::write(root.join("other.py"), "def alternate():\n    pass\n").unwrap();
        // Enough unrelated inventory to make accidental whole-project selection visible.
        for module in 0..4 {
            let mut source = String::new();
            for n in 0..60 {
                source.push_str(&format!(
                    "class Unrelated{n}:\n    def noop(self): pass\n\
                     def unrelated_{n}():\n    pass\n"
                ));
            }
            fs::write(root.join(format!("unrelated_{module}.py")), source).unwrap();
        }
        Some(Self {
            root,
            base: directory.join("analyses"),
            python,
            agent: directory.join("stub agent ; literal"),
            log: directory.join("calls.jsonl"),
            marker,
            _temp: temp,
        })
    }

    fn agent_env(&self, command: &mut Command) {
        command
            .env("FOCUS_LOG", &self.log)
            .env("FOCUS_ROOT", &self.root);
    }

    fn analyze(&self) -> Command {
        let mut command = cli();
        command
            .arg("analyze")
            .arg(&self.root)
            .arg("--out")
            .arg(&self.base)
            .arg("--python")
            .arg(&self.python)
            .arg("--agent")
            .arg(&self.agent)
            .args(["--model", "offline-focus-model", "--no-render"]);
        self.agent_env(&mut command);
        command
    }

    fn focused(&self, symbol: &str) -> Command {
        let mut command = self.analyze();
        command.args(["--focus", symbol]);
        command
    }

    fn install_agent(&self) {
        let script = self.agent.with_extension("py");
        fs::write(&script, AGENT).unwrap();
        let quote = |p: &Path| format!("'{}'", p.to_str().unwrap().replace('\'', "'\\''"));
        fs::write(
            &self.agent,
            format!(
                "#!/bin/sh\nexec {} -I -S {} \"$@\"\n",
                quote(&self.python),
                quote(&script)
            ),
        )
        .unwrap();
        fs::set_permissions(&self.agent, fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn calls(&self) -> Vec<Value> {
        match fs::read_to_string(&self.log) {
            Ok(log) => log
                .lines()
                .map(|s| serde_json::from_str(s).unwrap())
                .collect(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => panic!("read agent log: {e}"),
        }
    }

    fn safe(&self) {
        assert!(!self.marker.exists(), "target code was executed");
    }
}

const AGENT: &str = r#"import json
import os
from pathlib import Path
import sys

assert len(sys.argv) == 5, sys.argv
assert sys.argv[1:3] == ['execute', '--request'] and sys.argv[4] == '--json'
path = Path(sys.argv[3])
assert path.is_absolute()
request = json.loads(path.read_text())
with open(os.environ['FOCUS_LOG'], 'a') as log:
    log.write(json.dumps(request) + '\n')
assert not Path.cwd().resolve().is_relative_to(Path(os.environ['FOCUS_ROOT']).resolve())
assert request['protocolVersion'] == 'agent-execution.v1'
evidence = request['input']
assert evidence == json.loads((path.parent / 'evidence-packet.json').read_text())
assert evidence['complete'] is True
assert len(json.dumps(evidence, separators=(',', ':'), ensure_ascii=False).encode()) <= 196608
focus = evidence.get('focus')
if focus:
    assert focus['policyVersion'] == 'architecture-focus.v1'
    assert focus['anchor']['id'] in {f['id'] for f in evidence['facts']}
    ref = focus['anchor']['id']
    target = next((link['targetFactId'] for link in focus['links']), ref)
else:
    ref = evidence['facts'][0]['id']
    target = ref
proposal = {
    'schemaVersion': 'ideomancer.architecture.v1',
    'snapshotId': evidence['snapshotId'],
    'name': 'Synthetic Focused Workflow',
    'components': [
        {'id': 'entry', 'label': 'Entry', 'kind': 'component', 'evidenceIds': [ref],
         'rationale': 'Selected entrypoint syntax.'},
        {'id': 'dependency', 'label': 'Dependency', 'kind': 'component', 'evidenceIds': [target],
         'rationale': 'Selected static dependency candidate.'},
    ],
    'relationships': [
        {'from': 'entry', 'to': 'dependency', 'label': 'may delegate', 'basis': 'inferred',
         'evidenceIds': list(dict.fromkeys([ref, target])), 'rationale': 'Static candidates, not runtime proof.'},
    ],
    'uncertainties': ['Bounded source view; external dispatch is unresolved.'],
}
print(json.dumps({'protocolVersion': 'agent-execution.v1', 'requestId': request['requestId'],
                  'status': 'completed', 'output': proposal, 'model': 'offline-focus-model'}))
"#;

fn check_packet(report: &Value, symbol: &str, hops: usize, max: usize) -> Value {
    let run = run(report);
    let full = read_json(&run.join("evidence.json"));
    let packet = read_json(&run.join("evidence-packet.json"));
    let coverage = read_json(&run.join("evidence-coverage.json"));
    let state = read_json(&run.join("state.json"));
    let options = json!({"symbol": symbol, "hops": hops, "maxFacts": max});
    assert_eq!(report.get("focus"), Some(&options));
    assert_eq!(state.get("focus"), Some(&options));
    assert_eq!(state["packetPolicyVersion"], POLICY);
    assert_eq!(report["coverage"], coverage);
    assert_eq!(
        report["evidencePacket"],
        run.join("evidence-packet.json").to_str().unwrap()
    );
    assert_eq!(
        report["coverageReport"],
        run.join("evidence-coverage.json").to_str().unwrap()
    );
    assert_eq!(report["packetError"], Value::Null);
    for field in [
        "status",
        "snapshotId",
        "proposalSha256",
        "requiresAcceptance",
        "complete",
        "draftManifest",
        "draftMermaid",
        "draftSvg",
        "error",
    ] {
        assert!(
            report.get(field).is_some(),
            "lost existing report field {field}"
        );
    }
    let all = full["facts"].as_array().unwrap();
    let facts = packet["facts"].as_array().unwrap();
    assert!(!facts.is_empty() && facts.len() <= max);
    assert!(facts.len() < all.len(), "focus selected the whole project");
    let bytes = serde_json::to_vec(&packet).unwrap().len();
    assert!(bytes <= MAX_BYTES, "actual compact JSON exceeded cap");
    assert_eq!(coverage["inputBytes"], bytes);
    assert_eq!(coverage["maxBytes"], MAX_BYTES);
    assert_eq!(coverage["policyVersion"], POLICY);
    assert_eq!(coverage["selectionApplied"], true);
    assert_eq!(coverage["totalFacts"], all.len());
    assert_eq!(coverage["includedFacts"], facts.len());
    assert_eq!(coverage["omittedFacts"], all.len() - facts.len());
    assert_eq!(
        coverage["totalFiles"],
        full["files"].as_array().unwrap().len()
    );
    assert_eq!(packet["snapshotId"], full["snapshotId"]);
    assert_eq!(full["complete"], true);
    let inventory: BTreeMap<_, _> = all.iter().map(|f| (f["id"].as_str().unwrap(), f)).collect();
    let ids: BTreeSet<_> = facts.iter().map(|f| f["id"].as_str().unwrap()).collect();
    assert_eq!(ids.len(), facts.len());
    for fact in facts {
        assert_eq!(
            inventory.get(fact["id"].as_str().unwrap()).copied(),
            Some(fact)
        );
        assert!(!fact["path"].as_str().unwrap().starts_with("unrelated_"));
    }
    let paths: BTreeSet<_> = facts.iter().map(|f| f["path"].as_str().unwrap()).collect();
    assert_eq!(coverage["includedFiles"], paths.len());
    for (kind, counts) in coverage["byKind"].as_object().unwrap() {
        let total = all.iter().filter(|f| f["kind"] == kind.as_str()).count();
        let selected = facts.iter().filter(|f| f["kind"] == kind.as_str()).count();
        assert_eq!(counts["total"], total);
        assert_eq!(counts["included"], selected);
        assert_eq!(counts["omitted"], total - selected);
    }
    let focus = &packet["focus"];
    assert_eq!(focus["policyVersion"], POLICY);
    assert_eq!(focus["requestedSymbol"], symbol);
    assert_eq!(focus["maxHops"], hops);
    assert_eq!(focus["maxFacts"], max);
    let anchor = &focus["anchor"];
    let original = inventory[anchor["id"].as_str().unwrap()];
    assert!(ids.contains(anchor["id"].as_str().unwrap()));
    for field in ["id", "symbol", "path", "line", "endLine"] {
        assert_eq!(anchor[field], original[field], "anchor {field}");
    }
    assert!(matches!(
        original["kind"].as_str(),
        Some("function" | "async_function")
    ));
    assert!(focus["visitedSymbols"]
        .as_array()
        .unwrap()
        .contains(&anchor["symbol"]));
    assert!(focus["truncated"].is_boolean());
    assert!(focus["boundaries"].is_array());
    for link in focus["links"].as_array().unwrap() {
        assert!(link["kind"].as_str().is_some_and(|s| !s.is_empty()));
        for key in ["sourceFactId", "targetFactId"] {
            assert!(ids.contains(link[key].as_str().unwrap()));
        }
        for id in link["evidenceIds"].as_array().unwrap() {
            assert!(ids.contains(id.as_str().unwrap()));
        }
    }
    for unresolved in focus["unresolved"].as_array().unwrap() {
        assert!(ids.contains(unresolved["factId"].as_str().unwrap()));
        assert!(unresolved["reason"].as_str().is_some_and(|s| !s.is_empty()));
    }
    for (field, file) in [
        ("packetSha256", "evidence-packet.json"),
        ("coverageSha256", "evidence-coverage.json"),
    ] {
        assert_eq!(
            state[field],
            format!("{:x}", Sha256::digest(fs::read(run.join(file)).unwrap()))
        );
    }
    packet
}

#[test]
fn focus_flags_require_a_symbol_and_validate_ranges_as_usage_errors() {
    let temp = tempdir().unwrap();
    for args in [
        vec!["--focus-hops", "0"],
        vec!["--focus-facts", "1"],
        vec!["--focus", "app.endpoint", "--focus-hops", "6"],
        vec!["--focus", "app.endpoint", "--focus-hops", "-1"],
        vec!["--focus", "app.endpoint", "--focus-facts", "0"],
        vec!["--focus", "app.endpoint", "--focus-facts", "2001"],
        vec!["--focus", "app.endpoint", "--focus-facts", "invalid"],
    ] {
        let output = cli()
            .arg("analyze")
            .arg(temp.path())
            .args(&args)
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(2),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
}

#[test]
fn focused_skill_preserves_exact_global_skill_and_adds_workflow_guidance() {
    let normal = cli().arg("skill").output().unwrap();
    assert!(normal.status.success());
    assert_eq!(normal.stdout, GLOBAL_SKILL.as_bytes());
    let focused = cli().args(["skill", "--focus"]).output().unwrap();
    assert!(
        focused.status.success(),
        "{}",
        String::from_utf8_lossy(&focused.stderr)
    );
    let instructions = String::from_utf8(focused.stdout).unwrap();
    assert!(
        instructions.contains(GLOBAL_SKILL),
        "global built-in skill was modified"
    );
    let addition = instructions.replacen(GLOBAL_SKILL, "", 1).to_lowercase();
    for term in ["workflow", "handoff", "assembly", "focus"] {
        assert!(addition.contains(term), "focused guidance missing {term}");
    }
}

#[test]
fn extract_only_defaults_resolve_typed_workflow_and_exclude_unrelated_inventory() {
    let Some(f) = Fixture::new() else { return };
    let source = artifacts(&f.root);
    let report = success(
        &f.focused("app.endpoint")
            .arg("--extract-only")
            .output()
            .unwrap(),
    );
    assert_eq!(report["status"], "extracted");
    assert_eq!(report["requiresAcceptance"], false);
    let packet = check_packet(&report, "app.endpoint", 3, 600);
    let visited = packet["focus"]["visitedSymbols"].as_array().unwrap();
    for symbol in [
        "app.endpoint",
        "app.helper",
        "workflow.Workflow.run",
        "dependencies.Repository.execute",
    ] {
        assert!(
            visited.contains(&json!(symbol)),
            "missing {symbol}: {visited:?}"
        );
    }
    let links = packet["focus"]["links"].as_array().unwrap();
    assert!(links
        .iter()
        .any(|l| l["kind"] == "annotated_receiver_candidate"));
    let facts = packet["facts"].as_array().unwrap();
    for kind in ["binding", "parameter_annotation", "from_import"] {
        assert!(
            facts.iter().any(|fact| fact["kind"] == kind),
            "missing {kind}"
        );
    }
    assert!(
        packet["focus"]["unresolved"]
            .as_array()
            .unwrap()
            .iter()
            .any(|u| {
                facts
                    .iter()
                    .any(|fact| fact["id"] == u["factId"] && fact["value"] == "external_sdk.send()")
            }),
        "external call must be an unresolved boundary"
    );
    for field in [
        "proposalSha256",
        "draftManifest",
        "draftMermaid",
        "draftSvg",
    ] {
        assert!(report[field].is_null());
    }
    let before = artifacts(&run(&report));
    assert_eq!(
        success(
            &f.focused("app.endpoint")
                .arg("--extract-only")
                .output()
                .unwrap()
        ),
        report
    );
    assert_eq!(artifacts(&run(&report)), before);
    assert_eq!(artifacts(&f.root), source);
    assert!(f.calls().is_empty());
    assert!(!f.agent.exists());
    f.safe();
}

#[test]
fn zero_hops_retains_root_body_but_does_not_expand_helpers_and_suffix_is_unique() {
    let Some(f) = Fixture::new() else { return };
    let report = success(
        &f.focused("endpoint")
            .args(["--focus-hops", "0", "--extract-only"])
            .output()
            .unwrap(),
    );
    let packet = check_packet(&report, "endpoint", 0, 600);
    assert_eq!(packet["focus"]["anchor"]["symbol"], "app.endpoint");
    assert_eq!(packet["focus"]["visitedSymbols"], json!(["app.endpoint"]));
    assert_eq!(packet["focus"]["links"], json!([]));
    assert_eq!(packet["focus"]["truncated"], true);
    let facts = packet["facts"].as_array().unwrap();
    assert!(facts
        .iter()
        .any(|f| f["kind"] == "call" && f["value"] == "helper()"));
    assert!(!facts
        .iter()
        .any(|f| f["kind"] == "function" && f["symbol"] == "app.helper"));
    assert!(f.calls().is_empty());
    f.safe();
}

#[test]
fn fact_budget_counts_all_context_and_actual_json_byte_budget_is_enforced() {
    let Some(f) = Fixture::new() else { return };
    for max in [1, 4, 12] {
        let report = success(
            &f.focused("app.endpoint")
                .args(["--focus-facts", &max.to_string(), "--extract-only"])
                .output()
                .unwrap(),
        );
        let packet = check_packet(&report, "app.endpoint", 3, max);
        assert_eq!(packet["focus"]["truncated"], true);
        assert!(packet["focus"]["boundaries"]
            .to_string()
            .contains("fact_limit"));
        if max == 1 {
            assert_eq!(packet["facts"].as_array().unwrap().len(), 1);
            assert_eq!(packet["facts"][0]["symbol"], "app.endpoint");
        }
    }
    // Long expression shapes survive literal redaction (up to 512 bytes each).
    let mut source = String::from("def bulk():\n");
    for n in 0..500 {
        source.push_str(&format!("    external.{}_{n}()\n", "x".repeat(480)));
    }
    fs::write(f.root.join("bulk.py"), source).unwrap();
    let report = success(
        &f.focused("bulk.bulk")
            .args([
                "--focus-hops",
                "5",
                "--focus-facts",
                "2000",
                "--extract-only",
            ])
            .output()
            .unwrap(),
    );
    let packet = check_packet(&report, "bulk.bulk", 5, 2000);
    let full = read_json(&run(&report).join("evidence.json"));
    let body: Vec<_> = full["facts"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["symbol"] == "bulk.bulk")
        .collect();
    assert!(serde_json::to_vec(&body).unwrap().len() > MAX_BYTES);
    assert_eq!(packet["focus"]["truncated"], true);
    assert!(packet["focus"]["boundaries"]
        .to_string()
        .contains("byte_limit"));
    assert!(f.calls().is_empty());
    f.safe();
}

#[test]
fn missing_ambiguous_and_noncallable_anchors_fail_locally_before_agent() {
    let Some(f) = Fixture::new() else { return };
    f.install_agent();
    for (symbol, diagnostic) in [
        ("missing", "not found"),
        ("alternate", "ambiguous"),
        ("workflow.Workflow", "not found"),
    ] {
        for extract_only in [false, true] {
            let mut command = f.focused(symbol);
            if extract_only {
                command.arg("--extract-only");
            }
            let output = command.output().unwrap();
            assert_eq!(
                output.status.code(),
                Some(1),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let message = if output.stdout.is_empty() {
                String::from_utf8_lossy(&output.stderr).into_owned()
            } else {
                let report: Value =
                    serde_json::from_slice(&output.stdout).expect("JSON report if stdout present");
                if let Some(directory) = report["runDirectory"].as_str() {
                    let state = read_json(&Path::new(directory).join("state.json"));
                    assert_eq!(state["attempts"], 0);
                    assert!(Path::new(directory).join("evidence.json").is_file());
                    assert!(!Path::new(directory).join("proposal.json").exists());
                }
                format!("{report}\n{}", String::from_utf8_lossy(&output.stderr))
            };
            assert!(message.to_lowercase().contains(diagnostic), "{message}");
            if diagnostic == "ambiguous" {
                assert!(
                    message.contains("app.alternate") && message.contains("other.alternate"),
                    "{message}"
                );
            }
        }
    }
    assert!(f.calls().is_empty());
    f.safe();
}

#[test]
fn focused_analyze_uses_combined_skill_and_requires_explicit_acceptance_then_caches() {
    let Some(f) = Fixture::new() else { return };
    f.install_agent();
    let ready = success(
        &f.focused("app.endpoint")
            .args(["--timeout-seconds", "600"])
            .output()
            .unwrap(),
    );
    let packet = check_packet(&ready, "app.endpoint", 3, 600);
    let directory = run(&ready);
    assert_eq!(ready["status"], "proposal_ready");
    assert_eq!(ready["requiresAcceptance"], true);
    for (field, name) in [
        ("draftManifest", "presentation/draft.architecture.json"),
        ("draftMermaid", "presentation/diagram.mmd"),
        ("draftLegend", "presentation/legend.json"),
        ("draftViewer", "presentation/index.html"),
    ] {
        assert_eq!(ready[field], directory.join(name).to_str().unwrap());
        assert!(Path::new(ready[field].as_str().unwrap()).is_file());
    }
    assert_eq!(ready["presentationVersion"], "readable-v2");
    assert!(!directory.join("architecture.json").exists());
    assert!(!directory.join("diagram.mmd").exists());
    assert!(!directory.join("draft.svg").exists());
    let calls = f.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0]["input"], packet);
    assert_eq!(calls[0]["model"], "offline-focus-model");
    assert_eq!(calls[0]["timeoutMs"], 599000);
    let skill = cli().args(["skill", "--focus"]).output().unwrap();
    assert!(skill.status.success());
    assert_eq!(
        calls[0]["instructions"],
        String::from_utf8(skill.stdout).unwrap()
    );
    let before = artifacts(&directory);
    assert_eq!(success(&f.focused("app.endpoint").output().unwrap()), ready);
    assert_eq!(artifacts(&directory), before);
    let rejected = cli()
        .arg("accept")
        .arg(&directory)
        .args(["--proposal-sha256", &"0".repeat(64)])
        .output()
        .unwrap();
    assert_eq!(rejected.status.code(), Some(1));
    assert_eq!(
        artifacts(&directory),
        before,
        "wrong review fingerprint changed run"
    );
    let accepted = success(
        &cli()
            .arg("accept")
            .arg(&directory)
            .arg("--proposal-sha256")
            .arg(ready["proposalSha256"].as_str().unwrap())
            .output()
            .unwrap(),
    );
    assert_eq!(accepted["status"], "accepted");
    assert!(directory.join("architecture.json").is_file());
    let before = artifacts(&directory);
    let cached = success(&f.focused("app.endpoint").output().unwrap());
    assert_eq!(cached["status"], "accepted");
    assert_eq!(cached["requiresAcceptance"], false);
    assert_eq!(cached["proposalSha256"], ready["proposalSha256"]);
    assert_eq!(artifacts(&directory), before);
    assert_eq!(f.calls().len(), 1);
    f.safe();
}

#[test]
fn saved_focus_automatically_drives_lower_level_interpret_without_a_focus_flag() {
    let Some(f) = Fixture::new() else { return };
    let extracted = success(
        &f.focused("app.endpoint")
            .args(["--focus-hops", "1", "--focus-facts", "80", "--extract-only"])
            .output()
            .unwrap(),
    );
    let packet = check_packet(&extracted, "app.endpoint", 1, 80);
    f.install_agent();
    let mut command = cli();
    command
        .arg("interpret")
        .arg(run(&extracted))
        .arg("--agent")
        .arg(&f.agent)
        .args(["--model", "offline-focus-model"]);
    f.agent_env(&mut command);
    let ready = success(&command.output().unwrap());
    assert_eq!(ready["status"], "proposal_ready");
    let calls = f.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0]["input"], packet);
    let skill = cli().args(["skill", "--focus"]).output().unwrap();
    assert!(skill.status.success());
    assert_eq!(
        calls[0]["instructions"],
        String::from_utf8(skill.stdout).unwrap()
    );
    assert!(!run(&extracted).join("architecture.json").exists());
    f.safe();
}

#[test]
fn focus_settings_replace_current_analysis_and_reset_acceptance() {
    let Some(f) = Fixture::new() else { return };
    f.install_agent();
    let global = success(&f.analyze().output().unwrap());
    assert_eq!(global.get("focus"), Some(&Value::Null));
    let global_run = run(&global);
    let global_packet = read_json(&global_run.join("evidence-packet.json"));
    assert!(global_packet.get("focus").is_none_or(Value::is_null));
    assert_eq!(f.calls()[0]["instructions"], GLOBAL_SKILL);
    // Fingerprints remain metadata rather than user-facing directory names.
    let hash = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
    let key = json!({
        "version": "analyze.v1", "sourceRoot": f.root,
        "snapshotId": global["snapshotId"],
        "evidence": hash(&fs::read(global_run.join("evidence.json")).unwrap()),
        "python": f.python, "skill": hash(GLOBAL_SKILL.as_bytes()),
        "packetPolicy": "architecture-packet.v2-relations", "agent": f.agent,
        "model": "offline-focus-model",
    });
    assert_eq!(read_json(&global_run.join("identity.json")), key);
    assert_eq!(global_run.file_name().unwrap(), "synthetic-application");
    let first = success(&f.focused("app.endpoint").output().unwrap());
    success(
        &cli()
            .arg("accept")
            .arg(run(&first))
            .arg("--proposal-sha256")
            .arg(first["proposalSha256"].as_str().unwrap())
            .output()
            .unwrap(),
    );
    let accepted_run = run(&first);
    assert_eq!(accepted_run, global_run);
    for args in [
        vec!["--focus", "app.alternate"],
        vec!["--focus", "app.endpoint", "--focus-hops", "0"],
        vec!["--focus", "app.endpoint", "--focus-facts", "12"],
    ] {
        let report = success(&f.analyze().args(args).output().unwrap());
        assert_eq!(run(&report), global_run);
        assert_eq!(report["snapshotId"], global["snapshotId"]);
        assert_eq!(report["status"], "proposal_ready");
        assert!(!accepted_run.join("architecture.json").exists());
        assert!(!accepted_run.join("diagram.mmd").exists());
    }
    let calls = f.calls().len();
    assert_eq!(success(&f.analyze().output().unwrap()), global);
    assert_eq!(
        f.calls().len(),
        calls + 1,
        "returning to overview creates a fresh current analysis"
    );
    assert_eq!(
        fs::read_dir(global_run.parent().unwrap()).unwrap().count(),
        1
    );
    f.safe();
}
