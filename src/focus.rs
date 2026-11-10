//! Bounded source candidates, never runtime dispatch or target execution.
use crate::analysis::{Evidence, Fact};
use crate::packet::{
    lexical_contexts, Coverage, KindCoverage, LexicalContext, Prepared, MAX_INPUT_BYTES,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const POLICY_VERSION: &str = "architecture-focus.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FocusOptions {
    pub symbol: String,
    pub hops: usize,
    pub max_facts: usize,
}

fn callable(f: &Fact) -> bool {
    matches!(f.kind.as_str(), "function" | "async_function")
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

fn binding(f: &Fact) -> Option<(String, String)> {
    let v: Value = serde_json::from_str(&f.value).ok()?;
    Some((v["target"].as_str()?.into(), v["source"].as_str()?.into()))
}

struct Index<'a> {
    facts: Vec<&'a Fact>,
    ctx: BTreeMap<usize, LexicalContext>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Signature {
    positional: Vec<String>,
    positional_only: usize,
    keyword_only: Vec<String>,
    vararg: Option<String>,
    kwarg: Option<String>,
}

impl Index<'_> {
    fn module(&self, i: usize) -> Option<usize> {
        self.facts
            .iter()
            .position(|f| f.kind == "module" && f.path == self.facts[i].path)
    }

    fn parent(&self, i: usize) -> Option<usize> {
        let child = self.facts[i];
        self.facts
            .iter()
            .enumerate()
            .filter(|(j, f)| {
                *j != i
                    && f.path == child.path
                    && (callable(f) || f.kind == "class" || f.kind == "module")
                    && child
                        .symbol
                        .strip_prefix(&f.symbol)
                        .is_some_and(|s| s.starts_with('.'))
                    && (f.kind == "module"
                        || (f.line <= child.line && child.end_line <= f.end_line))
            })
            .max_by_key(|(j, f)| (f.symbol.len(), f.line, std::cmp::Reverse(*j)))
            .map(|(j, _)| j)
    }

    // Resolve a lexical name, never a global suffix. Imports must be visible in
    // the reference's actual scope, and conflicting evidence stays ambiguous.
    fn names(&self, i: usize, name: &str) -> Vec<(usize, Vec<usize>)> {
        if !dotted(name) {
            return vec![];
        }
        let Some(module) = self.module(i) else {
            return vec![];
        };
        let f = self.facts[i];
        let root = name.split('.').next().unwrap();
        let tail = &name[root.len()..];
        // Walk actual declaration instances, not symbol prefixes: repeated
        // definitions and nested classes must not lend one another locals.
        let annotation = matches!(
            f.kind.as_str(),
            "parameter_annotation" | "return_annotation"
        );
        let mut scope = self.ctx[&i].owner;
        if annotation {
            scope = self.ctx[&i].function.and_then(|j| self.parent(j));
        }
        while let Some(owner) = scope {
            scope = self.parent(owner);
            // Method bodies do not capture their class's attribute namespace.
            if self.facts[owner].kind == "class" && !annotation {
                continue;
            }
            if self.facts.iter().enumerate().any(|(j, b)| {
                b.path == f.path
                    && self.ctx[&j].owner == Some(owner)
                    && ((b.kind == "parameter"
                        && b.symbol == format!("{}.{}", self.facts[owner].symbol, root))
                        || (matches!(b.kind.as_str(), "binding" | "binding_invalidation")
                            && b.line <= f.line
                            && serde_json::from_str::<Value>(&b.value)
                                .ok()
                                .is_some_and(|v| v["target"].as_str() == Some(root))))
            }) {
                return vec![];
            }
            let declarations: Vec<_> = self
                .facts
                .iter()
                .enumerate()
                .filter(|(j, d)| {
                    d.path == f.path
                        && (callable(d) || d.kind == "class")
                        && d.symbol == format!("{}.{}", self.facts[owner].symbol, root)
                        && self.parent(*j) == Some(owner)
                })
                .map(|(j, _)| j)
                .collect();
            let mut imports = Vec::new();
            for (j, import) in self.facts.iter().enumerate() {
                if import.path != f.path
                    || !matches!(import.kind.as_str(), "import" | "from_import")
                    || self.ctx[&j].owner != Some(owner)
                {
                    continue;
                }
                let Ok(v) = serde_json::from_str::<Value>(&import.value) else {
                    continue;
                };
                let Some(names) = v["names"].as_array() else {
                    continue;
                };
                let matching: Vec<_> = names
                    .iter()
                    .filter(|alias| {
                        let Some(original) = alias["name"].as_str() else {
                            return false;
                        };
                        let local = alias["asname"].as_str().unwrap_or_else(|| {
                            if import.kind == "import" {
                                original.split('.').next().unwrap()
                            } else {
                                original
                            }
                        });
                        local == root
                    })
                    .collect();
                if matching.is_empty() {
                    continue;
                }
                if owner != module && import.line > f.line {
                    return vec![];
                }
                let mut base = v["module"].as_str().unwrap_or("").to_string();
                let level = v["level"].as_u64().unwrap_or(0) as usize;
                if level > 0 {
                    let mut package: Vec<_> = self.facts[module].symbol.split('.').collect();
                    if !f.path.ends_with("/__init__.py") && f.path != "__init__.py" {
                        package.pop();
                    }
                    if level > package.len() {
                        return vec![];
                    }
                    for _ in 1..level {
                        package.pop();
                    }
                    base = [package.join("."), base]
                        .into_iter()
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join(".");
                }
                for alias in matching {
                    let Some(original) = alias["name"].as_str() else {
                        continue;
                    };
                    let local = alias["asname"].as_str().unwrap_or_else(|| {
                        if import.kind == "import" {
                            original.split('.').next().unwrap()
                        } else {
                            original
                        }
                    });
                    if local != root {
                        continue;
                    }
                    let target = if import.kind == "import" {
                        if alias["asname"].as_str().is_some() {
                            original
                        } else {
                            local
                        }
                        .to_string()
                    } else if base.is_empty() {
                        original.to_string()
                    } else {
                        format!("{base}.{original}")
                    };
                    imports.push((format!("{target}{tail}"), j));
                }
            }
            let mut result = Vec::new();
            if !imports.is_empty() && !declarations.is_empty() {
                return result;
            }
            if !imports.is_empty() {
                // A conflicting or external alias must not silently fall back to a
                // same-spelled local declaration.
                let targets: BTreeSet<_> = imports.iter().map(|(s, _)| s).collect();
                if targets.len() != 1 {
                    return result;
                }
                for (target, import) in imports {
                    for (j, d) in self.facts.iter().enumerate() {
                        if (callable(d) || d.kind == "class") && d.symbol == target {
                            result.push((j, vec![import]));
                        }
                    }
                }
            } else if !declarations.is_empty() {
                if declarations.len() != 1 {
                    return result;
                }
                let declaration = declarations[0];
                let local = self.facts[declaration];
                let target = format!("{}{tail}", local.symbol);
                for (j, d) in self.facts.iter().enumerate() {
                    if d.path == f.path
                        && (callable(d) || d.kind == "class")
                        && d.symbol == target
                        && local.line <= d.line
                        && d.end_line <= local.end_line
                    {
                        result.push((
                            j,
                            if j == declaration {
                                vec![]
                            } else {
                                vec![declaration]
                            },
                        ));
                    }
                }
            } else {
                continue;
            }
            // Matching names in this scope block every outer fallback, even when
            // their target is external, missing, or ambiguous.
            result.sort();
            result.dedup();
            return result;
        }
        vec![]
    }

    fn method(&self, class: usize, method: &str) -> Option<usize> {
        let symbol = format!("{}.{}", self.facts[class].symbol, method);
        let found: Vec<_> = self
            .facts
            .iter()
            .enumerate()
            .filter(|(j, f)| callable(f) && f.symbol == symbol && self.ctx[j].class == Some(class))
            .map(|(j, _)| j)
            .collect();
        // Repeated definitions are not guessed, even though Python normally
        // uses the last one: decorators/metaclasses can change that behavior.
        (found.len() == 1).then(|| found[0])
    }

    // Without invocation/CFG ordering, any lexical replacement invalidates a
    // parameter or capture, even if its original annotation is still present.
    fn lexically_replaced(&self, owner: usize, name: &str) -> bool {
        self.facts.iter().enumerate().any(|(j, f)| {
            let declaration = callable(f) || f.kind == "class";
            let scope = if declaration {
                self.parent(j)
            } else {
                self.ctx[&j].owner
            };
            if f.path != self.facts[owner].path || scope != Some(owner) {
                return false;
            }
            if declaration {
                return f.symbol == format!("{}.{}", self.facts[owner].symbol, name);
            }
            let Ok(value) = serde_json::from_str::<Value>(&f.value) else {
                return false;
            };
            if matches!(f.kind.as_str(), "binding" | "binding_invalidation") {
                return value["target"].as_str() == Some(name);
            }
            matches!(f.kind.as_str(), "import" | "from_import")
                && value["names"].as_array().is_some_and(|aliases| {
                    aliases.iter().any(|alias| {
                        let Some(original) = alias["name"].as_str() else {
                            return true;
                        };
                        let local = alias["asname"].as_str().unwrap_or_else(|| {
                            if f.kind == "import" {
                                original.split('.').next().unwrap()
                            } else {
                                original
                            }
                        });
                        original == "*" || local == name
                    })
                })
        })
    }

    // Focus-only constructor provenance. Do not broaden packet receiver sharing.
    fn signature(&self, function: usize) -> Option<(usize, Signature)> {
        let signatures: Vec<_> = self
            .facts
            .iter()
            .enumerate()
            .filter(|(j, f)| f.kind == "signature" && self.ctx[j].owner == Some(function))
            .map(|(j, _)| j)
            .collect();
        if signatures.len() != 1 {
            return None;
        }
        let id = signatures[0];
        let signature: Signature = serde_json::from_str(&self.facts[id].value).ok()?;
        let names: Vec<_> = signature
            .positional
            .iter()
            .chain(&signature.keyword_only)
            .collect();
        let declared: BTreeSet<_> = self
            .facts
            .iter()
            .enumerate()
            .filter(|(j, f)| f.kind == "parameter" && self.ctx[j].owner == Some(function))
            .map(|(_, f)| &f.value)
            .collect();
        if signature.vararg.is_some()
            || signature.kwarg.is_some()
            || signature.positional_only > signature.positional.len()
            || names.len() > 128
            || names.len() != declared.len()
            || names.iter().any(|name| !dotted(name) || name.contains('.'))
            || names.into_iter().collect::<BTreeSet<_>>() != declared
        {
            return None;
        }
        Some((id, signature))
    }

    fn constructor_field(&self, call: usize, head: &str) -> Option<(usize, Vec<usize>)> {
        let (field, method) = head.strip_prefix("self.")?.split_once('.')?;
        if !dotted(field) || method.contains('.') {
            return None;
        }
        let class = self.ctx[&call].class?;
        let mut scope = self.ctx[&call].function?;
        let mut proof = vec![class];
        loop {
            if self.lexically_replaced(scope, "self") {
                return None;
            }
            proof.push(scope);
            // Captures must reach an actual instance method without crossing a class.
            let parameters: Vec<_> = self
                .facts
                .iter()
                .enumerate()
                .filter(|(j, f)| {
                    f.kind == "parameter" && self.ctx[j].owner == Some(scope) && f.value == "self"
                })
                .map(|(j, _)| j)
                .collect();
            let parent = self.parent(scope)?;
            if parent == class {
                if parameters.len() != 1 || !self.ctx[&parameters[0]].self_receiver {
                    return None;
                }
                proof.extend(parameters);
                break;
            }
            if !callable(self.facts[parent]) || !parameters.is_empty() {
                return None;
            }
            scope = parent;
        }
        let init = self.method(class, "__init__")?;
        proof.push(init);
        let receiver = format!("self.{field}");
        let assignments = |owner| {
            self.facts
                .iter()
                .enumerate()
                .filter(|(j, f)| {
                    self.ctx[j].owner == Some(owner)
                        && f.kind == "binding"
                        && binding(f).is_some_and(|(target, _)| target == receiver)
                })
                .map(|(j, _)| j)
                .collect::<Vec<_>>()
        };
        let local = assignments(init);
        let (assignment, annotation, parameter, owner_class) = if local.len() == 1 {
            let assignment = local[0];
            let (_, source) = binding(self.facts[assignment])?;
            let annotation = self
                .facts
                .iter()
                .enumerate()
                .filter(|(j, f)| {
                    f.kind == "parameter_annotation"
                        && self.ctx[j].owner == Some(init)
                        && f.symbol == format!("{}.{}", self.facts[init].symbol, source)
                })
                .map(|(j, _)| j)
                .collect::<Vec<_>>();
            if annotation.len() != 1 {
                return None;
            }
            (assignment, annotation[0], source, class)
        } else if local.is_empty() {
            let bases: Vec<_> = self
                .facts
                .iter()
                .enumerate()
                .filter(|(j, f)| {
                    f.kind == "base"
                        && self.ctx[j].class == Some(class)
                        && f.symbol == self.facts[class].symbol
                })
                .map(|(j, _)| j)
                .collect();
            if bases.len() != 1 {
                return None;
            }
            let base = bases[0];
            let base_value = &self.facts[base].value;
            let base_name = if let Some((name, argument)) = base_value.split_once('[') {
                // A source generic argument is not an evaluated DI expression.
                if !dotted(argument.strip_suffix(']')?) {
                    return None;
                }
                name
            } else {
                base_value
            };
            let resolved = self.names(base, base_name);
            if resolved.len() != 1 {
                return None;
            }
            let (base_class, imports) = &resolved[0];
            if self.facts[*base_class].kind != "class" {
                return None;
            }
            let base_init = self.method(*base_class, "__init__")?;
            let assigned = assignments(base_init);
            if assigned.len() != 1 {
                return None;
            }
            let assignment = assigned[0];
            let (_, parameter) = binding(self.facts[assignment])?;
            if !dotted(&parameter) || parameter.contains('.') {
                return None;
            }
            let (base_signature, signature) = self.signature(base_init)?;
            let (child_signature, child) = self.signature(init)?;
            if signature.positional.first().map(String::as_str) != Some("self")
                || child.positional.first().map(String::as_str) != Some("self")
            {
                return None;
            }
            let forwarding: Vec<_> = self
                .facts
                .iter()
                .enumerate()
                .filter(|(j, f)| {
                    f.kind == "call"
                        && self.ctx[j].owner == Some(init)
                        && f.value.contains(".__init__(")
                })
                .map(|(j, _)| j)
                .collect();
            if forwarding.len() != 1 {
                return None;
            }
            let forward = forwarding[0];
            if self.facts.iter().any(|f| {
                if f.path != self.facts[init].path {
                    return false;
                }
                if matches!(f.kind.as_str(), "parameter" | "function" | "class")
                    && f.symbol.rsplit('.').next() == Some("super")
                {
                    return true;
                }
                let Ok(value) = serde_json::from_str::<Value>(&f.value) else {
                    return false;
                };
                value["target"].as_str() == Some("super")
                    || value["names"].as_array().is_some_and(|names| {
                        names.iter().any(|alias| {
                            alias["name"].as_str() == Some("*")
                                || alias["asname"].as_str().or_else(|| alias["name"].as_str())
                                    == Some("super")
                        })
                    })
            }) {
                return None;
            }
            let args = self.facts[forward]
                .value
                .strip_prefix("super().__init__(")?
                .strip_suffix(')')?;
            let mut mapped = BTreeMap::new();
            let mut position = 1;
            let mut keywords = false;
            for arg in args.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                let (name, source) = if let Some((name, source)) = arg.split_once('=') {
                    keywords = true;
                    let name = name.trim();
                    if !signature
                        .positional
                        .iter()
                        .skip(signature.positional_only.max(1))
                        .any(|s| s == name)
                        && !signature.keyword_only.iter().any(|s| s == name)
                    {
                        return None;
                    }
                    (name, source.trim())
                } else {
                    if keywords {
                        return None;
                    }
                    let name = signature.positional.get(position)?;
                    position += 1;
                    (name.as_str(), arg)
                };
                if !dotted(source)
                    || source.contains('.')
                    || !child
                        .positional
                        .iter()
                        .skip(1)
                        .chain(&child.keyword_only)
                        .any(|s| s == source)
                    || mapped.insert(name, source).is_some()
                {
                    return None;
                }
            }
            // Defaults are deliberately not evaluated or inferred. Require a
            // complete, non-variadic argument map, including keyword-only slots.
            if mapped.len() + 1 != signature.positional.len() + signature.keyword_only.len() {
                return None;
            }
            let forwarded = *mapped.get(parameter.as_str())?;
            let annotations: Vec<_> = self
                .facts
                .iter()
                .enumerate()
                .filter(|(j, f)| {
                    f.kind == "parameter_annotation"
                        && self.ctx[j].owner == Some(init)
                        && f.symbol == format!("{}.{}", self.facts[init].symbol, forwarded)
                })
                .map(|(j, _)| j)
                .collect();
            if annotations.len() != 1 {
                return None;
            }
            proof.extend([
                base,
                *base_class,
                base_init,
                forward,
                base_signature,
                child_signature,
            ]);
            proof.extend(imports);
            (
                assignment,
                annotations[0],
                forwarded.to_string(),
                *base_class,
            )
        } else {
            return None;
        };
        let assignment_owner = self.ctx[&assignment].function?;
        let (_, assigned_parameter) = binding(self.facts[assignment])?;
        if self.lexically_replaced(init, &parameter)
            || self.lexically_replaced(assignment_owner, &assigned_parameter)
        {
            return None;
        }
        if self.facts.iter().any(|f| {
            (f.path == self.facts[class].path
                || f.path == self.facts[owner_class].path
                || f.path == self.facts[annotation].path)
                && f.kind == "from_import"
                && serde_json::from_str::<Value>(&f.value)
                    .ok()
                    .is_some_and(|v| {
                        v["names"]
                            .as_array()
                            .is_some_and(|names| names.iter().any(|name| name["name"] == "*"))
                    })
        }) {
            return None;
        }
        for constructor in [init, assignment_owner] {
            if self.lexically_replaced(constructor, "self") {
                return None;
            }
            let self_parameters: Vec<_> = self
                .facts
                .iter()
                .enumerate()
                .filter(|(j, f)| {
                    f.kind == "parameter"
                        && self.ctx[j].owner == Some(constructor)
                        && f.value == "self"
                })
                .map(|(j, _)| j)
                .collect();
            if self_parameters.len() != 1 || !self.ctx[&self_parameters[0]].self_receiver {
                return None;
            }
            if self
                .facts
                .iter()
                .enumerate()
                .any(|(j, f)| f.kind == "signature" && self.ctx[&j].owner == Some(constructor))
            {
                let (id, signature) = self.signature(constructor)?;
                if signature.positional.first().map(String::as_str) != Some("self") {
                    return None;
                }
                proof.push(id);
            }
            proof.extend(self_parameters);
        }
        for c in [class, owner_class] {
            if self
                .facts
                .iter()
                .filter(|f| f.kind == "class" && f.symbol == self.facts[c].symbol)
                .count()
                != 1
            {
                return None;
            }
        }
        // Reject all recorded writes, including later writes in closures: no CFG
        // or invocation ordering is available to make them harmless.
        if self.facts.iter().enumerate().any(|(j, f)| {
            if !matches!(f.kind.as_str(), "binding" | "binding_invalidation") || j == assignment {
                return false;
            }
            let Some(target) = serde_json::from_str::<Value>(&f.value)
                .ok()
                .and_then(|v| v["target"].as_str().map(str::to_owned))
            else {
                return false;
            };
            let in_class = matches!(self.ctx[&j].class, Some(c) if c == class || c == owner_class);
            in_class
                && (target == "self"
                    || target == receiver
                    || target.starts_with(&format!("{receiver}."))
                    || (target == field && self.ctx[&j].owner == self.ctx[&j].class)
                    || (target == parameter && self.ctx[&j].function == Some(init))
                    || (target == assigned_parameter
                        && self.ctx[&j].function == Some(assignment_owner)))
        }) {
            return None;
        }
        let types = annotation_types(&self.facts[annotation].value)?;
        if types.len() != 1 {
            return None;
        }
        let resolved = self.names(annotation, types[0]);
        if resolved.len() != 1 || self.facts[resolved[0].0].kind != "class" {
            return None;
        }
        let (target_class, imports) = &resolved[0];
        let target = self.method(*target_class, method)?;
        proof.extend([assignment, annotation, *target_class]);
        proof.extend(imports);
        Some((target, proof))
    }

    fn resolve(&self, call: usize) -> Result<(usize, Vec<usize>, &'static str), &'static str> {
        let f = self.facts[call];
        let head = f.value.split('(').next().unwrap_or("").trim();
        if !dotted(head) {
            return Err("external_or_dynamic_receiver");
        }
        let ctx = &self.ctx[&call];
        if head.starts_with("self.") {
            let mut scope = ctx.function;
            while let Some(owner) = scope.filter(|&j| callable(self.facts[j])) {
                if self.lexically_replaced(owner, "self") {
                    return Err("ambiguous_or_blocked_receiver_binding");
                }
                scope = self.parent(owner);
            }
        }
        if let Some((target, proof)) = self.constructor_field(call, head) {
            return Ok((target, proof, "annotated_receiver_candidate"));
        }
        if ctx.binding_blocked {
            return Err("ambiguous_or_blocked_receiver_binding");
        }
        if let Some(method) = head
            .strip_prefix("self.")
            .filter(|s| !s.contains('.') && ctx.binding.is_none())
        {
            if ctx.self_receiver {
                if let Some(class) = ctx.class {
                    if let Some(target) = self.method(class, method) {
                        return Ok((target, vec![class], "local_self_candidate"));
                    }
                }
            }
            return Err("unresolved_self_method_or_base");
        }
        if let Some(b) = ctx.binding {
            let (receiver, source) = binding(self.facts[b]).ok_or("unresolved_binding")?;
            if receiver.starts_with("self.")
                && self.ctx[&b]
                    .owner
                    .is_some_and(|owner| self.lexically_replaced(owner, "self"))
            {
                return Err("ambiguous_or_blocked_receiver_binding");
            }
            let Some(method) = head
                .strip_prefix(&format!("{receiver}."))
                .filter(|s| !s.contains('.'))
            else {
                return Err("external_or_dynamic_receiver");
            };
            let mut proof = vec![b];
            let type_name;
            let kind;
            let constructor;
            if let Some((name, _)) = source.split_once('(') {
                constructor = name.to_string();
                type_name = &constructor;
                kind = "constructed_receiver_candidate";
            } else {
                if self.ctx[&b]
                    .owner
                    .is_some_and(|owner| self.lexically_replaced(owner, &source))
                {
                    return Err("ambiguous_or_blocked_receiver_binding");
                }
                let annotations: Vec<_> = ctx
                    .neighborhood
                    .iter()
                    .copied()
                    .filter(|&j| {
                        self.facts[j].kind == "parameter_annotation"
                            && self.facts[j].symbol
                                == format!("{}.{}", self.facts[b].symbol, source)
                    })
                    .collect();
                if annotations.len() != 1 {
                    return Err("unresolved_receiver_annotation");
                }
                let a = annotations[0];
                proof.push(a);
                type_name = &self.facts[a].value;
                kind = "annotated_receiver_candidate";
            }
            let types =
                annotation_types(type_name).ok_or("unresolved_annotation_or_dynamic_binding")?;
            let reference = *proof
                .iter()
                .find(|&&j| self.facts[j].kind == "parameter_annotation")
                .unwrap_or(&b);
            let mut candidates = Vec::new();
            for name in types {
                let resolved = self.names(reference, name);
                if resolved.len() != 1 || self.facts[resolved[0].0].kind != "class" {
                    return Err("external_or_dynamic_receiver");
                }
                let (class, imports) = &resolved[0];
                let target = self
                    .method(*class, method)
                    .ok_or("unresolved_method_or_base")?;
                let mut ids = proof.clone();
                ids.push(*class);
                ids.extend(imports);
                candidates.push((target, ids, kind));
            }
            candidates.sort();
            candidates.dedup();
            if candidates.len() == 1 {
                return Ok(candidates.remove(0));
            }
            return Err("ambiguous_receiver_annotation");
        }
        if head.starts_with("self.") || head.starts_with("cls.") {
            return Err("external_or_dynamic_receiver");
        }
        let candidates = self.names(call, head);
        if candidates.len() == 1 && callable(self.facts[candidates[0].0]) {
            let (target, proof) = candidates.into_iter().next().unwrap();
            return Ok((target, proof, "local_call_candidate"));
        }
        Err("external_or_dynamic_receiver")
    }
}

