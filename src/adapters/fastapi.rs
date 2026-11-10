//! Direct FastAPI source semantics only; no runtime imports or registration execution.
use crate::analysis::{Evidence, Fact};
use crate::catalog::Item;
use crate::router_context::{Candidate, Context, ConverterEligibility};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

fn value(f: &Fact) -> Value {
    serde_json::from_str(&f.value).unwrap_or(Value::Null)
}

fn cite(f: &Fact) -> Value {
    json!({"factId":f.id,"kind":f.kind,"symbol":f.symbol,"path":f.path,"line":f.line,"endLine":f.end_line})
}

// Resolve only a unique source declaration/import, with competing module bindings
// rejected even when they appear later (module activation order is not modeled).
struct Resolver<'a> {
    evidence: &'a Evidence,
    aliases: Option<AliasIndex<'a>>,
    failure: Option<String>,
}

impl Resolver<'_> {
    fn resolve(&mut self, at: &Fact, name: &str, proof: &mut BTreeSet<String>) -> Option<String> {
        let e = self.evidence;
        let target = resolve_name(e, at, name, proof)?;
        let aliases = self.aliases.get_or_insert_with(|| AliasIndex::new(e));
        let escaped = if let Some(escaped) = aliases.escaped.get(&target) {
            escaped.clone()
        } else {
            let escaped = aliases.candidates(&target).into_iter().find_map(|binding| {
                let v = value(binding);
                // An ambiguous escaped binding cannot establish an unmodified identity.
                let unresolved = v["source"]
                    .as_str()
                    .and_then(|source| resolve_name(e, binding, source, &mut BTreeSet::new()))
                    .is_none_or(|name| name == target || target.starts_with(&format!("{name}.")));
                unresolved.then(|| {
                    format!(
                        "Canonical identity {}: {target}; {} ({}:{})",
                        if binding.kind == "router_mutation" {
                            "imported mutation"
                        } else {
                            "alias escape"
                        },
                        binding.id,
                        binding.path,
                        binding.line
                    )
                })
            });
            aliases.escaped.insert(target.clone(), escaped.clone());
            escaped
        };
        if let Some(escaped) = escaped {
            self.failure = Some(escaped);
            None
        } else {
            Some(target)
        }
    }
}

