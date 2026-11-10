//! Complete saved-snapshot class inventory. No filesystem reads or runtime inference.
use super::source_view::{self, escape_label, html_text, source, Documents};
use crate::analysis::Evidence;
use crate::catalog::{Catalog, Kind};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

const MAX_DOCUMENTS: usize = 8 * 1024 * 1024;
const MAX_FOLDER_NODES: usize = 24;
const LIMIT_ERROR: &str =
    "complete class overview exceeds the combined 8 MiB document limit; no classes were truncated";
const SCOPE_WARNING: &str = "The fixed scan scope prefers src/ when present and excludes scripts, tests, shadow and operator directories (.git, .venv, venv, __pycache__, node_modules, .ideomancer, caches, .cache, .pytest_cache, .mypy_cache, .ruff_cache, build, dist, migrations, alembic, env, data, db). JavaScript also excludes dotfolders, bower_components, assets, vendor, generated, coverage, init, config, configs, environments and *.spec.*, *.test.*, *.min.js, *.bundle.js, *.d.ts files. This is completeness within the saved catalog, not every class in the repository. No render-time exclusions are applied: every class already in the catalog is listed.";

fn relative_path(path: &str) -> Result<String, String> {
    let normalized = path.replace('\\', "/");
    if normalized.is_empty()
        || normalized.starts_with('/')
        || normalized.contains(':')
        || normalized.chars().any(char::is_control)
        || normalized.split('/').any(|part| part == "..")
    {
        return Err("class overview requires root-relative source paths without traversal".into());
    }
    let parts: Vec<_> = normalized
        .split('/')
        .filter(|s| !s.is_empty() && *s != ".")
        .collect();
    if parts.is_empty() {
        return Err("class overview source path has no filename".into());
    }
    Ok(parts.join("/"))
}

fn references(refs: &Value) -> String {
    let mut html = String::from("<ul>");
    if let Some(refs) = refs.as_array() {
        for r in refs {
            html.push_str(&format!(
                "<li><code>{}:{}-{}</code> ({})</li>",
                html_text(r["path"].as_str().unwrap_or("UNKNOWN")),
                r["line"],
                r["endLine"],
                html_text(r["kind"].as_str().unwrap_or("UNKNOWN"))
            ));
        }
    }
    html.push_str("</ul>");
    html
}

fn member_table(title: &str, members: &[Value]) -> String {
    let mut html = format!("<h4>{title} ({})</h4>", members.len());
    if members.is_empty() {
        html.push_str("<p>None observed in this snapshot.</p>");
        return html;
    }
    html.push_str("<div class=\"table-scroll\"><table><thead><tr><th>Name / Full symbol</th><th>Kind</th><th>Visibility</th><th>Static</th><th>Type</th><th>SourceRefs</th></tr></thead><tbody>");
    for m in members {
        let static_text = match m["static"].as_bool() {
            Some(true) => "yes",
            Some(false) => "no",
            None => "UNKNOWN",
        };
        html.push_str(&format!("<tr><td><code>{}</code><br><code>{}</code>{}</td><td>{}</td><td>{}</td><td>{static_text}</td><td><code>{}</code></td><td>{}</td></tr>",
            html_text(m["name"].as_str().unwrap_or("UNKNOWN")), html_text(m["symbol"].as_str().unwrap_or("UNKNOWN")),
            if m["signature"].is_object() { format!("<pre>{}</pre><small>Declared signature; default values intentionally absent.</small>", html_text(&source_view::signature_text(m))) } else { String::new() },
            html_text(m["memberKind"].as_str().unwrap_or("UNKNOWN")), html_text(m["visibility"].as_str().unwrap_or("UNKNOWN")),
            html_text(&m["type"].as_str().map(str::to_owned).unwrap_or_else(|| m["type"].to_string())), references(&m["sourceRefs"])));
    }
    html.push_str("</tbody></table></div>");
    html
}

