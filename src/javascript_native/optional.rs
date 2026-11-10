use super::*;

const STRUCTURAL_POLICY: &str = "javascript-structural-facts-1";
const RELATION_POLICY: &str = "javascript-class-relations-2";

impl<'a> File<'a> {
    fn literal(&self, n: Node<'_>) -> Option<String> {
        if n.kind() != "string" {
            return None;
        }
        let raw = self.raw(n);
        let inner = raw.get(1..raw.len().checked_sub(1)?)?;
        // Decode syntax without evaluating JavaScript. Escaped paths are retained
        // only when their value is unambiguous under both source grammars.
        let mut out = Vec::new();
        let mut cs = inner.chars();
        while let Some(c) = cs.next() {
            if c != '\\' {
                out.extend(c.encode_utf16(&mut [0; 2]).iter().copied());
                continue;
            }
            match cs.next()? {
                '\\' => out.push(b'\\' as u16),
                '\'' => out.push(b'\'' as u16),
                '"' => out.push(b'"' as u16),
                'n' => out.push(b'\n' as u16),
                'r' => out.push(b'\r' as u16),
                't' => out.push(b'\t' as u16),
                'b' => out.push(8),
                'f' => out.push(12),
                'v' => out.push(11),
                '0' => {
                    if cs.clone().next().is_some_and(|c| c.is_ascii_digit()) {
                        return None;
                    }
                    out.push(0);
                }
                '\n' | '\u{2028}' | '\u{2029}' => (),
                '\r' => {
                    if cs.clone().next() == Some('\n') {
                        cs.next();
                    }
                }
                'x' => {
                    let s: String = cs.by_ref().take(2).collect();
                    if s.len() != 2 {
                        return None;
                    }
                    out.push(u16::from_str_radix(&s, 16).ok()?);
                }
                'u' => {
                    let s = if cs.clone().next() == Some('{') {
                        cs.next();
                        let mut s = String::new();
                        loop {
                            let c = cs.next()?;
                            if c == '}' {
                                break;
                            }
                            s.push(c);
                            if s.len() > 6 {
                                return None;
                            }
                        }
                        s
                    } else {
                        let s: String = cs.by_ref().take(4).collect();
                        if s.len() != 4 {
                            return None;
                        }
                        s
                    };
                    let value = u32::from_str_radix(&s, 16).ok()?;
                    if value <= 0xffff {
                        out.push(value as u16);
                    } else {
                        out.extend(
                            char::from_u32(value)?
                                .encode_utf16(&mut [0; 2])
                                .iter()
                                .copied(),
                        );
                    }
                }
                c if c.is_ascii_digit() => return None,
                c => out.extend(c.encode_utf16(&mut [0; 2]).iter().copied()),
            }
        }
        String::from_utf16(&out).ok()
    }
    fn specifier(&self, n: Node<'_>) -> Option<String> {
        let s = self.literal(n)?;
        (s.len() <= 512
            && !s.is_empty()
            && !s.starts_with('/')
            && s.bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_@./$-".contains(&c)))
        .then_some(s)
    }
    fn require(&self, n: Node<'a>) -> bool {
        n.kind() == "identifier"
            && self.raw(n) == "require"
            && self.lookup(self.node_scope[&n.id()], "require").is_none()
            && !self.global_writes.contains("require")
    }
    fn args(&self, n: Node<'a>) -> Vec<Node<'a>> {
        field(n, "arguments")
            .map(children)
            .unwrap_or_default()
            .into_iter()
            .filter(|n| n.kind() != "comment")
            .collect()
    }
    fn top(&self, n: Node<'_>) -> bool {
        n.parent().is_some_and(|p| p.kind() == "program")
    }
    fn top_expression(&self, n: Node<'_>) -> bool {
        n.parent()
            .is_some_and(|p| p.kind() == "expression_statement" && self.top(p))
    }
    fn import_items(&self, n: Node<'a>) -> Vec<(Node<'a>, String, String, bool)> {
        let mut out = vec![];
        let mut todo = children(n);
        while let Some(c) = todo.pop() {
            match c.kind() {
                "import_specifier" => {
                    let imported = field(c, "name")
                        .and_then(|n| self.direct(n))
                        .unwrap_or("<literal>".into());
                    let local = field(c, "alias")
                        .or_else(|| field(c, "name"))
                        .and_then(|n| self.direct(n));
                    if let Some(local) = local {
                        out.push((c, imported, local, !self.has(c, "type")));
                    }
                }
                "namespace_import" => {
                    if let Some(name) = children(c).last().and_then(|n| self.direct(*n)) {
                        out.push((c, "*".into(), name, true));
                    }
                }
                "import_clause" => {
                    for x in children(c) {
                        if x.kind() == "identifier" {
                            out.push((c, "default".into(), self.raw(x).into(), true));
                        } else {
                            todo.push(x);
                        }
                    }
                }
                "named_imports" => todo.extend(children(c)),
                _ => (),
            }
        }
        out.sort_by_key(|(n, _, _, _)| n.start_byte());
        out
    }
    fn export_candidate(
        &self,
        n: Node<'a>,
        exported: &str,
        local: &str,
        mode: &str,
    ) -> Option<Value> {
        if mode == "esm" && exported == "default" {
            return None;
        }
        let i = self.root_binding(local)?;
        let b = &self.bindings[i];
        let cls = (b.kind == "class"
            && matches!(
                b.node.kind(),
                "class_declaration" | "abstract_class_declaration"
            ))
        .then(|| self.class_target(i))
        .flatten();
        let target = cls.or(b.target)?;
        let d = self.declarations.get(&target.id())?;
        let meta: Value = serde_json::from_str(&self.facts[d.meta].value).ok()?;
        if !matches!(d.kind.as_str(), "function" | "arrow" | "class")
            || meta["ownerFactId"] != self.facts[0].id
        {
            return None;
        }
        if mode == "commonjs"
            && (cls.is_some() || b.node.kind() == "variable_declarator")
            && b.node.end_byte() > n.start_byte()
        {
            return None;
        }
        let mut v = json!({"ownerFactId":self.facts[0].id,"mode":mode,"exported":exported,"local":local,"targetFactId":self.facts[d.fact].id,"targetKind":if cls.is_some(){"class"}else{"function"},"targetMetadataFactId":self.facts[d.meta].id,"bindingGuardFactId":b.guards[0]});
        if mode == "commonjs" {
            v["referenceStart"] = json!(self.u(n.start_byte()));
            v["initializedBeforeExport"] = json!(true);
        }
        Some(v)
    }
    pub(super) fn modules(&mut self) {
        let mut dependencies = BTreeSet::new();
        let mut esm = false;
        let mut commonjs = false;
        let mut imports_safe = !self.annex && !self.dynamic;
        let mut exports_safe = !self.annex && !self.dynamic;
        let mut require_safe = !self.global_writes.contains("require");
        let mut wrappers_safe =
            !self.global_writes.contains("module") && !self.global_writes.contains("exports");
        let mut imports: Vec<(Node<'a>, usize, Value)> = vec![];
        let mut exports: Vec<(Node<'a>, Value)> = vec![];
        let all = self.all.clone();
        for n in &all {
            if self.stopped() {
                return;
            }
            if n.kind() == "with_statement" {
                imports_safe = false;
                exports_safe = false;
            }
            if n.kind() == "import_alias" {
                commonjs = true;
                if !self.has(*n, "type") {
                    imports_safe = false;
                }
            }
            if matches!(
                n.kind(),
                "identifier"
                    | "property_identifier"
                    | "shorthand_property_identifier"
                    | "shorthand_property_identifier_pattern"
                    | "type_identifier"
            ) {
                let name = self.raw(*n);
                let p = n.parent();
                let s = self.node_scope[&n.id()];
                if matches!(name, "eval" | "arguments") && self.lookup(s, name).is_none() {
                    imports_safe = false;
                    exports_safe = false;
                }
                if matches!(name, "globalThis" | "global" | "window" | "self")
                    && !p.is_some_and(|p| {
                        matches!(p.kind(), "member_expression" | "subscript_expression")
                            && field(p, "object") == Some(*n)
                    })
                {
                    imports_safe = false;
                    exports_safe = false;
                }
                if matches!(name, "require" | "module" | "exports") {
                    let property = p.is_some_and(|p| {
                        (p.kind() == "member_expression" && field(p, "property") == Some(*n))
                            || (matches!(p.kind(), "pair" | "pair_pattern")
                                && field(p, "key") == Some(*n))
                            || (p.kind() == "import_specifier"
                                && field(p, "name") == Some(*n)
                                && field(p, "alias").is_some())
                    });
                    if !property {
                        commonjs = true;
                        if name == "require" {
                            if self.lookup(s, name).is_some()
                                || !p.is_some_and(|p| call(p) && field(p, "function") == Some(*n))
                            {
                                require_safe = false;
                            }
                        } else {
                            let valid = name == "module"
                                && p.is_some_and(|p| {
                                    p.kind() == "member_expression"
                                        && field(p, "object") == Some(*n)
                                        && field(p, "property")
                                            .is_some_and(|n| self.raw(n) == "exports")
                                        && p.parent().is_some_and(|a| {
                                            a.kind() == "assignment_expression"
                                                && field(a, "left") == Some(p)
                                                && self.top_expression(a)
                                                && field(a, "right").is_some_and(|r| {
                                                    r.kind() == "object" || callable(unwrap(r))
                                                })
                                        })
                                });
                            if self.lookup(s, name).is_some() || !valid {
                                wrappers_safe = false;
                            }
                        }
                    }
                }
            }
            if matches!(n.kind(), "member_expression" | "subscript_expression")
                && field(*n, "object").is_some_and(|o| {
                    matches!(self.raw(o), "globalThis" | "global" | "window" | "self")
                })
            {
                let key = field(*n, "property")
                    .and_then(|n| self.direct(n))
                    .or_else(|| field(*n, "index").and_then(|n| self.literal(n)));
                if key.is_none() || key.as_deref() == Some("require") {
                    require_safe = false;
                }
                if key.is_none() || matches!(key.as_deref(), Some("module" | "exports" | "eval")) {
                    imports_safe = false;
                    exports_safe = false;
                }
            }
            if n.kind() == "import_statement" {
                if let Some(req) = children(*n)
                    .into_iter()
                    .find(|n| n.kind() == "import_require_clause")
                {
                    commonjs = true;
                    if !self.has(*n, "type") {
                        imports_safe = false;
                    }
                    if let Some(s) = field(req, "source").and_then(|n| self.specifier(n)) {
                        if s.starts_with("./") || s.starts_with("../") {
                            dependencies.insert(s);
                        }
                    } else {
                        imports_safe = false;
                        exports_safe = false;
                    }
                    continue;
                }
                esm = true;
                let spec = field(*n, "source").and_then(|n| self.specifier(n));
                if let Some(s) = &spec {
                    if s.starts_with("./") || s.starts_with("../") {
                        dependencies.insert(s.clone());
                    }
                } else {
                    imports_safe = false;
                    exports_safe = false;
                }
                if !self.top(*n)
                    || self.has(*n, "assert")
                    || self.has(*n, "with")
                    || children(*n)
                        .iter()
                        .any(|n| matches!(n.kind(), "import_attribute" | "import_require_clause"))
                {
                    imports_safe = false;
                }
                let type_only = self.has(*n, "type");
                if !type_only {
                    for (item, imported, local, runtime) in self.import_items(*n) {
                        if !runtime || matches!(imported.as_str(), "*" | "default" | "<literal>") {
                            continue;
                        }
                        if let (Some(spec), Some(b)) = (&spec, self.root_binding(&local)) {
                            if self.bindings[b].node == item {
                                imports.push((item,b,json!({"ownerFactId":self.facts[0].id,"mode":"esm","moduleSpecifier":spec,"imported":imported,"local":local,"bindingGuardFactId":self.bindings[b].guards[0]})));
                            }
                        }
                    }
                }
            }
            if call(*n) {
                let Some(c) = field(*n, "function") else {
                    continue;
                };
                let dynamic = c.kind() == "import";
                let loader = c.kind() == "identifier" && self.raw(c) == "require";
                if self.type_position(*n) {
                    if dynamic {
                        if let Some(s) = self.args(*n).first().and_then(|n| self.specifier(*n)) {
                            if s.starts_with("./") || s.starts_with("../") {
                                dependencies.insert(s);
                            }
                        } else {
                            imports_safe = false;
                            exports_safe = false;
                        }
                    }
                    continue;
                }
                // Parenthesized/asserted loader calls still constrain cycles,
                // but do not establish the narrow named-import proof shape.
                let dependency_callee = unwrap(c);
                if dependency_callee != c && self.require(dependency_callee) {
                    if let Some(spec) = self.args(*n).first().and_then(|n| self.specifier(*n)) {
                        if spec.starts_with("./") || spec.starts_with("../") {
                            dependencies.insert(spec);
                        }
                    } else {
                        imports_safe = false;
                        exports_safe = false;
                    }
                }
                if dynamic || loader {
                    if loader && !self.require(c) {
                        imports_safe = false;
                        exports_safe = false;
                        continue;
                    }
                    let args = self.args(*n);
                    let spec = args.first().and_then(|n| self.specifier(*n));
                    if let Some(s) = &spec {
                        if s.starts_with("./") || s.starts_with("../") {
                            dependencies.insert(s.clone());
                        }
                    } else {
                        imports_safe = false;
                        exports_safe = false;
                    }
                    if dynamic {
                        esm = true;
                        imports_safe = false;
                        exports_safe = false;
                        continue;
                    }
                    commonjs = true;
                    if args.len() != 1 || args[0].kind() != "string" {
                        imports_safe = false;
                        exports_safe = false;
                    }
                    let v = n.parent();
                    let valid = v.is_some_and(|v| {
                        v.kind() == "variable_declarator"
                            && field(v, "value") == Some(*n)
                            && v.parent().is_some_and(|p| {
                                p.kind() == "lexical_declaration"
                                    && self.has(p, "const")
                                    && self.top(p)
                            })
                            && field(v, "name").is_some_and(|p| {
                                p.kind() == "object_pattern"
                                    && children(p).iter().all(|e| {
                                        matches!(
                                            e.kind(),
                                            "shorthand_property_identifier_pattern"
                                                | "pair_pattern"
                                        ) && (e.kind() != "pair_pattern"
                                            || (field(*e, "key")
                                                .and_then(|n| self.direct(n))
                                                .is_some()
                                                && field(*e, "value")
                                                    .is_some_and(|n| n.kind() == "identifier")))
                                    })
                            })
                    }) && args.len() == 1
                        && spec.is_some()
                        && !self.has(*n, "optional_chain");
                    if !valid {
                        imports_safe = false;
                        continue;
                    }
                    let v = v.unwrap();
                    for e in children(field(v, "name").unwrap()) {
                        let local = if e.kind() == "pair_pattern" {
                            field(e, "value").and_then(|n| self.direct(n))
                        } else {
                            self.direct(e)
                        };
                        let imported = if e.kind() == "pair_pattern" {
                            field(e, "key").and_then(|n| self.direct(n))
                        } else {
                            self.direct(e)
                        };
                        if let (Some(local), Some(imported)) = (local, imported) {
                            if let Some(b) = self.root_binding(&local) {
                                if self.bindings[b].node == v {
                                    imports.push((e,b,json!({"ownerFactId":self.facts[0].id,"mode":"commonjs","moduleSpecifier":spec,"imported":imported,"local":local,"initializationEnd":self.u(v.end_byte()),"bindingGuardFactId":self.bindings[b].guards[0]})));
                                }
                            }
                        }
                    }
                }
            }
            if n.kind() == "export_statement" {
                esm = true;
                if let Some(src) = field(*n, "source") {
                    if let Some(s) = self.specifier(src) {
                        if s.starts_with("./") || s.starts_with("../") {
                            dependencies.insert(s);
                        }
                    } else {
                        imports_safe = false;
                        exports_safe = false;
                    }
                }
                if self.has(*n, "type") {
                    continue;
                }
                if !self.top(*n) || self.has(*n, "default") || field(*n, "source").is_some() {
                    exports_safe = false;
                    continue;
                }
                if let Some(d) = field(*n, "declaration") {
                    if matches!(d.kind(), "interface_declaration" | "type_alias_declaration") {
                        continue;
                    }
                    let ds = if matches!(d.kind(), "lexical_declaration" | "variable_declaration") {
                        children(d)
                            .into_iter()
                            .filter(|n| n.kind() == "variable_declarator")
                            .collect()
                    } else {
                        vec![d]
                    };
                    for d in ds {
                        if let Some(name) = field(d, "name").and_then(|n| self.direct(n)) {
                            if let Some(v) = self.export_candidate(d, &name, &name, "esm") {
                                exports.push((d, v));
                            } else {
                                exports_safe = false;
                            }
                        } else {
                            exports_safe = false;
                        }
                    }
                } else if let Some(clause) = children(*n)
                    .into_iter()
                    .find(|n| n.kind() == "export_clause")
                {
                    for item in children(clause) {
                        if self.has(item, "type") {
                            continue;
                        }
                        let local = field(item, "name").and_then(|n| self.direct(n));
                        let exported = field(item, "alias")
                            .or_else(|| field(item, "name"))
                            .and_then(|n| self.direct(n));
                        if let (Some(local), Some(exported)) = (local, exported) {
                            if let Some(v) = self.export_candidate(item, &exported, &local, "esm") {
                                exports.push((item, v));
                            } else {
                                exports_safe = false;
                            }
                        } else {
                            exports_safe = false;
                        }
                    }
                } else {
                    exports_safe = false;
                }
            }
        }
        if self.export_assignments.len() > 1 {
            wrappers_safe = false;
        }
        if let Some(n) = self.export_assignments.first().copied() {
            commonjs = true;
            let rhs = field(n, "right").unwrap();
            if self.export_assignments.len() != 1
                || !wrappers_safe
                || rhs.kind() != "object"
                || !self.top_expression(n)
            {
                exports_safe = false;
            } else {
                let mut names = HashSet::new();
                for p in children(rhs) {
                    let exported = if p.kind() == "shorthand_property_identifier" {
                        self.direct(p)
                    } else {
                        field(p, "key").and_then(|n| self.direct(n))
                    };
                    let local = if p.kind() == "shorthand_property_identifier" {
                        self.direct(p)
                    } else if p.kind() == "pair" {
                        field(p, "value")
                            .filter(|n| n.kind() == "identifier")
                            .and_then(|n| self.direct(n))
                    } else {
                        None
                    };
                    if let (Some(exported), Some(local)) = (exported, local) {
                        if exported == "__proto__" || !names.insert(exported.clone()) {
                            exports_safe = false;
                        } else if let Some(v) =
                            self.export_candidate(p, &exported, &local, "commonjs")
                        {
                            exports.push((p, v));
                        } else {
                            exports_safe = false;
                        }
                    } else {
                        exports_safe = false;
                    }
                }
            }
        }
        imports_safe &= require_safe && wrappers_safe;
        exports_safe &= require_safe && wrappers_safe;
        if esm && commonjs {
            imports_safe = false;
            exports_safe = false;
        }
        self.module_mode = if esm && !commonjs {
            "esm"
        } else if commonjs && !esm {
            "commonjs"
        } else {
            "unknown"
        }
        .into();
        if imports_safe {
            for (n, b, v) in imports {
                let i = self.module_emit("js_module_import", n, v);
                self.module_imports.insert(b, i);
            }
        }
        if exports_safe {
            let mut names = HashSet::new();
            if exports
                .iter()
                .any(|(_, v)| !names.insert(v["exported"].as_str().unwrap().to_owned()))
            {
                exports_safe = false;
            }
        }
        if exports_safe {
            for (n, v) in exports {
                let i = self.module_emit("js_module_export", n, v);
                self.module_exports.insert(n.id(), i);
            }
        }
        self.esm = esm;
        self.imports_safe = imports_safe;
        self.exports_safe = exports_safe;
        self.wrappers_safe = wrappers_safe;
        self.module_emit("js_module_summary",self.all[0],json!({"ownerFactId":self.facts[0].id,"policy":"selected-relative-classes-1","mode":self.module_mode,"importsSafe":imports_safe,"exportsSafe":exports_safe,"dependencies":dependencies}));
    }
    fn import_fact(
        &mut self,
        n: Node<'a>,
        literal: Option<Node<'a>>,
        mode: &str,
        bindings: Value,
        dynamic: bool,
    ) {
        let Some(own) = self.owner(n, false) else {
            return;
        };
        let Some(spec) = literal.and_then(|n| self.specifier(n)) else {
            self.diagnostic("javascript_import_unresolved", n);
            return;
        };
        self.fact("js_import",n,json!({"ownerFactId":self.facts[own].id,"moduleSpecifier":spec,"mode":mode,"bindings":bindings,"dynamic":dynamic}),&self.facts[own].symbol.clone());
    }
    pub(super) fn calls_and_imports(&mut self) {
        for n in self.all.clone() {
            if self.stopped() {
                return;
            }
            if !self.eligible.contains(&n.id()) {
                continue;
            }
            if n.kind() == "call_expression" {
                let Some(own) = self.owner(n, false) else {
                    continue;
                };
                let Some(callee) = field(n, "function") else {
                    continue;
                };
                let f = self.fact(
                    "call",
                    n,
                    json!(self.expr(n)),
                    &self.facts[own].symbol.clone(),
                );
                let b = if callee.kind() == "identifier" {
                    self.resolve(n, self.raw(callee))
                } else {
                    None
                };
                let d = b
                    .and_then(|b| self.bindings[b].target)
                    .and_then(|t| self.declarations.get(&t.id()))
                    .cloned();
                let ib = if d.is_none() && self.imports_safe && callee.kind() == "identifier" {
                    self.unique(n, self.raw(callee)).filter(|i| {
                        let b = &self.bindings[*i];
                        b.writes == 0 && (b.kind == "import" || n.start_byte() >= b.node.end_byte())
                    })
                } else {
                    None
                };
                let imported = ib.and_then(|i| self.module_imports.get(&i)).copied();
                let mut support = vec![];
                if let Some(d) = &d {
                    support.extend([self.facts[d.fact].id.clone(), self.facts[d.meta].id.clone()]);
                    support.extend(self.bindings[b.unwrap()].guards.clone());
                } else if let Some(i) = imported {
                    support.push(self.facts[i].id.clone());
                    support.extend(self.bindings[ib.unwrap()].guards.clone());
                }
                let mut v = json!({"callFactId":self.facts[f].id,"ownerFactId":self.facts[own].id,"callee":self.expr(callee),"lookupScopeIds":self.scope_chain(n).iter().map(|s|&self.scopes[*s].id).collect::<Vec<_>>(),"supportingFactIds":support,"reason":if d.is_some(){"unique stable same-file lexical function binding"}else if imported.is_some(){"unique stable module import binding candidate"}else if self.annex{"UNKNOWN: possible sloppy block-function runtime binding"}else{"UNKNOWN: no proven local lexical function binding"}});
                if let Some(d) = &d {
                    v["targetFactId"] = json!(self.facts[d.fact].id);
                }
                if let Some(i) = imported {
                    v["importFactId"] = json!(self.facts[i].id);
                }
                self.fact("js_call", n, v.clone(), &self.facts[own].symbol.clone());
                if d.is_some() {
                    self.fact(
                        "js_local_call_candidate",
                        n,
                        v,
                        &self.facts[own].symbol.clone(),
                    );
                }
                let args = self.args(n);
                if callee.kind() == "import" {
                    self.import_fact(
                        n,
                        if args.len() == 1 { Some(args[0]) } else { None },
                        "esm",
                        json!([]),
                        true,
                    );
                } else if self.require(callee) {
                    let mut bindings = vec![];
                    if let Some(v) = n.parent().filter(|p| p.kind() == "variable_declarator") {
                        if let Some(name) = field(v, "name") {
                            if let Some(local) = self.direct(name) {
                                bindings.push(json!({"imported":"*","local":local}));
                            } else if name.kind() == "object_pattern" {
                                for e in children(name) {
                                    let local = if e.kind() == "pair_pattern" {
                                        field(e, "value").and_then(|n| self.direct(n))
                                    } else {
                                        self.direct(e)
                                    };
                                    let imported = if e.kind() == "pair_pattern" {
                                        field(e, "key").and_then(|n| self.direct(n))
                                    } else {
                                        self.direct(e)
                                    };
                                    if let (Some(local), Some(imported)) = (local, imported) {
                                        bindings.push(json!({"imported":imported,"local":local}));
                                    }
                                }
                            }
                        }
                    }
                    self.import_fact(
                        n,
                        if args.len() == 1 { Some(args[0]) } else { None },
                        "commonjs",
                        json!(bindings),
                        false,
                    );
                }
            } else if n.kind() == "import_statement" {
                if let Some(req) = children(n)
                    .into_iter()
                    .find(|n| n.kind() == "import_require_clause")
                {
                    let local = children(req)
                        .into_iter()
                        .find(|n| n.kind() == "identifier")
                        .and_then(|n| self.direct(n))
                        .unwrap_or_default();
                    self.import_fact(
                        n,
                        field(req, "source"),
                        "commonjs",
                        json!([{"imported":"*","local":local}]),
                        false,
                    );
                } else {
                    let bindings = self
                        .import_items(n)
                        .into_iter()
                        .map(|(_, imported, local, _)| json!({"imported":imported,"local":local}))
                        .collect::<Vec<_>>();
                    self.import_fact(n, field(n, "source"), "esm", json!(bindings), false);
                }
            }
        }
    }
    fn scope_chain(&self, n: Node<'_>) -> Vec<usize> {
        let mut out = vec![];
        let mut s = Some(self.node_scope[&n.id()]);
        while let Some(i) = s {
            out.push(i);
            s = self.scopes[i].parent;
        }
        out
    }
    pub(super) fn routes(&mut self) {
        let first = self.factory.and_then(|f| self.params(f).first().copied());
        let router = first.and_then(|p| self.direct(self.pname(p)));
        let rb = first.and_then(|p| {
            router.as_ref().and_then(|name| {
                self.factory
                    .and_then(|f| self.unique(f, name))
                    .filter(|i| self.bindings[*i].node == p)
            })
        });
        let escapes = rb.is_some_and(|rb| {
            self.all.iter().any(|n| {
                matches!(
                    n.kind(),
                    "identifier"
                        | "property_identifier"
                        | "shorthand_property_identifier"
                        | "type_identifier"
                ) && router.as_deref() == Some(self.raw(*n))
                    && Some(*n) != first.map(|p| self.pname(p))
                    && self.unique(*n, self.raw(*n)) == Some(rb)
                    && !n.parent().is_some_and(|p| {
                        p.kind() == "member_expression" && field(p, "object") == Some(*n)
                    })
            })
        });
        for n in self.all.clone() {
            if self.stopped() {
                return;
            }
            if n.kind() != "call_expression" || !self.eligible.contains(&n.id()) {
                continue;
            }
            let Some(c) = field(n, "function").filter(|n| n.kind() == "member_expression") else {
                continue;
            };
            let Some(receiver) = field(c, "object").filter(|n| n.kind() == "identifier") else {
                continue;
            };
            let Some(method) = field(c, "property").map(|n| self.raw(n).to_owned()) else {
                continue;
            };
            if !matches!(
                method.as_str(),
                "get"
                    | "post"
                    | "put"
                    | "patch"
                    | "delete"
                    | "options"
                    | "head"
                    | "trace"
                    | "connect"
            ) || self.raw(receiver) != router.as_deref().unwrap_or("router")
            {
                continue;
            }
            let Some(own) = self.owner(n, false) else {
                continue;
            };
            let args = self.args(n);
            let handler = if args.len() == 2 {
                let h = unwrap(args[1]);
                if h.kind() == "identifier" {
                    self.resolve(n, self.raw(h))
                        .and_then(|i| self.bindings[i].target)
                } else {
                    Some(h)
                }
            } else {
                None
            };
            let hd = handler
                .filter(|n| callable(*n))
                .and_then(|n| self.declarations.get(&n.id()))
                .cloned();
            let path = args
                .first()
                .and_then(|n| self.literal(*n))
                .filter(|s| s.len() <= 2048 && (s.is_empty() || s.starts_with('/')));
            let valid = self
                .factory
                .and_then(|f| self.declarations.get(&f.id()))
                .is_some_and(|d| d.fact == own)
                && !self.annex
                && router.as_deref() == Some("router")
                && rb.is_some_and(|i| {
                    self.bindings[i].writes == 0
                        && !self.mutated.contains(&i)
                        && !escapes
                        && self.unique(receiver, "router") == Some(i)
                })
                && first.is_some_and(|p| !self.rest(p) && !self.defaulted(p));
            if !valid
                || hd.is_none()
                || path.is_none()
                || self.has(n, "optional_chain")
                || self.has(c, "optional_chain")
            {
                self.diagnostic("javascript_api_route_unresolved", n);
                continue;
            }
            self.fact("api_route",n,json!({"methods":[method.to_uppercase()],"path":path.unwrap(),"pathIsFragment":true,"receiver":"router","handlerFactId":self.facts[hd.unwrap().fact].id,"association":"injected-router factory registration candidate; framework/mount/activation UNKNOWN"}),&self.facts[own].symbol.clone());
        }
    }
    fn rest(&self, p: Node<'a>) -> bool {
        p.kind() == "rest_pattern"
            || field(p, "pattern")
                .or_else(|| field(p, "name"))
                .is_some_and(|n| n.kind() == "rest_pattern")
            || self.has(p, "...")
    }
    fn defaulted(&self, p: Node<'a>) -> bool {
        field(p, "value").is_some()
            || matches!(p.kind(), "assignment_pattern" | "object_assignment_pattern")
    }
    fn opt(&mut self, family: usize, kind: &str, n: Node<'a>, v: Value, symbol: &str) -> String {
        let (code, max, bytes) = if family == 0 {
            (STRUCTURAL, 4000, 1024 * 1024)
        } else {
            (RELATIONS, 12000, 4 * 1024 * 1024)
        };
        if self.omissions.contains(code) {
            return String::new();
        }
        let f = self.make(kind, n, v, symbol);
        let id = f.id.clone();
        let size = serialized_size(&f) + 1;
        let out = if family == 0 {
            &mut self.structural
        } else {
            &mut self.relations
        };
        if self.optional_prior_usage[family].1 + out.len() >= max
            || self.optional_prior_usage[family].0 + self.optional_bytes[family] + size > bytes
        {
            self.omissions.insert(code);
            out.clear();
            self.optional_bytes[family] = 0;
        } else {
            self.optional_bytes[family] += size;
            out.push(f);
        }
        id
    }
    pub(super) fn structural_facts(&mut self) {
        if self.optional_stopped(STRUCTURAL) {
            return;
        }
        let mut candidates: Vec<(Node<'a>, Node<'a>, String, String, Vec<String>, bool)> = vec![];
        let mut returns: HashMap<usize, Vec<Node<'a>>> = HashMap::new();
        let export_lhs = self
            .export_assignments
            .iter()
            .filter_map(|n| field(*n, "left"))
            .map(|n| n.id())
            .collect::<HashSet<_>>();
        let mut wrapper_refs_safe = true;
        for n in self.all.clone() {
            if self.optional_stopped(STRUCTURAL) {
                return;
            }
            if n.kind() == "return_statement" {
                if let Some(f) = self.nearest_function(n) {
                    returns.entry(f.id()).or_default().push(n);
                }
            }
            if wrapper_refs_safe
                && matches!(
                    n.kind(),
                    "identifier"
                        | "property_identifier"
                        | "shorthand_property_identifier"
                        | "shorthand_property_identifier_pattern"
                        | "type_identifier"
                )
                && matches!(self.raw(n), "module" | "exports")
            {
                wrapper_refs_safe = n.parent().is_some_and(|p| {
                    p.kind() == "member_expression"
                        && (field(p, "property") == Some(n) || export_lhs.contains(&p.id()))
                });
            }
        }
        for n in self.export_assignments.clone() {
            if self.optional_stopped(STRUCTURAL) {
                return;
            }
            let value = unwrap(field(n, "right").unwrap());
            let b = if value.kind() == "identifier" {
                self.scopes[0]
                    .bindings
                    .get(self.raw(value))
                    .filter(|bs| bs.len() == 1)
                    .map(|bs| bs[0])
            } else {
                None
            };
            let f = b.and_then(|i| self.bindings[i].target).unwrap_or(value);
            let Some(d) = self
                .declarations
                .get(&f.id())
                .cloned()
                .filter(|d| d.kind != "class")
            else {
                continue;
            };
            let meta: Value = serde_json::from_str(&self.facts[d.meta].value).unwrap();
            if meta["ownerFactId"] != self.facts[0].id {
                continue;
            }
            let initialized = b.is_none_or(|b| {
                self.bindings[b].writes == 0
                    && self.bindings[b].guards.len() == 1
                    && (self.bindings[b].node.kind() != "variable_declarator"
                        || self.bindings[b].node.end_byte() <= n.start_byte())
            });
            let direct = self.top_expression(n);
            let ef=self.opt(0,"js_module_export",n,json!({"policy":STRUCTURAL_POLICY,"ownerFactId":self.facts[0].id,"mode":"commonjs","exported":"default","targetFactId":self.facts[d.fact].id,"targetKind":"function","targetMetadataFactId":self.facts[d.meta].id,"bindingGuardFactId":b.and_then(|b|self.bindings[b].guards.first()),"referenceStart":self.u(n.start_byte()),"initializedBeforeExport":direct&&initialized}),&self.source.module.clone());
            let stable = direct
                && initialized
                && self.export_assignments.len() == 1
                && self.export_write_count() == 1
                && !self.esm
                && (self.wrappers_safe || (b.is_some() && wrapper_refs_safe))
                && !self.global_writes.contains("module")
                && !self.global_writes.contains("exports")
                && self.lookup(self.node_scope[&n.id()], "module").is_none()
                && self.lookup(self.node_scope[&n.id()], "exports").is_none();
            let mut evidence = vec![ef];
            if let Some(b) = b {
                evidence.extend(self.bindings[b].guards.clone());
            }
            candidates.push((
                f,
                n,
                "commonjs_default".into(),
                "default".into(),
                evidence,
                stable,
            ));
        }
        for n in self.all.clone() {
            if self.optional_stopped(STRUCTURAL) {
                return;
            }
            let mut candidate = None;
            if matches!(
                n.kind(),
                "function_declaration" | "generator_function_declaration"
            ) && n.parent().is_some_and(|p| {
                p.kind() == "export_statement" && self.top(p) && !self.has(p, "default")
            }) {
                candidate = field(n, "name")
                    .and_then(|n| self.direct(n))
                    .map(|name| (n, name));
            } else if n.kind() == "variable_declarator"
                && n.parent().and_then(|p| p.parent()).is_some_and(|p| {
                    p.kind() == "export_statement" && self.top(p) && !self.has(p, "default")
                })
            {
                candidate = field(n, "name")
                    .and_then(|n| self.direct(n))
                    .and_then(|name| {
                        field(n, "value")
                            .map(unwrap)
                            .filter(|n| callable(*n))
                            .map(|f| (f, name))
                    });
            } else if n.kind() == "export_specifier"
                && !self.has(n, "type")
                && n.parent().and_then(|p| p.parent()).is_some_and(|p| {
                    self.top(p) && !self.has(p, "type") && field(p, "source").is_none()
                })
            {
                candidate = field(n, "name")
                    .and_then(|n| self.direct(n))
                    .and_then(|name| {
                        self.scopes[0]
                            .bindings
                            .get(&name)
                            .filter(|bs| bs.len() == 1)
                            .map(|bs| bs[0])
                    })
                    .and_then(|b| self.bindings[b].target)
                    .and_then(|f| {
                        field(n, "alias")
                            .or_else(|| field(n, "name"))
                            .and_then(|n| self.direct(n))
                            .map(|name| (f, name))
                    });
            }
            if let Some((f, name)) = candidate {
                if name == "default" || !self.declarations.contains_key(&f.id()) {
                    continue;
                }
                let ef = self.module_exports.get(&n.id()).copied();
                candidates.push((
                    f,
                    n,
                    "esm_named".into(),
                    name,
                    ef.map(|i| vec![self.facts[i].id.clone()])
                        .unwrap_or_default(),
                    ef.is_some() && self.exports_safe && self.module_mode == "esm",
                ));
            }
        }
        let mut counts = BTreeMap::new();
        for (_, _, _, name, _, _) in &candidates {
            if self.optional_stopped(STRUCTURAL) {
                return;
            }
            *counts.entry(name.clone()).or_insert(0usize) += 1;
        }
        for (f, _, export_kind, name, evidence, stable) in candidates {
            if self.optional_stopped(STRUCTURAL) {
                return;
            }
            let d = self.declarations[&f.id()].clone();
            let mut support = vec![
                self.facts[0].id.clone(),
                self.facts[d.fact].id.clone(),
                self.facts[d.meta].id.clone(),
            ];
            support.extend(evidence);
            let parameters=self.params(f).into_iter().enumerate().filter_map(|(position,p)|{let pf=*self.parameters.get(&p.id())?;support.push(self.facts[pf].id.clone());let name=self.direct(self.pname(p));let b=name.as_ref().and_then(|name|self.unique(f,name)).filter(|i|self.bindings[*i].node==p);if let Some(b)=b{support.extend(self.bindings[b].guards.clone());}Some(json!({"parameterFactId":self.facts[pf].id,"bindingGuardFactId":b.and_then(|b|self.bindings[b].guards.first()),"name":name.unwrap_or("<binding>".into()),"position":position,"optional":self.has(p,"?"),"rest":self.rest(p),"hasDefault":self.defaulted(p)}))}).collect::<Vec<_>>();
            let body = field(f, "body").unwrap();
            let block = body.kind() == "statement_block";
            let sites = if block {
                returns.get(&f.id()).cloned().unwrap_or_default()
            } else {
                vec![body]
            };
            let final_node = if block {
                children(body).into_iter().rfind(|n| n.kind() != "comment")
            } else {
                Some(body)
            };
            let mut result_kind = "unsupported";
            let mut returned = None;
            let mut return_facts = vec![];
            for site in &sites {
                if self.optional_stopped(STRUCTURAL) {
                    return;
                }
                let value = if block {
                    children(*site).first().copied()
                } else {
                    Some(*site)
                }
                .map(unwrap);
                let instance = value.is_some_and(|v| v.kind() == "new_expression");
                let reference = value
                    .and_then(|v| {
                        if instance {
                            field(v, "constructor")
                        } else {
                            Some(v)
                        }
                    })
                    .map(unwrap);
                let mut cls = reference.filter(|r| r.kind() == "class");
                let mut binding = None;
                if let Some(r) = reference.filter(|r| r.kind() == "identifier") {
                    binding = self.unique(r, self.raw(r));
                    if let Some(b) = binding {
                        let bn = &self.bindings[b];
                        let direct = bn.node.parent() == Some(body)
                            || bn.node.parent().is_some_and(|p| {
                                matches!(p.kind(), "lexical_declaration" | "variable_declaration")
                                    && p.parent() == Some(body)
                            });
                        if direct
                            && self.scopes[bn.scope].node == Some(f)
                            && bn.writes == 0
                            && bn.guards.len() == 1
                            && bn.node.end_byte() <= site.start_byte()
                        {
                            cls = self.class_target(b);
                        }
                    }
                }
                let guards = cls
                    .map(|cls| {
                        self.class_bindings
                            .get(&cls.id())
                            .into_iter()
                            .flatten()
                            .map(|i| &self.bindings[*i])
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let cd = cls.and_then(|c| self.declarations.get(&c.id())).cloned();
                let supported = cd.is_some()
                    && !self.dynamic
                    && !self.annex
                    && guards.iter().all(|b| {
                        b.writes == 0
                            && b.guards.len() == 1
                            && self.scopes[b.scope].bindings[&b.name].len() == 1
                    });
                let mut ids = vec![
                    self.facts[0].id.clone(),
                    self.facts[d.fact].id.clone(),
                    self.facts[d.meta].id.clone(),
                ];
                if supported {
                    let cd = cd.as_ref().unwrap();
                    ids.extend([
                        self.facts[cd.fact].id.clone(),
                        self.facts[cd.meta].id.clone(),
                    ]);
                    for b in guards {
                        ids.extend(b.guards.clone());
                    }
                }
                dedup(&mut ids);
                let kind = if supported {
                    if instance {
                        "instance"
                    } else {
                        "class"
                    }
                } else {
                    "unsupported"
                };
                let form = if reference.is_some_and(|r| r.kind() == "class") {
                    if instance {
                        "new_class_expression"
                    } else {
                        "class_expression"
                    }
                } else if reference.is_some_and(|r| r.kind() == "identifier") {
                    if instance {
                        "new_identifier"
                    } else {
                        "identifier"
                    }
                } else {
                    "unsupported"
                };
                let rf=self.opt(0,"js_factory_return",*site,json!({"policy":STRUCTURAL_POLICY,"factoryFactId":self.facts[d.fact].id,"valueKind":kind,"valueForm":form,"valueStart":value.map(|v|self.u(v.start_byte())),"valueEnd":value.map(|v|self.u(v.end_byte())),"classFactId":if supported{cd.as_ref().map(|d|&self.facts[d.fact].id)}else{None},"bindingGuardFactId":if supported{binding.and_then(|b|self.bindings[b].guards.first())}else{None},"supportingFactIds":ids}),&self.facts[d.fact].symbol.clone());
                support.extend(ids);
                support.push(rf.clone());
                return_facts.push(rf);
                if sites.len() == 1 && Some(*site) == final_node {
                    result_kind = kind;
                    returned = if supported {
                        cd.map(|d| self.facts[d.fact].id.clone())
                    } else {
                        None
                    };
                }
            }
            let ambiguous = sites.len() > 1
                || (sites.len() == 1 && Some(sites[0]) != final_node)
                || counts[&name] > 1;
            let stable = stable
                && !self.dynamic
                && !self.annex
                && !self.has(f, "async")
                && !self.has(f, "*");
            if ambiguous || !stable {
                result_kind = if ambiguous {
                    "ambiguous"
                } else {
                    "unsupported"
                };
                returned = None;
            }
            let safe = stable && !ambiguous && matches!(result_kind, "class" | "instance");
            dedup(&mut support);
            self.opt(0,"js_factory",f,json!({"policy":STRUCTURAL_POLICY,"factoryFactId":self.facts[d.fact].id,"moduleFactId":self.facts[0].id,"modulePath":self.source.path,"exportKind":export_kind,"exportedName":name,"parameters":parameters,"resultKind":result_kind,"returnedClassFactId":returned,"returnFactIds":return_facts,"supportingFactIds":support,"safe":safe,"reason":if safe{None}else if ambiguous{Some("multiple or conditional exports/returns")}else if !stable{Some("export or callable structure is not stable")}else{Some("no supported final class or instance return")}}),&self.facts[d.fact].symbol.clone());
        }
    }
    fn nearest_function(&self, n: Node<'a>) -> Option<Node<'a>> {
        let mut p = n.parent();
        while let Some(x) = p {
            if callable(x) || x.kind() == "class_static_block" {
                return Some(x);
            }
            p = x.parent();
        }
        None
    }
    fn stable(&self, b: usize) -> bool {
        let b = &self.bindings[b];
        !self.dynamic && !self.annex && b.writes == 0 && b.guards.len() == 1
    }
    fn imported_target(&self, b: usize, n: Node<'a>) -> Option<usize> {
        let bn = &self.bindings[b];
        if self.stable(b) && (bn.kind == "import" || n.start_byte() >= bn.node.end_byte()) {
            self.module_imports.get(&b).copied()
        } else {
            None
        }
    }
    fn parameter_kind(&self, b: usize, c: Node<'a>, own: Node<'a>) -> Option<&'static str> {
        let bn = &self.bindings[b];
        if !self.stable(b)
            || bn.kind != "parameter"
            || self.direct(self.pname(bn.node)).is_none()
            || self.rest(bn.node)
        {
            return None;
        }
        let f = self.scopes[bn.scope].node?;
        let d = self.declarations.get(&f.id())?;
        if f == own && d.kind == "constructor" {
            return Some("constructor_parameter");
        }
        if matches!(d.kind.as_str(), "method" | "constructor") {
            return None;
        }
        let mut p = c.parent();
        while let Some(n) = p {
            if n == f {
                return Some("factory_parameter");
            }
            p = n.parent();
        }
        None
    }
    fn class_guards(&self, c: Node<'a>) -> Vec<String> {
        self.class_bindings
            .get(&c.id())
            .into_iter()
            .flatten()
            .flat_map(|i| self.bindings[*i].guards.clone())
            .collect()
    }
    fn optional_stopped(&mut self, code: &str) -> bool {
        self.stopped() || self.omissions.contains(code)
    }
    fn context(&self, n: Node<'a>) -> Option<(Node<'a>, Node<'a>)> {
        if !self.eligible.contains(&n.id()) || self.type_position(n) {
            return None;
        }
        let mut own = None;
        let mut p = n.parent();
        while let Some(x) = p {
            if matches!(
                x.kind(),
                "type_annotation"
                    | "type_arguments"
                    | "required_parameter"
                    | "optional_parameter"
                    | "decorator"
                    | "computed_property_name"
                    | "class_heritage"
                    | "extends_clause"
            ) {
                return None;
            }
            if callable(x) {
                if own.is_some() || !self.declarations.contains_key(&x.id()) {
                    return None;
                }
                own = Some(x);
            }
            if class(x) {
                let own = own?;
                let d = self.declarations.get(&own.id())?;
                return (matches!(d.kind.as_str(), "method" | "constructor")
                    && self.declarations.contains_key(&x.id()))
                .then_some((x, own));
            }
            p = x.parent();
        }
        None
    }
    #[allow(clippy::too_many_arguments)]
    fn relation(
        &mut self,
        c: Node<'a>,
        own: Node<'a>,
        node: Node<'a>,
        relationship: &str,
        expression: String,
        b: Option<usize>,
        kind: &str,
        target: Option<Node<'a>>,
        member: Option<String>,
        declared: Option<String>,
        base: Option<usize>,
        reference: Node<'a>,
        scope_facts: &mut HashMap<usize, String>,
        reference_facts: &mut HashMap<usize, String>,
    ) {
        if self.optional_stopped(RELATIONS) {
            return;
        }
        let actual = unwrap(reference);
        let cd = self.declarations[&c.id()].clone();
        let od = self.declarations[&own.id()].clone();
        let imported = if kind == "import" {
            b.and_then(|b| self.imported_target(b, actual))
        } else {
            None
        };
        let chain = self.scope_chain(actual);
        let mut chain_ids = vec![];
        for s in &chain {
            if self.optional_stopped(RELATIONS) {
                return;
            }
            let id = if let Some(id) = scope_facts.get(s) {
                id.clone()
            } else {
                let sc = &self.scopes[*s];
                let n = sc.node.unwrap();
                let v = json!({"policy":RELATION_POLICY,"scopeId":sc.id,"parentScopeId":sc.parent.map(|p|&self.scopes[p].id),"scopeKind":sc.kind,"start":self.u(n.start_byte()),"end":self.u(n.end_byte())});
                let id = self.opt(1, "js_class_scope", n, v, &self.source.module.clone());
                scope_facts.insert(*s, id.clone());
                id
            };
            chain_ids.push(id);
        }
        if self.optional_stopped(RELATIONS) {
            return;
        }
        let reference_id = if let Some(id) = reference_facts.get(&actual.id()) {
            id.clone()
        } else {
            let id=self.opt(1,"js_class_reference",actual,json!({"policy":RELATION_POLICY,"classFactId":self.facts[cd.fact].id,"ownerFactId":self.facts[od.fact].id,"scopeId":self.scopes[self.node_scope[&actual.id()]].id,"start":self.u(actual.start_byte()),"end":self.u(actual.end_byte())}),&self.facts[cd.fact].symbol.clone());
            reference_facts.insert(actual.id(), id.clone());
            id
        };
        let mut support = vec![self.facts[0].id.clone(), self.facts[cd.fact].id.clone()];
        support.extend(self.class_guards(c));
        let mut p = Some(own);
        while let Some(n) = p {
            if let Some(d) = self.declarations.get(&n.id()) {
                support.extend([self.facts[d.fact].id.clone(), self.facts[d.meta].id.clone()]);
            }
            p = n.parent();
        }
        if let Some(b) = b {
            support.extend(self.bindings[b].guards.clone());
            let f = self.scopes[self.bindings[b].scope].node.unwrap();
            if let Some(d) = self.declarations.get(&f.id()) {
                support.extend([self.facts[d.fact].id.clone(), self.facts[d.meta].id.clone()]);
            }
        }
        if let Some(t) = target {
            let d = &self.declarations[&t.id()];
            support.extend([self.facts[d.fact].id.clone(), self.facts[d.meta].id.clone()]);
            support.extend(self.class_guards(t));
        }
        if let Some(i) = base {
            support.push(self.facts[i].id.clone());
        }
        if let Some(i) = imported {
            support.push(self.facts[i].id.clone());
        }
        support.push(reference_id.clone());
        support.extend(chain_ids);
        dedup(&mut support);
        self.opt(1,"js_class_relation",node,json!({"policy":RELATION_POLICY,"classFactId":self.facts[cd.fact].id,"ownerFactId":self.facts[od.fact].id,"referenceFactId":reference_id,"relationship":relationship,"expression":expression,"lookupScopeIds":chain.iter().map(|s|&self.scopes[*s].id).collect::<Vec<_>>(),"bindingKind":kind,"bindingGuardFactId":b.and_then(|b|self.bindings[b].guards.first()),"targetFactId":target.map(|t|&self.facts[self.declarations[&t.id()].fact].id),"importFactId":imported.map(|i|&self.facts[i].id),"memberName":member,"declaredType":declared,"supportingFactIds":support,"stableBinding":b.is_some_and(|b|self.stable(b))}),&self.facts[od.fact].symbol.clone());
    }
    pub(super) fn class_relations(&mut self) {
        if self.optional_stopped(RELATIONS) {
            return;
        }
        let mut scopes = HashMap::new();
        let mut references = HashMap::new();
        let mut stored = HashSet::new();
        let mut class_ids = HashSet::new();
        let mut member_callables = HashSet::new();
        for i in 0..self.facts.len() {
            if self.optional_stopped(RELATIONS) {
                return;
            }
            let f = &self.facts[i];
            if f.kind == "class" {
                class_ids.insert(f.id.clone());
            } else if f.kind == "js_member" {
                if let Ok(v) = serde_json::from_str::<Value>(&f.value) {
                    if let Some(id) = v["declarationFactId"].as_str() {
                        member_callables.insert(id.to_owned());
                    }
                }
            }
        }
        for n in self.all.clone() {
            if self.optional_stopped(RELATIONS) {
                return;
            }
            let Some(d) = self.declarations.get(&n.id()).cloned().filter(|d| {
                matches!(d.kind.as_str(), "method" | "constructor")
                    && member_callables.contains(&self.facts[d.fact].id)
            }) else {
                continue;
            };
            let owner: Value = serde_json::from_str(&self.facts[d.meta].value).unwrap();
            if !owner["ownerFactId"]
                .as_str()
                .is_some_and(|id| class_ids.contains(id))
            {
                continue;
            }
            let ps = self.params(n);
            let tp = field(n, "type_parameters")
                .map(children)
                .unwrap_or_default();
            if ps.len() > 64
                || tp.len() > 16
                || !tp.iter().all(|p| {
                    field(*p, "name")
                        .and_then(|n| self.direct(n))
                        .is_some_and(|name| name.len() <= 512)
                })
            {
                self.omissions.insert(RELATIONS);
                return;
            }
            let parameters=ps.iter().map(|p|json!({"name":self.direct(self.pname(*p)).unwrap_or("<binding>".into()),"pattern":self.direct(self.pname(*p)).is_none(),"optional":self.has(*p,"?"),"rest":self.rest(*p),"hasDefault":self.defaulted(*p),"type":field(*p,"type").map(|t|self.typ(t))})).collect::<Vec<_>>();
            let type_parameters = tp
                .iter()
                .filter_map(|p| field(*p, "name").and_then(|n| self.direct(n)))
                .collect::<Vec<_>>();
            self.opt(1,"js_callable_signature",n,json!({"policy":"javascript-class-members-1","declarationFactId":self.facts[d.fact].id,"ownerFactId":owner["ownerFactId"],"parameters":parameters,"returnType":field(n,"return_type").map(|n|self.typ(n)),"typeParameters":type_parameters}),&self.facts[d.fact].symbol.clone());
        }
        for c in self.all.clone().into_iter().filter(|n| class(*n)) {
            if self.optional_stopped(RELATIONS) {
                return;
            }
            if !self.declarations.contains_key(&c.id()) {
                continue;
            }
            for t in self.heritage(c) {
                if self.optional_stopped(RELATIONS) {
                    return;
                }
                let r = t;
                let b = if r.kind() == "identifier" && self.raw(r).len() <= 512 {
                    self.unique(r, self.raw(r))
                } else {
                    None
                };
                let candidate = b.and_then(|b| self.class_target(b));
                let target = candidate.filter(|t| {
                    *t != c
                        && b.is_some_and(|b| {
                            self.stable(b) && self.bindings[b].node.end_byte() < r.start_byte()
                        })
                });
                let kind = if target.is_some() {
                    "local_class"
                } else if b.and_then(|b| self.imported_target(b, r)).is_some() {
                    "import"
                } else {
                    b.and_then(|b| self.parameter_kind(b, c, c))
                        .unwrap_or("unresolved")
                };
                let base = self.bases.get(&t.id()).copied();
                self.relation(
                    c,
                    c,
                    t,
                    "extends",
                    self.expr(t),
                    b,
                    kind,
                    target,
                    None,
                    None,
                    base,
                    r,
                    &mut scopes,
                    &mut references,
                );
            }
        }
        for n in self.all.clone() {
            if self.optional_stopped(RELATIONS) {
                return;
            }
            if matches!(n.kind(), "required_parameter" | "optional_parameter")
                && self.parameter_property(n)
            {
                let Some(ctor) = self.nearest_function(n) else {
                    continue;
                };
                let Some(od) = self
                    .declarations
                    .get(&ctor.id())
                    .filter(|d| d.kind == "constructor")
                else {
                    continue;
                };
                let _ = od;
                let Some(c) = ctor.parent().and_then(|p| p.parent()).filter(|n| class(*n)) else {
                    continue;
                };
                let r = self.pname(n);
                if let Some(name) = self.direct(r) {
                    let b = self.unique(r, &name);
                    if let Some(kind) = b.and_then(|b| self.parameter_kind(b, c, ctor)) {
                        self.relation(
                            c,
                            ctor,
                            n,
                            "stores_parameter",
                            name.clone(),
                            b,
                            kind,
                            None,
                            Some(name),
                            field(n, "type").map(|t| self.typ(t)),
                            None,
                            r,
                            &mut scopes,
                            &mut references,
                        );
                    }
                }
            }
            if n.kind() != "assignment_expression" {
                continue;
            }
            let Some((c, ctor)) = self.context(n) else {
                continue;
            };
            if self.declarations[&ctor.id()].kind != "constructor" {
                continue;
            }
            let Some(lhs) = field(n, "left") else {
                continue;
            };
            let Some(value) = field(n, "right") else {
                continue;
            };
            if lhs.kind() == "member_expression"
                && field(lhs, "object").is_some_and(|n| n.kind() == "this")
                && !self.has(lhs, "optional_chain")
            {
                if let Some(name) = field(lhs, "property")
                    .and_then(|n| self.direct(n))
                    .filter(|s| s.len() <= 512)
                {
                    let cd = self.declarations[&c.id()].clone();
                    let od = self.declarations[&ctor.id()].clone();
                    self.opt(1,"js_constructor_field",n,json!({"policy":"javascript-class-members-1","classFactId":self.facts[cd.fact].id,"ownerFactId":self.facts[od.fact].id,"memberName":name,"visibility":if name.starts_with('#'){"private"}else{"public"},"static":false,"assignmentKind":"direct-constructor-assignment","sourceType":null,"supportingFactIds":[self.facts[cd.fact].id,self.facts[cd.meta].id,self.facts[od.fact].id,self.facts[od.meta].id]}),&self.facts[od.fact].symbol.clone());
                }
            }
            let mut root = lhs;
            let mut parts = vec![];
            while root.kind() == "member_expression" && !self.has(root, "optional_chain") {
                let Some(name) = field(root, "property")
                    .filter(|n| n.kind() == "property_identifier")
                    .and_then(|n| self.direct(n))
                else {
                    break;
                };
                parts.insert(0, name);
                root = field(root, "object").unwrap();
            }
            if !parts.is_empty() && root.kind() == "this" {
                self.store(
                    c,
                    ctor,
                    n,
                    value,
                    parts,
                    0,
                    &mut stored,
                    &mut scopes,
                    &mut references,
                );
            }
        }
        let mut selected = HashSet::new();
        for n in self.all.clone() {
            if self.optional_stopped(RELATIONS) {
                return;
            }
            if !matches!(n.kind(), "identifier" | "shorthand_property_identifier")
                || self.raw(n).len() > 512
                || stored.contains(&n.id())
            {
                continue;
            }
            let Some(p) = n.parent() else {
                continue;
            };
            if field(p, "name") == Some(n)
                || field(p, "property") == Some(n)
                || field(p, "key") == Some(n)
                || matches!(
                    p.kind(),
                    "break_statement" | "continue_statement" | "labeled_statement"
                )
            {
                continue;
            }
            let Some((c, own)) = self.context(n) else {
                continue;
            };
            let Some(b) = self.unique(n, self.raw(n)) else {
                continue;
            };
            let creation = p.kind() == "new_expression" && field(p, "constructor") == Some(n);
            let mut kind = self.parameter_kind(b, c, own);
            let mut target = None;
            if creation && kind.is_none() && self.stable(b) {
                if let Some(candidate) = self
                    .class_target(b)
                    .filter(|_| self.bindings[b].node.end_byte() < n.start_byte())
                {
                    kind = Some("local_class");
                    target = Some(candidate);
                } else if self.imported_target(b, n).is_some() {
                    kind = Some("import");
                }
            }
            let Some(kind) = kind else {
                continue;
            };
            if !creation && kind != "factory_parameter" {
                continue;
            }
            let relationship = if creation {
                "creates"
            } else {
                "uses_factory_binding"
            };
            if !selected.insert((c.id(), b, relationship)) {
                continue;
            }
            self.relation(
                c,
                own,
                if creation { p } else { n },
                relationship,
                self.raw(n).into(),
                Some(b),
                kind,
                target,
                None,
                None,
                None,
                n,
                &mut scopes,
                &mut references,
            );
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn store(
        &mut self,
        c: Node<'a>,
        ctor: Node<'a>,
        node: Node<'a>,
        value: Node<'a>,
        parts: Vec<String>,
        depth: usize,
        stored: &mut HashSet<usize>,
        scopes: &mut HashMap<usize, String>,
        references: &mut HashMap<usize, String>,
    ) {
        if self.optional_stopped(RELATIONS) {
            return;
        }
        if parts.join(".").len() > 512 {
            return;
        }
        if matches!(value.kind(), "identifier" | "shorthand_property_identifier")
            && self.raw(value).len() <= 512
        {
            let b = self.unique(value, self.raw(value));
            if let Some(kind) = b.and_then(|b| self.parameter_kind(b, c, ctor)) {
                let ty = b
                    .and_then(|b| field(self.bindings[b].node, "type"))
                    .map(|t| self.typ(t));
                self.relation(
                    c,
                    ctor,
                    node,
                    "stores_parameter",
                    self.raw(value).into(),
                    b,
                    kind,
                    None,
                    Some(parts.join(".")),
                    ty,
                    None,
                    value,
                    scopes,
                    references,
                );
                stored.insert(value.id());
            }
        } else if value.kind() == "object" && depth < 4 {
            let properties = children(value);
            let mut keys = HashSet::new();
            let valid = properties.iter().all(|p| {
                let name = if p.kind() == "shorthand_property_identifier" {
                    self.direct(*p)
                } else if p.kind() == "pair" {
                    field(*p, "key").and_then(|n| self.direct(n))
                } else {
                    None
                };
                name.is_some_and(|name| {
                    name.len() <= 512 && name != "__proto__" && keys.insert(name)
                })
            });
            if !valid {
                return;
            }
            for p in properties {
                if self.optional_stopped(RELATIONS) {
                    return;
                }
                let (name, v) = if p.kind() == "pair" {
                    (field(p, "key").unwrap(), field(p, "value").unwrap())
                } else {
                    (p, p)
                };
                let mut parts = parts.clone();
                parts.push(self.raw(name).into());
                self.store(
                    c,
                    ctor,
                    node,
                    v,
                    parts,
                    depth + 1,
                    stored,
                    scopes,
                    references,
                );
            }
        }
    }
}
fn dedup(ids: &mut Vec<String>) {
    let mut seen = HashSet::new();
    ids.retain(|id| seen.insert(id.clone()));
}
