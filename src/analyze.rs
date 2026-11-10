//! User-facing composition of the deterministic stages. Drafts never imply approval.
use crate::{analysis, focus, packet, presentation, workflow};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub struct Options {
    pub root: PathBuf,
    pub out: PathBuf,
    pub python: PathBuf,
    pub agent: PathBuf,
    pub model: Option<String>,
    pub timeout_seconds: u64,
    pub retry: bool,
    pub extract_only: bool,
    pub no_render: bool,
    pub focus: Option<focus::FocusOptions>,
    pub mmdc: PathBuf,
    pub expected_snapshot: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    run_directory: PathBuf,
    status: String,
    snapshot_id: String,
    proposal_sha256: Option<String>,
    requires_acceptance: bool,
    complete: bool,
    draft_manifest: Option<PathBuf>,
    draft_mermaid: Option<PathBuf>,
    draft_svg: Option<PathBuf>,
    draft_legend: Option<PathBuf>,
    draft_viewer: Option<PathBuf>,
    presentation_version: Option<String>,
    evidence_packet: Option<PathBuf>,
    coverage_report: Option<PathBuf>,
    coverage: Option<packet::Coverage>,
    packet_error: Option<String>,
    focus: Option<focus::FocusOptions>,
    pub error: Option<String>,
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn json_bytes(value: &impl Serialize) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn program_identity(program: &Path) -> Result<PathBuf, String> {
    if program.is_relative() && program.components().count() > 1 {
        Ok(std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(program))
    } else {
        Ok(program.to_path_buf())
    }
}

// The output container is explicit caller-owned storage, never an app config
// discovered inside the target. Refuse existing symlink components.
pub(crate) fn create_storage(path: &Path) -> Result<PathBuf, String> {
    #[cfg(unix)]
    {
        use rustix::fs::{mkdirat, open, openat, Mode, OFlags};
        use rustix::io::Errno;
        use std::path::Component;

        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
            fs::canonicalize(cwd).map_err(|e| e.to_string())?.join(path)
        };
        // These OS-owned aliases are trusted only at the filesystem root.
        // Never canonicalize an arbitrary output ancestor to bypass NOFOLLOW.
        #[cfg(target_os = "macos")]
        let absolute = if let Ok(rest) = absolute.strip_prefix("/var") {
            Path::new("/private/var").join(rest)
        } else if let Ok(rest) = absolute.strip_prefix("/tmp") {
            Path::new("/private/tmp").join(rest)
        } else {
            absolute
        };
        let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        let mut directory = open("/", flags, Mode::empty()).map_err(|e| e.to_string())?;
        let mut resolved = PathBuf::from("/");
        for component in absolute.components() {
            let name = match component {
                Component::RootDir | Component::CurDir => continue,
                Component::Normal(name) => name,
                Component::ParentDir => std::ffi::OsStr::new(".."),
                Component::Prefix(_) => return Err("Unsupported output storage prefix".into()),
            };
            let opened = match openat(&directory, name, flags, Mode::empty()) {
                Err(Errno::NOENT) if component != Component::ParentDir => {
                    match mkdirat(&directory, name, Mode::from_raw_mode(0o777)) {
                        Ok(()) | Err(Errno::EXIST) => {}
                        Err(error) => return Err(format!("Create output container: {error}")),
                    }
                    // Reopen even after EEXIST: another creator may have installed a symlink.
                    openat(&directory, name, flags, Mode::empty())
                }
                result => result,
            };
            directory = opened.map_err(|e| {
                format!(
                    "Output storage must be a non-symlink directory: {}: {e}",
                    resolved.join(name).display()
                )
            })?;
            if component == Component::ParentDir {
                resolved.pop();
            } else {
                resolved.push(name);
            }
        }
        Ok(resolved)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err("Safe output storage creation requires Unix directory descriptors".into())
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::create_storage;
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::path::Path;

    #[test]
    fn storage_rejects_existing_symlink_ancestor() {
        let temp = tempfile::tempdir().unwrap();
        let redirected = temp.path().join("redirected");
        fs::create_dir_all(redirected.join("existing")).unwrap();
        symlink(&redirected, temp.path().join(".ideomancer")).unwrap();
        for suffix in ["existing", "existing/new/deep", "missing/deep"] {
            assert!(create_storage(&temp.path().join(".ideomancer").join(suffix)).is_err());
        }
        assert!(!redirected.join("existing/new").exists());
        assert!(!redirected.join("missing").exists());
        assert_eq!(fs::read_dir(&redirected).unwrap().count(), 1);
    }

    #[test]
    fn storage_rejects_nested_and_dangling_symlinks_and_files() {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().join(".ideomancer");
        fs::create_dir(&base).unwrap();
        let redirected = temp.path().join("redirected");
        fs::create_dir(&redirected).unwrap();
        symlink(&redirected, base.join("nested")).unwrap();
        symlink(temp.path().join("absent"), base.join("dangling")).unwrap();
        fs::write(base.join("file"), b"unchanged").unwrap();
        for name in ["nested", "dangling", "file"] {
            assert!(create_storage(&base.join(name)).is_err());
            assert!(create_storage(&base.join(name).join("output")).is_err());
        }
        assert_eq!(fs::read_dir(redirected).unwrap().count(), 0);
        assert!(!temp.path().join("absent").exists());
        assert_eq!(fs::read(base.join("file")).unwrap(), b"unchanged");
    }

    #[test]
    fn storage_creates_nested_parents_with_spaces() {
        let temp = tempfile::tempdir().unwrap();
        let output = temp.path().join("new parent/.ideomancer/deep output");
        let expected = fs::canonicalize(temp.path())
            .unwrap()
            .join("new parent/.ideomancer/deep output");
        assert_eq!(create_storage(&output).unwrap(), expected);
        assert_eq!(create_storage(&output).unwrap(), expected);
        assert!(expected.is_dir());
    }

    #[test]
    fn storage_walks_absolute_parent_components_without_skipping_checks() {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        assert_eq!(
            create_storage(&root.join("first/../second/./output")).unwrap(),
            root.join("second/output")
        );
        assert!(root.join("first").is_dir());
        symlink(root.join("first"), root.join("link")).unwrap();
        assert!(create_storage(&root.join("link/../escaped")).is_err());
        assert!(!root.join("escaped").exists());
        assert_eq!(create_storage(Path::new("/../..")).unwrap(), Path::new("/"));
    }

    #[test]
    fn storage_relative_paths_are_rooted_at_canonical_invocation_cwd() {
        // A private child of cwd avoids mutating process-global cwd in parallel tests.
        let cwd = std::env::current_dir().unwrap();
        let temp = tempfile::tempdir_in(&cwd).unwrap();
        let relative =
            Path::new(temp.path().file_name().unwrap()).join("first/../with spaces/output");
        let expected = fs::canonicalize(temp.path())
            .unwrap()
            .join("with spaces/output");
        assert_eq!(create_storage(&relative).unwrap(), expected);
        assert!(expected.is_dir());
    }

    #[test]
    fn storage_concurrent_directory_creators_revalidate_existing_components() {
        let temp = tempfile::tempdir().unwrap();
        let output = temp.path().join("shared/nested/output");
        let barrier = std::sync::Barrier::new(8);
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..8)
                .map(|_| {
                    scope.spawn(|| {
                        barrier.wait();
                        create_storage(&output).unwrap()
                    })
                })
                .collect();
            for handle in handles {
                assert_eq!(handle.join().unwrap(), fs::canonicalize(&output).unwrap());
            }
        });
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn storage_accepts_only_root_level_system_aliases() {
        assert_eq!(
            create_storage(Path::new("/var")).unwrap(),
            Path::new("/private/var")
        );
        assert_eq!(
            create_storage(Path::new("/tmp")).unwrap(),
            Path::new("/private/tmp")
        );
        let temp = tempfile::tempdir().unwrap();
        symlink("/private/tmp", temp.path().join("tmp")).unwrap();
        assert!(create_storage(&temp.path().join("tmp/output")).is_err());
    }
}

