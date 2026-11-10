//! Bounded, deterministic source inventories, not runtime architecture models.
use crate::analysis::{Evidence, Fact};
use crate::catalog::{Catalog, Kind, Page};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

const MAX_DOCUMENT: usize = 8 * 1024 * 1024;

pub struct Documents {
    pub mermaid: String,
    pub legend: Value,
    pub html: String,
}

fn callable(f: &Fact) -> bool {
    matches!(f.kind.as_str(), "function" | "async_function")
}

fn contains(outer: &Fact, inner: &Fact) -> bool {
    outer.path == inner.path && outer.line <= inner.line && inner.end_line <= outer.end_line
}

pub(super) fn source(f: &Fact) -> Value {
    json!({"factId": f.id, "path": f.path, "line": f.line, "endLine": f.end_line,
        "symbol": f.symbol, "kind": f.kind})
}

pub(super) fn signature_text(member: &Value) -> String {
    let parameters = member["parameters"]
        .as_array()
        .into_iter()
        .flatten()
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
                    .map(|ty| format!(": {ty}"))
                    .unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let generics = member["typeParameters"]
        .as_array()
        .filter(|p| !p.is_empty())
        .map(|p| {
            format!(
                "<{}>",
                p.iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
        .unwrap_or_default();
    format!(
        "{}{generics}({parameters}){}",
        member["name"].as_str().unwrap_or("unknown"),
        member["returnType"]
            .as_str()
            .map(|ty| format!(" : {ty}"))
            .unwrap_or_default()
    )
}

fn ascii_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    chars
        .next()
        .is_some_and(|c| c == '_' || c.is_ascii_alphabetic())
        && chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

fn dotted_name(s: &str) -> bool {
    s.split('.').all(|part| {
        let mut chars = part.chars();
        chars.next().is_some_and(|c| c == '_' || c.is_alphabetic())
            && chars.all(|c| c == '_' || c.is_alphanumeric())
    })
}

fn short(s: &str) -> String {
    if s.chars().count() > 64 {
        format!("{}...", s.chars().take(61).collect::<String>())
    } else {
        s.into()
    }
}

fn short_tail(s: &str, limit: usize) -> String {
    let count = s.chars().count();
    if count > limit {
        format!(
            "...{}",
            s.chars().skip(count - (limit - 3)).collect::<String>()
        )
    } else {
        s.into()
    }
}

// Same numeric entity convention as mermaid::escape_label, additionally covering
// controls. Kept local so this module needs no changes to existing renderers.
pub(super) fn escape_label(s: &str) -> String {
    let mut out = String::new();
    for c in s.replace("\r\n", "\n").chars() {
        match c {
            '\n' | '\r' => out.push_str("<br/>"),
            '#' | '&' | '"' | '<' | '>' | '|' | '[' | ']' | '{' | '}' | '(' | ')' | '`' | '\\' => {
                out.push_str(&format!("#{};", c as u32))
            }
            c if c.is_control()
                || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}') =>
            {
                out.push_str(&format!("#{};", c as u32));
            }
            _ => out.push(c),
        }
    }
    out
}

pub(super) fn html_text(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c if c.is_control() && !matches!(c, '\n' | '\r' | '\t') => {
                out.push_str(&format!("&#{};", c as u32));
            }
            _ => out.push(c),
        }
    }
    out
}

// Resolve only simple names through a single lexical declaration or import.
// Conflicting declarations, rebinding, wildcard imports and compound bases are
// boundaries, not permission to choose a plausible class by its short name.
pub(super) fn resolve_base<'a>(
    base: &Fact,
    class: &Fact,
    facts: &[&'a Fact],
) -> Option<(&'a Fact, Vec<&'a Fact>)> {
    if !dotted_name(&base.value) {
        return None;
    }
    let (root, tail) = base
        .value
        .split_once('.')
        .map_or((base.value.as_str(), String::new()), |(root, tail)| {
            (root, format!(".{tail}"))
        });
    let modules: Vec<_> = facts
        .iter()
        .copied()
        .filter(|f| f.kind == "module" && f.path == class.path)
        .collect();
    if modules.len() != 1 {
        return None;
    }
    let module = modules[0];
    let mut scope = class.symbol.rsplit_once('.')?.0.to_string();
    let mut immediate = true;
    loop {
        if scope != module.symbol {
            let owners: Vec<_> = facts
                .iter()
                .copied()
                .filter(|f| {
                    f.path == class.path && f.symbol == scope && (f.kind == "class" || callable(f))
                })
                .collect();
            if owners.len() != 1 || !contains(owners[0], class) {
                return None;
            }
            // A base expression can read its immediate class body's locals,
            // but enclosing class namespaces are not Python closure scopes.
            if owners[0].kind == "class" && !immediate {
                scope = scope.rsplit_once('.')?.0.to_string();
                continue;
            }
        }
        immediate = false;
        let declaration_name = format!("{scope}.{root}");
        if facts.iter().any(|f| {
            f.path == class.path
                && f.kind == "parameter"
                && f.symbol == declaration_name
                && f.line <= class.line
        }) {
            return None;
        }
        let declarations: Vec<_> = facts
            .iter()
            .copied()
            .filter(|f| {
                f.path == class.path
                    && (f.kind == "class" || callable(f))
                    && f.symbol == declaration_name
            })
            .collect();
        let mut imports = Vec::new();
        let mut blocked = false;
        for f in facts
            .iter()
            .copied()
            .filter(|f| f.path == class.path && f.symbol == scope && f.line <= class.line)
        {
            if matches!(
                f.kind.as_str(),
                "binding" | "binding_invalidation" | "parameter"
            ) {
                let v: Value = serde_json::from_str(&f.value).unwrap_or(Value::Null);
                if v["target"].as_str() == Some(root) || f.value == root {
                    blocked = true;
                }
            }
            if !matches!(f.kind.as_str(), "import" | "from_import") {
                continue;
            }
            let v: Value = serde_json::from_str(&f.value).ok()?;
            for alias in v["names"].as_array()? {
                let original = alias["name"].as_str()?;
                if original == "*" {
                    blocked = true;
                }
                let local = alias["asname"].as_str().unwrap_or_else(|| {
                    if f.kind == "import" {
                        original.split('.').next().unwrap_or("")
                    } else {
                        original
                    }
                });
                if local != root {
                    continue;
                }
                let target = if f.kind == "import" {
                    if alias["asname"].is_string() {
                        original
                    } else {
                        local
                    }
                    .to_string()
                } else {
                    let mut prefix = v["module"].as_str()?.to_string();
                    let level = usize::try_from(v["level"].as_u64()?).ok()?;
                    if level > 0 {
                        let mut package: Vec<_> = module.symbol.split('.').collect();
                        if class.path != "__init__.py" && !class.path.ends_with("/__init__.py") {
                            package.pop();
                        }
                        if level > package.len() {
                            return None;
                        }
                        for _ in 1..level {
                            package.pop();
                        }
                        prefix = [package.join("."), prefix]
                            .into_iter()
                            .filter(|s| !s.is_empty())
                            .collect::<Vec<_>>()
                            .join(".");
                    }
                    [prefix, original.into()]
                        .into_iter()
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join(".")
                };
                imports.push((format!("{target}{tail}"), f));
            }
        }
        if blocked || declarations.len() + imports.len() > 1 {
            return None;
        }
        let origin_is_shadowed = |candidate: &Fact, target: &str| {
            facts.iter().any(|f| {
                if f.path != candidate.path
                    || !matches!(
                        f.kind.as_str(),
                        "binding" | "binding_invalidation" | "import" | "from_import"
                    )
                {
                    return false;
                }
                if !target.starts_with(&format!("{}.", f.symbol)) {
                    return false;
                }
                let v: Value = serde_json::from_str(&f.value).unwrap_or(Value::Null);
                let shadows = |name: &str| {
                    let bound = format!("{}.{name}", f.symbol);
                    target == bound || target.starts_with(&format!("{bound}."))
                };
                if matches!(f.kind.as_str(), "import" | "from_import") {
                    // Do not follow re-exports: a competing import in the
                    // originating namespace makes this declaration ambiguous.
                    v["names"].as_array().is_none_or(|names| {
                        names.iter().any(|alias| {
                            let Some(original) = alias["name"].as_str() else {
                                return true;
                            };
                            let local = alias["asname"].as_str().unwrap_or_else(|| {
                                if f.kind == "import" {
                                    original.split('.').next().unwrap_or("")
                                } else {
                                    original
                                }
                            });
                            original == "*" || shadows(local)
                        })
                    })
                } else {
                    v["target"].as_str().is_some_and(shadows)
                }
            })
        };
        if let Some((target, import)) = imports.first() {
            let candidates: Vec<_> = facts
                .iter()
                .copied()
                .filter(|f| (f.kind == "class" || callable(f)) && f.symbol == *target)
                .collect();
            if candidates.len() != 1
                || candidates[0].kind != "class"
                || origin_is_shadowed(candidates[0], target)
            {
                return None;
            }
            return Some((candidates[0], vec![*import]));
        }
        if let Some(declaration) = declarations.first() {
            if declaration.line >= class.line || declaration.kind != "class" {
                return None;
            }
            let target = format!("{}{tail}", declaration.symbol);
            let candidates: Vec<_> = facts
                .iter()
                .copied()
                .filter(|f| f.kind == "class" && f.path == class.path && f.symbol == target)
                .collect();
            return (candidates.len() == 1 && !origin_is_shadowed(candidates[0], &target))
                .then(|| (candidates[0], vec![]));
        }
        if scope == module.symbol {
            return None;
        }
        scope = scope.rsplit_once('.')?.0.to_string();
    }
}

