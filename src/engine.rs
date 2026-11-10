//! Versioned, capability-limited engine facades. Saved artifact formats are independent.
pub mod v1 {
    pub use crate::extensions::*;
    use crate::{catalog, discovery};
    use serde::{Deserialize, Serialize};
    use std::path::{Path, PathBuf};

    pub const API_VERSION: &str = "ideomancer.engine.v1";

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "snake_case")]
    pub enum Language {
        Python,
        #[serde(rename = "javascript_typescript")]
        JavaScriptTypeScript,
    }

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    pub struct Capabilities {
        pub schema_version: String,
        pub languages: Vec<Language>,
        pub discover: bool,
        pub browse: bool,
        pub class_diagrams: bool,
        pub source_only: bool,
        pub svg: bool,
    }

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(tag = "language", rename_all = "snake_case", deny_unknown_fields)]
    pub enum Toolchain {
        Python {
            python: PathBuf,
        },
        /// Compiled-in Tree-sitter grammars; no runtime/tool settings are accepted.
        #[serde(rename = "javascript_typescript_native")]
        NativeJavaScriptTypeScript {},
        /// Retained request shape only; discovery rejects retired external tools.
        #[serde(rename = "javascript_typescript")]
        JavaScriptTypeScript {
            node: PathBuf,
            typescript: PathBuf,
        },
    }

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    pub struct DiscoverRequest {
        pub source_root: PathBuf,
        pub output_container: PathBuf,
        pub toolchain: Toolchain,
    }

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    pub struct DiscoveryReport {
        pub schema_version: String,
        pub catalog_directory: PathBuf,
        pub source_root: PathBuf,
        pub snapshot_id: String,
        pub complete: bool,
        pub warnings: Vec<String>,
    }

    /// An identity-bound reference, not cached evidence or authority over a source root.
    #[derive(Clone, Debug)]
    pub struct CatalogRef {
        directory: PathBuf,
        source_root: PathBuf,
        snapshot_id: String,
        complete: bool,
    }

    impl CatalogRef {
        pub fn directory(&self) -> &Path {
            &self.directory
        }
        pub fn source_root(&self) -> &Path {
            &self.source_root
        }
        pub fn snapshot_id(&self) -> &str {
            &self.snapshot_id
        }
        pub fn is_complete(&self) -> bool {
            self.complete
        }
    }

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "snake_case")]
    pub enum ItemKind {
        Api,
        Class,
        Workflow,
    }

    impl From<&ItemKind> for catalog::Kind {
        fn from(kind: &ItemKind) -> Self {
            match kind {
                ItemKind::Api => Self::Api,
                ItemKind::Class => Self::Class,
                ItemKind::Workflow => Self::Workflow,
            }
        }
    }

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(default, rename_all = "camelCase", deny_unknown_fields)]
    pub struct BrowseOptions {
        pub kind: Option<ItemKind>,
        pub query: Option<String>,
        pub path: Option<String>,
        pub limit: usize,
        pub offset: usize,
    }

    impl Default for BrowseOptions {
        fn default() -> Self {
            Self {
                kind: None,
                query: None,
                path: None,
                limit: 20,
                offset: 0,
            }
        }
    }

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    pub struct CatalogCounts {
        pub api: usize,
        pub classes: usize,
        pub workflows: usize,
    }

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    pub struct CatalogItem {
        pub id: String,
        pub kind: ItemKind,
        pub role: String,
        pub label: String,
        pub symbol: String,
        pub path: String,
        pub line: usize,
        pub end_line: usize,
        pub fact_id: String,
        pub evidence_ids: Vec<String>,
        pub method: Option<String>,
        pub route_path: Option<String>,
        pub path_is_fragment: bool,
    }

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    pub struct BrowseReport {
        pub schema_version: String,
        pub snapshot_id: String,
        pub complete: bool,
        pub total_items: usize,
        pub matched_items: usize,
        pub offset: usize,
        pub limit: usize,
        pub next_offset: Option<usize>,
        pub counts: CatalogCounts,
        pub items: Vec<CatalogItem>,
        pub warnings: Vec<String>,
    }

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(tag = "kind", rename_all = "snake_case", from = "ClassSelectionWire")]
    pub enum ClassSelection {
        All,
        Entry {
            id: String,
        },
        Page {
            query: Option<String>,
            path: Option<String>,
            limit: usize,
            offset: usize,
        },
    }

    // Struct variants enforce unknown-field rejection even for empty selections;
    // Serde's internally tagged unit variants otherwise discard extra fields.
    #[derive(Deserialize)]
    #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
    enum ClassSelectionWire {
        All {},
        Entry {
            id: String,
        },
        Page {
            query: Option<String>,
            path: Option<String>,
            limit: usize,
            offset: usize,
        },
    }

    impl From<ClassSelectionWire> for ClassSelection {
        fn from(value: ClassSelectionWire) -> Self {
            match value {
                ClassSelectionWire::All {} => Self::All,
                ClassSelectionWire::Entry { id } => Self::Entry { id },
                ClassSelectionWire::Page {
                    query,
                    path,
                    limit,
                    offset,
                } => Self::Page {
                    query,
                    path,
                    limit,
                    offset,
                },
            }
        }
    }

    #[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(tag = "kind", rename_all = "snake_case", from = "RenderModeWire")]
    pub enum RenderMode {
        #[default]
        SourceOnly,
        Svg {
            mmdc: PathBuf,
        },
    }

    #[derive(Deserialize)]
    #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
    enum RenderModeWire {
        SourceOnly {},
        Svg { mmdc: PathBuf },
    }

    impl From<RenderModeWire> for RenderMode {
        fn from(value: RenderModeWire) -> Self {
            match value {
                RenderModeWire::SourceOnly {} => Self::SourceOnly,
                RenderModeWire::Svg { mmdc } => Self::Svg { mmdc },
            }
        }
    }

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    pub struct ClassCoverage {
        pub catalog_classes: usize,
        pub selected_classes: usize,
        pub home_classes: usize,
        pub all_selected_classes_represented: bool,
        pub diagram_count: usize,
        pub modeled_relations: usize,
        pub relations_drawn: usize,
        pub relations_omitted: usize,
        pub relation_accounting: String,
        pub relations_represented_as_fields: usize,
        pub relations_represented_as_fields_visible: usize,
        pub relations_represented_as_fields_hidden: usize,
    }

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    pub struct DiagramArtifact {
        pub id: String,
        pub title: String,
        pub mermaid: PathBuf,
        pub svg: Option<PathBuf>,
    }

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    pub struct ClassDiagramReport {
        pub schema_version: String,
        pub catalog_directory: PathBuf,
        pub source_root: PathBuf,
        pub snapshot_id: String,
        pub complete: bool,
        pub selection: ClassSelection,
        pub coverage: ClassCoverage,
        pub diagrams: Vec<DiagramArtifact>,
        pub viewer: PathBuf,
        pub legend: PathBuf,
        pub view_directory: PathBuf,
        pub views_index: PathBuf,
        pub reused: bool,
        pub warnings: Vec<String>,
    }

    #[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "snake_case")]
    pub enum ErrorCode {
        InvalidRequest,
        CatalogUnavailable,
        DiscoveryFailed,
        ClassDiagramFailed,
    }

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, thiserror::Error)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    #[error("{message}")]
    pub struct EngineError {
        pub code: ErrorCode,
        pub message: String,
    }

    fn error(code: ErrorCode, message: impl ToString) -> EngineError {
        EngineError {
            code,
            message: message.to_string(),
        }
    }

    // Resolve operator-selected tools before subprocess cwd changes, and never
    // turn a target-local executable into an extraction or rendering hook.
    fn trusted_tool(path: &Path, source_root: &Path) -> Result<PathBuf, EngineError> {
        let executable = |path: &Path| {
            std::fs::metadata(path).is_ok_and(|metadata| {
                if !metadata.is_file() {
                    return false;
                }
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    metadata.permissions().mode() & 0o111 != 0
                }
                #[cfg(not(unix))]
                {
                    true
                }
            })
        };
        let candidate = if path.is_relative() && path.components().count() == 1 {
            std::env::var_os("PATH")
                .and_then(|paths| {
                    std::env::split_paths(&paths)
                        .map(|dir| dir.join(path))
                        .find(|p| executable(p))
                })
                .ok_or_else(|| {
                    error(
                        ErrorCode::InvalidRequest,
                        format!("Selected tool {} is unavailable: no executable regular file on the operator PATH", path.display()),
                    )
                })?
        } else {
            if !executable(path) {
                return Err(error(
                    ErrorCode::InvalidRequest,
                    format!("Selected tool {} is unavailable: select an executable regular file and check its execute permissions", path.display()),
                ));
            }
            path.to_path_buf()
        };
        let resolved =
            std::fs::canonicalize(candidate).map_err(|e| error(ErrorCode::InvalidRequest, e))?;
        if resolved.starts_with(source_root) {
            return Err(error(
                ErrorCode::InvalidRequest,
                "Selected tool must be outside the source root",
            ));
        }
        Ok(resolved)
    }

    pub fn capabilities() -> Capabilities {
        Capabilities {
            schema_version: API_VERSION.into(),
            languages: vec![Language::Python, Language::JavaScriptTypeScript],
            discover: true,
            browse: true,
            class_diagrams: true,
            source_only: true,
            svg: true,
        }
    }

    pub fn discover(request: &DiscoverRequest) -> Result<DiscoveryReport, EngineError> {
        if matches!(request.toolchain, Toolchain::JavaScriptTypeScript { .. }) {
            return Err(error(
                ErrorCode::InvalidRequest,
                "JavaScript/TypeScript discovery is native Tree-sitter; use NativeJavaScriptTypeScript {} (javascript_typescript_native) without node/typescript settings",
            ));
        }
        let tools: Vec<&Path> = match &request.toolchain {
            Toolchain::Python { python } => vec![python],
            Toolchain::NativeJavaScriptTypeScript {} => vec![],
            Toolchain::JavaScriptTypeScript { .. } => unreachable!("retired toolchain rejected"),
        };
        if tools.iter().any(|path| path.as_os_str().is_empty()) {
            return Err(error(
                ErrorCode::InvalidRequest,
                "Tool paths must not be empty",
            ));
        }
        let source_root = std::fs::canonicalize(&request.source_root)
            .map_err(|e| error(ErrorCode::DiscoveryFailed, e))?;
        let store = match &request.toolchain {
            Toolchain::Python { python } => discovery::discover(
                &request.source_root,
                &request.output_container,
                &trusted_tool(python, &source_root)?,
            ),
            Toolchain::NativeJavaScriptTypeScript {} => {
                discovery::discover_javascript(&request.source_root, &request.output_container)
            }
            Toolchain::JavaScriptTypeScript { .. } => unreachable!("retired toolchain rejected"),
        }
        .map_err(|e| error(ErrorCode::DiscoveryFailed, e))?;
        Ok(DiscoveryReport {
            schema_version: API_VERSION.into(),
            catalog_directory: store.directory,
            source_root: store.catalog.source_root.into(),
            snapshot_id: store.catalog.snapshot_id,
            complete: store.catalog.complete,
            warnings: store.catalog.warnings,
        })
    }

    pub fn open_catalog(path: impl AsRef<Path>) -> Result<CatalogRef, EngineError> {
        let store =
            discovery::load(path.as_ref()).map_err(|e| error(ErrorCode::CatalogUnavailable, e))?;
        Ok(CatalogRef {
            directory: store.directory,
            source_root: store.catalog.source_root.into(),
            snapshot_id: store.catalog.snapshot_id,
            complete: store.catalog.complete,
        })
    }

    pub fn browse(
        handle: &CatalogRef,
        options: &BrowseOptions,
    ) -> Result<BrowseReport, EngineError> {
        if !(1..=100).contains(&options.limit) {
            return Err(error(
                ErrorCode::InvalidRequest,
                "Browse limit must be 1..=100",
            ));
        }
        let store = discovery::load(handle.directory())
            .map_err(|e| error(ErrorCode::CatalogUnavailable, e))?;
        if Path::new(&store.catalog.source_root) != handle.source_root()
            || store.catalog.snapshot_id != handle.snapshot_id()
        {
            return Err(error(
                ErrorCode::CatalogUnavailable,
                "Catalog handle is stale: source root or snapshot changed",
            ));
        }
        let page = catalog::page(
            &store.catalog,
            &catalog::Filter {
                kind: options.kind.as_ref().map(Into::into),
                query: options.query.clone(),
                path: options.path.clone(),
                limit: options.limit,
                offset: options.offset,
            },
        )
        .map_err(|e| error(ErrorCode::InvalidRequest, e))?;
        // Translate implementation artifacts, never re-export their mutable types.
        let items = page
            .items
            .into_iter()
            .map(|item| {
                serde_json::to_value(item)
                    .and_then(serde_json::from_value)
                    .map_err(|e| error(ErrorCode::CatalogUnavailable, e))
            })
            .collect::<Result<_, _>>()?;
        Ok(BrowseReport {
            schema_version: API_VERSION.into(),
            snapshot_id: page.snapshot_id,
            complete: page.complete,
            total_items: page.total_items,
            matched_items: page.matched_items,
            offset: page.offset,
            limit: page.limit,
            next_offset: page.next_offset,
            counts: CatalogCounts {
                api: *page.counts.get("api").unwrap_or(&0),
                classes: *page.counts.get("class").unwrap_or(&0),
                workflows: *page.counts.get("workflow").unwrap_or(&0),
            },
            items,
            warnings: page.warnings,
        })
    }

    pub fn class_diagrams(
        handle: &CatalogRef,
        selection: &ClassSelection,
        render: &RenderMode,
    ) -> Result<ClassDiagramReport, EngineError> {
        class_diagrams_internal(handle, selection, render, None)
    }

    pub(crate) fn class_diagrams_internal(
        handle: &CatalogRef,
        selection: &ClassSelection,
        render: &RenderMode,
        claims: Option<&ValidatedClaimsRef>,
    ) -> Result<ClassDiagramReport, EngineError> {
        let (entry, query, path, limit, offset, pagination_explicit) = match selection {
            ClassSelection::All => (None, None, None, 12, 0, false),
            ClassSelection::Entry { id } => {
                if id.is_empty() {
                    return Err(error(
                        ErrorCode::InvalidRequest,
                        "Class ID must not be empty",
                    ));
                }
                (Some(id.clone()), None, None, 12, 0, false)
            }
            ClassSelection::Page {
                query,
                path,
                limit,
                offset,
            } => {
                if !(1..=30).contains(limit) {
                    return Err(error(
                        ErrorCode::InvalidRequest,
                        "Class page limit must be 1..=30",
                    ));
                }
                (None, query.clone(), path.clone(), *limit, *offset, true)
            }
        };
        let (no_render, mmdc) = match render {
            RenderMode::SourceOnly => (true, PathBuf::new()),
            RenderMode::Svg { mmdc } => {
                if mmdc.as_os_str().is_empty() {
                    return Err(error(
                        ErrorCode::InvalidRequest,
                        "Renderer path must not be empty",
                    ));
                }
                (false, trusted_tool(mmdc, handle.source_root())?)
            }
        };
        let options = discovery::DiagramOptions {
            catalog: handle.directory.clone(),
            kind: Some(catalog::Kind::Class),
            entry,
            query,
            path,
            limit,
            offset,
            pagination_explicit,
            no_render,
            mmdc,
            view: None,
            openapi: None,
            extract_only: false,
            agent: PathBuf::new(),
            model: None,
            python: PathBuf::new(),
            focus_hops: 0,
            focus_facts: 0,
            timeout_seconds: 0,
            retry: false,
        };
        let (value, _) = discovery::diagram_augmented(
            options,
            Some((handle.source_root(), handle.snapshot_id())),
            claims,
        )
        .map_err(|e| error(ErrorCode::ClassDiagramFailed, e))?;
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Published {
            catalog_directory: PathBuf,
            snapshot_id: String,
            coverage: ClassCoverage,
            diagrams: Vec<DiagramArtifact>,
            viewer: PathBuf,
            legend: PathBuf,
            view_directory: PathBuf,
            views_index: PathBuf,
            reused: bool,
            complete: bool,
            warnings: Vec<String>,
        }
        let published: Published =
            serde_json::from_value(value).map_err(|e| error(ErrorCode::ClassDiagramFailed, e))?;
        Ok(ClassDiagramReport {
            schema_version: API_VERSION.into(),
            catalog_directory: published.catalog_directory,
            source_root: handle.source_root.clone(),
            snapshot_id: published.snapshot_id,
            complete: published.complete,
            selection: selection.clone(),
            coverage: published.coverage,
            diagrams: published.diagrams,
            viewer: published.viewer,
            legend: published.legend,
            view_directory: published.view_directory,
            views_index: published.views_index,
            reused: published.reused,
            warnings: published.warnings,
        })
    }
}
