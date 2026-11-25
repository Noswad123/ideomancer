//! Pattern-only descriptors fed by the collector's single counted CST walk.
use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Ignore,
    Name,
    Capture,
    Wildcard,
    Value,
    Singleton,
    Real,
    Imaginary,
    Text,
    Bytes,
    Pattern,
    As,
    Star,
    Rest,
    Keyword,
    KeywordAs,
}

struct Entry {
    kind: &'static str,
    class: Class,
    parent: Option<usize>,
    first: Option<usize>,
    last: Option<usize>,
    next: Option<usize>,
    start: usize,
    end: usize,
}

pub(super) struct Pattern {
    pub(super) working: usize,
    start: usize,
    brackets: usize,
    entries: Vec<Entry>,
    current: Option<usize>,
}

impl Pattern {
    pub(super) fn literal_payload(&self) -> bool {
        self.current
            .is_some_and(|i| matches!(self.entries[i].kind, "string_content" | "escape_sequence"))
    }

    // Covers the initial four-slot allocation and geometric Vec growth.
    pub(super) const ENTRY_WORK: usize = 4 * std::mem::size_of::<Entry>();

    pub(super) fn new(start: usize) -> Self {
        Self {
            working: 0,
            start,
            brackets: 0,
            entries: Vec::new(),
            current: None,
        }
    }

    pub(super) fn terminal<F: FnMut(Checkpoint) -> bool>(
        &mut self,
        node: Node<'_>,
        gap: &[u8],
        cancel: &Cancellation<F>,
    ) -> bool {
        for chunk in gap.chunks(4096) {
            if cancel.at(Checkpoint::Collector)
                || chunk.contains(&0)
                || (!self.literal_payload()
                    && self.brackets == 0
                    && chunk.iter().any(|b| matches!(b, b'\n' | b'\r')))
            {
                return false;
            }
        }
        if !node.is_named() {
            match node.kind() {
                "(" | "[" | "{" => self.brackets += 1,
                ")" | "]" | "}" if self.brackets > 0 => self.brackets -= 1,
                ")" | "]" | "}" => return false,
                _ => {}
            }
        }
        true
    }

    pub(super) fn enter<F: FnMut(Checkpoint) -> bool>(
        &mut self,
        node: Node<'_>,
        _field: Option<&str>,
        bytes: &[u8],
        cancel: &Cancellation<F>,
    ) -> bool {
        if cancel.at(Checkpoint::Collector) {
            return false;
        }
        let kind = node.kind();
        let text = &bytes[node.start_byte()..node.end_byte()];
        if node.child_count() == 0 {
            for chunk in text.chunks(4096) {
                if cancel.at(Checkpoint::Collector) || chunk.contains(&0) {
                    return false;
                }
            }
        }
        let class = match kind {
            "case_clause" if self.current.is_none() && node.start_byte() == self.start => {
                Class::Ignore
            }
            "case_pattern"
            | "dotted_name"
            | "class_pattern"
            | "keyword_pattern"
            | "splat_pattern"
            | "union_pattern"
            | "list_pattern"
            | "tuple_pattern"
            | "dict_pattern"
            | "as_pattern"
            | "complex_pattern"
            | "concatenated_string" => Class::Ignore,
            "identifier" => {
                if !identifier(std::str::from_utf8(text).unwrap_or(""), cancel) {
                    return false;
                }
                Class::Name
            }
            "integer" | "float" => {
                if !expressions::number(text, cancel) {
                    return false;
                }
                if matches!(text.last(), Some(b'j' | b'J')) {
                    Class::Imaginary
                } else {
                    Class::Real
                }
            }
            "true" | "false" | "none" => Class::Singleton,
            "string" => match expressions::string(text, cancel) {
                Some(false) => Class::Text,
                Some(true) => Class::Bytes,
                None => return false,
            },
            "string_start" | "string_content" | "string_end" | "escape_sequence" | "\\"
                if self.current.is_some_and(|p| {
                    matches!(self.entries[p].kind, "string" | "string_content")
                }) =>
            {
                Class::Ignore
            }
            "comment" if !type_comment(text, cancel) => Class::Ignore,
            "line_continuation" if matches!(text, b"\\\n" | b"\\\r\n") => Class::Ignore,
            "_" if !node.is_named() => Class::Wildcard,
            "case" | "as" | "(" | ")" | "[" | "]" | "{" | "}" | "," | "." | ":" | "=" | "*"
            | "**" | "|" | "+" | "-"
                if !node.is_named() =>
            {
                Class::Ignore
            }
            _ => return false,
        };
        let index = self.entries.len();
        self.entries.push(Entry {
            kind,
            class,
            parent: self.current,
            first: None,
            last: None,
            next: None,
            start: node.start_byte(),
            end: node.end_byte(),
        });
        if let Some(parent) = self.current {
            if let Some(last) = self.entries[parent].last {
                self.entries[last].next = Some(index);
            } else {
                self.entries[parent].first = Some(index);
            }
            self.entries[parent].last = Some(index);
        }
        self.current = Some(index);
        self.working += Self::ENTRY_WORK;
        true
    }

