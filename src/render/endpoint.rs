//! Deterministic endpoint/callable source views. Edges are candidates, never a trace.
use crate::adapters::vendor_sdk::{network_operation, supported_root};
use crate::analysis::{Evidence, Fact};
use crate::catalog::{Catalog, Item, Kind};
use crate::focus::{self, FocusOptions};
use crate::packet::{lexical_contexts, LexicalContext};
use crate::render::source_view::Documents;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const MAX_CALL_NODES: usize = 40;
const MAX_CALLABLE_NODES: usize = 20;
const MAX_NETWORK_NODES: usize = 20;
const PROVENANCE_RESERVE_BYTES: usize = 16 * 1024;
const ENVELOPE_RESERVE_BYTES: usize = 2 * 1024;

// Presentation only: these unresolved expressions remain UNKNOWN in the legend.
// This filter never establishes a collection type, logging target or network edge.
fn routine_expression(head: &str) -> bool {
    let parts: Vec<_> = head.split('.').collect();
    if !dotted(head) || parts.len() < 2 {
        return true;
    }
    let method = parts.last().copied().unwrap_or("");
    parts
        .iter()
        .any(|p| matches!(*p, "logger" | "logging" | "log"))
        || matches!(
            method,
            "debug" | "info" | "warning" | "error" | "exception" | "critical"
        )
        || (parts.len() <= 2 || parts.len() == 3 && parts[0] == "self")
            && matches!(
                method,
                "append"
                    | "extend"
                    | "insert"
                    | "pop"
                    | "remove"
                    | "clear"
                    | "get"
                    | "keys"
                    | "values"
                    | "items"
                    | "setdefault"
                    | "update"
                    | "copy"
                    | "add"
                    | "discard"
                    | "join"
                    | "split"
                    | "strip"
            )
}

#[derive(clap::ValueEnum, Serialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum View {
    Network,
    Services,
}

fn callable(f: &Fact) -> bool {
    matches!(f.kind.as_str(), "function" | "async_function")
}
fn citation(f: &Fact) -> Value {
    json!({"factId":f.id,"kind":f.kind,"symbol":f.symbol,"path":f.path,"line":f.line,"endLine":f.end_line})
}
fn dotted(s: &str) -> bool {
    !s.is_empty()
        && s.split('.').all(|p| {
            !p.is_empty()
                && p.chars()
                    .enumerate()
                    .all(|(i, c)| c == '_' || c.is_alphabetic() || (i > 0 && c.is_numeric()))
        })
}
fn label(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars().take(160) {
        match c {
            '\n' | '\r' => out.push_str("<br/>"),
            '#' | '&' | '"' | '<' | '>' | '|' | '[' | ']' | '{' | '}' | '(' | ')' | '`' | '\\' => {
                out.push_str(&format!("#{};", c as u32))
            }
            c if c.is_control() || matches!(c,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}') => {
                out.push(' ')
            }
            _ => out.push(c),
        }
    }
    out
}
fn html_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn short_symbol(s: &str) -> String {
    s.rsplit('.')
        .take(2)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(".")
}

fn parent(i: usize, facts: &[&Fact]) -> Option<usize> {
    let child = facts[i];
    facts
        .iter()
        .enumerate()
        .filter(|(j, f)| {
            *j != i
                && f.path == child.path
                && (callable(f) || f.kind == "class" || f.kind == "module")
                && child
                    .symbol
                    .strip_prefix(&f.symbol)
                    .is_some_and(|tail| tail.starts_with('.'))
                && (f.kind == "module" || (f.line <= child.line && child.end_line <= f.end_line))
        })
        .max_by_key(|(_, f)| (f.symbol.len(), f.line))
        .map(|(j, _)| j)
}

// Resolve provenance only, not callable targets (those come exclusively from focus).
// Import visibility is lexical and competing bindings fail closed.
fn imported_name(
    reference: usize,
    name: &str,
    facts: &[&Fact],
    ctx: &BTreeMap<usize, LexicalContext>,
    allow_local_relative: bool,
) -> Option<(String, Vec<usize>)> {
    if !dotted(name) {
        return None;
    }
    let f = facts[reference];
    let root = name.split('.').next()?;
    let tail = &name[root.len()..];
    let module = facts
        .iter()
        .position(|m| m.path == f.path && m.kind == "module")?;
    let mut scope = if matches!(
        f.kind.as_str(),
        "parameter_annotation" | "return_annotation"
    ) {
        ctx[&reference].function.and_then(|i| parent(i, facts))
    } else {
        ctx[&reference].owner
    };
    let mut visited = BTreeSet::new();
    while let Some(owner) = scope {
        if !visited.insert(owner) {
            return None;
        }
        scope = parent(owner, facts);
        if facts[owner].kind == "class"
            && !matches!(
                f.kind.as_str(),
                "parameter_annotation" | "return_annotation" | "attribute_annotation"
            )
        {
            continue;
        }
        let in_scope = |i: usize| {
            facts[i].path == f.path
                && if callable(facts[i]) || facts[i].kind == "class" {
                    parent(i, facts) == Some(owner)
                } else {
                    ctx[&i].owner == Some(owner)
                }
        };
        let mut found = Vec::new();
        for (i, import) in facts.iter().enumerate().filter(|(i, _)| in_scope(*i)) {
            if matches!(import.kind.as_str(), "binding" | "binding_invalidation")
                && (owner == module || import.line <= f.line)
            {
                let v: Value = serde_json::from_str(&import.value).ok()?;
                if v["target"].as_str() == Some(root) {
                    return None;
                }
            }
            if import.kind == "parameter" && import.symbol.rsplit('.').next() == Some(root) {
                return None;
            }
            if (callable(import) || import.kind == "class")
                && import.symbol == format!("{}.{root}", facts[owner].symbol)
            {
                found.push((format!("{}{tail}", import.symbol), vec![i]));
            }
            if !matches!(import.kind.as_str(), "import" | "from_import") {
                continue;
            }
            let v: Value = serde_json::from_str(&import.value).ok()?;
            for a in v["names"].as_array()? {
                let original = a["name"].as_str()?;
                if original == "*" {
                    return None;
                }
                let local = a["asname"].as_str().unwrap_or_else(|| {
                    if import.kind == "import" {
                        original.split('.').next().unwrap_or("")
                    } else {
                        original
                    }
                });
                if local != root {
                    continue;
                }
                if owner != module && import.line > f.line {
                    return None;
                }
                let mut import_proof = vec![i];
                let target = if import.kind == "import" {
                    if a["asname"].is_string() {
                        original
                    } else {
                        local
                    }
                    .to_string()
                } else {
                    let level = v["level"].as_u64().unwrap_or(0);
                    if level > 0 {
                        // Relative names can establish a local union alternative,
                        // never external library provenance or wrapper dispatch.
                        if !allow_local_relative {
                            return None;
                        }
                        let mut package: Vec<_> = facts[module].symbol.split('.').collect();
                        if !f.path.ends_with("/__init__.py") && f.path != "__init__.py" {
                            package.pop()?;
                        }
                        if level > package.len() as u64 {
                            return None;
                        }
                        for _ in 1..level {
                            package.pop()?;
                        }
                        let imported_module = v["module"].as_str()?;
                        if !imported_module.is_empty() {
                            if !dotted(imported_module) {
                                return None;
                            }
                            package.push(imported_module);
                        }
                        let target = format!("{}.{original}", package.join("."));
                        let classes: Vec<_> = facts
                            .iter()
                            .enumerate()
                            .filter(|(_, c)| {
                                c.kind == "class" && c.symbol == format!("{target}{tail}")
                            })
                            .map(|(j, _)| j)
                            .collect();
                        if classes.len() != 1 {
                            return None;
                        }
                        let class = classes[0];
                        let origins: Vec<_> = facts
                            .iter()
                            .enumerate()
                            .filter(|(_, m)| {
                                m.kind == "module"
                                    && m.path == facts[class].path
                                    && m.symbol == package.join(".")
                            })
                            .map(|(j, _)| j)
                            .collect();
                        if origins.len() != 1 {
                            return None;
                        }
                        import_proof.extend([module, origins[0], class]);
                        target
                    } else {
                        format!("{}.{}", v["module"].as_str()?, original)
                    }
                };
                found.push((format!("{target}{tail}"), import_proof));
            }
        }
        if !found.is_empty() {
            return (found.len() == 1).then(|| found.remove(0));
        }
    }
    None
}

fn parameter_shadowed(
    reference: usize,
    name: &str,
    facts: &[&Fact],
    ctx: &BTreeMap<usize, LexicalContext>,
) -> bool {
    let Some(owner) = ctx[&reference].owner else {
        return true;
    };
    facts.iter().enumerate().any(|(i, f)| {
        if f.path != facts[reference].path || f.line > facts[reference].line {
            return false;
        }
        if callable(f) || f.kind == "class" {
            return parent(i, facts) == Some(owner)
                && f.symbol == format!("{}.{name}", facts[owner].symbol);
        }
        if ctx[&i].owner != Some(owner) {
            return false;
        }
        if matches!(f.kind.as_str(), "binding" | "binding_invalidation") {
            return serde_json::from_str::<Value>(&f.value)
                .ok()
                .is_none_or(|v| {
                    v["target"]
                        .as_str()
                        .is_none_or(|t| t == name || t.starts_with(&format!("{name}.")))
                });
        }
        if matches!(f.kind.as_str(), "import" | "from_import") {
            return serde_json::from_str::<Value>(&f.value)
                .ok()
                .is_none_or(|v| {
                    v["names"].as_array().is_none_or(|a| {
                        a.iter().any(|alias| {
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
                            original == "*" || local == name
                        })
                    })
                });
        }
        false
    })
}

struct ReceiverType {
    name: String,
    reference: usize,
    proof: Vec<usize>,
    union: Option<Vec<String>>,
    stopping_reason: Option<String>,
}

impl ReceiverType {
    fn stopped(reference: usize, mut proof: Vec<usize>, reason: &str) -> Self {
        proof.sort_unstable();
        proof.dedup();
        Self {
            name: String::new(),
            reference,
            proof,
            union: None,
            stopping_reason: Some(reason.into()),
        }
    }

    fn plain(name: String, reference: usize, proof: Vec<usize>) -> Self {
        Self {
            name,
            reference,
            proof,
            union: None,
            stopping_reason: None,
        }
    }
}

#[derive(Default)]
struct NetworkResolution {
    candidates: Vec<(String, String, &'static str)>,
    proof: Vec<usize>,
    branches: Vec<Value>,
    stopping_reason: Option<String>,
}

// Only establish call shape, never interpret arguments or their return values.
fn direct_initializer(source: &str) -> Option<&str> {
    // The extractor's byte cap can hide a transformation even after a balanced call.
    if source.len() >= 512 {
        return None;
    }
    let (callee, _) = source.split_once('(')?;
    if !dotted(callee) {
        return None;
    }
    let mut stack = Vec::new();
    let mut quote = None;
    let mut escaped = false;
    for (offset, ch) in source[callee.len()..].char_indices() {
        if let Some(delimiter) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == delimiter {
                quote = None;
            }
            continue;
        }
        let tail = &source[callee.len() + offset..];
        if tail.starts_with("...") || tail.starts_with("'''") || tail.starts_with("\"\"\"") {
            return None;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            '(' | '[' | '{' => stack.push(ch),
            ')' | ']' | '}' => {
                let expected = match ch {
                    ')' => '(',
                    ']' => '[',
                    _ => '{',
                };
                if stack.pop() != Some(expected) {
                    return None;
                }
                if stack.is_empty() {
                    return (callee.len() + offset + ch.len_utf8() == source.len())
                        .then_some(callee);
                }
            }
            _ => {}
        }
    }
    None
}

fn binding_type(
    b: usize,
    facts: &[&Fact],
    ctx: &BTreeMap<usize, LexicalContext>,
) -> Option<ReceiverType> {
    let v: Value = serde_json::from_str(&facts[b].value).ok()?;
    let Some(source) = v["source"].as_str() else {
        return Some(ReceiverType::stopped(b, vec![b],
            "initializer unsupported or invalidated: complete direct callee call evidence unavailable"));
    };
    if source.contains('(') {
        let Some(constructor) = direct_initializer(source) else {
            return Some(ReceiverType::stopped(b, vec![b],
                "initializer is not one complete direct callee call: transformed, unsupported or truncated call shape"));
        };
        if let Some((receiver, method)) = constructor
            .split_once('.')
            .filter(|(_, m)| !m.contains('.'))
        {
            let symbol = format!("{}.{}", facts[b].symbol, receiver);
            if facts.iter().enumerate().any(|(i, f)| {
                f.path == facts[b].path
                    && f.symbol == symbol
                    && matches!(f.kind.as_str(), "parameter" | "parameter_annotation")
                    && ctx[&i].function == ctx[&b].function
            }) {
                return factory_return(b, receiver, method, facts, ctx);
            }
        }
        return dotted(constructor).then(|| ReceiverType::plain(constructor.into(), b, vec![b]));
    }
    if parameter_shadowed(b, source, facts, ctx) {
        return None;
    }
    let annotations: Vec<_> = facts
        .iter()
        .enumerate()
        .filter(|(i, a)| {
            a.kind == "parameter_annotation"
                && a.path == facts[b].path
                && a.symbol == format!("{}.{}", facts[b].symbol, source)
                && ctx[i].function == ctx[&b].function
        })
        .map(|(i, _)| i)
        .collect();
    if annotations.len() != 1 {
        return None;
    }
    let a = annotations[0];
    let value = facts[a]
        .value
        .strip_prefix("Optional[")
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or(&facts[a].value);
    dotted(value).then(|| ReceiverType::plain(value.into(), a, vec![b, a]))
}

// One direct member call on an annotated parameter, one directly declared method,
// Plain SDK returns or bounded direct named unions. No execution, MRO or wrappers.
fn factory_return(
    b: usize,
    receiver: &str,
    method: &str,
    facts: &[&Fact],
    ctx: &BTreeMap<usize, LexicalContext>,
) -> Option<ReceiverType> {
    if !dotted(receiver) || receiver.contains('.') || !dotted(method) {
        return None;
    }
    let owner = ctx[&b].function?;
    let mut proof = vec![b, owner];
    let parameters: Vec<_> = facts
        .iter()
        .enumerate()
        .filter(|(i, a)| {
            a.path == facts[b].path
                && a.kind == "parameter_annotation"
                && a.symbol == format!("{}.{}", facts[owner].symbol, receiver)
                && ctx[i].function == Some(owner)
        })
        .map(|(i, _)| i)
        .collect();
    proof.extend(parameters.iter().copied());
    if parameters.len() != 1 {
        return Some(ReceiverType::stopped(
            b,
            proof,
            "factory parameter annotation missing or ambiguous",
        ));
    }
    let a = parameters[0];
    if !dotted(&facts[a].value) {
        return Some(ReceiverType::stopped(
            a,
            proof,
            "unsupported factory parameter annotation",
        ));
    }
    if parameter_shadowed(b, receiver, facts, ctx) {
        return Some(ReceiverType::stopped(
            a,
            proof,
            "factory parameter shadowed or rebound",
        ));
    }
    let Some((qualified, imports)) = imported_name(a, &facts[a].value, facts, ctx, false) else {
        return Some(ReceiverType::stopped(
            a,
            proof,
            "factory class import/declaration unresolved or shadowed",
        ));
    };
    proof.extend(imports);
    let classes: Vec<_> = facts
        .iter()
        .enumerate()
        .filter(|(_, f)| f.kind == "class" && f.symbol == qualified)
        .map(|(i, _)| i)
        .collect();
    proof.extend(classes.iter().copied());
    if classes.len() != 1 {
        return Some(ReceiverType::stopped(
            a,
            proof,
            "local factory class missing or ambiguous",
        ));
    }
    let class = classes[0];
    let symbol = format!("{qualified}.{method}");
    let methods: Vec<_> = facts
        .iter()
        .enumerate()
        .filter(|(i, f)| {
            f.symbol == symbol
                && callable(f)
                && ctx[i].class == Some(class)
                && parent(*i, facts) == Some(class)
        })
        .map(|(i, _)| i)
        .collect();
    proof.extend(methods.iter().copied());
    if methods.len() != 1 {
        return Some(ReceiverType::stopped(
            a,
            proof,
            "direct factory method missing or ambiguous",
        ));
    }
    let target = methods[0];
    if facts[target].kind != "function" {
        return Some(ReceiverType::stopped(
            target,
            proof,
            "async factory method not interpreted",
        ));
    }
    let blockers: Vec<_> = facts
        .iter()
        .enumerate()
        .filter(|(i, f)| {
            f.path == facts[class].path
                && (f.kind == "decorator" && (f.symbol == symbol || f.symbol == qualified)
                    || ctx[i].class == Some(class)
                        && (f.symbol == format!("{qualified}.__getattr__")
                            || f.symbol == format!("{qualified}.__getattribute__")
                            || matches!(f.kind.as_str(), "binding" | "binding_invalidation")
                                && serde_json::from_str::<Value>(&f.value)
                                    .ok()
                                    .is_some_and(|v| {
                                        v["target"].as_str().is_some_and(|t| {
                                            t == method || t == format!("self.{method}")
                                        })
                                    })))
        })
        .map(|(i, _)| i)
        .collect();
    if !blockers.is_empty() {
        proof.extend(blockers);
        return Some(ReceiverType::stopped(
            target,
            proof,
            "factory decorators, dynamic lookup or method mutation not resolved",
        ));
    }
    let returns: Vec<_> = facts
        .iter()
        .enumerate()
        .filter(|(i, f)| {
            f.kind == "return_annotation"
                && f.path == facts[target].path
                && f.symbol == symbol
                && ctx[i].function == Some(target)
        })
        .map(|(i, _)| i)
        .collect();
    proof.extend(returns.iter().copied());
    if returns.len() != 1 {
        return Some(ReceiverType::stopped(
            target,
            proof,
            "factory return annotation missing or ambiguous",
        ));
    }
    let r = returns[0];
    proof.extend([b, owner, a, class, target, r]);
    proof.extend(
        facts
            .iter()
            .enumerate()
            .filter(|(i, f)| {
                f.kind == "parameter"
                    && f.path == facts[b].path
                    && f.symbol == facts[a].symbol
                    && ctx[i].function == Some(owner)
            })
            .map(|(i, _)| i),
    );
    proof.sort_unstable();
    proof.dedup();
    let annotation = &facts[r].value;
    if annotation.contains(" BitOr ") {
        let mut members: Vec<_> = annotation.split(" BitOr ").map(str::to_owned).collect();
        let unsupported = members.len() > 4
            || members.iter().any(|m| {
                !dotted(m)
                    || m.contains('.')
                    || matches!(
                        m.as_str(),
                        "Any" | "None" | "Optional" | "Union" | "Literal"
                    )
            });
        members.sort();
        members.dedup();
        return Some(ReceiverType {
            name: annotation.clone(), reference: r, proof,
            union: (!unsupported).then_some(members),
            stopping_reason: unsupported.then(|| "unsupported factory return union; direct plain named members only, maximum 4 branches".into()),
        });
    }
    let Some((sdk, imports)) = imported_name(r, annotation, facts, ctx, false) else {
        return Some(ReceiverType {
            name: annotation.clone(),
            reference: r,
            proof,
            union: None,
            stopping_reason: Some("unsupported or unresolved factory return annotation".into()),
        });
    };
    if !supported_root(&sdk) {
        return Some(ReceiverType {
            name: annotation.clone(),
            reference: r,
            proof,
            union: None,
            stopping_reason: Some("factory return is not a supported SDK type".into()),
        });
    }
    proof.extend(imports);
    Some(ReceiverType::plain(annotation.clone(), r, proof))
}