/// Direct Python ownership only: nested functions and ambiguous declarations do not leak members.
pub(super) fn python_members<'a>(
    fact: &Fact,
    facts: &[&'a Fact],
) -> (Vec<&'a Fact>, BTreeMap<String, Vec<&'a Fact>>) {
    let prefix = format!("{}.", fact.symbol);
    let mut methods: Vec<_> = facts
        .iter()
        .copied()
        .filter(|m| {
            callable(m)
                && contains(fact, m)
                && m.symbol
                    .strip_prefix(&prefix)
                    .is_some_and(|s| !s.contains('.'))
                && facts
                    .iter()
                    .filter(|owner| {
                        owner.kind == "class" && owner.symbol == fact.symbol && contains(owner, m)
                    })
                    .count()
                    == 1
        })
        .collect();
    methods.sort_by_key(|m| (m.line, m.end_line, &m.id));
    let mut attributes: BTreeMap<String, Vec<&Fact>> = BTreeMap::new();
    for binding in facts.iter().copied().filter(|b| {
        matches!(b.kind.as_str(), "binding" | "binding_invalidation") && contains(fact, b)
    }) {
        if methods
            .iter()
            .filter(|m| m.symbol == binding.symbol && contains(m, binding))
            .count()
            != 1
        {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(&binding.value) else {
            continue;
        };
        let Some(target) = v["target"].as_str() else {
            continue;
        };
        let Some(field) = target
            .strip_prefix("self.")
            .or_else(|| target.strip_prefix("cls."))
        else {
            continue;
        };
        if !field.is_empty() && !field.contains('.') && dotted_name(field) {
            attributes.entry(field.into()).or_default().push(binding);
        }
    }
    (methods, attributes)
}

/// Explicit JavaScript owner IDs and source-declared visibility, never name heuristics.
pub(super) fn javascript_members(
    fact: &Fact,
    js: &crate::js_focus::Index<'_>,
    by_id: &BTreeMap<&str, &Fact>,
) -> Result<Vec<Value>, String> {
    let mut members = Vec::new();
    for (meta, owner, value) in &js.members {
        if js.facts[*owner].id != fact.id {
            continue;
        }
        let name = value["name"]
            .as_str()
            .ok_or("Invalid JavaScript member name")?;
        let visibility = value["visibility"]
            .as_str()
            .ok_or("Invalid JavaScript visibility")?;
        if !matches!(visibility, "private" | "protected" | "public") {
            return Err("Invalid JavaScript member visibility".into());
        }
        let mut refs = vec![source(js.facts[*meta])];
        let mut ids = vec![js.facts[*meta].id.clone()];
        if let Some(id) = value["declarationFactId"].as_str() {
            let d = by_id
                .get(id)
                .ok_or("Unknown JavaScript member declaration")?;
            refs.push(source(d));
            ids.push(d.id.clone());
        }
        let symbol = value["declarationFactId"]
            .as_str()
            .and_then(|id| by_id.get(id))
            .map(|f| f.symbol.clone())
            .unwrap_or_else(|| format!("{}.{name}", fact.symbol));
        members.push(json!({"name":name,"symbol":symbol,"visibility":visibility,"static":value["static"],
            "memberKind":value["memberKind"],"type":value.get("type").cloned().unwrap_or(json!("UNKNOWN")),
            "sourceRefs":refs,"sourceFactIds":ids}));
    }
    let mut signature_declarations = BTreeSet::new();
    let member_identifier = |s: &str| {
        !s.is_empty()
            && s.len() <= 512
            && !s.as_bytes()[0].is_ascii_digit()
            && s.chars().all(|c| {
                !c.is_control()
                    && !c.is_whitespace()
                    && (!c.is_ascii() || c.is_ascii_alphanumeric() || matches!(c, '_' | '$'))
            })
    };
    for extra in by_id.values().filter(|f| {
        matches!(
            f.kind.as_str(),
            "js_constructor_field" | "js_callable_signature"
        )
    }) {
        let invalid = || format!("Invalid JavaScript class member proof: {}", extra.id);
        let v: Value = serde_json::from_str(&extra.value).map_err(|_| invalid())?;
        let owner = v["ownerFactId"]
            .as_str()
            .and_then(|id| by_id.get(id))
            .ok_or_else(invalid)?;
        let oi = js
            .facts
            .iter()
            .position(|f| f.id == owner.id)
            .ok_or_else(invalid)?;
        if v["policy"] != "javascript-class-members-1" || !contains(owner, extra) {
            return Err(invalid());
        }
        if extra.kind == "js_callable_signature" {
            let declaration = v["declarationFactId"]
                .as_str()
                .and_then(|id| by_id.get(id))
                .ok_or_else(invalid)?;
            let di = js
                .facts
                .iter()
                .position(|f| f.id == declaration.id)
                .ok_or_else(invalid)?;
            let params = v["parameters"].as_array().ok_or_else(invalid)?;
            let generics = v["typeParameters"].as_array().ok_or_else(invalid)?;
            let text = |v: &Value, max: usize| {
                v.as_str().is_some_and(|s| {
                    !s.is_empty() && s.len() <= max && !s.chars().any(char::is_control)
                })
            };
            let ty = |v: &Value| v.is_null() || text(v, 4096);
            if v.as_object().is_none_or(|o| o.len() != 6)
                || owner.kind != "class"
                || !callable(declaration)
                || js.owners.get(&di) != Some(&oi)
                || !js.declarations.get(&di).is_some_and(|(_, d)| {
                    matches!(d["kind"].as_str(), Some("method" | "constructor"))
                })
                || extra.path != declaration.path
                || extra.line != declaration.line
                || extra.end_line != declaration.end_line
                || params.len() > 64
                || generics.len() > 16
                || !ty(&v["returnType"])
                || v.get("returnType").is_none()
                || generics
                    .iter()
                    .any(|g| !g.as_str().is_some_and(member_identifier))
                || params.iter().any(|p| {
                    p.as_object().is_none_or(|o| o.len() != 6)
                        || !text(&p["name"], 4096)
                        || (p["pattern"] == false
                            && !p["name"].as_str().is_some_and(member_identifier))
                        || ["pattern", "optional", "rest", "hasDefault"]
                            .iter()
                            .any(|k| p[*k].as_bool().is_none())
                        || p.get("type").is_none()
                        || !ty(&p["type"])
                })
                || !signature_declarations.insert(declaration.id.as_str())
            {
                return Err(invalid());
            }
            if owner.id == fact.id {
                let m = members
                    .iter_mut()
                    .find(|m| {
                        m["sourceFactIds"]
                            .as_array()
                            .is_some_and(|ids| ids.contains(&json!(declaration.id)))
                    })
                    .ok_or_else(invalid)?;
                m["signature"] = json!({"policy":v["policy"],"declarationFactId":v["declarationFactId"],"ownerFactId":v["ownerFactId"]});
                m["parameters"] = v["parameters"].clone();
                m["returnType"] = v["returnType"].clone();
                m["typeParameters"] = v["typeParameters"].clone();
                m["sourceFactIds"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!(extra.id));
                m["sourceRefs"].as_array_mut().unwrap().push(source(extra));
                for i in js.closure(di)? {
                    let f = js.facts[i];
                    if !m["sourceFactIds"]
                        .as_array()
                        .unwrap()
                        .contains(&json!(f.id))
                    {
                        m["sourceFactIds"].as_array_mut().unwrap().push(json!(f.id));
                        m["sourceRefs"].as_array_mut().unwrap().push(source(f));
                    }
                }
            }
        } else {
            let class = v["classFactId"]
                .as_str()
                .and_then(|id| by_id.get(id))
                .ok_or_else(invalid)?;
            let ci = js
                .facts
                .iter()
                .position(|f| f.id == class.id)
                .ok_or_else(invalid)?;
            let name = v["memberName"].as_str().ok_or_else(invalid)?;
            let plain = name.strip_prefix('#').unwrap_or(name);
            let support = v["supportingFactIds"].as_array().ok_or_else(invalid)?;
            let class_meta = js.declarations.get(&ci).ok_or_else(invalid)?.0;
            let owner_meta = js.declarations.get(&oi).ok_or_else(invalid)?.0;
            if v.as_object().is_none_or(|o| o.len() != 9)
                || class.kind != "class"
                || !callable(owner)
                || js.owners.get(&oi) != Some(&ci)
                || js.declarations[&oi].1["kind"] != "constructor"
                || !contains(class, extra)
                || name.len() > 512
                || !member_identifier(plain)
                || v["visibility"]
                    != if name.starts_with('#') {
                        "private"
                    } else {
                        "public"
                    }
                || v["static"] != false
                || v["assignmentKind"] != "direct-constructor-assignment"
                || v.get("sourceType").is_none()
                || !v["sourceType"].is_null()
                || [
                    class.id.as_str(),
                    owner.id.as_str(),
                    js.facts[class_meta].id.as_str(),
                    js.facts[owner_meta].id.as_str(),
                ]
                .iter()
                .any(|id| !support.contains(&json!(id)))
            {
                return Err(invalid());
            }
            let mut ids = BTreeSet::from([extra.id.as_str()]);
            for id in support {
                let id = id.as_str().ok_or_else(invalid)?;
                let f = by_id.get(id).ok_or_else(invalid)?;
                if f.path != class.path
                    || !(id == class.id
                        || id == owner.id
                        || id == js.facts[class_meta].id
                        || id == js.facts[owner_meta].id
                        || (f.kind == "js_member"
                            && js.members.iter().any(|(mi, co, mv)| {
                                js.facts[*mi].id == id
                                    && *co == ci
                                    && mv["declarationFactId"] == owner.id
                            })))
                {
                    return Err(invalid());
                }
                ids.insert(id);
            }
            if class.id == fact.id {
                let position = members.iter().position(|m| {
                    m["memberKind"] == "field" && m["name"] == name && m["static"] == false
                });
                let m = if let Some(i) = position {
                    &mut members[i]
                } else {
                    members.push(json!({"name":name,"symbol":format!("{}.{name}",class.symbol),"memberKind":"field","visibility":v["visibility"],"static":false,"type":null,"sourceFactIds":[],"sourceRefs":[]}));
                    members.last_mut().unwrap()
                };
                m["observation"] = json!("direct constructor assignment; not definite initialization or lifetime ownership");
                for id in ids {
                    if !m["sourceFactIds"].as_array().unwrap().contains(&json!(id)) {
                        m["sourceFactIds"].as_array_mut().unwrap().push(json!(id));
                        m["sourceRefs"]
                            .as_array_mut()
                            .unwrap()
                            .push(source(by_id[id]));
                    }
                }
            }
        }
    }
    Ok(members)
}

pub(super) fn declared_bases<'a>(fact: &Fact, facts: &[&'a Fact]) -> Vec<&'a Fact> {
    facts
        .iter()
        .copied()
        .filter(|b| b.kind == "base" && b.symbol == fact.symbol && contains(fact, b))
        .collect()
}

