use crate::{analysis, analyze, catalog, discovery, focus, plan, render, validate, workflow};

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process;

use crate::manifest::{Manifest, Node, SCHEMA_VERSION};
use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;

#[derive(Debug, Parser)]
#[command(name = "ideomancer")]
#[command(version)]
#[command(about = "Discover software and explore source-grounded diagrams in manageable views")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Discover source entry-point candidates locally, then show one page (no agent).
    Discover {
        root: PathBuf,
        /// Explicit source language; no target configuration or manifest is executed.
        #[arg(long, alias = "language", value_enum, default_value = "python")]
        lang: analysis::Language,
        #[arg(long, default_value = ".ideomancer/catalogs")]
        out: PathBuf,
        #[arg(long, env = "IDEOMANCER_PYTHON", default_value = "python3.12")]
        python: PathBuf,
        /// Retired: native JS/TS discovery needs no Node executable.
        #[arg(long, env = "IDEOMANCER_NODE", hide = true)]
        node: Option<PathBuf>,
        /// Retired: JS/TS grammars are compiled into the engine.
        #[arg(long, env = "IDEOMANCER_TYPESCRIPT", hide = true)]
        typescript: Option<PathBuf>,
        #[command(flatten)]
        filter: CatalogFilter,
    },
    /// Browse a saved discovery snapshot without rescanning the application.
    Catalog {
        catalog: PathBuf,
        #[command(flatten)]
        filter: CatalogFilter,
    },
    /// Draw grouped class UML, a local source/callable view, or one workflow through an agent.
    Diagram {
        catalog: PathBuf,
        #[arg(long, value_enum, required_unless_present = "entry")]
        kind: Option<catalog::Kind>,
        /// Stable catalog ID, exact symbol or unique symbol suffix.
        #[arg(long)]
        entry: Option<String>,
        /// Local network/service candidates; requires one API or callable workflow entry, never an agent.
        #[arg(long, value_enum, requires = "entry")]
        view: Option<render::endpoint::View>,
        /// Local OpenAPI/Swagger JSON or YAML export. No URLs or application startup.
        #[arg(long, requires = "view")]
        openapi: Option<PathBuf>,
        #[arg(long = "js-profile", hide = true)]
        removed_profile: Option<String>,
        #[arg(long)]
        query: Option<String>,
        #[arg(long)]
        path: Option<String>,
        /// Select a bounded focus page; omitted for complete class structure.
        #[arg(long, value_parser = clap::value_parser!(u32).range(1..=30))]
        limit: Option<u32>,
        #[arg(long)]
        offset: Option<usize>,
        #[arg(long)]
        no_render: bool,
        /// Agent workflow path only (without --view): prepare evidence without an agent call.
        #[arg(long)]
        extract_only: bool,
        #[arg(long, env = "IDEOMANCER_MMDC", default_value = "mmdc")]
        mmdc: PathBuf,
        #[arg(long, env = "IDEOMANCER_AGENT", default_value = "djinn")]
        agent: PathBuf,
        #[arg(long, env = "IDEOMANCER_MODEL", default_value = "openai/gpt-6.1-sol")]
        model: Option<String>,
        #[arg(long, env = "IDEOMANCER_PYTHON", default_value = "python3.12")]
        python: PathBuf,
        #[arg(long, default_value_t = 3, value_parser = clap::value_parser!(u8).range(0..=5))]
        focus_hops: u8,
        #[arg(long, default_value_t = 600, value_parser = clap::value_parser!(u32).range(1..=2000))]
        focus_facts: u32,
        #[arg(long, default_value_t = 300, value_parser = clap::value_parser!(u64).range(1..=600))]
        timeout_seconds: u64,
        #[arg(long)]
        retry: bool,
    },
    /// Review a manually authored software plan. Never runs agents or implementation work.
    #[command(hide = true)]
    Plan {
        #[command(subcommand)]
        command: PlanCommand,
    },
    /// Analyze a Python app and return a draft diagram. Sends evidence to the selected agent unless --extract-only.
    Analyze {
        root: PathBuf,
        /// Output container; each application's current output replaces its previous run.
        #[arg(long, default_value = ".ideomancer/analyses")]
        out: PathBuf,
        #[arg(long, env = "IDEOMANCER_PYTHON", default_value = "python3.12")]
        python: PathBuf,
        /// Trusted protocol-compatible agent executable; may contact its provider.
        #[arg(long, env = "IDEOMANCER_AGENT", default_value = "djinn")]
        agent: PathBuf,
        #[arg(long, env = "IDEOMANCER_MODEL", default_value = "openai/gpt-6.1-sol")]
        model: Option<String>,
        #[arg(long, default_value_t = 300, value_parser = clap::value_parser!(u64).range(1..=600))]
        timeout_seconds: u64,
        /// Explicitly repeat failed or uncertain agent work.
        #[arg(long)]
        retry: bool,
        /// Only extract evidence: no agent/provider or diagram renderer.
        #[arg(long)]
        extract_only: bool,
        /// Generate the draft manifest and Mermaid source without requiring mmdc.
        #[arg(long)]
        no_render: bool,
        /// Focus on an exact callable symbol or a unique dot-suffix.
        #[arg(long)]
        focus: Option<String>,
        /// Local callable expansion depth for --focus (default: 3).
        #[arg(long, requires = "focus", value_parser = clap::value_parser!(u8).range(0..=5))]
        focus_hops: Option<u8>,
        /// Total selected-fact cap for --focus, including context (default: 600).
        #[arg(long, requires = "focus", value_parser = clap::value_parser!(u32).range(1..=2000))]
        focus_facts: Option<u32>,
        #[arg(long, env = "IDEOMANCER_MMDC", default_value = "mmdc")]
        mmdc: PathBuf,
    },
    /// Extract Python evidence without importing or running the application.
    Inspect {
        root: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value = "python3.12")]
        python: PathBuf,
    },
    /// Ask a protocol-compatible agent to propose an architecture for review.
    Interpret {
        run: PathBuf,
        /// Trusted executable implementing agent-execution.v1 (no shell).
        #[arg(long, default_value = "djinn")]
        agent: PathBuf,
        #[arg(long)]
        model: Option<String>,
        #[arg(long, default_value_t = 300, value_parser = clap::value_parser!(u64).range(1..=600))]
        timeout_seconds: u64,
        /// Explicitly repeat failed/uncertain work; may incur another model call.
        #[arg(long)]
        retry: bool,
    },
    /// Accept the exact reviewed proposal and generate repeatable diagram source.
    Accept {
        run: PathBuf,
        #[arg(long)]
        proposal_sha256: String,
    },
    /// Inspect a saved workflow and verify its artifact integrity.
    Status { run: PathBuf },
    /// Rebuild a readable draft view from saved evidence, without an agent call.
    Present {
        run: PathBuf,
        #[arg(long)]
        no_render: bool,
        #[arg(long, env = "IDEOMANCER_MMDC", default_value = "mmdc")]
        mmdc: PathBuf,
    },
    /// Print the exact built-in architecture workflow skill.
    Skill {
        /// Print the focused workflow instructions in addition to the overview skill.
        #[arg(long)]
        focus: bool,
    },

    /// Create a starter project map. Existing files are never overwritten.
    Init {
        #[arg(long)]
        name: String,
        /// Project identifier. Defaults to a slug derived from the name.
        #[arg(long)]
        id: Option<String>,
        /// Output path (parent directory must exist). Defaults to stdout.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Defaults to the output extension, or YAML for stdout.
        #[arg(long, value_enum)]
        format: Option<ManifestFormat>,
    },

    /// Check a manifest without generating a diagram.
    Validate {
        /// YAML/JSON input path, or '-' for stdin (the default).
        #[arg(default_value = "-")]
        input: String,
        /// Emit machine-readable validation diagnostics.
        #[arg(long)]
        json: bool,
    },

    /// Emit diagram source text, such as Mermaid.
    Emit {
        /// Input YAML or JSON manifest path. Use '-' to read stdin.
        input: String,

        /// Output source format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Mermaid)]
        format: OutputFormat,

        /// Optional output path. Defaults to stdout.
        #[arg(long)]
        out: Option<PathBuf>,
    },

    /// Render an actual diagram file using Mermaid CLI.
    Render {
        /// Input YAML or JSON manifest path. Use '-' to read stdin.
        input: String,

        /// Output diagram path. Extension controls Mermaid CLI output type, e.g. .svg, .png, or .pdf.
        #[arg(long)]
        out: PathBuf,

        /// Mermaid CLI executable to use.
        #[arg(long, default_value = "mmdc")]
        mmdc: PathBuf,
    },
}