// The extractor renders `T | None` as `T BitOr <literal>`; quoted forward
// references also become <literal> and must never be evaluated or guessed.
fn annotation_types(value: &str) -> Option<Vec<&str>> {
    let value = value.trim();
    let inner = if let Some(v) = value.strip_suffix(']') {
        if let Some((wrapper, inner)) = v.split_once('[') {
            match wrapper.rsplit('.').next()? {
                "Optional" | "Union" => inner.trim_matches(|c| c == '[' || c == ']'),
                _ => return None,
            }
        } else {
            value
        }
    } else {
        value
    };
    let mut result: Vec<_> = inner
        .split([',', '|'])
        .flat_map(|s| s.split(" BitOr "))
        .map(str::trim)
        .filter(|s| *s != "None" && *s != "NoneType")
        .collect();
    // A literal can mean None OR a redacted string, so only Optional[T] is
    // accepted as nullable evidence; literal union members remain unresolved.
    if result.is_empty() || result.iter().any(|s| !dotted(s)) {
        return None;
    }
    result.sort();
    result.dedup();
    Some(result)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Link {
    source_fact_id: String,
    target_fact_id: String,
    kind: String,
    evidence_ids: Vec<String>,
}

struct Selection<'a> {
    evidence: &'a Evidence,
    index: Index<'a>,
    options: &'a FocusOptions,
    max_bytes: usize,
    anchor: usize,
    chosen: BTreeSet<usize>,
    order: Vec<usize>,
    visited: BTreeSet<usize>,
    links: Vec<Link>,
    unresolved: BTreeMap<String, String>,
    boundaries: BTreeSet<String>,
    truncated: bool,
}