    fn next<F: FnMut(Checkpoint) -> bool>(
        &self,
        at: &mut Option<usize>,
        cancel: &Cancellation<F>,
    ) -> Result<Option<usize>, ()> {
        while let Some(i) = *at {
            if cancel.at(Checkpoint::Collector) {
                return Err(());
            }
            *at = self.entries[i].next;
            if !matches!(self.entries[i].kind, "comment" | "line_continuation") {
                return Ok(Some(i));
            }
        }
        Ok(None)
    }

    fn take<F: FnMut(Checkpoint) -> bool>(
        &self,
        at: &mut Option<usize>,
        kind: &str,
        cancel: &Cancellation<F>,
    ) -> Result<(), ()> {
        let i = self.next(at, cancel)?.ok_or(())?;
        if self.entries[i].kind != kind {
            return Err(());
        }
        Ok(())
    }

    fn item<F: FnMut(Checkpoint) -> bool>(
        &self,
        i: usize,
        at: &mut Option<usize>,
        cancel: &Cancellation<F>,
    ) -> Result<Class, ()> {
        let e = &self.entries[i];
        if e.kind == "-" {
            let n = self.next(at, cancel)?.ok_or(())?;
            if !matches!(self.entries[n].kind, "integer" | "float") {
                return Err(());
            }
            return Ok(self.entries[n].class);
        }
        Ok(e.class)
    }

    fn closed(class: Class) -> bool {
        matches!(
            class,
            Class::Capture
                | Class::Wildcard
                | Class::Value
                | Class::Singleton
                | Class::Real
                | Class::Imaginary
                | Class::Text
                | Class::Bytes
                | Class::Pattern
                | Class::As
        )
    }

