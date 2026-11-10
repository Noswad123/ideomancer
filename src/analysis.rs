//! Deterministic static Python evidence. Target code is never imported or run.
//! Dynamic discovery is unresolved evidence, not an incomplete traversal. Parse
//! and relevant filesystem failures, in contrast, set `complete` to false.
//! Filesystem extraction supports Unix only (Linux/macOS CI). Other platforms
//! fail closed rather than using pathname opens without equivalent guarantees.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use rustix::fs::{openat, statat, AtFlags, Dir, FileType, Mode, OFlags};

pub const EXTRACTOR_VERSION: &str = "python-static-8-generic-facts";
const POLICY_VERSION: &str = "python-scope-2-nofollow";
const MAX_FILES: usize = 5000;
const MAX_FILE: usize = 1024 * 1024;
const MAX_TOTAL: usize = 16 * 1024 * 1024;
const MAX_INPUT: usize = 40 * 1024 * 1024;
const MAX_OUTPUT: usize = 32 * 1024 * 1024;
const DEADLINE: Duration = Duration::from_secs(60);

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Evidence {
    pub schema_version: String,
    pub snapshot_id: String,
    pub extractor_version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub python_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<LanguageMetadata>,
    pub files: Vec<SourceFile>,
    pub facts: Vec<Fact>,
    pub diagnostics: Vec<Diagnostic>,
    pub complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Language {
    Python,
    #[value(name = "javascript-typescript", aliases = ["js", "javascript", "ts", "typescript"])]
    JavaScriptTypeScript,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LanguageMetadata {
    pub name: String,
    pub parser: String,
    pub parser_version: String,
    pub parser_sha256: String,
    pub runtime: String,
    pub runtime_version: String,
    pub scan_policy: String,
}

impl Evidence {
    pub fn validate_language(&self) -> Result<Language, String> {
        match (self.schema_version.as_str(), &self.language) {
            ("ideomancer.evidence.v1", None)
                if !self.python_version.is_empty()
                    && !self.extractor_version.starts_with("javascript-typescript-") =>
            {
                Ok(Language::Python)
            }
            ("ideomancer.evidence.v2", Some(metadata))
                if self.python_version.is_empty()
                    && crate::javascript::supported_version(&self.extractor_version)
                    && metadata.name == "javascript-typescript"
                    && metadata.parser == "typescript"
                    && metadata.runtime == "node"
                    && metadata.scan_policy == crate::javascript::SCAN_POLICY
                    && !metadata.parser_version.is_empty()
                    && !metadata.runtime_version.is_empty()
                    && metadata.parser_sha256.len() == 64
                    && metadata
                        .parser_sha256
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) =>
            {
                Ok(Language::JavaScriptTypeScript)
            }
            _ => Err("Unsupported or inconsistent evidence schema/language metadata".into()),
        }
    }

    pub fn is_javascript(&self) -> bool {
        self.validate_language() == Ok(Language::JavaScriptTypeScript)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceFile {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Fact {
    pub id: String,
    pub kind: String,
    pub path: String,
    pub line: usize,
    pub end_line: usize,
    pub symbol: String,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Diagnostic {
    pub code: String,
    pub path: String,
    pub message: String,
}

#[derive(Serialize)]
pub(crate) struct Source {
    path: String,
    module: String,
    hex: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Response {
    python_version: String,
    facts: Vec<Fact>,
    diagnostics: Vec<Diagnostic>,
    complete: bool,
}

pub(crate) fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn excluded(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | ".venv"
            | "venv"
            | "__pycache__"
            | "node_modules"
            | ".ideomancer"
            | "caches"
            | ".cache"
            | ".pytest_cache"
            | ".mypy_cache"
            | ".ruff_cache"
            | "build"
            | "dist"
            | "scripts"
            | "migrations"
            | "shadow"
            | "tests"
            | "alembic"
            | "env"
            | "data"
            | "db"
    )
}

impl Language {
    fn excluded(self, name: &str) -> bool {
        excluded(name)
            || (self == Self::JavaScriptTypeScript
                && (name.starts_with('.')
                    || matches!(
                        name,
                        "bower_components"
                            | "assets"
                            | "vendor"
                            | "generated"
                            | "coverage"
                            | "init"
                            | "config"
                            | "configs"
                            | "environments"
                    )))
    }

    fn source(self, path: &Path) -> bool {
        if self == Self::Python {
            return path.extension().is_some_and(|ext| ext == "py");
        }
        path.extension()
            .is_some_and(|ext| matches!(ext.to_str(), Some("js" | "jsx" | "ts" | "tsx")))
            && !self.skipped_file(path)
    }

    fn skipped_file(self, path: &Path) -> bool {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        self == Self::JavaScriptTypeScript
            && (name.contains(".spec.")
                || name.contains(".test.")
                || name.ends_with(".min.js")
                || name.ends_with(".bundle.js")
                || name.ends_with(".d.ts"))
    }
}

pub(crate) struct Scan {
    pub root: PathBuf,
    pub files: Vec<SourceFile>,
    pub sources: Vec<Source>,
    pub diagnostics: Vec<Diagnostic>,
    pub complete: bool,
    root_package: String,
}

#[cfg(not(unix))]
pub(crate) fn scan_javascript(_root: &Path) -> Result<Scan, String> {
    Err(
        "JavaScript extraction requires Unix descriptor-relative no-follow filesystem support"
            .into(),
    )
}

#[cfg(unix)]
pub(crate) fn scan_javascript(root: &Path) -> Result<Scan, String> {
    extract_with_policy(root, Language::JavaScriptTypeScript)
}

pub fn extract_javascript(root: &Path, node: &Path, typescript: &Path) -> Result<Evidence, String> {
    crate::javascript::extract(root, node, typescript)
}

fn diagnostic(diagnostics: &mut Vec<Diagnostic>, code: &str, path: String, message: String) {
    diagnostics.push(Diagnostic {
        code: code.into(),
        path,
        message,
    });
}

fn relative(root: &Path, path: &Path) -> Result<String, String> {
    path.strip_prefix(root)
        .map_err(|e| e.to_string())?
        .to_str()
        .map(|p| p.replace(std::path::MAIN_SEPARATOR, "/"))
        .ok_or_else(|| {
            "Python scan encountered a non-UTF-8 path; rename it before extraction".into()
        })
}

/// Reads each eligible file once, hashes those exact bytes, and sends only those
/// bytes to an isolated, embedded stdlib helper. Hard limits fail the operation,
/// rather than returning a deceptively successful partial snapshot.
#[cfg(not(unix))]
pub fn extract(_root: &Path, _python: &Path) -> Result<Evidence, String> {
    Err("Python evidence extraction requires Unix descriptor-relative no-follow filesystem support (Linux/macOS); no insecure fallback is available".into())
}

#[cfg(unix)]
fn directory_flags() -> OFlags {
    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC
}

/// Every untrusted component is a single openat operation. In particular,
/// NOFOLLOW on a slash-containing pathname would protect only its final name.
#[cfg(unix)]
pub(crate) fn open_anchored(
    root: &fs::File,
    path: &Path,
    directory: bool,
) -> std::io::Result<fs::File> {
    use std::path::Component;
    let components: Vec<_> = path.components().collect();
    if components
        .iter()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(std::io::Error::other(
            "Expected a root-relative path with only normal components",
        ));
    }
    let mut current = fs::File::from(openat(root, ".", directory_flags(), Mode::empty())?);
    for (index, component) in components.iter().enumerate() {
        let last = index + 1 == components.len();
        let flags = if last && !directory {
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC
        } else {
            directory_flags()
        };
        current = fs::File::from(openat(
            &current,
            component.as_os_str(),
            flags,
            Mode::empty(),
        )?);
    }
    let metadata = current.metadata()?;
    if (directory && !metadata.is_dir()) || (!directory && !metadata.is_file()) {
        return Err(std::io::Error::other(
            "Opened descriptor is not the required regular file/directory",
        ));
    }
    Ok(current)
}

#[cfg(unix)]
fn open_root(root: &Path) -> Result<(fs::File, PathBuf), String> {
    // Resolve operator-supplied ancestors first (e.g. macOS /var -> /private/var),
    // but never canonicalize the final root name. Walk the resolved pathname
    // from / with NOFOLLOW at every component, so subsequent swaps fail closed.
    let absolute = std::env::current_dir()
        .map_err(|e| e.to_string())?
        .join(root);
    let absolute: PathBuf = absolute.components().collect();
    let resolved = if absolute == Path::new("/") {
        absolute
    } else {
        let name = absolute
            .file_name()
            .ok_or("Python source root must name a non-symlink directory")?;
        fs::canonicalize(
            absolute
                .parent()
                .ok_or("Python source root has no parent")?,
        )
        .map_err(|e| format!("Resolve source root parent: {e}"))?
        .join(name)
    };
    let anchor = fs::File::from(
        rustix::fs::open("/", directory_flags(), Mode::empty())
            .map_err(|e| format!("Open filesystem anchor: {e}"))?,
    );
    let descriptor = open_anchored(
        &anchor,
        resolved.strip_prefix("/").map_err(|e| e.to_string())?,
        true,
    )
    .map_err(|e| format!("Python source root must be an existing non-symlink directory: {e}"))?;
    Ok((descriptor, resolved))
}

#[cfg(unix)]
/// Extract on Unix using descriptor-relative no-follow traversal and reads.
pub fn extract(root: &Path, python: &Path) -> Result<Evidence, String> {
    let Scan {
        files,
        sources,
        mut diagnostics,
        mut complete,
        root_package,
        ..
    } = extract_with_policy(root, Language::Python)?;
    let input = serde_json::to_vec(&serde_json::json!({ "sources": sources }))
        .map_err(|e| e.to_string())?;
    if input.len() > MAX_INPUT {
        return Err("Python extractor input exceeds the 40 MiB protocol limit".into());
    }
    let response = run_helper(python, input)?;
    complete &= response.complete;
    diagnostics.extend(response.diagnostics);
    diagnostics.sort_by(|a, b| (&a.path, &a.code, &a.message).cmp(&(&b.path, &b.code, &b.message)));
    diagnostics.dedup_by(|a, b| a.path == b.path && a.code == b.code && a.message == b.message);
    let snapshot = serde_json::to_vec(&(
        POLICY_VERSION,
        EXTRACTOR_VERSION,
        &response.python_version,
        root_package,
        &files,
    ))
    .map_err(|e| e.to_string())?;
    Ok(Evidence {
        schema_version: "ideomancer.evidence.v1".into(),
        snapshot_id: hash(&snapshot),
        extractor_version: EXTRACTOR_VERSION.into(),
        python_version: response.python_version,
        language: None,
        files,
        facts: response.facts,
        diagnostics,
        complete,
    })
}

#[cfg(unix)]
fn extract_with_policy(root: &Path, policy: Language) -> Result<Scan, String> {
    use std::os::unix::ffi::OsStrExt;
    let language = if policy == Language::Python {
        "Python"
    } else {
        "JavaScript/TypeScript"
    };
    let (root_fd, root) = open_root(root)?;
    let mut complete = true;
    let mut diagnostics = Vec::new();
    let scan = match statat(&root_fd, "src", AtFlags::SYMLINK_NOFOLLOW) {
        Ok(meta) if FileType::from_raw_mode(meta.st_mode) == FileType::Symlink => {
            complete = false;
            diagnostic(
                &mut diagnostics,
                "symlink_skipped",
                "src".into(),
                "Source directory symlink not followed".into(),
            );
            PathBuf::from("src")
        }
        Ok(meta) if FileType::from_raw_mode(meta.st_mode) == FileType::Directory => {
            PathBuf::from("src")
        }
        Ok(_) => PathBuf::new(),
        Err(rustix::io::Errno::NOENT) => PathBuf::new(),
        Err(e) => return Err(format!("Inspect source directory: {e}")),
    };
    let mut pending = if complete {
        vec![scan.clone()]
    } else {
        Vec::new()
    };
    let mut paths = Vec::new();
    while let Some(path) = pending.pop() {
        let rel = relative(Path::new(""), &path)?;
        let directory = match open_anchored(&root_fd, &path, true) {
            Ok(directory) => directory,
            Err(e) => {
                complete = false;
                diagnostic(&mut diagnostics, "traversal_io", rel, e.to_string());
                continue;
            }
        };
        let entries = match Dir::read_from(&directory) {
            Ok(entries) => entries,
            Err(e) => {
                complete = false;
                diagnostic(&mut diagnostics, "traversal_io", rel, e.to_string());
                continue;
            }
        };
        for entry in entries {
            match entry {
                Ok(entry) => {
                    let name = std::ffi::OsStr::from_bytes(entry.file_name().to_bytes());
                    if name == "." || name == ".." {
                        continue;
                    }
                    let child = path.join(name);
                    let child_rel = relative(Path::new(""), &child)?;
                    let meta =
                        match statat(&directory, entry.file_name(), AtFlags::SYMLINK_NOFOLLOW) {
                            Ok(meta) => meta,
                            Err(e) => {
                                if !policy.excluded(name.to_str().unwrap_or("")) {
                                    complete = false;
                                    diagnostic(
                                        &mut diagnostics,
                                        "traversal_io",
                                        child_rel,
                                        e.to_string(),
                                    );
                                }
                                continue;
                            }
                        };
                    let kind = FileType::from_raw_mode(meta.st_mode);
                    let skip = policy.excluded(name.to_str().unwrap_or(""))
                        || (kind != FileType::Directory && policy.skipped_file(&child));
                    if kind == FileType::Symlink {
                        complete &= skip;
                        diagnostic(
                            &mut diagnostics,
                            "symlink_skipped",
                            child_rel,
                            if skip {
                                "Excluded symlink not followed (outside configured scope)"
                            } else {
                                "Symlink not followed; relevant source may be unavailable"
                            }
                            .into(),
                        );
                    } else if skip {
                        continue;
                    } else if kind == FileType::Directory {
                        pending.push(child);
                    } else if policy.source(&child) {
                        if kind != FileType::RegularFile {
                            complete = false;
                            diagnostic(
                                &mut diagnostics,
                                "special_file_skipped",
                                child_rel,
                                format!("Non-regular {language} source not opened"),
                            );
                        } else {
                            if meta.st_size > MAX_FILE as i64 {
                                return Err(format!(
                                    "{language} source {child_rel} exceeds the 1 MiB file limit"
                                ));
                            }
                            paths.push(child);
                            if paths.len() > MAX_FILES {
                                return Err(format!(
                                    "{language} source exceeds the 5000 file limit"
                                ));
                            }
                        }
                    }
                }
                Err(e) => {
                    complete = false;
                    diagnostic(&mut diagnostics, "traversal_io", rel.clone(), e.to_string());
                }
            }
        }
    }
    paths.sort();
    let root_package = if policy == Language::Python && paths.contains(&scan.join("__init__.py")) {
        if scan.as_os_str().is_empty() {
            root.file_name()
        } else {
            scan.file_name()
        }
        .and_then(|n| n.to_str())
        .unwrap_or("")
    } else {
        ""
    };
    let mut files = Vec::new();
    let mut sources = Vec::new();
    let mut total = 0;
    for path in paths {
        let rel = relative(Path::new(""), &path)?;
        let bytes = match open_anchored(&root_fd, &path, false).and_then(|file| {
            let mut bytes = Vec::new();
            file.take((MAX_FILE + 1) as u64).read_to_end(&mut bytes)?;
            Ok(bytes)
        }) {
            Ok(bytes) => bytes,
            Err(e) => {
                complete = false;
                diagnostic(&mut diagnostics, "source_io", rel, e.to_string());
                continue;
            }
        };
        if bytes.len() > MAX_FILE {
            return Err(format!(
                "{language} source {rel} exceeds the 1 MiB file limit"
            ));
        }
        total += bytes.len();
        if total > MAX_TOTAL {
            return Err(format!("{language} source exceeds the 16 MiB total limit"));
        }
        let local = relative(&scan, &path)?;
        let mut parts: Vec<_> = local
            .strip_suffix(".py")
            .unwrap_or(&local)
            .split('/')
            .collect();
        if parts.last() == Some(&"__init__") {
            parts.pop();
        }
        if !root_package.is_empty() {
            parts.insert(0, root_package);
        }
        let module = parts.join(".");
        let module = if policy == Language::JavaScriptTypeScript {
            rel.replace('/', ".")
        } else if module.is_empty() {
            "__root__".into()
        } else {
            module
        };
        files.push(SourceFile {
            path: rel.clone(),
            sha256: hash(&bytes),
        });
        let mut hex = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            use std::fmt::Write;
            write!(hex, "{byte:02x}").expect("write to String");
        }
        sources.push(Source {
            path: rel,
            module,
            hex,
        });
    }
    let root_package = root_package.into();
    Ok(Scan {
        root,
        files,
        sources,
        diagnostics,
        complete,
        root_package,
    })
}

#[cfg(unix)]
fn read_available(
    pipe: &mut Option<impl Read>,
    bytes: &mut Vec<u8>,
    limit: usize,
) -> std::io::Result<bool> {
    let Some(handle) = pipe.as_mut() else {
        return Ok(false);
    };
    let mut buffer = [0; 16 * 1024];
    let available = buffer.len().min(limit + 1 - bytes.len());
    match handle.read(&mut buffer[..available]) {
        Ok(0) => {
            *pipe = None;
            Ok(true)
        }
        Ok(count) => {
            bytes.extend_from_slice(&buffer[..count]);
            if bytes.len() > limit {
                return Err(std::io::Error::other("bounded output limit exceeded"));
            }
            Ok(true)
        }
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
            ) =>
        {
            Ok(false)
        }
        Err(error) => Err(error),
    }
}

#[cfg(unix)]
fn run_helper(python: &Path, input: Vec<u8>) -> Result<Response, String> {
    // Resolve relative executable paths before changing the child's cwd. Bare
    // executable names still use the operator's PATH, not a target virtualenv.
    let executable: PathBuf = if python.components().count() > 1 && python.is_relative() {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(python)
    } else {
        python.into()
    };
    let neutral = tempfile::tempdir().map_err(|e| format!("Create isolated Python cwd: {e}"))?;
    let mut command = Command::new(&executable);
    command
        .args(["-I", "-S", "-c", include_str!("python_extract.py")])
        .current_dir(neutral.path())
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for key in ["PATH", "SYSTEMROOT", "WINDIR"] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    let output = run_bounded(command, input, "Python")?;
    serde_json::from_slice(&output).map_err(|e| format!("Invalid Python extractor response: {e}"))
}

#[cfg(unix)]
pub(crate) fn run_bounded(
    command: Command,
    input: Vec<u8>,
    language: &'static str,
) -> Result<Vec<u8>, String> {
    run_bounded_with_timeout(command, input, language, DEADLINE)
}

#[cfg(unix)]
fn run_bounded_with_timeout(
    mut command: Command,
    input: Vec<u8>,
    language: &'static str,
    timeout: Duration,
) -> Result<Vec<u8>, String> {
    use std::os::fd::AsFd;
    use std::os::unix::process::CommandExt;
    if input.len() > MAX_INPUT {
        return Err(format!(
            "{language} extractor input exceeds the 40 MiB protocol limit"
        ));
    }
    command.process_group(0);
    let start = Instant::now();
    let mut child = command.spawn().map_err(|e| {
        if language == "Python" {
            format!("Cannot start Python interpreter {}: {e}. Select an installed Python >=3.12 executable; no environment is activated or installed.", Path::new(command.get_program()).display())
        } else {
            format!("Cannot start {language} interpreter: {e}. Select an installed executable; no environment is activated or installed.")
        }
    })?;
    let mut stdin = Some(child.stdin.take().expect("piped stdin"));
    let mut stdout = Some(child.stdout.take().expect("piped stdout"));
    let mut stderr = Some(child.stderr.take().expect("piped stderr"));
    let pid = rustix::process::Pid::from_raw(child.id() as i32).expect("positive child PID");
    let kill_group = || {
        let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
    };
    let mut status = None;
    let result = (|| {
        for fd in [
            stdin.as_ref().unwrap().as_fd(),
            stdout.as_ref().unwrap().as_fd(),
            stderr.as_ref().unwrap().as_fd(),
        ] {
            let flags = rustix::fs::fcntl_getfl(fd)
                .map_err(|e| format!("Read extractor pipe flags: {e}"))?;
            rustix::fs::fcntl_setfl(fd, flags | OFlags::NONBLOCK)
                .map_err(|e| format!("Set nonblocking extractor pipe: {e}"))?;
        }
        let mut output = Vec::new();
        let mut errors = Vec::new();
        let mut written = 0;
        let mut write_error = None;
        loop {
            if start.elapsed() >= timeout {
                return Err(format!("{language} extractor exceeded its {timeout:?} deadline (including protocol I/O)"));
            }
            if status.is_none() {
                status = child
                    .try_wait()
                    .map_err(|e| format!("Wait for {language} extractor: {e}"))?;
                if status.is_some() {
                    kill_group();
                }
            }
            let mut progress = false;
            if written == input.len() {
                stdin = None;
            }
            if let Some(pipe) = stdin.as_mut() {
                let end = input.len().min(written + 64 * 1024);
                match pipe.write(&input[written..end]) {
                    Ok(0) => {
                        write_error = Some(std::io::Error::new(
                            std::io::ErrorKind::WriteZero,
                            "extractor stdin closed",
                        ));
                        stdin = None;
                    }
                    Ok(count) => {
                        written += count;
                        progress = true;
                    }
                    Err(error)
                        if matches!(
                            error.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                        ) => {}
                    Err(error) => {
                        write_error = Some(error);
                        stdin = None;
                    }
                }
            }
            progress |= read_available(&mut stdout, &mut output, MAX_OUTPUT)
                .map_err(|e| format!("Read {language} extractor stdout: {e}"))?;
            progress |= read_available(&mut stderr, &mut errors, 64 * 1024)
                .map_err(|e| format!("Read {language} extractor stderr: {e}"))?;
            if status.is_some() && stdin.is_none() && stdout.is_none() && stderr.is_none() {
                break;
            }
            if !progress {
                thread::sleep(Duration::from_millis(1));
            }
        }
        let status = status.expect("child exited and all pipes closed");
        if !status.success() {
            return Err(if language == "Python" {
                format!(
                    "Python extractor failed ({status}): {}",
                    String::from_utf8_lossy(&errors)
                )
            } else {
                format!("{language} extractor failed ({status}); helper stderr withheld to avoid exposing source or configuration data")
            });
        }
        if let Some(error) = write_error {
            return Err(format!("Send sources to {language} extractor: {error}"));
        }
        Ok(output)
    })();
    // Nonblocking pipes are dropped on every exit, including detached-session
    // pipe holders. No reader/writer threads can outlive the deadline or leak.
    kill_group();
    if status.is_none() {
        let _ = child.kill();
        let _ = child.wait();
    }
    result
}

#[derive(Deserialize)]
struct Import {
    module: String,
    level: usize,
    names: Vec<ImportName>,
}

#[derive(Deserialize)]
struct ImportName {
    name: String,
}

/// One node per module, never per call or runtime object. Imports refer only to
/// exact, unique local module names; unresolved/external imports create no edge.
pub fn module_manifest(evidence: &Evidence, name: &str) -> crate::manifest::Manifest {
    use crate::manifest::{Edge, Manifest, Node};
    let modules: Vec<_> = evidence
        .facts
        .iter()
        .filter(|f| f.kind == "module")
        .collect();
    let by_path: BTreeMap<_, _> = modules.iter().map(|f| (f.path.as_str(), *f)).collect();
    let mut names: BTreeMap<String, Vec<&Fact>> = BTreeMap::new();
    for module in &modules {
        names.entry(module.value.clone()).or_default().push(module);
    }
    let mut nodes: Vec<_> = modules
        .iter()
        .map(|module| Node {
            id: module.id.clone(),
            label: Some(module.path.clone()),
            kind: Some("module".into()),
        })
        .collect();
    nodes.sort_by(|a, b| a.id.cmp(&b.id));
    let mut edges = BTreeSet::new();
    for fact in &evidence.facts {
        if fact.kind != "import" && fact.kind != "from_import" {
            continue;
        }
        let Some(source) = by_path.get(fact.path.as_str()) else {
            continue;
        };
        let Ok(import) = serde_json::from_str::<Import>(&fact.value) else {
            continue;
        };
        let mut base = import.module;
        if import.level > 0 {
            let mut package: Vec<_> = source.value.split('.').collect();
            if !source.path.ends_with("/__init__.py") && source.path != "__init__.py" {
                package.pop();
            }
            if import.level > package.len() {
                continue;
            }
            package.truncate(package.len() - (import.level - 1));
            if !base.is_empty() {
                package.push(&base);
            }
            base = package.join(".");
        }
        for item in import.names {
            let candidate = if fact.kind == "import" {
                item.name.clone()
            } else if item.name == "*" {
                base.clone()
            } else if base.is_empty() {
                item.name.clone()
            } else {
                format!("{base}.{}", item.name)
            };
            let resolved = match names.get(&candidate) {
                Some(matches) if matches.len() == 1 => Some(matches[0]),
                Some(_) => None,
                None if fact.kind == "from_import" => names
                    .get(&base)
                    .filter(|matches| matches.len() == 1)
                    .map(|matches| matches[0]),
                None => None,
            };
            if let Some(target) = resolved {
                if target.id != source.id {
                    edges.insert((source.id.clone(), target.id.clone()));
                }
            }
        }
    }
    Manifest {
        schema_version: crate::manifest::SCHEMA_VERSION.into(),
        id: format!("modules_{}", hash(name.as_bytes())),
        name: name.into(),
        nodes,
        edges: edges
            .into_iter()
            .map(|(from, to)| Edge {
                from,
                to,
                label: Some("imports".into()),
            })
            .collect(),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn legacy_language_serialization_is_unchanged_and_validation_fails_closed() {
        let legacy = r#"{"schemaVersion":"ideomancer.evidence.v1","snapshotId":"s","extractorVersion":"python-static-7-call-targets","pythonVersion":"3.12","files":[],"facts":[],"diagnostics":[],"complete":true}"#;
        let mut evidence: Evidence = serde_json::from_str(legacy).unwrap();
        assert_eq!(serde_json::to_string(&evidence).unwrap(), legacy);
        assert_eq!(evidence.validate_language(), Ok(Language::Python));
        assert!(!evidence.is_javascript());
        evidence.schema_version = "ideomancer.evidence.v99".into();
        assert!(evidence.validate_language().is_err());
        evidence.schema_version = "ideomancer.evidence.v2".into();
        assert!(evidence.validate_language().is_err());
    }

    #[test]
    fn javascript_scan_uses_fixed_scope_and_root_relative_modules() {
        let root = tempfile::tempdir().unwrap();
        for path in ["src/a.ts", "src/a.js", "src/ui/view.tsx", "src/ui/view.jsx"] {
            source(root.path(), path, "export class Example {}");
        }
        for path in [
            "outside.ts",
            "src/a.py",
            "src/a.spec.ts",
            "src/a.test.jsx",
            "src/a.min.js",
            "src/a.bundle.js",
            "src/a.d.ts",
            "src/config/plugin.ts",
            "src/node_modules/p/index.js",
            "src/.hidden/a.ts",
            "src/vendor/a.ts",
            "src/generated/a.ts",
            "src/init/a.ts",
            "src/configs/a.ts",
            "src/environments/a.ts",
            "src/assets/a.ts",
            "src/bower_components/a.js",
            "src/coverage/a.ts",
        ] {
            source(root.path(), path, "throw new Error('must not load')");
        }
        let scan = scan_javascript(root.path()).unwrap();
        assert!(scan.complete);
        assert_eq!(
            scan.files
                .iter()
                .map(|f| f.path.as_str())
                .collect::<Vec<_>>(),
            ["src/a.js", "src/a.ts", "src/ui/view.jsx", "src/ui/view.tsx"]
        );
        assert_eq!(scan.sources[0].module, "src.a.js");
        assert_eq!(scan.files[0].sha256, hash(b"export class Example {}"));
    }

    #[test]
    fn javascript_scan_reuses_nofollow_and_special_file_checks() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        source(external.path(), "secret.ts", "secret");
        symlink(
            external.path().join("secret.ts"),
            root.path().join("linked.ts"),
        )
        .unwrap();
        let scan = scan_javascript(root.path()).unwrap();
        assert!(!scan.complete);
        assert!(scan.files.is_empty());
        assert!(scan.diagnostics.iter().any(|d| d.code == "symlink_skipped"));
        assert!(Command::new("mkfifo")
            .arg(root.path().join("special.ts"))
            .status()
            .unwrap()
            .success());
        let scan = scan_javascript(root.path()).unwrap();
        assert!(!scan.complete);
        assert!(scan.files.is_empty());
        assert!(scan
            .diagnostics
            .iter()
            .any(|d| d.code == "special_file_skipped"));
        let link = root.path().join("root-link");
        symlink(external.path(), &link).unwrap();
        assert!(scan_javascript(&link).is_err());
    }

    #[test]
    fn javascript_scan_enforces_source_size_limit() {
        let root = tempfile::tempdir().unwrap();
        source(root.path(), "large.ts", vec![b' '; MAX_FILE + 1]);
        assert!(scan_javascript(root.path())
            .err()
            .unwrap()
            .contains("1 MiB"));
    }

    fn python() -> Option<PathBuf> {
        for candidate in ["python3.12", "python3.13", "python3"] {
            let Ok(output) = Command::new(candidate)
                .args([
                    "-I",
                    "-S",
                    "-c",
                    "import sys; sys.exit(sys.version_info < (3, 12))",
                ])
                .output()
            else {
                continue;
            };
            if output.status.success() {
                return Some(candidate.into());
            }
        }
        eprintln!("SKIP: static extraction integration test requires an installed Python >=3.12");
        None
    }

    fn source(root: &Path, path: &str, content: impl AsRef<[u8]>) {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn graph_edges(evidence: &Evidence) -> BTreeSet<(String, String)> {
        let graph = module_manifest(evidence, "Synthetic");
        let labels: BTreeMap<_, _> = graph
            .nodes
            .iter()
            .map(|n| (n.id.as_str(), n.label.as_deref().unwrap()))
            .collect();
        assert_eq!(graph.nodes.len(), evidence.files.len());
        for node in &graph.nodes {
            assert!(node.id.starts_with("module_"));
            assert!(node
                .id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_'));
            assert!(evidence
                .facts
                .iter()
                .any(|f| f.kind == "module" && f.id == node.id));
        }
        graph
            .edges
            .iter()
            .map(|edge| {
                (
                    labels[edge.from.as_str()].into(),
                    labels[edge.to.as_str()].into(),
                )
            })
            .collect()
    }

    #[test]
    fn deterministic_rich_evidence_without_execution_or_literals() {
        let Some(python) = python() else {
            return;
        };
        let root = tempfile::tempdir().unwrap();
        let marker = root.path().join("executed");
        let content = format!(
            r#"
from pathlib import Path
from injection import Inject
Path({:?}).write_text("TARGET_EXECUTED")
SECRET = "DO_NOT_CAPTURE_LITERAL_7f3a"
@decorate(option="DO_NOT_CAPTURE_LITERAL_7f3a")
class Service(Base):
    def __init__(self, dep: Dependency = Inject(Dependency)):
        self.dep = dep
    async def run(self) -> Result:
        def nested():
            return self.dep.work()
        return nested()
type Pair[T] = tuple[T, T]
"#,
            marker.to_str().unwrap()
        );
        source(root.path(), "src/app/service.py", &content);
        let first = extract(root.path(), &python).unwrap();
        let second = extract(root.path(), &python).unwrap();
        let serialized = serde_json::to_string(&first).unwrap();
        assert_eq!(first.schema_version, "ideomancer.evidence.v1");
        assert_eq!(serialized, serde_json::to_string(&second).unwrap());
        assert!(first.complete);
        assert!(!marker.exists());
        assert!(!serialized.contains("DO_NOT_CAPTURE_LITERAL_7f3a"));
        assert!(!serialized.contains("TARGET_EXECUTED"));
        assert_eq!(first.files[0].sha256, hash(content.as_bytes()));
        for kind in [
            "module",
            "class",
            "function",
            "async_function",
            "base",
            "decorator",
            "parameter_annotation",
            "parameter_default",
            "return_annotation",
            "call",
            "from_import",
        ] {
            assert!(
                first.facts.iter().any(|fact| fact.kind == kind),
                "missing {kind}"
            );
        }
        assert!(first
            .facts
            .iter()
            .any(|f| f.symbol == "app.service.Service.run.nested"));
        assert!(first
            .facts
            .iter()
            .any(|f| f.kind == "parameter_default" && f.value == "Inject(Dependency)"));
        source(
            root.path(),
            "src/app/service.py",
            format!("{content}\n# changed\n"),
        );
        let changed = extract(root.path(), &python).unwrap();
        assert_ne!(first.snapshot_id, changed.snapshot_id);
        assert_eq!(
            first.facts.iter().find(|f| f.kind == "module").unwrap().id,
            changed
                .facts
                .iter()
                .find(|f| f.kind == "module")
                .unwrap()
                .id
        );
    }

    #[test]
    fn class_attribute_annotations_are_scoped_redacted_source_evidence() {
        let Some(python) = python() else {
            return;
        };
        let root = tempfile::tempdir().unwrap();
        source(
            root.path(),
            "context.py",
            r#"from benchling_sdk.benchling import Benchling
raise RuntimeError("Target must not execute")
module_local: Benchling
class Context:
    benchling: Benchling
    initialized: Benchling = make_client("PRIVATE_CREDENTIAL")
    quoted: "PRIVATE_TYPE"
    multi: (
        Benchling
    ) = None
    def __init__(self):
        local: Benchling
        self.instance: Benchling
        def nested():
            benchling: Benchling
    class Inner:
        benchling: Benchling
def factory():
    benchling: Benchling
    class Local:
        benchling: Benchling
    return Local
"#,
        );
        let e = extract(root.path(), &python).unwrap();
        assert!(e.complete);
        let fields: Vec<_> = e
            .facts
            .iter()
            .filter(|f| f.kind == "attribute_annotation")
            .map(|f| (f.symbol.as_str(), f.value.as_str(), f.line, f.end_line))
            .collect();
        assert_eq!(
            fields,
            vec![
                ("context.Context.benchling", "Benchling", 5, 5),
                ("context.Context.initialized", "Benchling", 6, 6),
                ("context.Context.quoted", "<literal>", 7, 7),
                ("context.Context.multi", "Benchling", 8, 10),
                ("context.Context.Inner.benchling", "Benchling", 17, 17),
                ("context.factory.Local.benchling", "Benchling", 21, 21),
            ]
        );
        let serialized = serde_json::to_string(&e).unwrap();
        assert!(!serialized.contains("PRIVATE_CREDENTIAL"));
        assert!(!serialized.contains("PRIVATE_TYPE"));
        assert_eq!(
            serialized,
            serde_json::to_string(&extract(root.path(), &python).unwrap()).unwrap()
        );
    }

    #[test]
    fn signatures_preserve_syntax_order_and_kinds_without_defaults_or_execution() {
        let Some(python) = python() else {
            return;
        };
        let root = tempfile::tempdir().unwrap();
        let parameters = (0..129)
            .map(|i| format!("p{i}"))
            .collect::<Vec<_>>()
            .join(", ");
        source(
            root.path(),
            "signatures.py",
            format!(
                r#"
raise RuntimeError("Target must not execute")
def ordered(z, a, /, b="PRIVATE_DEFAULT", *rest, k="PRIVATE_KEYWORD", **options):
    async def nested(x, *, y):
        pass
class Router:
    def __init__(self, logger, service, router):
        pass
def oversized({parameters}):
    pass
def long_name({}):
    pass
"#,
                "p".repeat(17000)
            ),
        );
        let e = extract(root.path(), &python).unwrap();
        assert!(e.complete);
        assert_eq!(e.extractor_version, "python-static-8-generic-facts");
        let signatures: Vec<_> = e.facts.iter().filter(|f| f.kind == "signature").collect();
        assert_eq!(
            signatures.len(),
            e.facts
                .iter()
                .filter(|f| matches!(f.kind.as_str(), "function" | "async_function"))
                .count()
        );
        for (symbol, expected) in [
            (
                "signatures.ordered",
                serde_json::json!({"positional": ["z", "a", "b"], "positionalOnly": 2, "keywordOnly": ["k"], "vararg": "rest", "kwarg": "options"}),
            ),
            (
                "signatures.ordered.nested",
                serde_json::json!({"positional": ["x"], "positionalOnly": 0, "keywordOnly": ["y"]}),
            ),
            (
                "signatures.Router.__init__",
                serde_json::json!({"positional": ["self", "logger", "service", "router"], "positionalOnly": 0, "keywordOnly": []}),
            ),
            (
                "signatures.oversized",
                serde_json::json!({"truncated": true}),
            ),
            (
                "signatures.long_name",
                serde_json::json!({"truncated": true}),
            ),
        ] {
            let signature = signatures.iter().find(|f| f.symbol == symbol).unwrap();
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&signature.value).unwrap(),
                expected,
                "{symbol}"
            );
            let declaration = e
                .facts
                .iter()
                .find(|f| {
                    matches!(f.kind.as_str(), "function" | "async_function") && f.symbol == symbol
                })
                .unwrap();
            assert_eq!(
                (signature.line, signature.end_line),
                (declaration.line, declaration.end_line)
            );
            assert!(signature.value.len() <= 16384);
        }
        let serialized = serde_json::to_string(&e).unwrap();
        assert!(!serialized.contains("PRIVATE_DEFAULT"));
        assert!(!serialized.contains("PRIVATE_KEYWORD"));
        assert_eq!(
            serialized,
            serde_json::to_string(&extract(root.path(), &python).unwrap()).unwrap()
        );
    }

    #[test]
    fn router_metadata_is_bounded_source_syntax_and_empty_route_is_known() {
        let Some(python) = python() else {
            return;
        };
        let root = tempfile::tempdir().unwrap();
        source(
            root.path(),
            "app.py",
            r#"from fastapi import APIRouter as Router
router = Router(prefix='/caps')
ordinary = Config(prefix='/private_config', description='PRIVATE_DESCRIPTION')
@decorate(kind='PRIVATE_DECORATOR')
class SyntheticContext: pass
@router.get('')
def empty():
    client.post('/private_payload', json={'key':'PRIVATE_PAYLOAD'})
"#,
        );
        let evidence = extract(root.path(), &python).unwrap();
        assert_eq!(evidence.extractor_version, EXTRACTOR_VERSION);
        assert!(evidence.complete && evidence.diagnostics.is_empty());
        let decorator = evidence
            .facts
            .iter()
            .find(|f| f.kind == "decorator" && f.symbol == "app.SyntheticContext")
            .unwrap();
        assert_eq!(decorator.value, "decorate(kind=<literal>)");
        assert!(evidence.facts.iter().all(|f| !matches!(
            f.kind.as_str(),
            "router_registration"
                | "router_class_wiring"
                | "router_registry_rule"
                | "router_factory_rule"
        )));
        let route = evidence
            .facts
            .iter()
            .find(|f| f.kind == "api_route")
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&route.value).unwrap();
        assert_eq!(value["path"], "");
        assert_eq!(
            evidence
                .facts
                .iter()
                .filter(|f| f.kind == "router_prefix")
                .count(),
            1
        );
        let encoded = serde_json::to_string(&evidence).unwrap();
        for private in ["PRIVATE_", "private_config", "private_payload"] {
            assert!(!encoded.contains(private));
        }
        let catalog = crate::catalog::build(&evidence, "synthetic").unwrap();
        let mut original = evidence;
        original.facts.retain(|f| !f.kind.starts_with("router_"));
        assert_eq!(
            serde_json::to_value(catalog.items).unwrap(),
            serde_json::to_value(crate::catalog::build(&original, "synthetic").unwrap().items)
                .unwrap()
        );
    }

    #[test]
    fn route_discovery_preserves_contract_fragments_and_callable_scopes() {
        let Some(python) = python() else {
            return;
        };
        let root = tempfile::tempdir().unwrap();
        let content = r#"from fastapi import FastAPI, APIRouter
app = FastAPI(title="PRIVATE_TITLE")
router = APIRouter(prefix="/PRIVATE_PREFIX")
alias = router
@app.get("/items", headers={"PRIVATE_HEADER": "PRIVATE_TOKEN"})
@alias.api_route(path="/items/{item}", methods=["post", "GET", "POST"])
async def items():
    client.get("/PRIVATE_CLIENT_PATH", json={"PRIVATE_PAYLOAD": "PRIVATE_PASSWORD"})
    return "PRIVATE_RESULT"
class Service:
    def register(self):
        @self.router.post(
            path="/nested", response_model="PRIVATE_MODEL"
        )
        def handler():
            return client.post("/PRIVATE_BODY_PATH")
        return handler
@router.route("/health", methods=("HEAD", "OPTIONS"))
def health(): pass
@app.get("/same")
@app.get("/same")
def repeated(): pass
@app.get("/PRIVATE_CLASS_PATH")
class NotAHandler: pass
def plain():
    client.get("/PRIVATE_ORDINARY_PATH")
"#;
        source(root.path(), "src/routes.py", content);
        let evidence = extract(root.path(), &python).unwrap();
        assert!(evidence.complete);
        assert!(evidence.diagnostics.is_empty());
        assert_eq!(evidence.extractor_version, "python-static-8-generic-facts");
        let routes: Vec<_> = evidence
            .facts
            .iter()
            .filter(|f| f.kind == "api_route")
            .collect();
        assert_eq!(routes.len(), 6);
        for (route, (symbol, methods, path, receiver, line, end)) in routes.iter().zip([
            ("routes.items", vec!["GET"], "/items", "app", 5, 5),
            (
                "routes.items",
                vec!["GET", "POST"],
                "/items/{item}",
                "alias",
                6,
                6,
            ),
            (
                "routes.Service.register.handler",
                vec!["POST"],
                "/nested",
                "self.router",
                12,
                14,
            ),
            (
                "routes.health",
                vec!["HEAD", "OPTIONS"],
                "/health",
                "router",
                18,
                18,
            ),
            ("routes.repeated", vec!["GET"], "/same", "app", 20, 20),
            ("routes.repeated", vec!["GET"], "/same", "app", 21, 21),
        ]) {
            assert_eq!(route.symbol, symbol);
            assert_eq!((route.line, route.end_line), (line, end));
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&route.value).unwrap(),
                serde_json::json!({
                    "methods": methods, "path": path,
                    "receiver": receiver, "pathIsFragment": true
                })
            );
            assert!(evidence
                .facts
                .iter()
                .any(|f| f.symbol == symbol
                    && matches!(f.kind.as_str(), "function" | "async_function")));
            for kind in ["decorator", "call"] {
                let old = evidence
                    .facts
                    .iter()
                    .find(|f| f.kind == kind && f.symbol == symbol && f.line == line)
                    .unwrap();
                assert!(!old.value.contains(path));
                assert!(old.value.contains("<literal>"));
            }
        }
        assert_eq!(
            routes.iter().map(|f| &f.id).collect::<BTreeSet<_>>().len(),
            routes.len()
        );
        let serialized = serde_json::to_string(&evidence).unwrap();
        assert!(!serialized.contains("PRIVATE_"));
        assert_eq!(
            serialized,
            serde_json::to_string(&extract(root.path(), &python).unwrap()).unwrap()
        );
    }

    #[test]
    fn route_discovery_never_evaluates_dynamic_or_invalid_contracts() {
        let Some(python) = python() else {
            return;
        };
        let root = tempfile::tempdir().unwrap();
        let oversized = format!("/PRIVATE_OVERSIZED{}", "x".repeat(2048));
        let boundary = format!("/{}", "x".repeat(2047));
        let mut content = String::from(
            "import os\nrouter = APIRouter(prefix='/PRIVATE_PREFIX')\nPATH = '/PRIVATE_CONSTANT'\nMETHODS = ['GET']\n",
        );
        let paths = [
            "PATH".to_owned(),
            "f'/PRIVATE_FORMAT{suffix}'".into(),
            "os.getenv('PRIVATE_ENV')".into(),
            "'/PRIVATE_LEFT' + '/PRIVATE_RIGHT'".into(),
            "'PRIVATE_NO_SLASH'".into(),
            "b'/PRIVATE_BYTES'".into(),
            "123456789".into(),
            format!("'{oversized}'"),
            "None".into(),
            "*args".into(),
        ];
        for (i, path) in paths.iter().enumerate() {
            content.push_str(&format!("@router.get({path})\ndef dynamic_{i}(): pass\n"));
        }
        for (i, methods) in [
            "",
            ", methods=METHODS",
            ", methods=['GET', 'PRIVATE_UNKNOWN_METHOD']",
            ", methods=['GET', method]",
            ", methods='GET'",
            ", methods=[f'PRIVATE_METHOD{suffix}']",
        ]
        .iter()
        .enumerate()
        {
            content.push_str(&format!(
                "@router.route('/known'{methods})\ndef unknown_{i}(): pass\n"
            ));
        }
        content.push_str(&format!(
            "@router.get('{boundary}')\ndef boundary(): pass\n"
        ));
        content.push_str("@router.api_route(path='/empty', methods=[])\ndef empty(): pass\n");
        source(root.path(), "app.py", &content);
        let evidence = extract(root.path(), &python).unwrap();
        assert!(evidence.complete);
        let routes: Vec<_> = evidence
            .facts
            .iter()
            .filter(|f| f.kind == "api_route")
            .collect();
        assert_eq!(routes.len(), paths.len() + 8);
        for route in routes {
            let value: serde_json::Value = serde_json::from_str(&route.value).unwrap();
            assert_eq!(value["receiver"], "router");
            assert_eq!(value["pathIsFragment"], true);
            if route.symbol.starts_with("app.dynamic_") {
                assert!(value["path"].is_null());
                assert_eq!(value["methods"], serde_json::json!(["GET"]));
            } else if route.symbol.starts_with("app.unknown_") {
                assert_eq!(value["path"], "/known");
                assert_eq!(value["methods"], serde_json::json!([]));
            } else if route.symbol == "app.boundary" {
                assert_eq!(value["path"], boundary);
            } else {
                assert_eq!(route.symbol, "app.empty");
                assert_eq!(value["methods"], serde_json::json!([]));
            }
        }
        assert_eq!(evidence.diagnostics.len(), paths.len() + 6);
        for diagnostic in &evidence.diagnostics {
            assert_eq!(diagnostic.code, "dynamic_api_route");
            assert_eq!(diagnostic.path, "app.py");
            assert!(diagnostic.message.starts_with("line "));
        }
        let serialized = serde_json::to_string(&evidence).unwrap();
        assert!(!serialized.contains("PRIVATE_"));
        assert!(!serialized.contains("123456789"));
        assert!(!serialized.contains(&oversized));
        assert_eq!(
            serialized,
            serde_json::to_string(&extract(root.path(), &python).unwrap()).unwrap()
        );
    }

    #[test]
    fn route_discovery_whitelists_http_attributes_and_bounds_receivers() {
        let Some(python) = python() else {
            return;
        };
        let root = tempfile::tempdir().unwrap();
        let methods = [
            "GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "HEAD", "TRACE",
        ];
        let receiver = format!("Factory('PRIVATE_RECEIVER').{}", "router".repeat(120));
        let mut content = String::new();
        for method in methods {
            content.push_str(&format!(
                "@{receiver}.{}('/contract', methods=PRIVATE_METHODS)\ndef handle_{}(): pass\n",
                method.to_lowercase(),
                method.to_lowercase()
            ));
        }
        content.push_str(
            "@router.connect('/PRIVATE_CONNECT')\ndef connect(): pass\n\
             @router.GET('/PRIVATE_UPPERCASE')\ndef uppercase(): pass\n\
             @router.get\ndef not_called(): pass\n\
             @wrap(client.get('/PRIVATE_WRAPPED'))\ndef wrapped(): pass\n",
        );
        source(root.path(), "app.py", content);
        let evidence = extract(root.path(), &python).unwrap();
        assert!(evidence.complete);
        assert!(evidence.diagnostics.is_empty());
        let routes: Vec<_> = evidence
            .facts
            .iter()
            .filter(|f| f.kind == "api_route")
            .collect();
        assert_eq!(routes.len(), methods.len());
        for (route, method) in routes.iter().zip(methods) {
            let value: serde_json::Value = serde_json::from_str(&route.value).unwrap();
            assert_eq!(value["methods"], serde_json::json!([method]));
            assert_eq!(value["path"], "/contract");
            assert!(value["receiver"].as_str().unwrap().len() <= 512);
            assert!(value["receiver"]
                .as_str()
                .unwrap()
                .starts_with("Factory(<literal>)."));
        }
        assert!(!serde_json::to_string(&evidence)
            .unwrap()
            .contains("PRIVATE_"));
    }

    #[test]
    fn entrypoint_discovery_is_decorator_only_and_redacts_options() {
        let Some(python) = python() else {
            return;
        };
        let root = tempfile::tempdir().unwrap();
        source(
            root.path(),
            "app.py",
            r#"from worker import task
@cli.command('PRIVATE_COMMAND', help='PRIVATE_HELP')
def command():
    cli.command('PRIVATE_BODY_COMMAND')
@queue.task(name='PRIVATE_TASK')
@scheduler.job('PRIVATE_SCHEDULE')
@scheduler.scheduled_job(trigger='PRIVATE_TRIGGER')
async def job(): pass
@task()
def bare_call(): pass
@task
def bare(): pass
@unknown('PRIVATE_UNKNOWN')
def public_fallback(): pass
@cli.command('PRIVATE_CLASS')
class NotAFunction: pass
def ordinary():
    queue.task('PRIVATE_BODY_TASK')
"#,
        );
        let evidence = extract(root.path(), &python).unwrap();
        assert!(evidence.complete);
        assert!(evidence.diagnostics.is_empty());
        let entries: Vec<_> = evidence
            .facts
            .iter()
            .filter(|f| f.kind == "entrypoint")
            .collect();
        assert_eq!(entries.len(), 6);
        for (entry, (symbol, role, decorator)) in entries.iter().zip([
            ("app.command", "cli_command", "cli.command"),
            ("app.job", "job", "queue.task"),
            ("app.job", "job", "scheduler.job"),
            ("app.job", "job", "scheduler.scheduled_job"),
            ("app.bare_call", "job", "task"),
            ("app.bare", "job", "task"),
        ]) {
            assert_eq!(entry.symbol, symbol);
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&entry.value).unwrap(),
                serde_json::json!({ "role": role, "decorator": decorator })
            );
            assert!(evidence.facts.iter().any(|f| f.kind == "decorator"
                && f.symbol == symbol
                && f.line == entry.line
                && f.end_line == entry.end_line));
        }
        assert!(!evidence.facts.iter().any(|f| f.kind == "api_route"));
        assert!(evidence
            .facts
            .iter()
            .any(|f| f.kind == "function" && f.symbol == "app.public_fallback"));
        let serialized = serde_json::to_string(&evidence).unwrap();
        assert!(!serialized.contains("PRIVATE_"));
        assert_eq!(
            serialized,
            serde_json::to_string(&extract(root.path(), &python).unwrap()).unwrap()
        );
    }

    #[test]
    fn complex_call_arguments_cannot_consume_the_callee_budget() {
        let Some(python) = python() else {
            return;
        };
        let root = tempfile::tempdir().unwrap();
        let row = format!("[{}]", vec!["0"; 16].join(","));
        let matrix = format!("[{}]", vec![row; 16].join(","));
        source(root.path(), "app.py", format!("from starlette.convertors import register_url_convertor as reg\nreg('PRIVATE_PATH', Custom({matrix}))\n"));
        let evidence = extract(root.path(), &python).unwrap();
        let calls: Vec<_> = evidence.facts.iter().filter(|f| f.kind == "call").collect();
        assert_eq!(calls.len(), 2, "no duplicate auxiliary call facts");
        assert!(calls.iter().any(|f| f.value.starts_with("reg(")));
        assert!(calls.iter().any(|f| f.value.starts_with("Custom(")));
        assert!(calls.iter().all(|f| f.value.len() <= 512));
        assert!(!serde_json::to_string(&evidence)
            .unwrap()
            .contains("PRIVATE_PATH"));
    }

    #[test]
    fn bindings_preserve_constructor_context_and_nested_scopes() {
        let Some(python) = python() else {
            return;
        };
        let root = tempfile::tempdir().unwrap();
        source(
            root.path(),
            "src/service.py",
            r#"
app = FastAPI(title="PRIVATE_APP_TITLE")
class Service:
    shared = Factory()
    def __init__(self, service: Dependency, ctx: Context):
        self.service = service
        self.search: SearchOrchestrator = SearchOrchestrator(ctx=ctx)
        self.container.worker = service.worker
        first = second = self.service
        def nested():
            local: Worker = Worker(self.service)
            return local.run()
        after = service
    @classmethod
    def configure(cls, service):
        cls.service = service
"#,
        );
        let evidence = extract(root.path(), &python).unwrap();
        assert!(evidence.complete);
        assert_eq!(evidence.extractor_version, "python-static-8-generic-facts");
        let bindings: BTreeSet<_> = evidence
            .facts
            .iter()
            .filter(|f| f.kind == "binding")
            .map(|f| {
                let value: serde_json::Value = serde_json::from_str(&f.value).unwrap();
                assert_eq!(value.as_object().unwrap().len(), 2);
                (
                    f.symbol.as_str(),
                    value["target"].as_str().unwrap().to_owned(),
                    value["source"].as_str().unwrap().to_owned(),
                )
            })
            .collect();
        let expected: BTreeSet<_> = [
            ("service", "app", "FastAPI(title=<literal>)"),
            ("service.Service", "shared", "Factory()"),
            ("service.Service.__init__", "self.service", "service"),
            (
                "service.Service.__init__",
                "self.search",
                "SearchOrchestrator(ctx=ctx)",
            ),
            (
                "service.Service.__init__",
                "self.container.worker",
                "service.worker",
            ),
            ("service.Service.__init__", "first", "self.service"),
            ("service.Service.__init__", "second", "self.service"),
            (
                "service.Service.__init__.nested",
                "local",
                "Worker(self.service)",
            ),
            ("service.Service.__init__", "after", "service"),
            ("service.Service.configure", "cls.service", "service"),
        ]
        .into_iter()
        .map(|(scope, target, source)| (scope, target.to_owned(), source.to_owned()))
        .collect();
        assert_eq!(bindings, expected);
        let chained: Vec<_> = evidence
            .facts
            .iter()
            .filter(|f| f.kind == "binding" && f.value.contains("\"source\":\"self.service\""))
            .collect();
        assert_eq!(chained.len(), 2);
        assert_eq!(chained[0].line, chained[1].line);
        assert_ne!(chained[0].id, chained[1].id);
        for (kind, scope, value) in [
            (
                "parameter_annotation",
                "service.Service.__init__.service",
                "Dependency",
            ),
            (
                "call",
                "service.Service.__init__",
                "SearchOrchestrator(ctx=ctx)",
            ),
            ("call", "service.Service.__init__.nested", "local.run()"),
        ] {
            assert!(evidence
                .facts
                .iter()
                .any(|f| f.kind == kind && f.symbol == scope && f.value == value));
        }
        let serialized = serde_json::to_string(&evidence).unwrap();
        assert!(!serialized.contains("PRIVATE_APP_TITLE"));
        assert_eq!(
            serialized,
            serde_json::to_string(&extract(root.path(), &python).unwrap()).unwrap()
        );
        assert!(module_manifest(&evidence, "Bindings").edges.is_empty());
    }

    #[test]
    fn bindings_skip_data_and_unresolved_targets_without_losing_calls() {
        let Some(python) = python() else {
            return;
        };
        let root = tempfile::tempdir().unwrap();
        source(
            root.path(),
            "app.py",
            r#"
text = "PRIVATE_CONSTANT"
number = 123456789
flag = True
empty = None
negative = -987654321
mapping = {"password": "PRIVATE_PASSWORD", "service": Factory()}
items = [Factory(), "PRIVATE_LIST"]
pair = (Factory(), "PRIVATE_TUPLE")
options = {"PRIVATE_SET"}
formatted = f"PRIVATE_FORMAT{value}"
declared: "PRIVATE_ANNOTATION"
annotated: str = "PRIVATE_ANNOTATED"
left, right = Factory()
[first, *rest] = Factory()
other.service = Factory()
self["PRIVATE_KEY"] = Factory()
self.container["PRIVATE_KEY"].service = Factory()
Factory().service = Factory()
computed = left or right
augmented += Factory()
"#,
        );
        let evidence = extract(root.path(), &python).unwrap();
        assert!(evidence.complete);
        assert!(!evidence.facts.iter().any(|f| f.kind == "binding"));
        let invalidated: BTreeSet<_> = evidence
            .facts
            .iter()
            .filter(|f| f.kind == "binding_invalidation")
            .map(|f| {
                let v: serde_json::Value = serde_json::from_str(&f.value).unwrap();
                assert_eq!(v.as_object().unwrap().len(), 1);
                v["target"].as_str().unwrap().to_owned()
            })
            .collect();
        assert_eq!(
            invalidated,
            [
                "text",
                "number",
                "flag",
                "empty",
                "negative",
                "mapping",
                "items",
                "pair",
                "options",
                "formatted",
                "annotated",
                "left",
                "right",
                "first",
                "rest",
                "computed"
            ]
            .into_iter()
            .map(str::to_owned)
            .collect()
        );
        // The two identical Factory() calls on one line share an existing fact ID.
        assert_eq!(
            evidence.facts.iter().filter(|f| f.kind == "call").count(),
            10
        );
        let serialized = serde_json::to_string(&evidence).unwrap();
        for secret in ["PRIVATE_", "123456789", "987654321", "password"] {
            assert!(!serialized.contains(secret), "leaked {secret}");
        }
    }

    #[test]
    fn lexical_parameters_cover_every_argument_and_are_deterministic() {
        let Some(python) = python() else {
            return;
        };
        let root = tempfile::tempdir().unwrap();
        source(root.path(), "app.py", "async def run(\n    positional, /, normal: Type = 'PRIVATE_DEFAULT',\n    *args, keyword, typed: Type = None, **kwargs\n):\n    pass\n");
        let e = extract(root.path(), &python).unwrap();
        let parameters: Vec<_> = e.facts.iter().filter(|f| f.kind == "parameter").collect();
        assert_eq!(parameters.len(), 6);
        for (name, line) in [
            ("positional", 2),
            ("normal", 2),
            ("args", 3),
            ("keyword", 3),
            ("typed", 3),
            ("kwargs", 3),
        ] {
            let f = parameters.iter().find(|f| f.value == name).unwrap();
            assert_eq!(f.symbol, format!("app.run.{name}"));
            assert_eq!((f.line, f.end_line), (line, line));
        }
        let serialized = serde_json::to_string(&e).unwrap();
        assert!(!serialized.contains("PRIVATE_DEFAULT"));
        assert_eq!(
            serialized,
            serde_json::to_string(&extract(root.path(), &python).unwrap()).unwrap()
        );
    }

    #[test]
    fn extracted_parameters_shadow_global_and_imported_callables() {
        let Some(python) = python() else {
            return;
        };
        for imported in [false, true] {
            for annotation in ["", ": Helper"] {
                let root = tempfile::tempdir().unwrap();
                source(
                    root.path(),
                    "dep.py",
                    "def helper():\n    pass\ndef unrelated():\n    pass\nclass Helper:\n    pass\n",
                );
                let prefix = if imported {
                    "from dep import unrelated as helper, Helper\n"
                } else {
                    "def helper():\n    pass\nclass Helper:\n    pass\n"
                };
                source(
                    root.path(),
                    "app.py",
                    format!("{prefix}def run(helper{annotation}):\n    helper()\n"),
                );
                let e = extract(root.path(), &python).unwrap();
                let p = crate::focus::prepare(
                    &e,
                    &crate::focus::FocusOptions {
                        symbol: "app.run".into(),
                        hops: 2,
                        max_facts: 100,
                    },
                )
                .unwrap();
                assert!(
                    p.input["focus"]["links"].as_array().unwrap().is_empty(),
                    "{}",
                    p.input
                );
                assert_eq!(p.input["focus"]["unresolved"].as_array().unwrap().len(), 1);
                assert!(p.input["facts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|f| f["kind"] == "parameter"));
            }
        }
    }

    #[test]
    fn extracted_literal_overwrites_block_old_receiver_candidates() {
        let Some(python) = python() else {
            return;
        };
        for assignment in [
            "obj = None",
            "obj = {'PRIVATE_KEY': 'PRIVATE_VALUE'}",
            "obj: Dependency = None",
            "obj, other = pair",
            "obj: Dependency",
        ] {
            let root = tempfile::tempdir().unwrap();
            source(root.path(), "app.py", format!("class Dependency:\n    def execute(self):\n        pass\ndef run():\n    obj = Dependency()\n    {assignment}\n    obj.execute()\n"));
            let e = extract(root.path(), &python).unwrap();
            let p = crate::focus::prepare(
                &e,
                &crate::focus::FocusOptions {
                    symbol: "app.run".into(),
                    hops: 2,
                    max_facts: 100,
                },
            )
            .unwrap();
            let bare = assignment == "obj: Dependency";
            assert_eq!(
                p.input["focus"]["links"].as_array().unwrap().len(),
                usize::from(bare),
                "{assignment}: {}",
                p.input
            );
            let invalidations: Vec<_> = e
                .facts
                .iter()
                .filter(|f| f.kind == "binding_invalidation")
                .collect();
            assert_eq!(invalidations.is_empty(), bare);
            for f in invalidations {
                let v: serde_json::Value = serde_json::from_str(&f.value).unwrap();
                assert_eq!(v.as_object().unwrap().len(), 1);
                assert!(v["target"] == "obj" || v["target"] == "other");
            }
            assert!(!serde_json::to_string(&e).unwrap().contains("PRIVATE_"));
        }
    }

    #[test]
    fn extracted_constructor_field_and_ancestor_overwrites_block_old_context() {
        let Some(python) = python() else {
            return;
        };
        for target in ["self.container.dep", "self.container"] {
            for in_method in [false, true] {
                let root = tempfile::tempdir().unwrap();
                let overwrite = format!("        {target} = None\n");
                source(root.path(), "app.py", format!("class Dependency:\n    def execute(self):\n        pass\nclass Service:\n    def __init__(self):\n        self.container.dep = Dependency()\n{}    def run(self):\n{}        self.container.dep.execute()\n",
                    if in_method { "" } else { &overwrite }, if in_method { &overwrite } else { "" }));
                let e = extract(root.path(), &python).unwrap();
                let refs: Vec<_> = e.facts.iter().collect();
                let contexts = crate::packet::lexical_contexts(&refs);
                let call = refs
                    .iter()
                    .position(|f| f.value == "self.container.dep.execute()")
                    .unwrap();
                let invalidation = contexts[&call].binding.unwrap();
                assert_eq!(refs[invalidation].kind, "binding_invalidation");
                assert!(!contexts[&call]
                    .neighborhood
                    .iter()
                    .any(|&i| refs[i].kind == "binding"));
                let p = crate::focus::prepare(
                    &e,
                    &crate::focus::FocusOptions {
                        symbol: "app.Service.run".into(),
                        hops: 2,
                        max_facts: 100,
                    },
                )
                .unwrap();
                assert!(
                    p.input["focus"]["links"].as_array().unwrap().is_empty(),
                    "{}",
                    p.input
                );
            }
        }
    }

    #[test]
    fn binding_expressions_are_bounded_and_redact_nested_credentials() {
        let Some(python) = python() else {
            return;
        };
        let root = tempfile::tempdir().unwrap();
        let long_name = "dependency".repeat(100);
        source(
            root.path(),
            "app.py",
            format!(
                r#"
{long_name} = {long_name}("PRIVATE_LONG_SECRET")
client = Client(token="PRIVATE_TOKEN", options={{"password": "PRIVATE_PASSWORD"}}, values=["PRIVATE_VALUE"], callback=lambda: "PRIVATE_LAMBDA", endpoint=f"PRIVATE_ENDPOINT{{name}}")
dynamic = getattr(service, "PRIVATE_ATTRIBUTE")
"#
            ),
        );
        let evidence = extract(root.path(), &python).unwrap();
        assert!(evidence.complete);
        let bindings: Vec<_> = evidence
            .facts
            .iter()
            .filter(|f| f.kind == "binding")
            .collect();
        assert_eq!(bindings.len(), 3);
        for binding in bindings {
            let value: serde_json::Value = serde_json::from_str(&binding.value).unwrap();
            for key in ["target", "source"] {
                assert!(value[key].as_str().unwrap().len() <= 512);
            }
        }
        let serialized = serde_json::to_string(&evidence).unwrap();
        assert!(!serialized.contains("PRIVATE_"));
        assert!(!serialized.contains("password"));
        assert!(module_manifest(&evidence, "Dynamic").edges.is_empty());
    }

    #[test]
    fn resolves_local_absolute_relative_and_alias_imports_only() {
        let Some(python) = python() else {
            return;
        };
        let root = tempfile::tempdir().unwrap();
        source(root.path(), "src/app/__init__.py", "from . import worker\n");
        source(
            root.path(),
            "src/app/worker.py",
            "from .util import Tool\nimport app.util as u\nfrom external import X\n",
        );
        source(root.path(), "src/app/util.py", "class Tool: pass\n");
        source(root.path(), "src/app/sub/__init__.py", "");
        source(
            root.path(),
            "src/app/sub/job.py",
            "from .. import util\nfrom ..worker import run\n",
        );
        let evidence = extract(root.path(), &python).unwrap();
        assert!(evidence.complete);
        let expected: BTreeSet<(String, String)> = [
            ("src/app/__init__.py", "src/app/worker.py"),
            ("src/app/worker.py", "src/app/util.py"),
            ("src/app/sub/job.py", "src/app/util.py"),
            ("src/app/sub/job.py", "src/app/worker.py"),
        ]
        .into_iter()
        .map(|(a, b)| (a.into(), b.into()))
        .collect();
        assert_eq!(graph_edges(&evidence), expected);
        assert_eq!(module_manifest(&evidence, "Map").nodes.len(), 5);
    }

    #[test]
    fn root_package_and_ambiguous_modules() {
        let Some(python) = python() else {
            return;
        };
        let outer = tempfile::tempdir().unwrap();
        let root = outer.path().join("app");
        source(&root, "__init__.py", "from . import worker\n");
        source(
            &root,
            "worker.py",
            "import app.helper\nfrom . import ambiguous\n",
        );
        source(&root, "helper.py", "");
        source(&root, "ambiguous.py", "");
        source(&root, "ambiguous/__init__.py", "");
        let evidence = extract(&root, &python).unwrap();
        assert!(evidence
            .facts
            .iter()
            .any(|f| f.kind == "module" && f.value == "app.worker"));
        assert_eq!(
            graph_edges(&evidence),
            [
                ("__init__.py".into(), "worker.py".into()),
                ("worker.py".into(), "helper.py".into()),
            ]
            .into_iter()
            .collect::<BTreeSet<_>>()
        );
    }

    #[test]
    fn excludes_operator_and_non_python_data_and_prefers_src() {
        let Some(python) = python() else {
            return;
        };
        let root = tempfile::tempdir().unwrap();
        for prefix in [
            ".git",
            ".venv",
            "venv",
            "__pycache__",
            "node_modules",
            ".ideomancer",
            "caches",
            "build",
            "dist",
            "scripts",
            "migrations",
            "shadow",
            "tests",
            "alembic",
            "env",
            "data",
            "db",
        ] {
            source(
                root.path(),
                &format!("src/{prefix}/bad.py"),
                "this is invalid python !!!",
            );
        }
        source(root.path(), "src/app.py", "pass\n");
        source(root.path(), "src/.env", [0xff, 0xfe, 0xfd]);
        source(root.path(), "src/database.db", [0xff, 0xfe]);
        source(root.path(), "outside.py", "invalid !!!");
        let evidence = extract(root.path(), &python).unwrap();
        assert!(evidence.complete);
        assert_eq!(evidence.files.len(), 1);
        assert_eq!(evidence.files[0].path, "src/app.py");
    }

    #[test]
    fn parse_failure_is_incomplete_but_dynamic_discovery_is_not() {
        let Some(python) = python() else {
            return;
        };
        let root = tempfile::tempdir().unwrap();
        source(
            root.path(),
            "app.py",
            "from importlib import import_module as load\nload(name)\npkgutil.walk_packages()\n",
        );
        let evidence = extract(root.path(), &python).unwrap();
        assert!(evidence.complete);
        assert_eq!(
            evidence
                .diagnostics
                .iter()
                .filter(|d| d.code == "dynamic_discovery_unresolved")
                .count(),
            2
        );
        assert!(module_manifest(&evidence, "Dynamic").edges.is_empty());
        source(root.path(), "broken.py", "def bad(:\n");
        let evidence = extract(root.path(), &python).unwrap();
        assert!(!evidence.complete);
        assert!(evidence
            .diagnostics
            .iter()
            .any(|d| d.code == "parse_failure" && d.path == "broken.py"));
        assert!(evidence
            .facts
            .iter()
            .any(|f| f.kind == "module" && f.path == "broken.py"));
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_reported_never_read_and_root_symlink_rejected() {
        use std::os::unix::fs::symlink;
        let Some(python) = python() else {
            return;
        };
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        source(outside.path(), "private.py", "INVALID !!!");
        source(root.path(), "app.py", "pass");
        symlink(
            outside.path().join("private.py"),
            root.path().join("linked.py"),
        )
        .unwrap();
        symlink(outside.path(), root.path().join("package")).unwrap();
        symlink(outside.path(), root.path().join(".venv")).unwrap();
        let evidence = extract(root.path(), &python).unwrap();
        assert!(!evidence.complete);
        assert_eq!(evidence.files.len(), 1);
        assert_eq!(
            evidence
                .diagnostics
                .iter()
                .filter(|d| d.code == "symlink_skipped")
                .count(),
            3
        );
        let linked_root = outside.path().join("root");
        symlink(root.path(), &linked_root).unwrap();
        assert!(extract(&linked_root, &python)
            .unwrap_err()
            .contains("non-symlink"));
        assert!(extract(&linked_root.join("."), &python)
            .unwrap_err()
            .contains("non-symlink"));
        fs::remove_file(root.path().join("linked.py")).unwrap();
        fs::remove_file(root.path().join("package")).unwrap();
        assert!(extract(root.path(), &python).unwrap().complete);
        symlink(outside.path(), root.path().join("src")).unwrap();
        let linked_src = extract(root.path(), &python).unwrap();
        assert!(!linked_src.complete);
        assert!(linked_src.files.is_empty());
        assert!(linked_src.diagnostics.iter().any(|d| d.path == "src"));
    }

    #[test]
    fn anchored_file_open_rejects_replacement_symlink_and_keeps_open_fd() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        source(root.path(), "app.py", "original source");
        source(outside.path(), "private.py", "must never be read");
        let (anchor, _) = open_root(root.path()).unwrap();
        let path = Path::new("app.py");
        let before = statat(&anchor, path, AtFlags::SYMLINK_NOFOLLOW).unwrap();
        assert_eq!(
            FileType::from_raw_mode(before.st_mode),
            FileType::RegularFile
        );
        let mut opened = open_anchored(&anchor, path, false).unwrap();
        assert!(opened.metadata().unwrap().is_file());
        assert!(rustix::fs::fcntl_getfl(&opened)
            .unwrap()
            .contains(OFlags::NONBLOCK));
        fs::remove_file(root.path().join(path)).unwrap();
        symlink(outside.path().join("private.py"), root.path().join(path)).unwrap();
        assert!(open_anchored(&anchor, path, false).is_err());
        let mut contents = String::new();
        opened.read_to_string(&mut contents).unwrap();
        assert_eq!(contents, "original source");
        // Even an in-root symlink is refused, not merely an escaping link.
        fs::remove_file(root.path().join(path)).unwrap();
        source(root.path(), "other.py", "pass");
        symlink("other.py", root.path().join(path)).unwrap();
        assert!(open_anchored(&anchor, path, false).is_err());
    }

    #[test]
    fn anchored_open_rejects_replaced_parent_at_every_depth() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        source(root.path(), "pkg/deep/app.py", "original");
        source(outside.path(), "deep/app.py", "outside");
        source(outside.path(), "app.py", "outside");
        let (anchor, _) = open_root(root.path()).unwrap();
        let path = Path::new("pkg/deep/app.py");
        assert!(open_anchored(&anchor, path, false).is_ok());
        fs::rename(root.path().join("pkg"), root.path().join("saved_pkg")).unwrap();
        symlink(outside.path(), root.path().join("pkg")).unwrap();
        assert!(open_anchored(&anchor, path, false).is_err());
        assert!(open_anchored(&anchor, Path::new("pkg"), true).is_err());
        fs::remove_file(root.path().join("pkg")).unwrap();
        fs::rename(root.path().join("saved_pkg"), root.path().join("pkg")).unwrap();
        fs::rename(root.path().join("pkg/deep"), root.path().join("saved_deep")).unwrap();
        symlink(outside.path(), root.path().join("pkg/deep")).unwrap();
        assert!(open_anchored(&anchor, path, false).is_err());
        assert!(open_anchored(&anchor, Path::new("pkg/deep"), true).is_err());
    }

    #[test]
    fn anchored_open_rejects_fifo_replacement_and_non_regular_descriptors() {
        let root = tempfile::tempdir().unwrap();
        source(root.path(), "app.py", "original");
        let (anchor, _) = open_root(root.path()).unwrap();
        let path = Path::new("app.py");
        let discovered = statat(&anchor, path, AtFlags::SYMLINK_NOFOLLOW).unwrap();
        assert_eq!(
            FileType::from_raw_mode(discovered.st_mode),
            FileType::RegularFile
        );
        fs::remove_file(root.path().join(path)).unwrap();
        assert!(Command::new("mkfifo")
            .arg(root.path().join(path))
            .status()
            .unwrap()
            .success());
        // No writer exists. This would block indefinitely without O_NONBLOCK;
        // the opened FIFO must then be rejected by descriptor metadata.
        let error = open_anchored(&anchor, path, false).unwrap_err();
        assert!(error.to_string().contains("regular file"));
        fs::create_dir(root.path().join("directory.py")).unwrap();
        assert!(open_anchored(&anchor, Path::new("directory.py"), false).is_err());
        source(root.path(), "regular.py", "pass");
        assert!(open_anchored(&anchor, Path::new("regular.py"), true).is_err());
        for invalid in ["../outside.py", "pkg/../../outside.py", "/etc/passwd"] {
            assert!(open_anchored(&anchor, Path::new(invalid), false).is_err());
        }
    }

    #[test]
    fn anchored_root_survives_path_replacement_without_following_new_root() {
        use std::os::unix::fs::symlink;
        let outer = tempfile::tempdir().unwrap();
        let root = outer.path().join("root");
        let outside = outer.path().join("outside");
        source(&root, "app.py", "original");
        source(&outside, "app.py", "outside");
        let (anchor, _) = open_root(&root).unwrap();
        fs::rename(&root, outer.path().join("original_root")).unwrap();
        symlink(&outside, &root).unwrap();
        assert!(open_root(&root).is_err());
        let mut opened = open_anchored(&anchor, Path::new("app.py"), false).unwrap();
        let mut contents = String::new();
        opened.read_to_string(&mut contents).unwrap();
        assert_eq!(contents, "original");
    }

    #[test]
    fn hard_size_and_count_limits_fail_instead_of_partial_success() {
        let root = tempfile::tempdir().unwrap();
        let missing = Path::new("/nonexistent/ideomancer-python");
        source(root.path(), "big.py", vec![b'#'; MAX_FILE + 1]);
        assert!(extract(root.path(), missing).unwrap_err().contains("1 MiB"));
        fs::remove_file(root.path().join("big.py")).unwrap();
        for i in 0..17 {
            source(root.path(), &format!("m{i}.py"), vec![b'#'; MAX_FILE]);
        }
        assert!(extract(root.path(), missing)
            .unwrap_err()
            .contains("16 MiB"));
        let count_root = tempfile::tempdir().unwrap();
        for i in 0..=MAX_FILES {
            source(count_root.path(), &format!("m{i}.py"), "");
        }
        assert!(extract(count_root.path(), missing)
            .unwrap_err()
            .contains("5000"));
    }

    #[test]
    fn missing_interpreter_is_actionable_and_root_must_exist() {
        let root = tempfile::tempdir().unwrap();
        let error = extract(root.path(), Path::new("/nonexistent/ideomancer-python")).unwrap_err();
        assert!(error.contains("Select an installed Python >=3.12"));
        assert!(extract(&root.path().join("missing"), Path::new("python3.12")).is_err());
    }

    #[test]
    fn bounded_runner_deadline_includes_detached_pipe_holders_after_child_exit() {
        let Some(python) = python() else {
            return;
        };
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                if let Some(pid) = fs::read_to_string(&self.0)
                    .ok()
                    .and_then(|text| text.parse::<i32>().ok())
                    .and_then(rustix::process::Pid::from_raw)
                {
                    let _ = rustix::process::kill_process(pid, rustix::process::Signal::KILL);
                }
            }
        }
        let script = r#"
