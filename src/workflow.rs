//! Persisted deterministic controller; agents supply proposals, never transitions.
use crate::analysis::{self, Evidence};
use crate::focus::{self, FocusOptions};
use crate::manifest::{Edge, Manifest, Node, SCHEMA_VERSION};
use crate::packet;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::{Duration, Instant};

pub const SKILL: &str = include_str!("../skills/architecture-interpretation/SKILL.md");
const FOCUS_SKILL: &str = include_str!("../skills/workflow-focus/SKILL.md");

pub(crate) fn instructions_for(focused: bool) -> String {
    if focused {
        format!("{SKILL}\n\n{FOCUS_SKILL}")
    } else {
        SKILL.to_string()
    }
}

fn packet_policy(focused: bool) -> &'static str {
    if focused {
        focus::POLICY_VERSION
    } else {
        packet::POLICY_VERSION
    }
}
const PROTOCOL: &str = "agent-execution.v1";
const STATE_VERSION: &str = "ideomancer.run.v1";
const PROPOSAL_VERSION: &str = "ideomancer.architecture.v1";
const FILE_LIMIT: u64 = 40 * 1024 * 1024;
const REQUEST_LIMIT: usize = 16 * 1024 * 1024;
const RESPONSE_LIMIT: usize = 8 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunState {
    schema_version: String,
    run_id: String,
    source_root: PathBuf,
    python: PathBuf,
    pub snapshot_id: String,
    evidence_sha256: String,
    skill_sha256: String,
    pub status: String,
    pub complete: bool,
    modules_sha256: String,
    modules_mermaid_sha256: String,
    attempts: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    agent: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model_requested: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model_reported: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposal_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    accepted_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    diagram_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    agent_timeout_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    elapsed_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    packet_policy_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) packet_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) coverage_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) packet_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) focus: Option<FocusOptions>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Proposal {
    schema_version: String,
    snapshot_id: String,
    name: String,
    components: Vec<Component>,
    relationships: Vec<Relationship>,
    uncertainties: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Component {
    id: String,
    label: String,
    kind: String,
    evidence_ids: Vec<String>,
    rationale: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Relationship {
    from: String,
    to: String,
    label: String,
    evidence_ids: Vec<String>,
    basis: String,
    rationale: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AgentResponse {
    protocol_version: String,
    request_id: String,
    status: String,
    output: Value,
    model: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FailureResponse {
    protocol_version: String,
    request_id: Option<String>,
    status: String,
    error: String,
    error_kind: Option<String>,
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn encoded(value: &impl Serialize) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn read(path: &Path) -> Result<Vec<u8>, String> {
    let meta = fs::symlink_metadata(path).map_err(|e| format!("Read {}: {e}", path.display()))?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > FILE_LIMIT {
        return Err(format!(
            "Artifact {} must be a bounded regular non-symlink file",
            path.display()
        ));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(FILE_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > FILE_LIMIT {
        return Err("Artifact exceeded size limit".into());
    }
    Ok(bytes)
}

fn parse<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, String> {
    serde_json::from_slice(bytes).map_err(|e| format!("Invalid workflow artifact: {e}"))
}

fn write_new(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dir.join(name))
        .map_err(|e| format!("Create {name}: {e}"))?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| format!("Write {name}: {e}"))
}

fn matching_or_absent(dir: &Path, name: &str, bytes: &[u8]) -> Result<bool, String> {
    match fs::symlink_metadata(dir.join(name)) {
        Ok(_) => {
            if read(&dir.join(name))? != bytes {
                return Err(format!(
                    "Conflicting artifact {name}; it was not overwritten"
                ));
            }
            Ok(true)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.to_string()),
    }
}

fn publish(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
    if matching_or_absent(dir, name, bytes)? {
        return Ok(());
    }
    // Stage complete contents, then atomically publish without clobbering.
    let mut temp = tempfile::NamedTempFile::new_in(dir).map_err(|e| e.to_string())?;
    temp.write_all(bytes).map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    match temp.persist_noclobber(dir.join(name)) {
        Ok(_) => Ok(()),
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
            matching_or_absent(dir, name, bytes).map(|_| ())
        }
        Err(error) => Err(error.to_string()),
    }
}

fn replace(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
    if let Ok(meta) = fs::symlink_metadata(dir.join(name)) {
        if !meta.is_file() || meta.file_type().is_symlink() {
            return Err(format!("Unsafe artifact {name}"));
        }
    }
    let mut temp = tempfile::NamedTempFile::new_in(dir).map_err(|e| e.to_string())?;
    temp.write_all(bytes).map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist(dir.join(name)).map_err(|e| e.to_string())?;
    Ok(())
}

fn directory(path: &Path) -> Result<PathBuf, String> {
    let path: PathBuf = path.components().collect();
    let meta = fs::symlink_metadata(&path).map_err(|e| format!("Run directory: {e}"))?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err("Run must be a non-symlink directory".into());
    }
    fs::canonicalize(path).map_err(|e| e.to_string())
}

pub(crate) struct Lock(PathBuf);
impl Lock {
    pub(crate) fn acquire(dir: &Path) -> Result<Self, String> {
        let parent = crate::analyze::create_storage(
            dir.parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new(".")),
        )?;
        let name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("Storage slot needs a UTF-8 name")?;
        let path = parent.join(format!(".{name}.lock"));
        fs::OpenOptions::new().write(true).create_new(true).open(&path)
            .map_err(|e| format!("Storage is locked or unavailable at {}: {e}. After a crash verify its process is stopped and reconcile uncertain agent work before removing the lock.", path.display()))?;
        Ok(Self(path))
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub(crate) fn app_name(root: &Path) -> Result<String, String> {
    let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
    let name = root
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Source root needs a UTF-8 directory name")?;
    let name: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .take(80)
        .collect();
    let name = name.trim_matches('-');
    if name.is_empty() {
        return Err("Source root needs a readable application name".into());
    }
    Ok(name.into())
}

pub(crate) fn reject_legacy_storage(base: &Path) -> Result<(), String> {
    for entry in fs::read_dir(base).map_err(|e| e.to_string())? {
        let name = entry.map_err(|e| e.to_string())?.file_name();
        let name = name.to_string_lossy();
        if name == ".analyze.lock"
            || (name.len() == 64 && name.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err("Legacy generated storage exists; stop old processes and reconcile prior agent work before explicitly clearing it or choosing a new --out. No agent was invoked.".into());
        }
    }
    Ok(())
}

pub(crate) type TreeSnapshot = BTreeMap<PathBuf, Option<String>>;

// Bound both traversal and reads; directories are recorded too, including empty ones.
pub(crate) fn tree_snapshot(root: &Path) -> Result<Option<TreeSnapshot>, String> {
    match fs::symlink_metadata(root) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
        Ok(_) => {}
    }
    let mut snapshot = BTreeMap::new();
    let mut pending = vec![PathBuf::new()];
    let mut total = 0_u64;
    while let Some(relative) = pending.pop() {
        if snapshot.len() >= 10_000 || relative.components().count() > 32 {
            return Err("Storage tree exceeds snapshot bounds".into());
        }
        let path = root.join(&relative);
        let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if meta.file_type().is_symlink() {
            return Err(format!("Unsafe storage symlink: {}", path.display()));
        }
        if meta.is_dir() {
            snapshot.insert(relative.clone(), None);
            for entry in fs::read_dir(&path).map_err(|e| e.to_string())? {
                if pending.len() + snapshot.len() >= 10_000 {
                    return Err("Storage tree exceeds snapshot bounds".into());
                }
                pending.push(relative.join(entry.map_err(|e| e.to_string())?.file_name()));
            }
        } else if meta.is_file() {
            if meta.len() > FILE_LIMIT || total + meta.len() > 256 * 1024 * 1024 {
                return Err("Storage tree exceeds snapshot byte bounds".into());
            }
            let mut options = fs::OpenOptions::new();
            options.read(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.custom_flags(
                    (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32,
                );
            }
            let file = options.open(&path).map_err(|e| e.to_string())?;
            if !file.metadata().map_err(|e| e.to_string())?.is_file() {
                return Err("Storage snapshot requires regular files".into());
            }
            let mut bytes = Vec::new();
            file.take(FILE_LIMIT + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            total += bytes.len() as u64;
            if bytes.len() as u64 > FILE_LIMIT || total > 256 * 1024 * 1024 {
                return Err("Storage tree exceeds snapshot byte bounds".into());
            }
            snapshot.insert(relative, Some(hash(&bytes)));
        } else {
            return Err(format!("Unsafe storage artifact: {}", path.display()));
        }
    }
    Ok(Some(snapshot))
}

fn reconcile_staging(stage: tempfile::TempDir, error: String) -> String {
    let path = stage.keep();
    format!(
        "{error}. Storage was preserved for reconciliation at {}",
        path.display()
    )
}

// Caller holds the external slot lock and captured expected before validating
// the old output. Own the guard so no error can drop a disputed displaced tree.
pub(crate) fn publish_current(
    stage: tempfile::TempDir,
    staging: &Path,
    destination: &Path,
    expected: Option<&TreeSnapshot>,
) -> Result<(), String> {
    let publish = || -> Result<(), String> {
        fs::File::open(staging)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
        if tree_snapshot(destination)?.as_ref() != expected {
            return Err(format!(
                "Storage changed before publication: {}",
                destination.display()
            ));
        }
        match fs::symlink_metadata(destination) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                publish_inspection(staging, destination)
            }
            Err(e) => Err(e.to_string()),
            Ok(meta) if !meta.is_dir() || meta.file_type().is_symlink() => {
                Err("Storage slot must be a non-symlink directory".into())
            }
            Ok(_) => {
                #[cfg(unix)]
                {
                    rustix::fs::renameat_with(
                        rustix::fs::CWD,
                        staging,
                        rustix::fs::CWD,
                        destination,
                        rustix::fs::RenameFlags::EXCHANGE,
                    )
                    .map_err(|e| format!("Atomic storage exchange is unavailable: {e}"))?;
                }
                #[cfg(not(unix))]
                {
                    return Err("Atomic storage exchange requires Unix".into());
                }
                Ok(())
            }
        }?;
        fs::File::open(destination.parent().ok_or("Storage slot has no parent")?)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())
    };
    if let Err(error) = publish() {
        return Err(reconcile_staging(stage, error));
    }
    finish_staging(stage, staging, expected)
}

pub(crate) fn managed_files(
    dir: &Path,
    files: &[&str],
    directories: &[&str],
) -> Result<(), String> {
    directory(dir)?;
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        let name = name.to_str().ok_or("Unrecognized storage filename")?;
        let meta = fs::symlink_metadata(entry.path()).map_err(|e| e.to_string())?;
        if meta.file_type().is_symlink()
            || !(meta.is_file() && files.contains(&name)
                || meta.is_dir() && directories.contains(&name))
        {
            return Err(format!(
                "Unknown or unsafe storage artifact: {name}; reconcile it before replacement"
            ));
        }
    }
    Ok(())
}

fn finish_staging(
    stage: tempfile::TempDir,
    displaced: &Path,
    expected: Option<&TreeSnapshot>,
) -> Result<(), String> {
    match tree_snapshot(displaced) {
        Ok(actual) if actual.as_ref() == expected => {}
        Ok(_) => {
            return Err(reconcile_staging(
                stage,
                "Displaced storage changed; it was not deleted".into(),
            ))
        }
        Err(error) => {
            return Err(reconcile_staging(
                stage,
                format!("Cannot verify displaced storage; it was not deleted: {error}"),
            ))
        }
    }
    let parent = match stage.path().parent() {
        Some(parent) => parent.to_path_buf(),
        None => return Err(reconcile_staging(stage, "Staging has no parent".into())),
    };
    let kept = stage.keep();
    if expected.is_some() {
        fs::remove_dir_all(displaced).map_err(|e| {
            format!(
                "Remove verified displaced storage: {e}; reconcile {}",
                kept.display()
            )
        })?;
    }
    // Analysis stages contain a published child. Never recursively remove the
    // wrapper: a newly added file there must survive too.
    if kept != displaced {
        fs::remove_dir(&kept)
            .map_err(|e| format!("Remove staging wrapper: {e}; reconcile {}", kept.display()))?;
    }
    fs::File::open(parent)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())
}

