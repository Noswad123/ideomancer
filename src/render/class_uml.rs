//! Bounded, source-grounded UML documents built only from saved snapshot evidence.
use super::source_view::{html_text, source};
use super::viewer;
use crate::analysis::{Evidence, Fact};
use crate::catalog::{Catalog, Kind, Page};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

const MAX_BYTES: usize = 8 * 1024 * 1024;
const LIMIT: &str = "class UML exceeds the combined 8 MiB HTML, legend and Mermaid limit; no classes were truncated";
type Edges = BTreeMap<(String, String, bool), (BTreeSet<String>, Vec<usize>)>;
type ClassScopes<'a> = BTreeMap<String, (&'a Fact, Value)>;

pub struct Diagram {
    pub id: String,
    pub title: String,
    pub mermaid: String,
}

pub struct Bundle {
    pub diagrams: Vec<Diagram>,
    pub legend: Value,
    pub html: String,
}

// Encode source punctuation as Mermaid entities, including syntax meaningful in
// member declarations. Only renderer-owned punctuation can become UML syntax.
fn label(s: &str) -> String {
    s.chars()
        .take(120)
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, ' ' | '_' | '.' | '/') {
                c.to_string()
            } else if c.is_control() {
                " ".into()
            } else {
                format!("#{};", c as u32)
            }
        })
        .collect()
}

fn name(e: &Value) -> String {
    let symbol = e["symbol"].as_str().unwrap_or("UNKNOWN");
    let short = symbol.rsplit('.').next().unwrap_or(symbol);
    if short.contains("anonymous")
        || short.starts_with('<')
        || short.strip_prefix("class_L").is_some_and(|s| {
            s.split_once("_C")
                .is_some_and(|(l, c)| l.parse::<usize>().is_ok() && c.parse::<usize>().is_ok())
        })
    {
        format!(
            "{} - anonymous class alias (not declared name)",
            e["path"].as_str().unwrap().rsplit('/').next().unwrap()
        )
    } else {
        short.to_string()
    }
}

fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 512
        && s.split('.').all(|part| {
            // Preserve the trusted parser's exact Unicode representation. Rust's
            // alphabetic predicates are not ECMAScript IdentifierStart/Continue.
            !part.is_empty()
                && !part.as_bytes()[0].is_ascii_digit()
                && part.chars().all(|c| {
                    !c.is_control()
                        && !c.is_whitespace()
                        && (!c.is_ascii() || c.is_ascii_alphanumeric() || matches!(c, '_' | '$'))
                })
        })
}

fn within(outer: &Fact, inner: &Fact) -> bool {
    outer.path == inner.path && outer.line <= inner.line && inner.end_line <= outer.end_line
}

fn boundary_key(r: &Value) -> String {
    format!(
        "{}|{}|{}",
        r["classId"],
        r["bindingGuardFactId"],
        if r["bindingGuardFactId"].is_null() {
            r["expression"].to_string()
        } else {
            String::new()
        }
    )
}

fn class_scopes<'a>(
    by_id: &BTreeMap<&str, &'a Fact>,
    policy: &str,
) -> Result<ClassScopes<'a>, String> {
    let invalid = || "invalid JavaScript class scope graph".to_string();
    let mut scopes = ClassScopes::new();
    let mut roots = BTreeSet::new();
    for fact in by_id.values().filter(|f| f.kind == "js_class_scope") {
        let v: Value = serde_json::from_str(&fact.value).map_err(|_| invalid())?;
        let id = v["scopeId"]
            .as_str()
            .filter(|id| !id.is_empty() && id.len() <= 4096 && !id.chars().any(char::is_control))
            .ok_or_else(invalid)?;
        if v["policy"] != policy
            || !matches!(
                v["scopeKind"].as_str(),
                Some("module" | "function" | "class" | "block")
            )
            || !matches!((v["start"].as_u64(), v["end"].as_u64()), (Some(a), Some(b)) if a < b && b <= 1024*1024)
            || !v.get("parentScopeId").is_some_and(|p| {
                p.is_null()
                    || p.as_str().is_some_and(|id| {
                        !id.is_empty() && id.len() <= 4096 && !id.chars().any(char::is_control)
                    })
            })
            || (v["parentScopeId"].is_null() != (v["scopeKind"] == "module"))
            || (v["parentScopeId"].is_null() && !roots.insert(fact.path.as_str()))
        {
            return Err(invalid());
        }
        if scopes.insert(id.to_string(), (*fact, v)).is_some() {
            return Err(invalid());
        }
    }
    for id in scopes.keys() {
        let mut current = id.as_str();
        let mut seen = BTreeSet::new();
        loop {
            if !seen.insert(current) || seen.len() > 256 {
                return Err(invalid());
            }
            let (fact, v) = scopes.get(current).ok_or_else(invalid)?;
            let Some(parent) = v["parentScopeId"].as_str() else {
                break;
            };
            let (parent_fact, p) = scopes.get(parent).ok_or_else(invalid)?;
            if !within(parent_fact, fact)
                || p["start"].as_u64().unwrap() > v["start"].as_u64().unwrap()
                || v["end"].as_u64().unwrap() > p["end"].as_u64().unwrap()
            {
                return Err(invalid());
            }
            current = parent;
        }
    }
    Ok(scopes)
}

/// Validate the entire receipt before promoting any portion to a relationship.
fn relation(
    fact: &Fact,
    by_id: &BTreeMap<&str, &Fact>,
    js: &crate::js_focus::Index<'_>,
    graph: &ClassScopes<'_>,
    policy: &str,
) -> Result<Value, String> {
    let invalid = || format!("invalid JavaScript class relation proof: {}", fact.id);
    let v: Value = serde_json::from_str(&fact.value).map_err(|_| invalid())?;
    let keys = [
        "policy",
        "classFactId",
        "ownerFactId",
        "relationship",
        "expression",
        "bindingKind",
        "bindingGuardFactId",
        "targetFactId",
        "memberName",
        "declaredType",
        "supportingFactIds",
        "stableBinding",
        "lookupScopeIds",
        "referenceFactId",
    ];
    if !v.is_object()
        || keys.iter().any(|k| v.get(k).is_none())
        || v["policy"] != policy
        || (policy == "javascript-class-relations-2" && v.get("importFactId").is_none())
        || !matches!(
            v["relationship"].as_str(),
            Some("extends" | "stores_parameter" | "uses_factory_binding" | "creates")
        )
        || !matches!(
            v["bindingKind"].as_str(),
            Some(
                "local_class"
                    | "factory_parameter"
                    | "constructor_parameter"
                    | "import"
                    | "unresolved"
            )
        )
        || v["expression"].as_str().is_none_or(str::is_empty)
        || v["stableBinding"].as_bool().is_none()
        || !(v["memberName"].is_null() || v["memberName"].as_str().is_some_and(identifier))
        || !(v["declaredType"].is_null() || v["declaredType"].is_string())
    {
        return Err(invalid());
    }
    let get = |key: &str| {
        v[key]
            .as_str()
            .and_then(|id| by_id.get(id).copied())
            .ok_or_else(invalid)
    };
    let class = get("classFactId")?;
    let owner = get("ownerFactId")?;
    if class.kind != "class"
        || !within(class, fact)
        || !(owner.kind == "class" || crate::js_focus::callable(owner))
        || !within(owner, fact)
    {
        return Err(invalid());
    }
    let ci = js
        .facts
        .iter()
        .position(|f| f.id == class.id)
        .ok_or_else(invalid)?;
    let oi = js
        .facts
        .iter()
        .position(|f| f.id == owner.id)
        .ok_or_else(invalid)?;
    let extends = v["relationship"] == "extends";
    let owner_matches = if extends {
        oi == ci
    } else {
        js.owners.get(&oi) == Some(&ci)
            && js.declarations.get(&oi).is_some_and(|(_, d)| {
                if v["relationship"] == "stores_parameter" {
                    d["kind"] == "constructor"
                } else {
                    matches!(d["kind"].as_str(), Some("method" | "constructor"))
                }
            })
    };
    if !owner_matches {
        return Err(invalid());
    }
    let reference = get("referenceFactId")?;
    let rv: Value = serde_json::from_str(&reference.value).map_err(|_| invalid())?;
    if reference.kind != "js_class_reference"
        || !within(fact, reference)
        || !within(class, reference)
        || !within(owner, reference)
        || rv["policy"] != v["policy"]
        || rv["classFactId"] != v["classFactId"]
        || rv["ownerFactId"] != v["ownerFactId"]
        || !matches!((rv["start"].as_u64(),rv["end"].as_u64()),(Some(a),Some(b)) if a < b && b <= 1024*1024)
    {
        return Err(invalid());
    }
    let reference_scope = rv["scopeId"].as_str().ok_or_else(invalid)?;
    let scopes = v["lookupScopeIds"].as_array().ok_or_else(invalid)?;
    let mut seen = BTreeSet::new();
    if scopes.is_empty()
        || scopes.len() > 256
        || scopes.iter().any(|s| {
            s.as_str().is_none_or(|s| {
                s.is_empty() || s.len() > 4096 || s.chars().any(char::is_control) || !seen.insert(s)
            })
        })
    {
        return Err(invalid());
    }
    // Cross-check independent saved facts; this is not cryptographic semantic
    // reproof if an attacker rewrites the graph, reference and receipts together.
    let mut chain = Vec::new();
    let mut scope_fact_ids = Vec::new();
    let mut current = reference_scope;
    loop {
        let (scope_fact, s) = graph.get(current).ok_or_else(invalid)?;
        if scope_fact.path != fact.path
            || s["policy"] != v["policy"]
            || chain.contains(&current)
            || chain.len() == 256
            || !within(scope_fact, reference)
            || s["start"].as_u64().unwrap() > rv["start"].as_u64().unwrap()
            || rv["end"].as_u64().unwrap() > s["end"].as_u64().unwrap()
        {
            return Err(invalid());
        }
        chain.push(current);
        scope_fact_ids.push(scope_fact.id.as_str());
        let Some(parent) = s["parentScopeId"].as_str() else {
            break;
        };
        current = parent;
    }
    if chain
        .iter()
        .copied()
        .ne(scopes.iter().map(|s| s.as_str().unwrap()))
    {
        return Err(invalid());
    }
    for (id, (scope_fact, s)) in graph {
        if scope_fact.path == fact.path
            && s["start"].as_u64().unwrap() <= rv["start"].as_u64().unwrap()
            && rv["end"].as_u64().unwrap() <= s["end"].as_u64().unwrap()
            && !chain.contains(&id.as_str())
        {
            return Err(invalid());
        }
    }
    // The first matching lexical scope is authoritative, not a claimed outer guard.
    let mut guard_scopes = BTreeMap::new();
    for (i, g) in &js.guards {
        if js.facts[*i].path != fact.path {
            continue;
        }
        let scope = g["scopeId"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 4096 && !s.chars().any(char::is_control))
            .ok_or_else(invalid)?;
        let kind = g["scopeKind"]
            .as_str()
            .filter(|k| matches!(*k, "module" | "function" | "class" | "block"))
            .ok_or_else(invalid)?;
        let start = g["scopeStart"].as_u64().ok_or_else(invalid)?;
        let end = g["scopeEnd"].as_u64().ok_or_else(invalid)?;
        if start >= end
            || end > 1024 * 1024
            || g["declarationCount"].as_u64().is_none_or(|n| n == 0)
            || g["writes"].as_u64().is_none()
            || guard_scopes
                .insert(scope, (kind, start, end))
                .is_some_and(|old| old != (kind, start, end))
        {
            return Err(invalid());
        }
        if let Some((scope_fact, s)) = graph.get(scope) {
            if scope_fact.path != fact.path
                || s["scopeKind"] != kind
                || s["start"] != start
                || s["end"] != end
            {
                return Err(invalid());
            }
        } else if start <= rv["start"].as_u64().unwrap() && rv["end"].as_u64().unwrap() <= end {
            return Err(invalid());
        }
    }
    let supporting = v["supportingFactIds"].as_array().ok_or_else(invalid)?;
    if !supporting.contains(&json!(reference.id))
        || scope_fact_ids
            .iter()
            .any(|id| !supporting.contains(&json!(id)))
    {
        return Err(invalid());
    }
    let mut ids = BTreeSet::from([fact.id.as_str(), class.id.as_str(), owner.id.as_str()]);
    for id in supporting {
        let id = id.as_str().ok_or_else(invalid)?;
        let f = by_id.get(id).ok_or_else(invalid)?;
        if f.path != fact.path
            || (f.kind == "js_module_import"
                && (policy != "javascript-class-relations-2" || v["importFactId"] != f.id))
            || !matches!(
                f.kind.as_str(),
                "module"
                    | "js_binding_guard"
                    | "js_declaration"
                    | "class"
                    | "function"
                    | "async_function"
                    | "base"
                    | "js_member"
                    | "js_import"
                    | "js_module_import"
                    | "js_class_reference"
                    | "js_class_scope"
            )
        {
            return Err(invalid());
        }
        ids.insert(id);
    }
    let guard = if v["bindingGuardFactId"].is_null() {
        None
    } else {
        let g = get("bindingGuardFactId")?;
        if g.kind != "js_binding_guard" || g.path != fact.path || !supporting.contains(&json!(g.id))
        {
            return Err(invalid());
        }
        let gv: Value = serde_json::from_str(&g.value).map_err(|_| invalid())?;
        if gv["declarationCount"].as_u64().is_none()
            || gv["writes"].as_u64().is_none()
            || (v["stableBinding"] == true
                && (gv["declarationCount"].as_u64() != Some(1) || gv["writes"].as_u64() != Some(0)))
            || gv["scopeId"].as_str().is_none_or(str::is_empty)
            || !matches!(
                gv["scopeKind"].as_str(),
                Some("module" | "function" | "block" | "class" | "catch" | "function-body")
            )
            || !matches!((gv["scopeStart"].as_u64(), gv["scopeEnd"].as_u64()), (Some(a), Some(b)) if a < b && b <= 1024 * 1024)
            || gv["name"].as_str().is_none_or(|s| !identifier(s))
            || v["expression"].as_str().unwrap().split('.').next() != gv["name"].as_str()
        {
            return Err(invalid());
        }
        let nearest = scopes
            .iter()
            .find_map(|scope| {
                let matching: Vec<_> = js
                    .guards
                    .iter()
                    .filter(|(i, other)| {
                        js.facts[*i].path == fact.path
                            && other["scopeId"] == *scope
                            && other["name"] == gv["name"]
                    })
                    .collect();
                (!matching.is_empty()).then_some(matching)
            })
            .ok_or_else(invalid)?;
        if nearest.len() != 1 || js.facts[nearest[0].0].id != g.id {
            return Err(invalid());
        }
        if matches!(
            v["bindingKind"].as_str(),
            Some("factory_parameter" | "constructor_parameter")
        ) && (gv["bindingKind"] != "parameter"
            || gv["scopeKind"] != "function"
            || !gv["declarationFactId"].is_null())
        {
            return Err(invalid());
        }
        if v["bindingKind"] == "constructor_parameter"
            && (!within(owner, g)
                || !js
                    .declarations
                    .get(&oi)
                    .is_some_and(|(_, d)| d["kind"] == "constructor"))
        {
            return Err(invalid());
        }
        if v["bindingKind"] == "factory_parameter" {
            if g.end_line > class.line {
                return Err(invalid());
            }
            let mut ancestor = ci;
            let mut found = false;
            while let Some(parent) = js.owners.get(&ancestor).copied() {
                if crate::js_focus::callable(js.facts[parent])
                    && within(js.facts[parent], g)
                    && js.declarations.get(&parent).is_some_and(|(_, d)| {
                        !matches!(d["kind"].as_str(), Some("method" | "constructor"))
                    })
                {
                    found = true;
                    break;
                }
                ancestor = parent;
            }
            if !found {
                return Err(invalid());
            }
        }
        Some((g, gv))
    };
    if v["stableBinding"] == true && guard.is_none() {
        return Err(invalid());
    }
    if !matches!(v["bindingKind"].as_str(), Some("unresolved" | "import"))
        && (guard.is_none() || v["stableBinding"] != true)
    {
        return Err(invalid());
    }
    if v["stableBinding"] == false && !extends {
        return Err(invalid());
    }
    if policy == "javascript-class-relations-2" {
        if v["bindingKind"] == "import" && !v["importFactId"].is_null() {
            let import = get("importFactId")?;
            let (guard_fact, gv) = guard.as_ref().ok_or_else(invalid)?;
            let iv: Value = serde_json::from_str(&import.value).map_err(|_| invalid())?;
            let import_owner = iv["ownerFactId"]
                .as_str()
                .and_then(|id| by_id.get(id))
                .ok_or_else(invalid)?;
            if import.kind != "js_module_import"
                || import.path != fact.path
                || import_owner.kind != "module"
                || !within(import_owner, import)
                || !supporting.contains(&json!(import_owner.id))
                || !supporting.contains(&json!(import.id))
                || iv["bindingGuardFactId"] != guard_fact.id
                || iv["local"] != v["expression"]
                || gv["name"] != iv["local"]
                || !matches!(gv["bindingKind"].as_str(), Some("import" | "const"))
                || !gv["declarationFactId"].is_null()
                || !within(guard_fact, import)
                || v["stableBinding"] != true
                || !v["targetFactId"].is_null()
            {
                return Err(invalid());
            }
        } else if !v["importFactId"].is_null() {
            return Err(invalid());
        }
    }
    if !v["targetFactId"].is_null() {
        let target = get("targetFactId")?;
        let (_, gv) = guard.as_ref().ok_or_else(invalid)?;
        let ti = js
            .facts
            .iter()
            .position(|f| f.id == target.id)
            .ok_or_else(invalid)?;
        if target.kind != "class"
            || target.id == class.id
            || target.path != class.path
            || v["bindingKind"] != "local_class"
            || !matches!(
                gv["bindingKind"].as_str(),
                Some("class" | "const" | "let" | "var")
            )
            || (!gv["declarationFactId"].is_null() && gv["declarationFactId"] != target.id)
            || !js
                .declarations
                .get(&ti)
                .is_some_and(|(_, d)| d["kind"] == "class" && d["name"] == gv["name"])
            || !supporting.contains(&json!(target.id))
            || !supporting.contains(&json!(js.facts[js.declarations[&ti].0].id))
            || !within(guard.as_ref().unwrap().0, target)
            || (extends && target.end_line > fact.line)
        {
            return Err(invalid());
        }
        let target_scope = *js.owners.get(&ti).ok_or_else(invalid)?;
        let mut scope = if v["relationship"] == "creates" {
            oi
        } else {
            ci
        };
        while scope != target_scope {
            scope = *js.owners.get(&scope).ok_or_else(invalid)?;
        }
    } else if v["bindingKind"] == "local_class" {
        return Err(invalid());
    }
    if v["relationship"] == "stores_parameter"
        && (!matches!(
            v["bindingKind"].as_str(),
            Some("factory_parameter" | "constructor_parameter")
        ) || v["memberName"].is_null())
    {
        return Err(invalid());
    }
    let source_ids = json!(ids);
    let refs = json!(ids.iter().map(|id| source(by_id[id])).collect::<Vec<_>>());
    let mut out = v;
    out["siteFactId"] = json!(fact.id);
    out["sourceFactIds"] = source_ids;
    out["sourceRefs"] = refs;
    Ok(out)
}