struct AliasIndex<'a> {
    names: Vec<(&'a str, String, String)>,
    bindings: BTreeMap<(&'a str, String), Vec<&'a Fact>>,
    mutations: BTreeMap<(&'a str, String), Vec<&'a Fact>>,
    escaped: BTreeMap<String, Option<String>>,
}

impl<'a> AliasIndex<'a> {
    fn new(e: &'a Evidence) -> Self {
        let modules: BTreeMap<_, _> = e
            .facts
            .iter()
            .filter(|f| f.kind == "module")
            .map(|f| (f.path.as_str(), f))
            .collect();
        let mut index = Self {
            names: Vec::new(),
            bindings: BTreeMap::new(),
            mutations: BTreeMap::new(),
            escaped: BTreeMap::new(),
        };
        for f in &e.facts {
            if matches!(f.kind.as_str(), "class" | "function" | "async_function") {
                index.names.push((
                    &f.path,
                    f.symbol.rsplit('.').next().unwrap_or("").to_owned(),
                    f.symbol.clone(),
                ));
            }
            if matches!(f.kind.as_str(), "import" | "from_import") {
                let v = value(f);
                let Some(module) = modules.get(f.path.as_str()) else {
                    continue;
                };
                for alias in v["names"].as_array().into_iter().flatten() {
                    if let Some((local, imported)) = import_name(module, f, alias) {
                        index.names.push((&f.path, local, imported));
                    }
                }
            }
            if f.kind == "binding" {
                let v = value(f);
                if let Some(source) = v["source"].as_str().filter(|s| {
                    s.chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.'))
                }) {
                    index
                        .bindings
                        .entry((&f.path, source.to_owned()))
                        .or_default()
                        .push(f);
                }
            }
            if f.kind == "router_mutation" {
                let v = value(f);
                if let Some(target) = v["target"].as_str().filter(|s| {
                    s.chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.'))
                }) {
                    let mut prefix = target;
                    loop {
                        index
                            .mutations
                            .entry((&f.path, prefix.to_owned()))
                            .or_default()
                            .push(f);
                        let Some((parent, _)) = prefix.rsplit_once('.') else {
                            break;
                        };
                        prefix = parent;
                    }
                }
            }
        }
        index
    }

    // Only spellings of this canonical target can trigger lexical alias checks.
    fn candidates(&self, target: &str) -> Vec<&'a Fact> {
        let mut candidates = BTreeMap::new();
        for (path, local, canonical) in &self.names {
            if let Some(tail) = target
                .strip_prefix(canonical)
                .filter(|s| s.is_empty() || s.starts_with('.'))
            {
                let spelling = format!("{local}{tail}");
                if let Some(mutations) = self.mutations.get(&(*path, spelling.clone())) {
                    for mutation in mutations {
                        candidates.insert(mutation.id.as_str(), *mutation);
                    }
                }
                // Escaping any qualified ancestor also escapes access to this target.
                let mut prefix = spelling.as_str();
                loop {
                    if let Some(bindings) = self.bindings.get(&(*path, prefix.to_owned())) {
                        for binding in bindings {
                            candidates.insert(binding.id.as_str(), *binding);
                        }
                    }
                    if prefix == local {
                        break;
                    }
                    let Some((parent, _)) = prefix.rsplit_once('.') else {
                        break;
                    };
                    prefix = parent;
                }
            }
        }
        candidates.into_values().collect()
    }
}

fn import_name(module: &Fact, import: &Fact, alias: &Value) -> Option<(String, String)> {
    let original = alias["name"].as_str()?;
    if original == "*" {
        return None;
    }
    let local = alias["asname"]
        .as_str()
        .unwrap_or(if import.kind == "import" {
            original.split('.').next()?
        } else {
            original
        });
    let target = if import.kind == "import" {
        if alias["asname"].is_string() {
            original
        } else {
            local
        }
        .to_owned()
    } else {
        let v = value(import);
        let mut base = v["module"].as_str()?.to_owned();
        let level = v["level"].as_u64().unwrap_or(0) as usize;
        if level > 0 {
            let mut parts: Vec<_> = module.symbol.split('.').collect();
            if !module.path.ends_with("/__init__.py") && module.path != "__init__.py" {
                parts.pop()?;
            }
            if level > parts.len() {
                return None;
            }
            parts.truncate(parts.len() - level + 1);
            if !base.is_empty() {
                parts.push(&base);
            }
            base = parts.join(".");
        }
        format!("{base}.{original}")
    };
    Some((local.to_owned(), target))
}

fn visible_scope(e: &Evidence, at: &Fact, scope: &str, binding_line: usize) -> bool {
    e.facts.iter().any(|f| {
        f.path == at.path
            && f.symbol == scope
            && (f.kind == "module"
                || matches!(f.kind.as_str(), "class" | "function" | "async_function")
                    && (at.symbol == scope || at.symbol.starts_with(&format!("{scope}.")))
                    && f.line <= at.line
                    && at.end_line <= f.end_line
                    && f.line <= binding_line
                    && binding_line <= f.end_line)
    })
}

fn resolve_name(
    e: &Evidence,
    at: &Fact,
    name: &str,
    proof: &mut BTreeSet<String>,
) -> Option<String> {
    let module = e
        .facts
        .iter()
        .find(|f| f.path == at.path && f.kind == "module")?;
    proof.insert(module.id.clone());
    let root = name.split('.').next()?;
    let tail = &name[root.len()..];
    if e.facts.iter().any(|f| {
        f.path == at.path
            && matches!(
                f.kind.as_str(),
                "binding" | "binding_invalidation" | "router_mutation"
            )
            && value(f)["target"].as_str().is_some_and(|t| {
                t == root
                    || t.starts_with(&format!("{root}."))
                    || name.starts_with(&format!("{t}."))
            })
            && visible_scope(e, at, &f.symbol, f.line)
    }) {
        return None;
    }
    if e.facts.iter().any(|f| {
        f.path == at.path
            && f.kind == "parameter"
            && f.value == root
            && f.symbol
                .rsplit_once('.')
                .is_some_and(|(owner, _)| visible_scope(e, at, owner, f.line))
    }) {
        return None;
    }
    let mut targets = Vec::new();
    for f in &e.facts {
        if f.path != at.path {
            continue;
        }
        if matches!(f.kind.as_str(), "class" | "function" | "async_function") {
            if let Some((owner, local)) = f.symbol.rsplit_once('.') {
                if local == root && visible_scope(e, at, owner, f.line) {
                    if owner != module.symbol {
                        return None;
                    }
                    targets.push((format!("{}{tail}", f.symbol), f.id.clone()));
                }
            }
        }
        if !matches!(f.kind.as_str(), "import" | "from_import") {
            continue;
        }
        let v = value(f);
        for a in v["names"].as_array()? {
            let (local, imported) = import_name(module, f, a)?;
            if local != root {
                continue;
            }
            if f.symbol != module.symbol {
                if visible_scope(e, at, &f.symbol, f.line) {
                    return None;
                }
                continue;
            }
            targets.push((format!("{imported}{tail}"), f.id.clone()));
        }
    }
    if targets.len() != 1 {
        return None;
    }
    let (name, id) = targets.pop()?;
    proof.insert(id);
    Some(name)
}

// Deliberately scope-insensitive: an observed possible registration anywhere may
// replace a built-in. Track lexical import/binding aliases, never execute them.
fn converter_registration_proof(e: &Evidence) -> BTreeSet<String> {
    // These are possible-name edges, not resolved runtime identities. Competing
    // declarations retain every possible registration rather than select a winner.
    let mut edges = Vec::new();
    let mut calls = Vec::new();
    let mut modules: BTreeMap<&str, Vec<&Fact>> = BTreeMap::new();
    let mut classes: BTreeMap<(&str, &str), Vec<&Fact>> = BTreeMap::new();
    for f in &e.facts {
        if f.kind == "module" {
            modules.entry(&f.path).or_default().push(f);
        } else if f.kind == "class" {
            classes.entry((&f.path, &f.symbol)).or_default().push(f);
        }
    }
    let mut seeds = BTreeMap::from([(
        "starlette.convertors.register_url_convertor".to_owned(),
        BTreeSet::new(),
    )]);
    for f in &e.facts {
        if !matches!(
            f.kind.as_str(),
            "binding" | "import" | "from_import" | "call"
        ) {
            continue;
        }
        for module in modules.get(f.path.as_str()).into_iter().flatten() {
            let mut ids = BTreeSet::from([f.id.clone(), module.id.clone()]);
            let mut owner = f.symbol.as_str();
            loop {
                ids.extend(
                    classes
                        .get(&(f.path.as_str(), owner))
                        .into_iter()
                        .flatten()
                        .map(|c| c.id.clone()),
                );
                let Some((parent, _)) = owner.rsplit_once('.') else {
                    break;
                };
                owner = parent;
            }
            let v = value(f);
            if matches!(f.kind.as_str(), "import" | "from_import") {
                for alias in v["names"].as_array().into_iter().flatten() {
                    if let Some((local, canonical)) = import_name(module, f, alias) {
                        edges.push((canonical, format!("{}.{local}", f.symbol), ids.clone()));
                    } else if alias["name"] == "*" && f.kind == "from_import" {
                        // Unknown __all__ cannot exclude a possible registration.
                        if let Some((_, canonical)) =
                            import_name(module, f, &json!({"name":"__wildcard__"}))
                        {
                            if let Some(base) = canonical.strip_suffix(".__wildcard__") {
                                edges.push((base.to_owned(), f.symbol.clone(), ids.clone()));
                            }
                        }
                    }
                }
            } else {
                let source = if f.kind == "call" {
                    f.value.split('(').next()
                } else {
                    v["source"].as_str()
                };
                let Some(source) = source else {
                    continue;
                };
                // Include lexical ancestors conservatively, including unknown
                // function scopes. No instance lookup or MRO is interpreted.
                let mut owner = f.symbol.as_str();
                loop {
                    let name = format!("{owner}.{source}");
                    if source.rsplit('.').next() == Some("register_url_convertor") {
                        seeds.entry(name.clone()).or_insert_with(|| ids.clone());
                    }
                    if f.kind == "call" {
                        calls.push((name, ids.clone()));
                    } else if let Some(target) = v["target"].as_str() {
                        edges.push((name, format!("{}.{target}", f.symbol), ids.clone()));
                    }
                    if owner == module.symbol {
                        break;
                    }
                    let Some((parent, _)) = owner.rsplit_once('.') else {
                        break;
                    };
                    owner = parent;
                }
            }
        }
    }
    let mut spellings = seeds;
    let mut traversed_proof = BTreeSet::new();
    let mut pending: Vec<_> = spellings.keys().cloned().collect();
    while let Some(name) = pending.pop() {
        let ids = spellings[&name].clone();
        for (source, target, edge_ids) in &edges {
            if let Some(tail) = name
                .strip_prefix(source)
                .filter(|t| t.is_empty() || t.starts_with('.'))
            {
                // A suspected spelling may already be seeded. Still retain the
                // observed import/binding chain that also reaches that spelling.
                traversed_proof.extend(edge_ids.iter().cloned());
                let alias = format!("{target}{tail}");
                if !spellings.contains_key(&alias) {
                    if spellings.len() >= 4096
                        || alias.len() > 2048
                        || ids.len() + edge_ids.len() > 64
                    {
                        // Alias expansion is bounded. Retain the observed graph
                        // as blocking proof rather than silently declaring safety.
                        return edges
                            .iter()
                            .flat_map(|(_, _, ids)| ids.iter().cloned())
                            .chain(calls.iter().flat_map(|(_, ids)| ids.iter().cloned()))
                            .collect();
                    }
                    let mut proof = ids.clone();
                    proof.extend(edge_ids.iter().cloned());
                    spellings.insert(alias.clone(), proof);
                    pending.push(alias);
                }
            }
        }
    }
    let mut proof = BTreeSet::new();
    for (name, call_ids) in calls {
        if let Some(ids) = spellings.get(&name) {
            proof.extend(ids.iter().cloned());
            proof.extend(call_ids);
        }
    }
    if !proof.is_empty() {
        proof.extend(traversed_proof);
    }
    proof
}

pub fn derive(evidence: &Evidence, item: &Item) -> Context {
    let mut context = Context {
        status: "unknown".into(), basis: "source-backed convention candidate; not runtime full-path or activation proof".into(),
        unknown_status: "Outer application mount and dynamic module activation UNKNOWN".into(),
        warnings: vec!["Source-syntax conventions only; outer mounts, runtime registration, reachability and activation are not proven.".into()],
        ..Context::default()
    };
    let mut proof: BTreeSet<String> = item.evidence_ids.iter().cloned().collect();
    let mut resolver = Resolver {
        evidence,
        aliases: None,
        failure: None,
    };
    let stage = "direct FastAPI route metadata";
    let outcome = (|| -> Option<Vec<String>> {
        let fragment = item.route_path.as_deref()?;
        if !fragment.is_empty() && !fragment.starts_with('/') {
            return None;
        }
        let routes: Vec<_> = evidence
            .facts
            .iter()
            .filter(|f| f.kind == "api_route" && item.evidence_ids.contains(&f.id))
            .collect();
        if routes.len() != 1 {
            return None;
        }
        let route = routes[0];
        let r = value(route);
        let receiver = r["receiver"].as_str()?;
        if receiver == "self.router" {
            return None;
        }
        {
            let prefixes: Vec<_> = evidence
                .facts
                .iter()
                .filter(|f| {
                    f.path == route.path
                        && f.kind == "router_prefix"
                        && value(f)["target"] == receiver
                })
                .collect();
            if prefixes.len() != 1 {
                return None;
            }
            let f = prefixes[0];
            proof.insert(f.id.clone());
            let v = value(f);
            if v["supported"] != true
                || resolver.resolve(f, v["constructor"].as_str()?, &mut proof)?
                    != "fastapi.APIRouter"
            {
                return None;
            }
            // Only a module-level direct router binding, with no alias/rebind/mutation.
            let module = evidence
                .facts
                .iter()
                .find(|m| m.kind == "module" && m.path == route.path)?;
            if f.symbol != module.symbol || f.end_line >= route.line {
                return None;
            }
            for binding in evidence.facts.iter().filter(|b| b.path == route.path) {
                let v = value(binding);
                if binding.kind == "binding" && v["source"] == receiver {
                    return None;
                }
                if matches!(
                    binding.kind.as_str(),
                    "class" | "function" | "async_function"
                ) {
                    if let Some((owner, local)) = binding.symbol.rsplit_once('.') {
                        if local == receiver && visible_scope(evidence, route, owner, binding.line)
                        {
                            return None;
                        }
                    }
                }
                if matches!(binding.kind.as_str(), "import" | "from_import")
                    && visible_scope(evidence, route, &binding.symbol, binding.line)
                {
                    for alias in v["names"].as_array()? {
                        let original = alias["name"].as_str()?;
                        let local =
                            alias["asname"]
                                .as_str()
                                .unwrap_or(if binding.kind == "import" {
                                    original.split('.').next()?
                                } else {
                                    original
                                });
                        if local == receiver || original == "*" {
                            return None;
                        }
                    }
                }
            }
            if evidence.facts.iter().any(|b| {
                b.path == route.path
                    && (b.kind == "router_mutation"
                        && value(b)["target"]
                            .as_str()
                            .is_some_and(|t| t.starts_with(&format!("{receiver}.")))
                        || b.kind == "parameter"
                            && b.value == receiver
                            && item.symbol.starts_with(&format!(
                                "{}.",
                                b.symbol.rsplit_once('.').map_or("", |(p, _)| p)
                            )))
            }) {
                return None;
            }
            if evidence.facts.iter().any(|b| {
                b.path == route.path
                    && matches!(b.kind.as_str(), "binding" | "binding_invalidation")
                    && b.line != f.line
                    && value(b)["target"]
                        .as_str()
                        .is_some_and(|t| t == receiver || t.starts_with(&format!("{receiver}.")))
            }) {
                return None;
            }
            let mut prefix = v["prefix"].as_str()?.to_owned();
            let includes: Vec<_> = evidence
                .facts
                .iter()
                .filter(|f| {
                    f.path == route.path
                        && f.kind == "router_include"
                        && value(f)["router"] == receiver
                })
                .collect();
            if includes.len() > 1 {
                return None;
            }
            if let Some(include) = includes.first() {
                proof.insert(include.id.clone());
                let v = value(include);
                if v["supported"] != true || include.symbol != module.symbol {
                    return None;
                }
                let host = v["receiver"].as_str()?;
                let hosts: Vec<_> = evidence
                    .facts
                    .iter()
                    .filter(|h| {
                        h.path == route.path
                            && matches!(h.kind.as_str(), "router_host" | "router_prefix")
                            && value(h)["target"] == host
                    })
                    .collect();
                if hosts.len() != 1 {
                    return None;
                }
                let h = hosts[0];
                proof.insert(h.id.clone());
                if h.symbol != module.symbol
                    || h.end_line >= include.line
                    || f.end_line >= include.line
                {
                    return None;
                }
                let hv = value(h);
                let identity = resolver.resolve(h, hv["constructor"].as_str()?, &mut proof)?;
                let outer_prefix = if h.kind == "router_host" && identity == "fastapi.FastAPI" {
                    ""
                } else if h.kind == "router_prefix"
                    && identity == "fastapi.APIRouter"
                    && hv["supported"] == true
                {
                    hv["prefix"].as_str()?
                } else {
                    return None;
                };
                if evidence.facts.iter().any(|b| {
                    b.path == route.path
                        && matches!(
                            b.kind.as_str(),
                            "binding" | "binding_invalidation" | "router_mutation"
                        )
                        && b.line != h.line
                        && value(b)["target"]
                            .as_str()
                            .is_some_and(|t| t == host || t.starts_with(&format!("{host}.")))
                }) {
                    return None;
                }
                prefix = format!("{outer_prefix}{}{prefix}", v["prefix"].as_str()?);
            }
            Some(vec![prefix])
        }
    })();
    if let Some(prefixes) = outcome {
        context.candidates = prefixes
            .into_iter()
            .map(|prefix| Candidate {
                composed_path: format!("{prefix}{}", item.route_path.as_deref().unwrap_or("")),
                prefix,
                full_path_proven: false,
            })
            .collect();
        context.status = "supported".into();
        let registrations = converter_registration_proof(evidence);
        let call_targets_complete = matches!(
            evidence.extractor_version.as_str(),
            "python-static-7-call-targets" | "python-static-8-generic-facts"
        );
        context.converter_projection = ConverterEligibility {
            status: if !call_targets_complete { "unknown" } else if registrations.is_empty() { "eligible" } else { "blocked" }.into(),
            framework: "fastapi.APIRouter".into(),
            profile: "fastapi-starlette-builtins-v1".into(),
            reason: if !call_targets_complete {
                "Rediscovery required: evidence lacks complete callee preservation; literal context association retained."
            } else if registrations.is_empty() {
                "Observed FastAPI source identity; no possible converter registration observed. Hidden runtime modifications remain UNKNOWN."
            } else {
                "Possible converter registration or alias-proof boundary observed; built-in overrides and registration scope UNKNOWN; literal matching retained."
            }.into(),
        };
        proof.extend(registrations);
    } else {
        context.unknown_status = format!("Router context unresolved at stage: {stage}; outer mount and dynamic module activation UNKNOWN");
        if let Some(failure) = resolver.failure {
            context.warnings.push(failure);
        }
        context.warnings.push(
            "Router context unresolved, unsupported or conflicting; no prefix winner selected."
                .into(),
        );
    }
    context.source_refs = evidence
        .facts
        .iter()
        .filter(|f| proof.contains(&f.id))
        .map(cite)
        .collect();
    context.source_refs.sort_by_key(Value::to_string);
    context.evidence_ids = proof.into_iter().collect();
    context
}
