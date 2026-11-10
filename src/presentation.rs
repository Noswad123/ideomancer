//! Current draft presentation. Callers hold the workflow's external slot lock.
use crate::render::{
    draft,
    presentation::{self, Input},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const LIMIT: u64 = 8 * 1024 * 1024;
const SCHEMA: &str = "ideomancer.presentation-cache.v1";

pub struct Output {
    pub mermaid: PathBuf,
    pub legend: PathBuf,
    pub viewer: PathBuf,
    pub svg: Option<PathBuf>,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Receipt {
    schema_version: String,
    presentation_version: String,
    proposal_sha256: String,
    snapshot_id: String,
    manifest_sha256: String,
    mermaid_sha256: String,
    legend_sha256: String,
    html_sha256: String,
    svg_sha256: Option<String>,
    renderer: Option<PathBuf>,
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn json(value: &impl Serialize) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    Ok(bytes)
}
fn read(path: &Path) -> Result<Vec<u8>, String> {
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
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > LIMIT {
        return Err("Presentation artifact must be a bounded regular non-symlink file".into());
    }
    let mut bytes = Vec::new();
    file.take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > LIMIT {
        return Err("Presentation artifact exceeds 8 MiB".into());
    }
    Ok(bytes)
}
fn verify(path: &Path, expected: &str) -> Result<(), String> {
    if hash(&read(path)?) != expected {
        return Err(format!(
            "Presentation integrity failure: {}",
            path.display()
        ));
    }
    Ok(())
}
fn write(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() as u64 > LIMIT {
        return Err("Presentation artifact exceeds 8 MiB".into());
    }
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(dir.join(name))
        .map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())
}

pub(crate) fn validate(dir: &Path) -> Result<(), String> {
    crate::workflow::managed_files(
        dir,
        &[
            "receipt.json",
            "diagram.mmd",
            "legend.json",
            "index.html",
            "diagram.svg",
            "draft.architecture.json",
        ],
        &[],
    )?;
    let receipt: Receipt = serde_json::from_slice(&read(&dir.join("receipt.json"))?)
        .map_err(|_| "Invalid presentation receipt")?;
    if receipt.schema_version != SCHEMA {
        return Err("Unsupported presentation receipt".into());
    }
    for (name, expected) in [
        ("diagram.mmd", &receipt.mermaid_sha256),
        ("legend.json", &receipt.legend_sha256),
        ("index.html", &receipt.html_sha256),
        ("draft.architecture.json", &receipt.manifest_sha256),
    ] {
        verify(&dir.join(name), expected)?;
    }
    if let Some(expected) = &receipt.svg_sha256 {
        verify(&dir.join("diagram.svg"), expected)?;
    } else if fs::symlink_metadata(dir.join("diagram.svg")).is_ok() {
        return Err("Unrecorded presentation SVG exists".into());
    }
    Ok(())
}

pub fn save(
    run: &Path,
    input: &Input,
    manifest: &[u8],
    renderer: &Path,
    no_render: bool,
) -> Result<Output, String> {
    let documents = presentation::build(input)?;
    let legend = json(&documents.legend)?;
    let mut receipt = Receipt {
        schema_version: SCHEMA.into(),
        presentation_version: presentation::VERSION.into(),
        proposal_sha256: input.proposal_sha256.clone(),
        snapshot_id: input.snapshot_id.clone(),
        manifest_sha256: hash(manifest),
        mermaid_sha256: hash(documents.mermaid.as_bytes()),
        legend_sha256: hash(&legend),
        html_sha256: hash(documents.html.as_bytes()),
        svg_sha256: None,
        renderer: None,
    };
    let dir = run.join("presentation");
    let previous = crate::workflow::tree_snapshot(&dir)?;
    let mut reusable = false;
    if fs::symlink_metadata(&dir).is_ok() {
        validate(&dir)?;
        let saved: Receipt = serde_json::from_slice(&read(&dir.join("receipt.json"))?)
            .map_err(|_| "Invalid presentation receipt")?;
        if saved.proposal_sha256 != input.proposal_sha256 || saved.snapshot_id != input.snapshot_id
        {
            return Err("Presentation does not match saved proposal/source".into());
        }
        reusable = saved.presentation_version == receipt.presentation_version
            && saved.manifest_sha256 == receipt.manifest_sha256
            && saved.mermaid_sha256 == receipt.mermaid_sha256
            && saved.legend_sha256 == receipt.legend_sha256
            && saved.html_sha256 == receipt.html_sha256;
        if !reusable && saved.presentation_version == receipt.presentation_version {
            return Err("Presentation contents conflict with their saved proposal/source".into());
        }
        if reusable {
            receipt = saved;
        }
    }
    let mut output = Output {
        mermaid: dir.join("diagram.mmd"),
        legend: dir.join("legend.json"),
        viewer: dir.join("index.html"),
        svg: None,
        error: None,
    };
    if reusable && (no_render || receipt.svg_sha256.is_some()) {
        if !no_render {
            output.svg = Some(dir.join("diagram.svg"));
        }
        return Ok(output);
    }
    let svg = if no_render {
        None
    } else {
        match draft::svg(&documents.mermaid, renderer) {
            Ok(bytes) => Some(bytes),
            Err(error) => {
                output.error = Some(error);
                None
            }
        }
    };
    // An unsuccessful rendering leaves an already valid source-only bundle alone.
    if reusable && svg.is_none() {
        return Ok(output);
    }
    receipt.svg_sha256 = svg.as_deref().map(hash);
    receipt.renderer = if svg.is_some() {
        Some(
            if renderer.is_relative() && renderer.components().count() > 1 {
                std::env::current_dir()
                    .map_err(|e| e.to_string())?
                    .join(renderer)
            } else {
                renderer.into()
            },
        )
    } else {
        None
    };
    let stage = tempfile::Builder::new()
        .prefix(".presentation-init-")
        .tempdir_in(run)
        .map_err(|e| e.to_string())?;
    write(stage.path(), "diagram.mmd", documents.mermaid.as_bytes())?;
    write(stage.path(), "legend.json", &legend)?;
    write(stage.path(), "index.html", documents.html.as_bytes())?;
    write(stage.path(), "draft.architecture.json", manifest)?;
    if let Some(bytes) = &svg {
        write(stage.path(), "diagram.svg", bytes)?;
        output.svg = Some(dir.join("diagram.svg"));
    }
    write(stage.path(), "receipt.json", &json(&receipt)?)?;
    validate(stage.path())?;
    let staged = stage.path().to_path_buf();
    crate::workflow::publish_current(stage, &staged, &dir, previous.as_ref())?;
    Ok(output)
}
