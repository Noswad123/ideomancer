//! Saved-evidence-only relative module proofs. Never invokes a runtime or filesystem.
use crate::analysis::Evidence;
use crate::js_focus::{callable, Index};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub const POLICY: &str = "selected-relative-functions-1";
pub const CLASS_POLICY: &str = "selected-relative-classes-1";
const EXTENSIONS: [&str; 4] = ["js", "jsx", "ts", "tsx"];

fn saved_path(path: &str) -> bool {
    !path.is_empty()
        && !path.contains(['\\', ':', '?', '#'])
        && !path.chars().any(char::is_control)
        && path.split('/').all(|p| !matches!(p, "" | "." | ".."))
}

fn candidate_paths(from: &str, spec: &str, files: &BTreeSet<String>) -> Option<Vec<String>> {
    if !saved_path(from)
        || !(spec.starts_with("./") || spec.starts_with("../"))
        || spec.contains(['\\', ':', '?', '#'])
        || spec.chars().any(char::is_control)
        || spec.ends_with('/')
    {
        return None;
    }
    let mut parts: Vec<_> = from.split('/').collect();
    parts.pop();
    for part in spec.split('/') {
        match part {
            "." => (),
            ".." => {
                parts.pop()?;
            }
            "" => return None,
            part => parts.push(part),
        }
    }
    let path = parts.join("/");
    if !saved_path(&path) {
        return None;
    }
    if let Some((_, ext)) = parts.last()?.rsplit_once('.') {
        return (EXTENSIONS.contains(&ext) && files.contains(&path)).then_some(vec![path]);
    }
    Some(
        EXTENSIONS
            .iter()
            .map(|ext| format!("{path}.{ext}"))
            .filter(|p| files.contains(p))
            .collect(),
    )
}

fn resolve_path(from: &str, spec: &str, files: &BTreeSet<String>) -> Option<String> {
    let mut paths = candidate_paths(from, spec, files)?;
    (paths.len() == 1).then(|| paths.pop().unwrap())
}

// Iterative Kosaraju: work is bounded by the selected inventory and known edges,
// including unsafe/unsupported imports recorded only in summary dependencies.
fn cyclic_nodes(edges: &[BTreeSet<usize>]) -> BTreeSet<usize> {
    let mut reverse = vec![Vec::new(); edges.len()];
    for (a, targets) in edges.iter().enumerate() {
        for &b in targets {
            reverse[b].push(a);
        }
    }
    let mut seen = BTreeSet::new();
    let mut order = Vec::new();
    for root in 0..edges.len() {
        let mut stack = vec![(root, false)];
        while let Some((node, done)) = stack.pop() {
            if done {
                order.push(node);
            } else if seen.insert(node) {
                stack.push((node, true));
                stack.extend(edges[node].iter().rev().map(|&n| (n, false)));
            }
        }
    }
    seen.clear();
    let mut cyclic = BTreeSet::new();
    for &root in order.iter().rev() {
        if seen.contains(&root) {
            continue;
        }
        let mut component = Vec::new();
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            if seen.insert(node) {
                component.push(node);
                stack.extend(&reverse[node]);
            }
        }
        if component.len() > 1 || edges[root].contains(&root) {
            cyclic.extend(component);
        }
    }
    cyclic
}

fn identifier(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .enumerate()
            .all(|(i, c)| c == '_' || c == '$' || c.is_alphabetic() || i > 0 && c.is_numeric())
}

// Class names are parser-validated upstream. Preserve their exact representation,
// including combining marks and joiners, rather than approximating JS ID_Start.
fn class_identifier(name: &str) -> bool {
    !name.is_empty() && name.len() <= 512 && !name.chars().any(char::is_control)
}

pub struct Modules {
    enabled: bool,
    classes_enabled: bool,
    inventory_complete: bool,
    files: BTreeSet<String>,
    metadata: BTreeMap<usize, Value>,
    summaries: BTreeMap<String, (usize, usize)>,
    invalid: BTreeSet<String>,
    cyclic: BTreeSet<String>,
}

impl Modules {
    pub fn new(index: &Index<'_>, evidence: &Evidence) -> Self {
        let mut s = Self {
            enabled: crate::javascript::supports_modules(&evidence.extractor_version),
            classes_enabled: crate::javascript::supports_class_module_targets(
                &evidence.extractor_version,
            ),
            inventory_complete: evidence.complete,
            files: BTreeSet::new(),
            metadata: BTreeMap::new(),
            summaries: BTreeMap::new(),
            invalid: BTreeSet::new(),
            cyclic: BTreeSet::new(),
        };
        // Unreadable/rejected selected files may be absent from `files`, hiding
        // an extension ambiguity or a reverse dependency. Missing evidence must
        // not turn an uncertain module graph into a unique target.
        if !s.enabled || !s.inventory_complete {
            s.enabled = false;
            return s;
        }
        let summary_policy = if s.classes_enabled {
            CLASS_POLICY
        } else {
            POLICY
        };
        for file in &evidence.files {
            if !saved_path(&file.path)
                || !s.files.insert(file.path.clone())
                || file.sha256.len() != 64
                || !file.sha256.bytes().all(|b| b.is_ascii_hexdigit())
            {
                s.enabled = false;
            }
        }
        if !s.enabled {
            return s;
        }
        let mut module_owners = BTreeMap::new();
        for (i, f) in index
            .facts
            .iter()
            .enumerate()
            .filter(|(_, f)| f.kind == "module")
        {
            module_owners
                .entry(f.path.as_str())
                .and_modify(|owner| *owner = None)
                .or_insert(Some(i));
        }
        for (i, f) in index.facts.iter().enumerate() {
            if !matches!(
                f.kind.as_str(),
                "js_module_summary" | "js_module_import" | "js_module_export" | "js_import"
            ) {
                continue;
            }
            let Ok(v) = serde_json::from_str::<Value>(&f.value) else {
                s.invalid.insert(f.path.clone());
                continue;
            };
            let owner = index.reference(i, &v, "ownerFactId").ok().filter(|&owner| {
                let module = module_owners.get(f.path.as_str()).copied().flatten();
                index.contains(owner, i)
                    && if f.kind == "js_import" {
                        module.is_some()
                    } else {
                        module == Some(owner)
                    }
            });
            if owner.is_none() {
                s.invalid.insert(f.path.clone());
            }
            if f.kind == "js_module_summary" {
                if let Some(owner) = owner {
                    if !index.same_span(owner, i)
                        || v["policy"] != summary_policy
                        || !matches!(v["mode"].as_str(), Some("esm" | "commonjs" | "unknown"))
                        || v["importsSafe"].as_bool().is_none()
                        || v["exportsSafe"].as_bool().is_none()
                        || v["dependencies"].as_array().is_none_or(|deps| {
                            deps.iter().any(|d| {
                                d.as_str()
                                    .is_none_or(|d| !(d.starts_with("./") || d.starts_with("../")))
                            })
                        })
                        || s.summaries.insert(f.path.clone(), (i, owner)).is_some()
                    {
                        s.invalid.insert(f.path.clone());
                    }
                }
            }
            s.metadata.insert(i, v);
        }
        if s.files
            .iter()
            .any(|p| !s.summaries.contains_key(p) || s.invalid.contains(p))
        {
            s.enabled = false;
            return s;
        }
        let dependencies: BTreeMap<_, BTreeSet<_>> = s
            .summaries
            .iter()
            .map(|(path, (summary, _))| {
                (
                    path.as_str(),
                    s.metadata[summary]["dependencies"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|d| d.as_str().unwrap())
                        .collect(),
                )
            })
            .collect();
        // General imports include unsupported namespace/type-only bindings and
        // dynamic imports. Export records with specifiers also constrain the graph.
        // A summary may not erase any known relative dependency to conceal a cycle.
        for (&i, v) in &s.metadata {
            if let Some(spec) = v["moduleSpecifier"]
                .as_str()
                .filter(|s| s.starts_with("./") || s.starts_with("../"))
            {
                let path = &index.facts[i].path;
                if dependencies
                    .get(path.as_str())
                    .is_none_or(|deps| !deps.contains(spec))
                {
                    s.invalid.insert(path.clone());
                }
            }
        }
        // A missing/invalid summary could hide a cycle. Do not promote any module
        // proof when the selected inventory's dependency graph is incomplete.
        if s.files
            .iter()
            .any(|p| !s.summaries.contains_key(p) || s.invalid.contains(p))
        {
            s.enabled = false;
            return s;
        }
        let paths: Vec<_> = s.files.iter().cloned().collect();
        let positions: BTreeMap<_, _> = paths
            .iter()
            .enumerate()
            .map(|(i, p)| (p.clone(), i))
            .collect();
        let mut edges = vec![BTreeSet::new(); paths.len()];
        for (path, &(summary, _)) in &s.summaries {
            let Some(&from) = positions.get(path) else {
                continue;
            };
            if let Some(deps) = s.metadata[&summary]["dependencies"].as_array() {
                for dep in deps {
                    if let Some(targets) = dep
                        .as_str()
                        .and_then(|d| candidate_paths(path, d, &s.files))
                    {
                        // Ambiguous dependencies cannot link, but every possible
                        // selected edge still participates in cycle suppression.
                        for target in targets {
                            edges[from].insert(positions[&target]);
                        }
                    }
                }
            }
        }
        s.cyclic = cyclic_nodes(&edges)
            .into_iter()
            .map(|i| paths[i].clone())
            .collect();
        s
    }