fn verify(dir: &Path, name: &str, expected: &str) -> Result<Vec<u8>, String> {
    let bytes = read(&dir.join(name))?;
    if hash(&bytes) != expected {
        return Err(format!("Artifact integrity failure: {name}"));
    }
    Ok(bytes)
}

fn load(dir: &Path) -> Result<(RunState, Evidence), String> {
    // Manual inspect directories are caller-owned and never exchanged. Only
    // managed current analyses require an exclusively generated inventory.
    if fs::symlink_metadata(dir.join("identity.json")).is_ok() {
        let mut files = vec![
            "state.json",
            "identity.json",
            "evidence.json",
            "modules.json",
            "modules.mmd",
            "evidence-packet.json",
            "evidence-coverage.json",
            "proposal.json",
            "architecture.json",
            "diagram.mmd",
            "request.json",
        ];
        let attempts: Vec<String> = fs::read_dir(dir)
            .map_err(|e| e.to_string())?
            .map(|entry| entry.map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| {
                name.strip_prefix("attempt-")
                    .and_then(|s| {
                        s.strip_suffix(".request.json")
                            .or_else(|| s.strip_suffix(".result.json"))
                    })
                    .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
            })
            .collect();
        files.extend(attempts.iter().map(String::as_str));
        managed_files(dir, &files, &["presentation"])?;
        if fs::symlink_metadata(dir.join("presentation")).is_ok() {
            crate::presentation::validate(&dir.join("presentation"))?;
        }
    }
    let state: RunState = parse(&read(&dir.join("state.json"))?)?;
    if fs::symlink_metadata(dir.join("identity.json")).is_ok() {
        let identity: Value = parse(&read(&dir.join("identity.json"))?)?;
        let expected_keys = [
            "version",
            "sourceRoot",
            "snapshotId",
            "evidence",
            "python",
            "skill",
            "packetPolicy",
            "agent",
            "model",
            "focus",
        ];
        if identity
            .as_object()
            .is_none_or(|o| o.keys().any(|key| !expected_keys.contains(&key.as_str())))
            || identity["version"] != "analyze.v1"
            || identity["sourceRoot"] != json!(state.source_root)
            || identity["snapshotId"] != state.snapshot_id
            || identity["evidence"] != state.evidence_sha256
            || identity["python"] != json!(state.python)
            || identity["skill"] != state.skill_sha256
            || identity["packetPolicy"] != json!(state.packet_policy_version)
            || identity.get("focus")
                != state
                    .focus
                    .as_ref()
                    .map(serde_json::to_value)
                    .transpose()
                    .map_err(|e| e.to_string())?
                    .as_ref()
            || !identity["agent"].is_string()
            || !(identity["model"].is_string() || identity["model"].is_null())
        {
            return Err("Analysis identity does not match saved workflow state".into());
        }
        if state.attempts > 0
            && (identity["agent"] != json!(state.agent)
                || identity["model"] != json!(state.model_requested))
        {
            return Err("Analysis agent/model identity does not match saved workflow state".into());
        }
    }
    if state.schema_version != STATE_VERSION
        || !matches!(
            state.status.as_str(),
            "extracted" | "running" | "proposal_ready" | "accepted" | "failed" | "indeterminate"
        )
    {
        return Err("Unsupported workflow state".into());
    }
    let evidence: Evidence = parse(&verify(dir, "evidence.json", &state.evidence_sha256)?)?;
    if evidence.validate_language()? != analysis::Language::Python {
        return Err("Agent workflows currently support Python/FastAPI evidence only; use a local JS/TS --view diagram instead".into());
    }
    if evidence.snapshot_id != state.snapshot_id || evidence.complete != state.complete {
        return Err("Evidence does not match workflow state".into());
    }
    verify(dir, "modules.json", &state.modules_sha256)?;
    verify(dir, "modules.mmd", &state.modules_mermaid_sha256)?;
    if let Some(expected) = &state.packet_sha256 {
        let input: Value = parse(&verify(dir, "evidence-packet.json", expected)?)?;
        if input["snapshotId"].as_str() != Some(&state.snapshot_id) {
            return Err("Evidence packet does not match this snapshot".into());
        }
        match &state.focus {
            Some(focus) => {
                if input["focus"]["requestedSymbol"].as_str() != Some(focus.symbol.as_str())
                    || input["focus"]["maxHops"].as_u64() != Some(focus.hops as u64)
                    || input["focus"]["maxFacts"].as_u64() != Some(focus.max_facts as u64)
                    || state.packet_policy_version.as_deref() != Some(focus::POLICY_VERSION)
                {
                    return Err("Focused packet does not match its saved analysis settings".into());
                }
            }
            None if input.get("focus").is_some() => {
                return Err("Overview run contains unexpected focus metadata".into())
            }
            None => {}
        }
        let coverage_hash = state
            .coverage_sha256
            .as_deref()
            .ok_or("Missing packet coverage fingerprint")?;
        let coverage: packet::Coverage =
            parse(&verify(dir, "evidence-coverage.json", coverage_hash)?)?;
        if Some(coverage.policy_version.as_str()) != state.packet_policy_version.as_deref()
            || coverage.input_bytes != serde_json::to_vec(&input).map_err(|e| e.to_string())?.len()
            || coverage.input_bytes > packet::MAX_INPUT_BYTES
        {
            return Err("Evidence packet coverage does not match its saved input".into());
        }
    } else if state.coverage_sha256.is_some()
        || (state.packet_policy_version.is_some() && state.packet_error.is_none())
    {
        return Err("Missing packet artifact or failure explanation".into());
    }
    if let Some(expected) = &state.request_sha256 {
        let file = state
            .request_file
            .as_deref()
            .ok_or("Missing immutable request reference")?;
        if !file.starts_with("attempt-")
            || !file.ends_with(".request.json")
            || file.contains('/')
            || file.contains('\\')
        {
            return Err("Invalid request artifact reference".into());
        }
        verify(dir, file, expected)?;
    }
    if let Some(expected) = &state.proposal_sha256 {
        verify(dir, "proposal.json", expected)?;
    }
    if let Some(expected) = &state.accepted_sha256 {
        verify(dir, "architecture.json", expected)?;
    }
    if let Some(expected) = &state.diagram_sha256 {
        verify(dir, "diagram.mmd", expected)?;
    }
    if matches!(state.status.as_str(), "proposal_ready" | "accepted")
        && state.proposal_sha256.is_none()
    {
        return Err("Reviewable state has no proposal fingerprint".into());
    }
    if state.status == "accepted"
        && (state.accepted_sha256.is_none() || state.diagram_sha256.is_none())
    {
        return Err("Accepted state has no output fingerprints".into());
    }
    Ok((state, evidence))
}