fn read(path: &Path) -> Result<Vec<u8>, String> {
    let meta =
        fs::symlink_metadata(path).map_err(|e| format!("Read draft {}: {e}", path.display()))?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > 8 * 1024 * 1024 {
        return Err("Draft artifacts must be bounded regular non-symlink files".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("Draft artifact exceeds limit".into());
    }
    Ok(bytes)
}

fn same_or_absent(path: &Path, expected: &[u8]) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(_) if read(path)? == expected => Ok(true),
        Ok(_) => Err(format!(
            "Conflicting draft artifact {}; not overwritten",
            path.display()
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.to_string()),
    }
}

fn publish(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if same_or_absent(path, bytes)? {
        return Ok(());
    }
    let parent = path.parent().ok_or("Draft has no parent directory")?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    temporary.write_all(bytes).map_err(|e| e.to_string())?;
    temporary.as_file().sync_all().map_err(|e| e.to_string())?;
    match temporary.persist_noclobber(path) {
        Ok(_) => Ok(()),
        Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => {
            same_or_absent(path, bytes).map(|_| ())
        }
        Err(e) => Err(e.to_string()),
    }
}

pub fn run(options: Options) -> Result<Report, String> {
    let source_root = fs::canonicalize(&options.root).map_err(|e| e.to_string())?;
    let base = create_storage(&options.out)?;
    workflow::reject_legacy_storage(&base)?;
    let run = base.join(workflow::app_name(&source_root)?);
    let _lock = workflow::Lock::acquire(&run)?;
    let previous = workflow::tree_snapshot(&run)?;
    let evidence = analysis::extract(&options.root, &options.python)?;
    if options
        .expected_snapshot
        .as_deref()
        .is_some_and(|expected| expected != evidence.snapshot_id)
    {
        return Err("Catalog snapshot is stale for this source/interpreter. Rediscover before starting a workflow diagram; no agent was invoked.".into());
    }
    let mut identity = json!({"version":"analyze.v1", "sourceRoot":source_root,
        "snapshotId":evidence.snapshot_id,"evidence":hash(&json_bytes(&evidence)?),"python":program_identity(&options.python)?,
        "skill":hash(workflow::instructions_for(options.focus.is_some()).as_bytes()),
        "packetPolicy":if options.focus.is_some() { focus::POLICY_VERSION } else { packet::POLICY_VERSION },
        "agent":program_identity(&options.agent)?,"model":options.model});
    if let Some(focus) = &options.focus {
        identity["focus"] = serde_json::to_value(focus).map_err(|e| e.to_string())?;
    }
    let mut replace = false;
    if fs::symlink_metadata(&run).is_ok() {
        let state = workflow::status_locked(&run)?;
        let saved: serde_json::Value = serde_json::from_slice(&read(&run.join("identity.json"))?)
            .map_err(|_| "Invalid analysis identity")?;
        if saved["sourceRoot"] != identity["sourceRoot"] {
            return Err(
                "Application name collides with another source root; choose a different --out"
                    .into(),
            );
        }
        if saved != identity {
            if matches!(
                state.status.as_str(),
                "running" | "failed" | "indeterminate"
            ) {
                return Err("Prior agent work failed or is uncertain. Reconcile and explicitly retry the saved run before changing analysis inputs.".into());
            }
            if matches!(state.status.as_str(), "proposal_ready" | "accepted") {
                workflow::draft_manifest(&run)?;
            }
            replace = true;
        }
    }
    let stage = if replace || !run.exists() {
        Some(
            tempfile::Builder::new()
                .prefix(".analysis-init-")
                .tempdir_in(&base)
                .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };
    // Inspection publishes into a missing child, not the already-created TempDir.
    let initial = stage
        .as_ref()
        .map(|s| s.path().join("run"))
        .unwrap_or_else(|| run.clone());
    let state = workflow::save_inspection_locked(
        &options.root,
        &initial,
        &options.python,
        evidence,
        options.focus.clone(),
    )?;
    if let Some(stage) = stage {
        publish(&initial.join("identity.json"), &json_bytes(&identity)?)?;
        workflow::publish_current(stage, &initial, &run, previous.as_ref())?;
    }
    let mut report = Report {
        run_directory: run.clone(),
        status: state.status.clone(),
        snapshot_id: state.snapshot_id.clone(),
        proposal_sha256: state.proposal_sha256.clone(),
        requires_acceptance: state.status == "proposal_ready" && state.proposal_sha256.is_some(),
        complete: state.complete,
        draft_manifest: None,
        draft_mermaid: None,
        draft_svg: None,
        draft_legend: None,
        draft_viewer: None,
        presentation_version: None,
        evidence_packet: state
            .packet_sha256
            .as_ref()
            .map(|_| run.join("evidence-packet.json")),
        coverage_report: state
            .coverage_sha256
            .as_ref()
            .map(|_| run.join("evidence-coverage.json")),
        coverage: workflow::packet_coverage(&run, &state)?,
        packet_error: state.packet_error.clone(),
        focus: state.focus.clone(),
        error: None,
    };
    if !state.complete {
        report.error = Some(
            "Incomplete extraction; inspect evidence diagnostics. No agent was invoked.".into(),
        );
        return Ok(report);
    }
    if options.extract_only && !(options.focus.is_some() && state.packet_error.is_some()) {
        return Ok(report);
    }
    if let Some(error) = &state.packet_error {
        report.error = Some(format!(
            "Cannot prepare agent evidence: {error}. No agent was invoked."
        ));
        return Ok(report);
    }
    match workflow::interpret_locked(
        &run,
        &options.agent,
        options.model,
        options.timeout_seconds,
        options.retry,
    ) {
        Ok(state) => {
            report.status = state.status.clone();
            report.proposal_sha256 = state.proposal_sha256.clone();
            report.requires_acceptance =
                state.status == "proposal_ready" && state.proposal_sha256.is_some();
        }
        Err(error) => {
            let state = workflow::status_locked(&run)?;
            report.requires_acceptance =
                state.status == "proposal_ready" && state.proposal_sha256.is_some();
            report.status = state.status;
            report.proposal_sha256 = state.proposal_sha256;
            report.error = Some(format!("{error} Run: {}", run.display()));
            return Ok(report);
        }
    }
    if let Err(error) = drafts(&run, &options.mmdc, options.no_render, &mut report) {
        report.error = Some(format!("{error} Run: {}", run.display()));
    }
    Ok(report)
}

pub(crate) fn present(run: &Path, renderer: &Path, no_render: bool) -> Result<Report, String> {
    let _lock = workflow::Lock::acquire(run)?;
    let state = workflow::status_locked(run)?;
    let run = fs::canonicalize(run).map_err(|e| e.to_string())?;
    let mut report = Report {
        run_directory: run.clone(),
        status: state.status.clone(),
        snapshot_id: state.snapshot_id.clone(),
        proposal_sha256: state.proposal_sha256.clone(),
        requires_acceptance: state.status == "proposal_ready" && state.proposal_sha256.is_some(),
        complete: state.complete,
        draft_manifest: None,
        draft_mermaid: None,
        draft_svg: None,
        draft_legend: None,
        draft_viewer: None,
        presentation_version: None,
        evidence_packet: state
            .packet_sha256
            .as_ref()
            .map(|_| run.join("evidence-packet.json")),
        coverage_report: state
            .coverage_sha256
            .as_ref()
            .map(|_| run.join("evidence-coverage.json")),
        coverage: workflow::packet_coverage(&run, &state)?,
        packet_error: state.packet_error.clone(),
        focus: state.focus.clone(),
        error: None,
    };
    if let Err(error) = drafts(&run, renderer, no_render, &mut report) {
        report.error = Some(format!("{error} Run: {}", run.display()));
    }
    Ok(report)
}

fn drafts(run: &Path, renderer: &Path, no_render: bool, report: &mut Report) -> Result<(), String> {
    let (_, graph) = workflow::draft_manifest(run)?;
    let input = workflow::presentation_input(run)?;
    let view = presentation::save(run, &input, &json_bytes(&graph)?, renderer, no_render)?;
    let manifest_path = run.join("presentation/draft.architecture.json");
    report.draft_manifest = Some(manifest_path);
    report.draft_mermaid = Some(view.mermaid);
    report.draft_svg = view.svg;
    report.draft_legend = Some(view.legend);
    report.draft_viewer = Some(view.viewer);
    report.presentation_version = Some(crate::render::presentation::VERSION.into());
    if let Some(error) = view.error {
        return Err(error);
    }
    Ok(())
}