    fn summary(&self, path: &str, safe: &str) -> Result<(usize, usize, &str), String> {
        if !self.inventory_complete {
            return Err("incomplete_selected_source_inventory".into());
        }
        if !self.enabled {
            return Err("module_proof_unavailable_or_legacy_extractor".into());
        }
        if self.cyclic.contains(path) {
            return Err("cyclic_module_component".into());
        }
        let &(meta, owner) = self.summaries.get(path).ok_or("missing_module_summary")?;
        let v = &self.metadata[&meta];
        if v[safe] != true {
            return Err("unsafe_module_summary".into());
        }
        let mode = v["mode"]
            .as_str()
            .filter(|m| matches!(*m, "esm" | "commonjs"))
            .ok_or("unsupported_module_format")?;
        Ok((meta, owner, mode))
    }

    // Validate all scope definitions before choosing the unique nearest guard.
    // Import and export bindings must both live at the unique module root.
    fn guard(
        &self,
        index: &Index<'_>,
        path: &str,
        name: &str,
        scopes: Option<&Value>,
    ) -> Result<(usize, Value), String> {
        let mut definitions = BTreeMap::new();
        let mut matching = BTreeMap::<String, Vec<(usize, &Value)>>::new();
        let mut roots = BTreeSet::new();
        for (i, v) in &index.guards {
            if index.facts[*i].path != path {
                continue;
            }
            let scope = v["scopeId"]
                .as_str()
                .filter(|s| !s.is_empty() && s.len() <= 4096 && !s.chars().any(char::is_control))
                .ok_or("invalid_module_guard_scope")?;
            let kind = v["scopeKind"]
                .as_str()
                .filter(|s| matches!(*s, "module" | "function" | "class" | "block"))
                .ok_or("invalid_module_guard_scope")?;
            let start = v["scopeStart"]
                .as_u64()
                .ok_or("invalid_module_guard_span")?;
            let end = v["scopeEnd"].as_u64().ok_or("invalid_module_guard_span")?;
            if start > end || end > 1024 * 1024 {
                return Err("invalid_module_guard_span".into());
            }
            let definition = (kind, start, end);
            if definitions
                .insert(scope.to_string(), definition)
                .is_some_and(|d| d != definition)
            {
                return Err("competing_module_guard_scope".into());
            }
            if kind == "module" {
                roots.insert(scope.to_string());
            }
            if v["name"].as_str() == Some(name) {
                matching.entry(scope.to_string()).or_default().push((*i, v));
            }
        }
        if roots.len() != 1 {
            return Err("ambiguous_module_root_scope".into());
        }
        let root = roots.first().unwrap();
        let (_, root_start, root_end) = definitions[root];
        if root_start >= root_end
            || definitions
                .values()
                .any(|&(_, start, end)| start < root_start || end > root_end)
        {
            return Err("inconsistent_module_root_span".into());
        }
        let lookup = if let Some(scopes) = scopes {
            let scopes = scopes
                .as_array()
                .filter(|a| !a.is_empty() && a.len() <= 256)
                .ok_or("missing_module_lookup_scopes")?;
            let mut seen = BTreeSet::new();
            let mut lookup = Vec::new();
            let mut inner = None;
            for scope in scopes {
                let scope = scope
                    .as_str()
                    .filter(|s| {
                        !s.is_empty() && s.len() <= 4096 && !s.chars().any(char::is_control)
                    })
                    .ok_or("invalid_module_lookup_scope")?;
                if !seen.insert(scope) {
                    return Err("duplicate_module_lookup_scope".into());
                }
                if let Some(&(_, start, end)) = definitions.get(scope) {
                    if inner.is_some_and(|(a, b)| a < start || b > end) {
                        return Err("inconsistent_module_lookup_spans".into());
                    }
                    inner = Some((start, end));
                }
                lookup.push(scope.to_string());
            }
            if lookup.last() != Some(root) {
                return Err("missing_module_root_lookup".into());
            }
            lookup
        } else {
            vec![root.clone()]
        };
        let (scope, candidates) = lookup
            .iter()
            .find_map(|scope| matching.get(scope).map(|m| (scope, m)))
            .ok_or("missing_module_binding_guard")?;
        if scope != root || candidates.len() != 1 {
            return Err("shadowed_or_ambiguous_module_binding".into());
        }
        let (i, v) = candidates[0];
        if v["declarationCount"].as_u64() != Some(1) || v["writes"].as_u64() != Some(0) {
            return Err("unstable_module_binding".into());
        }
        Ok((i, v.clone()))
    }