pub fn build(
    catalog: &Catalog,
    evidence: &Evidence,
    page: &Page,
    kind: &Kind,
) -> Result<Documents, String> {
    if *kind == Kind::Workflow {
        return Err(
            "Workflow source views are unsupported; use focused analysis for workflows.".into(),
        );
    }
    if page.items.is_empty() || page.items.len() > 30 {
        return Err("source view requires 1..=30 selected items; request a smaller page".into());
    }
    if page.limit == 0
        || page.items.len() > page.limit
        || page.snapshot_id != catalog.snapshot_id
        || catalog.snapshot_id != evidence.snapshot_id
        || catalog.schema_version != crate::catalog::VERSION
        || catalog.extractor_version != evidence.extractor_version
        || page.complete != catalog.complete
        || catalog.complete != evidence.complete
    {
        return Err("source view page, catalog and evidence metadata do not match".into());
    }
    // Rebuilding checks source metadata attachment as well as known fact IDs; a
    // caller-supplied page cannot turn unrelated evidence into a claimed edge.
    let grounded = crate::catalog::build(evidence, &catalog.source_root)?;
    let mut seen = BTreeSet::new();
    for item in &page.items {
        let value = serde_json::to_value(item).map_err(|e| e.to_string())?;
        if item.kind != *kind
            || !seen.insert(&item.id)
            || !catalog
                .items
                .iter()
                .any(|i| i.id == item.id && serde_json::to_value(i).ok().as_ref() == Some(&value))
            || !grounded
                .items
                .iter()
                .any(|i| i.id == item.id && serde_json::to_value(i).ok().as_ref() == Some(&value))
        {
            return Err(
                "selected item is duplicate, wrong-kind, unknown or not source-grounded".into(),
            );
        }
    }
    let mut by_id = BTreeMap::new();
    for f in &evidence.facts {
        by_id.insert(f.id.as_str(), f);
    }
    let facts: Vec<_> = by_id.values().copied().collect();
    let js = if evidence.is_javascript() {
        Some(crate::js_focus::Index::new(facts.clone())?)
    } else {
        None
    };
    let name = if *kind == Kind::Api { "api" } else { "class" };
    let page_number = (page.offset / page.limit).saturating_add(1);
    let title = if *kind == Kind::Api {
        format!("API source candidates \u{2014} page{page_number}")
    } else {
        format!("Class structure \u{2014} page{page_number}")
    };
    let mut warnings: BTreeSet<String> = catalog
        .warnings
        .iter()
        .chain(&page.warnings)
        .chain(&grounded.warnings)
        .cloned()
        .collect();
    warnings.insert("Static snapshot inventory, not a live check or runtime architecture. Paths, symbols and route fragments are local source metadata; keep these documents local.".into());
    let diagram_header = if *kind == Kind::Api {
        "flowchart LR\n"
    } else {
        "classDiagram\n  direction LR\n"
    };
    let mut mermaid = format!("---\ntitle: {title}\n---\n{diagram_header}");
    let mut entries = Vec::new();
    let mut edges = Vec::new();
    let mut handlers: BTreeMap<&str, String> = BTreeMap::new();
    let selected_classes: BTreeMap<_, _> = page
        .items
        .iter()
        .enumerate()
        .map(|(i, item)| (item.fact_id.as_str(), format!("C{:03}", i + 1)))
        .collect();
    for (index, item) in page.items.iter().enumerate() {
        let fact = by_id[item.fact_id.as_str()];
        if fact.line == 0 || fact.end_line < fact.line {
            return Err("selected source declaration has invalid line bounds".into());
        }
        let refs: Vec<_> = item
            .evidence_ids
            .iter()
            .map(|id| {
                by_id
                    .get(id.as_str())
                    .map(|f| source(f))
                    .ok_or_else(|| "selected evidence ID is unknown".to_string())
            })
            .collect::<Result<_, _>>()?;
        let mut entry = serde_json::to_value(item).map_err(|e| e.to_string())?;
        entry["sourceRefs"] = json!(refs);
        entry["confidence"] = json!("source-syntax-only");
        entry["sourceFactIds"] = json!(item.evidence_ids);
        entry["sourceModel"] = json!("static-candidate");
        if *kind == Kind::Api {
            let key = format!("R{:03}", index + 1);
            let next_handler = format!("H{:03}", handlers.len() + 1);
            let handler = handlers
                .entry(&fact.id)
                .or_insert_with(|| {
                    let file = fact.path.rsplit(['/', '\\']).next().unwrap_or(&fact.path);
                    let owner = facts
                        .iter()
                        .copied()
                        .filter(|class| {
                            if let Some(js) = &js {
                                let mut owner = js.facts.iter().position(|f| f.id == fact.id);
                                while let Some(i) = owner {
                                    owner = js.owners.get(&i).copied();
                                    if let Some(parent) = owner {
                                        if js.facts[parent].kind == "class" {
                                            return js.facts[parent].id == class.id;
                                        }
                                    }
                                }
                                return false;
                            }
                            class.kind == "class"
                                && contains(class, fact)
                                && fact
                                    .symbol
                                    .strip_prefix(&class.symbol)
                                    .is_some_and(|tail| tail.starts_with('.'))
                        })
                        .max_by_key(|class| {
                            (
                                class.symbol.len(),
                                class.line,
                                std::cmp::Reverse(class.end_line),
                                &class.id,
                            )
                        });
                    let owner_name = owner.map_or_else(
                        || file.rsplit_once('.').map_or(file, |(stem, _)| stem),
                        |class| class.symbol.rsplit('.').next().unwrap_or(&class.symbol),
                    );
                    let callable_name = fact.symbol.rsplit('.').next().unwrap_or(&fact.symbol);
                    // Keep the distinctive name tails, not a shared module prefix.
                    let label = format!(
                        "{next_handler} {}\n{}",
                        short_tail(owner_name, 24),
                        short_tail(callable_name, 32)
                    );
                    let escaped = escape_label(&label);
                    mermaid.push_str(&format!(
                        "  {next_handler}[\"{}\"]\n",
                        if js.is_some() {
                            escaped.replace('$', "#36;")
                        } else {
                            escaped
                        }
                    ));
                    next_handler
                })
                .clone();
            let route_label = format!(
                "{} {} [fragment]",
                item.method.as_deref().unwrap_or("UNKNOWN method"),
                item.route_path.as_deref().unwrap_or("DYNAMIC route")
            );
            mermaid.push_str(&format!(
                "  {key}[\"{}\"]\n",
                escape_label(&short(&route_label))
            ));
            mermaid.push_str(&format!("  {key} -->|\"declares handler\"| {handler}\n\n"));
            let metadata: Vec<_> = item
                .evidence_ids
                .iter()
                .filter(|id| by_id[id.as_str()].kind == "api_route")
                .cloned()
                .collect();
            edges.push(json!({"from": key, "to": handler, "relationship": "declares handler",
                "sourceFactIds": metadata, "handlerFactId": fact.id, "confidence": "source-syntax-only"}));
            entry["diagramId"] = json!(key);
            entry["handlerId"] = json!(handler);
            entry["fullRouteLabel"] = json!(route_label);
            entry["pathIsFragment"] = json!(item.path_is_fragment);
            entry["methodUnknown"] = json!(item.method.is_none());
            entry["dynamicRoute"] = json!(item.route_path.is_none());
            if item.method.is_none() {
                warnings.insert("Some route methods are unknown; none was inferred.".into());
            }
            if item.route_path.is_none() {
                warnings.insert("Some route paths are dynamic and unresolved.".into());
            }
        } else if let Some(js) = &js {
            let key = &selected_classes[item.fact_id.as_str()];
            let display = fact.symbol.rsplit('.').next().unwrap_or(&fact.symbol);
            mermaid.push_str(&format!(
                "  class {key}[\"{}\"] {{\n",
                escape_label(&short(display)).replace('$', "#36;")
            ));
            let mut methods = Vec::new();
            let mut attributes = Vec::new();
            let mut visible_methods = 0;
            let mut visible_attributes = 0;
            for mut detail in javascript_members(fact, js, &by_id)? {
                let value = &detail;
                let name = value["name"]
                    .as_str()
                    .ok_or("Invalid JavaScript member name")?;
                let visibility = value["visibility"]
                    .as_str()
                    .ok_or("Invalid JavaScript visibility")?;
                let access = match visibility {
                    "private" => '-',
                    "protected" => '#',
                    "public" => '+',
                    _ => return Err("Invalid JavaScript member visibility".into()),
                };
                let method = value["memberKind"] != "field";
                let count = if method {
                    &mut visible_methods
                } else {
                    &mut visible_attributes
                };
                let visible = *count < if method { 12 } else { 8 };
                if visible {
                    *count += 1;
                    mermaid.push_str(&format!(
                        "    {access}{}{}{}\n",
                        escape_label(&short(name)).replace('$', "#36;"),
                        if method { "()" } else { "" },
                        if value["static"] == true { "$" } else { "" }
                    ));
                }
                detail["visible"] = json!(visible);
                detail["omissionReason"] = json!(if visible { None } else { Some("member limit") });
                if method {
                    methods.push(detail);
                } else {
                    attributes.push(detail);
                }
            }
            mermaid.push_str("  }\n\n");
            let bases: Vec<_> = declared_bases(fact, &facts).into_iter()
                .map(|b|json!({"expression":b.value,"diagramId":null,"status":"unresolved boundary","sourceRefs":[source(b)],"sourceFactIds":[b.id]})).collect();
            warnings.insert("JavaScript members use explicit class owner IDs and declared visibility; inheritance, prototypes and DI remain unresolved boundaries.".into());
            entry["diagramId"] = json!(key);
            entry["visibleMethods"] = json!(visible_methods);
            entry["visibleAttributes"] = json!(visible_attributes);
            entry["omittedMethods"] = json!(methods.len() - visible_methods);
            entry["omittedAttributes"] = json!(attributes.len() - visible_attributes);
            entry["methods"] = json!(methods);
            entry["attributes"] = json!(attributes);
            entry["declaredBases"] = json!(bases);
            entry["unresolvedInherits"] = json!(bases);
        } else {
            let key = &selected_classes[item.fact_id.as_str()];
            let display = fact.symbol.rsplit('.').next().unwrap_or(&fact.symbol);
            mermaid.push_str(&format!(
                "  class {key}[\"{}\"] {{\n",
                escape_label(&short(display))
            ));
            let prefix = format!("{}.", fact.symbol);
            let (methods, attributes) = python_members(fact, &facts);
            let mut method_details = Vec::new();
            let mut visible_methods = 0;
            for m in &methods {
                let label = m.symbol.strip_prefix(&prefix).unwrap_or("");
                let safe = ascii_identifier(label);
                let visible = safe && visible_methods < 12;
                if visible {
                    visible_methods += 1;
                    let access = if label.starts_with('_') { '-' } else { '+' };
                    mermaid.push_str(&format!("    {access}{label}()\n"));
                }
                if !safe {
                    warnings.insert("Non-ASCII or unsupported member names are omitted from Mermaid; full labels remain in the legend.".into());
                }
                method_details.push(json!({"name": label, "symbol": m.symbol, "visible": visible,
                    "omissionReason": if visible { None } else if !safe { Some("unsupported identifier") } else { Some("member limit") },
                    "parameters": "UNKNOWN (not rendered)", "sourceRefs": [source(m)], "sourceFactIds": [m.id]}));
            }
            let mut attribute_details = Vec::new();
            let mut visible_attributes = 0;
            for (field, bindings) in &attributes {
                let safe = ascii_identifier(field);
                let visible = safe && visible_attributes < 8;
                if visible {
                    visible_attributes += 1;
                    let access = if field.starts_with('_') { '-' } else { '+' };
                    mermaid.push_str(&format!("    {access}{field}\n"));
                }
                if !safe {
                    warnings.insert("Non-ASCII or unsupported member names are omitted from Mermaid; full labels remain in the legend.".into());
                }
                attribute_details.push(
                    json!({"name": field, "visible": visible, "type": "UNKNOWN",
                    "sourceRefs": bindings.iter().map(|b| source(b)).collect::<Vec<_>>(),
                    "sourceFactIds": bindings.iter().map(|b| &b.id).collect::<Vec<_>>()}),
                );
            }
            mermaid.push_str("  }\n\n");
            let mut bases = Vec::new();
            for base in declared_bases(fact, &facts) {
                let resolved = resolve_base(base, fact, &facts);
                let selected = resolved.as_ref().and_then(|(target, _)| {
                    selected_classes
                        .get(target.id.as_str())
                        .filter(|_| target.id != fact.id)
                });
                let mut citations = vec![source(base)];
                let mut ids = vec![base.id.clone()];
                if let Some((target, imports)) = &resolved {
                    citations.push(source(target));
                    ids.push(target.id.clone());
                    for import in imports {
                        citations.push(source(import));
                        ids.push(import.id.clone());
                    }
                }
                if let Some(base_key) = selected {
                    edges.push(json!({"from": base_key, "to": key, "relationship": "declared inheritance",
                        "sourceFactIds": ids, "sourceRefs": citations, "confidence": "source-syntax-only"}));
                } else {
                    warnings.insert("Some declared base expressions are unresolved, ambiguous, compound, or outside this page; no inheritance edge was inferred for them.".into());
                }
                bases.push(json!({"expression": base.value, "diagramId": selected,
                    "status": if selected.is_some() { "selected declaration" } else { "unresolved boundary" },
                    "sourceRefs": citations, "sourceFactIds": ids}));
            }
            entry["diagramId"] = json!(key);
            entry["methods"] = json!(method_details);
            entry["attributes"] = json!(attribute_details);
            entry["visibleMethods"] = json!(visible_methods);
            entry["visibleAttributes"] = json!(visible_attributes);
            entry["omittedMethods"] = json!(methods.len() - visible_methods);
            entry["omittedAttributes"] = json!(attributes.len() - visible_attributes);
            entry["declaredBases"] = json!(bases);
            entry["unresolvedInherits"] = json!(bases
                .iter()
                .filter(|b| b["diagramId"].is_null())
                .collect::<Vec<_>>());
        }
        entries.push(entry);
    }
    if *kind == Kind::Api {
        warnings.insert("pathIsFragment: route literals are fragments, not full URLs. Router prefixes, API activation and real request flow are not established; edges only map declaration metadata to handlers. No service edges are inferred.".into());
    } else {
        for edge in &edges {
            mermaid.push_str(&format!(
                "  {} <|-- {}\n",
                edge["from"].as_str().unwrap(),
                edge["to"].as_str().unwrap()
            ));
        }
        if js.is_none() {
            warnings.insert("Only directly owned source methods and direct self/cls field bindings are listed; this is not an exhaustive attribute or type model. Composition and injection dependencies are not inferred. No implementation or runtime topology is proposed.".into());
        }
    }
    let legend = json!({"schemaVersion": "ideomancer.source-view.v1", "kind": name,
        "title": title, "snapshotId": catalog.snapshot_id, "sourceRoot": catalog.source_root,
        "extractorVersion": catalog.extractor_version, "sourceModel": "static-candidate",
        "complete": page.complete, "entries": entries, "edges": edges, "warnings": warnings,
        "page": {"number": page_number, "offset": page.offset, "limit": page.limit,
            "selectedItems": page.items.len(), "totalItems": page.total_items,
            "matchedItems": page.matched_items, "nextOffset": page.next_offset, "counts": page.counts},
        "layout": {"direction": "LR", "compactLabelCharacters": 64,
            "maxVisibleMethods": 12, "maxVisibleAttributes": 8},
        "reportCode": "STATIC_SOURCE_INVENTORY"});
    let legend_text = serde_json::to_string_pretty(&legend).map_err(|e| e.to_string())?;
    if mermaid.len() > MAX_DOCUMENT || legend_text.len() > MAX_DOCUMENT {
        return Err("source view document exceeds the 8 MiB limit; request a smaller page".into());
    }
    // A preformatted inventory preserves every full label and citation without
    // making source paths, symbols, or URL-shaped literals into active links.
    let selection_note = if *kind == Kind::Class {
        format!(
            "<p><strong>Focused selection: {} of {} catalog classes shown.</strong> This is not the application-wide UML. An all-class bundle may not exist. Generate one with <code>ideomancer diagram SAVEDDIR --kind class</code>, replacing SAVEDDIR with the saved catalog directory.</p>",
            page.items.len(),
            catalog.items.iter().filter(|item| item.kind == Kind::Class).count()
        )
    } else {
        String::new()
    };
    let html = super::viewer::document(
        &title,
        &format!("<h1>{}</h1><p>Static snapshot source inventory, not a live runtime check. Keep proprietary source metadata local.</p>{}{}<h2>SourceRefs and full entry details</h2><pre>{}</pre>",
            html_text(&title), selection_note, super::viewer::diagram("Static source diagram", "Static source candidates. Full labels and source references follow."), html_text(&legend_text)),
    );
    if html.len() > MAX_DOCUMENT {
        return Err("source view HTML exceeds the 8 MiB limit; request a smaller page".into());
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
    use crate::catalog::{self, Filter};

    #[test]
    fn javascript_members_keep_owner_visibility_caps_and_unresolved_bases() {
        let mut e = crate::js_focus::tests::source();
        use crate::js_focus::tests::add;
        add(
            &mut e,
            "class",
            "class",
            "app.$Service",
            50,
            99,
            json!("$Service"),
        );
        add(
            &mut e,
            "class-meta",
            "js_declaration",
            "app.$Service",
            50,
            99,
            json!({"declarationFactId":"class","ownerFactId":"module","kind":"class","name":"$Service","publicCandidate":true,"visibility":"public"}),
        );
        add(
            &mut e,
            "base",
            "base",
            "app.$Service",
            50,
            50,
            json!("ExternalBase"),
        );
        for n in 0..15 {
            let name = if n == 0 {
                "_public".to_string()
            } else {
                format!("$method{n}")
            };
            let id = format!("method{n}");
            let line = 51 + n;
            add(
                &mut e,
                &id,
                "function",
                &format!("app.$Service.{name}"),
                line,
                line,
                json!(name),
            );
            add(
                &mut e,
                &format!("dm{n}"),
                "js_declaration",
                &format!("app.$Service.{name}"),
                line,
                line,
                json!({"declarationFactId":id,"ownerFactId":"class","kind":"method","name":name,"publicCandidate":true,"visibility":"public"}),
            );
            add(
                &mut e,
                &format!("member{n}"),
                "js_member",
                &format!("app.$Service.{name}"),
                line,
                line,
                json!({"declarationFactId":id,"ownerFactId":"class","memberKind":"method","name":name,"visibility":"public","static":false}),
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
                json!({"ownerFactId":"class","memberKind":"field","name":if n==0 {"#private".into()} else {format!("field{n}")},"visibility":if n==0 {"private"} else {"protected"},"static":false,"type":"RedactedShape"}),
            );
        }
        let d = documents(&e, Kind::Class);
        let entry = &d.legend["entries"][0];
        assert_eq!(entry["visibleMethods"], 12);
        assert_eq!(entry["omittedMethods"], 3);
        assert_eq!(entry["visibleAttributes"], 8);
        assert_eq!(entry["omittedAttributes"], 2);
        assert!(d.mermaid.contains("+_public()"));
        assert!(d.mermaid.contains("-#35;private"));
        assert!(d.legend["edges"].as_array().unwrap().is_empty());
        assert_eq!(entry["declaredBases"][0]["status"], "unresolved boundary");
        let broken = e.facts.iter_mut().find(|f| f.id == "field0").unwrap();
        let mut v: Value = serde_json::from_str(&broken.value).unwrap();
        v["ownerFactId"] = json!("work");
        broken.value = v.to_string();
        assert!(catalog::build(&e, "local").is_err());
    }

    fn fact(id: &str, kind: &str, symbol: &str, line: usize, end: usize, value: &str) -> Fact {
        Fact {
            id: id.into(),
            kind: kind.into(),
            path: "pkg/app.py".into(),
            symbol: symbol.into(),
            line,
            end_line: end,
            value: value.into(),
        }
    }

    fn evidence(facts: Vec<Fact>) -> Evidence {
        Evidence {
            schema_version: "ideomancer.evidence.v1".into(),
            snapshot_id: "fixed".into(),
            extractor_version: "synthetic".into(),
            python_version: "synthetic".into(),
            language: None,
            files: vec![],
            facts,
            diagnostics: vec![],
            complete: true,
        }
    }

    fn class_source() -> Evidence {
        evidence(vec![
            fact("module", "module", "pkg.app", 1, 1, ""),
            fact("base", "class", "pkg.app.Base", 2, 9, "Base"),
            fact("class", "class", "pkg.app.Child", 10, 80, "Child"),
            fact("inherits", "base", "pkg.app.Child", 10, 10, "Base"),
            fact(
                "method",
                "async_function",
                "pkg.app.Child.submit",
                12,
                20,
                "submit",
            ),
            fact(
                "field",
                "binding",
                "pkg.app.Child.submit",
                13,
                13,
                r#"{"target":"self.client","source":"Client()"}"#,
            ),
            fact(
                "nested",
                "function",
                "pkg.app.Child.submit.nested",
                15,
                17,
                "",
            ),
            fact(
                "leak",
                "binding",
                "pkg.app.Child.submit.nested",
                16,
                16,
                r#"{"target":"self.leak"}"#,
            ),
            fact("helper", "function", "pkg.app.Child._helper", 22, 24, ""),
        ])
    }

    fn documents(e: &Evidence, kind: Kind) -> Documents {
        let c = catalog::build(e, "/local/source").unwrap();
        let p = catalog::page(
            &c,
            &Filter {
                kind: Some(kind.clone()),
                limit: 12,
                ..Filter::default()
            },
        )
        .unwrap();
        build(&c, e, &p, &kind).unwrap()
    }

    fn api_source() -> Evidence {
        evidence(vec![
            fact(
                "route",
                "api_route",
                "pkg.app.handler",
                1,
                1,
                r#"{"methods":["GET","POST"],"path":"/private/{id}","pathIsFragment":true}"#,
            ),
            fact(
                "dynamic",
                "api_route",
                "pkg.app.handler",
                2,
                2,
                r#"{"methods":[],"path":null,"pathIsFragment":true}"#,
            ),
            fact("handler", "function", "pkg.app.handler", 3, 8, ""),
        ])
    }

    #[test]
    fn api_fragments_dynamic_methods_and_citations_not_request_topology() {
        let d = documents(&api_source(), Kind::Api);
        assert!(!d.html.contains("../class/index.html"));
        assert!(d.mermaid.contains("\nflowchart LR\n"));
        assert!(d.mermaid.contains("API source candidates \u{2014} page1"));
        assert_eq!(d.mermaid.matches("declares handler").count(), 3);
        assert_eq!(d.mermaid.matches("H001[\"").count(), 1);
        assert!(!d.mermaid.contains("H002"));
        assert!(d.mermaid.contains("UNKNOWN method DYNAMIC route"));
        assert_eq!(d.legend["entries"].as_array().unwrap().len(), 3);
        assert!(d.legend["entries"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["pathIsFragment"] == true));
        assert!(d.legend["edges"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["handlerFactId"] == "handler"));
        assert!(d.legend.to_string().contains("Router prefixes"));
        assert!(!d.mermaid.contains("service"));
        assert!(d.mermaid.contains("H001 app<br/>handler"));

        let prefix = "pkg.synthetic.long_shared_module_prefix.Router";
        let mut e = evidence(vec![
            fact("router", "class", prefix, 1, 50, "Router"),
            fact(
                "register",
                "function",
                &format!("{prefix}.register"),
                2,
                49,
                "",
            ),
        ]);
        for (index, name) in ["route_a", "route_b"].into_iter().enumerate() {
            let symbol = format!("{prefix}.register.{name}");
            let line = 10 + index * 10;
            e.facts.push(fact(
                &format!("route{index}"),
                "api_route",
                &symbol,
                line,
                line,
                &json!({"methods":["GET"],"path":format!("/{name}"),"pathIsFragment":true})
                    .to_string(),
            ));
            e.facts.push(fact(
                &format!("handler{index}"),
                "function",
                &symbol,
                line + 1,
                line + 5,
                "",
            ));
        }
        let d = documents(&e, Kind::Api);
        assert!(d.mermaid.contains("H001 Router<br/>route_a"));
        assert!(d.mermaid.contains("H002 Router<br/>route_b"));
        assert!(!d.mermaid.contains("long_shared_module_prefix"));
        assert_eq!(d.legend["edges"].as_array().unwrap().len(), 2);
        for entry in d.legend["entries"].as_array().unwrap() {
            let symbol = entry["symbol"].as_str().unwrap();
            assert!(symbol.starts_with(&format!("{prefix}.register.route_")));
            assert!(d.html.contains(symbol));
            assert_eq!(entry["path"], "pkg/app.py");
            assert!(entry["sourceRefs"]
                .as_array()
                .unwrap()
                .iter()
                .all(
                    |reference| reference["symbol"] == symbol && reference["path"] == "pkg/app.py"
                ));
        }
        assert_eq!(
            short_tail(&format!("{}route_b", "x".repeat(80)), 32),
            format!("...{}route_b", "x".repeat(22))
        );
    }

    #[test]
    fn direct_members_inheritance_and_no_scope_leak() {
        let d = documents(&class_source(), Kind::Class);
        assert!(!d.html.contains("href=\"../class/index.html\""));
        assert!(d.html.contains("ideomancer diagram SAVEDDIR --kind class"));
        assert!(d.mermaid.contains("\nclassDiagram\n"));
        assert!(d.mermaid.contains("Class structure \u{2014} page1"));
        assert!(d.mermaid.contains("class C001[\"Base\"]"));
        assert!(d.mermaid.contains("+submit()") && d.mermaid.contains("-_helper()"));
        assert!(d.mermaid.contains("+client\n"));
        assert!(d.mermaid.contains("C001 <|-- C002"));
        assert!(!d.mermaid.contains("nested") && !d.mermaid.contains("leak"));
        assert!(!d.mermaid.contains("*--"));
        assert!(d
            .legend
            .to_string()
            .contains("Composition and injection dependencies are not inferred"));
    }

    #[test]
    fn base_lookup_skips_outer_class_but_reads_immediate_class_body() {
        let e = evidence(vec![
            fact("module", "module", "pkg.app", 1, 1, ""),
            fact("module_base", "class", "pkg.app.Base", 2, 3, "Base"),
            fact("outer", "class", "pkg.app.Outer", 5, 50, "Outer"),
            fact("outer_base", "class", "pkg.app.Outer.Base", 6, 7, "Base"),
            fact("make", "function", "pkg.app.Outer.make", 10, 20, "make"),
            fact(
                "method_child",
                "class",
                "pkg.app.Outer.make.Child",
                12,
                15,
                "Child",
            ),
            fact(
                "method_base",
                "base",
                "pkg.app.Outer.make.Child",
                12,
                12,
                "Base",
            ),
            fact(
                "body_child",
                "class",
                "pkg.app.Outer.Child",
                22,
                25,
                "Child",
            ),
            fact("body_base", "base", "pkg.app.Outer.Child", 22, 22, "Base"),
            fact("inner", "class", "pkg.app.Outer.Inner", 30, 40, "Inner"),
            fact(
                "deep_child",
                "class",
                "pkg.app.Outer.Inner.Child",
                32,
                35,
                "Child",
            ),
            fact(
                "deep_base",
                "base",
                "pkg.app.Outer.Inner.Child",
                32,
                32,
                "Base",
            ),
        ]);
        let d = documents(&e, Kind::Class);
        let entries = d.legend["entries"].as_array().unwrap();
        let edges = d.legend["edges"].as_array().unwrap();
        assert_eq!(edges.len(), 3);
        for (child, base, declaration) in [
            ("method_child", "module_base", "method_base"),
            ("body_child", "outer_base", "body_base"),
            ("deep_child", "module_base", "deep_base"),
        ] {
            let child_id = &entries.iter().find(|e| e["factId"] == child).unwrap()["diagramId"];
            let base_id = &entries.iter().find(|e| e["factId"] == base).unwrap()["diagramId"];
            let edge = edges.iter().find(|e| &e["to"] == child_id).unwrap();
            assert_eq!(&edge["from"], base_id);
            assert_eq!(edge["sourceFactIds"], json!([declaration, base]));
            assert_eq!(edge["sourceRefs"][1]["factId"], base);
        }
    }

    #[test]
    fn base_lookup_retains_function_captures_across_class_boundaries() {
        let e = evidence(vec![
            fact("module", "module", "pkg.app", 1, 1, ""),
            fact("module_base", "class", "pkg.app.Base", 2, 3, "Base"),
            fact("factory", "function", "pkg.app.factory", 5, 60, "factory"),
            fact(
                "captured_base",
                "class",
                "pkg.app.factory.Base",
                6,
                7,
                "Base",
            ),
            fact("outer", "class", "pkg.app.factory.Outer", 10, 50, "Outer"),
            fact(
                "outer_base",
                "class",
                "pkg.app.factory.Outer.Base",
                11,
                12,
                "Base",
            ),
            fact(
                "make",
                "function",
                "pkg.app.factory.Outer.make",
                15,
                30,
                "make",
            ),
            fact(
                "child",
                "class",
                "pkg.app.factory.Outer.make.Child",
                20,
                25,
                "Child",
            ),
            fact(
                "inherits",
                "base",
                "pkg.app.factory.Outer.make.Child",
                20,
                20,
                "Base",
            ),
        ]);
        let d = documents(&e, Kind::Class);
        let entries = d.legend["entries"].as_array().unwrap();
        let edges = d.legend["edges"].as_array().unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(
            edges[0]["sourceFactIds"],
            json!(["inherits", "captured_base"])
        );
        assert_eq!(
            edges[0]["from"],
            entries
                .iter()
                .find(|e| e["factId"] == "captured_base")
                .unwrap()["diagramId"]
        );
        assert_eq!(
            edges[0]["to"],
            entries.iter().find(|e| e["factId"] == "child").unwrap()["diagramId"]
        );

        let mut repeated = e;
        repeated.facts.push(fact(
            "other_factory",
            "function",
            "pkg.app.factory",
            70,
            90,
            "factory",
        ));
        let d = documents(&repeated, Kind::Class);
        assert!(d.legend["edges"].as_array().unwrap().is_empty());
        assert!(
            d.legend["entries"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["factId"] == "child")
                .unwrap()["unresolvedInherits"]
                .as_array()
                .unwrap()
                .len()
                == 1
        );
    }

    #[test]
    fn imported_alias_and_relative_import_resolution() {
        for (kind, import) in [
            (
                "from_import",
                r#"{"module":"base","level":1,"names":[{"name":"Base","asname":"Alias"}]}"#,
            ),
            (
                "import",
                r#"{"module":"","level":0,"names":[{"name":"pkg.base","asname":"b"}]}"#,
            ),
        ] {
            let mut e = class_source();
            e.facts[1].path = "pkg/base.py".into();
            e.facts[1].symbol = "pkg.base.Base".into();
            e.facts[3].value = if kind == "import" { "b.Base" } else { "Alias" }.into();
            e.facts.push(fact("import", kind, "pkg.app", 1, 1, import));
            let d = documents(&e, Kind::Class);
            assert_eq!(d.legend["edges"].as_array().unwrap().len(), 1);
            assert!(d.legend["edges"][0]["sourceFactIds"]
                .as_array()
                .unwrap()
                .contains(&json!("import")));
        }
    }

    #[test]
    fn imported_base_origin_import_rebinding_is_an_unresolved_boundary() {
        for (nested, scope, kind, value, blocked) in [
            (
                false,
                "pkg.base",
                "from_import",
                r#"{"module":"pkg.alternate","level":0,"names":[{"name":"Base","asname":null}]}"#,
                true,
            ),
            (
                false,
                "pkg.base",
                "import",
                r#"{"module":"","level":0,"names":[{"name":"pkg.alternate","asname":"Base"}]}"#,
                true,
            ),
            (
                true,
                "pkg.base",
                "from_import",
                r#"{"module":"pkg.alternate","level":0,"names":[{"name":"Other","asname":"Outer"}]}"#,
                true,
            ),
            (
                true,
                "pkg.base.Outer",
                "from_import",
                r#"{"module":"pkg.alternate","level":0,"names":[{"name":"Base","asname":null}]}"#,
                true,
            ),
            (
                false,
                "pkg.base",
                "from_import",
                r#"{"module":"pkg.alternate","level":0,"names":[{"name":"*","asname":null}]}"#,
                true,
            ),
            (
                false,
                "pkg.base.make",
                "from_import",
                r#"{"module":"pkg.alternate","level":0,"names":[{"name":"Base","asname":null}]}"#,
                false,
            ),
            (
                false,
                "pkg.base.Outer",
                "import",
                r#"{"module":"","level":0,"names":[{"name":"pkg.alternate","asname":"Base"}]}"#,
                false,
            ),
        ] {
            let mut e = class_source();
            e.facts[1].path = "pkg/base.py".into();
            e.facts[1].symbol = if nested {
                "pkg.base.Outer.Base"
            } else {
                "pkg.base.Base"
            }
            .into();
            e.facts[1].end_line = 3;
            e.facts[3].value = if nested { "Outer.Base" } else { "Base" }.into();
            let mut outer = fact("origin_outer", "class", "pkg.base.Outer", 1, 9, "Outer");
            outer.path = "pkg/base.py".into();
            e.facts.push(outer);
            let mut make = fact("origin_make", "function", "pkg.base.make", 5, 9, "make");
            make.path = "pkg/base.py".into();
            e.facts.push(make);
            let mut alternate = fact(
                "alternate_base",
                "class",
                "pkg.alternate.Base",
                1,
                3,
                "Base",
            );
            alternate.path = "pkg/alternate.py".into();
            e.facts.push(alternate);
            e.facts.push(fact("import", "from_import", "pkg.app", 1, 1,
                &json!({"module":"pkg.base","level":0,"names":[{"name":if nested { "Outer" } else { "Base" },"asname":null}]}).to_string()));

            // The ordinary control must cite the original declaration exactly.
            let control = documents(&e, Kind::Class);
            let edges = control.legend["edges"].as_array().unwrap();
            assert_eq!(edges.len(), 1);
            assert_eq!(
                edges[0]["sourceFactIds"],
                json!(["inherits", "base", "import"])
            );
            let entries = control.legend["entries"].as_array().unwrap();
            assert_eq!(
                edges[0]["from"],
                entries.iter().find(|e| e["factId"] == "base").unwrap()["diagramId"]
            );
            assert_eq!(
                edges[0]["to"],
                entries.iter().find(|e| e["factId"] == "class").unwrap()["diagramId"]
            );

            let mut shadow = fact("origin_import", kind, scope, 8, 8, value);
            shadow.path = "pkg/base.py".into();
            e.facts.push(shadow);
            let d = documents(&e, Kind::Class);
            let edges = d.legend["edges"].as_array().unwrap();
            if blocked {
                assert!(
                    edges.is_empty(),
                    "competing origin import must never use the original base: {scope} {value}"
                );
                let child = d.legend["entries"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|e| e["factId"] == "class")
                    .unwrap();
                assert_eq!(
                    child["unresolvedInherits"][0]["sourceFactIds"],
                    json!(["inherits"])
                );
            } else {
                assert_eq!(d.legend["edges"], control.legend["edges"]);
            }
        }
    }

    #[test]
    fn local_dotted_base_origin_rebinding_is_an_unresolved_boundary() {
        for (kind, value, blocked) in [
            (
                "from_import",
                r#"{"module":"pkg.alternate","level":0,"names":[{"name":"Base","asname":null}]}"#,
                true,
            ),
            (
                "import",
                r#"{"module":"","level":0,"names":[{"name":"pkg.alternate","asname":"Base"}]}"#,
                true,
            ),
            (
                "from_import",
                r#"{"module":"pkg.alternate","level":0,"names":[{"name":"*","asname":null}]}"#,
                true,
            ),
            ("binding", r#"{"target":"Base"}"#, true),
            ("binding_invalidation", r#"{"target":"Base"}"#, true),
            (
                "from_import",
                r#"{"module":"pkg.alternate","level":0,"names":[{"name":"Other","asname":null}]}"#,
                false,
            ),
        ] {
            let mut e = class_source();
            e.facts[1].symbol = "pkg.app.Outer.Base".into();
            e.facts[1].end_line = 3;
            e.facts[3].value = "Outer.Base".into();
            e.facts
                .push(fact("outer", "class", "pkg.app.Outer", 1, 9, "Outer"));
            let mut alternate = fact(
                "alternate_base",
                "class",
                "pkg.alternate.Base",
                1,
                3,
                "Base",
            );
            alternate.path = "pkg/alternate.py".into();
            e.facts.push(alternate);

            let control = documents(&e, Kind::Class);
            assert_eq!(control.legend["edges"].as_array().unwrap().len(), 1);
            assert_eq!(
                control.legend["edges"][0]["sourceFactIds"],
                json!(["inherits", "base"])
            );

            e.facts
                .push(fact("origin_rebinding", kind, "pkg.app.Outer", 8, 8, value));
            let d = documents(&e, Kind::Class);
            if blocked {
                assert!(
                    d.legend["edges"].as_array().unwrap().is_empty(),
                    "competing origin binding must not use the original dotted base: {kind} {value}"
                );
                assert!(!d.mermaid.contains("<|--"));
                let child = d.legend["entries"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|e| e["factId"] == "class")
                    .unwrap();
                assert_eq!(child["declaredBases"][0]["status"], "unresolved boundary");
                assert_eq!(
                    child["unresolvedInherits"][0]["sourceFactIds"],
                    json!(["inherits"])
                );
            } else {
                assert_eq!(d.legend["edges"], control.legend["edges"]);
            }
        }
    }

    #[test]
    fn ambiguous_generic_and_off_page_bases_are_boundaries() {
        for expression in ["Base[T]", "factory()", "Base, Other", "external.Base"] {
            let mut e = class_source();
            e.facts[3].value = expression.into();
            let d = documents(&e, Kind::Class);
            assert!(d.legend["edges"].as_array().unwrap().is_empty());
            assert!(d.legend.to_string().contains(expression));
        }
        let mut e = class_source();
        e.facts
            .push(fact("duplicate", "class", "pkg.app.Base", 90, 100, "Base"));
        assert!(documents(&e, Kind::Class).legend["edges"]
            .as_array()
            .unwrap()
            .is_empty());
        let mut e = class_source();
        e.facts.push(fact(
            "rebound",
            "binding_invalidation",
            "pkg.app",
            9,
            9,
            r#"{"target":"Base"}"#,
        ));
        assert!(documents(&e, Kind::Class).legend["edges"]
            .as_array()
            .unwrap()
            .is_empty());
        let e = class_source();
        let c = catalog::build(&e, "root").unwrap();
        let p = catalog::page(
            &c,
            &Filter {
                kind: Some(Kind::Class),
                query: Some("Child".into()),
                ..Filter::default()
            },
        )
        .unwrap();
        let d = build(&c, &e, &p, &Kind::Class).unwrap();
        assert!(!d.mermaid.contains("Base") && !d.mermaid.contains("<|--"));
    }

    #[test]
    fn repeated_class_bounds_and_member_limits() {
        let mut e = class_source();
        for n in 0..15 {
            e.facts.push(fact(
                &format!("m{n}"),
                "function",
                &format!("pkg.app.Child.method{n}"),
                30 + n,
                30 + n,
                "",
            ));
        }
        for n in 0..10 {
            e.facts.push(fact(
                &format!("a{n}"),
                "binding",
                "pkg.app.Child.submit",
                14,
                14,
                &json!({"target":format!("cls.attr{n}"),"source":"Unknown()"}).to_string(),
            ));
        }
        e.facts.extend([
            fact("repeat", "class", "pkg.app.Child", 100, 110, "Child"),
            fact(
                "repeat_method",
                "function",
                "pkg.app.Child.later",
                101,
                102,
                "",
            ),
        ]);
        let d = documents(&e, Kind::Class);
        let entries = d.legend["entries"].as_array().unwrap();
        let first = entries.iter().find(|e| e["factId"] == "class").unwrap();
        assert_eq!(first["visibleMethods"], 12);
        assert_eq!(first["omittedMethods"], 5);
        assert_eq!(first["visibleAttributes"], 8);
        assert_eq!(first["omittedAttributes"], 3);
        assert!(!first["methods"].to_string().contains("later"));
        let later = entries.iter().find(|e| e["factId"] == "repeat").unwrap();
        assert_eq!(later["methods"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn hostile_source_is_inert_and_full_labels_survive() {
        let hostile = "/private/\"[]`<script>\n\u{1b}javascript:bad";
        let mut e = api_source();
        e.facts[0].value =
            json!({"methods":["GET"],"path":hostile,"pathIsFragment":true}).to_string();
        let d = documents(&e, Kind::Api);
        assert!(d
            .mermaid
            .contains("#34;#91;#93;#96;#60;script#62;<br/>#27;"));
        assert_eq!(
            d.legend["entries"]
                .as_array()
                .unwrap()
                .iter()
                .find(|v| v["routePath"] == hostile)
                .unwrap()["routePath"],
            hostile
        );
        assert_eq!(d.html.matches("<script>").count(), 1);
        for link in d.html.split("href=\"").skip(1) {
            assert_eq!(link.split('"').next().unwrap(), "diagram.svg");
        }
        assert!(d.html.contains("img-src 'self'; script-src 'sha256-"));
        assert!(d.html.contains("data-viewer-viewport"));
        assert_eq!(d.html.matches("src=\"diagram.svg\"").count(), 1);
        assert_eq!(escape_label("a\r\nb\rc\t"), "a<br/>b<br/>c#9;");
        let mut e = class_source();
        e.facts.push(fact(
            "unicode",
            "function",
            "pkg.app.Child.\u{63d0}\u{4ea4}",
            50,
            51,
            "",
        ));
        let d = documents(&e, Kind::Class);
        assert!(!d.mermaid.contains("\u{63d0}\u{4ea4}"));
        assert!(d.legend.to_string().contains("\u{63d0}\u{4ea4}"));
    }

    #[test]
    fn invalid_selection_snapshot_fact_and_workflow_fail_closed() {
        let e = class_source();
        let c = catalog::build(&e, "root").unwrap();
        let mut p = catalog::page(
            &c,
            &Filter {
                kind: Some(Kind::Class),
                ..Filter::default()
            },
        )
        .unwrap();
        assert!(build(&c, &e, &p, &Kind::Workflow)
            .err()
            .unwrap()
            .contains("focused analysis"));
        assert!(build(&c, &e, &p, &Kind::Api).is_err());
        p.items[0].fact_id = "unknown".into();
        assert!(build(&c, &e, &p, &Kind::Class).is_err());
        p.items = vec![
            c.items
                .iter()
                .find(|i| i.kind == Kind::Class)
                .unwrap()
                .clone();
            31
        ];
        assert!(build(&c, &e, &p, &Kind::Class).is_err());
        p.items.clear();
        assert!(build(&c, &e, &p, &Kind::Class).is_err());
        let mut p = catalog::page(
            &c,
            &Filter {
                kind: Some(Kind::Class),
                ..Filter::default()
            },
        )
        .unwrap();
        p.snapshot_id = "different".into();
        assert!(build(&c, &e, &p, &Kind::Class).is_err());
    }

    #[test]
    fn deterministic_under_fact_reordering_and_no_approval_state() {
        let mut e = class_source();
        let first = documents(&e, Kind::Class);
        e.facts.reverse();
        let second = documents(&e, Kind::Class);
        assert_eq!(first.mermaid, second.mermaid);
        assert_eq!(first.legend, second.legend);
        assert_eq!(first.html, second.html);
        for forbidden in ["requireAcceptance", "modelProposalSha", "approvalState"] {
            assert!(!first.legend.to_string().contains(forbidden));
        }
    }

    #[test]
    fn document_cap_rejects_oversized_local_metadata() {
        let e = api_source();
        let mut c = catalog::build(&e, "root").unwrap();
        c.source_root = "x".repeat(MAX_DOCUMENT);
        let p = catalog::page(
            &c,
            &Filter {
                kind: Some(Kind::Api),
                ..Filter::default()
            },
        )
        .unwrap();
        assert!(build(&c, &e, &p, &Kind::Api)
            .err()
            .unwrap()
            .contains("8 MiB"));
    }

    #[test]
    #[ignore = "requires locally installed mmdc and its browser; synthetic sources only"]
    fn real_mermaid_syntax_smoke() {
        let temp = tempfile::tempdir().unwrap();
        let mut classes = class_source();
        classes.facts.push(fact(
            "unicode_class",
            "class",
            "pkg.app.\u{65e5}\u{672c}\u{8a9e}",
            90,
            95,
            "",
        ));
        let mut routes = api_source();
        routes.facts[0].value = json!({"methods":["GET"],
            "path":"/private/\"[]`<script>\n\u{1b}x", "pathIsFragment":true})
        .to_string();
        for (label, d) in [
            ("class", documents(&classes, Kind::Class)),
            ("api", documents(&routes, Kind::Api)),
        ] {
            let input = temp.path().join(format!("{label}.mmd"));
            let output = temp.path().join(format!("{label}.svg"));
            std::fs::write(&input, d.mermaid).unwrap();
            let result = std::process::Command::new("mmdc")
                .arg("-i")
                .arg(input)
                .arg("-o")
                .arg(&output)
                .output()
                .expect("mmdc must be installed");
            assert!(
                result.status.success(),
                "{label}: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert!(std::fs::metadata(output).unwrap().len() > 0);
        }
    }
}
