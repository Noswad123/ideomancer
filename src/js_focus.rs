//! JavaScript candidates backed by explicit, atomically retained proof metadata.
use crate::analysis::{Evidence, Fact};
use crate::focus::FocusOptions;
use crate::packet::{Coverage, KindCoverage, Prepared, MAX_INPUT_BYTES};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const POLICY_VERSION: &str = "javascript-focus.v2-selected-relative-functions-1";

pub(crate) fn callable(f: &Fact) -> bool {
    matches!(f.kind.as_str(), "function" | "async_function")
}

pub(crate) struct Index<'a> {
    pub facts: Vec<&'a Fact>,
    pub owners: BTreeMap<usize, usize>,
    pub declarations: BTreeMap<usize, (usize, Value)>,
    pub calls: BTreeMap<usize, (usize, Value)>,
    pub members: Vec<(usize, usize, Value)>,
    pub(crate) guards: Vec<(usize, Value)>,
    ids: BTreeMap<&'a str, usize>,
}

impl<'a> Index<'a> {
    pub fn new(facts: Vec<&'a Fact>) -> Result<Self, String> {
        let mut unique = BTreeMap::<&str, &Fact>::new();
        let mut canonical = Vec::new();
        for f in facts {
            if let Some(previous) = unique.insert(&f.id, f) {
                if serde_json::to_vec(previous).map_err(|e| e.to_string())?
                    != serde_json::to_vec(f).map_err(|e| e.to_string())?
                {
                    return Err("Conflicting JavaScript facts share an ID".into());
                }
            } else {
                canonical.push(f);
            }
        }
        let mut s = Self {
            facts: canonical,
            owners: BTreeMap::new(),
            declarations: BTreeMap::new(),
            calls: BTreeMap::new(),
            members: vec![],
            guards: vec![],
            ids: BTreeMap::new(),
        };
        for (i, f) in s.facts.iter().enumerate() {
            s.ids.insert(&f.id, i);
            if f.line == 0 || f.end_line < f.line {
                return Err("Invalid JavaScript source span".into());
            }
        }
        for i in 0..s.facts.len() {
            let f = s.facts[i];
            if !matches!(f.kind.as_str(), "js_declaration" | "js_call" | "js_member") {
                continue;
            }
            let v: Value =
                serde_json::from_str(&f.value).map_err(|_| "Invalid JavaScript metadata")?;
            if f.kind != "js_call"
                && (v["name"].as_str().is_none_or(str::is_empty)
                    || !matches!(
                        v["visibility"].as_str(),
                        Some("public" | "private" | "protected")
                    ))
            {
                return Err("Invalid JavaScript declaration/member name or visibility".into());
            }
            let owner = s.reference(i, &v, "ownerFactId")?;
            if !(callable(s.facts[owner])
                || matches!(s.facts[owner].kind.as_str(), "class" | "module"))
                || !s.contains(owner, i)
            {
                return Err("Invalid JavaScript metadata owner/span".into());
            }
            if f.kind == "js_member" {
                if s.facts[owner].kind != "class"
                    || !matches!(
                        v["memberKind"].as_str(),
                        Some("method" | "constructor" | "field")
                    )
                {
                    return Err("Invalid JavaScript member owner/kind".into());
                }
                if !v["declarationFactId"].is_null() {
                    let d = s.reference(i, &v, "declarationFactId")?;
                    if !callable(s.facts[d]) || !s.contains(owner, d) || !s.contains(i, d) {
                        return Err("Invalid JavaScript member declaration".into());
                    }
                }
                s.members.push((i, owner, v));
                continue;
            }
            let key = if f.kind == "js_call" {
                "callFactId"
            } else {
                "declarationFactId"
            };
            let d = s.reference(i, &v, key)?;
            if !s.same_span(d, i) || !s.contains(owner, d) || d == owner {
                return Err("Invalid JavaScript referenced source span".into());
            }
            if f.kind == "js_call" {
                if s.facts[d].kind != "call" {
                    return Err("Invalid JavaScript call reference kind".into());
                }
                if !v["targetFactId"].is_null() {
                    let target = s.reference(i, &v, "targetFactId")?;
                    if !callable(s.facts[target]) {
                        return Err("Invalid JavaScript target reference kind".into());
                    }
                }
                for id in v["supportingFactIds"]
                    .as_array()
                    .ok_or("Invalid JavaScript supporting IDs")?
                {
                    let j = *s
                        .ids
                        .get(id.as_str().ok_or("Invalid JavaScript supporting ID")?)
                        .ok_or("Unknown JavaScript supporting ID")?;
                    if s.facts[j].path != f.path {
                        return Err("Cross-file JavaScript proof is unsupported".into());
                    }
                }
                if s.calls.insert(d, (i, v)).is_some() {
                    return Err("Ambiguous JavaScript call metadata".into());
                }
            } else {
                if !(callable(s.facts[d]) || s.facts[d].kind == "class")
                    || !matches!(
                        v["kind"].as_str(),
                        Some(
                            "function"
                                | "arrow"
                                | "method"
                                | "constructor"
                                | "factory"
                                | "callback"
                                | "class"
                        )
                    )
                {
                    return Err("Invalid JavaScript declaration kind".into());
                }
                if s.declarations.insert(d, (i, v)).is_some() {
                    return Err("Ambiguous JavaScript declaration metadata".into());
                }
            }
            s.owners.insert(d, owner);
        }
        for &d in s.declarations.keys() {
            s.closure(d)?;
        }
        for (i, f) in s.facts.iter().enumerate() {
            if f.kind == "js_local_call_candidate" {
                let v: Value = serde_json::from_str(&f.value)
                    .map_err(|_| "Invalid JavaScript candidate metadata")?;
                let call = s.reference(i, &v, "callFactId")?;
                let (_, canonical) = s
                    .calls
                    .get(&call)
                    .ok_or("Missing JavaScript candidate call metadata")?;
                if &v != canonical || !s.same_span(i, call) {
                    return Err("Conflicting JavaScript local candidate metadata".into());
                }
            }
            if (callable(f) || f.kind == "class") && !s.declarations.contains_key(&i) {
                return Err("Missing JavaScript source declaration metadata".into());
            }
            if f.kind == "call" && !s.calls.contains_key(&i) {
                return Err("Missing JavaScript source call metadata".into());
            }
            if f.kind == "js_binding_guard" {
                let v: Value = serde_json::from_str(&f.value)
                    .map_err(|_| "Invalid JavaScript binding metadata")?;
                if !v["declarationFactId"].is_null() {
                    let target = s.reference(i, &v, "declarationFactId")?;
                    if !callable(s.facts[target])
                        || !s.contains(i, target)
                        || !s.declarations.contains_key(&target)
                    {
                        return Err("Invalid JavaScript binding declaration reference".into());
                    }
                }
                s.guards.push((i, v));
            }
        }
        for (i, f) in s
            .facts
            .iter()
            .enumerate()
            .filter(|(_, f)| f.kind == "js_import")
        {
            let v: Value =
                serde_json::from_str(&f.value).map_err(|_| "Invalid JavaScript import metadata")?;
            let owner = s.reference(i, &v, "ownerFactId")?;
            if !(callable(s.facts[owner])
                || matches!(s.facts[owner].kind.as_str(), "class" | "module"))
                || !s.contains(owner, i)
            {
                return Err("Invalid JavaScript import owner".into());
            }
        }
        for &(i, owner, ref v) in &s.members {
            if !v["declarationFactId"].is_null() {
                let d = s.reference(i, v, "declarationFactId")?;
                if s.owners.get(&d) != Some(&owner) {
                    return Err("Conflicting JavaScript member ownership".into());
                }
            }
        }
        Ok(s)
    }

