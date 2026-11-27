//! One AST normalization, separate helper-render and helper-visit edges.
use super::facts::{Driver, Error, Result};
use ruff_python_ast::{self as ast, Expr, InterpolatedStringElement, Operator};
use ruff_text_size::{Ranged, TextRange};

pub(super) enum Shape {
    Name(String),
    Text(Vec<TextPart>),
    Attribute(usize, String),
    Call {
        func: usize,
        args: Vec<usize>,
        keywords: Vec<(String, usize)>,
    },
    Subscript(usize, usize),
    Sequence,
    Binary(usize, &'static str, usize),
    Starred(usize),
    Opaque(&'static str),
}

pub(super) struct TextPart {
    pub(super) range: TextRange,
    pub(super) raw: bool,
}

pub(super) struct Node {
    pub(super) shape: Shape,
    pub(super) children: Vec<usize>,
    pub(super) range: TextRange,
}

impl Driver<'_> {
    fn edge(&mut self, children: &mut Vec<usize>, id: usize) -> Result<()> {
        self.tick()?;
        self.edges += 1;
        if self.edges > self.edge_limit() {
            return Err(Error::Resource);
        }
        self.charge_arena(4 * std::mem::size_of::<usize>())?;
        children.push(id);
        Ok(())
    }

    fn child(&mut self, children: &mut Vec<usize>, expr: &Expr, depth: usize) -> Result<usize> {
        let id = self.normalize(expr, depth + 1)?;
        self.edge(children, id)?;
        Ok(id)
    }

    fn generators(
        &mut self,
        children: &mut Vec<usize>,
        values: &[ast::Comprehension],
        depth: usize,
    ) -> Result<()> {
        for value in values {
            self.node(depth + 1)?;
            self.child(children, &value.target, depth + 1)?;
            self.child(children, &value.iter, depth + 1)?;
            for test in &value.ifs {
                self.child(children, test, depth + 1)?;
            }
        }
        Ok(())
    }

    fn formatted<'b>(
        &mut self,
        children: &mut Vec<usize>,
        elements: impl Iterator<Item = &'b InterpolatedStringElement>,
        depth: usize,
    ) -> Result<()> {
        for element in elements {
            self.node(depth + 1)?;
            match element {
                InterpolatedStringElement::Literal(_) => {}
                InterpolatedStringElement::Interpolation(field) => {
                    self.child(children, &field.expression, depth + 1)?;
                    if let Some(spec) = &field.format_spec {
                        self.node(depth + 2)?;
                        self.formatted(children, spec.elements.iter(), depth + 1)?;
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn normalize(&mut self, expr: &Expr, depth: usize) -> Result<usize> {
        self.node(depth)?;
        self.charge_arena(4 * std::mem::size_of::<Node>())?;
        let mut children = Vec::new();
        let shape = match expr {
            Expr::Name(n) => Shape::Name(self.arena_text(n.id.as_str())?),
            Expr::Attribute(n) => Shape::Attribute(
                self.child(&mut children, &n.value, depth)?,
                self.arena_text(n.attr.as_str())?,
            ),
            Expr::Call(n) => {
                let func = self.child(&mut children, &n.func, depth)?;
                let mut args = Vec::new();
                let mut keywords = Vec::new();
                for arg in &n.arguments.args {
                    let id = self.child(&mut children, arg, depth)?;
                    self.edge(&mut args, id)?;
                }
                for kw in &n.arguments.keywords {
                    self.node(depth + 1)?;
                    let id = self.child(&mut children, &kw.value, depth)?;
                    self.charge_arena(4 * std::mem::size_of::<(String, usize)>())?;
                    keywords.push((
                        self.arena_text(kw.arg.as_ref().map_or("**", |n| n.as_str()))?,
                        id,
                    ));
                }
                Shape::Call {
                    func,
                    args,
                    keywords,
                }
            }
            Expr::Subscript(n) => Shape::Subscript(
                self.child(&mut children, &n.value, depth)?,
                self.child(&mut children, &n.slice, depth)?,
            ),
            Expr::Tuple(n) => {
                for e in &n.elts {
                    self.child(&mut children, e, depth)?;
                }
                Shape::Sequence
            }
            Expr::List(n) => {
                for e in &n.elts {
                    self.child(&mut children, e, depth)?;
                }
                Shape::Sequence
            }
            Expr::Set(n) => {
                for e in &n.elts {
                    self.child(&mut children, e, depth)?;
                }
                Shape::Sequence
            }
            Expr::BinOp(n) => Shape::Binary(
                self.child(&mut children, &n.left, depth)?,
                operator(n.op),
                self.child(&mut children, &n.right, depth)?,
            ),
            Expr::Starred(n) => Shape::Starred(self.child(&mut children, &n.value, depth)?),
            Expr::BoolOp(n) => {
                for e in &n.values {
                    self.child(&mut children, e, depth)?;
                }
                Shape::Opaque("<BoolOp>")
            }
            Expr::Named(n) => {
                self.child(&mut children, &n.target, depth)?;
                self.child(&mut children, &n.value, depth)?;
                Shape::Opaque("<NamedExpr>")
            }
            Expr::UnaryOp(n) => {
                self.child(&mut children, &n.operand, depth)?;
                Shape::Opaque("<UnaryOp>")
            }
            Expr::Lambda(n) => {
                if let Some(p) = &n.parameters {
                    self.node(depth + 1)?;
                    for _ in p.posonlyargs.iter().chain(&p.args).chain(&p.kwonlyargs) {
                        self.node(depth + 2)?;
                    }
                    if p.vararg.is_some() {
                        self.node(depth + 2)?;
                    }
                    if p.kwarg.is_some() {
                        self.node(depth + 2)?;
                    }
                    // ast.arguments visits kw_defaults before defaults. Lambdas
                    // have no annotations and do not push an Extractor scope.
                    for a in &p.kwonlyargs {
                        if let Some(e) = &a.default {
                            self.child(&mut children, e, depth)?;
                        }
                    }
                    for a in p.posonlyargs.iter().chain(&p.args) {
                        if let Some(e) = &a.default {
                            self.child(&mut children, e, depth)?;
                        }
                    }
                }
                self.child(&mut children, &n.body, depth)?;
                Shape::Opaque("<Lambda>")
            }
            Expr::If(n) => {
                self.child(&mut children, &n.test, depth)?;
                self.child(&mut children, &n.body, depth)?;
                self.child(&mut children, &n.orelse, depth)?;
                Shape::Opaque("<IfExp>")
            }
            Expr::Dict(n) => {
                for item in &n.items {
                    self.tick()?;
                    if let Some(key) = &item.key {
                        self.child(&mut children, key, depth)?;
                    }
                }
                for item in &n.items {
                    self.child(&mut children, &item.value, depth)?;
                }
                Shape::Opaque("<Dict>")
            }
            Expr::ListComp(n) => {
                self.child(&mut children, &n.elt, depth)?;
                self.generators(&mut children, &n.generators, depth)?;
                Shape::Opaque("<ListComp>")
            }
            Expr::SetComp(n) => {
                self.child(&mut children, &n.elt, depth)?;
                self.generators(&mut children, &n.generators, depth)?;
                Shape::Opaque("<SetComp>")
            }
            Expr::DictComp(n) => {
                self.child(&mut children, &n.key, depth)?;
                self.child(&mut children, &n.value, depth)?;
                self.generators(&mut children, &n.generators, depth)?;
                Shape::Opaque("<DictComp>")
            }
            Expr::Generator(n) => {
                self.child(&mut children, &n.elt, depth)?;
                self.generators(&mut children, &n.generators, depth)?;
                Shape::Opaque("<GeneratorExp>")
            }
            Expr::Await(n) => {
                self.child(&mut children, &n.value, depth)?;
                Shape::Opaque("<Await>")
            }
            Expr::Yield(n) => {
                if let Some(e) = &n.value {
                    self.child(&mut children, e, depth)?;
                }
                Shape::Opaque("<Yield>")
            }
            Expr::YieldFrom(n) => {
                self.child(&mut children, &n.value, depth)?;
                Shape::Opaque("<YieldFrom>")
            }
            Expr::Compare(n) => {
                self.child(&mut children, &n.left, depth)?;
                for e in &n.comparators {
                    self.child(&mut children, e, depth)?;
                }
                Shape::Opaque("<Compare>")
            }
            Expr::FString(n) => {
                self.formatted(&mut children, n.value.elements(), depth)?;
                Shape::Opaque("<JoinedStr>")
            }
            Expr::Slice(n) => {
                for e in [&n.lower, &n.upper, &n.step].into_iter().flatten() {
                    self.child(&mut children, e, depth)?;
                }
                Shape::Opaque("<Slice>")
            }
            Expr::StringLiteral(n) => {
                let mut parts = Vec::new();
                for part in n.value.iter() {
                    self.tick()?;
                    self.charge_arena(4 * std::mem::size_of::<TextPart>())?;
                    parts.push(TextPart {
                        range: part.content_range(),
                        raw: part.flags.prefix().is_raw(),
                    });
                }
                Shape::Text(parts)
            }
            Expr::BytesLiteral(_)
            | Expr::NumberLiteral(_)
            | Expr::BooleanLiteral(_)
            | Expr::NoneLiteral(_)
            | Expr::EllipsisLiteral(_) => Shape::Opaque("<literal>"),
            Expr::TString(_) | Expr::IpyEscapeCommand(_) => {
                return Err(Error::Unsupported("expression variant"))
            }
        };
        let range = if matches!(expr, Expr::Generator(_)) {
            self.generator_range(expr.range())?
        } else {
            expr.range()
        };
        let id = self.arena.len();
        self.arena.push(Node {
            shape,
            children,
            range,
        });
        Ok(id)
    }

    pub(super) fn render(&mut self, id: usize) -> Result<String> {
        // Each live recursive frame owns a fixed 512-byte buffer. Depth 13 is
        // terminal; the shared 80-node budget is independent for each render.
        self.charge(15 * 512)?;
        let result = render(&self.arena, id, 0, &mut 80, &mut || (self.check)());
        self.sink.release(14 * 512);
        result
    }
}

struct Clipped {
    text: String,
    saturated: bool,
}

fn append(out: &mut Clipped, text: &str) {
    if out.saturated {
        return;
    }
    let remaining = 512 - out.text.len();
    let mut end = remaining.min(text.len());
    out.saturated = text.len() > remaining;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    out.text.push_str(&text[..end]);
}

fn render(
    arena: &[Node],
    id: usize,
    depth: usize,
    budget: &mut i32,
    check: &mut impl FnMut() -> bool,
) -> Result<String> {
    if check() {
        return Err(Error::Cancelled);
    }
    *budget -= 1;
    let mut out = Clipped {
        text: String::with_capacity(512),
        saturated: false,
    };
    if *budget < 0 || depth > 12 {
        out.text.push_str("...");
        return Ok(out.text);
    }
    let mut child = |id| render(arena, id, depth + 1, budget, check);
    match &arena[id].shape {
        Shape::Name(n) => append(&mut out, n),
        Shape::Text(_) => append(&mut out, "<literal>"),
        Shape::Opaque(n) => append(&mut out, n),
        Shape::Attribute(value, name) => {
            append(&mut out, &child(*value)?);
            append(&mut out, ".");
            append(&mut out, name);
        }
        Shape::Call {
            func,
            args,
            keywords,
        } => {
            append(&mut out, &child(*func)?);
            append(&mut out, "(");
            let mut comma = false;
            for id in args.iter().take(16) {
                if comma {
                    append(&mut out, ", ");
                }
                comma = true;
                append(&mut out, &child(*id)?);
            }
            if args.len() > 16 {
                if comma {
                    append(&mut out, ", ");
                }
                append(&mut out, "...");
                comma = true;
            }
            for (name, id) in keywords.iter().take(16) {
                if comma {
                    append(&mut out, ", ");
                }
                comma = true;
                append(&mut out, name);
                append(&mut out, "=");
                append(&mut out, &child(*id)?);
            }
            if keywords.len() > 16 {
                if comma {
                    append(&mut out, ", ");
                }
                append(&mut out, "...");
            }
            append(&mut out, ")");
        }
        Shape::Subscript(value, slice) => {
            append(&mut out, &child(*value)?);
            append(&mut out, "[");
            append(&mut out, &child(*slice)?);
            append(&mut out, "]");
        }
        Shape::Sequence => {
            append(&mut out, "[");
            for (i, id) in arena[id].children.iter().take(16).enumerate() {
                if i != 0 {
                    append(&mut out, ", ");
                }
                append(&mut out, &child(*id)?);
            }
            append(&mut out, "]");
        }
        Shape::Binary(left, op, right) => {
            append(&mut out, &child(*left)?);
            append(&mut out, " ");
            append(&mut out, op);
            append(&mut out, " ");
            append(&mut out, &child(*right)?);
        }
        Shape::Starred(value) => {
            append(&mut out, "*");
            append(&mut out, &child(*value)?);
        }
    }
    Ok(out.text)
}

fn operator(op: Operator) -> &'static str {
    match op {
        Operator::Add => "Add",
        Operator::Sub => "Sub",
        Operator::Mult => "Mult",
        Operator::MatMult => "MatMult",
        Operator::Div => "Div",
        Operator::Mod => "Mod",
        Operator::Pow => "Pow",
        Operator::LShift => "LShift",
        Operator::RShift => "RShift",
        Operator::BitOr => "BitOr",
        Operator::BitXor => "BitXor",
        Operator::BitAnd => "BitAnd",
        Operator::FloorDiv => "FloorDiv",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ruff_render_utf8_clip_does_not_fill_a_partial_codepoint_gap() {
        let node = |shape| Node {
            shape,
            children: Vec::new(),
            range: TextRange::default(),
        };
        let arena = vec![
            node(Shape::Name("a".repeat(511))),
            node(Shape::Call {
                func: 0,
                args: Vec::new(),
                keywords: vec![("\u{03bb}".into(), 0)],
            }),
        ];
        // The callee and '(' fill this node exactly, with no trailing syntax.
        assert_eq!(
            render(&arena, 1, 0, &mut 80, &mut || false).unwrap(),
            format!("{}(", "a".repeat(511))
        );
        let arena = vec![
            node(Shape::Name("a".repeat(509))),
            node(Shape::Call {
                func: 0,
                args: Vec::new(),
                keywords: vec![("\u{1f600}".into(), 0)],
            }),
        ];
        // Final concatenation cuts the four-byte keyword after its first two
        // bytes. Decoding ignores that fragment, not refilling it with '='.
        assert_eq!(
            render(&arena, 1, 0, &mut 80, &mut || false).unwrap(),
            format!("{}(", "a".repeat(509))
        );
        let mut budget = 80;
        render(&arena, 1, 0, &mut budget, &mut || false).unwrap();
        assert_eq!(budget, 77, "render children even after output saturates");
        let mut arena = vec![
            node(Shape::Name("a".repeat(506))),
            node(Shape::Name("\u{1f600}".into())),
            node(Shape::Sequence),
        ];
        arena[2].children = vec![0, 1];
        assert_eq!(
            render(&arena, 2, 0, &mut 80, &mut || false).unwrap(),
            format!("[{}, ", "a".repeat(506))
        );
    }
}