pub fn status(run: &Path) -> Result<RunState, String> {
    let _lock = Lock::acquire(run)?;
    status_locked(run)
}

pub(crate) fn status_locked(run: &Path) -> Result<RunState, String> {
    let dir = directory(run)?;
    load(&dir).map(|(state, _)| state)
}

fn program_path(program: &Path) -> Result<PathBuf, String> {
    if program.is_relative() && program.components().count() > 1 {
        Ok(std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(program))
    } else {
        Ok(program.into())
    }
}

pub fn inspect(root: &Path, out: &Path, python: &Path) -> Result<RunState, String> {
    let evidence = analysis::extract(root, python)?;
    save_inspection(root, out, python, evidence)
}

pub(crate) fn save_inspection(
    root: &Path,
    out: &Path,
    python: &Path,
    evidence: Evidence,
) -> Result<RunState, String> {
    save_inspection_with_focus(root, out, python, evidence, None)
}

pub(crate) fn save_inspection_with_focus(
    root: &Path,
    out: &Path,
    python: &Path,
    evidence: Evidence,
    focus: Option<FocusOptions>,
) -> Result<RunState, String> {
    let _lock = Lock::acquire(out)?;
    save_inspection_locked(root, out, python, evidence, focus)
}

pub(crate) fn save_inspection_locked(
    root: &Path,
    out: &Path,
    python: &Path,
    evidence: Evidence,
    focus: Option<FocusOptions>,
) -> Result<RunState, String> {
    if evidence.validate_language()? != analysis::Language::Python {
        return Err("Agent workflows currently support Python/FastAPI evidence only; use a local JS/TS --view diagram instead".into());
    }
    if evidence.files.is_empty() {
        return Err("No eligible Python sources found".into());
    }
    let source_root = fs::canonicalize(root).map_err(|e| e.to_string())?;
    let python = program_path(python)?;
    let evidence_bytes = encoded(&evidence)?;
    let evidence_sha256 = hash(&evidence_bytes);
    if fs::symlink_metadata(out).is_ok() {
        let dir = directory(out)?;
        let (state, _) = load(&dir)?;
        if state.source_root != source_root
            || state.python != python
            || state.evidence_sha256 != evidence_sha256
            || state.skill_sha256 != hash(instructions_for(focus.is_some()).as_bytes())
            || state.packet_policy_version.as_deref() != Some(packet_policy(focus.is_some()))
            || state.focus != focus
        {
            return Err("Run inputs changed; choose a new --out directory. Existing decisions were not overwritten.".into());
        }
        return Ok(state);
    }
    // Stage the entire initial snapshot before publishing its directory. A crash
    // cannot leave a half-initialized deterministic run path blocking recovery.
    let out = if out.is_absolute() {
        out.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(out)
    };
    let parent = out.parent().ok_or("Run directory has no parent")?;
    let staging = tempfile::Builder::new()
        .prefix(".ideomancer-init-")
        .tempdir_in(parent)
        .map_err(|e| format!("Stage initial inspection (parent must exist): {e}"))?;
    let dir = staging.path();
    let graph = analysis::module_manifest(
        &evidence,
        "Python source modules (not runtime architecture)",
    );
    let modules = encoded(&graph)?;
    let mermaid = crate::render::mermaid::render(&graph);
    let prepared = match &focus {
        Some(options) => focus::prepare(&evidence, options),
        None => packet::prepare(&evidence),
    };
    let (packet_bytes, coverage_bytes, packet_error) = match prepared {
        Ok(prepared) => (
            Some(encoded(&prepared.input)?),
            Some(encoded(&prepared.coverage)?),
            None,
        ),
        Err(error) => (None, None, Some(error)),
    };
    let skill_sha256 = hash(instructions_for(focus.is_some()).as_bytes());
    let policy = packet_policy(focus.is_some());
    let mut identity = serde_json::to_value((
        STATE_VERSION,
        &evidence.snapshot_id,
        &source_root,
        &python,
        &skill_sha256,
        policy,
    ))
    .map_err(|e| e.to_string())?;
    if let Some(focus) = &focus {
        identity
            .as_array_mut()
            .ok_or("Invalid run identity")?
            .push(serde_json::to_value(focus).map_err(|e| e.to_string())?);
    }
    let state = RunState {
        schema_version: STATE_VERSION.into(),
        run_id: hash(&encoded(&identity)?),
        source_root,
        python,
        snapshot_id: evidence.snapshot_id.clone(),
        evidence_sha256,
        skill_sha256,
        status: "extracted".into(),
        complete: evidence.complete,
        modules_sha256: hash(&modules),
        modules_mermaid_sha256: hash(mermaid.as_bytes()),
        attempts: 0,
        request_file: None,
        request_id: None,
        agent: None,
        model_requested: None,
        model_reported: None,
        request_sha256: None,
        proposal_sha256: None,
        accepted_sha256: None,
        diagram_sha256: None,
        error: None,
        agent_timeout_ms: None,
        elapsed_ms: None,
        packet_policy_version: Some(policy.into()),
        packet_sha256: packet_bytes.as_deref().map(hash),
        coverage_sha256: coverage_bytes.as_deref().map(hash),
        packet_error,
        focus,
    };
    write_new(dir, "evidence.json", &evidence_bytes)?;
    write_new(dir, "modules.json", &modules)?;
    write_new(dir, "modules.mmd", mermaid.as_bytes())?;
    if let Some(bytes) = &packet_bytes {
        write_new(dir, "evidence-packet.json", bytes)?;
    }
    if let Some(bytes) = &coverage_bytes {
        write_new(dir, "evidence-coverage.json", bytes)?;
    }
    write_new(dir, "state.json", &encoded(&state)?)?;
    publish_inspection(dir, &out)?;
    Ok(state)
}