    pub fn resolve(
        &self,
        index: &Index<'_>,
        call: usize,
    ) -> Result<(usize, BTreeSet<usize>), String> {
        let (call_meta, call_v) = &index.calls[&call];
        if !call_v["targetFactId"].is_null() {
            return Err("competing_module_call_target".into());
        }
        let import = index.reference(*call_meta, call_v, "importFactId")?;
        let local = call_v["callee"].as_str().ok_or("invalid_import_name")?;
        let (target, mut proof) =
            self.resolve_binding(index, call, import, local, &call_v["lookupScopeIds"], None)?;
        let (import_guard, _) = self.guard(
            index,
            &index.facts[call].path,
            local,
            Some(&call_v["lookupScopeIds"]),
        )?;
        let support = call_v["supportingFactIds"]
            .as_array()
            .ok_or("missing_module_call_support")?;
        if support.len() != 2
            || !support.iter().any(|s| s == &index.facts[import].id)
            || !support.iter().any(|s| s == &index.facts[import_guard].id)
        {
            return Err("incomplete_module_call_support".into());
        }
        proof.extend(index.closure(index.owners[&call])?);
        proof.extend([call, *call_meta]);
        Ok((target, proof))
    }

    pub fn resolve_class(
        &self,
        index: &Index<'_>,
        source: usize,
        import: usize,
        guard: usize,
        local: &str,
        lookup_scopes: &Value,
    ) -> Result<(usize, BTreeSet<usize>), String> {
        if !self.classes_enabled {
            return Err("class_module_proof_unavailable_or_legacy_extractor".into());
        }
        if index
            .facts
            .get(source)
            .is_none_or(|f| f.kind != "js_class_relation")
        {
            return Err("invalid_class_module_source".into());
        }
        let relation: Value = serde_json::from_str(&index.facts[source].value)
            .map_err(|_| "invalid_class_module_source")?;
        if !relation["targetFactId"].is_null()
            || index.reference(source, &relation, "importFactId")? != import
        {
            return Err("conflicting_class_module_import_identity".into());
        }
        let class = index.reference(source, &relation, "classFactId")?;
        if index.facts[class].kind != "class" || !index.contains(class, source) {
            return Err("invalid_class_module_source".into());
        }
        let (target, mut proof) =
            self.resolve_binding(index, source, import, local, lookup_scopes, Some(guard))?;
        let import_v = &self.metadata[&import];
        if import_v["mode"] == "commonjs" {
            // A valid import receipt can be attached to an earlier reference.
            // Compare independent byte offsets, not relation metadata or lines.
            let reference = index.reference(source, &relation, "referenceFactId")?;
            if index.facts[reference].kind != "js_class_reference" {
                return Err("invalid_class_module_reference".into());
            }
            let reference_v: Value = serde_json::from_str(&index.facts[reference].value)
                .map_err(|_| "invalid_class_module_reference")?;
            let start = reference_v["start"]
                .as_u64()
                .filter(|&n| n <= 1024 * 1024)
                .ok_or("missing_or_invalid_commonjs_import_order_proof")?;
            let end = import_v["initializationEnd"]
                .as_u64()
                .filter(|&n| n <= 1024 * 1024)
                .ok_or("missing_or_invalid_commonjs_import_order_proof")?;
            let guard_v = &index
                .guards
                .iter()
                .find(|(i, _)| *i == guard)
                .ok_or("invalid_import_binding_guard")?
                .1;
            if guard_v["initializationEnd"].as_u64() != Some(end) || end > start {
                return Err("uninitialized_or_unproven_commonjs_import_binding".into());
            }
            proof.insert(reference);
        }
        proof.extend(index.closure(class)?);
        proof.insert(source);
        Ok((target, proof))
    }

