//! Local discovery and bounded diagram pages. Source views never invoke an agent.
use crate::{
    analysis::{self, Evidence},
    analyze,
    catalog::{self, Catalog, Filter, Kind, Page},
    focus, render, workflow,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const LIMIT: u64 = 40 * 1024 * 1024;
const VIEW_VERSION: &str = "source-views.v1";
const CLASS_VERSION: &str = "source-views.v2-class-uml";
const CLASS_POLICY_VERSION: &str = "ideomancer.class-uml.v1";
const CLASS_METADATA_LIMIT: usize = 8 * 1024 * 1024;
const CLASS_SVG_LIMIT: usize = 64 * 1024 * 1024;
const CLASS_DIAGRAM_LIMIT: usize = 128;
const VIEWS: [(&str, &str); 5] = [
    ("class", "Class structure"),
    ("class-focus", "Class focus"),
    ("api", "API inventory"),
    ("network", "Network call candidates"),
    ("services", "Service and call candidates"),
];

pub struct Store {
    pub directory: PathBuf,
    pub catalog: Catalog,
    pub evidence: Evidence,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Browse {
    pub catalog_directory: PathBuf,
    pub snapshot_id: String,
    pub complete: bool,
    pub page: Page,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Receipt {
    schema_version: String,
    catalog_sha256: String,
    evidence_sha256: String,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ViewReceipt {
    schema_version: String,
    snapshot_id: String,
    mermaid_sha256: String,
    legend_sha256: String,
    html_sha256: String,
    svg_sha256: Option<String>,
    renderer: Option<PathBuf>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ClassReceipt {
    schema_version: String,
    manifest_hash: String,
    snapshot_id: String,
    policy_version: String,
    legend_hash: String,
    html_hash: String,
    diagrams: Vec<ClassDiagramReceipt>,
    artifacts: Vec<String>,
    renderer: Option<PathBuf>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ClassDiagramReceipt {
    id: String,
    title: String,
    mmd_sha: String,
    svg_sha: Option<String>,
}

// An explicitly tagged wire type rejects unknown fields and duplicate keys.
#[derive(Deserialize)]
#[serde(tag = "schemaVersion", deny_unknown_fields)]
enum ViewVersion {
    #[serde(rename = "source-views.v1")]
    V1 {
        #[serde(rename = "snapshotId")]
        snapshot_id: String,
        #[serde(rename = "mermaidSha256")]
        mermaid_sha256: String,
        #[serde(rename = "legendSha256")]
        legend_sha256: String,
        #[serde(rename = "htmlSha256")]
        html_sha256: String,
        #[serde(rename = "svgSha256")]
        svg_sha256: Option<String>,
        renderer: Option<PathBuf>,
    },
    #[serde(rename = "source-views.v2-class-uml")]
    V2 {
        #[serde(rename = "manifestHash")]
        manifest_hash: String,
        #[serde(rename = "snapshotId")]
        snapshot_id: String,
        #[serde(rename = "policyVersion")]
        policy_version: String,
        #[serde(rename = "legendHash")]
        legend_hash: String,
        #[serde(rename = "htmlHash")]
        html_hash: String,
        diagrams: Vec<ClassDiagramReceipt>,
        artifacts: Vec<String>,
        renderer: Option<PathBuf>,
    },
}

enum ValidatedView {
    V1(ViewReceipt),
    V2(ClassReceipt),
}

impl ValidatedView {
    fn snapshot_id(&self) -> &str {
        match self {
            Self::V1(r) => &r.snapshot_id,
            Self::V2(r) => &r.snapshot_id,
        }
    }
}

pub struct DiagramOptions {
    pub catalog: PathBuf,
    pub kind: Option<Kind>,
    pub entry: Option<String>,
    pub view: Option<render::endpoint::View>,
    pub openapi: Option<PathBuf>,
    pub query: Option<String>,
    pub path: Option<String>,
    pub limit: usize,
    pub offset: usize,
    pub pagination_explicit: bool,
    pub no_render: bool,
    pub extract_only: bool,
    pub mmdc: PathBuf,
    pub agent: PathBuf,
    pub model: Option<String>,
    pub python: PathBuf,
    pub focus_hops: usize,
    pub focus_facts: usize,
    pub timeout_seconds: u64,
    pub retry: bool,
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn encoded(value: &impl Serialize) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    if bytes.len() as u64 > LIMIT {
        return Err("Discovery artifact exceeds 40 MiB".into());
    }
    Ok(bytes)
}
fn directory(path: &Path) -> Result<PathBuf, String> {
    let path: PathBuf = path.components().collect();
    let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err("Catalog/view directory must not be a symlink".into());
    }
    fs::canonicalize(path).map_err(|e| e.to_string())
}
fn read(path: &Path) -> Result<Vec<u8>, String> {
    let meta = fs::symlink_metadata(path)
        .map_err(|e| format!("Read discovery {}: {e}", path.display()))?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > LIMIT {
        return Err("Discovery artifacts must be bounded regular non-symlink files".into());
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
    let file = options.open(path).map_err(|e| e.to_string())?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Discovery artifact is not regular".into());
    }
    let mut bytes = Vec::new();
    file.take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > LIMIT {
        return Err("Discovery artifact exceeded 40 MiB".into());
    }
    Ok(bytes)
}
fn verify(path: &Path, expected: &str) -> Result<Vec<u8>, String> {
    let bytes = read(path)?;
    if hash(&bytes) != expected {
        return Err(format!("Discovery integrity failure: {}", path.display()));
    }
    Ok(bytes)
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())
}

fn validate_v1_view(path: &Path) -> Result<ViewReceipt, String> {
    workflow::managed_files(
        path,
        &[
            "receipt.json",
            "diagram.mmd",
            "legend.json",
            "index.html",
            "diagram.svg",
        ],
        &[],
    )?;
    let receipt: ViewReceipt = serde_json::from_slice(&read(&path.join("receipt.json"))?)
        .map_err(|_| "Invalid source-view receipt")?;
    if receipt.schema_version != VIEW_VERSION {
        return Err("Unsupported source-view receipt".into());
    }
    verify(&path.join("diagram.mmd"), &receipt.mermaid_sha256)?;
    verify(&path.join("legend.json"), &receipt.legend_sha256)?;
    verify(&path.join("index.html"), &receipt.html_sha256)?;
    if let Some(expected) = &receipt.svg_sha256 {
        verify(&path.join("diagram.svg"), expected)?;
    } else if fs::symlink_metadata(path.join("diagram.svg")).is_ok() {
        return Err("Unrecorded source-view SVG exists".into());
    }
    Ok(receipt)
}

fn group_id(id: &str) -> bool {
    id.len() == 10 && id.starts_with("group-") && id.as_bytes()[6..].iter().all(u8::is_ascii_digit)
}

fn class_manifest_hash(receipt: &impl Serialize) -> Result<String, String> {
    let mut manifest = serde_json::to_value(receipt).map_err(|e| e.to_string())?;
    manifest.as_object_mut().unwrap().remove("manifestHash");
    manifest.sort_all_objects();
    // Detect un-rehashed metadata mutations, not adversarial authentication:
    // anyone who can replace the receipt can also recompute this unkeyed hash.
    Ok(hash(
        &serde_json::to_vec(&manifest).map_err(|e| e.to_string())?,
    ))
}

fn class_artifacts(receipt: &ClassReceipt) -> Result<BTreeSet<String>, String> {
    if receipt.schema_version != CLASS_VERSION
        || receipt.policy_version != CLASS_POLICY_VERSION
        || receipt.diagrams.len() > CLASS_DIAGRAM_LIMIT
        || receipt.artifacts.len() > 3 + 2 * CLASS_DIAGRAM_LIMIT
    {
        return Err("Unsupported or oversized class UML receipt".into());
    }
    let mut expected: BTreeSet<String> = ["receipt.json", "legend.json", "index.html"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    for diagram in &receipt.diagrams {
        if !group_id(&diagram.id)
            || !expected.insert(format!("{}.mmd", diagram.id))
            || diagram.svg_sha.is_some() != receipt.renderer.is_some()
        {
            return Err("Invalid, duplicate or inconsistent class UML diagram reference".into());
        }
        if diagram.svg_sha.is_some() {
            expected.insert(format!("{}.svg", diagram.id));
        }
    }
    let actual: BTreeSet<_> = receipt.artifacts.iter().cloned().collect();
    if actual.len() != receipt.artifacts.len() || actual != expected {
        return Err("Class UML artifact list must match exactly, including receipt.json".into());
    }
    Ok(expected)
}

fn validate_class_view(path: &Path, receipt: ClassReceipt) -> Result<ClassReceipt, String> {
    // Flat artifacts are not enough if a parent redirects through a symlink.
    for ancestor in path.ancestors().filter(|p| !p.as_os_str().is_empty()) {
        let metadata = fs::symlink_metadata(ancestor).map_err(|e| e.to_string())?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err("Class UML path components must be non-symlink directories".into());
        }
    }
    let expected = class_artifacts(&receipt)?;
    let names: Vec<_> = expected.iter().map(String::as_str).collect();
    workflow::managed_files(path, &names, &[])?;
    let mut metadata_size = read(&path.join("receipt.json"))?.len();
    metadata_size += verify(&path.join("legend.json"), &receipt.legend_hash)?.len();
    metadata_size += verify(&path.join("index.html"), &receipt.html_hash)?.len();
    let mut svg_size = 0;
    for diagram in &receipt.diagrams {
        metadata_size += verify(&path.join(format!("{}.mmd", diagram.id)), &diagram.mmd_sha)?.len();
        if let Some(expected) = &diagram.svg_sha {
            let bytes = verify(&path.join(format!("{}.svg", diagram.id)), expected)?;
            if bytes.len() > 8 * 1024 * 1024 {
                return Err("Class UML SVG exceeds 8 MiB".into());
            }
            svg_size += bytes.len();
        }
        if metadata_size > CLASS_METADATA_LIMIT || svg_size > CLASS_SVG_LIMIT {
            return Err("Class UML bundle exceeds aggregate size limits".into());
        }
    }
    if metadata_size > CLASS_METADATA_LIMIT {
        return Err("Class UML metadata exceeds 8 MiB".into());
    }
    Ok(receipt)
}

fn validate_view(path: &Path) -> Result<ValidatedView, String> {
    let bytes = read(&path.join("receipt.json"))?;
    if bytes.len() > CLASS_METADATA_LIMIT {
        return Err("Source-view receipt exceeds 8 MiB".into());
    }
    match serde_json::from_slice::<ViewVersion>(&bytes)
        .map_err(|e| format!("Invalid or unsupported source-view receipt: {e}"))?
    {
        ViewVersion::V1 {
            snapshot_id,
            mermaid_sha256,
            legend_sha256,
            html_sha256,
            svg_sha256,
            renderer,
        } => {
            // The tagged parse above prevents unknown-field/duplicate-key bypasses.
            let _ = (
                snapshot_id,
                mermaid_sha256,
                legend_sha256,
                html_sha256,
                svg_sha256,
                renderer,
            );
            validate_v1_view(path).map(ValidatedView::V1)
        }
        ViewVersion::V2 {
            manifest_hash,
            snapshot_id,
            policy_version,
            legend_hash,
            html_hash,
            diagrams,
            artifacts,
            renderer,
        } => {
            // Parse only after typed validation rejects unknown/duplicate fields.
            // Hash the actual JSON so omitted nullable fields differ from explicit nulls.
            let manifest: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            if manifest_hash != class_manifest_hash(&manifest)? {
                return Err("Class UML manifest integrity failure".into());
            }
            validate_class_view(
                path,
                ClassReceipt {
                    schema_version: CLASS_VERSION.into(),
                    manifest_hash,
                    snapshot_id,
                    policy_version,
                    legend_hash,
                    html_hash,
                    diagrams,
                    artifacts,
                    renderer,
                },
            )
            .map(ValidatedView::V2)
        }
    }
}

fn html_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

// A derived navigation page, not source evidence or a new history/cache identity.
// Caller holds the catalog lock; every linked bundle is verified before publication.
fn write_views_index(store: &Store) -> Result<PathBuf, String> {
    let base = store.directory.join("views");
    let app = Path::new(&store.catalog.source_root)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Application");
    let mut html = format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'\"><title>{} — Views</title></head><body>\n<h1>{} — Views</h1>\n<p>Current local diagrams. Static source candidates, not a runtime trace. Each view keeps its latest selection.</p>\n",
        html_text(app), html_text(app)
    );
    for (name, title) in VIEWS {
        let view = base.join(name);
        if !view.try_exists().map_err(|e| e.to_string())? {
            continue;
        }
        let receipt = validate_view(&view)?;
        if receipt.snapshot_id() != store.catalog.snapshot_id {
            return Err("Source view does not match its catalog snapshot".into());
        }
        html.push_str(&format!(
            "<section><h2><a href=\"{name}/index.html\">{title}</a></h2>\n<ul>"
        ));
        match receipt {
            ValidatedView::V1(r) => {
                html.push_str(&format!("<li><a href=\"{name}/diagram.mmd\">Mermaid source</a></li><li><a href=\"{name}/legend.json\">Legend and source references</a></li>"));
                if r.svg_sha256.is_some() {
                    html.push_str(&format!(
                        "<li><a href=\"{name}/diagram.svg\">SVG diagram</a></li>"
                    ));
                }
            }
            ValidatedView::V2(r) => {
                html.push_str(&format!(
                    "<li><a href=\"{name}/legend.json\">Legend and source references</a></li>"
                ));
                for diagram in r.diagrams {
                    let id = diagram.id;
                    let title = html_text(&diagram.title);
                    html.push_str(&format!(
                        "<li><a href=\"{name}/{id}.mmd\">{title} Mermaid source</a></li>"
                    ));
                    if diagram.svg_sha.is_some() {
                        html.push_str(&format!(
                            "<li><a href=\"{name}/{id}.svg\">{title} SVG diagram</a></li>"
                        ));
                    }
                }
            }
        }
        html.push_str("</ul></section>\n");
    }
    html.push_str("</body></html>\n");
    let path = base.join("index.html");
    match fs::symlink_metadata(&path) {
        Ok(_) if read(&path)? == html.as_bytes() => return Ok(path),
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.to_string()),
    }
    let mut temp = tempfile::NamedTempFile::new_in(&base).map_err(|e| e.to_string())?;
    temp.write_all(html.as_bytes())
        .and_then(|_| temp.as_file().sync_all())
        .map_err(|e| e.to_string())?;
    temp.persist(&path).map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn discover(root: &Path, out: &Path, python: &Path) -> Result<Store, String> {
    discover_with(root, out, || analysis::extract(root, python))
}

pub fn discover_javascript(
    root: &Path,
    out: &Path,
    node: &Path,
    typescript: &Path,
) -> Result<Store, String> {
    discover_with(root, out, || {
        analysis::extract_javascript(root, node, typescript)
    })
}

fn discover_with(
    root: &Path,
    out: &Path,
    extract: impl FnOnce() -> Result<Evidence, String>,
) -> Result<Store, String> {
    let source = fs::canonicalize(root).map_err(|e| e.to_string())?;
    let base = analyze::create_storage(out)?;
    workflow::reject_legacy_storage(&base)?;
    let destination = base.join(workflow::app_name(&source)?);
    let _lock = workflow::Lock::acquire(&destination)?;
    let previous = workflow::tree_snapshot(&destination)?;
    let evidence = extract()?;
    let catalog = catalog::build(
        &evidence,
        source.to_str().ok_or("Source root is not UTF-8")?,
    )?;
    let catalog_bytes = encoded(&catalog)?;
    let evidence_bytes = encoded(&evidence)?;
    let receipt = Receipt {
        schema_version: catalog::VERSION.into(),
        catalog_sha256: hash(&catalog_bytes),
        evidence_sha256: hash(&evidence_bytes),
    };
    if fs::symlink_metadata(&destination).is_ok() {
        let saved = load_locked(&destination)?;
        if saved.catalog.source_root != catalog.source_root {
            return Err(
                "Application name collides with another source root; choose a different --out"
                    .into(),
            );
        }
        if read(&destination.join("catalog.json"))? == catalog_bytes
            && read(&destination.join("evidence.json"))? == evidence_bytes
        {
            return Ok(saved);
        }
    }
    let stage = tempfile::Builder::new()
        .prefix(".catalog-init-")
        .tempdir_in(&base)
        .map_err(|e| e.to_string())?;
    write_new(&stage.path().join("catalog.json"), &catalog_bytes)?;
    write_new(&stage.path().join("evidence.json"), &evidence_bytes)?;
    write_new(&stage.path().join("receipt.json"), &encoded(&receipt)?)?;
    let staged = stage.path().to_path_buf();
    workflow::publish_current(stage, &staged, &destination, previous.as_ref())?;
    load_locked(&destination)
}

pub fn load(path: &Path) -> Result<Store, String> {
    let _lock = workflow::Lock::acquire(path)?;
    load_locked(path)
}

fn load_locked(path: &Path) -> Result<Store, String> {
    load_checked(path, true)
}

// Publication rechecks source identities while its private staging tree exists.
fn load_checked(path: &Path, check_views: bool) -> Result<Store, String> {
    let directory = directory(path)?;
    workflow::managed_files(
        &directory,
        &["receipt.json", "catalog.json", "evidence.json"],
        &["views"],
    )?;
    if check_views && fs::symlink_metadata(directory.join("views")).is_ok() {
        workflow::managed_files(
            &directory.join("views"),
            &["index.html"],
            &[
                "api",
                "class",
                "class-focus",
                "class-overview",
                "network",
                "services",
            ],
        )?;
        let index = directory.join("views/index.html");
        if fs::symlink_metadata(&index).is_ok() {
            // Navigation is regenerable, but unsafe/nonregular files still fail closed.
            read(&index)?;
        }
    }
    let receipt: Receipt = serde_json::from_slice(&read(&directory.join("receipt.json"))?)
        .map_err(|_| "Invalid catalog receipt")?;
    if receipt.schema_version != catalog::VERSION {
        return Err("Unsupported catalog version; rediscover source".into());
    }
    let catalog: Catalog = serde_json::from_slice(&verify(
        &directory.join("catalog.json"),
        &receipt.catalog_sha256,
    )?)
    .map_err(|_| "Invalid discovery catalog")?;
    let evidence: Evidence = serde_json::from_slice(&verify(
        &directory.join("evidence.json"),
        &receipt.evidence_sha256,
    )?)
    .map_err(|_| "Invalid saved discovery evidence")?;
    if catalog.schema_version != catalog::VERSION
        || catalog.snapshot_id != evidence.snapshot_id
        || catalog.extractor_version != evidence.extractor_version
        || catalog.complete != evidence.complete
    {
        return Err("Catalog does not match its saved evidence".into());
    }
    if check_views && directory.join("views").exists() {
        for name in VIEWS
            .iter()
            .map(|(name, _)| *name)
            .chain(["class-overview"])
        {
            let view = directory.join("views").join(name);
            if view.try_exists().map_err(|e| e.to_string())? {
                let receipt = validate_view(&view)?;
                if receipt.snapshot_id() != catalog.snapshot_id {
                    return Err("Source view does not match its catalog snapshot".into());
                }
                if matches!(receipt, ValidatedView::V2(_))
                    && !matches!(name, "class" | "class-focus")
                {
                    return Err("Class UML bundle is stored in a non-class view".into());
                }
            }
        }
    }
    // Rebuild from the stored evidence, never the live app, to bind item identity
    // and all source claims even if a receipt was manually regenerated.
    let rebuilt = catalog::build(&evidence, &catalog.source_root)?;
    if encoded(&rebuilt)? != encoded(&catalog)? {
        return Err("Catalog entries do not match their source evidence".into());
    }
    Ok(Store {
        directory,
        catalog,
        evidence,
    })
}

pub fn browse(store: &Store, filter: &Filter) -> Result<Browse, String> {
    Ok(Browse {
        catalog_directory: store.directory.clone(),
        snapshot_id: store.catalog.snapshot_id.clone(),
        complete: store.catalog.complete,
        page: catalog::page(&store.catalog, filter)?,
    })
}

pub fn diagram(options: DiagramOptions) -> Result<(Value, bool), String> {
    diagram_expected(options, None)
}

// The facade supplies identity expectations; CLI selections retain legacy behavior.
pub(crate) fn diagram_expected(
    options: DiagramOptions,
    expected: Option<(&Path, &str)>,
) -> Result<(Value, bool), String> {
    diagram_augmented(options, expected, None)
}

pub(crate) fn diagram_augmented(
    options: DiagramOptions,
    expected: Option<(&Path, &str)>,
    claims: Option<&crate::extensions::ValidatedClaimsRef>,
) -> Result<(Value, bool), String> {
    if options.view.is_some() && options.extract_only {
        return Err(
            "--extract-only cannot be combined with a local --view; use --no-render".into(),
        );
    }
    let _lock = workflow::Lock::acquire(&options.catalog)?;
    let store = load_locked(&options.catalog)?;
    if let Some(claims) = claims {
        crate::extensions::check_store(claims, &store)?;
    }
    if let Some((source_root, snapshot_id)) = expected {
        if Path::new(&store.catalog.source_root) != source_root
            || store.catalog.snapshot_id != snapshot_id
        {
            return Err("Catalog handle is stale: source root or snapshot changed".into());
        }
        if options.kind != Some(Kind::Class)
            || options.view.is_some()
            || options.openapi.is_some()
            || options.extract_only
        {
            return Err("Engine class operations require class-only selection".into());
        }
        if let Some(id) = &options.entry {
            if !store
                .catalog
                .items
                .iter()
                .any(|item| item.id == *id && item.kind == Kind::Class)
            {
                return Err("Class entry must be an exact class catalog ID".into());
            }
        }
    }
    if options.entry.is_some()
        && (options.query.is_some()
            || options.path.is_some()
            || options.offset != 0
            || options.limit != 12)
    {
        return Err("Use either one --entry or a filtered page, not both".into());
    }
    // A selected kind can disambiguate an API handler's separate workflow entry.
    // Explicit IDs remain exact, and ambiguity within one kind is still an error.
    let selection_catalog = if let Some(kind) = &options.kind {
        let mut filtered = store.catalog.clone();
        filtered.items.retain(|item| &item.kind == kind);
        Some(filtered)
    } else {
        None
    };
    let selected = options
        .entry
        .as_deref()
        .map(|selection| {
            if store.catalog.items.iter().any(|item| item.id == selection) {
                catalog::resolve(&store.catalog, selection)
            } else {
                catalog::resolve(
                    selection_catalog.as_ref().unwrap_or(&store.catalog),
                    selection,
                )
            }
        })
        .transpose()?;
    let kind = options
        .kind
        .clone()
        .or_else(|| selected.map(|item| item.kind.clone()))
        .ok_or("Choose --kind api/class or a catalog --entry")?;
    if options.view.is_some() {
        let item = selected.ok_or("--view network/services requires one API or callable workflow --entry; it never invokes a workflow agent")?;
        if item.kind != kind {
            return Err(format!(
                "Selected {:?} entry does not match the diagram kind {:?}",
                item.kind, kind
            ));
        }
        if !matches!(item.kind, Kind::Api | Kind::Workflow) {
            return Err(
                "--view network/services requires one API or callable workflow --entry".into(),
            );
        }
        if options.openapi.is_some() && item.kind != Kind::Api {
            return Err("--openapi requires one API --entry with --view network/services".into());
        }
    }
    if options.openapi.is_some() && options.view.is_none() {
        return Err("--openapi requires --view network/services".into());
    }
    if kind == Kind::Workflow && options.view.is_none() {
        let item = selected
            .ok_or("Select one workflow entry; workflow batches are not invoked automatically")?;
        if item.kind == Kind::Class {
            return Err("A class entry needs --kind class, not workflow analysis".into());
        }
        if store.evidence.is_javascript() {
            return Err("JavaScript/TypeScript agent analysis is not supported in this slice. Select --view services or --view network for a local saved-evidence diagram; no Python or agent was invoked".into());
        }
        let report = analyze::run(analyze::Options {
            root: store.catalog.source_root.clone().into(),
            out: ".ideomancer/analyses".into(),
            python: options.python,
            agent: options.agent,
            model: options.model,
            timeout_seconds: options.timeout_seconds,
            retry: options.retry,
            extract_only: options.extract_only,
            no_render: options.no_render,
            focus: Some(focus::FocusOptions {
                symbol: item.fact_id.clone(),
                hops: options.focus_hops,
                max_facts: options.focus_facts,
            }),
            mmdc: options.mmdc,
            expected_snapshot: Some(store.catalog.snapshot_id.clone()),
        })?;
        let success = report.error.is_none();
        let mut value = serde_json::to_value(&report).map_err(|e| e.to_string())?;
        value["catalogDirectory"] = json!(store.directory);
        value["entryId"] = json!(item.id);
        value["entrySymbol"] = json!(item.symbol);
        return Ok((value, success));
    }
    if options.extract_only {
        return Err("--extract-only is for workflow selection; use --no-render for local API/class source views".into());
    }
    let complete_classes = kind == Kind::Class
        && options.entry.is_none()
        && options.query.is_none()
        && options.path.is_none()
        && !options.pagination_explicit
        && options.view.is_none();
    if complete_classes {
        let documents =
            render::class_uml::build_augmented(&store.catalog, &store.evidence, None, claims)?;
        let (mut report, reused) =
            publish_class_view_checked(&store, &options, "class", &documents, claims)?;
        report["kind"] = json!(Kind::Class);
        report["selection"] = json!("all-catalog-classes");
        report["coverage"] = documents.legend["coverage"].clone();
        report["page"] = Value::Null;
        report["reused"] = json!(reused);
        report["viewsIndex"] = json!(write_views_index(&store)?);
        if expected.is_some() {
            engine_class_metadata(&mut report, &store, &documents);
        }
        return Ok((report, true));
    }
    let mut page = catalog::page(
        &store.catalog,
        &Filter {
            kind: Some(kind.clone()),
            query: options.query.clone(),
            path: options.path.clone(),
            limit: options.limit,
            offset: options.offset,
        },
    )?;
    if let Some(item) = selected {
        if item.kind != kind {
            return Err("Selected entry does not match the diagram kind".into());
        }
        page.items = vec![item.clone()];
        page.matched_items = 1;
        page.offset = 0;
        page.limit = 1;
        page.next_offset = None;
    }
    if kind == Kind::Class {
        let documents = render::class_uml::build_augmented(
            &store.catalog,
            &store.evidence,
            Some(&page),
            claims,
        )?;
        let (mut report, reused) =
            publish_class_view_checked(&store, &options, "class-focus", &documents, claims)?;
        report["kind"] = json!(Kind::Class);
        report["selection"] = json!("selected-classes");
        report["coverage"] = documents.legend["coverage"].clone();
        report["page"] = json!(page);
        report["reused"] = json!(reused);
        report["viewsIndex"] = json!(write_views_index(&store)?);
        if expected.is_some() {
            engine_class_metadata(&mut report, &store, &documents);
        }
        return Ok((report, true));
    }
    let documents = if let Some(view) = &options.view {
        let spec_path = if kind == Kind::Api {
            match &options.openapi {
                Some(path) => Some(path.clone()),
                None => {
                    let root = Path::new(&store.catalog.source_root);
                    let paths: Vec<_> = [
                        "openapi.json",
                        "openapi.yaml",
                        "openapi.yml",
                        "swagger.json",
                        "swagger.yaml",
                        "swagger.yml",
                    ]
                    .iter()
                    .map(|name| root.join(name))
                    .filter(|path| fs::symlink_metadata(path).is_ok())
                    .collect();
                    if paths.len() > 1 {
                        return Err(
                            "Multiple local API contracts found; select one with --openapi FILE"
                                .into(),
                        );
                    }
                    paths.into_iter().next()
                }
            }
        } else {
            None
        };
        let spec = spec_path.as_deref().map(crate::openapi::load).transpose()?;
        render::endpoint::build(
            &store.catalog,
            &store.evidence,
            &page.items[0],
            view,
            spec.as_ref(),
            options.focus_hops,
            options.focus_facts,
        )?
    } else {
        render::source_view::build(&store.catalog, &store.evidence, &page, &kind)?
    };
    let view_name = match &options.view {
        Some(render::endpoint::View::Network) => "network",
        Some(render::endpoint::View::Services) => "services",
        None => match kind {
            Kind::Api => "api",
            Kind::Class => "class",
            Kind::Workflow => unreachable!(),
        },
    };
    let (mut report, _) = publish_view(&store, &options, view_name, &documents)?;
    report["kind"] = json!(kind);
    report["page"] = json!(page);
    report["viewsIndex"] = json!(write_views_index(&store)?);
    Ok((report, true))
}

fn engine_class_metadata(report: &mut Value, store: &Store, documents: &render::class_uml::Bundle) {
    report["complete"] = json!(store.catalog.complete);
    report["warnings"] = json!(store.catalog.warnings);
    if let Some(warnings) = documents.legend["warnings"].as_array() {
        report["warnings"]
            .as_array_mut()
            .unwrap()
            .extend(warnings.iter().cloned());
    }
}

// Caller holds the catalog lock. All documents are validated before touching storage.
#[cfg(test)]
fn publish_class_view(
    store: &Store,
    options: &DiagramOptions,
    view_name: &str,
    documents: &render::class_uml::Bundle,
) -> Result<(Value, bool), String> {
    publish_class_view_checked(store, options, view_name, documents, None)
}

fn publish_class_view_checked(
    store: &Store,
    options: &DiagramOptions,
    view_name: &str,
    documents: &render::class_uml::Bundle,
    claims: Option<&crate::extensions::ValidatedClaimsRef>,
) -> Result<(Value, bool), String> {
    if !matches!(view_name, "class" | "class-focus") {
        return Err("Invalid class UML destination".into());
    }
    if documents.legend["schemaVersion"] != CLASS_POLICY_VERSION
        || documents.legend["snapshotId"] != store.catalog.snapshot_id
    {
        return Err("Class UML documents have an unsupported model policy or snapshot".into());
    }
    let legend = encoded(&documents.legend)?;
    let renderer = if options.mmdc.is_relative() && options.mmdc.components().count() > 1 {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(&options.mmdc)
    } else {
        options.mmdc.clone()
    };
    let mut receipt = ClassReceipt {
        schema_version: CLASS_VERSION.into(),
        manifest_hash: "0".repeat(64),
        snapshot_id: store.catalog.snapshot_id.clone(),
        policy_version: CLASS_POLICY_VERSION.into(),
        legend_hash: hash(&legend),
        html_hash: hash(documents.html.as_bytes()),
        diagrams: documents
            .diagrams
            .iter()
            .map(|diagram| ClassDiagramReceipt {
                id: diagram.id.clone(),
                title: diagram.title.clone(),
                mmd_sha: hash(diagram.mermaid.as_bytes()),
                svg_sha: (!options.no_render).then(|| "0".repeat(64)),
            })
            .collect(),
        artifacts: vec![
            "receipt.json".into(),
            "legend.json".into(),
            "index.html".into(),
        ],
        renderer: (!options.no_render).then_some(renderer.clone()),
    };
    for diagram in &receipt.diagrams {
        receipt.artifacts.push(format!("{}.mmd", diagram.id));
        if !options.no_render {
            receipt.artifacts.push(format!("{}.svg", diagram.id));
        }
    }
    class_artifacts(&receipt)?;
    let metadata_size = legend.len()
        + documents.html.len()
        + encoded(&receipt)?.len()
        + documents
            .diagrams
            .iter()
            .map(|d| d.mermaid.len())
            .sum::<usize>();
    if metadata_size > CLASS_METADATA_LIMIT {
        return Err(
            "Class UML metadata exceeds combined 8 MiB limit; no classes were truncated".into(),
        );
    }

    let base = analyze::create_storage(&store.directory.join("views"))?;
    let destination = base.join(view_name);
    let previous = workflow::tree_snapshot(&destination)?;
    let saved = if previous.is_some() {
        Some(validate_view(&destination)?)
    } else {
        None
    };
    let reuse = matches!(&saved, Some(ValidatedView::V2(saved))
        if saved.snapshot_id == receipt.snapshot_id
        && saved.policy_version == receipt.policy_version
        && saved.legend_hash == receipt.legend_hash
        && saved.html_hash == receipt.html_hash
        && saved.diagrams.len() == receipt.diagrams.len()
        && saved.diagrams.iter().zip(&receipt.diagrams).all(|(a,b)|
            a.id == b.id && a.title == b.title && a.mmd_sha == b.mmd_sha)
        && (options.no_render || saved.renderer.as_ref() == Some(&renderer)
            && saved.diagrams.iter().all(|d| d.svg_sha.is_some())));
    if !reuse {
        let stage = tempfile::Builder::new()
            .prefix(".class-uml-init-")
            .tempdir_in(&base)
            .map_err(|e| e.to_string())?;
        write_new(&stage.path().join("legend.json"), &legend)?;
        write_new(&stage.path().join("index.html"), documents.html.as_bytes())?;
        let started = Instant::now();
        let budget = Duration::from_secs(180);
        let mut svg_size = 0;
        for (diagram, saved) in documents.diagrams.iter().zip(&mut receipt.diagrams) {
            write_new(
                &stage.path().join(format!("{}.mmd", diagram.id)),
                diagram.mermaid.as_bytes(),
            )?;
            if !options.no_render {
                let remaining = budget
                    .checked_sub(started.elapsed())
                    .ok_or("Class UML aggregate render budget exhausted")?;
                let svg =
                    render::draft::svg_with_timeout(&diagram.mermaid, &options.mmdc, remaining)?;
                if started.elapsed() > budget {
                    return Err("Class UML aggregate render budget exhausted".into());
                }
                svg_size += svg.len();
                if svg.len() > 8 * 1024 * 1024 || svg_size > CLASS_SVG_LIMIT {
                    return Err("Class UML SVGs exceed aggregate 64 MiB limit".into());
                }
                saved.svg_sha = Some(hash(&svg));
                write_new(&stage.path().join(format!("{}.svg", diagram.id)), &svg)?;
            }
        }
        receipt.manifest_hash = class_manifest_hash(&receipt)?;
        write_new(&stage.path().join("receipt.json"), &encoded(&receipt)?)?;
        // Verify the exact staged tree before the atomic exchange, including every SVG.
        validate_view(stage.path())?;
        if let Some(claims) = claims {
            crate::extensions::check_store(claims, &load_checked(&store.directory, false)?)?;
        }
        let staged = stage.path().to_path_buf();
        workflow::publish_current(stage, &staged, &destination, previous.as_ref())?;
    } else if workflow::tree_snapshot(&destination)? != previous {
        return Err("Class UML storage changed during validation".into());
    }
    if let Some(claims) = claims {
        crate::extensions::check_store(claims, &load_locked(&store.directory)?)?;
    }
    let destination = directory(&destination)?;
    let ValidatedView::V2(saved) = validate_view(&destination)? else {
        return Err("Expected class UML v2 bundle".into());
    };
    let diagrams: Vec<_> = saved.diagrams.iter().map(|diagram| json!({
        "id": diagram.id, "title": diagram.title,
        "mermaid": destination.join(format!("{}.mmd", diagram.id)),
        "svg": diagram.svg_sha.as_ref().map(|_| destination.join(format!("{}.svg", diagram.id))),
    })).collect();
    Ok((
        json!({
            "catalogDirectory": store.directory, "viewDirectory": destination,
            "snapshotId": store.catalog.snapshot_id, "view": view_name,
            "diagrams": diagrams, "mermaid": Value::Null, "svg": Value::Null,
            "legend": destination.join("legend.json"), "viewer": destination.join("index.html"),
            "error": Value::Null,
        }),
        reuse,
    ))
}

// Caller holds the catalog lock. Each current-only bundle is published atomically.
fn publish_view(
    store: &Store,
    options: &DiagramOptions,
    view_name: &str,
    documents: &render::source_view::Documents,
) -> Result<(Value, bool), String> {
    let legend = encoded(&documents.legend)?;
    let renderer = if options.mmdc.is_relative() && options.mmdc.components().count() > 1 {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(&options.mmdc)
    } else {
        options.mmdc.clone()
    };
    let base = analyze::create_storage(&store.directory.join("views"))?;
    let destination = base.join(view_name);
    let previous = workflow::tree_snapshot(&destination)?;
    let mut reuse = false;
    if fs::symlink_metadata(&destination).is_ok() {
        let saved = validate_v1_view(&destination)?;
        reuse = saved.snapshot_id == store.catalog.snapshot_id
            && saved.mermaid_sha256 == hash(documents.mermaid.as_bytes())
            && saved.legend_sha256 == hash(&legend)
            && saved.html_sha256 == hash(documents.html.as_bytes())
            && (options.no_render
                || saved.svg_sha256.is_some() && saved.renderer.as_ref() == Some(&renderer));
    }
    if !reuse {
        let stage = tempfile::Builder::new()
            .prefix(".source-view-init-")
            .tempdir_in(&base)
            .map_err(|e| e.to_string())?;
        write_new(
            &stage.path().join("diagram.mmd"),
            documents.mermaid.as_bytes(),
        )?;
        write_new(&stage.path().join("legend.json"), &legend)?;
        write_new(&stage.path().join("index.html"), documents.html.as_bytes())?;
        let svg = if options.no_render {
            None
        } else {
            Some(render::draft::svg(&documents.mermaid, &options.mmdc)?)
        };
        if let Some(svg) = &svg {
            write_new(&stage.path().join("diagram.svg"), svg)?;
        }
        let receipt = ViewReceipt {
            schema_version: VIEW_VERSION.into(),
            snapshot_id: store.catalog.snapshot_id.clone(),
            mermaid_sha256: hash(documents.mermaid.as_bytes()),
            legend_sha256: hash(&legend),
            html_sha256: hash(documents.html.as_bytes()),
            svg_sha256: svg.as_deref().map(hash),
            renderer: if options.no_render {
                None
            } else {
                Some(renderer)
            },
        };
        write_new(&stage.path().join("receipt.json"), &encoded(&receipt)?)?;
        let staged = stage.path().to_path_buf();
        workflow::publish_current(stage, &staged, &destination, previous.as_ref())?;
    }
    let destination = directory(&destination)?;
    let receipt: ViewReceipt = serde_json::from_slice(&read(&destination.join("receipt.json"))?)
        .map_err(|_| "Invalid source-view receipt")?;
    if receipt.schema_version != VIEW_VERSION
        || receipt.snapshot_id != store.catalog.snapshot_id
        || receipt.mermaid_sha256 != hash(documents.mermaid.as_bytes())
        || receipt.legend_sha256 != hash(&legend)
        || receipt.html_sha256 != hash(documents.html.as_bytes())
        || (!options.no_render && receipt.svg_sha256.is_none())
    {
        return Err("Source-view receipt does not match this page".into());
    }
    verify(&destination.join("diagram.mmd"), &receipt.mermaid_sha256)?;
    verify(&destination.join("legend.json"), &receipt.legend_sha256)?;
    verify(&destination.join("index.html"), &receipt.html_sha256)?;
    if let Some(hash) = &receipt.svg_sha256 {
        verify(&destination.join("diagram.svg"), hash)?;
    } else if fs::symlink_metadata(destination.join("diagram.svg")).is_ok() {
        return Err(
            "Unrecorded source-view SVG exists; it was not displayed or overwritten".into(),
        );
    }
    Ok((
        json!({"catalogDirectory":store.directory,"viewDirectory":destination,"snapshotId":store.catalog.snapshot_id,"view":view_name,
        "mermaid":destination.join("diagram.mmd"),"legend":destination.join("legend.json"),"viewer":destination.join("index.html"),
        "svg":if options.no_render{None}else{Some(destination.join("diagram.svg"))},"error":Value::Null}),
        reuse,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saved_catalog(root: &Path, destination: &Path) -> Catalog {
        use crate::analysis::Fact;
        let fact = |kind: &str, symbol: &str, line, end_line, value: &str| Fact {
            id: format!("{kind}{line}"),
            kind: kind.into(),
            symbol: symbol.into(),
            path: "app.py".into(),
            line,
            end_line,
            value: value.into(),
        };
        let evidence = Evidence {
            schema_version: "ideomancer.evidence.v1".into(),
            snapshot_id: "saved".into(),
            extractor_version: "test".into(),
            python_version: "test".into(),
            language: None,
            files: vec![],
            diagnostics: vec![],
            complete: true,
            facts: vec![
                fact("module", "app", 1, 100, "app"),
                fact("function", "app.run", 2, 5, "run"),
                fact(
                    "entrypoint",
                    "app.command",
                    7,
                    7,
                    r#"{"role":"cli_command"}"#,
                ),
                fact("function", "app.command", 8, 10, "command"),
                fact("entrypoint", "app.job", 12, 12, r#"{"role":"job"}"#),
                fact("async_function", "app.job", 13, 15, "job"),
                fact("class", "app.Worker", 20, 30, "Worker"),
                fact("function", "app.Worker.run", 21, 25, "run"),
                fact(
                    "api_route",
                    "app.endpoint",
                    40,
                    40,
                    r#"{"methods":["GET"],"path":"/x","pathIsFragment":true}"#,
                ),
                fact("function", "app.endpoint", 41, 45, "endpoint"),
            ],
        };
        let catalog = catalog::build(&evidence, root.to_str().unwrap()).unwrap();
        let catalog_bytes = encoded(&catalog).unwrap();
        let evidence_bytes = encoded(&evidence).unwrap();
        write_new(&destination.join("catalog.json"), &catalog_bytes).unwrap();
        write_new(&destination.join("evidence.json"), &evidence_bytes).unwrap();
        write_new(
            &destination.join("receipt.json"),
            &encoded(&Receipt {
                schema_version: catalog::VERSION.into(),
                catalog_sha256: hash(&catalog_bytes),
                evidence_sha256: hash(&evidence_bytes),
            })
            .unwrap(),
        )
        .unwrap();
        catalog
    }

    fn options(catalog: &Path, entry: Option<&str>) -> DiagramOptions {
        DiagramOptions {
            catalog: catalog.into(),
            kind: None,
            entry: entry.map(str::to_owned),
            view: Some(render::endpoint::View::Services),
            openapi: None,
            query: None,
            path: None,
            limit: 12,
            offset: 0,
            pagination_explicit: false,
            no_render: true,
            extract_only: false,
            mmdc: "/missing/native-renderer".into(),
            agent: "/missing/native-agent".into(),
            model: Some("must-not-run".into()),
            python: "/missing/native-python".into(),
            focus_hops: 1,
            focus_facts: 100,
            timeout_seconds: 1,
            retry: true,
        }
    }

    fn class_options(catalog: &Path) -> DiagramOptions {
        let mut opts = options(catalog, None);
        opts.kind = Some(Kind::Class);
        opts.view = None;
        opts
    }

    #[cfg(unix)]
    #[test]
    fn invalid_class_focus_never_publishes_artifacts_or_invokes_renderer() {
        use std::os::unix::fs::PermissionsExt;

        let tools = tempfile::tempdir().unwrap();
        let renderer = tools.path().join("mmdc");
        fs::write(
            &renderer,
            "#!/bin/sh\nprintf invoked > \"$0.called\"\nexit 1\n",
        )
        .unwrap();
        fs::set_permissions(&renderer, fs::Permissions::from_mode(0o700)).unwrap();
        for state in ["fresh", "overview", "focus"] {
            let root = tempfile::tempdir().unwrap();
            let destination = tempfile::tempdir().unwrap();
            saved_catalog(root.path(), destination.path());
            if state != "fresh" {
                let mut opts = class_options(destination.path());
                opts.pagination_explicit = state == "focus";
                diagram(opts).unwrap();
            }
            let before = workflow::tree_snapshot(destination.path()).unwrap();
            let modified = || {
                before
                    .as_ref()
                    .unwrap()
                    .keys()
                    .map(|path| {
                        fs::metadata(destination.path().join(path))
                            .unwrap()
                            .modified()
                            .unwrap()
                    })
                    .collect::<Vec<_>>()
            };
            let mtimes = modified();
            for no_render in [true, false] {
                for selection in ["invalid-path", "query", "path", "offset", "entry"] {
                    let mut opts = class_options(destination.path());
                    opts.no_render = no_render;
                    opts.mmdc = renderer.clone();
                    let expected = match selection {
                        "invalid-path" => {
                            opts.path = Some("../outside".into());
                            "root-relative prefix"
                        }
                        "entry" => {
                            opts.entry = Some("unknown-entry".into());
                            "no catalog candidate"
                        }
                        _ => {
                            match selection {
                                "query" => opts.query = Some("no-match".into()),
                                "path" => opts.path = Some("no-match".into()),
                                _ => {
                                    opts.offset = usize::MAX;
                                    opts.pagination_explicit = true;
                                }
                            }
                            "1..=30"
                        }
                    };
                    let error = diagram(opts).unwrap_err();
                    assert!(error.contains(expected), "{state}/{selection}: {error}");
                    assert!(!tools.path().join("mmdc.called").exists());
                    assert_eq!(workflow::tree_snapshot(destination.path()).unwrap(), before);
                    assert_eq!(modified(), mtimes);
                }
            }
        }
    }

    #[test]
    fn class_structure_and_focus_are_separate_saved_only_current_bundles() {
        let root = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        let catalog = saved_catalog(root.path(), destination.path());
        root.close().unwrap();
        let (overview, success) = diagram(class_options(destination.path())).unwrap();
        assert!(success);
        assert_eq!(overview["view"], "class");
        assert_eq!(overview["selection"], "all-catalog-classes");
        assert!(overview["page"].is_null());
        assert!(!destination.path().join("views/class-overview").exists());
        assert!(!destination.path().join("views/class-focus").exists());
        let overview_path = destination.path().join("views/class");
        let before = workflow::tree_snapshot(&overview_path).unwrap();
        let modified = fs::metadata(overview_path.join("receipt.json"))
            .unwrap()
            .modified()
            .unwrap();
        for selection in ["entry", "query", "path", "limit", "offset"] {
            let mut opts = class_options(destination.path());
            match selection {
                "entry" => {
                    opts.entry = Some(
                        catalog
                            .items
                            .iter()
                            .find(|i| i.kind == Kind::Class)
                            .unwrap()
                            .id
                            .clone(),
                    );
                }
                "query" => opts.query = Some("Worker".into()),
                "path" => opts.path = Some("app.py".into()),
                _ => opts.pagination_explicit = true,
            }
            let (focus, success) = diagram(opts).unwrap();
            assert!(success);
            assert_eq!(focus["view"], "class-focus");
            assert!(focus.get("overviewViewer").is_none());
            assert!(!focus["page"].is_null());
            assert_eq!(workflow::tree_snapshot(&overview_path).unwrap(), before);
            assert_eq!(
                fs::metadata(overview_path.join("receipt.json"))
                    .unwrap()
                    .modified()
                    .unwrap(),
                modified
            );
        }
        let focus_before =
            workflow::tree_snapshot(&destination.path().join("views/class-focus")).unwrap();
        let (repeat, _) = diagram(class_options(destination.path())).unwrap();
        assert_eq!(repeat["reused"], true);
        assert_eq!(
            workflow::tree_snapshot(&destination.path().join("views/class-focus")).unwrap(),
            focus_before
        );
        let index = fs::read_to_string(destination.path().join("views/index.html")).unwrap();
        assert!(index.find("Class structure").unwrap() < index.find("Class focus").unwrap());
        assert!(!index.contains("/diagram.mmd"));
        for diagram in overview["diagrams"].as_array().unwrap() {
            assert!(index.contains(&format!("class/{}.mmd", diagram["id"].as_str().unwrap())));
        }
    }

    #[test]
    fn class_focus_does_not_add_structure_and_failed_render_preserves_current() {
        let root = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        saved_catalog(root.path(), destination.path());
        let mut opts = class_options(destination.path());
        opts.pagination_explicit = true;
        diagram(opts).unwrap();
        let overview = destination.path().join("views/class");
        assert!(!overview.exists());
        assert!(load(destination.path()).is_ok());
        let mut opts = class_options(destination.path());
        opts.query = Some("Worker".into());
        diagram(opts).unwrap();
        let focus = destination.path().join("views/class-focus");
        let focus_before = workflow::tree_snapshot(&focus).unwrap();
        let mut opts = class_options(destination.path());
        opts.query = Some("Worker".into());
        opts.no_render = false;
        assert!(diagram(opts).is_err());
        assert_eq!(workflow::tree_snapshot(&focus).unwrap(), focus_before);
        assert!(!overview.exists());
        diagram(class_options(destination.path())).unwrap();
        let before = workflow::tree_snapshot(&overview).unwrap();
        let (report, _) = diagram(class_options(destination.path())).unwrap();
        assert_eq!(report["reused"], true);
        assert_eq!(workflow::tree_snapshot(&overview).unwrap(), before);
        // Unknown notes and corrupt artifacts must never be replaced, including on focus commands.
        fs::write(overview.join("notes.txt"), "manual notes").unwrap();
        let mut opts = class_options(destination.path());
        opts.pagination_explicit = true;
        assert!(diagram(opts).is_err());
        assert_eq!(workflow::tree_snapshot(&focus).unwrap(), focus_before);
        fs::remove_file(overview.join("notes.txt")).unwrap();
        let saved: ClassReceipt =
            serde_json::from_slice(&read(&overview.join("receipt.json")).unwrap()).unwrap();
        fs::write(
            overview.join(format!("{}.mmd", saved.diagrams[0].id)),
            "corrupted",
        )
        .unwrap();
        let error = diagram(class_options(destination.path())).unwrap_err();
        assert!(error.contains("integrity"));
        assert_eq!(workflow::tree_snapshot(&focus).unwrap(), focus_before);
    }

    #[test]
    fn empty_class_catalog_has_an_overview_without_a_page() {
        let root = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        saved_catalog(root.path(), destination.path());
        let mut evidence = load(destination.path()).unwrap().evidence;
        evidence.facts.clear();
        let output = tempfile::tempdir().unwrap();
        let store = discover_with(root.path(), output.path(), || Ok(evidence)).unwrap();
        root.close().unwrap();
        let (report, success) = diagram(class_options(&store.directory)).unwrap();
        assert!(success);
        assert_eq!(report["selection"], "all-catalog-classes");
        assert!(report["page"].is_null());
        assert!(load(&store.directory).is_ok());
    }

    #[test]
    fn class_v2_receipt_rejects_tamper_unknown_files_and_unsafe_references() {
        let root = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        saved_catalog(root.path(), destination.path());
        let (report, _) = diagram(class_options(destination.path())).unwrap();
        let view = Path::new(report["viewDirectory"].as_str().unwrap());
        let receipt_path = view.join("receipt.json");
        let original = read(&receipt_path).unwrap();
        let parsed: ClassReceipt = serde_json::from_slice(&original).unwrap();
        assert_eq!(parsed.schema_version, CLASS_VERSION);
        assert!(parsed.artifacts.contains(&"receipt.json".to_owned()));
        for field in ["renderer", "svgSha"] {
            let mut value: Value = serde_json::from_slice(&original).unwrap();
            if field == "renderer" {
                value.as_object_mut().unwrap().remove(field);
            } else {
                value["diagrams"][0].as_object_mut().unwrap().remove(field);
            }
            fs::write(&receipt_path, encoded(&value).unwrap()).unwrap();
            assert!(
                validate_view(view)
                    .err()
                    .unwrap()
                    .contains("manifest integrity"),
                "{field}"
            );
        }
        fs::write(&receipt_path, &original).unwrap();
        for name in ["group-9999.svg", "notes.txt", ".hidden"] {
            fs::write(view.join(name), "unrecorded").unwrap();
            assert!(validate_view(view).is_err(), "{name}");
            let before = workflow::tree_snapshot(view).unwrap();
            assert!(diagram(class_options(destination.path())).is_err());
            assert_eq!(workflow::tree_snapshot(view).unwrap(), before);
            fs::remove_file(view.join(name)).unwrap();
        }
        fs::create_dir(view.join("hidden-directory")).unwrap();
        assert!(validate_view(view).is_err());
        fs::remove_dir(view.join("hidden-directory")).unwrap();
        for mutation in [
            "unknown",
            "escape",
            "duplicate",
            "missing-receipt",
            "version",
            "policy",
            "count",
            "svg",
            "hash",
        ] {
            let mut value: Value = serde_json::from_slice(&original).unwrap();
            match mutation {
                "unknown" => value["injected"] = json!(true),
                "escape" => value["diagrams"][0]["id"] = json!("../escape"),
                "duplicate" => {
                    let d = value["diagrams"][0].clone();
                    value["diagrams"].as_array_mut().unwrap().push(d);
                }
                "missing-receipt" => value["artifacts"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|v| v != "receipt.json"),
                "version" => value["schemaVersion"] = json!("source-views.v3"),
                "policy" => value["policyVersion"] = json!("future-policy"),
                "count" => {
                    value["diagrams"] =
                        json!(vec![value["diagrams"][0].clone(); CLASS_DIAGRAM_LIMIT + 1])
                }
                "svg" => {
                    value["diagrams"][0]["svgSha"] = json!(hash(b"svg"));
                    value["renderer"] = json!("renderer");
                }
                _ => value["diagrams"][0]["mmdSha"] = json!(hash(b"tampered")),
            }
            // Keep exercising structural/file validation behind the manifest check.
            if let Ok(mut receipt) = serde_json::from_value::<ClassReceipt>(value.clone()) {
                receipt.manifest_hash = class_manifest_hash(&receipt).unwrap();
                value = serde_json::to_value(receipt).unwrap();
            }
            fs::write(&receipt_path, encoded(&value).unwrap()).unwrap();
            assert!(validate_view(view).is_err(), "{mutation}");
        }
        fs::write(&receipt_path, &original).unwrap();
        let duplicated = String::from_utf8(original.clone()).unwrap().replacen(
            "\"snapshotId\":",
            "\"snapshotId\": \"injected\", \"snapshotId\":",
            1,
        );
        fs::write(&receipt_path, duplicated).unwrap();
        assert!(validate_view(view).is_err());
        fs::write(&receipt_path, &original).unwrap();
        #[cfg(unix)]
        {
            let alias = destination.path().join("alias");
            std::os::unix::fs::symlink(view, &alias).unwrap();
            assert!(validate_view(&alias).is_err());
            fs::remove_file(alias).unwrap();
            let artifact = view.join(format!("{}.mmd", parsed.diagrams[0].id));
            let bytes = read(&artifact).unwrap();
            fs::remove_file(&artifact).unwrap();
            std::os::unix::fs::symlink(root.path().join("outside"), &artifact).unwrap();
            assert!(validate_view(view).is_err());
            fs::remove_file(&artifact).unwrap();
            fs::write(artifact, bytes).unwrap();
        }
        assert!(validate_view(view).is_ok());
    }

    #[test]
    fn legacy_v1_class_upgrades_but_inventory_is_preserved_and_not_advertised() {
        let root = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        saved_catalog(root.path(), destination.path());
        let store = load(destination.path()).unwrap();
        let opts = class_options(destination.path());
        let page = catalog::page(
            &store.catalog,
            &Filter {
                kind: Some(Kind::Class),
                query: None,
                path: None,
                limit: 12,
                offset: 0,
            },
        )
        .unwrap();
        let documents =
            render::source_view::build(&store.catalog, &store.evidence, &page, &Kind::Class)
                .unwrap();
        publish_view(&store, &opts, "class", &documents).unwrap();
        let inventory = render::class_overview::build(&store.catalog, &store.evidence).unwrap();
        publish_view(&store, &opts, "class-overview", &inventory).unwrap();
        let legacy =
            workflow::tree_snapshot(&store.directory.join("views/class-overview")).unwrap();
        let (report, _) = diagram(opts).unwrap();
        assert_eq!(report["view"], "class");
        assert!(matches!(
            validate_view(&store.directory.join("views/class")).unwrap(),
            ValidatedView::V2(_)
        ));
        assert!(!store.directory.join("views/class/diagram.mmd").exists());
        assert_eq!(
            workflow::tree_snapshot(&store.directory.join("views/class-overview")).unwrap(),
            legacy
        );
        let index = fs::read_to_string(store.directory.join("views/index.html")).unwrap();
        assert!(!index.contains("class-overview"));
    }

    #[cfg(unix)]
    #[test]
    fn class_bundle_stages_all_groups_and_reuses_saved_svgs_without_mtime_changes() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        let tools = tempfile::tempdir().unwrap();
        saved_catalog(root.path(), destination.path());
        let store = load(destination.path()).unwrap();
        let mut documents =
            render::class_uml::build(&store.catalog, &store.evidence, None).unwrap();
        documents.diagrams.push(render::class_uml::Diagram {
            id: "group-0002".into(),
            title: "Second group".into(),
            mermaid: documents.diagrams[0].mermaid.clone(),
        });
        let mut opts = class_options(destination.path());
        publish_class_view(&store, &opts, "class", &documents).unwrap();
        let view = store.directory.join("views/class");
        let before = workflow::tree_snapshot(&view).unwrap();
        let renderer = tools.path().join("mmdc");
        fs::write(&renderer, "#!/bin/sh\nset -eu\nmarker=\"$(dirname \"$0\")/called\"\nif test -f \"$marker\"; then exit 7; fi\ntouch \"$marker\"\nprintf '<svg width=\"10\" height=\"10\"></svg>' > \"$4\"\n").unwrap();
        fs::set_permissions(&renderer, fs::Permissions::from_mode(0o700)).unwrap();
        opts.no_render = false;
        opts.mmdc = renderer.clone();
        assert!(publish_class_view(&store, &opts, "class", &documents).is_err());
        assert_eq!(workflow::tree_snapshot(&view).unwrap(), before);
        assert_eq!(
            fs::read_dir(store.directory.join("views")).unwrap().count(),
            1
        );
        fs::write(
            &renderer,
            "#!/bin/sh\ntouch \"$0.called\"\nprintf '<svg width=\"10\" height=\"10\"></svg>' > \"$4\"\n",
        )
        .unwrap();
        let (report, reused) = publish_class_view(&store, &opts, "class", &documents).unwrap();
        assert!(!reused);
        assert_eq!(report["diagrams"].as_array().unwrap().len(), 2);
        let before = workflow::tree_snapshot(&view).unwrap();
        let mtimes: Vec<_> = fs::read_dir(&view)
            .unwrap()
            .map(|e| {
                let path = e.unwrap().path();
                (
                    path.clone(),
                    fs::metadata(path).unwrap().modified().unwrap(),
                )
            })
            .collect();
        for no_render in [false, true] {
            opts.no_render = no_render;
            let (repeat, reused) = publish_class_view(&store, &opts, "class", &documents).unwrap();
            assert!(reused);
            assert_eq!(repeat, report);
            for diagram in repeat["diagrams"].as_array().unwrap() {
                assert!(Path::new(diagram["svg"].as_str().unwrap()).is_file());
            }
            assert_eq!(workflow::tree_snapshot(&view).unwrap(), before);
            for (path, mtime) in &mtimes {
                assert_eq!(fs::metadata(path).unwrap().modified().unwrap(), *mtime);
            }
        }
        write_views_index(&store).unwrap();
        let called = renderer.with_extension("called");
        fs::remove_file(&called).unwrap();
        let receipt_path = view.join("receipt.json");
        let original = read(&receipt_path).unwrap();
        for mutation in [
            "title",
            "order",
            "renderer",
            "artifacts",
            "artifact-order",
            "missing-hash",
        ] {
            let mut value: Value = serde_json::from_slice(&original).unwrap();
            match mutation {
                "title" => value["diagrams"][0]["title"] = json!("Changed title"),
                "order" => value["diagrams"].as_array_mut().unwrap().reverse(),
                "renderer" => value["renderer"] = json!("different-renderer"),
                "artifacts" => {
                    value["artifacts"].as_array_mut().unwrap().pop();
                }
                "artifact-order" => value["artifacts"].as_array_mut().unwrap().reverse(),
                _ => {
                    value.as_object_mut().unwrap().remove("manifestHash");
                }
            }
            fs::write(&receipt_path, encoded(&value).unwrap()).unwrap();
            let before = workflow::tree_snapshot(&store.directory).unwrap();
            let mtimes: Vec<_> = before
                .as_ref()
                .unwrap()
                .keys()
                .map(|path| {
                    let path = store.directory.join(path);
                    let mtime = fs::metadata(&path).unwrap().modified().unwrap();
                    (path, mtime)
                })
                .collect();
            let error = validate_view(&view).err().unwrap();
            assert!(
                error.contains(if mutation == "missing-hash" {
                    "manifestHash"
                } else {
                    "manifest integrity"
                }),
                "{mutation}: {error}"
            );
            assert!(load(&store.directory).is_err(), "{mutation}");
            assert!(write_views_index(&store).is_err(), "{mutation}");
            for no_render in [false, true] {
                opts.no_render = no_render;
                assert!(
                    publish_class_view(&store, &opts, "class", &documents).is_err(),
                    "{mutation}"
                );
                let mut command = class_options(&store.directory);
                command.no_render = no_render;
                command.mmdc = renderer.clone();
                assert!(diagram(command).is_err(), "{mutation}");
            }
            assert!(!called.exists(), "{mutation} invoked renderer");
            assert_eq!(
                workflow::tree_snapshot(&store.directory).unwrap(),
                before,
                "{mutation}"
            );
            for (path, mtime) in mtimes {
                assert_eq!(
                    fs::metadata(path).unwrap().modified().unwrap(),
                    mtime,
                    "{mutation}"
                );
            }
        }
        fs::write(&receipt_path, original).unwrap();
        let svg = view.join("group-0002.svg");
        fs::write(svg, "tampered SVG").unwrap();
        assert!(validate_view(&view).err().unwrap().contains("integrity"));
        let tampered = workflow::tree_snapshot(&view).unwrap();
        assert!(publish_class_view(&store, &opts, "class", &documents).is_err());
        assert_eq!(workflow::tree_snapshot(&view).unwrap(), tampered);
    }

    #[test]
    fn class_bundle_count_and_metadata_bounds_reject_before_storage_creation() {
        let root = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        saved_catalog(root.path(), destination.path());
        let store = load(destination.path()).unwrap();
        let opts = class_options(destination.path());
        let mut documents =
            render::class_uml::build(&store.catalog, &store.evidence, None).unwrap();
        for i in 2..=CLASS_DIAGRAM_LIMIT + 1 {
            documents.diagrams.push(render::class_uml::Diagram {
                id: format!("group-{i:04}"),
                title: "group".into(),
                mermaid: "classDiagram\n".into(),
            });
        }
        assert!(publish_class_view(&store, &opts, "class", &documents).is_err());
        assert!(!store.directory.join("views").exists());
        documents.diagrams.truncate(1);
        documents.html = "x".repeat(CLASS_METADATA_LIMIT);
        assert!(publish_class_view(&store, &opts, "class", &documents).is_err());
        assert!(!store.directory.join("views").exists());
    }

    #[test]
    fn callable_dispatch_uses_saved_evidence_without_contract_probes_or_tools() {
        let root = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        let catalog = saved_catalog(root.path(), destination.path());
        // Either malformed file alone, or multiple conventional names, would fail API loading.
        fs::write(root.path().join("openapi.json"), "not JSON").unwrap();
        for item in catalog.items.iter().filter(|i| i.kind == Kind::Workflow) {
            for view in [
                render::endpoint::View::Services,
                render::endpoint::View::Network,
            ] {
                let mut opts = options(destination.path(), Some(&item.id));
                opts.view = Some(view.clone());
                let (report, success) = diagram(opts).unwrap();
                assert!(success);
                assert_eq!(report["kind"], "workflow");
                let legend: Value =
                    serde_json::from_slice(&fs::read(report["legend"].as_str().unwrap()).unwrap())
                        .unwrap();
                assert_eq!(legend["item"]["id"], item.id);
                assert_eq!(legend["contractMatch"]["status"], "not-applicable");
                let receipt =
                    Path::new(report["viewDirectory"].as_str().unwrap()).join("receipt.json");
                let modified = fs::metadata(&receipt).unwrap().modified().unwrap();
                let before = fs::read(&receipt).unwrap();
                let mut repeat = options(destination.path(), Some(&item.id));
                repeat.view = Some(view);
                repeat.kind = Some(Kind::Workflow);
                assert_eq!(diagram(repeat).unwrap().0, report);
                assert_eq!(fs::read(&receipt).unwrap(), before);
                assert_eq!(
                    fs::metadata(&receipt).unwrap().modified().unwrap(),
                    modified
                );
            }
            fs::write(root.path().join("swagger.yaml"), "also malformed: [").unwrap();
        }
        let api = catalog.items.iter().find(|i| i.kind == Kind::Api).unwrap();
        assert!(diagram(options(destination.path(), Some(&api.id)))
            .err()
            .unwrap()
            .contains("Multiple local API contracts"));
        fs::remove_file(root.path().join("swagger.yaml")).unwrap();
        assert!(diagram(options(destination.path(), Some(&api.id)))
            .err()
            .unwrap()
            .contains("OpenAPI"));
        root.close().unwrap();
        let item = catalog
            .items
            .iter()
            .find(|i| i.kind == Kind::Workflow)
            .unwrap();
        assert!(diagram(options(destination.path(), Some(&item.id))).is_ok());
        assert!(!destination.path().join("analyses").exists());
    }

    #[test]
    fn native_invalid_options_reject_before_contract_reads_or_agent_dispatch() {
        let root = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        let catalog = saved_catalog(root.path(), destination.path());
        let workflow = catalog
            .items
            .iter()
            .find(|i| i.kind == Kind::Workflow)
            .unwrap();
        let api = catalog.items.iter().find(|i| i.kind == Kind::Api).unwrap();
        let class = catalog
            .items
            .iter()
            .find(|i| i.kind == Kind::Class)
            .unwrap();
        for (entry, kind) in [
            (Some(api.id.as_str()), Some(Kind::Workflow)),
            (Some(workflow.id.as_str()), Some(Kind::Api)),
            (Some(class.id.as_str()), None),
            (None, Some(Kind::Workflow)),
        ] {
            let mut opts = options(destination.path(), entry);
            opts.kind = kind;
            let error = diagram(opts).err().unwrap();
            assert!(!error.contains("No such file"), "{error}");
        }
        for (name, content) in [
            ("missing.json", None),
            ("valid.json", Some(r#"{"openapi":"3.1.0","paths":{}}"#)),
            ("malformed.yaml", Some("[")),
        ] {
            let path = root.path().join(name);
            if let Some(content) = content {
                fs::write(&path, content).unwrap();
            }
            let mut opts = options(destination.path(), Some(&workflow.id));
            opts.openapi = Some(path);
            assert!(diagram(opts)
                .err()
                .unwrap()
                .contains("--openapi requires one API"));
        }
        let mut opts = options(destination.path(), Some(&workflow.id));
        opts.extract_only = true;
        opts.catalog = "/missing/catalog".into();
        assert!(diagram(opts).err().unwrap().contains("--extract-only"));
        assert!(!destination.path().join("views").exists());
    }
}