#[cfg(unix)]
pub(crate) fn publish_inspection(staging: &Path, destination: &Path) -> Result<(), String> {
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        staging,
        rustix::fs::CWD,
        destination,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(|e| format!("Publish initialized inspection without clobbering: {e}"))
}

#[cfg(not(unix))]
pub(crate) fn publish_inspection(_: &Path, _: &Path) -> Result<(), String> {
    Err("Atomic inspection publication is supported on Unix only".into())
}

/// A checked projection of the saved proposal. Caller holds the external run lock.
pub(crate) fn draft_manifest(run: &Path) -> Result<(RunState, Manifest), String> {
    let dir = directory(run)?;
    let (state, evidence) = load(&dir)?;
    if !matches!(state.status.as_str(), "proposal_ready" | "accepted") {
        return Err("A draft requires a validated reviewable proposal".into());
    }
    let fingerprint = state
        .proposal_sha256
        .as_deref()
        .ok_or("Missing proposal fingerprint")?;
    let proposal: Proposal = parse(&verify(&dir, "proposal.json", fingerprint)?)?;
    validate_sent_refs(&proposal, &recorded_input(&dir, &state)?)?;
    let mut graph = validate_proposal(&proposal, &evidence)?;
    graph.id = format!("draft_{}", graph.id);
    graph.name = format!("DRAFT — architecture proposal: {}", graph.name);
    Ok((state, graph))
}

