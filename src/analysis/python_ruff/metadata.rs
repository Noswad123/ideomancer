//! Closed helper metadata, fed only by source-backed ordinary text parts.
//! Reserve before allocation: part slots belong to the statement arena; retention
//! reserves both capped buffers, refunds the unused charge, then drops the box
//! before its final refund. Encoders count, charge, and allocate exactly once.
//! Header preludes own charged ASCII only, never arena IDs or captured owners.
use super::*;
use crate::analysis::python_ruff::literals;
use crate::analysis::python_scalars::{CodePoint, Retention, TextLimit};
use projection::CallFact;
use std::io::Write;

enum Value<'a> {
    Text(&'a str),
    Points(Option<&'a [CodePoint]>),
    Bool(bool),
    Methods(&'a [bool; 8]),
    Null,
}

enum Metadata<'a> {
    Entrypoint {
        decorator: &'a str,
        role: &'static str,
    },
    Route {
        methods: &'a [bool; 8],
        path: Option<&'a [CodePoint]>,
        receiver: &'a str,
    },
    Host {
        constructor: &'a str,
        target: &'a str,
    },
    Prefix {
        constructor: &'a str,
        prefix: Option<&'a [CodePoint]>,
        supported: bool,
        target: &'a str,
    },
    Include {
        prefix: Option<&'a [CodePoint]>,
        receiver: &'a str,
        router: Option<&'a str>,
        supported: bool,
    },
}

impl Metadata<'_> {
    fn write<W: Write, C: FnMut() -> bool>(&self, out: &mut AsciiJson<W, C>) -> Result<()> {
        use Value::*;
        // Fixed stack fields keep this encoder closed and allocation-free in its
        // counting pass. Keys are in Python sort_keys order.
        let encode = |fields: &[(&str, Value<'_>)], out: &mut AsciiJson<W, C>| {
            out.raw(b"{")?;
            for (i, (key, value)) in fields.iter().enumerate() {
                if i != 0 {
                    out.raw(b",")?;
                }
                out.text(key)?;
                out.raw(b":")?;
                match value {
                    Text(text) => out.text(text)?,
                    Points(Some(points)) => out.points(points.iter().copied())?,
                    Points(None) | Null => out.raw(b"null")?,
                    Bool(true) => out.raw(b"true")?,
                    Bool(false) => out.raw(b"false")?,
                    Methods(methods) => {
                        out.raw(b"[")?;
                        let mut comma = false;
                        for (present, name) in methods.iter().zip(literals::METHODS) {
                            if *present {
                                if comma {
                                    out.raw(b",")?;
                                }
                                comma = true;
                                out.text(name)?;
                            }
                        }
                        out.raw(b"]")?;
                    }
                }
            }
            out.raw(b"}")
        };
        match self {
            Self::Entrypoint { decorator, role } => {
                encode(&[("decorator", Text(decorator)), ("role", Text(role))], out)
            }
            Self::Route {
                methods,
                path,
                receiver,
            } => encode(
                &[
                    ("methods", Methods(methods)),
                    ("path", Points(*path)),
                    ("pathIsFragment", Bool(true)),
                    ("receiver", Text(receiver)),
                ],
                out,
            ),
            Self::Host {
                constructor,
                target,
            } => encode(
                &[("constructor", Text(constructor)), ("target", Text(target))],
                out,
            ),
            Self::Prefix {
                constructor,
                prefix,
                supported,
                target,
            } => encode(
                &[
                    ("constructor", Text(constructor)),
                    ("prefix", Points(*prefix)),
                    ("supported", Bool(*supported)),
                    ("target", Text(target)),
                ],
                out,
            ),
            Self::Include {
                prefix,
                receiver,
                router,
                supported,
            } => encode(
                &[
                    ("prefix", Points(*prefix)),
                    ("receiver", Text(receiver)),
                    ("router", router.map_or(Null, Text)),
                    ("supported", Bool(*supported)),
                ],
                out,
            ),
        }
        .map_err(shared::Failure::from)?;
        Ok(())
    }
}

fn literal_error(error: literals::Error) -> Error {
    match error {
        literals::Error::Cancelled => Error::Cancelled,
        literals::Error::InvalidNamedEscape => Error::Unsupported("literal validation invariant"),
        _ => Error::Unsupported("literal decoding"),
    }
}

impl Driver<'_> {
    fn encode_metadata(&mut self, metadata: &Metadata<'_>) -> Result<String> {
        let mut count = AsciiJson::new(shared::CountWriter::new(OUTPUT), OUTPUT, &mut *self.check);
        metadata.write(&mut count)?;
        let size = count.into_inner().count();
        self.charge(size)?;
        let mut out = AsciiJson::new(Vec::with_capacity(size), size, &mut *self.check);
        metadata.write(&mut out)?;
        Ok(String::from_utf8(out.into_inner()).expect("ASCII metadata"))
    }

    fn retained(
        &mut self,
        id: Option<usize>,
        limit: TextLimit,
    ) -> Result<Option<Box<[CodePoint]>>> {
        let Some(id) = id else {
            return Ok(None);
        };
        if !matches!(self.arena[id].shape, Shape::Text(_)) {
            return Ok(None);
        }
        let reserved = literals::retention_work(limit);
        self.charge(reserved)?;
        // The validated callback supplies exactly source.bytes with its BOM
        // stripped. Reuse that UTF-8 view, not another scan or decoded AST value.
        let source = self.source_text;
        let Shape::Text(parts) = &self.arena[id].shape else {
            unreachable!()
        };
        let retained = literals::retain_text(
            parts.iter().map(|part| literals::Part {
                content: &source[part.range.start().to_usize()..part.range.end().to_usize()],
                raw: part.raw,
            }),
            limit,
            &mut *self.check,
        );
        let retained = match retained {
            Ok(retained) => retained,
            Err(error) => {
                self.release(reserved)?;
                return Err(literal_error(error));
            }
        };
        self.release(reserved - literals::retained_bytes(&retained))?;
        Ok(match retained {
            Retention::Retained(points) => Some(points),
            Retention::Oversize => None,
        })
    }

    fn drop_points(&mut self, points: Option<Box<[CodePoint]>>) -> Result<()> {
        let size = points.as_deref().map_or(0, std::mem::size_of_val);
        drop(points);
        self.release(size)
    }

    fn keyword(&mut self, id: usize, wanted: &str) -> Result<(usize, Option<usize>)> {
        let Shape::Call { keywords, .. } = &self.arena[id].shape else {
            return Ok((0, None));
        };
        let mut count = 0;
        let mut found = None;
        for (key, value) in keywords {
            if (self.check)() {
                return Err(Error::Cancelled);
            }
            if key == wanted {
                count += 1;
                found = Some(*value);
            }
        }
        Ok((count, if count == 1 { found } else { None }))
    }

    pub(super) fn include_metadata(&mut self, id: usize) -> Result<Option<CallFact>> {
        let Shape::Call {
            func,
            args,
            keywords,
        } = &self.arena[id].shape
        else {
            return Ok(None);
        };
        let Shape::Attribute(receiver, attr) = &self.arena[*func].shape else {
            return Ok(None);
        };
        if attr != "include_router" {
            return Ok(None);
        }
        let receiver_id = *receiver;
        let router_id = if args.len() == 1 { Some(args[0]) } else { None };
        let simple = router_id.is_some_and(|id| matches!(self.arena[id].shape, Shape::Name(_)));
        let mut keywords_supported = true;
        for (name, _) in keywords {
            if (self.check)() {
                return Err(Error::Cancelled);
            }
            keywords_supported &= name == "prefix";
        }
        let (count, prefix_id) = self.keyword(id, "prefix")?;
        let prefix = self.retained(prefix_id, TextLimit::Prefix)?;
        let valid = count == 0 || prefix.as_deref().is_some_and(literals::valid_prefix);
        let receiver = self.render(receiver_id)?;
        let router = router_id.map(|id| self.render(id)).transpose()?;
        let value = self.encode_metadata(&Metadata::Include {
            prefix: if count == 0 {
                Some(&[])
            } else if valid {
                prefix.as_deref()
            } else {
                None
            },
            receiver: &receiver,
            router: router.as_deref(),
            supported: valid && simple && keywords_supported,
        })?;
        self.drop_points(prefix)?;
        self.drop_text(receiver)?;
        if let Some(router) = router {
            self.drop_text(router)?;
        }
        Ok(Some(CallFact {
            kind: "router_include",
            value,
        }))
    }

    fn constructor(&mut self, callee: &str, expected: &str) -> Result<bool> {
        let (head, tail) = callee
            .find('.')
            .map_or((callee, ""), |i| (&callee[..i], &callee[i..]));
        let alias =
            self.projection
                .lookup(self.scope, head, &mut work(self.sink, self.check, None))?;
        Ok(if let Some(alias) = alias {
            self.projection
                .target(alias)
                .bytes()
                .chain(tail.bytes())
                .eq(expected.bytes())
        } else {
            callee.rsplit('.').next() == expected.rsplit('.').next()
        })
    }

    pub(super) fn constructor_metadata(
        &mut self,
        targets: &[usize],
        value: usize,
        range: TextRange,
    ) -> Result<()> {
        let Shape::Call {
            func,
            args,
            keywords,
        } = &self.arena[value].shape
        else {
            return Ok(());
        };
        let func = *func;
        let mut supported_args = args.is_empty();
        for (key, _) in keywords {
            if (self.check)() {
                return Err(Error::Cancelled);
            }
            supported_args &= matches!(key.as_str(), "prefix" | "tags");
        }
        let callee = self.render(func)?;
        let host = self.constructor(&callee, "fastapi.FastAPI")?;
        let router = self.constructor(&callee, "fastapi.APIRouter")?;
        if host {
            for &id in targets {
                self.tick()?;
                if let Shape::Name(name) = &self.arena[id].shape {
                    if name.chars().take(129).count() <= 128 {
                        let target = self.render(id)?;
                        let value = self.encode_metadata(&Metadata::Host {
                            constructor: &callee,
                            target: &target,
                        })?;
                        self.emit("router_host", range, value, None)?;
                        self.drop_text(target)?;
                    }
                }
            }
        }
        if router {
            let (_, prefix_id) = self.keyword(value, "prefix")?;
            let prefix = self.retained(prefix_id, TextLimit::Prefix)?;
            let valid = prefix.as_deref().is_some_and(literals::valid_prefix);
            for &id in targets {
                self.tick()?;
                if let Shape::Name(name) = &self.arena[id].shape {
                    if name.chars().take(129).count() <= 128 {
                        let target = self.render(id)?;
                        let value = self.encode_metadata(&Metadata::Prefix {
                            constructor: &callee,
                            target: &target,
                            prefix: if valid { prefix.as_deref() } else { None },
                            supported: valid && supported_args,
                        })?;
                        self.emit("router_prefix", range, value, None)?;
                        self.drop_text(target)?;
                    }
                }
            }
            self.drop_points(prefix)?;
        }
        self.drop_text(callee)
    }

    pub(super) fn definition_range(
        &mut self,
        range: TextRange,
        name: TextRange,
        class: bool,
        asynchronous: bool,
    ) -> Result<TextRange> {
        let range = self.corrected_range(type_comments::NodeKind::Statement, range);
        let end = self
            .keywords
            .partition_point(|(_, r)| r.start() < name.start());
        let Some(index) = end.checked_sub(1) else {
            return Err(Error::Unsupported("definition keyword"));
        };
        self.tick()?;
        let (kind, keyword) = self.keywords[index];
        let expected = if class {
            TokenKind::Class
        } else {
            TokenKind::Def
        };
        if kind != expected || keyword.start() < range.start() {
            return Err(Error::Unsupported("definition keyword"));
        }
        let start = if asynchronous {
            let Some((TokenKind::Async, previous)) = index.checked_sub(1).map(|i| self.keywords[i])
            else {
                return Err(Error::Unsupported("async keyword"));
            };
            if previous.start() < range.start() {
                return Err(Error::Unsupported("async keyword"));
            }
            previous.start()
        } else {
            keyword.start()
        };
        Ok(TextRange::new(start, range.end()))
    }

    pub(super) fn decorators(
        &mut self,
        decorators: &[ast::Decorator],
        schedule: &mut HeaderSchedule,
        discovery: bool,
        depth: usize,
    ) -> Result<()> {
        for decorator in decorators {
            self.node(depth + 1)?;
            self.arena_scope(|driver| {
                let id = driver.normalize(&decorator.expression, depth + 1)?;
                let value = driver.render(id)?;
                driver.emit("decorator", driver.arena[id].range, value, None)?;
                if discovery {
                    driver.discovery(id)?;
                }
                driver.visits(id, Some((schedule, HeaderGroup::Decorators)))
            })?;
        }
        Ok(())
    }

    fn route_warning(&mut self, id: usize, path: bool) -> Result<()> {
        let line = self.span(self.arena[id].range).0;
        self.charge(256)?;
        let mut message = String::with_capacity(256);
        use std::fmt::Write;
        write!(
            message,
            "line {line}: {}",
            if path {
                "route path is not a bounded direct slash-prefixed string; prefix not inferred"
            } else {
                "route methods are not a literal HTTP whitelist; methods unknown"
            }
        )
        .expect("String writer");
        let capacity = message.capacity();
        let result = self
            .sink
            .warn("dynamic_api_route", &self.source.path, message, &mut || {
                (self.check)()
            });
        self.release(capacity)?;
        result?;
        #[cfg(test)]
        if let Some(trace) = &mut self.trace {
            trace.push(Observation::Warning {
                warning: self.sink.warnings().last().unwrap().clone(),
            });
        }
        Ok(())
    }

    fn method(&mut self, id: usize) -> Result<Option<&'static str>> {
        if !matches!(self.arena[id].shape, Shape::Text(_)) {
            return Ok(None);
        }
        self.charge(8)?;
        let source = self.source_text;
        let Shape::Text(parts) = &self.arena[id].shape else {
            unreachable!()
        };
        let result = literals::method(literals::decode(
            parts.iter().map(|part| literals::Part {
                content: &source[part.range.start().to_usize()..part.range.end().to_usize()],
                raw: part.raw,
            }),
            &mut *self.check,
        ));
        self.release(8)?;
        match result {
            Ok(method) => Ok(method),
            Err(error) => Err(literal_error(error)),
        }
    }

    fn discovery(&mut self, id: usize) -> Result<()> {
        let called = matches!(self.arena[id].shape, Shape::Call { .. });
        let func = if let Shape::Call { func, .. } = self.arena[id].shape {
            func
        } else {
            id
        };
        let (role, route, verb) = match &self.arena[func].shape {
            Shape::Name(name) if name == "task" => (Some("job"), false, None),
            Shape::Attribute(_, attr) if called => {
                let role = match attr.as_str() {
                    "command" => Some("cli_command"),
                    "task" | "job" | "scheduled_job" => Some("job"),
                    _ => None,
                };
                let verb = match attr.as_str() {
                    "delete" => Some(0),
                    "get" => Some(1),
                    "head" => Some(2),
                    "options" => Some(3),
                    "patch" => Some(4),
                    "post" => Some(5),
                    "put" => Some(6),
                    "trace" => Some(7),
                    _ => None,
                };
                (
                    role,
                    verb.is_some() || matches!(attr.as_str(), "route" | "api_route"),
                    verb,
                )
            }
            _ => (None, false, None),
        };
        let range = self.arena[id].range;
        if let Some(role) = role {
            let decorator = self.render(func)?;
            let value = self.encode_metadata(&Metadata::Entrypoint {
                decorator: &decorator,
                role,
            })?;
            self.emit("entrypoint", range, value, None)?;
            self.drop_text(decorator)?;
        }
        if !route {
            return Ok(());
        }
        self.charge(8)?;
        let mut methods = [false; 8];
        let (path_count, keyword_path) = self.keyword(id, "path")?;
        let Shape::Call { args, .. } = &self.arena[id].shape else {
            unreachable!()
        };
        let positional = args.first().copied();
        let path_id = if path_count + usize::from(positional.is_some()) == 1 {
            positional.or(keyword_path)
        } else {
            None
        };
        let path = self.retained(path_id, TextLimit::Route)?;
        let valid_path = path
            .as_deref()
            .is_some_and(|p| p.is_empty() || p[0].value() == 47);
        if !valid_path {
            self.route_warning(id, true)?;
        }
        if let Some(verb) = verb {
            methods[verb] = true;
        } else {
            let (_, method_id) = self.keyword(id, "methods")?;
            let mut valid = false;
            if let Some(method_id) = method_id {
                if matches!(self.arena[method_id].shape, Shape::Sequence) {
                    valid = true;
                    for i in 0..self.arena[method_id].children.len() {
                        let child = self.arena[method_id].children[i];
                        if let Some(method) = self.method(child)? {
                            methods[literals::METHODS
                                .iter()
                                .position(|m| *m == method)
                                .expect("whitelist")] = true;
                        } else {
                            valid = false;
                        }
                    }
                }
            }
            if !valid {
                methods.fill(false);
                self.route_warning(id, false)?;
            }
        }
        let Shape::Attribute(receiver, _) = self.arena[func].shape else {
            unreachable!()
        };
        let receiver = self.render(receiver)?;
        let value = self.encode_metadata(&Metadata::Route {
            methods: &methods,
            path: if valid_path { path.as_deref() } else { None },
            receiver: &receiver,
        })?;
        self.emit("api_route", range, value, None)?;
        self.drop_text(receiver)?;
        self.drop_points(path)?;
        self.release(8)
    }
}
