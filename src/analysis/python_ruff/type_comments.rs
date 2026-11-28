//! CPython 3.12 `ast.parse(type_comments=True)` attachment compatibility.
//!
//! This is a pre-semantic pass over an already diagnostic-free Ruff module. It
//! never parses comment payloads, retains source text, or walks expression trees.
//! The caller charges through `budget`, and must drop the plan before refunding
//! `working_charge` (also on rollback). Charges include temporary event storage;
//! deliberately retaining that high-water charge avoids a refund callback API.
//! Integrators apply Assign to assignment spans, Parameter to both arg-backed
//! fact kinds, and Statement to all other statement/declaration spans before
//! using their own LineIndex. This does not correct decorator-inclusive starts.
//! Actual comments inside f-string replacement fields have a finite exclusion
//! until CPython's context-dependent PEG error locations are established.
//! Parameter-internal comments with a lexically active complex-expression prefix
//! are also excluded, rather than guessing PEG invalid-rule locations.
use ruff_python_ast::{ExceptHandler, Mod, ModModule, Parameter, Stmt};
use ruff_python_parser::{Parsed, Token, TokenKind, Tokens};
use ruff_text_size::{Ranged, TextRange, TextSize};
use std::mem::size_of;

const COMPLEX_PARAMETER_COMMENT: &str =
    "actual type comment in parameter expression with unresolved complex prefix";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Error {
    Syntax { line: usize },
    WorkingLimit,
    Cancelled,
    UnsupportedCategory(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum NodeKind {
    Assign,
    Parameter,
    /// Any other statement, including function/class declarations. A final
    /// assignment's comment extends every enclosing suite's end as well.
    Statement,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Limits {
    pub(super) nodes: usize,
    pub(super) depth: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            nodes: 200_000,
            depth: 256,
        }
    }
}

#[derive(Debug)]
pub(super) struct CommentPlan {
    overrides: Vec<Override>,
    /// Total successful charge callbacks, including scratch high-water storage.
    pub(super) working_charge: usize,
    /// Token, source-byte and compatibility-node checkpoints, not semantic nodes.
    pub(super) normalization_work: usize,
    pub(super) compatibility_nodes: usize,
    pub(super) max_depth: usize,
    pub(super) ignore_count: usize,
}

impl CommentPlan {
    /// Use Parameter for both parameter and parameter_annotation fact spans.
    pub(super) fn corrected_range(&self, kind: NodeKind, raw: TextRange) -> TextRange {
        let key = (kind, raw.start(), raw.end());
        if let Ok(index) = self
            .overrides
            .binary_search_by_key(&key, |item| (item.kind, item.raw.start(), item.raw.end()))
        {
            return self.overrides[index].corrected;
        }
        if kind == NodeKind::Statement {
            let count = self
                .overrides
                .partition_point(|item| item.kind == NodeKind::Assign);
            // Assign statements cannot nest inside one another, so their ends
            // are ordered just like their starts. This avoids per-ancestor data.
            if let Ok(index) =
                self.overrides[..count].binary_search_by_key(&raw.end(), |item| item.raw.end())
            {
                return TextRange::new(raw.start(), self.overrides[index].corrected.end());
            }
        }
        raw
    }
}

#[derive(Debug)]
struct Override {
    kind: NodeKind,
    raw: TextRange,
    corrected: TextRange,
}

struct Event {
    token: usize,
    attached: bool,
}

/// Frozen integration API. `check() == true` means cancellation, as in the
/// semantic driver. Only the module callback with BOTH diagnostic lists empty
/// may call this function, with the exact UTF-8 text view Ruff parsed. Failure
/// emits no facts; caller owns transactional
/// rollback and the budget scope containing all successful charges.
pub(super) fn analyze(
    text: &str,
    parsed: &Parsed<Mod>,
    budget: &mut impl FnMut(usize) -> Result<(), Error>,
    check: &mut dyn FnMut() -> bool,
) -> Result<CommentPlan, Error> {
    analyze_with_limits(text, parsed, budget, check, Limits::default())
}

pub(super) fn analyze_with_limits(
    text: &str,
    parsed: &Parsed<Mod>,
    budget: &mut impl FnMut(usize) -> Result<(), Error>,
    check: &mut dyn FnMut() -> bool,
    limits: Limits,
) -> Result<CommentPlan, Error> {
    if !parsed.has_no_syntax_errors() {
        return Err(Error::UnsupportedCategory(
            "type-comment pass requires validated AST",
        ));
    }
    let Mod::Module(module) = parsed.syntax() else {
        return Err(Error::UnsupportedCategory("non-module type-comment input"));
    };
    analyze_module_with_limits(
        text.as_bytes(),
        module,
        parsed.tokens(),
        budget,
        check,
        limits,
    )
}

/// Byte-pipeline admission only: ordinary comments are opaque, but CPython's
/// type/ignore payload semantics for malformed UTF-8 are not established here.
/// Call only with tokens from the same diagnostic-free accepted byte module.
pub(super) fn check_byte_comment_payloads(
    bytes: &[u8],
    tokens: &Tokens,
    check: &mut dyn FnMut() -> bool,
) -> Result<(), Error> {
    for token in tokens {
        if check() {
            return Err(Error::Cancelled);
        }
        if token.kind() != TokenKind::Comment {
            continue;
        }
        let mut tail = bytes
            .get(token.start().to_usize()..token.end().to_usize())
            .and_then(|raw| raw.strip_prefix(b"#"))
            .ok_or(Error::UnsupportedCategory("invalid byte comment range"))?;
        while matches!(tail.first(), Some(b' ' | b'\t')) {
            if check() {
                return Err(Error::Cancelled);
            }
            tail = &tail[1..];
        }
        if let Some(payload) = tail.strip_prefix(b"type:") {
            // Checkpoints bound cooperative work around the local UTF-8 view;
            // validation itself is bounded by the caller's source-size limit.
            for _ in payload.chunks(256) {
                if check() {
                    return Err(Error::Cancelled);
                }
            }
            if std::str::from_utf8(payload).is_err() {
                return Err(Error::UnsupportedCategory(
                    "malformed UTF-8 type-comment payload",
                ));
            }
        }
    }
    if check() {
        return Err(Error::Cancelled);
    }
    Ok(())
}

/// Private shared pass, not an engine API. The caller must supply the borrowed
/// module and tokens from the same source after BOTH diagnostic channels are
/// empty. Byte callers separately exclude malformed type/ignore payloads before
/// this pass; ordinary comments remain opaque. No payload parsing or reparse.
pub(super) fn analyze_module_with_limits(
    bytes: &[u8],
    module: &ModModule,
    tokens: &Tokens,
    budget: &mut impl FnMut(usize) -> Result<(), Error>,
    check: &mut dyn FnMut() -> bool,
    limits: Limits,
) -> Result<CommentPlan, Error> {
    let mut pass = Pass {
        bytes,
        tokens,
        events: Vec::new(),
        budget,
        check,
        limits,
        plan: CommentPlan {
            overrides: Vec::new(),
            working_charge: 0,
            normalization_work: 0,
            compatibility_nodes: 0,
            max_depth: 0,
            ignore_count: 0,
        },
    };
    // Count before reserving: every push below is covered, without geometric
    // Vec growth or an uncharged second copy of the token/source buffers.
    let mut count = 0usize;
    let tokens: &[Token] = tokens;
    for token in tokens {
        pass.tick()?;
        if token.kind() == TokenKind::Comment && pass.comment_kind(token.range())? == Some(false) {
            count = count.checked_add(1).ok_or(Error::WorkingLimit)?;
        }
    }
    pass.reserve(count)?;
    let mut fstrings = 0usize;
    for (index, token) in tokens.iter().enumerate() {
        pass.tick()?;
        match token.kind() {
            TokenKind::FStringStart => fstrings += 1,
            TokenKind::FStringEnd => fstrings = fstrings.saturating_sub(1),
            _ => {}
        }
        if token.kind() == TokenKind::Comment {
            match pass.comment_kind(token.range())? {
                Some(true) => pass.plan.ignore_count += 1,
                Some(false) => {
                    // CPython's PEG error location can move to an earlier
                    // replacement-expression delimiter, not this comment line.
                    // Ignore markers here are proven and remain admitted.
                    if fstrings != 0 {
                        return Err(Error::UnsupportedCategory(
                            "actual type comment in f-string replacement field",
                        ));
                    }
                    pass.events.push(Event {
                        token: index,
                        attached: false,
                    });
                }
                None => {}
            }
        }
    }
    if !pass.events.is_empty() {
        pass.body(&module.body, 1)?;
        for index in 0..pass.events.len() {
            pass.tick()?;
            if !pass.events[index].attached {
                return Err(pass.syntax(pass.tokens[pass.events[index].token].start())?);
            }
        }
    }
    pass.tick()?;
    pass.plan
        .overrides
        .sort_unstable_by_key(|item| (item.kind, item.raw.start(), item.raw.end()));
    Ok(pass.plan)
}

struct Pass<'a, 'b, B> {
    bytes: &'a [u8],
    tokens: &'a [Token],
    events: Vec<Event>,
    budget: &'b mut B,
    check: &'b mut dyn FnMut() -> bool,
    limits: Limits,
    plan: CommentPlan,
}

impl<B: FnMut(usize) -> Result<(), Error>> Pass<'_, '_, B> {
    fn tick(&mut self) -> Result<(), Error> {
        if (self.check)() {
            return Err(Error::Cancelled);
        }
        self.plan.normalization_work = self
            .plan
            .normalization_work
            .checked_add(1)
            .ok_or(Error::WorkingLimit)?;
        Ok(())
    }

    fn node(&mut self, depth: usize) -> Result<(), Error> {
        self.tick()?;
        if depth > self.limits.depth || self.plan.compatibility_nodes >= self.limits.nodes {
            return Err(Error::WorkingLimit);
        }
        self.plan.compatibility_nodes += 1;
        self.plan.max_depth = self.plan.max_depth.max(depth);
        Ok(())
    }

    fn reserve(&mut self, count: usize) -> Result<(), Error> {
        let bytes = count
            .checked_mul(size_of::<Event>() + size_of::<Override>())
            .ok_or(Error::WorkingLimit)?;
        (self.budget)(bytes)?;
        self.plan.working_charge = bytes;
        self.events
            .try_reserve_exact(count)
            .map_err(|_| Error::WorkingLimit)?;
        self.plan
            .overrides
            .try_reserve_exact(count)
            .map_err(|_| Error::WorkingLimit)?;
        Ok(())
    }

    /// None = ordinary, true = TYPE_IGNORE, false = TYPE_COMMENT. CPython's
    /// suffix boundary is ASCII alphanumeric OR any non-ASCII byte, not XID.
    fn comment_kind(&mut self, range: TextRange) -> Result<Option<bool>, Error> {
        let bytes = self.bytes;
        let mut start = range.start().to_usize() + 1;
        let end = range.end().to_usize();
        while start < end && matches!(bytes[start], b' ' | b'\t') {
            self.tick()?;
            start += 1;
        }
        if !bytes[start..end].starts_with(b"type:") {
            return Ok(None);
        }
        start += 5;
        while start < end && matches!(bytes[start], b' ' | b'\t') {
            self.tick()?;
            start += 1;
        }
        let tail = &bytes[start..end];
        let ignore = tail.starts_with(b"ignore")
            && tail
                .get(6)
                .is_none_or(|byte| !byte.is_ascii_alphanumeric() && byte.is_ascii());
        Ok(Some(ignore))
    }

    fn syntax(&mut self, at: TextSize) -> Result<Error, Error> {
        // Attachments before an error are processed in source order. An earlier
        // unclaimed TYPE_COMMENT wins over a later contextual grammar failure.
        let mut at = at;
        for index in 0..self.events.len() {
            self.tick()?;
            let event = &self.events[index];
            let start = self.tokens[event.token].start();
            if start >= at {
                break;
            }
            if !event.attached {
                at = start;
                break;
            }
        }
        let mut line = 1;
        let mut cr = false;
        for byte in &self.bytes[..at.to_usize()] {
            self.tick()?;
            if *byte == b'\r' || (*byte == b'\n' && !cr) {
                line += 1;
            }
            cr = *byte == b'\r';
        }
        Ok(Error::Syntax { line })
    }

    fn after(&self, end: TextSize) -> usize {
        self.tokens.partition_point(|token| token.end() <= end)
    }

    fn event_at(&self, token: usize) -> Option<usize> {
        self.events
            .binary_search_by_key(&token, |event| event.token)
            .ok()
    }

    // Return the next grammar token, retaining TYPE_COMMENT but filtering
    // ordinary/ignore comments. Parameter slots additionally ignore physical NL.
    fn next(&mut self, mut index: usize, parameter: bool) -> Result<usize, Error> {
        while let Some(token) = self.tokens.get(index).copied() {
            self.tick()?;
            match token.kind() {
                TokenKind::Comment if self.event_at(index).is_none() => {}
                TokenKind::NonLogicalNewline if parameter => {}
                _ => break,
            }
            index += 1;
        }
        Ok(index)
    }

    fn attach(&mut self, event: usize) {
        self.events[event].attached = true;
    }

    fn assignment(&mut self, raw: TextRange) -> Result<(), Error> {
        let index = self.next(self.after(raw.end()), false)?;
        if let Some(event) = self.event_at(index) {
            self.attach(event);
            self.plan.overrides.push(Override {
                kind: NodeKind::Assign,
                raw,
                corrected: TextRange::new(raw.start(), self.tokens[index].end()),
            });
        }
        Ok(())
    }

    fn parameter(
        &mut self,
        parameter: &Parameter,
        end: TextSize,
        depth: usize,
    ) -> Result<(), Error> {
        self.node(depth)?;
        self.parameter_internal(parameter, end)?;
        let mut index = self.next(self.after(end), true)?;
        let comma = self
            .tokens
            .get(index)
            .is_some_and(|token| token.kind() == TokenKind::Comma);
        if comma {
            index = self.next(index + 1, true)?;
        }
        if let Some(event) = self.event_at(index) {
            self.attach(event);
            let following = self.next(index + 1, true)?;
            if !comma
                && self
                    .tokens
                    .get(following)
                    .is_none_or(|token| token.kind() != TokenKind::Rpar)
            {
                return Err(self.syntax(
                    self.tokens
                        .get(following)
                        .map_or(self.tokens[index].end(), Ranged::start),
                )?);
            }
            // CPython keeps arg's original name/annotation extent, not the
            // comment or default extent. The kind is shared by both fact uses.
            let raw = parameter.range();
            self.plan.overrides.push(Override {
                kind: NodeKind::Parameter,
                raw,
                corrected: TextRange::new(parameter.name.start(), raw.end()),
            });
        }
        Ok(())
    }

    fn parameter_internal(&mut self, parameter: &Parameter, end: TextSize) -> Result<(), Error> {
        let event = self
            .events
            .partition_point(|event| self.tokens[event.token].start() < parameter.name.end());
        let Some(comment) = self.events.get(event).map(|event| self.tokens[event.token]) else {
            return Ok(());
        };
        if comment.start() >= end {
            return Ok(());
        }
        // Inspect only the expression containing the first internal comment.
        // The annotation preceding a default has already completed successfully.
        let start = if comment.start() >= parameter.end() {
            parameter.end()
        } else {
            parameter.name.end()
        };
        let mut nesting = 0usize;
        // A lambda/if/else/for/async/yield token remains active until its
        // containing delimiter closes (or this whole expression ends). Keeping
        // only the shallowest such depth is conservative: no PEG completeness
        // claim is made for these productions. Closed subexpressions and opaque
        // f-strings do not contaminate an otherwise ordinary prefix.
        let mut complex_depth: Option<usize> = None;
        let mut fstrings = 0usize;
        let mut previous = TokenKind::Name;
        for index in self.after(start)..self.events[event].token {
            self.tick()?;
            let kind = self.tokens[index].kind();
            match kind {
                TokenKind::FStringStart => {
                    fstrings += 1;
                    continue;
                }
                TokenKind::FStringEnd => {
                    fstrings -= 1;
                    if fstrings == 0 {
                        previous = kind;
                    }
                    continue;
                }
                _ if fstrings != 0 => continue,
                TokenKind::Comment | TokenKind::NonLogicalNewline => continue,
                TokenKind::Lpar | TokenKind::Lsqb | TokenKind::Lbrace => nesting += 1,
                TokenKind::Rpar | TokenKind::Rsqb | TokenKind::Rbrace => {
                    nesting -= 1;
                    if complex_depth.is_some_and(|depth| nesting < depth) {
                        complex_depth = None;
                    }
                }
                TokenKind::Lambda
                | TokenKind::If
                | TokenKind::Else
                | TokenKind::For
                | TokenKind::Async
                | TokenKind::Yield => {
                    complex_depth = Some(complex_depth.map_or(nesting, |depth| depth.min(nesting)));
                }
                _ => {}
            }
            previous = kind;
        }
        if complex_depth.is_some() {
            // Preserve an earlier ordinary grammar failure instead of letting a
            // later finite exclusion hide it. No payloads or ASTs are reparsed.
            for prior in 0..event {
                self.tick()?;
                if !self.events[prior].attached {
                    return Err(self.syntax(self.tokens[self.events[prior].token].start())?);
                }
            }
            return Err(Error::UnsupportedCategory(COMPLEX_PARAMETER_COMMENT));
        }
        // In the remaining ordinary expression grammar, a terminal token at
        // parameter depth zero completes a prefix. CPython consumes TYPE_COMMENT
        // there, then fails the required ')' lookahead at the following token.
        let at = if nesting == 0
            && matches!(
                previous,
                TokenKind::Name
                    | TokenKind::Case
                    | TokenKind::Match
                    | TokenKind::Type
                    | TokenKind::Int
                    | TokenKind::Float
                    | TokenKind::Complex
                    | TokenKind::String
                    | TokenKind::FStringEnd
                    | TokenKind::True
                    | TokenKind::False
                    | TokenKind::None
                    | TokenKind::Ellipsis
                    | TokenKind::Rpar
                    | TokenKind::Rsqb
                    | TokenKind::Rbrace
            ) {
            self.attach(event);
            let following = self.next(self.events[event].token + 1, true)?;
            self.tokens
                .get(following)
                .map_or(comment.end(), Ranged::start)
        } else {
            comment.start()
        };
        Err(self.syntax(at)?)
    }

    fn header(&mut self, body: &[Stmt], function: bool) -> Result<(), Error> {
        let Some(first) = body.first() else {
            return Ok(());
        };
        let body_start = first.start();
        let mut index = self
            .tokens
            .partition_point(|token| token.start() < body_start);
        // The final non-trivia token before the first statement is the suite's
        // colon. This also handles decorators on that first statement.
        while index > 0 {
            self.tick()?;
            index -= 1;
            match self.tokens[index].kind() {
                TokenKind::Comment
                | TokenKind::Newline
                | TokenKind::NonLogicalNewline
                | TokenKind::Indent
                | TokenKind::Dedent => {}
                TokenKind::Colon => break,
                _ => return Err(Error::UnsupportedCategory("suite boundary")),
            }
        }
        let colon = index;
        index += 1;
        let mut newline = false;
        let mut inline = false;
        let mut standalone = false;
        while index < self.tokens.len() && self.tokens[index].start() < body_start {
            self.tick()?;
            let kind = self.tokens[index].kind();
            if matches!(kind, TokenKind::Newline | TokenKind::NonLogicalNewline) {
                newline = true;
            }
            if let Some(event) = self.event_at(index) {
                if !newline {
                    self.attach(event);
                    inline = true;
                } else if function && !standalone {
                    self.attach(event);
                    standalone = true;
                } else {
                    return Err(self.syntax(self.tokens[index].start())?);
                }
            }
            index += 1;
        }
        if inline && standalone {
            return Err(self.syntax(body_start)?);
        }
        // Colon is used only as an attachment boundary; never change declaration
        // ranges (Ruff includes decorators, corrected separately by the driver).
        debug_assert_eq!(self.tokens[colon].kind(), TokenKind::Colon);
        Ok(())
    }

    fn body(&mut self, body: &[Stmt], depth: usize) -> Result<(), Error> {
        for stmt in body {
            self.node(depth)?;
            match stmt {
                Stmt::Assign(assign) => self.assignment(assign.range())?,
                Stmt::FunctionDef(function) => {
                    self.node(depth + 1)?; // arguments wrapper
                    for arg in function
                        .parameters
                        .posonlyargs
                        .iter()
                        .chain(&function.parameters.args)
                    {
                        self.parameter(&arg.parameter, arg.end(), depth + 2)?;
                    }
                    if let Some(arg) = &function.parameters.vararg {
                        self.parameter(arg, arg.end(), depth + 2)?;
                    }
                    for arg in &function.parameters.kwonlyargs {
                        self.parameter(&arg.parameter, arg.end(), depth + 2)?;
                    }
                    if let Some(arg) = &function.parameters.kwarg {
                        self.parameter(arg, arg.end(), depth + 2)?;
                    }
                    self.header(&function.body, true)?;
                    self.body(&function.body, depth + 1)?;
                }
                Stmt::ClassDef(class) => self.body(&class.body, depth + 1)?,
                Stmt::For(for_stmt) => {
                    self.header(&for_stmt.body, false)?;
                    self.body(&for_stmt.body, depth + 1)?;
                    self.body(&for_stmt.orelse, depth + 1)?;
                }
                Stmt::With(with_stmt) => {
                    self.header(&with_stmt.body, false)?;
                    self.body(&with_stmt.body, depth + 1)?;
                }
                Stmt::While(while_stmt) => {
                    self.body(&while_stmt.body, depth + 1)?;
                    self.body(&while_stmt.orelse, depth + 1)?;
                }
                Stmt::If(if_stmt) => {
                    self.body(&if_stmt.body, depth + 1)?;
                    for clause in &if_stmt.elif_else_clauses {
                        self.node(depth + 1)?;
                        self.body(&clause.body, depth + 2)?;
                    }
                }
                Stmt::Try(try_stmt) => {
                    self.body(&try_stmt.body, depth + 1)?;
                    for handler in &try_stmt.handlers {
                        self.node(depth + 1)?;
                        let ExceptHandler::ExceptHandler(handler) = handler;
                        self.body(&handler.body, depth + 2)?;
                    }
                    self.body(&try_stmt.orelse, depth + 1)?;
                    self.body(&try_stmt.finalbody, depth + 1)?;
                }
                Stmt::Match(match_stmt) => {
                    for case in &match_stmt.cases {
                        self.node(depth + 1)?;
                        self.body(&case.body, depth + 2)?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruff_python_ast::PythonVersion;
    use ruff_python_parser::{parse_unchecked, Mode, ParseOptions};

    fn parse(text: &str) -> Parsed<Mod> {
        let parsed = parse_unchecked(
            text,
            ParseOptions::from(Mode::Module)
                .with_target_version(PythonVersion::PY312)
                .with_ast_only(true),
        );
        assert!(
            parsed.has_no_syntax_errors(),
            "synthetic Ruff fixture must be diagnostic-free"
        );
        parsed
    }

    fn plan(text: &str) -> Result<CommentPlan, Error> {
        analyze(text, &parse(text), &mut |_| Ok(()), &mut || false)
    }

    #[test]
    fn borrowed_module_pass_preserves_text_plans_and_errors() {
        let mut cases = parameter_expression_cases();
        cases.extend(complex_parameter_cases().into_iter().map(|text| (text, 0)));
        cases.extend([
            ("x=1 # type: A\n".to_owned(), 0),
            ("# type: ignore[unit]\nx=1\n".to_owned(), 0),
            ("def f():\n x=1 # type:A\n".to_owned(), 0),
            ("pass # type:A\n".to_owned(), 0),
        ]);
        for (text, _) in cases {
            let parsed = parse(&text);
            let Mod::Module(module) = parsed.syntax() else {
                panic!()
            };
            let expected = analyze(&text, &parsed, &mut |_| Ok(()), &mut || false);
            let actual = analyze_module_with_limits(
                text.as_bytes(),
                module,
                parsed.tokens(),
                &mut |_| Ok(()),
                &mut || false,
                Limits::default(),
            );
            match (actual, expected) {
                (Ok(actual), Ok(expected)) => {
                    assert_eq!(actual.working_charge, expected.working_charge);
                    assert_eq!(actual.normalization_work, expected.normalization_work);
                    assert_eq!(actual.compatibility_nodes, expected.compatibility_nodes);
                    assert_eq!(actual.max_depth, expected.max_depth);
                    assert_eq!(actual.ignore_count, expected.ignore_count);
                    let rows = |plan: CommentPlan| {
                        plan.overrides
                            .into_iter()
                            .map(|item| (item.kind, item.raw, item.corrected))
                            .collect::<Vec<_>>()
                    };
                    assert_eq!(rows(actual), rows(expected), "{text:?}");
                }
                (Err(actual), Err(expected)) => assert_eq!(actual, expected, "{text:?}"),
                (actual, expected) => panic!("{text:?}: {actual:?} != {expected:?}"),
            }
        }
    }

    #[test]
    fn byte_payload_gate_excludes_only_malformed_type_and_ignore_comments() {
        use ruff_python_parser::byte::{parse_module_checked, ByteParseLimits};

        for (bytes, excluded) in [
            (b"# ordinary \xff\r\nx=1 # type:A\r\n".as_slice(), false),
            (b"#\x0ctype: \xff\nx=1\n".as_slice(), false),
            (b"# Type: \xff\nx=1\n".as_slice(), false),
            (b"# type : \xff\nx=1\n".as_slice(), false),
            (b"x=f'{1 # ordinary \xff\n}'\n".as_slice(), false),
            (b"# type: ignore[unit]\nx=1\n".as_slice(), false),
            (b"x=1 # type: \xff\n".as_slice(), true),
            (b"#\t type: ignore[\xff]\nx=1\n".as_slice(), true),
            (b"# type: ignore\xff\nx=1\n".as_slice(), true),
            (b"x=f'{1 # type: ignore[\xff]\n}'\n".as_slice(), true),
        ] {
            let mut check = || false;
            let parsed = parse_module_checked(
                bytes,
                ParseOptions::from(Mode::Module)
                    .with_target_version(PythonVersion::PY312)
                    .with_ast_only(true),
                ByteParseLimits::default(),
                &mut check,
            )
            .unwrap();
            let gate = check_byte_comment_payloads(bytes, parsed.tokens(), &mut || false);
            if excluded {
                assert_eq!(
                    gate,
                    Err(Error::UnsupportedCategory(
                        "malformed UTF-8 type-comment payload"
                    )),
                    "{bytes:?}"
                );
                continue;
            }
            gate.unwrap();
            parsed.with_module(|module, tokens| {
                let plan = analyze_module_with_limits(
                    bytes,
                    module,
                    tokens,
                    &mut |_| Ok(()),
                    &mut || false,
                    Limits::default(),
                )
                .unwrap();
                if bytes.starts_with(b"# ordinary") {
                    let raw = module.body[0].range();
                    assert_eq!(
                        plan.corrected_range(NodeKind::Assign, raw).end().to_usize(),
                        bytes.len() - 2
                    );
                }
            });
        }
    }

    #[test]
    fn byte_payload_gate_is_cancellable_and_text_wrapper_checks_both_channels() {
        let text = format!(
            "#{}type: ignore[{}]\nx=1\n",
            " ".repeat(512),
            "a".repeat(1024)
        );
        let parsed = parse(&text);
        let mut total = 0;
        check_byte_comment_payloads(text.as_bytes(), parsed.tokens(), &mut || {
            total += 1;
            false
        })
        .unwrap();
        for cancel_at in 1..=total {
            let mut calls = 0;
            assert_eq!(
                check_byte_comment_payloads(text.as_bytes(), parsed.tokens(), &mut || {
                    calls += 1;
                    calls == cancel_at
                }),
                Err(Error::Cancelled)
            );
        }
        for text in ["x=)\n", "type A[T = int] = int\n"] {
            let parsed = parse_unchecked(
                text,
                ParseOptions::from(Mode::Module)
                    .with_target_version(PythonVersion::PY312)
                    .with_ast_only(true),
            );
            assert!(parsed.has_syntax_errors());
            assert_eq!(
                analyze(text, &parsed, &mut |_| Ok(()), &mut || false).unwrap_err(),
                Error::UnsupportedCategory("type-comment pass requires validated AST")
            );
        }
    }

    #[test]
    fn placement_payloads_and_error_lines() {
        for text in [
            "x=1 # type: A[B ! not parsed\n",
            "x=1 # type:\n",
            "x=1; y=2 # type: A\n",
            "for x in xs: # type: A\n pass\n",
            "async for x in xs: # type: A\n pass\n",
            "with a: # type: A\n pass\n",
            "async with a: # type: A\n pass\n",
            "def f(): # type:\n pass\n",
            "async def f(a: T)->R: # type: nonsense\n pass\n",
            "def f():\n # ordinary\n\n# type: nonsense\n pass\n",
            "def f(a: T, # type: A\n /, b=1, # type: B\n *args, # type: C\n c: U=2, # type: D\n **kw # type: E\n): pass\n",
            "def f(a\n # type: A\n): pass\n",
            "x='# type: A'\n",
            "x=f'# type: A {1}'\n",
            "x=f'{1 # type: ignore\n}'\n",
            "#\x0ctype: A\npass\n",
        ] {
            assert!(plan(text).is_ok(), "valid fixture");
        }
        for (text, line) in [
            ("# type: A\nx=1\n", 1),
            ("pass # type: A\n", 1),
            ("x: T=1 # type: A\n", 1),
            ("x+=1 # type: A\n", 1),
            ("x=1; # type: A\n", 1),
            ("if x: # type: A\n pass\n", 1),
            ("for x in xs:\n # type: A\n pass\n", 2),
            ("def f(a, /, # type: A\n):pass\n", 1),
            ("def f(*, # type: A\n a):pass\n", 1),
            ("def f(a # type: A\n, b):pass\n", 2),
            ("def f(a # type: A\n=1):pass\n", 2),
            ("def f(a # type: A\n: T):pass\n", 2),
            ("def f(a: T # type: A\n=1):pass\n", 2),
            ("def f(a: (\n T\n) # type:A\n=1):pass\n", 4),
            ("def f(a # type:A\n # type:B\n=1):pass\n", 2),
            ("def f(a, # type: A\n # type: B\n):pass\n", 2),
            ("def f():\n # type: A\n # type: B\n pass\n", 3),
            ("def f(): # type: A\n # type: B\n pass\n", 3),
            ("def f(): # type: A\n # type: B\n # type: C\n pass\n", 3),
            ("x=(1 # type: A\n)\n", 1),
            ("f(1, # type: A\n)\n", 1),
            ("x=(lambda a: a # type: A\n)\n", 1),
            ("# type:A\ndef f(): # type:A\n # type:B\n pass\n", 1),
            ("def f(*args, # type:A\n b # type:B\n, c):pass\n", 3),
        ] {
            assert_eq!(plan(text).unwrap_err(), Error::Syntax { line }, "{text:?}");
        }
    }

    #[test]
    fn ignore_suffix_boundary_and_lexical_comments_only() {
        for suffix in ["", "_", "!", "[", "[code]", "[malformed", " rest", "\t"] {
            let text = format!("# type: ignore{suffix}\nx=(1 # type: ignore{suffix}\n)\n");
            let plan = plan(&text).unwrap();
            assert_eq!(plan.ignore_count, 2);
            assert!(plan.overrides.is_empty());
        }
        for suffix in ["A", "0", "XYZ", "\u{e9}", "\u{200c}", "\u{a0}"] {
            assert_eq!(
                plan(&format!("# type: ignore{suffix}\n")).unwrap_err(),
                Error::Syntax { line: 1 }
            );
            let text = format!("x=1 # type: ignore{suffix}\n");
            assert_eq!(plan(&text).unwrap().ignore_count, 0);
        }
        let lexical_plan = plan("x='''# type: A\n# type: ignore'''\ny=f'# type: A {1}'\n").unwrap();
        assert!(lexical_plan.overrides.is_empty());
        assert_eq!(lexical_plan.ignore_count, 0);
        assert!(matches!(
            plan("x=f'''{(\n1 # type: A\n)}'''\n"),
            Err(Error::UnsupportedCategory(_))
        ));
    }

    #[test]
    fn spans_are_end_exclusive_and_propagate_through_suites() {
        for newline in ["\n", "\r\n", "\r"] {
            let text = format!(
                "def f():{newline} x=(1){space}\\{newline} # type: \u{e9}{newline}",
                space = " "
            );
            let parsed = parse(&text);
            let plan = analyze(&text, &parsed, &mut |_| Ok(()), &mut || false).unwrap();
            let Mod::Module(module) = parsed.syntax() else {
                panic!()
            };
            let Stmt::FunctionDef(function) = &module.body[0] else {
                panic!()
            };
            let raw = function.body[0].range();
            let corrected = plan.corrected_range(NodeKind::Assign, raw);
            let comment_end = text.find("\u{e9}").unwrap() + "\u{e9}".len();
            assert_eq!(corrected.end().to_usize(), comment_end);
            assert_eq!(corrected.start(), raw.start());
            assert_eq!(
                plan.corrected_range(NodeKind::Statement, function.range())
                    .end(),
                corrected.end()
            );
            assert_eq!(plan.corrected_range(NodeKind::Parameter, raw), raw);
        }
        let text = "def f(a: T,\n # type: A\n b=1,\n # type: B\n *args,\n # type: C\n **kw\n # type: D\n):pass\n";
        let parsed = parse(text);
        let plan = analyze(text, &parsed, &mut |_| Ok(()), &mut || false).unwrap();
        let Mod::Module(module) = parsed.syntax() else {
            panic!()
        };
        let Stmt::FunctionDef(function) = &module.body[0] else {
            panic!()
        };
        for parameter in function.parameters.iter() {
            let parameter = parameter.as_parameter();
            let corrected = plan.corrected_range(NodeKind::Parameter, parameter.range());
            assert_eq!(corrected.start(), parameter.name.start());
            assert_eq!(corrected.end(), parameter.end());
        }
    }

    #[cfg(unix)]
    #[test]
    fn parameter_spans_exclude_defaults_but_keep_annotation_parentheses() {
        let text = "def f(a=1, # type:A\n b: (T)=2, # type:B\n *args: U, # type:C\n **kw # type:D\n):pass\n";
        let parsed = parse(text);
        let plan = analyze(text, &parsed, &mut |_| Ok(()), &mut || false).unwrap();
        assert_eq!(
            selected_spans(text, &parsed, &plan),
            vec![
                [1, 1, 6, 1, 7],
                [1, 2, 1, 2, 7],
                [1, 3, 2, 3, 9],
                [1, 4, 3, 4, 5],
                [2, 0, 0, 5, 6],
            ]
        );
    }

    fn parameter_expression_cases() -> Vec<(String, usize)> {
        let mut cases = Vec::new();
        for (expression, line) in [
            ("A # type:X\n | B", 2),
            ("A | # type:X\n B", 1),
            ("(A # type:X\n | B)", 1),
            ("(A) # type:X\n | B", 2),
            ("A # type:X\n .b", 2),
            ("A. # type:X\n b", 1),
            ("A # type:X\n [B]", 2),
            ("A[B # type:X\n ]", 1),
            ("A(B # type:X\n )", 1),
            ("[A, # type:X\n B]", 1),
            ("{A: # type:X\n B}", 1),
            ("not A # type:X\n and B", 2),
            ("A is # type:X\n not B", 1),
            ("A is not # type:X\n B", 1),
            ("await # type:X\n A", 1),
            ("await A # type:X\n + B", 2),
            ("A # type:X\n if B else C", 2),
            ("A # type:X\n # type:Y\n | B", 2),
            ("A # type:X\n # type:ignore\n | B", 3),
            ("A # type:X\n # ordinary\n | B", 3),
            ("A # type:ignore\n | B", 0),
            ("\"A\" # type:X\n \"B\"", 2),
            ("f\"{A}\" # type:X\n + B", 2),
            ("... # type:X\n or B", 2),
            ("match # type:X\n | case", 2),
            ("(lambda x: x)(A) # type:X\n + B", 2),
            ("(A if B else C) # type:X\n + D", 2),
            ("[x for x in xs] # type:X\n + B", 2),
            ("[x # type:X\n for x in xs]", 1),
            ("A # type:X\r\n | B", 2),
            ("A | # type:X\r B", 1),
        ] {
            for slot in [
                "a: EXPR",
                "a=EXPR",
                "*, a=EXPR",
                "*args: EXPR",
                "**kw: EXPR",
                "a: T=EXPR",
            ] {
                for leader in ["def f(", "async def f("] {
                    cases.push((
                        format!("{leader}{}):pass\n", slot.replace("EXPR", expression)),
                        line,
                    ));
                }
            }
        }
        cases.extend([
            (
                "# type:earlier\ndef f(a: A # type:X\n | B):pass\n".to_owned(),
                1,
            ),
            (
                "# type:earlier\ndef f(a=A if B\n # type:X\n else C):pass\n".to_owned(),
                1,
            ),
            ("def f(a=1, /, b=A # type:X\n + B):pass\n".to_owned(), 2),
            ("def f(*args: *A # type:X\n | B):pass\n".to_owned(), 2),
            (
                "def f(a: (lambda x: x)=A # type:X\n + B):pass\n".to_owned(),
                2,
            ),
            ("def f(a=A if B else C, # type:X\n):pass\n".to_owned(), 0),
        ]);
        cases
    }

    fn complex_parameter_cases() -> Vec<String> {
        let mut cases = Vec::new();
        for expression in [
            "A if B\n # type:X\n else C",
            "lambda x\n # type:X\n : x",
            "lambda x: A # type:X\n + B",
            "[x for x # type:X\n in xs]",
            "[x for x in xs # type:X\n if x]",
            "(yield A # type:X\n)",
            "[x async # type:X\n for x in xs]",
        ] {
            for slot in [
                "a: EXPR",
                "a=EXPR",
                "*, a=EXPR",
                "*args: EXPR",
                "**kw: EXPR",
                "a: T=EXPR",
            ] {
                cases.push(format!(
                    "def f({}):pass\n",
                    slot.replace("EXPR", expression)
                ));
            }
        }
        cases
    }

    #[test]
    fn parameter_expression_comments_match_locations_or_explicitly_exclude_complex_prefixes() {
        for (text, line) in parameter_expression_cases() {
            let actual = plan(&text);
            if line == 0 {
                assert!(actual.is_ok(), "{text:?}: {actual:?}");
            } else {
                assert_eq!(actual.unwrap_err(), Error::Syntax { line }, "{text:?}");
            }
        }
        for text in complex_parameter_cases() {
            assert_eq!(
                plan(&text).unwrap_err(),
                Error::UnsupportedCategory(COMPLEX_PARAMETER_COMMENT),
                "{text:?}"
            );
            assert!(plan(&text.replace("# type:X", "# type:ignore")).is_ok());
        }
    }

    #[test]
    fn parameter_internal_scans_are_cancelable_including_exclusion_paths() {
        for text in [
            "def f(a: (lambda x: x)(A) # type:X\n + B):pass\n",
            "def f(a=A if B\n # type:X\n else C):pass\n",
        ] {
            let parsed = parse(text);
            let mut checkpoints = 0;
            assert!(analyze(text, &parsed, &mut |_| Ok(()), &mut || {
                checkpoints += 1;
                false
            })
            .is_err());
            for cancel_at in 0..checkpoints {
                let mut calls = 0;
                assert!(matches!(
                    analyze(text, &parsed, &mut |_| Ok(()), &mut || {
                        let cancelled = calls == cancel_at;
                        calls += 1;
                        cancelled
                    }),
                    Err(Error::Cancelled)
                ));
            }
        }
    }

    #[test]
    fn charge_cancellation_and_separate_compatibility_limits() {
        let text =
            "match value:\n case _:\n  class C:\n   def f(a, # type: A\n   ):\n    x=1 # type: B\n";
        let parsed = parse(text);
        let mut charged = 0;
        let plan = analyze(
            text,
            &parsed,
            &mut |bytes| {
                charged += bytes;
                Ok(())
            },
            &mut || false,
        )
        .unwrap();
        assert_eq!(plan.working_charge, charged);
        assert_eq!(charged, 2 * (size_of::<Event>() + size_of::<Override>()));
        assert_eq!(plan.compatibility_nodes, 7); // match, case, class, def, arguments, arg, assign
        assert_eq!(plan.max_depth, 6);
        let work = plan.normalization_work;
        drop(plan); // caller may refund `charged` only now
        for cancel_at in 0..work {
            let mut calls = 0;
            let result = analyze(text, &parsed, &mut |_| Ok(()), &mut || {
                let cancelled = calls == cancel_at;
                calls += 1;
                cancelled
            });
            assert!(matches!(result, Err(Error::Cancelled)));
        }
        assert!(matches!(
            analyze(
                text,
                &parsed,
                &mut |_| Err(Error::WorkingLimit),
                &mut || false
            ),
            Err(Error::WorkingLimit)
        ));
        for limits in [
            Limits {
                nodes: 6,
                depth: 256,
            },
            Limits {
                nodes: 200_000,
                depth: 5,
            },
        ] {
            assert!(matches!(
                analyze_with_limits(text, &parsed, &mut |_| Ok(()), &mut || false, limits),
                Err(Error::WorkingLimit)
            ));
        }
    }

    #[cfg(unix)]
    fn oracle_cases() -> Vec<String> {
        let mut cases = Vec::new();
        for suffix in [
            "",
            "_",
            "!",
            "[",
            "[code]",
            "[malformed",
            " rest",
            "\t",
            "A",
            "0",
            "XYZ",
            "\u{e9}",
            "\u{200c}",
            "\u{a0}",
        ] {
            for context in [
                "# type: ignoreSUFFIX\npass\n",
                "x=1 # type: ignoreSUFFIX\n",
                "x=(1 # type: ignoreSUFFIX\n)\n",
                "pass # type: ignoreSUFFIX\n",
                "def f(a, # type: ignoreSUFFIX\n):pass\n",
                "def f(): # type: ignoreSUFFIX\n pass\n",
                "class C: # type: ignoreSUFFIX\n pass\n",
            ] {
                cases.push(context.replace("SUFFIX", suffix));
            }
        }
        for parameter in ["a", "a: T", "a=1", "a: T=1", "*a", "*a: T", "**a", "**a: T"] {
            for placement in [
                ", # type:A\n",
                ",\n # type:A\n",
                " # type:A\n",
                "\n # type:A\n",
                " # type:A\n,\n",
                ", # type:A\n # type:B\n",
            ] {
                cases.push(format!("def f({parameter}{placement}):pass\n"));
            }
        }
        for asynchronous in ["", "async "] {
            for inline in ["", " # type:A", " # type:ignore", " # ordinary"] {
                for standalone in [
                    "",
                    " # type:B\n",
                    " # ordinary\n # type:B\n",
                    " # type:B\n # type:C\n",
                ] {
                    cases.push(format!(
                        "{asynchronous}def f(a: T)->R:{inline}\n{standalone} pass\n"
                    ));
                }
            }
        }
        for statement in [
            "x=1",
            "x: T=1",
            "x+=1",
            "pass",
            "return 1",
            "break",
            "continue",
            "del x",
            "raise E",
            "assert x",
            "import p",
            "from p import x",
            "global x",
            "nonlocal x",
            "f()",
            "type A = int",
        ] {
            for marker in ["A", "ignore"] {
                cases.push(format!("{statement} # type: {marker}\n"));
            }
        }
        for header in [
            "class C",
            "if x",
            "while x",
            "for x in xs",
            "async for x in xs",
            "with a",
            "async with a",
            "def f()",
            "async def f()",
        ] {
            for marker in ["A", "ignore"] {
                cases.push(format!("{header}: # type: {marker}\n pass\n"));
            }
        }
        for newline in ["\n", "\r\n", "\r"] {
            for body in [
                "x=(\n 1\n) # type: A\n",
                "x=1 \\\n # type: \u{e9}\n",
                "def f():\n x=1 \\\n # type: A\n",
                "class C:\n if x:\n  y=1 \\\n # type: A\n",
                "x=1; y=2 # type:\n",
                "x=1; # type: A\n",
                "def f(a,\n # type: A\n):pass\n",
                "def f(): # type: A\n # type: B\n pass\n",
            ] {
                cases.push(body.replace('\n', newline));
            }
        }
        for source in [
            "x=1 #type : A\n",
            "x=1 #\x0ctype: A\n",
            "x=1 #\ttype: A\n",
            "x=1 #  type: A\n",
            "x=1 # type:\x0cignore\n",
            "x=1 # type:\x0bignore\n",
            "x=1 # type: [not an annotation!!\n",
            "# type:\npass\n",
            "x=1 \\\n # ordinary\n # type: A\n",
            "x='''# type: A\n# type: ignore'''\n",
            "x=f'# type: A {1}'\n",
            "x=f'{1 # type: ignore\n}'\n",
            "x=f'''{(\n1 # type: ignore\n)}'''\n",
            "x=(lambda a: a # type: A\n)\n",
            "f(1, # type: A\n)\n",
            "x=[1, # type: A\n]\n",
            "x={1:2, # type: A\n}\n",
            "x=[a # type: A\n for a in b]\n",
            "@dec # type: A\ndef f():pass\n",
            "@dec # type: ignore\ndef f():pass\n",
            "def f():\n# type: A\n pass\n",
            "def f():\n\n # ordinary\n # type: A\n pass\n",
            "def f():\\\n # type: A\n pass\n",
            "def f(): # type:A\n # type:B\n @dec\n def g():pass\n",
            "def f(): # type:A\n # type:B\n # type:C\n pass\n",
            "def f(a # type:A\n=1):pass\n",
            "def f(a: T # type:A\n=1):pass\n",
            "def f(a, /, # type:A\n):pass\n",
            "def f(*, # type:A\n a):pass\n",
            "def f(a, # type:A\n /, b=1, # type:B\n *args: T, # type:C\n c: U=2, # type:D\n **kw # type:E\n):pass\n",
            "def f(a: (\n T\n),\n # type:A\n b=2):pass\n",
            "for x in xs:\n # type:A\n pass\n",
            "with a:\n # type:A\n pass\n",
            "with (a, # type:A\n b):pass\n",
            "with (a, b): # type:A\n pass\n",
            "for x in xs: # type:A\n pass\nelse: # type:B\n pass\n",
            "try: # type:A\n pass\nexcept:pass\n",
            "try:\n x=1 # type:A\nexcept E:\n x=2 # type:B\nelse:\n x=3 # type:C\nfinally:\n x=4 # type:D\n",
            "try:\n pass\nexcept* E:\n x=1 # type:A\n",
            "if a:\n x=1 # type:A\nelif b:\n x=2 # type:B\nelse:\n x=3 # type:C\n",
            "while a:\n x=1 # type:A\nelse:\n x=2 # type:B\n",
            "match a:\n case [x] if b:\n  x=1 # type:A\n case _:\n  x=2 # type:B\n",
            "match a: # type:A\n case _:pass\n",
            "match a:\n case _: # type:A\n  pass\n",
            "def f(a, / # type:A\n):pass\n",
            "def f(* # type:A\n, a):pass\n",
            "pass; x=1 # type:A\n",
            "x=y=1 # type:A\n",
            "a,b=(1,2) # type:A\n",
            "with (\n a,\n b,\n): # type:A\n pass\n",
            "async with (a, b): # type:A\n pass\n",
            "def f(a)->(T): # type:A\n pass\n",
            "@dec # type:ignore\nclass C:\n x=1 # type:A\n",
            "def f():\n x=1 # type:A\n\n",
            "def f(*\n args, # type:A\n):pass\n",
            "def f(**\n kw, # type:A\n):pass\n",
            "def f(a: (\n T\n) # type:A\n):pass\n",
            "def f(a, # type:A\n # type:ignore\n b):pass\n",
            "def f(a # type:A\n # type:ignore\n):pass\n",
            "# type:ignore\n\n@dec\nclass C:pass\n",
        ] {
            cases.push(source.to_owned());
        }
        cases
    }

    #[cfg(unix)]
    fn location(text: &str, offset: TextSize) -> (usize, usize) {
        let mut line = 1;
        let mut column = 0;
        let mut cr = false;
        for byte in &text.as_bytes()[..offset.to_usize()] {
            if *byte == b'\r' || (*byte == b'\n' && !cr) {
                line += 1;
                column = 0;
            } else if *byte != b'\n' || !cr {
                column += 1;
            }
            cr = *byte == b'\r';
        }
        (line, column)
    }

    #[cfg(unix)]
    fn selected_spans(text: &str, parsed: &Parsed<Mod>, plan: &CommentPlan) -> Vec<[usize; 5]> {
        let Mod::Module(module) = parsed.syntax() else {
            panic!()
        };
        let row = |kind, range: TextRange| {
            let (line, column) = location(text, range.start());
            let (end_line, end_column) = location(text, range.end());
            [kind, line, column, end_line, end_column]
        };
        let mut rows = Vec::new();
        for statement in &module.body {
            let range = plan.corrected_range(NodeKind::Statement, statement.range());
            let (line, column) = location(text, range.end());
            rows.push([2, 0, 0, line, column]);
            if let Stmt::Assign(assign) = statement {
                rows.push(row(
                    0,
                    plan.corrected_range(NodeKind::Assign, assign.range()),
                ));
            }
            if let Stmt::FunctionDef(function) = statement {
                for parameter in function.parameters.iter() {
                    // AnyParameterRef::range includes the default for ordinary
                    // parameters; production and CPython use the inner arg.
                    let parameter = parameter.as_parameter();
                    let range = plan.corrected_range(NodeKind::Parameter, parameter.range());
                    rows.push(row(1, TextRange::new(parameter.name.start(), range.end())));
                }
            }
        }
        rows.sort_unstable();
        rows
    }

    #[cfg(unix)]
    #[test]
    #[ignore = "requires explicit trusted CPython 3.12.11 / Unicode 15 oracle"]
    fn trusted_type_comment_matrix_acceptance_lines_and_selected_spans() {
        use std::path::PathBuf;
        use std::process::{Command, Stdio};

        #[derive(serde::Deserialize)]
        struct Observation {
            line: usize,
            ignores: usize,
            spans: Vec<[usize; 5]>,
        }

        let python = PathBuf::from(
            std::env::var_os("IDEOMANCER_TEST_PYTHON")
                .expect("explicit trusted CPython 3.12.11 executable required"),
        );
        assert!(python.is_absolute(), "trusted oracle path must be absolute");
        let mut cases = oracle_cases();
        assert_eq!(cases.len(), 312, "preserve the complete contextual matrix");
        cases.extend([
            "def f(a # type:A\n: T):pass\n".to_owned(),
            "def f(a # type:A\n: T # type:B\n=1):pass\n".to_owned(),
            "def f(a # type:A\n # type:B\n=1):pass\n".to_owned(),
            "def f(a: (\n T\n) # type:A\n=1):pass\n".to_owned(),
            "def f(a # type:A\r\n=1):pass\r\n".to_owned(),
            "def f(a: T # type:A\r=1):pass\r".to_owned(),
            "# type:X\ndef f(a # type:A\n=1):pass\n".to_owned(),
            "def f(a=1, b: (T)=2):pass\n".to_owned(),
            "def f(a=(\n 1\n), # type:A\n b: (T)=2):pass\n".to_owned(),
            "def f(a=1, # type:A\n b: (T)=2, # type:B\n *args: U, # type:C\n **kw # type:D\n):pass\n".to_owned(),
        ]);
        cases.extend(
            parameter_expression_cases()
                .into_iter()
                .map(|(text, _)| text),
        );
        let complex = complex_parameter_cases();
        cases.extend(
            complex
                .iter()
                .map(|text| text.replace("# type:X", "# type:ignore")),
        );
        let supported_cases = cases.len();
        assert_eq!(
            supported_cases, 742,
            "preserve ordinary and ignore controls"
        );
        cases.extend(complex);
        let complex_cases = cases.len();
        cases.extend([
            "x=f'{1 # type: A\n}'\n".to_owned(),
            "x=f'''{(\n1 # type: A\n)}'''\n".to_owned(),
            "x=f'''{1:\n{2 # type: A\n}}'''\n".to_owned(),
        ]);
        // This script only parses independently synthetic strings. It neither
        // evaluates nor compiles/imports their contents. No comment payloads are
        // returned: only acceptance, error lines, ignore counts and positions.
        let script = r#"
import ast, json, sys, unicodedata
assert sys.version_info[:3] == (3, 12, 11), sys.version
assert unicodedata.unidata_version == '15.0.0'
result = []
def span(kind, node):
    return [kind, node.lineno, node.col_offset, node.end_lineno, node.end_col_offset]
for source in json.load(sys.stdin):
    try:
        tree = ast.parse(source, type_comments=True)
    except SyntaxError as error:
        result.append(dict(line=error.lineno, ignores=0, spans=[]))
        continue
    rows = []
    for node in tree.body:
        rows.append([2, 0, 0, node.end_lineno, node.end_col_offset])
        if isinstance(node, ast.Assign):
            rows.append(span(0, node))
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            args = node.args
            for arg in args.posonlyargs + args.args + args.kwonlyargs:
                rows.append(span(1, arg))
            for arg in (args.vararg, args.kwarg):
                if arg is not None:
                    rows.append(span(1, arg))
    result.append(dict(line=0, ignores=len(tree.type_ignores), spans=sorted(rows)))
json.dump(result, sys.stdout)
"#;
        let mut command = Command::new(python);
        command
            .args(["-I", "-S", "-B", "-c", script])
            .current_dir("/")
            .env_clear()
            .env("TMPDIR", "/")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let output = crate::analysis::run_bounded(
            command,
            serde_json::to_vec(&cases).unwrap(),
            "trusted type-comment oracle",
        )
        .unwrap();
        let observations: Vec<Observation> = serde_json::from_slice(&output).unwrap();
        assert_eq!(observations.len(), cases.len());
        for (index, (text, expected)) in cases.iter().zip(observations).enumerate() {
            let parsed = parse(text);
            let actual = analyze(text, &parsed, &mut |_| Ok(()), &mut || false);
            if index >= supported_cases {
                assert!(
                    expected.line > 0,
                    "case {index}: Python must reject this finite exclusion"
                );
                let category = if index < complex_cases {
                    COMPLEX_PARAMETER_COMMENT
                } else {
                    "actual type comment in f-string replacement field"
                };
                assert_eq!(
                    actual.unwrap_err(),
                    Error::UnsupportedCategory(category),
                    "case {index}"
                );
                continue;
            }
            match actual {
                Ok(plan) => {
                    assert_eq!(expected.line, 0, "case {index}: Python rejected");
                    assert_eq!(plan.ignore_count, expected.ignores, "case {index}: ignores");
                    assert_eq!(
                        selected_spans(text, &parsed, &plan),
                        expected.spans,
                        "case {index}: spans"
                    );
                }
                Err(Error::Syntax { line }) => {
                    assert_eq!(line, expected.line, "case {index}: syntax line")
                }
                Err(error) => panic!("case {index}: unexpected compatibility failure {error:?}"),
            }
        }
    }
}