    pub(crate) fn contains(&self, owner: usize, child: usize) -> bool {
        let (o, c) = (self.facts[owner], self.facts[child]);
        o.path == c.path && o.line <= c.line && c.end_line <= o.end_line
    }
    pub(crate) fn same_span(&self, a: usize, b: usize) -> bool {
        let (a, b) = (self.facts[a], self.facts[b]);
        a.path == b.path && a.line == b.line && a.end_line == b.end_line
    }
    pub(crate) fn reference(&self, meta: usize, v: &Value, key: &str) -> Result<usize, String> {
        let id = v[key]
            .as_str()
            .ok_or_else(|| format!("Missing JavaScript {key}"))?;
        let i = *self
            .ids
            .get(id)
            .ok_or_else(|| format!("Unknown JavaScript {key}"))?;
        if self.facts[i].path != self.facts[meta].path {
            return Err("Cross-file JavaScript reference is unsupported".into());
        }
        Ok(i)
    }
    pub fn closure(&self, mut d: usize) -> Result<BTreeSet<usize>, String> {
        let mut ids = BTreeSet::new();
        loop {
            if !ids.insert(d) {
                return Err("Cyclic JavaScript ownership".into());
            }
            if self.facts[d].kind == "module" {
                break;
            }
            let (meta, _) = self
                .declarations
                .get(&d)
                .ok_or("Missing JavaScript declaration metadata")?;
            ids.insert(*meta);
            d = *self
                .owners
                .get(&d)
                .ok_or("Missing JavaScript source owner")?;
        }
        Ok(ids)
    }
    fn resolve(&self, call: usize) -> Result<(usize, BTreeSet<usize>), String> {
        let (meta, v) = self.calls.get(&call).ok_or("missing_js_call_metadata")?;
        let name = v["callee"].as_str().ok_or("missing_js_callee")?;
        if name.is_empty()
            || name.contains('.')
            || !name
                .chars()
                .enumerate()
                .all(|(i, c)| c == '_' || c == '$' || c.is_alphabetic() || i > 0 && c.is_numeric())
        {
            return Err("unsupported_receiver_this_DI_or_dynamic_dispatch".into());
        }
        if v["targetFactId"].is_null() {
            return Err(v["reason"]
                .as_str()
                .unwrap_or("external_import_CommonJS_or_unresolved_binding")
                .into());
        }
        let target = self.reference(*meta, v, "targetFactId")?;
        let (target_meta, declaration) = self
            .declarations
            .get(&target)
            .ok_or("missing_target_declaration")?;
        if matches!(
            declaration["kind"].as_str(),
            Some("method" | "constructor" | "callback" | "class")
        ) {
            return Err("unsupported_method_constructor_or_callback_target".into());
        }
        // Declaration owners describe rendered source structure, not JS lexical
        // bindings: class methods and sibling blocks must not shadow each other.
        let scopes = v["lookupScopeIds"]
            .as_array()
            .ok_or("missing_js_scope_proof_rediscover")?;
        if scopes.is_empty() || scopes.len() > 256 {
            return Err("invalid_js_lookup_scope_chain".into());
        }
        let valid_id =
            |id: &str| !id.is_empty() && id.len() <= 4096 && !id.chars().any(char::is_control);
        let mut seen = BTreeSet::new();
        for scope in scopes {
            let id = scope.as_str().ok_or("invalid_js_lookup_scope_id")?;
            if !valid_id(id) || !seen.insert(id) {
                return Err("invalid_or_duplicate_js_lookup_scope_id".into());
            }
        }
        let mut matching = Vec::new();
        let mut definitions = BTreeMap::new();
        for (i, guard) in &self.guards {
            if self.facts[*i].path != self.facts[call].path {
                continue;
            }
            let Some(scope) = guard["scopeId"].as_str() else {
                if guard["name"].as_str() == Some(name) {
                    return Err("missing_js_binding_scope_proof_rediscover".into());
                }
                continue;
            };
            if !valid_id(scope)
                || !matches!(
                    guard["scopeKind"].as_str(),
                    Some("module" | "function" | "class" | "block")
                )
                || !matches!((guard["scopeStart"].as_u64(), guard["scopeEnd"].as_u64()), (Some(start), Some(end)) if start <= end && end <= 1024 * 1024)
            {
                return Err("invalid_js_binding_scope_metadata".into());
            }
            let definition = (
                &guard["scopeKind"],
                guard["scopeStart"].as_u64().unwrap(),
                guard["scopeEnd"].as_u64().unwrap(),
            );
            if definitions
                .insert(scope, definition)
                .is_some_and(|previous| previous != definition)
            {
                return Err("inconsistent_js_binding_scope_metadata".into());
            }
            if guard["name"].as_str() == Some(name) {
                matching.push((*i, guard, scope));
            }
        }
        let mut inner = None;
        for scope in scopes {
            if let Some((_, start, end)) = definitions.get(scope.as_str().unwrap()) {
                if inner.is_some_and(|(a, b)| a < *start || b > *end) {
                    return Err("inconsistent_js_lookup_scope_spans".into());
                }
                inner = Some((*start, *end));
            }
        }
        let nearest = scopes
            .iter()
            .find_map(|scope| {
                let guards: Vec<_> = matching
                    .iter()
                    .filter(|(_, _, id)| Some(*id) == scope.as_str())
                    .collect();
                (!guards.is_empty()).then_some(guards)
            })
            .ok_or("missing_js_lexical_binding_proof_rediscover")?;
        if nearest.len() != 1 {
            return Err("ambiguous_js_lexical_binding_guards".into());
        }
        let (chosen_guard, guard, _) = *nearest[0];
        if guard["declarationFactId"].as_str() != Some(&self.facts[target].id)
            || guard["declarationCount"].as_u64() != Some(1)
            || guard["writes"].as_u64() != Some(0)
            || !matches!(
                guard["bindingKind"].as_str(),
                Some("function" | "const" | "let" | "var")
            )
            || !self.contains(chosen_guard, target)
        {
            return Err("ambiguous_shadowed_or_invalid_js_lexical_binding".into());
        }
        let mut proof = self.closure(target)?;
        proof.extend(self.closure(self.owners[&call])?);
        proof.extend([call, *meta]);
        let support: BTreeSet<_> = v["supportingFactIds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| self.ids[id.as_str().unwrap()])
            .collect();
        if !support.contains(&target)
            || !support.contains(target_meta)
            || !support.contains(&chosen_guard)
        {
            return Err("incomplete_target_proof".into());
        }
        for &i in &support {
            if i == target || i == *target_meta {
                continue;
            }
            if i != chosen_guard {
                return Err("unsupported_binding_proof_kind".into());
            }
        }
        proof.extend(support);
        Ok((target, proof))
    }
}

pub fn prepare_with_byte_limit(
    evidence: &Evidence,
    options: &FocusOptions,
    max_bytes: usize,
) -> Result<Prepared, String> {
    evidence.validate_language()?;
    if !evidence.is_javascript() {
        return Err("JavaScript focus requires JavaScript evidence".into());
    }
    if !(1..=MAX_INPUT_BYTES).contains(&max_bytes)
        || options.hops > 5
        || !(1..=2000).contains(&options.max_facts)
        || options.symbol.trim().is_empty()
    {
        return Err(
            "Focus requires a symbol, hops 0..=5, maxFacts 1..=2000 and maxBytes 1..=196608".into(),
        );
    }
    let mut facts: Vec<_> = evidence.facts.iter().collect();
    facts.sort_by_key(|f| {
        (
            &f.path, &f.symbol, f.line, f.end_line, &f.kind, &f.id, &f.value,
        )
    });
    let index = Index::new(facts)?;
    let modules = crate::js_modules::Modules::new(&index, evidence);
    if index
        .facts
        .iter()
        .any(|f| !evidence.files.iter().any(|file| file.path == f.path))
    {
        return Err("JavaScript source file provenance missing".into());
    }
    let exact: Vec<_> = index
        .facts
        .iter()
        .enumerate()
        .filter(|(_, f)| callable(f) && (f.id == options.symbol || f.symbol == options.symbol))
        .map(|(i, _)| i)
        .collect();
    let candidates = if exact.is_empty() {
        index
            .facts
            .iter()
            .enumerate()
            .filter(|(_, f)| callable(f) && f.symbol.ends_with(&format!(".{}", options.symbol)))
            .map(|(i, _)| i)
            .collect()
    } else {
        exact
    };
    if candidates.len() != 1 {
        let suggestions = index
            .facts
            .iter()
            .filter(|f| callable(f))
            .take(12)
            .map(|f| format!("{} ({}:{}, id {})", f.symbol, f.path, f.line, f.id))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(format!("Focus anchor {}: {:?}. Use an exact qualified callable symbol or fact ID. Candidates (up to 12): {suggestions}", if candidates.is_empty() {"not found"} else {"ambiguous"}, options.symbol));
    }
    let anchor = candidates[0];
    let root = index.closure(anchor)?;
    let mut chosen = root.clone();
    let mut links = Vec::<Value>::new();
    let mut unresolved = BTreeMap::<String, String>::new();
    let mut visited = BTreeSet::new();
    let mut boundaries = BTreeSet::from(["Only proven selected relative named-function imports are module candidates; package/index resolution, interop, cycles, runtime execution, DI receivers, this, prototypes and inherited methods remain unsupported".to_string()]);
    let mut bundles = Vec::new();
    let mut queue = VecDeque::from([(anchor, 0)]);
    while let Some((owner, depth)) = queue.pop_front() {
        if !visited.insert(owner) {
            continue;
        }
        for (&call, (meta, _)) in &index.calls {
            if index.owners.get(&call) != Some(&owner) {
                continue;
            }
            let mut bundle = index.closure(owner)?;
            bundle.extend([call, *meta]);
            let mut link = None;
            let mut next = None;
            let module_call = !index.calls[&call].1["importFactId"].is_null();
            let resolved = if module_call {
                modules.resolve(&index, call)
            } else {
                index.resolve(call)
            };
            match resolved {
                Ok((target, proof)) if depth < options.hops => {
                    bundle.extend(proof);
                    link = Some(if module_call {
                        json!({"sourceFactId":index.facts[call].id,"targetFactId":index.facts[target].id,"kind":"js_module_call_candidate","resolutionPolicy":crate::js_modules::POLICY,"evidenceIds":bundle.iter().map(|i| &index.facts[*i].id).collect::<Vec<_>>()})
                    } else {
                        json!({"sourceFactId":index.facts[call].id,"targetFactId":index.facts[target].id,"kind":"js_local_call_candidate","evidenceIds":bundle.iter().map(|i| &index.facts[*i].id).collect::<Vec<_>>()})
                    });
                    next = Some(target);
                }
                Ok(_) => {
                    boundaries.insert("depth_limit: target closure omitted".into());
                }
                Err(reason) => {
                    if unresolved.len() < 100 {
                        unresolved.insert(index.facts[call].id.clone(), reason);
                    } else {
                        boundaries.insert("unresolved_limit: metadata capped at 100 calls".into());
                    }
                }
            }
            if chosen.union(&bundle).count() <= options.max_facts {
                let added: BTreeSet<_> = bundle.difference(&chosen).copied().collect();
                chosen.extend(&bundle);
                if let Some(link) = link {
                    links.push(link);
                }
                if let Some(target) = next {
                    queue.push_back((target, depth + 1));
                }
                bundles.push(added);
            } else {
                boundaries.insert("fact_limit: atomic call/target proof omitted".into());
            }
        }
    }
    // Optional body details never truncate values or displace required root context.
    for (i, f) in index.facts.iter().enumerate() {
        if chosen.contains(&i)
            || callable(f)
            || matches!(f.kind.as_str(), "class" | "call")
            || f.kind.starts_with("js_")
        {
            continue;
        }
        let owners: Vec<_> = index
            .declarations
            .keys()
            .copied()
            .filter(|d| index.contains(*d, i))
            .collect();
        let nearest = owners
            .into_iter()
            .min_by_key(|d| index.facts[*d].end_line - index.facts[*d].line);
        if nearest.is_some_and(|d| visited.contains(&d)) && chosen.len() < options.max_facts {
            chosen.insert(i);
            bundles.push(BTreeSet::from([i]));
        }
    }
    loop {
        if chosen.len() > options.max_facts {
            return Err("Focus root declaration/context exceeds fact limit".into());
        }
        let paths: BTreeSet<_> = chosen
            .iter()
            .map(|i| index.facts[*i].path.as_str())
            .collect();
        let mut files: Vec<_> = evidence
            .files
            .iter()
            .filter(|f| paths.contains(f.path.as_str()))
            .collect();
        files.sort_by_key(|f| (&f.path, &f.sha256));
        let mut coverage = Coverage {
            policy_version: POLICY_VERSION.into(),
            selection_applied: true,
            input_bytes: 0,
            max_bytes,
            total_facts: index.facts.len(),
            included_facts: chosen.len(),
            omitted_facts: index.facts.len() - chosen.len(),
            total_files: evidence.files.len(),
            included_files: files.len(),
            by_kind: BTreeMap::new(),
            warnings: boundaries.iter().cloned().collect(),
        };
        coverage.warnings.push("Focused static JavaScript source candidates only; callers, omitted facts and runtime dispatch are not analyzed.".into());
        for (i, f) in index.facts.iter().enumerate() {
            let n = coverage
                .by_kind
                .entry(f.kind.clone())
                .or_insert(KindCoverage {
                    total: 0,
                    included: 0,
                    omitted: 0,
                });
            n.total += 1;
            if chosen.contains(&i) {
                n.included += 1;
            } else {
                n.omitted += 1;
            }
        }
        let retained: BTreeSet<_> = chosen.iter().map(|i| index.facts[*i].id.as_str()).collect();
        let kept_links: Vec<_> = links
            .iter()
            .filter(|l| {
                l["evidenceIds"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|id| retained.contains(id.as_str().unwrap()))
            })
            .collect();
        let a = index.facts[anchor];
        let input = json!({"schemaVersion":evidence.schema_version,"snapshotId":evidence.snapshot_id,"extractorVersion":evidence.extractor_version,
            "language":evidence.language,"complete":evidence.complete,"diagnostics":evidence.diagnostics,"files":files,
            "facts":chosen.iter().map(|i|index.facts[*i]).collect::<Vec<_>>(),
            "focus":{"policyVersion":POLICY_VERSION,"requestedSymbol":options.symbol,"anchor":{"id":a.id,"symbol":a.symbol,"path":a.path,"line":a.line,"endLine":a.end_line},
                "maxHops":options.hops,"maxFacts":options.max_facts,"links":kept_links,"visitedSymbols":visited.iter().filter(|i|chosen.contains(i)).map(|i| &index.facts[*i].symbol).collect::<Vec<_>>(),
                "unresolved":unresolved.iter().filter(|(id,_)|retained.contains(id.as_str())).map(|(id,reason)|json!({"factId":id,"reason":reason})).collect::<Vec<_>>(),
                "truncated":chosen.len()<index.facts.len(),"boundaries":boundaries},
            "selection":{"policyVersion":POLICY_VERSION,"totalFacts":coverage.total_facts,"includedFacts":coverage.included_facts,"omittedFacts":coverage.omitted_facts,"maxBytes":max_bytes,"byKind":coverage.by_kind,"warnings":coverage.warnings}});
        coverage.input_bytes = serde_json::to_vec(&input).map_err(|e| e.to_string())?.len();
        if coverage.input_bytes <= max_bytes {
            return Ok(Prepared { input, coverage });
        }
        boundaries.insert("byte_limit: atomic proof bundles omitted to fit compact JSON".into());
        let Some(bundle) = bundles.pop() else {
            return Err("Focus root declaration/context and metadata cannot fit byte limit".into());
        };
        for i in bundle {
            if !root.contains(&i) {
                chosen.remove(&i);
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::analysis::{LanguageMetadata, SourceFile};

    pub(crate) fn add(
        e: &mut Evidence,
        id: &str,
        kind: &str,
        symbol: &str,
        line: usize,
        end: usize,
        value: Value,
    ) {
        e.facts.push(Fact {
            id: id.into(),
            kind: kind.into(),
            symbol: symbol.into(),
            path: "app.ts".into(),
            line,
            end_line: end,
            value: value
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| value.to_string()),
        });
    }

    pub(crate) fn source() -> Evidence {
        let mut e = Evidence {
            schema_version: "ideomancer.evidence.v2".into(),
            snapshot_id: "js-fixture".into(),
            extractor_version: crate::javascript::EXTRACTOR_VERSION.into(),
            python_version: String::new(),
            language: Some(LanguageMetadata {
                name: "javascript-typescript".into(),
                parser: "typescript".into(),
                parser_version: "5.9.3".into(),
                parser_sha256: "a".repeat(64),
                runtime: "node".into(),
                runtime_version: "v24.0.0".into(),
                scan_policy: crate::javascript::SCAN_POLICY.into(),
            }),
            files: vec![SourceFile {
                path: "app.ts".into(),
                sha256: "b".repeat(64),
            }],
            facts: vec![],
            diagnostics: vec![],
            complete: true,
        };
        add(&mut e, "module", "module", "app", 1, 100, json!("app"));
        add(&mut e, "run", "function", "app.$run", 2, 20, json!("$run"));
        add(
            &mut e,
            "run-meta",
            "js_declaration",
            "app.$run",
            2,
            20,
            json!({"declarationFactId":"run","ownerFactId":"module","kind":"function","name":"$run","publicCandidate":true,"visibility":"public"}),
        );
        add(
            &mut e,
            "work",
            "function",
            "app.work",
            30,
            40,
            json!("work"),
        );
        add(
            &mut e,
            "work-meta",
            "js_declaration",
            "app.work",
            30,
            40,
            json!({"declarationFactId":"work","ownerFactId":"module","kind":"function","name":"work","publicCandidate":true,"visibility":"public"}),
        );
        add(
            &mut e,
            "work-guard",
            "js_binding_guard",
            "app",
            30,
            40,
            json!({"name":"work","bindingKind":"function","declarationCount":1,"writes":0,"declarationFactId":"work","scopeStart":0,"scopeEnd":1000,"scopeId":"scope-module","scopeKind":"module"}),
        );
        add(&mut e, "call", "call", "app.$run", 5, 5, json!("work()"));
        add(
            &mut e,
            "call-meta",
            "js_call",
            "app.$run",
            5,
            5,
            json!({"callFactId":"call","ownerFactId":"run","callee":"work","targetFactId":"work","supportingFactIds":["work","work-meta","work-guard"],"lookupScopeIds":["scope-run-body","scope-run","scope-module"],"reason":"unique stable same-file lexical function binding"}),
        );
        add(
            &mut e,
            "this-call",
            "call",
            "app.$run",
            6,
            6,
            json!("this.client.get()"),
        );
        add(
            &mut e,
            "this-meta",
            "js_call",
            "app.$run",
            6,
            6,
            json!({"callFactId":"this-call","ownerFactId":"run","callee":"this.client.get","supportingFactIds":[],"lookupScopeIds":["scope-run-body","scope-run","scope-module"],"reason":"UNKNOWN: no proven local lexical function binding"}),
        );
        e
    }

    fn options(max_facts: usize) -> FocusOptions {
        FocusOptions {
            symbol: "$run".into(),
            hops: 2,
            max_facts,
        }
    }

    #[test]
    fn explicit_local_proofs_owners_and_unsupported_receivers() {
        let e = source();
        let p = prepare_with_byte_limit(&e, &options(100), MAX_INPUT_BYTES).unwrap();
        assert!(p.input.get("pythonVersion").is_none());
        assert_eq!(p.input["language"]["name"], "javascript-typescript");
        assert_eq!(p.input["focus"]["links"].as_array().unwrap().len(), 1);
        assert_eq!(
            p.input["focus"]["links"][0]["kind"],
            "js_local_call_candidate"
        );
        assert!(p.input["focus"]["unresolved"]
            .to_string()
            .contains("unsupported_receiver"));
        assert_eq!(
            p.coverage.input_bytes,
            serde_json::to_vec(&p.input).unwrap().len()
        );
        assert_eq!(
            p.coverage.included_facts + p.coverage.omitted_facts,
            e.facts.len()
        );
    }

    #[test]
    fn byte_and_fact_caps_keep_root_and_whole_link_proofs() {
        let e = source();
        let full = prepare_with_byte_limit(&e, &options(100), MAX_INPUT_BYTES).unwrap();
        for cap in [3, 4, 7, 9, 100] {
            for bytes in [2500, 3500, full.coverage.input_bytes, MAX_INPUT_BYTES] {
                if let Ok(p) = prepare_with_byte_limit(&e, &options(cap), bytes) {
                    let ids: BTreeSet<_> = p.input["facts"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|f| f["id"].as_str().unwrap())
                        .collect();
                    assert!(["run", "run-meta", "module"]
                        .iter()
                        .all(|id| ids.contains(id)));
                    for link in p.input["focus"]["links"].as_array().unwrap() {
                        assert!(ids.contains(link["targetFactId"].as_str().unwrap()));
                        assert!(link["evidenceIds"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .all(|id| ids.contains(id.as_str().unwrap())));
                    }
                    assert!(p.coverage.input_bytes <= bytes && ids.len() <= cap);
                }
            }
        }
        assert!(prepare_with_byte_limit(&e, &options(2), MAX_INPUT_BYTES).is_err());
        assert!(prepare_with_byte_limit(&e, &options(100), 100).is_err());
    }

    #[test]
    fn bogus_references_ambiguity_and_modified_guards_fail_closed() {
        let mut e = source();
        let guard = e.facts.iter_mut().find(|f| f.id == "work-guard").unwrap();
        let mut v: Value = serde_json::from_str(&guard.value).unwrap();
        v["writes"] = json!(1);
        guard.value = v.to_string();
        assert!(prepare_with_byte_limit(&e, &options(100), MAX_INPUT_BYTES)
            .unwrap()
            .input["focus"]["links"]
            .as_array()
            .unwrap()
            .is_empty());
        for key in ["ownerFactId", "callFactId", "targetFactId"] {
            let mut e = source();
            let f = e.facts.iter_mut().find(|f| f.id == "call-meta").unwrap();
            let mut v: Value = serde_json::from_str(&f.value).unwrap();
            v[key] = json!("missing");
            f.value = v.to_string();
            assert!(prepare_with_byte_limit(&e, &options(100), MAX_INPUT_BYTES).is_err());
        }
        let mut e = source();
        add(
            &mut e,
            "other-work",
            "function",
            "app.other",
            50,
            60,
            json!("work"),
        );
        add(
            &mut e,
            "other-meta",
            "js_declaration",
            "app.other",
            50,
            60,
            json!({"declarationFactId":"other-work","ownerFactId":"module","kind":"function","name":"work","publicCandidate":true,"visibility":"public"}),
        );
        add(
            &mut e,
            "other-guard",
            "js_binding_guard",
            "app",
            50,
            60,
            json!({"name":"work","bindingKind":"function","declarationCount":2,"writes":0,"declarationFactId":"other-work","scopeStart":0,"scopeEnd":1000,"scopeId":"scope-module","scopeKind":"module"}),
        );
        let p = prepare_with_byte_limit(&e, &options(100), MAX_INPUT_BYTES).unwrap();
        assert!(p.input["focus"]["links"].as_array().unwrap().is_empty());
        assert!(p.input["focus"]["unresolved"]
            .to_string()
            .contains("ambiguous"));
        let mut e = source();
        add(
            &mut e,
            "other-run",
            "function",
            "app.nested.$run",
            50,
            60,
            json!("$run"),
        );
        add(
            &mut e,
            "other-run-meta",
            "js_declaration",
            "app.nested.$run",
            50,
            60,
            json!({"declarationFactId":"other-run","ownerFactId":"module","kind":"function","name":"$run","publicCandidate":true,"visibility":"public"}),
        );
        assert!(prepare_with_byte_limit(&e, &options(100), MAX_INPUT_BYTES)
            .err()
            .unwrap()
            .contains("ambiguous"));
    }

    #[test]
    fn unresolved_cap_full_values_and_permutations_are_explicit() {
        let mut e = source();
        for n in 0..110 {
            let id = format!("unknown{n}");
            add(
                &mut e,
                &id,
                "call",
                "app.$run",
                7,
                7,
                json!("deps.client.get()"),
            );
            add(
                &mut e,
                &format!("unknown-meta{n}"),
                "js_call",
                "app.$run",
                7,
                7,
                json!({"callFactId":id,"ownerFactId":"run","callee":"deps.client.get","supportingFactIds":[],"lookupScopeIds":["scope-run-body","scope-run","scope-module"],"reason":"unsupported receiver"}),
            );
        }
        let p = prepare_with_byte_limit(&e, &options(1000), MAX_INPUT_BYTES).unwrap();
        assert_eq!(
            p.input["focus"]["unresolved"].as_array().unwrap().len(),
            100
        );
        assert!(p.input["focus"]["boundaries"]
            .to_string()
            .contains("unresolved_limit"));
        e.facts.reverse();
        e.files.reverse();
        assert_eq!(
            p.input,
            prepare_with_byte_limit(&e, &options(1000), MAX_INPUT_BYTES)
                .unwrap()
                .input
        );
        let mut e = source();
        e.facts.iter_mut().find(|f| f.id == "work").unwrap().value = "x".repeat(MAX_INPUT_BYTES);
        let p = prepare_with_byte_limit(&e, &options(100), MAX_INPUT_BYTES).unwrap();
        assert!(p.input["focus"]["links"].as_array().unwrap().is_empty());
        assert!(!p.input["facts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["id"] == "work"));
        assert!(p.input["focus"]["boundaries"]
            .to_string()
            .contains("byte_limit"));
    }

    #[test]
    fn lexical_scope_proof_is_required_and_nearer_noncallable_bindings_block() {
        for (field, value) in [
            ("lookupScopeIds", Value::Null),
            ("lookupScopeIds", json!(["scope-module", "scope-module"])),
            ("lookupScopeIds", json!(["other-module"])),
            ("lookupScopeIds", json!([42])),
        ] {
            let mut e = source();
            let call = e.facts.iter_mut().find(|f| f.id == "call-meta").unwrap();
            let mut v: Value = serde_json::from_str(&call.value).unwrap();
            v[field] = value;
            call.value = v.to_string();
            let p = prepare_with_byte_limit(&e, &options(100), MAX_INPUT_BYTES).unwrap();
            assert!(p.input["focus"]["links"].as_array().unwrap().is_empty());
            assert!(!p.input["focus"]["unresolved"]
                .as_array()
                .unwrap()
                .is_empty());
        }
        for (field, value) in [
            ("scopeId", Value::Null),
            ("scopeKind", json!("unknown")),
            ("bindingKind", json!("method")),
            ("declarationCount", json!(2)),
            ("scopeEnd", json!(0)),
        ] {
            let mut e = source();
            let guard = e.facts.iter_mut().find(|f| f.id == "work-guard").unwrap();
            let mut v: Value = serde_json::from_str(&guard.value).unwrap();
            v[field] = value;
            if field == "scopeEnd" {
                v["scopeStart"] = json!(1);
            }
            guard.value = v.to_string();
            assert!(prepare_with_byte_limit(&e, &options(100), MAX_INPUT_BYTES)
                .unwrap()
                .input["focus"]["links"]
                .as_array()
                .unwrap()
                .is_empty());
        }
        for kind in ["parameter", "import", "const", "let", "var", "class"] {
            let mut e = source();
            add(
                &mut e,
                "shadow",
                "js_binding_guard",
                "app.$run",
                3,
                3,
                json!({"name":"work","bindingKind":kind,"declarationFactId":null,"declarationCount":1,"writes":0,"scopeId":"scope-run","scopeKind":"function","scopeStart":1,"scopeEnd":200}),
            );
            let p = prepare_with_byte_limit(&e, &options(100), MAX_INPUT_BYTES).unwrap();
            assert!(
                p.input["focus"]["links"].as_array().unwrap().is_empty(),
                "{kind}"
            );
            assert!(p.input["focus"]["unresolved"]
                .to_string()
                .contains("shadowed"));
        }
    }

    #[test]
    fn identical_duplicate_facts_are_canonical_but_conflicting_ids_are_rejected() {
        let mut e = source();
        let expected = prepare_with_byte_limit(&e, &options(100), MAX_INPUT_BYTES).unwrap();
        let duplicates: Vec<Fact> = serde_json::from_value(json!(e.facts)).unwrap();
        e.facts.extend(duplicates);
        let actual = prepare_with_byte_limit(&e, &options(100), MAX_INPUT_BYTES).unwrap();
        assert_eq!(actual.input, expected.input);
        assert_eq!(json!(actual.coverage), json!(expected.coverage));
        e.facts.last_mut().unwrap().value.push(' ');
        assert!(prepare_with_byte_limit(&e, &options(100), MAX_INPUT_BYTES)
            .err()
            .unwrap()
            .contains("Conflicting"));
    }

    fn extract_source(code: &str) -> Option<Evidence> {
        use std::path::PathBuf;
        let Some(compiler) = std::env::var_os("IDEOMANCER_TEST_TYPESCRIPT").map(PathBuf::from)
        else {
            eprintln!("SKIP: set IDEOMANCER_TEST_TYPESCRIPT to an existing trusted compiler file");
            return None;
        };
        assert!(
            compiler.is_absolute() && compiler.is_file(),
            "Invalid trusted test compiler path"
        );
        let node = PathBuf::from(
            std::env::var_os("IDEOMANCER_TEST_NODE").unwrap_or_else(|| "node".into()),
        );
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("app.ts"), code).unwrap();
        let e = crate::javascript::extract(root.path(), &node, &compiler).unwrap();
        assert!(e.complete);
        Some(e)
    }

    #[test]
    fn extracted_scope_bindings_ground_native_targets_without_owner_name_guesses() {
        let Some(e) = extract_source(
            r#"
function helper() { return 0; }
export class Service {
  helper() { return 1; }
  run() { helper(); }
}
export function runBlocks() {
  { function helper() { return 2; } helper(); }
  { function helper() { return 3; } helper(); }
}
export function ambiguous() {
  var f = () => falseBodyOne();
  var f = () => falseBodyTwo();
  function g() { falseBodyThree(); }
  function g() { falseBodyFour(); }
  f(); g();
}
"#,
        ) else {
            return;
        };
        let module = &e.facts.iter().find(|f| f.kind == "module").unwrap().symbol;
        let catalog = crate::catalog::build(&e, "local").unwrap();
        for (suffix, count) in [("Service.run", 1), ("runBlocks", 2), ("ambiguous", 0)] {
            let item = catalog
                .items
                .iter()
                .find(|i| {
                    i.kind == crate::catalog::Kind::Workflow
                        && i.symbol.ends_with(&format!(".{suffix}"))
                })
                .unwrap();
            let native = crate::render::endpoint::build(
                &catalog,
                &e,
                item,
                &crate::render::endpoint::View::Services,
                None,
                2,
                100,
            )
            .unwrap();
            let links = native.legend["focus"]["links"].as_array().unwrap();
            assert_eq!(links.len(), count, "{suffix}");
            let targets: BTreeSet<_> = links
                .iter()
                .map(|l| l["targetFactId"].as_str().unwrap())
                .collect();
            assert_eq!(targets.len(), count, "distinct sibling bindings");
            if suffix == "runBlocks" {
                for link in links {
                    let call = e
                        .facts
                        .iter()
                        .find(|f| Some(f.id.as_str()) == link["sourceFactId"].as_str())
                        .unwrap();
                    let target = e
                        .facts
                        .iter()
                        .find(|f| Some(f.id.as_str()) == link["targetFactId"].as_str())
                        .unwrap();
                    assert_eq!(
                        call.line, target.line,
                        "each inline block call targets its own helper"
                    );
                }
            }
            for target in targets {
                let target = e.facts.iter().find(|f| f.id == target).unwrap();
                assert_eq!(
                    target.symbol,
                    if suffix == "Service.run" {
                        format!("{module}.helper")
                    } else {
                        format!("{module}.runBlocks.helper")
                    }
                );
            }
            let calls = native.legend["calls"].as_array().unwrap();
            assert!(calls.iter().all(|c| c["ownerFactId"] == item.fact_id));
            assert!(!calls
                .iter()
                .any(|c| c["label"].as_str().unwrap().starts_with("falseBody")));
            if suffix == "ambiguous" {
                assert_eq!(calls.len(), 2);
            }
            assert_eq!(native.legend["counts"]["networkGroups"], 0);
        }
    }

    #[test]
    fn extracted_single_class_with_or_without_newline_has_canonical_native_inventory() {
        for code in [
            "export class Service {}",
            "export class Service {}\n",
            "export class Service { run() {} }",
            "export class Service { run() {} }\n",
        ] {
            let Some(e) = extract_source(code) else {
                return;
            };
            let catalog = crate::catalog::build(&e, "local").unwrap();
            let page = crate::catalog::page(
                &catalog,
                &crate::catalog::Filter {
                    kind: Some(crate::catalog::Kind::Class),
                    ..Default::default()
                },
            )
            .unwrap();
            let native = crate::render::source_view::build(
                &catalog,
                &e,
                &page,
                &crate::catalog::Kind::Class,
            )
            .unwrap();
            assert_eq!(native.legend["entries"].as_array().unwrap().len(), 1);
            assert_eq!(
                native.legend["entries"][0]["methods"]
                    .as_array()
                    .unwrap()
                    .len(),
                usize::from(code.contains("run()"))
            );
            if code.contains("run()") {
                let entry = catalog
                    .items
                    .iter()
                    .find(|i| i.kind == crate::catalog::Kind::Workflow)
                    .unwrap();
                let callable = crate::render::endpoint::build(
                    &catalog,
                    &e,
                    entry,
                    &crate::render::endpoint::View::Services,
                    None,
                    1,
                    100,
                )
                .unwrap();
                assert_eq!(callable.legend["item"]["factId"], entry.fact_id);
                assert!(callable.legend["focus"]["links"]
                    .as_array()
                    .unwrap()
                    .is_empty());
            }
            assert!(e.facts.iter().all(|f| f.kind != "call"));
        }
    }

    #[test]
    fn sloppy_block_function_collisions_never_choose_a_call_or_route_target() {
        for source in [
            "function run(){function helper(){};{function helper(){}};helper();}",
            "module.exports=router=>{function handler(req){wrong();}{function handler(req){right();}}router.post('/x',handler);};",
        ] {
            let Some(e) = extract_source(source) else { return };
            assert!(e.complete);
            assert!(e.diagnostics.iter().any(|d| d.code == "javascript_annex_b_binding_unresolved"));
            assert!(!e.facts.iter().any(|f| f.kind == "api_route" || f.kind == "js_local_call_candidate"));
        }
        let Some(e) = extract_source("'use strict'; export function run(){{function helper(){} helper();}{function helper(){} helper();}}") else { return };
        let p = prepare_with_byte_limit(
            &e,
            &FocusOptions {
                symbol: "run".into(),
                hops: 1,
                max_facts: 100,
            },
            MAX_INPUT_BYTES,
        )
        .unwrap();
        assert_eq!(p.input["focus"]["links"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn extracted_typescript_and_commonjs_callbacks_ground_native_views() {
        let Some(e) = extract_source(
            r#"
function work() { return 1; }
export function $run() { work(); this.client.get(); }
export class Service extends ExternalBase {
  constructor(private dependency: Unknown) {}
  protected field: Unknown;
  #secret: Unknown;
  _public() { work(); }
  arrow = () => work();
}
interface Context { client: unknown; }
module.exports = (router, deps: Context) => router.post('/x', req => { work(); deps.client.get(); deps.client.scope().findAll(); });
"#,
        ) else {
            return;
        };
        let p = prepare_with_byte_limit(&e, &options(100), MAX_INPUT_BYTES).unwrap();
        assert_eq!(p.input["focus"]["links"].as_array().unwrap().len(), 1);
        assert!(p.input.get("pythonVersion").is_none());
        let catalog = crate::catalog::build(&e, "local").unwrap();
        let page = crate::catalog::page(
            &catalog,
            &crate::catalog::Filter {
                kind: Some(crate::catalog::Kind::Class),
                ..Default::default()
            },
        )
        .unwrap();
        let classes =
            crate::render::source_view::build(&catalog, &e, &page, &crate::catalog::Kind::Class)
                .unwrap();
        assert!(classes.mermaid.contains("+_public()"));
        assert!(classes.mermaid.contains("-dependency"));
        assert!(classes.legend["edges"].as_array().unwrap().is_empty());
        assert_eq!(
            classes.legend["entries"][0]["declaredBases"][0]["expression"],
            "ExternalBase"
        );
        let api = catalog
            .items
            .iter()
            .find(|i| i.kind == crate::catalog::Kind::Api)
            .expect("real callback handler metadata");
        let native = crate::render::endpoint::build(
            &catalog,
            &e,
            api,
            &crate::render::endpoint::View::Services,
            None,
            2,
            100,
        )
        .unwrap();
        assert_eq!(native.legend["counts"]["networkGroups"], 0);
        assert_eq!(
            native.legend["routerContext"]["status"],
            "unsupported-language"
        );
        assert!(native.legend["calls"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["ownerFactId"].as_str() == Some(&api.fact_id)
                && c["targetCandidate"]["kind"] == "js_local_call_candidate"));
        assert!(native.legend["calls"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["label"] == "deps.client.get"
                && c["targetCandidate"].is_null()
                && c["classification"] == "UNKNOWN unresolved destination"));
        assert!(native.legend["calls"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["label"] == "deps.client.scope().findAll"));
    }

    #[test]
    fn typescript_implements_is_not_a_runtime_base_class() {
        let Some(e) = extract_source("interface Contract { run(): void } export class Service implements Contract { run() {} }") else { return };
        assert!(!e.facts.iter().any(|f| f.kind == "base"));
        let catalog = crate::catalog::build(&e, "local").unwrap();
        let page = crate::catalog::page(
            &catalog,
            &crate::catalog::Filter {
                kind: Some(crate::catalog::Kind::Class),
                ..Default::default()
            },
        )
        .unwrap();
        let native =
            crate::render::source_view::build(&catalog, &e, &page, &crate::catalog::Kind::Class)
                .unwrap();
        assert!(native.legend["entries"][0]["declaredBases"]
            .as_array()
            .unwrap()
            .is_empty());
    }

    #[test]
    #[ignore = "requires explicitly provisioned TypeScript, Node, mmdc, and Chromium; no downloads"]
    fn real_mermaid_accepts_javascript_members() {
        let Some(e) = extract_source("export class Service extends ExternalBase { constructor(private dependency: unknown) {} protected field: unknown; #secret: unknown; _public() {} $execute() {} arrow = () => {}; }") else { return };
        let catalog = crate::catalog::build(&e, "local").unwrap();
        let page = crate::catalog::page(
            &catalog,
            &crate::catalog::Filter {
                kind: Some(crate::catalog::Kind::Class),
                ..Default::default()
            },
        )
        .unwrap();
        let docs =
            crate::render::source_view::build(&catalog, &e, &page, &crate::catalog::Kind::Class)
                .unwrap();
        let renderer = std::env::var_os("IDEOMANCER_MMDC").unwrap_or_else(|| "mmdc".into());
        let svg = crate::render::draft::svg(&docs.mermaid, std::path::Path::new(&renderer))
            .unwrap_or_else(|error| panic!("{error}\n{}", docs.mermaid));
        assert!(std::str::from_utf8(&svg).unwrap().contains("Service"));
    }
}
