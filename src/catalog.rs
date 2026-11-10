//! Pure, source-syntax discovery. No runtime registration or activation is inferred.
use crate::analysis::{Evidence, Fact};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const VERSION: &str = "ideomancer.catalog.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Api,
    Workflow,
    Class,
}

impl Kind {
    fn name(&self) -> &'static str {
        match self {
            Self::Api => "api",
            Self::Workflow => "workflow",
            Self::Class => "class",
        }
    }

    fn rank(&self) -> u8 {
        match self {
            Self::Api => 0,
            Self::Workflow => 1,
            Self::Class => 2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Catalog {
    pub schema_version: String,
    pub source_root: String,
    pub snapshot_id: String,
    pub extractor_version: String,
    pub complete: bool,
    pub items: Vec<Item>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Item {
    pub id: String,
    pub kind: Kind,
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

#[derive(Debug)]
pub struct Filter {
    pub kind: Option<Kind>,
    pub query: Option<String>,
    pub path: Option<String>,
    pub limit: usize,
    pub offset: usize,
}

impl Default for Filter {
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub snapshot_id: String,
    pub complete: bool,
    pub total_items: usize,
    pub matched_items: usize,
    pub offset: usize,
    pub limit: usize,
    pub next_offset: Option<usize>,
    pub counts: BTreeMap<String, usize>,
    pub items: Vec<Item>,
    pub warnings: Vec<String>,
}

fn callable(f: &Fact) -> bool {
    matches!(f.kind.as_str(), "function" | "async_function")
}

fn item(f: &Fact, kind: Kind, role: &str) -> Item {
    Item {
        id: String::new(),
        kind,
        role: role.into(),
        label: f.symbol.clone(),
        symbol: f.symbol.clone(),
        path: f.path.clone(),
        line: f.line,
        end_line: f.end_line,
        fact_id: f.id.clone(),
        evidence_ids: vec![f.id.clone()],
        method: None,
        route_path: None,
        path_is_fragment: false,
    }
}

pub fn build(evidence: &Evidence, source_root: &str) -> Result<Catalog, String> {
    evidence.validate_language()?;
    let javascript = evidence.is_javascript();
    if javascript
        && evidence
            .facts
            .iter()
            .any(|fact| !evidence.files.iter().any(|file| file.path == fact.path))
    {
        return Err("JavaScript source file provenance missing".into());
    }
    let mut unique = BTreeMap::new();
    for fact in &evidence.facts {
        if let Some(old) = unique.insert(fact.id.as_str(), fact) {
            if serde_json::to_vec(old).map_err(|e| e.to_string())?
                != serde_json::to_vec(fact).map_err(|e| e.to_string())?
            {
                return Err("conflicting source facts share an ID".into());
            }
        }
    }
    let facts: Vec<&Fact> = unique.values().copied().collect();
    let js = if javascript {
        Some(crate::js_focus::Index::new(facts.clone())?)
    } else {
        None
    };
    let mut warnings = BTreeSet::from([
        "Static source-syntax candidates only: no runtime endpoints, activation, router prefixes, or business-workflow completeness are inferred.".to_string(),
        "Source completeness describes the extractor's selected scope, not exhaustive discovery; default scope excludes scripts, shadow, tests, and other excluded paths. Dynamic registration and prefixes remain unresolved.".to_string(),
    ]);
    if !evidence.complete {
        warnings.insert("Source extraction is incomplete; candidates may be missing.".into());
    }
    // Diagnostic bodies and unrelated fact values can contain source literals.
    if !evidence.diagnostics.is_empty() {
        warnings.insert("Source diagnostics are present; extraction or dynamic discovery may be unresolved. Consult the source evidence diagnostics.".into());
    }
    let mut attached: BTreeMap<&str, Vec<&Fact>> = BTreeMap::new();
    for meta in facts
        .iter()
        .copied()
        .filter(|f| matches!(f.kind.as_str(), "api_route" | "entrypoint"))
    {
        if javascript {
            let value: serde_json::Value = serde_json::from_str(&meta.value).unwrap_or_default();
            let owner = value["handlerFactId"]
                .as_str()
                .and_then(|id| unique.get(id))
                .copied();
            if let Some(owner) = owner.filter(|f| callable(f) && f.path == meta.path) {
                if js
                    .as_ref()
                    .unwrap()
                    .declarations
                    .keys()
                    .any(|i| facts[*i].id == owner.id)
                {
                    attached.entry(&owner.id).or_default().push(meta);
                    continue;
                }
            }
            warnings.insert(
                "JavaScript API metadata has no source-grounded explicit handlerFactId; ignored."
                    .into(),
            );
            continue;
        }
        let mut owners: Vec<_> = facts
            .iter()
            .copied()
            .filter(|f| {
                callable(f)
                    && f.path == meta.path
                    && f.symbol == meta.symbol
                    && meta.end_line < f.line
            })
            .collect();
        owners.sort_by_key(|f| (f.line, &f.id));
        match owners.first() {
            Some(owner) if owners.get(1).is_none_or(|other| other.line != owner.line) => {
                attached.entry(&owner.id).or_default().push(meta);
            }
            _ => {
                warnings.insert(
                    "Some discovery metadata has no unambiguous following callable declaration."
                        .into(),
                );
            }
        }
    }
    let mut items = Vec::new();
    for f in &facts {
        if f.kind == "class" {
            let mut entry = item(f, Kind::Class, "class_declaration");
            entry.evidence_ids.extend(
                facts
                    .iter()
                    .filter(|b| {
                        b.kind == "base"
                            && b.path == f.path
                            && b.symbol == f.symbol
                            && f.line <= b.line
                            && b.end_line <= f.end_line
                    })
                    .map(|b| b.id.clone()),
            );
            items.push(entry);
            continue;
        }
        if !callable(f) {
            continue;
        }
        let mut roles: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for meta in attached.get(f.id.as_str()).into_iter().flatten() {
            let Ok(value) = serde_json::from_str::<serde_json::Value>(&meta.value) else {
                warnings.insert("Malformed discovery metadata was ignored.".into());
                continue;
            };
            if meta.kind == "entrypoint" {
                if let Some(role @ ("cli_command" | "job")) = value["role"].as_str() {
                    roles.entry(role.into()).or_default().push(meta.id.clone());
                } else {
                    warnings.insert("Unknown entrypoint metadata was ignored.".into());
                }
                continue;
            }
            if value["pathIsFragment"].as_bool() != Some(true)
                || !value["methods"].is_array()
                || !(value["path"].is_null() || value["path"].is_string())
            {
                warnings.insert("Malformed API metadata was ignored.".into());
                continue;
            }
            roles
                .entry("http_handler".into())
                .or_default()
                .push(meta.id.clone());
            let methods = value["methods"].as_array().unwrap();
            let known = [
                "GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "HEAD", "TRACE", "CONNECT",
            ];
            let valid = methods
                .iter()
                .all(|m| m.as_str().is_some_and(|m| known.contains(&m)));
            let methods: BTreeSet<Option<String>> = if valid && !methods.is_empty() {
                methods
                    .iter()
                    .map(|m| Some(m.as_str().unwrap().into()))
                    .collect()
            } else {
                warnings
                    .insert("Some API methods are unknown; no HTTP method was inferred.".into());
                BTreeSet::from([None])
            };
            for method in methods {
                let mut entry = item(f, Kind::Api, "http_handler");
                entry.method = method;
                entry.route_path = value["path"].as_str().map(str::to_owned);
                entry.path_is_fragment = true;
                entry.evidence_ids.push(meta.id.clone());
                if entry.route_path.is_none() {
                    warnings.insert("Some API paths are dynamic; path fragments and router prefixes are unresolved.".into());
                }
                entry.label = format!(
                    "{} {} [path fragment] {}",
                    entry.method.as_deref().unwrap_or("unknown"),
                    entry.route_path.as_deref().unwrap_or("dynamic"),
                    f.symbol
                );
                items.push(entry);
            }
        }
        // Only immediate module functions and direct class methods are implicit
        // workflow candidates. Bound ranges distinguish repeated declarations.
        let parent_symbol = f.symbol.rsplit_once('.').map(|(parent, _)| parent);
        let parent = facts
            .iter()
            .copied()
            .filter(|p| {
                p.path == f.path
                    && Some(p.symbol.as_str()) == parent_symbol
                    && (p.kind == "module"
                        || ((callable(p) || p.kind == "class")
                            && p.line <= f.line
                            && f.end_line <= p.end_line
                            && p.id != f.id))
            })
            .max_by_key(|p| (p.line, std::cmp::Reverse(p.end_line), &p.id));
        if javascript && roles.is_empty() {
            let js = js.as_ref().unwrap();
            if let Some((d, (meta, value))) =
                js.declarations.iter().find(|(i, _)| facts[**i].id == f.id)
            {
                if value["publicCandidate"].as_bool() == Some(true)
                    && value["visibility"] == "public"
                    && !matches!(
                        value["kind"].as_str(),
                        Some("constructor" | "callback" | "class")
                    )
                {
                    let owner = facts[js.owners[d]];
                    if matches!(owner.kind.as_str(), "module" | "class") {
                        roles.insert(
                            if owner.kind == "class" {
                                "class_method"
                            } else {
                                "module_function"
                            }
                            .into(),
                            vec![facts[*meta].id.clone()],
                        );
                    }
                }
            }
        }
        if !javascript
            && roles.is_empty()
            && !f.symbol.rsplit('.').next().unwrap_or("").starts_with('_')
        {
            if let Some(parent) = parent {
                if matches!(parent.kind.as_str(), "module" | "class") {
                    roles.insert(
                        if parent.kind == "class" {
                            "class_method"
                        } else {
                            "module_function"
                        }
                        .into(),
                        vec![],
                    );
                }
            }
        }
        for (role, ids) in roles {
            let mut entry = item(f, Kind::Workflow, &role);
            entry.evidence_ids.extend(ids);
            entry.label = format!("{} ({role} candidate)", f.symbol);
            items.push(entry);
        }
    }
    let mut identities = BTreeMap::new();
    for entry in &mut items {
        entry.evidence_ids.sort();
        entry.evidence_ids.dedup();
        let bytes = serde_json::to_vec(entry).map_err(|e| e.to_string())?;
        let digest = format!("{:x}", Sha256::digest(&bytes));
        let id = format!("{}_{}", entry.kind.name(), &digest[..16]);
        if let Some(previous) = identities.insert(id.clone(), bytes.clone()) {
            if previous != bytes {
                return Err("catalog item ID collision".into());
            }
            return Err("duplicate catalog record from distinct source facts".into());
        }
        entry.id = id;
    }
    // Stable kind/path/symbol/method/route/ID order, independent of input order.
    items.sort_by(|a, b| {
        (
            a.kind.rank(),
            &a.path,
            &a.symbol,
            &a.method,
            &a.route_path,
            &a.id,
        )
            .cmp(&(
                b.kind.rank(),
                &b.path,
                &b.symbol,
                &b.method,
                &b.route_path,
                &b.id,
            ))
    });
    if items.is_empty() {
        warnings
            .insert("No eligible catalog items were found in the selected source scope.".into());
    }
    Ok(Catalog {
        schema_version: VERSION.into(),
        source_root: source_root.into(),
        snapshot_id: evidence.snapshot_id.clone(),
        extractor_version: evidence.extractor_version.clone(),
        complete: evidence.complete,
        items,
        warnings: warnings.into_iter().collect(),
    })
}

fn relative_path(path: &str) -> Result<String, String> {
    let path = path.replace('\\', "/");
    if path.starts_with('/')
        || path.contains(':')
        || path.chars().any(char::is_control)
        || path.split('/').any(|part| part == "..")
    {
        return Err(
            "path must be a root-relative prefix without traversal or an absolute path".into(),
        );
    }
    Ok(path
        .split('/')
        .filter(|p| !p.is_empty() && *p != ".")
        .collect::<Vec<_>>()
        .join("/"))
}

pub fn page(catalog: &Catalog, filter: &Filter) -> Result<Page, String> {
    if !(1..=100).contains(&filter.limit) {
        return Err("limit must be in 1..=100".into());
    }
    let prefix = filter.path.as_deref().map(relative_path).transpose()?;
    let query = filter.query.as_deref().map(str::to_lowercase);
    let mut counts: BTreeMap<String, usize> = ["api", "workflow", "class"]
        .into_iter()
        .map(|k| (k.into(), 0))
        .collect();
    let mut matched = Vec::new();
    for entry in &catalog.items {
        *counts.entry(entry.kind.name().into()).or_default() += 1;
        if filter.kind.as_ref().is_some_and(|k| k != &entry.kind) {
            continue;
        }
        if let Some(prefix) = &prefix {
            if !relative_path(&entry.path)?.starts_with(prefix) {
                continue;
            }
        }
        if let Some(query) = &query {
            if ![
                Some(entry.label.as_str()),
                Some(&entry.symbol),
                Some(&entry.path),
                entry.method.as_deref(),
                entry.route_path.as_deref(),
            ]
            .into_iter()
            .flatten()
            .any(|field| field.to_lowercase().contains(query))
            {
                continue;
            }
        }
        matched.push(entry);
    }
    let matched_items = matched.len();
    let start = filter.offset.min(matched_items);
    let end = start.saturating_add(filter.limit).min(matched_items);
    Ok(Page {
        snapshot_id: catalog.snapshot_id.clone(),
        complete: catalog.complete,
        total_items: catalog.items.len(),
        matched_items,
        offset: filter.offset,
        limit: filter.limit,
        next_offset: (end < matched_items).then_some(end),
        counts,
        items: matched[start..end]
            .iter()
            .map(|entry| (*entry).clone())
            .collect(),
        warnings: catalog.warnings.clone(),
    })
}

pub fn resolve<'a>(catalog: &'a Catalog, selection: &str) -> Result<&'a Item, String> {
    if let Some(entry) = catalog.items.iter().find(|entry| entry.id == selection) {
        return Ok(entry);
    }
    let mut candidates: Vec<_> = catalog
        .items
        .iter()
        .filter(|entry| entry.symbol == selection)
        .collect();
    if candidates.is_empty() && !selection.is_empty() {
        let suffix = format!(".{selection}");
        candidates = catalog
            .items
            .iter()
            .filter(|entry| entry.symbol.ends_with(&suffix))
            .collect();
    }
    if candidates.len() == 1 {
        return Ok(candidates[0]);
    }
    if candidates.is_empty() {
        return Err("no catalog candidate matches the selection; choose an item ID".into());
    }
    candidates.sort_by_key(|entry| &entry.id);
    let ids = candidates
        .iter()
        .take(10)
        .map(|entry| entry.id.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    Err(format!(
        "ambiguous catalog selection ({} candidates); choose an explicit item ID: {ids}{}",
        candidates.len(),
        if candidates.len() > 10 { ", ..." } else { "" }
    ))
}

// Encode source text as inert text, including punctuation that could create
// Markdown links, raw HTML, or automatic URLs. Bound each displayed field.
fn text(value: &str) -> String {
    let mut out = String::new();
    let mut chars = value.chars().peekable();
    let mut count = 0;
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            continue;
        }
        if count == 512 {
            out.push_str("...");
            break;
        }
        count += 1;
        if c.is_control() || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}') {
            out.push(' ');
        } else if matches!(c, '&' | '<' | '>' | ':' | '@') {
            out.push_str(&format!("&#{};", c as u32));
        } else {
            if matches!(
                c,
                '\\' | '`' | '*' | '_' | '[' | ']' | '(' | ')' | '#' | '!' | '|' | '~'
            ) {
                out.push('\\');
            }
            out.push(c);
        }
    }
    out
}