import os, sys, time
start_r, start_w = os.pipe()
ready_r, ready_w = os.pipe()
pid = os.fork()
if pid == 0:
    os.close(start_w)
    os.close(ready_r)
    if os.read(start_r, 1) != b'1': os._exit(0)
    os.close(start_r)
    os.setsid()
    os.write(ready_w, b'1')
    os.close(ready_w)
    time.sleep(30)
    os._exit(0)
os.close(start_r)
os.close(ready_w)
# Record before permitting detachment so cleanup always knows the escaped PID.
with open(sys.argv[1], 'w') as file: file.write(str(pid))
os.write(start_w, b'1')
os.close(start_w)
os.read(ready_r, 1)
os.close(ready_r)
with open(sys.argv[2], 'w') as file: file.write('direct child exiting successfully')
os.write(1, b'buffered output before exit')
os._exit(0)
"#;
        // Parallel suites can delay interpreter startup beyond 100ms. Retry
        // incomplete setup, never a completed fixture or the I/O deadline itself.
        for _ in 0..10 {
            let files = tempfile::tempdir().unwrap();
            let pid_file = files.path().join("detached.pid");
            let cleanup = Cleanup(pid_file.clone());
            let exited = files.path().join("exited");
            let mut command = Command::new(&python);
            command
                .args(["-I", "-S", "-c", script])
                .arg(&pid_file)
                .arg(&exited)
                .current_dir(files.path())
                .env_clear()
                .env("PATH", std::env::var_os("PATH").unwrap_or_default())
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            let start = Instant::now();
            // Large stdin also exercises a detached reader retaining an unwritten pipe.
            let result = run_bounded_with_timeout(
                command,
                vec![b'x'; 1024 * 1024],
                "Python",
                Duration::from_millis(100),
            );
            drop(cleanup);
            let error = result.unwrap_err();
            assert!(error.contains("deadline"), "{error}");
            assert!(start.elapsed() < Duration::from_secs(2));
            if exited.is_file() {
                assert!(pid_file.is_file());
                return;
            }
        }
        panic!("fixture never completed setup within its 100ms deadline");
    }

    #[test]
    fn bounded_runner_handles_backpressure_partial_writes_and_postexit_drain() {
        let Some(python) = python() else {
            return;
        };
        let files = tempfile::tempdir().unwrap();
        let script = r#"
import sys
sys.stdout.buffer.write(b'o' * (1024 * 1024))
sys.stdout.buffer.flush()
sys.stderr.buffer.write(b'e' * (60 * 1024))
sys.stderr.buffer.flush()
while data := sys.stdin.buffer.read(8193):
    sys.stdout.buffer.write(data)
sys.stdout.buffer.flush()
"#;
        let mut command = Command::new(python);
        command
            .args(["-I", "-S", "-c", script])
            .current_dir(files.path())
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let input: Vec<_> = (0..4 * 1024 * 1024)
            .map(|index| (index % 251) as u8)
            .collect();
        let mut expected = vec![b'o'; 1024 * 1024];
        expected.extend_from_slice(&input);
        let output =
            run_bounded_with_timeout(command, input, "Python", Duration::from_secs(5)).unwrap();
        assert_eq!(output, expected);
    }

    #[test]
    fn bounded_runner_overflow_cancels_blocked_input_and_withholds_js_stderr() {
        let Some(python) = python() else {
            return;
        };
        let files = tempfile::tempdir().unwrap();
        let script = format!("import os\nwhile True: os.write(1, b'x' * {})\n", 64 * 1024);
        let mut command = Command::new(&python);
        command
            .args(["-I", "-S", "-c", &script])
            .current_dir(files.path())
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let start = Instant::now();
        let error = run_bounded_with_timeout(
            command,
            vec![b'x'; 1024 * 1024],
            "JavaScript/TypeScript",
            Duration::from_secs(5),
        )
        .unwrap_err();
        assert!(error.contains("bounded output limit"), "{error}");
        assert!(start.elapsed() < Duration::from_secs(5));
        let mut command = Command::new(python);
        command
            .args([
                "-I",
                "-S",
                "-c",
                "import sys; sys.stderr.write('PRIVATE_SOURCE_SYNTAX_ERROR'); sys.exit(1)",
            ])
            .current_dir(files.path())
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let error = run_bounded_with_timeout(
            command,
            Vec::new(),
            "JavaScript/TypeScript",
            Duration::from_secs(1),
        )
        .unwrap_err();
        assert!(error.contains("stderr withheld"));
        assert!(!error.contains("PRIVATE_SOURCE"));
    }

    #[test]
    fn bounded_output_and_strict_schema() {
        let mut pipe = Some(&b"123456"[..]);
        let mut bytes = Vec::new();
        assert!(read_available(&mut pipe, &mut bytes, 4)
            .unwrap_err()
            .to_string()
            .contains("bounded output"));
        assert_eq!(bytes.len(), 5);
        assert!(
            serde_json::from_str::<SourceFile>(r#"{"path":"a.py","sha256":"x","extra":true}"#)
                .is_err()
        );
        assert!(serde_json::from_str::<Diagnostic>(
            r#"{"code":"x","path":"a","message":"x","extra":1}"#
        )
        .is_err());
    }
}

#[cfg(all(test, not(unix)))]
#[test]
fn extraction_fails_closed_on_unsupported_platforms() {
    let error = extract(Path::new("."), Path::new("python3.12")).unwrap_err();
    assert!(error.contains("requires Unix"));
    assert!(error.contains("no insecure fallback"));
}