    fn resolve_binding(
        &self,
        index: &Index<'_>,
        source: usize,
        import: usize,
        local: &str,
        lookup_scopes: &Value,
        class_guard: Option<usize>,
    ) -> Result<(usize, BTreeSet<usize>), String> {
        let is_class = class_guard.is_some();
        let valid_name = if is_class {
            class_identifier
        } else {
            identifier
        };
        let path = &index.facts[source].path;
        let (summary, owner, mode) = self.summary(path, "importsSafe")?;
        if index
            .facts
            .get(import)
            .is_none_or(|f| f.kind != "js_module_import" || f.path != *path)
        {
            return Err("invalid_module_import_kind".into());
        }
        let v = self
            .metadata
            .get(&import)
            .ok_or("missing_module_import_metadata")?;
        let import_local = v["local"]
            .as_str()
            .filter(|n| valid_name(n))
            .ok_or("invalid_import_name")?;
        let imported = v["imported"]
            .as_str()
            .filter(|n| valid_name(n) && !matches!(*n, "default" | "*"))
            .ok_or("unsupported_import_name")?;
        if import_local != local
            || v["mode"] != mode
            || index.reference(import, v, "ownerFactId")? != owner
        {
            return Err("conflicting_module_import_identity".into());
        }
        if self
            .metadata
            .iter()
            .filter(|(i, m)| {
                index.facts[**i].kind == "js_module_import"
                    && index.facts[**i].path == *path
                    && m["local"] == local
            })
            .count()
            != 1
        {
            return Err("competing_module_imports".into());
        }
        let (import_guard, guard) = self.guard(index, path, local, Some(lookup_scopes))?;
        if index.reference(import, v, "bindingGuardFactId")? != import_guard
            || class_guard.is_some_and(|g| g != import_guard)
            || !guard["declarationFactId"].is_null()
            || guard["bindingKind"] != if mode == "esm" { "import" } else { "const" }
            || !index.contains(import_guard, import)
        {
            return Err("invalid_import_binding_guard".into());
        }
        let spec = v["moduleSpecifier"]
            .as_str()
            .ok_or("missing_relative_specifier")?;
        if !self.metadata[&summary]["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d == spec)
        {
            return Err("missing_module_dependency_proof".into());
        }
        let target_path = resolve_path(path, spec, &self.files)
            .ok_or("relative_module_missing_ambiguous_or_unsupported")?;
        let (target_summary, target_owner, target_mode) =
            self.summary(&target_path, "exportsSafe")?;
        if mode != target_mode {
            return Err("module_format_interop_unsupported".into());
        }
        let exports: Vec<_> = self
            .metadata
            .iter()
            .filter(|(i, m)| {
                index.facts[**i].kind == "js_module_export"
                    && index.facts[**i].path == target_path
                    && m["exported"] == imported
            })
            .collect();
        if exports.len() != 1 {
            return Err("missing_or_competing_module_exports".into());
        }
        let (&export, ex) = exports[0];
        let target = index.reference(export, ex, "targetFactId")?;
        let (target_meta, decl) = index
            .declarations
            .get(&target)
            .ok_or("missing_module_target_declaration")?;
        let name = ex["local"]
            .as_str()
            .filter(|n| valid_name(n))
            .ok_or("invalid_export_name")?;
        if ex["mode"] != mode
            || index.reference(export, ex, "ownerFactId")? != target_owner
            || index.reference(export, ex, "targetMetadataFactId")? != *target_meta
            || if is_class {
                ex["targetKind"] != "class"
                    || index.facts[target].kind != "class"
                    || decl["kind"] != "class"
            } else {
                !callable(index.facts[target])
                    || !matches!(decl["kind"].as_str(), Some("function" | "arrow"))
                    || (self.classes_enabled || ex.get("targetKind").is_some())
                        && ex["targetKind"] != "function"
            }
            || decl["name"] != name
            || index.owners.get(&target) != Some(&target_owner)
        {
            return Err("invalid_module_export_target".into());
        }
        let (export_guard, guard) = self.guard(index, &target_path, name, None)?;
        if index.reference(export, ex, "bindingGuardFactId")? != export_guard
            || if is_class {
                guard["bindingKind"] != "class"
                    || !guard["declarationFactId"].is_null()
                    || guard["initializationEnd"]
                        .as_u64()
                        .is_none_or(|end| end > 1024 * 1024)
            } else {
                index.reference(export_guard, &guard, "declarationFactId")? != target
                    || !matches!(
                        guard["bindingKind"].as_str(),
                        Some("function" | "const" | "let" | "var")
                    )
            }
            || !index.contains(export_guard, target)
        {
            return Err("invalid_export_binding_guard".into());
        }
        if mode == "commonjs" {
            let reference_start = ex["referenceStart"]
                .as_u64()
                .filter(|&n| n <= 1024 * 1024)
                .ok_or("missing_or_invalid_commonjs_export_order_proof")?;
            if ex["initializedBeforeExport"] != true
                || matches!(
                    guard["bindingKind"].as_str(),
                    Some("class" | "const" | "let" | "var")
                ) && guard["initializationEnd"]
                    .as_u64()
                    .is_none_or(|end| end > reference_start)
            {
                return Err("uninitialized_or_unproven_commonjs_export_binding".into());
            }
        }
        let mut proof = index.closure(target)?;
        proof.extend([
            summary,
            import,
            import_guard,
            target_summary,
            export,
            export_guard,
        ]);
        Ok((target, proof))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::SourceFile;
    use crate::focus::FocusOptions;
    use crate::js_focus::{prepare_with_byte_limit, tests::add};
    use crate::packet::MAX_INPUT_BYTES;
    use serde_json::json;

    fn source() -> Evidence {
        let mut e = crate::js_focus::tests::source();
        for f in &mut e.facts {
            if matches!(f.id.as_str(), "work" | "work-meta" | "work-guard") {
                f.path = "lib/work.ts".into();
                f.symbol = "lib.work.work".into();
            }
        }
        mutate(&mut e, "work-meta", "ownerFactId", json!("target-module"));
        mutate(&mut e, "call-meta", "targetFactId", Value::Null);
        mutate(&mut e, "call-meta", "importFactId", json!("import"));
        mutate(
            &mut e,
            "call-meta",
            "supportingFactIds",
            json!(["import", "import-guard"]),
        );
        e.files.push(SourceFile {
            path: "lib/work.ts".into(),
            sha256: "c".repeat(64),
        });
        add(
            &mut e,
            "target-module",
            "module",
            "lib.work",
            1,
            100,
            json!("lib.work"),
        );
        e.facts.last_mut().unwrap().path = "lib/work.ts".into();
        add(
            &mut e,
            "summary",
            "js_module_summary",
            "app",
            1,
            100,
            json!({"ownerFactId":"module","policy":CLASS_POLICY,"mode":"esm","importsSafe":true,"exportsSafe":true,"dependencies":["./lib/work"]}),
        );
        add(
            &mut e,
            "target-summary",
            "js_module_summary",
            "lib.work",
            1,
            100,
            json!({"ownerFactId":"target-module","policy":CLASS_POLICY,"mode":"esm","importsSafe":true,"exportsSafe":true,"dependencies":[]}),
        );
        e.facts.last_mut().unwrap().path = "lib/work.ts".into();
        add(
            &mut e,
            "import",
            "js_module_import",
            "app",
            1,
            1,
            json!({"ownerFactId":"module","moduleSpecifier":"./lib/work","mode":"esm","imported":"work","local":"work","bindingGuardFactId":"import-guard"}),
        );
        add(
            &mut e,
            "import-guard",
            "js_binding_guard",
            "app",
            1,
            1,
            json!({"name":"work","bindingKind":"import","declarationCount":1,"writes":0,"declarationFactId":null,"scopeStart":0,"scopeEnd":1000,"scopeId":"scope-module","scopeKind":"module"}),
        );
        add(
            &mut e,
            "export",
            "js_module_export",
            "lib.work",
            30,
            40,
            json!({"ownerFactId":"target-module","mode":"esm","exported":"work","local":"work","targetFactId":"work","targetMetadataFactId":"work-meta","bindingGuardFactId":"work-guard","targetKind":"function"}),
        );
        e.facts.last_mut().unwrap().path = "lib/work.ts".into();
        e
    }

    fn mutate(e: &mut Evidence, id: &str, field: &str, value: Value) {
        let fact = e.facts.iter_mut().find(|f| f.id == id).unwrap();
        let mut v: Value = serde_json::from_str(&fact.value).unwrap();
        v[field] = value;
        fact.value = v.to_string();
    }

    fn class_source(commonjs: bool) -> Evidence {
        let mut e = source();
        e.extractor_version = crate::javascript::STRUCTURAL_EXTRACTOR_VERSION.into();
        for id in ["run", "work"] {
            e.facts.iter_mut().find(|f| f.id == id).unwrap().kind = "class".into();
            mutate(&mut e, &format!("{id}-meta"), "kind", json!("class"));
        }
        mutate(&mut e, "work-guard", "declarationFactId", Value::Null);
        mutate(&mut e, "work-guard", "bindingKind", json!("class"));
        mutate(&mut e, "work-guard", "initializationEnd", json!(600));
        mutate(&mut e, "export", "targetKind", json!("class"));
        // Both ends use aliases; neither local name is an export lookup key.
        mutate(&mut e, "export", "exported", json!("PublicClass"));
        mutate(&mut e, "import", "imported", json!("PublicClass"));
        mutate(&mut e, "import", "local", json!("LocalClass"));
        mutate(&mut e, "import-guard", "name", json!("LocalClass"));
        add(
            &mut e,
            "relation",
            "js_class_relation",
            "app.$run",
            5,
            5,
            json!({"classFactId":"run","ownerFactId":"run","importFactId":"import","targetFactId":null,"referenceFactId":"reference"}),
        );
        add(
            &mut e,
            "reference",
            "js_class_reference",
            "app.$run",
            5,
            5,
            json!({"classFactId":"run","ownerFactId":"run","start":100,"end":110}),
        );
        if commonjs {
            for id in ["summary", "target-summary", "import", "export"] {
                mutate(&mut e, id, "mode", json!("commonjs"));
            }
            mutate(&mut e, "import-guard", "bindingKind", json!("const"));
            mutate(&mut e, "import-guard", "initializationEnd", json!(100));
            mutate(&mut e, "import", "initializationEnd", json!(100));
            mutate(&mut e, "export", "referenceStart", json!(600));
            mutate(&mut e, "export", "initializedBeforeExport", json!(true));
        }
        e
    }

    fn resolve_class_fixture(e: &Evidence) -> Result<BTreeSet<String>, String> {
        let index = Index::new(e.facts.iter().collect())?;
        let pos = |id: &str| index.facts.iter().position(|f| f.id == id).unwrap();
        let (target, proof) = Modules::new(&index, e).resolve_class(
            &index,
            pos("relation"),
            pos("import"),
            pos("import-guard"),
            "LocalClass",
            &json!(["scope-run", "scope-module"]),
        )?;
        assert_eq!(index.facts[target].id, "work");
        Ok(proof.iter().map(|&i| index.facts[i].id.clone()).collect())
    }

    #[test]
    fn named_class_aliases_have_atomic_kind_specific_proofs() {
        for commonjs in [false, true] {
            let e = class_source(commonjs);
            let mut proof = resolve_class_fixture(&e).unwrap();
            if commonjs {
                assert!(proof.remove("reference"));
            }
            assert_eq!(
                proof,
                [
                    "run",
                    "run-meta",
                    "module",
                    "relation",
                    "summary",
                    "import",
                    "import-guard",
                    "target-module",
                    "target-summary",
                    "export",
                    "work",
                    "work-meta",
                    "work-guard"
                ]
                .map(str::to_string)
                .into_iter()
                .collect()
            );
            let index = Index::new(e.facts.iter().collect()).unwrap();
            let call = index.facts.iter().position(|f| f.id == "call").unwrap();
            // Even an otherwise matching import may not turn a class into a call target.
            let mut e = e;
            mutate(&mut e, "call-meta", "callee", json!("LocalClass"));
            let index = Index::new(e.facts.iter().collect()).unwrap();
            assert_eq!(
                Modules::new(&index, &e).resolve(&index, call).unwrap_err(),
                "invalid_module_export_target"
            );
        }
    }

    #[test]
    fn commonjs_class_imports_require_independent_reference_order() {
        let e = class_source(true);
        // Equality is valid, even when all receipts share the same source line.
        assert!(resolve_class_fixture(&e).is_ok());
        for (id, field, value) in [
            ("reference", "start", json!(99)),
            ("reference", "start", Value::Null),
            ("reference", "start", json!(-1)),
            ("reference", "start", json!(1.5)),
            ("reference", "start", json!(1024 * 1024 + 1)),
            ("relation", "referenceFactId", Value::Null),
            ("relation", "referenceFactId", json!("import")),
            ("import", "initializationEnd", Value::Null),
            ("import", "initializationEnd", json!(-1)),
            ("import", "initializationEnd", json!(1.5)),
            ("import", "initializationEnd", json!(1024 * 1024 + 1)),
            ("import", "initializationEnd", json!(99)),
            ("import-guard", "initializationEnd", Value::Null),
            ("import-guard", "initializationEnd", json!(-1)),
            ("import-guard", "initializationEnd", json!(1.5)),
            ("import-guard", "initializationEnd", json!(1024 * 1024 + 1)),
            ("import-guard", "initializationEnd", json!(99)),
        ] {
            let mut e = class_source(true);
            mutate(&mut e, id, field, value);
            // A forged source-point hint on the relation is not order evidence.
            mutate(&mut e, "relation", "start", json!(1000));
            assert!(resolve_class_fixture(&e).is_err(), "{id}.{field}");
        }
        let mut e = class_source(true);
        mutate(&mut e, "import", "initializationEnd", json!(101));
        mutate(&mut e, "import-guard", "initializationEnd", json!(101));
        assert_eq!(
            resolve_class_fixture(&e).unwrap_err(),
            "uninitialized_or_unproven_commonjs_import_binding"
        );
        // Named ESM imports are hoisted, and require no initialization receipts.
        let mut e = class_source(false);
        mutate(&mut e, "reference", "start", json!(0));
        assert!(resolve_class_fixture(&e).is_ok());
    }

    #[test]
    fn incomplete_source_inventory_cannot_establish_unique_module_targets() {
        let mut e = class_source(false);
        e.complete = false;
        assert_eq!(
            resolve_class_fixture(&e).unwrap_err(),
            "incomplete_selected_source_inventory"
        );

        let mut e = source();
        e.complete = false;
        let index = Index::new(e.facts.iter().collect()).unwrap();
        let call = index.facts.iter().position(|f| f.id == "call").unwrap();
        assert_eq!(
            Modules::new(&index, &e).resolve(&index, call).unwrap_err(),
            "incomplete_selected_source_inventory"
        );
    }

    #[test]
    fn class_module_mutations_and_incomplete_snapshots_fail_closed() {
        for (id, field, value) in [
            ("export", "targetKind", Value::Null),
            ("export", "targetKind", json!("function")),
            ("export", "local", json!("wrong")),
            ("export", "targetMetadataFactId", json!("run-meta")),
            ("export", "initializedBeforeExport", json!(false)),
            ("export", "referenceStart", json!(599)),
            ("work-guard", "initializationEnd", Value::Null),
            ("work-guard", "bindingKind", json!("const")),
            ("work-guard", "writes", json!(1)),
            ("import-guard", "writes", json!(1)),
            ("import", "bindingGuardFactId", json!("module")),
            ("import", "imported", json!("default")),
            ("import", "imported", json!("*")),
            ("import", "moduleSpecifier", json!("package")),
            ("import", "moduleSpecifier", json!("./missing")),
            ("target-summary", "mode", json!("esm")),
            ("target-summary", "dependencies", json!(["../app"])),
            ("target-summary", "dependencies", json!(false)),
            ("summary", "importsSafe", json!(false)),
            ("relation", "importFactId", json!("work")),
        ] {
            let mut e = class_source(true);
            mutate(&mut e, id, field, value);
            assert!(resolve_class_fixture(&e).is_err(), "{id}.{field}");
        }
        for id in ["summary", "target-summary", "work-guard"] {
            let mut e = class_source(false);
            e.facts.retain(|f| f.id != id);
            assert!(resolve_class_fixture(&e).is_err(), "missing {id}");
        }
        for version in [
            "javascript-typescript-static-2-modules",
            "javascript-typescript-static-3-operations",
            "javascript-typescript-static-4-class-relations",
        ] {
            let mut e = class_source(false);
            e.extractor_version = version.into();
            for id in ["summary", "target-summary"] {
                mutate(&mut e, id, "policy", json!(POLICY));
            }
            assert!(resolve_class_fixture(&e).unwrap_err().contains("legacy"));
        }
        // ESM live bindings do not use the export statement's evaluation order.
        let mut e = class_source(false);
        mutate(&mut e, "export", "referenceStart", json!(1));
        mutate(&mut e, "export", "initializedBeforeExport", json!(false));
        assert!(resolve_class_fixture(&e).is_ok());
    }

    #[test]
    fn class_proofs_preserve_unicode_and_reject_ambiguity() {
        let mut e = class_source(false);
        // The exact guard/declaration spelling is authoritative, not a Rust
        // approximation of JavaScript identifier categories.
        let name = "A\u{301}\u{200d}";
        for (id, field) in [
            ("export", "local"),
            ("work-meta", "name"),
            ("work-guard", "name"),
        ] {
            mutate(&mut e, id, field, json!(name));
        }
        assert!(resolve_class_fixture(&e).is_ok());
        for id in ["export", "work-guard", "import", "import-guard"] {
            let mut e = class_source(false);
            let mut competing: crate::analysis::Fact =
                serde_json::from_value(json!(e.facts.iter().find(|f| f.id == id).unwrap()))
                    .unwrap();
            competing.id.push_str("-competing");
            e.facts.push(competing);
            assert!(resolve_class_fixture(&e).is_err(), "{id}");
        }
        let mut e = class_source(false);
        e.files.push(SourceFile {
            path: "lib/work.js".into(),
            sha256: "d".repeat(64),
        });
        for id in ["target-module", "target-summary"] {
            let mut fact: crate::analysis::Fact =
                serde_json::from_value(json!(e.facts.iter().find(|f| f.id == id).unwrap()))
                    .unwrap();
            fact.id.push_str("-js");
            fact.path = "lib/work.js".into();
            e.facts.push(fact);
        }
        mutate(
            &mut e,
            "target-summary-js",
            "ownerFactId",
            json!("target-module-js"),
        );
        assert_eq!(
            resolve_class_fixture(&e).unwrap_err(),
            "relative_module_missing_ambiguous_or_unsupported"
        );
    }

    fn prepare(
        e: &Evidence,
        facts: usize,
        bytes: usize,
    ) -> Result<crate::packet::Prepared, String> {
        prepare_with_byte_limit(
            e,
            &FocusOptions {
                symbol: "$run".into(),
                hops: 2,
                max_facts: facts,
            },
            bytes,
        )
    }

    #[test]
    fn complete_module_proofs_are_atomic_and_version_bound() {
        let mut e = source();
        let full = prepare(&e, 100, MAX_INPUT_BYTES).unwrap();
        let links = full.input["focus"]["links"].as_array().unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0]["kind"], "js_module_call_candidate");
        assert_eq!(links[0]["resolutionPolicy"], POLICY);
        let required = [
            "module",
            "run",
            "run-meta",
            "call",
            "call-meta",
            "summary",
            "import",
            "import-guard",
            "target-module",
            "target-summary",
            "export",
            "work",
            "work-meta",
            "work-guard",
        ];
        for version in [
            crate::javascript::MODULE_EXTRACTOR_VERSION,
            crate::javascript::OPERATION_EXTRACTOR_VERSION,
            crate::javascript::CLASS_RELATION_EXTRACTOR_VERSION,
        ] {
            e.extractor_version = version.into();
            for id in ["summary", "target-summary"] {
                mutate(&mut e, id, "policy", json!(POLICY));
            }
            assert_eq!(
                prepare(&e, 100, MAX_INPUT_BYTES).unwrap().input["focus"]["links"],
                full.input["focus"]["links"]
            );
        }
        for id in required {
            assert!(
                links[0]["evidenceIds"]
                    .as_array()
                    .unwrap()
                    .contains(&json!(id)),
                "{id}"
            );
        }
        assert_eq!(full.input["files"].as_array().unwrap().len(), 2);
        for cap in [3, 5, 9, 13, 14, 100] {
            for bytes in [
                2000,
                4000,
                full.coverage.input_bytes - 1,
                full.coverage.input_bytes,
            ] {
                if let Ok(p) = prepare(&e, cap, bytes) {
                    let facts = p.input["facts"].as_array().unwrap();
                    assert!(facts.len() <= cap);
                    assert_eq!(
                        p.coverage.input_bytes,
                        serde_json::to_vec(&p.input).unwrap().len()
                    );
                    assert!(p.coverage.input_bytes <= bytes);
                    for l in p.input["focus"]["links"].as_array().unwrap() {
                        for id in l["evidenceIds"].as_array().unwrap() {
                            assert!(facts.iter().any(|f| f["id"] == *id));
                        }
                    }
                }
            }
        }
        e.extractor_version = crate::javascript::LEGACY_EXTRACTOR_VERSION.into();
        assert!(e.validate_language().is_ok());
        let legacy = prepare(&e, 100, MAX_INPUT_BYTES).unwrap();
        assert!(legacy.input["focus"]["links"]
            .as_array()
            .unwrap()
            .is_empty());
        assert!(legacy.input["focus"]["unresolved"]
            .to_string()
            .contains("legacy"));
        for version in [
            crate::javascript::STRUCTURAL_EXTRACTOR_VERSION,
            crate::javascript::LEGACY_EXTRACTOR_VERSION,
            "javascript-typescript-static-999",
        ] {
            e.extractor_version = version.into();
            e.schema_version = "ideomancer.evidence.v1".into();
            e.language = None;
            e.python_version = "3.12".into();
            assert!(e.validate_language().is_err());
        }
    }