/// Read a checked, saved proposal and its citations. No live source scan, agent,
/// authentication or acceptance is involved. Caller holds the external run lock.
pub(crate) fn presentation_input(run: &Path) -> Result<crate::render::presentation::Input, String> {
    use crate::render::presentation::{
        Component as ViewComponent, EvidenceRef, Input, Relationship as ViewRelationship,
    };
    let dir = directory(run)?;
    let (state, evidence) = load(&dir)?;
    if !matches!(state.status.as_str(), "proposal_ready" | "accepted") {
        return Err("Presentation requires a validated saved proposal".into());
    }
    let fingerprint = state
        .proposal_sha256
        .as_deref()
        .ok_or("Missing proposal fingerprint")?;
    let proposal: Proposal = parse(&verify(&dir, "proposal.json", fingerprint)?)?;
    validate_proposal(&proposal, &evidence)?;
    validate_sent_refs(&proposal, &recorded_input(&dir, &state)?)?;
    let facts: BTreeMap<_, _> = evidence.facts.iter().map(|f| (f.id.as_str(), f)).collect();
    let refs = |ids: &[String]| -> Result<Vec<EvidenceRef>, String> {
        ids.iter()
            .map(|id| {
                let fact = facts
                    .get(id.as_str())
                    .ok_or("Missing presentation evidence")?;
                Ok(EvidenceRef {
                    id: fact.id.clone(),
                    path: fact.path.clone(),
                    kind: fact.kind.clone(),
                    symbol: fact.symbol.clone(),
                    summary: fact.value.clone(),
                    line: fact.line,
                    end_line: fact.end_line,
                })
            })
            .collect()
    };
    let components = proposal
        .components
        .iter()
        .map(|node| {
            Ok(ViewComponent {
                id: node.id.clone(),
                label: node.label.clone(),
                kind: node.kind.clone(),
                rationale: node.rationale.clone(),
                evidence: refs(&node.evidence_ids)?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let relationships = proposal
        .relationships
        .iter()
        .map(|edge| {
            Ok(ViewRelationship {
                from: edge.from.clone(),
                to: edge.to.clone(),
                label: edge.label.clone(),
                basis: edge.basis.clone(),
                rationale: edge.rationale.clone(),
                evidence: refs(&edge.evidence_ids)?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Input {
        title: proposal.name,
        snapshot_id: state.snapshot_id,
        proposal_sha256: fingerprint.to_string(),
        components,
        relationships,
        uncertainties: proposal.uncertainties,
    })
}

/// Ground a software-plan discussion in immutable architecture artifacts. Manual
/// citations in the plan are text only; no application, test or constraint file
/// is opened here, and architecture acceptance is not implementation permission.
pub(crate) fn planning_context(run: &Path) -> Result<crate::plan::ArchitectureContext, String> {
    let _lock = Lock::acquire(run)?;
    let dir = directory(run)?;
    // Coordinate with replacement of the current analysis before reading it.
    let (state, evidence) = load(&dir)?;
    if !matches!(state.status.as_str(), "proposal_ready" | "accepted") {
        return Err("Software planning needs a validated saved architecture proposal".into());
    }
    let fingerprint = state
        .proposal_sha256
        .as_deref()
        .ok_or("Missing architecture proposal fingerprint")?;
    let proposal: Proposal = parse(&verify(&dir, "proposal.json", fingerprint)?)?;
    validate_proposal(&proposal, &evidence)?;
    let input = recorded_input(&dir, &state)?;
    validate_sent_refs(&proposal, &input)?;
    let sent = input["facts"]
        .as_array()
        .ok_or("Missing recorded architecture facts")?;
    let full: BTreeMap<_, _> = evidence
        .facts
        .iter()
        .map(|fact| (fact.id.as_str(), fact))
        .collect();
    let mut facts = BTreeMap::new();
    for value in sent {
        let id = value["id"]
            .as_str()
            .ok_or("Invalid recorded fact identity")?;
        let fact = full
            .get(id)
            .ok_or("Recorded fact is absent from the saved inventory")?;
        facts.insert(
            id.to_string(),
            crate::plan::FactReference {
                path: fact.path.clone(),
                line: fact.line,
                end_line: fact.end_line,
                kind: fact.kind.clone(),
                symbol: fact.symbol.clone(),
            },
        );
    }
    Ok(crate::plan::ArchitectureContext {
        snapshot_id: state.snapshot_id,
        proposal_sha256: fingerprint.to_string(),
        status: state.status,
        component_ids: proposal
            .components
            .into_iter()
            .map(|component| component.id)
            .collect(),
        facts,
    })
}

fn response_schema(snapshot: &str) -> Value {
    let text = json!({"type":"string", "pattern":"\\S", "maxLength":4096});
    let refs = json!({"type":"array", "minItems":1, "maxItems":128, "uniqueItems":true, "items":{"type":"string"}});
    json!({
        "$schema":"http://json-schema.org/draft-07/schema#", "type":"object", "additionalProperties":false,
        "required":["schemaVersion","snapshotId","name","components","relationships","uncertainties"],
        "properties":{
            "schemaVersion":{"const":PROPOSAL_VERSION}, "snapshotId":{"const":snapshot}, "name":text,
            "components":{"type":"array","minItems":1,"maxItems":40,"items":{
                "type":"object","additionalProperties":false,"required":["id","label","kind","evidenceIds","rationale"],
                "properties":{"id":{"type":"string","pattern":"^[A-Za-z_][A-Za-z0-9_]*$","maxLength":128},
                    "label":text,"kind":text,"evidenceIds":refs,"rationale":text}}},
            "relationships":{"type":"array","maxItems":160,"items":{
                "type":"object","additionalProperties":false,"required":["from","to","label","evidenceIds","basis","rationale"],
                "properties":{"from":text,"to":text,"label":text,"evidenceIds":refs,
                    "basis":{"enum":["observed","inferred"]},"rationale":text}}},
            "uncertainties":{"type":"array","maxItems":128,"items":text}
        }
    })
}

#[cfg(test)]
fn agent_input(evidence: &Evidence) -> Result<Value, String> {
    packet::prepare(evidence).map(|prepared| prepared.input)
}

pub(crate) fn packet_coverage(
    run: &Path,
    state: &RunState,
) -> Result<Option<packet::Coverage>, String> {
    state
        .coverage_sha256
        .as_deref()
        .map(|expected| parse(&verify(run, "evidence-coverage.json", expected)?))
        .transpose()
}

fn stored_packet(run: &Path, state: &RunState) -> Result<Value, String> {
    if state.packet_policy_version.as_deref() != Some(packet_policy(state.focus.is_some())) {
        return Err("Packet policy changed or is missing; create a new analysis run. Saved proposals were not replaced.".into());
    }
    if let Some(error) = &state.packet_error {
        return Err(format!("Evidence packet unavailable: {error}"));
    }
    let expected = state
        .packet_sha256
        .as_deref()
        .ok_or("Missing evidence packet fingerprint")?;
    parse(&verify(run, "evidence-packet.json", expected)?)
}

fn recorded_input(run: &Path, state: &RunState) -> Result<Value, String> {
    let name = state
        .request_file
        .as_deref()
        .ok_or("Missing recorded request")?;
    let fingerprint = state
        .request_sha256
        .as_deref()
        .ok_or("Missing request fingerprint")?;
    let request: Value = parse(&verify(run, name, fingerprint)?)?;
    request
        .get("input")
        .cloned()
        .ok_or("Recorded request has no evidence input".into())
}

fn validate_sent_refs(proposal: &Proposal, input: &Value) -> Result<(), String> {
    let facts = input["facts"]
        .as_array()
        .ok_or("Recorded input has no evidence facts")?;
    let allowed: BTreeSet<_> = facts
        .iter()
        .filter_map(|fact| fact["id"].as_str())
        .collect();
    for id in proposal
        .components
        .iter()
        .flat_map(|c| &c.evidence_ids)
        .chain(proposal.relationships.iter().flat_map(|r| &r.evidence_ids))
    {
        if !allowed.contains(id.as_str()) {
            return Err("Proposal cites evidence that was not delivered in this packet".into());
        }
    }
    Ok(())
}

fn validate_proposal(proposal: &Proposal, evidence: &Evidence) -> Result<Manifest, String> {
    if proposal.schema_version != PROPOSAL_VERSION || proposal.snapshot_id != evidence.snapshot_id {
        return Err("Proposal schema/snapshot does not match this run".into());
    }
    if proposal.components.is_empty()
        || proposal.components.len() > 40
        || proposal.relationships.len() > 160
        || proposal.uncertainties.len() > 128
    {
        return Err("Proposal component/relationship bounds exceeded".into());
    }
    let facts: BTreeSet<_> = evidence.facts.iter().map(|fact| fact.id.as_str()).collect();
    let text = |value: &str| -> Result<(), String> {
        if value.trim().is_empty() || value.chars().count() > 4096 {
            Err("Proposal text must be nonblank and at most 4096 characters".into())
        } else {
            Ok(())
        }
    };
    let check_refs = |refs: &[String]| -> Result<(), String> {
        let unique: BTreeSet<_> = refs.iter().collect();
        if refs.is_empty()
            || refs.len() > 128
            || unique.len() != refs.len()
            || refs.iter().any(|id| !facts.contains(id.as_str()))
        {
            Err("Proposal references missing, duplicate or invalid evidence IDs".into())
        } else {
            Ok(())
        }
    };
    text(&proposal.name)?;
    for component in &proposal.components {
        if component.id.len() > 128 {
            return Err("Component ID is too long".into());
        }
        text(&component.label)?;
        text(&component.kind)?;
        text(&component.rationale)?;
        check_refs(&component.evidence_ids)?;
    }
    for edge in &proposal.relationships {
        text(&edge.from)?;
        text(&edge.to)?;
        text(&edge.label)?;
        text(&edge.rationale)?;
        check_refs(&edge.evidence_ids)?;
        if !matches!(edge.basis.as_str(), "observed" | "inferred") {
            return Err("Unknown relationship basis".into());
        }
    }
    for uncertainty in &proposal.uncertainties {
        text(uncertainty)?;
    }
    let graph = Manifest {
        schema_version: SCHEMA_VERSION.into(),
        id: format!("architecture_{}", evidence.snapshot_id),
        name: proposal.name.clone(),
        nodes: proposal
            .components
            .iter()
            .map(|node| Node {
                id: node.id.clone(),
                label: Some(node.label.clone()),
                kind: Some(node.kind.clone()),
            })
            .collect(),
        edges: proposal
            .relationships
            .iter()
            .map(|edge| Edge {
                from: edge.from.clone(),
                to: edge.to.clone(),
                label: Some(format!("{} [{}]", edge.label, edge.basis)),
            })
            .collect(),
    };
    let errors = crate::validate::validate_manifest(&graph);
    if !errors.is_empty() {
        return Err(format!("Invalid proposal graph: {}", errors.join("; ")));
    }
    Ok(graph)
}

#[derive(Debug)]
struct AgentFailure {
    uncertain: bool,
    message: String,
}

fn agent_budget_ms(wall_seconds: u64) -> u64 {
    let wall = wall_seconds * 1000;
    // Leave time for process startup, local validation and the final protocol
    // document before the controller's hard deadline. Small test/CLI budgets
    // retain 90%; normal requests reserve one second.
    wall - (wall / 10).min(1000)
}

fn bounded(pipe: impl Read, limit: usize, overflow: Arc<AtomicBool>) -> Vec<u8> {
    let mut bytes = Vec::new();
    if pipe
        .take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.len() > limit
    {
        overflow.store(true, Ordering::SeqCst);
    }
    bytes
}

fn execute(agent: &Path, request: &Path, seconds: u64) -> Result<(bool, Vec<u8>), AgentFailure> {
    let fail = |message: &str| AgentFailure {
        uncertain: false,
        message: message.into(),
    };
    let neutral =
        tempfile::tempdir().map_err(|_| fail("Cannot create neutral agent working directory"))?;
    let mut child = Command::new(agent).args(["execute", "--request"]).arg(request).arg("--json")
        .current_dir(neutral.path()).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().map_err(|_| fail("Cannot start agent executable; build Djinn with execute support or select a protocol-compatible --agent"))?;
    let overflow = Arc::new(AtomicBool::new(false));
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let out_flag = overflow.clone();
    let err_flag = overflow.clone();
    let out = thread::spawn(move || bounded(stdout, RESPONSE_LIMIT, out_flag));
    let err = thread::spawn(move || bounded(stderr, 64 * 1024, err_flag));
    let start = Instant::now();
    let result = loop {
        if start.elapsed() > Duration::from_secs(seconds) {
            let _ = child.kill();
            let _ = child.wait();
            break Err(AgentFailure { uncertain: true, message: "Agent deadline elapsed; remote completion is unknown. Reconcile before explicit --retry.".into() });
        }
        if overflow.load(Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            break Err(fail("Agent output exceeded limits; no automatic retry"));
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status.success()),
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(AgentFailure {
                    uncertain: true,
                    message: "Cannot determine agent completion".into(),
                });
            }
        }
    };
    // Do not wait indefinitely for pipes inherited by adapter descendants. Normal
    // Djinn execution has none; arbitrary adapters are trusted operator programs.
    if result.is_err() {
        return result.map(|success| (success, Vec::new()));
    }
    while !out.is_finished() || !err.is_finished() {
        if start.elapsed() > Duration::from_secs(seconds) {
            return Err(AgentFailure {
                uncertain: true,
                message: "Agent descendants retained output pipes; reconcile before --retry".into(),
            });
        }
        thread::sleep(Duration::from_millis(10));
    }
    let output = out.join().map_err(|_| fail("Agent output reader failed"))?;
    let _ = err.join();
    if overflow.load(Ordering::SeqCst) {
        return Err(fail("Agent output exceeded limits"));
    }
    Ok((result.expect("checked success"), output))
}

pub fn interpret(
    run: &Path,
    agent: &Path,
    model: Option<String>,
    seconds: u64,
    retry: bool,
) -> Result<RunState, String> {
    let _lock = Lock::acquire(run)?;
    interpret_locked(run, agent, model, seconds, retry)
}

pub(crate) fn interpret_locked(
    run: &Path,
    agent: &Path,
    model: Option<String>,
    seconds: u64,
    retry: bool,
) -> Result<RunState, String> {
    if seconds == 0 || seconds > 600 || model.as_ref().is_some_and(|m| m.trim().is_empty()) {
        return Err("Invalid timeout or model".into());
    }
    let dir = directory(run)?;
    let (mut state, evidence) = load(&dir)?;
    if !state.complete {
        return Err("Incomplete source evidence cannot be interpreted; resolve diagnostics and create a new run".into());
    }
    if state.skill_sha256 != hash(instructions_for(state.focus.is_some()).as_bytes()) {
        return Err("Built-in skill changed; create a new run".into());
    }
    let input = stored_packet(&dir, &state)?;
    let agent = program_path(agent)?;
    if fs::symlink_metadata(dir.join("identity.json")).is_ok() {
        let identity: Value = parse(&read(&dir.join("identity.json"))?)?;
        if identity["agent"] != json!(agent) || identity["model"] != json!(model) {
            return Err("Managed analysis agent/model changed; reconcile the saved invocation before analyzing different inputs".into());
        }
    }
    if matches!(state.status.as_str(), "proposal_ready" | "accepted") {
        if state.agent.as_ref() != Some(&agent) || state.model_requested != model {
            return Err(
                "Agent/model changed; use a new run rather than overwrite reviewed decisions"
                    .into(),
            );
        }
        let proposal: Proposal = parse(&verify(
            &dir,
            "proposal.json",
            state
                .proposal_sha256
                .as_deref()
                .ok_or("Missing proposal fingerprint")?,
        )?)?;
        validate_proposal(&proposal, &evidence)?;
        validate_sent_refs(&proposal, &input)?;
        return Ok(state);
    }
    if matches!(
        state.status.as_str(),
        "failed" | "indeterminate" | "running"
    ) && !retry
    {
        return Err("Prior agent work failed or is uncertain. Reconcile its outcome, then use explicit --retry if another call is intended.".into());
    }
    let mut attempt = state
        .attempts
        .checked_add(1)
        .ok_or("Attempt counter overflow")?;
    // Preserve requests orphaned by interruption before the state commit.
    loop {
        let request = dir.join(format!("attempt-{attempt:04}.request.json"));
        let result = dir.join(format!("attempt-{attempt:04}.result.json"));
        if fs::symlink_metadata(request).is_err() && fs::symlink_metadata(result).is_err() {
            break;
        }
        attempt = attempt.checked_add(1).ok_or("Attempt counter overflow")?;
    }
    let request_id = hash(&encoded(&(&state.run_id, attempt, &agent, &model))?);
    let request_file = format!("attempt-{attempt:04}.request.json");
    let timeout_ms = agent_budget_ms(seconds);
    let instructions = instructions_for(state.focus.is_some());
    let mut request = json!({"protocolVersion":PROTOCOL,"requestId":request_id,"instructions":instructions,"timeoutMs":timeout_ms,
        "input":input,"responseSchema":response_schema(&state.snapshot_id)});
    if let Some(model) = &model {
        request["model"] = Value::String(model.clone());
    }
    let bytes = encoded(&request)?;
    if bytes.len() > REQUEST_LIMIT {
        return Err(
            "Evidence request exceeds 16 MiB; narrow the source root. No model was invoked.".into(),
        );
    }
    publish(&dir, &request_file, &bytes)?;
    state.agent = Some(agent.clone());
    state.model_requested = model;
    state.request_sha256 = Some(hash(&bytes));
    state.request_file = Some(request_file.clone());
    state.request_id = Some(request_id.clone());
    state.attempts = attempt;
    state.status = "running".into();
    state.error = None;
    state.agent_timeout_ms = Some(timeout_ms);
    state.elapsed_ms = None;
    replace(&dir, "state.json", &encoded(&state)?)?;
    // request.json is a convenience copy, never the adapter's execution input.
    // Committed state references an immutable attempt file, so interruption here
    // cannot corrupt the prior attempt's request hash.
    let started = Instant::now();
    let result = replace(&dir, "request.json", &bytes)
        .map_err(|message| AgentFailure { uncertain:false, message })
        .and_then(|_| execute(&agent, &dir.join(&request_file), seconds))
        .and_then(|(success, bytes)| {
        let failure = |message: String| AgentFailure {
            uncertain: false,
            message,
        };
        if let Ok(response) = serde_json::from_slice::<FailureResponse>(&bytes) {
            if response.protocol_version == PROTOCOL && response.request_id.as_deref() == Some(&request_id) && response.status == "failed" {
                let message: String = response.error.chars().filter(|c| !c.is_control()).take(1024).collect();
                return Err(AgentFailure {
                    uncertain: response.error_kind.as_deref() == Some("indeterminate"),
                    message: format!("Agent failure: {message}. No automatic retry."),
                });
            }
        }
        if !success { return Err(failure("Agent exited unsuccessfully without a valid failure envelope. No automatic retry.".into())); }
        let response: AgentResponse = serde_json::from_slice(&bytes).map_err(|_| {
            failure("Agent did not return one strict protocol JSON response".into())
        })?;
        if response.protocol_version != PROTOCOL
            || response.request_id != request_id
            || response.status != "completed"
            || response.model.trim().is_empty()
        {
            return Err(failure(
                "Agent response protocol, request ID, status or model is invalid".into(),
            ));
        }
        let proposal: Proposal = serde_json::from_value(response.output)
            .map_err(|_| failure("Agent returned an invalid proposal structure".into()))?;
        validate_proposal(&proposal, &evidence).map_err(failure)?;
        validate_sent_refs(&proposal, &input).map_err(failure)?;
        Ok((proposal, response.model))
    });
    let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    state.elapsed_ms = Some(elapsed_ms);
    match result {
        Ok((proposal, model)) => {
            let bytes = encoded(&proposal)?;
            replace(&dir, "proposal.json", &bytes)?;
            state.proposal_sha256 = Some(hash(&bytes));
            state.model_reported = Some(model);
            state.status = "proposal_ready".into();
            publish(
                &dir,
                &format!("attempt-{attempt:04}.result.json"),
                &encoded(
                    &json!({"requestId":request_id,"status":state.status,"proposalSha256":state.proposal_sha256,"model":state.model_reported,"timeoutMs":timeout_ms,"elapsedMs":elapsed_ms}),
                )?,
            )?;
            replace(&dir, "state.json", &encoded(&state)?)?;
            Ok(state)
        }
        Err(failure) => {
            state.status = if failure.uncertain {
                "indeterminate"
            } else {
                "failed"
            }
            .into();
            state.error = Some(failure.message.clone());
            publish(
                &dir,
                &format!("attempt-{attempt:04}.result.json"),
                &encoded(
                    &json!({"requestId":request_id,"status":state.status,"error":state.error,"timeoutMs":timeout_ms,"elapsedMs":elapsed_ms}),
                )?,
            )?;
            replace(&dir, "state.json", &encoded(&state)?)?;
            Err(failure.message)
        }
    }
}

pub fn accept(run: &Path, fingerprint: &str) -> Result<RunState, String> {
    let _lock = Lock::acquire(run)?;
    let dir = directory(run)?;
    let (mut state, evidence) = load(&dir)?;
    if !matches!(state.status.as_str(), "proposal_ready" | "accepted")
        || state.proposal_sha256.as_deref() != Some(fingerprint)
    {
        return Err("Acceptance requires the exact proposalSha256 of a reviewable proposal".into());
    }
    // Parse exactly the bytes covered by the user's review fingerprint; an
    // editor replacing the pathname after this check cannot change our input.
    let proposal: Proposal = parse(&verify(&dir, "proposal.json", fingerprint)?)?;
    validate_sent_refs(&proposal, &recorded_input(&dir, &state)?)?;
    let graph = validate_proposal(&proposal, &evidence)?;
    if !state.complete {
        return Err("Incomplete evidence cannot be accepted".into());
    }
    if state.status == "accepted" {
        return Ok(state);
    }
    if evidence.extractor_version != analysis::EXTRACTOR_VERSION {
        return Err("Extractor changed since inspection; generate and review a new run before first acceptance".into());
    }
    let current = analysis::extract(&state.source_root, &state.python)?;
    if !current.complete || current.snapshot_id != state.snapshot_id {
        return Err(
            "Source changed since inspection; create a new run and review a fresh proposal".into(),
        );
    }
    let manifest = encoded(&graph)?;
    let mermaid = crate::render::mermaid::render(&graph);
    // Preflight all destinations. Identical staged outputs from an interrupted
    // acceptance are reusable; different existing files are never overwritten.
    matching_or_absent(&dir, "architecture.json", &manifest)?;
    matching_or_absent(&dir, "diagram.mmd", mermaid.as_bytes())?;
    publish(&dir, "architecture.json", &manifest)?;
    publish(&dir, "diagram.mmd", mermaid.as_bytes())?;
    state.accepted_sha256 = Some(hash(&manifest));
    state.diagram_sha256 = Some(hash(mermaid.as_bytes()));
    state.status = "accepted".into();
    replace(&dir, "state.json", &encoded(&state)?)?;
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caller_budget_reserves_protocol_delivery_time() {
        for (wall, agent) in [
            (1, 900),
            (3, 2700),
            (10, 9000),
            (300, 299000),
            (600, 599000),
        ] {
            assert_eq!(agent_budget_ms(wall), agent);
            assert!(agent < wall * 1000);
            assert!((100..=600000).contains(&agent));
        }
    }

    #[cfg(unix)]
    #[test]
    fn current_storage_exchanges_complete_directories_and_removes_old_contents() {
        let parent = tempfile::tempdir().unwrap();
        let destination = parent.path().join("readable-app");
        fs::create_dir(&destination).unwrap();
        write_new(&destination, "old.json", b"old complete contents").unwrap();
        let lock = Lock::acquire(&destination).unwrap();
        assert!(Lock::acquire(&destination).is_err());
        let expected = tree_snapshot(&destination).unwrap();
        let staging = tempfile::tempdir_in(parent.path()).unwrap();
        write_new(staging.path(), "new.json", b"new complete contents").unwrap();
        let staged = staging.path().to_path_buf();
        publish_current(staging, &staged, &destination, expected.as_ref()).unwrap();
        assert_eq!(
            fs::read(destination.join("new.json")).unwrap(),
            b"new complete contents"
        );
        assert!(!destination.join("old.json").exists());
        assert!(!staged.exists());
        assert!(
            Lock::acquire(&destination).is_err(),
            "exchange moved the lock"
        );
        drop(lock);
        assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn current_storage_refuses_symlink_destinations_without_touching_targets() {
        let parent = tempfile::tempdir().unwrap();
        let target = parent.path().join("target");
        fs::create_dir(&target).unwrap();
        write_new(&target, "sentinel", b"preserve").unwrap();
        let destination = parent.path().join("app");
        std::os::unix::fs::symlink(&target, &destination).unwrap();
        let staging = tempfile::tempdir_in(parent.path()).unwrap();
        write_new(staging.path(), "new", b"new").unwrap();
        let staged = staging.path().to_path_buf();
        let error = publish_current(staging, &staged, &destination, None).unwrap_err();
        assert!(error.contains("reconciliation"));
        assert!(staged.join("new").is_file());
        assert_eq!(fs::read(target.join("sentinel")).unwrap(), b"preserve");
        assert_eq!(fs::read_dir(target).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn displaced_tree_changes_are_preserved_before_recursive_cleanup() {
        for change in ["added", "modified", "symlink"] {
            let parent = tempfile::tempdir().unwrap();
            let destination = parent.path().join("app");
            fs::create_dir(&destination).unwrap();
            write_new(&destination, "old.json", b"validated old contents").unwrap();
            let expected = tree_snapshot(&destination).unwrap();
            let stage = tempfile::tempdir_in(parent.path()).unwrap();
            write_new(stage.path(), "new.json", b"new contents").unwrap();
            let staged = stage.path().to_path_buf();
            // Exercise the check after EXCHANGE, including edits that the
            // pre-publication comparison could not have observed.
            rustix::fs::renameat_with(
                rustix::fs::CWD,
                &staged,
                rustix::fs::CWD,
                &destination,
                rustix::fs::RenameFlags::EXCHANGE,
            )
            .unwrap();
            match change {
                "added" => fs::write(staged.join("notes.txt"), b"manual notes").unwrap(),
                "modified" => fs::write(staged.join("old.json"), b"manual edits").unwrap(),
                "symlink" => std::os::unix::fs::symlink(&destination, staged.join("link")).unwrap(),
                _ => unreachable!(),
            }
            let error = finish_staging(stage, &staged, expected.as_ref()).unwrap_err();
            assert!(error.contains(&staged.display().to_string()));
            assert!(error.contains("reconciliation"));
            assert!(staged.join("old.json").is_file());
            match change {
                "added" => assert_eq!(fs::read(staged.join("notes.txt")).unwrap(), b"manual notes"),
                "modified" => {
                    assert_eq!(fs::read(staged.join("old.json")).unwrap(), b"manual edits")
                }
                "symlink" => assert!(fs::symlink_metadata(staged.join("link"))
                    .unwrap()
                    .file_type()
                    .is_symlink()),
                _ => unreachable!(),
            }
            assert_eq!(
                fs::read(destination.join("new.json")).unwrap(),
                b"new contents"
            );
        }
    }

    #[test]
    fn changed_destination_is_not_exchanged_and_candidate_is_kept() {
        let parent = tempfile::tempdir().unwrap();
        let destination = parent.path().join("app");
        fs::create_dir(&destination).unwrap();
        write_new(&destination, "old.json", b"old").unwrap();
        let expected = tree_snapshot(&destination).unwrap();
        let stage = tempfile::tempdir_in(parent.path()).unwrap();
        write_new(stage.path(), "new.json", b"new").unwrap();
        let staged = stage.path().to_path_buf();
        fs::write(destination.join("notes.txt"), b"manual notes").unwrap();
        let error = publish_current(stage, &staged, &destination, expected.as_ref()).unwrap_err();
        assert!(error.contains(&staged.display().to_string()));
        assert_eq!(
            fs::read(destination.join("notes.txt")).unwrap(),
            b"manual notes"
        );
        assert!(staged.join("new.json").is_file());
        assert!(!destination.join("new.json").exists());
    }

    #[cfg(unix)]
    #[test]
    fn initial_inspection_publishes_only_a_complete_directory_without_clobbering() {
        let parent = tempfile::tempdir().unwrap();
        let staging = tempfile::tempdir_in(parent.path()).unwrap();
        let destination = parent.path().join("run");
        write_new(staging.path(), "evidence.json", b"staged evidence").unwrap();
        assert!(!destination.exists());
        write_new(staging.path(), "state.json", b"complete initial state").unwrap();
        publish_inspection(staging.path(), &destination).unwrap();
        assert_eq!(
            fs::read(destination.join("evidence.json")).unwrap(),
            b"staged evidence"
        );
        assert_eq!(
            fs::read(destination.join("state.json")).unwrap(),
            b"complete initial state"
        );
        let replacement = tempfile::tempdir_in(parent.path()).unwrap();
        write_new(replacement.path(), "state.json", b"replacement").unwrap();
        assert!(publish_inspection(replacement.path(), &destination).is_err());
        assert_eq!(
            fs::read(destination.join("state.json")).unwrap(),
            b"complete initial state"
        );
        let empty = parent.path().join("unrelated-empty-directory");
        fs::create_dir(&empty).unwrap();
        assert!(publish_inspection(replacement.path(), &empty).is_err());
        assert_eq!(fs::read_dir(empty).unwrap().count(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn abandoned_initial_staging_does_not_occupy_the_deterministic_run_path() {
        let parent = tempfile::tempdir().unwrap();
        let abandoned = parent.path().join(".ideomancer-init-abandoned");
        fs::create_dir(&abandoned).unwrap();
        write_new(&abandoned, "evidence.json", b"interrupted partial write").unwrap();
        let destination = parent.path().join("run");
        assert!(!destination.exists());
        let fresh = tempfile::tempdir_in(parent.path()).unwrap();
        write_new(fresh.path(), "state.json", b"new complete state").unwrap();
        publish_inspection(fresh.path(), &destination).unwrap();
        assert_eq!(
            fs::read(destination.join("state.json")).unwrap(),
            b"new complete state"
        );
        assert!(abandoned.join("evidence.json").exists());
    }

    #[test]
    fn unsupported_language_metadata_never_initializes_an_agent_workflow() {
        let root = tempfile::tempdir().unwrap();
        let out = root.path().join("run");
        let js = Evidence {
            schema_version: "ideomancer.evidence.v2".into(),
            extractor_version: crate::javascript::EXTRACTOR_VERSION.into(),
            language: Some(analysis::LanguageMetadata {
                name: "javascript-typescript".into(),
                parser: "typescript".into(),
                parser_version: "5.8.3".into(),
                parser_sha256: "a".repeat(64),
                runtime: "node".into(),
                runtime_version: "v24.11.1".into(),
                scan_policy: crate::javascript::SCAN_POLICY.into(),
            }),
            ..Evidence::default()
        };
        assert!(
            save_inspection_locked(root.path(), &out, Path::new("missing-python"), js, None)
                .unwrap_err()
                .contains("Python/FastAPI evidence only")
        );
        assert!(!out.exists());
        let missing_python_version = Evidence {
            schema_version: "ideomancer.evidence.v1".into(),
            extractor_version: analysis::EXTRACTOR_VERSION.into(),
            ..Evidence::default()
        };
        assert!(save_inspection_locked(
            root.path(),
            &out,
            Path::new("missing-python"),
            missing_python_version,
            None
        )
        .unwrap_err()
        .contains("language metadata"));
        assert!(!out.exists());
    }

    #[test]
    fn interpretation_packet_is_deterministic_bounded_and_explicitly_partial() {
        let evidence = Evidence {
            schema_version: "ideomancer.evidence.v1".into(),
            snapshot_id: "snapshot".into(),
            extractor_version: analysis::EXTRACTOR_VERSION.into(),
            python_version: "3.12.0".into(),
            language: None,
            files: vec![analysis::SourceFile {
                path: "app.py".into(),
                sha256: "hash".into(),
            }],
            facts: (0..4000)
                .map(|i| analysis::Fact {
                    id: format!("fact_{i}"),
                    kind: if i == 3999 { "module" } else { "call" }.into(),
                    path: "app.py".into(),
                    line: i + 1,
                    end_line: i + 1,
                    symbol: format!("app.function{i}"),
                    value: "x".repeat(256),
                })
                .collect(),
            diagnostics: vec![],
            complete: true,
        };
        let input = agent_input(&evidence).unwrap();
        assert_eq!(input, agent_input(&evidence).unwrap());
        assert!(serde_json::to_vec(&input).unwrap().len() <= packet::MAX_INPUT_BYTES);
        assert!(input["selection"]["omittedFacts"].as_u64().unwrap() > 0);
        assert!(input["facts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|fact| fact["id"] == "fact_3999"));
        let known: BTreeSet<_> = evidence.facts.iter().map(|fact| fact.id.as_str()).collect();
        assert!(input["facts"]
            .as_array()
            .unwrap()
            .iter()
            .all(|fact| known.contains(fact["id"].as_str().unwrap())));
    }

    #[test]
    fn interpretation_rejects_metadata_that_leaves_no_usable_fact_budget() {
        let evidence = Evidence {
            schema_version: "ideomancer.evidence.v1".into(),
            snapshot_id: "snapshot".into(),
            extractor_version: analysis::EXTRACTOR_VERSION.into(),
            python_version: "3.12.0".into(),
            language: None,
            files: vec![],
            complete: true,
            facts: vec![analysis::Fact {
                id: "module_a".into(),
                kind: "module".into(),
                path: "a.py".into(),
                line: 1,
                end_line: 1,
                symbol: "a".into(),
                value: "a".repeat(70_000),
            }],
            diagnostics: vec![analysis::Diagnostic {
                code: "test".into(),
                path: "a.py".into(),
                message: "x".repeat(150_000),
            }],
        };
        assert!(agent_input(&evidence).is_err());
    }
}