impl Selection<'_> {
    fn boundary(&mut self, reason: &str) {
        self.truncated = true;
        self.boundaries.insert(reason.into());
    }

    fn add(&mut self, ids: &[usize]) -> bool {
        let new: BTreeSet<_> = ids
            .iter()
            .copied()
            .filter(|i| !self.chosen.contains(i))
            .collect();
        if self.chosen.len() + new.len() > self.options.max_facts {
            self.boundary("fact_limit: context or candidate omitted");
            return false;
        }
        for i in new {
            self.chosen.insert(i);
            self.order.push(i);
        }
        true
    }

    fn render(&self) -> Result<Prepared, String> {
        let facts = &self.index.facts;
        let paths: BTreeSet<_> = self
            .chosen
            .iter()
            .map(|&i| facts[i].path.as_str())
            .collect();
        let mut coverage = Coverage {
            policy_version: POLICY_VERSION.into(), selection_applied: true,
            input_bytes: 0, max_bytes: self.max_bytes,
            total_facts: facts.len(), included_facts: self.chosen.len(),
            omitted_facts: facts.len() - self.chosen.len(), total_files: self.evidence.files.len(),
            included_files: self.evidence.files.iter().filter(|f| paths.contains(f.path.as_str())).count(),
            by_kind: BTreeMap::new(),
            warnings: vec![
                "Focused source scope only; omitted facts/files and callers are not analyzed. Full inventory remains local.".into(),
                "Links are static candidates, not runtime guarantees; no CFG, MRO, registry or plugin evaluation is performed.".into(),
                format!("Depth limit {} callable transitions; fact limit {}; compact byte limit {}.", self.options.hops, self.options.max_facts, self.max_bytes),
                "Unresolved/external receivers do not imply a target is not called. Unresolved metadata is capped at 100 entries.".into(),
            ],
        };
        coverage.warnings.extend(self.boundaries.iter().cloned());
        for (i, f) in facts.iter().enumerate() {
            let count = coverage
                .by_kind
                .entry(f.kind.clone())
                .or_insert(KindCoverage {
                    total: 0,
                    included: 0,
                    omitted: 0,
                });
            count.total += 1;
            if self.chosen.contains(&i) {
                count.included += 1;
            } else {
                count.omitted += 1;
            }
        }
        let mut input = json!({"schemaVersion": self.evidence.schema_version,
            "snapshotId": self.evidence.snapshot_id, "extractorVersion": self.evidence.extractor_version,
            "pythonVersion": self.evidence.python_version, "diagnostics": self.evidence.diagnostics,
            "complete": self.evidence.complete});
        input["facts"] = json!(self.chosen.iter().map(|&i| facts[i]).collect::<Vec<_>>());
        let mut files: Vec<_> = self
            .evidence
            .files
            .iter()
            .filter(|f| paths.contains(f.path.as_str()))
            .collect();
        files.sort_by_key(|f| (&f.path, &f.sha256));
        input["files"] = json!(files);
        let anchor = facts[self.anchor];
        let selected_ids: BTreeSet<_> = self.chosen.iter().map(|&i| facts[i].id.as_str()).collect();
        let links: Vec<_> = self
            .links
            .iter()
            .filter(|l| {
                selected_ids.contains(l.source_fact_id.as_str())
                    && selected_ids.contains(l.target_fact_id.as_str())
                    && l.evidence_ids
                        .iter()
                        .all(|id| selected_ids.contains(id.as_str()))
            })
            .collect();
        let visited: Vec<_> = self
            .visited
            .iter()
            .filter(|i| self.chosen.contains(i))
            .map(|&i| facts[i].symbol.as_str())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        input["focus"] = json!({"policyVersion": POLICY_VERSION, "requestedSymbol": self.options.symbol,
            "anchor": {"id": anchor.id, "symbol": anchor.symbol, "path": anchor.path, "line": anchor.line, "endLine": anchor.end_line},
            "maxHops": self.options.hops, "maxFacts": self.options.max_facts, "visitedSymbols": visited,
            "links": links, "unresolved": self.unresolved.iter().filter(|(id, _)| selected_ids.contains(id.as_str())).map(|(id, reason)| json!({"factId": id, "reason": reason})).collect::<Vec<_>>(),
            "truncated": self.truncated, "boundaries": self.boundaries});
        input["selection"] = json!({"policyVersion": POLICY_VERSION, "totalFacts": coverage.total_facts,
            "includedFacts": coverage.included_facts, "omittedFacts": coverage.omitted_facts,
            "maxBytes": self.max_bytes, "byKind": coverage.by_kind, "warnings": coverage.warnings});
        coverage.input_bytes = serde_json::to_vec(&input).map_err(|e| e.to_string())?.len();
        Ok(Prepared { input, coverage })
    }
}

