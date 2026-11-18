//! Borrowed-span validation and AST-shaped descriptors from the counted walk.
use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    Annotation,
    Default,
    Return,
    BodyValue,
    AssignmentTarget,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Ignore,
    Reference,
    Annotation,
    Tuple,
    Text,
    Bytes,
    Number,
    Constant,
    Unary,
    Call,
    Arguments,
    Keyword,
}

#[derive(Clone, Copy)]
enum Shape {
    Ignore,
    Name,
    Literal,
    Unary,
    Alias(usize),
    Attribute(usize, usize),
    Subscript(usize, usize, bool),
    Tuple(Option<usize>),
    Call(usize, usize),
    Arguments(Option<usize>),
    Keyword(usize, usize),
}

struct Entry {
    kind: &'static str,
    parent: Option<usize>,
    first: Option<usize>,
    last: Option<usize>,
    next: Option<usize>,
    next_value: Option<usize>,
    last_value: Option<usize>,
    class: Class,
    shape: Shape,
    start: usize,
    end: usize,
    line: usize,
    end_line: usize,
}

pub(super) struct Expression {
    pub(super) mode: Mode,
    pub(super) working: usize,
    pub(super) terminal_end_line: usize,
    pub(super) start: usize,
    base_depth: usize,
    bracket_depth: usize,
    entries: Vec<Entry>,
    current: Option<usize>,
}

impl Expression {
    // Four entries per logical node also covers Vec's initial four-slot
    // allocation, not just subsequent geometric capacity growth.
    pub(super) const ENTRY_WORK: usize = 4 * std::mem::size_of::<Entry>();
    // At most fourteen simultaneous 512-byte renderer results, plus the result
    // handed to the collector. No literal body or unbounded shape is allocated.
    pub(super) const RENDER_WORK: usize = 16 * 512;

    pub(super) fn new(mode: Mode, start: usize) -> Self {
        let base_depth = usize::from(matches!(mode, Mode::Annotation | Mode::Default));
        Self {
            mode,
            working: 0,
            terminal_end_line: 0,
            start,
            base_depth,
            bracket_depth: base_depth,
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
                || (self.bracket_depth == 0 && chunk.iter().any(|b| matches!(b, b'\n' | b'\r')))
            {
                return false;
            }
        }
        // Only grammar punctuation changes implicit continuation. Brackets and
        // newlines inside literal payloads or comments are not syntax tokens.
        if !node.is_named() {
            match node.kind() {
                "(" | "[" => self.bracket_depth += 1,
                ")" | "]" if self.bracket_depth > self.base_depth => self.bracket_depth -= 1,
                ")" | "]" => return false,
                _ => {}
            }
        }
        if node.kind() != "comment" {
            self.terminal_end_line = node.end_position().row + 1;
        }
        true
    }

