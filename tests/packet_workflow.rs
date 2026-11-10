//! Synthetic, offline contracts for persisted, bounded architecture evidence.
#![cfg(unix)]

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;
use std::time::SystemTime;
use tempfile::{tempdir, TempDir};

const POLICY: &str = "architecture-packet.v2-relations";
const MAX_BYTES: usize = 192 * 1024;
const SECRET: &str = "synthetic-credential-DO-NOT-COPY-7f943";
const RAW_SOURCE: &str = "synthetic-docstring-not-agent-input-98ab";

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
            "SKIP packet workflow test: install Python 3.12 or set \
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

fn failure(output: &Output) -> String {
    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn read_json(path: &Path) -> Value {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

fn write_json(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

fn artifacts(directory: &Path) -> BTreeMap<PathBuf, (Vec<u8>, SystemTime)> {
    let mut result = BTreeMap::new();
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            for (child, contents) in artifacts(&path) {
                result.insert(
                    PathBuf::from(path.file_name().unwrap()).join(child),
                    contents,
                );
            }
        } else {
            result.insert(
                PathBuf::from(path.file_name().unwrap()),
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
    _temp: TempDir,
    root: PathBuf,
    base: PathBuf,
    run: PathBuf,
    python: PathBuf,
    agent: PathBuf,
    log: PathBuf,
    marker: PathBuf,
}

impl Fixture {
    fn new(large: bool) -> Option<Self> {
        let python = python()?.to_path_buf();
        let temp = tempdir().unwrap();
        let directory = fs::canonicalize(temp.path()).unwrap();
        let root = directory.join("synthetic source");
        fs::create_dir(&root).unwrap();
        let marker = directory.join("SOURCE_MUST_NOT_EXECUTE");
        fs::write(
            root.join("dependencies.py"),
            "class Base: pass\nclass Repository:\n    def execute(self): pass\n\
             def register(cls): return cls\ndef provide(value): return Repository()\n",
        )
        .unwrap();
        fs::write(
            root.join("entry.py"),
            format!(
                "from pathlib import Path\nPath({}).write_text('executed')\n",
                serde_json::to_string(marker.to_str().unwrap()).unwrap()
            ),
        )
        .unwrap();
        for module in 0..if large { 4 } else { 1 } {
            let mut source = format!(
                "\"\"\"{RAW_SOURCE}\"\"\"\nimport dependencies\n\
                 from dependencies import Base, Repository, register, provide\n"
            );
            for scope in 0..3 {
                source.push_str(&format!(
                    "\n@register\nclass Service{scope}(Base):\n\
                     \x20   def __init__(self, repo: Repository = provide('{SECRET}')):\n\
                     \x20       self.repo = repo\n\
                     \x20       self.client = provide('{SECRET}')\n\
                     \x20   def run(self) -> Repository:\n\
                     \x20       local: Repository = self.repo\n\
                     \x20       local.execute()\n\
                     \x20       return self.repo.execute()\n"
                ));
            }
            fs::write(root.join(format!("service_{module}.py")), source).unwrap();
        }
        if large {
            // 1,200 DTOs plus methods overwhelm the packet without approaching
            // the extractor's file limit or needing any real application code.
            for module in 0..12 {
                let mut source = String::new();
                for class in 0..100 {
                    source.push_str(&format!("class Dto{class:04}:\n    def noop(self): pass\n"));
                }
                fs::write(root.join(format!("aaa_dto_{module:02}.py")), source).unwrap();
            }
        }
        Some(Self {
            root,
            base: directory.join("analyses"),
            run: directory.join("inspect run"),
            python,
            // Extraction tests intentionally never create an agent executable.
            agent: directory.join("stub agent ; literal"),
            log: directory.join("agent-calls.jsonl"),
            marker,
            _temp: temp,
        })
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
            .arg("--no-render");
        self.agent_env(&mut command);
        command
    }

    fn inspect(&self) -> Output {
        cli()
            .arg("inspect")
            .arg(&self.root)
            .arg("--out")
            .arg(&self.run)
            .arg("--python")
            .arg(&self.python)
            .env("IDEOMANCER_AGENT", &self.agent)
            .output()
            .unwrap()
    }

    fn interpret(&self, run: &Path) -> Command {
        let mut command = cli();
        command
            .arg("interpret")
            .arg(run)
            .arg("--agent")
            .arg(&self.agent);
        self.agent_env(&mut command);
        command
    }

    fn agent_env(&self, command: &mut Command) {
        command
            .env("PACKET_LOG", &self.log)
            .env("PACKET_ROOT", &self.root)
            .env_remove("PACKET_OMITTED_ID");
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
        assert!(!self.marker.exists(), "target source was executed");
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
with open(os.environ['PACKET_LOG'], 'a') as log:
    log.write(json.dumps(request) + '\n')
assert not Path.cwd().resolve().is_relative_to(Path(os.environ['PACKET_ROOT']).resolve())
assert request['protocolVersion'] == 'agent-execution.v1'
evidence = request['input']
assert evidence == json.loads((path.parent / 'evidence-packet.json').read_text())
assert evidence['complete'] is True
assert len(json.dumps(evidence, separators=(',', ':'), ensure_ascii=False).encode()) <= 196608
ids = [f['id'] for f in evidence['facts'] if f['kind'] in ('binding', 'call')]
assert ids
ref = os.environ.get('PACKET_OMITTED_ID', ids[0])
if 'PACKET_OMITTED_ID' in os.environ:
    assert ref not in {f['id'] for f in evidence['facts']}
proposal = {
    'schemaVersion': 'ideomancer.architecture.v1',
    'snapshotId': evidence['snapshotId'],
    'name': 'Synthetic Packet Architecture',
    'components': [
        {'id': 'service', 'label': 'Service', 'kind': 'component', 'evidenceIds': [ref],
         'rationale': 'A lexical source grouping, not runtime proof.'},
        {'id': 'repository', 'label': 'Repository', 'kind': 'component', 'evidenceIds': [ref],
         'rationale': 'A grouping based on selected static evidence.'},
    ],
    'relationships': [
        {'from': 'service', 'to': 'repository', 'label': 'may use', 'basis': 'inferred',
         'evidenceIds': [ref], 'rationale': 'Static lexical evidence only.'},
    ],
    'uncertainties': ['No runtime relationship has been proved.'],
}
print(json.dumps({'protocolVersion': 'agent-execution.v1', 'requestId': request['requestId'],
                  'status': 'completed', 'output': proposal, 'model': 'offline-packet-stub'}))
"#;

fn check_packet(run: &Path, selected: bool) -> (Value, Value) {
    let full = read_json(&run.join("evidence.json"));
    let packet = read_json(&run.join("evidence-packet.json"));
    let coverage = read_json(&run.join("evidence-coverage.json"));
    let state = read_json(&run.join("state.json"));
    assert_eq!(full["extractorVersion"], "python-static-8-generic-facts");
    assert_eq!(full["complete"], true);
    assert_eq!(packet["snapshotId"], full["snapshotId"]);
    assert_eq!(state["packetPolicyVersion"], POLICY);
    assert!(state.get("packetError").is_none_or(Value::is_null));
    for (field, file) in [
        ("packetSha256", "evidence-packet.json"),
        ("coverageSha256", "evidence-coverage.json"),
    ] {
        assert_eq!(
            state[field],
            format!("{:x}", Sha256::digest(fs::read(run.join(file)).unwrap()))
        );
    }
    let bytes = serde_json::to_vec(&packet).unwrap();
    assert!(bytes.len() <= MAX_BYTES);
    assert_eq!(coverage["inputBytes"], bytes.len());
    assert_eq!(coverage["maxBytes"], MAX_BYTES);
    assert_eq!(coverage["policyVersion"], POLICY);
    assert_eq!(coverage["selectionApplied"], selected);
    let all = full["facts"].as_array().unwrap();
    let included = packet["facts"].as_array().unwrap();
    assert!(!included.is_empty());
    assert_eq!(coverage["totalFacts"], all.len());
    assert_eq!(coverage["includedFacts"], included.len());
    assert_eq!(coverage["omittedFacts"], all.len() - included.len());
    assert_eq!(
        coverage["totalFiles"],
        full["files"].as_array().unwrap().len()
    );
    let inventory: BTreeMap<_, _> = all.iter().map(|f| (f["id"].as_str().unwrap(), f)).collect();
    assert_eq!(inventory.len(), all.len());
    let mut ids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for fact in included {
        let id = fact["id"].as_str().unwrap();
        assert!(ids.insert(id), "duplicate selected ID");
        assert_eq!(
            inventory.get(id).copied(),
            Some(fact),
            "invented or edited fact"
        );
        paths.insert(fact["path"].as_str().unwrap());
    }
    assert_eq!(coverage["includedFiles"], paths.len());
    let counts = |facts: &[Value]| {
        let mut counts = BTreeMap::<String, usize>::new();
        for f in facts {
            *counts
                .entry(f["kind"].as_str().unwrap().to_owned())
                .or_default() += 1;
        }
        counts
    };
    let total_counts = counts(all);
    let included_counts = counts(included);
    let by_kind = coverage["byKind"].as_object().unwrap();
    assert_eq!(by_kind.len(), total_counts.len());
    for (kind, total) in total_counts {
        let n = included_counts.get(&kind).copied().unwrap_or(0);
        assert_eq!(by_kind[&kind]["total"], total, "{kind}");
        assert_eq!(by_kind[&kind]["included"], n, "{kind}");
        assert_eq!(by_kind[&kind]["omitted"], total - n, "{kind}");
    }
    for kind in [
        "import",
        "from_import",
        "base",
        "decorator",
        "parameter_annotation",
        "parameter_default",
        "return_annotation",
        "binding",
        "call",
    ] {
        assert!(
            included_counts.get(kind).is_some_and(|n| *n > 0),
            "lost {kind}"
        );
    }
    for fact in all.iter().filter(|f| f["kind"] == "binding") {
        let binding: Value = serde_json::from_str(fact["value"].as_str().unwrap()).unwrap();
        assert!(binding["target"].is_string());
        assert!(binding["source"].is_string());
    }
    assert!(
        all.iter().any(|fact| {
            fact["kind"] == "binding"
                && serde_json::from_str::<Value>(fact["value"].as_str().unwrap()).unwrap()
                    == serde_json::json!({"target": "self.client", "source": "provide(<literal>)"})
        }),
        "fixture must exercise literal redaction inside binding values"
    );
    let warnings = coverage["warnings"].as_array().unwrap();
    assert!(!warnings.is_empty());
    assert!(warnings.iter().all(Value::is_string));
    assert!(warnings
        .iter()
        .any(|w| w.as_str().unwrap().to_lowercase().contains("runtime")));
    if selected {
        assert!(serde_json::to_vec(&full).unwrap().len() > MAX_BYTES);
        assert!(included.len() < all.len());
        assert_eq!(packet["selection"]["policyVersion"], POLICY);
        assert_eq!(packet["selection"]["byKind"], coverage["byKind"]);
        assert_eq!(packet["selection"]["warnings"], coverage["warnings"]);
        assert!(warnings
            .iter()
            .any(|w| w.as_str().unwrap().to_lowercase().contains("omit")));
    } else {
        assert_eq!(
            packet, full,
            "small packet must be the unchanged evidence JSON"
        );
        assert!(packet.get("selection").is_none());
    }
    for value in [&full, &packet] {
        let text = value.to_string();
        assert!(!text.contains(SECRET), "literal credential leaked");
        assert!(!text.contains(RAW_SOURCE), "raw source leaked");
        assert!(
            text.contains("<literal>"),
            "fixture did not exercise redaction"
        );
    }
    (packet, coverage)
}

fn check_report(report: &Value, run: &Path, coverage: &Value) {
    for (field, file) in [
        ("evidencePacket", "evidence-packet.json"),
        ("coverageReport", "evidence-coverage.json"),
    ] {
        let path = Path::new(report.get(field).expect(field).as_str().unwrap());
        assert!(path.is_absolute());
        assert_eq!(path, run.join(file));
    }
    assert_eq!(report.get("coverage"), Some(coverage));
    assert_eq!(report.get("packetError"), Some(&Value::Null));
    for field in [
        "runDirectory",
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
            "missing existing report field {field}"
        );
    }
}

#[test]
fn extract_only_persists_small_and_selected_packets_without_an_agent_or_rewrites() {
    for large in [false, true] {
        let Some(f) = Fixture::new(large) else { return };
        let source_files = artifacts(&f.root);
        assert!(!f.agent.exists());
        let report = success(&f.analyze().arg("--extract-only").output().unwrap());
        let run = Path::new(report["runDirectory"].as_str().unwrap());
        assert_eq!(report["status"], "extracted");
        let (packet, coverage) = check_packet(run, large);
        check_report(&report, run, &coverage);
        for field in [
            "proposalSha256",
            "draftManifest",
            "draftMermaid",
            "draftSvg",
        ] {
            assert!(report[field].is_null(), "{field}");
        }
        assert_eq!(report["requiresAcceptance"], false);
        let original = artifacts(run);
        assert_eq!(
            success(&f.analyze().arg("--extract-only").output().unwrap()),
            report
        );
        assert_eq!(artifacts(run), original, "repeat rewrote bytes or mtimes");

        let inspected = success(&f.inspect());
        assert_eq!(inspected["status"], "extracted");
        let (inspect_packet, inspect_coverage) = check_packet(&f.run, large);
        assert_eq!(
            inspect_packet, packet,
            "same input produced a different packet"
        );
        assert_eq!(inspect_coverage, coverage);
        let original = artifacts(&f.run);
        assert_eq!(success(&f.inspect()), inspected);
        assert_eq!(artifacts(&f.run), original);
        assert!(f.calls().is_empty());
        assert!(
            !f.agent.exists(),
            "extraction must not require even a stub agent"
        );
        assert_eq!(
            artifacts(&f.root),
            source_files,
            "source files were changed"
        );
        f.safe();
    }
}

#[test]
fn selected_packet_preserves_connected_service_scopes_across_modules() {
    let Some(f) = Fixture::new(true) else { return };
    success(&f.inspect());
    let (packet, _) = check_packet(&f.run, true);
    let facts = packet["facts"].as_array().unwrap();
    for module in 0..4 {
        let path = format!("service_{module}.py");
        for scope in 0..3 {
            let class = format!("service_{module}.Service{scope}");
            let has = |kind: &str, symbol: &str, value: Option<&str>| {
                facts.iter().any(|f| {
                    f["path"] == path
                        && f["kind"] == kind
                        && f["symbol"] == symbol
                        && value.is_none_or(|v| f["value"] == v)
                })
            };
            assert!(has("class", &class, None), "missing lexical owner {class}");
            assert!(has("base", &class, Some("Base")));
            assert!(has("decorator", &class, Some("register")));
            assert!(has("function", &format!("{class}.__init__"), None));
            assert!(has("function", &format!("{class}.run"), None));
            assert!(has(
                "parameter_annotation",
                &format!("{class}.__init__.repo"),
                Some("Repository")
            ));
            assert!(
                facts.iter().any(|f| {
                    f["path"] == path
                        && f["kind"] == "binding"
                        && f["symbol"] == format!("{class}.__init__")
                        && serde_json::from_str::<Value>(f["value"].as_str().unwrap()).unwrap()
                            == serde_json::json!({"target": "self.repo", "source": "repo"})
                }),
                "missing constructor binding for {class}"
            );
            assert!(has(
                "call",
                &format!("{class}.run"),
                Some("self.repo.execute()")
            ));
        }
    }
    f.safe();
}

#[test]
fn normal_analyze_sends_exact_saved_packet_and_accepts_only_selected_references() {
    let Some(f) = Fixture::new(true) else { return };
    f.install_agent();
    let report = success(&f.analyze().output().unwrap());
    let run = Path::new(report["runDirectory"].as_str().unwrap());
    assert_eq!(report["status"], "proposal_ready");
    let (packet, coverage) = check_packet(run, true);
    check_report(&report, run, &coverage);
    let calls = f.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0]["input"], packet);
    let ids: BTreeSet<_> = packet["facts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["id"].as_str().unwrap())
        .collect();
    let proposal = read_json(&run.join("proposal.json"));
    for item in proposal["components"]
        .as_array()
        .unwrap()
        .iter()
        .chain(proposal["relationships"].as_array().unwrap())
    {
        for id in item["evidenceIds"].as_array().unwrap() {
            assert!(ids.contains(id.as_str().unwrap()));
        }
    }
    assert!(!run.join("architecture.json").exists());
    assert!(!run.join("draft.svg").exists());
    let original = artifacts(run);
    assert_eq!(success(&f.analyze().output().unwrap()), report);
    assert_eq!(artifacts(run), original);
    assert_eq!(f.calls().len(), 1);
    f.safe();
}

#[test]
fn full_inventory_but_omitted_reference_is_rejected_as_failed() {
    let Some(f) = Fixture::new(true) else { return };
    let report = success(&f.analyze().arg("--extract-only").output().unwrap());
    let run = Path::new(report["runDirectory"].as_str().unwrap());
    let (packet, _) = check_packet(run, true);
    let full = read_json(&run.join("evidence.json"));
    let included: BTreeSet<_> = packet["facts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["id"].as_str().unwrap())
        .collect();
    let omitted = full["facts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| !included.contains(f["id"].as_str().unwrap()))
        .expect("fixture must have an actual omitted original fact")["id"]
        .as_str()
        .unwrap();
    f.install_agent();
    let output = f
        .analyze()
        .env("PACKET_OMITTED_ID", omitted)
        .output()
        .unwrap();
    failure(&output);
    let failed: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(failed["status"], "failed");
    assert_eq!(read_json(&run.join("state.json"))["status"], "failed");
    assert_eq!(f.calls().len(), 1);
    assert_eq!(f.calls()[0]["input"], packet);
    assert!(!run.join("architecture.json").exists());
    assert!(!run.join("draft.architecture.json").exists());
    f.safe();
}

#[test]
fn valid_json_packet_or_coverage_tampering_fails_before_agent_and_preserves_hashes() {
    for artifact in ["evidence-packet.json", "evidence-coverage.json"] {
        let Some(f) = Fixture::new(false) else { return };
        success(&f.inspect());
        check_packet(&f.run, false);
        f.install_agent();
        let state = fs::read(f.run.join("state.json")).unwrap();
        let path = f.run.join(artifact);
        let mut changed = read_json(&path);
        if artifact == "evidence-packet.json" {
            changed["facts"][0]["value"] = "tampered static fact".into();
        } else {
            changed["warnings"] = serde_json::json!(["tampered coverage warning"]);
        }
        write_json(&path, &changed);
        let original = artifacts(&f.run);
        let status = cli().arg("status").arg(&f.run).output().unwrap();
        let interpret = f.interpret(&f.run).output().unwrap();
        let message = failure(&interpret);
        assert!(
            message.to_lowercase().contains("integrity") || message.contains(artifact),
            "{message}"
        );
        if !status.status.success() {
            failure(&status);
        }
        assert!(f.calls().is_empty(), "tampered artifact reached agent");
        assert_eq!(
            fs::read(f.run.join("state.json")).unwrap(),
            state,
            "saved expected hashes must not be updated to bless tampering"
        );
        assert_eq!(
            artifacts(&f.run),
            original,
            "tampering was repaired or artifacts changed"
        );
        f.safe();
    }
}

#[test]
fn legacy_states_load_but_missing_packet_policy_never_rebuilds_or_changes_old_proposals() {
    for proposal_ready in [false, true] {
        let Some(f) = Fixture::new(false) else { return };
        success(&f.inspect());
        check_packet(&f.run, false);
        f.install_agent();
        if proposal_ready {
            assert_eq!(
                success(&f.interpret(&f.run).output().unwrap())["status"],
                "proposal_ready"
            );
        }
        let state_path = f.run.join("state.json");
        let mut legacy = read_json(&state_path);
        for field in [
            "packetPolicyVersion",
            "packetSha256",
            "coverageSha256",
            "packetError",
        ] {
            legacy.as_object_mut().unwrap().remove(field);
        }
        // Keep all original artifacts, including a genuinely generated proposal.
        // Only state fields are removed to emulate the previous persisted schema.
        write_json(&state_path, &legacy);
        let original = artifacts(&f.run);
        let loaded = success(&cli().arg("status").arg(&f.run).output().unwrap());
        assert_eq!(loaded["status"], legacy["status"]);
        assert_eq!(loaded["evidenceSha256"], legacy["evidenceSha256"]);
        assert_eq!(loaded["proposalSha256"], legacy["proposalSha256"]);
        let calls = f.calls().len();
        let message = failure(&f.interpret(&f.run).output().unwrap()).to_lowercase();
        assert!(
            message.contains("packet")
                && (message.contains("policy") || message.contains("legacy")),
            "missing policy needs a clear diagnostic: {message}"
        );
        assert_eq!(f.calls().len(), calls, "legacy state dispatched an agent");
        assert_eq!(
            artifacts(&f.run),
            original,
            "legacy artifacts were silently upgraded"
        );
        f.safe();
    }
}

#[test]
fn oversized_packet_metadata_preserves_complete_inventory_and_blocks_agent_dispatch() {
    let Some(f) = Fixture::new(false) else { return };
    // These calls are parsed, never executed. Their diagnostics describe a
    // static-analysis limitation, not an incomplete source inventory.
    for module in 0..2200 {
        fs::write(
            f.root.join(format!("overflow_{module:04}.py")),
            "import importlib\nimportlib.import_module('synthetic_not_imported')\n",
        )
        .unwrap();
    }
    f.install_agent();
    let extracted = success(&f.analyze().arg("--extract-only").output().unwrap());
    let run = Path::new(extracted["runDirectory"].as_str().unwrap());
    assert_eq!(extracted["status"], "extracted");
    assert_eq!(extracted["complete"], true);
    assert_eq!(extracted["error"], Value::Null);
    assert_eq!(extracted["requiresAcceptance"], false);
    let packet_error = extracted["packetError"]
        .as_str()
        .expect("packet failure diagnostic");
    assert!(!packet_error.trim().is_empty());
    assert!(
        packet_error.to_lowercase().contains("metadata"),
        "{packet_error}"
    );
    for field in [
        "evidencePacket",
        "coverageReport",
        "coverage",
        "proposalSha256",
    ] {
        assert_eq!(extracted.get(field), Some(&Value::Null), "{field}");
    }
    let evidence = read_json(&run.join("evidence.json"));
    assert_eq!(evidence["complete"], true);
    let files = evidence["files"].as_array().unwrap();
    assert_eq!(files.len(), 2203);
    assert!(files.len() < 5000);
    assert!(evidence["facts"].as_array().unwrap().len() >= files.len());
    assert_eq!(
        evidence["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["code"] == "dynamic_discovery_unresolved")
            .count(),
        2200
    );
    let mut metadata = evidence.clone();
    metadata["facts"] = serde_json::json!([]);
    assert!(
        serde_json::to_vec(&metadata).unwrap().len() > MAX_BYTES,
        "metadata alone must exceed the packet budget"
    );
    let state = read_json(&run.join("state.json"));
    assert_eq!(state["status"], "extracted");
    assert_eq!(state["complete"], true);
    assert_eq!(state["packetPolicyVersion"], POLICY);
    assert_eq!(state["packetError"], packet_error);
    assert_eq!(state["attempts"], 0);
    assert_eq!(
        state["evidenceSha256"],
        format!(
            "{:x}",
            Sha256::digest(fs::read(run.join("evidence.json")).unwrap())
        )
    );
    for field in ["packetSha256", "coverageSha256", "requestSha256"] {
        assert!(
            state.get(field).is_none_or(Value::is_null),
            "unexpected {field}"
        );
    }
    for name in [
        "evidence-packet.json",
        "evidence-coverage.json",
        "request.json",
        "proposal.json",
    ] {
        assert!(!run.join(name).exists(), "unexpected {name}");
    }
    for name in ["modules.json", "modules.mmd"] {
        assert!(
            run.join(name).is_file(),
            "missing inventory artifact {name}"
        );
    }
    assert!(f.calls().is_empty());
    let original = artifacts(run);
    let output = f.analyze().output().unwrap();
    failure(&output);
    let blocked: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(blocked["runDirectory"], extracted["runDirectory"]);
    assert_eq!(blocked["status"], "extracted");
    assert_eq!(blocked["complete"], true);
    assert_eq!(blocked["packetError"], packet_error);
    assert!(blocked["error"]
        .as_str()
        .is_some_and(|s| !s.trim().is_empty()));
    for field in ["evidencePacket", "coverageReport", "coverage"] {
        assert_eq!(blocked.get(field), Some(&Value::Null), "{field}");
    }
    assert_eq!(
        success(&cli().arg("status").arg(run).output().unwrap()),
        state
    );
    assert_eq!(
        artifacts(run),
        original,
        "blocked analysis changed saved inventory or hashes"
    );
    assert!(
        f.calls().is_empty(),
        "packet preparation failure reached the agent"
    );
    f.safe();
}

#[test]
fn old_extractor_metadata_blocks_first_acceptance_but_reuses_already_accepted_artifacts() {
    for already_accepted in [false, true] {
        let Some(mut f) = Fixture::new(false) else {
            return;
        };
        let python_log = f.base.with_file_name("python-calls.log");
        let wrapper = f.base.with_file_name("tracked-python");
        let quote = |p: &Path| format!("'{}'", p.to_str().unwrap().replace('\'', "'\\''"));
        fs::write(
            &wrapper,
            format!(
                "#!/bin/sh\nprintf 'called\\n' >> {}\nexec {} \"$@\"\n",
                quote(&python_log),
                quote(&f.python)
            ),
        )
        .unwrap();
        fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).unwrap();
        f.python = wrapper;
        success(&f.inspect());
        check_packet(&f.run, false);
        f.install_agent();
        let ready = success(&f.interpret(&f.run).output().unwrap());
        assert_eq!(ready["status"], "proposal_ready");
        let fingerprint = ready["proposalSha256"].as_str().unwrap();
        let accept = || {
            cli()
                .arg("accept")
                .arg(&f.run)
                .arg("--proposal-sha256")
                .arg(fingerprint)
                .output()
                .unwrap()
        };
        if already_accepted {
            assert_eq!(success(&accept())["status"], "accepted");
        }

        // Emulate only the historical extractor-version guard. Keep the actual
        // snapshot, packet, request, proposal and published output identities;
        // this is not a reconstruction of a historical extractor's snapshot.
        let evidence_path = f.run.join("evidence.json");
        let mut evidence = read_json(&evidence_path);
        evidence["extractorVersion"] = "python-static-1".into();
        write_json(&evidence_path, &evidence);
        let state_path = f.run.join("state.json");
        let mut state = read_json(&state_path);
        state["evidenceSha256"] =
            format!("{:x}", Sha256::digest(fs::read(&evidence_path).unwrap())).into();
        write_json(&state_path, &state);
        assert_eq!(state["snapshotId"], ready["snapshotId"]);
        assert_eq!(state["proposalSha256"], fingerprint);
        assert_eq!(
            success(&cli().arg("status").arg(&f.run).output().unwrap()),
            state,
            "emulated state must pass saved-artifact integrity checks"
        );
        let original = artifacts(&f.run);
        let python_calls = fs::read(&python_log).unwrap();
        assert!(
            !python_calls.is_empty(),
            "Python tracking wrapper was not exercised"
        );
        let agent_calls = f.calls();
        assert_eq!(agent_calls.len(), 1);
        let output = accept();
        if already_accepted {
            let reused = success(&output);
            assert_eq!(reused, state);
            assert_eq!(reused["status"], "accepted");
            for (field, artifact) in [
                ("proposalSha256", "proposal.json"),
                ("acceptedSha256", "architecture.json"),
                ("diagramSha256", "diagram.mmd"),
            ] {
                assert_eq!(
                    reused[field],
                    format!(
                        "{:x}",
                        Sha256::digest(fs::read(f.run.join(artifact)).unwrap())
                    ),
                    "changed {field}"
                );
            }
        } else {
            let message = failure(&output).to_lowercase();
            assert!(
                message.contains("extractor") && message.contains("new run"),
                "{message}"
            );
            assert!(!f.run.join("architecture.json").exists());
            assert!(!f.run.join("diagram.mmd").exists());
        }
        assert_eq!(
            artifacts(&f.run),
            original,
            "acceptance rewrote artifacts or fingerprints"
        );
        assert_eq!(
            fs::read(&python_log).unwrap(),
            python_calls,
            "old-extractor acceptance must not re-extract target source"
        );
        assert_eq!(
            f.calls(),
            agent_calls,
            "acceptance dispatched another agent"
        );
        f.safe();
    }
}
