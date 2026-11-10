//! Bounded static evidence, not inferred runtime architecture.
use crate::analysis::{Evidence, Fact};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const POLICY_VERSION: &str = "architecture-packet.v2-relations";
pub const MAX_INPUT_BYTES: usize = 192 * 1024;

pub struct Prepared {
    pub input: Value,
    pub coverage: Coverage,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Coverage {
    pub policy_version: String,
    pub selection_applied: bool,
    pub input_bytes: usize,
    pub max_bytes: usize,
    pub total_facts: usize,
    pub included_facts: usize,
    pub omitted_facts: usize,
    pub total_files: usize,
    pub included_files: usize,
    pub by_kind: BTreeMap<String, KindCoverage>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KindCoverage {
    pub total: usize,
    pub included: usize,
    pub omitted: usize,
}

fn category(kind: &str) -> usize {
    match kind {
        "module" | "class" | "function" | "async_function" => 0,
        "import" | "from_import" => 1,
        "call" => 3,
        "parameter"
        | "signature"
        | "attribute_annotation"
        | "binding_invalidation"
        | "api_route"
        | "entrypoint" => 4,
        "router_prefix" | "router_include" | "router_mutation" => 4,
        "router_host" => 4,
        _ => 2,
    }
}

fn within(child: &str, parent: &str) -> bool {
    child == parent
        || child
            .strip_prefix(parent)
            .is_some_and(|s| s.starts_with('.'))
}

fn binding(f: &Fact) -> Option<(String, String)> {
    if !matches!(f.kind.as_str(), "binding" | "binding_invalidation") {
        return None;
    }
    let v: Value = serde_json::from_str(&f.value).ok()?;
    let source = if f.kind == "binding_invalidation" {
        ""
    } else {
        v["source"].as_str()?
    };
    Some((v["target"].as_str()?.into(), source.into()))
}

fn receiver(call: &Fact, target: &str) -> bool {
    call.kind == "call"
        && call
            .value
            .strip_prefix(target)
            .is_some_and(|s| s.starts_with('.') || s.starts_with('('))
}

fn priority(f: &Fact) -> u8 {
    if f.kind == "binding" || f.symbol.contains(".__init__.") {
        return 0;
    }
    let head = f.value.split('(').next().unwrap_or("");
    if head.starts_with("self.")
        && head.matches('.').count() >= 2
        && !head.starts_with("self.logger.")
        && !head.starts_with("self.log.")
    {
        return 0;
    }
    if ["register", "provide", "include_router", "add_route"]
        .iter()
        .any(|s| head.split('.').any(|part| part.contains(s)))
        || head
            .rsplit('.')
            .next()
            .is_some_and(|s| s.starts_with(char::is_uppercase))
    {
        return 1;
    }
    if matches!(head, "list" | "dict" | "set" | "len" | "str" | "print")
        || head.starts_with("logger.")
        || head.starts_with("logging.")
    {
        return 3;
    }
    2
}

// Recursively interleave packages, files, then lexical scopes. A large first
// package/file/scope receives one turn, not all of a category's quota.
fn fair_order(ids: Vec<usize>, facts: &[&Fact], depth: usize) -> Vec<usize> {
    if depth == 3 {
        let mut ids = ids;
        ids.sort_by_key(|&i| (priority(facts[i]), i));
        return ids;
    }
    let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for i in ids {
        let f = facts[i];
        let key = match depth {
            0 => f.path.rsplit_once('/').map_or("", |(p, _)| p),
            1 => &f.path,
            _ => &f.symbol,
        };
        groups.entry(key.into()).or_default().push(i);
    }
    let mut queues: Vec<VecDeque<usize>> = groups
        .into_values()
        .map(|g| fair_order(g, facts, depth + 1).into())
        .collect();
    let mut result = Vec::new();
    loop {
        let before = result.len();
        for q in &mut queues {
            if let Some(i) = q.pop_front() {
                result.push(i);
            }
        }
        if result.len() == before {
            return result;
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Scope {
    owner: Option<usize>,
    class: Option<usize>,
    function: Option<usize>,
}

struct Context {
    scopes: BTreeMap<usize, Scope>,
    call_bindings: BTreeMap<usize, usize>,
}

fn context(facts: &[&Fact], same_file: &[usize]) -> Context {
    let declarations: Vec<_> = same_file
        .iter()
        .copied()
        .filter(|&j| category(&facts[j].kind) == 0)
        .collect();
    let mut scopes = BTreeMap::new();
    for &i in same_file {
        let f = facts[i];
        let enclosing: Vec<_> = declarations
            .iter()
            .copied()
            .filter(|&j| {
                let d = facts[j];
                within(&f.symbol, &d.symbol)
                    && (d.kind == "module" || (d.line <= f.line && f.end_line <= d.end_line))
            })
            .collect();
        let nearest = |kind: &str| {
            enclosing
                .iter()
                .copied()
                .filter(|&j| {
                    kind == "scope"
                        || facts[j].kind == kind
                        || (kind == "function" && facts[j].kind == "async_function")
                })
                .max_by_key(|&j| {
                    (
                        facts[j].symbol.len(),
                        facts[j].line,
                        std::cmp::Reverse(facts[j].end_line),
                        std::cmp::Reverse(j),
                    )
                })
        };
        scopes.insert(
            i,
            Scope {
                owner: nearest("scope"),
                class: nearest("class"),
                function: nearest("function"),
            },
        );
    }
    let mut result = Context {
        scopes,
        call_bindings: BTreeMap::new(),
    };
    for &i in same_file {
        if facts[i].kind == "call" {
            if let Some(b) = latest_binding(i, facts, same_file, &result.scopes) {
                result.call_bindings.insert(i, b);
            }
        }
    }
    result
}

// Only direct methods share the conventional self/cls receiver. Nested
// functions, static methods, and explicitly rebound receiver names do not.
fn method_receiver(
    i: usize,
    root: &str,
    facts: &[&Fact],
    same_file: &[usize],
    scopes: &BTreeMap<usize, Scope>,
) -> bool {
    let scope = scopes[&i];
    let (Some(class), Some(function)) = (scope.class, scope.function) else {
        return false;
    };
    let f = facts[function];
    if scope.owner != Some(function)
        || f.symbol.rsplit_once('.').map(|(p, _)| p) != Some(facts[class].symbol.as_str())
    {
        return false;
    }
    let previous_end = same_file
        .iter()
        .copied()
        .filter(|&j| {
            matches!(facts[j].kind.as_str(), "function" | "async_function")
                && facts[j].symbol == f.symbol
                && facts[j].end_line < f.line
        })
        .map(|j| facts[j].end_line)
        .max()
        .unwrap_or(facts[class].line)
        .max(facts[class].line);
    let mut classmethod = false;
    for &j in same_file {
        let d = facts[j];
        if d.kind == "decorator"
            && d.symbol == f.symbol
            && d.line > previous_end
            && d.end_line < f.line
        {
            let name = d
                .value
                .split('(')
                .next()
                .unwrap_or("")
                .rsplit('.')
                .next()
                .unwrap_or("");
            if name == "staticmethod" {
                return false;
            }
            classmethod |= name == "classmethod";
        }
        if let Some((target, _)) = binding(d) {
            if target == root && scopes[&j].owner == Some(function) && d.line <= facts[i].line {
                return false;
            }
        }
    }
    (root == "self" && !classmethod) || (root == "cls" && classmethod)
}

fn latest_binding(
    call: usize,
    facts: &[&Fact],
    same_file: &[usize],
    scopes: &BTreeMap<usize, Scope>,
) -> Option<usize> {
    let c = facts[call];
    let cs = scopes[&call];
    let mut candidates = Vec::new();
    for &b in same_file {
        let Some((target, _)) = binding(facts[b]) else {
            continue;
        };
        if !receiver(c, &target) {
            continue;
        }
        let bs = scopes[&b];
        let same_scope = cs.owner.is_some() && cs.owner == bs.owner && c.symbol == facts[b].symbol;
        if same_scope {
            // Without column/CFG evidence, same-line order is ambiguous.
            if facts[b].end_line == c.line {
                return None;
            }
            if facts[b].end_line < c.line {
                candidates.push((true, target.len(), facts[b].line, b));
            }
            continue;
        }
        let Some(root) = target
            .split('.')
            .next()
            .filter(|r| matches!(*r, "self" | "cls"))
        else {
            continue;
        };
        if cs.class.is_none()
            || cs.class != bs.class
            || !method_receiver(call, root, facts, same_file, scopes)
            || !method_receiver(b, root, facts, same_file, scopes)
        {
            continue;
        }
        let Some(function) = bs.function else {
            continue;
        };
        if !facts[function].symbol.ends_with(".__init__") {
            continue;
        }
        // A later definition of __init__ replaces the earlier declaration even
        // if the later body has no matching assignment.
        if same_file.iter().copied().any(|j| {
            matches!(facts[j].kind.as_str(), "function" | "async_function")
                && facts[j].symbol == facts[function].symbol
                && scopes[&j].class == bs.class
                && facts[j].line > facts[function].line
        }) {
            continue;
        }
        candidates.push((false, target.len(), facts[b].line, b));
    }
    candidates.sort_unstable();
    let best = candidates.pop()?;
    // Competing assignments at the same source line cannot be ordered safely.
    if candidates
        .last()
        .is_some_and(|other| (other.0, other.1, other.2) == (best.0, best.1, best.2))
    {
        return None;
    }
    let (target, _) = binding(facts[best.3])?;
    let binding_owner = scopes[&best.3].owner;
    if let Some(j) = same_file
        .iter()
        .copied()
        .filter(|&j| {
            binding(facts[j]).is_some_and(|(other, _)| {
                target != other
                && within(&target, &other)
                // Constructor context has its own assignment timeline,
                // independent of the calling method's source position.
                && ((scopes[&j].owner == binding_owner
                    && facts[j].line > facts[best.3].line
                    && (!best.0 || facts[j].line <= c.line))
                    || (!best.0 && scopes[&j].owner == cs.owner && facts[j].line <= c.line))
            })
        })
        .max_by_key(|&j| facts[j].line)
    {
        return (facts[j].kind == "binding_invalidation").then_some(j);
    }
    Some(best.3)
}

fn neighborhood(i: usize, facts: &[&Fact], same_file: &[usize], context: &Context) -> Vec<usize> {
    let anchor = facts[i];
    let mut result = vec![i];
    // At most one enclosing declaration of each kind; nearest lexical owner
    // wins, with line containment preventing unrelated same-name declarations.
    for kind in ["module", "class", "function", "async_function"] {
        if let Some(j) = same_file
            .iter()
            .copied()
            .filter(|&j| {
                let f = facts[j];
                f.kind == kind
                    && within(&anchor.symbol, &f.symbol)
                    && (kind == "module"
                        || (f.line <= anchor.line && f.end_line >= anchor.end_line))
            })
            .max_by_key(|&j| (facts[j].symbol.len(), std::cmp::Reverse(j)))
        {
            result.push(j);
        }
    }
    // Auxiliary lexical guards travel with their owner, never as global seeds.
    result.extend(same_file.iter().copied().filter(|&j| {
        facts[j].kind == "parameter"
            && context.scopes[&i].owner.is_some()
            && context.scopes[&j].owner == context.scopes[&i].owner
    }));
    let relevant_binding = if matches!(anchor.kind.as_str(), "binding" | "binding_invalidation") {
        Some(i)
    } else if anchor.kind == "call" {
        context.call_bindings.get(&i).copied()
    } else if anchor.kind == "parameter_annotation" {
        same_file
            .iter()
            .copied()
            .filter(|&j| {
                binding(facts[j]).is_some_and(|(_, source)| {
                    anchor.symbol == format!("{}.{}", facts[j].symbol, source)
                        && context.scopes[&i].owner.is_some()
                        && context.scopes[&i].owner == context.scopes[&j].owner
                })
            })
            .max_by_key(|&j| (facts[j].line, std::cmp::Reverse(j)))
    } else {
        None
    };
    if let Some(b) = relevant_binding {
        result.push(b);
        if let Some((target, source)) = binding(facts[b]) {
            if let Some(a) = same_file.iter().copied().find(|&j| {
                facts[j].kind == "parameter_annotation"
                    && facts[j].symbol == format!("{}.{}", facts[b].symbol, source)
                    && context.scopes[&b].owner.is_some()
                    && context.scopes[&j].owner == context.scopes[&b].owner
            }) {
                result.push(a);
            }
            if anchor.kind != "call" {
                if let Some(c) = same_file
                    .iter()
                    .copied()
                    .filter(|&j| {
                        receiver(facts[j], &target) && context.call_bindings.get(&j) == Some(&b)
                    })
                    .min_by_key(|&j| (priority(facts[j]), j))
                {
                    result.push(c);
                    if let Some(owner) = same_file.iter().copied().find(|&j| {
                        matches!(facts[j].kind.as_str(), "function" | "async_function")
                            && facts[j].symbol == facts[c].symbol
                            && facts[j].line <= facts[c].line
                            && facts[j].end_line >= facts[c].end_line
                    }) {
                        result.push(owner);
                    }
                }
            }
            if let Some(c) = same_file.iter().copied().find(|&j| {
                matches!(facts[j].kind.as_str(), "function" | "async_function")
                    && facts[j].symbol == facts[b].symbol
                    && facts[j].line <= facts[b].line
                    && facts[j].end_line >= facts[b].end_line
            }) {
                result.push(c);
            }
        }
    }
    result.sort_unstable();
    result.dedup();
    result
}

/// Indexed lexical guards shared with focused selection. Indices refer to the
/// caller's stable fact slice; this does not change global packet selection.
pub(crate) struct LexicalContext {
    pub owner: Option<usize>,
    pub class: Option<usize>,
    pub function: Option<usize>,
    pub binding: Option<usize>,
    /// A matching assignment exists, but lexical guards cannot select it.
    pub binding_blocked: bool,
    pub self_receiver: bool,
    pub neighborhood: Vec<usize>,
}

pub(crate) fn lexical_contexts(facts: &[&Fact]) -> BTreeMap<usize, LexicalContext> {
    let mut files: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, f) in facts.iter().enumerate() {
        files.entry(&f.path).or_default().push(i);
    }
    let mut result = BTreeMap::new();
    for ids in files.values() {
        let ctx = context(facts, ids);
        for &i in ids {
            let scope = ctx.scopes[&i];
            result.insert(
                i,
                LexicalContext {
                    owner: scope.owner,
                    class: scope.class,
                    function: scope.function,
                    binding: ctx.call_bindings.get(&i).copied(),
                    binding_blocked: facts[i].kind == "call"
                        && !ctx.call_bindings.contains_key(&i)
                        && ids.iter().copied().any(|j| {
                            let Some((target, _)) = binding(facts[j]) else {
                                return false;
                            };
                            if !receiver(facts[i], &target) {
                                return false;
                            }
                            let other = ctx.scopes[&j];
                            if scope.owner.is_some() && scope.owner == other.owner {
                                return facts[j].symbol == facts[i].symbol
                                    && facts[j].line <= facts[i].line;
                            }
                            let root = target.split('.').next().unwrap_or("");
                            scope.class.is_some()
                                && scope.class == other.class
                                && other
                                    .function
                                    .is_some_and(|f| facts[f].symbol.ends_with(".__init__"))
                                && method_receiver(i, root, facts, ids, &ctx.scopes)
                                && method_receiver(j, root, facts, ids, &ctx.scopes)
                        }),
                    self_receiver: method_receiver(i, "self", facts, ids, &ctx.scopes),
                    neighborhood: neighborhood(i, facts, ids, &ctx),
                },
            );
        }
    }
    result
}

fn alternatives(
    i: usize,
    facts: &[&Fact],
    same_file: &[usize],
    context: &Context,
) -> [Vec<usize>; 3] {
    let full = neighborhood(i, facts, same_file, context);
    let owner = full
        .iter()
        .copied()
        .filter(|&j| matches!(facts[j].kind.as_str(), "class" | "module"))
        .max_by_key(|&j| (facts[j].kind == "class", facts[j].symbol.len()));
    let mut minimal = vec![i];
    if let Some(j) = owner {
        minimal.push(j);
    }
    minimal.sort_unstable();
    minimal.dedup();
    [full, minimal, vec![i]]
}

fn warnings(c: &Coverage, reserve: bool) -> Vec<String> {
    let mut w = vec!["Static evidence does not prove runtime architecture; dynamic bindings remain unresolved, not confidence scores.".into()];
    if c.selection_applied {
        w.push("Partial selection: omitted facts may contain architectural relationships.".into());
        w.push("Bounded lexical neighborhoods may omit context when it cannot fit; no runtime edges are inferred.".into());
    }
    for (kind, count) in &c.by_kind {
        if matches!(category(kind), 1..=3) && (reserve || count.included == 0) {
            w.push(format!("Available relationship kind {kind} could not be included within the packet budget."));
        }
    }
    if reserve
        || !c
            .by_kind
            .iter()
            .any(|(k, n)| matches!(category(k), 1..=3) && n.included > 0)
    {
        w.push("Declarations-only packet: no relationship evidence is included.".into());
    }
    w.push(format!(
        "Selected facts cover {} of {} source files.",
        c.included_files, c.total_files
    ));
    w
}

fn selection(c: &Coverage) -> Value {
    json!({"policyVersion": POLICY_VERSION, "totalFacts": c.total_facts,
        "includedFacts": c.included_facts, "omittedFacts": c.omitted_facts,
        "maxBytes": MAX_INPUT_BYTES, "byKind": c.by_kind, "warnings": c.warnings,
        "note": "Original static facts only. Selection is not an exhaustive dependency graph."})
}

pub fn prepare(evidence: &Evidence) -> Result<Prepared, String> {
    if evidence.language.is_some() || evidence.schema_version == "ideomancer.evidence.v2" {
        evidence.validate_language()?;
    }
    if evidence.facts.is_empty() {
        return Err("Cannot prepare an empty facts packet for agent dispatch".into());
    }
    let mut input = serde_json::to_value(evidence).map_err(|e| e.to_string())?;
    let original_bytes = serde_json::to_vec(&input).map_err(|e| e.to_string())?.len();
    let mut coverage = Coverage {
        policy_version: POLICY_VERSION.into(),
        selection_applied: original_bytes > MAX_INPUT_BYTES,
        input_bytes: original_bytes,
        max_bytes: MAX_INPUT_BYTES,
        total_facts: evidence.facts.len(),
        included_facts: evidence.facts.len(),
        omitted_facts: 0,
        total_files: evidence.files.len(),
        included_files: evidence
            .facts
            .iter()
            .map(|f| &f.path)
            .collect::<BTreeSet<_>>()
            .len(),
        by_kind: BTreeMap::new(),
        warnings: Vec::new(),
    };
    for f in &evidence.facts {
        let n = coverage
            .by_kind
            .entry(f.kind.clone())
            .or_insert(KindCoverage {
                total: 0,
                included: 0,
                omitted: 0,
            });
        n.total += 1;
        n.included += 1;
    }
    coverage.warnings = warnings(&coverage, false);
    if !coverage.selection_applied {
        return Ok(Prepared { input, coverage });
    }

    let mut facts: Vec<&Fact> = evidence.facts.iter().collect();
    facts.sort_by(|a, b| {
        (
            &a.path, &a.symbol, a.line, a.end_line, &a.kind, &a.id, &a.value,
        )
            .cmp(&(
                &b.path, &b.symbol, b.line, b.end_line, &b.kind, &b.id, &b.value,
            ))
    });
    let values: Vec<Value> = facts
        .iter()
        .map(serde_json::to_value)
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    let sizes: Vec<usize> = values
        .iter()
        .map(|v| serde_json::to_vec(v).map(|v| v.len() + 1))
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    // Normalize only selected packets. Small packets must remain exactly equal
    // to to_value(evidence), including the caller's original array order.
    input["files"]
        .as_array_mut()
        .unwrap()
        .sort_by_key(|v| v.to_string());
    input["facts"] = json!([]);
    coverage.omitted_facts = coverage.total_facts;
    for n in coverage.by_kind.values_mut() {
        n.omitted = n.total;
    }
    coverage.warnings = warnings(&coverage, true);
    input["selection"] = selection(&coverage);
    // Reserve upper bounds for every warning and both count columns, plus
    // numeric-width slack. UTF-8 and JSON escaping are measured, not estimated.
    let overhead = serde_json::to_vec(&input).map_err(|e| e.to_string())?.len() + 128;
    let budget = MAX_INPUT_BYTES
        .checked_sub(overhead)
        .ok_or("Packet metadata leaves no useful fact budget (192 KiB limit)")?;
    let mut by_file: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, f) in facts.iter().enumerate() {
        by_file.entry(&f.path).or_default().push(i);
    }
    let contexts: BTreeMap<_, _> = by_file
        .iter()
        .map(|(&path, ids)| (path, context(&facts, ids)))
        .collect();
    let mut queues = Vec::new();
    for cat in 0..4 {
        queues.push(fair_order(
            (0..facts.len())
                .filter(|&i| category(&facts[i].kind) == cat)
                .collect(),
            &facts,
            0,
        ));
    }
    let mut chosen = BTreeSet::new();
    let mut used = 0usize;
    let mut spent = [0usize; 4];
    let quotas = [
        budget * 20 / 100,
        budget * 15 / 100,
        budget * 35 / 100,
        budget * 30 / 100,
    ];
    {
        let mut add = |i: usize, limit: usize| {
            if chosen.contains(&i) {
                return true;
            }
            if sizes[i] > limit || sizes[i] > budget - used {
                return false;
            }
            let cat = category(&facts[i].kind);
            for group in alternatives(
                i,
                &facts,
                &by_file[facts[i].path.as_str()],
                &contexts[facts[i].path.as_str()],
            ) {
                let cost: usize = group
                    .iter()
                    .filter(|j| !chosen.contains(*j))
                    .map(|&j| sizes[j])
                    .sum();
                if cost <= limit && cost <= budget - used {
                    chosen.extend(group);
                    used += cost;
                    spent[cat] += cost;
                    return true;
                }
            }
            false
        };
        // Seed each available relationship kind before declarations. Each seed has
        // an equal bounded allowance so one giant fact cannot swallow all seeds.
        let kinds: Vec<_> = coverage
            .by_kind
            .keys()
            .filter(|k| matches!(category(k), 1..=3))
            .collect();
        let mut missing = Vec::new();
        for kind in &kinds {
            if !queues[category(kind)].iter().copied().any(|i| {
                facts[i].kind.as_str() == kind.as_str() && add(i, budget / kinds.len().max(1))
            }) {
                missing.push(*kind);
            }
        }
        // Exceptionally large representatives may exceed a fair seed allowance.
        // Retry them before bulk filling; representation takes precedence over a
        // category share, but never over the global byte limit.
        for kind in missing {
            for &i in &queues[category(kind)] {
                if facts[i].kind == *kind && add(i, budget) {
                    break;
                }
            }
        }
    }
    // All newly added context is charged to its anchor's category, and shared
    // context is paid once. Unused shares are then redistributed relations first.
    for redistribute in [false, true] {
        for cat in [2, 3, 1, 0] {
            for &i in &queues[cat] {
                if chosen.contains(&i) {
                    continue;
                }
                let limit = if redistribute {
                    budget - used
                } else {
                    quotas[cat].saturating_sub(spent[cat]).min(budget - used)
                };
                if sizes[i] > limit {
                    continue;
                }
                for group in alternatives(
                    i,
                    &facts,
                    &by_file[facts[i].path.as_str()],
                    &contexts[facts[i].path.as_str()],
                ) {
                    let cost: usize = group
                        .iter()
                        .filter(|j| !chosen.contains(*j))
                        .map(|&j| sizes[j])
                        .sum();
                    if cost <= limit {
                        chosen.extend(group);
                        used += cost;
                        spent[cat] += cost;
                        break;
                    }
                }
            }
        }
    }
    if chosen.is_empty() {
        return Err(
            "Packet metadata leaves no useful fact budget: no original fact fits within 192 KiB"
                .into(),
        );
    }
    coverage.included_facts = chosen.len();
    coverage.omitted_facts = coverage.total_facts - chosen.len();
    coverage.included_files = chosen
        .iter()
        .map(|&i| &facts[i].path)
        .collect::<BTreeSet<_>>()
        .len();
    for n in coverage.by_kind.values_mut() {
        n.included = 0;
    }
    for &i in &chosen {
        coverage.by_kind.get_mut(&facts[i].kind).unwrap().included += 1;
    }
    for n in coverage.by_kind.values_mut() {
        n.omitted = n.total - n.included;
    }
    coverage.warnings = warnings(&coverage, false);
    input["facts"] = Value::Array(chosen.iter().map(|&i| values[i].clone()).collect());
    input["selection"] = selection(&coverage);
    coverage.input_bytes = serde_json::to_vec(&input).map_err(|e| e.to_string())?.len();
    if coverage.input_bytes > MAX_INPUT_BYTES {
        return Err("Architecture packet exceeded its compact JSON byte budget".into());
    }
    Ok(Prepared { input, coverage })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::SourceFile;

    fn evidence() -> Evidence {
        Evidence {
            schema_version: "evidence.v1".into(),
            snapshot_id: "snapshot".into(),
            extractor_version: "static-bindings".into(),
            python_version: "3.12".into(),
            language: None,
            files: Vec::new(),
            facts: Vec::new(),
            diagnostics: Vec::new(),
            complete: true,
        }
    }

    fn fact(
        e: &mut Evidence,
        path: &str,
        kind: &str,
        symbol: &str,
        line: usize,
        end: usize,
        value: &str,
    ) {
        e.facts.push(Fact {
            id: format!("fact_{}", e.facts.len()),
            kind: kind.into(),
            path: path.into(),
            symbol: symbol.into(),
            line,
            end_line: end,
            value: value.into(),
        });
    }

    fn large() -> Evidence {
        let mut e = evidence();
        for file in 0..32 {
            let path = format!("pkg{}/{file:03}.py", file % 4);
            let module = format!("pkg{file}.module");
            e.files.push(SourceFile {
                path: path.clone(),
                sha256: "0".repeat(64),
            });
            fact(&mut e, &path, "module", &module, 1, 10000, &module);
            for class in 0..80 {
                let symbol = format!("{module}.Dto{class:03}");
                fact(
                    &mut e,
                    &path,
                    "class",
                    &symbol,
                    1,
                    9000,
                    &"Declaration".repeat(18),
                );
            }
            let class = format!("{module}.Service");
            fact(&mut e, &path, "class", &class, 1, 200, "Service");
            fact(
                &mut e,
                &path,
                "function",
                &format!("{class}.__init__"),
                10,
                30,
                "__init__",
            );
            fact(
                &mut e,
                &path,
                "function",
                &format!("{class}.run"),
                40,
                80,
                "run",
            );
            fact(
                &mut e,
                &path,
                "parameter_annotation",
                &format!("{class}.__init__.dep"),
                10,
                10,
                "Dependency",
            );
            fact(
                &mut e,
                &path,
                "parameter_default",
                &format!("{class}.__init__.dep"),
                10,
                10,
                "Provider()",
            );
            fact(
                &mut e,
                &path,
                "binding",
                &format!("{class}.__init__"),
                20,
                20,
                r#"{"target":"self.dep","source":"dep"}"#,
            );
            fact(
                &mut e,
                &path,
                "call",
                &format!("{class}.run"),
                50,
                50,
                "self.dep.execute()",
            );
            fact(&mut e, &path, "decorator", &class, 1, 1, "register()");
            fact(&mut e, &path, "base", &class, 1, 1, "Base");
            fact(
                &mut e,
                &path,
                "import",
                &module,
                1,
                1,
                r#"{"module":"dependency"}"#,
            );
            fact(
                &mut e,
                &path,
                "from_import",
                &module,
                2,
                2,
                r#"{"module":"dependency","names":[{"name":"Dependency"}]}"#,
            );
        }
        e
    }

    fn check(e: &Evidence, p: &Prepared) {
        let selected = p.input["facts"].as_array().unwrap();
        assert!(!selected.is_empty());
        assert_eq!(
            p.coverage.input_bytes,
            serde_json::to_vec(&p.input).unwrap().len()
        );
        assert!(p.coverage.input_bytes <= MAX_INPUT_BYTES);
        assert_eq!(selected.len(), p.coverage.included_facts);
        assert_eq!(
            e.facts.len(),
            p.coverage.included_facts + p.coverage.omitted_facts
        );
        assert_eq!(p.coverage.total_files, e.files.len());
        let original: BTreeMap<_, _> = e
            .facts
            .iter()
            .map(|f| (f.id.as_str(), serde_json::to_value(f).unwrap()))
            .collect();
        let mut kinds = BTreeMap::<String, usize>::new();
        let mut files = BTreeSet::new();
        for f in selected {
            assert_eq!(&original[f["id"].as_str().unwrap()], f);
            *kinds.entry(f["kind"].as_str().unwrap().into()).or_default() += 1;
            files.insert(f["path"].as_str().unwrap());
        }
        assert_eq!(p.coverage.included_files, files.len());
        for (k, n) in &p.coverage.by_kind {
            assert_eq!(n.included, *kinds.get(k).unwrap_or(&0));
            assert_eq!(n.total, n.included + n.omitted);
        }
    }

    #[test]
    fn small_input_is_exactly_unchanged() {
        let mut e = evidence();
        fact(&mut e, "z.py", "module", "z", 1, 1, "z");
        fact(&mut e, "a.py", "module", "a", 1, 1, "a");
        let p = prepare(&e).unwrap();
        assert_eq!(p.input, serde_json::to_value(&e).unwrap());
        assert!(!p.coverage.selection_applied);
        assert!(p
            .coverage
            .warnings
            .iter()
            .any(|s| s.contains("Declarations-only")));
        check(&e, &p);
    }

    #[test]
    fn declaration_heavy_retains_relationships_and_connected_scopes() {
        let e = large();
        let p = prepare(&e).unwrap();
        assert!(p.coverage.selection_applied);
        check(&e, &p);
        for kind in [
            "import",
            "from_import",
            "call",
            "binding",
            "decorator",
            "base",
            "parameter_annotation",
            "parameter_default",
        ] {
            assert!(p.coverage.by_kind[kind].included > 0, "{kind}");
        }
        let selected = p.input["facts"].as_array().unwrap();
        for file in &e.files {
            let facts: Vec<_> = selected.iter().filter(|f| f["path"] == file.path).collect();
            assert!(facts.iter().any(|f| f["kind"] == "call"), "{}", file.path);
            assert!(facts.iter().any(|f| f["kind"] == "binding"));
            assert!(facts.iter().any(|f| f["kind"] == "parameter_annotation"));
            assert!(facts.iter().any(|f| f["kind"] == "module"));
        }
    }

    #[test]
    fn large_permutations_have_identical_packets_and_coverage() {
        let mut e = large();
        let first = prepare(&e).unwrap();
        e.files.reverse();
        e.facts.reverse();
        let second = prepare(&e).unwrap();
        assert_eq!(first.input, second.input);
        assert_eq!(
            serde_json::to_value(first.coverage).unwrap(),
            serde_json::to_value(second.coverage).unwrap()
        );
        e.facts.rotate_left(137);
        assert_eq!(second.input, prepare(&e).unwrap().input);
    }

    #[test]
    fn unicode_escaping_and_metadata_are_in_actual_budget() {
        let mut e = large();
        e.snapshot_id = "\u{e9}\\\"\n".repeat(4000);
        for f in &mut e.facts {
            f.value.push_str(" \u{e9}\\\"\n");
        }
        let p = prepare(&e).unwrap();
        check(&e, &p);
        assert_eq!(p.input["snapshotId"], e.snapshot_id);
        assert_eq!(p.input["files"].as_array().unwrap().len(), e.files.len());
        assert_eq!(
            p.input["selection"]["byKind"],
            serde_json::to_value(&p.coverage.by_kind).unwrap()
        );
    }

    #[test]
    fn oversized_metadata_and_unusable_fact_budget_fail() {
        let mut e = large();
        e.snapshot_id = "x".repeat(MAX_INPUT_BYTES);
        assert!(prepare(&e)
            .err()
            .unwrap()
            .contains("metadata leaves no useful fact budget"));
        let mut e = evidence();
        e.snapshot_id = "x".repeat(MAX_INPUT_BYTES - 4000);
        fact(&mut e, "a.py", "module", "a", 1, 1, &"x".repeat(5000));
        assert!(prepare(&e).err().unwrap().contains("no useful fact budget"));
    }

    #[test]
    fn huge_relationship_is_reported_not_replaced_with_fabrication() {
        let mut e = large();
        fact(
            &mut e,
            "a.py",
            "return_annotation",
            "a.run",
            1,
            1,
            &"T".repeat(MAX_INPUT_BYTES),
        );
        let p = prepare(&e).unwrap();
        check(&e, &p);
        assert_eq!(p.coverage.by_kind["return_annotation"].included, 0);
        assert!(p
            .coverage
            .warnings
            .iter()
            .any(|w| w.contains("return_annotation")));
    }

    #[test]
    fn lexical_boundaries_and_line_containment_do_not_attach_other_classes() {
        assert!(!within("m.ServiceExtra.run", "m.Service"));
        let mut e = evidence();
        fact(&mut e, "m.py", "class", "m.Service", 1, 100, "Service");
        fact(
            &mut e,
            "m.py",
            "class",
            "m.ServiceExtra",
            101,
            200,
            "ServiceExtra",
        );
        fact(
            &mut e,
            "m.py",
            "binding",
            "m.Service.__init__",
            20,
            20,
            r#"{"target":"self.dep","source":"dep"}"#,
        );
        fact(
            &mut e,
            "m.py",
            "call",
            "m.ServiceExtra.run",
            150,
            150,
            "self.dep.run()",
        );
        let refs: Vec<_> = e.facts.iter().collect();
        let ids = [0, 1, 2, 3];
        assert_eq!(
            neighborhood(3, &refs, &ids, &context(&refs, &ids)),
            vec![1, 3]
        );
    }

    #[test]
    fn coverage_is_strict_camel_case() {
        let mut e = evidence();
        fact(&mut e, "a.py", "module", "a", 1, 1, "a");
        let p = prepare(&e).unwrap();
        let mut v = serde_json::to_value(p.coverage).unwrap();
        assert!(v.get("policyVersion").is_some());
        v["extra"] = json!(true);
        assert!(serde_json::from_value::<Coverage>(v).is_err());
        assert!(serde_json::from_value::<KindCoverage>(
            json!({"total":1,"included":1,"omitted":0,"extra":0})
        )
        .is_err());
    }

    #[test]
    fn empty_packets_fail_before_dispatch() {
        assert!(prepare(&evidence())
            .err()
            .unwrap()
            .contains("empty facts packet"));
    }

    #[test]
    fn compact_limit_is_inclusive_not_a_character_estimate() {
        let mut e = evidence();
        fact(&mut e, "a.py", "module", "a", 1, 1, "");
        let base = serde_json::to_vec(&e).unwrap().len();
        e.facts[0].value = "x".repeat(MAX_INPUT_BYTES - base);
        let p = prepare(&e).unwrap();
        assert_eq!(p.coverage.input_bytes, MAX_INPUT_BYTES);
        assert_eq!(p.input, serde_json::to_value(&e).unwrap());
        e.facts[0].value.push('x');
        assert!(prepare(&e).is_err());
    }

    #[test]
    fn separate_byte_shares_cannot_be_consumed_by_declarations() {
        let mut e = evidence();
        fact(&mut e, "m.py", "module", "m", 1, 10000, "m");
        for kind in ["class", "import", "binding", "call"] {
            for n in 0..500 {
                let scope = if kind == "class" { "Dto" } else { "work" };
                fact(
                    &mut e,
                    "m.py",
                    kind,
                    &format!("m.{scope}{n}"),
                    n + 1,
                    n + 1,
                    &"x".repeat(400),
                );
            }
        }
        let p = prepare(&e).unwrap();
        check(&e, &p);
        let mut bytes = [0usize; 4];
        for f in p.input["facts"].as_array().unwrap() {
            bytes[category(f["kind"].as_str().unwrap())] +=
                serde_json::to_vec(f).unwrap().len() + 1;
        }
        let sum: usize = bytes.iter().sum();
        assert!(bytes[0] * 100 / sum <= 25, "{bytes:?}");
        for (cat, minimum) in [(1, 10), (2, 25), (3, 20)] {
            assert!(bytes[cat] * 100 / sum >= minimum, "{bytes:?}");
        }
    }

    #[test]
    fn late_service_scope_survives_early_dto_utility_flood() {
        let mut e = large();
        let path = e.files[0].path.clone();
        for n in 0..3000 {
            fact(
                &mut e,
                &path,
                "call",
                "pkg0.module.ADto.convert",
                n + 1,
                n + 1,
                "dict(values)",
            );
        }
        let p = prepare(&e).unwrap();
        check(&e, &p);
        assert!(p.input["facts"].as_array().unwrap().iter().any(|f| {
            f["path"] == path
                && f["symbol"] == "pkg0.module.Service.run"
                && f["value"] == "self.dep.execute()"
        }));
    }

    #[test]
    fn oversized_owner_does_not_starve_small_relationship_anchors() {
        let mut e = large();
        let path = e.files[0].path.clone();
        e.facts
            .retain(|f| f.path == path && !f.symbol.contains(".Dto"));
        for f in &mut e.facts {
            if matches!(f.kind.as_str(), "module" | "class" | "function") {
                f.value = "x".repeat(MAX_INPUT_BYTES);
            }
        }
        let p = prepare(&e).unwrap();
        check(&e, &p);
        for kind in [
            "call",
            "binding",
            "import",
            "from_import",
            "decorator",
            "base",
            "parameter_annotation",
            "parameter_default",
        ] {
            assert!(p.coverage.by_kind[kind].included > 0, "{kind}");
        }
    }

    fn neighbors(e: &Evidence, anchor: usize) -> Vec<usize> {
        let facts: Vec<_> = e.facts.iter().collect();
        let ids: Vec<_> = (0..facts.len())
            .filter(|&j| facts[j].path == facts[anchor].path)
            .collect();
        neighborhood(anchor, &facts, &ids, &context(&facts, &ids))
    }

    #[test]
    fn nested_class_and_independent_receiver_functions_do_not_share_outer_bindings() {
        let mut e = evidence();
        fact(&mut e, "m.py", "module", "m", 1, 200, "m");
        fact(&mut e, "m.py", "class", "m.Outer", 2, 200, "Outer");
        fact(
            &mut e,
            "m.py",
            "function",
            "m.Outer.__init__",
            3,
            10,
            "__init__",
        );
        fact(
            &mut e,
            "m.py",
            "parameter_annotation",
            "m.Outer.__init__.dep",
            3,
            3,
            "OuterDependency",
        );
        fact(
            &mut e,
            "m.py",
            "binding",
            "m.Outer.__init__",
            5,
            5,
            r#"{"target":"self.dep","source":"dep"}"#,
        );
        fact(&mut e, "m.py", "class", "m.Outer.Inner", 20, 60, "Inner");
        fact(
            &mut e,
            "m.py",
            "function",
            "m.Outer.Inner.run",
            21,
            30,
            "run",
        );
        fact(
            &mut e,
            "m.py",
            "call",
            "m.Outer.Inner.run",
            25,
            25,
            "self.dep.execute()",
        );
        fact(
            &mut e,
            "m.py",
            "function",
            "m.Outer.factory",
            70,
            100,
            "factory",
        );
        fact(
            &mut e,
            "m.py",
            "function",
            "m.Outer.factory.worker",
            80,
            90,
            "worker",
        );
        fact(
            &mut e,
            "m.py",
            "call",
            "m.Outer.factory.worker",
            85,
            85,
            "self.dep.execute()",
        );
        fact(
            &mut e,
            "m.py",
            "decorator",
            "m.Outer.static",
            110,
            110,
            "staticmethod",
        );
        fact(
            &mut e,
            "m.py",
            "function",
            "m.Outer.static",
            111,
            120,
            "static",
        );
        fact(
            &mut e,
            "m.py",
            "call",
            "m.Outer.static",
            115,
            115,
            "self.dep.execute()",
        );
        for call in [7, 10, 13] {
            let n = neighbors(&e, call);
            assert!(!n.contains(&4), "outer binding attached to {call}: {n:?}");
            assert!(!n.contains(&3));
            assert!(!neighbors(&e, 4).contains(&call));
            assert!(!neighbors(&e, 3).contains(&call));
        }
    }

    #[test]
    fn nearest_preceding_assignment_replaces_old_call_context() {
        let mut e = evidence();
        fact(&mut e, "m.py", "function", "m.run", 1, 50, "run");
        fact(
            &mut e,
            "m.py",
            "binding",
            "m.run",
            5,
            5,
            r#"{"target":"client","source":"Old()"}"#,
        );
        fact(
            &mut e,
            "m.py",
            "binding",
            "m.run",
            10,
            10,
            r#"{"target":"client","source":"New()"}"#,
        );
        fact(&mut e, "m.py", "call", "m.run", 15, 15, "client.send()");
        assert_eq!(neighbors(&e, 3), vec![0, 2, 3]);
        assert!(!neighbors(&e, 1).contains(&3));
        assert!(neighbors(&e, 2).contains(&3));
        // Same-line assignments are deliberately not ordered without columns.
        e.facts[2].line = 15;
        e.facts[2].end_line = 15;
        assert_eq!(neighbors(&e, 3), vec![0, 3]);
    }

    #[test]
    fn repeated_function_instances_keep_their_own_bindings_and_annotations() {
        let mut e = evidence();
        for (start, value) in [(1, "OldDependency"), (40, "NewDependency")] {
            fact(
                &mut e,
                "m.py",
                "function",
                "m.run",
                start,
                start + 30,
                "run",
            );
            fact(
                &mut e,
                "m.py",
                "parameter_annotation",
                "m.run.client",
                start,
                start,
                value,
            );
            fact(
                &mut e,
                "m.py",
                "binding",
                "m.run",
                start + 5,
                start + 5,
                r#"{"target":"client","source":"client"}"#,
            );
            fact(
                &mut e,
                "m.py",
                "call",
                "m.run",
                start + 10,
                start + 10,
                "client.send()",
            );
        }
        assert_eq!(neighbors(&e, 3), vec![0, 1, 2, 3]);
        assert_eq!(neighbors(&e, 7), vec![4, 5, 6, 7]);
        assert_eq!(neighbors(&e, 5), vec![4, 5, 6, 7]);
        assert_eq!(neighbors(&e, 6), vec![4, 5, 6, 7]);
    }

    #[test]
    fn repeated_class_instances_do_not_share_constructor_context() {
        let mut e = evidence();
        for start in [1, 100] {
            fact(
                &mut e,
                "m.py",
                "class",
                "m.Service",
                start,
                start + 90,
                "Service",
            );
            fact(
                &mut e,
                "m.py",
                "function",
                "m.Service.__init__",
                start + 1,
                start + 20,
                "__init__",
            );
            fact(
                &mut e,
                "m.py",
                "parameter_annotation",
                "m.Service.__init__.dep",
                start + 1,
                start + 1,
                if start == 1 { "Old" } else { "New" },
            );
            fact(
                &mut e,
                "m.py",
                "binding",
                "m.Service.__init__",
                start + 5,
                start + 5,
                r#"{"target":"self.dep","source":"dep"}"#,
            );
            fact(
                &mut e,
                "m.py",
                "function",
                "m.Service.run",
                start + 30,
                start + 60,
                "run",
            );
            fact(
                &mut e,
                "m.py",
                "call",
                "m.Service.run",
                start + 40,
                start + 40,
                "self.dep.execute()",
            );
        }
        assert_eq!(neighbors(&e, 5), vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(neighbors(&e, 11), vec![6, 7, 8, 9, 10, 11]);
        assert_eq!(neighbors(&e, 8), vec![6, 7, 8, 9, 10, 11]);
        assert_eq!(neighbors(&e, 9), vec![6, 7, 8, 9, 10, 11]);
    }

    #[test]
    fn rebound_method_receiver_does_not_use_constructor_binding() {
        let mut e = evidence();
        fact(&mut e, "m.py", "class", "m.Service", 1, 100, "Service");
        fact(
            &mut e,
            "m.py",
            "function",
            "m.Service.__init__",
            2,
            20,
            "__init__",
        );
        fact(
            &mut e,
            "m.py",
            "binding",
            "m.Service.__init__",
            5,
            5,
            r#"{"target":"self.dep","source":"dep"}"#,
        );
        fact(&mut e, "m.py", "function", "m.Service.run", 30, 60, "run");
        fact(
            &mut e,
            "m.py",
            "binding",
            "m.Service.run",
            35,
            35,
            r#"{"target":"self","source":"Other()"}"#,
        );
        fact(
            &mut e,
            "m.py",
            "call",
            "m.Service.run",
            40,
            40,
            "self.dep.execute()",
        );
        let n = neighbors(&e, 5);
        assert!(!n.contains(&2));
        assert!(n.contains(&4));
        assert!(!neighbors(&e, 2).contains(&5));
    }

    #[test]
    fn constrained_neighborhood_budget_never_substitutes_old_binding_chain() {
        let mut e = evidence();
        fact(&mut e, "m.py", "function", "m.run", 1, 50, "run");
        fact(
            &mut e,
            "m.py",
            "binding",
            "m.run",
            5,
            5,
            r#"{"target":"client","source":"Old()"}"#,
        );
        fact(
            &mut e,
            "m.py",
            "binding",
            "m.run",
            10,
            10,
            r#"{"target":"client","source":"NewWithLargerContext()"}"#,
        );
        fact(&mut e, "m.py", "call", "m.run", 15, 15, "client.send()");
        let facts: Vec<_> = e.facts.iter().collect();
        let ids = [0, 1, 2, 3];
        let ctx = context(&facts, &ids);
        let sizes: Vec<_> = facts
            .iter()
            .map(|f| serde_json::to_vec(f).unwrap().len() + 1)
            .collect();
        // Enough for the obsolete chain, but not the current one. Fall back to
        // less context, never a cheaper, incorrect historical assignment.
        let budget = sizes[0] + sizes[1] + sizes[3];
        let selected = alternatives(3, &facts, &ids, &ctx)
            .into_iter()
            .find(|group| group.iter().map(|&j| sizes[j]).sum::<usize>() <= budget)
            .unwrap();
        assert!(!selected.contains(&1));
        assert!(!selected.contains(&2));
        assert!(selected.contains(&3));
        let p = prepare(&e).unwrap();
        check(&e, &p);
    }

    #[test]
    fn replacement_constructor_does_not_reuse_prior_constructor_bindings() {
        let mut e = evidence();
        fact(&mut e, "m.py", "class", "m.Service", 1, 100, "Service");
        fact(
            &mut e,
            "m.py",
            "function",
            "m.Service.__init__",
            2,
            20,
            "__init__",
        );
        fact(
            &mut e,
            "m.py",
            "binding",
            "m.Service.__init__",
            5,
            5,
            r#"{"target":"self.dep","source":"Old()"}"#,
        );
        fact(
            &mut e,
            "m.py",
            "function",
            "m.Service.__init__",
            30,
            40,
            "__init__",
        );
        fact(&mut e, "m.py", "function", "m.Service.run", 50, 80, "run");
        fact(
            &mut e,
            "m.py",
            "call",
            "m.Service.run",
            60,
            60,
            "self.dep.execute()",
        );
        assert_eq!(neighbors(&e, 5), vec![0, 4, 5]);
        assert!(!neighbors(&e, 2).contains(&5));
    }

    #[test]
    fn ambiguous_assignments_are_not_ordered_by_fact_id() {
        let mut e = evidence();
        fact(&mut e, "m.py", "function", "m.run", 1, 30, "run");
        fact(
            &mut e,
            "m.py",
            "binding",
            "m.run",
            5,
            5,
            r#"{"target":"client","source":"Old()"}"#,
        );
        fact(
            &mut e,
            "m.py",
            "binding",
            "m.run",
            5,
            5,
            r#"{"target":"client","source":"New()"}"#,
        );
        fact(&mut e, "m.py", "call", "m.run", 10, 10, "client.send()");
        assert_eq!(neighbors(&e, 3), vec![0, 3]);
        e.facts.swap(1, 2);
        assert_eq!(neighbors(&e, 3), vec![0, 3]);
    }

    fn replaced_constructor_container(run_first: bool) -> Evidence {
        let mut e = evidence();
        let (init, run) = if run_first { (60, 10) } else { (10, 60) };
        fact(&mut e, "m.py", "class", "m.Service", 1, 120, "Service");
        fact(
            &mut e,
            "m.py",
            "function",
            "m.Service.__init__",
            init,
            init + 30,
            "__init__",
        );
        fact(
            &mut e,
            "m.py",
            "binding",
            "m.Service.__init__",
            init + 5,
            init + 5,
            r#"{"target":"self.container.dep","source":"OldDependency()"}"#,
        );
        fact(
            &mut e,
            "m.py",
            "binding",
            "m.Service.__init__",
            init + 10,
            init + 10,
            r#"{"target":"self.container","source":"NewContainer()"}"#,
        );
        fact(
            &mut e,
            "m.py",
            "function",
            "m.Service.run",
            run,
            run + 20,
            "run",
        );
        fact(
            &mut e,
            "m.py",
            "call",
            "m.Service.run",
            run + 5,
            run + 5,
            "self.container.dep.send()",
        );
        e
    }

    #[test]
    fn constructor_ancestor_reassignment_invalidates_nested_binding_in_either_method_order() {
        for run_first in [false, true] {
            let e = replaced_constructor_container(run_first);
            let n = neighbors(&e, 5);
            assert!(
                !n.contains(&2),
                "stale nested binding with run_first={run_first}: {n:?}"
            );
            assert!(!n.contains(&1));
            assert!(n.contains(&5));
        }
    }

    #[test]
    fn router_context_kinds_are_support_only_not_global_packet_anchors() {
        let mut e = large();
        let kinds = [
            "router_prefix",
            "router_include",
            "router_host",
            "router_mutation",
        ];
        for kind in kinds {
            assert_eq!(category(kind), 4);
            fact(&mut e, "support.py", kind, "support.Router", 1, 2, "{}");
        }
        let p = prepare(&e).unwrap();
        for kind in kinds {
            assert_eq!(p.coverage.by_kind[kind].included, 0);
        }
    }

    #[test]
    fn auxiliary_flood_does_not_enter_global_seeds_or_bulk_quotas() {
        let mut e = large();
        for line in 1..=1000 {
            fact(
                &mut e,
                "support.py",
                "attribute_annotation",
                "support.SyntheticContext.client",
                line,
                line,
                "ExternalClient",
            );
            fact(
                &mut e,
                "support.py",
                "signature",
                "support.run",
                line,
                line,
                r#"{"positional":["arg"],"positionalOnly":0,"keywordOnly":[]}"#,
            );
            fact(
                &mut e,
                "support.py",
                "parameter",
                "support.run.arg",
                line,
                line,
                "arg",
            );
            fact(
                &mut e,
                "support.py",
                "binding_invalidation",
                "support.run",
                line,
                line,
                r#"{"target":"obj"}"#,
            );
        }
        let p = prepare(&e).unwrap();
        assert!(p.coverage.selection_applied);
        check(&e, &p);
        for kind in [
            "parameter",
            "signature",
            "attribute_annotation",
            "binding_invalidation",
        ] {
            assert_eq!(p.coverage.by_kind[kind].included, 0);
            assert!(!p
                .coverage
                .warnings
                .iter()
                .any(|w| w.contains(&format!("relationship kind {kind}"))));
        }
        for kind in ["call", "binding", "import"] {
            assert!(p.coverage.by_kind[kind].included > 0);
        }
    }

    #[test]
    fn stale_constructor_nested_binding_cannot_recruit_method_call() {
        for run_first in [false, true] {
            let e = replaced_constructor_container(run_first);
            assert!(!neighbors(&e, 2).contains(&5), "run_first={run_first}");
        }
    }

    #[test]
    fn invalidations_are_support_only_and_expose_no_valid_dependency() {
        for run_first in [false, true] {
            let mut e = replaced_constructor_container(run_first);
            e.facts[3].kind = "binding_invalidation".into();
            e.facts[3].value = r#"{"target":"self.container"}"#.into();
            let refs: Vec<_> = e.facts.iter().collect();
            let ctx = lexical_contexts(&refs);
            assert_eq!(ctx[&5].binding, Some(3));
            assert!(ctx[&5].neighborhood.contains(&3));
            assert!(!ctx[&5].neighborhood.contains(&2));
            assert_eq!(binding(refs[3]), Some(("self.container".into(), "".into())));
            let p = prepare(&e).unwrap();
            assert!(!p
                .coverage
                .warnings
                .iter()
                .any(|w| w.contains("relationship kind binding_invalidation")));
            assert_eq!(
                p.input["facts"][3]["value"],
                r#"{"target":"self.container"}"#
            );
        }
        assert_eq!(category("parameter"), 4);
        assert_eq!(category("binding_invalidation"), 4);
    }

    #[test]
    fn method_ancestor_reassignment_is_checked_independently_of_constructor_source_position() {
        for run_first in [false, true] {
            let mut e = replaced_constructor_container(run_first);
            e.facts[3].symbol = "m.Service.run".into();
            e.facts[3].line = e.facts[4].line + 2;
            e.facts[3].end_line = e.facts[3].line;
            let n = neighbors(&e, 5);
            assert!(!n.contains(&2), "run_first={run_first}: {n:?}");
            assert!(!neighbors(&e, 2).contains(&5));
        }
    }

    #[test]
    fn constrained_fallback_cannot_restore_stale_constructor_nested_chain() {
        for run_first in [false, true] {
            let e = replaced_constructor_container(run_first);
            let facts: Vec<_> = e.facts.iter().collect();
            let ids: Vec<_> = (0..facts.len()).collect();
            let ctx = context(&facts, &ids);
            let sizes: Vec<_> = facts
                .iter()
                .map(|f| serde_json::to_vec(f).unwrap().len() + 1)
                .collect();
            let budget = sizes[0] + sizes[5];
            let selected = alternatives(5, &facts, &ids, &ctx)
                .into_iter()
                .find(|group| group.iter().map(|&j| sizes[j]).sum::<usize>() <= budget)
                .unwrap();
            assert!(selected.contains(&5));
            assert!(!selected
                .iter()
                .any(|&j| facts[j].value.contains("OldDependency")));
            let reverse = alternatives(2, &facts, &ids, &ctx);
            assert!(reverse.iter().all(|group| !group.contains(&5)));
        }
    }
}