pub fn markdown(page: &Page, catalog_directory: &str) -> String {
    let shown = page.items.len().min(100);
    let start = if shown == 0 {
        0
    } else {
        page.offset.saturating_add(1)
    };
    let end = if shown == 0 {
        0
    } else {
        page.offset.saturating_add(shown)
    };
    let mut out = format!("# Source Catalog\n\nStatic discovery candidates only. No agent used; no runtime endpoints or activation verified.\n\nCatalog directory: {}\n\nSnapshot: {}\n\nSource complete: {} (not discovery exhaustiveness)\n\nTotal: {}; matched: {}; current range: {}-{}; limit: {}; nextOffset: {}\n\n",
        text(catalog_directory), text(&page.snapshot_id), page.complete, page.total_items,
        page.matched_items, start, end, page.limit, page.next_offset.map(|n| n.to_string()).unwrap_or_else(|| "none".into()));
    for entry in page.items.iter().take(100) {
        out.push_str(&format!(
            "- **{}**: {}\n  ID: {}; role: {}; source: {}:{}-{}\n",
            entry.kind.name(),
            text(&entry.label),
            if entry
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
            {
                entry.id.clone()
            } else {
                text(&entry.id)
            },
            text(&entry.role),
            text(&entry.path),
            entry.line,
            entry.end_line
        ));
        if entry.kind == Kind::Api {
            out.push_str(&format!(
                "  Method: {}; path FRAGMENT: {} (prefix and activation unresolved)\n",
                text(entry.method.as_deref().unwrap_or("unknown")),
                text(entry.route_path.as_deref().unwrap_or("dynamic"))
            ));
        }
    }
    if shown == 0 {
        out.push_str("No candidates on this page.\n");
    }
    out.push_str("\nFor a source-grounded diagram, select an explicit catalog item ID in the appropriate mode. Symbols shared by routes or candidate kinds require an ID; no target is guessed.\n\n");
    for warning in page.warnings.iter().take(100) {
        out.push_str(&format!("Warning: {}\n\n", text(warning)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fact(id: &str, kind: &str, symbol: &str, line: usize, end_line: usize, value: &str) -> Fact {
        Fact {
            id: id.into(),
            kind: kind.into(),
            path: "pkg/app.py".into(),
            line,
            end_line,
            symbol: symbol.into(),
            value: value.into(),
        }
    }

    fn evidence(facts: Vec<Fact>) -> Evidence {
        Evidence {
            schema_version: "ideomancer.evidence.v1".into(),
            snapshot_id: "snapshot".into(),
            extractor_version: "synthetic".into(),
            python_version: "synthetic".into(),
            language: None,
            files: vec![],
            facts,
            diagnostics: vec![],
            complete: true,
        }
    }

    fn sample() -> Evidence {
        evidence(vec![
            fact("module", "module", "pkg.app", 1, 1, ""),
            fact("route", "api_route", "pkg.app._handler", 2, 2, &json!({"methods":["GET","POST"],"path":"/items","receiver":"router","pathIsFragment":true}).to_string()),
            fact("dynamic", "api_route", "pkg.app._handler", 3, 3, &json!({"methods":[],"path":null,"receiver":"router","pathIsFragment":true}).to_string()),
            fact("handler", "async_function", "pkg.app._handler", 4, 10, "SECRET_IGNORED"),
            fact("nested", "function", "pkg.app._handler.nested", 5, 6, ""),
            fact("private", "function", "pkg.app._private", 11, 12, ""),
            fact("public", "function", "pkg.app.public", 13, 14, ""),
            fact("class1", "class", "pkg.app.Service", 20, 30, ""),
            fact("base1", "base", "pkg.app.Service", 20, 20, "SECRET_BASE"),
            fact("method", "function", "pkg.app.Service.run", 21, 25, ""),
            fact("init", "function", "pkg.app.Service.__init__", 26, 29, ""),
            fact("class2", "class", "pkg.app.Service", 40, 50, ""),
            fact("base2", "base", "pkg.app.Service", 40, 40, ""),
            fact("cli", "entrypoint", "pkg.app._cli", 60, 60, r#"{"role":"cli_command","decorator":"app.command"}"#),
            fact("command", "function", "pkg.app._cli", 61, 65, ""),
            fact("job", "entrypoint", "pkg.app._job", 70, 70, r#"{"role":"job","decorator":"task"}"#),
            fact("worker", "function", "pkg.app._job", 71, 75, ""),
            fact("literal", "binding", "pkg.app", 80, 80, "TOP_SECRET_DO_NOT_COPY"),
        ])
    }

    #[test]
    fn javascript_explicit_handlers_and_visibility_do_not_use_python_conventions() {
        let mut e = crate::js_focus::tests::source();
        crate::js_focus::tests::add(
            &mut e,
            "route",
            "api_route",
            "app",
            80,
            80,
            json!({"handlerFactId":"run","methods":["POST"],"path":"/x","pathIsFragment":true}),
        );
        let c = build(&e, "local").unwrap();
        assert!(c
            .items
            .iter()
            .any(|i| i.kind == Kind::Api && i.fact_id == "run"));
        assert!(c
            .items
            .iter()
            .any(|i| i.kind == Kind::Workflow && i.fact_id == "work"));
        let f = e.facts.iter_mut().find(|f| f.id == "work-meta").unwrap();
        let mut v: serde_json::Value = serde_json::from_str(&f.value).unwrap();
        v["visibility"] = json!("protected");
        f.value = v.to_string();
        assert!(!build(&e, "local")
            .unwrap()
            .items
            .iter()
            .any(|i| i.fact_id == "work"));
        let route = e.facts.iter_mut().find(|f| f.id == "route").unwrap();
        let mut v: serde_json::Value = serde_json::from_str(&route.value).unwrap();
        v["handlerFactId"] = json!("module");
        route.value = v.to_string();
        assert!(!build(&e, "local")
            .unwrap()
            .items
            .iter()
            .any(|i| i.kind == Kind::Api));
    }

    #[test]
    fn discovers_routes_roles_and_scoped_classes_without_literals() {
        let catalog = build(&sample(), "/source").unwrap();
        let apis: Vec<_> = catalog
            .items
            .iter()
            .filter(|i| i.kind == Kind::Api)
            .collect();
        assert_eq!(apis.len(), 3);
        assert!(apis
            .iter()
            .all(|i| i.fact_id == "handler" && i.path_is_fragment && i.evidence_ids.len() == 2));
        assert!(apis.iter().any(|i| i.method.as_deref() == Some("POST")));
        assert!(apis
            .iter()
            .any(|i| i.method.is_none() && i.route_path.is_none() && i.label.contains("dynamic")));
        let workflows: Vec<_> = catalog
            .items
            .iter()
            .filter(|i| i.kind == Kind::Workflow)
            .collect();
        assert_eq!(workflows.len(), 5);
        for role in [
            "http_handler",
            "cli_command",
            "job",
            "module_function",
            "class_method",
        ] {
            assert!(workflows.iter().any(|i| i.role == role));
        }
        assert!(!workflows
            .iter()
            .any(|i| ["nested", "private", "init"].contains(&i.fact_id.as_str())));
        for (id, base, other) in [("class1", "base1", "base2"), ("class2", "base2", "base1")] {
            let class = catalog.items.iter().find(|i| i.fact_id == id).unwrap();
            assert!(class.evidence_ids.contains(&base.into()));
            assert!(!class.evidence_ids.contains(&other.into()));
        }
        assert!(!serde_json::to_string(&catalog).unwrap().contains("SECRET"));
        assert!(catalog
            .warnings
            .iter()
            .any(|w| w.contains("dynamic") || w.contains("Dynamic")));
    }

    #[test]
    fn stable_permutations_duplicates_and_conflicts() {
        let mut source = sample();
        let first = build(&source, "root").unwrap();
        source.facts.reverse();
        source.files.reverse();
        assert_eq!(
            serde_json::to_value(&first).unwrap(),
            serde_json::to_value(build(&source, "root").unwrap()).unwrap()
        );
        source.snapshot_id = "different_snapshot".into();
        assert_eq!(
            serde_json::to_value(&first.items).unwrap(),
            serde_json::to_value(build(&source, "another_root").unwrap().items).unwrap()
        );
        source
            .facts
            .iter_mut()
            .find(|f| f.id == "public")
            .unwrap()
            .id = "changed_source_location_id".into();
        let changed = build(&source, "root").unwrap();
        let old = first.items.iter().find(|i| i.fact_id == "public").unwrap();
        let new = changed
            .items
            .iter()
            .find(|i| i.fact_id == "changed_source_location_id")
            .unwrap();
        assert_ne!(old.id, new.id);
        source
            .facts
            .push(fact("module", "module", "pkg.app", 1, 1, ""));
        assert_eq!(
            first.items.len(),
            build(&source, "root").unwrap().items.len()
        );
        source
            .facts
            .push(fact("module", "module", "different", 1, 1, ""));
        assert!(build(&source, "root").is_err());
        for i in first.items {
            assert_eq!(i.id.len(), i.kind.name().len() + 17);
        }
    }

    #[test]
    fn repeated_decorators_attach_to_the_following_instance() {
        let mut source = sample();
        source.facts.extend([
            fact(
                "later_route",
                "api_route",
                "pkg.app._handler",
                90,
                91,
                r#"{"methods":["DELETE"],"path":"/later","pathIsFragment":true}"#,
            ),
            fact("later_handler", "function", "pkg.app._handler", 92, 95, ""),
        ]);
        let c = build(&source, "root").unwrap();
        let later = c
            .items
            .iter()
            .find(|i| i.kind == Kind::Api && i.method.as_deref() == Some("DELETE"))
            .unwrap();
        assert_eq!(later.fact_id, "later_handler");
        assert!(later.evidence_ids.contains(&"later_route".into()));
        assert!(!later.evidence_ids.contains(&"route".into()));
    }

    #[test]
    fn filters_counts_literal_queries_and_overflow() {
        let c = build(&sample(), "root").unwrap();
        assert_eq!(Filter::default().limit, 20);
        let mut f = Filter {
            kind: Some(Kind::Api),
            query: Some("pOsT".into()),
            path: Some("pkg\\app".into()),
            ..Filter::default()
        };
        let p = page(&c, &f).unwrap();
        assert_eq!(p.matched_items, 1);
        assert_eq!(p.counts["api"], 3);
        assert_eq!(p.total_items, c.items.len());
        f.query = Some(".*".into());
        assert_eq!(page(&c, &f).unwrap().matched_items, 0);
        f.query = None;
        f.offset = usize::MAX;
        let p = page(&c, &f).unwrap();
        assert!(p.items.is_empty());
        assert_eq!(p.next_offset, None);
        for path in [
            "/tmp",
            "../pkg",
            "pkg/../app",
            "C:\\app",
            "\\\\server\\share",
        ] {
            f.path = Some(path.into());
            assert!(page(&c, &f).is_err());
        }
        f.path = None;
        for limit in [0, 101, usize::MAX] {
            f.limit = limit;
            assert!(page(&c, &f).is_err());
        }
        f = Filter {
            limit: 1,
            ..Filter::default()
        };
        let first = page(&c, &f).unwrap();
        assert_eq!(first.next_offset, Some(1));
        f.offset = c.items.len() - 1;
        let last = page(&c, &f).unwrap();
        assert_eq!(last.next_offset, None);
        assert_eq!(first.counts, last.counts);
        assert_eq!(first.warnings, last.warnings);
        assert_eq!(first.complete, last.complete);
        let mut incomplete = c.clone();
        incomplete.complete = false;
        assert!(!page(&incomplete, &f).unwrap().complete);
    }

    #[test]
    fn resolves_only_unambiguous_targets_and_explicit_ids() {
        let c = build(&sample(), "root").unwrap();
        assert_eq!(resolve(&c, "public").unwrap().fact_id, "public");
        assert_eq!(resolve(&c, "app.Service.run").unwrap().fact_id, "method");
        for name in ["_handler", "pkg.app._handler", "Service"] {
            let error = resolve(&c, name).unwrap_err();
            assert!(error.contains("ambiguous") && error.contains("explicit item ID"));
        }
        assert!(resolve(&c, "handler").is_err());
        assert!(resolve(&c, "").is_err());
        for i in &c.items {
            assert_eq!(resolve(&c, &i.id).unwrap().id, i.id);
        }
        let mut many = c.clone();
        many.items = (0..50)
            .map(|n| {
                let mut i = c.items[0].clone();
                i.id = format!("api_{n:016}");
                i
            })
            .collect();
        let error = resolve(&many, &many.items[0].symbol).unwrap_err();
        assert!(error.contains("50 candidates") && error.contains("..."));
        assert!(!error.contains(&many.items[49].id));
    }

    #[test]
    fn empty_schema_strict_types_and_inert_bounded_markdown() {
        let empty = build(&evidence(vec![]), "root").unwrap();
        assert!(empty.items.is_empty() && empty.warnings.iter().any(|w| w.contains("No eligible")));
        let mut e = evidence(vec![]);
        e.schema_version = "other".into();
        assert!(build(&e, "root").is_err());
        let mut c = build(&sample(), "root").unwrap();
        let mut value = serde_json::to_value(&c).unwrap();
        value["unknown"] = json!(true);
        assert!(serde_json::from_value::<Catalog>(value).is_err());
        let malicious =
            "<script>alert(1)</script> [x](javascript:bad) https://bad.test \u{1b}[31mRED\n# title";
        c.items[0].label = malicious.into();
        c.items[0].path = malicious.into();
        let mut p = page(&c, &Filter::default()).unwrap();
        p.warnings.push(malicious.into());
        let rendered = markdown(&p, malicious);
        for unsafe_text in [
            "<script>",
            "[x](",
            "https://",
            "javascript:",
            "\u{1b}",
            "\n# title",
        ] {
            assert!(!rendered.contains(unsafe_text));
        }
        assert!(
            rendered.contains("path FRAGMENT")
                && rendered.contains("No agent used")
                && rendered.contains("nextOffset")
        );
        p.items = (0..150).map(|_| c.items[0].clone()).collect();
        assert_eq!(markdown(&p, "root").matches("  ID:").count(), 100);
    }
}