#[derive(Debug, clap::Args)]
struct CatalogFilter {
    #[arg(long, value_enum)]
    kind: Option<catalog::Kind>,
    #[arg(long)]
    query: Option<String>,
    #[arg(long)]
    path: Option<String>,
    #[arg(long, default_value_t = 20, value_parser = clap::value_parser!(u32).range(1..=100))]
    limit: u32,
    #[arg(long, default_value_t = 0)]
    offset: usize,
    #[arg(long)]
    json: bool,
}
impl CatalogFilter {
    fn selection(&self) -> catalog::Filter {
        catalog::Filter {
            kind: self.kind.clone(),
            query: self.query.clone(),
            path: self.path.clone(),
            limit: self.limit as usize,
            offset: self.offset,
        }
    }
}

#[derive(Debug, Subcommand)]
enum PlanCommand {
    /// Validate grounding and references, then show a draft and its decision blockers.
    Review {
        input: PathBuf,
        /// Optional Markdown output; identical output is reused, conflicting files are refused.
        #[arg(long, conflicts_with = "json")]
        out: Option<PathBuf>,
        /// Return nonzero while requirements, questions, or decisions still block review readiness.
        #[arg(long)]
        check_ready: bool,
        /// Print structural validity and readiness diagnostics instead of Markdown.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Clone, Debug, ValueEnum)]
enum OutputFormat {
    Mermaid,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ManifestFormat {
    Yaml,
    Json,
}

#[derive(Serialize)]
struct ValidationReport {
    valid: bool,
    errors: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
enum IdeomancerError {
    #[error("{0}")]
    Workflow(String),
    #[error("{0}")]
    Usage(String),

    #[error("failed to serialize manifest: {0}")]
    SerializeManifest(String),

    #[error("failed to write stdout: {0}")]
    WriteStdout(#[source] io::Error),

    #[error("failed to create temporary Mermaid input: {0}")]
    TemporaryInput(#[source] io::Error),

    #[error("failed to read input {path:?}: {source}")]
    ReadFile {
        path: String,
        #[source]
        source: io::Error,
    },

    #[error("failed to read stdin: {0}")]
    ReadStdin(#[source] io::Error),

    #[error("failed to parse manifest as YAML or JSON: {0}")]
    ParseManifest(#[source] serde_yaml::Error),

    #[error("invalid manifest:\n{0}")]
    InvalidManifest(String),

    #[error("failed to write output {path:?}: {source}")]
    WriteFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to run Mermaid CLI {mmdc:?}: {source}")]
    RunMermaidCli {
        mmdc: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("Mermaid CLI failed with status {status}:\n{stderr}")]
    MermaidCliFailed { status: String, stderr: String },
}

pub(crate) fn main() {
    match run() {
        Ok(true) => {}
        Ok(false) => process::exit(1),
        Err(err) => {
            eprintln!("error: {err}");
            process::exit(1);
        }
    }
}

fn run() -> Result<bool, IdeomancerError> {
    let cli = Cli::parse();

    match cli.command {
        Command::Discover {
            root,
            lang,
            out,
            python,
            node,
            typescript,
            filter,
        } => {
            let store = match lang {
                analysis::Language::Python => discovery::discover(&root, &out, &python),
                analysis::Language::JavaScriptTypeScript => {
                    if node.is_some() || typescript.is_some() {
                        return Err(IdeomancerError::Usage(
                            "JavaScript/TypeScript discovery is native Tree-sitter: remove --node/--typescript and unset IDEOMANCER_NODE/IDEOMANCER_TYPESCRIPT; no external parser tools are used".into(),
                        ));
                    }
                    discovery::discover_javascript(&root, &out)
                }
            }.map_err(IdeomancerError::Workflow)?;
            let report = discovery::browse(&store, &filter.selection())
                .map_err(IdeomancerError::Workflow)?;
            if filter.json {
                write_stdout(&format!(
                    "{}\n",
                    serde_json::to_string(&report)
                        .map_err(|e| IdeomancerError::Workflow(e.to_string()))?
                ))?;
            } else {
                write_stdout(&catalog::markdown(
                    &report.page,
                    &report.catalog_directory.to_string_lossy(),
                ))?;
            }
            return Ok(report.complete);
        }
        Command::Catalog {
            catalog: path,
            filter,
        } => {
            let store = discovery::load(&path).map_err(IdeomancerError::Workflow)?;
            let report = discovery::browse(&store, &filter.selection())
                .map_err(IdeomancerError::Workflow)?;
            if filter.json {
                write_stdout(&format!(
                    "{}\n",
                    serde_json::to_string(&report)
                        .map_err(|e| IdeomancerError::Workflow(e.to_string()))?
                ))?;
            } else {
                write_stdout(&catalog::markdown(
                    &report.page,
                    &report.catalog_directory.to_string_lossy(),
                ))?;
            }
            return Ok(report.complete);
        }
        Command::Diagram {
            catalog,
            kind,
            entry,
            view,
            openapi,
            removed_profile,
            query,
            path,
            limit,
            offset,
            no_render,
            extract_only,
            mmdc,
            agent,
            model,
            python,
            focus_hops,
            focus_facts,
            timeout_seconds,
            retry,
        } => {
            if removed_profile.is_some() {
                return Err(IdeomancerError::Usage("The framework profile feature was removed; use the typed facts and claims extension API".into()));
            }
            let (report, success) = discovery::diagram(discovery::DiagramOptions {
                catalog,
                kind,
                entry,
                view,
                openapi,
                query,
                path,
                pagination_explicit: limit.is_some() || offset.is_some(),
                limit: limit.unwrap_or(12) as usize,
                offset: offset.unwrap_or(0),
                no_render,
                extract_only,
                mmdc,
                agent,
                model,
                python,
                focus_hops: focus_hops as usize,
                focus_facts: focus_facts as usize,
                timeout_seconds,
                retry,
            })
            .map_err(IdeomancerError::Workflow)?;
            write_stdout(&format!(
                "{}\n",
                serde_json::to_string(&report)
                    .map_err(|e| IdeomancerError::Workflow(e.to_string()))?
            ))?;
            return Ok(success);
        }
        Command::Plan {
            command:
                PlanCommand::Review {
                    input,
                    out,
                    check_ready,
                    json,
                },
        } => {
            return run_plan_review(&input, out.as_deref(), check_ready, json);
        }
        Command::Analyze {
            root,
            out,
            python,
            agent,
            model,
            timeout_seconds,
            retry,
            extract_only,
            no_render,
            focus,
            focus_hops,
            focus_facts,
            mmdc,
        } => {
            let report = analyze::run(analyze::Options {
                root,
                out,
                python,
                agent,
                model,
                timeout_seconds,
                retry,
                extract_only,
                no_render,
                expected_snapshot: None,
                focus: focus.map(|symbol| focus::FocusOptions {
                    symbol,
                    hops: focus_hops.unwrap_or(3) as usize,
                    max_facts: focus_facts.unwrap_or(600) as usize,
                }),
                mmdc,
            })
            .map_err(IdeomancerError::Workflow)?;
            let json = serde_json::to_string(&report)
                .map_err(|error| IdeomancerError::Workflow(error.to_string()))?;
            write_stdout(&format!("{json}\n"))?;
            return Ok(report.error.is_none());
        }
        Command::Inspect { root, out, python } => {
            let state =
                workflow::inspect(&root, &out, &python).map_err(IdeomancerError::Workflow)?;
            workflow_status(&state)?;
            return Ok(state.complete);
        }
        Command::Interpret {
            run,
            agent,
            model,
            timeout_seconds,
            retry,
        } => {
            let state = workflow::interpret(&run, &agent, model, timeout_seconds, retry)
                .map_err(IdeomancerError::Workflow)?;
            workflow_status(&state)?;
        }
        Command::Accept {
            run,
            proposal_sha256,
        } => {
            let state =
                workflow::accept(&run, &proposal_sha256).map_err(IdeomancerError::Workflow)?;
            workflow_status(&state)?;
        }
        Command::Status { run } => {
            let state = workflow::status(&run).map_err(IdeomancerError::Workflow)?;
            workflow_status(&state)?;
        }
        Command::Present {
            run,
            no_render,
            mmdc,
        } => {
            let report =
                analyze::present(&run, &mmdc, no_render).map_err(IdeomancerError::Workflow)?;
            let json = serde_json::to_string(&report)
                .map_err(|e| IdeomancerError::Workflow(e.to_string()))?;
            write_stdout(&format!("{json}\n"))?;
            return Ok(report.error.is_none());
        }
        Command::Skill { focus } => write_stdout(&workflow::instructions_for(focus))?,
        Command::Init {
            name,
            id,
            out,
            format,
        } => {
            let id = id.unwrap_or_else(|| slug(&name));
            if id.is_empty() && !name.trim().is_empty() {
                return Err(IdeomancerError::Usage(
                    "cannot derive an identifier from this name; supply --id explicitly"
                        .to_string(),
                ));
            }
            let manifest = Manifest {
                schema_version: SCHEMA_VERSION.to_string(),
                id,
                name: name.clone(),
                nodes: vec![Node {
                    id: "app".to_string(),
                    label: Some(name),
                    kind: None,
                }],
                edges: Vec::new(),
            };
            ensure_valid(&manifest)?;
            let format = match format {
                Some(format) => format,
                None => match out.as_deref() {
                    None => ManifestFormat::Yaml,
                    Some(path) => match path.extension().and_then(|ext| ext.to_str()) {
                        Some("json") => ManifestFormat::Json,
                        Some("yaml" | "yml") => ManifestFormat::Yaml,
                        _ => return Err(IdeomancerError::Usage(
                            "init output must end in .json, .yaml, or .yml; otherwise specify --format".to_string()
                        )),
                    },
                },
            };
            let source = match format {
                ManifestFormat::Yaml => serde_yaml::to_string(&manifest)
                    .map_err(|err| IdeomancerError::SerializeManifest(err.to_string()))?,
                ManifestFormat::Json => serde_json::to_string_pretty(&manifest)
                    .map(|json| format!("{json}\n"))
                    .map_err(|err| IdeomancerError::SerializeManifest(err.to_string()))?,
            };
            if let Some(path) = out {
                // Atomic no-clobber creation also refuses existing symlinks.
                let mut file = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .map_err(|source| IdeomancerError::WriteFile {
                        path: path.clone(),
                        source,
                    })?;
                file.write_all(source.as_bytes())
                    .map_err(|source| IdeomancerError::WriteFile { path, source })?;
            } else {
                write_stdout(&source)?;
            }
        }
        Command::Validate { input, json } => {
            let errors = match parse_manifest(&input) {
                Ok(manifest) => validate::validate_manifest(&manifest),
                Err(err @ IdeomancerError::ParseManifest(_)) => vec![err.to_string()],
                Err(err) => return Err(err),
            };
            let report = ValidationReport {
                valid: errors.is_empty(),
                errors,
            };
            if json {
                let output = serde_json::to_string(&report)
                    .map_err(|err| IdeomancerError::SerializeManifest(err.to_string()))?;
                write_stdout(&format!("{output}\n"))?;
            } else if report.valid {
                write_stdout("Manifest is valid.\n")?;
            } else {
                eprintln!(
                    "error: invalid manifest:\n{}",
                    format_errors(&report.errors)
                );
            }
            return Ok(report.valid);
        }
        Command::Emit { input, format, out } => {
            let manifest = load_manifest(&input)?;
            let rendered = emit_diagram_source(&manifest, format);

            if let Some(path) = out {
                fs::write(&path, rendered)
                    .map_err(|source| IdeomancerError::WriteFile { path, source })?;
            } else {
                write_stdout(&rendered)?;
            }
        }
        Command::Render { input, out, mmdc } => {
            let manifest = load_manifest(&input)?;
            let mermaid = render::mermaid::render(&manifest);
            render_mermaid_diagram(&mermaid, &out, &mmdc)?;
        }
    }

    Ok(true)
}

fn workflow_status(state: &workflow::RunState) -> Result<(), IdeomancerError> {
    let json =
        serde_json::to_string(state).map_err(|e| IdeomancerError::Workflow(e.to_string()))?;
    write_stdout(&format!("{json}\n"))
}

fn read_review_file(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let meta =
        fs::symlink_metadata(path).map_err(|e| format!("Read plan {}: {e}", path.display()))?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > limit {
        return Err("Plan/review must be a bounded regular non-symlink file".into());
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
        return Err("Plan is not a regular file".into());
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("Plan/review exceeds its file size limit".into());
    }
    Ok(bytes)
}

fn save_plan_review(path: &Path, source: &Path, markdown: &str) -> Result<(), String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(path)
    };
    if fs::canonicalize(&absolute).ok().as_deref() == Some(source) {
        return Err("Review output must not replace the input plan".into());
    }
    if fs::symlink_metadata(&absolute).is_ok() {
        // Reuse only exact, bounded regular-file content; never replace notes.
        if read_review_file(&absolute, 8 * 1024 * 1024)? == markdown.as_bytes() {
            return Ok(());
        }
        return Err(
            "Review output already exists with different content; use another --out path".into(),
        );
    }
    let parent = absolute.parent().ok_or("Review output has no parent")?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|e| format!("Create review output (parent must exist): {e}"))?;
    temporary
        .write_all(markdown.as_bytes())
        .map_err(|e| e.to_string())?;
    temporary.as_file().sync_all().map_err(|e| e.to_string())?;
    temporary
        .persist_noclobber(&absolute)
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn run_plan_review(
    input: &Path,
    out: Option<&Path>,
    check_ready: bool,
    json_output: bool,
) -> Result<bool, IdeomancerError> {
    let loaded = (|| -> Result<_, String> {
        let bytes = read_review_file(input, 1024 * 1024)?;
        let text = std::str::from_utf8(&bytes).map_err(|_| "Plan is not UTF-8")?;
        let plan = plan::parse(text)?;
        let source = fs::canonicalize(input).map_err(|e| e.to_string())?;
        let run = Path::new(&plan.architecture.run_directory);
        let run = if run.is_absolute() {
            run.to_path_buf()
        } else {
            source
                .parent()
                .ok_or("Plan has no parent directory")?
                .join(run)
        };
        let context = workflow::planning_context(&run)?;
        let review = plan::review(&plan, &context);
        Ok((plan, context, review, source))
    })();
    let (plan, context, review, source) = match loaded {
        Ok(loaded) => loaded,
        Err(error) if json_output => {
            let review = plan::Review {
                valid: false,
                ready_for_plan_review: false,
                errors: vec![error],
                blockers: vec![],
                warnings: vec!["No implementation is authorized.".into()],
            };
            let json = serde_json::to_string(&review)
                .map_err(|e| IdeomancerError::Workflow(e.to_string()))?;
            write_stdout(&format!("{json}\n"))?;
            return Ok(false);
        }
        Err(error) => return Err(IdeomancerError::Workflow(error)),
    };
    if json_output {
        let json =
            serde_json::to_string(&review).map_err(|e| IdeomancerError::Workflow(e.to_string()))?;
        write_stdout(&format!("{json}\n"))?;
    } else if review.valid {
        let markdown =
            plan::markdown(&plan, &context, &review).map_err(IdeomancerError::Workflow)?;
        if let Some(path) = out {
            save_plan_review(path, &source, &markdown).map_err(IdeomancerError::Workflow)?;
        } else {
            write_stdout(&markdown)?;
        }
    } else {
        return Err(IdeomancerError::Workflow(format!(
            "Invalid software plan:\n{}",
            format_errors(&review.errors)
        )));
    }
    Ok(review.valid && (!check_ready || review.ready_for_plan_review))
}

fn load_manifest(input: &str) -> Result<Manifest, IdeomancerError> {
    let manifest = parse_manifest(input)?;
    ensure_valid(&manifest)?;
    Ok(manifest)
}

fn parse_manifest(input: &str) -> Result<Manifest, IdeomancerError> {
    let source = read_input(input)?;
    // Deserialize through Value so scalar types are preserved: serde_yaml's
    // direct String deserializer otherwise accepts booleans, numbers and null.
    let value: serde_yaml::Value =
        serde_yaml::from_str(&source).map_err(IdeomancerError::ParseManifest)?;
    serde_yaml::from_value(value).map_err(IdeomancerError::ParseManifest)
}

fn ensure_valid(manifest: &Manifest) -> Result<(), IdeomancerError> {
    let errors = validate::validate_manifest(manifest);
    if !errors.is_empty() {
        return Err(IdeomancerError::InvalidManifest(format_errors(&errors)));
    }
    Ok(())
}

fn format_errors(errors: &[String]) -> String {
    errors
        .iter()
        .map(|err| format!("- {err}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn write_stdout(output: &str) -> Result<(), IdeomancerError> {
    io::stdout()
        .lock()
        .write_all(output.as_bytes())
        .map_err(IdeomancerError::WriteStdout)
}

fn slug(name: &str) -> String {
    name.split(|ch: char| !ch.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join("-")
}

fn emit_diagram_source(manifest: &Manifest, format: OutputFormat) -> String {
    match format {
        OutputFormat::Mermaid => render::mermaid::render(manifest),
    }
}

fn render_mermaid_diagram(
    mermaid_source: &str,
    out: &Path,
    mmdc: &Path,
) -> Result<(), IdeomancerError> {
    if !matches!(
        out.extension().and_then(|ext| ext.to_str()),
        Some("svg" | "png" | "pdf")
    ) {
        return Err(IdeomancerError::Usage(
            "render output must end in .svg, .png, or .pdf".to_string(),
        ));
    }
    let mut input = tempfile::Builder::new()
        .prefix("ideomancer-")
        .suffix(".mmd")
        .tempfile()
        .map_err(IdeomancerError::TemporaryInput)?;
    input
        .write_all(mermaid_source.as_bytes())
        .map_err(IdeomancerError::TemporaryInput)?;

    let output = process::Command::new(mmdc)
        .arg("-i")
        .arg(input.path())
        .arg("-o")
        .arg(out)
        .output()
        .map_err(|source| IdeomancerError::RunMermaidCli {
            mmdc: mmdc.to_path_buf(),
            source,
        })?;
    if !output.status.success() {
        return Err(IdeomancerError::MermaidCliFailed {
            status: output
                .status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "terminated by signal".to_string()),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    Ok(())
}

fn read_input(input: &str) -> Result<String, IdeomancerError> {
    if input == "-" {
        let mut buffer = String::new();
        io::stdin()
            .read_to_string(&mut buffer)
            .map_err(IdeomancerError::ReadStdin)?;
        return Ok(buffer);
    }

    fs::read_to_string(input).map_err(|source| IdeomancerError::ReadFile {
        path: input.to_string(),
        source,
    })
}