pub fn build(catalog: &Catalog, evidence: &Evidence) -> Result<Documents, String> {
    if catalog.schema_version != crate::catalog::VERSION
        || catalog.snapshot_id != evidence.snapshot_id
        || catalog.extractor_version != evidence.extractor_version
        || catalog.complete != evidence.complete
    {
        return Err("class overview catalog and evidence metadata do not match".into());
    }
    // One grounding pass for the entire inventory, not one focused page per class.
    let grounded = crate::catalog::build(evidence, &catalog.source_root)?;
    let expected: BTreeMap<_, _> = grounded
        .items
        .iter()
        .filter(|i| i.kind == Kind::Class)
        .map(|i| (i.id.as_str(), i))
        .collect();
    let mut inventory = BTreeMap::new();
    for item in catalog.items.iter().filter(|i| i.kind == Kind::Class) {
        if !item.id.starts_with("class_")
            || !item
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
            || inventory.insert(item.id.as_str(), item).is_some()
            || expected.get(item.id.as_str()).is_none_or(|fresh| {
                serde_json::to_value(fresh).ok() != serde_json::to_value(item).ok()
            })
        {
            return Err("class overview has duplicate, unsafe, unknown or non-source-grounded catalog classes".into());
        }
    }
    if inventory.len() != expected.len() {
        return Err(
            "class overview catalog does not contain the exact freshly grounded class inventory"
                .into(),
        );
    }
    let by_id: BTreeMap<_, _> = evidence.facts.iter().map(|f| (f.id.as_str(), f)).collect();
    let facts: Vec<_> = by_id.values().copied().collect();
    for fact in &facts {
        relative_path(&fact.path)?;
        if fact.line == 0 || fact.end_line < fact.line {
            return Err("class overview evidence has invalid source line bounds".into());
        }
        if !evidence.files.is_empty() && !evidence.files.iter().any(|file| file.path == fact.path) {
            return Err("class overview source file provenance missing".into());
        }
    }
    let js = if evidence.is_javascript() {
        Some(crate::js_focus::Index::new(facts.clone())?)
    } else {
        None
    };
    let mut warnings: BTreeSet<_> = catalog
        .warnings
        .iter()
        .chain(&grounded.warnings)
        .cloned()
        .collect();
    warnings.insert(SCOPE_WARNING.into());
    warnings.insert("Static snapshot source organization only, not runtime architecture. Folder coexistence establishes no dependency edges. Keep source metadata local.".into());
    warnings.insert("Only directly owned observed members are listed, not inherited or dynamically created members. Python visibility and static/type information are UNKNOWN unless evidenced; self/cls bindings do not establish composition or injection.".into());
    if js.is_some() {
        warnings.insert("JavaScript members use explicit owner IDs and declared visibility. JavaScript base expressions remain unresolved boundaries; no inheritance, prototype or DI relationship is inferred.".into());
    }
    if !catalog.complete {
        warnings.insert("INCOMPLETE SNAPSHOT: extraction was incomplete; classes and members may be missing even though every saved catalog class is listed.".into());
    }

    let class_ids: BTreeMap<_, _> = inventory
        .values()
        .map(|i| (i.fact_id.as_str(), i.id.as_str()))
        .collect();
    let mut items: Vec<_> = inventory.values().copied().collect();
    items.sort_by_key(|i| (&i.path, &i.symbol, i.line, &i.id));
    let mut entries = Vec::new();
    let mut folders: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut semantic_bytes = 0usize;
    for item in items {
        let fact = by_id[item.fact_id.as_str()];
        let path = relative_path(&item.path)?;
        let folder = path
            .rsplit_once('/')
            .map_or(".", |(folder, _)| folder)
            .to_string();
        let mut entry = serde_json::to_value(item).map_err(|e| e.to_string())?;
        entry["anchor"] = json!(format!("class_{}", item.id));
        entry["sourceRefs"] = json!(item
            .evidence_ids
            .iter()
            .map(|id| source(by_id[id.as_str()]))
            .collect::<Vec<_>>());
        entry["sourceFactIds"] = json!(item.evidence_ids);
        entry["sourceModel"] = json!("static-candidate");
        entry["confidence"] = json!("source-syntax-only");
        entry["folder"] = json!(folder);
        let (methods, attributes) = if let Some(js) = &js {
            let mut methods = Vec::new();
            let mut attributes = Vec::new();
            for mut m in source_view::javascript_members(fact, js, &by_id)? {
                if !m["symbol"].is_string() {
                    m["symbol"] = m["sourceRefs"]
                        .as_array()
                        .and_then(|refs| refs.last())
                        .map(|r| r["symbol"].clone())
                        .unwrap_or(Value::Null);
                }
                if m["memberKind"] == "field" {
                    attributes.push(m);
                } else {
                    methods.push(m);
                }
            }
            (methods, attributes)
        } else {
            let (methods, attributes) = source_view::python_members(fact, &facts);
            let methods = methods.into_iter().map(|m| json!({"name":m.symbol.rsplit('.').next().unwrap_or(&m.symbol),
                "symbol":m.symbol,"memberKind":m.kind,"visibility":"UNKNOWN (Python naming is not access control)",
                "static":null,"type":"UNKNOWN","parameters":"UNKNOWN","sourceRefs":[source(m)],"sourceFactIds":[m.id]})).collect();
            let attributes = attributes.into_iter().map(|(name, bindings)| json!({"symbol":format!("{}.{name}",fact.symbol),
                "name":name,"memberKind":"observed self/cls binding","visibility":"UNKNOWN (Python naming is not access control)",
                "static":null,"type":"UNKNOWN","sourceRefs":bindings.iter().map(|b|source(b)).collect::<Vec<_>>(),
                "sourceFactIds":bindings.iter().map(|b|&b.id).collect::<Vec<_>>()})).collect();
            (methods, attributes)
        };
        let mut bases = Vec::new();
        for base in source_view::declared_bases(fact, &facts) {
            let resolved = if js.is_none() {
                source_view::resolve_base(base, fact, &facts)
            } else {
                None
            };
            let mut refs = vec![source(base)];
            let mut ids = vec![base.id.clone()];
            let mut target_id = None;
            let mut target_symbol = None;
            let status = if let Some((target, imports)) = resolved {
                refs.push(source(target));
                ids.push(target.id.clone());
                for import in imports {
                    refs.push(source(import));
                    ids.push(import.id.clone());
                }
                target_id = class_ids.get(target.id.as_str()).copied();
                target_symbol = Some(target.symbol.as_str());
                if target_id.is_some() {
                    "resolved catalog declaration"
                } else {
                    "resolved declaration outside catalog"
                }
            } else {
                "unresolved boundary"
            };
            bases.push(
                json!({"expression":base.value,"status":status,"targetClassId":target_id,
                "targetSymbol":target_symbol,"sourceRefs":refs,"sourceFactIds":ids}),
            );
        }
        entry["methods"] = json!(methods);
        entry["attributes"] = json!(attributes);
        entry["declaredBases"] = json!(bases);
        entry["unresolvedInherits"] = json!(bases
            .iter()
            .filter(|b| b["status"] == "unresolved boundary")
            .collect::<Vec<_>>());
        entry["methodCount"] = json!(methods.len());
        entry["fieldCount"] = json!(attributes.len());
        semantic_bytes += serde_json::to_vec(&entry).map_err(|e| e.to_string())?.len();
        if semantic_bytes > MAX_DOCUMENTS {
            return Err(LIMIT_ERROR.into());
        }
        folders.entry(folder).or_default().push(entries.len());
        entries.push(entry);
    }
    let aggregated_folders = if folders.len() > MAX_FOLDER_NODES {
        folders.len() - (MAX_FOLDER_NODES - 1)
    } else {
        0
    };
    let individual_nodes = folders.len() - aggregated_folders;
    let mut mermaid =
        String::from("---\ntitle: Catalog classes by source folder\n---\nflowchart TB\n");
    let mut groups = Vec::new();
    let mut aggregated_classes = 0;
    for (index, (path, indices)) in folders.iter().enumerate() {
        let aggregated = index >= individual_nodes;
        let diagram_id = format!(
            "F{:03}",
            if aggregated {
                MAX_FOLDER_NODES
            } else {
                index + 1
            }
        );
        if aggregated {
            aggregated_classes += indices.len();
        } else {
            mermaid.push_str(&format!(
                "  {diagram_id}[\"{}\"]\n",
                escape_label(&format!("{path}\n{} classes", indices.len())).replace('$', "#36;")
            ));
        }
        groups.push(json!({"path":path,"classIds":indices.iter().map(|i|&entries[*i]["id"]).collect::<Vec<_>>(),
            "count":indices.len(),"diagramId":diagram_id,"diagramAggregated":aggregated}));
    }
    if aggregated_folders > 0 {
        mermaid.push_str(&format!("  F024[\"Other folders: {aggregated_folders} folders, {aggregated_classes} classes\"]\n"));
    }
    if folders.is_empty() {
        mermaid.push_str("  E000[\"No catalog classes\"]\n");
    }
    let folder_nodes = individual_nodes + usize::from(aggregated_folders > 0);
    let legend = json!({"schemaVersion":"ideomancer.class-overview.v1","kind":"class","scope":"all-catalog-classes",
        "sourceModel":"static-candidate","snapshotId":catalog.snapshot_id,"sourceRoot":catalog.source_root,
        "extractorVersion":catalog.extractor_version,"complete":catalog.complete,
        "snapshot":{"id":catalog.snapshot_id,"extractorVersion":catalog.extractor_version,"complete":catalog.complete},
        "entries":entries,"groups":groups,"edges":[],"warnings":warnings,
        "coverage":{"catalogClasses":inventory.len(),"listedClasses":entries.len(),"folderCount":folders.len(),
            "diagramFolderNodes":folder_nodes,"diagramNodes":if folders.is_empty(){1}else{folder_nodes},
            "aggregatedFolders":aggregated_folders,"aggregatedClasses":aggregated_classes,
            "allCatalogClassesListed":true,"grounding":"exact fresh catalog class inventory", "renderTimeExclusions":false},
        "layout":{"maxFolderNodes":MAX_FOLDER_NODES,"meaning":"source organization only; no dependency edges"}});
    let legend_text = serde_json::to_string_pretty(&legend).map_err(|e| e.to_string())?;
    let mut body = format!("<h1>All Catalog Classes</h1><p><strong>{} catalog classes / {} listed</strong> in {} source folders.</p><p>Snapshot: <code>{}</code>. Extractor: <code>{}</code>.</p>",
        inventory.len(), entries.len(), folders.len(), html_text(&catalog.snapshot_id), html_text(&catalog.extractor_version));
    if !catalog.complete {
        body.push_str("<p class=\"incomplete\" role=\"alert\"><strong>INCOMPLETE SNAPSHOT: classes and members may be missing. All saved catalog classes are listed, not all repository classes.</strong></p>");
    }
    body.push_str(&format!("<p>{}</p><h2>Source Organization Only</h2><p>The diagram groups containing source folders and class counts, not runtime architecture or dependencies. The complete class list and all observed members follow below, including any folders aggregated as Other folders.</p><figure><img src=\"diagram.svg\" alt=\"Class counts by source folder\"><figcaption>Source organization only. No dependency edges. If the image is unavailable, the full inventory below remains usable without SVG or JavaScript.</figcaption></figure><h2>All Classes</h2><p>Use browser Find to locate classes or members. Expand a class for its full observed members and declaration bases.</p><p>Focus command template: replace <code>SAVEDDIR</code> with the directory containing the saved catalog and evidence, not the target source directory. The source root is not a saved-directory copy target. Commands are plain text, never executable source links.</p>", html_text(SCOPE_WARNING)));
    for (folder, indices) in &folders {
        body.push_str(&format!(
            "<section><h3>{} ({} classes)</h3>",
            html_text(folder),
            indices.len()
        ));
        for index in indices {
            let e = &entries[*index];
            let id = e["id"].as_str().unwrap();
            let symbol = e["symbol"].as_str().unwrap();
            body.push_str(&format!("<details id=\"{}\" data-class-id=\"{}\"><summary>{} &#183; {}:{} &#183; {} methods / {} fields</summary><p>Full symbol: <code>{}</code></p><p>Catalog ID: <code>{}</code></p><pre>ideomancer diagram SAVEDDIR --kind class --entry '{}'</pre><h4>Declaration SourceRefs</h4>{}",
                html_text(e["anchor"].as_str().unwrap()), html_text(id), html_text(symbol.rsplit('.').next().unwrap_or(symbol)),
                html_text(e["path"].as_str().unwrap()), e["line"], e["methodCount"], e["fieldCount"], html_text(symbol), html_text(id), html_text(id), references(&e["sourceRefs"])));
            body.push_str(&member_table("Methods", e["methods"].as_array().unwrap()));
            body.push_str(&member_table("Fields", e["attributes"].as_array().unwrap()));
            body.push_str("<h4>Declared Bases</h4>");
            let bases = e["declaredBases"].as_array().unwrap();
            if bases.is_empty() {
                body.push_str("<p>None declared in the saved evidence.</p>");
            } else {
                body.push_str("<div class=\"table-scroll\"><table><thead><tr><th>Expression</th><th>Status</th><th>Resolved full symbol</th><th>SourceRefs</th></tr></thead><tbody>");
                for base in bases {
                    body.push_str(&format!("<tr><td><code>{}</code></td><td>{}</td><td><code>{}</code></td><td>{}</td></tr>",
                        html_text(base["expression"].as_str().unwrap()), html_text(base["status"].as_str().unwrap()),
                        html_text(base["targetSymbol"].as_str().unwrap_or("UNKNOWN")), references(&base["sourceRefs"])));
                }
                body.push_str("</tbody></table></div>");
            }
            body.push_str("</details>");
            if body.len() + legend_text.len() + mermaid.len() > MAX_DOCUMENTS {
                return Err(LIMIT_ERROR.into());
            }
        }
        body.push_str("</section>");
    }
    if entries.is_empty() {
        body.push_str("<p>No classes in the saved catalog.</p>");
    }
    body.push_str("<h2>Snapshot Boundaries</h2><ul>");
    for warning in &warnings {
        body.push_str(&format!("<li>{}</li>", html_text(warning)));
    }
    body.push_str("</ul><p><a href=\"legend.json\">Full JSON metadata and source evidence references</a>. The legend preserves all fact IDs, full symbols and source references; readable citations above show source locations and fact kinds.</p>");
    // Native HTML needs no new controller and remains fully usable without a diagram.
    let html = format!("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; img-src 'self'; style-src 'unsafe-inline'; object-src 'none'; base-uri 'none'; form-action 'none'\"><title>All Catalog Classes</title><style>body{{font:16px/1.5 system-ui,sans-serif;max-width:100rem;margin:2rem auto;padding:0 1rem;color:#20252b;background:#fafbfc}}summary{{cursor:pointer;padding:.7rem;font-weight:650;overflow-wrap:anywhere}}details{{border:1px solid #bbc5cf;border-radius:.3rem;margin:.6rem 0;padding:.3rem .7rem}}h3{{border-bottom:2px solid #536779}}code,pre{{overflow-wrap:anywhere;white-space:pre-wrap}}table{{border-collapse:collapse;width:100%;font-size:.9rem}}th,td{{text-align:left;vertical-align:top;border:1px solid #bbc5cf;padding:.5rem;overflow-wrap:anywhere}}.table-scroll{{overflow-x:auto}}td ul{{padding-left:1rem}}img{{max-width:100%;height:auto}}.incomplete{{border:3px solid #a23b16;background:#fff1df;padding:1rem}}@media(max-width:600px){{body{{margin:1rem auto}}details{{padding:.2rem}}th,td{{min-width:7rem}}}}</style></head><body>{body}</body></html>");
    if html.len() + legend_text.len() + mermaid.len() > MAX_DOCUMENTS {
        return Err(LIMIT_ERROR.into());
    }
    Ok(Documents {
        mermaid,
        legend,
        html,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::{Fact, SourceFile};

    fn fact(
        id: &str,
        kind: &str,
        path: &str,
        symbol: &str,
        line: usize,
        end_line: usize,
        value: &str,
    ) -> Fact {
        Fact {
            id: id.into(),
            kind: kind.into(),
            path: path.into(),
            symbol: symbol.into(),
            line,
            end_line,
            value: value.into(),
        }
    }

    fn evidence(count: usize, folder_count: usize) -> Evidence {
        let mut e = Evidence {
            schema_version: "ideomancer.evidence.v1".into(),
            snapshot_id: "saved-snapshot".into(),
            extractor_version: "synthetic".into(),
            python_version: "synthetic".into(),
            complete: true,
            ..Evidence::default()
        };
        for n in 0..count {
            let path = if folder_count == 0 {
                "app.py".into()
            } else {
                format!("folder{:03}/app.py", n % folder_count)
            };
            let symbol = format!("app.Class{n:03}");
            e.facts.push(fact(
                &format!("class{n}"),
                "class",
                &path,
                &symbol,
                n * 100 + 2,
                n * 100 + 99,
                "",
            ));
            if !e.files.iter().any(|f| f.path == path) {
                e.files.push(SourceFile {
                    path,
                    sha256: "f".repeat(64),
                });
            }
        }
        e
    }

    fn documents(e: &Evidence) -> Documents {
        build(
            &crate::catalog::build(e, "/not/a/saved/directory").unwrap(),
            e,
        )
        .unwrap()
    }

    #[test]
    fn all_88_classes_once_even_with_more_than_30_in_one_folder() {
        let e = evidence(88, 1);
        let c = crate::catalog::build(&e, "/source").unwrap();
        let d = build(&c, &e).unwrap();
        assert_eq!(d.legend["coverage"]["catalogClasses"], 88);
        assert_eq!(d.legend["coverage"]["listedClasses"], 88);
        assert_eq!(d.legend["coverage"]["allCatalogClassesListed"], true);
        assert_eq!(d.legend["entries"].as_array().unwrap().len(), 88);
        assert_eq!(d.html.matches("data-class-id=").count(), 88);
        for item in &c.items {
            assert_eq!(
                d.html
                    .matches(&format!(
                        "<details id=\"class_{}\" data-class-id=\"{}\">",
                        item.id, item.id
                    ))
                    .count(),
                1
            );
            assert!(d.html.contains(&format!("--entry '{}'", item.id)));
        }
        assert_eq!(d.mermaid.matches("[\"").count(), 1);
        assert!(d.mermaid.contains("88 classes"));
        assert!(!d.mermaid.contains("Class000"));
        assert!(!d.mermaid.contains("-->"));
        assert!(d.html.contains("88 catalog classes / 88 listed"));
        assert!(!d.html.contains("../class/index.html"));
        assert!(!d.html.contains("<script"));
        assert!(d.html.contains("SAVEDDIR"));
    }

    #[test]
    fn folder_overflow_is_counted_not_truncated() {
        let e = evidence(88, 40);
        let d = documents(&e);
        assert_eq!(d.legend["coverage"]["folderCount"], 40);
        assert_eq!(d.legend["coverage"]["diagramFolderNodes"], 24);
        assert_eq!(d.legend["coverage"]["aggregatedFolders"], 17);
        assert_eq!(d.mermaid.matches("[\"").count(), 24);
        assert_eq!(d.html.matches("<section><h3>").count(), 40);
        assert!(d.mermaid.contains("Other folders: 17 folders"));
        let groups = d.legend["groups"].as_array().unwrap();
        let total: u64 = groups.iter().map(|g| g["count"].as_u64().unwrap()).sum();
        let aggregated: u64 = groups
            .iter()
            .filter(|g| g["diagramAggregated"] == true)
            .map(|g| g["count"].as_u64().unwrap())
            .sum();
        assert_eq!(total, 88);
        assert_eq!(d.legend["coverage"]["aggregatedClasses"], aggregated);
        assert!(d.mermaid.contains(&format!("{aggregated} classes")));
        let d = documents(&evidence(24, 24));
        assert_eq!(d.legend["coverage"]["aggregatedFolders"], 0);
        assert_eq!(d.mermaid.matches("[\"").count(), 24);
    }

    #[test]
    fn empty_and_incomplete_snapshots_are_valid_and_honest() {
        let mut e = evidence(0, 0);
        e.complete = false;
        let d = documents(&e);
        assert_eq!(d.legend["coverage"]["catalogClasses"], 0);
        assert_eq!(d.legend["coverage"]["listedClasses"], 0);
        assert_eq!(d.legend["coverage"]["folderCount"], 0);
        assert_eq!(d.legend["coverage"]["diagramFolderNodes"], 0);
        assert_eq!(d.legend["coverage"]["allCatalogClassesListed"], true);
        assert!(d.html.contains("role=\"alert\"") && d.html.contains("INCOMPLETE SNAPSHOT"));
        assert!(d.html.contains("No classes in the saved catalog"));
        for scope in ["scripts", "tests", ".ideomancer", "operator directories"] {
            assert!(d.html.contains(scope));
        }
    }

    #[test]
    fn all_observed_python_members_and_conservative_bases_are_readable() {
        let mut e = evidence(1, 0);
        e.facts
            .push(fact("module", "module", "app.py", "app", 1, 1, ""));
        e.facts
            .push(fact("parent", "class", "app.py", "app.Parent", 1, 1, ""));
        e.facts.push(fact(
            "base",
            "base",
            "app.py",
            "app.Class000",
            2,
            2,
            "Parent",
        ));
        e.facts.push(fact(
            "unknown",
            "base",
            "app.py",
            "app.Class000",
            2,
            2,
            "External[Type]",
        ));
        for n in 0..16 {
            let symbol = format!("app.Class000.method{n:02}");
            e.facts.push(fact(
                &format!("method{n}"),
                "function",
                "app.py",
                &symbol,
                3 + n * 3,
                5 + n * 3,
                "",
            ));
            e.facts.push(fact(
                &format!("binding{n}"),
                "binding",
                "app.py",
                &symbol,
                4 + n * 3,
                4 + n * 3,
                &json!({"target":format!("self.field{n:02}")}).to_string(),
            ));
        }
        let d = documents(&e);
        let entry = d.legend["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["factId"] == "class0")
            .unwrap();
        assert_eq!(entry["methodCount"], 16);
        assert_eq!(entry["fieldCount"], 16);
        assert!(d.html.contains("<code>method15</code>"));
        assert!(d.html.contains("<code>field15</code>"));
        assert!(d.html.contains("<h3>. (2 classes)</h3>"));
        let bases = entry["declaredBases"].as_array().unwrap();
        assert!(bases
            .iter()
            .any(|b| b["status"] == "resolved catalog declaration"
                && b["targetSymbol"] == "app.Parent"));
        assert_eq!(entry["unresolvedInherits"].as_array().unwrap().len(), 1);
        assert!(d.html.contains("unresolved boundary"));
        assert!(d.legend["edges"].as_array().unwrap().is_empty());
    }

    #[test]
    fn javascript_owners_visibility_static_types_and_uncapped_members() {
        use crate::js_focus::tests::{add, source};
        let mut e = source();
        e.facts
            .iter_mut()
            .find(|f| f.id == "module")
            .unwrap()
            .end_line = 200;
        for (id, name, line) in [("class", "$Service", 50), ("other", "Other", 100)] {
            add(
                &mut e,
                id,
                "class",
                &format!("app.{name}"),
                line,
                line + 49,
                json!(name),
            );
            add(
                &mut e,
                &format!("{id}-meta"),
                "js_declaration",
                &format!("app.{name}"),
                line,
                line + 49,
                json!({"declarationFactId":id,"ownerFactId":"module","kind":"class","name":name,"publicCandidate":true,"visibility":"public"}),
            );
        }
        add(
            &mut e,
            "base",
            "base",
            "app.$Service",
            50,
            50,
            json!("Other"),
        );
        for n in 0..15 {
            let name = format!("$method{n:02}");
            let symbol = format!("app.$Service.{name}");
            let id = format!("method{n}");
            add(
                &mut e,
                &id,
                "function",
                &symbol,
                51 + n,
                51 + n,
                json!(name),
            );
            add(
                &mut e,
                &format!("decl{n}"),
                "js_declaration",
                &symbol,
                51 + n,
                51 + n,
                json!({"declarationFactId":id,"ownerFactId":"class","kind":"method","name":name,"publicCandidate":true,"visibility":"public"}),
            );
            add(
                &mut e,
                &format!("member{n}"),
                "js_member",
                &symbol,
                51 + n,
                51 + n,
                json!({"declarationFactId":id,"ownerFactId":"class","memberKind":"method","name":name,"visibility":"public","static":true}),
            );
        }
        for n in 0..10 {
            add(
                &mut e,
                &format!("field{n}"),
                "js_member",
                "app.$Service",
                80 + n,
                80 + n,
                json!({"ownerFactId":"class","memberKind":"field","name":format!("#field{n:02}"),"visibility":"private","static":false,"type":"DeclaredType"}),
            );
        }
        add(
            &mut e,
            "other-field",
            "js_member",
            "app.Other",
            101,
            101,
            json!({"ownerFactId":"other","memberKind":"field","name":"onlyOther","visibility":"protected","static":false}),
        );
        let d = documents(&e);
        let entry = d.legend["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["factId"] == "class")
            .unwrap();
        assert_eq!(entry["methodCount"], 15);
        assert_eq!(entry["fieldCount"], 10);
        assert!(entry["methods"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["static"] == true && m["visibility"] == "public"));
        assert!(entry["attributes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["type"] == "DeclaredType" && m["visibility"] == "private"));
        assert_eq!(entry["declaredBases"][0]["status"], "unresolved boundary");
        assert!(entry["declaredBases"][0]["targetClassId"].is_null());
        assert!(
            d.html.contains("<code>$method14</code>") && d.html.contains("<code>#field09</code>")
        );
        assert!(d.html.contains("<td>protected</td>"));
        let mut permuted = e;
        permuted.facts.reverse();
        let again = documents(&permuted);
        assert_eq!(d.html, again.html);
        assert_eq!(d.legend, again.legend);
    }

    #[test]
    fn hostile_labels_are_inert_and_saved_directory_is_never_source_root() {
        let mut e = evidence(1, 0);
        let hostile = "<svg onload='bad()'><script>bad()</script>&\"";
        e.facts[0].symbol = format!("app.{hostile}");
        e.facts[0].path = "scripts/<img onerror='bad()'>/app.py".into();
        e.files[0].path = e.facts[0].path.clone();
        let d = documents(&e);
        assert!(d.html.contains("&lt;svg onload=&#39;bad()&#39;&gt;"));
        assert!(!d.html.contains("<svg") && !d.html.contains("<script"));
        assert_eq!(d.html.matches("href=").count(), 1);
        assert!(d.html.contains("href=\"legend.json\""));
        assert!(!d.html.contains("diagram /not/a/saved/directory"));
        assert!(d.mermaid.contains("#60;img"));
        assert_eq!(d.legend["coverage"]["listedClasses"], 1);
        assert_eq!(
            d.legend["groups"][0]["path"],
            "scripts/<img onerror='bad()'>"
        );
    }

    #[test]
    fn grounding_rejects_omissions_duplicates_tampering_and_conflicting_facts() {
        let mut e = evidence(2, 1);
        let mut c = crate::catalog::build(&e, "/source").unwrap();
        c.items.pop();
        assert!(build(&c, &e).is_err());
        let mut c = crate::catalog::build(&e, "/source").unwrap();
        c.items.push(c.items[0].clone());
        assert!(build(&c, &e).is_err());
        let mut c = crate::catalog::build(&e, "/source").unwrap();
        c.items[0].symbol = "tampered".into();
        assert!(build(&c, &e).is_err());
        let mut c = crate::catalog::build(&e, "/source").unwrap();
        c.items[0].id = "class_' unsafe".into();
        assert!(build(&c, &e).is_err());
        let c = crate::catalog::build(&e, "/source").unwrap();
        let mut duplicate: Fact =
            serde_json::from_value(serde_json::to_value(&e.facts[0]).unwrap()).unwrap();
        duplicate.symbol = "conflict".into();
        e.facts.push(duplicate);
        assert!(build(&c, &e).err().unwrap().contains("conflicting"));
    }

    #[test]
    fn stable_permutations_and_relative_path_prefix_invariants() {
        let mut e = evidence(12, 2);
        e.facts[0].path = "src/app.py".into();
        e.facts[1].path = "src-extra/app.py".into();
        e.files.push(SourceFile {
            path: "src/app.py".into(),
            sha256: "f".repeat(64),
        });
        e.files.push(SourceFile {
            path: "src-extra/app.py".into(),
            sha256: "f".repeat(64),
        });
        let c = crate::catalog::build(&e, "/source/src").unwrap();
        let d = build(&c, &e).unwrap();
        let paths: BTreeSet<_> = d.legend["groups"]
            .as_array()
            .unwrap()
            .iter()
            .map(|g| g["path"].as_str().unwrap())
            .collect();
        assert!(paths.contains("src") && paths.contains("src-extra"));
        let mut shuffled = c.clone();
        shuffled.items.reverse();
        e.facts.reverse();
        e.files.reverse();
        let again = build(&shuffled, &e).unwrap();
        assert_eq!(d.html, again.html);
        assert_eq!(d.legend, again.legend);
        assert_eq!(d.mermaid, again.mermaid);
        shuffled.source_root = "/completely/different/prefix".into();
        let moved = build(&shuffled, &e).unwrap();
        assert_eq!(d.legend["entries"], moved.legend["entries"]);
        assert_eq!(d.legend["groups"], moved.legend["groups"]);
        for invalid in [
            "../outside.py",
            "/absolute.py",
            "C:\\outside.py",
            "a/../b.py",
        ] {
            assert!(relative_path(invalid).is_err());
        }
        assert_eq!(relative_path("pkg\\app.py").unwrap(), "pkg/app.py");
        assert_eq!(relative_path("./app.py").unwrap(), "app.py");
    }

    #[test]
    fn large_inventory_fits_without_duplicating_json_or_omitting_members() {
        let mut e = evidence(473, 6);
        for n in 0..473 {
            let class = &e.facts[n];
            let path = class.path.clone();
            let symbol = class.symbol.clone();
            for m in 0..4 {
                let method = format!("{symbol}.method{m}");
                let line = n * 100 + 10 + m * 3;
                e.facts.push(fact(
                    &format!("fact-method-{n}-{m}"),
                    "function",
                    &path,
                    &method,
                    line,
                    line + 2,
                    "",
                ));
                e.facts.push(fact(
                    &format!("fact-binding-{n}-{m}"),
                    "binding",
                    &path,
                    &method,
                    line + 1,
                    line + 1,
                    &json!({"target":format!("self.field{m}")}).to_string(),
                ));
            }
        }
        let d = documents(&e);
        let legend_text = serde_json::to_string_pretty(&d.legend).unwrap();
        let combined = d.html.len() + legend_text.len() + d.mermaid.len();
        assert!(combined <= MAX_DOCUMENTS);
        assert!(combined + html_text(&legend_text).len() > MAX_DOCUMENTS);
        assert!(!d.html.contains("&quot;sourceRefs&quot;"));
        assert!(d.html.contains("href=\"legend.json\""));
        assert_eq!(d.html.matches("data-class-id=").count(), 473);
        assert_eq!(d.legend["coverage"]["listedClasses"], 473);
        assert!(!d.html.contains("fact-method-") && !d.html.contains("fact-binding-"));
        let html_codes: BTreeSet<_> = d
            .html
            .split("<code>")
            .skip(1)
            .filter_map(|s| s.split_once("</code>").map(|(code, _)| code))
            .collect();
        let facts: BTreeMap<_, _> = e.facts.iter().map(|f| (f.id.as_str(), f)).collect();
        for entry in d.legend["entries"].as_array().unwrap() {
            assert_eq!(entry["methodCount"], 4);
            assert_eq!(entry["fieldCount"], 4);
            for member in entry["methods"]
                .as_array()
                .unwrap()
                .iter()
                .chain(entry["attributes"].as_array().unwrap())
            {
                assert!(html_codes.contains(html_text(member["symbol"].as_str().unwrap()).as_str()));
                for reference in member["sourceRefs"].as_array().unwrap() {
                    let id = reference["factId"].as_str().unwrap();
                    assert!(member["sourceFactIds"]
                        .as_array()
                        .unwrap()
                        .contains(&json!(id)));
                    assert_eq!(*reference, source(facts[id]));
                    assert!(html_codes.contains(
                        format!(
                            "{}:{}-{}",
                            reference["path"].as_str().unwrap(),
                            reference["line"],
                            reference["endLine"]
                        )
                        .as_str()
                    ));
                }
            }
        }
    }

    #[test]
    fn combined_document_limit_fails_clearly_without_truncation() {
        let mut e = evidence(1, 0);
        // JSON and readable HTML together exceed the budget, even though each alone is smaller.
        e.facts[0].symbol = format!("app.{}", "x".repeat(2_000_000));
        let c = crate::catalog::build(&e, "/source").unwrap();
        let result = build(&c, &e);
        assert!(result.is_err());
        let error = result.err().unwrap();
        assert!(error.contains("combined 8 MiB") && error.contains("no classes were truncated"));
    }
}