fn field_type(
    class: usize,
    field: &str,
    facts: &[&Fact],
    ctx: &BTreeMap<usize, LexicalContext>,
) -> Option<ReceiverType> {
    let target = format!("self.{field}");
    let writes: Vec<_> = facts
        .iter()
        .enumerate()
        .filter(|(i, b)| {
            b.path == facts[class].path
                && ctx[i].class == Some(class)
                && matches!(b.kind.as_str(), "binding" | "binding_invalidation")
                && serde_json::from_str::<Value>(&b.value)
                    .ok()
                    .is_some_and(|v| {
                        v["target"].as_str().is_some_and(|t| {
                            t == target || t == field || target.starts_with(&format!("{t}."))
                        })
                    })
        })
        .map(|(i, _)| i)
        .collect();
    // Only an explicit class-body annotation fact can prove an annotated field.
    let annotations: Vec<_> = facts
        .iter()
        .enumerate()
        .filter(|(i, a)| {
            a.kind == "attribute_annotation"
                && a.path == facts[class].path
                && ctx[i].class == Some(class)
                && ctx[i].owner == Some(class)
                && a.symbol == format!("{}.{field}", facts[class].symbol)
        })
        .map(|(i, _)| i)
        .collect();
    if writes.is_empty() && annotations.len() == 1 {
        let a = annotations[0];
        return dotted(&facts[a].value)
            .then(|| ReceiverType::plain(facts[a].value.clone(), a, vec![class, a]));
    }
    if writes.len() != 1 || !annotations.is_empty() {
        return None;
    }
    let b = writes[0];
    if facts[b].symbol != format!("{}.__init__", facts[class].symbol) {
        return None;
    }
    if facts
        .iter()
        .enumerate()
        .filter(|(i, f)| callable(f) && f.symbol == facts[b].symbol && ctx[i].class == Some(class))
        .count()
        != 1
    {
        return None;
    }
    let mut result = binding_type(b, facts, ctx)?;
    result.proof.push(class);
    Some(result)
}

fn network_proof(
    call: usize,
    facts: &[&Fact],
    ctx: &BTreeMap<usize, LexicalContext>,
) -> Option<NetworkResolution> {
    let c = &ctx[&call];
    if c.binding_blocked {
        return None;
    }
    let head = facts[call].value.split('(').next()?.trim();
    if !dotted(head) {
        return None;
    }
    let mut proof = vec![call];
    let mut union = None;
    let mut stopping_reason = None;
    let (mut name, mut reference, mut suffix) = if let Some(b) = c.binding {
        let v: Value = serde_json::from_str(&facts[b].value).ok()?;
        let target = v["target"].as_str()?;
        let suffix = head.strip_prefix(&format!("{target}."))?.to_string();
        let t = binding_type(b, facts, ctx)?;
        proof.extend(t.proof);
        union = t.union;
        stopping_reason = t.stopping_reason;
        (t.name, t.reference, suffix)
    } else if let Some(rest) = head.strip_prefix("self.").filter(|_| c.self_receiver) {
        let (field, tail) = rest.split_once('.')?;
        let t = field_type(c.class?, field, facts, ctx)?;
        proof.extend(t.proof);
        union = t.union;
        stopping_reason = t.stopping_reason;
        (t.name, t.reference, tail.into())
    } else {
        let root = head.split('.').next()?;
        let annotations: Vec<_> = facts
            .iter()
            .enumerate()
            .filter(|(i, a)| {
                a.kind == "parameter_annotation"
                    && a.path == facts[call].path
                    && a.symbol == format!("{}.{}", facts[call].symbol, root)
                    && ctx[i].function == c.function
            })
            .map(|(i, _)| i)
            .collect();
        if annotations.len() == 1 {
            if parameter_shadowed(call, root, facts, ctx) {
                return None;
            }
            let a = annotations[0];
            proof.push(a);
            (
                facts[a].value.clone(),
                a,
                head.strip_prefix(&format!("{root}."))?.into(),
            )
        } else {
            (head.to_string(), call, String::new())
        }
    };
    // At most three typed field transitions, with every declaration/import cited.
    for _ in 0..=3 {
        if stopping_reason.is_some() {
            proof.sort_unstable();
            proof.dedup();
            return Some(NetworkResolution {
                proof,
                stopping_reason,
                ..Default::default()
            });
        }
        if let Some(members) = union.take() {
            let mut result = NetworkResolution::default();
            for member in members {
                let Some((qualified, imports)) =
                    imported_name(reference, &member, facts, ctx, true)
                else {
                    result.stopping_reason = Some(
                        "incomplete union branch provenance: unresolved or shadowed member".into(),
                    );
                    result.branches.push(json!({"member":member,"classification":"UNKNOWN","stoppingReason":"member import/declaration unresolved or ambiguous"}));
                    continue;
                };
                proof.extend(imports);
                let local_classes: Vec<_> = facts
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| f.kind == "class" && f.symbol == qualified)
                    .map(|(i, _)| i)
                    .collect();
                proof.extend(local_classes.iter().copied());
                if local_classes.len() > 1
                    || matches!(
                        qualified.as_str(),
                        "typing.Any"
                            | "typing.Optional"
                            | "typing.Union"
                            | "typing.Literal"
                            | "builtins.object"
                    )
                {
                    result.stopping_reason = Some(
                        "unsupported or ambiguous union member; whole annotation rejected".into(),
                    );
                }
                let impersonated = facts.iter().any(|f| {
                    f.kind == "module"
                        && (f.symbol == qualified.split('.').next().unwrap_or("")
                            || qualified.starts_with(&format!("{}.", f.symbol)))
                });
                let candidate = (!impersonated)
                    .then(|| network_operation(&qualified, &suffix))
                    .flatten();
                let reason = if candidate.is_some() {
                    "runtime selection UNKNOWN"
                } else {
                    "union alternative/dynamic dispatch not resolved"
                };
                result.branches.push(json!({"member":member,"qualifiedName":qualified,"classification":if candidate.is_some() {"declared SDK return branch candidate; runtime selection UNKNOWN"} else {"UNKNOWN"},"client":candidate.as_ref().map(|c| &c.0),"operation":candidate.as_ref().map(|c| &c.1),"stoppingReason":reason,"runtimeSelection":"UNKNOWN","url":"UNKNOWN","httpVerb":"UNKNOWN","requestOccurrence":"UNKNOWN","runtimeSecurity":"UNKNOWN"}));
                if let Some((client, operation, _)) = candidate {
                    result.candidates.push((
                        client,
                        operation,
                        "declared SDK return branch candidate; runtime selection UNKNOWN",
                    ));
                }
            }
            proof.sort_unstable();
            proof.dedup();
            result.proof = proof;
            if result.stopping_reason.is_some() {
                result.candidates.clear();
            }
            return Some(result);
        }
        let (qualified, p) = imported_name(reference, &name, facts, ctx, false)?;
        proof.extend(p);
        if supported_root(&qualified) {
            let (client, operation, kind) = network_operation(&qualified, &suffix)?;
            // A local package using a library's name must not impersonate it.
            if facts.iter().any(|f| {
                f.kind == "module"
                    && (f.symbol == qualified.split('.').next().unwrap_or("")
                        || qualified.starts_with(&format!("{}.", f.symbol)))
            }) {
                return None;
            }
            proof.sort_unstable();
            proof.dedup();
            return Some(NetworkResolution {
                candidates: vec![(client, operation, kind)],
                proof,
                ..Default::default()
            });
        }
        let classes: Vec<_> = facts
            .iter()
            .enumerate()
            .filter(|(_, f)| f.kind == "class" && f.symbol == qualified)
            .map(|(i, _)| i)
            .collect();
        if classes.len() != 1 {
            return None;
        }
        let class = classes[0];
        proof.push(class);
        let (field, rest) = suffix.split_once('.')?;
        let receiver = head.strip_suffix(&suffix)?.trim_end_matches('.');
        let actual_field = format!("{receiver}.{field}");
        // An observed assignment to this instance field/ancestor blocks the type
        // candidate. Do not borrow a class annotation across an explicit rebind.
        if facts.iter().enumerate().any(|(j, b)| {
            b.path == facts[call].path
                && ctx[&j].owner == c.owner
                && b.line <= facts[call].line
                && matches!(b.kind.as_str(), "binding" | "binding_invalidation")
                && serde_json::from_str::<Value>(&b.value)
                    .ok()
                    .is_some_and(|v| {
                        v["target"].as_str().is_some_and(|t| {
                            t == actual_field
                                || actual_field.starts_with(&format!("{t}.")) && !proof.contains(&j)
                        })
                    })
        }) {
            return None;
        }
        let t = field_type(class, field, facts, ctx)?;
        proof.extend(t.proof);
        name = t.name;
        reference = t.reference;
        union = t.union;
        stopping_reason = t.stopping_reason;
        suffix = rest.into();
    }
    None
}

fn schema_description(v: &Value, depth: usize) -> String {
    if let Some(r) = v["$ref"].as_str() {
        return format!(
            "{r} ({})",
            v["status"].as_str().unwrap_or("reference; not expanded")
        );
    }
    if let Some(b) = v["boundary"].as_str() {
        return b.into();
    }
    if v.is_boolean() {
        return format!("schema {v}");
    }
    let shape = v["type"]
        .as_str()
        .map(str::to_owned)
        .or_else(|| {
            v["type"].as_array().map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(" | ")
            })
        })
        .unwrap_or_else(|| {
            if v.get("properties").is_some() {
                "object".into()
            } else {
                "unspecified schema".into()
            }
        });
    if depth >= 2 {
        return shape;
    }
    if let Some(p) = v["properties"].as_object() {
        let fields = p
            .iter()
            .take(8)
            .map(|(name, s)| format!("{name}: {}", schema_description(s, depth + 1)))
            .collect::<Vec<_>>()
            .join(", ");
        return format!(
            "{shape} {{ {fields}{} }}",
            if p.len() > 8 { ", ..." } else { "" }
        );
    }
    if let Some(items) = v.get("items") {
        return format!("{shape} of {}", schema_description(items, depth + 1));
    }
    for k in ["oneOf", "anyOf", "allOf"] {
        if let Some(a) = v[k].as_array() {
            return format!(
                "{k}: {}",
                a.iter()
                    .take(4)
                    .map(|s| schema_description(s, depth + 1))
                    .collect::<Vec<_>>()
                    .join("; ")
            );
        }
    }
    shape
}

fn body_html(v: &Value) -> String {
    if let Some(b) = v["boundary"].as_str() {
        return format!("<p>{}</p>", html_text(b));
    }
    let mut out = String::new();
    if let Some(m) = v["content"].as_object().filter(|m| !m.is_empty()) {
        out.push_str("<ul>");
        for (media, schema) in m {
            out.push_str(&format!(
                "<li><code>{}</code>: {}</li>",
                html_text(media),
                html_text(&schema_description(schema, 0))
            ));
        }
        out.push_str("</ul>");
    } else if let Some(s) = v
        .get("schema")
        .filter(|s| s.as_object().is_some_and(|m| !m.is_empty()) || s.is_boolean())
    {
        out.push_str(&format!("<p>{}</p>", html_text(&schema_description(s, 0))));
    } else {
        out.push_str(
            "<p>No body schema retained. This does not prove absence of a runtime payload.</p>",
        );
    }
    out
}