    fn validate<F: FnMut(Checkpoint) -> bool>(
        &self,
        index: usize,
        bytes: &[u8],
        cancel: &Cancellation<F>,
    ) -> Result<Class, ()> {
        if cancel.at(Checkpoint::Collector) {
            return Err(());
        }
        let e = &self.entries[index];
        let mut at = e.first;
        let class = match e.kind {
            "case_pattern" => {
                let i = self.next(&mut at, cancel)?.ok_or(())?;
                let class = self.item(i, &mut at, cancel)?;
                if !Self::closed(class)
                    && !matches!(class, Class::Star | Class::Keyword | Class::KeywordAs)
                {
                    return Err(());
                }
                class
            }
            "dotted_name" => {
                let first = self.next(&mut at, cancel)?.ok_or(())?;
                if self.entries[first].class != Class::Name {
                    return Err(());
                }
                let mut count = 1;
                while let Some(i) = self.next(&mut at, cancel)? {
                    if self.entries[i].kind != "." {
                        return Err(());
                    }
                    let n = self.next(&mut at, cancel)?.ok_or(())?;
                    if self.entries[n].class != Class::Name {
                        return Err(());
                    }
                    count += 1;
                }
                if count > 1 {
                    Class::Value
                } else if &bytes[self.entries[first].start..self.entries[first].end] == b"_" {
                    Class::Wildcard
                } else {
                    Class::Capture
                }
            }
            "splat_pattern" => {
                let op = self.next(&mut at, cancel)?.ok_or(())?;
                let n = self.next(&mut at, cancel)?.ok_or(())?;
                let name = &self.entries[n];
                if !matches!(name.class, Class::Name | Class::Wildcard) {
                    return Err(());
                }
                match self.entries[op].kind {
                    "*" => Class::Star,
                    "**" if &bytes[name.start..name.end] != b"_" => Class::Rest,
                    _ => return Err(()),
                }
            }
            "as_pattern" => {
                let i = self.next(&mut at, cancel)?.ok_or(())?;
                let left = self.entries[i].class;
                if left == Class::As || (!Self::closed(left) && left != Class::Keyword) {
                    return Err(());
                }
                self.take(&mut at, "as", cancel)?;
                let n = self.next(&mut at, cancel)?.ok_or(())?;
                let name = &self.entries[n];
                if name.class != Class::Name || &bytes[name.start..name.end] == b"_" {
                    return Err(());
                }
                // The CST attaches `as name` outside a keyword's case_pattern;
                // in the AST it belongs to that keyword's value pattern.
                if left == Class::Keyword {
                    Class::KeywordAs
                } else {
                    Class::As
                }
            }
            "keyword_pattern" => {
                let mut parent = e.parent;
                loop {
                    if cancel.at(Checkpoint::Collector) {
                        return Err(());
                    }
                    let p = parent.ok_or(())?;
                    match self.entries[p].kind {
                        "class_pattern" => break,
                        "case_pattern" | "as_pattern" => parent = self.entries[p].parent,
                        _ => return Err(()),
                    }
                }
                let name = self.next(&mut at, cancel)?.ok_or(())?;
                if self.entries[name].class != Class::Name {
                    return Err(());
                }
                self.take(&mut at, "=", cancel)?;
                let i = self.next(&mut at, cancel)?.ok_or(())?;
                if !Self::closed(self.item(i, &mut at, cancel)?) {
                    return Err(());
                }
                Class::Keyword
            }
            "complex_pattern" => {
                let mut left = self.next(&mut at, cancel)?.ok_or(())?;
                if self.entries[left].kind == "-" {
                    left = self.next(&mut at, cancel)?.ok_or(())?;
                }
                if self.entries[left].class != Class::Real {
                    return Err(());
                }
                let op = self.next(&mut at, cancel)?.ok_or(())?;
                if !matches!(self.entries[op].kind, "+" | "-") {
                    return Err(());
                }
                let right = self.next(&mut at, cancel)?.ok_or(())?;
                if self.entries[right].class != Class::Imaginary {
                    return Err(());
                }
                Class::Value
            }
            "union_pattern" => {
                let mut count = 0;
                loop {
                    let i = self.next(&mut at, cancel)?.ok_or(())?;
                    if !Self::closed(self.item(i, &mut at, cancel)?) {
                        return Err(());
                    }
                    count += 1;
                    let Some(separator) = self.next(&mut at, cancel)? else {
                        break;
                    };
                    if self.entries[separator].kind != "|" {
                        return Err(());
                    }
                }
                if count < 2 {
                    return Err(());
                }
                Class::Pattern
            }
            "concatenated_string" => {
                let i = self.next(&mut at, cancel)?.ok_or(())?;
                let category = self.entries[i].class;
                if !matches!(category, Class::Text | Class::Bytes) {
                    return Err(());
                }
                let mut count = 1;
                while let Some(i) = self.next(&mut at, cancel)? {
                    if self.entries[i].class != category {
                        return Err(());
                    }
                    count += 1;
                }
                if count < 2 {
                    return Err(());
                }
                category
            }
            "case_clause" | "list_pattern" | "tuple_pattern" | "class_pattern" | "dict_pattern" => {
                let (open, close) = match e.kind {
                    "case_clause" => ("case", None),
                    "list_pattern" => ("[", Some("]")),
                    "dict_pattern" => ("{", Some("}")),
                    "class_pattern" => {
                        let name = self.next(&mut at, cancel)?.ok_or(())?;
                        if self.entries[name].kind != "dotted_name" {
                            return Err(());
                        }
                        ("(", Some(")"))
                    }
                    _ => ("(", Some(")")),
                };
                self.take(&mut at, open, cancel)?;
                let mut count = 0;
                let mut comma = false;
                let mut star = false;
                let mut keyword = false;
                let mut rest = false;
                let mut want_item = true;
                let mut only = Class::Ignore;
                loop {
                    let Some(i) = self.next(&mut at, cancel)? else {
                        if close.is_some() {
                            return Err(());
                        }
                        break;
                    };
                    let child = &self.entries[i];
                    if close == Some(child.kind) {
                        break;
                    }
                    if child.kind == "," && !want_item {
                        comma = true;
                        want_item = true;
                        continue;
                    }
                    if !want_item || rest {
                        return Err(());
                    }
                    let class = self.item(i, &mut at, cancel)?;
                    match e.kind {
                        "dict_pattern" if class == Class::Rest => rest = true,
                        "dict_pattern" => {
                            if !matches!(
                                child.kind,
                                "dotted_name"
                                    | "true"
                                    | "false"
                                    | "none"
                                    | "integer"
                                    | "float"
                                    | "-"
                                    | "complex_pattern"
                                    | "string"
                                    | "concatenated_string"
                            ) || !matches!(
                                class,
                                Class::Value
                                    | Class::Singleton
                                    | Class::Real
                                    | Class::Imaginary
                                    | Class::Text
                                    | Class::Bytes
                            ) {
                                return Err(());
                            }
                            self.take(&mut at, ":", cancel)?;
                            let value = self.next(&mut at, cancel)?.ok_or(())?;
                            if !Self::closed(self.entries[value].class) {
                                return Err(());
                            }
                        }
                        "class_pattern" if matches!(class, Class::Keyword | Class::KeywordAs) => {
                            keyword = true;
                        }
                        "class_pattern" if keyword || !Self::closed(class) => return Err(()),
                        "case_clause" | "list_pattern" | "tuple_pattern"
                            if class == Class::Star =>
                        {
                            star = true;
                        }
                        _ if !Self::closed(class) => return Err(()),
                        _ => {}
                    }
                    only = class;
                    count += 1;
                    want_item = false;
                }
                if (count == 0 && (comma || e.kind == "case_clause"))
                    || (e.kind == "case_clause" && star && !comma)
                    || (e.kind == "tuple_pattern" && count == 1 && !comma && star)
                {
                    return Err(());
                }
                if e.kind == "tuple_pattern" && count == 1 && !comma {
                    if only == Class::As {
                        Class::Pattern
                    } else {
                        only
                    }
                } else {
                    Class::Pattern
                }
            }
            _ => return Ok(e.class),
        };
        if self.next(&mut at, cancel)?.is_some() {
            return Err(());
        }
        Ok(class)
    }