fn class_box(out: &mut String, entry: &Value, context: bool) {
    let id = entry["diagramId"].as_str().unwrap();
    out.push_str(&format!(
        "  class {id}[\"{}{}\"] {{\n",
        if context { "context: " } else { "" },
        label(&name(entry))
    ));
    if !context {
        let fields = entry["attributes"].as_array().unwrap();
        let methods = entry["methods"].as_array().unwrap();
        for field in fields.iter().take(8) {
            let ty = field["type"].as_str().filter(|s| !s.starts_with("UNKNOWN"));
            out.push_str(&format!(
                "    {}{}{}{}\n",
                visibility(field),
                label(field["name"].as_str().unwrap_or("unknown")),
                ty.map(|s| format!(" : {}", label(s))).unwrap_or_default(),
                if field["static"] == true { "$" } else { "" }
            ));
        }
        let mut api: Vec<_> = methods
            .iter()
            .filter(|m| {
                m["memberKind"] == "constructor"
                    || m["name"] == "__init__"
                    || m["visibility"] == "public"
                    || (m["visibility"]
                        .as_str()
                        .is_some_and(|s| s.starts_with("UNKNOWN"))
                        && !m["name"].as_str().unwrap_or("").starts_with('_'))
            })
            .collect();
        api.sort_by_key(|m| {
            let name = m["name"].as_str().unwrap_or("");
            (
                if m["memberKind"] == "constructor" || name == "__init__" {
                    0
                } else if name.starts_with('_') {
                    2
                } else {
                    1
                },
                m["sourceRefs"][0]["line"].as_u64().unwrap_or(u64::MAX),
                name,
            )
        });
        for m in api.iter().take(12) {
            out.push_str(&format!(
                "    {}{}{}\n",
                visibility(m),
                method_label(m),
                if m["static"] == true { "$" } else { "" }
            ));
        }
        out.push_str("  }\n");
        let omitted_methods = methods.len() - api.len().min(12);
        let omitted_fields = fields.len().saturating_sub(8);
        if omitted_methods + omitted_fields > 0 {
            out.push_str(&format!("  note for {id} \"{omitted_methods} more methods / {omitted_fields} more fields in details\"\n"));
        }
    } else {
        out.push_str("  }\n");
    }
}

