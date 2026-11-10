//! In-process syntax extraction. No target modules or configuration are loaded.
//! Parser cancellation and traversal budgets are cooperative, not a memory sandbox.
use crate::analysis::{self, Diagnostic, Fact};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::time::{Duration, Instant};
use tree_sitter::{Node, ParseOptions, Parser};

#[path = "javascript_native/optional.rs"]
mod optional;

pub(crate) struct NativeSource {
    pub path: String,
    pub module: String,
    pub bytes: Vec<u8>,
}
pub(crate) struct NativeResult {
    pub facts: Vec<Fact>,
    pub diagnostics: Vec<Diagnostic>,
    pub complete: bool,
}

pub(crate) fn fingerprint() -> String {
    analysis::hash(
        &serde_json::to_vec(&(
        "tree-sitter=0.25.10;tree-sitter-javascript=0.25.0;tree-sitter-typescript=0.23.2;tree-sitter-language=0.1.5;native-1;",
            tree_sitter_javascript::NODE_TYPES,
            tree_sitter_typescript::TYPESCRIPT_NODE_TYPES,
            tree_sitter_typescript::TSX_NODE_TYPES,
            include_str!("javascript_native.rs"),
            include_str!("javascript_native/optional.rs"),
        )).expect("embedded native fingerprint components are serializable"),
    )
}

const MAX_FACTS: usize = 100_000;
const MAX_OUTPUT: usize = 32 * 1024 * 1024;
const STRUCTURAL: &str = "javascript_structural_facts_omitted";
const RELATIONS: &str = "javascript_class_relations_omitted";

pub(crate) fn extract_sources(sources: &[NativeSource]) -> Result<NativeResult, String> {
    if sources.len() > 5000
        || sources.iter().map(|s| s.bytes.len()).sum::<usize>() > 16 * 1024 * 1024
    {
        return Err("Native JavaScript input exceeds source budget".into());
    }
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut result = NativeResult {
        facts: vec![],
        diagnostics: vec![],
        complete: true,
    };
    let mut seen = HashSet::new();
    let mut omissions = BTreeSet::new();
    let mut usage = [(0usize, 0usize); 2];
    let mut baseline_count = 0;
    let mut baseline_bytes = 0;
    for source in sources {
        if source.path.is_empty()
            || source.path.len() > 4096
            || source.module.len() > 4096
            || source.path.starts_with('/')
            || source.path.contains('\\')
            || source.path.split('/').any(|p| matches!(p, "" | "." | ".."))
            || !seen.insert(&source.path)
            || source.bytes.len() > 1024 * 1024
        {
            return Err("Invalid bounded native JavaScript source".into());
        }
        if baseline_count >= MAX_FACTS || Instant::now() >= deadline {
            result.complete = false;
            push_diag(&mut result.diagnostics, "resource_limit", "", 1);
            break;
        }
        if baseline_bytes > MAX_OUTPUT - 1024 {
            result.facts.clear();
            result.complete = false;
            omissions.extend([STRUCTURAL, RELATIONS]);
            push_diag(&mut result.diagnostics, "resource_limit", "", 1);
            break;
        }
        let module = module_fact(source);
        let text = match std::str::from_utf8(&source.bytes) {
            Ok(text) => text,
            Err(_) => {
                result.facts.push(module);
                baseline_count += 1;
                baseline_bytes += serialized_size(result.facts.last().unwrap()) + 1;
                result.complete = false;
                push_diag(
                    &mut result.diagnostics,
                    "javascript_source_unavailable",
                    &source.path,
                    1,
                );
                continue;
            }
        };
        let grammar: tree_sitter_language::LanguageFn = if source.path.ends_with(".tsx") {
            tree_sitter_typescript::LANGUAGE_TSX
        } else if [".ts", ".mts", ".cts"]
            .iter()
            .any(|ext| source.path.ends_with(ext))
        {
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT
        } else {
            tree_sitter_javascript::LANGUAGE
        };
        let mut parser = Parser::new();
        parser
            .set_language(&grammar.into())
            .map_err(|e| format!("Initialize native grammar: {e}"))?;
        let mut progress = |_: &tree_sitter::ParseState| Instant::now() >= deadline;
        let mut input = |offset: usize, _| source.bytes.get(offset..).unwrap_or_default();
        let tree = parser.parse_with_options(
            &mut input,
            None,
            Some(ParseOptions::new().progress_callback(&mut progress)),
        );
        let Some(tree) = tree else {
            result.facts.push(module);
            baseline_count += 1;
            baseline_bytes += serialized_size(result.facts.last().unwrap()) + 1;
            result.complete = false;
            push_diag(&mut result.diagnostics, "resource_limit", &source.path, 1);
            continue;
        };
        if tree.root_node().has_error() {
            let mut file = File::new(source, text, module, deadline);
            let mut pending = vec![(tree.root_node(), 0)];
            let mut visited = 0;
            let mut errors = 0;
            while let Some((n, depth)) = pending.pop() {
                visited += 1;
                if visited > 200_000 || depth > 256 || Instant::now() >= deadline {
                    break;
                }
                if n.is_error() || n.is_missing() {
                    file.diagnostic("javascript_parse_error", n);
                    errors += 1;
                    if errors == 20 {
                        break;
                    }
                }
                if n.has_error() {
                    let mut cursor = n.walk();
                    let mut cs = n.children(&mut cursor).collect::<Vec<_>>();
                    cs.reverse();
                    pending.extend(cs.into_iter().map(|c| (c, depth + 1)));
                }
            }
            if errors == 0 {
                file.diagnostic("javascript_parse_error", tree.root_node());
            }
            result.facts.push(file.facts.remove(0));
            baseline_count += 1;
            baseline_bytes += serialized_size(result.facts.last().unwrap()) + 1;
            result.complete = false;
            for d in file.diagnostics {
                if result.diagnostics.len() < 5000 {
                    result.diagnostics.push(d);
                }
            }
            continue;
        }
        let mut file = File::new(source, text, module, deadline);
        file.optional_prior_usage = usage;
        file.baseline_prior_bytes = baseline_bytes;
        file.baseline_prior_count = baseline_count;
        if file.walk(tree.root_node(), 0, 0, false, false).is_err() {
            result.facts.push(file.facts.remove(0));
            baseline_count += 1;
            baseline_bytes += serialized_size(result.facts.last().unwrap()) + 1;
            result.complete = false;
            push_diag(&mut result.diagnostics, "resource_limit", &source.path, 1);
            continue;
        }
        file.analyze();
        if file.output_exhausted {
            result.facts.clear();
            result.complete = false;
            omissions.extend([STRUCTURAL, RELATIONS]);
            push_diag(&mut result.diagnostics, "resource_limit", "", 1);
            break;
        }
        if file.exhausted
            || baseline_count + file.facts.len() > MAX_FACTS
            || Instant::now() >= deadline
        {
            result.facts.push(file.facts.remove(0));
            baseline_count += 1;
            baseline_bytes += serialized_size(result.facts.last().unwrap()) + 1;
            result.complete = false;
            push_diag(&mut result.diagnostics, "resource_limit", &source.path, 1);
            continue;
        }
        baseline_count += file.facts.len();
        baseline_bytes += file.baseline_bytes;
        if result.facts.len() + file.facts.len() > MAX_FACTS {
            result.facts.retain(|f| {
                let family = optional_family(f);
                if let Some(code) = family {
                    omissions.insert(code);
                }
                family.is_none()
            });
        }
        for (i, (family, code, bytes, count)) in [
            (&mut file.structural, STRUCTURAL, 1024 * 1024, 4000),
            (&mut file.relations, RELATIONS, 4 * 1024 * 1024, 12000),
        ]
        .into_iter()
        .enumerate()
        {
            let size = family.iter().map(|f| serialized_size(f) + 1).sum::<usize>();
            if file.omissions.contains(code)
                || usage[i].0 + size > bytes
                || usage[i].1 + family.len() > count
                || result.facts.len() + file.facts.len() + family.len() > MAX_FACTS
            {
                omissions.insert(code);
                push_diag(&mut file.diagnostics, code, &source.path, 1);
            } else {
                usage[i].0 += size;
                usage[i].1 += family.len();
                file.facts.append(family);
            }
        }
        result.facts.extend(file.facts);
        for d in file.diagnostics {
            if result.diagnostics.len() < 5000 {
                result.diagnostics.push(d);
            }
        }
    }
    let mut unique = HashSet::new();
    result.facts.retain(|f| unique.insert(f.id.clone()));
    if result.facts.len() > MAX_FACTS
        || serialized_size(&result.facts) + serialized_size(&result.diagnostics) > MAX_OUTPUT - 1024
    {
        result.facts.retain(|f| {
            let family = optional_family(f);
            if let Some(code) = family {
                omissions.insert(code);
            }
            family.is_none()
        });
    }
    if serialized_size(&result.facts) + serialized_size(&result.diagnostics) > MAX_OUTPUT - 1024 {
        result.facts.clear();
        result.complete = false;
        push_diag(&mut result.diagnostics, "resource_limit", "", 1);
    }
    result.diagnostics.truncate(5000 - omissions.len());
    for code in omissions {
        push_diag(&mut result.diagnostics, code, "", 1);
    }
    Ok(result)
}

