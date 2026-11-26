//! Borrowed-span validation and AST-shaped descriptors from the counted walk.
use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    Base,
    HeaderValue,
    AssignmentAnnotation,
    Annotation,
    Default,
    Return,
    BodyValue,
    ExceptionType,
    AssignmentTarget,
    NeutralTarget,
    ScalarTarget,
    DeleteTarget,
}

// Raw syntax permissions, independent of value classes and resolved aliases.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Role {
    Expression,
    Named,
    Yield,
    Grouped,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Ignore,
    Reference,
    Annotation,
    Text,
    Bytes,
    Formatted,
    Number,
    Constant,
    Unary,
    Call,
    Arguments,
    Keyword,
    Core,
    Pair,
    Slice,
    Starred,
    Unpack,
    // Validated grammar roles, never values or renderable AST nodes.
    Structure,
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
    Starred(usize),
    Unpack(usize),
    Binary(usize, &'static str, usize),
    Slice(u8),
    Opaque(&'static str),
}

struct Entry {
    kind: &'static str,
    named: bool,
    role: Role,
    parent: Option<usize>,
    first: Option<usize>,
    last: Option<usize>,
    next: Option<usize>,
    next_value: Option<usize>,
    last_value: Option<usize>,
    calls: projection::Calls,
    next_call: Option<usize>,
    keyword_default: bool,
    class: Class,
    shape: Shape,
    start: usize,
    end: usize,
    line: usize,
    end_line: usize,
    literal: Option<literals::Start>,
    field_depth: u8,
    bracket_depth: usize,
}

pub(super) struct Expression {
    pub(super) mode: Mode,
    pub(super) working: usize,
    pub(super) terminal_end_line: usize,
    pub(super) start: usize,
    pub(super) handler_alias: bool,
    pub(super) star_expressions: bool,
    pub(super) role: Role,
    base_depth: usize,
    bracket_depth: usize,
    entries: Vec<Entry>,
    current: Option<usize>,
}

impl Expression {
    pub(super) fn literal_payload(&self) -> bool {
        self.current.is_some_and(|i| {
            matches!(
                self.entries[i].kind,
                "string_content" | "format_specifier" | "escape_sequence" | "escape_interpolation"
            )
        })
    }

    // Four entries per logical node also covers Vec's initial four-slot
    // allocation, not just subsequent geometric capacity growth.
    pub(super) const ENTRY_WORK: usize = 4 * std::mem::size_of::<Entry>();
    // At most fourteen simultaneous 512-byte renderer results, plus the result
    // handed to the collector. No literal body or unbounded shape is allocated.
    pub(super) const RENDER_WORK: usize = 16 * 512;

    pub(super) fn new(mode: Mode, start: usize) -> Self {
        let base_depth = usize::from(matches!(
            mode,
            Mode::Base | Mode::Annotation | Mode::Default
        ));
        Self {
            mode,
            working: 0,
            terminal_end_line: 0,
            start,
            handler_alias: false,
            star_expressions: false,
            role: Role::Expression,
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
        if node.kind() == "line_continuation"
            && (!matches!(node.end_byte() - node.start_byte(), 2 | 3)
                || node.end_position().row != node.start_position().row + 1
                || node.end_position().column != 0)
        {
            // The pinned token also admits backslash-NUL. enter checks the
            // exact captured payload; terminal must not accept that alternative.
            return false;
        }
        let payload = self.literal_payload();
        for chunk in gap.chunks(4096) {
            if cancel.at(Checkpoint::Collector)
                || (!payload
                    && self.bracket_depth == 0
                    && chunk.iter().any(|b| matches!(b, b'\n' | b'\r')))
            {
                return false;
            }
        }
        // Only grammar punctuation changes implicit continuation. Brackets and
        // newlines inside literal payloads or comments are not syntax tokens.
        if !payload && !node.is_named() {
            match node.kind() {
                "(" | "[" | "{" => self.bracket_depth += 1,
                ")" | "]" | "}" if self.bracket_depth > self.base_depth => self.bracket_depth -= 1,
                ")" | "]" | "}" => return false,
                _ => {}
            }
        }
        if !matches!(node.kind(), "comment" | "line_continuation") {
            self.terminal_end_line = node.end_position().row + 1;
        }
        true
    }

    pub(super) fn set_base_depth(&mut self, depth: usize) {
        self.base_depth = depth;
        self.bracket_depth = depth;
    }

    pub(super) fn enter<F: FnMut(Checkpoint) -> bool>(
        &mut self,
        node: Node<'_>,
        bytes: &[u8],
        cancel: &Cancellation<F>,
    ) -> bool {
        let kind = node.kind();
        let text = &bytes[node.start_byte()..node.end_byte()];
        let parent = self.current.map(|i| self.entries[i].kind).unwrap_or("");
        if kind == "lambda" && node.is_named() {
            let mut ancestor = self.current;
            while let Some(i) = ancestor {
                if cancel.at(Checkpoint::Collector) {
                    return false;
                }
                let entry = &self.entries[i];
                if matches!(entry.kind, "interpolation" | "format_expression") {
                    // A field's opening brace grants continuation, but only an
                    // additional real delimiter protects a lambda's colon.
                    if self.bracket_depth <= entry.bracket_depth + 1 {
                        return false;
                    }
                    break;
                }
                ancestor = entry.parent;
            }
        }
        let mut field_depth = self.current.map_or(0, |i| self.entries[i].field_depth);
        let mut literal = self.current.and_then(|i| self.entries[i].literal);
        if !matches!(
            parent,
            "string"
                | "string_content"
                | "format_specifier"
                | "escape_sequence"
                | "escape_interpolation"
        ) {
            literal = None;
        }
        let (class, shape) = match kind {
            _ if parent == "string_content"
                || matches!(parent, "escape_sequence" | "escape_interpolation") =>
            {
                if node.is_named() && !matches!(kind, "escape_sequence" | "escape_interpolation") {
                    return false;
                }
                (Class::Ignore, Shape::Ignore)
            }
            _ if parent == "format_specifier" && !node.is_named() => {
                if kind != ":" && !literal_segment(text, literal.unwrap(), true, cancel) {
                    return false;
                }
                (Class::Ignore, Shape::Ignore)
            }
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
            "string" => {
                field_depth = 0;
                let Some(start) = node.child(0).filter(|n| n.kind() == "string_start") else {
                    return false;
                };
                let Ok(info) = literals::start(&bytes[start.start_byte()..start.end_byte()]) else {
                    return false;
                };
                literal = Some(info);
                match info.flavor {
                    literals::Flavor::Formatted => (Class::Formatted, Shape::Opaque("<JoinedStr>")),
                    _ => match string(text, cancel) {
                        Some(false) => (Class::Text, Shape::Literal),
                        Some(true) => (Class::Bytes, Shape::Literal),
                        None => return false,
                    },
                }
            }
            "string_content" if parent == "string" => {
                let Some(info) = literal else {
                    return false;
                };
                if matches!(info.flavor, literals::Flavor::Formatted)
                    && !literal_segment(text, info, false, cancel)
                {
                    return false;
                }
                (Class::Ignore, Shape::Ignore)
            }
            "string_start" | "string_end" if parent == "string" => {
                if kind == "string_end" {
                    let Some(info) = literal else {
                        return false;
                    };
                    if text.len() != (if info.triple { 3 } else { 1 })
                        || text.iter().any(|&b| b != info.quote)
                    {
                        return false;
                    }
                }
                (Class::Ignore, Shape::Ignore)
            }
            "interpolation" | "format_expression"
                if (kind == "interpolation"
                    && parent == "string"
                    && literal
                        .is_some_and(|s| matches!(s.flavor, literals::Flavor::Formatted)))
                    || (kind == "format_expression" && parent == "format_specifier") =>
            {
                // CPython 3.12 tokenizer.h MAX_EXPR_NESTING is three fields
                // per f-string; a nested f-string starts its own counter.
                field_depth += 1;
                if field_depth > 3 {
                    return false;
                }
                (Class::Ignore, Shape::Ignore)
            }
            "format_specifier" if matches!(parent, "interpolation" | "format_expression") => {
                // Inherit only the owning string's flags, not its body/span.
                let mut owner = self.current;
                while let Some(i) = owner {
                    if cancel.at(Checkpoint::Collector) {
                        return false;
                    }
                    if self.entries[i].kind == "string" {
                        literal = self.entries[i].literal;
                        break;
                    }
                    owner = self.entries[i].parent;
                }
                if literal.is_none() {
                    return false;
                }
                (Class::Ignore, Shape::Ignore)
            }
            "type_conversion"
                if matches!(parent, "interpolation" | "format_expression")
                    && matches!(text, b"!s" | b"!r" | b"!a") =>
            {
                (Class::Ignore, Shape::Ignore)
            }
            "line_continuation" if matches!(text, b"\\\n" | b"\\\r\n") => {
                (Class::Ignore, Shape::Ignore)
            }
            "type"
            | "parenthesized_expression"
            | "attribute"
            | "member_type"
            | "generic_type"
            | "union_type"
            | "subscript"
            | "tuple"
            | "concatenated_string"
            | "unary_operator" => (Class::Ignore, Shape::Ignore),
            "type_parameter"
                if self.current.is_some_and(|p| {
                    self.entries[p].kind == "generic_type" && self.entries[p].last_value.is_some()
                }) =>
            {
                (Class::Ignore, Shape::Ignore)
            }
            "constrained_type" if self.type_slice() => (Class::Ignore, Shape::Ignore),
            "splat_type"
                if self.type_slice()
                    || (self.star_expressions
                        && self.current.is_none_or(|p| {
                            self.entries[p].kind == "type" && self.entries[p].parent.is_none()
                        })) =>
            {
                (Class::Ignore, Shape::Ignore)
            }
            "call" | "argument_list" | "keyword_argument"
                if !matches!(
                    self.mode,
                    Mode::Base
                        | Mode::HeaderValue
                        | Mode::Annotation
                        | Mode::AssignmentAnnotation
                        | Mode::Return
                        | Mode::Default
                ) =>
            {
                (Class::Ignore, Shape::Ignore)
            }
            "lambda"
            | "await"
            | "yield"
            | "named_expression"
            | "lambda_parameters"
            | "default_parameter"
            | "positional_separator"
            | "keyword_separator"
            | "dictionary_splat_pattern"
            | "list_comprehension"
            | "set_comprehension"
            | "dictionary_comprehension"
            | "generator_expression"
            | "for_in_clause"
            | "if_clause" => (Class::Ignore, Shape::Ignore),
            "for" | "async" | "from" | ":=" if !node.is_named() => (Class::Ignore, Shape::Ignore),
            "as_pattern" if self.mode == Mode::ExceptionType && !self.handler_alias => {
                // The grammar can attach a handler's `as name` to the final
                // else operand or lambda body. Only those ungrouped trailing
                // expression spines may carry it; grouping/defaults forbid it.
                let mut ancestor = self.current;
                if ancestor.is_none() {
                    return false;
                }
                while let Some(i) = ancestor {
                    if !matches!(self.entries[i].kind, "conditional_expression" | "lambda") {
                        return false;
                    }
                    let mut child = self.entries[i].first;
                    let mut values = 0;
                    while let Some(j) = child {
                        if cancel.at(Checkpoint::Collector) {
                            return false;
                        }
                        values += usize::from(self.entries[j].class != Class::Ignore);
                        child = self.entries[j].next;
                    }
                    let expected = if self.entries[i].kind == "lambda" {
                        usize::from(
                            self.entries[i]
                                .last_value
                                .is_some_and(|p| self.entries[p].kind == "lambda_parameters"),
                        )
                    } else {
                        2
                    };
                    if values != expected {
                        return false;
                    }
                    ancestor = self.entries[i].parent;
                }
                self.handler_alias = true;
                (Class::Ignore, Shape::Ignore)
            }
            "as_pattern_target"
                if self.mode == Mode::ExceptionType
                    && self
                        .current
                        .is_some_and(|i| self.entries[i].kind == "as_pattern") =>
            {
                if node.child_count() != 1
                    || node
                        .child(0)
                        .is_none_or(|child| child.kind() != "identifier")
                {
                    return false;
                }
                (Class::Ignore, Shape::Ignore)
            }
            "as" if !node.is_named()
                && self.mode == Mode::ExceptionType
                && self
                    .current
                    .is_some_and(|i| self.entries[i].kind == "as_pattern") =>
            {
                (Class::Ignore, Shape::Ignore)
            }
            "list"
            | "list_pattern"
            | "tuple_pattern"
            | "pattern_list"
            | "list_splat_pattern"
            | "list_splat"
            | "dictionary_splat"
            | "as_pattern_target"
            | "set"
            | "dictionary"
            | "pair"
            | "expression_list"
            | "binary_operator"
            | "boolean_operator"
            | "comparison_operator"
            | "not_operator"
            | "conditional_expression"
            | "slice" => (Class::Ignore, Shape::Ignore),
            // Composite comparison operators have counted terminal children.
            "not in" | "is not" => (Class::Ignore, Shape::Ignore),
            "{" | "}" | ":" | "*" | "@" | "/" | "//" | "%" | "**" | "<<" | ">>" | "|" | "^"
            | "&" | "~" | "and" | "or" | "not" | "in" | "is" | "<" | "<=" | "==" | "!=" | ">="
            | ">" | "if" | "else"
                if !node.is_named() =>
            {
                (Class::Ignore, Shape::Ignore)
            }
            "=" if !node.is_named() => (Class::Ignore, Shape::Ignore),
            "comment" => {
                if type_comment(text, cancel) {
                    // TYPE_IGNORE is skipped by pegen even inside replacement
                    // expressions; TYPE_COMMENT is not an expression token.
                    let mut at = 1;
                    while matches!(text.get(at), Some(b' ' | b'\t')) {
                        if cancel.at(Checkpoint::Collector) {
                            return false;
                        }
                        at += 1;
                    }
                    at += 5; // type:
                    while matches!(text.get(at), Some(b' ' | b'\t')) {
                        if cancel.at(Checkpoint::Collector) {
                            return false;
                        }
                        at += 1;
                    }
                    if field_depth == 0
                        || !text[at..].starts_with(b"ignore")
                        || text
                            .get(at + 6)
                            .is_some_and(|b| !b.is_ascii() || b.is_ascii_alphanumeric())
                    {
                        return false;
                    }
                }
                (Class::Ignore, Shape::Ignore)
            }
            "(" | ")" | "[" | "]" | "," | "." | "+" | "-" if !node.is_named() => {
                (Class::Ignore, Shape::Ignore)
            }
            _ => return false,
        };
        let index = self.entries.len();
        let role = self.current.map_or(self.role, |p| {
            let parent = &self.entries[p];
            match parent.kind {
                "type" => parent.role,
                "parenthesized_expression" => Role::Grouped,
                "interpolation" | "format_expression" => Role::Yield,
                "argument_list" | "list" | "tuple" | "set" | "type_parameter" => Role::Named,
                "subscript" if parent.last_value.is_some() => Role::Named,
                "list_comprehension" | "set_comprehension" | "generator_expression"
                    if parent.last_value.is_none() =>
                {
                    Role::Named
                }
                _ => Role::Expression,
            }
        });
        if kind == "named_expression"
            && (node
                .child_by_field_name("name")
                .is_none_or(|n| n.kind() != "identifier")
                || node.child_by_field_name("value").is_none())
        {
            return false;
        }
        self.entries.push(Entry {
            kind,
            named: node.is_named(),
            role,
            parent: self.current,
            first: None,
            last: None,
            next: None,
            next_value: None,
            last_value: None,
            calls: projection::Calls::default(),
            next_call: None,
            keyword_default: false,
            class,
            shape,
            start: node.start_byte(),
            end: node.end_byte(),
            line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
            literal,
            field_depth,
            bracket_depth: self.bracket_depth,
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
        if matches!(kind, "string_content" | "format_specifier") {
            self.terminal_end_line = entry.end_line;
        }
        if entry.named
            && ((kind == "named_expression" && !matches!(entry.role, Role::Named | Role::Grouped))
                || (kind == "yield" && !matches!(entry.role, Role::Yield | Role::Grouped)))
        {
            return Err(());
        }
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
        if matches!(kind, "list_splat" | "dictionary_splat")
            && !(self.mode == Mode::Base && entry.parent.is_none())
            && !entry
                .parent
                .is_some_and(|p| matches!(self.entries[p].kind, "argument_list" | "subscript"))
            && first.is_some_and(|i| {
                matches!(
                    self.entries[i].kind,
                    "conditional_expression"
                        | "boolean_operator"
                        | "comparison_operator"
                        | "not_operator"
                )
            })
        {
            // Displays unpack a bitwise_or; call/index splats accept an expression.
            // Inspect the wrapper, so grouping still admits the wider operand.
            return Err(());
        }
        // The CST can attach a leading splat to a primary's callee/receiver
        // instead of its complete value: Call(Starred(f), args). Lift it to
        // Starred(Call(f, args)), reusing the splat entry rather than allocating.
        let raw_first = first;
        // A Base arena omits its enclosing class argument_list. Only its root
        // and the same operator hoist path as call arguments inherit that role.
        let mut wide_star = self.mode == Mode::Base && entry.parent.is_none();
        if first.is_some_and(|i| matches!(self.entries[i].shape, Shape::Starred(_))) {
            let mut parent = entry.parent;
            while let Some(i) = parent {
                if cancel.at(Checkpoint::Collector) {
                    return Err(());
                }
                match self.entries[i].kind {
                    "subscript" | "argument_list" => {
                        wide_star = true;
                        break;
                    }
                    "boolean_operator" | "comparison_operator" | "conditional_expression" => {
                        if self.mode == Mode::Base && self.entries[i].parent.is_none() {
                            wide_star = true;
                            break;
                        }
                        parent = self.entries[i].parent;
                    }
                    _ => break,
                }
            }
        }
        let lifted = first.filter(|&i| {
            (matches!(kind, "call" | "attribute" | "subscript" | "binary_operator")
                || (wide_star
                    && matches!(
                        kind,
                        "boolean_operator" | "comparison_operator" | "conditional_expression"
                    )))
                && matches!(self.entries[i].shape, Shape::Starred(_))
        });
        if let Some(i) = lifted {
            let Shape::Starred(value) = self.entries[i].shape else {
                unreachable!()
            };
            first = Some(value);
        }
        let normalized = match kind {
            "interpolation" | "format_expression" => {
                let value = first.ok_or(())?;
                if count != 1
                    || (!self.body_value(value) && self.entries[value].class != Class::Starred)
                    || matches!(self.entries[value].kind, "lambda" | "named_expression")
                {
                    // G4: := consumes the format colon in this CST, including
                    // turning nested replacement fields into sets/dicts. Those
                    // lost field boundaries need a grammar fix, not call guesses.
                    // Ungrouped lambda has the same colon-role ambiguity.
                    return Err(());
                }
                let mut phase = 0;
                let mut child = entry.first;
                while let Some(i) = child {
                    if cancel.at(Checkpoint::Collector) {
                        return Err(());
                    }
                    let e = &self.entries[i];
                    match e.kind {
                        "{" if phase == 0 => phase = 1,
                        "comment" | "line_continuation" => {}
                        "=" if phase == 2 => phase = 3,
                        "type_conversion" if matches!(phase, 2 | 3) => phase = 4,
                        "format_specifier" if matches!(phase, 2..=4) => phase = 5,
                        "}" if matches!(phase, 2..=5) => phase = 6,
                        _ if phase == 1 && i == value => phase = 2,
                        _ => return Err(()),
                    }
                    child = e.next;
                }
                if phase != 6 {
                    return Err(());
                }
                Some((Class::Ignore, Shape::Opaque("<FormattedValue>")))
            }
            "format_specifier" => {
                let info = entry.literal.ok_or(())?;
                let mut child = entry.first;
                let mut end = entry.start;
                while let Some(i) = child {
                    if cancel.at(Checkpoint::Collector) {
                        return Err(());
                    }
                    let e = &self.entries[i];
                    if e.named && e.kind != "format_expression" {
                        return Err(());
                    }
                    if !literal_segment(&bytes[end..e.start], info, true, cancel) {
                        return Err(());
                    }
                    end = e.end;
                    child = e.next;
                }
                if !literal_segment(&bytes[end..entry.end], info, true, cancel) {
                    return Err(());
                }
                None
            }
            "named_expression" => {
                if count != 2
                    || self.entries[first.ok_or(())?].kind != "identifier"
                    || !self.body_value(second.ok_or(())?)
                {
                    return Err(());
                }
                Some((Class::Core, Shape::Opaque("<NamedExpr>")))
            }
            "yield" if entry.named => {
                let mut child = entry.first;
                let mut from = false;
                while let Some(i) = child {
                    if cancel.at(Checkpoint::Collector) {
                        return Err(());
                    }
                    from |= self.entries[i].kind == "from" && !self.entries[i].named;
                    child = self.entries[i].next;
                }
                if count > 1
                    || (from && count != 1)
                    || first.is_some_and(|i| {
                        !self.body_value(i) && (from || self.entries[i].class != Class::Starred)
                    })
                    || (from && first.is_some_and(|i| self.entries[i].kind == "expression_list"))
                {
                    return Err(());
                }
                Some((
                    Class::Core,
                    Shape::Opaque(if from { "<YieldFrom>" } else { "<Yield>" }),
                ))
            }
            "await" if count == 1 => {
                let operand = first.unwrap();
                if self.entries[operand].kind == "binary_operator" {
                    let Shape::Binary(left, "Pow", right) = self.entries[operand].shape else {
                        return Err(());
                    };
                    if !self.primary(left) {
                        return Err(());
                    }
                    // The pinned grammar puts power inside await. Python puts
                    // await on its left primary; recycle the power descriptor.
                    self.entries[operand].class = Class::Core;
                    self.entries[operand].shape = Shape::Opaque("<Await>");
                    Some((Class::Core, Shape::Binary(operand, "Pow", right)))
                } else {
                    if !self.primary(operand) {
                        return Err(());
                    }
                    Some((Class::Core, Shape::Opaque("<Await>")))
                }
            }
            "lambda_parameters" => {
                // AST parsing allows duplicate names. Validate borrowed roles,
                // not function signature/alias state or compiler constraints.
                let mut child = entry.first;
                let mut want_parameter = true;
                let mut positional = false;
                let mut slash = false;
                let mut star = false;
                let mut bare_star = false;
                let mut keyword_only = false;
                let mut kwarg = false;
                let mut default_seen = false;
                while let Some(i) = child {
                    if cancel.at(Checkpoint::Collector) {
                        return Err(());
                    }
                    let e = &self.entries[i];
                    match e.kind {
                        "comment" | "line_continuation" => {}
                        "," if !want_parameter => want_parameter = true,
                        _ if !want_parameter || kwarg => return Err(()),
                        "positional_separator" if positional && !slash && !star => {
                            slash = true;
                            want_parameter = false;
                        }
                        "keyword_separator" | "list_splat_pattern" if !star => {
                            star = true;
                            bare_star = e.kind == "keyword_separator";
                            want_parameter = false;
                        }
                        "dictionary_splat_pattern" => {
                            kwarg = true;
                            want_parameter = false;
                        }
                        "identifier" | "default_parameter" => {
                            if star {
                                keyword_only = true;
                            } else {
                                if default_seen && e.kind != "default_parameter" {
                                    return Err(());
                                }
                                default_seen |= e.kind == "default_parameter";
                                positional = true;
                            }
                            want_parameter = false;
                        }
                        _ => return Err(()),
                    }
                    child = e.next;
                }
                if (!positional && !star && !kwarg) || (bare_star && !keyword_only) {
                    return Err(());
                }
                Some((Class::Structure, Shape::Ignore))
            }
            "default_parameter" => {
                if count != 2
                    || self.entries[first.ok_or(())?].kind != "identifier"
                    || !self.body_value(second.ok_or(())?)
                    || entry
                        .parent
                        .is_none_or(|p| self.entries[p].kind != "lambda_parameters")
                {
                    return Err(());
                }
                Some((Class::Structure, Shape::Ignore))
            }
            "positional_separator" | "keyword_separator" | "dictionary_splat_pattern" => {
                if entry
                    .parent
                    .is_none_or(|p| self.entries[p].kind != "lambda_parameters")
                    || (kind == "dictionary_splat_pattern"
                        && (count != 1 || self.entries[first.ok_or(())?].kind != "identifier"))
                {
                    return Err(());
                }
                Some((Class::Structure, Shape::Ignore))
            }
            "list_splat_pattern"
                if entry
                    .parent
                    .is_some_and(|p| self.entries[p].kind == "lambda_parameters") =>
            {
                if count != 1 || self.entries[first.ok_or(())?].kind != "identifier" {
                    return Err(());
                }
                Some((Class::Structure, Shape::Ignore))
            }
            "lambda" if entry.first.is_some() => {
                let body = if count == 1 {
                    first.ok_or(())?
                } else if count == 2 && self.entries[first.ok_or(())?].kind == "lambda_parameters" {
                    second.ok_or(())?
                } else {
                    return Err(());
                };
                if !self.body_value(body) {
                    return Err(());
                }
                Some((Class::Core, Shape::Opaque("<Lambda>")))
            }
            "for_in_clause" => {
                let (target, iter) = (first.ok_or(())?, second.ok_or(())?);
                // The pinned CST admits comma-separated iterables and wider
                // expression roots than Python. Check wrappers before aliases.
                if count != 2
                    || comma
                    || !self.valid_target(target, Mode::NeutralTarget, cancel)
                    || !self.body_value(iter)
                    || matches!(self.entries[iter].kind, "lambda" | "conditional_expression")
                {
                    return Err(());
                }
                Some((Class::Structure, Shape::Ignore))
            }
            "if_clause" => {
                let value = first.ok_or(())?;
                if count != 1
                    || !self.body_value(value)
                    || matches!(
                        self.entries[value].kind,
                        "lambda" | "conditional_expression"
                    )
                {
                    return Err(());
                }
                Some((Class::Structure, Shape::Ignore))
            }
            "list_comprehension"
            | "set_comprehension"
            | "dictionary_comprehension"
            | "generator_expression" => {
                let result = first.ok_or(())?;
                if (kind == "dictionary_comprehension" && self.entries[result].class != Class::Pair)
                    || (kind != "dictionary_comprehension" && !self.body_value(result))
                {
                    return Err(());
                }
                let mut child = self.entries[result].next_value;
                let mut fors = 0;
                while let Some(i) = child {
                    if cancel.at(Checkpoint::Collector) {
                        return Err(());
                    }
                    match self.entries[i].kind {
                        "for_in_clause" => fors += 1,
                        "if_clause" if fors != 0 => {}
                        _ => return Err(()),
                    }
                    child = self.entries[i].next_value;
                }
                if fors == 0 {
                    return Err(());
                }
                Some((
                    Class::Core,
                    Shape::Opaque(match kind {
                        "list_comprehension" => "<ListComp>",
                        "set_comprehension" => "<SetComp>",
                        "dictionary_comprehension" => "<DictComp>",
                        _ => "<GeneratorExp>",
                    }),
                ))
            }
            "as_pattern_target"
                if self.mode == Mode::ExceptionType
                    && count == 1
                    && self.entries[first.unwrap()].kind == "identifier" =>
            {
                Some((Class::Ignore, Shape::Ignore))
            }
            "as_pattern" if count == 1 && self.body_value(first.unwrap()) => {
                let i = first.unwrap();
                Some((self.entries[i].class, Shape::Alias(self.resolve(i))))
            }
            "not in" | "is not" => {
                let expected = if kind == "not in" {
                    ["not", "in"]
                } else {
                    ["is", "not"]
                };
                let mut at = 0;
                let mut child = entry.first;
                while let Some(i) = child {
                    if cancel.at(Checkpoint::Collector) {
                        return Err(());
                    }
                    let e = &self.entries[i];
                    if !matches!(e.kind, "comment" | "line_continuation") {
                        if expected.get(at) != Some(&e.kind) {
                            return Err(());
                        }
                        at += 1;
                    }
                    child = e.next;
                }
                if at != 2 {
                    return Err(());
                }
                None
            }
            "binary_operator" | "boolean_operator" | "union_type" if count == 2 => {
                let (a, b) = (first.unwrap(), second.unwrap());
                if kind == "union_type"
                    && [a, b].into_iter().any(|i| {
                        let raw = if self.entries[i].kind == "type" {
                            self.entries[i].last_value.unwrap()
                        } else {
                            i
                        };
                        matches!(
                            self.entries[raw].kind,
                            "lambda"
                                | "conditional_expression"
                                | "boolean_operator"
                                | "comparison_operator"
                                | "not_operator"
                                | "named_expression"
                                | "yield"
                        )
                    })
                {
                    // Type grammar operands are wider than bitwise-or operands.
                    // Grouping must remain visible, just as for ordinary core ops.
                    return Err(());
                }
                let mut op = self.entries[raw_first.unwrap()].next.ok_or(())?;
                while matches!(self.entries[op].kind, "comment" | "line_continuation") {
                    if cancel.at(Checkpoint::Collector) {
                        return Err(());
                    }
                    op = self.entries[op].next.ok_or(())?;
                }
                if !self.body_value(a) || !self.body_value(b) {
                    return Err(());
                }
                if kind == "boolean_operator" {
                    // Boolean operands are disjunction/conjunction roles, not
                    // arbitrary expressions. Grouping stays visible here.
                    if matches!(self.entries[a].kind, "lambda")
                        || matches!(self.entries[b].kind, "lambda")
                        || !matches!(self.entries[op].kind, "and" | "or")
                    {
                        return Err(());
                    }
                    Some((Class::Core, Shape::Opaque("<BoolOp>")))
                } else {
                    let op = match self.entries[op].kind {
                        "+" => "Add",
                        "-" => "Sub",
                        "*" => "Mult",
                        "@" => "MatMult",
                        "/" => "Div",
                        "//" => "FloorDiv",
                        "%" => "Mod",
                        "**" => "Pow",
                        "<<" => "LShift",
                        ">>" => "RShift",
                        "|" => "BitOr",
                        "^" => "BitXor",
                        "&" => "BitAnd",
                        _ => return Err(()),
                    };
                    Some((Class::Core, Shape::Binary(a, op, b)))
                }
            }
            "comparison_operator" if count >= 2 => {
                let mut child = entry.first;
                let mut operand = true;
                while let Some(i) = child {
                    if cancel.at(Checkpoint::Collector) {
                        return Err(());
                    }
                    let e = &self.entries[i];
                    if !matches!(e.kind, "comment" | "line_continuation") {
                        if operand {
                            let value = if Some(i) == raw_first {
                                first.unwrap()
                            } else {
                                i
                            };
                            if !self.body_value(value) || self.entries[value].kind == "lambda" {
                                return Err(());
                            }
                        } else if !matches!(
                            e.kind,
                            "<" | "<="
                                | "=="
                                | "!="
                                | ">="
                                | ">"
                                | "in"
                                | "not in"
                                | "is"
                                | "is not"
                        ) {
                            return Err(());
                        }
                        operand = !operand;
                    }
                    child = e.next;
                }
                if operand {
                    return Err(());
                }
                Some((Class::Core, Shape::Opaque("<Compare>")))
            }
            "not_operator" if count == 1 && self.body_value(first.unwrap()) => {
                if self.entries[first.unwrap()].kind == "lambda" {
                    return Err(());
                }
                Some((Class::Core, Shape::Opaque("<UnaryOp>")))
            }
            "conditional_expression" if count == 3 => {
                // The grammar accepts bare conditional/lambda tests;
                // Python requires grouping there. Inspect the CST wrapper,
                // not its resolved alias, to preserve parenthesized tests.
                if matches!(
                    self.entries[second.unwrap()].kind,
                    "conditional_expression" | "lambda"
                ) {
                    return Err(());
                }
                let mut child = raw_first;
                while let Some(i) = child {
                    let value = if Some(i) == raw_first {
                        first.unwrap()
                    } else {
                        i
                    };
                    if cancel.at(Checkpoint::Collector) || !self.body_value(value) {
                        return Err(());
                    }
                    child = self.entries[i].next_value;
                }
                Some((Class::Core, Shape::Opaque("<IfExp>")))
            }
            "pair"
                if count == 2
                    && self.body_value(first.unwrap())
                    && self.body_value(second.unwrap()) =>
            {
                Some((Class::Pair, Shape::Ignore))
            }
            "slice" => {
                let mut child = entry.first;
                let mut colons = 0;
                let mut bound = false;
                while let Some(i) = child {
                    if cancel.at(Checkpoint::Collector) {
                        return Err(());
                    }
                    let e = &self.entries[i];
                    if e.kind == ":" {
                        colons += 1;
                        bound = false;
                    } else if !matches!(e.kind, "comment" | "line_continuation") {
                        if bound || !self.body_value(i) {
                            return Err(());
                        }
                        bound = true;
                    }
                    child = e.next;
                }
                if !(1..=2).contains(&colons) {
                    return Err(());
                }
                Some((Class::Slice, Shape::Slice(colons)))
            }
            "constrained_type" if count == 2 => {
                let (a, b) = (first.unwrap(), second.unwrap());
                let colons = match self.entries[self.resolve(b)].shape {
                    Shape::Slice(n) => n + 1,
                    _ if self.body_value(b) => 1,
                    _ => return Err(()),
                };
                if !self.body_value(a) || colons > 2 {
                    return Err(());
                }
                Some((Class::Slice, Shape::Slice(colons)))
            }
            "splat_type" if count == 1 && self.body_value(first.unwrap()) => {
                // ** is declaration syntax, never an expression index unpack.
                let mut child = entry.first;
                let mut star = false;
                while let Some(i) = child {
                    if cancel.at(Checkpoint::Collector) || self.entries[i].kind == "**" {
                        return Err(());
                    }
                    star |= self.entries[i].kind == "*";
                    child = self.entries[i].next;
                }
                if !star {
                    return Err(());
                }
                Some((Class::Starred, Shape::Starred(first.unwrap())))
            }
            "list_splat" | "list_splat_pattern"
                if count == 1 && self.body_value(first.unwrap()) =>
            {
                Some((Class::Starred, Shape::Starred(first.unwrap())))
            }
            "dictionary_splat" if count == 1 && self.body_value(first.unwrap()) => {
                Some((Class::Unpack, Shape::Unpack(first.unwrap())))
            }
            "tuple_pattern" if count == 1 && !comma => {
                let i = first.unwrap();
                if self.entries[self.resolve(i)].class == Class::Starred {
                    return Err(());
                }
                Some((self.entries[i].class, Shape::Alias(self.resolve(i))))
            }
            "list" | "set" | "dictionary" | "expression_list" | "tuple" | "list_pattern"
            | "tuple_pattern" | "pattern_list" => {
                let mut child = entry.first;
                let mut want_value = true;
                while let Some(i) = child {
                    if cancel.at(Checkpoint::Collector) {
                        return Err(());
                    }
                    let e = &self.entries[i];
                    match e.kind {
                        "(" | ")" | "[" | "]" | "{" | "}" | "comment" | "line_continuation" => {}
                        "," if !want_value => want_value = true,
                        _ => {
                            let valid = if kind == "dictionary" {
                                matches!(e.class, Class::Pair | Class::Unpack)
                            } else {
                                self.body_value(i) || e.class == Class::Starred
                            };
                            if !want_value || !valid {
                                return Err(());
                            }
                            want_value = false;
                        }
                    }
                    child = e.next;
                }
                if (count == 0 && (comma || matches!(kind, "set" | "expression_list")))
                    || (matches!(kind, "expression_list" | "pattern_list") && !comma)
                    || (kind == "tuple"
                        && count == 1
                        && !comma
                        && self.entries[first.unwrap()].class == Class::Starred)
                {
                    return Err(());
                }
                Some((
                    Class::Core,
                    if kind == "dictionary" {
                        Shape::Opaque("<Dict>")
                    } else {
                        Shape::Tuple(first)
                    },
                ))
            }
            "keyword_argument" if count == 2 => {
                let (key, value) = (first.unwrap(), second.unwrap());
                if !matches!(self.entries[key].shape, Shape::Name) || !self.body_value(value) {
                    return Err(());
                }
                Some((Class::Keyword, Shape::Keyword(key, value)))
            }
            "argument_list" => {
                if count == 0 && comma {
                    return Err(());
                }
                let mut child = first;
                let mut keyword_seen = false;
                let mut unpack_seen = false;
                while let Some(i) = child {
                    if cancel.at(Checkpoint::Collector) {
                        return Err(());
                    }
                    let item = &self.entries[i];
                    if matches!(item.shape, Shape::Keyword(..) | Shape::Unpack(..)) {
                        keyword_seen = true;
                        unpack_seen |= matches!(item.shape, Shape::Unpack(..));
                    } else if matches!(item.shape, Shape::Starred(..)) {
                        if unpack_seen {
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
                if !self.body_value(callee)
                    || !(self.entries[args].class == Class::Arguments
                        || self.entries[args].kind == "generator_expression")
                {
                    return Err(());
                }
                let callee = self.resolve(callee);
                let name = match self.entries[callee].shape {
                    Shape::Attribute(_, last) => {
                        let name = &self.entries[last];
                        &bytes[name.start..name.end]
                    }
                    Shape::Name => &bytes[self.entries[callee].start..self.entries[callee].end],
                    _ => b"".as_slice(),
                };
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
            "type" | "parenthesized_expression" | "as_pattern_target" if count == 1 => {
                let i = first.unwrap();
                if kind == "parenthesized_expression"
                    && self.entries[self.resolve(i)].class == Class::Starred
                {
                    return Err(());
                }
                Some((self.entries[i].class, Shape::Alias(self.resolve(i))))
            }
            "attribute" | "member_type" if count == 2 => {
                let (a, b) = (first.unwrap(), second.unwrap());
                if kind == "member_type" {
                    let raw = if self.entries[a].kind == "type" {
                        self.entries[a].last_value.unwrap()
                    } else {
                        a
                    };
                    if !self.primary(raw) {
                        return Err(());
                    }
                }
                if !self.body_value(a) || !matches!(self.entries[b].shape, Shape::Name) {
                    return Err(());
                }
                Some((Class::Reference, Shape::Attribute(a, b)))
            }
            "type_parameter" => {
                let mut child = first;
                while let Some(i) = child {
                    if cancel.at(Checkpoint::Collector) {
                        return Err(());
                    }
                    let e = &self.entries[i];
                    if !self.body_value(i) && !matches!(e.class, Class::Slice | Class::Starred) {
                        return Err(());
                    }
                    child = e.next_value;
                }
                if count == 0 {
                    return Err(());
                }
                if count == 1 && !comma && self.entries[first.unwrap()].class != Class::Starred {
                    let i = first.unwrap();
                    Some((self.entries[i].class, Shape::Alias(self.resolve(i))))
                } else {
                    Some((Class::Core, Shape::Tuple(first)))
                }
            }
            "generic_type" | "subscript" if count >= 2 => {
                let (base, slice) = (first.unwrap(), second.unwrap());
                if !self.body_value(base) {
                    return Err(());
                }
                let mut child = Some(slice);
                while let Some(i) = child {
                    if cancel.at(Checkpoint::Collector) {
                        return Err(());
                    }
                    let e = &self.entries[i];
                    if !self.body_value(i) && !matches!(e.class, Class::Slice | Class::Starred) {
                        return Err(());
                    }
                    child = e.next_value;
                }
                if kind == "generic_type" && count != 2 {
                    return Err(());
                }
                Some((
                    Class::Core,
                    Shape::Subscript(
                        base,
                        slice,
                        kind == "subscript"
                            && (count > 2 || comma || self.entries[slice].class == Class::Starred),
                    ),
                ))
            }
            "concatenated_string" if count >= 2 => {
                let mut category = self.entries[first.unwrap()].class;
                if !matches!(category, Class::Text | Class::Bytes | Class::Formatted) {
                    return Err(());
                }
                let mut child = first;
                while let Some(i) = child {
                    if cancel.at(Checkpoint::Collector) {
                        return Err(());
                    }
                    let e = &self.entries[i];
                    if e.class != Class::Ignore
                        && (e.class == Class::Bytes) != (category == Class::Bytes)
                    {
                        return Err(());
                    }
                    if e.class == Class::Formatted {
                        category = Class::Formatted;
                    }
                    child = e.next;
                }
                Some((
                    category,
                    if category == Class::Formatted {
                        Shape::Opaque("<JoinedStr>")
                    } else {
                        Shape::Literal
                    },
                ))
            }
            "unary_operator" if count == 1 => {
                if !self.body_value(first.unwrap()) {
                    return Err(());
                }
                Some((Class::Unary, Shape::Unary))
            }
            "type"
            | "parenthesized_expression"
            | "attribute"
            | "member_type"
            | "generic_type"
            | "union_type"
            | "constrained_type"
            | "splat_type"
            | "subscript"
            | "concatenated_string"
            | "unary_operator"
            | "binary_operator"
            | "boolean_operator"
            | "comparison_operator"
            | "not_operator"
            | "conditional_expression"
            | "pair" => {
                return Err(());
            }
            "await" if entry.named => return Err(()),
            _ => None,
        };
        if let Some((class, shape)) = normalized {
            if let Some(star) = lifted {
                let value = first.unwrap();
                self.entries[star].class = class;
                self.entries[star].shape = shape;
                self.entries[star].start = self.entries[value].start;
                self.entries[star].line = self.entries[value].line;
                self.entries[star].end = self.entries[index].end;
                self.entries[star].end_line = self.terminal_end_line;
                let mut child = self.entries[index].first;
                while let Some(i) = child {
                    if cancel.at(Checkpoint::Collector) {
                        return Err(());
                    }
                    if i != star {
                        self.entries[i].parent = Some(star);
                    }
                    child = self.entries[i].next;
                }
                self.entries[index].class = Class::Starred;
                self.entries[index].shape = Shape::Starred(star);
            } else {
                self.entries[index].class = class;
                self.entries[index].shape = shape;
            }
        }
        self.entries[index].end_line = self.terminal_end_line;
        self.schedule_calls(index, lifted, cancel)?;
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
            let valid = if self.target_mode() {
                self.valid_target(index, self.mode, cancel)
            } else {
                self.body_value(index) || (self.star_expressions && class == Class::Starred)
            };
            if !valid {
                return Err(());
            }
            return Ok(true);
        }
        Ok(false)
    }

    fn append_calls(&mut self, chain: &mut projection::Calls, index: usize) {
        let child = self.entries[index].calls;
        chain.append(child, |last, next| self.entries[last].next_call = next);
    }

    fn first_value<F: FnMut(Checkpoint) -> bool>(
        &self,
        index: usize,
        cancel: &Cancellation<F>,
    ) -> Option<usize> {
        // Each pair has exactly two validated values and one colon; no subtree
        // search or renderer alias resolution is involved here.
        let mut child = self.entries[index].first;
        while let Some(i) = child {
            if cancel.at(Checkpoint::Collector) {
                return None;
            }
            if self.entries[i].class != Class::Ignore {
                return Some(i);
            }
            child = self.entries[i].next;
        }
        None
    }

    fn schedule_calls<F: FnMut(Checkpoint) -> bool>(
        &mut self,
        index: usize,
        lifted: Option<usize>,
        cancel: &Cancellation<F>,
    ) -> Result<(), ()> {
        let kind = self.entries[index].kind;
        let mut chain = projection::Calls::default();
        if kind == "call" {
            // Splat normalization can move the renderable Call to a recycled
            // descriptor. Emit that identity once, at the raw call's visit point.
            let call = lifted.unwrap_or(index);
            chain = projection::Calls {
                first: Some(call),
                last: Some(call),
            };
        }
        let passes = match kind {
            "argument_list" | "dictionary" | "lambda_parameters" => 2,
            "conditional_expression" => 3,
            // These admitted families have helper fields in the same order as
            // their raw children (including opaque nodes and f-string fields).
            "call"
            | "lambda"
            | "default_parameter"
            | "for_in_clause"
            | "if_clause"
            | "list_comprehension"
            | "set_comprehension"
            | "dictionary_comprehension"
            | "generator_expression"
            | "pair"
            | "await"
            | "yield"
            | "named_expression"
            | "type"
            | "parenthesized_expression"
            | "attribute"
            | "member_type"
            | "generic_type"
            | "type_parameter"
            | "union_type"
            | "constrained_type"
            | "splat_type"
            | "subscript"
            | "tuple"
            | "list"
            | "set"
            | "expression_list"
            | "list_pattern"
            | "tuple_pattern"
            | "pattern_list"
            | "list_splat"
            | "list_splat_pattern"
            | "dictionary_splat"
            | "dictionary_splat_pattern"
            | "keyword_argument"
            | "binary_operator"
            | "boolean_operator"
            | "comparison_operator"
            | "not_operator"
            | "unary_operator"
            | "slice"
            | "string"
            | "concatenated_string"
            | "interpolation"
            | "format_expression"
            | "format_specifier"
            | "as_pattern"
            | "as_pattern_target" => 1,
            // Remaining admitted leaves/payload tokens cannot contain calls.
            _ if self.entries[index].first.is_none()
                || matches!(
                    kind,
                    "string_content"
                        | "escape_sequence"
                        | "escape_interpolation"
                        | "not in"
                        | "is not"
                        | "positional_separator"
                        | "keyword_separator"
                ) =>
            {
                1
            }
            _ => return Err(()),
        };
        if kind == "lambda_parameters" {
            let mut keyword = false;
            let mut child = self.entries[index].first;
            while let Some(i) = child {
                if cancel.at(Checkpoint::Collector) {
                    return Err(());
                }
                keyword |= matches!(
                    self.entries[i].kind,
                    "keyword_separator" | "list_splat_pattern"
                );
                self.entries[i].keyword_default =
                    keyword && self.entries[i].kind == "default_parameter";
                child = self.entries[i].next;
            }
        }
        for pass in 0..passes {
            let mut child = self.entries[index].first;
            let mut value = 0;
            while let Some(i) = child {
                if cancel.at(Checkpoint::Collector) {
                    return Err(());
                }
                let entry = &self.entries[i];
                let next = entry.next;
                let selected = match kind {
                    "argument_list" => {
                        usize::from(matches!(entry.class, Class::Keyword | Class::Unpack)) == pass
                    }
                    "lambda_parameters" => usize::from(!entry.keyword_default) == pass,
                    "conditional_expression" => {
                        let selected = entry.class != Class::Ignore && value == [1, 0, 2][pass];
                        value += usize::from(entry.class != Class::Ignore);
                        selected
                    }
                    "dictionary" => false,
                    _ => true,
                };
                if kind == "dictionary" {
                    if entry.kind == "pair" {
                        let key = self.first_value(i, cancel).ok_or(())?;
                        let part = if pass == 0 {
                            key
                        } else {
                            self.entries[key].next_value.ok_or(())?
                        };
                        self.append_calls(&mut chain, part);
                    } else if pass == 1 {
                        // Unpacks have no AST key, but their values still visit.
                        self.append_calls(&mut chain, i);
                    }
                } else if selected {
                    self.append_calls(&mut chain, i);
                }
                child = next;
            }
        }
        self.entries[index].calls = chain;
        Ok(())
    }

    pub(super) fn root_starred(&self) -> bool {
        self.entries
            .first()
            .is_some_and(|e| e.class == Class::Starred)
    }

    fn resolve(&self, mut i: usize) -> usize {
        while let Shape::Alias(next) = self.entries[i].shape {
            i = next;
        }
        i
    }

    fn body_value(&self, index: usize) -> bool {
        // Headers and defaults share core values, but reject calls at admission.
        // Slices, pairs and splats are context-only, not general values.
        !matches!(
            self.entries[index].class,
            Class::Slice | Class::Pair | Class::Starred | Class::Unpack
        ) && matches!(
            self.entries[self.resolve(index)].shape,
            Shape::Name
                | Shape::Attribute(..)
                | Shape::Call(..)
                | Shape::Literal
                | Shape::Unary
                | Shape::Tuple(..)
                | Shape::Subscript(..)
                | Shape::Binary(..)
                | Shape::Opaque(_)
        )
    }

    fn primary(&self, index: usize) -> bool {
        self.body_value(index)
            && matches!(
                self.entries[index].kind,
                "identifier"
                    | "none"
                    | "ellipsis"
                    | "true"
                    | "false"
                    | "integer"
                    | "float"
                    | "string"
                    | "concatenated_string"
                    | "parenthesized_expression"
                    | "tuple"
                    | "list"
                    | "set"
                    | "dictionary"
                    | "call"
                    | "attribute"
                    | "member_type"
                    | "generic_type"
                    | "subscript"
                    | "list_comprehension"
                    | "set_comprehension"
                    | "dictionary_comprehension"
                    | "generator_expression"
            )
    }

    fn type_slice(&self) -> bool {
        let Some(mut parent) = self.current else {
            return false;
        };
        if self.entries[parent].kind == "type" {
            let Some(outer) = self.entries[parent].parent else {
                return false;
            };
            parent = outer;
        }
        self.entries[parent].kind == "type_parameter"
            || (self.entries[parent].kind == "constrained_type"
                && self.entries[parent].last_value.is_some())
    }

    fn target_mode(&self) -> bool {
        matches!(
            self.mode,
            Mode::AssignmentTarget | Mode::NeutralTarget | Mode::ScalarTarget | Mode::DeleteTarget
        )
    }

    fn valid_target<F: FnMut(Checkpoint) -> bool>(
        &self,
        index: usize,
        mode: Mode,
        cancel: &Cancellation<F>,
    ) -> bool {
        if cancel.at(Checkpoint::Collector) {
            return false;
        }
        let index = self.resolve(index);
        match self.entries[index].shape {
            Shape::Name | Shape::Attribute(..) | Shape::Subscript(..) => true,
            Shape::Starred(value) if !matches!(mode, Mode::ScalarTarget | Mode::DeleteTarget) => {
                self.valid_target(value, mode, cancel)
            }
            Shape::Tuple(mut child)
                if mode != Mode::ScalarTarget && self.entries[index].kind != "set" =>
            {
                while let Some(i) = child {
                    if !self.valid_target(i, mode, cancel) {
                        return false;
                    }
                    child = self.entries[i].next_value;
                }
                true
            }
            _ => false,
        }
    }

    pub(super) fn router_guard<F: FnMut(Checkpoint) -> bool>(
        &self,
        bytes: &[u8],
        cancel: &Cancellation<F>,
    ) -> bool {
        let root = self.resolve(0);
        // Delete's ungrouped expression list is several AST targets, unlike a Tuple.
        if self.mode == Mode::DeleteTarget && self.entries[root].kind == "expression_list" {
            let Shape::Tuple(mut child) = self.entries[root].shape else {
                return false;
            };
            while let Some(i) = child {
                if cancel.at(Checkpoint::Collector) || self.router_target(i, bytes, cancel) {
                    return true;
                }
                child = self.entries[i].next_value;
            }
            false
        } else {
            self.router_target(root, bytes, cancel)
        }
    }

    fn router_target<F: FnMut(Checkpoint) -> bool>(
        &self,
        index: usize,
        bytes: &[u8],
        cancel: &Cancellation<F>,
    ) -> bool {
        let index = self.resolve(index);
        let Shape::Attribute(_, attr) = self.entries[index].shape else {
            return false;
        };
        let attr = &self.entries[attr];
        if matches!(&bytes[attr.start..attr.end], b"router" | b"prefix")
            || self
                .render_node(index, 0, &mut 80, bytes, cancel)
                .contains(".router.")
        {
            return true;
        }
        // Retain the conservative raw interior-router exclusion, but only on
        // an actual top-level Attribute, never on destructured target leaves.
        let mut current = index;
        while let Shape::Attribute(base, attr) = self.entries[current].shape {
            if cancel.at(Checkpoint::Collector) {
                return true;
            }
            let attr = &self.entries[attr];
            if &bytes[attr.start..attr.end] == b"router" {
                return true;
            }
            current = self.resolve(base);
        }
        false
    }

    pub(super) fn name_span(&self) -> Option<std::ops::Range<usize>> {
        let root = &self.entries[self.resolve(0)];
        matches!(root.shape, Shape::Name).then_some(root.start..root.end)
    }

    pub(super) fn targets<'a, F: FnMut(Checkpoint) -> bool>(
        &'a self,
        bytes: &'a [u8],
        cancel: &'a Cancellation<F>,
    ) -> impl Iterator<Item = (usize, bool)> + 'a {
        self.entries
            .iter()
            .enumerate()
            .take_while(|_| !cancel.at(Checkpoint::Collector))
            .filter_map(move |(i, e)| {
                if !matches!(e.shape, Shape::Name | Shape::Attribute(..)) {
                    return None;
                }
                let mut parent = e.parent;
                let mut destructured = false;
                while let Some(p) = parent {
                    if cancel.at(Checkpoint::Collector) {
                        return None;
                    }
                    match self.entries[p].shape {
                        Shape::Alias(_) => {}
                        Shape::Tuple(_) | Shape::Starred(_) => destructured = true,
                        _ => return None,
                    }
                    parent = self.entries[p].parent;
                }
                self.eligible(i, bytes, cancel).then_some((i, destructured))
            })
    }

    fn eligible<F: FnMut(Checkpoint) -> bool>(
        &self,
        index: usize,
        bytes: &[u8],
        cancel: &Cancellation<F>,
    ) -> bool {
        let mut current = self.resolve(index);
        if matches!(self.entries[current].shape, Shape::Name) {
            return true;
        }
        loop {
            if cancel.at(Checkpoint::Collector) {
                return false;
            }
            match self.entries[current].shape {
                Shape::Attribute(base, _) => current = self.resolve(base),
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
        let mut next = self.entries.first().and_then(|entry| entry.calls.first);
        std::iter::from_fn(move || {
            if cancel.at(Checkpoint::Collector) {
                return None;
            }
            let current = next?;
            next = self.entries[current].next_call;
            Some(current)
        })
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

    pub(super) fn render_target<F: FnMut(Checkpoint) -> bool>(
        &self,
        index: usize,
        bytes: &[u8],
        cancel: &Cancellation<F>,
    ) -> String {
        self.render_node(index, 0, &mut 80, bytes, cancel)
            .into_boxed_str()
            .into_string()
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
            Shape::Opaque(label) => append(&mut result, label),
            Shape::Slice(_) => append(&mut result, "<Slice>"),
            Shape::Starred(value) => {
                append(&mut result, "*");
                append(
                    &mut result,
                    &self.render_node(value, depth + 1, budget, bytes, cancel),
                );
            }
            Shape::Binary(a, op, b) => {
                append(
                    &mut result,
                    &self.render_node(a, depth + 1, budget, bytes, cancel),
                );
                append(&mut result, " ");
                append(&mut result, op);
                append(&mut result, " ");
                append(
                    &mut result,
                    &self.render_node(b, depth + 1, budget, bytes, cancel),
                );
            }
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
                let first = match self.entries[arguments].shape {
                    Shape::Arguments(first) => first,
                    // A direct generator is the sole positional argument. Keep
                    // its own opaque shape; no synthetic descriptor is needed.
                    Shape::Opaque("<GeneratorExp>") => Some(arguments),
                    _ => unreachable!("validated arguments"),
                };
                let mut positional = 0;
                let mut keyword = 0;
                let mut emitted = false;
                // AST groups positional arguments (including *) before keywords
                // (including **). Linked descriptors need no temporary vectors.
                for keywords in [false, true] {
                    let mut current = first;
                    while let Some(i) = current {
                        if cancel.at(Checkpoint::Collector) {
                            break;
                        }
                        let entry = &self.entries[i];
                        current = entry.next_value;
                        let is_keyword =
                            matches!(entry.shape, Shape::Keyword(..) | Shape::Unpack(..));
                        if is_keyword != keywords {
                            continue;
                        }
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
                        } else if let Shape::Unpack(value) = entry.shape {
                            append(&mut result, "**=");
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
                }
                append(&mut result, ")");
            }
            Shape::Arguments(_) | Shape::Keyword(..) | Shape::Unpack(_) => {
                unreachable!("descriptor, not expression")
            }
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

// The scanner's format-spec token accepts outer quotes as text. Check lexical
// boundaries on literal-only segments, never on a whole interpolated span.
fn literal_segment<F: FnMut(Checkpoint) -> bool>(
    text: &[u8],
    info: literals::Start,
    format: bool,
    cancel: &Cancellation<F>,
) -> bool {
    if literals::segment(text, info, format, cancel).is_err() {
        return false;
    }
    let mut at = 0;
    let width = if info.triple { 3 } else { 1 };
    while at < text.len() {
        if cancel.at(Checkpoint::Collector) {
            return false;
        }
        let b = text[at];
        if matches!(b, b'\n' | b'\r') && !info.triple {
            return false;
        }
        if b == b'\\' && text.get(at + 1).is_some_and(|b| !matches!(b, b'{' | b'}')) {
            at += if text.get(at + 1..at + 3) == Some(b"\r\n") {
                3
            } else {
                2
            };
            continue;
        }
        if b == info.quote && text.get(at..at + width) == Some(&[b, b, b][..width]) {
            return false;
        }
        if matches!(b, b'{' | b'}') {
            if format || text.get(at + 1) != Some(&b) {
                return false;
            }
            at += 1;
        }
        at += 1;
    }
    true
}

pub(super) fn string<F: FnMut(Checkpoint) -> bool>(
    text: &[u8],
    cancel: &Cancellation<F>,
) -> Option<bool> {
    match literals::string(text, cancel).ok()? {
        literals::Flavor::Text => Some(false),
        literals::Flavor::Bytes => Some(true),
        literals::Flavor::Formatted => None,
    }
}

// Keep the pattern consumer's boolean seam; numeric categories and Python's
// default decimal-integer digit policy belong to the shared lexical validator.
pub(super) fn number<F: FnMut(Checkpoint) -> bool>(text: &[u8], cancel: &Cancellation<F>) -> bool {
    literals::number(text, cancel).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn semantic_trace(text: &str) -> Vec<String> {
        facts(format!("{text}\n"), CandidateStatus::AcceptedFacts);
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(text, None).unwrap();
        assert!(!tree.root_node().has_error());
        let root = tree
            .root_node()
            .named_child(0)
            .unwrap()
            .named_child(0)
            .unwrap();
        let cancel = Cancellation {
            expired: Cell::new(false),
            check: RefCell::new(|_| false),
        };
        let mut expression = Expression::new(Mode::BodyValue, 0);
        fn lower<F: FnMut(Checkpoint) -> bool>(
            expression: &mut Expression,
            node: Node<'_>,
            bytes: &[u8],
            cancel: &Cancellation<F>,
        ) {
            assert!(expression.enter(node, bytes, cancel), "{}", node.kind());
            if node.child_count() == 0 {
                assert!(expression.terminal(node, &[], cancel));
            }
            for i in 0..node.child_count() {
                lower(expression, node.child(i).unwrap(), bytes, cancel);
            }
            assert!(expression.exit(bytes, cancel).is_ok(), "{}", node.kind());
        }
        lower(&mut expression, root, text.as_bytes(), &cancel);
        expression
            .calls(&cancel)
            .map(|index| expression.render_callee(index, text.as_bytes(), &cancel))
            .collect()
    }

    #[test]
    fn projection_semantic_calls_match_python312_nodevisitor_field_order() {
        // Expected preorders are Python 3.12 ast.NodeVisitor order, not runtime
        // evaluation order and not the final fact sort. All callees are unique.
        for (text, expected) in [
            ("outer(key=keyword(), *star(), later=last(), **unpack())",
                vec!["outer", "star", "keyword", "last", "unpack"]),
            ("factory()(key=keyword(), *star())",
                vec!["factory()", "factory", "star", "keyword"]),
            ("{key1(): value1(), **unpack(), key2(): value2()}",
                vec!["key1", "key2", "value1", "unpack", "value2"]),
            ("body() if condition() else alternate()", vec!["condition", "body", "alternate"]),
            ("lambda a=pos1(), b=pos2(), *, c=kw1(), d=kw2(): body()",
                vec!["kw1", "kw2", "pos1", "pos2", "body"]),
            ("[element() for receiver()[index()] in iterable() if guard() for other()[slot()] in second() if final()]",
                vec!["element", "receiver", "index", "iterable", "guard", "other", "slot", "second", "final"]),
            ("{key(): value() for receiver()[slot()] in iterable() if guard()}",
                vec!["key", "value", "receiver", "slot", "iterable", "guard"]),
            ("(element() for receiver()[slot()] in iterable() if guard())",
                vec!["element", "receiver", "slot", "iterable", "guard"]),
            ("{element() for receiver()[slot()] in iterable() if guard()}",
                vec!["element", "receiver", "slot", "iterable", "guard"]),
            ("f'{body() if test() else alternate():{width()}.{precision()}}'",
                vec!["test", "body", "alternate", "width", "precision"]),
            ("await left() ** right()", vec!["left", "right"]),
            ("[*factory()(key=keyword(), *star())]", vec!["factory()", "factory", "star", "keyword"]),
            ("sink(*factory()(key=keyword(), *star())[index()] if test() else alternate())",
                vec!["sink", "test", "factory()", "factory", "star", "keyword", "index", "alternate"]),
            ("sink((yield body()), (named := other()))", vec!["sink", "body", "other"]),
        ] {
            assert_eq!(semantic_trace(text), expected, "{text}");
        }
    }

    #[test]
    fn projection_semantic_calls_survive_opaque_and_all_render_caps() {
        let arguments = (0..100)
            .map(|i| format!("hidden{i}()"))
            .collect::<Vec<_>>()
            .join(", ");
        let text = format!(
            "outer([{arguments}], payload=({{key(): value()}} if test() else alternate()))"
        );
        let mut expected = vec!["outer".to_owned()];
        expected.extend((0..100).map(|i| format!("hidden{i}")));
        expected.extend(["test", "key", "value", "alternate"].map(str::to_owned));
        assert_eq!(semantic_trace(&text), expected);
        // No individual display reaches sixteen items here: this separately
        // exhausts the shared eighty-node and 512-byte renderer limits.
        let labels = (0..100)
            .map(|i| format!("hidden{i}_{}", "x".repeat(48)))
            .collect::<Vec<_>>();
        let groups = (0..10)
            .map(|group| {
                let values = (0..10)
                    .map(|item| format!("{}()", labels[group * 10 + item]))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{values}]")
            })
            .collect::<Vec<_>>()
            .join(", ");
        let mut grid_expected = vec!["outer".to_owned()];
        grid_expected.extend(labels);
        assert_eq!(semantic_trace(&format!("outer([{groups}])")), grid_expected);
        let nested = format!("{}last(){}", "wrapper(".repeat(20), ")".repeat(20));
        let trace = semantic_trace(&nested);
        assert_eq!(trace.len(), 21);
        assert_eq!(trace.last().unwrap(), "last");
    }

    #[cfg(unix)]
    #[test]
    #[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
    fn projection_semantic_call_trace_matches_trusted_ast() {
        let python = std::path::PathBuf::from(
            std::env::var_os("IDEOMANCER_TEST_PYTHON")
                .expect("explicit trusted Python selection is required"),
        );
        assert!(python.is_absolute() && python.is_file());
        let baseline = crate::analysis::extract_captured(&python, &[]).unwrap();
        eprintln!("Semantic call-order oracle: {}", baseline.python_version);
        let mut inputs: Vec<String> = [
            "outer(key=keyword(), *star(), later=last(), **unpack())",
            "factory()(key=keyword(), *star())",
            "{key1(): value1(), **unpack(), key2(): value2()}",
            "{**first(), **second()}",
            "body() if condition() else alternate()",
            "lambda a=pos1(), b=pos2(), *, c=kw1(), d=kw2(): body()",
            "lambda a=pos1(), /, b=pos2(), *args, c=kw1(), d=kw2(), **kwargs: body()",
            "[element() for receiver()[index()] in iterable() if guard() for other()[slot()] in second() if final()]",
            "{key(): value() for receiver()[slot()] in iterable() if guard()}",
            "(element() for receiver()[slot()] in iterable() if guard())",
            "{element() for receiver()[slot()] in iterable() if guard()}",
            "f'{body() if test() else alternate():{width()}.{precision()}}'",
            "await left() ** right()",
            "[*factory()(key=keyword(), *star())]",
            "sink(*factory()(key=keyword(), *star())[index()] if test() else alternate())",
            "sink((yield body()), (named := other()))",
        ].into_iter().map(str::to_owned).collect();
        let hidden = (0..100)
            .map(|i| format!("hidden{i}()"))
            .collect::<Vec<_>>()
            .join(", ");
        inputs.push(format!(
            "outer([{hidden}], payload=({{key(): value()}} if test() else alternate()))"
        ));
        inputs.push(format!("{}last(){}", "wrapper(".repeat(20), ")".repeat(20)));
        // ast.unparse is used only for these synthetic simple callee names and
        // chained calls. The traversal itself is the stdlib NodeVisitor; no
        // candidate event order is copied into the oracle or hidden by sorting.
        let script = r#"
import ast, json, sys
assert sys.version_info >= (3, 12)
class Calls(ast.NodeVisitor):
    def __init__(self):
        self.names = []
    def visit_Call(self, node):
        self.names.append(ast.unparse(node.func))
        self.generic_visit(node)
traces = []
for text in json.load(sys.stdin):
    visitor = Calls()
    visitor.visit(ast.parse(text, type_comments=True))
    traces.append(visitor.names)
json.dump(traces, sys.stdout, ensure_ascii=True)
"#;
        let neutral = tempfile::tempdir().unwrap();
        let mut command = std::process::Command::new(python);
        command
            .args(["-I", "-S", "-c", script])
            .current_dir(neutral.path())
            .env_clear()
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let output =
            crate::analysis::run_bounded(command, serde_json::to_vec(&inputs).unwrap(), "Python")
                .unwrap();
        let expected: Vec<Vec<String>> = serde_json::from_slice(&output).unwrap();
        assert_eq!(inputs.len(), expected.len());
        for (text, expected) in inputs.iter().zip(expected) {
            assert_eq!(semantic_trace(text), expected, "{text}");
        }
    }

    #[test]
    fn projection_early_metadata_finalizes_spans_ids_and_rolls_back() {
        let text = "def f():\n    probe()\n    # not definition evidence\n";
        let result = facts(text, CandidateStatus::AcceptedFacts);
        let declaration = result.iter().find(|fact| fact.kind == "function").unwrap();
        assert_eq!((declaration.line, declaration.end_line), (1, 2));
        assert_eq!(
            declaration.id,
            format!(
                "fact_{:x}",
                Sha256::digest(br#"["headers.py", "function", 1, 2, "headers.f", "f"]"#)
            )
        );
        assert!(result.iter().all(|fact| fact.end_line != usize::MAX));
        let sources = [source("from arbitrary import eval as probe\ndef f():\n    probe()\n    from nested import exec as probe\n    probe()\n")];
        for limits in [
            FactLimits {
                count: 3,
                ..FactLimits::default()
            },
            FactLimits {
                working: 1024,
                ..FactLimits::default()
            },
            FactLimits {
                warnings: 1,
                ..FactLimits::default()
            },
        ] {
            let report = extract(&sources, Limits::default(), limits, |_| false).unwrap();
            assert_eq!(report.files[0].status, CandidateStatus::ResourceLimit);
            assert_eq!(report.facts.len(), 1);
            assert!(report.diagnostics.is_empty());
        }
    }

    #[test]
    fn projection_definition_finalization_cancels_every_fact_id_checkpoint() {
        let sources = [source("def f():\n    def g(): pass\n")];
        let mut count = 0;
        let result = extract(
            &sources,
            Limits::default(),
            FactLimits::default(),
            |phase| {
                count += usize::from(phase == Checkpoint::FactId);
                false
            },
        )
        .unwrap();
        assert_eq!(result.files[0].status, CandidateStatus::AcceptedFacts);
        assert!(count > 0);
        for stop in 1..=count {
            let mut seen = 0;
            let report = extract(
                &sources,
                Limits::default(),
                FactLimits::default(),
                |phase| {
                    seen += usize::from(phase == Checkpoint::FactId);
                    phase == Checkpoint::FactId && seen == stop
                },
            )
            .unwrap();
            assert_eq!(report.facts.len(), 1, "checkpoint {stop}");
            assert_eq!(report.facts[0].kind, "module");
            assert_eq!(report.files[0].status, CandidateStatus::ResourceLimit);
            assert_eq!(
                report.files[0].diagnostic,
                Some(CandidateDiagnostic::Parse(DiagnosticCode::Deadline))
            );
            assert!(report.diagnostics.is_empty());
        }
    }

    fn source(text: impl AsRef<[u8]>) -> Source {
        Source {
            path: "headers.py".into(),
            module: "headers".into(),
            bytes: text.as_ref().to_vec(),
            hex: "invalid transport".into(),
        }
    }

    fn facts(text: impl AsRef<[u8]>, status: CandidateStatus) -> Vec<CandidateFact> {
        let sources = [source(text)];
        let report = extract_fact_candidates(&sources).unwrap();
        assert_eq!(
            report.files[0].status,
            status,
            "{report:?}\nsource: {:?}",
            String::from_utf8_lossy(&sources[0].bytes)
        );
        report.facts
    }

    #[test]
    fn policy14_broad_call_free_headers() {
        // G4: this Python 3.12 AST-positive spelling hits a pinned-grammar
        // ERROR in annotations; grouping below preserves the semantic control.
        let ungrouped = facts(
            "class C(Box[A | B][start:stop].member):\n field: Box[A | B][start:stop].member\ndef f(x: Box[A | B][start:stop].member) -> Box[A | B][start:stop].member: pass\n",
            CandidateStatus::SyntaxError,
        );
        assert_eq!(ungrouped.len(), 1);
        for (value, rendered) in [
            ("A | B", "A BitOr B"),
            ("[A, {B: C}]", "[A, <Dict>]"),
            ("lambda x=A: x", "<Lambda>"),
            ("[x for x in items if ready]", "<ListComp>"),
            (
                "(Box[A | B][start:stop].member)",
                "Box[A BitOr B][<Slice>].member",
            ),
            ("(await item ** power)", "<Await> Pow power"),
        ] {
            let result = facts(
                format!("class C({value}):\n field: {value}\ndef f(x: {value}) -> {value}: pass\n"),
                CandidateStatus::AcceptedFacts,
            );
            for kind in ["base", "parameter_annotation", "return_annotation"] {
                assert!(result.iter().any(|f| f.kind == kind && f.value == rendered));
            }
            assert!(result.iter().all(|f| f.kind != "call"));
        }
    }

    #[test]
    fn policy14_headers_reject_calls_even_under_opaque_values() {
        for value in [
            "A | hidden()",
            "[hidden()]",
            "lambda x=hidden(): x",
            "lambda: hidden()",
            "[x for x in hidden()]",
            "{x: hidden() for x in items}",
            "Box[hidden():stop]",
            "(await hidden())",
            "(yield hidden())",
            "(named := hidden())",
        ] {
            for text in [
                format!("class C({value}): pass\n"),
                format!("class C(metaclass={value}): pass\n"),
                format!("def f(x: {value}): pass\n"),
                format!("def f() -> {value}: pass\n"),
                format!("item: {value}\n"),
                format!("def f(x={value}): pass\n"),
                format!("def f[T: {value}](): pass\n"),
            ] {
                let result = facts(text, CandidateStatus::UnsupportedSyntax);
                assert_eq!(result.len(), 1);
                assert_eq!(result[0].kind, "module");
            }
        }
    }

    #[test]
    fn policy14_class_starred_bases_and_neutral_header_values() {
        let result = facts(
            concat!(
                "class C[T: A | B](*bases, Box[A | B], metaclass=[Meta], **options): pass\n",
                "def f[T:\n A | B](x: Box[*Ts]) -> Box[A:B:C]: pass\n",
            ),
            CandidateStatus::AcceptedFacts,
        );
        let bases: Vec<_> = result
            .iter()
            .filter(|f| f.kind == "base")
            .map(|f| f.value.as_str())
            .collect();
        assert_eq!(bases, ["*bases", "Box[A BitOr B]"]);
        assert!(result
            .iter()
            .any(|f| f.kind == "parameter_annotation" && f.value == "Box[[*Ts]]"));
        assert!(result
            .iter()
            .any(|f| f.kind == "return_annotation" && f.value == "Box[<Slice>]"));
        assert!(result.iter().all(|f| f.kind != "call"));
        for text in [
            "def f(x: *Ts): pass\n",
            "item: *Ts\n",
            "def f() -> *Ts: pass\n",
        ] {
            let result = facts(text, CandidateStatus::UnsupportedSyntax);
            assert_eq!(result.len(), 1);
        }
    }

    #[test]
    fn policy14_observed_continuations_tabs_and_semantic_spans() {
        for newline in ["\n", "\r\n"] {
            let text =
                format!("def f(x:\tA \\{newline}\t| B) -> A \\{newline}\t| B: pass{newline}");
            let result = facts(text, CandidateStatus::AcceptedFacts);
            for (kind, span) in [
                ("parameter_annotation", (1, 2)),
                ("return_annotation", (2, 3)),
            ] {
                let fact = result.iter().find(|f| f.kind == kind).unwrap();
                assert_eq!(fact.value, "A BitOr B");
                assert_eq!((fact.line, fact.end_line), span);
            }
        }
        let result = facts("def f(x: A \\\n): pass\n", CandidateStatus::AcceptedFacts);
        let annotation = result
            .iter()
            .find(|f| f.kind == "parameter_annotation")
            .unwrap();
        assert_eq!((annotation.line, annotation.end_line), (1, 1));
    }

    #[test]
    fn policy14_invalid_join_payloads_and_unbracketed_newlines() {
        for text in [
            "def f() -> A \\\0 | B: pass\n",
            "def f() -> A \\ \n | B: pass\n",
            "def f() -> A\n | B: pass\n",
            "item: A\n | B\n",
        ] {
            let sources = [source(text)];
            let report = extract_fact_candidates(&sources).unwrap();
            assert!(
                matches!(
                    report.files[0].status,
                    CandidateStatus::UnsupportedSyntax | CandidateStatus::SyntaxError
                ),
                "{report:?}"
            );
            assert_eq!(report.facts.len(), 1);
        }
    }

    #[test]
    fn policy14_header_rendering_and_work_are_bounded() {
        let value = std::iter::repeat_n("Member", 48)
            .collect::<Vec<_>>()
            .join(" | ");
        let text = format!("def f(x: {value}): pass\n");
        let result = facts(&text, CandidateStatus::AcceptedFacts);
        let annotation = result
            .iter()
            .find(|f| f.kind == "parameter_annotation")
            .unwrap();
        assert!(annotation.value.len() <= 512);
        assert!(annotation.value.contains("..."));
        assert_eq!((annotation.line, annotation.end_line), (1, 1));
        let sources = [source(text)];
        let report = extract(
            &sources,
            Limits::default(),
            FactLimits {
                working: 1024,
                ..FactLimits::default()
            },
            |_| false,
        )
        .unwrap();
        assert_eq!(report.files[0].status, CandidateStatus::ResourceLimit);
        assert_eq!(report.facts.len(), 1);
    }
}