fn contract_html(association: &Value, spec: Option<&crate::openapi::Document>) -> String {
    let mut out = String::from("<section><h2>Local API Contract</h2>");
    out.push_str(&format!("<p>Association: <strong>{}</strong>. {}. Contract declarations are not runtime registration, reachability or security verification.</p>",
        html_text(association["status"].as_str().unwrap_or("unknown")),html_text(association["basis"].as_str().unwrap_or("No contract supplied/read"))));
    let Some(spec) = spec else {
        out.push_str("<p>Source-only view. No runtime-generated spec was read. Request, response, security and servers remain unknown.</p></section>");
        return out;
    };
    out.push_str(&format!(
        "<p>Local provenance: <code>{}</code>, version {}, SHA-256 <code>{}</code>.</p>",
        html_text(&spec.path),
        html_text(&spec.version),
        html_text(&spec.sha256)
    ));
    if association["status"] != "candidate" {
        out.push_str("<p>No unique operation association. Request, response, security and server details are not attributed to the selected handler. Candidate contracts remain in the legend.</p></section>");
        return out;
    }
    let op = &association["candidates"][0];
    out.push_str(&format!(
        "<h3>{} {}</h3><p>{}</p>",
        html_text(op["method"].as_str().unwrap_or("UNKNOWN")),
        html_text(op["path"].as_str().unwrap_or("UNKNOWN")),
        html_text(op["summary"].as_str().unwrap_or("No summary declared"))
    ));
    out.push_str("<h4>Request</h4>");
    if let Some(required) = op["request"]["required"].as_bool() {
        out.push_str(&format!("<p>Body required (declared): {required}.</p>"));
    }
    out.push_str(&body_html(&op["request"]));
    out.push_str("<h4>Parameters</h4><p>Swagger request body/formData parameters, if declared, are included here.</p><ul>");
    if let Some(params) = op["parameters"].as_array() {
        for p in params {
            if let Some(b) = p["boundary"].as_str() {
                out.push_str(&format!("<li>{}</li>", html_text(b)));
                continue;
            }
            out.push_str(&format!(
                "<li><code>{}</code> ({}, required: {}): {}</li>",
                html_text(p["name"].as_str().unwrap_or("UNKNOWN")),
                html_text(p["in"].as_str().unwrap_or("UNKNOWN")),
                p["required"]
                    .as_bool()
                    .map(|b| b.to_string())
                    .unwrap_or_else(|| "not specified".into()),
                html_text(&schema_description(&p["schema"], 0))
            ));
        }
        if params.is_empty() {
            out.push_str("<li>No parameters retained.</li>");
        }
    }
    out.push_str("</ul><h4>Responses</h4>");
    if let Some(responses) = op["responses"].as_object().filter(|m| !m.is_empty()) {
        for (status, r) in responses {
            out.push_str(&format!(
                "<h5>Declared status {}</h5>{}",
                html_text(status),
                body_html(r)
            ));
        }
    } else {
        out.push_str("<p>No response schema retained.</p>");
    }
    out.push_str(&format!(
        "<h4>Security (Declared, Not Verified)</h4><p>Scope: {}</p><ul>",
        html_text(op["securityBasis"].as_str().unwrap_or("unknown"))
    ));
    if let Some(requirements) = op["security"].as_array() {
        if requirements.is_empty() {
            out.push_str("<li>No security requirement declared at this scope. Anonymous runtime access is not verified.</li>");
        }
        for requirement in requirements {
            if let Some(b) = requirement["boundary"].as_str() {
                out.push_str(&format!("<li>{}</li>", html_text(b)));
                continue;
            }
            let names = requirement
                .as_object()
                .map(|m| {
                    m.iter()
                        .map(|(name, scopes)| {
                            let scopes = scopes
                                .as_array()
                                .map(|a| {
                                    a.iter()
                                        .filter_map(Value::as_str)
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                })
                                .unwrap_or_else(|| "unknown scopes".into());
                            format!("{name} [{scopes}]")
                        })
                        .collect::<Vec<_>>()
                        .join(" AND ")
                })
                .unwrap_or_default();
            out.push_str(&format!(
                "<li>Alternative: {}</li>",
                html_text(if names.is_empty() {
                    "anonymous alternative declared"
                } else {
                    &names
                })
            ));
        }
    } else {
        out.push_str("<li>Security absent or unresolved; not verified.</li>");
    }
    out.push_str("</ul>");
    if let Some(schemes) = spec.security_schemes.as_object().filter(|m| !m.is_empty()) {
        out.push_str("<p>Declared schemes:</p><ul>");
        for (name, scheme) in schemes {
            out.push_str(&format!(
                "<li>{}: {} {}{}</li>",
                html_text(name),
                html_text(scheme["type"].as_str().unwrap_or("unresolved")),
                html_text(scheme["scheme"].as_str().unwrap_or("")),
                scheme["flows"]
                    .as_object()
                    .map(|m| format!(
                        " (flows: {})",
                        html_text(&m.keys().cloned().collect::<Vec<_>>().join(", "))
                    ))
                    .unwrap_or_default()
            ));
        }
        out.push_str("</ul>");
    }
    out.push_str(&format!("<h4>Servers (Declared, Not Contacted)</h4><p>Scope: {}. Server variables/defaults are not substituted. These are inert contract labels, not outgoing network destinations.</p><ul>",html_text(op["serverBasis"].as_str().unwrap_or("unknown"))));
    if let Some(servers) = op["servers"].as_array() {
        for s in servers {
            out.push_str(&format!(
                "<li><code>{}</code></li>",
                html_text(s["url"].as_str().unwrap_or("UNKNOWN/unresolved server"))
            ));
        }
        if servers.is_empty() {
            out.push_str("<li>No server URL retained; no default server inferred.</li>");
        }
    } else {
        for key in ["host", "basePath", "schemes"] {
            let v = &op["servers"][key];
            let text = v
                .as_str()
                .map(str::to_owned)
                .or_else(|| {
                    v.as_array().map(|a| {
                        a.iter()
                            .filter_map(Value::as_str)
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                })
                .unwrap_or_else(|| "UNKNOWN".into());
            out.push_str(&format!(
                "<li>{key}: <code>{}</code></li>",
                html_text(&text)
            ));
        }
    }
    out.push_str("</ul><p>Schema summaries are bounded; references are not recursively expanded. Examples and defaults are omitted.</p></section>");
    out
}

// Preserve the complete focus envelope, but restore full facts and file records
// for every retained citation before measuring compact UTF-8 JSON bytes.
fn retained_input(
    focused_input: &Value,
    evidence: &Evidence,
    facts: &[&Fact],
    retained: &BTreeSet<String>,
) -> Value {
    let mut input = focused_input.clone();
    let selected: Vec<_> = facts
        .iter()
        .copied()
        .filter(|f| retained.contains(&f.id))
        .collect();
    let paths: BTreeSet<_> = selected.iter().map(|f| f.path.as_str()).collect();
    let mut files: Vec<_> = evidence
        .files
        .iter()
        .filter(|f| paths.contains(f.path.as_str()))
        .collect();
    files.sort_by_key(|f| (&f.path, &f.sha256));
    input["facts"] = json!(selected);
    input["files"] = json!(files);
    input["selection"]["includedFacts"] = json!(selected.len());
    input["selection"]["omittedFacts"] = json!(facts.len() - selected.len());
    if let Some(kinds) = input["selection"]["byKind"].as_object_mut() {
        for (kind, counts) in kinds {
            let included = selected.iter().filter(|f| &f.kind == kind).count();
            let total = facts.iter().filter(|f| &f.kind == kind).count();
            counts["included"] = json!(included);
            counts["omitted"] = json!(total - included);
        }
    }
    input
}

pub fn build(
    catalog: &Catalog,
    evidence: &Evidence,
    item: &Item,
    view: &View,
    spec: Option<&crate::openapi::Document>,
    hops: usize,
    max_facts: usize,
) -> Result<Documents, String> {
    let grounded = crate::catalog::build(evidence, &catalog.source_root)?;
    let selected = serde_json::to_value(item).map_err(|e| e.to_string())?;
    let is_api = item.kind == Kind::Api;
    let metadata_citations = if is_api {
        "route citations"
    } else {
        "entry metadata citations"
    };
    if !matches!(item.kind, Kind::Api | Kind::Workflow)
        || catalog.schema_version != crate::catalog::VERSION
        || catalog.snapshot_id != evidence.snapshot_id
        || catalog.extractor_version != evidence.extractor_version
        || catalog.complete != evidence.complete
        || !catalog
            .items
            .iter()
            .any(|i| serde_json::to_value(i).ok().as_ref() == Some(&selected))
        || !grounded
            .items
            .iter()
            .any(|i| serde_json::to_value(i).ok().as_ref() == Some(&selected))
    {
        return Err(
            "Source view requires a source-grounded API or callable workflow item and matching catalog/evidence"
                .into(),
        );
    }
    if !evidence
        .facts
        .iter()
        .any(|f| f.id == item.fact_id && callable(f))
    {
        return Err(
            "Source view entry must anchor an exact function/async_function fact ID".into(),
        );
    }
    if !is_api && spec.is_some() {
        return Err("Local API contracts apply only to API entries".into());
    }
    if !(1..=2000).contains(&max_facts) {
        return Err("Endpoint max-facts must be in 1..=2000".into());
    }
    let route_reserve = item
        .evidence_ids
        .iter()
        .filter(|id| **id != item.fact_id)
        .count();
    let mut focus_budget = max_facts
        .checked_sub(route_reserve)
        .filter(|n| *n > 0)
        .ok_or_else(|| format!("Source view fact limit cannot retain selected {metadata_citations}; increase max-facts"))?;
    let mut facts: Vec<_> = evidence.facts.iter().collect();
    facts.sort_by_key(|f| (&f.path, &f.symbol, f.line, f.end_line, &f.kind, &f.id));
    facts.dedup_by(|a, b| a.id == b.id);
    let route_facts: Vec<_> = facts
        .iter()
        .copied()
        .filter(|f| f.id != item.fact_id && item.evidence_ids.contains(&f.id))
        .collect();
    let route_paths: BTreeSet<_> = route_facts.iter().map(|f| f.path.as_str()).collect();
    let mut route_files: Vec<_> = evidence
        .files
        .iter()
        .filter(|f| route_paths.contains(f.path.as_str()))
        .collect();
    route_files.sort_by_key(|f| (&f.path, &f.sha256));
    let route_reserve_bytes = serde_json::to_vec(&json!({"facts":route_facts,"files":route_files}))
        .map_err(|e| e.to_string())?
        .len();
    let base_reserved_bytes =
        PROVENANCE_RESERVE_BYTES + ENVELOPE_RESERVE_BYTES + route_reserve_bytes;
    let base_focus_max_bytes = crate::packet::MAX_INPUT_BYTES.checked_sub(base_reserved_bytes)
        .filter(|n| *n > 0).ok_or_else(|| format!(
            "Source view max-bytes budget boundary: restored {metadata_citations} and provenance/envelope reservation require {base_reserved_bytes} compact JSON bytes; maximum {}", crate::packet::MAX_INPUT_BYTES))?;
    let javascript = evidence.is_javascript();
    let js_services = javascript && matches!(view, View::Services);
    let label = |s: &str| {
        let escaped = label(s);
        if javascript {
            escaped.replace('$', "#36;")
        } else {
            escaped
        }
    };
    let mut router_context = if javascript && is_api {
        crate::router_context::Context {
            status: "unsupported-language".into(),
            unknown_status: "JavaScript router, DI, prefix and converter inference is unsupported"
                .into(),
            ..Default::default()
        }
    } else if is_api {
        crate::router_context::derive(evidence, item)
    } else {
        crate::router_context::Context {
            status: "not-applicable".into(),
            ..Default::default()
        }
    };
    let context_facts: Vec<_> = facts
        .iter()
        .copied()
        .filter(|f| {
            router_context.status == "supported"
                && router_context.evidence_ids.contains(&f.id)
                && !item.evidence_ids.contains(&f.id)
        })
        .collect();
    let context_paths: BTreeSet<_> = context_facts.iter().map(|f| f.path.as_str()).collect();
    let mut context_files: Vec<_> = evidence
        .files
        .iter()
        .filter(|f| {
            context_paths.contains(f.path.as_str()) && !route_paths.contains(f.path.as_str())
        })
        .collect();
    context_files.sort_by_key(|f| (&f.path, &f.sha256));
    let context_proof_facts = context_facts.len();
    let context_proof_bytes = if router_context.status == "supported" {
        serde_json::to_vec(&json!({"facts":context_facts,"files":context_files}))
            .map_err(|e| e.to_string())?
            .len()
    } else {
        0
    };
    let (mut context_reserve_facts, mut context_reserve_bytes) = (0, 0);
    if router_context.status == "supported" {
        if context_proof_facts < focus_budget && context_proof_bytes < base_focus_max_bytes {
            context_reserve_facts = context_proof_facts;
            context_reserve_bytes = context_proof_bytes;
            focus_budget -= context_reserve_facts;
        } else {
            router_context.status = "budget-unknown".into();
            router_context.warnings.push(format!("Router context cannot be reserved before focus: proof requires {context_proof_facts} additional facts and {context_proof_bytes} compact facts/files bytes; no narrowing"));
        }
    }
    let mut reserved_bytes = base_reserved_bytes + context_reserve_bytes;
    let mut focus_max_bytes = crate::packet::MAX_INPUT_BYTES - reserved_bytes;
    let mut prepared = focus::prepare_with_byte_limit(
        evidence,
        &FocusOptions {
            symbol: item.fact_id.clone(),
            hops,
            max_facts: focus_budget,
        },
        focus_max_bytes,
    );
    if prepared.is_err() && context_reserve_bytes > 0 {
        router_context.status = "budget-unknown".into();
        router_context.warnings.push("Router context reservation leaves insufficient focus anchor/envelope budget; no narrowing".into());
        focus_budget += context_reserve_facts;
        context_reserve_facts = 0;
        context_reserve_bytes = 0;
        reserved_bytes = base_reserved_bytes;
        focus_max_bytes = base_focus_max_bytes;
        prepared = focus::prepare_with_byte_limit(
            evidence,
            &FocusOptions {
                symbol: item.fact_id.clone(),
                hops,
                max_facts: focus_budget,
            },
            focus_max_bytes,
        );
    }
    let prepared = prepared?;
    let focused: Vec<Fact> =
        serde_json::from_value(prepared.input["facts"].clone()).map_err(|e| e.to_string())?;
    let ctx = if javascript {
        BTreeMap::new()
    } else {
        lexical_contexts(&facts)
    };
    let js_index = if javascript {
        Some(crate::js_focus::Index::new(facts.clone())?)
    } else {
        None
    };
    let display_symbol = |i: usize| {
        if js_services {
            if let Some(index) = &js_index {
                if let Some((_, meta)) = index.declarations.get(&i) {
                    if let Some(name) = meta["name"].as_str() {
                        if let Some(owner) = index.owners.get(&i) {
                            if facts[*owner].kind == "class" {
                                if let Some(class_name) = index
                                    .declarations
                                    .get(owner)
                                    .and_then(|(_, m)| m["name"].as_str())
                                {
                                    return format!("{class_name}.{name}");
                                }
                            }
                        }
                        return name.to_string();
                    }
                }
            }
        }
        short_symbol(&facts[i].symbol)
    };
    let owners: BTreeMap<usize, usize> = if let Some(index) = &js_index {
        index.owners.clone()
    } else {
        ctx.iter()
            .filter_map(|(i, c)| c.owner.map(|owner| (*i, owner)))
            .collect()
    };
    let by_id: BTreeMap<_, _> = facts
        .iter()
        .enumerate()
        .map(|(i, f)| (f.id.as_str(), i))
        .collect();
    let mut retained: BTreeSet<String> = focused.iter().map(|f| f.id.clone()).collect();
    let input_bytes = |ids: &BTreeSet<String>| -> Result<usize, String> {
        serde_json::to_vec(&retained_input(&prepared.input, evidence, &facts, ids))
            .map(|bytes| bytes.len())
            .map_err(|e| e.to_string())
    };
    let focused_input_bytes = input_bytes(&retained)?;
    // Selected route/entry metadata citations count against the same fact budget.
    if item
        .evidence_ids
        .iter()
        .filter(|id| !retained.contains(*id))
        .count()
        + retained.len()
        > max_facts
    {
        return Err(
            format!("Source view fact limit cannot retain selected {metadata_citations}; increase max-facts"),
        );
    }
    retained.extend(item.evidence_ids.iter().cloned());
    let base_input_bytes = input_bytes(&retained)?;
    if base_input_bytes > crate::packet::MAX_INPUT_BYTES {
        return Err(format!("Source view max-bytes budget boundary: full focused facts and restored {metadata_citations} require {base_input_bytes} compact JSON bytes; maximum {}", crate::packet::MAX_INPUT_BYTES));
    }
    let mut retained_input_bytes = base_input_bytes;
    if router_context.status == "budget-unknown" {
        router_context.converter_projection = Default::default();
        router_context.unknown_status = "Complete router context proof cannot fit alongside focus within endpoint fact/byte budget; no narrowing".into();
        router_context.candidates.clear();
        router_context.evidence_ids.clear();
        router_context.source_refs.clear();
    }
    let mut context_ids = retained.clone();
    context_ids.extend(router_context.evidence_ids.iter().cloned());
    let context_bytes = input_bytes(&context_ids)?;
    if context_ids.len() <= max_facts && context_bytes <= crate::packet::MAX_INPUT_BYTES {
        retained = context_ids;
        retained_input_bytes = context_bytes;
    } else {
        router_context.status = "budget-unknown".into();
        router_context.converter_projection = Default::default();
        router_context.unknown_status =
            "Complete router context proof exceeds endpoint fact/byte budget; no narrowing".into();
        router_context.candidates.clear();
        router_context.evidence_ids.clear();
        router_context.source_refs.clear();
        router_context.warnings.push(format!("Router context proof requires {} facts and {context_bytes} compact JSON input bytes; limits {max_facts} facts and {} bytes", context_ids.len(), crate::packet::MAX_INPUT_BYTES));
    }
    let mut warnings: BTreeSet<String> = prepared
        .coverage
        .warnings
        .iter()
        .chain(&catalog.warnings)
        .cloned()
        .collect();
    warnings.insert("Static source activation candidates only, not runtime activation, execution order, request tracing or reachability. Conditional calls are not linearized. Solid edges denote observed source call sites; dashed edges denote possible targets. Network destinations and runtime security are UNKNOWN unless separately declared, never verified.".into());
    warnings.insert(format!("Source view byte selection policy endpoint-headroom-v1: focus cap {focus_max_bytes} bytes; reserved {reserved_bytes} bytes ({PROVENANCE_RESERVE_BYTES} provenance, {route_reserve_bytes} entry metadata/files, {context_reserve_bytes} router context facts/files and {context_reserve_facts} facts, {ENVELOPE_RESERVE_BYTES} envelope margin). Focus truncation boundaries remain visible; all retained input is checked against {} compact JSON bytes.", crate::packet::MAX_INPUT_BYTES));
    let anchor = *by_id
        .get(item.fact_id.as_str())
        .ok_or("Endpoint handler fact missing")?;
    let handler = facts[anchor];
    let handler_name = handler.symbol.rsplit('.').next().unwrap_or("");
    let unique_name = facts
        .iter()
        .filter(|f| callable(f) && f.symbol.rsplit('.').next() == Some(handler_name))
        .count()
        == 1;
    let association = if is_api {
        spec.map(|s| if javascript { crate::openapi::match_item(s,item,false) } else { crate::openapi::match_item_with_context(s,item,unique_name,&router_context) }).unwrap_or_else(|| json!({"status":"source-only","boundary":"No local contract supplied/read. Runtime-generated OpenAPI was not read; full paths, servers and security remain unknown."}))
    } else {
        json!({"status":"not-applicable"})
    };
    let mut mermaid = String::from("flowchart LR\n");
    let inbound = if !is_api {
        String::new()
    } else if association["status"] == "candidate" {
        let op = &association["candidates"][0];
        format!(
            "{} {} [contract candidate]",
            op["method"].as_str().unwrap_or("UNKNOWN"),
            op["path"].as_str().unwrap_or("UNKNOWN")
        )
    } else {
        format!(
            "{} {} [source fragment]",
            item.method.as_deref().unwrap_or("UNKNOWN"),
            item.route_path.as_deref().unwrap_or("DYNAMIC")
        )
    };
    let root_label = if js_services && is_api {
        format!(
            "Handler for {} {}",
            item.method.as_deref().unwrap_or("UNKNOWN"),
            item.route_path.as_deref().unwrap_or("DYNAMIC")
        )
    } else if js_services {
        display_symbol(anchor)
    } else if is_api {
        short_symbol(&handler.symbol)
    } else {
        format!("{}\n{} candidate", handler.symbol, item.role)
    };
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    if is_api {
        if js_services {
            mermaid.push_str(&format!(
                "  O[\"{}\"]\n  H[\"{}\"]\n  O -.->|\"declared handler candidate\"| H\n",
                label(&inbound),
                label(&root_label)
            ));
        } else {
            mermaid.push_str(&format!("  O[\"{}\"]\n  A[\"Internal application snapshot\"]\n  H[\"{}\"]\n  O -.->|\"declared handler candidate\"| H\n  H --- A\n",label(&inbound),label(&root_label)));
        }
        nodes.push(json!({"id":"O","label":inbound,"kind":"inbound declaration","sourceRefs":item.evidence_ids.iter().map(|id|citation(facts[by_id[id.as_str()]])).collect::<Vec<_>>()}));
        edges.push(json!({"from":"O","to":"H","kind":"declared handler candidate","sourceFactIds":item.evidence_ids}));
    } else if js_services {
        mermaid.push_str(&format!("  H[\"{}\"]\n", label(&root_label)));
    } else {
        mermaid.push_str(&format!(
            "  A[\"Internal application snapshot\"]\n  H[\"{}\"]\n  H --- A\n",
            label(&root_label)
        ));
    }
    if !js_services {
        nodes.push(
            json!({"id":"A","label":"Internal application snapshot","kind":"source boundary"}),
        );
    }
    nodes.extend([
        if is_api {
            json!({"id":"H","label":if js_services {&root_label} else {&handler.symbol},"kind":"handler","sourceRefs":[citation(handler)]})
        } else {
            json!({"id":"H","label":root_label,"kind":"callable entry candidate","role":item.role,"sourceRefs":item.evidence_ids.iter().map(|id|citation(facts[by_id[id.as_str()]])).collect::<Vec<_>>()})
        },
    ]);
    if !js_services {
        edges.push(
            json!({"from":"H","to":"A","kind":"snapshot membership","sourceFactIds":[handler.id]}),
        );
    }
    let links = prepared.input["focus"]["links"]
        .as_array()
        .ok_or("Focus links missing")?;
    let mut callable_nodes = BTreeMap::from([(handler.id.as_str(), "H".to_string())]);
    let mut boundaries = Vec::new();
    let mut calls = Vec::new();
    let mut network_groups: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut rendered_calls = 0;
    let mut rendered_call_groups = 0;
    let mut unclassified_calls = 0;
    for call in focused.iter().filter(|f| f.kind == "call") {
        let i = by_id[call.id.as_str()];
        let link = links
            .iter()
            .find(|l| l["sourceFactId"].as_str() == Some(&call.id));
        let net = if link.is_none() && !javascript {
            network_proof(i, &facts, &ctx)
        } else {
            None
        };
        let mut resolution = net.unwrap_or_else(|| NetworkResolution {
            proof: vec![i],
            stopping_reason: Some(
                if javascript { "unsupported-language: JavaScript network/DI receiver and this provenance are not inferred; module calls require validated selected relative named-function proofs".into() }
                else { "source receiver/factory proof unresolved, ambiguous or rebound".into() },
            ),
            ..Default::default()
        });
        let mut budget_diagnostic = Value::Null;
        if !resolution.proof.is_empty() {
            let mut candidate = retained.clone();
            candidate.extend(resolution.proof.iter().map(|i| facts[*i].id.clone()));
            let candidate_bytes = input_bytes(&candidate)?;
            budget_diagnostic = json!({"candidateInputBytes":candidate_bytes,"maxInputBytes":crate::packet::MAX_INPUT_BYTES,"candidateFactCount":candidate.len(),"maxFacts":max_facts});
            if candidate.len() <= max_facts && candidate_bytes <= crate::packet::MAX_INPUT_BYTES {
                retained = candidate;
                retained_input_bytes = candidate_bytes;
            } else {
                resolution.candidates.clear();
                resolution.branches.clear();
                resolution.proof = vec![i];
                resolution.stopping_reason = Some(format!(
                    "network provenance {} budget boundary; complete SDK and alternative proof cannot be retained",
                    if candidate.len() > max_facts { "max-facts" } else { "max-bytes" }
                ));
                warnings.insert(
                    "Network provenance omitted at fact/byte limit; destination remains UNKNOWN."
                        .into(),
                );
            }
        }
        let refs: Vec<_> = resolution
            .proof
            .iter()
            .map(|i| citation(facts[*i]))
            .collect();
        for branch in &mut resolution.branches {
            branch["sourceRefs"] = json!(refs);
        }
        let trace = json!({"stoppingReason":resolution.stopping_reason,"branchCoverage":if resolution.stopping_reason.is_some() {"incomplete"} else {"complete"},"branches":resolution.branches,"sourceRefs":refs,"budget":budget_diagnostic,"runtimeSelection":"UNKNOWN","url":"UNKNOWN","httpVerb":"UNKNOWN","requestOccurrence":"UNKNOWN","runtimeSecurity":"UNKNOWN"});
        let accepted = resolution.candidates.first().cloned();
        let alternatives = resolution
            .branches
            .iter()
            .filter(|b| b["classification"] == "UNKNOWN")
            .filter_map(|b| b["member"].as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let head = if let Some(index) = &js_index {
            index
                .calls
                .get(&i)
                .filter(|(meta, _)| retained.contains(&facts[*meta].id))
                .and_then(|(_, meta)| meta["callee"].as_str())
                .unwrap_or(&call.value)
        } else {
            call.value.split('(').next().unwrap_or("UNKNOWN")
        };
        let (kind, destination, refs, client, operation) =
            if let Some((provenance, operation, kind)) = accepted {
                (
                    kind,
                    format!(
                        "{provenance}\n{operation}\nDestination UNKNOWN{}",
                        if resolution.branches.is_empty() {
                            String::new()
                        } else {
                            format!(
                                "\nUnion dispatch UNKNOWN; unresolved alternatives: {}",
                                if alternatives.is_empty() {
                                    "none; SDK selection UNKNOWN"
                                } else {
                                    &alternatives
                                }
                            )
                        }
                    ),
                    refs.clone(),
                    Some(provenance),
                    Some(operation),
                )
            } else {
                (
                    "UNKNOWN unresolved destination",
                    format!("{head}\nUNKNOWN destination\nNot classified as network"),
                    if refs.is_empty() {
                        vec![citation(call)]
                    } else {
                        refs.clone()
                    },
                    None,
                    None,
                )
            };
        let provenance_basis = if link.is_some() {
            "focus static target candidate"
        } else if client.is_some() && !resolution.branches.is_empty() {
            "declared SDK return branch candidate; runtime selection UNKNOWN"
        } else if client.is_some() && refs.iter().any(|r| r["kind"] == "return_annotation") {
            "annotated factory-return candidate; return type and runtime dispatch not verified"
        } else if client.is_some() {
            "static import/type candidate; runtime dispatch not verified"
        } else {
            "unresolved source expression; no accepted network provenance"
        };
        calls.push(json!({"callFactId":call.id,"label":head,"sourceRefs":[citation(call)],
            "ownerFactId":owners.get(&i).map(|i|&facts[*i].id),
            "classification":if link.is_some() {"resolved static target candidate"} else {kind},"targetCandidate":link,
            "client":client,"operation":operation,"provenanceBasis":provenance_basis,"resolutionTrace":trace,"branches":resolution.branches}));
        if link.is_none() {
            let boundary = json!({"callFactId":call.id,"classification":kind,"label":destination,"client":client,"operation":operation,"sourceRefs":refs,"provenanceBasis":provenance_basis,"resolutionTrace":trace,"branches":resolution.branches});
            boundaries.push(boundary.clone());
            if let Some(client) = &client {
                network_groups
                    .entry(format!("{client}\n{provenance_basis}"))
                    .or_default()
                    .push(boundary);
                for (other_client, other_operation, other_kind) in
                    resolution.candidates.iter().skip(1)
                {
                    let other = json!({"callFactId":call.id,"classification":other_kind,"client":other_client,"operation":other_operation,"sourceRefs":refs,"provenanceBasis":provenance_basis,"branches":resolution.branches,"resolutionTrace":trace});
                    network_groups
                        .entry(format!("{other_client}\n{provenance_basis}"))
                        .or_default()
                        .push(other);
                }
            } else {
                unclassified_calls += 1;
            }
        }
    }
    let mut routine_calls = 0;
    if matches!(view, View::Services) {
        // Select a connected, handler-rooted BFS tree before spending any call
        // nodes on optional details. File ordering cannot starve root edges.
        let mut outgoing: BTreeMap<usize, Vec<(usize, usize)>> = BTreeMap::new();
        for link in links {
            let call = *by_id
                .get(
                    link["sourceFactId"]
                        .as_str()
                        .ok_or("Focus call ID missing")?,
                )
                .ok_or("Focus call missing")?;
            let target = *by_id
                .get(
                    link["targetFactId"]
                        .as_str()
                        .ok_or("Focus target ID missing")?,
                )
                .ok_or("Focus target missing")?;
            if let Some(owner) = owners.get(&call) {
                outgoing.entry(*owner).or_default().push((call, target));
            }
        }
        for children in outgoing.values_mut() {
            children.sort_by_key(|(call, target)| {
                (
                    facts[*call].line,
                    &facts[*call].id,
                    &facts[*target].symbol,
                    &facts[*target].id,
                )
            });
        }
        let mut distances = BTreeMap::from([(anchor, 0usize)]);
        let mut queue = VecDeque::from([anchor]);
        let mut connectors = BTreeSet::new();
        let mut callable_order = Vec::new();
        while let Some(owner) = queue.pop_front() {
            for &(call, target) in outgoing.get(&owner).into_iter().flatten() {
                let next_distance = distances[&owner] + 1;
                if let std::collections::btree_map::Entry::Vacant(entry) = distances.entry(target) {
                    entry.insert(next_distance);
                    queue.push_back(target);
                }
                if callable_nodes.contains_key(facts[owner].id.as_str())
                    && !callable_nodes.contains_key(facts[target].id.as_str())
                    && callable_nodes.len() < MAX_CALLABLE_NODES
                {
                    let key = format!("S{}", callable_nodes.len());
                    callable_nodes.insert(facts[target].id.as_str(), key);
                    callable_order.push(target);
                    connectors.insert(facts[call].id.as_str());
                }
            }
        }
        let mut routine_by_owner: BTreeMap<usize, usize> = BTreeMap::new();
        let mut eligible = Vec::new();
        for (number, call) in calls.iter_mut().enumerate() {
            let owner = call["ownerFactId"].as_str().unwrap_or("");
            let head = call["label"].as_str().unwrap_or("");
            let routine = call["targetCandidate"].is_null()
                && call["client"].is_null()
                && (routine_expression(head)
                    || javascript
                        && dotted(head)
                        && matches!(
                            head.rsplit('.').next(),
                            Some("toUpperCase" | "trim" | "toLowerCase")
                        ));
            if routine {
                routine_calls += 1;
                *routine_by_owner
                    .entry(
                        owners
                            .get(&by_id[call["callFactId"].as_str().unwrap()])
                            .copied()
                            .unwrap_or(anchor),
                    )
                    .or_default() += 1;
                call["omissionReason"] =
                    json!("routine/unqualified unresolved expression; retained in legend");
            } else if callable_nodes.contains_key(owner) {
                eligible.push(number);
                call["omissionReason"] = json!("call-node limit");
            } else {
                call["omissionReason"] = json!("owner outside connected diagram");
            }
        }
        // Among unresolved details, keep one distinctive field/client expression
        // per callable ahead of repeated shallow calls. This is a layout policy,
        // not SDK or network classification.
        eligible.sort_by_key(|n| {
            let call = &calls[*n];
            let f = facts[by_id[call["callFactId"].as_str().unwrap()]];
            (
                std::cmp::Reverse(call["label"].as_str().unwrap_or("").split('.').count()),
                f.line,
                &f.path,
                &f.id,
            )
        });
        let mut first_unresolved = BTreeSet::new();
        let mut seen_owners = BTreeSet::new();
        let mut seen_expressions = BTreeSet::new();
        let mut duplicates = Vec::new();
        eligible.retain(|n| {
            let call = &calls[*n];
            if !call["targetCandidate"].is_null() || !call["client"].is_null() {
                return true;
            }
            let owner = call["ownerFactId"].as_str().unwrap_or("").to_string();
            if !seen_expressions.insert((
                owner.clone(),
                call["label"].as_str().unwrap_or("").to_string(),
            )) {
                duplicates.push(*n);
                return false;
            }
            if seen_owners.insert(owner) {
                first_unresolved.insert(*n);
            }
            true
        });
        for n in duplicates {
            calls[n]["omissionReason"] = json!("repeated unresolved operation; retained in legend");
        }
        eligible.sort_by_key(|n| {
            let call = &calls[*n];
            let f = facts[by_id[call["callFactId"].as_str().unwrap()]];
            let owner = owners
                .get(&by_id[call["callFactId"].as_str().unwrap()])
                .copied()
                .unwrap_or(anchor);
            let priority = if connectors.contains(f.id.as_str()) {
                0
            } else if !call["client"].is_null() {
                1
            } else if first_unresolved.contains(n) {
                2
            } else if !call["targetCandidate"].is_null() {
                3
            } else {
                4
            };
            (
                priority,
                distances.get(&owner).copied().unwrap_or(usize::MAX),
                std::cmp::Reverse(call["label"].as_str().unwrap_or("").split('.').count()),
                &f.path,
                f.line,
                &f.id,
            )
        });
        for target in callable_order {
            let f = facts[target];
            let key = &callable_nodes[f.id.as_str()];
            let routine = routine_by_owner.get(&target).copied().unwrap_or(0);
            let display = if routine > 0 {
                format!(
                    "{}\n{routine} routine calls in legend",
                    display_symbol(target)
                )
            } else {
                display_symbol(target)
            };
            mermaid.push_str(&format!("  {key}[\"{}\"]\n", label(&display)));
            nodes.push(json!({"id":key,"label":f.symbol,"kind":"callable/service collaborator candidate","distance":distances[&target],"routineCallsInLegend":routine,"sourceRefs":[citation(f)]}));
        }
        // Group presentation only; each selected call keeps its original target proof.
        let mut groups: Vec<Vec<usize>> = Vec::new();
        let mut resolved_groups: BTreeMap<(String, String, String, String), usize> =
            BTreeMap::new();
        for number in eligible {
            let call = &calls[number];
            let link = &call["targetCandidate"];
            if js_services && !link.is_null() {
                let key = (
                    call["ownerFactId"].as_str().unwrap_or("").to_string(),
                    link["targetFactId"].as_str().unwrap_or("").to_string(),
                    link["kind"].to_string(),
                    link["resolutionPolicy"].to_string(),
                );
                if let Some(&group) = resolved_groups.get(&key) {
                    groups[group].push(number);
                    continue;
                }
                resolved_groups.insert(key, groups.len());
            }
            groups.push(vec![number]);
        }
        for group in groups.into_iter().take(MAX_CALL_NODES) {
            let number = group[0];
            let call = &calls[number];
            let id = call["callFactId"].as_str().unwrap();
            let f = facts[by_id[id]];
            let owner = &callable_nodes[call["ownerFactId"].as_str().unwrap()];
            let head = call["label"].as_str().unwrap_or("UNKNOWN");
            let key = format!("C{number}");
            if js_services {
                let link = &call["targetCandidate"];
                let call_ids: Vec<_> = group
                    .iter()
                    .map(|n| calls[*n]["callFactId"].clone())
                    .collect();
                let mut proof_ids = BTreeSet::new();
                for n in &group {
                    proof_ids.insert(calls[*n]["callFactId"].as_str().unwrap());
                    proof_ids.extend(
                        calls[*n]["targetCandidate"]["evidenceIds"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(Value::as_str),
                    );
                }
                let refs: Vec<_> = proof_ids
                    .iter()
                    .map(|id| citation(facts[by_id[id]]))
                    .collect();
                if let Some(target_id) = link["targetFactId"].as_str() {
                    let target = if let Some(target) = callable_nodes.get(target_id) {
                        target.clone()
                    } else {
                        let boundary = format!("B{number}");
                        let display = format!(
                            "{}\nTarget omitted at callable limit",
                            display_symbol(by_id[target_id])
                        );
                        mermaid.push_str(&format!("  {boundary}[\"{}\"]\n", label(&display)));
                        nodes.push(json!({"id":boundary,"label":display,"kind":"omitted target boundary","targetFactId":target_id,"sourceRefs":[citation(facts[by_id[target_id]])]}));
                        boundary
                    };
                    let display = format!(
                        "{} source call {}",
                        group.len(),
                        if group.len() == 1 { "site" } else { "sites" }
                    );
                    mermaid.push_str(&format!("  {owner} -.->|\"{display}\"| {target}\n"));
                    edges.push(json!({"id":key,"from":owner,"to":target,"kind":"static target candidate","basis":link["kind"],"resolutionPolicy":link["resolutionPolicy"],"ownerFactId":call["ownerFactId"],"targetFactId":target_id,"callFactIds":call_ids,"siteCount":group.len(),"sourceFactIds":proof_ids,"sourceRefs":refs}));
                } else {
                    let display = format!("{head}\nImplementation unresolved");
                    mermaid.push_str(&format!(
                        "  {key}[\"{}\"]\n  {owner} -->|\"observed call site\"| {key}\n",
                        label(&display)
                    ));
                    nodes.push(json!({"id":key,"label":display,"kind":"unresolved implementation","sourceRefs":refs}));
                    edges.push(json!({"from":owner,"to":key,"kind":"observed call site","callFactIds":call_ids,"siteCount":1,"sourceRefs":refs}));
                }
                rendered_calls += group.len();
                rendered_call_groups += 1;
                for n in group {
                    calls[n]["diagramId"] = json!(key);
                    calls[n]["omissionReason"] = Value::Null;
                }
                continue;
            }
            mermaid.push_str(&format!(
                "  {key}[\"{}\"]\n  {owner} -->|\"observed call site\"| {key}\n",
                label(head)
            ));
            nodes.push(json!({"id":key,"label":head,"kind":"observed call site","sourceRefs":[citation(f)]}));
            edges.push(json!({"from":owner,"to":key,"kind":"observed call site","sourceRefs":[citation(f)]}));
            let link = &call["targetCandidate"];
            if !link.is_null() {
                let target_id = link["targetFactId"]
                    .as_str()
                    .ok_or("Focus target ID missing")?;
                if let Some(target) = callable_nodes.get(target_id) {
                    mermaid.push_str(&format!(
                        "  {key} -.->|\"static target candidate\"| {target}\n"
                    ));
                    edges.push(json!({"from":key,"to":target,"kind":"static target candidate","basis":link["kind"],"sourceFactIds":link["evidenceIds"]}));
                } else {
                    let target = facts[by_id[target_id]];
                    let boundary = format!("B{number}");
                    let display = format!(
                        "{}\nTarget omitted at callable limit",
                        short_symbol(&target.symbol)
                    );
                    mermaid.push_str(&format!("  {boundary}[\"{}\"]\n  {key} -.->|\"target outside diagram\"| {boundary}\n",label(&display)));
                    nodes.push(json!({"id":boundary,"label":display,"kind":"omitted target boundary","targetFactId":target_id,"sourceRefs":[citation(target)]}));
                    edges.push(json!({"from":key,"to":boundary,"kind":"omitted target boundary","sourceFactIds":link["evidenceIds"]}));
                }
            } else if let Some(b) = boundaries
                .iter()
                .find(|b| b["callFactId"].as_str() == Some(id))
            {
                let boundary = format!("B{number}");
                let display = b["label"].as_str().unwrap_or("UNKNOWN destination");
                let kind = b["classification"]
                    .as_str()
                    .unwrap_or("UNKNOWN unresolved destination");
                mermaid.push_str(&format!(
                    "  {boundary}[\"{}\"]\n  {key} -.->|\"{}\"| {boundary}\n",
                    label(display),
                    label(kind)
                ));
                nodes.push(
                    json!({"id":boundary,"label":display,"kind":kind,"sourceRefs":b["sourceRefs"]}),
                );
                edges.push(
                    json!({"from":key,"to":boundary,"kind":kind,"basis":b["provenanceBasis"],"branches":b["branches"],"sourceRefs":b["sourceRefs"]}),
                );
            }
            rendered_calls += 1;
            rendered_call_groups += 1;
            calls[number]["diagramId"] = json!(key);
            calls[number]["omissionReason"] = Value::Null;
        }
    }
    if matches!(view, View::Network) {
        for (number, (_, group)) in network_groups.iter().take(MAX_NETWORK_NODES).enumerate() {
            let client = group[0]["client"].as_str().unwrap_or("UNKNOWN");
            let alternatives: BTreeSet<_> = group
                .iter()
                .flat_map(|b| b["branches"].as_array().into_iter().flatten())
                .filter(|b| b["classification"] == "UNKNOWN")
                .filter_map(|b| b["member"].as_str())
                .collect();
            let alternatives = alternatives.into_iter().collect::<Vec<_>>().join(", ");
            let operations: BTreeSet<_> = group
                .iter()
                .filter_map(|b| b["operation"].as_str())
                .collect();
            let visible = operations
                .iter()
                .take(4)
                .copied()
                .collect::<Vec<_>>()
                .join(", ");
            let overflow = if operations.len() > 4 {
                format!(" (+{} more)", operations.len() - 4)
            } else {
                String::new()
            };
            let destination = format!(
                "{}\n{visible}{overflow}\nDestination UNKNOWN{}",
                short_symbol(client),
                if group
                    .iter()
                    .any(|b| b["branches"].as_array().is_some_and(|a| !a.is_empty()))
                {
                    format!(
                        "\nUnion dispatch UNKNOWN; unresolved alternatives: {}",
                        if alternatives.is_empty() {
                            "none; SDK selection UNKNOWN"
                        } else {
                            &alternatives
                        }
                    )
                } else {
                    String::new()
                }
            );
            let kind = group[0]["classification"]
                .as_str()
                .unwrap_or("network boundary candidate");
            let refs: BTreeMap<_, _> = group
                .iter()
                .flat_map(|b| b["sourceRefs"].as_array().into_iter().flatten())
                .filter_map(|r| r["factId"].as_str().map(|id| (id, r)))
                .collect();
            let key = format!("N{number}");
            mermaid.push_str(&format!(
                "  {key}[\"{}\"]\n  A -.->|\"{}\"| {key}\n",
                label(&destination),
                label(kind)
            ));
            nodes.push(json!({"id":key,"label":destination,"kind":kind,"client":client,"operations":operations,"basis":group[0]["provenanceBasis"],"branches":group.iter().map(|b| &b["branches"]).collect::<Vec<_>>(),
                "callFactIds":group.iter().map(|b|&b["callFactId"]).collect::<Vec<_>>(),"sourceRefs":refs.values().collect::<Vec<_>>()}));
            edges.push(json!({"from":"A","to":key,"kind":kind,"basis":group[0]["provenanceBasis"],"branches":group.iter().map(|b| &b["branches"]).collect::<Vec<_>>(),"client":client,"operations":operations,
                "callFactIds":group.iter().map(|b|&b["callFactId"]).collect::<Vec<_>>(),"sourceRefs":refs.values().collect::<Vec<_>>()}));
        }
    }
    let omitted_calls = if matches!(view, View::Services) {
        calls.len() - rendered_calls
    } else {
        0
    };
    let omitted_network_groups = network_groups.len().saturating_sub(MAX_NETWORK_NODES);
    let omitted_callable_nodes = if matches!(view, View::Services) {
        links
            .iter()
            .filter_map(|l| l["targetFactId"].as_str())
            .collect::<BTreeSet<_>>()
            .iter()
            .filter(|id| !callable_nodes.contains_key(**id))
            .count()
    } else {
        0
    };
    if omitted_calls > 0 || omitted_network_groups > 0 || omitted_callable_nodes > 0 {
        warnings.insert(format!("Diagram selection/layout: {omitted_calls} call sites ({routine_calls} routine/unqualified expressions), {omitted_callable_nodes} callable targets and {omitted_network_groups} network groups omitted from graph; selected call records, candidate links and boundaries remain in the legend."));
    }
    let contract_scope = if !is_api {
        "Callable source view".to_string()
    } else if spec.is_none() {
        "Source-only: no local contract supplied".to_string()
    } else {
        format!(
            "Local contract: {} (unverified)",
            association["status"].as_str().unwrap_or("unknown")
        )
    };
    let scope = match view {
        View::Network => format!("{}\n{} selected calls; {unclassified_calls} unclassified\n{contract_scope}",
            if network_groups.is_empty() {"No proven network candidates".to_string()} else {format!("{} provenance-backed client groups",network_groups.len())},calls.len()),
        View::Services => format!("Service scope: {rendered_calls}/{} call sites shown\n{routine_calls} routine; {omitted_calls} omitted in legend\n{contract_scope}",calls.len()),
    };
    mermaid.push_str(&format!("  INFO[\"{}\"]\n", label(&scope)));
    nodes.push(json!({"id":"INFO","label":scope,"kind":"scope summary","networkGroups":network_groups.len(),"unclassifiedCalls":unclassified_calls,
        "selectedCalls":calls.len(),"routineCallsInLegend":routine_calls,"omittedCallSites":omitted_calls,"contractStatus":association["status"]}));
    warnings.insert(format!("{unclassified_calls} unresolved/unclassified calls retained in the legend, not asserted as network traffic. HTTP/SDK operation names are source expressions, not verified verbs, URLs or remote paths."));
    let limits = json!({"hops":hops,"maxFacts":max_facts,"byteSelectionPolicy":"endpoint-headroom-v1","focusMaxBytes":focus_max_bytes,"focusMaxFacts":focus_budget,"reservedBytes":reserved_bytes,"provenanceReserveBytes":PROVENANCE_RESERVE_BYTES,"routeReserveBytes":route_reserve_bytes,"contextReserveBytes":context_reserve_bytes,"contextReserveFacts":context_reserve_facts,"contextProofBytes":context_proof_bytes,"contextProofFacts":context_proof_facts,"envelopeReserveBytes":ENVELOPE_RESERVE_BYTES,"provenanceBytes":retained_input_bytes.saturating_sub(base_input_bytes),"baseInputBytes":base_input_bytes,"routeAttachmentBytes":base_input_bytes.saturating_sub(focused_input_bytes),"retainedInputBytes":retained_input_bytes,"maxInputBytes":crate::packet::MAX_INPUT_BYTES,"inputByteFormat":"compact UTF-8 JSON; full retained facts and files; complete focus envelope and selection metadata","maxCallNodes":MAX_CALL_NODES,"maxCallableNodes":MAX_CALLABLE_NODES,"maxNetworkNodes":MAX_NETWORK_NODES});
    let mut limits = limits;
    if js_services {
        limits["maxCallGroups"] = json!(MAX_CALL_NODES);
    }
    limits["entryMetadataReserveFacts"] = json!(route_reserve);
    limits["entryMetadataReserveBytes"] = json!(route_reserve_bytes);
    limits["entryMetadataBytes"] = json!(base_input_bytes.saturating_sub(focused_input_bytes));
    if !is_api {
        limits["routeReserveBytes"] = json!(0);
        limits["routeAttachmentBytes"] = json!(0);
    }
    let mut legend = json!({"schemaVersion":if is_api {"ideomancer.endpoint-view.v1"} else {"ideomancer.callable-view.v1"},"view":view,"snapshotId":catalog.snapshot_id,
        "sourceRoot":catalog.source_root,"extractorVersion":catalog.extractor_version,"complete":catalog.complete,
        "item":item,"sourceModel":"static-candidate","contractMatch":association,"routerContext":if is_api {json!(router_context)} else {json!({"status":"not-applicable"})},
        "contract":spec.map(|s|json!({"path":s.path,"sha256":s.sha256,"version":s.version,"securitySchemes":s.security_schemes,"boundaries":s.boundaries})),
        "nodes":nodes,"edges":edges,"calls":calls,"boundaries":boundaries,"focus":prepared.input["focus"],"coverage":prepared.coverage,
        "retainedFactCount":retained.len(),"sourceRefs":retained.iter().map(|id|citation(facts[by_id[id.as_str()]])).collect::<Vec<_>>(),
        "warnings":warnings,"counts":{"selectedCalls":calls.len(),"renderedCallSites":rendered_calls,"omittedCallSites":omitted_calls,
             "unclassifiedCalls":unclassified_calls,"networkGroups":network_groups.len(),"omittedNetworkGroups":omitted_network_groups,"omittedCallableNodes":omitted_callable_nodes,"routineCallsInLegend":routine_calls},
        "limits":limits,
        "reportCode":if is_api {"STATIC_ENDPOINT_CANDIDATES"} else {"STATIC_CALLABLE_CANDIDATES"}});
    if js_services {
        legend["counts"]["renderedCallGroups"] = json!(rendered_call_groups);
    }
    let details = serde_json::to_string_pretty(&legend).map_err(|e| e.to_string())?;
    let title = match (is_api, view) {
        (true, View::Network) => "Endpoint Network Candidates",
        (true, View::Services) => "Endpoint Service Activation Candidates",
        (false, View::Network) => "Callable Network Candidates",
        (false, View::Services) => "Callable Service and Call Candidates",
    };
    let contract_section = if is_api {
        contract_html(&association, spec)
    } else {
        String::new()
    };
    let mut unresolved_section = String::new();
    if matches!(view, View::Services) && unclassified_calls > 0 {
        let mut operations: BTreeMap<&str, Vec<&Value>> = BTreeMap::new();
        for call in &calls {
            if call["classification"] == "UNKNOWN unresolved destination" {
                operations
                    .entry(call["label"].as_str().unwrap_or("UNKNOWN"))
                    .or_default()
                    .push(call);
            }
        }
        unresolved_section.push_str("<section><h2>Unresolved Source Operations</h2><p>UNKNOWN destinations, not classified as network. Includes selected operations omitted from the diagram at layout limits. Forward annotations and dynamic wrappers are not interpreted; bounded direct factory union branches remain unselected alternatives.</p><ul>");
        for (operation, sites) in operations {
            let source = &sites[0]["sourceRefs"][0];
            let location = format!(
                "{}:{}",
                source["path"].as_str().unwrap_or("UNKNOWN"),
                source["line"]
            );
            unresolved_section.push_str(&format!("<li><code>{}</code>: UNKNOWN destination ({} source call sites); <code>{}</code>. Full citations remain in the legend.</li>",html_text(operation),sites.len(),html_text(&location)));
        }
        unresolved_section.push_str("</ul></section>");
    }
    let citations_title = if is_api {
        "Contract, Boundaries and Source Citations"
    } else {
        "Boundaries and Source Citations"
    };
    let diagram_title = if is_api {
        "Endpoint source candidates"
    } else {
        "Callable source candidates"
    };
    let html = super::viewer::document(title, &format!("<h1>{title}</h1><p>Static source candidates, not a runtime trace or activation order. Dashed targets are unverified. Keep source metadata local.</p><p>{unclassified_calls} unclassified calls are retained in the legend, not asserted as network traffic. {omitted_calls} service call sites omitted by presentation selection/limits, including {routine_calls} routine expressions.</p>{contract_section}{}{unresolved_section}<h2>Readable Diagram Source</h2><pre>{}</pre><h2>{citations_title}</h2><pre>{}</pre>",super::viewer::diagram(diagram_title, "Calls and target candidates are static evidence, not an execution trace. Source references remain outside this viewport."),html_text(&mermaid),html_text(&details)));
    if mermaid.len().max(details.len()).max(html.len()) > 8 * 1024 * 1024 {
        return Err("Endpoint view exceeds the 8 MiB document limit".into());
    }
    Ok(Documents {
        mermaid,
        legend,
        html,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn supported_router_context_is_reserved_before_fact_and_byte_focus_truncation() {
        use crate::router_context::tests::{extract, DIRECT};
        let calls = (0..600)
            .map(|n| {
                format!(
                    "    self.remote.operation_{n}{}(request)\n",
                    "x".repeat(300)
                )
            })
            .collect::<String>();
        let source = DIRECT.replace(
            "    return 'PRIVATE_PAYLOAD'",
            &format!("{calls}    return 'PRIVATE_PAYLOAD'"),
        );
        let Some((evidence, catalog, item)) = extract(&source) else {
            return;
        };
        for max_facts in [80, 2000] {
            let d = super::build(
                &catalog,
                &evidence,
                &item,
                &super::View::Services,
                None,
                1,
                max_facts,
            )
            .unwrap();
            assert_eq!(d.legend["routerContext"]["status"], "supported");
            let limits = &d.legend["limits"];
            let ids: BTreeSet<String> = d.legend["sourceRefs"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r["factId"].as_str().unwrap().to_owned())
                .collect();
            let context_ids: BTreeSet<_> = d.legend["routerContext"]["evidenceIds"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_owned())
                .collect();
            assert!(context_ids.is_subset(&ids));
            let context_facts: Vec<_> = evidence
                .facts
                .iter()
                .filter(|f| context_ids.contains(&f.id) && !item.evidence_ids.contains(&f.id))
                .collect();
            let reserve_bytes = serde_json::to_vec(&json!({"facts":context_facts,"files":[]}))
                .unwrap()
                .len();
            assert_eq!(limits["contextProofBytes"], reserve_bytes);
            assert_eq!(limits["contextReserveBytes"], reserve_bytes);
            assert_eq!(limits["contextReserveFacts"], context_facts.len());
            assert_eq!(
                limits["focusMaxFacts"].as_u64().unwrap() as usize
                    + context_facts.len()
                    + item.evidence_ids.len()
                    - 1,
                max_facts
            );
            assert_eq!(
                limits["focusMaxBytes"].as_u64().unwrap()
                    + limits["reservedBytes"].as_u64().unwrap(),
                crate::packet::MAX_INPUT_BYTES as u64
            );
            assert!(ids.len() <= max_facts);
            let prepared = crate::focus::prepare_with_byte_limit(
                &evidence,
                &crate::focus::FocusOptions {
                    symbol: item.fact_id.clone(),
                    hops: 1,
                    max_facts: limits["focusMaxFacts"].as_u64().unwrap() as usize,
                },
                limits["focusMaxBytes"].as_u64().unwrap() as usize,
            )
            .unwrap();
            assert!(prepared.coverage.omitted_facts > 0);
            let mut facts: Vec<_> = evidence.facts.iter().collect();
            facts.sort_by_key(|f| (&f.path, &f.symbol, f.line, f.end_line, &f.kind, &f.id));
            let bytes = serde_json::to_vec(&super::retained_input(
                &prepared.input,
                &evidence,
                &facts,
                &ids,
            ))
            .unwrap()
            .len();
            assert_eq!(limits["retainedInputBytes"], bytes);
            assert!(bytes <= crate::packet::MAX_INPUT_BYTES);
        }
    }

    #[test]
    fn converter_registration_proof_is_reserved_and_budget_failure_disables_projection() {
        use crate::router_context::tests::{extract, DIRECT};
        let source = format!("{}\nfrom starlette.convertors import register_url_convertor as reg\nreg('path', Custom())\n", DIRECT.replace("'/items'", "'/{name:path}'"));
        let Some((mut evidence, catalog, item)) = extract(&source) else {
            return;
        };
        let contract = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(contract.path(), br#"{"openapi":"3.1.0","paths":{"/caps/{name}":{"get":{}},"/caps/{name:path}":{"get":{}}}}"#).unwrap();
        let doc = crate::openapi::load(&contract.path().canonicalize().unwrap()).unwrap();
        let full = super::build(
            &catalog,
            &evidence,
            &item,
            &super::View::Services,
            Some(&doc),
            0,
            200,
        )
        .unwrap();
        assert_eq!(
            full.legend["routerContext"]["converterProjection"]["status"],
            "blocked"
        );
        assert_eq!(full.legend["contractMatch"]["status"], "candidate");
        assert_eq!(
            full.legend["contractMatch"]["candidates"][0]["path"],
            "/caps/{name:path}"
        );
        let registration = evidence
            .facts
            .iter_mut()
            .find(|f| f.kind == "call" && f.value.starts_with("reg("))
            .unwrap();
        let id = registration.id.clone();
        assert!(full.legend["routerContext"]["evidenceIds"]
            .as_array()
            .unwrap()
            .contains(&json!(id)));
        assert!(full.legend["sourceRefs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["factId"] == id));
        registration.value = format!("reg({})", "x".repeat(crate::packet::MAX_INPUT_BYTES));
        let limited = super::build(
            &catalog,
            &evidence,
            &item,
            &super::View::Services,
            Some(&doc),
            0,
            200,
        )
        .unwrap();
        assert_eq!(limited.legend["routerContext"]["status"], "budget-unknown");
        assert_ne!(
            limited.legend["routerContext"]["converterProjection"]["status"],
            "eligible"
        );
        assert_eq!(limited.legend["contractMatch"]["status"], "unknown");
        assert!(limited.legend["routerContext"]["sourceRefs"]
            .as_array()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn router_context_narrows_only_with_complete_exact_budgeted_proof() {
        use crate::router_context::tests::{extract, DIRECT};
        let Some((mut evidence, catalog, item)) = extract(DIRECT) else {
            return;
        };
        let doc = crate::openapi::Document {
            path: "synthetic.json".into(),
            sha256: "synthetic".into(),
            version: "3.1.0".into(),
            operations: ["/api/caps/items", "/tails/items"]
                .into_iter()
                .map(|path| crate::openapi::Operation {
                    method: "GET".into(),
                    path: path.into(),
                    pointer: path.into(),
                    operation_id: None,
                    summary: None,
                    tags: vec![],
                    parameters: vec![],
                    request: serde_json::Value::Null,
                    responses: serde_json::Value::Null,
                    security: serde_json::Value::Null,
                    security_basis: "declared".into(),
                    servers: serde_json::Value::Null,
                    server_basis: "unknown".into(),
                    boundaries: vec![],
                })
                .collect(),
            security_schemes: serde_json::Value::Null,
            boundaries: vec![],
        };
        let full = super::build(
            &catalog,
            &evidence,
            &item,
            &super::View::Services,
            Some(&doc),
            1,
            200,
        )
        .unwrap();
        assert_eq!(full.legend["routerContext"]["status"], "supported");
        assert_eq!(full.legend["contractMatch"]["status"], "candidate");
        assert_eq!(
            full.legend["contractMatch"]["candidates"][0]["path"],
            "/api/caps/items"
        );
        let refs = full.legend["sourceRefs"].as_array().unwrap();
        for id in full.legend["routerContext"]["evidenceIds"]
            .as_array()
            .unwrap()
        {
            assert!(refs.iter().any(|r| r["factId"] == *id));
        }
        let ids = refs
            .iter()
            .map(|r| r["factId"].as_str().unwrap().to_owned())
            .collect();
        let mut facts: Vec<_> = evidence.facts.iter().collect();
        facts.sort_by_key(|f| (&f.path, &f.symbol, f.line, f.end_line, &f.kind, &f.id));
        let prepared = crate::focus::prepare_with_byte_limit(
            &evidence,
            &crate::focus::FocusOptions {
                symbol: item.fact_id.clone(),
                hops: 1,
                max_facts: full.legend["limits"]["focusMaxFacts"].as_u64().unwrap() as usize,
            },
            full.legend["limits"]["focusMaxBytes"].as_u64().unwrap() as usize,
        )
        .unwrap();
        let bytes = serde_json::to_vec(&super::retained_input(
            &prepared.input,
            &evidence,
            &facts,
            &ids,
        ))
        .unwrap()
        .len();
        assert_eq!(full.legend["limits"]["retainedInputBytes"], bytes);
        assert!(bytes <= crate::packet::MAX_INPUT_BYTES);
        let limited = super::build(
            &catalog,
            &evidence,
            &item,
            &super::View::Services,
            Some(&doc),
            1,
            item.evidence_ids.len(),
        )
        .unwrap();
        assert_eq!(limited.legend["routerContext"]["status"], "budget-unknown");
        assert_eq!(limited.legend["contractMatch"]["status"], "unknown");
        let rule = evidence
            .facts
            .iter_mut()
            .find(|f| f.kind == "router_prefix")
            .unwrap();
        let mut value: serde_json::Value = serde_json::from_str(&rule.value).unwrap();
        value["padding"] = serde_json::json!("x".repeat(crate::packet::MAX_INPUT_BYTES));
        rule.value = value.to_string();
        let limited = super::build(
            &catalog,
            &evidence,
            &item,
            &super::View::Services,
            Some(&doc),
            1,
            200,
        )
        .unwrap();
        assert_eq!(limited.legend["routerContext"]["status"], "budget-unknown");
        assert_eq!(limited.legend["contractMatch"]["status"], "unknown");
        assert!(limited.legend["routerContext"]["sourceRefs"]
            .as_array()
            .unwrap()
            .is_empty());
    }
    use super::*;

    #[test]
    fn javascript_services_group_source_sites_with_complete_shared_proof() {
        use crate::js_focus::tests::add;
        let mut e = crate::js_focus::tests::source();
        for n in 0..3 {
            let id = format!("repeat{n}");
            add(
                &mut e,
                &id,
                "call",
                "app.$run",
                8 + n,
                8 + n,
                json!("work()"),
            );
            add(
                &mut e,
                &format!("repeat-meta{n}"),
                "js_call",
                "app.$run",
                8 + n,
                8 + n,
                json!({"callFactId":id,"ownerFactId":"run","callee":"work","targetFactId":"work","supportingFactIds":["work","work-meta","work-guard"],"lookupScopeIds":["scope-run-body","scope-run","scope-module"]}),
            );
        }
        // A recursive call must not merge with the same target under another owner.
        add(
            &mut e,
            "recursive",
            "call",
            "app.work",
            31,
            31,
            json!("work()"),
        );
        add(
            &mut e,
            "recursive-meta",
            "js_call",
            "app.work",
            31,
            31,
            json!({"callFactId":"recursive","ownerFactId":"work","callee":"work","targetFactId":"work","supportingFactIds":["work","work-meta","work-guard"],"lookupScopeIds":["scope-module"]}),
        );
        let c = crate::catalog::build(&e, "local").unwrap();
        let item = c.items.iter().find(|i| i.fact_id == "run").unwrap();
        let d = build(&c, &e, item, &View::Services, None, 2, 100).unwrap();
        let edges: Vec<_> = d.legend["edges"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|edge| edge["kind"] == "static target candidate")
            .collect();
        assert_eq!(edges.len(), 2);
        let group = edges
            .iter()
            .find(|edge| edge["ownerFactId"] == "run")
            .unwrap();
        assert_eq!(group["siteCount"], 4);
        assert_eq!(group["callFactIds"].as_array().unwrap().len(), 4);
        let refs: BTreeSet<_> = group["sourceRefs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["factId"].as_str().unwrap())
            .collect();
        let grouped_calls: Vec<_> = d.legend["calls"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|call| call["ownerFactId"] == "run" && !call["targetCandidate"].is_null())
            .collect();
        assert_eq!(grouped_calls.len(), 4);
        for call in grouped_calls {
            assert_eq!(call["diagramId"], group["id"]);
            for id in call["targetCandidate"]["evidenceIds"].as_array().unwrap() {
                assert!(
                    refs.contains(id.as_str().unwrap()),
                    "missing grouped proof {id}"
                );
            }
        }
        assert!(d.mermaid.contains("4 source call sites"));
        assert!(!d.mermaid.contains("UNKNOWN destination"));
        assert!(!d.mermaid.contains("Not classified as network"));
        assert_eq!(d.legend["counts"]["renderedCallSites"], 6);
        assert_eq!(d.legend["counts"]["renderedCallGroups"], 3);
        e.facts.reverse();
        let repeat = build(&c, &e, item, &View::Services, None, 2, 100).unwrap();
        assert_eq!(d.mermaid, repeat.mermaid);
        assert_eq!(d.legend, repeat.legend);
    }

    #[test]
    fn javascript_services_show_one_unresolved_implementation_and_hide_routine_strings() {
        use crate::js_focus::tests::add;
        let mut e = crate::js_focus::tests::source();
        for f in e
            .facts
            .iter_mut()
            .filter(|f| f.id == "this-call" || f.id == "this-meta")
        {
            f.value = f
                .value
                .replace("this.client.get", "this.fileUploadService.upload");
        }
        for (n, method) in ["toUpperCase", "trim", "toLowerCase"].iter().enumerate() {
            let id = format!("string{n}");
            let head = format!("id.{method}");
            add(
                &mut e,
                &id,
                "call",
                "app.$run",
                10 + n,
                10 + n,
                json!(format!("{head}()")),
            );
            add(
                &mut e,
                &format!("string-meta{n}"),
                "js_call",
                "app.$run",
                10 + n,
                10 + n,
                json!({"callFactId":id,"ownerFactId":"run","callee":head,"supportingFactIds":[],"lookupScopeIds":["scope-module"]}),
            );
        }
        add(
            &mut e,
            "route",
            "api_route",
            "app",
            80,
            80,
            json!({"handlerFactId":"run","methods":["POST"],"path":"/upload","pathIsFragment":true}),
        );
        let c = crate::catalog::build(&e, "local").unwrap();
        let item = c.items.iter().find(|i| i.kind == Kind::Api).unwrap();
        let d = build(&c, &e, item, &View::Services, None, 2, 100).unwrap();
        assert!(d.mermaid.contains("Handler for POST /upload"));
        assert_eq!(
            d.mermaid.matches("this.fileUploadService.upload").count(),
            1
        );
        assert!(d.mermaid.contains("Implementation unresolved"));
        for noise in [
            "destination",
            "Not classified as network",
            "toUpperCase",
            "trim",
            "toLowerCase",
            "Internal application snapshot",
        ] {
            assert!(!d.mermaid.contains(noise), "{noise}: {}", d.mermaid);
        }
        assert_eq!(d.legend["counts"]["selectedCalls"], 5);
        assert_eq!(d.legend["counts"]["renderedCallSites"], 2);
        assert_eq!(d.legend["counts"]["routineCallsInLegend"], 3);
        assert_eq!(d.legend["counts"]["omittedCallSites"], 3);
        assert_eq!(d.legend["counts"]["networkGroups"], 0);
        let unresolved = d.legend["calls"]
            .as_array()
            .unwrap()
            .iter()
            .find(|call| call["callFactId"] == "this-call")
            .unwrap();
        assert_eq!(
            unresolved["classification"],
            "UNKNOWN unresolved destination"
        );
        assert!(unresolved["targetCandidate"].is_null());
        let network = build(&c, &e, item, &View::Network, None, 2, 100).unwrap();
        assert_eq!(network.legend["calls"].as_array().unwrap().len(), 5);
        assert_eq!(network.legend["counts"]["routineCallsInLegend"], 0);
    }

    #[test]
    fn javascript_native_calls_use_explicit_owners_and_never_python_network_proof() {
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
        let c = crate::catalog::build(&e, "local").unwrap();
        let item = c.items.iter().find(|i| i.kind == Kind::Api).unwrap();
        for view in [View::Services, View::Network] {
            let d = build(&c, &e, item, &view, None, 2, 100).unwrap();
            assert_eq!(d.legend["routerContext"]["status"], "unsupported-language");
            assert_eq!(d.legend["counts"]["networkGroups"], 0);
            let calls = d.legend["calls"].as_array().unwrap();
            assert!(calls.iter().all(|call| call["ownerFactId"] == "run"));
            assert!(calls
                .iter()
                .any(|call| call["targetCandidate"]["targetFactId"] == "work"));
            assert!(calls
                .iter()
                .any(|call| call["resolutionTrace"]["stoppingReason"]
                    .as_str()
                    .is_some_and(|s| s.contains("unsupported-language"))));
            assert!(
                d.legend["limits"]["retainedInputBytes"].as_u64().unwrap()
                    <= crate::packet::MAX_INPUT_BYTES as u64
            );
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("openapi.json");
        std::fs::write(&path,json!({"openapi":"3.0.3","paths":{"/other":{"post":{"operationId":"$run","responses":{}}}}}).to_string()).unwrap();
        let spec = crate::openapi::load(&std::fs::canonicalize(&path).unwrap()).unwrap();
        let d = build(&c, &e, item, &View::Services, Some(&spec), 2, 100).unwrap();
        assert_eq!(d.legend["contractMatch"]["status"], "unmatched");
        assert_eq!(d.legend["routerContext"]["status"], "unsupported-language");
    }

    fn fact(kind: &str, symbol: &str, line: usize, end_line: usize, value: &str) -> Fact {
        Fact {
            id: format!("{kind}{line}"),
            kind: kind.into(),
            symbol: symbol.into(),
            path: "app.py".into(),
            line,
            end_line,
            value: value.into(),
        }
    }
    fn source() -> Evidence {
        Evidence {
            schema_version: "ideomancer.evidence.v1".into(),
            snapshot_id: "s".into(),
            extractor_version: "test".into(),
            python_version: "test".into(),
            language: None,
            files: vec![],
            diagnostics: vec![],
            complete: true,
            facts: vec![
                fact("module", "app", 1, 100, "app"),
                fact(
                    "import",
                    "app",
                    2,
                    2,
                    r#"{"module":"","level":0,"names":[{"name":"requests","asname":"r"}]}"#,
                ),
                fact(
                    "api_route",
                    "app.checkin",
                    3,
                    3,
                    r#"{"methods":["POST"],"path":"/checkin","pathIsFragment":true}"#,
                ),
                fact("function", "app.checkin", 4, 20, "checkin"),
                fact("call", "app.checkin", 5, 5, "service()"),
                fact("function", "app.service", 25, 40, "service"),
                fact("call", "app.service", 26, 26, "r.post(<literal>)"),
                fact("call", "app.service", 27, 27, "unknown.get(<literal>)"),
            ],
        }
    }
    fn draw(e: &Evidence, view: View, hops: usize, max: usize) -> Documents {
        let c = crate::catalog::build(e, "local").unwrap();
        let i = c.items.iter().find(|i| i.kind == Kind::Api).unwrap();
        build(&c, e, i, &view, None, hops, max).unwrap()
    }

    #[test]
    fn callable_views_are_kind_based_and_keep_shared_call_provenance() {
        let mut e = source();
        e.facts.extend([
            fact(
                "entrypoint",
                "app.command",
                45,
                45,
                r#"{"role":"cli_command"}"#,
            ),
            fact("function", "app.command", 46, 49, "command"),
            fact("call", "app.command", 47, 47, "service()"),
            fact("entrypoint", "app.job", 50, 50, r#"{"role":"job"}"#),
            fact("async_function", "app.job", 51, 55, "job"),
            fact("call", "app.job", 52, 52, "service()"),
            fact("class", "app.Worker", 60, 80, "Worker"),
            fact("function", "app.Worker.run", 61, 70, "run"),
            fact("call", "app.Worker.run", 62, 62, "service()"),
        ]);
        let c = crate::catalog::build(&e, "local").unwrap();
        let mut roles = BTreeSet::new();
        for item in c.items.iter().filter(|i| i.kind == Kind::Workflow) {
            roles.insert(item.role.as_str());
            for view in [View::Services, View::Network] {
                let d = build(&c, &e, item, &view, None, 2, 100).unwrap();
                assert_eq!(d.legend["schemaVersion"], "ideomancer.callable-view.v1");
                assert_eq!(d.legend["reportCode"], "STATIC_CALLABLE_CANDIDATES");
                assert_eq!(
                    d.legend["contractMatch"],
                    json!({"status":"not-applicable"})
                );
                assert_eq!(
                    d.legend["routerContext"],
                    json!({"status":"not-applicable"})
                );
                assert!(d.legend["contract"].is_null());
                for field in [
                    "routeReserveBytes",
                    "routeAttachmentBytes",
                    "contextReserveBytes",
                    "contextReserveFacts",
                    "contextProofBytes",
                    "contextProofFacts",
                ] {
                    assert_eq!(d.legend["limits"][field], 0, "{field}");
                }
                assert!(!d.mermaid.contains("declared handler"));
                assert!(!d.mermaid.contains("UNKNOWN DYNAMIC"));
                assert!(!d.html.contains("Local API Contract"));
                assert!(!d.html.contains("Endpoint"));
                assert!(d.mermaid.contains(&item.symbol));
                assert!(d.mermaid.contains(&format!("{} candidate", item.role)));
                assert_eq!(d.legend["counts"]["networkGroups"], 1);
                assert!(d.legend["nodes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|n| n["id"] != "O"));
                for key in ["method", "path", "servers", "security"] {
                    assert!(d.legend["contractMatch"].get(key).is_none());
                }
                for id in &item.evidence_ids {
                    assert!(d.legend["sourceRefs"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|r| r["factId"] == *id));
                }
            }
        }
        assert_eq!(
            roles,
            BTreeSet::from([
                "http_handler",
                "module_function",
                "class_method",
                "cli_command",
                "job"
            ])
        );
    }

    #[test]
    fn selected_native_metadata_obeys_fact_and_byte_caps() {
        for role in ["cli_command", "job"] {
            let mut e = source();
            e.facts.retain(|f| f.kind != "api_route");
            e.facts.push(fact(
                "entrypoint",
                "app.checkin",
                3,
                3,
                &json!({"role":role}).to_string(),
            ));
            let c = crate::catalog::build(&e, "local").unwrap();
            let item = c.items.iter().find(|i| i.role == role).unwrap();
            assert!(build(&c, &e, item, &View::Services, None, 0, 1)
                .err()
                .unwrap()
                .contains("entry metadata"));
            let d = build(&c, &e, item, &View::Services, None, 0, 8).unwrap();
            assert_eq!(d.legend["limits"]["entryMetadataReserveFacts"], 1);
            assert!(
                d.legend["limits"]["entryMetadataReserveBytes"]
                    .as_u64()
                    .unwrap()
                    > 0
            );
            assert!(d.legend["retainedFactCount"].as_u64().unwrap() <= 8);
            assert!(
                d.legend["limits"]["retainedInputBytes"].as_u64().unwrap()
                    <= crate::packet::MAX_INPUT_BYTES as u64
            );
            let meta = e.facts.iter_mut().find(|f| f.kind == "entrypoint").unwrap();
            meta.value = json!({"role":role,"padding":"x".repeat(crate::packet::MAX_INPUT_BYTES)})
                .to_string();
            let c = crate::catalog::build(&e, "local").unwrap();
            let item = c.items.iter().find(|i| i.role == role).unwrap();
            let error = build(&c, &e, item, &View::Services, None, 0, 100)
                .err()
                .unwrap();
            assert!(
                error.contains("max-bytes") && error.contains("entry metadata"),
                "{error}"
            );
        }
    }

    #[test]
    fn both_native_kinds_require_grounded_exact_callable_anchors() {
        let mut e = source();
        e.facts.push(fact("class", "app.Worker", 60, 80, "Worker"));
        let c = crate::catalog::build(&e, "local").unwrap();
        for kind in [Kind::Api, Kind::Workflow] {
            let item = c.items.iter().find(|i| i.kind == kind).unwrap();
            assert!(build(&c, &e, item, &View::Network, None, 1, 100).is_ok());
            let mut changed = c.clone();
            changed.schema_version = "unsupported".into();
            assert!(build(&changed, &e, item, &View::Network, None, 1, 100).is_err());
            changed = c.clone();
            changed.snapshot_id = "other".into();
            assert!(build(&changed, &e, item, &View::Network, None, 1, 100).is_err());
            let mut forged = item.clone();
            forged.fact_id = "class60".into();
            changed = c.clone();
            changed.items.push(forged.clone());
            assert!(build(&changed, &e, &forged, &View::Network, None, 1, 100).is_err());
            let mut missing = source();
            missing.facts.retain(|f| f.id != item.fact_id);
            assert!(build(&c, &missing, item, &View::Network, None, 1, 100).is_err());
        }
        let mut fake = c
            .items
            .iter()
            .find(|i| i.kind == Kind::Class)
            .unwrap()
            .clone();
        fake.kind = Kind::Workflow;
        let mut forged = c.clone();
        forged.items.push(fake.clone());
        assert!(build(&forged, &e, &fake, &View::Services, None, 1, 100).is_err());
    }

    fn factory_source(return_type: &str) -> Evidence {
        let mut e = source();
        e.facts.retain(|f| f.line < 25);
        e.facts.extend([
            fact(
                "from_import",
                "app",
                21,
                21,
                r#"{"module":"pkg.factory","level":0,"names":[{"name":"Factory","asname":null}]}"#,
            ),
            fact("class", "app.Context", 30, 45, "Context"),
            fact("function", "app.Context.__init__", 31, 44, "__init__"),
            fact("parameter", "app.Context.__init__.self", 31, 31, "self"),
            fact(
                "parameter",
                "app.Context.__init__.factory",
                32,
                32,
                "factory",
            ),
            fact(
                "parameter_annotation",
                "app.Context.__init__.factory",
                32,
                32,
                "Factory",
            ),
            fact(
                "binding",
                "app.Context.__init__",
                34,
                34,
                r#"{"target":"self.benchling","source":"factory.manufacture(env)"}"#,
            ),
            fact("function", "app.service", 50, 90, "service"),
            fact(
                "binding",
                "app.service",
                51,
                51,
                r#"{"target":"ctx","source":"Context(factory=provided)"}"#,
            ),
            fact(
                "call",
                "app.service",
                52,
                52,
                "ctx.benchling.containers.checkin(<literal>)",
            ),
        ]);
        e.facts.extend([
            fact("module","pkg.factory",1,100,"pkg.factory"),
            fact("from_import","pkg.factory",2,2,r#"{"module":"benchling_sdk.benchling","level":0,"names":[{"name":"Benchling","asname":null}]}"#),
            fact("class","pkg.factory.Factory",10,60,"Factory"),
            fact("function","pkg.factory.Factory.manufacture",20,40,"manufacture"),
            fact("parameter","pkg.factory.Factory.manufacture.self",20,20,"self"),
            fact("return_annotation","pkg.factory.Factory.manufacture",20,20,return_type),
        ].into_iter().map(|mut f| {f.path="pkg/factory.py".into();f.id=format!("factory_{}",f.id);f}));
        e
    }

    #[test]
    fn single_sdk_factory_return_has_full_cited_candidate_proof() {
        let e = factory_source("Benchling");
        let d = draw(&e, View::Network, 2, 200);
        assert!(d.mermaid.contains("containers.checkin"));
        let boundary = d.legend["boundaries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|b| b["classification"] == "Benchling SDK boundary candidate")
            .unwrap();
        assert!(boundary["provenanceBasis"]
            .as_str()
            .unwrap()
            .contains("annotated factory-return candidate"));
        let ids: BTreeSet<_> = boundary["sourceRefs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["factId"].as_str().unwrap())
            .collect();
        for required in [
            "call52",
            "binding51",
            "binding34",
            "function31",
            "parameter32",
            "parameter_annotation32",
            "from_import21",
            "factory_class10",
            "factory_function20",
            "factory_return_annotation20",
            "factory_from_import2",
        ] {
            assert!(ids.contains(required), "missing factory proof {required}");
        }
    }

    #[test]
    fn factory_union_forward_dynamic_and_missing_returns_remain_unknown() {
        let e = union_source("Benchling BitOr DynamicClientWrapper");
        let d = draw(&e, View::Network, 2, 200);
        assert_eq!(d.legend["counts"]["networkGroups"], 1);
        assert!(d.mermaid.contains("runtime selection UNKNOWN"));
        for return_type in [
            "Benchling | DynamicClientWrapper",
            "Union[[Benchling, DynamicClientWrapper]]",
            "<literal>",
            "Optional[Benchling]",
            "Any",
            "DynamicClientWrapper",
        ] {
            let e = factory_source(return_type);
            let d = draw(&e, View::Network, 2, 200);
            assert_eq!(
                d.legend["counts"]["networkGroups"], 0,
                "unsafe return annotation {return_type}"
            );
            assert_eq!(
                d.legend["boundaries"][0]["classification"],
                "UNKNOWN unresolved destination"
            );
        }
        let mut e = factory_source("Benchling");
        e.facts.retain(|f| f.kind != "return_annotation");
        assert_eq!(
            draw(&e, View::Network, 2, 200).legend["counts"]["networkGroups"],
            0
        );
        let mut e = factory_source("Benchling");
        e.facts
            .iter_mut()
            .find(|f| f.id == "parameter_annotation32")
            .unwrap()
            .value = "Factory BitOr Other".into();
        assert_eq!(
            draw(&e, View::Network, 2, 200).legend["counts"]["networkGroups"],
            0
        );
    }

    fn union_source(annotation: &str) -> Evidence {
        let mut e = factory_source(annotation);
        e.facts.extend([
            fact("from_import", "pkg.factory", 3, 3, r#"{"module":"pkg.wrapper","level":0,"names":[{"name":"DynamicClientWrapper","asname":null}]}"#),
            fact("from_import", "pkg.factory", 4, 4, r#"{"module":"benchling_api_client","level":0,"names":[{"name":"Benchling","asname":"OtherSDK"}]}"#),
        ].into_iter().map(|mut f| { f.path = "pkg/factory.py".into(); f.id = format!("union_{}", f.id); f }));
        e.facts.extend(
            [
                fact("module", "pkg.wrapper", 1, 30, "pkg.wrapper"),
                fact(
                    "class",
                    "pkg.wrapper.DynamicClientWrapper",
                    2,
                    20,
                    "DynamicClientWrapper",
                ),
                fact(
                    "function",
                    "pkg.wrapper.DynamicClientWrapper.__getattr__",
                    3,
                    10,
                    "__getattr__",
                ),
                fact(
                    "return_annotation",
                    "pkg.wrapper.DynamicClientWrapper.__getattr__",
                    3,
                    3,
                    "Any",
                ),
            ]
            .into_iter()
            .map(|mut f| {
                f.path = "pkg/wrapper.py".into();
                f.id = format!("wrapper_{}", f.id);
                f
            }),
        );
        e
    }

    #[test]
    fn initializer_shape_is_complete_balanced_and_quote_aware() {
        for source in [
            "factory.manufacture(select([env], mode=<literal>), config={key: value})",
            r#"SDK(select(')[{', escaped="\"("))"#,
        ] {
            assert!(direct_initializer(source).is_some(), "{source}");
        }
        for source in [
            "factory.manufacture(env).as_local_adapter()",
            "factory.manufacture(env)[<literal>]",
            "factory.manufacture(env).helper",
            "factory.manufacture(env)()",
            "SDK().helper()",
            "SDK()[<literal>]",
            "SDK()()",
            "SDK(...)",
            "SDK(select(env)",
            "SDK([env))",
            "SDK('unterminated)",
        ] {
            assert!(direct_initializer(source).is_none(), "{source}");
        }
        let capped = format!("SDK({})", "a".repeat(507));
        assert_eq!(capped.len(), 512);
        assert!(direct_initializer(&capped).is_none());
    }

    #[test]
    fn complete_retained_input_counts_routes_files_envelope_and_rejected_proof() {
        let mut e = union_source("Benchling BitOr DynamicClientWrapper");
        e.files = ["app.py", "pkg/factory.py", "pkg/wrapper.py"]
            .into_iter()
            .map(|path| crate::analysis::SourceFile {
                path: path.into(),
                sha256: "a".repeat(64),
            })
            .collect();
        for n in 0..500 {
            let mut detail = fact("return", "app.service", 60, 60, &"x".repeat(480));
            detail.id = format!("detail_{n:03}");
            e.facts.push(detail);
        }
        let d = draw(&e, View::Network, 4, 600);
        assert_eq!(d.legend["counts"]["networkGroups"], 1);
        let boundary = &d.legend["boundaries"][0];
        assert_eq!(boundary["branches"].as_array().unwrap().len(), 2);
        assert_eq!(boundary["resolutionTrace"]["branchCoverage"], "complete");
        let c = crate::catalog::build(&e, "local").unwrap();
        let item = c.items.iter().find(|i| i.kind == Kind::Api).unwrap();
        let cap = d.legend["limits"]["focusMaxBytes"].as_u64().unwrap() as usize;
        assert_eq!(
            cap + d.legend["limits"]["reservedBytes"].as_u64().unwrap() as usize,
            crate::packet::MAX_INPUT_BYTES
        );
        assert_eq!(d.legend["coverage"]["maxBytes"], cap);
        assert_eq!(d.legend["limits"]["provenanceReserveBytes"], 16 * 1024);
        let prepared = focus::prepare_with_byte_limit(
            &e,
            &FocusOptions {
                symbol: item.fact_id.clone(),
                hops: 4,
                max_facts: 599,
            },
            cap,
        )
        .unwrap();
        assert!(prepared.coverage.input_bytes <= cap);
        assert!(prepared.coverage.input_bytes > cap - 1024);
        assert!(prepared.input["focus"]["boundaries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b.as_str().unwrap().starts_with("byte_limit:")));
        let mut facts: Vec<_> = e.facts.iter().collect();
        facts.sort_by_key(|f| (&f.path, &f.symbol, f.line, f.end_line, &f.kind, &f.id));
        let retained: BTreeSet<String> = d.legend["sourceRefs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["factId"].as_str().unwrap().into())
            .collect();
        let input = retained_input(&prepared.input, &e, &facts, &retained);
        let bytes = serde_json::to_vec(&input).unwrap().len();
        assert!(bytes <= crate::packet::MAX_INPUT_BYTES);
        assert_eq!(d.legend["limits"]["retainedInputBytes"], bytes);
        assert!(item.evidence_ids.iter().all(|id| retained.contains(id)));
        assert!(retained.contains("union_from_import3"));
        assert!(retained.contains("factory_return_annotation20"));
        assert_eq!(input["files"].as_array().unwrap().len(), 3);
        assert_eq!(input["focus"], prepared.input["focus"]);
        eprintln!("endpoint headroom regression: focus cap={cap}, selected={} bytes, retained={bytes}, max={}", prepared.coverage.input_bytes, crate::packet::MAX_INPUT_BYTES);

        // The reservation is headroom, not permission to publish oversized proof.
        let import = e
            .facts
            .iter_mut()
            .find(|f| f.id == "union_from_import3")
            .unwrap();
        let original_import = import.value.clone();
        let mut oversized: Value = serde_json::from_str(&import.value).unwrap();
        oversized["padding"] = json!("x".repeat(crate::packet::MAX_INPUT_BYTES));
        import.value = oversized.to_string();
        let rejected = draw(&e, View::Network, 4, 600);
        assert_eq!(rejected.legend["counts"]["networkGroups"], 0);
        assert!(
            rejected.legend["boundaries"][0]["resolutionTrace"]["stoppingReason"]
                .as_str()
                .unwrap()
                .contains("max-bytes budget boundary")
        );
        assert!(!rejected.legend["sourceRefs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["factId"] == "union_from_import3"));
        assert_eq!(rejected.legend["boundaries"][0]["branches"], json!([]));
        e.facts
            .iter_mut()
            .find(|f| f.id == "union_from_import3")
            .unwrap()
            .value = original_import;

        // Focus drops the oversized route attachment; endpoint must not publish
        // candidates after restoring it, even though the focused packet fits.
        let mut value: Value = serde_json::from_str(
            &e.facts
                .iter()
                .find(|f| f.kind == "api_route")
                .unwrap()
                .value,
        )
        .unwrap();
        value["padding"] = json!("x".repeat(crate::packet::MAX_INPUT_BYTES));
        e.facts
            .iter_mut()
            .find(|f| f.kind == "api_route")
            .unwrap()
            .value = value.to_string();
        let c = crate::catalog::build(&e, "local").unwrap();
        let item = c.items.iter().find(|i| i.kind == Kind::Api).unwrap();
        let prepared = focus::prepare(
            &e,
            &FocusOptions {
                symbol: item.fact_id.clone(),
                hops: 2,
                max_facts: 199,
            },
        )
        .unwrap();
        assert!(prepared.coverage.input_bytes <= crate::packet::MAX_INPUT_BYTES);
        assert!(!prepared.input["facts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["kind"] == "api_route"));
        assert!(prepared.input["facts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["id"] == "call52"));
        let mut restored: BTreeSet<String> = prepared.input["facts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["id"].as_str().unwrap().into())
            .collect();
        restored.extend(item.evidence_ids.iter().cloned());
        let facts: Vec<_> = e.facts.iter().collect();
        assert!(
            serde_json::to_vec(&retained_input(&prepared.input, &e, &facts, &restored))
                .unwrap()
                .len()
                > crate::packet::MAX_INPUT_BYTES
        );
        let Err(error) = build(&c, &e, item, &View::Network, None, 2, 200) else {
            panic!("oversized restored route published")
        };
        assert!(error.contains("restored route citations"));
        assert!(error.contains("max-bytes budget boundary"));
    }

    #[test]
    fn factory_union_branches_are_independent_cited_and_order_invariant() {
        let e = union_source("Benchling BitOr DynamicClientWrapper");
        let d = draw(&e, View::Network, 2, 200);
        let b = &d.legend["boundaries"][0];
        assert_eq!(b["branches"].as_array().unwrap().len(), 2);
        assert_eq!(
            b["branches"][0]["qualifiedName"],
            "benchling_sdk.benchling.Benchling"
        );
        assert_eq!(b["branches"][1]["classification"], "UNKNOWN");
        assert_eq!(b["resolutionTrace"]["branchCoverage"], "complete");
        for branch in b["branches"].as_array().unwrap() {
            let ids: BTreeSet<_> = branch["sourceRefs"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r["factId"].as_str().unwrap())
                .collect();
            for id in [
                "call52",
                "binding51",
                "binding34",
                "parameter_annotation32",
                "factory_return_annotation20",
                "factory_function20",
                "factory_from_import2",
                "union_from_import3",
                "wrapper_class2",
            ] {
                assert!(ids.contains(id), "missing {id}");
            }
            assert!(
                !ids.contains("wrapper_return_annotation3"),
                "must not unwrap dynamic wrapper"
            );
        }
        let reverse = draw(
            &union_source("DynamicClientWrapper BitOr Benchling"),
            View::Network,
            2,
            200,
        );
        assert_eq!(b["branches"], reverse.legend["boundaries"][0]["branches"]);
        let mut reordered = union_source("Benchling BitOr DynamicClientWrapper");
        reordered.facts.reverse();
        assert_eq!(d.legend, draw(&reordered, View::Network, 2, 200).legend);
        let two = draw(
            &union_source("OtherSDK BitOr Benchling"),
            View::Network,
            2,
            200,
        );
        assert_eq!(two.legend["counts"]["networkGroups"], 2);
        assert_eq!(
            two.legend["boundaries"][0]["branches"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        for b in two.legend["boundaries"][0]["branches"].as_array().unwrap() {
            assert!(b["classification"]
                .as_str()
                .unwrap()
                .contains("SDK return branch"));
        }
    }

    #[test]
    fn factory_union_unsupported_members_and_budget_fail_closed() {
        for annotation in [
            "Benchling BitOr DynamicClientWrapper BitOr Missing",
            "Benchling BitOr Any",
            "Benchling BitOr Optional[OtherSDK]",
            "Benchling BitOr list[OtherSDK]",
            "Benchling BitOr <literal>",
            "Benchling BitOr OtherSDK BitOr DynamicClientWrapper BitOr Missing BitOr Fifth",
        ] {
            let d = draw(&union_source(annotation), View::Network, 2, 200);
            assert_eq!(d.legend["counts"]["networkGroups"], 0, "{annotation}");
            assert_eq!(
                d.legend["boundaries"][0]["resolutionTrace"]["branchCoverage"],
                "incomplete"
            );
            assert!(d.legend["boundaries"][0]["resolutionTrace"]["stoppingReason"].is_string());
        }
        let mut saw_budget = false;
        for max in 15..40 {
            let e = union_source("Benchling BitOr DynamicClientWrapper");
            let c = crate::catalog::build(&e, "local").unwrap();
            let item = c.items.iter().find(|i| i.kind == Kind::Api).unwrap();
            let Ok(d) = build(&c, &e, item, &View::Network, None, 2, max) else {
                continue;
            };
            assert!(d.legend["retainedFactCount"].as_u64().unwrap() <= max as u64);
            if d.legend["boundaries"].as_array().unwrap().iter().any(|b| {
                b["resolutionTrace"]["stoppingReason"]
                    .as_str()
                    .is_some_and(|s| s.contains("budget boundary"))
            }) {
                saw_budget = true;
                assert_eq!(d.legend["counts"]["networkGroups"], 0);
                assert!(!d.mermaid.contains("SDK return branch candidate"));
            }
        }
        assert!(saw_budget);
        let mut e = union_source("Benchling BitOr DynamicClientWrapper");
        let import = e
            .facts
            .iter_mut()
            .find(|f| f.id == "union_from_import3")
            .unwrap();
        let mut value: Value = serde_json::from_str(&import.value).unwrap();
        value["padding"] = json!("x".repeat(crate::packet::MAX_INPUT_BYTES));
        import.value = value.to_string();
        let d = draw(&e, View::Network, 2, 200);
        assert_eq!(d.legend["counts"]["networkGroups"], 0);
        assert!(
            d.legend["boundaries"][0]["resolutionTrace"]["stoppingReason"]
                .as_str()
                .unwrap()
                .contains("budget boundary")
        );
        assert_eq!(d.legend["boundaries"][0]["branches"], json!([]));
    }

    #[test]
    fn factory_union_cap_and_network_group_basis_are_explicit() {
        for annotation in [
            "Benchling BitOr OtherSDK BitOr DynamicClientWrapper",
            "Benchling BitOr OtherSDK BitOr DynamicClientWrapper BitOr ThirdSDK",
        ] {
            let mut e = union_source(annotation);
            let mut import = fact(
                "from_import",
                "pkg.factory",
                5,
                5,
                r#"{"module":"httpx","level":0,"names":[{"name":"Client","asname":"ThirdSDK"}]}"#,
            );
            import.id = "third_sdk_import".into();
            import.path = "pkg/factory.py".into();
            e.facts.push(import);
            let d = draw(&e, View::Network, 2, 200);
            assert_eq!(d.legend["counts"]["networkGroups"], 2);
            assert_eq!(
                d.legend["boundaries"][0]["branches"]
                    .as_array()
                    .unwrap()
                    .len(),
                annotation.split(" BitOr ").count()
            );
        }
        let mut e = union_source("Benchling BitOr DynamicClientWrapper");
        e.facts.push(fact("from_import", "app", 22, 22, r#"{"module":"benchling_sdk.benchling","level":0,"names":[{"name":"Benchling","asname":null}]}"#));
        e.facts.push(fact(
            "binding",
            "app.service",
            53,
            53,
            r#"{"target":"sdk","source":"Benchling()"}"#,
        ));
        e.facts.push(fact(
            "call",
            "app.service",
            54,
            54,
            "sdk.containers.checkin()",
        ));
        let d = draw(&e, View::Network, 2, 200);
        assert_eq!(d.legend["counts"]["networkGroups"], 2);
        let network_edges: Vec<_> = d.legend["edges"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|edge| edge["from"] == "A")
            .collect();
        assert_eq!(network_edges.len(), 2);
        assert_ne!(network_edges[0]["basis"], network_edges[1]["basis"]);
        let s = draw(&e, View::Services, 2, 200);
        assert!(s.legend["edges"]
            .as_array()
            .unwrap()
            .iter()
            .any(|edge| edge["basis"]
                == "declared SDK return branch candidate; runtime selection UNKNOWN"));
    }

    #[test]
    fn factory_union_rebindings_ambiguity_and_shadowing_still_block_edges() {
        for case in ["class", "method", "field", "shadow", "impersonation"] {
            let mut e = union_source("Benchling BitOr DynamicClientWrapper");
            let mut f = match case {
                "class" => fact("class", "pkg.factory.Factory", 61, 80, "Factory"),
                "method" => fact(
                    "function",
                    "pkg.factory.Factory.manufacture",
                    41,
                    49,
                    "manufacture",
                ),
                "field" => fact(
                    "binding",
                    "app.service",
                    51,
                    51,
                    r#"{"target":"ctx.benchling","source":"other"}"#,
                ),
                "shadow" => fact(
                    "binding",
                    "pkg.factory",
                    5,
                    5,
                    r#"{"target":"Benchling","source":"other"}"#,
                ),
                _ => fact("module", "benchling_sdk", 1, 10, "benchling_sdk"),
            };
            f.id = format!("bad_{case}");
            if matches!(case, "class" | "method" | "shadow") {
                f.path = "pkg/factory.py".into();
            }
            if case == "impersonation" {
                f.path = "benchling_sdk/__init__.py".into();
            }
            e.facts.push(f);
            assert_eq!(
                draw(&e, View::Network, 2, 200).legend["counts"]["networkGroups"],
                0,
                "{case}"
            );
        }
    }

    #[test]
    fn factory_method_uniqueness_decorators_rebindings_and_helpers_fail_closed() {
        for case in [
            "duplicate",
            "decorated",
            "dynamic",
            "receiver_rebound",
            "method_rebound",
            "type_shadowed",
            "auth_return",
        ] {
            let mut e = factory_source("Benchling");
            let mut add = match case {
                "duplicate" => fact(
                    "function",
                    "pkg.factory.Factory.manufacture",
                    41,
                    49,
                    "manufacture",
                ),
                "decorated" => fact(
                    "decorator",
                    "pkg.factory.Factory.manufacture",
                    19,
                    19,
                    "unknown_decorator",
                ),
                "dynamic" => fact(
                    "function",
                    "pkg.factory.Factory.__getattr__",
                    45,
                    49,
                    "__getattr__",
                ),
                "receiver_rebound" => fact(
                    "binding_invalidation",
                    "app.Context.__init__",
                    33,
                    33,
                    r#"{"target":"factory"}"#,
                ),
                "method_rebound" => fact(
                    "binding_invalidation",
                    "pkg.factory.Factory",
                    11,
                    11,
                    r#"{"target":"manufacture"}"#,
                ),
                "type_shadowed" => fact(
                    "binding_invalidation",
                    "pkg.factory.Factory",
                    12,
                    12,
                    r#"{"target":"Benchling"}"#,
                ),
                "auth_return" => {
                    e.facts
                        .iter_mut()
                        .find(|f| f.kind == "return_annotation")
                        .unwrap()
                        .value = "ClientCredentialsOAuth2".into();
                    fact(
                        "from_import",
                        "pkg.factory",
                        3,
                        3,
                        r#"{"module":"benchling_sdk.auth.client_credentials_oauth2","level":0,"names":[{"name":"ClientCredentialsOAuth2","asname":null}]}"#,
                    )
                }
                _ => unreachable!(),
            };
            if case != "receiver_rebound" {
                add.path = "pkg/factory.py".into();
                add.id = format!("extra_{}", add.id);
            }
            e.facts.push(add);
            assert_eq!(
                draw(&e, View::Network, 2, 200).legend["counts"]["networkGroups"],
                0,
                "unsafe factory case {case}"
            );
        }
        let mut e = factory_source("Benchling");
        e.facts
            .iter_mut()
            .find(|f| f.id == "factory_function20")
            .unwrap()
            .kind = "async_function".into();
        assert_eq!(
            draw(&e, View::Network, 2, 200).legend["counts"]["networkGroups"],
            0
        );
    }

    #[test]
    fn unresolved_factory_operation_is_prioritized_over_repeated_unresolved_calls() {
        let mut e = factory_source("Benchling BitOr DynamicClientWrapper");
        let service = e.facts.iter_mut().find(|f| f.id == "function50").unwrap();
        service.end_line = 350;
        let call = e.facts.iter_mut().find(|f| f.id == "call52").unwrap();
        call.line = 300;
        call.end_line = 300;
        for line in 100..150 {
            e.facts
                .push(fact("call", "app.service", line, line, "unknown.work()"));
        }
        let d = draw(&e, View::Services, 2, 300);
        assert!(d.legend["counts"]["omittedCallSites"].as_u64().unwrap() > 0);
        assert!(d.mermaid.contains("ctx.benchling.containers.checkin"));
        let readable = d.html.split("<h2>Readable Diagram Source").next().unwrap();
        assert!(readable.contains("ctx.benchling.containers.checkin</code>: UNKNOWN destination"));
        assert!(readable.contains("app.py:300"));
        assert_eq!(d.html.matches("<script>").count(), 1);
        assert!(d.html.contains("script-src 'sha256-"));
    }

    #[test]
    fn source_only_call_sites_targets_network_provenance_and_unknown() {
        let e = source();
        let s = draw(&e, View::Services, 2, 100);
        assert!(s.mermaid.contains("observed call site"));
        assert!(s.mermaid.contains("static target candidate"));
        assert!(s.mermaid.contains("app.service"));
        assert_eq!(s.legend["contractMatch"]["status"], "source-only");
        let n = draw(&e, View::Network, 2, 100);
        let b = n.legend["boundaries"].as_array().unwrap();
        assert_eq!(
            b.iter()
                .filter(|b| b["classification"] == "HTTP client/import boundary candidate")
                .count(),
            1
        );
        assert_eq!(
            b.iter()
                .filter(|b| b["classification"] == "UNKNOWN unresolved destination")
                .count(),
            1
        );
        assert!(b
            .iter()
            .find(|b| b["classification"] == "HTTP client/import boundary candidate")
            .unwrap()["sourceRefs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["kind"] == "import"));
        assert_eq!(n.html.matches("<script>").count(), 1);
        assert!(n.html.contains("data-viewer-viewport"));
        assert!(!n.html.contains("https://cdn"));
        assert!(!n.mermaid.contains("Not classified as network"));
        assert_eq!(n.legend["counts"]["unclassifiedCalls"], 1);
        let limited = draw(&e, View::Services, 0, 100);
        assert!(limited.legend["focus"]["truncated"].as_bool().unwrap());
        assert!(!limited.mermaid.contains("static target candidate"));
    }

    #[test]
    fn ambiguous_contract_and_inert_labels() {
        let mut e = source();
        e.facts
            .iter_mut()
            .find(|f| f.kind == "api_route")
            .unwrap()
            .value =
            json!({"methods":["POST"],"path":"/checkin","pathIsFragment":true}).to_string();
        let c = crate::catalog::build(&e, "<script>root</script>").unwrap();
        let i = c.items.iter().find(|i| i.kind == Kind::Api).unwrap();
        let op = |path: &str| crate::openapi::Operation {
            method: "POST".into(),
            path: path.into(),
            pointer: "/x".into(),
            operation_id: Some("checkin".into()),
            summary: Some("</pre><script>alert(1)</script>".into()),
            tags: vec![],
            parameters: vec![],
            request: json!({"required":true,"content":{"application/json":{"type":"object","properties":{"containerId":{"type":"string"}}}}}),
            responses: json!({"202":{"content":{"application/json":{"$ref":"#/components/schemas/CheckinResult","status":"local reference; not expanded"}}}}),
            security: json!([{"token":["write"]}]),
            security_basis: "declared".into(),
            servers: json!([{"url":"https://server.example/<script>"}]),
            server_basis: "unknown".into(),
            boundaries: vec![],
        };
        let mut spec = crate::openapi::Document {
            path: "<script>spec</script>".into(),
            sha256: "0".repeat(64),
            version: "3.1.0".into(),
            operations: vec![op("/v1/checkin"), op("/v2/checkin")],
            security_schemes: json!({}),
            boundaries: vec![],
        };
        let d = build(&c, &e, i, &View::Network, Some(&spec), 2, 100).unwrap();
        assert_eq!(d.legend["contractMatch"]["status"], "ambiguous");
        assert_eq!(d.html.matches("<script>").count(), 1);
        assert!(d.html.contains("script-src 'sha256-"));
        assert!(d.html.contains("&lt;script&gt;"));
        spec.operations.pop();
        let d = build(&c, &e, i, &View::Network, Some(&spec), 2, 100).unwrap();
        assert_eq!(d.legend["contractMatch"]["status"], "candidate");
        let readable = d.html.split("<h2>Readable Diagram Source").next().unwrap();
        for text in [
            "<h4>Request</h4>",
            "containerId: string",
            "Declared status 202",
            "#/components/schemas/CheckinResult",
            "token [write]",
            "Servers (Declared, Not Contacted)",
            "https://server.example/&lt;script&gt;",
        ] {
            assert!(
                readable.contains(text),
                "missing readable contract metadata {text}"
            );
        }
        for link in d.html.split("href=\"").skip(1) {
            assert_eq!(link.split('"').next().unwrap(), "diagram.svg");
        }
        assert!(!d.mermaid.contains("server.example"));
        assert!(d.legend["contractMatch"]["basis"]
            .as_str()
            .unwrap()
            .contains("suffix"));
        spec.operations[0].path = "/checkin".into();
        assert_eq!(
            crate::openapi::match_item(&spec, i, true)["basis"],
            "method and suffix fragment candidate; prefix/activation unverified"
        );
        spec.operations[0].path = "/other".into();
        assert_eq!(
            crate::openapi::match_item(&spec, i, false)["status"],
            "unmatched"
        );
        assert_eq!(
            crate::openapi::match_item(&spec, i, true)["status"],
            "candidate"
        );
    }

    #[test]
    fn import_rebinding_is_unknown_and_order_is_deterministic() {
        let mut e = source();
        let a = draw(&e, View::Network, 2, 100);
        e.facts.reverse();
        let b = draw(&e, View::Network, 2, 100);
        assert_eq!(a.mermaid, b.mermaid);
        assert_eq!(a.legend, b.legend);
        e.facts.push(fact(
            "binding_invalidation",
            "app.service",
            25,
            25,
            r#"{"target":"r"}"#,
        ));
        let d = draw(&e, View::Network, 2, 100);
        assert!(!d.mermaid.contains("HTTP client/import boundary candidate"));
    }

    #[test]
    fn annotated_parameter_provenance_rejects_competing_imports_and_declarations() {
        let mut base = source();
        base.facts.retain(|f| f.line < 25 && f.kind != "call");
        base.facts.extend([
            fact(
                "from_import",
                "app",
                2,
                2,
                r#"{"module":"httpx","level":0,"names":[{"name":"Client","asname":null}]}"#,
            ),
            fact("parameter", "app.checkin.client", 4, 4, "client"),
            fact("parameter_annotation", "app.checkin.client", 4, 4, "Client"),
            fact("call", "app.checkin", 10, 10, "client.get()"),
        ]);
        assert_eq!(
            draw(&base, View::Network, 2, 100).legend["counts"]["networkGroups"],
            1
        );
        for shadow in [
            fact(
                "from_import",
                "app.checkin",
                6,
                6,
                r#"{"module":"local_stub","level":0,"names":[{"name":"client","asname":null}]}"#,
            ),
            fact(
                "import",
                "app.checkin",
                6,
                6,
                r#"{"module":"","level":0,"names":[{"name":"local_stub","asname":"client"}]}"#,
            ),
            fact("function", "app.checkin.client", 6, 8, "client"),
            fact("class", "app.checkin.client", 6, 8, "client"),
        ] {
            let mut e: Evidence =
                serde_json::from_value(serde_json::to_value(&base).unwrap()).unwrap();
            e.facts.push(shadow);
            let d = draw(&e, View::Network, 2, 100);
            assert_eq!(d.legend["counts"]["networkGroups"], 0, "{}", d.legend);
            assert!(!d.mermaid.contains("HTTP client/import boundary candidate"));
        }
        base.facts.push(fact(
            "from_import",
            "app.checkin",
            6,
            6,
            r#"{"module":"local_stub","level":0,"names":[{"name":"client","asname":"other"}]}"#,
        ));
        assert_eq!(
            draw(&base, View::Network, 2, 100).legend["counts"]["networkGroups"],
            1
        );
    }

    #[test]
    fn typed_context_benchling_boundary_requires_import_and_field_evidence() {
        let mut e = source();
        e.facts.retain(|f| f.line < 25);
        e.facts.extend([
            fact("from_import","app",21,21,r#"{"module":"benchling_sdk.benchling","level":0,"names":[{"name":"Benchling","asname":null}]}"#),
            fact("class","app.Context",30,45,"Context"),
            fact("function","app.Context.__init__",31,44,"__init__"),
            fact("parameter","app.Context.__init__.self",31,31,"self"),
            fact("parameter_annotation","app.Context.__init__.benchling",31,31,"Benchling"),
            fact("binding","app.Context.__init__",32,32,r#"{"target":"self.benchling","source":"benchling"}"#),
            fact("function","app.service",50,90,"service"),
            fact("binding","app.service",51,51,r#"{"target":"ctx","source":"Context(benchling=client)"}"#),
            fact("call","app.service",52,52,"ctx.benchling.custom_entities.create(<literal>)"),
        ]);
        let d = draw(&e, View::Network, 2, 100);
        let sdk = d.legend["boundaries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|b| b["classification"] == "Benchling SDK boundary candidate");
        assert!(sdk.is_some(), "{}", d.legend);
        let refs = sdk.unwrap()["sourceRefs"].as_array().unwrap();
        assert!(refs.iter().any(|r| r["kind"] == "parameter_annotation"));
        assert!(refs.iter().any(|r| r["kind"] == "from_import"));
        e.facts.retain(|f| f.kind != "from_import");
        assert!(!draw(&e, View::Network, 2, 100)
            .mermaid
            .contains("Benchling SDK boundary candidate"));
    }

    #[test]
    fn fact_limits_and_forged_catalog_attachment_fail_closed() {
        let e = source();
        let mut c = crate::catalog::build(&e, "local").unwrap();
        let i = c
            .items
            .iter()
            .find(|i| i.kind == Kind::Api)
            .unwrap()
            .clone();
        for limit in [4, 8, 16] {
            let d = build(&c, &e, &i, &View::Services, None, 2, limit).unwrap();
            assert!(d.legend["retainedFactCount"].as_u64().unwrap() <= limit as u64);
        }
        assert!(build(&c, &e, &i, &View::Services, None, 6, 100).is_err());
        assert!(build(&c, &e, &i, &View::Services, None, 2, 0).is_err());
        let entry = c.items.iter_mut().find(|entry| entry.id == i.id).unwrap();
        entry.route_path = Some("/forged".into());
        let forged = entry.clone();
        assert!(build(&c, &e, &forged, &View::Services, None, 2, 100).is_err());
    }

    #[test]
    fn network_groups_operations_not_constructors_or_helpers() {
        let mut e = source();
        e.facts
            .iter_mut()
            .find(|f| f.symbol == "app.service" && callable(f))
            .unwrap()
            .end_line = 200;
        e.facts.extend([
            fact(
                "import",
                "app",
                21,
                21,
                r#"{"module":"","level":0,"names":[{"name":"httpx","asname":"hx"}]}"#,
            ),
            fact(
                "import",
                "app",
                22,
                22,
                r#"{"module":"","level":0,"names":[{"name":"benchling_sdk","asname":"b"}]}"#,
            ),
            fact("call", "app.service", 28, 28, "hx.AsyncClient()"),
            fact(
                "binding",
                "app.service",
                29,
                29,
                r#"{"target":"client","source":"hx.AsyncClient()"}"#,
            ),
            fact("call", "app.service", 30, 30, "client.get(<literal>)"),
            fact("call", "app.service", 31, 31, "b.Benchling()"),
            fact(
                "binding",
                "app.service",
                32,
                32,
                r#"{"target":"sdk","source":"b.Benchling()"}"#,
            ),
            fact(
                "call",
                "app.service",
                33,
                33,
                "sdk.containers.checkin(<literal>)",
            ),
            fact(
                "call",
                "app.service",
                34,
                34,
                "sdk.containers.checkin(<literal>)",
            ),
            fact(
                "call",
                "app.service",
                35,
                35,
                "sdk.auth.client_credentials(<literal>)",
            ),
            fact(
                "call",
                "app.service",
                36,
                36,
                "sdk.models.ContainerCreate(<literal>)",
            ),
            fact("call", "app.service", 37, 37, "hx.BasicAuth(<literal>)"),
            fact(
                "call",
                "app.service",
                38,
                38,
                "client.headers.get(<literal>)",
            ),
            fact("call", "app.service", 39, 39, "client.request(<literal>)"),
        ]);
        let d = draw(&e, View::Network, 2, 200);
        let nodes = d.legend["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 7); // inbound, handler, app, scope note, three clients
        let sdk = nodes
            .iter()
            .find(|n| n["client"] == "benchling_sdk.Benchling")
            .unwrap();
        assert_eq!(sdk["operations"], json!(["containers.checkin"]));
        assert_eq!(sdk["callFactIds"], json!(["call33", "call34"]));
        let http = nodes
            .iter()
            .find(|n| n["client"] == "httpx.AsyncClient")
            .unwrap();
        assert_eq!(http["operations"], json!(["get", "request"]));
        assert!(d.mermaid.contains("containers.checkin"));
        for helper in [
            "BasicAuth",
            "ContainerCreate",
            "client_credentials",
            "headers.get",
            "Not classified as network",
        ] {
            assert!(!d.mermaid.contains(helper));
        }
        assert_eq!(d.legend["counts"]["unclassifiedCalls"], 7);
        assert_eq!(d.legend["calls"].as_array().unwrap().len(), 13);
    }

    #[test]
    fn service_graph_caps_do_not_discard_selected_call_records() {
        let mut e = source();
        e.facts
            .iter_mut()
            .find(|f| f.symbol == "app.service" && callable(f))
            .unwrap()
            .end_line = 200;
        for line in 50..150 {
            e.facts.push(fact(
                "call",
                "app.service",
                line,
                line,
                "unknown.domain(<literal>)",
            ));
        }
        for index in 0..50 {
            e.facts.push(fact(
                "call",
                "app.service",
                150 + index,
                150 + index,
                &format!("helper{index}()"),
            ));
            e.facts.push(fact(
                "function",
                &format!("app.helper{index}"),
                300 + index,
                300 + index,
                "helper",
            ));
        }
        let d = draw(&e, View::Services, 2, 1000);
        let calls = d.legend["calls"].as_array().unwrap();
        let nodes = d.legend["nodes"].as_array().unwrap();
        assert!(calls.len() > MAX_CALL_NODES);
        assert!(nodes.len() <= 3 + MAX_CALLABLE_NODES + 2 * MAX_CALL_NODES);
        assert_eq!(
            nodes
                .iter()
                .filter(|n| n["kind"] == "observed call site")
                .count(),
            MAX_CALL_NODES
        );
        assert_eq!(
            d.legend["counts"]["omittedCallSites"].as_u64().unwrap() as usize,
            calls.len() - MAX_CALL_NODES
        );
        assert!(
            nodes
                .iter()
                .filter(|n| n["kind"] == "callable/service collaborator candidate")
                .count()
                < MAX_CALLABLE_NODES
        );
        let n = draw(&e, View::Network, 2, 1000);
        assert_eq!(n.legend["nodes"].as_array().unwrap().len(), 5);
        assert_eq!(n.legend["counts"]["unclassifiedCalls"], 101);
        assert!(!n.mermaid.contains("unknown.domain"));
        assert!(nodes.iter().any(|n| n["kind"] == "omitted target boundary"));
        let ids: BTreeSet<_> = nodes.iter().map(|n| n["id"].as_str().unwrap()).collect();
        let edges = d.legend["edges"].as_array().unwrap();
        for node in nodes.iter().filter(|n| n["kind"] == "observed call site") {
            assert!(
                edges
                    .iter()
                    .any(|e| e["from"] == node["id"] && ids.contains(e["to"].as_str().unwrap())),
                "dangling observed call {node}"
            );
        }
    }

    #[test]
    fn empty_network_diagram_has_inert_scope_counts_and_contract_absence() {
        let e = factory_source("Benchling BitOr DynamicClientWrapper");
        let d = draw(&e, View::Network, 3, 200);
        assert!(d.mermaid.contains("No proven network candidates"));
        assert!(d.mermaid.contains("selected calls; 1 unclassified"));
        assert!(d
            .mermaid
            .contains("Source-only: no local contract supplied"));
        let info = d.legend["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["kind"] == "scope summary")
            .unwrap();
        assert_eq!(info["networkGroups"], 0);
        assert_eq!(info["unclassifiedCalls"], 1);
        assert_eq!(info["contractStatus"], "source-only");
        assert!(d.legend["edges"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["from"] != info["id"] && e["to"] != info["id"]));
        assert!(!d.mermaid.contains("SDK boundary candidate"));
    }

    #[test]
    fn handler_rooted_topology_and_sdk_expression_survive_alphabetic_builtin_noise() {
        let mut e = source();
        e.facts.clear();
        let mut add =
            |path: &str, kind: &str, symbol: &str, line: usize, end: usize, value: &str| {
                let mut f = fact(kind, symbol, line, end, value);
                f.path = path.into();
                f.id = format!("{path}:{kind}:{symbol}:{line}");
                e.facts.push(f);
            };
        add("z_routes.py", "module", "pkg.routes", 1, 500, "");
        add(
            "z_routes.py",
            "from_import",
            "pkg.routes",
            2,
            2,
            r#"{"module":"pkg.containers","level":0,"names":[{"name":"ContainerService","asname":null}]}"#,
        );
        add(
            "z_routes.py",
            "api_route",
            "pkg.routes.handler",
            4,
            4,
            r#"{"methods":["POST"],"path":"/containers/checkin","pathIsFragment":true}"#,
        );
        add("z_routes.py", "function", "pkg.routes.handler", 5, 20, "");
        add(
            "z_routes.py",
            "binding",
            "pkg.routes.handler",
            6,
            6,
            r#"{"target":"service","source":"ContainerService()"}"#,
        );
        add(
            "z_routes.py",
            "call",
            "pkg.routes.handler",
            7,
            7,
            "service.checkin()",
        );
        for (path, module, class, method, dependency) in [
            (
                "c_containers.py",
                "pkg.containers",
                "ContainerService",
                "checkin",
                Some(("pkg.workflows", "CheckinWorkflow", "workflow", "run")),
            ),
            (
                "b_workflows.py",
                "pkg.workflows",
                "CheckinWorkflow",
                "run",
                Some(("pkg.commands", "CommandService", "command", "execute")),
            ),
            (
                "a_commands.py",
                "pkg.commands",
                "CommandService",
                "execute",
                None,
            ),
        ] {
            let class_symbol = format!("{module}.{class}");
            let symbol = format!("{class_symbol}.{method}");
            add(path, "module", module, 1, 500, "");
            add(path, "class", &class_symbol, 10, 400, "");
            add(path, "function", &symbol, 30, 350, "");
            add(path, "parameter", &format!("{symbol}.self"), 30, 30, "self");
            for line in 31..111 {
                let value = [
                    "list()",
                    "getattr(self,<literal>)",
                    "items.append(<literal>)",
                    "self.logger.info(<literal>)",
                    "payload.get(<literal>)",
                ][line % 5];
                add(path, "call", &symbol, line, line, value);
            }
            if let Some((dep_module, dep_class, field, dep_method)) = dependency {
                add(path,"from_import",module,2,2,&json!({"module":dep_module,"level":0,"names":[{"name":dep_class,"asname":null}]}).to_string());
                let init = format!("{class_symbol}.__init__");
                add(path, "function", &init, 11, 20, "");
                add(path, "parameter", &format!("{init}.self"), 11, 11, "self");
                add(
                    path,
                    "binding",
                    &init,
                    12,
                    12,
                    &json!({"target":format!("self.{field}"),"source":format!("{dep_class}()")})
                        .to_string(),
                );
                add(
                    path,
                    "call",
                    &symbol,
                    300,
                    300,
                    &format!("self.{field}.{dep_method}()"),
                );
            } else {
                add(
                    path,
                    "from_import",
                    module,
                    2,
                    2,
                    r#"{"module":"requests","level":0,"names":[{"name":"post","asname":"send_request"}]}"#,
                );
                add(
                    path,
                    "call",
                    &symbol,
                    300,
                    300,
                    "self.ctx.benchling.containers.checkin(<literal>)",
                );
                add(path, "call", &symbol, 301, 301, "send_request(<literal>)");
            }
        }
        let d = draw(&e, View::Services, 4, 1000);
        let nodes = d.legend["nodes"].as_array().unwrap();
        let edges = d.legend["edges"].as_array().unwrap();
        let mut reached = BTreeSet::from(["H"]);
        let mut queue = VecDeque::from(["H"]);
        while let Some(id) = queue.pop_front() {
            for edge in edges.iter().filter(|e| e["from"].as_str() == Some(id)) {
                let target = edge["to"].as_str().unwrap();
                if reached.insert(target) {
                    queue.push_back(target);
                }
            }
        }
        for (symbol, distance) in [
            ("pkg.containers.ContainerService.checkin", 1),
            ("pkg.workflows.CheckinWorkflow.run", 2),
            ("pkg.commands.CommandService.execute", 3),
        ] {
            let node = nodes
                .iter()
                .find(|n| n["label"] == symbol)
                .unwrap_or_else(|| panic!("missing {symbol}: {}", d.legend));
            assert_eq!(node["distance"], distance);
            assert!(
                reached.contains(node["id"].as_str().unwrap()),
                "disconnected {symbol}"
            );
        }
        for node in nodes
            .iter()
            .filter(|n| n["kind"] == "callable/service collaborator candidate")
        {
            assert!(reached.contains(node["id"].as_str().unwrap()));
        }
        for expression in [
            "service.checkin",
            "self.workflow.run",
            "self.command.execute",
            "self.ctx.benchling.containers.checkin",
            "send_request",
        ] {
            assert!(
                d.mermaid.contains(expression),
                "starved {expression}: {}",
                d.mermaid
            );
        }
        assert_eq!(d.legend["counts"]["routineCallsInLegend"], 240);
        assert_eq!(d.legend["counts"]["renderedCallSites"], 5);
        assert_eq!(d.legend["calls"].as_array().unwrap().len(), 245);
        assert!(nodes.iter().all(|n| n["label"] != "list"
            && n["label"] != "getattr"
            && n["label"] != "items.append"
            && n["label"] != "self.logger.info"
            && n["label"] != "payload.get"));
        let sdk = d.legend["calls"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["label"] == "self.ctx.benchling.containers.checkin")
            .unwrap();
        assert_eq!(sdk["classification"], "UNKNOWN unresolved destination");
        assert!(sdk["diagramId"].is_string());
        assert!(
            nodes
                .iter()
                .filter(|n| n["kind"] == "observed call site")
                .count()
                <= MAX_CALL_NODES
        );
        assert!(
            nodes
                .iter()
                .filter(|n| n["kind"] == "callable/service collaborator candidate")
                .count()
                < MAX_CALLABLE_NODES
        );
        e.facts.reverse();
        let reversed = draw(&e, View::Services, 4, 1000);
        assert_eq!(d.mermaid, reversed.mermaid);
        assert_eq!(d.legend, reversed.legend);
    }

    #[test]
    fn class_attribute_provenance_is_explicit_and_rebinding_blocks_it() {
        let mut e = source();
        e.facts.retain(|f| f.line < 25);
        e.facts.extend([
            fact("from_import","app",21,21,r#"{"module":"benchling_sdk.benchling","level":0,"names":[{"name":"Benchling","asname":null}]}"#),
            fact("class","app.Context",30,45,"Context"),
            fact("attribute_annotation","app.Context.benchling",31,31,"Benchling"),
            fact("function","app.service",50,90,"service"),
            fact("parameter_annotation","app.service.ctx",50,50,"Context"),
            fact("call","app.service",52,52,"ctx.benchling.containers.checkin(<literal>)"),
        ]);
        let d = draw(&e, View::Network, 2, 100);
        assert!(d.mermaid.contains("containers.checkin"));
        assert!(d.legend["boundaries"][0]["sourceRefs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["kind"] == "attribute_annotation"));
        for (symbol, line, target) in [
            ("app.service", 51, "ctx.benchling"),
            ("app.Context.reset", 41, "self.benchling"),
            ("app.Context", 30, "Benchling"),
        ] {
            let mut blocked: Evidence =
                serde_json::from_value(serde_json::to_value(&e).unwrap()).unwrap();
            if symbol.ends_with("reset") {
                blocked
                    .facts
                    .push(fact("function", symbol, 40, 44, "reset"));
            }
            blocked.facts.push(fact(
                "binding_invalidation",
                symbol,
                line,
                line,
                &json!({"target":target}).to_string(),
            ));
            assert!(!draw(&blocked, View::Network, 2, 100)
                .mermaid
                .contains("Benchling SDK boundary candidate"));
        }
        e.facts.retain(|f| f.kind != "attribute_annotation");
        assert!(!draw(&e, View::Network, 2, 100)
            .mermaid
            .contains("Benchling SDK boundary candidate"));
    }
}