fn optional_family(f: &Fact) -> Option<&'static str> {
    match f.kind.as_str() {
        "js_factory" | "js_factory_return" => Some(STRUCTURAL),
        "js_class_relation"
        | "js_class_scope"
        | "js_class_reference"
        | "js_callable_signature"
        | "js_constructor_field" => Some(RELATIONS),
        "js_module_export" if f.value.contains("javascript-structural-facts-1") => Some(STRUCTURAL),
        _ => None,
    }
}
fn serialized_size(value: &impl serde::Serialize) -> usize {
    struct Counter(usize);
    impl std::io::Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 += bytes.len();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut out = Counter(0);
    serde_json::to_writer(&mut out, value).expect("native facts are serializable");
    out.0
}
fn push_diag(out: &mut Vec<Diagnostic>, code: &str, path: &str, line: usize) {
    if out.len() < 5000 {
        out.push(Diagnostic {
            code: code.into(),
            path: path.into(),
            message: format!("line {line}: {code}"),
        });
    }
}
fn module_fact(s: &NativeSource) -> Fact {
    Fact {
        id: format!("module_{}", analysis::hash(s.path.as_bytes())),
        kind: "module".into(),
        path: s.path.clone(),
        line: 1,
        end_line: 1,
        symbol: s.module.clone(),
        value: s.module.clone(),
    }
}
fn children(n: Node<'_>) -> Vec<Node<'_>> {
    let mut c = n.walk();
    n.named_children(&mut c)
        .filter(|n| n.kind() != "comment")
        .collect()
}
fn bounded_tree(root: Node<'_>, deadline: Instant) -> bool {
    let mut cursor = root.walk();
    let mut depth = 0;
    let mut nodes = 0;
    loop {
        let n = cursor.node();
        if Instant::now() >= deadline {
            return false;
        }
        if n.is_named() && n.kind() != "comment" {
            nodes += 1;
            if nodes > 200_000 || depth > 256 {
                return false;
            }
        }
        if cursor.goto_first_child() {
            depth += 1;
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return true;
            }
            depth -= 1;
        }
    }
}
fn field<'a>(n: Node<'a>, name: &str) -> Option<Node<'a>> {
    n.child_by_field_name(name)
}
fn callable(n: Node<'_>) -> bool {
    matches!(
        n.kind(),
        "function_declaration"
            | "generator_function_declaration"
            | "function_expression"
            | "generator_function"
            | "arrow_function"
            | "method_definition"
    ) && field(n, "body").is_some()
}
fn call(n: Node<'_>) -> bool {
    n.kind() == "call_expression" && field(n, "arguments").is_some_and(|n| n.kind() == "arguments")
}
fn class(n: Node<'_>) -> bool {
    matches!(
        n.kind(),
        "class_declaration" | "class" | "abstract_class_declaration"
    )
}
fn unwrap(mut n: Node<'_>) -> Node<'_> {
    while matches!(
        n.kind(),
        "parenthesized_expression"
            | "as_expression"
            | "satisfies_expression"
            | "non_null_expression"
            | "type_assertion"
    ) {
        let next = field(n, "expression").or_else(|| {
            children(n)
                .into_iter()
                .find(|x| x.kind() != "type_arguments")
        });
        match next {
            Some(x) => n = x,
            None => break,
        }
    }
    n
}
#[derive(Clone)]
struct Scope<'a> {
    id: String,
    parent: Option<usize>,
    kind: &'static str,
    node: Option<Node<'a>>,
    bindings: BTreeMap<String, Vec<usize>>,
}
#[derive(Clone)]
struct Binding<'a> {
    scope: usize,
    name: String,
    node: Node<'a>,
    kind: String,
    target: Option<Node<'a>>,
    writes: usize,
    guards: Vec<String>,
}
#[derive(Clone)]
struct Decl {
    fact: usize,
    meta: usize,
    kind: String,
}
struct File<'a> {
    source: &'a NativeSource,
    text: &'a str,
    names: HashMap<usize, String>,
    utf16: Vec<usize>,
    lines: Vec<usize>,
    facts: Vec<Fact>,
    baseline_prior_bytes: usize,
    baseline_prior_count: usize,
    baseline_bytes: usize,
    output_exhausted: bool,
    structural: Vec<Fact>,
    relations: Vec<Fact>,
    diagnostics: Vec<Diagnostic>,
    omissions: BTreeSet<&'static str>,
    optional_prior_usage: [(usize, usize); 2],
    optional_bytes: [usize; 2],
    all: Vec<Node<'a>>,
    eligible: HashSet<usize>,
    scopes: Vec<Scope<'a>>,
    node_scope: HashMap<usize, usize>,
    parameter_scopes: HashMap<usize, usize>,
    bindings: Vec<Binding<'a>>,
    class_bindings: HashMap<usize, Vec<usize>>,
    declarations: HashMap<usize, Decl>,
    parameters: HashMap<usize, usize>,
    bases: HashMap<usize, usize>,
    module_imports: HashMap<usize, usize>,
    module_exports: HashMap<usize, usize>,
    global_writes: HashSet<String>,
    mutated: HashSet<usize>,
    property_writes: Vec<Node<'a>>,
    annex: bool,
    dynamic: bool,
    imports_safe: bool,
    exports_safe: bool,
    wrappers_safe: bool,
    esm: bool,
    module_mode: String,
    factory: Option<Node<'a>>,
    export_assignments: Vec<Node<'a>>,
    deadline: Instant,
    exhausted: bool,
}
impl<'a> File<'a> {
    fn new(source: &'a NativeSource, text: &'a str, mut module: Fact, deadline: Instant) -> Self {
        let mut utf16 = vec![0; text.len() + 1];
        let mut units = 0;
        let mut lines = vec![0];
        let mut previous_cr = false;
        for (i, c) in text.char_indices() {
            for v in &mut utf16[i..i + c.len_utf8()] {
                *v = units;
            }
            units += c.len_utf16();
            utf16[i + c.len_utf8()] = units;
            if c == '\n' && previous_cr {
                *lines.last_mut().unwrap() = i + 1;
            } else if matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}') {
                lines.push(i + c.len_utf8());
            }
            previous_cr = c == '\r';
        }
        module.end_line = lines.partition_point(|i| *i <= text.len().saturating_sub(1));
        let baseline_bytes = serialized_size(&module) + 1;
        Self {
            source,
            text,
            names: HashMap::new(),
            utf16,
            lines,
            facts: vec![module],
            baseline_prior_bytes: 0,
            baseline_prior_count: 0,
            baseline_bytes,
            output_exhausted: false,
            structural: vec![],
            relations: vec![],
            diagnostics: vec![],
            omissions: BTreeSet::new(),
            optional_prior_usage: [(0, 0); 2],
            optional_bytes: [0; 2],
            all: vec![],
            eligible: HashSet::new(),
            scopes: vec![Scope {
                id: format!(
                    "scope_{}",
                    analysis::hash(format!("{}:module", source.path).as_bytes())
                ),
                parent: None,
                kind: "module",
                node: None,
                bindings: BTreeMap::new(),
            }],
            node_scope: HashMap::new(),
            parameter_scopes: HashMap::new(),
            bindings: vec![],
            class_bindings: HashMap::new(),
            declarations: HashMap::new(),
            parameters: HashMap::new(),
            bases: HashMap::new(),
            module_imports: HashMap::new(),
            module_exports: HashMap::new(),
            global_writes: HashSet::new(),
            mutated: HashSet::new(),
            property_writes: vec![],
            annex: false,
            dynamic: false,
            imports_safe: true,
            exports_safe: true,
            wrappers_safe: true,
            esm: false,
            module_mode: "unknown".into(),
            factory: None,
            export_assignments: vec![],
            deadline,
            exhausted: false,
        }
    }
    fn raw(&self, n: Node<'_>) -> &str {
        self.names
            .get(&n.id())
            .map(String::as_str)
            .unwrap_or(&self.text[n.byte_range()])
    }
    fn direct(&self, n: Node<'_>) -> Option<String> {
        matches!(
            n.kind(),
            "identifier"
                | "type_identifier"
                | "property_identifier"
                | "private_property_identifier"
                | "shorthand_property_identifier"
                | "shorthand_property_identifier_pattern"
        )
        .then(|| identifier_name(self.raw(n)).unwrap_or_else(|| self.raw(n).to_owned()))
    }
    fn u(&self, byte: usize) -> usize {
        self.utf16[byte]
    }
    fn line(&self, byte: usize) -> usize {
        self.lines.partition_point(|i| *i <= byte)
    }
    fn diagnostic(&mut self, code: &str, n: Node<'_>) {
        let line = self.line(n.start_byte());
        push_diag(&mut self.diagnostics, code, &self.source.path, line);
    }
    fn make(&self, kind: &str, n: Node<'_>, value: Value, symbol: &str) -> Fact {
        let value = if let Value::String(s) = value {
            s
        } else {
            value.to_string()
        };
        let start = if n.kind() == "program" {
            0
        } else {
            n.start_byte()
        };
        let end = n.end_byte();
        let id = format!(
            "fact_{}",
            analysis::hash(
                json!([
                    self.source.path,
                    kind,
                    self.u(start),
                    self.u(end),
                    symbol,
                    value
                ])
                .to_string()
                .as_bytes()
            )
        );
        Fact {
            id,
            kind: kind.into(),
            path: self.source.path.clone(),
            line: self.line(start),
            end_line: self.line(end.saturating_sub(1).max(start)),
            symbol: symbol.into(),
            value,
        }
    }
    fn fact(&mut self, kind: &str, n: Node<'_>, value: Value, symbol: &str) -> usize {
        if self.stopped() || self.facts.len() >= MAX_FACTS {
            self.exhausted = true;
            return 0;
        }
        let f = self.make(kind, n, value, symbol);
        let size = serialized_size(&f) + 1;
        if self.baseline_prior_bytes + self.baseline_bytes + size > MAX_OUTPUT - 1024 {
            self.output_exhausted = true;
            self.exhausted = true;
            return 0;
        }
        self.baseline_bytes += size;
        let i = self.facts.len();
        self.facts.push(f);
        i
    }
    fn module_emit(&mut self, kind: &str, n: Node<'_>, v: Value) -> usize {
        self.fact(kind, n, v, &self.source.module.clone())
    }
    fn scope(&mut self, parent: usize, kind: &'static str, n: Node<'a>) -> usize {
        let id = format!(
            "scope_{}",
            analysis::hash(
                json!([
                    self.source.path,
                    kind,
                    self.u(n.start_byte()),
                    self.u(n.end_byte()),
                    self.scopes[parent].id
                ])
                .to_string()
                .as_bytes()
            )
        );
        let i = self.scopes.len();
        self.scopes.push(Scope {
            id,
            parent: Some(parent),
            kind,
            node: Some(n),
            bindings: BTreeMap::new(),
        });
        i
    }
    fn bind(
        &mut self,
        scope: usize,
        name: String,
        node: Node<'a>,
        kind: &str,
        target: Option<Node<'a>>,
    ) {
        let i = self.bindings.len();
        self.scopes[scope]
            .bindings
            .entry(name.clone())
            .or_default()
            .push(i);
        self.bindings.push(Binding {
            scope,
            name,
            node,
            kind: kind.into(),
            target,
            writes: 0,
            guards: vec![],
        });
    }
    fn pattern(
        &mut self,
        s: usize,
        n: Node<'a>,
        node: Node<'a>,
        kind: &str,
        target: Option<Node<'a>>,
    ) {
        if self.stopped() {
            return;
        }
        if let Some(name) = self.direct(n) {
            self.bind(s, name, node, kind, target);
            return;
        }
        match n.kind() {
            "object_pattern" | "array_pattern" => {
                for c in children(n) {
                    self.pattern(s, c, node, kind, None);
                }
            }
            "pair_pattern" => {
                if let Some(v) = field(n, "value") {
                    self.pattern(s, v, node, kind, None);
                }
            }
            "assignment_pattern" | "object_assignment_pattern" => {
                if let Some(v) = field(n, "left") {
                    self.pattern(s, v, node, kind, None);
                }
            }
            "rest_pattern" => {
                if let Some(v) = n.named_child(0) {
                    self.pattern(s, v, node, kind, None);
                }
            }
            _ => (),
        }
    }
    fn lookup(&self, mut s: usize, name: &str) -> Option<&Vec<usize>> {
        loop {
            if let Some(bs) = self.scopes[s].bindings.get(name) {
                return Some(bs);
            }
            s = self.scopes[s].parent?;
        }
    }
    fn unique(&self, n: Node<'_>, name: &str) -> Option<usize> {
        self.lookup(*self.node_scope.get(&n.id())?, name)
            .filter(|bs| bs.len() == 1)
            .map(|bs| bs[0])
    }
    fn params(&self, n: Node<'a>) -> Vec<Node<'a>> {
        if let Some(ps) = field(n, "parameters") {
            children(ps)
                .into_iter()
                .filter(|n| n.kind() != "comment")
                .collect()
        } else {
            field(n, "parameter").into_iter().collect()
        }
    }
    fn pname(&self, p: Node<'a>) -> Node<'a> {
        let n = field(p, "pattern")
            .or_else(|| field(p, "name"))
            .unwrap_or(p);
        if n.kind() == "assignment_pattern" {
            field(n, "left").unwrap_or(n)
        } else if n.kind() == "rest_pattern" {
            n.named_child(0).unwrap_or(n)
        } else {
            n
        }
    }
    fn initializer(&self, n: Node<'a>) -> Option<Node<'a>> {
        field(n, "value").or_else(|| field(n, "right"))
    }
    fn walk(
        &mut self,
        n: Node<'a>,
        mut s: usize,
        depth: usize,
        omitted: bool,
        strict: bool,
    ) -> Result<(), ()> {
        if self.exhausted
            || self.all.len() >= 200_000
            || depth > 256
            || Instant::now() >= self.deadline
        {
            return Err(());
        }
        if depth == 0 {
            // Binding patterns are visited before recursive walk reaches them.
            // Bound the entire CST first, including deeply nested declarations.
            if !bounded_tree(n, self.deadline) {
                return Err(());
            }
            self.scopes[0].node = Some(n);
        }
        if self.direct(n).is_some() && self.text[n.byte_range()].contains('\\') {
            if let Some(name) = identifier_name(&self.text[n.byte_range()]) {
                self.names.insert(n.id(), name);
            }
        }
        self.all.push(n);
        let p = n.parent();
        let strict = strict
            || (depth == 0
                && (children(n)
                    .iter()
                    .any(|n| matches!(n.kind(), "import_statement" | "export_statement"))
                    || self.source.path.ends_with(".mjs")
                    || self.source.path.ends_with(".mts")
                    || self.strict_body(n)));
        if matches!(
            n.kind(),
            "function_declaration" | "generator_function_declaration"
        ) && !strict
            && p.is_some_and(|p| {
                p.kind() != "program"
                    && !(p.kind() == "statement_block" && p.parent().is_some_and(callable))
            })
        {
            if !self.annex {
                self.diagnostic("javascript_annex_b_binding_unresolved", n);
            }
            self.annex = true;
        }
        let strict = strict
            || class(n)
            || (callable(n) && field(n, "body").is_some_and(|b| self.strict_body(b)));
        let parameter_boundary = p.is_some_and(|p| {
            matches!(p.kind(), "required_parameter" | "optional_parameter")
                && (field(p, "value") == Some(n)
                    || (field(p, "pattern") == Some(n) && self.direct(n).is_none()))
        }) || p.is_some_and(|p| {
            p.kind() == "assignment_pattern"
                && p.parent().is_some_and(|p| p.kind() == "formal_parameters")
                && field(p, "right") == Some(n)
        }) || (matches!(n.kind(), "object_pattern" | "array_pattern")
            && p.is_some_and(|p| {
                p.kind() == "formal_parameters"
                    || (matches!(p.kind(), "assignment_pattern" | "rest_pattern")
                        && p.parent().is_some_and(|p| p.kind() == "formal_parameters"))
            }));
        let boundary = matches!(
            n.kind(),
            "class_static_block"
                | "class_heritage"
                | "extends_clause"
                | "implements_clause"
                | "computed_property_name"
                | "decorator"
        ) || (n.kind() == "method_definition" && self.accessor(n))
            || parameter_boundary
            || p.is_some_and(|p| {
                matches!(p.kind(), "public_field_definition" | "field_definition")
                    && field(p, "value") == Some(n)
                    && !callable(unwrap(n))
            });
        if boundary && !omitted {
            self.diagnostic("javascript_unsupported_execution_boundary", n);
        }
        let omitted = omitted || boundary;
        if parameter_boundary {
            // Defaults cannot see declarations in the body. Reuse parameter
            // bindings for write accounting, but bypass the body's environment.
            if let Some(parameter_scope) = self.parameter_scopes.get(&s) {
                s = *parameter_scope;
            } else if let Some(parent) = self.scopes[s].parent {
                let original = s;
                let parameters = self.scopes[s]
                    .node
                    .and_then(|f| field(f, "parameters"))
                    .unwrap_or(n);
                let bindings = self.scopes[s].bindings.clone();
                s = self.scope(parent, "function", parameters);
                self.scopes[s].bindings = bindings;
                self.parameter_scopes.insert(original, s);
            }
        }
        if !omitted && !(n.kind() == "call_expression" && (self.type_position(n) || !call(n))) {
            self.eligible.insert(n.id());
        }
        if matches!(
            n.kind(),
            "function_declaration" | "generator_function_declaration" | "function_signature"
        ) {
            if let Some(name) = field(n, "name").and_then(|n| self.direct(n)) {
                self.bind(s, name, n, "function", callable(n).then_some(n));
            }
        } else if matches!(
            n.kind(),
            "class_declaration"
                | "abstract_class_declaration"
                | "enum_declaration"
                | "internal_module"
        ) {
            if let Some(name) = field(n, "name").and_then(|n| self.direct(n)) {
                self.bind(
                    s,
                    name,
                    n,
                    if class(n) { "class" } else { "unsupported" },
                    None,
                );
            }
        } else if n.kind() == "variable_declarator" {
            let kind = if p.is_some_and(|p| self.has(p, "const")) {
                "const"
            } else if p.is_some_and(|p| self.has(p, "let")) {
                "let"
            } else {
                "var"
            };
            let mut dest = s;
            if kind == "var" {
                while self.scopes[dest].parent.is_some()
                    && !matches!(self.scopes[dest].kind, "module" | "function")
                {
                    dest = self.scopes[dest].parent.unwrap();
                }
            }
            if let Some(name) = field(n, "name") {
                self.pattern(
                    dest,
                    name,
                    n,
                    kind,
                    self.initializer(n).map(unwrap).filter(|n| callable(*n)),
                );
            }
        } else if n.kind() == "import_specifier" {
            if let Some(name) = field(n, "alias")
                .or_else(|| field(n, "name"))
                .and_then(|n| self.direct(n))
            {
                self.bind(s, name, n, "import", None);
            }
        } else if n.kind() == "namespace_import" {
            if let Some(name) = children(n).last().and_then(|n| self.direct(*n)) {
                self.bind(s, name, n, "import", None);
            }
        } else if n.kind() == "import_clause" {
            for c in children(n) {
                if c.kind() == "identifier" {
                    self.bind(s, self.direct(c).unwrap(), n, "import", None);
                }
            }
        } else if matches!(n.kind(), "import_require_clause" | "import_alias") {
            if let Some(name) = children(n)
                .into_iter()
                .find(|n| n.kind() == "identifier")
                .and_then(|n| self.direct(n))
            {
                self.bind(s, name, n, "import", None);
            }
        }
        if callable(n)
            || matches!(
                n.kind(),
                "function_signature" | "method_signature" | "abstract_method_signature"
            )
        {
            s = self.scope(s, "function", n);
            if matches!(n.kind(), "function_expression" | "generator_function") {
                if let Some(name) = field(n, "name").and_then(|n| self.direct(n)) {
                    self.bind(s, name, n, "function", Some(n));
                }
            }
            for p in self.params(n) {
                self.pattern(s, self.pname(p), p, "parameter", None);
            }
        } else if class(n) {
            s = self.scope(s, "class", n);
            if let Some(name) = field(n, "name").and_then(|n| self.direct(n)) {
                self.bind(s, name, n, "class", None);
            }
        } else if n.kind() == "class_static_block" {
            s = self.scope(s, "function", n);
        } else if matches!(
            n.kind(),
            "statement_block"
                | "switch_body"
                | "for_statement"
                | "for_in_statement"
                | "catch_clause"
        ) && !(n.kind() == "statement_block" && p.is_some_and(callable))
        {
            s = self.scope(s, "block", n);
            if n.kind() == "catch_clause" {
                if let Some(p) = field(n, "parameter") {
                    self.pattern(s, p, p, "catch", None);
                }
            }
        }
        if n.kind() == "for_in_statement" {
            if let (Some(kind), Some(lhs)) = (field(n, "kind"), field(n, "left")) {
                let kind = kind.kind();
                let mut dest = s;
                if kind == "var" {
                    while self.scopes[dest].parent.is_some()
                        && !matches!(self.scopes[dest].kind, "module" | "function")
                    {
                        dest = self.scopes[dest].parent.unwrap();
                    }
                }
                self.pattern(dest, lhs, lhs, kind, None);
            }
        }
        self.node_scope.insert(n.id(), s);
        for c in children(n) {
            self.walk(c, s, depth + 1, omitted, strict)?;
        }
        if self.exhausted {
            Err(())
        } else {
            Ok(())
        }
    }
    fn strict_body(&self, n: Node<'_>) -> bool {
        for c in children(n) {
            if c.kind() == "comment" {
                continue;
            }
            if c.kind() != "expression_statement" {
                break;
            }
            let Some(e) = c.named_child(0) else {
                break;
            };
            if e.kind() != "string" {
                break;
            }
            if matches!(self.raw(e), "'use strict'" | "\"use strict\"") {
                return true;
            }
        }
        false
    }
    fn accessor(&self, n: Node<'_>) -> bool {
        let mut c = n.walk();
        let found = n
            .children(&mut c)
            .any(|n| matches!(n.kind(), "get" | "set"));
        found
    }
    fn has(&self, n: Node<'_>, token: &str) -> bool {
        let mut c = n.walk();
        let found = n.children(&mut c).any(|n| {
            n.kind() == token
                || (matches!(n.kind(), "accessibility_modifier" | "override_modifier")
                    && self.raw(n) == token)
        });
        found
    }
    fn visibility(&self, n: Node<'_>) -> &'static str {
        if field(n, "name").is_some_and(|n| n.kind() == "private_property_identifier")
            || self.has(n, "private")
        {
            "private"
        } else if self.has(n, "protected") {
            "protected"
        } else {
            "public"
        }
    }
    fn owner(&self, n: Node<'a>, include_class: bool) -> Option<usize> {
        let mut p = n.parent();
        while let Some(x) = p {
            if let Some(d) = self.declarations.get(&x.id()) {
                if include_class || callable(x) {
                    return Some(d.fact);
                }
            }
            if callable(x)
                || matches!(
                    x.kind(),
                    "class_static_block"
                        | "class_heritage"
                        | "extends_clause"
                        | "required_parameter"
                        | "optional_parameter"
                )
                || (matches!(x.kind(), "public_field_definition" | "field_definition")
                    && field(x, "value").is_some_and(|v| !callable(unwrap(v))))
            {
                return None;
            }
            p = x.parent();
        }
        Some(0)
    }
    fn expr(&self, n: Node<'_>) -> String {
        let mut budget = 80;
        self.render(n, false, 0, &mut budget)
    }
    fn typ(&self, n: Node<'_>) -> String {
        let mut budget = 80;
        self.render(n, true, 0, &mut budget)
    }
    fn render(&self, n: Node<'_>, ty: bool, depth: usize, budget: &mut usize) -> String {
        if *budget == 0 || depth > 12 {
            return "...".into();
        }
        *budget -= 1;
        if (ty || self.type_position(n))
            && matches!(n.kind(), "member_expression" | "call_expression")
        {
            let mut base = n;
            while let Some(next) = match base.kind() {
                "member_expression" => field(base, "object"),
                "call_expression" => field(base, "function"),
                _ => None,
            } {
                base = next;
            }
            if base.kind() == "import" {
                return "<ImportType>".into();
            }
        }
        let mut child = |n| self.render(n, ty, depth + 1, budget);
        let value = match n.kind() {
            "identifier"
            | "type_identifier"
            | "property_identifier"
            | "private_property_identifier"
            | "this"
            | "super"
            | "nested_type_identifier" => {
                if n.kind() == "nested_type_identifier" {
                    children(n)
                        .into_iter()
                        .map(&mut child)
                        .collect::<Vec<_>>()
                        .join(".")
                } else {
                    self.raw(n).into()
                }
            }
            "predefined_type" => {
                if ty {
                    self.raw(n).into()
                } else {
                    format!("<{}Keyword>", syntax_label(self.raw(n)))
                }
            }
            "string" | "number" | "true" | "false" | "null" | "undefined" | "regex"
            | "template_string" => "<literal>".into(),
            "literal_type" | "template_literal_type" => {
                if ty {
                    "<literal>".into()
                } else {
                    format!("<{}>", syntax_label(n.kind()))
                }
            }
            "member_expression" => format!(
                "{}.{}",
                field(n, "object").map(&mut child).unwrap_or_default(),
                field(n, "property").map(&mut child).unwrap_or_default()
            ),
            "subscript_expression" => format!(
                "{}[{}]",
                field(n, "object").map(&mut child).unwrap_or_default(),
                field(n, "index").map(&mut child).unwrap_or_default()
            ),
            "call_expression" | "new_expression" => {
                let callee = field(
                    n,
                    if n.kind() == "call_expression" {
                        "function"
                    } else {
                        "constructor"
                    },
                )
                .map(&mut child)
                .unwrap_or_default();
                let args = field(n, "arguments")
                    .map(children)
                    .unwrap_or_default()
                    .into_iter()
                    .take(16)
                    .map(&mut child)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "{}{}({})",
                    if n.kind() == "new_expression" {
                        "new "
                    } else {
                        ""
                    },
                    callee,
                    args
                )
            }
            "parenthesized_expression" | "parenthesized_type" => {
                format!("({})", n.named_child(0).map(&mut child).unwrap_or_default())
            }
            "binary_expression" | "assignment_expression" | "augmented_assignment_expression" => {
                format!(
                    "{} <operator> {}",
                    field(n, "left").map(&mut child).unwrap_or_default(),
                    field(n, "right").map(&mut child).unwrap_or_default()
                )
            }
            "await_expression" => format!(
                "await {}",
                n.named_child(0).map(&mut child).unwrap_or_default()
            ),
            "array" | "tuple_type" => format!(
                "[{}]",
                children(n)
                    .into_iter()
                    .take(16)
                    .map(&mut child)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            "type_annotation" => n.named_child(0).map(&mut child).unwrap_or_default(),
            "generic_type" if ty => {
                let name = field(n, "name").map(&mut child).unwrap_or_default();
                let args = field(n, "type_arguments")
                    .map(children)
                    .unwrap_or_default()
                    .into_iter()
                    .map(&mut child)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{name}<{args}>")
            }
            "generic_type" => field(n, "name").map(&mut child).unwrap_or_default(),
            "union_type" | "intersection_type" if ty => children(n)
                .into_iter()
                .map(&mut child)
                .collect::<Vec<_>>()
                .join(if n.kind() == "union_type" {
                    " | "
                } else {
                    " & "
                }),
            "array_type" if ty => {
                format!("{}[]", n.named_child(0).map(&mut child).unwrap_or_default())
            }
            "rest_type" if ty => format!(
                "...{}",
                n.named_child(0).map(&mut child).unwrap_or_default()
            ),
            "optional_type" if ty => {
                format!("{}?", n.named_child(0).map(&mut child).unwrap_or_default())
            }
            "readonly_type" | "index_type_query" if ty => format!(
                "{} {}",
                if n.kind() == "readonly_type" {
                    "readonly"
                } else {
                    "keyof"
                },
                n.named_child(0).map(&mut child).unwrap_or_default()
            ),
            "lookup_type" if ty => {
                let cs = children(n);
                format!(
                    "{}[{}]",
                    cs.first().copied().map(&mut child).unwrap_or_default(),
                    cs.get(1).copied().map(&mut child).unwrap_or_default()
                )
            }
            "required_parameter" | "optional_parameter" if ty => {
                let name = field(n, "name")
                    .or_else(|| field(n, "pattern"))
                    .map(&mut child)
                    .unwrap_or_default();
                let value = field(n, "type").map(&mut child).unwrap_or_default();
                format!(
                    "{}{}: {}",
                    name,
                    if n.kind() == "optional_parameter" {
                        "?"
                    } else {
                        ""
                    },
                    value
                )
            }
            "object_type" => "<TypeLiteral>".into(),
            "object" => "<ObjectLiteralExpression>".into(),
            "arrow_function" => "<ArrowFunction>".into(),
            "function_expression" => "<FunctionExpression>".into(),
            "class" => "<ClassExpression>".into(),
            "as_expression" => "<AsExpression>".into(),
            "jsx_element" | "jsx_self_closing_element" => "<JsxElement>".into(),
            _ => format!("<{}>", syntax_label(n.kind())),
        };
        bounded(value, 512)
    }
    fn analyze(&mut self) {
        let declarations = self
            .all
            .iter()
            .filter(|n| self.eligible.contains(&n.id()) && (class(**n) || callable(**n)))
            .count();
        let guards = self
            .bindings
            .iter()
            .filter(|b| {
                self.eligible.contains(&b.node.id())
                    || (b.kind == "parameter"
                        && b.node
                            .parent()
                            .is_some_and(|p| p.kind() == "formal_parameters"))
            })
            .count();
        if self.baseline_prior_count + 1 + 2 * declarations + guards > MAX_FACTS {
            self.exhausted = true;
            return;
        }
        let all = self.all.clone();
        for n in &all {
            if self.stopped() {
                return;
            }
            if matches!(
                n.kind(),
                "assignment_expression"
                    | "augmented_assignment_expression"
                    | "update_expression"
                    | "for_in_statement"
            ) || (n.kind() == "unary_expression" && self.has(*n, "delete"))
            {
                if let Some(lhs) = field(*n, "left")
                    .or_else(|| field(*n, "argument"))
                    .or_else(|| field(*n, "operand"))
                {
                    self.write(lhs, self.node_scope[&n.id()]);
                }
            }
        }
        self.dynamic = all.iter().any(|n| {
            n.kind() == "with_statement"
                || (call(*n)
                    && field(*n, "function").is_some_and(|c| {
                        self.raw(c) == "eval"
                            && self.lookup(self.node_scope[&n.id()], "eval").is_none()
                    }))
        });
        // Invalid/unmodeled identifier escapes must not alias stable bindings.
        if let Some(n) = all
            .iter()
            .find(|n| {
                matches!(n.kind(), "identifier" | "type_identifier") && self.raw(**n).contains('\\')
            })
            .copied()
        {
            self.dynamic = true;
            self.diagnostic("javascript_identifier_escape_unresolved", n);
        }
        self.export_assignments = all
            .iter()
            .copied()
            .filter(|n| {
                n.kind() == "assignment_expression"
                    && self.eligible.contains(&n.id())
                    && field(*n, "left").is_some_and(|l| self.module_exports_reference(l))
            })
            .collect();
        if self.export_assignments.len() == 1
            && self.export_write_count() == 1
            && !self.dynamic
            && !self.global_writes.contains("module")
            && !self.global_writes.contains("exports")
        {
            let n = self.export_assignments[0];
            let s = self.node_scope[&n.id()];
            if self.lookup(s, "module").is_none() && self.lookup(s, "exports").is_none() {
                self.factory = field(n, "right").map(unwrap).filter(|n| callable(*n));
            }
        }
        for n in &all {
            if self.stopped() {
                return;
            }
            if self.eligible.contains(&n.id()) && (class(*n) || callable(*n)) {
                self.declaration(*n);
            }
        }
        if self.stopped() {
            return;
        }
        self.guards();
        if self.stopped() {
            return;
        }
        for i in 0..self.bindings.len() {
            if self.stopped() {
                return;
            }
            if let Some(c) = self.class_target(i) {
                self.class_bindings.entry(c.id()).or_default().push(i);
            }
        }
        self.members();
        if self.stopped() {
            return;
        }
        self.modules();
        if self.stopped() {
            return;
        }
        self.calls_and_imports();
        if self.stopped() {
            return;
        }
        self.routes();
        if self.stopped() {
            return;
        }
        self.structural_facts();
        if self.stopped() {
            return;
        }
        self.class_relations();
    }
    fn stopped(&mut self) -> bool {
        self.exhausted |= Instant::now() >= self.deadline;
        self.exhausted
    }
    fn write(&mut self, n: Node<'a>, s: usize) {
        let n = unwrap(n);
        if n.kind() == "identifier" {
            let name = self.raw(n).to_owned();
            if let Some(bs) = self.lookup(s, &name).cloned() {
                for b in bs {
                    self.bindings[b].writes += 1;
                }
            } else {
                self.global_writes.insert(name);
            }
        } else if matches!(n.kind(), "member_expression" | "subscript_expression") {
            self.property_writes.push(n);
            let mut base = n;
            while matches!(base.kind(), "member_expression" | "subscript_expression") {
                if let Some(x) = field(base, "object") {
                    base = x;
                } else {
                    break;
                }
            }
            if base.kind() == "identifier" {
                if let Some(bs) = self.lookup(s, self.raw(base)).cloned() {
                    self.mutated.extend(bs);
                }
            }
        } else if matches!(
            n.kind(),
            "object_pattern"
                | "array_pattern"
                | "object"
                | "array"
                | "rest_pattern"
                | "spread_element"
        ) {
            for c in children(n) {
                self.write(c, s);
            }
        } else if matches!(n.kind(), "pair" | "pair_pattern") {
            if let Some(v) = field(n, "value") {
                self.write(v, s);
            }
        } else if matches!(
            n.kind(),
            "assignment_pattern" | "object_assignment_pattern" | "assignment_expression"
        ) {
            if let Some(v) = field(n, "left") {
                self.write(v, s);
            }
        } else if matches!(
            n.kind(),
            "shorthand_property_identifier_pattern" | "shorthand_property_identifier"
        ) {
            let name = self.raw(n).to_owned();
            if let Some(bs) = self.lookup(s, &name).cloned() {
                for b in bs {
                    self.bindings[b].writes += 1;
                }
            } else {
                self.global_writes.insert(name);
            }
        }
    }
    fn declaration(&mut self, n: Node<'a>) {
        let Some(own) = self.owner(n, true) else {
            return;
        };
        let p = n.parent();
        let named = p
            .filter(|p| p.kind() == "variable_declarator")
            .and_then(|p| field(p, "name"))
            .and_then(|n| self.direct(n));
        let arrow_field = p
            .filter(|p| matches!(p.kind(), "public_field_definition" | "field_definition"))
            .and_then(|p| field(p, "name"))
            .and_then(|n| self.direct(n));
        let constructor = n.kind() == "method_definition"
            && field(n, "name").is_some_and(|n| self.raw(n) == "constructor");
        let name = named
            .clone()
            .or(arrow_field.clone())
            .or_else(|| field(n, "name").and_then(|n| self.direct(n)))
            .unwrap_or_else(|| {
                format!(
                    "{}_L{}_C{}",
                    if class(n) {
                        "class"
                    } else if Some(n) == self.factory {
                        "factory"
                    } else {
                        "callback"
                    },
                    self.line(n.start_byte()),
                    self.u(n.start_byte()) - self.u(self.lines[self.line(n.start_byte()) - 1]) + 1
                )
            });
        let symbol = format!("{}.{}", self.facts[own].symbol, name);
        let vis = self.visibility(if arrow_field.is_some() { p.unwrap() } else { n });
        let kind = if class(n) {
            "class"
        } else if constructor {
            "constructor"
        } else if Some(n) == self.factory {
            "factory"
        } else if n.kind() == "method_definition" || arrow_field.is_some() {
            "method"
        } else if named.is_some()
            || matches!(
                n.kind(),
                "function_declaration" | "generator_function_declaration"
            )
        {
            if n.kind() == "arrow_function" {
                "arrow"
            } else {
                "function"
            }
        } else {
            "callback"
        };
        let f = self.fact(
            if class(n) {
                "class"
            } else if self.has(n, "async") {
                "async_function"
            } else {
                "function"
            },
            n,
            json!(name),
            &symbol,
        );
        let mut meta = json!({"declarationFactId":self.facts[f].id,"ownerFactId":self.facts[own].id,"kind":kind,"name":name,"publicCandidate":vis=="public"&&!constructor&&kind!="callback"&&(kind!="method"||field(n,"name").and_then(|n|self.direct(n)).is_some()||arrow_field.is_some()),"visibility":vis});
        if class(n) {
            meta["classForm"] = json!(if n.kind() == "class" {
                "class_expression"
            } else {
                "class_declaration"
            });
            meta["start"] = json!(self.u(n.start_byte()));
            meta["end"] = json!(self.u(n.end_byte()));
        }
        if kind == "method" {
            meta["static"] =
                json!(self.has(if arrow_field.is_some() { p.unwrap() } else { n }, "static"));
        }
        let m = self.fact("js_declaration", n, meta, &symbol);
        self.declarations.insert(
            n.id(),
            Decl {
                fact: f,
                meta: m,
                kind: kind.into(),
            },
        );
        if class(n) {
            for base in self.heritage(n) {
                if self.stopped() {
                    return;
                }
                let v = self.expr(base);
                let i = self.fact("base", base, json!(v), &symbol);
                self.bases.insert(base.id(), i);
            }
        } else {
            for p in self.params(n) {
                if self.stopped() {
                    return;
                }
                let name = self.direct(self.pname(p)).unwrap_or("<binding>".into());
                let sym = format!("{symbol}.{name}");
                let i = self.fact("parameter", p, json!(name), &sym);
                self.parameters.insert(p.id(), i);
                if let Some(t) = field(p, "type") {
                    self.fact("parameter_annotation", p, json!(self.expr(t)), &sym);
                }
            }
            if let Some(t) = field(n, "return_type") {
                self.fact("return_annotation", t, json!(self.expr(t)), &symbol);
            }
        }
    }
    fn heritage(&self, n: Node<'a>) -> Vec<Node<'a>> {
        let mut result = vec![];
        for h in children(n) {
            if h.kind() == "class_heritage" {
                for c in children(h) {
                    if c.kind() == "extends_clause" {
                        result.extend(
                            children(c)
                                .into_iter()
                                .filter(|n| n.kind() != "type_arguments"),
                        );
                    } else if c.kind() != "implements_clause" {
                        result.push(c);
                    }
                }
            }
        }
        result
    }
    fn guards(&mut self) {
        for i in 0..self.bindings.len() {
            if self.stopped() {
                return;
            }
            let b = self.bindings[i].clone();
            let parameter_pattern = b.kind == "parameter"
                && b.node
                    .parent()
                    .is_some_and(|p| p.kind() == "formal_parameters");
            if !self.eligible.contains(&b.node.id()) && !parameter_pattern {
                continue;
            }
            let s = &self.scopes[b.scope];
            let sn = s.node.unwrap();
            let mut v = json!({"name":b.name,"bindingKind":b.kind,"scopeId":s.id,"scopeKind":s.kind,"scopeStart":self.u(sn.start_byte()),"scopeEnd":self.u(sn.end_byte()),"declarationCount":s.bindings[&b.name].len(),"writes":b.writes,"declarationFactId":b.target.and_then(|t|self.declarations.get(&t.id())).map(|d|&self.facts[d.fact].id)});
            if b.kind == "class" || b.node.kind() == "variable_declarator" {
                v["initializationEnd"] = json!(self.u(b.node.end_byte()));
            } else if b.kind == "function" && b.target.is_some() {
                v["initializationEnd"] = Value::Null;
            }
            let f = self.module_emit("js_binding_guard", b.node, v);
            self.bindings[i].guards.push(self.facts[f].id.clone());
        }
    }
    fn members(&mut self) {
        for c in self.all.clone().into_iter().filter(|n| class(*n)) {
            let Some(cd) = self.declarations.get(&c.id()).cloned() else {
                continue;
            };
            let Some(body) = field(c, "body") else {
                continue;
            };
            for m in children(body) {
                if self.stopped() {
                    return;
                }
                if !matches!(
                    m.kind(),
                    "method_definition"
                        | "public_field_definition"
                        | "field_definition"
                        | "method_signature"
                        | "abstract_method_signature"
                ) {
                    continue;
                }
                let Some(name) = field(m, "name").and_then(|n| self.direct(n)) else {
                    continue;
                };
                let target = if callable(m) && !self.accessor(m) {
                    Some(m)
                } else {
                    field(m, "value").map(unwrap).filter(|n| callable(*n))
                };
                let kind = if name == "constructor" {
                    "constructor"
                } else if target.is_some()
                    || matches!(m.kind(), "method_signature" | "abstract_method_signature")
                {
                    "method"
                } else {
                    "field"
                };
                self.member(cd.fact, m, &name, kind, target);
                if name == "constructor" {
                    for p in self.params(m) {
                        if self.parameter_property(p) {
                            if let Some(name) = self.direct(self.pname(p)) {
                                self.member(cd.fact, p, &name, "field", None);
                            }
                        }
                    }
                }
            }
        }
    }
    fn parameter_property(&self, p: Node<'_>) -> bool {
        self.has(p, "public")
            || self.has(p, "private")
            || self.has(p, "protected")
            || self.has(p, "readonly")
    }
    fn member(
        &mut self,
        owner: usize,
        n: Node<'a>,
        name: &str,
        kind: &str,
        target: Option<Node<'a>>,
    ) {
        let mut v = json!({"ownerFactId":self.facts[owner].id,"name":name,"memberKind":kind,"visibility":self.visibility(n),"static":self.has(n,"static")});
        if let Some(d) = target.and_then(|t| self.declarations.get(&t.id())) {
            v["declarationFactId"] = json!(self.facts[d.fact].id);
        }
        if let Some(t) = field(n, "type").or_else(|| field(n, "return_type")) {
            v["type"] = json!(self.typ(t));
        }
        self.fact(
            "js_member",
            n,
            v,
            &format!("{}.{}", self.facts[owner].symbol, name),
        );
    }
    fn resolve(&self, n: Node<'a>, name: &str) -> Option<usize> {
        if self.dynamic || self.annex {
            return None;
        }
        let i = self.unique(n, name)?;
        let b = &self.bindings[i];
        let t = b.target?;
        let d = self.declarations.get(&t.id())?;
        if b.writes > 0
            || b.guards.len() != 1
            || self.facts[d.fact].value != name
            || (matches!(b.kind.as_str(), "const" | "let" | "var")
                && n.start_byte() < b.node.end_byte())
        {
            None
        } else {
            Some(i)
        }
    }
    fn root_binding(&self, name: &str) -> Option<usize> {
        let bs = self.scopes[0].bindings.get(name)?;
        if bs.len() != 1 {
            return None;
        }
        let i = bs[0];
        let b = &self.bindings[i];
        (b.writes == 0 && b.guards.len() == 1).then_some(i)
    }
    fn class_target(&self, i: usize) -> Option<Node<'a>> {
        let b = &self.bindings[i];
        let n = if b.node.kind() == "variable_declarator" {
            let name = field(b.node, "name")?;
            if name.kind() != "identifier" || self.raw(name) != b.name {
                return None;
            }
            self.initializer(b.node).map(unwrap)?
        } else {
            b.node
        };
        (class(n) && self.declarations.contains_key(&n.id())).then_some(n)
    }
    fn export_write_count(&self) -> usize {
        self.property_writes
            .iter()
            .filter(|n| {
                let mut n = **n;
                while matches!(n.kind(), "member_expression" | "subscript_expression") {
                    n = field(n, "object").unwrap();
                }
                matches!(self.raw(n), "module" | "exports")
            })
            .count()
    }
    fn module_exports_reference(&self, n: Node<'_>) -> bool {
        n.kind() == "member_expression"
            && !self.has(n, "optional_chain")
            && field(n, "object")
                .is_some_and(|n| n.kind() == "identifier" && self.raw(n) == "module")
            && field(n, "property").is_some_and(|n| self.raw(n) == "exports")
    }
    fn type_position(&self, n: Node<'_>) -> bool {
        let mut child = n;
        let mut parent = n.parent();
        while let Some(p) = parent {
            if p.kind().ends_with("_type")
                || matches!(
                    p.kind(),
                    "type_annotation"
                        | "type_arguments"
                        | "type_parameters"
                        | "type_query"
                        | "type_alias_declaration"
                        | "interface_declaration"
                        | "implements_clause"
                        | "extends_type_clause"
                        | "asserts_annotation"
                        | "type_predicate_annotation"
                )
            {
                return true;
            }
            if matches!(p.kind(), "as_expression" | "satisfies_expression")
                && p.named_child(0) != Some(child)
            {
                return true;
            }
            if callable(p) || class(p) || p.kind() == "program" {
                break;
            }
            child = p;
            parent = p.parent();
        }
        false
    }
}
fn identifier_name(raw: &str) -> Option<String> {
    let mut cs = raw.chars();
    let mut out = String::new();
    while let Some(c) = cs.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        if cs.next() != Some('u') {
            return None;
        }
        let digits = if cs.clone().next() == Some('{') {
            cs.next();
            let mut digits = String::new();
            loop {
                let c = cs.next()?;
                if c == '}' {
                    break;
                }
                digits.push(c);
                if digits.len() > 6 {
                    return None;
                }
            }
            digits
        } else {
            let digits = cs.by_ref().take(4).collect::<String>();
            if digits.len() != 4 {
                return None;
            }
            digits
        };
        let c = char::from_u32(u32::from_str_radix(&digits, 16).ok()?)?;
        if c.is_control() {
            return None;
        }
        out.push(c);
    }
    Some(out)
}
fn syntax_label(kind: &str) -> String {
    kind.split('_')
        .map(|s| {
            let mut cs = s.chars();
            cs.next()
                .map(|c| c.to_ascii_uppercase().to_string() + cs.as_str())
                .unwrap_or_default()
        })
        .collect()
}
fn bounded(mut s: String, limit: usize) -> String {
    if s.len() > limit {
        let mut end = limit;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    fn extract(path: &str, text: &str) -> NativeResult {
        extract_sources(&[NativeSource {
            path: path.into(),
            module: path.replace('/', "."),
            bytes: text.as_bytes().to_vec(),
        }])
        .unwrap()
    }
    fn values(r: &NativeResult, kind: &str) -> Vec<Value> {
        r.facts
            .iter()
            .filter(|f| f.kind == kind)
            .map(|f| serde_json::from_str(&f.value).unwrap())
            .collect()
    }
    fn evidence(sources: &[NativeSource]) -> analysis::Evidence {
        let result = extract_sources(sources).unwrap();
        valid(&result);
        let files = sources
            .iter()
            .map(|s| analysis::SourceFile {
                path: s.path.clone(),
                sha256: analysis::hash(&s.bytes),
            })
            .collect::<Vec<_>>();
        analysis::Evidence {
            schema_version: "ideomancer.evidence.v2".into(),
            snapshot_id: analysis::hash(&serde_json::to_vec(&files).unwrap()),
            extractor_version: crate::javascript::EXTRACTOR_VERSION.into(),
            language: Some(analysis::LanguageMetadata {
                name: "javascript-typescript".into(),
                parser: "tree-sitter".into(),
                parser_version: crate::javascript::PARSER_VERSION.into(),
                parser_sha256: fingerprint(),
                runtime: "native".into(),
                runtime_version: env!("CARGO_PKG_VERSION").into(),
                scan_policy: crate::javascript::SCAN_POLICY.into(),
            }),
            files,
            facts: result.facts,
            diagnostics: result.diagnostics,
            complete: result.complete,
            ..Default::default()
        }
    }
    fn valid(r: &NativeResult) {
        assert!(r.complete, "{:?}", r.diagnostics);
        crate::js_focus::Index::new(r.facts.iter().collect()).unwrap();
        let ids = r
            .facts
            .iter()
            .map(|f| f.id.as_str())
            .collect::<HashSet<_>>();
        for f in &r.facts {
            if let Ok(v) = serde_json::from_str::<Value>(&f.value) {
                if let Some(ids2) = v["supportingFactIds"].as_array() {
                    for id in ids2 {
                        assert!(ids.contains(id.as_str().unwrap()), "{}: {id}", f.kind);
                    }
                }
                for key in [
                    "declarationFactId",
                    "ownerFactId",
                    "factoryFactId",
                    "moduleFactId",
                    "callFactId",
                    "classFactId",
                    "bindingGuardFactId",
                    "referenceFactId",
                    "targetFactId",
                    "importFactId",
                    "targetMetadataFactId",
                ] {
                    if let Some(id) = v[key].as_str() {
                        assert!(ids.contains(id), "{} {key}: {id}", f.kind);
                    }
                }
                if let Some(owner) = v["ownerFactId"]
                    .as_str()
                    .and_then(|id| r.facts.iter().find(|f| f.id == id))
                {
                    assert!(
                        owner.path == f.path
                            && owner.line <= f.line
                            && owner.end_line >= f.end_line,
                        "{}: invalid owner span",
                        f.kind
                    );
                }
            }
        }
    }
    #[test]
    fn grammar_matrix_and_redaction() {
        for(path,text)in[
            ("a.js","export function run(x = 'PRIVATE_DEFAULT', ...rest) { return x.run('PRIVATE_PAYLOAD'); }"),
            ("a.jsx","export const view = x => <main title='PRIVATE_TITLE'>{x.render('PRIVATE_PAYLOAD')}</main>;"),
            ("a.ts","export class A { constructor(private dep: Service) { this.dep = dep; } method<T>(x: T | 'PRIVATE_TYPE'): Promise<T> { return this.dep.run(x, 'PRIVATE_PAYLOAD'); } }"),
            ("a.tsx","export const view = (x: Service) => <main title='PRIVATE_TITLE'>{x.render('PRIVATE_PAYLOAD')}</main>;"),
        ]{let r=extract(path,text);valid(&r);let encoded=serde_json::to_string(&r.facts).unwrap();assert!(!encoded.contains("PRIVATE_"),"{path}: {encoded}");assert!(r.facts.iter().any(|f|f.kind=="js_declaration"));}
    }
    #[test]
    fn invalid_sources_have_module_only() {
        for text in [
            "export function broken( {",
            "class Good {}\nconst x = ;",
            "function f() { return (1; }",
        ] {
            let r = extract("a.ts", text);
            assert!(!r.complete);
            assert_eq!(r.facts.len(), 1);
            assert_eq!(r.facts[0].kind, "module");
        }
        let r = extract_sources(&[NativeSource {
            path: "a.js".into(),
            module: "a".into(),
            bytes: vec![0xff],
        }])
        .unwrap();
        assert!(!r.complete);
        assert_eq!(r.facts.len(), 1);
    }
    #[test]
    fn lexical_resolution_rejects_shadow_writes_tdz_and_annex_b() {
        for text in [
            "function target() {} function run(target) { target(); }",
            "function target() {} function run() { target(); let target = 1; }",
            "function target() {} target = other; target();",
            "target(); const target = () => {};",
            "{ function target() {} } target();",
            "function target() {} function run() { eval('PRIVATE_CODE'); target(); }",
            "function target() {} function run() { function omitted(x = (target = other)) {} target(); }",
            "{ const/**/target = () => {}; } target();",
        ]{let r=extract("a.js",text);valid(&r);assert!(values(&r,"js_local_call_candidate").is_empty(),"{text}");}
        let r = extract(
            "a.js",
            "function target() {} function run() { target(); } run();",
        );
        valid(&r);
        assert_eq!(values(&r, "js_local_call_candidate").len(), 2);
    }
    #[test]
    fn boundaries_do_not_emit_recovered_callable_facts() {
        let r=extract("a.ts","function helper() {} class C extends mixin(() => helper()) { field = helper(); get value() { helper(); return 1; } method(x = helper()) { helper(); } [helper()]() { helper(); } static { helper(); } }");
        valid(&r);
        assert!(r
            .diagnostics
            .iter()
            .any(|d| d.code == "javascript_unsupported_execution_boundary"));
        assert_eq!(r.facts.iter().filter(|f| f.kind == "call").count(), 2);
    }
    #[test]
    fn utf16_factory_forms_and_class_receipts() {
        for (body, form, expected) in [
            ("class C {} return C;", "identifier", "C"),
            ("class C {} return new C();", "new_identifier", "new C()"),
            ("return class C {};", "class_expression", "class C {}"),
            (
                "return new class C {}();",
                "new_class_expression",
                "new class C {}()",
            ),
            ("const C = class {}; return C;", "identifier", "C"),
        ] {
            let text = format!("// \u{1f600}\r\nmodule.exports = () => {{ {body} }};");
            let r = extract("a.js", &text);
            valid(&r);
            let ret = values(&r, "js_factory_return").remove(0);
            assert_eq!(ret["valueForm"], form);
            let utf16 = text.encode_utf16().collect::<Vec<_>>();
            let a = ret["valueStart"].as_u64().unwrap() as usize;
            let b = ret["valueEnd"].as_u64().unwrap() as usize;
            assert_eq!(String::from_utf16(&utf16[a..b]).unwrap(), expected);
            assert_eq!(values(&r, "js_factory")[0]["safe"], true);
        }
    }
    #[test]
    fn module_candidates_and_independent_relations() {
        let r=extract("a.ts","import { Base, Service as Dependency } from './base'; export class C extends Base { constructor(private dep: Dependency) { this.dep = dep; } run() { return new Dependency(); } }");
        valid(&r);
        assert_eq!(values(&r, "js_module_import").len(), 2);
        assert_eq!(values(&r, "js_module_export").len(), 1);
        let relations = values(&r, "js_class_relation");
        assert!(relations
            .iter()
            .any(|v| v["relationship"] == "extends" && v["bindingKind"] == "import"));
        assert!(relations
            .iter()
            .any(|v| v["relationship"] == "stores_parameter"));
        assert!(relations.iter().any(|v| v["relationship"] == "creates"));
        assert!(!values(&r, "js_class_scope").is_empty());
        assert!(!values(&r, "js_class_reference").is_empty());
    }
    #[test]
    fn narrow_router_only_and_literal_privacy() {
        let r=extract("a.js","module.exports = (router, client) => { router.get('/ok', () => client.get('PRIVATE_PAYLOAD')); router.getAsync('/PRIVATE_ASYNC', () => {}); }; ");
        valid(&r);
        assert_eq!(values(&r, "api_route").len(), 1);
        assert!(!serde_json::to_string(&r.facts)
            .unwrap()
            .contains("PRIVATE_"));
        for text in ["const router = sdk(); router.get('/PRIVATE_PATH', () => {});","module.exports = (router) => { const alias = router; router.get('/PRIVATE_PATH', () => {}); };","module.exports = (router) => { router.get = other; router.get('/PRIVATE_PATH', () => {}); };"]{let r=extract("a.js",text);valid(&r);assert!(values(&r,"api_route").is_empty());assert!(!serde_json::to_string(&r.facts).unwrap().contains("PRIVATE_"));}
    }
    #[test]
    fn deterministic_identity_and_input_boundaries() {
        let a = extract("a.ts", "export class C {}\n");
        let b = extract("a.ts", "export class C {}\n");
        assert_eq!(
            serde_json::to_vec(&a.facts).unwrap(),
            serde_json::to_vec(&b.facts).unwrap()
        );
        assert_eq!(fingerprint().len(), 64);
        assert!(extract_sources(&[NativeSource {
            path: "../escape.js".into(),
            module: "escape".into(),
            bytes: vec![]
        }])
        .is_err());
        let source = NativeSource {
            path: "a.js".into(),
            module: "a".into(),
            bytes: vec![b' '; 1024 * 1024 + 1],
        };
        assert!(extract_sources(&[source]).is_err());
        let text = format!("const x = {}0{};", "[".repeat(260), "]".repeat(260));
        let r = extract("a.js", &text);
        assert!(!r.complete);
        assert_eq!(r.facts.len(), 1);
    }
    #[test]
    fn wrappers_and_loop_bindings_cannot_hide_escapes() {
        for text in [
            "const { run } = require('./work'); const escape = {module}; module.exports = {run};",
            "const { run } = require('./work'); const escape = {require}; function work() { run(); }",
            "const { run } = require('./work'); class C { field = eval('PRIVATE_CODE'); }",
        ]{let r=extract("a.js",text);valid(&r);assert_eq!(values(&r,"js_module_summary")[0]["importsSafe"],false,"{text}");assert!(values(&r,"js_module_import").is_empty());}
        let r = extract(
            "a.js",
            "function target() {} for (const target of values) { target(); }",
        );
        valid(&r);
        assert!(values(&r, "js_local_call_candidate").is_empty());
        let r=extract("a.js","module.exports = router => { const escape = {router}; router.get('/PRIVATE_ROUTE', () => {}); };");
        valid(&r);
        assert!(values(&r, "api_route").is_empty());
        let r=extract("a.js","module.exports = dep => { class C { constructor() { this.options = {dep}; } method() { return {dep}; } } return C; };");
        valid(&r);
        let rels = values(&r, "js_class_relation");
        assert!(rels
            .iter()
            .any(|v| v["relationship"] == "stores_parameter" && v["memberName"] == "options.dep"));
        assert!(rels
            .iter()
            .any(|v| v["relationship"] == "uses_factory_binding"));
    }
    #[test]
    fn optional_budgets_are_transactional_and_omissions_durable() {
        let noise = "({[key]: 0});\n".repeat(5000);
        let factories = (0..8000)
            .map(|i| format!("export function f{i}() {{}}\n"))
            .collect::<String>();
        let methods = (0..12001)
            .map(|i| format!("m{i}() {{}}\n"))
            .collect::<String>();
        for (text, code, kind) in [
            (factories, STRUCTURAL, "js_factory"),
            (
                format!("export class C {{\n{methods}}}"),
                RELATIONS,
                "js_callable_signature",
            ),
        ] {
            let r = extract_sources(&[
                NativeSource {
                    path: "noise.js".into(),
                    module: "noise".into(),
                    bytes: noise.as_bytes().to_vec(),
                },
                NativeSource {
                    path: "z.ts".into(),
                    module: "z".into(),
                    bytes: text.as_bytes().to_vec(),
                },
            ])
            .unwrap();
            assert!(r.complete, "{:?}", r.diagnostics);
            assert!(r.diagnostics.len() <= 5000);
            assert_eq!(
                r.diagnostics
                    .iter()
                    .filter(|d| d.path.is_empty() && d.code == code)
                    .count(),
                1
            );
            assert!(!r.facts.iter().any(|f| f.kind == kind));
            assert!(r
                .facts
                .iter()
                .any(|f| f.path == "z.ts" && f.kind == "js_module_export"));
        }
    }
    #[test]
    fn escaped_identifiers_share_bindings_without_changing_source_offsets() {
        let r = extract(
            "a.js",
            r"function t\u0061rget() {} function run() { target(); } run();",
        );
        valid(&r);
        assert_eq!(values(&r, "js_local_call_candidate").len(), 2);
        let r = extract(
            "a.js",
            r"function target() {} function run(t\u0061rget) { target(); }",
        );
        valid(&r);
        assert!(values(&r, "js_local_call_candidate").is_empty());
        let r = extract("a.js", r"m\u006fdule.exports = () => class C {};");
        valid(&r);
        assert_eq!(values(&r, "js_factory")[0]["safe"], true);
        let r = extract(
            "a.js",
            r"const build = () => class C {}; build = other; module.exports = build;",
        );
        valid(&r);
        assert_eq!(values(&r, "js_factory")[0]["safe"], false);
    }
    #[test]
    fn baseline_fact_and_serialized_output_limits_fail_closed() {
        let source = (0..40000)
            .map(|i| format!("function f{i}(){{}}\n"))
            .collect::<String>();
        let r = extract("a.js", &source);
        assert!(!r.complete);
        assert_eq!(r.facts.len(), 1);
        assert!(r.diagnostics.iter().any(|d| d.code == "resource_limit"));
        let source = format!(
            "class {} {{ {} }}",
            "C".repeat(30000),
            (0..400)
                .map(|i| format!("m{i}(){{}}\n"))
                .collect::<String>()
        );
        let r = extract("a.js", &source);
        assert!(!r.complete);
        assert!(r.facts.is_empty());
        assert!(r.diagnostics.iter().any(|d| d.code == "resource_limit"));
        assert!(r
            .diagnostics
            .iter()
            .any(|d| d.code == RELATIONS && d.path.is_empty()));
    }
    #[test]
    fn type_imports_and_tagged_templates_are_not_runtime_calls() {
        let r = extract(
            "a.ts",
            "type T = import('./base').Base; export class C { field: import('./other').Other; } ",
        );
        valid(&r);
        assert!(!r
            .facts
            .iter()
            .any(|f| f.kind == "call" || f.kind == "js_import"));
        let summary = &values(&r, "js_module_summary")[0];
        assert_eq!(summary["importsSafe"], true);
        assert_eq!(summary["dependencies"], json!(["./base", "./other"]));
        let r = extract(
            "a.js",
            "function helper() {} tag`PRIVATE_TEMPLATE${helper()}`;",
        );
        valid(&r);
        assert_eq!(r.facts.iter().filter(|f| f.kind == "call").count(), 1);
        assert!(!serde_json::to_string(&r.facts)
            .unwrap()
            .contains("PRIVATE_TEMPLATE"));
        let r = extract(
            "a.js",
            r"module.exports = router => { router.get('/\uD83D\uDE00', () => {}); };",
        );
        valid(&r);
        assert_eq!(values(&r, "api_route")[0]["path"], "/\u{1f600}");
    }
    #[test]
    fn signatures_preserve_declared_types_and_modifiers_not_defaults() {
        let r=extract("a.ts","class Signature { constructor(message: string = 'PRIVATE_DEFAULT') { this.message = message; } static build<T>(value: T, optional?: string, count: number = 73, ...rest: Array<T>): Promise<T> { return value; } }");
        valid(&r);
        let signatures = values(&r, "js_callable_signature");
        let build = &signatures[1];
        assert_eq!(build["typeParameters"], json!(["T"]));
        assert_eq!(build["returnType"], "Promise<T>");
        assert_eq!(
            build["parameters"],
            json!([
                {"name":"value","pattern":false,"optional":false,"rest":false,"hasDefault":false,"type":"T"},
                {"name":"optional","pattern":false,"optional":true,"rest":false,"hasDefault":false,"type":"string"},
                {"name":"count","pattern":false,"optional":false,"rest":false,"hasDefault":true,"type":"number"},
                {"name":"rest","pattern":false,"optional":false,"rest":true,"hasDefault":false,"type":"Array<T>"}
            ])
        );
        assert!(!serde_json::to_string(&r.facts)
            .unwrap()
            .contains("PRIVATE_DEFAULT"));
    }
    #[test]
    fn module_summaries_cover_leading_trivia_like_module_receipts() {
        for text in [
            "\n\nexport function run() {}\n",
            "\r\n\u{2028}import {work} from './work'; export function run() { work(); }\n",
        ] {
            let r = extract("a.ts", text);
            valid(&r);
            let module = &r.facts[0];
            let summary = r
                .facts
                .iter()
                .find(|f| f.kind == "js_module_summary")
                .unwrap();
            assert_eq!(
                (summary.line, summary.end_line),
                (module.line, module.end_line)
            );
        }
    }
    #[test]
    fn parameter_defaults_cannot_borrow_body_bindings_for_safety_analysis() {
        for path in ["a.js", "a.ts"] {
            for text in [
                "function target() {} function outer(x = (target = other)) { var target; } target();",
                "function target() {} function outer({x = (target = other)} = {}) { function target() {} } target();",
                "function target() {} function outer(x = eval('PRIVATE_CODE')) { let eval; } target();",
            ] {let r=extract(path,text);valid(&r);assert!(values(&r,"js_local_call_candidate").is_empty(),"{path}: {text}");}
            let r = extract(
                path,
                "function target() {} function outer(target, x = (target = other)) {} target();",
            );
            valid(&r);
            assert_eq!(values(&r, "js_local_call_candidate").len(), 1);
            let r=extract(path,"const {work} = require('./work'); function outer(x = require('./other')) { var require; } function run() { work(); } module.exports = {run};");
            valid(&r);
            assert_eq!(
                values(&r, "js_module_summary")[0]["dependencies"],
                json!(["./other", "./work"])
            );
            let r=extract(path,"module.exports = (dep = 'PRIVATE_DEFAULT') => { class C { constructor() { this.dep = dep; } } return C; };");
            valid(&r);
            assert_eq!(values(&r, "js_factory")[0]["safe"], true);
        }
    }
    #[test]
    fn signatures_require_named_member_receipts_and_patterns_retain_guards() {
        let r=extract("a.ts","class C { constructor() {} ['fixed'](x = 'PRIVATE_DEFAULT') {} \"quoted\"() {} normal() {} }");
        valid(&r);
        let members = values(&r, "js_member");
        let signatures = values(&r, "js_callable_signature");
        assert_eq!(signatures.len(), 2);
        for signature in signatures {
            assert!(members
                .iter()
                .any(|m| m["declarationFactId"] == signature["declarationFactId"]));
        }
        let r = extract(
            "a.js",
            "function target() {} function run({target}) { target(); }",
        );
        valid(&r);
        assert!(values(&r, "js_binding_guard")
            .iter()
            .any(|g| g["name"] == "target" && g["bindingKind"] == "parameter"));
        assert!(values(&r, "js_local_call_candidate").is_empty());
    }
    #[test]
    fn many_defaults_share_one_bounded_parameter_environment() {
        let text = format!(
            "function run({}) {{}}",
            (0..2000)
                .map(|i| format!("p{i}=0"))
                .collect::<Vec<_>>()
                .join(",")
        );
        let source = NativeSource {
            path: "a.js".into(),
            module: "a".into(),
            bytes: text.as_bytes().to_vec(),
        };
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_javascript::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(&source.bytes, None).unwrap();
        let mut file = File::new(
            &source,
            &text,
            module_fact(&source),
            Instant::now() + Duration::from_secs(60),
        );
        file.walk(tree.root_node(), 0, 0, false, false).unwrap();
        assert_eq!(file.parameter_scopes.len(), 1);
        assert_eq!(file.scopes.len(), 3);
        assert_eq!(file.bindings.len(), 2001);
    }
    #[test]
    fn symbol_amplification_stops_during_fact_materialization() {
        let text = format!(
            "class {} {{ {} }}",
            "C".repeat(250000),
            (0..2000)
                .map(|i| format!("m{i}(){{}}\n"))
                .collect::<String>()
        );
        let source = NativeSource {
            path: "a.js".into(),
            module: "a".into(),
            bytes: text.as_bytes().to_vec(),
        };
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_javascript::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(&source.bytes, None).unwrap();
        let mut file = File::new(
            &source,
            &text,
            module_fact(&source),
            Instant::now() + Duration::from_secs(60),
        );
        file.walk(tree.root_node(), 0, 0, false, false).unwrap();
        file.analyze();
        assert!(file.output_exhausted);
        assert!(file.exhausted);
        assert!(file.facts.len() < 200);
        assert!(serialized_size(&file.facts) < MAX_OUTPUT);
    }
    #[test]
    fn destructuring_default_writes_invalidate_factories_calls_and_routes() {
        for path in ["a.js", "a.ts"] {
            let r = extract(
                path,
                "module.exports=()=>{class C{} class Other{} ({C=Other}={C:Other});return C;};",
            );
            valid(&r);
            assert_eq!(values(&r, "js_factory")[0]["safe"], false);
            assert!(values(&r, "js_binding_guard")
                .iter()
                .any(|g| g["name"] == "C" && g["writes"] == 1));
            let r = extract(path, "function target(){} ({target=other}={}); target();");
            valid(&r);
            assert!(values(&r, "js_local_call_candidate").is_empty());
            let r=extract(path,"module.exports=router=>{({router={get(){}}}={});router.get('/PRIVATE_NOT_A_ROUTE',()=>{});};");
            valid(&r);
            assert!(values(&r, "api_route").is_empty());
            assert!(!serde_json::to_string(&r.facts)
                .unwrap()
                .contains("PRIVATE_NOT_A_ROUTE"));
            let r = extract(
                path,
                "module.exports=()=>{const {C}=class Other{};return C;};",
            );
            valid(&r);
            assert_eq!(values(&r, "js_factory")[0]["safe"], false);
        }
    }
    #[test]
    fn loop_strings_are_not_directives_and_declarations_do_not_change_enclosing_strictness() {
        for body in ["for(let i=0;i<1;i++)", "while(flag)", "do"] {
            let tail = if body == "do" { "while(flag);" } else { "" };
            let text=format!("function target(){{return 'outer';}} {body}{{'use strict';function target(){{'use strict';return 'block';}}}}{tail} function run(){{target();}}");
            let r = extract("a.js", &text);
            valid(&r);
            assert!(values(&r, "js_local_call_candidate").is_empty(), "{text}");
            assert!(r
                .diagnostics
                .iter()
                .any(|d| d.code == "javascript_annex_b_binding_unresolved"));
        }
        for text in [
            "'use strict'; function target(){} for(let i=0;i<1;i++){'use strict';function target(){}} function run(){target();}",
            "function run(){'use strict'; {function target(){} target();}}",
        ] {let r=extract("a.js",text);valid(&r);assert!(!r.diagnostics.iter().any(|d|d.code=="javascript_annex_b_binding_unresolved"));assert_eq!(values(&r,"js_local_call_candidate").len(),1);}
    }
    #[test]
    fn wrapped_require_dependencies_preserve_cycles_without_import_proofs() {
        let sources = [
            (
                "a.js",
                "const {b}=require('./b');function a(){b();}module.exports={a};",
            ),
            ("b.js", "(require)('./c');function b(){}module.exports={b};"),
            (
                "c.js",
                "const {a}=require('./a');function c(){a();}module.exports={c};",
            ),
        ]
        .map(|(path, text)| NativeSource {
            path: path.into(),
            module: path.into(),
            bytes: text.as_bytes().to_vec(),
        });
        let e = evidence(&sources);
        let summary = e
            .facts
            .iter()
            .find(|f| f.path == "b.js" && f.kind == "js_module_summary")
            .unwrap();
        let summary: Value = serde_json::from_str(&summary.value).unwrap();
        assert_eq!(summary["dependencies"], json!(["./c"]));
        assert!(!e
            .facts
            .iter()
            .any(|f| f.path == "b.js" && f.kind == "js_module_import"));
        let prepared = crate::js_focus::prepare_with_byte_limit(
            &e,
            &crate::focus::FocusOptions {
                symbol: "c.js.c".into(),
                hops: 3,
                max_facts: 100,
            },
            crate::packet::MAX_INPUT_BYTES,
        )
        .unwrap();
        assert!(prepared.input["focus"]["links"]
            .as_array()
            .unwrap()
            .is_empty());
        assert!(prepared.input["focus"]["unresolved"]
            .as_array()
            .unwrap()
            .iter()
            .any(|u| u["reason"] == "cyclic_module_component"));
        let r = extract(
            "a.js",
            "const {work}=(require)('./work');function run(){work();}module.exports={run};",
        );
        valid(&r);
        assert_eq!(
            values(&r, "js_module_summary")[0]["dependencies"],
            json!(["./work"])
        );
        assert!(values(&r, "js_module_import").is_empty());
        let r = extract(
            "a.js",
            "function run(require){(require)('./PRIVATE_SHADOW_PAYLOAD');}",
        );
        valid(&r);
        assert_eq!(
            values(&r, "js_module_summary")[0]["dependencies"],
            json!([])
        );
        assert!(!serde_json::to_string(&r.facts)
            .unwrap()
            .contains("PRIVATE_SHADOW_PAYLOAD"));
    }
    #[test]
    fn js_defaults_supply_no_class_relationships_but_ts_parameter_properties_remain() {
        for (path, text) in [
            (
                "a.js",
                "module.exports=dep=>class C{constructor(x=dep){} run(x=dep){} field=(x=dep)=>x;};",
            ),
            ("a.js", "module.exports=dep=>class C{run({x=dep}={}){}};"),
            ("a.ts", "module.exports=dep=>class C{run(x=dep){}};"),
        ] {
            let e = evidence(&[NativeSource {
                path: path.into(),
                module: path.into(),
                bytes: text.as_bytes().to_vec(),
            }]);
            assert!(!e.facts.iter().any(|f| f.kind == "js_class_relation"));
            let catalog = crate::catalog::build(&e, "synthetic").unwrap();
            crate::render::class_uml::build(&catalog, &e, None).unwrap();
        }
        let text="module.exports=dep=>class C{constructor(public value:Service=dep){}run(){return dep;}};";
        let r = extract("a.ts", text);
        valid(&r);
        let relations = values(&r, "js_class_relation");
        assert_eq!(
            relations
                .iter()
                .filter(|r| r["relationship"] == "stores_parameter")
                .count(),
            1
        );
        assert_eq!(
            relations
                .iter()
                .filter(|r| r["relationship"] == "uses_factory_binding")
                .count(),
            1
        );
    }
    #[test]
    fn deep_binding_patterns_fail_closed_before_recursive_analysis() {
        let pattern = format!("{}x{}", "[".repeat(20000), "]".repeat(20000));
        for text in [
            format!("const {pattern}=value;"),
            format!("function run({pattern}){{}}"),
            format!("try{{}}catch({pattern}){{}}"),
        ] {
            for path in ["a.js", "a.ts"] {
                let r = extract(path, &text);
                assert!(!r.complete);
                assert_eq!(r.facts.len(), 1);
                assert_eq!(r.facts[0].kind, "module");
                assert!(r.diagnostics.iter().any(|d| d.code == "resource_limit"));
            }
        }
    }
    #[test]
    fn class_relation_stress_is_indexed_transactional_and_cancellable() {
        let text = (0..10000)
            .map(|i| format!("class C{i} extends External{{}}\n"))
            .collect::<String>();
        let source = NativeSource {
            path: "a.js".into(),
            module: "a".into(),
            bytes: text.as_bytes().to_vec(),
        };
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_javascript::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(&source.bytes, None).unwrap();
        let mut file = File::new(
            &source,
            &text,
            module_fact(&source),
            Instant::now() + Duration::from_secs(60),
        );
        file.walk(tree.root_node(), 0, 0, false, false).unwrap();
        file.analyze();
        assert!(!file.exhausted);
        assert_eq!(file.class_bindings.len(), 10000);
        assert!(file.omissions.contains(RELATIONS));
        assert!(file.relations.is_empty());
        for bindings in file.class_bindings.values() {
            assert_eq!(bindings.len(), 2);
            assert!(bindings.iter().all(|i| file.bindings[*i].guards.len() == 1));
        }
        file.omissions.remove(RELATIONS);
        file.deadline = Instant::now() - Duration::from_millis(1);
        file.class_relations();
        assert!(file.exhausted);
        assert!(file.relations.is_empty());
    }
}