    pub(super) fn enter<F: FnMut(Checkpoint) -> bool>(
        &mut self,
        node: Node<'_>,
        bytes: &[u8],
        cancel: &Cancellation<F>,
    ) -> bool {
        let kind = node.kind();
        let text = &bytes[node.start_byte()..node.end_byte()];
        let in_string = self
            .current
            .is_some_and(|i| matches!(self.entries[i].kind, "string" | "string_content"));
        let (class, shape) = match kind {
            "identifier" => {
                if !identifier(std::str::from_utf8(text).unwrap_or(""), cancel) {
                    return false;
                }
                (Class::Reference, Shape::Name)
            }
            "none" | "ellipsis" => (Class::Annotation, Shape::Literal),
            "true" | "false" => (Class::Constant, Shape::Literal),
            "integer" | "float" => {
                if !number(text, cancel) {
                    return false;
                }
                (Class::Number, Shape::Literal)
            }
            "string" => match string(text, cancel) {
                Some(false) => (Class::Text, Shape::Literal),
                Some(true) => (Class::Bytes, Shape::Literal),
                None => return false,
            },
            "string_start" | "string_content" | "string_end" if in_string => {
                (Class::Ignore, Shape::Ignore)
            }
            "type"
            | "parenthesized_expression"
            | "attribute"
            | "member_type"
            | "generic_type"
            | "type_parameter"
            | "subscript"
            | "tuple"
            | "concatenated_string"
            | "unary_operator" => (Class::Ignore, Shape::Ignore),
            "call" | "argument_list" | "keyword_argument" if self.mode == Mode::BodyValue => {
                (Class::Ignore, Shape::Ignore)
            }
            "=" if self.mode == Mode::BodyValue && !node.is_named() => {
                (Class::Ignore, Shape::Ignore)
            }
            "comment" if !type_comment(text, cancel) => (Class::Ignore, Shape::Ignore),
            "(" | ")" | "[" | "]" | "," | "." | "+" | "-" if !node.is_named() => {
                (Class::Ignore, Shape::Ignore)
            }
            _ => return false,
        };
        let index = self.entries.len();
        self.entries.push(Entry {
            kind,
            parent: self.current,
            first: None,
            last: None,
            next: None,
            next_value: None,
            last_value: None,
            class,
            shape,
            start: node.start_byte(),
            end: node.end_byte(),
            line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
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

    pub(super) fn exit<F: FnMut(Checkpoint) -> bool>(
        &mut self,
        bytes: &[u8],
        cancel: &Cancellation<F>,
    ) -> Result<bool, ()> {
        let index = self.current.ok_or(())?;
        let entry = &self.entries[index];
        let kind = entry.kind;
        let mut child = entry.first;
        let mut first = None;
        let mut second = None;
        let mut count = 0;
        let mut comma = false;
        while let Some(i) = child {
            if cancel.at(Checkpoint::Collector) {
                return Err(());
            }
            let item = &self.entries[i];
            comma |= item.kind == ",";
            if item.class != Class::Ignore {
                count += 1;
                if first.is_none() {
                    first = Some(i);
                } else if second.is_none() {
                    second = Some(i);
                }
            }
            child = item.next;
        }
        let annotation =
            |class| matches!(class, Class::Reference | Class::Annotation | Class::Text);
        let normalized = match kind {
            "keyword_argument" if count == 2 => {
                let (key, value) = (first.unwrap(), second.unwrap());
                if !matches!(self.entries[key].shape, Shape::Name) || !self.body_value(value) {
                    return Err(());
                }
                Some((Class::Keyword, Shape::Keyword(key, value)))
            }
            "argument_list" => {
                // Borrowed names and the set's temporary buckets are covered by
                // ENTRY_WORK; no keyword text or subtrees are copied.
                let mut keywords = HashSet::new();
                let mut child = first;
                let mut keyword_seen = false;
                while let Some(i) = child {
                    if cancel.at(Checkpoint::Collector) {
                        return Err(());
                    }
                    let item = &self.entries[i];
                    if let Shape::Keyword(key, _) = item.shape {
                        keyword_seen = true;
                        let key = &self.entries[key];
                        if !keywords.insert(&bytes[key.start..key.end]) {
                            return Err(());
                        }
                    } else if keyword_seen || !self.body_value(i) {
                        return Err(());
                    }
                    child = item.next_value;
                }
                Some((Class::Arguments, Shape::Arguments(first)))
            }
            "call" if count == 2 => {
                let (callee, args) = (first.unwrap(), second.unwrap());
                if self.entries[callee].class != Class::Reference
                    || self.entries[args].class != Class::Arguments
                {
                    return Err(());
                }
                let callee = self.resolve(callee);
                let last = match self.entries[callee].shape {
                    Shape::Attribute(_, last) => last,
                    Shape::Name => callee,
                    _ => return Err(()),
                };
                let name = &self.entries[last];
                let name = &bytes[name.start..name.end];
                if constructor_terminal(name)
                    || (name == b"include_router"
                        && matches!(self.entries[callee].shape, Shape::Attribute(..)))
                {
                    return Err(());
                }
                // The extractor checks a separate expression(func), not the
                // callee shape nested within expression(call). Clipping can
                // create a special terminal name absent from the raw spelling.
                // Collector reserves RENDER_WORK before this exit callback.
                let standalone = self.render_node(callee, 0, &mut 80, bytes, cancel);
                if constructor_terminal(standalone.rsplit('.').next().unwrap_or("").as_bytes()) {
                    return Err(());
                }
                // Keep the original callee wrapper for the Call's start position.
                let callee = first.unwrap();
                Some((Class::Call, Shape::Call(callee, args)))
            }
            "type" | "parenthesized_expression" if count == 1 => {
                let i = first.unwrap();
                Some((self.entries[i].class, Shape::Alias(self.resolve(i))))
            }
            "attribute" | "member_type" if count == 2 => {
                let (a, b) = (first.unwrap(), second.unwrap());
                if self.entries[a].class != Class::Reference
                    || !matches!(self.entries[b].shape, Shape::Name)
                {
                    return Err(());
                }
                Some((Class::Reference, Shape::Attribute(a, b)))
            }
            "tuple" | "type_parameter" => {
                let mut child = first;
                while let Some(i) = child {
                    if cancel.at(Checkpoint::Collector) {
                        return Err(());
                    }
                    let e = &self.entries[i];
                    if e.class != Class::Ignore
                        && !annotation(e.class)
                        && !(kind == "type_parameter"
                            && count == 1
                            && !comma
                            && e.class == Class::Tuple)
                    {
                        return Err(());
                    }
                    child = e.next;
                }
                if count == 0 && kind == "type_parameter" {
                    return Err(());
                }
                if kind == "type_parameter" && count == 1 && !comma {
                    let i = first.unwrap();
                    Some((self.entries[i].class, Shape::Alias(self.resolve(i))))
                } else {
                    Some((Class::Tuple, Shape::Tuple(first)))
                }
            }
            "generic_type" | "subscript" if count >= 2 => {
                let (base, slice) = (first.unwrap(), second.unwrap());
                if self.entries[base].class != Class::Reference {
                    return Err(());
                }
                let mut child = Some(slice);
                while let Some(i) = child {
                    if cancel.at(Checkpoint::Collector) {
                        return Err(());
                    }
                    let e = &self.entries[i];
                    if e.class != Class::Ignore
                        && !annotation(e.class)
                        && !(e.class == Class::Tuple && count == 2 && !comma)
                    {
                        return Err(());
                    }
                    child = e.next;
                }
                if kind == "generic_type" && count != 2 {
                    return Err(());
                }
                Some((
                    Class::Annotation,
                    Shape::Subscript(base, slice, kind == "subscript" && (count > 2 || comma)),
                ))
            }
            "concatenated_string" if count >= 2 => {
                let category = self.entries[first.unwrap()].class;
                if !matches!(category, Class::Text | Class::Bytes) {
                    return Err(());
                }
                let mut child = first;
                while let Some(i) = child {
                    if cancel.at(Checkpoint::Collector) {
                        return Err(());
                    }
                    let e = &self.entries[i];
                    if e.class != Class::Ignore && e.class != category {
                        return Err(());
                    }
                    child = e.next;
                }
                Some((category, Shape::Literal))
            }
            "unary_operator" if count == 1 => {
                if self.entries[first.unwrap()].class != Class::Number {
                    return Err(());
                }
                Some((Class::Unary, Shape::Unary))
            }
            "type"
            | "parenthesized_expression"
            | "attribute"
            | "member_type"
            | "generic_type"
            | "subscript"
            | "concatenated_string"
            | "unary_operator" => {
                return Err(());
            }
            _ => None,
        };
        if let Some((class, shape)) = normalized {
            self.entries[index].class = class;
            self.entries[index].shape = shape;
        }
        self.entries[index].end_line = self.terminal_end_line;
        self.current = self.entries[index].parent;
        if self.entries[index].class != Class::Ignore {
            if let Some(parent) = self.current {
                if let Some(last) = self.entries[parent].last_value {
                    self.entries[last].next_value = Some(index);
                }
                self.entries[parent].last_value = Some(index);
            }
        }
        if self.current.is_none() {
            if self.bracket_depth != self.base_depth {
                return Err(());
            }
            let class = self.entries[index].class;
            let valid = if self.mode == Mode::BodyValue {
                self.body_value(index)
            } else if self.mode == Mode::AssignmentTarget {
                self.valid_target(index, bytes, cancel)
            } else if self.mode == Mode::Default {
                matches!(
                    self.entries[self.resolve(index)].shape,
                    Shape::Literal | Shape::Unary
                )
            } else {
                annotation(class)
            };
            if !valid {
                return Err(());
            }
            return Ok(true);
        }
        Ok(false)
    }

    fn resolve(&self, mut i: usize) -> usize {
        while let Shape::Alias(next) = self.entries[i].shape {
            i = next;
        }
        i
    }

    fn body_value(&self, index: usize) -> bool {
        matches!(
            self.entries[self.resolve(index)].shape,
            Shape::Name | Shape::Attribute(..) | Shape::Call(..) | Shape::Literal | Shape::Unary
        )
    }

    fn valid_target<F: FnMut(Checkpoint) -> bool>(
        &self,
        index: usize,
        bytes: &[u8],
        cancel: &Cancellation<F>,
    ) -> bool {
        if matches!(self.entries[index].shape, Shape::Name) {
            return true;
        }
        let mut current = index;
        let mut outer = true;
        loop {
            if cancel.at(Checkpoint::Collector) {
                return false;
            }
            match self.entries[current].shape {
                Shape::Attribute(base, attr) => {
                    let attr = &self.entries[attr];
                    let name = &bytes[attr.start..attr.end];
                    if name == b"router" || (outer && name == b"prefix") {
                        return false;
                    }
                    current = base;
                    outer = false;
                }
                Shape::Name => {
                    let root = &self.entries[current];
                    return matches!(&bytes[root.start..root.end], b"self" | b"cls");
                }
                _ => return false,
            }
        }
    }

    pub(super) fn binding(&self) -> bool {
        matches!(
            self.entries[self.resolve(0)].shape,
            Shape::Name | Shape::Attribute(..) | Shape::Call(..)
        )
    }

    pub(super) fn calls<'a, F: FnMut(Checkpoint) -> bool>(
        &'a self,
        cancel: &'a Cancellation<F>,
    ) -> impl Iterator<Item = usize> + 'a {
        self.entries
            .iter()
            .enumerate()
            .take_while(|_| !cancel.at(Checkpoint::Collector))
            .filter_map(|(i, e)| matches!(e.shape, Shape::Call(..)).then_some(i))
    }

    pub(super) fn call_span(&self, index: usize) -> (usize, usize) {
        let entry = &self.entries[index];
        (entry.line, entry.end_line)
    }

    pub(super) fn render_call<F: FnMut(Checkpoint) -> bool>(
        &self,
        index: usize,
        bytes: &[u8],
        cancel: &Cancellation<F>,
    ) -> String {
        self.render_node(index, 0, &mut 80, bytes, cancel)
            .into_boxed_str()
            .into_string()
    }

    pub(super) fn render_callee<F: FnMut(Checkpoint) -> bool>(
        &self,
        index: usize,
        bytes: &[u8],
        cancel: &Cancellation<F>,
    ) -> String {
        let Shape::Call(callee, _) = self.entries[index].shape else {
            unreachable!("validated call")
        };
        self.render_node(callee, 0, &mut 80, bytes, cancel)
    }

    pub(super) fn span(&self) -> (usize, usize) {
        let root = &self.entries[self.resolve(0)];
        (root.line, root.end_line)
    }

    pub(super) fn render<F: FnMut(Checkpoint) -> bool>(
        &self,
        bytes: &[u8],
        cancel: &Cancellation<F>,
    ) -> String {
        self.render_node(0, 0, &mut 80, bytes, cancel)
    }

    fn render_node<F: FnMut(Checkpoint) -> bool>(
        &self,
        index: usize,
        depth: usize,
        budget: &mut i32,
        bytes: &[u8],
        cancel: &Cancellation<F>,
    ) -> String {
        *budget -= 1;
        if *budget < 0 || depth > 12 || cancel.at(Checkpoint::Collector) {
            return "...".into();
        }
        let entry = &self.entries[self.resolve(index)];
        let mut result = String::with_capacity(512);
        match entry.shape {
            Shape::Name => append(
                &mut result,
                std::str::from_utf8(&bytes[entry.start..entry.end]).unwrap(),
            ),
            Shape::Literal => append(&mut result, "<literal>"),
            Shape::Unary => append(&mut result, "<UnaryOp>"),
            Shape::Attribute(a, b) => {
                append(
                    &mut result,
                    &self.render_node(a, depth + 1, budget, bytes, cancel),
                );
                append(&mut result, ".");
                let attr = &self.entries[b];
                append(
                    &mut result,
                    std::str::from_utf8(&bytes[attr.start..attr.end]).unwrap(),
                );
            }
            Shape::Subscript(base, slice, tuple) => {
                append(
                    &mut result,
                    &self.render_node(base, depth + 1, budget, bytes, cancel),
                );
                append(&mut result, "[");
                let value = if tuple {
                    self.render_tuple(slice, depth + 1, budget, bytes, cancel)
                } else {
                    self.render_node(slice, depth + 1, budget, bytes, cancel)
                };
                append(&mut result, &value);
                append(&mut result, "]");
            }
            Shape::Tuple(first) => {
                // render_node already charged this tuple's AST node.
                self.tuple_items(&mut result, first, depth, budget, bytes, cancel);
            }
            Shape::Call(callee, arguments) => {
                append(
                    &mut result,
                    &self.render_node(callee, depth + 1, budget, bytes, cancel),
                );
                append(&mut result, "(");
                let Shape::Arguments(mut current) = self.entries[arguments].shape else {
                    unreachable!("validated arguments")
                };
                let mut positional = 0;
                let mut keyword = 0;
                let mut emitted = false;
                while let Some(i) = current {
                    if cancel.at(Checkpoint::Collector) {
                        break;
                    }
                    let entry = &self.entries[i];
                    current = entry.next_value;
                    let is_keyword = matches!(entry.shape, Shape::Keyword(..));
                    let count = if is_keyword {
                        &mut keyword
                    } else {
                        &mut positional
                    };
                    *count += 1;
                    if *count > 17 {
                        continue;
                    }
                    if emitted {
                        append(&mut result, ", ");
                    }
                    emitted = true;
                    if *count == 17 {
                        append(&mut result, "...");
                    } else if let Shape::Keyword(key, value) = entry.shape {
                        let key = &self.entries[key];
                        append(
                            &mut result,
                            std::str::from_utf8(&bytes[key.start..key.end]).unwrap(),
                        );
                        append(&mut result, "=");
                        append(
                            &mut result,
                            &self.render_node(value, depth + 1, budget, bytes, cancel),
                        );
                    } else {
                        append(
                            &mut result,
                            &self.render_node(i, depth + 1, budget, bytes, cancel),
                        );
                    }
                }
                append(&mut result, ")");
            }
            Shape::Arguments(_) | Shape::Keyword(..) => unreachable!("descriptor, not expression"),
            Shape::Alias(_) | Shape::Ignore => unreachable!("normalized expression"),
        }
        result
    }

    fn render_tuple<F: FnMut(Checkpoint) -> bool>(
        &self,
        first: usize,
        depth: usize,
        budget: &mut i32,
        bytes: &[u8],
        cancel: &Cancellation<F>,
    ) -> String {
        *budget -= 1;
        if *budget < 0 || depth > 12 || cancel.at(Checkpoint::Collector) {
            return "...".into();
        }
        let mut result = String::with_capacity(512);
        self.tuple_items(&mut result, Some(first), depth, budget, bytes, cancel);
        result
    }

    fn tuple_items<F: FnMut(Checkpoint) -> bool>(
        &self,
        result: &mut String,
        first: Option<usize>,
        depth: usize,
        budget: &mut i32,
        bytes: &[u8],
        cancel: &Cancellation<F>,
    ) {
        append(result, "[");
        let mut current = first;
        let mut count = 0;
        while let Some(i) = current {
            let e = &self.entries[i];
            current = e.next_value;
            if count == 16 {
                break;
            }
            if count != 0 {
                append(result, ", ");
            }
            append(
                result,
                &self.render_node(i, depth + 1, budget, bytes, cancel),
            );
            count += 1;
        }
        append(result, "]");
    }
}

fn append(output: &mut String, text: &str) {
    let mut end = text.len().min(512 - output.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    output.push_str(&text[..end]);
}

fn string<F: FnMut(Checkpoint) -> bool>(text: &[u8], cancel: &Cancellation<F>) -> Option<bool> {
    let quote = text
        .iter()
        .take(3)
        .position(|b| matches!(b, b'\'' | b'"'))?;
    let prefix = &text[..quote];
    let bytes = match prefix {
        b"" | b"r" | b"R" | b"u" | b"U" => false,
        b"b" | b"B" => true,
        [a, b]
            if (a.eq_ignore_ascii_case(&b'b') && b.eq_ignore_ascii_case(&b'r'))
                || (a.eq_ignore_ascii_case(&b'r') && b.eq_ignore_ascii_case(&b'b')) =>
        {
            true
        }
        _ => return None,
    };
    let q = text[quote];
    let width = if text.get(quote..quote + 3) == Some(&[q, q, q]) {
        3
    } else {
        1
    };
    if text.len() < quote + 2 * width || text[text.len() - width..].iter().any(|&b| b != q) {
        return None;
    }
    let body = &text[quote + width..text.len() - width];
    for chunk in body.chunks(4096) {
        if cancel.at(Checkpoint::Collector)
            || chunk.iter().any(|&b| {
                b == b'\\'
                    || b == 0
                    || (bytes && !b.is_ascii())
                    || (width == 1 && matches!(b, b'\n' | b'\r'))
            })
        {
            return None;
        }
    }
    Some(bytes)
}

// Validate lexical forms, never values. In particular, no Python-2 suffixes or
// octal spelling and no decimal conversion subject to interpreter digit limits.
fn number<F: FnMut(Checkpoint) -> bool>(text: &[u8], cancel: &Cancellation<F>) -> bool {
    if text.len() > 1024 || cancel.at(Checkpoint::Collector) {
        return false;
    }
    fn digits(text: &[u8], radix: u8, prefix_underscore: bool) -> bool {
        let mut previous = prefix_underscore;
        let mut any = false;
        for &b in text {
            if b == b'_' {
                if !previous {
                    return false;
                }
                previous = false;
            } else {
                let valid = match radix {
                    2 => matches!(b, b'0'..=b'1'),
                    8 => matches!(b, b'0'..=b'7'),
                    16 => b.is_ascii_hexdigit(),
                    _ => b.is_ascii_digit(),
                };
                if !valid {
                    return false;
                }
                previous = true;
                any = true;
            }
        }
        any && previous
    }
    if text.len() >= 2 && text[0] == b'0' {
        let radix = match text[1] {
            b'x' | b'X' => 16,
            b'o' | b'O' => 8,
            b'b' | b'B' => 2,
            _ => 0,
        };
        if radix != 0 {
            return digits(&text[2..], radix, true);
        }
    }
    if text.iter().filter(|b| b.is_ascii_digit()).count() > 512 {
        return false;
    }
    let imaginary = matches!(text.last(), Some(b'j' | b'J'));
    let text = if imaginary {
        &text[..text.len() - 1]
    } else {
        text
    };
    let exponent = text.iter().position(|b| matches!(b, b'e' | b'E'));
    let mantissa = if let Some(i) = exponent {
        let exp = &text[i + 1..];
        let exp = if matches!(exp.first(), Some(b'+' | b'-')) {
            &exp[1..]
        } else {
            exp
        };
        if !digits(exp, 10, false) {
            return false;
        }
        &text[..i]
    } else {
        text
    };
    if let Some(dot) = mantissa.iter().position(|&b| b == b'.') {
        let (left, right) = (&mantissa[..dot], &mantissa[dot + 1..]);
        (!left.is_empty() || !right.is_empty())
            && (left.is_empty() || digits(left, 10, false))
            && (right.is_empty() || digits(right, 10, false))
    } else {
        digits(mantissa, 10, false)
            && (exponent.is_some()
                || imaginary
                || mantissa[0] != b'0'
                || mantissa.iter().all(|b| matches!(b, b'0' | b'_')))
    }
}