    pub(super) fn exit<F: FnMut(Checkpoint) -> bool>(
        &mut self,
        bytes: &[u8],
        cancel: &Cancellation<F>,
    ) -> Result<bool, ()> {
        let i = self.current.ok_or(())?;
        let class = self.validate(i, bytes, cancel)?;
        self.entries[i].class = class;
        self.current = self.entries[i].parent;
        Ok(self.current.is_none())
    }

    pub(super) fn finish<F: FnMut(Checkpoint) -> bool>(
        &mut self,
        bytes: &[u8],
        cancel: &Cancellation<F>,
    ) -> bool {
        self.current == Some(0)
            && self.entries[0].kind == "case_clause"
            && self.brackets == 0
            && self.validate(0, bytes, cancel).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;

    fn source(path: &str, text: impl AsRef<[u8]>) -> Source {
        Source {
            path: path.into(),
            module: path.trim_end_matches(".py").into(),
            bytes: text.as_ref().to_vec(),
            hex: "invalid transport".into(),
        }
    }

    fn accepted(pattern: &str) {
        let sources = [source(
            "patterns.py",
            format!("match subject:\n    case {pattern}:\n        pass\n"),
        )];
        let report = extract_fact_candidates(&sources).unwrap();
        assert_eq!(
            report.files[0].status,
            CandidateStatus::AcceptedFacts,
            "{pattern}"
        );
        assert!(report.facts.iter().all(|f| f.kind == "module"), "{pattern}");
        assert!(report.diagnostics.is_empty());
    }

    fn rejected(pattern: &str) {
        let sources = [source(
            "patterns.py",
            format!("match subject:\n    case {pattern}:\n        hidden()\n"),
        )];
        let report = extract_fact_candidates(&sources).unwrap();
        assert_ne!(
            report.files[0].status,
            CandidateStatus::AcceptedFacts,
            "{pattern}"
        );
        assert!(report.facts.iter().all(|f| f.kind == "module"));
    }

    #[test]
    fn all_eight_families_have_no_pattern_facts() {
        for pattern in [
            "constants.VALUE",
            "None",
            "True",
            "False",
            "[head, *tail]",
            "{'key': value, **rest}",
            "pkg.Record(item, flag=True)",
            "*tail,",
            "(1 | 2) as chosen",
            "_",
            "capture",
            "()",
            "[]",
            "{}",
            "[Record(value=(1 | 2) as selected), {'key': [*rest]}]",
            "(value as first) as second",
            "Record(key=value as selected)",
            "{constants.KEY: value, True: other}",
        ] {
            accepted(pattern);
        }
    }

    #[test]
    fn numeric_and_string_literal_policy() {
        for pattern in [
            "-1",
            "-2.5",
            "3j",
            "-3j",
            "1+2j",
            "-1-2j",
            "'text'",
            "b'bytes'",
            "'one' 'two'",
            "b'one' b'two'",
            "{'a' 'b': -1+2j}",
            "'escaped\\n'",
            "r'raw\\n'",
            "b'\\x41'",
            "'\\uD800'",
            "'\\q\\777'",
            "0x_FF",
            "0_0",
            "1e9999",
            "1e-9999j",
        ] {
            accepted(pattern);
        }
        for pattern in [
            "1+2",
            "1j+2j",
            "+1",
            "--1",
            "1+-2j",
            "f'{value}'",
            "f''",
            "rf''",
            "'a' b'b'",
            "'\\x0'",
            "'\\u000'",
            "'\\U00110000'",
            "01",
            "0x1j",
        ] {
            rejected(pattern);
        }
        accepted(&"7".repeat(513));
        accepted(&"7".repeat(4300));
        accepted(&"0".repeat(4301));
        accepted(&format!("{}.0", "7".repeat(4301)));
        accepted(&format!("{}j", "7".repeat(4301)));
        accepted(&format!("0x{}", "f".repeat(4301)));
        rejected(&"7".repeat(4301));
    }

    #[test]
    fn compiler_only_capture_constraints_are_not_enforced() {
        for pattern in [
            "[same, same]",
            "left | right",
            "_ | 1",
            "Record(key=a, key=b)",
            "[*left, *right]",
            "value as value",
            "{1: a, 1: b}",
        ] {
            accepted(pattern);
        }
    }

    #[test]
    fn grammar_roles_and_empty_comma_positions_fail_closed() {
        for pattern in [
            "",
            ",",
            "(,)",
            "[,]",
            "{,}",
            "Record(,)",
            "key=value",
            "[key=value]",
            "Record(key=value, positional)",
            "Record(*args)",
            "[*args as name]",
            "**rest",
            "*rest",
            "(*rest)",
            "[**rest]",
            "{*rest}",
            "{**_}",
            "{**first, **second}",
            "{**rest, 'key': value}",
            "{capture: value}",
            "{[1]: value}",
            "{Record(): value}",
            "{(1): value}",
            "value as _",
            "[await]",
            "value as first as second",
            "Record(key=value as first as second)",
        ] {
            rejected(pattern);
        }
        for pattern in [
            "(*rest,)",
            "*rest, other",
            "(capture)",
            "Record(match=case)",
        ] {
            accepted(pattern);
        }
    }

    #[test]
    fn physical_continuation_and_comments() {
        accepted("[\n        first, # ordinary comment\n        second\n    ]");
        accepted("1 | \\\n        2");
        accepted("1 | \\\r\n        2");
        rejected("1 |\n        2");
        rejected("[first, # type: int\n        second]");
        rejected("['nul\0payload']");
    }

    #[test]
    fn pattern_names_do_not_write_aliases_or_create_calls() {
        let sources = [source("aliases.py", "import importlib as loader\nmatch subject():\n    case loader | Record(loader) if guard():\n        loader.import_module('inside')\nloader.import_module('after')\n")];
        let report = extract_fact_candidates(&sources).unwrap();
        assert_eq!(report.files[0].status, CandidateStatus::AcceptedFacts);
        assert_eq!(report.facts.iter().filter(|f| f.kind == "call").count(), 4);
        assert_eq!(report.diagnostics.len(), 2);
        assert!(!report.facts.iter().any(|f| f.kind == "binding"));
    }

    #[test]
    fn nested_resources_cancellation_and_per_file_rollback() {
        let pattern = format!("{}capture{}", "[".repeat(24), "]".repeat(24));
        accepted(&pattern);
        let sources = [
            source("good.py", "visible()\n"),
            source(
                "bad.py",
                format!("match subject:\n    case {pattern}:\n        hidden()\n"),
            ),
        ];
        let report = extract(
            &sources,
            Limits::default(),
            FactLimits {
                working: 64 * super::Pattern::ENTRY_WORK,
                ..FactLimits::default()
            },
            |_| false,
        )
        .unwrap();
        assert_eq!(report.files[0].status, CandidateStatus::AcceptedFacts);
        assert_eq!(report.files[1].status, CandidateStatus::ResourceLimit);
        assert!(!report
            .facts
            .iter()
            .any(|f| f.path == "bad.py" && f.kind != "module"));
        let mut checks = 0;
        let report = extract(
            &sources,
            Limits::default(),
            FactLimits::default(),
            |phase| {
                checks += usize::from(phase == Checkpoint::Collector);
                checks > 40
            },
        )
        .unwrap();
        assert!(report
            .files
            .iter()
            .any(|f| f.status == CandidateStatus::ResourceLimit));
        assert!(report.facts.iter().all(|f| f.kind == "module"));
    }
}