fn method_label(m: &Value) -> String {
    let parameters = m["parameters"]
        .as_array()
        .map(|params| {
            params
                .iter()
                .map(|p| {
                    format!(
                        "{}{}{}{}",
                        if p["rest"] == true { "..." } else { "" },
                        p["name"].as_str().unwrap_or("pattern"),
                        if p["optional"] == true || p["hasDefault"] == true {
                            "?"
                        } else {
                            ""
                        },
                        p["type"]
                            .as_str()
                            .map(|s| format!(": {s}"))
                            .unwrap_or_default()
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    let generics = m["typeParameters"]
        .as_array()
        .filter(|g| !g.is_empty())
        .map(|g| {
            format!(
                "<{}>",
                g.iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
        .unwrap_or_default();
    // Source punctuation is encoded independently of renderer-owned method syntax.
    format!(
        "{}({}){}",
        label(&format!(
            "{}{generics}",
            m["name"].as_str().unwrap_or("unknown")
        )),
        label(&parameters),
        m["returnType"]
            .as_str()
            .map(|s| format!(" {}", label(s)))
            .unwrap_or_default()
    )
}

fn visibility(m: &Value) -> &'static str {
    match m["visibility"].as_str() {
        Some("private") => "-",
        Some("protected") => "#",
        Some("public") => "+",
        _ => "",
    }
}

// Keep every citation link while serializing each full saved source only once.
fn normalize_sources(
    value: &mut Value,
    sources: &mut BTreeMap<String, Value>,
) -> Result<(), String> {
    match value {
        Value::Object(object) => {
            if object.len() == 6
                && ["factId", "path", "line", "endLine", "symbol", "kind"]
                    .iter()
                    .all(|k| object.contains_key(*k))
            {
                let id = object["factId"]
                    .as_str()
                    .ok_or("invalid UML source citation")?
                    .to_string();
                if let Some(previous) = sources.insert(id.clone(), value.clone()) {
                    if previous != *value {
                        return Err("conflicting UML source citations".into());
                    }
                }
                *value = json!({"factId":id});
            } else {
                for child in object.values_mut() {
                    normalize_sources(child, sources)?;
                }
                for (key, ids_key) in [
                    ("sourceRefs", "sourceFactIds"),
                    ("projectionSourceRefs", "projectionSupportingFactIds"),
                ] {
                    if let (Some(ids), Some(refs)) = (
                        object.get(ids_key).and_then(Value::as_array),
                        object.get(key).and_then(Value::as_array),
                    ) {
                        let indices = refs
                            .iter()
                            .map(|r| {
                                ids.iter()
                                    .position(|id| *id == r["factId"])
                                    .ok_or("UML source citation missing its proof ID")
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        object.insert(key.into(), json!(indices));
                    }
                }
            }
        }
        Value::Array(items) => {
            for child in items {
                normalize_sources(child, sources)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn build(
    catalog: &Catalog,
    evidence: &Evidence,
    selection: Option<&Page>,
) -> Result<Bundle, String> {
    build_augmented(catalog, evidence, selection, None)
}

pub(crate) fn build_augmented(
    catalog: &Catalog,
    evidence: &Evidence,
    selection: Option<&Page>,
    augmentation: Option<&crate::extensions::ValidatedClaimsRef>,
) -> Result<Bundle, String> {
    // This performs exact fresh-catalog grounding, full provenance and JS Index
    // validation once. Its inventory UI is deliberately not part of this bundle.
    let semantic = super::class_overview::build(catalog, evidence)?.legend;
    let mut scan_warnings: Vec<String> = semantic["warnings"].as_array().into_iter().flatten()
        .filter_map(Value::as_str)
        .filter(|s| !s.starts_with("JavaScript members use explicit owner IDs"))
        .map(|s| if selection.is_some() {s.replace("No render-time exclusions are applied: every class already in the catalog is listed.","Selected-class coverage is reported explicitly; this is not an all-catalog selection.")} else {s.to_string()}).collect();
    if evidence.is_javascript()
        && !crate::javascript::supports_class_relations(&evidence.extractor_version)
    {
        scan_warnings.push("Legacy JavaScript evidence has no versioned class relationship capability; declared bases remain boundaries. Rediscover to obtain lexical inheritance and supplied-parameter dependency facts.".into());
    }
    let all = semantic["entries"]
        .as_array()
        .ok_or("missing class semantic entries")?;
    let mut selected = BTreeSet::new();
    if let Some(page) = selection {
        if !(1..=30).contains(&page.items.len())
            || page.limit == 0
            || page.items.len() > page.limit
            || page.snapshot_id != catalog.snapshot_id
            || page.complete != catalog.complete
            || page.total_items != catalog.items.len()
            || page.matched_items < page.items.len()
        {
            return Err("class UML requires a matching selected page of 1..=30 classes".into());
        }
        for item in &page.items {
            if item.kind != Kind::Class
                || !selected.insert(item.id.as_str())
                || !catalog.items.iter().any(|i| {
                    i.id == item.id
                        && serde_json::to_value(i).ok() == serde_json::to_value(item).ok()
                })
            {
                return Err(
                    "class UML selection is duplicate, wrong-kind or not source-grounded".into(),
                );
            }
        }
    } else {
        selected.extend(all.iter().map(|e| e["id"].as_str().unwrap()));
    }
    let mut entries: Vec<Value> = all
        .iter()
        .filter(|e| selected.contains(e["id"].as_str().unwrap()))
        .cloned()
        .collect();
    entries.sort_by_key(|e| {
        (
            e["folder"].as_str().unwrap().to_string(),
            e["path"].as_str().unwrap().to_string(),
            e["symbol"].as_str().unwrap().to_string(),
            e["line"].as_u64(),
            e["id"].as_str().unwrap().to_string(),
        )
    });
    let by_id: BTreeMap<_, _> = evidence.facts.iter().map(|f| (f.id.as_str(), f)).collect();
    let fact_classes: BTreeMap<_, _> = all
        .iter()
        .map(|e| (e["factId"].as_str().unwrap(), e["id"].as_str().unwrap()))
        .collect();
    let mut relations = Vec::new();
    let mut validated = Vec::new();
    if evidence.is_javascript()
        && crate::javascript::supports_class_relations(&evidence.extractor_version)
    {
        let js = crate::js_focus::Index::new(by_id.values().copied().collect())?;
        let modules_enabled =
            crate::javascript::supports_class_module_targets(&evidence.extractor_version);
        let policy = if modules_enabled {
            "javascript-class-relations-2"
        } else {
            "javascript-class-relations-1"
        };
        let scopes = class_scopes(&by_id, policy)?;
        let modules = crate::js_modules::Modules::new(&js, evidence);
        for fact in by_id.values().filter(|f| f.kind == "js_class_relation") {
            let mut r = relation(fact, &by_id, &js, &scopes, policy)?;
            if modules_enabled {
                let mut status = if r["targetFactId"].is_null() {
                    "unresolved"
                } else {
                    "resolved"
                };
                let mut reason = if status == "resolved" {
                    "validated same-file class declaration".to_string()
                } else {
                    "unsupported or unresolved class binding".to_string()
                };
                if r["bindingKind"] == "import" && !r["importFactId"].is_null() {
                    let index_of = |id: &Value| {
                        js.facts
                            .iter()
                            .position(|f| Some(f.id.as_str()) == id.as_str())
                            .ok_or("missing validated class relation fact".to_string())
                    };
                    let source = js.facts.iter().position(|f| f.id == fact.id).unwrap();
                    match modules.resolve_class(
                        &js,
                        source,
                        index_of(&r["importFactId"])?,
                        index_of(&r["bindingGuardFactId"])?,
                        r["expression"].as_str().unwrap(),
                        &r["lookupScopeIds"],
                    ) {
                        Ok((target, proof)) => {
                            r["targetFactId"] = json!(js.facts[target].id);
                            let mut ids: BTreeSet<String> = r["sourceFactIds"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|v| v.as_str().unwrap().to_string())
                                .collect();
                            ids.insert(js.facts[target].id.clone());
                            ids.extend(proof.iter().map(|i| js.facts[*i].id.clone()));
                            r["sourceRefs"] = json!(ids
                                .iter()
                                .map(|id| super::source_view::source(by_id[id.as_str()]))
                                .collect::<Vec<_>>());
                            r["sourceFactIds"] = json!(ids);
                            status = "resolved";
                            reason = "validated selected relative named class export".into();
                        }
                        Err(boundary) => reason = boundary,
                    }
                }
                r["resolution"] = json!({"policy":"selected-relative-classes-1","status":status,"reason":reason,"snapshotInventoryDependency":"whole saved selected-file module inventory"});
            }
            validated.push(r);
        }
    }
    for (i, e) in entries.iter_mut().enumerate() {
        e["diagramId"] = json!(format!("C{:03}", i + 1));
        let receipts: Vec<_> = validated
            .iter()
            .filter(|r| r["classFactId"] == e["factId"])
            .collect();
        for r in &receipts {
            if r["relationship"] == "stores_parameter" {
                let field = r["memberName"].as_str().unwrap();
                if !e["attributes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|m| m["name"] == field && m["static"] == false)
                {
                    let symbol = format!("{}.{}", e["symbol"].as_str().unwrap(), field);
                    e["attributes"].as_array_mut().unwrap().push(json!({"name":field,"symbol":symbol,"memberKind":"observed constructor store","visibility":"UNKNOWN","static":false,"type":r["declaredType"],"sourceFactIds":r["sourceFactIds"],"sourceRefs":r["sourceRefs"]}));
                } else {
                    let m = e["attributes"]
                        .as_array_mut()
                        .unwrap()
                        .iter_mut()
                        .find(|m| m["name"] == field && m["static"] == false)
                        .unwrap();
                    if (m["type"].is_null()
                        || m["type"].as_str().is_some_and(|t| t.starts_with("UNKNOWN")))
                        && r["declaredType"].is_string()
                    {
                        m["type"] = r["declaredType"].clone();
                    }
                    for (id, reference) in r["sourceFactIds"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .zip(r["sourceRefs"].as_array().unwrap())
                    {
                        if !m["sourceFactIds"].as_array().unwrap().contains(id) {
                            m["sourceFactIds"].as_array_mut().unwrap().push(id.clone());
                            m["sourceRefs"]
                                .as_array_mut()
                                .unwrap()
                                .push(reference.clone());
                        }
                    }
                }
            }
            let target = r["targetFactId"]
                .as_str()
                .and_then(|f| fact_classes.get(f))
                .copied();
            let mut r = (*r).clone();
            r["classId"] = e["id"].clone();
            r["targetClassId"] = json!(target);
            relations.push(r);
        }
        for base in e["declaredBases"].as_array().unwrap() {
            let expression = base["expression"].as_str().unwrap_or("UNKNOWN");
            if receipts.iter().any(|r| {
                r["relationship"] == "extends"
                    && (r["expression"] == expression
                        || serde_json::from_str::<Value>(expression).ok().as_ref()
                            == Some(&r["expression"]))
            }) {
                continue;
            }
            let base_fact = base["sourceFactIds"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|id| id.as_str().and_then(|id| by_id.get(id).copied()))
                .find(|f| f.kind == "base")
                .ok_or("class UML base has no source fact")?;
            let owners: Vec<_> = by_id
                .values()
                .filter(|f| {
                    f.kind == "class" && f.symbol == base_fact.symbol && within(f, base_fact)
                })
                .collect();
            if owners.len() != 1 || owners[0].id != e["factId"].as_str().unwrap() {
                scan_warnings.push(format!("Ambiguous declaration-base ownership at {}:{}-{}; fallback base {} omitted for {}. Class-specific relation receipts remain authoritative.",base_fact.path,base_fact.line,base_fact.end_line,base_fact.id,e["factId"].as_str().unwrap()));
                continue;
            }
            relations.push(json!({"siteFactId":base_fact.id,"classId":e["id"],"classFactId":e["factId"],"ownerFactId":e["factId"],"relationship":"extends","expression":expression,"targetClassId":base["targetClassId"],"bindingKind":"unresolved","bindingGuardFactId":null,"memberName":null,"declaredType":null,"stableBinding":false,"sourceFactIds":base["sourceFactIds"],"sourceRefs":base["sourceRefs"]}));
        }
        e["fieldCount"] = json!(e["attributes"].as_array().unwrap().len());
        for (index, field) in e["attributes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .enumerate()
        {
            field["visible"] = json!(index < 8);
        }
        e["visibleFieldCount"] = json!(e["fieldCount"].as_u64().unwrap().min(8));
        e["omittedFieldCount"] = json!(e["fieldCount"].as_u64().unwrap().saturating_sub(8));
        // Replace the inventory's legacy unresolved-base assessment only after
        // validating the complete relation receipt, not from names or annotations.
        let bases: Vec<_> = relations.iter().filter(|r|r["classId"] == e["id"] && r["relationship"] == "extends")
            .map(|r|json!({"expression":r["expression"],"targetClassId":r["targetClassId"],"status":if r["targetClassId"].is_null(){"unresolved boundary"}else{"resolved catalog declaration"},"sourceFactIds":r["sourceFactIds"],"sourceRefs":r["sourceRefs"]})).collect();
        e["unresolvedInherits"] = json!(bases
            .iter()
            .filter(|b| b["targetClassId"].is_null())
            .collect::<Vec<_>>());
        e["declaredBases"] = json!(bases);
    }
    if let Some(claims) = augmentation {
        crate::extensions::compose(claims, &mut relations)?;
    }
    let js = if evidence.is_javascript() {
        Some(crate::js_focus::Index::new(
            by_id.values().copied().collect(),
        )?)
    } else {
        None
    };
    for i in 0..relations.len() {
        let r = &relations[i];
        let mut represented = false;
        let mut usage = BTreeSet::new();
        if r["relationship"] == "stores_parameter" {
            let ty = r["declaredType"].as_str().or_else(|| {
                entries
                    .iter()
                    .find(|e| e["id"] == r["classId"])
                    .and_then(|e| {
                        e["attributes"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|m| m["name"] == r["memberName"] && m["static"] == false)
                    })
                    .and_then(|m| m["type"].as_str())
            });
            let primitive = ty.is_some_and(|t| {
                matches!(
                    t.trim(),
                    "string" | "number" | "boolean" | "bigint" | "symbol" | "null" | "undefined"
                ) || (evidence.extractor_version
                    == crate::javascript::CLASS_RELATION_EXTRACTOR_VERSION
                    && matches!(
                        t,
                        "<StringKeyword>"
                            | "<NumberKeyword>"
                            | "<BooleanKeyword>"
                            | "<BigIntKeyword>"
                            | "<SymbolKeyword>"
                            | "<NullKeyword>"
                            | "<UndefinedKeyword>"
                    ))
            });
            let named = ty.is_some_and(|t| {
                !t.starts_with("UNKNOWN")
                    && !matches!(
                        t.trim(),
                        "any" | "unknown" | "object" | "void" | "never" | "this"
                    )
                    && identifier(t.split('<').next().unwrap_or("").trim())
                    && !primitive
            });
            let explicit = relations.iter().any(|other| {
                other["classId"] == r["classId"]
                    && other["bindingGuardFactId"] == r["bindingGuardFactId"]
                    && !r["bindingGuardFactId"].is_null()
                    && matches!(
                        other["relationship"].as_str(),
                        Some("creates" | "uses_factory_binding")
                    )
            });
            if let Some(js) = &js {
                let receiver = format!("this.{}", r["memberName"].as_str().unwrap());
                for (call, (meta, cv)) in &js.calls {
                    let Some(owner) = js.owners.get(call) else {
                        continue;
                    };
                    if js.owners.get(owner).is_some_and(|ci| {
                        js.facts[*ci].id == r["classFactId"].as_str().unwrap_or("")
                    }) && js
                        .declarations
                        .get(owner)
                        .is_some_and(|(_, d)| d["kind"] == "method")
                        && js.members.iter().any(|(_, _, m)| {
                            m["declarationFactId"] == js.facts[*owner].id && m["static"] == false
                        })
                        && cv["callee"].as_str().is_some_and(|c| {
                            c == receiver || c.starts_with(&format!("{receiver}."))
                        })
                    {
                        usage.insert(js.facts[*call].id.clone());
                        usage.insert(js.facts[*meta].id.clone());
                        usage.insert(js.facts[*owner].id.clone());
                        usage.insert(js.facts[js.declarations[owner].0].id.clone());
                    }
                }
            }
            represented = primitive
                || (r["targetClassId"].is_null() && !named && !explicit && usage.is_empty());
        }
        let r = &mut relations[i];
        r["representedAsField"] = json!(represented);
        if represented {
            let field = entries
                .iter()
                .find(|e| e["id"] == r["classId"])
                .and_then(|e| {
                    e["attributes"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|m| m["name"] == r["memberName"] && m["static"] == false)
                })
                .ok_or("projected class relation has no instance field")?;
            r["representedAsFieldVisible"] = field["visible"].clone();
        }
        if r["relationship"] == "stores_parameter" {
            r["projectionReason"] = json!(if represented {
                "storage observation represented as a field; not a claim that no dependency exists"
            } else {
                "identified class, named declared type or explicit non-storage use"
            });
            r["projectionSupportingFactIds"] = json!(usage);
            r["projectionSourceRefs"] = json!(usage
                .iter()
                .map(|id| source(by_id[id.as_str()]))
                .collect::<Vec<_>>());
        }
    }
    // Prefer bases and storage/construction over incidental lexical uses when a
    // single high-fan-out class exceeds the display budget.
    relations.sort_by_key(|r| {
        (
            match r["relationship"].as_str() {
                Some("extends") => 0,
                Some("stores_parameter") => 1,
                Some("creates") => 2,
                _ => 3,
            },
            r["classId"].as_str().unwrap_or("").to_string(),
            r["sourceRefs"][0]["line"].as_u64().unwrap_or(u64::MAX),
            r["expression"].as_str().unwrap_or("").to_string(),
        )
    });
    // Packing and drawing share boundary identities. Preserve small file sets
    // when they fit, but split dense sets before omitting avoidable relations.
    let selected_ids: BTreeSet<_> = entries.iter().map(|e| e["id"].as_str().unwrap()).collect();
    let fits = |home: &[usize]| {
        if home.len() > 12 {
            return false;
        }
        let ids: BTreeSet<_> = home
            .iter()
            .map(|i| entries[*i]["id"].as_str().unwrap())
            .collect();
        let mut boundaries = BTreeSet::new();
        let mut context = BTreeSet::new();
        for r in relations.iter().filter(|r| {
            ids.contains(r["classId"].as_str().unwrap()) && r["representedAsField"] != true
        }) {
            if let Some(target) = r["targetClassId"]
                .as_str()
                .filter(|t| selected_ids.contains(t))
            {
                if !ids.contains(target) {
                    context.insert(target);
                }
            } else {
                boundaries.insert(boundary_key(r));
            }
        }
        boundaries.len() <= 24 && context.len() <= 8
    };
    let mut homes: Vec<Vec<usize>> = Vec::new();
    let mut pending: Vec<usize> = Vec::new();
    let mut start = 0;
    while start < entries.len() {
        let mut end = start + 1;
        while end < entries.len() && entries[end]["path"] == entries[start]["path"] {
            end += 1;
        }
        if !pending.is_empty() && entries[pending[0]]["folder"] != entries[start]["folder"] {
            homes.push(std::mem::take(&mut pending));
        }
        let file: Vec<_> = (start..end).collect();
        if fits(&file) {
            let combined: Vec<_> = pending.iter().chain(&file).copied().collect();
            if !pending.is_empty() && !fits(&combined) {
                homes.push(std::mem::take(&mut pending));
            }
            pending.extend(file);
        } else {
            for i in file {
                let mut combined = pending.clone();
                combined.push(i);
                if !pending.is_empty() && !fits(&combined) {
                    homes.push(std::mem::take(&mut pending));
                }
                pending.push(i);
            }
        }
        start = end;
    }
    if !pending.is_empty() {
        homes.push(pending);
    }
    if homes.is_empty() {
        homes.push(vec![]);
    }
    for (g, home) in homes.iter().enumerate() {
        for i in home {
            entries[*i]["homeGroupId"] = json!(format!("group-{:04}", g + 1));
        }
    }
    let entry_by_id: BTreeMap<_, _> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| (e["id"].as_str().unwrap(), i))
        .collect();
    let mut groups = Vec::new();
    let mut diagrams = Vec::new();
    for (g, home) in homes.iter().enumerate() {
        let id = format!("group-{:04}", g + 1);
        let title = if home.is_empty() {
            "No catalog classes".into()
        } else {
            format!(
                "{}: {}{}",
                entries[home[0]]["folder"].as_str().unwrap(),
                name(&entries[home[0]]),
                if home.len() > 1 {
                    format!(
                        " to {} ({} classes)",
                        name(&entries[*home.last().unwrap()]),
                        home.len()
                    )
                } else {
                    String::new()
                }
            )
        };
        let home_ids: BTreeSet<_> = home
            .iter()
            .map(|i| entries[*i]["id"].as_str().unwrap())
            .collect();
        // A broad inheritance fan-out otherwise puts every long constructor
        // signature into one extremely wide row. Keep those families vertical
        // beside their base, without changing relationship meaning or grouping.
        let mut base_children = BTreeMap::<&str, usize>::new();
        for r in relations.iter().filter(|r| {
            r["relationship"] == "extends" && home_ids.contains(r["classId"].as_str().unwrap())
        }) {
            if let Some(base) = r["targetClassId"].as_str() {
                *base_children.entry(base).or_default() += 1;
            }
        }
        let direction = if base_children.values().any(|children| *children >= 4) {
            "LR"
        } else {
            "TB"
        };
        let mut mmd = format!("classDiagram\n  direction {direction}\n");
        for i in home {
            class_box(&mut mmd, &entries[*i], false);
        }
        if home.is_empty() {
            mmd.push_str("  %% No classes in the saved catalog\n");
        }
        let mut context = BTreeSet::new();
        let mut boundaries: BTreeMap<String, String> = BTreeMap::new();
        let mut drawn = Vec::new();
        let mut omitted = Vec::new();
        let mut fields = Vec::new();
        let mut edges = Edges::new();
        for (ri, r) in relations
            .iter()
            .enumerate()
            .filter(|(_, r)| home_ids.contains(r["classId"].as_str().unwrap()))
        {
            if r["representedAsField"] == true {
                fields.push(ri);
                continue;
            }
            let from = entries[entry_by_id[r["classId"].as_str().unwrap()]]["diagramId"]
                .as_str()
                .unwrap()
                .to_string();
            let target = r["targetClassId"].as_str();
            let to = if let Some(ti) = target.and_then(|id| entry_by_id.get(id)).copied() {
                if !home.contains(&ti) && !context.contains(&ti) {
                    if context.len() == 8 {
                        omitted.push(ri);
                        continue;
                    }
                    context.insert(ti);
                    class_box(&mut mmd, &entries[ti], true);
                }
                entries[ti]["diagramId"].as_str().unwrap().to_string()
            } else {
                let key = boundary_key(r);
                if let Some(id) = boundaries.get(&key) {
                    id.clone()
                } else {
                    if boundaries.len() == 24 {
                        omitted.push(ri);
                        continue;
                    }
                    let bid = format!("B{:03}", boundaries.len() + 1);
                    let boundary_label = if let Some(tid) = target {
                        all.iter()
                            .find(|e| e["id"] == tid)
                            .map(name)
                            .unwrap_or_else(|| r["expression"].as_str().unwrap().into())
                    } else {
                        r["expression"].as_str().unwrap().to_string()
                    };
                    mmd.push_str(&format!(
                        "  class {bid}[\"{}: {}\"]\n",
                        if target.is_some() {
                            "outside selection"
                        } else if matches!(
                            r["bindingKind"].as_str(),
                            Some("factory_parameter" | "constructor_parameter")
                        ) {
                            "supplied parameter"
                        } else if r["relationship"] == "extends" {
                            "unresolved base"
                        } else {
                            "unresolved binding"
                        },
                        label(&boundary_label)
                    ));
                    boundaries.insert(key, bid.clone());
                    bid
                }
            };
            let extends = r["relationship"] == "extends";
            let basis = match r["relationship"].as_str().unwrap() {
                "extends" => "declared extends",
                "stores_parameter" => "stores argument",
                "uses_factory_binding" => "uses factory binding",
                "creates" => "creates",
                _ => unreachable!(),
            };
            let edge = edges.entry((from, to, extends)).or_default();
            edge.0.insert(basis.into());
            for basis in r["augmentationBases"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                edge.0.insert(
                    match basis {
                        "framework_contract" => "framework contract",
                        "explicit_configuration" => "explicit configuration",
                        _ => "source syntax",
                    }
                    .into(),
                );
            }
            edge.1.push(ri);
            drawn.push(ri);
        }
        let mut drawn_edges = Vec::new();
        for ((from, to, extends), (mut basis, indices)) in edges {
            if basis.contains("stores argument") || basis.contains("creates") {
                basis.remove("uses factory binding");
            }
            let text = basis.into_iter().collect::<Vec<_>>().join(" / ");
            if extends {
                mmd.push_str(&format!("  {to} <|-- {from} : {text}\n"));
            } else {
                mmd.push_str(&format!("  {from} ..> {to} : {text}\n"));
            }
            drawn_edges.push(json!({"from":from,"to":to,"kind":if extends{"generalization"}else{"dependency"},"label":text,"relationshipIndices":indices}));
        }
        groups.push(json!({"id":id,"title":title,"homeClassIds":home.iter().map(|i|&entries[*i]["id"]).collect::<Vec<_>>(),"contextClassIds":context.iter().map(|i|&entries[*i]["id"]).collect::<Vec<_>>(),"contextNodes":context.iter().map(|i|json!({"classId":entries[*i]["id"],"diagramId":entries[*i]["diagramId"],"homeGroupId":entries[*i]["homeGroupId"]})).collect::<Vec<_>>(),"boundaryIds":boundaries.values().collect::<Vec<_>>(),"boundaries":boundaries.iter().map(|(scope,id)|json!({"id":id,"scope":scope})).collect::<Vec<_>>(),"relationsDrawn":drawn,"relationsOmitted":omitted,"relationCounts":{"modeled":drawn.len()+omitted.len(),"shown":drawn.len(),"omitted":omitted.len()}}));
        groups.last_mut().unwrap()["drawnEdges"] = json!(drawn_edges);
        groups.last_mut().unwrap()["direction"] = json!(direction);
        groups.last_mut().unwrap()["relationsRepresentedAsFields"] = json!(fields);
        for (key, visible) in [
            ("relationsRepresentedAsFieldsVisible", true),
            ("relationsRepresentedAsFieldsHidden", false),
        ] {
            groups.last_mut().unwrap()[key] = json!(fields
                .iter()
                .filter(|i| relations[**i]["representedAsFieldVisible"] == visible)
                .collect::<Vec<_>>());
        }
        groups.last_mut().unwrap()["relationCounts"]["fieldsVisible"] = json!(fields
            .iter()
            .filter(|i| relations[**i]["representedAsFieldVisible"] == true)
            .count());
        groups.last_mut().unwrap()["relationCounts"]["fieldsHidden"] = json!(fields
            .iter()
            .filter(|i| relations[**i]["representedAsFieldVisible"] == false)
            .count());
        groups.last_mut().unwrap()["relationCounts"]["fields"] = json!(fields.len());
        groups.last_mut().unwrap()["relationCounts"]["modeled"] =
            json!(drawn.len() + omitted.len() + fields.len());
        diagrams.push(Diagram {
            id,
            title,
            mermaid: mmd,
        });
    }
    for (i, r) in relations.iter_mut().enumerate() {
        r["id"] = json!(format!("R{:04}", i + 1));
    }
    let legend = json!({"schemaVersion":"ideomancer.class-uml.v1","sourceModel":"static-candidate","selection":if selection.is_some(){"selected"}else{"all"},"snapshotId":catalog.snapshot_id,"sourceRoot":catalog.source_root,"extractorVersion":catalog.extractor_version,"complete":catalog.complete,"entries":entries,"relationships":relations,"groups":groups,"coverage":{"catalogClasses":all.len(),"selectedClasses":entries.len(),"homeClasses":entries.len(),"allSelectedClassesRepresented":true,"diagramCount":diagrams.len(),"modeledRelations":relations.len(),"relationsDrawn":groups.iter().map(|g|g["relationCounts"]["shown"].as_u64().unwrap()).sum::<u64>(),"relationsOmitted":groups.iter().map(|g|g["relationCounts"]["omitted"].as_u64().unwrap()).sum::<u64>()},"layout":{"maxHomeClasses":12,"maxContextClasses":8,"maxBoundaries":24,"maxVisibleMethods":12,"maxVisibleFields":8},"warnings":["Static source candidates only, not runtime architecture, DI registration or provider identity. Folder grouping establishes no dependency.","Hollow triangles point to declared bases. Dotted arrows show observed argument storage, factory binding use or construction, never composition or aggregation.","Unresolved boundaries are scoped to the owning class and binding proof. Declared types are annotations, not implementations. No JavaScript abstractness is inferred from names.","All members and modeled relationships are retained in metadata; diagram caps never omit home classes."],"scanWarnings":scan_warnings});
    let mut legend = legend;
    legend["modelVersion"] = json!("class-uml-semantics-2");
    legend["coverage"]["relationAccounting"] = json!(
        "modeled = shown + omitted + fields; fields are state projection, omitted are layout caps"
    );
    legend["coverage"]["relationsRepresentedAsFields"] = json!(groups
        .iter()
        .map(|g| g["relationCounts"]["fields"].as_u64().unwrap())
        .sum::<u64>());
    for (key, count) in [
        ("relationsRepresentedAsFieldsVisible", "fieldsVisible"),
        ("relationsRepresentedAsFieldsHidden", "fieldsHidden"),
    ] {
        legend["coverage"][key] = json!(groups
            .iter()
            .map(|g| g["relationCounts"][count].as_u64().unwrap())
            .sum::<u64>());
    }
    legend["warnings"].as_array_mut().unwrap().push(json!("Storage-only observations are represented as attributes, not claims of no dependency. Conditional constructor assignments do not establish definite initialization, lifetime ownership or composition. Parameter ? includes optional/defaulted parameters; default values are intentionally absent. Only source-declared types and return types are shown."));
    let title = if selection.is_some() {
        "Selected Class UML"
    } else {
        "Catalog Class UML"
    };
    let mut body = format!("<h1>{title}</h1><p>{} of {} catalog classes; {} UML groups. Source-syntax candidates, not runtime architecture.</p><p>Snapshot: <code>{}</code>. <a href=\"legend.json\">Full JSON metadata and proof references</a>.</p>",entries.len(),all.len(),diagrams.len(),html_text(&catalog.snapshot_id));
    if !catalog.complete {
        body.push_str("<p role=\"alert\"><strong>INCOMPLETE SNAPSHOT: classes or members may be missing from the saved catalog.</strong></p>");
    }
    if selection.is_some() {
        body.push_str("<p>This focused bundle does not imply an all-class bundle exists. Generate it from the saved snapshot with <code>ideomancer diagram SAVEDDIR --kind class</code>, replacing SAVEDDIR with the saved catalog directory.</p>");
    }
    body.push_str("<p>Hollow triangles point toward declared bases. Dotted arrows mean stores argument, uses factory binding or creates. Boundaries are unresolved or outside this selection, not inferred providers. Full members and relations remain below without SVG or JavaScript.</p><nav aria-label=\"UML groups\"><ul>");
    for d in &diagrams {
        body.push_str(&format!(
            "<li><a href=\"#{}\">{}</a></li>",
            d.id,
            html_text(&d.title)
        ));
    }
    body.push_str("</ul></nav>");
    for (d, g) in diagrams.iter().zip(&groups) {
        body.push_str(&format!("<section id=\"{}\"><h2>{}</h2><p>{} home classes; {} modeled relations shown by {} UML arrows / {} projected as fields ({} visible, {} hidden by the 8-field-per-class image cap) / {} relation records omitted from arrows by image layout caps. Hidden field projections are a subset of projected fields, not additional modeled relations. All fields and relations remain in details and JSON.</p><p><a href=\"{}.mmd\">Mermaid source</a> | <a href=\"{}.svg\">SVG image</a></p>",d.id,html_text(&d.title),g["homeClassIds"].as_array().unwrap().len(),g["relationCounts"]["shown"],g["drawnEdges"].as_array().unwrap().len(),g["relationCounts"]["fields"],g["relationCounts"]["fieldsVisible"],g["relationCounts"]["fieldsHidden"],g["relationCounts"]["omitted"],d.id,d.id));
        body.push_str(&viewer::diagram_for(
            &d.title,
            "Source-grounded UML. Use class details when the image is unavailable.",
            &format!("{}.svg", d.id),
            &d.id,
        )?);
        for c in g["contextNodes"].as_array().unwrap() {
            let e = &entries[entry_by_id[c["classId"].as_str().unwrap()]];
            body.push_str(&format!(
                "<p>Context: {}. <a href=\"#{}\">Home group</a>.</p>",
                html_text(&name(e)),
                c["homeGroupId"].as_str().unwrap()
            ));
        }
        body.push_str("</section>");
    }
    body.push_str("<h2>Class Index and Full Details</h2>");
    body.push_str("<p>Storage-only observations are represented as fields, not layout omissions or claims of no dependency. A parameter ? means optional or defaulted; default values are intentionally absent. Constructor assignments are observations, not definite initialization or composition.</p>");
    if entries.is_empty() {
        body.push_str("<p>No classes in the saved catalog.</p>");
    }
    for e in &entries {
        body.push_str(&format!("<details id=\"details-{}\" data-class-id=\"{}\"><summary>{}</summary><p><code>{}</code> at <code>{}:{}-{}</code>. <a href=\"#{}\">Home UML group</a>.</p>",e["diagramId"].as_str().unwrap(),e["id"].as_str().unwrap(),html_text(&name(e)),html_text(e["symbol"].as_str().unwrap()),html_text(e["path"].as_str().unwrap()),e["line"],e["endLine"],e["homeGroupId"].as_str().unwrap()));
        for (heading, key) in [("Methods", "methods"), ("Fields", "attributes")] {
            body.push_str(&format!(
                "<h3>{heading} ({})</h3><ul>",
                e[key].as_array().unwrap().len()
            ));
            for m in e[key].as_array().unwrap() {
                body.push_str(&format!(
                    "<li><code>{}</code> [{}; {}]{}",
                    html_text(
                        m["symbol"]
                            .as_str()
                            .or(m["name"].as_str())
                            .unwrap_or("UNKNOWN")
                    ),
                    html_text(m["memberKind"].as_str().unwrap_or("UNKNOWN")),
                    html_text(m["visibility"].as_str().unwrap_or("UNKNOWN")),
                    m["type"]
                        .as_str()
                        .map(|t| format!(" declared type: <code>{}</code>", html_text(t)))
                        .unwrap_or_default()
                ));
                let mut locations = BTreeSet::new();
                for r in m["sourceRefs"].as_array().into_iter().flatten() {
                    let location = format!(
                        "{}:{}-{}",
                        r["path"].as_str().unwrap_or("UNKNOWN"),
                        r["line"],
                        r["endLine"]
                    );
                    if !locations.insert(location.clone()) {
                        continue;
                    }
                    body.push_str(&format!(" <code>{}</code>", html_text(&location)));
                }
                body.push_str("</li>");
                if m["signature"].is_object() {
                    body.push_str(&format!("<li>Full declared signature (default values intentionally absent): <code>{}</code></li>", html_text(&super::source_view::signature_text(m))));
                }
            }
            body.push_str("</ul>");
        }
        body.push_str("<h3>All Modeled Relations</h3><ul>");
        for r in relations.iter().filter(|r| r["classId"] == e["id"]) {
            body.push_str(&format!(
                "<li>{}: <code>{}</code>{}{} ({})</li>",
                html_text(r["relationship"].as_str().unwrap()),
                html_text(r["expression"].as_str().unwrap()),
                r["memberName"]
                    .as_str()
                    .map(|s| format!(" stored as <code>{}</code>", html_text(s)))
                    .unwrap_or_default(),
                r["declaredType"]
                    .as_str()
                    .map(|s| format!(" declared type <code>{}</code>", html_text(s)))
                    .unwrap_or_default(),
                if r["representedAsField"] == true {
                    "represented as field; dependency status not asserted"
                } else if r["targetClassId"].is_null() {
                    "unresolved scoped boundary"
                } else {
                    "source-grounded class declaration"
                }
            ));
        }
        body.push_str("</ul></details>");
        if body.len() > MAX_BYTES {
            return Err(LIMIT.into());
        }
    }
    body.push_str("<h2>Snapshot Boundaries</h2><ul>");
    for w in legend["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .chain(legend["scanWarnings"].as_array().into_iter().flatten())
    {
        body.push_str(&format!(
            "<li>{}</li>",
            html_text(w.as_str().unwrap_or("UNKNOWN"))
        ));
    }
    body.push_str(
        "</ul><p><a href=\"legend.json\">Complete metadata and all proof references</a>. The legend stores full citations once in <code>proofSources</code>, keyed by fact ID. Each <code>sourceRefs</code> integer indexes that record's <code>sourceFactIds</code>; resolve the resulting fact ID through <code>proofSources</code> for its complete citation.</p>",
    );
    let html = viewer::document(title, &body);
    let mut sources = BTreeMap::new();
    normalize_sources(&mut legend, &mut sources)?;
    if let Some(claims) = augmentation {
        legend["augmentation"] = json!(claims.augmentation());
        for site in claims.augmentation_evidence() {
            let fact = by_id.get(site).ok_or("Missing augmentation evidence")?;
            sources.insert(site.to_string(), source(fact));
        }
    }
    legend["proofSources"] = json!(sources);
    legend["sourceReferenceFormat"] = json!({"policy":"fact-id-source-index-1","sourceRefs":"each integer indexes the same record's sourceFactIds; resolve the fact ID through proofSources[factId]","projectionSourceRefs":"each integer indexes the same record's projectionSupportingFactIds; resolve the fact ID through proofSources[factId]","proofIds":"all citation links, sourceFactIds, supportingFactIds and projectionSupportingFactIds are retained without truncation","signatures":"signature retains receipt policy and declaration identity; unabridged parameters, returnType and typeParameters are stored once on the member"});
    let legend_bytes = serde_json::to_vec_pretty(&legend)
        .map_err(|e| e.to_string())?
        .len();
    let mermaid_bytes = diagrams.iter().map(|d| d.mermaid.len()).sum::<usize>();
    let bytes = html.len() + legend_bytes + mermaid_bytes;
    if bytes > MAX_BYTES {
        return Err(format!("{LIMIT} (HTML: {}, legend: {legend_bytes}, Mermaid: {mermaid_bytes}, total: {bytes}, limit: {MAX_BYTES} bytes; legend entries: {}, relationships: {}, proofSources: {})", html.len(), serde_json::to_vec_pretty(&legend["entries"]).map_err(|e|e.to_string())?.len(),serde_json::to_vec_pretty(&legend["relationships"]).map_err(|e|e.to_string())?.len(),serde_json::to_vec_pretty(&legend["proofSources"]).map_err(|e|e.to_string())?.len()));
    }
    Ok(Bundle {
        diagrams,
        legend,
        html,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::SourceFile;

    #[test]
    #[ignore = "requires an explicit saved catalog directory; reads only catalog.json and evidence.json"]
    fn saved_catalog_bundle_size_regression() {
        let directory = std::path::PathBuf::from(
            std::env::var_os("IDEOMANCER_TEST_SAVED_CATALOG").expect("saved catalog directory"),
        );
        let catalog: Catalog =
            serde_json::from_slice(&std::fs::read(directory.join("catalog.json")).unwrap())
                .unwrap();
        let evidence: Evidence =
            serde_json::from_slice(&std::fs::read(directory.join("evidence.json")).unwrap())
                .unwrap();
        let b = build(&catalog, &evidence, None).unwrap();
        eprintln!(
            "saved UML: snapshot={} complete={} classes={} groups={} HTML={} legend={} Mermaid={}",
            catalog.snapshot_id,
            catalog.complete,
            b.legend["coverage"]["homeClasses"],
            b.diagrams.len(),
            b.html.len(),
            serde_json::to_vec_pretty(&b.legend).unwrap().len(),
            b.diagrams.iter().map(|d| d.mermaid.len()).sum::<usize>()
        );
        assert_eq!(
            b.legend["coverage"]["homeClasses"],
            b.legend["coverage"]["catalogClasses"]
        );
        assert_eq!(b.legend["coverage"]["allSelectedClassesRepresented"], true);
    }

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

    fn evidence(n: usize) -> Evidence {
        let mut e = Evidence {
            schema_version: "ideomancer.evidence.v1".into(),
            snapshot_id: "saved".into(),
            extractor_version: "synthetic".into(),
            python_version: "synthetic".into(),
            complete: true,
            ..Evidence::default()
        };
        for i in 0..n {
            e.facts.push(fact(
                &format!("class{i}"),
                "class",
                "services/app.py",
                &format!("app.Class{i:03}"),
                i * 100 + 2,
                i * 100 + 99,
                "",
            ));
        }
        e.files.push(SourceFile {
            path: "services/app.py".into(),
            sha256: "a".repeat(64),
        });
        e
    }

    fn bundle(e: &Evidence) -> Bundle {
        build(&crate::catalog::build(e, "/source").unwrap(), e, None).unwrap()
    }

    fn proof_refs<'a>(legend: &'a Value, record: &Value) -> Vec<&'a Value> {
        record["sourceFactIds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| &legend["proofSources"][id.as_str().unwrap()])
            .collect()
    }

    #[test]
    fn broad_inheritance_families_avoid_a_single_extremely_wide_row() {
        let mut e = evidence(9);
        e.facts
            .push(fact("module", "module", "services/app.py", "app", 1, 1, ""));
        for i in 1..9 {
            e.facts.push(fact(
                &format!("base{i}"),
                "base",
                "services/app.py",
                &format!("app.Class{i:03}"),
                i * 100 + 2,
                i * 100 + 2,
                "Class000",
            ));
        }
        let b = bundle(&e);
        assert_eq!(b.diagrams.len(), 1);
        assert!(b.diagrams[0].mermaid.contains("direction LR"));
        assert_eq!(b.legend["groups"][0]["direction"], "LR");
        assert_eq!(b.legend["coverage"]["homeClasses"], 9);
        assert_eq!(b.legend["coverage"]["relationsDrawn"], 8);
        assert_eq!(b.legend["coverage"]["relationsOmitted"], 0);
    }

    #[test]
    fn citation_index_preserves_every_full_source_and_proof_id() {
        let e = member_fixture();
        let b = bundle(&e);
        let sources = &b.legend["proofSources"];
        assert_eq!(
            b.legend["sourceReferenceFormat"]["policy"],
            "fact-id-source-index-1"
        );
        for (id, reference) in sources.as_object().unwrap() {
            assert_eq!(
                *reference,
                source(e.facts.iter().find(|f| f.id == *id).unwrap())
            );
        }
        fn check(value: &Value, sources: &Value) {
            match value {
                Value::Object(object) => {
                    for key in [
                        "sourceFactIds",
                        "supportingFactIds",
                        "projectionSupportingFactIds",
                    ] {
                        for id in object
                            .get(key)
                            .and_then(Value::as_array)
                            .into_iter()
                            .flatten()
                        {
                            assert_eq!(
                                sources[id.as_str().unwrap()]["factId"],
                                *id,
                                "missing {key}: {id}"
                            );
                        }
                    }
                    for (refs, ids) in [
                        ("sourceRefs", "sourceFactIds"),
                        ("projectionSourceRefs", "projectionSupportingFactIds"),
                    ] {
                        if let Some(refs) = object.get(refs).and_then(Value::as_array) {
                            assert_eq!(refs.len(), value[ids].as_array().unwrap().len());
                            for r in refs {
                                let id = &value[ids][r.as_u64().unwrap() as usize];
                                assert_eq!(sources[id.as_str().unwrap()]["factId"], *id);
                            }
                        }
                    }
                    for child in object.values() {
                        check(child, sources);
                    }
                }
                Value::Array(items) => {
                    for child in items {
                        check(child, sources);
                    }
                }
                _ => {}
            }
        }
        check(&b.legend, sources);
        let mut reversed = e;
        reversed.facts.reverse();
        assert_eq!(b.legend, bundle(&reversed).legend);
    }

    #[test]
    fn large_member_inventory_indexes_duplicate_proofs_not_members_or_signatures() {
        let mut e = member_fixture();
        let prefix = "long_namespace_".repeat(64);
        for f in &mut e.facts {
            f.symbol = format!("{prefix}.{}", f.symbol);
        }
        let template: Fact =
            serde_json::from_value(json!(e.facts.iter().find(|f| f.id == "field-0").unwrap()))
                .unwrap();
        for i in 0..400 {
            let mut f: Fact = serde_json::from_value(json!(template)).unwrap();
            f.id = format!("large-field-{i}");
            let mut v: Value = serde_json::from_str(&f.value).unwrap();
            v["memberName"] = json!(format!("state{i}"));
            f.value = v.to_string();
            e.facts.push(f);
        }
        let b = bundle(&e);
        let child = b.legend["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["factId"] == "child")
            .unwrap();
        assert_eq!(child["attributes"].as_array().unwrap().len(), 408);
        for i in 0..400 {
            let field = child["attributes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|m| m["name"] == format!("state{i}"))
                .unwrap();
            for id in [
                format!("large-field-{i}"),
                "child".into(),
                "child-meta".into(),
                "ctor".into(),
                "ctor-meta".into(),
            ] {
                assert!(field["sourceFactIds"]
                    .as_array()
                    .unwrap()
                    .contains(&json!(id)));
                assert_eq!(b.legend["proofSources"][&id]["factId"], id);
            }
        }
        assert!(b.html.contains("state399"));
        assert!(b.html.contains("constructor(Dep?)"));
        assert!(!b.html.contains("&quot;parameters&quot;"));
        for method in child["methods"].as_array().unwrap() {
            assert_eq!(method["signature"].as_object().unwrap().len(), 3);
            assert!(method["parameters"].is_array());
            assert!(method["typeParameters"].is_array());
        }
        let mut expanded = b.legend.clone();
        fn expand(value: &mut Value, sources: &Value) {
            match value {
                Value::Object(object) => {
                    if let Some(ids) = object.get("sourceFactIds").and_then(Value::as_array) {
                        object.insert(
                            "sourceRefs".into(),
                            json!(ids
                                .iter()
                                .map(|id| &sources[id.as_str().unwrap()])
                                .collect::<Vec<_>>()),
                        );
                    }
                    for child in object.values_mut() {
                        expand(child, sources);
                    }
                }
                Value::Array(items) => {
                    for child in items {
                        expand(child, sources);
                    }
                }
                _ => {}
            }
        }
        expand(&mut expanded, &b.legend["proofSources"]);
        assert!(
            serde_json::to_vec_pretty(&expanded).unwrap().len()
                > 2 * serde_json::to_vec_pretty(&b.legend).unwrap().len()
        );
        assert!(
            b.html.len()
                + serde_json::to_vec_pretty(&b.legend).unwrap().len()
                + b.diagrams.iter().map(|d| d.mermaid.len()).sum::<usize>()
                <= MAX_BYTES
        );
    }

    #[test]
    fn all_88_class_boxes_have_exactly_one_home_and_bounded_groups() {
        let e = evidence(88);
        let b = bundle(&e);
        assert_eq!(b.legend["schemaVersion"], "ideomancer.class-uml.v1");
        assert_eq!(b.legend["coverage"]["homeClasses"], 88);
        assert_eq!(b.legend["coverage"]["allSelectedClassesRepresented"], true);
        assert_eq!(b.diagrams.len(), 8);
        let mut home_ids = BTreeSet::new();
        for (d, g) in b
            .diagrams
            .iter()
            .zip(b.legend["groups"].as_array().unwrap())
        {
            assert!(d.mermaid.starts_with("classDiagram\n"));
            assert!(d.id.starts_with("group-"));
            assert!(g["homeClassIds"].as_array().unwrap().len() <= 12);
            for id in g["homeClassIds"].as_array().unwrap() {
                assert!(home_ids.insert(id.as_str().unwrap()));
            }
        }
        assert_eq!(home_ids.len(), 88);
        assert_eq!(
            b.diagrams
                .iter()
                .map(|d| d.mermaid.matches("  class C").count())
                .sum::<usize>(),
            88
        );
        assert_eq!(b.html.matches("data-viewer data-viewer-state").count(), 8);
        assert!(!b.html.contains("class-overview"));
        assert!(!b.html.contains("<svg"));
        assert_eq!(b.html.matches("<script>").count(), 1);
        assert!(b.html.find("<section id=\"group-").unwrap() < b.html.find("Class Index").unwrap());
        assert!(!b.html.contains("&quot;sourceFactIds&quot;"));
        let mut reversed = e;
        reversed.facts.reverse();
        let again = bundle(&reversed);
        assert_eq!(b.legend, again.legend);
        assert_eq!(b.html, again.html);
    }

    #[test]
    fn small_same_file_hierarchy_is_not_split_after_other_files() {
        let mut e = evidence(14);
        for (i, f) in e.facts.iter_mut().enumerate() {
            f.path = if i < 5 {
                "services/a.py"
            } else {
                "services/errors.py"
            }
            .into();
        }
        e.files = ["services/a.py", "services/errors.py"]
            .into_iter()
            .map(|path| SourceFile {
                path: path.into(),
                sha256: "a".repeat(64),
            })
            .collect();
        let b = bundle(&e);
        assert_eq!(b.diagrams.len(), 2);
        assert_eq!(
            b.legend["groups"][1]["homeClassIds"]
                .as_array()
                .unwrap()
                .len(),
            9
        );
    }

    #[test]
    fn dense_file_is_split_before_relations_are_omitted() {
        let mut e = evidence(12);
        for c in 0..12 {
            for d in 0..3 {
                e.facts.push(fact(
                    &format!("base-{c}-{d}"),
                    "base",
                    "services/app.py",
                    &format!("app.Class{c:03}"),
                    c * 100 + 2,
                    c * 100 + 2,
                    &format!("External{d}"),
                ));
            }
        }
        let b = bundle(&e);
        assert_eq!(b.diagrams.len(), 2);
        assert_eq!(b.legend["coverage"]["homeClasses"], 12);
        assert_eq!(b.legend["coverage"]["modeledRelations"], 36);
        assert_eq!(b.legend["coverage"]["relationsOmitted"], 0);
        for group in b.legend["groups"].as_array().unwrap() {
            assert!(group["boundaryIds"].as_array().unwrap().len() <= 24);
        }
    }

    #[test]
    fn cross_group_context_budget_also_splits_without_relation_omissions() {
        let mut e = evidence(36);
        e.facts
            .push(fact("module", "module", "services/app.py", "app", 1, 1, ""));
        for c in 24..36 {
            e.facts.push(fact(
                &format!("base-{c}"),
                "base",
                "services/app.py",
                &format!("app.Class{c:03}"),
                c * 100 + 2,
                c * 100 + 2,
                &format!("Class{:03}", c - 24),
            ));
        }
        let b = bundle(&e);
        assert_eq!(b.diagrams.len(), 4);
        assert_eq!(b.legend["coverage"]["homeClasses"], 36);
        assert_eq!(b.legend["coverage"]["relationsOmitted"], 0);
        for group in b.legend["groups"].as_array().unwrap() {
            assert!(group["contextClassIds"].as_array().unwrap().len() <= 8);
        }
    }

    #[test]
    fn ambiguous_legacy_base_ownership_is_omitted_with_a_warning() {
        let mut e = evidence(0);
        for id in ["first", "second"] {
            e.facts
                .push(fact(id, "class", "services/app.py", "app.C", 1, 1, ""));
        }
        for base in ["A", "B"] {
            e.facts
                .push(fact(base, "base", "services/app.py", "app.C", 1, 1, base));
        }
        let b = bundle(&e);
        assert_eq!(b.legend["coverage"]["homeClasses"], 2);
        assert_eq!(b.legend["coverage"]["modeledRelations"], 0);
        assert!(b.legend["scanWarnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w
                .as_str()
                .unwrap()
                .contains("Ambiguous declaration-base ownership")));
        assert!(b.html.contains("fallback base"));
    }

    #[test]
    fn empty_incomplete_and_selected_pages_are_honest() {
        let mut e = evidence(0);
        e.complete = false;
        let b = bundle(&e);
        assert_eq!(b.diagrams.len(), 1);
        assert!(b.diagrams[0].mermaid.starts_with("classDiagram"));
        assert!(b.html.contains("INCOMPLETE SNAPSHOT"));
        assert_eq!(b.legend["coverage"]["homeClasses"], 0);
        let e = evidence(88);
        let c = crate::catalog::build(&e, "/source").unwrap();
        let mut p = crate::catalog::page(
            &c,
            &crate::catalog::Filter {
                kind: Some(Kind::Class),
                limit: 30,
                ..Default::default()
            },
        )
        .unwrap();
        let b = build(&c, &e, Some(&p)).unwrap();
        assert_eq!(b.legend["coverage"]["catalogClasses"], 88);
        assert_eq!(b.legend["coverage"]["homeClasses"], 30);
        assert_eq!(b.legend["selection"], "selected");
        assert!(!b.html.contains("href=\"../class/index.html\""));
        assert!(b.html.contains("ideomancer diagram SAVEDDIR --kind class"));
        p.items.push(p.items[0].clone());
        assert!(build(&c, &e, Some(&p)).is_err());
        p.items.clear();
        assert!(build(&c, &e, Some(&p)).is_err());
        let mut tampered = c.clone();
        tampered.items.pop();
        assert!(build(&tampered, &e, None).is_err());
    }

    #[test]
    fn python_bases_members_context_and_omissions_preserve_full_model() {
        let mut e = evidence(25);
        e.facts
            .push(fact("module", "module", "services/app.py", "app", 1, 1, ""));
        e.facts.push(fact(
            "base",
            "base",
            "services/app.py",
            "app.Class024",
            2402,
            2402,
            "Class000",
        ));
        for i in 0..30 {
            e.facts.push(fact(
                &format!("boundary{i}"),
                "base",
                "services/app.py",
                "app.Class024",
                2402,
                2402,
                &format!("External{i}"),
            ));
        }
        for i in 0..16 {
            e.facts.push(fact(
                &format!("method{i}"),
                "function",
                "services/app.py",
                &format!("app.Class024.method{i:02}"),
                2403 + i,
                2403 + i,
                "",
            ));
        }
        let b = bundle(&e);
        let g = b.legend["groups"].as_array().unwrap().last().unwrap();
        assert_eq!(g["contextClassIds"].as_array().unwrap().len(), 1);
        assert_eq!(g["boundaryIds"].as_array().unwrap().len(), 24);
        assert_eq!(g["relationCounts"]["omitted"], 6);
        assert_eq!(b.legend["relationships"].as_array().unwrap().len(), 31);
        let d = b.diagrams.last().unwrap();
        assert!(d.mermaid.contains("C001 <|-- C025 : declared extends"));
        assert!(d.mermaid.contains("context: Class000"));
        assert!(d.mermaid.contains("4 more methods"));
        assert!(b.html.contains("app.Class024.method15"));
        assert!(b.html.contains("External29"));
        assert!(b
            .html
            .contains("Context: Class000. <a href=\"#group-0001\""));
    }

    #[test]
    fn hostile_labels_are_inert_and_all_documents_share_one_limit() {
        let mut e = evidence(1);
        e.facts[0].symbol = "app.Evil<script>alert(1)</script>\"{}".into();
        let b = bundle(&e);
        assert!(b.html.contains("&lt;script&gt;"));
        assert_eq!(b.html.matches("<script>").count(), 1);
        assert!(!b.diagrams[0].mermaid.contains("<script>"));
        assert!(b.diagrams[0].mermaid.contains("#60;"));
        let mut e = evidence(473);
        for n in 0..473 {
            let path = e.facts[n].path.clone();
            let symbol = e.facts[n].symbol.clone();
            for m in 0..4 {
                let method = format!("{symbol}.method{m}");
                let line = n * 100 + 10 + m * 3;
                e.facts.push(fact(
                    &format!("method-{n}-{m}"),
                    "function",
                    &path,
                    &method,
                    line,
                    line + 2,
                    "",
                ));
                e.facts.push(fact(
                    &format!("binding-{n}-{m}"),
                    "binding",
                    &path,
                    &method,
                    line + 1,
                    line + 1,
                    &json!({"target":format!("self.field{m}")}).to_string(),
                ));
            }
        }
        let b = bundle(&e);
        assert_eq!(b.legend["coverage"]["homeClasses"], 473);
        assert!(
            b.html.len()
                + serde_json::to_vec_pretty(&b.legend).unwrap().len()
                + b.diagrams.iter().map(|d| d.mermaid.len()).sum::<usize>()
                <= MAX_BYTES
        );
        let mut e = evidence(1);
        e.facts[0].symbol = format!("app.{}", "x".repeat(2_000_000));
        assert!(build(&crate::catalog::build(&e, "/source").unwrap(), &e, None).is_err());
    }

    fn javascript() -> Evidence {
        use crate::js_focus::tests::{add, source};
        let mut e = source();
        e.extractor_version = crate::javascript::CLASS_RELATION_EXTRACTOR_VERSION.into();
        e.facts.clear();
        add(&mut e, "module", "module", "app", 1, 100, json!("app"));
        for (id, owner, kind, name, line, end) in [
            ("parent", "module", "class", "Parent", 2, 10),
            ("factory", "module", "factory", "factory", 15, 100),
            ("child", "factory", "class", "Child", 20, 90),
            ("ctor", "child", "constructor", "constructor", 21, 30),
            ("method", "child", "method", "run", 31, 40),
        ] {
            let symbol = match id {
                "parent" => "app.Parent",
                "factory" => "app.factory",
                "child" => "app.factory.Child",
                "ctor" => "app.factory.Child.constructor",
                _ => "app.factory.Child.run",
            };
            add(
                &mut e,
                id,
                if kind == "class" { "class" } else { "function" },
                symbol,
                line,
                end,
                json!(name),
            );
            add(
                &mut e,
                &format!("{id}-meta"),
                "js_declaration",
                symbol,
                line,
                end,
                json!({"declarationFactId":id,"ownerFactId":owner,"kind":kind,"name":name,"publicCandidate":true,"visibility":"public"}),
            );
        }
        for (id, name, kind, scope, line, end) in [
            ("parent-guard", "Parent", "class", "module", 2, 10),
            ("base-guard", "Base", "parameter", "factory", 15, 15),
            ("dep-guard", "Dep", "parameter", "ctor", 21, 21),
        ] {
            add(
                &mut e,
                id,
                "js_binding_guard",
                "app",
                line,
                end,
                json!({"name":name,"bindingKind":kind,"scopeId":scope,"scopeKind":if scope=="module"{"module"}else{"function"},"scopeStart":if scope=="module"{0}else if scope=="factory"{100}else{200},"scopeEnd":if scope=="ctor"{300}else{1000},"declarationCount":1,"writes":0,"declarationFactId":null}),
            );
        }
        for (id, parent, kind, start, end, line, end_line) in [
            ("module", None, "module", 0, 1000, 1, 100),
            ("factory", Some("module"), "function", 100, 1000, 15, 100),
            ("child", Some("factory"), "class", 150, 900, 20, 90),
            ("ctor", Some("child"), "function", 200, 300, 21, 30),
            ("method", Some("child"), "function", 400, 500, 31, 40),
        ] {
            add(
                &mut e,
                &format!("scope-{id}"),
                "js_class_scope",
                "app",
                line,
                end_line,
                json!({"policy":"javascript-class-relations-1","scopeId":id,"parentScopeId":parent,"scopeKind":kind,"start":start,"end":end}),
            );
        }
        add(
            &mut e,
            "base",
            "base",
            "app.factory.Child",
            20,
            20,
            json!("Base"),
        );
        for (id, owner, rel, expr, kind, guard, target, member, line) in [
            (
                "inherit",
                "child",
                "extends",
                "Base",
                "factory_parameter",
                "base-guard",
                None,
                None,
                20,
            ),
            (
                "store",
                "ctor",
                "stores_parameter",
                "Dep",
                "constructor_parameter",
                "dep-guard",
                None,
                Some("service"),
                23,
            ),
            (
                "use",
                "method",
                "uses_factory_binding",
                "Base",
                "factory_parameter",
                "base-guard",
                None,
                None,
                32,
            ),
            (
                "new",
                "method",
                "creates",
                "Base",
                "factory_parameter",
                "base-guard",
                None,
                None,
                33,
            ),
        ] {
            let scopes = if owner == "ctor" {
                vec!["ctor", "child", "factory", "module"]
            } else if owner == "method" {
                vec!["method", "child", "factory", "module"]
            } else {
                vec!["child", "factory", "module"]
            };
            let reference = format!("reference-{id}");
            let start = match id {
                "inherit" => 160,
                "store" => 230,
                "use" => 420,
                _ => 430,
            };
            add(
                &mut e,
                &reference,
                "js_class_reference",
                "app.factory.Child",
                line,
                line,
                json!({"policy":"javascript-class-relations-1","classFactId":"child","ownerFactId":owner,"scopeId":scopes[0],"start":start,"end":start+4}),
            );
            let mut support = vec![
                "module".to_string(),
                "child".into(),
                "child-meta".into(),
                guard.into(),
                "factory".into(),
                "factory-meta".into(),
                reference.clone(),
            ];
            support.extend(scopes.iter().map(|id| format!("scope-{id}")));
            add(
                &mut e,
                id,
                "js_class_relation",
                "app.factory.Child",
                line,
                line,
                json!({"policy":"javascript-class-relations-1","classFactId":"child","ownerFactId":owner,"referenceFactId":reference,"relationship":rel,"expression":expr,"bindingKind":kind,"bindingGuardFactId":guard,"targetFactId":target as Option<&str>,"memberName":member,"declaredType":if member.is_some(){Some("DeclaredService")}else{None},"supportingFactIds":support,"stableBinding":true,"lookupScopeIds":scopes}),
            );
        }
        e
    }

    fn edit(e: &mut Evidence, id: &str, key: &str, value: Value) {
        let f = e.facts.iter_mut().find(|f| f.id == id).unwrap();
        let mut v: Value = serde_json::from_str(&f.value).unwrap();
        v[key] = value;
        f.value = v.to_string();
    }

    fn member_fixture() -> Evidence {
        use crate::js_focus::tests::add;
        let mut e = javascript();
        e.extractor_version = crate::javascript::STRUCTURAL_EXTRACTOR_VERSION.into();
        for f in &mut e.facts {
            if matches!(
                f.kind.as_str(),
                "js_class_relation" | "js_class_scope" | "js_class_reference"
            ) {
                let mut v: Value = serde_json::from_str(&f.value).unwrap();
                v["policy"] = json!("javascript-class-relations-2");
                if f.kind == "js_class_relation" {
                    v["importFactId"] = Value::Null;
                }
                f.value = v.to_string();
            }
        }
        edit(&mut e, "store", "memberName", json!("worker"));
        edit(&mut e, "store", "declaredType", Value::Null);
        for (id, name, kind, line, end) in [
            ("ctor", "constructor", "constructor", 21, 30),
            ("method", "run", "method", 31, 40),
        ] {
            add(
                &mut e,
                &format!("{id}-member"),
                "js_member",
                &format!("app.factory.Child.{name}"),
                line,
                end,
                json!({"declarationFactId":id,"ownerFactId":"child","memberKind":kind,"name":name,"visibility":"public","static":false}),
            );
            add(
                &mut e,
                &format!("{id}-signature"),
                "js_callable_signature",
                &format!("app.factory.Child.{name}"),
                line,
                end,
                json!({"policy":"javascript-class-members-1","declarationFactId":id,"ownerFactId":"child",
                    "parameters":[{"name":if id=="ctor"{"Dep"}else{"input"},"pattern":false,"optional":false,"rest":false,"hasDefault":true,"type":if id=="ctor"{Value::Null}else{json!("T")}}],
                    "returnType":if id=="ctor"{Value::Null}else{json!("number")},"typeParameters":if id=="ctor"{json!([])}else{json!(["T"])}}),
            );
        }
        for (i, name) in ["errorType", "stackTrace", "cache", "#private"]
            .iter()
            .enumerate()
        {
            add(
                &mut e,
                &format!("field-{i}"),
                "js_constructor_field",
                "app.factory.Child.constructor",
                24 + i,
                24 + i,
                json!({"policy":"javascript-class-members-1","classFactId":"child","ownerFactId":"ctor","memberName":name,"visibility":if name.starts_with('#'){"private"}else{"public"},"static":false,"assignmentKind":"direct-constructor-assignment","sourceType":null,"supportingFactIds":["child","child-meta","ctor","ctor-meta"]}),
            );
        }
        add(
            &mut e,
            "declared-field",
            "js_member",
            "app.factory.Child.stackTrace",
            22,
            22,
            json!({"ownerFactId":"child","memberKind":"field","name":"stackTrace","visibility":"protected","static":false,"type":"DeclaredTrace"}),
        );
        for (i, name) in ["registryId", "message", "loggerService"]
            .iter()
            .enumerate()
        {
            let mut f: Fact =
                serde_json::from_value(json!(e.facts.iter().find(|f| f.id == "store").unwrap()))
                    .unwrap();
            f.id = format!("data-store-{i}");
            e.facts.push(f);
            edit(
                &mut e,
                &format!("data-store-{i}"),
                "memberName",
                json!(name),
            );
        }
        add(
            &mut e,
            "worker-call",
            "call",
            "app.factory.Child.run",
            32,
            32,
            json!("this.worker.run()"),
        );
        add(
            &mut e,
            "worker-call-meta",
            "js_call",
            "app.factory.Child.run",
            32,
            32,
            json!({"callFactId":"worker-call","ownerFactId":"method","callee":"this.worker.run","targetFactId":null,"supportingFactIds":[],"lookupScopeIds":["method","child","factory","module"]}),
        );
        e
    }

    #[test]
    fn constructor_fields_signatures_and_storage_projection_preserve_provenance() {
        let e = member_fixture();
        let b = bundle(&e);
        assert_eq!(b.legend["modelVersion"], "class-uml-semantics-2");
        let child = b.legend["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["factId"] == "child")
            .unwrap();
        let fields = child["attributes"].as_array().unwrap();
        for name in [
            "errorType",
            "stackTrace",
            "cache",
            "#private",
            "worker",
            "registryId",
            "message",
            "loggerService",
        ] {
            assert_eq!(
                fields.iter().filter(|f| f["name"] == name).count(),
                1,
                "{name}"
            );
        }
        let trace = fields.iter().find(|f| f["name"] == "stackTrace").unwrap();
        assert_eq!(trace["type"], "DeclaredTrace");
        assert_eq!(trace["visibility"], "protected");
        assert!(trace["sourceFactIds"]
            .as_array()
            .unwrap()
            .contains(&json!("field-1")));
        assert!(fields
            .iter()
            .filter(|f| matches!(f["name"].as_str(), Some("errorType" | "cache" | "#private")))
            .all(|f| f["type"].is_null()));
        let ctor = child["methods"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["name"] == "constructor")
            .unwrap();
        assert_eq!(ctor["parameters"][0]["hasDefault"], true);
        for id in [
            "ctor-signature",
            "ctor",
            "ctor-meta",
            "child",
            "child-meta",
            "module",
        ] {
            assert!(
                ctor["sourceFactIds"]
                    .as_array()
                    .unwrap()
                    .contains(&json!(id)),
                "{id}"
            );
            assert!(proof_refs(&b.legend, ctor)
                .iter()
                .any(|r| r["factId"] == id));
        }
        let rs = b.legend["relationships"].as_array().unwrap();
        assert_eq!(
            rs.iter()
                .filter(|r| r["representedAsField"] == true)
                .count(),
            3
        );
        let worker = rs.iter().find(|r| r["memberName"] == "worker").unwrap();
        assert_eq!(worker["representedAsField"], false);
        assert!(worker["projectionSupportingFactIds"]
            .as_array()
            .unwrap()
            .contains(&json!("worker-call-meta")));
        assert_eq!(b.legend["coverage"]["relationsRepresentedAsFields"], 3);
        for g in b.legend["groups"].as_array().unwrap() {
            let n = &g["relationCounts"];
            assert_eq!(
                n["modeled"].as_u64().unwrap(),
                n["shown"].as_u64().unwrap()
                    + n["omitted"].as_u64().unwrap()
                    + n["fields"].as_u64().unwrap()
            );
        }
        let mmd = &b.diagrams[0].mermaid;
        assert!(mmd.contains("constructor(Dep#63;)"));
        assert!(mmd.contains("run#60;T#62;(input#63;#58; T) number"));
        for name in ["registryId", "message", "loggerService"] {
            assert!(!mmd.contains(&format!("supplied parameter: {name}")));
        }
        let mut primitive = member_fixture();
        edit(&mut primitive, "store", "declaredType", json!("string"));
        assert_eq!(
            bundle(&primitive).legend["coverage"]["relationsRepresentedAsFields"],
            4
        );
        let mut static_method = member_fixture();
        edit(&mut static_method, "method-member", "static", json!(true));
        let static_bundle = bundle(&static_method);
        assert_eq!(
            static_bundle.legend["coverage"]["relationsRepresentedAsFields"],
            4
        );
        assert!(static_bundle.diagrams[0].mermaid.contains(" number$"));
    }

    #[test]
    fn state_projection_is_not_a_layout_omission_or_boundary_budget() {
        let mut e = member_fixture();
        for i in 0..35 {
            let mut f: Fact =
                serde_json::from_value(json!(e.facts.iter().find(|f| f.id == "store").unwrap()))
                    .unwrap();
            f.id = format!("extra-store-{i}");
            e.facts.push(f);
            edit(
                &mut e,
                &format!("extra-store-{i}"),
                "memberName",
                json!(format!("data{i}")),
            );
        }
        let b = bundle(&e);
        assert_eq!(b.legend["coverage"]["relationsRepresentedAsFields"], 38);
        assert_eq!(b.legend["coverage"]["relationsOmitted"], 0);
        assert_eq!(b.legend["coverage"]["modeledRelations"], 42);
        assert_eq!(b.legend["coverage"]["relationsDrawn"], 4);
        assert_eq!(
            b.legend["groups"][0]["boundaryIds"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        let child = b.legend["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["factId"] == "child")
            .unwrap();
        assert_eq!(child["fieldCount"], 43);
        assert!(b.html.contains("data34"));
        assert!(b.diagrams[0].mermaid.contains("35 more fields in details"));
        for r in b.legend["relationships"].as_array().unwrap() {
            assert!(r["sourceFactIds"]
                .as_array()
                .unwrap()
                .iter()
                .all(|id| b.legend["proofSources"][id.as_str().unwrap()]["factId"] == *id));
        }
    }

    #[test]
    fn creates_resolves_method_local_class_bindings_without_relaxing_guards() {
        use crate::js_focus::tests::add;
        for binding in ["class", "const"] {
            let mut e = member_fixture();
            add(
                &mut e,
                "inner",
                "class",
                "app.factory.Child.run.Inner",
                32,
                32,
                json!("Inner"),
            );
            add(
                &mut e,
                "inner-meta",
                "js_declaration",
                "app.factory.Child.run.Inner",
                32,
                32,
                json!({"declarationFactId":"inner","ownerFactId":"method","kind":"class","name":"Inner","visibility":"public","publicCandidate":false}),
            );
            add(
                &mut e,
                "inner-guard",
                "js_binding_guard",
                "app.factory.Child.run",
                32,
                32,
                json!({"name":"Inner","bindingKind":binding,"scopeId":"method","scopeKind":"function","scopeStart":400,"scopeEnd":500,"declarationCount":1,"writes":0,"declarationFactId":null}),
            );
            for (key, value) in [
                ("expression", json!("Inner")),
                ("bindingKind", json!("local_class")),
                ("bindingGuardFactId", json!("inner-guard")),
                ("targetFactId", json!("inner")),
            ] {
                edit(&mut e, "new", key, value);
            }
            let r = e.facts.iter_mut().find(|f| f.id == "new").unwrap();
            let mut v: Value = serde_json::from_str(&r.value).unwrap();
            v["supportingFactIds"].as_array_mut().unwrap().extend([
                json!("inner"),
                json!("inner-meta"),
                json!("inner-guard"),
            ]);
            r.value = v.to_string();
            let b = bundle(&e);
            let r = b.legend["relationships"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["relationship"] == "creates")
                .unwrap();
            assert_eq!(r["targetFactId"], "inner");
            assert!(!r["targetClassId"].is_null());
            for (key, value) in [
                ("writes", json!(1)),
                ("scopeId", json!("factory")),
                ("declarationCount", json!(2)),
            ] {
                let mut altered: Evidence = serde_json::from_value(json!(e)).unwrap();
                edit(&mut altered, "inner-guard", key, value);
                let c = crate::catalog::build(&altered, "/source").unwrap();
                assert!(
                    build(&c, &altered, None).is_err(),
                    "accepted {binding} {key}"
                );
            }
        }
    }

    #[test]
    fn static_and_instance_fields_keep_separate_slots_and_citations() {
        use crate::js_focus::tests::add;
        for name in ["errorType", "worker"] {
            let mut e = member_fixture();
            edit(&mut e, "store", "memberName", json!(name));
            add(
                &mut e,
                "static-field",
                "js_member",
                &format!("app.factory.Child.{name}"),
                22,
                22,
                json!({"ownerFactId":"child","memberKind":"field","name":name,"visibility":"private","static":true,"type":"StaticType"}),
            );
            let b = bundle(&e);
            let child = b.legend["entries"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["factId"] == "child")
                .unwrap();
            let slots: Vec<_> = child["attributes"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|m| m["name"] == name)
                .collect();
            assert_eq!(slots.len(), 2);
            let static_slot = slots.iter().find(|m| m["static"] == true).unwrap();
            assert_eq!(static_slot["type"], "StaticType");
            assert_eq!(static_slot["visibility"], "private");
            assert_eq!(static_slot["sourceFactIds"], json!(["static-field"]));
            let instance = slots.iter().find(|m| m["static"] == false).unwrap();
            assert!(instance["type"].is_null());
            assert!(instance["sourceFactIds"]
                .as_array()
                .unwrap()
                .contains(&json!("store")));
            assert!(!instance["sourceFactIds"]
                .as_array()
                .unwrap()
                .contains(&json!("static-field")));
            if name == "errorType" {
                assert!(instance["sourceFactIds"]
                    .as_array()
                    .unwrap()
                    .contains(&json!("field-0")));
            }
            assert!(b.diagrams[0]
                .mermaid
                .contains(&format!("-{name} : StaticType$")));
        }
    }

    #[test]
    fn type_keywords_are_not_named_class_dependency_evidence() {
        for keyword in ["any", "unknown", "object", "void", "never", "this"] {
            let mut e = member_fixture();
            edit(&mut e, "data-store-0", "declaredType", json!(keyword));
            edit(&mut e, "store", "declaredType", json!(keyword));
            let b = bundle(&e);
            let relationships = b.legend["relationships"].as_array().unwrap();
            assert_eq!(
                relationships
                    .iter()
                    .find(|r| r["memberName"] == "registryId")
                    .unwrap()["representedAsField"],
                true,
                "{keyword} is a type keyword, not an identified class or named type"
            );
            assert_eq!(
                relationships
                    .iter()
                    .find(|r| r["memberName"] == "worker")
                    .unwrap()["representedAsField"],
                false,
                "an independently observed receiver call still supplies dependency evidence"
            );
        }
    }

    #[test]
    fn legacy_primitive_markers_remain_field_only_without_rewriting_saved_types() {
        for marker in [
            "<StringKeyword>",
            "<NumberKeyword>",
            "<BooleanKeyword>",
            "<BigIntKeyword>",
            "<SymbolKeyword>",
            "<NullKeyword>",
            "<UndefinedKeyword>",
        ] {
            let mut e = member_fixture();
            edit(&mut e, "store", "declaredType", json!(marker));
            // New helper receipts must use source types, not a renderer fallback.
            let current = bundle(&e);
            assert_eq!(
                current.legend["relationships"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|r| r["memberName"] == "worker")
                    .unwrap()["representedAsField"],
                false
            );
            e.extractor_version = crate::javascript::CLASS_RELATION_EXTRACTOR_VERSION.into();
            for f in &mut e.facts {
                if matches!(
                    f.kind.as_str(),
                    "js_class_relation" | "js_class_reference" | "js_class_scope"
                ) {
                    let mut v: Value = serde_json::from_str(&f.value).unwrap();
                    v["policy"] = json!("javascript-class-relations-1");
                    if f.kind == "js_class_relation" {
                        v.as_object_mut().unwrap().remove("importFactId");
                    }
                    f.value = v.to_string();
                }
            }
            let b = bundle(&e);
            let r = b.legend["relationships"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["memberName"] == "worker")
                .unwrap();
            assert_eq!(r["representedAsField"], true, "{marker}");
            assert_eq!(r["declaredType"], marker);
        }
    }

    #[test]
    fn member_receipts_and_extractor_policies_fail_closed() {
        for (id, key, value) in [
            ("field-0", "ownerFactId", json!("method")),
            ("field-0", "classFactId", json!("parent")),
            ("field-0", "supportingFactIds", json!(["child", "ctor"])),
            ("field-0", "sourceType", json!("literal")),
            ("field-0", "memberName", json!("nested.field")),
            ("field-3", "visibility", json!("public")),
            ("ctor-signature", "ownerFactId", json!("factory")),
            ("ctor-signature", "declarationFactId", json!("method")),
            ("ctor-signature", "returnType", json!(false)),
            ("ctor-signature", "returnType", json!("X".repeat(4097))),
            ("ctor-signature", "parameters", json!(vec![json!({}); 65])),
            ("ctor-signature", "typeParameters", json!(vec!["T"; 17])),
            ("ctor-signature", "defaultValue", json!("MUST_NOT_RETAIN")),
            (
                "scope-method",
                "policy",
                json!("javascript-class-relations-1"),
            ),
            (
                "reference-store",
                "policy",
                json!("javascript-class-relations-1"),
            ),
            ("store", "policy", json!("javascript-class-relations-1")),
            (
                "store",
                "lookupScopeIds",
                json!(["child", "factory", "module"]),
            ),
        ] {
            let mut e = member_fixture();
            edit(&mut e, id, key, value);
            let c = crate::catalog::build(&e, "/source").unwrap();
            assert!(build(&c, &e, None).is_err(), "accepted {id}.{key}");
        }
        let mut e = member_fixture();
        let f = e.facts.iter_mut().find(|f| f.id == "store").unwrap();
        let mut v: Value = serde_json::from_str(&f.value).unwrap();
        v.as_object_mut().unwrap().remove("importFactId");
        f.value = v.to_string();
        let c = crate::catalog::build(&e, "/source").unwrap();
        assert!(build(&c, &e, None).is_err());
        let mut e = javascript();
        edit(
            &mut e,
            "inherit",
            "policy",
            json!("javascript-class-relations-2"),
        );
        let c = crate::catalog::build(&e, "/source").unwrap();
        assert!(build(&c, &e, None).is_err());
    }

    #[test]
    fn scoped_parameter_dependencies_and_inheritance_never_guess_provider_names() {
        let mut e = javascript();
        use crate::js_focus::tests::add;
        add(
            &mut e,
            "same-name-provider",
            "class",
            "app.Base",
            11,
            14,
            json!("Base"),
        );
        add(
            &mut e,
            "same-name-provider-meta",
            "js_declaration",
            "app.Base",
            11,
            14,
            json!({"declarationFactId":"same-name-provider","ownerFactId":"module","kind":"class","name":"Base","publicCandidate":true,"visibility":"public"}),
        );
        let b = bundle(&e);
        let relations = b.legend["relationships"].as_array().unwrap();
        assert_eq!(relations.len(), 4);
        assert!(relations.iter().all(|r| r["targetClassId"].is_null()));
        assert!(relations.iter().all(|r| proof_refs(&b.legend, r).len() >= 4
            && proof_refs(&b.legend, r)
                .iter()
                .all(|s| s["path"].is_string())));
        assert!(b.diagrams[0].mermaid.contains("<|--"));
        assert!(b.diagrams[0].mermaid.contains("..>"));
        assert!(b.diagrams[0].mermaid.contains("stores argument"));
        assert!(b.diagrams[0].mermaid.contains("service : DeclaredService"));
        assert!(!b.diagrams[0].mermaid.contains("*--"));
        assert!(!b.diagrams[0].mermaid.contains("o--"));
        assert!(!b
            .html
            .contains("JavaScript base expressions remain unresolved boundaries; no inheritance"));
    }

    #[test]
    fn old_javascript_snapshots_keep_unresolved_bases_and_observed_members() {
        use crate::js_focus::tests::add;
        let mut e = javascript();
        e.facts.retain(|f| {
            !matches!(
                f.kind.as_str(),
                "js_class_relation" | "js_class_reference" | "js_class_scope"
            )
        });
        add(
            &mut e,
            "method-member",
            "js_member",
            "app.factory.Child.run",
            31,
            40,
            json!({"declarationFactId":"method","ownerFactId":"child","memberKind":"method","name":"run","visibility":"public","static":false}),
        );
        for version in [
            crate::javascript::LEGACY_EXTRACTOR_VERSION,
            crate::javascript::MODULE_EXTRACTOR_VERSION,
            crate::javascript::STRUCTURAL_EXTRACTOR_VERSION,
        ] {
            e.extractor_version = version.into();
            let b = bundle(&e);
            assert_eq!(b.legend["relationships"].as_array().unwrap().len(), 1);
            assert!(b.legend["relationships"][0]["targetClassId"].is_null());
            assert!(b.diagrams[0].mermaid.contains("+run()"));
            assert!(!b.diagrams[0].mermaid.contains("..>"));
        }
    }

    #[test]
    fn local_extends_requires_same_file_class_guard_proof() {
        let mut e = javascript();
        e.facts.retain(|f| f.id != "base");
        let f = e.facts.iter_mut().find(|f| f.id == "inherit").unwrap();
        let mut v: Value = serde_json::from_str(&f.value).unwrap();
        v["expression"] = json!("Parent");
        v["bindingKind"] = json!("local_class");
        v["bindingGuardFactId"] = json!("parent-guard");
        v["targetFactId"] = json!("parent");
        v["supportingFactIds"] = json!([
            "parent",
            "parent-meta",
            "parent-guard",
            "child",
            "child-meta",
            "reference-inherit",
            "scope-child",
            "scope-factory",
            "scope-module"
        ]);
        f.value = v.to_string();
        let b = bundle(&e);
        let parent = b.legend["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["factId"] == "parent")
            .unwrap();
        let child = b.legend["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["factId"] == "child")
            .unwrap();
        assert!(b.diagrams[0].mermaid.contains(&format!(
            "{} <|-- {}",
            parent["diagramId"].as_str().unwrap(),
            child["diagramId"].as_str().unwrap()
        )));
        for (id, key, value) in [
            ("inherit", "targetFactId", json!("factory")),
            ("inherit", "targetFactId", json!("child")),
            ("inherit", "bindingGuardFactId", json!("base-guard")),
            ("inherit", "ownerFactId", json!("ctor")),
            ("inherit", "supportingFactIds", json!([])),
            ("parent-guard", "writes", json!(1)),
            ("parent-guard", "declarationCount", json!(2)),
            ("parent-guard", "scopeEnd", json!(0)),
            ("dep-guard", "name", json!("Wrong")),
            ("dep-guard", "scopeId", json!("module")),
        ] {
            let mut altered: Evidence =
                serde_json::from_value(serde_json::to_value(&e).unwrap()).unwrap();
            let f = altered.facts.iter_mut().find(|f| f.id == id).unwrap();
            let mut v: Value = serde_json::from_str(&f.value).unwrap();
            v[key] = value;
            f.value = v.to_string();
            let c = crate::catalog::build(&altered, "/source").unwrap();
            assert!(build(&c, &altered, None).is_err(), "accepted {id}.{key}");
        }
    }

    #[test]
    fn nearest_shadowing_guard_and_direct_owner_matrix_fail_closed() {
        use crate::js_focus::tests::add;
        for case in [
            "shadow",
            "stores",
            "callback",
            "inner-class",
            "outer-extends",
            "missing-scopes",
            "reversed-scopes",
            "duplicate-scopes",
        ] {
            let mut e = javascript();
            if case == "shadow" {
                add(
                    &mut e,
                    "shadow",
                    "js_binding_guard",
                    "app.factory.Child.run",
                    31,
                    31,
                    json!({"name":"Base","bindingKind":"parameter","scopeId":"method","scopeKind":"function","scopeStart":400,"scopeEnd":500,"declarationCount":1,"writes":0,"declarationFactId":null}),
                );
            } else if case == "callback" {
                add(
                    &mut e,
                    "callback",
                    "function",
                    "app.factory.Child.run.callback",
                    32,
                    34,
                    json!("callback"),
                );
                add(
                    &mut e,
                    "callback-meta",
                    "js_declaration",
                    "app.factory.Child.run.callback",
                    32,
                    34,
                    json!({"declarationFactId":"callback","ownerFactId":"method","kind":"callback","name":"callback","visibility":"public","publicCandidate":false}),
                );
            } else if case == "inner-class" {
                add(
                    &mut e,
                    "inner",
                    "class",
                    "app.factory.Child.Inner",
                    31,
                    40,
                    json!("Inner"),
                );
                add(
                    &mut e,
                    "inner-meta",
                    "js_declaration",
                    "app.factory.Child.Inner",
                    31,
                    40,
                    json!({"declarationFactId":"inner","ownerFactId":"child","kind":"class","name":"Inner","visibility":"public","publicCandidate":true}),
                );
                let f = e.facts.iter_mut().find(|f| f.id == "method-meta").unwrap();
                let mut v: Value = serde_json::from_str(&f.value).unwrap();
                v["ownerFactId"] = json!("inner");
                f.value = v.to_string();
            }
            let id = if case == "outer-extends" {
                "inherit"
            } else {
                "use"
            };
            let f = e.facts.iter_mut().find(|f| f.id == id).unwrap();
            let mut v: Value = serde_json::from_str(&f.value).unwrap();
            match case {
                "stores" => {
                    v["relationship"] = json!("stores_parameter");
                    v["memberName"] = json!("fakeField");
                }
                "callback" => {
                    v["ownerFactId"] = json!("callback");
                }
                "outer-extends" => {
                    v["ownerFactId"] = json!("factory");
                }
                "missing-scopes" => {
                    v.as_object_mut().unwrap().remove("lookupScopeIds");
                }
                "reversed-scopes" => {
                    v["lookupScopeIds"] = json!(["module", "factory", "child", "method"]);
                }
                "duplicate-scopes" => {
                    v["lookupScopeIds"] = json!(["method", "factory", "factory", "module"]);
                }
                _ => {}
            }
            f.value = v.to_string();
            let c = crate::catalog::build(&e, "/source").unwrap();
            assert!(build(&c, &e, None).is_err(), "accepted {case}");
        }
    }

    #[test]
    fn independent_reference_and_scope_graph_reject_truncated_or_reanchored_chains() {
        use crate::js_focus::tests::add;
        for original in [javascript(), member_fixture()] {
            bundle(&original);
            for (case, edits) in [
                (
                    "truncated",
                    vec![("use", "lookupScopeIds", json!(["factory", "module"]))],
                ),
                (
                    "drop-inner",
                    vec![(
                        "use",
                        "lookupScopeIds",
                        json!(["child", "factory", "module"]),
                    )],
                ),
                (
                    "drop-middle",
                    vec![(
                        "use",
                        "lookupScopeIds",
                        json!(["method", "factory", "module"]),
                    )],
                ),
                (
                    "starts-root",
                    vec![("use", "lookupScopeIds", json!(["module"]))],
                ),
                (
                    "unknown-substitution",
                    vec![(
                        "use",
                        "lookupScopeIds",
                        json!(["method", "unknown", "factory", "module"]),
                    )],
                ),
                (
                    "parent-gap",
                    vec![("scope-method", "parentScopeId", json!("unknown"))],
                ),
                (
                    "parent-cycle",
                    vec![("scope-method", "parentScopeId", json!("method"))],
                ),
                (
                    "non-nested-parent",
                    vec![("scope-child", "parentScopeId", json!("ctor"))],
                ),
                (
                    "non-module-root",
                    vec![("scope-method", "parentScopeId", Value::Null)],
                ),
                (
                    "guard-graph-kind",
                    vec![("scope-factory", "scopeKind", json!("class"))],
                ),
                (
                    "guard-graph-span",
                    vec![("scope-factory", "start", json!(101))],
                ),
                (
                    "unknown-reference-scope",
                    vec![("reference-use", "scopeId", json!("unknown"))],
                ),
                (
                    "reference-outside-scope",
                    vec![
                        ("reference-use", "start", json!(510)),
                        ("reference-use", "end", json!(514)),
                    ],
                ),
                (
                    "reference-wrong-class",
                    vec![("reference-use", "classFactId", json!("parent"))],
                ),
                (
                    "reference-wrong-owner",
                    vec![("reference-use", "ownerFactId", json!("ctor"))],
                ),
                (
                    "reference-wrong-policy",
                    vec![("reference-use", "policy", json!("unknown"))],
                ),
                (
                    "missing-reference",
                    vec![("use", "referenceFactId", Value::Null)],
                ),
                (
                    "reanchor-to-outer",
                    vec![
                        ("reference-use", "scopeId", json!("factory")),
                        ("use", "lookupScopeIds", json!(["factory", "module"])),
                    ],
                ),
                (
                    "reanchor-to-root",
                    vec![
                        ("reference-use", "scopeId", json!("module")),
                        ("use", "lookupScopeIds", json!(["module"])),
                    ],
                ),
                (
                    "skip-registered-middle",
                    vec![
                        ("scope-method", "parentScopeId", json!("factory")),
                        (
                            "use",
                            "lookupScopeIds",
                            json!(["method", "factory", "module"]),
                        ),
                    ],
                ),
                (
                    "shadow-and-truncate",
                    vec![("use", "lookupScopeIds", json!(["factory", "module"]))],
                ),
                ("scope-collision", vec![]),
                ("reference-line", vec![]),
            ] {
                let mut e: Evidence =
                    serde_json::from_value(serde_json::to_value(&original).unwrap()).unwrap();
                // Isolate this receipt so rejection cannot come from another relation.
                e.facts
                    .retain(|f| f.kind != "js_class_relation" || f.id == "use");
                for (id, key, value) in edits {
                    let f = e.facts.iter_mut().find(|f| f.id == id).unwrap();
                    let mut v: Value = serde_json::from_str(&f.value).unwrap();
                    v[key] = value;
                    f.value = v.to_string();
                }
                if case == "shadow-and-truncate" {
                    add(
                        &mut e,
                        "shadow",
                        "js_binding_guard",
                        "app.factory.Child.run",
                        31,
                        31,
                        json!({"name":"Base","bindingKind":"parameter","scopeId":"method","scopeKind":"function","scopeStart":400,"scopeEnd":500,"declarationCount":1,"writes":0,"declarationFactId":null}),
                    );
                } else if case == "scope-collision" {
                    add(
                        &mut e,
                        "collision",
                        "js_class_scope",
                        "app",
                        31,
                        40,
                        json!({"policy":"javascript-class-relations-1","scopeId":"method","parentScopeId":"child","scopeKind":"function","start":400,"end":500}),
                    );
                } else if case == "reference-line" {
                    let f = e
                        .facts
                        .iter_mut()
                        .find(|f| f.id == "reference-use")
                        .unwrap();
                    f.line = 33;
                    f.end_line = 33;
                }
                let c = crate::catalog::build(&e, "/source").unwrap();
                assert!(build(&c, &e, None).is_err(), "accepted {case}");
            }
            for missing in [
                "reference-use",
                "scope-method",
                "scope-child",
                "scope-factory",
                "scope-module",
            ] {
                let mut e: Evidence =
                    serde_json::from_value(serde_json::to_value(&original).unwrap()).unwrap();
                let f = e.facts.iter_mut().find(|f| f.id == "use").unwrap();
                let mut v: Value = serde_json::from_str(&f.value).unwrap();
                v["supportingFactIds"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|id| id != missing);
                f.value = v.to_string();
                let c = crate::catalog::build(&e, "/source").unwrap();
                assert!(
                    build(&c, &e, None).is_err(),
                    "accepted missing proof {missing}"
                );
            }
        }
    }

    #[test]
    #[ignore = "requires installed mmdc and Chromium"]
    fn mermaid_syntax_with_installed_renderer() {
        for e in [evidence(0), javascript()] {
            for d in bundle(&e).diagrams {
                crate::render::draft::svg(&d.mermaid, std::path::Path::new("mmdc")).unwrap();
            }
        }
    }

    fn saved_javascript(source: &str) -> Evidence {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("app.ts"), source).unwrap();
        let e = crate::javascript::extract(directory.path()).unwrap();
        assert!(e.complete, "{:?}", e.diagnostics);
        directory.close().unwrap();
        e
    }

    #[test]
    fn helper_method_local_class_declarations_and_expressions_keep_instance_state_distinct() {
        let e = saved_javascript(
            r#"
class Outer {
  static value: number = 1;
  constructor(value) { this.value = value; this.cache = 'CACHE_SENTINEL'; }
  make() { class Inner {} return new Inner(); }
  expression() { const Inner = class {}; return new Inner(); }
}
"#,
        );
        let b = bundle(&e);
        let outer = b.legend["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["symbol"] == "app.ts.Outer")
            .unwrap();
        let creates: Vec<_> = b.legend["relationships"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["classId"] == outer["id"] && r["relationship"] == "creates")
            .collect();
        assert_eq!(creates.len(), 2);
        let mut targets = BTreeSet::new();
        for r in creates {
            assert_eq!(r["bindingKind"], "local_class");
            assert!(!r["targetClassId"].is_null());
            assert!(r["sourceFactIds"]
                .as_array()
                .unwrap()
                .contains(&r["targetFactId"]));
            targets.insert(r["targetFactId"].as_str().unwrap());
        }
        assert_eq!(targets.len(), 2);
        let values: Vec<_> = outer["attributes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| m["name"] == "value")
            .collect();
        assert_eq!(values.len(), 2);
        let static_slot = values.iter().find(|m| m["static"] == true).unwrap();
        let instance = values.iter().find(|m| m["static"] == false).unwrap();
        assert_eq!(static_slot["type"], "number");
        assert!(instance["type"].is_null());
        assert!(proof_refs(&b.legend, static_slot)
            .iter()
            .all(|r| r["kind"] == "js_member"));
        for kind in ["js_constructor_field", "js_class_relation"] {
            assert!(proof_refs(&b.legend, instance)
                .iter()
                .any(|r| r["kind"] == kind));
        }
        assert!(!b.legend.to_string().contains("CACHE_SENTINEL"));
    }

    #[test]
    fn helper_nine_parameter_properties_report_eight_visible_and_one_hidden_projection() {
        let parameters = (0..9)
            .map(|i| format!("public field{i}: number"))
            .collect::<Vec<_>>()
            .join(", ");
        let e = saved_javascript(&format!(
            "class Counted {{ constructor({parameters}) {{}} }}"
        ));
        let b = bundle(&e);
        let entry = &b.legend["entries"][0];
        let fields = entry["attributes"].as_array().unwrap();
        assert_eq!(fields.len(), 9);
        assert_eq!(entry["visibleFieldCount"], 8);
        assert_eq!(entry["omittedFieldCount"], 1);
        assert_eq!(fields.iter().filter(|f| f["visible"] == true).count(), 8);
        assert_eq!(b.legend["coverage"]["modeledRelations"], 9);
        assert_eq!(b.legend["coverage"]["relationsDrawn"], 0);
        assert_eq!(b.legend["coverage"]["relationsOmitted"], 0);
        assert_eq!(b.legend["coverage"]["relationsRepresentedAsFields"], 9);
        assert_eq!(
            b.legend["coverage"]["relationsRepresentedAsFieldsVisible"],
            8
        );
        assert_eq!(
            b.legend["coverage"]["relationsRepresentedAsFieldsHidden"],
            1
        );
        let g = &b.legend["groups"][0];
        assert_eq!(
            g["relationsRepresentedAsFieldsVisible"]
                .as_array()
                .unwrap()
                .len(),
            8
        );
        assert_eq!(
            g["relationsRepresentedAsFieldsHidden"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(g["relationCounts"]["fields"], 9);
        assert_eq!(g["relationCounts"]["fieldsVisible"], 8);
        assert_eq!(g["relationCounts"]["fieldsHidden"], 1);
        let hidden = fields.iter().find(|f| f["visible"] == false).unwrap();
        assert!(!b.diagrams[0]
            .mermaid
            .contains(&format!("+{} : number", hidden["name"].as_str().unwrap())));
        assert!(b.diagrams[0].mermaid.contains("1 more fields in details"));
        assert!(b
            .html
            .contains("9 projected as fields (8 visible, 1 hidden"));
        assert!(b.html.contains(hidden["name"].as_str().unwrap()));
        assert!(fields.iter().all(|f| proof_refs(&b.legend, f)
            .iter()
            .any(|r| r["kind"] == "js_class_relation")));
        assert_eq!(
            entry["methods"][0]["parameters"].as_array().unwrap().len(),
            9
        );
    }

    #[test]
    fn helper_computed_and_string_methods_do_not_supply_orphan_signatures_or_values() {
        let e = saved_javascript(
            r#"
class Unusual {
  constructor(value: 'SOURCE_TYPE_SENTINEL' = 'CTOR_DEFAULT_SENTINEL') {
    this.value = value;
    this.cache = 'FIELD_LITERAL_SENTINEL';
  }
  ['fixed'](value = 'COMPUTED_DEFAULT_SENTINEL') { return value; }
  "quoted"(value = 'QUOTED_DEFAULT_SENTINEL') { return value; }
  [Symbol.iterator]() { return null; }
  normal(value = 'NORMAL_DEFAULT_SENTINEL') { return value; }
}
"#,
        );
        let b = bundle(&e);
        let entry = &b.legend["entries"][0];
        let methods = entry["methods"].as_array().unwrap();
        assert_eq!(methods.len(), 2);
        for name in ["constructor", "normal"] {
            let m = methods.iter().find(|m| m["name"] == name).unwrap();
            assert_eq!(m["parameters"][0]["hasDefault"], true);
            assert!(proof_refs(&b.legend, m)
                .iter()
                .any(|r| r["kind"] == "js_callable_signature"));
        }
        for signature in e.facts.iter().filter(|f| f.kind == "js_callable_signature") {
            let v: Value = serde_json::from_str(&signature.value).unwrap();
            assert!(e.facts.iter().filter(|f| f.kind == "js_member").any(
                |f| serde_json::from_str::<Value>(&f.value).unwrap()["declarationFactId"]
                    == v["declarationFactId"]
            ));
        }
        assert!(e
            .facts
            .iter()
            .filter(|f| f.kind == "js_constructor_field")
            .all(|f| serde_json::from_str::<Value>(&f.value).unwrap()["sourceType"].is_null()));
        let saved_and_rendered = format!(
            "{}{}{}{}",
            json!(e),
            b.legend,
            b.html,
            b.diagrams[0].mermaid
        );
        for sentinel in [
            "SOURCE_TYPE_SENTINEL",
            "CTOR_DEFAULT_SENTINEL",
            "FIELD_LITERAL_SENTINEL",
            "COMPUTED_DEFAULT_SENTINEL",
            "QUOTED_DEFAULT_SENTINEL",
            "NORMAL_DEFAULT_SENTINEL",
        ] {
            assert!(
                !saved_and_rendered.contains(sentinel),
                "retained {sentinel}"
            );
        }
    }

    #[test]
    fn helper_primitive_types_remain_storage_fields_even_with_receiver_calls() {
        let e = saved_javascript("class ErrorData { constructor(message: string) { this.message = message; } run(): string { return this.message.trim(); } }");
        let b = bundle(&e);
        let r = b.legend["relationships"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["relationship"] == "stores_parameter")
            .unwrap();
        assert_eq!(
            r["declaredType"], "string",
            "helper must preserve source primitive spellings"
        );
        assert_eq!(r["representedAsField"], true);
        assert_eq!(b.legend["coverage"]["relationsRepresentedAsFields"], 1);
        assert!(b.diagrams[0].mermaid.contains("run() string"));
        assert!(!b.diagrams[0].mermaid.contains("supplied parameter"));
    }

    #[test]
    fn helper_commonjs_import_receipt_cannot_promote_a_preinitialization_reference() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(
            directory.path().join("base.ts"),
            "class Base {} module.exports = {Base};",
        )
        .unwrap();
        std::fs::write(directory.path().join("app.ts"), "class Early extends Late {}\nconst {Base: Late} = require('./base');\nclass Ready extends Late {}").unwrap();
        let mut e = crate::javascript::extract(directory.path()).unwrap();
        assert!(e.complete);
        directory.close().unwrap();
        let original = bundle(&e);
        let entries = original.legend["entries"].as_array().unwrap();
        let early = entries
            .iter()
            .find(|c| c["symbol"].as_str().unwrap().ends_with(".Early"))
            .unwrap();
        let ready = entries
            .iter()
            .find(|c| c["symbol"].as_str().unwrap().ends_with(".Ready"))
            .unwrap();
        let rs = original.legend["relationships"].as_array().unwrap();
        assert!(
            rs.iter().find(|r| r["classId"] == early["id"]).unwrap()["targetClassId"].is_null()
        );
        let valid_import = rs.iter().find(|r| r["classId"] == ready["id"]).unwrap();
        assert!(
            !valid_import["targetClassId"].is_null(),
            "ready import: {}",
            valid_import["resolution"]
        );
        let relation = e
            .facts
            .iter_mut()
            .find(|f| {
                f.kind == "js_class_relation"
                    && serde_json::from_str::<Value>(&f.value).unwrap()["classFactId"]
                        == early["factId"]
            })
            .unwrap();
        let mut v: Value = serde_json::from_str(&relation.value).unwrap();
        v["bindingKind"] = json!("import");
        v["importFactId"] = valid_import["importFactId"].clone();
        v["supportingFactIds"]
            .as_array_mut()
            .unwrap()
            .push(valid_import["importFactId"].clone());
        relation.value = v.to_string();
        let b = bundle(&e);
        let attempted = b.legend["relationships"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["classId"] == early["id"])
            .unwrap();
        assert!(
            attempted["targetClassId"].is_null(),
            "promoted a preinitialization CommonJS class import: {}",
            attempted["resolution"]
        );
    }

    #[test]
    fn helper_module_classes_and_error_state_survive_deleted_sources() {
        for commonjs in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            std::fs::create_dir(directory.path().join("lib")).unwrap();
            let base = "Bas\u{0301}";
            let alias = "Alias\u{200d}";
            let (base_source, imports, functions, cycle_a, cycle_b, bad) = if commonjs {
                (format!("class {base} {{ run() {{}} }} module.exports = {{{base}}};"),
                    format!("const {{ {base}: {alias} }} = require('./lib/base'); const {{ factory: FunctionAlias }} = require('./functions'); const {{ Missing }} = require('external-package'); const {{ CycleA }} = require('./cycle-a'); const {{ Bad }} = require('./bad');"),
                    "function factory() {} module.exports = {factory};".to_string(),
                    "const {CycleB} = require('./cycle-b'); class CycleA extends CycleB {} module.exports = {CycleA};".to_string(),
                    "const {CycleA} = require('./cycle-a'); class CycleB extends CycleA {} module.exports = {CycleB};".to_string(),
                    "class Bad {} module.exports = {Bad, dynamic: unknownValue};".to_string())
            } else {
                (format!("export class {base} {{ run() {{}} }}"),
                    format!("import {{ {base} as {alias} }} from './lib/base'; import {{ factory as FunctionAlias }} from './functions'; import {{ Missing }} from 'external-package'; import {{ CycleA }} from './cycle-a'; import {{ Bad }} from './bad';"),
                    "export function factory() {}".to_string(),
                    "import {CycleB} from './cycle-b'; export class CycleA extends CycleB {}".to_string(),
                    "import {CycleA} from './cycle-a'; export class CycleB extends CycleA {}".to_string(),
                    "export class Bad {} export * from './functions';".to_string())
            };
            for (path, contents) in [
                ("lib/base.ts", base_source),
                ("functions.ts", functions),
                ("cycle-a.ts", cycle_a),
                ("cycle-b.ts", cycle_b),
                ("bad.ts", bad),
            ] {
                std::fs::write(directory.path().join(path), contents).unwrap();
            }
            let app = format!("{imports}\nclass Child extends {alias} {{ make() {{ return new {alias}(); }} }}\nclass FunctionChild extends FunctionAlias {{}}\nclass External extends Missing {{}}\nclass Cyclic extends CycleA {{}}\nclass BadChild extends Bad {{}}\n") + r#"
class DefaultIntegrationError extends Error {
  constructor(registryId, message = 'SECRET_DEFAULT', worker) {
    super(message);
    this.registryId = registryId;
    this.message = message;
    this.worker = worker;
    this.stackTrace = 'SECRET_STACK';
    this.errorType = { token: 'SECRET_ERROR' };
    this.cache = { value: 'SECRET_CACHE' };
  }
  run() { return this.worker.run(); }
  static from(code?): DefaultIntegrationError { return new DefaultIntegrationError(code, 'SECRET_CALL', null); }
}
"#;
            std::fs::write(directory.path().join("app.ts"), app).unwrap();
            if commonjs {
                std::fs::write(directory.path().join("early.ts"), format!("class Early extends Late {{}} const {{ {base}: Late }} = require('./lib/base');")).unwrap();
            }
            let e = crate::javascript::extract(directory.path()).unwrap();
            assert!(e.complete, "{:?}", e.diagnostics);
            directory.close().unwrap();
            let b = bundle(&e);
            let entries = b.legend["entries"].as_array().unwrap();
            let class = |name: &str| {
                entries
                    .iter()
                    .find(|c| c["symbol"].as_str().unwrap().rsplit('.').next() == Some(name))
                    .unwrap()
            };
            let rs = b.legend["relationships"].as_array().unwrap();
            let child = class("Child");
            let imported: Vec<_> = rs.iter().filter(|r| r["classId"] == child["id"]).collect();
            assert_eq!(imported.len(), 2);
            for r in imported {
                assert_eq!(r["bindingKind"], "import");
                assert_eq!(r["expression"], alias);
                assert_eq!(r["targetClassId"], class(base)["id"]);
                assert_eq!(r["resolution"]["status"], "resolved");
                assert_eq!(r["resolution"]["policy"], "selected-relative-classes-1");
                assert_eq!(
                    r["resolution"]["snapshotInventoryDependency"],
                    "whole saved selected-file module inventory"
                );
                assert!(proof_refs(&b.legend, r)
                    .iter()
                    .any(|p| p["path"] == "lib/base.ts" && p["kind"] == "class"));
                assert!(proof_refs(&b.legend, r)
                    .iter()
                    .any(|p| p["kind"] == "js_module_import" && p["factId"] == r["importFactId"]));
            }
            let home = b.legend["groups"]
                .as_array()
                .unwrap()
                .iter()
                .find(|g| g["homeClassIds"].as_array().unwrap().contains(&child["id"]))
                .unwrap();
            assert!(home["contextClassIds"]
                .as_array()
                .unwrap()
                .contains(&class(base)["id"]));
            assert!(!home["homeClassIds"]
                .as_array()
                .unwrap()
                .contains(&class(base)["id"]));
            for name in ["FunctionChild", "External", "Cyclic", "BadChild"] {
                let r = rs
                    .iter()
                    .find(|r| r["classId"] == class(name)["id"] && r["relationship"] == "extends")
                    .unwrap();
                assert!(r["targetClassId"].is_null(), "promoted {name}: {r}");
                assert_eq!(r["resolution"]["status"], "unresolved");
            }
            if commonjs {
                assert!(rs
                    .iter()
                    .filter(|r| r["classId"] == class("Early")["id"])
                    .all(|r| r["targetClassId"].is_null()));
            }
            let error = class("DefaultIntegrationError");
            let fields = error["attributes"].as_array().unwrap();
            for name in [
                "registryId",
                "message",
                "worker",
                "stackTrace",
                "errorType",
                "cache",
            ] {
                assert_eq!(
                    fields.iter().filter(|f| f["name"] == name).count(),
                    1,
                    "missing/duplicate {name}"
                );
            }
            for name in ["stackTrace", "errorType", "cache"] {
                let f = fields.iter().find(|f| f["name"] == name).unwrap();
                assert!(f["type"].is_null());
                assert!(proof_refs(&b.legend, f)
                    .iter()
                    .any(|r| r["kind"] == "js_constructor_field"));
            }
            for name in ["registryId", "message", "worker"] {
                let r = rs
                    .iter()
                    .find(|r| r["classId"] == error["id"] && r["memberName"] == name)
                    .unwrap();
                assert_eq!(r["representedAsField"], name != "worker", "{r}");
            }
            let ctor = error["methods"]
                .as_array()
                .unwrap()
                .iter()
                .find(|m| m["memberKind"] == "constructor")
                .unwrap();
            assert_eq!(ctor["parameters"].as_array().unwrap().len(), 3);
            assert_eq!(ctor["parameters"][1]["name"], "message");
            assert_eq!(ctor["parameters"][1]["hasDefault"], true);
            assert!(proof_refs(&b.legend, ctor)
                .iter()
                .any(|r| r["kind"] == "js_callable_signature"));
            let output = format!(
                "{}{}{}",
                b.legend,
                b.html,
                b.diagrams
                    .iter()
                    .map(|d| d.mermaid.as_str())
                    .collect::<String>()
            );
            for secret in [
                "SECRET_DEFAULT",
                "SECRET_STACK",
                "SECRET_ERROR",
                "SECRET_CACHE",
                "SECRET_CALL",
            ] {
                assert!(!output.contains(secret), "retained {secret}");
            }
            assert!(b.diagrams.iter().any(|d| d
                .mermaid
                .contains("from(code#63;) DefaultIntegrationError$")));
            let mut legacy: Evidence = serde_json::from_value(json!(e)).unwrap();
            legacy.extractor_version = crate::javascript::CLASS_RELATION_EXTRACTOR_VERSION.into();
            let language = legacy.language.as_mut().unwrap();
            language.parser = "typescript".into();
            language.runtime = "node".into();
            language.parser_version = "synthetic-legacy-typescript".into();
            language.runtime_version = "synthetic-legacy-node".into();
            let imports: BTreeSet<_> = legacy
                .facts
                .iter()
                .filter(|f| f.kind == "js_module_import")
                .map(|f| f.id.clone())
                .collect();
            for f in &mut legacy.facts {
                if matches!(
                    f.kind.as_str(),
                    "js_class_relation" | "js_class_reference" | "js_class_scope"
                ) {
                    let mut v: Value = serde_json::from_str(&f.value).unwrap();
                    v["policy"] = json!("javascript-class-relations-1");
                    if f.kind == "js_class_relation" {
                        v.as_object_mut().unwrap().remove("importFactId");
                        v["supportingFactIds"]
                            .as_array_mut()
                            .unwrap()
                            .retain(|id| !imports.contains(id.as_str().unwrap()));
                    }
                    f.value = v.to_string();
                }
            }
            let legacy_bundle = bundle(&legacy);
            let legacy_relations = legacy_bundle.legend["relationships"].as_array().unwrap();
            assert!(legacy_relations
                .iter()
                .filter(|r| r["classId"] == child["id"])
                .all(|r| r["targetClassId"].is_null()));
            assert!(legacy_relations
                .iter()
                .all(|r| r.get("resolution").is_none()));
            for (kind, key, value, rejects) in [
                (
                    "js_class_relation",
                    "targetFactId",
                    json!(class(base)["factId"]),
                    true,
                ),
                (
                    "js_class_relation",
                    "importFactId",
                    json!(class(base)["factId"]),
                    true,
                ),
                (
                    "js_class_reference",
                    "scopeId",
                    json!("missing-scope"),
                    true,
                ),
                ("js_module_export", "targetKind", json!("function"), false),
                (
                    "js_module_export",
                    "initializedBeforeExport",
                    json!(false),
                    false,
                ),
            ] {
                if !commonjs && key == "initializedBeforeExport" {
                    continue;
                }
                let mut altered: Evidence = serde_json::from_value(json!(e)).unwrap();
                let id = altered
                    .facts
                    .iter()
                    .find(|f| {
                        f.kind == kind
                            && if kind == "js_class_relation" || kind == "js_class_reference" {
                                serde_json::from_str::<Value>(&f.value).unwrap()["classFactId"]
                                    == child["factId"]
                            } else {
                                f.path == "lib/base.ts"
                            }
                    })
                    .unwrap()
                    .id
                    .clone();
                edit(&mut altered, &id, key, value);
                let catalog = crate::catalog::build(&altered, "/deleted/source").unwrap();
                let result = build(&catalog, &altered, None);
                if rejects {
                    assert!(result.is_err(), "accepted {kind}.{key}");
                } else {
                    let boundary = result.unwrap();
                    assert!(
                        boundary.legend["relationships"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .filter(|r| r["classId"] == child["id"])
                            .all(|r| r["targetClassId"].is_null()),
                        "promoted {kind}.{key}"
                    );
                }
            }
        }
    }

    #[test]
    fn helper_receipts_render_without_source_reads_in_model_build() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("app.ts"),r#"
class Parent {}
class Creator { make() { return new Parent(); } }
class Child extends Parent { constructor(public service: DeclaredService) { this.service = service; } }
function factory(Base, Dep) {
  return class extends Base {
    constructor(Argument: DeclaredArgument) {
      super();
      this.argument = Argument;
      this.dep = Dep;
      this.state = {cache: {}};
    }
    run() { return new Dep(); }
  };
}
let Mutable = Parent;
Mutable = Child;
class Unsupported extends Mutable {}
class AbstractModelService {}
function inherited(AbstractModelService) { return class extends AbstractModelService {}; }
function one(Base) { return class extends Base {}; }
function two(Base) { return class extends Base {}; }
class A {} class B {}
{class C extends A {}}{class C extends B {}}
"#.to_string() + "\nclass Cafe\u{0301} {}\nclass Bas\u{200c} {}\nclass UnicodeChild extends Cafe\u{0301} {}\nclass JoinerChild extends Bas\u{200c} {}\nfunction unicode(D\u{200c}, E\u{200d}) { return class extends D\u{200c} { constructor(P\u{0301}: DeclaredType) { super(); this.f\u{200d} = P\u{0301}; this.g\u{0301} = E\u{200d}; } run() { return new E\u{200d}(); } }; }\n").unwrap();
        let e = crate::javascript::extract(directory.path()).unwrap();
        assert!(
            e.facts.iter().any(|f| f.kind == "js_class_relation"),
            "helper did not emit relation receipts"
        );
        // The model must remain valid after its input source directory is gone.
        directory.close().unwrap();
        let b = bundle(&e);
        let rs = b.legend["relationships"].as_array().unwrap();
        assert!(rs
            .iter()
            .any(|r| r["relationship"] == "extends" && !r["targetClassId"].is_null()));
        assert!(rs
            .iter()
            .any(|r| r["relationship"] == "stores_parameter"
                && r["declaredType"] == "DeclaredArgument"));
        assert!(rs
            .iter()
            .any(|r| r["expression"] == "Base" && r["targetClassId"].is_null()));
        assert!(rs.iter().any(|r| r["relationship"] == "creates"));
        assert!(rs.iter().any(|r| r["relationship"] == "creates"
            && r["bindingKind"] == "local_class"
            && !r["targetClassId"].is_null()));
        assert!(rs
            .iter()
            .any(|r| r["expression"] == "AbstractModelService" && r["targetClassId"].is_null()));
        let base_guards: BTreeSet<_> = rs
            .iter()
            .filter(|r| r["expression"] == "Base" && r["relationship"] == "extends")
            .map(|r| r["bindingGuardFactId"].as_str().unwrap())
            .collect();
        assert_eq!(base_guards.len(), 3);
        assert!(!rs.iter().any(|r| r["expression"] == "cache"));
        let classes = b.legend["entries"].as_array().unwrap();
        let repeated: Vec<_> = classes
            .iter()
            .filter(|c| c["symbol"].as_str().unwrap().ends_with(".C"))
            .collect();
        assert_eq!(repeated.len(), 2);
        assert_eq!(repeated[0]["line"], repeated[1]["line"]);
        let mut expressions = BTreeSet::new();
        for c in repeated {
            let bases: Vec<_> = rs
                .iter()
                .filter(|r| r["classId"] == c["id"] && r["relationship"] == "extends")
                .collect();
            assert_eq!(bases.len(), 1);
            assert!(!bases[0]["targetClassId"].is_null());
            expressions.insert(bases[0]["expression"].as_str().unwrap());
        }
        assert_eq!(expressions, BTreeSet::from(["A", "B"]));
        for expression in [
            "Cafe\u{0301}",
            "Bas\u{200c}",
            "D\u{200c}",
            "E\u{200d}",
            "P\u{0301}",
        ] {
            assert!(
                rs.iter().any(|r| r["expression"] == expression),
                "missing exact Unicode expression {expression:?}"
            );
        }
        for expression in ["Cafe\u{0301}", "Bas\u{200c}"] {
            assert!(rs
                .iter()
                .any(|r| r["expression"] == expression && !r["targetClassId"].is_null()));
        }
        assert!(rs.iter().any(|r| r["memberName"] == "f\u{200d}"));
        assert!(rs.iter().any(|r| r["memberName"] == "g\u{0301}"));
        assert!(b
            .diagrams
            .iter()
            .any(|d| d.mermaid.contains("#8204;") || d.mermaid.contains("#8205;")));
        assert!(b
            .diagrams
            .iter()
            .any(|d| d.mermaid.contains("anonymous class alias")));
        let mut tampered: Evidence =
            serde_json::from_value(serde_json::to_value(&e).unwrap()).unwrap();
        let f = tampered
            .facts
            .iter_mut()
            .find(|f| {
                f.kind == "js_class_relation"
                    && serde_json::from_str::<Value>(&f.value).unwrap()["relationship"] == "creates"
            })
            .unwrap();
        let mut v: Value = serde_json::from_str(&f.value).unwrap();
        v["lookupScopeIds"].as_array_mut().unwrap().remove(0);
        f.value = v.to_string();
        let c = crate::catalog::build(&tampered, "/source").unwrap();
        assert!(
            build(&c, &tampered, None).is_err(),
            "accepted real-helper truncated lookup chain"
        );
    }
}