    #[test]
    fn module_metadata_mutations_fail_closed() {
        for (id, field, value) in [
            ("summary", "importsSafe", json!(false)),
            ("target-summary", "exportsSafe", json!(false)),
            ("summary", "policy", json!("future-policy")),
            ("summary", "dependencies", json!([])),
            ("target-summary", "dependencies", json!(["../app.ts"])),
            ("target-summary", "mode", json!("commonjs")),
            ("import", "local", json!("other")),
            ("import", "ownerFactId", json!("run")),
            ("import", "moduleSpecifier", json!("./lib/work.js")),
            ("import", "bindingGuardFactId", json!("module")),
            ("import-guard", "writes", json!(1)),
            ("import-guard", "scopeKind", json!("function")),
            ("import-guard", "bindingKind", json!("let")),
            ("import-guard", "scopeEnd", json!(0)),
            (
                "call-meta",
                "lookupScopeIds",
                json!(["scope-module", "scope-module"]),
            ),
            ("call-meta", "supportingFactIds", json!(["import"])),
            ("export", "local", json!("wrong")),
            ("export", "targetMetadataFactId", json!("work")),
            ("export", "bindingGuardFactId", json!("target-module")),
            ("work-guard", "declarationCount", json!(2)),
            ("work-guard", "writes", json!(1)),
            ("work-guard", "scopeKind", json!("function")),
            ("work-guard", "scopeEnd", json!(0)),
            ("work-meta", "kind", json!("callback")),
        ] {
            let mut e = source();
            mutate(&mut e, id, field, value);
            if let Ok(p) = prepare(&e, 100, MAX_INPUT_BYTES) {
                assert!(
                    p.input["focus"]["links"].as_array().unwrap().is_empty(),
                    "{id}.{field}"
                );
                assert!(!p.input["focus"]["unresolved"]
                    .as_array()
                    .unwrap()
                    .is_empty());
            }
        }
        for id in [
            "summary",
            "target-summary",
            "import",
            "export",
            "import-guard",
            "work-guard",
        ] {
            let mut e = source();
            let mut duplicate: crate::analysis::Fact =
                serde_json::from_value(json!(e.facts.iter().find(|f| f.id == id).unwrap()))
                    .unwrap();
            duplicate.id.push_str("-competing");
            e.facts.push(duplicate);
            assert!(
                prepare(&e, 100, MAX_INPUT_BYTES).unwrap().input["focus"]["links"]
                    .as_array()
                    .unwrap()
                    .is_empty(),
                "{id}"
            );
        }
        let mut e = source();
        add(
            &mut e,
            "shadow",
            "js_binding_guard",
            "app.$run",
            3,
            3,
            json!({"name":"work","bindingKind":"const","declarationFactId":null,"declarationCount":1,"writes":0,"scopeId":"scope-run","scopeKind":"function","scopeStart":1,"scopeEnd":200}),
        );
        assert!(
            prepare(&e, 100, MAX_INPUT_BYTES).unwrap().input["focus"]["links"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn summary_policy_and_function_target_kind_are_version_bound() {
        for version in [
            crate::javascript::MODULE_EXTRACTOR_VERSION,
            crate::javascript::OPERATION_EXTRACTOR_VERSION,
            crate::javascript::CLASS_RELATION_EXTRACTOR_VERSION,
            crate::javascript::STRUCTURAL_EXTRACTOR_VERSION,
        ] {
            let current = crate::javascript::supports_class_module_targets(version);
            let mut e = source();
            e.extractor_version = version.into();
            let expected = if current { CLASS_POLICY } else { POLICY };
            for id in ["summary", "target-summary"] {
                mutate(&mut e, id, "policy", json!(expected));
            }
            assert_eq!(
                prepare(&e, 100, MAX_INPUT_BYTES).unwrap().input["focus"]["links"]
                    .as_array()
                    .unwrap()
                    .len(),
                1
            );
            for kind in [
                Value::Null,
                json!("class"),
                json!("unknown"),
                json!("function"),
            ] {
                mutate(&mut e, "export", "targetKind", kind.clone());
                let p = prepare(&e, 100, MAX_INPUT_BYTES).unwrap();
                assert_eq!(
                    !p.input["focus"]["links"].as_array().unwrap().is_empty(),
                    kind == "function",
                    "{version}: {kind}"
                );
            }
            let export = e.facts.iter_mut().find(|f| f.id == "export").unwrap();
            let mut v: Value = serde_json::from_str(&export.value).unwrap();
            v.as_object_mut().unwrap().remove("targetKind");
            export.value = v.to_string();
            assert_eq!(
                !prepare(&e, 100, MAX_INPUT_BYTES).unwrap().input["focus"]["links"]
                    .as_array()
                    .unwrap()
                    .is_empty(),
                !current
            );
            mutate(&mut e, "export", "targetKind", json!("function"));
            for id in ["summary", "target-summary"] {
                mutate(
                    &mut e,
                    id,
                    "policy",
                    json!(if current { POLICY } else { CLASS_POLICY }),
                );
                assert!(
                    prepare(&e, 100, MAX_INPUT_BYTES).unwrap().input["focus"]["links"]
                        .as_array()
                        .unwrap()
                        .is_empty()
                );
                mutate(&mut e, id, "policy", json!(expected));
            }
        }
    }

    #[test]
    fn helper_named_class_exports_resolve_without_source_reads() {
        for (app, target) in [
            ("import {PublicClass as LocalClass} from './work'; export class App extends LocalClass {}", "class Work {} export {Work as PublicClass};"),
            ("const {PublicClass: LocalClass} = require('./work'); class App extends LocalClass {} module.exports = {App};", "class Work {} module.exports = {PublicClass: Work};"),
        ] {
            let root = tempfile::tempdir().unwrap();
            std::fs::write(root.path().join("app.ts"), app).unwrap();
            std::fs::write(root.path().join("work.ts"), target).unwrap();
            let e = crate::javascript::extract(root.path()).unwrap();
            assert!(e.complete);
            root.close().unwrap();
            let index = Index::new(e.facts.iter().collect()).unwrap();
            let modules = Modules::new(&index, &e);
            let source = index.facts.iter().position(|f| f.kind == "js_class_relation").unwrap();
            let relation: Value = serde_json::from_str(&index.facts[source].value).unwrap();
            let import = index.reference(source, &relation, "importFactId").unwrap();
            let guard = index.reference(source, &relation, "bindingGuardFactId").unwrap();
            let (target, proof) = modules.resolve_class(&index, source, import, guard, "LocalClass", &relation["lookupScopeIds"]).unwrap();
            assert_eq!(index.facts[target].kind, "class");
            assert_eq!(index.facts[target].path, "work.ts");
            assert!(proof.contains(&target) && proof.contains(&source) && proof.contains(&guard));
        }
    }

    #[test]
    fn helper_module_contract_reaches_generic_renderer_without_source_reads() {
        for (app, target) in [
            ("import {work as task} from './work'; export function run(){ task(); }", "export function work(){ return 1; }"),
            ("const {work: task} = require('./work'); function run(){ task(); } module.exports = {run};", "function work(){ return 1; } module.exports = {work};"),
        ] {
            let root = tempfile::tempdir().unwrap();
            std::fs::write(root.path().join("app.ts"), app).unwrap();
            std::fs::write(root.path().join("work.ts"), target).unwrap();
            let e = crate::javascript::extract(root.path()).unwrap();
            assert!(e.complete);
            let catalog = crate::catalog::build(&e, "local").unwrap();
            let item = catalog.items.iter().find(|i| i.kind == crate::catalog::Kind::Workflow && i.symbol.ends_with(".run")).unwrap();
            root.close().unwrap();
            let native = crate::render::endpoint::build(&catalog, &e, item, &crate::render::endpoint::View::Services, None, 2, 100).unwrap();
            let links = native.legend["focus"]["links"].as_array().unwrap();
            assert_eq!(links.len(), 1, "{}", native.legend["focus"]);
            assert_eq!(links[0]["kind"], "js_module_call_candidate");
            assert!(native.legend["calls"].as_array().unwrap().iter().any(|c| c["targetCandidate"]["kind"] == "js_module_call_candidate"));
        }
    }

    #[test]
    fn all_dependency_edges_suppress_cycles_and_duplicates_remain_canonical() {
        let mut e = source();
        let expected = prepare(&e, 100, MAX_INPUT_BYTES).unwrap();
        let duplicates: Vec<crate::analysis::Fact> =
            serde_json::from_value(json!(e.facts)).unwrap();
        e.facts.extend(duplicates);
        e.facts.reverse();
        e.files.reverse();
        assert_eq!(
            prepare(&e, 100, MAX_INPUT_BYTES).unwrap().input,
            expected.input
        );

        let mut e = source();
        // No js_module_import is needed for this dependency: it still forms a
        // cycle even when the importing half cannot safely produce link proofs.
        mutate(&mut e, "target-summary", "importsSafe", json!(false));
        mutate(&mut e, "target-summary", "dependencies", json!(["../app"]));
        let p = prepare(&e, 100, MAX_INPUT_BYTES).unwrap();
        assert!(p.input["focus"]["links"].as_array().unwrap().is_empty());
        assert!(p.input["focus"]["unresolved"]
            .to_string()
            .contains("cyclic_module_component"));

        let mut e = source();
        e.facts.retain(|f| f.id != "target-summary");
        assert!(
            prepare(&e, 100, MAX_INPUT_BYTES).unwrap().input["focus"]["links"]
                .as_array()
                .unwrap()
                .is_empty()
        );

        let mut e = crate::js_focus::tests::source();
        e.extractor_version = crate::javascript::LEGACY_EXTRACTOR_VERSION.into();
        let p = prepare(&e, 100, MAX_INPUT_BYTES).unwrap();
        assert_eq!(
            p.input["focus"]["links"][0]["kind"],
            "js_local_call_candidate"
        );
        let index = Index::new(e.facts.iter().collect()).unwrap();
        let modules = Modules::new(&index, &e);
        assert!(!modules.enabled);
        assert!(modules.files.is_empty() && modules.metadata.is_empty());
    }

    #[test]
    fn known_imports_and_export_specifiers_cannot_hide_reverse_cycle_dependencies() {
        for kind in ["js_module_import", "js_import", "js_module_export"] {
            let mut e = source();
            mutate(&mut e, "target-summary", "dependencies", json!(["../app"]));
            add(
                &mut e,
                "reverse",
                kind,
                "lib.work",
                35,
                35,
                json!({"ownerFactId":if kind == "js_import" { "work" } else { "target-module" },
                    "moduleSpecifier":"../app","mode":"esm","imported":"*","local":"back",
                    "bindings":[{"imported":"*","local":"back"}],"dynamic":false}),
            );
            e.facts.last_mut().unwrap().path = "lib/work.ts".into();
            let p = prepare(&e, 100, MAX_INPUT_BYTES).unwrap();
            assert!(p.input["focus"]["links"].as_array().unwrap().is_empty());
            assert!(p.input["focus"]["unresolved"]
                .to_string()
                .contains("cyclic_module_component"));

            // Removing the dependency without removing its known record may not
            // turn the formerly cyclic forward call into a promoted link.
            mutate(&mut e, "target-summary", "dependencies", json!([]));
            let p = prepare(&e, 100, MAX_INPUT_BYTES).unwrap();
            assert!(
                p.input["focus"]["links"].as_array().unwrap().is_empty(),
                "{kind}"
            );
            assert!(p.input["focus"]["unresolved"]
                .to_string()
                .contains("module_proof_unavailable"));
        }
        let mut e = source();
        add(
            &mut e,
            "competing-module",
            "module",
            "app",
            1,
            100,
            json!("app"),
        );
        assert!(
            prepare(&e, 100, MAX_INPUT_BYTES).unwrap().input["focus"]["links"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn commonjs_exports_require_initialization_order_but_esm_and_hoisting_do_not() {
        let mut e = source();
        for id in ["summary", "target-summary", "import", "export"] {
            mutate(&mut e, id, "mode", json!("commonjs"));
        }
        mutate(&mut e, "import-guard", "bindingKind", json!("const"));
        mutate(&mut e, "export", "referenceStart", json!(700));
        mutate(&mut e, "export", "initializedBeforeExport", json!(true));
        // Function declarations are hoisted and need no initializationEnd.
        assert_eq!(
            prepare(&e, 100, MAX_INPUT_BYTES).unwrap().input["focus"]["links"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        for kind in ["const", "let", "var"] {
            mutate(&mut e, "work-guard", "bindingKind", json!(kind));
            mutate(&mut e, "work-meta", "kind", json!("arrow"));
            mutate(&mut e, "work-guard", "initializationEnd", json!(700));
            assert_eq!(
                prepare(&e, 100, MAX_INPUT_BYTES).unwrap().input["focus"]["links"]
                    .as_array()
                    .unwrap()
                    .len(),
                1
            );
            for (id, field, value) in [
                ("export", "initializedBeforeExport", Value::Null),
                ("export", "initializedBeforeExport", json!(false)),
                ("export", "initializedBeforeExport", json!("true")),
                ("export", "referenceStart", Value::Null),
                ("export", "referenceStart", json!(-1)),
                ("export", "referenceStart", json!(1.5)),
                ("export", "referenceStart", json!(1024 * 1024 + 1)),
                ("work-guard", "initializationEnd", Value::Null),
                ("work-guard", "initializationEnd", json!(-1)),
                ("work-guard", "initializationEnd", json!(1.5)),
                ("work-guard", "initializationEnd", json!(701)),
                ("work-guard", "initializationEnd", json!(1024 * 1024 + 1)),
            ] {
                let mut bad: Evidence = serde_json::from_value(json!(e)).unwrap();
                mutate(&mut bad, id, field, value);
                let p = prepare(&bad, 100, MAX_INPUT_BYTES).unwrap();
                assert!(
                    p.input["focus"]["links"].as_array().unwrap().is_empty(),
                    "{kind} {id}.{field}"
                );
                assert!(p.input["focus"]["unresolved"]
                    .to_string()
                    .contains("commonjs_export"));
            }
        }
        for id in ["summary", "target-summary", "import", "export"] {
            mutate(&mut e, id, "mode", json!("esm"));
        }
        mutate(&mut e, "import-guard", "bindingKind", json!("import"));
        mutate(&mut e, "export", "initializedBeforeExport", json!(false));
        mutate(&mut e, "work-guard", "initializationEnd", Value::Null);
        assert_eq!(
            prepare(&e, 100, MAX_INPUT_BYTES).unwrap().input["focus"]["links"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn selected_paths_are_exact_and_never_use_runtime_resolution() {
        let files = [
            "app.ts",
            "lib/work.ts",
            "lib/work.js",
            "lib/only.ts",
            "lib/index.ts",
        ]
        .map(str::to_string)
        .into_iter()
        .collect();
        assert_eq!(
            resolve_path("app.ts", "./lib/only", &files).as_deref(),
            Some("lib/only.ts")
        );
        assert_eq!(
            resolve_path("lib/a.ts", "../lib/work.js", &files).as_deref(),
            Some("lib/work.js")
        );
        for spec in [
            "./lib/work",
            "./lib",
            "./lib/only.js",
            "../lib/only",
            "./../lib/only",
            "lib/only",
            "/lib/only.ts",
            "https://x",
            "./lib\\only.ts",
            "./lib/only.ts?x",
            "./lib/only.cjs",
        ] {
            assert_eq!(resolve_path("app.ts", spec, &files), None, "{spec}");
        }
    }

    #[test]
    fn iterative_cycles_include_self_edges_but_not_neighbors() {
        let edges = vec![
            BTreeSet::from([1]),
            BTreeSet::from([2]),
            BTreeSet::from([1, 3]),
            BTreeSet::new(),
            BTreeSet::from([4]),
        ];
        assert_eq!(cyclic_nodes(&edges), BTreeSet::from([1, 2, 4]));
    }
}