pub fn prepare(evidence: &Evidence, options: &FocusOptions) -> Result<Prepared, String> {
    prepare_with_byte_limit(evidence, options, MAX_INPUT_BYTES)
}

pub fn prepare_with_byte_limit(
    evidence: &Evidence,
    options: &FocusOptions,
    max_bytes: usize,
) -> Result<Prepared, String> {
    if evidence.is_javascript() {
        return crate::js_focus::prepare_with_byte_limit(evidence, options, max_bytes);
    }
    if evidence.language.is_some() || evidence.schema_version == "ideomancer.evidence.v2" {
        evidence.validate_language()?;
    }
    if !(1..=MAX_INPUT_BYTES).contains(&max_bytes) {
        return Err(format!("Focus maxBytes must be in 1..={MAX_INPUT_BYTES}"));
    }
    if options.symbol.trim().is_empty() {
        return Err("Focus symbol must not be blank; use a qualified callable symbol".into());
    }
    if options.hops > 5 {
        return Err("Focus hops must be in 0..=5".into());
    }
    if !(1..=2000).contains(&options.max_facts) {
        return Err("Focus maxFacts must be in 1..=2000".into());
    }
    let mut facts: Vec<_> = evidence.facts.iter().collect();
    facts.sort_by_key(|f| {
        (
            &f.path, &f.symbol, f.line, f.end_line, &f.kind, &f.id, &f.value,
        )
    });
    let exact: Vec<_> = facts
        .iter()
        .enumerate()
        .filter(|(_, f)| callable(f) && (f.symbol == options.symbol || f.id == options.symbol))
        .map(|(i, _)| i)
        .collect();
    let candidates: Vec<_> = if !exact.is_empty() {
        exact
    } else {
        facts
            .iter()
            .enumerate()
            .filter(|(_, f)| callable(f) && f.symbol.ends_with(&format!(".{}", options.symbol)))
            .map(|(i, _)| i)
            .collect()
    };
    if candidates.len() != 1 {
        let suggestions: Vec<_> = if candidates.is_empty() {
            facts
                .iter()
                .enumerate()
                .filter(|(_, f)| callable(f))
                .map(|(i, _)| i)
                .collect()
        } else {
            candidates.clone()
        };
        let list = suggestions
            .iter()
            .take(12)
            .map(|&i| {
                format!(
                    "{} ({}:{}, id {})",
                    facts[i].symbol, facts[i].path, facts[i].line, facts[i].id
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        return Err(format!("Focus anchor {}: {:?}. Use an exact qualified callable symbol or fact ID. Candidates (up to 12): {}", if candidates.is_empty() { "not found" } else { "ambiguous" }, options.symbol, list));
    }
    let anchor = candidates[0];
    let ctx = lexical_contexts(&facts);
    let mut s = Selection { evidence, index: Index { facts, ctx }, options, max_bytes, anchor,
        chosen: BTreeSet::new(), order: Vec::new(), visited: BTreeSet::new(), links: Vec::new(),
        unresolved: BTreeMap::new(), boundaries: BTreeSet::from(["external_or_dynamic_receiver: external SDKs, nested receivers, registries and redacted literals are not evaluated".into()]), truncated: false };
    s.add(&[anchor]);
    let mut queue = VecDeque::from([(anchor, 0)]);
    let mut deferred = Vec::new();
    while let Some((decl, depth)) = queue.pop_front() {
        if !s.visited.insert(decl) {
            continue;
        }
        let context = s.index.ctx[&decl].neighborhood.clone();
        s.add(&context);
        let d = s.index.facts[decl];
        let mut body: Vec<_> = s
            .index
            .facts
            .iter()
            .enumerate()
            .filter(|(i, f)| {
                *i != decl
                    && f.path == d.path
                    && s.index.ctx[i].owner == Some(decl)
                    && !callable(f)
                    && f.kind != "class"
            })
            .map(|(i, _)| i)
            .collect();
        // Decorators precede the declaration's line range; attach only the
        // decorator interval after the previous same-name declaration.
        let mut decorated = vec![decl];
        if let Some(class) = s.index.ctx[&decl].class {
            decorated.push(class);
        }
        for decorated in decorated {
            let d = s.index.facts[decorated];
            let previous = s
                .index
                .facts
                .iter()
                .filter(|f| {
                    f.kind == d.kind
                        && f.path == d.path
                        && f.symbol == d.symbol
                        && f.end_line < d.line
                })
                .map(|f| f.end_line)
                .max()
                .unwrap_or(0);
            body.extend(
                s.index
                    .facts
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| {
                        matches!(f.kind.as_str(), "decorator" | "api_route" | "entrypoint")
                            && f.path == d.path
                            && f.symbol == d.symbol
                            && f.line > previous
                            && f.end_line < d.line
                    })
                    .map(|(i, _)| i),
            );
        }
        body.sort_by_key(|&i| {
            (
                match s.index.facts[i].kind.as_str() {
                    "call" => 0,
                    "binding" => 1,
                    "parameter_annotation" => 2,
                    _ => 3,
                },
                i,
            )
        });
        body.dedup();
        let mut details = VecDeque::new();
        for i in body {
            let is_call = s.index.facts[i].kind == "call";
            if !is_call {
                details.push_back(i);
                continue;
            }
            let context = s.index.ctx[&i].neighborhood.clone();
            if !s.add(&context) {
                s.add(&[i]);
            }
            if !s.chosen.contains(&i) || !is_call {
                continue;
            }
            match s.index.resolve(i) {
                Ok((target, mut proof, kind)) => {
                    if depth == options.hops {
                        s.boundary("depth_limit: candidate callable bodies beyond maxHops omitted");
                        continue;
                    }
                    proof.push(i);
                    proof.extend(s.index.ctx[&i].neighborhood.iter().copied());
                    proof.extend(s.index.ctx[&target].neighborhood.iter().copied());
                    proof.push(target);
                    proof.sort_unstable();
                    proof.dedup();
                    if s.add(&proof) {
                        s.links.push(Link {
                            source_fact_id: s.index.facts[i].id.clone(),
                            target_fact_id: s.index.facts[target].id.clone(),
                            kind: kind.into(),
                            evidence_ids: proof
                                .iter()
                                .map(|&j| s.index.facts[j].id.clone())
                                .collect(),
                        });
                        queue.push_back((target, depth + 1));
                    } else {
                        s.boundary("fact_limit: candidate link omitted with its target/context");
                    }
                }
                Err(reason) => {
                    if s.unresolved.len() < 100 {
                        s.unresolved
                            .insert(s.index.facts[i].id.clone(), reason.into());
                    } else {
                        s.boundary(
                            "unresolved_limit: additional unresolved calls omitted from metadata",
                        );
                    }
                }
            }
        }
        deferred.push(details);
    }
    // Interleave remaining body details across visited declarations only after
    // candidate traversal has had its turn. Required binding/type context was
    // already charged with each call, not borrowed from this optional fill.
    loop {
        let mut progress = false;
        for details in &mut deferred {
            if let Some(i) = details.pop_front() {
                progress = true;
                let context = s.index.ctx[&i].neighborhood.clone();
                if !s.add(&context) {
                    s.add(&[i]);
                }
            }
        }
        if !progress {
            break;
        }
    }
    // Measure the complete compact packet, including focus/selection metadata.
    // Remove lower-priority additions, never the anchor, and invalidate links
    // atomically by their evidence IDs in render().
    let byte_error = || {
        if max_bytes == MAX_INPUT_BYTES {
            "Focus anchor and retained metadata cannot fit within 192 KiB".into()
        } else {
            format!("Focus anchor and retained metadata cannot fit within {max_bytes} bytes")
        }
    };
    loop {
        let prepared = s.render()?;
        if prepared.coverage.input_bytes <= max_bytes {
            return Ok(prepared);
        }
        s.boundary("byte_limit: selected facts/context/links omitted to fit compact JSON");
        let Some(i) = s.order.pop() else {
            return Err(byte_error());
        };
        if i == anchor {
            return Err(byte_error());
        }
        s.chosen.remove(&i);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::SourceFile;

    fn evidence() -> Evidence {
        Evidence {
            schema_version: "evidence.v1".into(),
            snapshot_id: "snapshot".into(),
            extractor_version: "python-static-2-bindings".into(),
            python_version: "3.12".into(),
            language: None,
            files: vec![],
            facts: vec![],
            diagnostics: vec![],
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
    ) -> String {
        let id = format!("f{}", e.facts.len());
        if !e.files.iter().any(|f| f.path == path) {
            e.files.push(SourceFile {
                path: path.into(),
                sha256: "0".repeat(64),
            });
        }
        e.facts.push(Fact {
            id: id.clone(),
            path: path.into(),
            kind: kind.into(),
            symbol: symbol.into(),
            line,
            end_line: end,
            value: value.into(),
        });
        id
    }

    fn options(symbol: &str, hops: usize, max_facts: usize) -> FocusOptions {
        FocusOptions {
            symbol: symbol.into(),
            hops,
            max_facts,
        }
    }

    fn base() -> Evidence {
        let mut e = evidence();
        fact(
            &mut e,
            "pkg/service.py",
            "module",
            "pkg.service",
            1,
            200,
            "pkg.service",
        );
        fact(
            &mut e,
            "pkg/service.py",
            "class",
            "pkg.service.Service",
            2,
            100,
            "Service",
        );
        fact(
            &mut e,
            "pkg/service.py",
            "function",
            "pkg.service.Service.run",
            20,
            50,
            "run",
        );
        e
    }

    #[test]
    fn explicit_byte_cap_is_bounded_deterministic_and_default_is_unchanged() {
        let mut e = base();
        for _ in 0..80 {
            fact(
                &mut e,
                "pkg/service.py",
                "return",
                "pkg.service.Service.run",
                25,
                25,
                &"x".repeat(480),
            );
        }
        let opts = options("pkg.service.Service.run", 4, 600);
        let default = prepare(&e, &opts).unwrap();
        let explicit = prepare_with_byte_limit(&e, &opts, MAX_INPUT_BYTES).unwrap();
        assert_eq!(default.input, explicit.input);
        assert_eq!(
            serde_json::to_value(&default.coverage).unwrap(),
            serde_json::to_value(&explicit.coverage).unwrap()
        );
        assert_eq!(default.coverage.max_bytes, MAX_INPUT_BYTES);
        let cap = 16 * 1024;
        let bounded = prepare_with_byte_limit(&e, &opts, cap).unwrap();
        assert_eq!(bounded.coverage.max_bytes, cap);
        assert_eq!(bounded.input["selection"]["maxBytes"], cap);
        assert_eq!(
            bounded.coverage.input_bytes,
            serde_json::to_vec(&bounded.input).unwrap().len()
        );
        assert!(bounded.coverage.input_bytes <= cap);
        assert!(bounded.coverage.included_facts < default.coverage.included_facts);
        assert!(bounded.input["focus"]["truncated"].as_bool().unwrap());
        e.facts.reverse();
        assert_eq!(
            bounded.input,
            prepare_with_byte_limit(&e, &opts, cap).unwrap().input
        );
        for invalid in [0, MAX_INPUT_BYTES + 1] {
            assert!(prepare_with_byte_limit(&e, &opts, invalid)
                .err()
                .unwrap()
                .contains("maxBytes"));
        }
        assert!(prepare_with_byte_limit(&e, &opts, 1)
            .err()
            .unwrap()
            .contains("cannot fit within 1 bytes"));
    }

    fn check(e: &Evidence, p: &Prepared, max: usize) {
        assert!(p.coverage.input_bytes <= MAX_INPUT_BYTES);
        assert_eq!(
            p.coverage.input_bytes,
            serde_json::to_vec(&p.input).unwrap().len()
        );
        let selected = p.input["facts"].as_array().unwrap();
        assert!(selected.len() <= max);
        assert_eq!(selected.len(), p.coverage.included_facts);
        assert_eq!(p.coverage.total_facts, e.facts.len());
        assert_eq!(p.coverage.total_files, e.files.len());
        let ids: BTreeSet<_> = selected.iter().map(|f| f["id"].as_str().unwrap()).collect();
        assert!(ids.contains(p.input["focus"]["anchor"]["id"].as_str().unwrap()));
        for f in selected {
            assert!(e
                .facts
                .iter()
                .any(|original| serde_json::to_value(original).unwrap() == *f));
        }
        for link in p.input["focus"]["links"].as_array().unwrap() {
            for key in ["sourceFactId", "targetFactId"] {
                assert!(ids.contains(link[key].as_str().unwrap()));
            }
            for id in link["evidenceIds"].as_array().unwrap() {
                assert!(ids.contains(id.as_str().unwrap()));
            }
        }
    }

    fn links(p: &Prepared) -> usize {
        p.input["focus"]["links"].as_array().unwrap().len()
    }

    #[cfg(unix)]
    #[test]
    fn extracted_constructor_forwarding_and_lexical_self_capture() {
        let python =
            std::env::var_os("IDEOMANCER_TEST_PYTHON").unwrap_or_else(|| "python3.12".into());
        if !std::process::Command::new(&python)
            .args([
                "-I",
                "-S",
                "-c",
                "import sys; sys.exit(sys.version_info < (3, 12))",
            ])
            .status()
            .is_ok_and(|s| s.success())
        {
            eprintln!("SKIP: constructor capture regression requires Python >=3.12");
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let base = r#"
raise RuntimeError("Source must not execute")
class BaseRouter:
    def __init__(self, logger, service, router):
        self.service = service
"#;
        let source = r#"
from base import BaseRouter
from service import ContainerService
raise RuntimeError("Source must not execute")
class ContainerRouter(BaseRouter):
    def __init__(self, logger, service: ContainerService, router):
        super().__init__(logger, service, router)
    def register(self):
        def checkin(request):
            return self.service.checkin_containers(request)
"#;
        std::fs::write(
            root.path().join("service.py"),
            r#"
raise RuntimeError("Service must not execute")
class ContainerService:
    def checkin_containers(self, request):
        pass
"#,
        )
        .unwrap();
        let extras = (0..13)
            .map(|i| format!("p{i}"))
            .collect::<Vec<_>>()
            .join(", ");
        let wide_base = base.replace(
            "self, logger, service, router",
            &format!("self, logger, service, router, {extras}"),
        );
        let wide_source = source.replace(
            "service: ContainerService, router",
            &format!("service: ContainerService, router, {extras}"),
        );
        let wide_keywords = format!("logger, service, router, {extras}")
            .split(", ")
            .map(|name| format!("{name}={name}"))
            .collect::<Vec<_>>()
            .join(", ");
        let cases = [
            ("bounded_arguments", wide_source.replace("super().__init__(logger, service, router)", &format!("super().__init__(logger, service, router, {extras})")), wide_base.clone(), true),
            ("truncated_excess_keywords", wide_source.replace("super().__init__(logger, service, router)", &format!("super().__init__({wide_keywords}, missing=service)")), wide_base.clone(), false),
            ("truncated_excess_arguments", wide_source.replace("super().__init__(logger, service, router)", &format!("super().__init__(logger, service, router, {extras}, service)")), wide_base, false),
            ("positional", source.to_string(), base.to_string(), true),
            ("generic_base", source.replace("ContainerRouter(BaseRouter)", "ContainerRouter(BaseRouter[ContainerService])"), base.to_string(), true),
            ("dynamic_base", source.replace("ContainerRouter(BaseRouter)", "ContainerRouter(BaseRouter[choose()])"), base.to_string(), false),
            ("shadow_super", source.replace("        super()", "        super = other\n        super()"), base.to_string(), false),
            ("wildcard_import", source.replace("from base import BaseRouter", "from base import BaseRouter\nfrom unknown import *"), base.to_string(), false),
            ("keyword", source.replace("super().__init__(logger, service, router)", "super().__init__(logger=logger, service=service, router=router)"), base.to_string(), true),
            ("unrelated_import", source.replace("            return self", "            from local_stub import unrelated\n            return self"), base.to_string(), true),
            ("same_class", source.replace("super().__init__(logger, service, router)", "self.service = service"), base.to_string(), true),
            ("shadow", source.replace("def checkin(request)", "def checkin(self, request)"), base.to_string(), false),
            ("multiple_bases", source.replace("ContainerRouter(BaseRouter)", "ContainerRouter(BaseRouter, Other)"), base.to_string(), false),
            ("rebound_parameter", source.replace("        super()", "        service = other\n        super()"), base.to_string(), false),
            ("imported_parameter", source.replace("        super()", "        from local_stub import service\n        super()"), base.to_string(), false),
            ("aliased_parameter", source.replace("        super()", "        from local_stub import replacement as service\n        super()"), base.to_string(), false),
            ("import_module_parameter", source.replace("        super()", "        import local_stub as service\n        super()"), base.to_string(), false),
            ("import_dotted_parameter", source.replace("        super()", "        import service.stub\n        super()"), base.to_string(), false),
            ("declared_parameter", source.replace("        super()", "        def service():\n            pass\n        super()"), base.to_string(), false),
            ("class_parameter", source.replace("        super()", "        class service:\n            pass\n        super()"), base.to_string(), false),
            ("base_imported_parameter", source.to_string(), base.replace("        self.service", "        from local_stub import service\n        self.service"), false),
            ("same_class_imported_parameter", source.replace("super().__init__(logger, service, router)", "from local_stub import service\n        self.service = service"), base.to_string(), false),
            ("direct_imported_parameter", source.replace("super().__init__(logger, service, router)", "from local_stub import service\n        self.service = service").replace("        def checkin(request):\n            return", "        return"), base.to_string(), false),
            ("direct_imported_self", source.replace("super().__init__(logger, service, router)", "from local_stub import self\n        self.service = service").replace("        def checkin(request):\n            return", "        return"), base.to_string(), false),
            ("imported_outer_self", source.replace("        def checkin", "        from local_stub import replacement as self\n        def checkin"), base.to_string(), false),
            ("imported_nested_self", source.replace("            return self", "            from local_stub import self\n            return self"), base.to_string(), false),
            ("declared_outer_self", source.replace("        def checkin", "        def self():\n            pass\n        def checkin"), base.to_string(), false),
            ("imported_constructor_self", source.replace("        super()", "        from local_stub import self\n        super()"), base.to_string(), false),
            ("rebound_base_parameter", source.to_string(), base.replace("        self.service", "        service = other\n        self.service"), false),
            ("rebound_self", source.replace("        def checkin", "        self = other\n        def checkin"), base.to_string(), false),
            ("late_rebound_self", source.replace("            return self", "            self = other\n            return self"), base.to_string(), false),
            ("overwrite_field", source.replace("        def checkin", "        self.service = other\n        def checkin"), base.to_string(), false),
            ("static_self", source.replace("    def register", "    @staticmethod\n    def register"), base.to_string(), false),
            ("repeated_init", source.replace("    def register", "    def __init__(self):\n        pass\n    def register"), base.to_string(), false),
            ("repeated_forward", source.replace("        super().__init__(logger, service, router)", "        super().__init__(logger, service, router)\n        super().__init__(logger, service, router)"), base.to_string(), false),
            ("ambiguous_import", source.replace("from base import BaseRouter", "from base import BaseRouter\nfrom unknown import BaseRouter"), base.to_string(), false),
            ("generic_service_parameter", source.replace("ContainerRouter(BaseRouter)", "ContainerRouter(BaseRouter[ContainerService])"), base.replace("class BaseRouter:", "from typing import Generic, TypeVar\nServiceType = TypeVar(\"ServiceType\")\nclass BaseRouter(Generic[ServiceType]):").replace("self, logger, service, router", "self, logger, service: ServiceType, router"), true),
            ("signature_free", source.to_string(), base.to_string(), false),
            ("truncated_signature", source.to_string(), base.to_string(), false),
            ("starargs", source.replace("super().__init__(logger, service, router)", "super().__init__(*args)"), base.to_string(), false),
            ("starkwargs", source.replace("super().__init__(logger, service, router)", "super().__init__(logger, **kwargs)"), base.to_string(), false),
            ("base_vararg", source.to_string(), base.replace("self, logger, service, router", "self, logger, service, router, *extra"), false),
            ("base_kwarg", source.to_string(), base.replace("self, logger, service, router", "self, logger, service, router, **extra"), false),
            ("child_vararg", source.replace("service: ContainerService, router", "service: ContainerService, router, *extra"), base.to_string(), false),
            ("positional_only", source.replace("super().__init__(logger, service, router)", "super().__init__(logger, service, router=router)"), base.replace("self, logger, service, router", "self, logger, service, /, *, router"), true),
            ("positional_only_keyword", source.replace("super().__init__(logger, service, router)", "super().__init__(logger=logger, service=service, router=router)"), base.replace("self, logger, service, router", "self, logger, service, /, router"), false),
            ("keyword_only", source.replace("super().__init__(logger, service, router)", "super().__init__(logger, router=router, service=service)"), base.replace("self, logger, service, router", "self, logger, *, service, router"), true),
            ("keyword_only_positional", source.to_string(), base.replace("self, logger, service, router", "self, logger, *, service, router"), false),
            ("duplicate_argument", source.replace("super().__init__(logger, service, router)", "super().__init__(logger, service, router=router, service=service)"), base.to_string(), false),
            ("unknown_keyword", source.replace("super().__init__(logger, service, router)", "super().__init__(logger, service=service, missing=router)"), base.to_string(), false),
            ("missing_argument", source.replace("super().__init__(logger, service, router)", "super().__init__(logger, service)"), base.to_string(), false),
            ("extra_argument", source.replace("super().__init__(logger, service, router)", "super().__init__(logger, service, router, service)"), base.to_string(), false),
            ("mismatched_order", source.to_string(), base.replace("self, logger, service, router", "self, service, logger, router"), false),
            ("swapped_positionals", source.replace("super().__init__(logger, service, router)", "super().__init__(service, logger, router)"), base.to_string(), false),
        ];
        for (name, source, base, expected) in cases {
            std::fs::write(root.path().join("router.py"), source).unwrap();
            std::fs::write(root.path().join("base.py"), base).unwrap();
            let mut e =
                crate::analysis::extract(root.path(), std::path::Path::new(&python)).unwrap();
            if name == "signature_free" {
                e.facts.retain(|f| f.kind != "signature");
            }
            if name == "truncated_signature" {
                e.facts
                    .iter_mut()
                    .find(|f| f.kind == "signature" && f.symbol == "base.BaseRouter.__init__")
                    .unwrap()
                    .value = r#"{"truncated":true}"#.into();
            }
            assert!(e.complete, "{name}");
            let anchor = if name.starts_with("direct_") {
                "ContainerRouter.register"
            } else {
                "ContainerRouter.register.checkin"
            };
            let p = prepare(&e, &options(anchor, 1, 200)).unwrap();
            check(&e, &p, 200);
            assert_eq!(links(&p), usize::from(expected), "{name}");
            if expected {
                let proof = p.input["focus"]["links"][0]["evidenceIds"]
                    .as_array()
                    .unwrap();
                for (kind, symbol) in [
                    ("class", "router.ContainerRouter"),
                    ("function", "router.ContainerRouter.register"),
                    (
                        "parameter_annotation",
                        "router.ContainerRouter.__init__.service",
                    ),
                    ("class", "service.ContainerService"),
                    ("function", "service.ContainerService.checkin_containers"),
                ] {
                    assert!(
                        e.facts.iter().any(|f| f.kind == kind
                            && f.symbol == symbol
                            && proof.contains(&json!(f.id))),
                        "{name}: {kind} {symbol}"
                    );
                }
                for kind in ["binding", "from_import"] {
                    assert!(
                        e.facts
                            .iter()
                            .any(|f| f.kind == kind && proof.contains(&json!(f.id))),
                        "{name}: {kind}"
                    );
                }
                if name != "same_class" {
                    for (kind, symbol) in [
                        ("base", "router.ContainerRouter"),
                        ("class", "base.BaseRouter"),
                        ("function", "base.BaseRouter.__init__"),
                        ("signature", "base.BaseRouter.__init__"),
                        ("signature", "router.ContainerRouter.__init__"),
                    ] {
                        assert!(
                            e.facts.iter().any(|f| f.kind == kind
                                && f.symbol == symbol
                                && proof.contains(&json!(f.id))),
                            "{name}: {kind} {symbol}"
                        );
                    }
                    assert!(e.facts.iter().any(|f| f.kind == "call"
                        && f.value.starts_with("super().__init__(")
                        && proof.contains(&json!(f.id))));
                }
            } else {
                assert!(
                    !p.input["focus"]["unresolved"]
                        .as_array()
                        .unwrap()
                        .is_empty(),
                    "{name}"
                );
            }
        }
    }

    #[test]
    fn exact_suffix_id_and_invalid_options() {
        let e = base();
        for name in ["pkg.service.Service.run", "Service.run", "f2"] {
            let p = prepare(&e, &options(name, 0, 20)).unwrap();
            check(&e, &p, 20);
            assert_eq!(
                p.input["focus"]["anchor"]["symbol"],
                "pkg.service.Service.run"
            );
        }
        for o in [
            options(" ", 0, 1),
            options("Service.run", 6, 1),
            options("Service.run", 0, 0),
            options("Service.run", 0, 2001),
        ] {
            assert!(prepare(&e, &o).is_err());
        }
    }

    #[test]
    fn ambiguous_and_missing_list_locations() {
        let mut e = base();
        fact(
            &mut e,
            "other.py",
            "function",
            "other.Service.run",
            5,
            10,
            "run",
        );
        let error = prepare(&e, &options("Service.run", 1, 20)).err().unwrap();
        assert!(
            error.contains("ambiguous")
                && error.contains("other.py:5")
                && error.contains("pkg/service.py:20")
        );
        assert!(prepare(&e, &options("missing", 1, 20))
            .err()
            .unwrap()
            .contains("not found"));
    }

    #[test]
    fn helpers_self_calls_cycles_and_depth() {
        let mut e = base();
        fact(
            &mut e,
            "pkg/service.py",
            "parameter",
            "pkg.service.Service.run.self",
            20,
            20,
            "self",
        );
        fact(
            &mut e,
            "pkg/service.py",
            "call",
            "pkg.service.Service.run",
            22,
            22,
            "self.next()",
        );
        fact(
            &mut e,
            "pkg/service.py",
            "function",
            "pkg.service.Service.next",
            60,
            80,
            "next",
        );
        fact(
            &mut e,
            "pkg/service.py",
            "call",
            "pkg.service.Service.next",
            65,
            65,
            "helper()",
        );
        fact(
            &mut e,
            "pkg/service.py",
            "function",
            "pkg.service.helper",
            110,
            150,
            "helper",
        );
        fact(
            &mut e,
            "pkg/service.py",
            "call",
            "pkg.service.helper",
            120,
            120,
            "helper()",
        );
        for (hops, expected) in [(0, 0), (1, 1), (2, 2), (5, 3)] {
            let p = prepare(&e, &options("Service.run", hops, 100)).unwrap();
            check(&e, &p, 100);
            assert_eq!(links(&p), expected);
            if hops < 2 {
                assert_eq!(p.input["focus"]["truncated"], true);
            }
        }
    }

    fn imported_receiver(relative: bool, constructed: bool) -> Evidence {
        let mut e = base();
        let import = json!({"module": if relative { "dep" } else { "pkg.dep" }, "level": if relative { 1 } else { 0 }, "names": [{"name": "Dependency", "asname": "D"}]});
        fact(
            &mut e,
            "pkg/service.py",
            "from_import",
            "pkg.service",
            1,
            1,
            &import.to_string(),
        );
        if constructed {
            fact(
                &mut e,
                "pkg/service.py",
                "binding",
                "pkg.service.Service.run",
                21,
                21,
                r#"{"target":"obj","source":"D()"}"#,
            );
            fact(
                &mut e,
                "pkg/service.py",
                "call",
                "pkg.service.Service.run",
                22,
                22,
                "obj.execute()",
            );
        } else {
            fact(
                &mut e,
                "pkg/service.py",
                "function",
                "pkg.service.Service.__init__",
                3,
                15,
                "__init__",
            );
            fact(
                &mut e,
                "pkg/service.py",
                "parameter_annotation",
                "pkg.service.Service.__init__.dep",
                3,
                3,
                "Optional[D]",
            );
            fact(
                &mut e,
                "pkg/service.py",
                "binding",
                "pkg.service.Service.__init__",
                5,
                5,
                r#"{"target":"self.dep","source":"dep"}"#,
            );
            fact(
                &mut e,
                "pkg/service.py",
                "call",
                "pkg.service.Service.run",
                22,
                22,
                "self.dep.execute()",
            );
        }
        fact(&mut e, "pkg/dep.py", "module", "pkg.dep", 1, 100, "pkg.dep");
        fact(
            &mut e,
            "pkg/dep.py",
            "class",
            "pkg.dep.Dependency",
            2,
            90,
            "Dependency",
        );
        fact(
            &mut e,
            "pkg/dep.py",
            "function",
            "pkg.dep.Dependency.execute",
            5,
            20,
            "execute",
        );
        e
    }

    #[test]
    fn aliases_relative_imports_and_typed_constructor_fields() {
        for relative in [false, true] {
            for constructed in [false, true] {
                let e = imported_receiver(relative, constructed);
                let p = prepare(&e, &options("Service.run", 1, 100)).unwrap();
                check(&e, &p, 100);
                assert_eq!(links(&p), 1, "{relative} {constructed}");
                assert_eq!(
                    p.input["focus"]["links"][0]["kind"],
                    if constructed {
                        "constructed_receiver_candidate"
                    } else {
                        "annotated_receiver_candidate"
                    }
                );
                assert!(p.input["facts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|f| f["kind"] == "from_import"));
            }
        }
    }

    #[test]
    fn imported_module_alias_callable() {
        let mut e = base();
        fact(
            &mut e,
            "pkg/service.py",
            "import",
            "pkg.service",
            1,
            1,
            r#"{"module":"","level":0,"names":[{"name":"pkg.util","asname":"u"}]}"#,
        );
        fact(
            &mut e,
            "pkg/service.py",
            "call",
            "pkg.service.Service.run",
            23,
            23,
            "u.helper()",
        );
        fact(
            &mut e,
            "pkg/util.py",
            "module",
            "pkg.util",
            1,
            100,
            "pkg.util",
        );
        fact(
            &mut e,
            "pkg/util.py",
            "function",
            "pkg.util.helper",
            2,
            10,
            "helper",
        );
        assert_eq!(
            links(&prepare(&e, &options("Service.run", 1, 100)).unwrap()),
            1
        );
    }

    #[test]
    fn unrelated_method_and_external_sdk_are_boundaries() {
        let mut e = imported_receiver(false, true);
        for call in [
            "unknown.execute()",
            "self.ctx.client.execute()",
            "self.missing()",
            "execute()",
        ] {
            fact(
                &mut e,
                "pkg/service.py",
                "call",
                "pkg.service.Service.run",
                25,
                25,
                call,
            );
        }
        let p = prepare(&e, &options("Service.run", 2, 100)).unwrap();
        check(&e, &p, 100);
        assert_eq!(links(&p), 1);
        assert_eq!(p.input["focus"]["unresolved"].as_array().unwrap().len(), 4);
        e.facts
            .iter_mut()
            .find(|f| f.kind == "binding")
            .unwrap()
            .value = r#"{"target":"obj","source":"Inject(<literal>)"}"#.into();
        assert_eq!(
            links(&prepare(&e, &options("Service.run", 2, 100)).unwrap()),
            0
        );
    }

    #[test]
    fn reassignment_and_same_line_ambiguity_do_not_use_old_type() {
        for line in [21, 22] {
            let mut e = imported_receiver(false, true);
            fact(
                &mut e,
                "pkg/service.py",
                "binding",
                "pkg.service.Service.run",
                line,
                line,
                r#"{"target":"obj","source":"External()"}"#,
            );
            assert_eq!(
                links(&prepare(&e, &options("Service.run", 1, 100)).unwrap()),
                0
            );
        }
    }

    #[test]
    fn nested_class_does_not_inherit_outer_field_context() {
        let mut e = imported_receiver(false, false);
        fact(
            &mut e,
            "pkg/service.py",
            "class",
            "pkg.service.Service.Inner",
            30,
            49,
            "Inner",
        );
        fact(
            &mut e,
            "pkg/service.py",
            "function",
            "pkg.service.Service.Inner.run",
            31,
            45,
            "run",
        );
        fact(
            &mut e,
            "pkg/service.py",
            "call",
            "pkg.service.Service.Inner.run",
            35,
            35,
            "self.dep.execute()",
        );
        let p = prepare(&e, &options("Inner.run", 1, 100)).unwrap();
        assert_eq!(links(&p), 0);
        assert!(!p.input["facts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["kind"] == "binding"));
    }

    #[test]
    fn repeated_class_and_constructor_do_not_reuse_earlier_binding() {
        let mut e = imported_receiver(false, false);
        fact(
            &mut e,
            "pkg/service.py",
            "function",
            "pkg.service.Service.__init__",
            16,
            19,
            "__init__",
        );
        assert_eq!(
            links(&prepare(&e, &options("Service.run", 1, 100)).unwrap()),
            0
        );
        fact(
            &mut e,
            "pkg/service.py",
            "class",
            "pkg.service.Service",
            110,
            190,
            "Service",
        );
        let id = fact(
            &mut e,
            "pkg/service.py",
            "function",
            "pkg.service.Service.run",
            120,
            150,
            "run",
        );
        fact(
            &mut e,
            "pkg/service.py",
            "call",
            "pkg.service.Service.run",
            125,
            125,
            "self.dep.execute()",
        );
        assert_eq!(links(&prepare(&e, &options(&id, 1, 100)).unwrap()), 0);
    }

    #[test]
    fn literal_annotations_and_ambiguous_unions_are_not_evaluated() {
        for value in [
            "<literal>",
            "Union[[D, Unknown]]",
            "D BitOr <literal>",
            "Any",
        ] {
            let mut e = imported_receiver(false, false);
            e.facts
                .iter_mut()
                .find(|f| f.kind == "parameter_annotation")
                .unwrap()
                .value = value.into();
            let p = prepare(&e, &options("Service.run", 2, 100)).unwrap();
            assert_eq!(links(&p), 0, "{value}");
            assert!(!p.input["focus"]["unresolved"]
                .as_array()
                .unwrap()
                .is_empty());
        }
    }

    #[test]
    fn fact_caps_include_context_and_small_input_still_focuses() {
        let mut e = imported_receiver(false, false);
        fact(
            &mut e,
            "unrelated.py",
            "function",
            "unrelated.noop",
            1,
            10,
            "noop",
        );
        for max in [1, 2, 4, 8, 20] {
            let p = prepare(&e, &options("Service.run", 2, max)).unwrap();
            check(&e, &p, max);
            assert!(!p.input["facts"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["symbol"] == "unrelated.noop"));
        }
    }

    #[test]
    fn actual_unicode_byte_bound_and_anchor_failure() {
        let mut e = imported_receiver(false, false);
        e.snapshot_id = "\u{e9}\\\"\n".repeat(2000);
        for n in 0..100 {
            fact(
                &mut e,
                "pkg/service.py",
                "return_annotation",
                "pkg.service.Service.run",
                24 + n % 10,
                24 + n % 10,
                &"\u{e9}\\\"\n".repeat(1500),
            );
        }
        let p = prepare(&e, &options("Service.run", 2, 2000)).unwrap();
        check(&e, &p, 2000);
        assert_eq!(p.input["focus"]["truncated"], true);
        e.facts[2].value = "x".repeat(MAX_INPUT_BYTES);
        assert!(prepare(&e, &options("Service.run", 2, 2000)).is_err());
    }

    #[test]
    fn fact_and_file_permutations_are_deterministic() {
        let mut e = imported_receiver(true, false);
        let p = prepare(&e, &options("Service.run", 3, 100)).unwrap();
        e.facts.reverse();
        e.files.reverse();
        let q = prepare(&e, &options("Service.run", 3, 100)).unwrap();
        assert_eq!(p.input, q.input);
        assert_eq!(
            serde_json::to_value(p.coverage).unwrap(),
            serde_json::to_value(q.coverage).unwrap()
        );
    }

    #[test]
    fn repeated_outer_function_cannot_borrow_previous_nested_helper() {
        let mut e = evidence();
        fact(&mut e, "m.py", "module", "m", 1, 200, "m");
        fact(&mut e, "m.py", "function", "m.run", 2, 50, "run");
        fact(&mut e, "m.py", "function", "m.run.helper", 5, 10, "helper");
        let id = fact(&mut e, "m.py", "function", "m.run", 60, 100, "run");
        fact(&mut e, "m.py", "call", "m.run", 65, 65, "helper()");
        let p = prepare(&e, &options(&id, 2, 100)).unwrap();
        assert_eq!(links(&p), 0);
        assert_eq!(p.input["focus"]["unresolved"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn decorators_are_retained_but_static_self_is_not_resolved() {
        let mut e = base();
        fact(
            &mut e,
            "pkg/service.py",
            "decorator",
            "pkg.service.Service",
            1,
            1,
            "register(<literal>)",
        );
        fact(
            &mut e,
            "pkg/service.py",
            "decorator",
            "pkg.service.Service.run",
            19,
            19,
            "staticmethod",
        );
        fact(
            &mut e,
            "pkg/service.py",
            "call",
            "pkg.service.Service.run",
            22,
            22,
            "self.next()",
        );
        fact(
            &mut e,
            "pkg/service.py",
            "function",
            "pkg.service.Service.next",
            60,
            80,
            "next",
        );
        let p = prepare(&e, &options("Service.run", 2, 100)).unwrap();
        assert_eq!(links(&p), 0);
        assert_eq!(
            p.input["facts"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|f| f["kind"] == "decorator")
                .count(),
            2
        );
    }

    #[test]
    fn rebound_self_method_does_not_follow_original_declaration() {
        let mut e = base();
        fact(
            &mut e,
            "pkg/service.py",
            "function",
            "pkg.service.Service.next",
            60,
            80,
            "next",
        );
        fact(
            &mut e,
            "pkg/service.py",
            "binding",
            "pkg.service.Service.run",
            21,
            21,
            r#"{"target":"self.next","source":"external"}"#,
        );
        fact(
            &mut e,
            "pkg/service.py",
            "call",
            "pkg.service.Service.run",
            22,
            22,
            "self.next()",
        );
        assert_eq!(
            links(&prepare(&e, &options("Service.run", 2, 100)).unwrap()),
            0
        );
    }

    #[test]
    fn typed_and_untyped_parameters_shadow_global_helpers_and_imports() {
        for typed in [false, true] {
            for imported in [false, true] {
                let mut e = base();
                fact(
                    &mut e,
                    "pkg/service.py",
                    "parameter",
                    "pkg.service.Service.run.helper",
                    20,
                    20,
                    "helper",
                );
                if typed {
                    fact(
                        &mut e,
                        "pkg/service.py",
                        "parameter_annotation",
                        "pkg.service.Service.run.helper",
                        20,
                        20,
                        "Callable",
                    );
                }
                if imported {
                    fact(
                        &mut e,
                        "pkg/service.py",
                        "from_import",
                        "pkg.service",
                        1,
                        1,
                        r#"{"module":"other","level":0,"names":[{"name":"helper","asname":null}]}"#,
                    );
                    fact(&mut e, "other.py", "module", "other", 1, 10, "other");
                    fact(
                        &mut e,
                        "other.py",
                        "function",
                        "other.helper",
                        2,
                        10,
                        "helper",
                    );
                } else {
                    fact(
                        &mut e,
                        "pkg/service.py",
                        "function",
                        "pkg.service.helper",
                        110,
                        150,
                        "helper",
                    );
                }
                fact(
                    &mut e,
                    "pkg/service.py",
                    "call",
                    "pkg.service.Service.run",
                    22,
                    22,
                    "helper()",
                );
                let p = prepare(&e, &options("Service.run", 2, 100)).unwrap();
                check(&e, &p, 100);
                assert_eq!(links(&p), 0, "typed={typed}, imported={imported}");
                assert_eq!(p.input["focus"]["unresolved"].as_array().unwrap().len(), 1);
            }
        }
    }

    #[test]
    fn parameter_body_receiver_is_unresolved_but_own_annotation_uses_outer_names() {
        let mut e = imported_receiver(false, false);
        let annotation = e
            .facts
            .iter_mut()
            .find(|f| f.kind == "parameter_annotation")
            .unwrap();
        annotation.symbol = "pkg.service.Service.__init__.D".into();
        annotation.value = "D".into();
        e.facts
            .iter_mut()
            .find(|f| f.kind == "binding")
            .unwrap()
            .value = r#"{"target":"self.dep","source":"D"}"#.into();
        fact(
            &mut e,
            "pkg/service.py",
            "parameter",
            "pkg.service.Service.__init__.D",
            3,
            3,
            "D",
        );
        fact(
            &mut e,
            "pkg/service.py",
            "parameter",
            "pkg.service.Service.run.D",
            20,
            20,
            "D",
        );
        fact(
            &mut e,
            "pkg/service.py",
            "parameter_annotation",
            "pkg.service.Service.run.D",
            20,
            20,
            "D",
        );
        fact(
            &mut e,
            "pkg/service.py",
            "call",
            "pkg.service.Service.run",
            23,
            23,
            "D.execute()",
        );
        let p = prepare(&e, &options("Service.run", 2, 100)).unwrap();
        check(&e, &p, 100);
        assert_eq!(links(&p), 1);
        assert_eq!(
            p.input["focus"]["links"][0]["kind"],
            "annotated_receiver_candidate"
        );
        assert_eq!(p.input["focus"]["unresolved"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn parameters_of_previous_function_instance_do_not_shadow_later_body() {
        let mut e = evidence();
        fact(&mut e, "m.py", "module", "m", 1, 100, "m");
        fact(&mut e, "m.py", "function", "m.helper", 2, 10, "helper");
        fact(&mut e, "m.py", "function", "m.run", 20, 30, "run");
        fact(
            &mut e,
            "m.py",
            "parameter",
            "m.run.helper",
            20,
            20,
            "helper",
        );
        let id = fact(&mut e, "m.py", "function", "m.run", 40, 50, "run");
        fact(&mut e, "m.py", "call", "m.run", 45, 45, "helper()");
        assert_eq!(links(&prepare(&e, &options(&id, 1, 100)).unwrap()), 1);
    }

    #[test]
    fn relative_imports_respect_module_and_package_root_boundaries() {
        for (path, module, level, valid) in [
            ("pkg/service.py", "pkg.service", 1, true),
            ("pkg/service.py", "pkg.service", 2, false),
            ("pkg/__init__.py", "pkg", 1, true),
            ("pkg/__init__.py", "pkg", 2, false),
            ("pkg/sub/__init__.py", "pkg.sub", 2, true),
            ("pkg/sub/__init__.py", "pkg.sub", 3, false),
            ("service.py", "service", 1, false),
        ] {
            let mut e = evidence();
            fact(&mut e, path, "module", module, 1, 100, module);
            let symbol = format!("{module}.run");
            fact(&mut e, path, "function", &symbol, 10, 20, "run");
            let import = json!({"module": "dep", "level": level, "names": [{"name": "helper", "asname": null}]});
            fact(
                &mut e,
                path,
                "from_import",
                module,
                2,
                2,
                &import.to_string(),
            );
            fact(&mut e, path, "call", &symbol, 15, 15, "helper()");
            fact(&mut e, "pkg/dep.py", "module", "pkg.dep", 1, 20, "pkg.dep");
            fact(
                &mut e,
                "pkg/dep.py",
                "function",
                "pkg.dep.helper",
                2,
                10,
                "helper",
            );
            fact(&mut e, "dep.py", "module", "dep", 1, 20, "dep");
            fact(&mut e, "dep.py", "function", "dep.helper", 2, 10, "helper");
            // An invalid matching import also must not fall back to this name.
            if !valid {
                fact(
                    &mut e,
                    path,
                    "function",
                    &format!("{module}.helper"),
                    30,
                    40,
                    "helper",
                );
            }
            let p = prepare(&e, &options(&symbol, 1, 100)).unwrap();
            check(&e, &p, 100);
            assert_eq!(links(&p), usize::from(valid), "{path}, level={level}");
            if !valid {
                assert_eq!(p.input["focus"]["unresolved"].as_array().unwrap().len(), 1);
            }
        }
    }

    #[test]
    fn target_only_invalidations_block_import_and_declaration_fallback() {
        for imported in [false, true] {
            let mut e = base();
            let module = if imported { "dep" } else { "pkg.service" };
            if imported {
                fact(
                    &mut e,
                    "pkg/service.py",
                    "from_import",
                    "pkg.service",
                    1,
                    1,
                    r#"{"module":"dep","level":0,"names":[{"name":"helper","asname":null}]}"#,
                );
                fact(&mut e, "dep.py", "module", "dep", 1, 100, "dep");
            }
            fact(
                &mut e,
                if imported { "dep.py" } else { "pkg/service.py" },
                "function",
                &format!("{module}.helper"),
                110,
                120,
                "helper",
            );
            fact(
                &mut e,
                "pkg/service.py",
                "binding_invalidation",
                "pkg.service.Service.run",
                21,
                21,
                r#"{"target":"helper"}"#,
            );
            fact(
                &mut e,
                "pkg/service.py",
                "call",
                "pkg.service.Service.run",
                22,
                22,
                "helper()",
            );
            let p = prepare(&e, &options("Service.run", 2, 100)).unwrap();
            check(&e, &p, 100);
            assert_eq!(links(&p), 0);
            assert_eq!(p.input["focus"]["unresolved"].as_array().unwrap().len(), 1);
        }
    }

    #[cfg(unix)]
    #[test]
    fn extracted_lexical_shadowing_selects_exact_local_targets_without_execution() {
        let python =
            std::env::var_os("IDEOMANCER_TEST_PYTHON").unwrap_or_else(|| "python3.12".into());
        let available = std::process::Command::new(&python)
            .args([
                "-I",
                "-S",
                "-c",
                "import sys; sys.exit(sys.version_info < (3, 12))",
            ])
            .status();
        if !available.is_ok_and(|s| s.success()) {
            eprintln!("SKIP: focus extraction regression requires Python >=3.12");
            return;
        }
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("m.py"),
            r#"
from dep import helper as chosen, Worker
raise RuntimeError("Target modules must never execute")
def local_work():
    pass
def run():
    def chosen():
        local_work()
    chosen()
def construct():
    class Worker:
        def execute(self):
            local_work()
    obj = Worker()
    obj.execute()
def blocked():
    from unavailable import helper as chosen
    chosen()
def ambiguous():
    from dep import helper as chosen
    from unavailable import helper as chosen
    chosen()
def invalidated():
    chosen = None
    chosen()
def parameter(chosen):
    chosen()
def repeated():
    def chosen():
        local_work()
def repeated():
    chosen()
"#,
        )
        .unwrap();
        std::fs::write(
            root.path().join("dep.py"),
            r#"
raise RuntimeError("Imported modules must never execute")
def helper():
    foreign_work()
class Worker:
    def execute(self):
        foreign_work()
def foreign_work():
    pass
"#,
        )
        .unwrap();
        let e = crate::analysis::extract(root.path(), std::path::Path::new(&python)).unwrap();
        assert!(e.complete);
        assert!(e
            .facts
            .iter()
            .any(|f| f.kind == "binding_invalidation" && f.symbol == "m.invalidated"));
        assert!(e
            .facts
            .iter()
            .any(|f| f.kind == "parameter" && f.symbol == "m.parameter.chosen"));
        for (anchor, target) in [
            ("m.run", "m.run.chosen"),
            ("m.construct", "m.construct.Worker.execute"),
        ] {
            let p = prepare(&e, &options(anchor, 2, 100)).unwrap();
            check(&e, &p, 100);
            let expected = &e
                .facts
                .iter()
                .find(|f| callable(f) && f.symbol == target)
                .unwrap()
                .id;
            assert!(
                p.input["focus"]["links"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|l| l["targetFactId"] == *expected),
                "{anchor}: expected {target}"
            );
            assert!(!p.input["facts"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["path"] == "dep.py"));
        }
        for anchor in ["m.blocked", "m.ambiguous", "m.invalidated", "m.parameter"] {
            let p = prepare(&e, &options(anchor, 2, 100)).unwrap();
            assert_eq!(links(&p), 0, "{anchor}");
            assert!(!p.input["facts"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["path"] == "dep.py"));
            assert_eq!(
                p.input["focus"]["unresolved"].as_array().unwrap().len(),
                1,
                "{anchor}"
            );
        }
        // A later repeated function may use the module alias, but never the
        // nested helper belonging to an earlier declaration instance.
        let last = e
            .facts
            .iter()
            .filter(|f| callable(f) && f.symbol == "m.repeated")
            .max_by_key(|f| f.line)
            .unwrap();
        let p = prepare(&e, &options(&last.id, 1, 100)).unwrap();
        let expected = &e
            .facts
            .iter()
            .find(|f| callable(f) && f.symbol == "dep.helper")
            .unwrap()
            .id;
        assert_eq!(p.input["focus"]["links"][0]["targetFactId"], *expected);
        assert!(!p.input["facts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["symbol"] == "m.repeated.chosen"));
    }

    #[cfg(unix)]
    #[test]
    fn extracted_same_line_receiver_writes_do_not_expand_original_method() {
        let python =
            std::env::var_os("IDEOMANCER_TEST_PYTHON").unwrap_or_else(|| "python3.12".into());
        let available = std::process::Command::new(&python)
            .args([
                "-I",
                "-S",
                "-c",
                "import sys; sys.exit(sys.version_info < (3, 12))",
            ])
            .status();
        if !available.is_ok_and(|s| s.success()) {
            eprintln!("SKIP: focus extraction regression requires Python >=3.12");
            return;
        }
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("m.py"),
            r#"
raise RuntimeError("Target modules must never execute")
class Service:
    def work(self):
        original_body()
    def named(self, replacement):
        self.work = replacement; self.work()
    def literal(self):
        self.work = None; self.work()
    def root_named(self, replacement):
        self = replacement; self.work()
    def root_literal(self):
        self = None; self.work()
    def untouched(self):
        self.work()
def original_body():
    pass
"#,
        )
        .unwrap();
        let e = crate::analysis::extract(root.path(), std::path::Path::new(&python)).unwrap();
        assert!(e.complete);
        let original = e
            .facts
            .iter()
            .find(|f| callable(f) && f.symbol == "m.Service.work")
            .unwrap();
        for (method, kind) in [
            ("named", "binding"),
            ("literal", "binding_invalidation"),
            ("root_named", "binding"),
            ("root_literal", "binding_invalidation"),
        ] {
            let symbol = format!("m.Service.{method}");
            let call = e
                .facts
                .iter()
                .find(|f| f.kind == "call" && f.symbol == symbol && f.value == "self.work()")
                .unwrap();
            assert!(e
                .facts
                .iter()
                .any(|f| f.kind == kind && f.symbol == symbol && f.line == call.line));
            let p = prepare(&e, &options(&symbol, 3, 100)).unwrap();
            check(&e, &p, 100);
            assert_eq!(links(&p), 0, "{symbol}");
            assert!(!p.input["facts"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["id"] == original.id
                    || f["symbol"] == "m.Service.work"
                    || f["symbol"] == "m.original_body"));
            assert!(p.input["focus"]["unresolved"]
                .as_array()
                .unwrap()
                .iter()
                .any(|u| u["factId"] == call.id));
        }
        let p = prepare(&e, &options("m.Service.untouched", 1, 100)).unwrap();
        assert_eq!(p.input["focus"]["links"][0]["targetFactId"], original.id);
    }
}
