//! Parser-independent path scopes and helper-order scheduling primitives.
use sha2::{Digest, Sha256};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ScopeId(usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Error {
    Unsupported,
    WorkingLimit,
    Cancelled,
}

// A zero charge is a replay checkpoint. Positive charges precede allocation;
// callers retain charges on failure until their whole-file transaction rolls back.
type Result<T> = std::result::Result<T, Error>;

#[derive(Default)]
pub(super) struct AliasScope {
    parent: Option<ScopeId>,
    children: HashMap<[u8; 32], (String, ScopeId)>,
    aliases: HashMap<[u8; 32], (String, String)>,
    depth: usize,
}

#[derive(Default)]
pub(super) struct Projection {
    scopes: Vec<AliasScope>,
}

#[derive(Clone, Copy)]
pub(super) struct AliasId(ScopeId, [u8; 32]);

fn key(text: &str, work: &mut impl FnMut(usize) -> Result<()>) -> Result<[u8; 32]> {
    let mut hash = Sha256::new();
    for chunk in text.as_bytes().chunks(256) {
        work(0)?;
        hash.update(chunk);
    }
    Ok(hash.finalize().into())
}

fn same(a: &str, b: &str, work: &mut impl FnMut(usize) -> Result<()>) -> Result<bool> {
    if a.len() != b.len() {
        return Ok(false);
    }
    for (a, b) in a.as_bytes().chunks(256).zip(b.as_bytes().chunks(256)) {
        work(0)?;
        if a != b {
            return Ok(false);
        }
    }
    Ok(true)
}

fn copy(text: &str, work: &mut impl FnMut(usize) -> Result<()>) -> Result<String> {
    let mut value = String::with_capacity(text.len());
    for (i, ch) in text.chars().enumerate() {
        if i % 256 == 0 {
            work(0)?;
        }
        value.push(ch);
    }
    Ok(value)
}

impl Projection {
    pub(super) fn root(&mut self, work: &mut impl FnMut(usize) -> Result<()>) -> Result<()> {
        work(4 * std::mem::size_of::<AliasScope>())?;
        self.scopes.push(AliasScope::default());
        Ok(())
    }

    pub(super) fn enter(
        &mut self,
        parent: ScopeId,
        name: &str,
        max_depth: usize,
        work: &mut impl FnMut(usize) -> Result<()>,
    ) -> Result<ScopeId> {
        work(0)?;
        let scope = self.scopes.get(parent.0).ok_or(Error::Unsupported)?;
        if scope.depth >= max_depth {
            return Err(Error::WorkingLimit);
        }
        let depth = scope.depth + 1;
        let key = key(name, work)?;
        if let Some((stored, id)) = scope.children.get(&key) {
            return if same(stored, name, work)? {
                Ok(*id)
            } else {
                Err(Error::Unsupported)
            };
        }
        // Four logical slots cover initial/geometric vector and hash capacity,
        // including control bytes and transient old buffers during growth.
        work(
            4 * (std::mem::size_of::<AliasScope>()
                + std::mem::size_of::<([u8; 32], (String, ScopeId))>()
                + 64)
                + name.len(),
        )?;
        let name = copy(name, work)?;
        let id = ScopeId(self.scopes.len());
        self.scopes.push(AliasScope {
            parent: Some(parent),
            depth,
            ..AliasScope::default()
        });
        self.scopes[parent.0].children.insert(key, (name, id));
        Ok(id)
    }

    // Returns the replaced target's charge, released only after replacement.
    pub(super) fn import(
        &mut self,
        scope: ScopeId,
        local: &str,
        prefix: &str,
        name: &str,
        work: &mut impl FnMut(usize) -> Result<()>,
    ) -> Result<usize> {
        work(0)?;
        let key = key(local, work)?;
        let aliases = &mut self
            .scopes
            .get_mut(scope.0)
            .ok_or(Error::Unsupported)?
            .aliases;
        let old = if let Some((stored, target)) = aliases.get(&key) {
            if !same(stored, local, work)? {
                return Err(Error::Unsupported);
            }
            Some(target.len())
        } else {
            None
        };
        let size = prefix
            .len()
            .checked_add(usize::from(!prefix.is_empty()))
            .and_then(|n| n.checked_add(name.len()))
            .ok_or(Error::WorkingLimit)?;
        let entry = if old.is_none() {
            local.len() + 4 * (std::mem::size_of::<([u8; 32], (String, String))>() + 64)
        } else {
            0
        };
        work(size.checked_add(entry).ok_or(Error::WorkingLimit)?)?;
        let mut target = String::with_capacity(size);
        for part in [prefix, if prefix.is_empty() { "" } else { "." }, name] {
            for (i, ch) in part.chars().enumerate() {
                if i % 256 == 0 {
                    work(0)?;
                }
                target.push(ch);
            }
        }
        if let Some(old) = old {
            aliases.get_mut(&key).unwrap().1 = target;
            Ok(old)
        } else {
            aliases.insert(key, (copy(local, work)?, target));
            Ok(0)
        }
    }

    // Resolve exactly once at the nearest lexical prefix. Targets stay unclipped.
    pub(super) fn lookup(
        &self,
        mut scope: ScopeId,
        head: &str,
        work: &mut impl FnMut(usize) -> Result<()>,
    ) -> Result<Option<AliasId>> {
        let key = key(head, work)?;
        loop {
            work(0)?;
            let current = self.scopes.get(scope.0).ok_or(Error::Unsupported)?;
            if let Some((stored, _)) = current.aliases.get(&key) {
                if !same(stored, head, work)? {
                    return Err(Error::Unsupported);
                }
                return Ok(Some(AliasId(scope, key)));
            }
            let Some(parent) = current.parent else {
                return Ok(None);
            };
            scope = parent;
        }
    }

    pub(super) fn target(&self, id: AliasId) -> &str {
        &self.scopes[id.0 .0].aliases[&id.1].1
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DefinitionKind {
    Function,
    Class,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    Metadata,
    Arguments,
    Bases,
    Keywords,
    Body,
    Decorators,
    Return,
    TypeParameters,
}

pub(super) struct CallObservation {
    pub(super) line: usize,
    pub(super) end: usize,
    pub(super) value: String,
    // Deliberately unresolved until the scheduled visit, under its owner.
    pub(super) callee: String,
}

pub(super) struct Shape {
    pub(super) line: usize,
    pub(super) end: usize,
    pub(super) value: String,
}

pub(super) enum HeaderMetadata {
    Parameter {
        order: u8,
        name: String,
        line: usize,
        end: usize,
        annotation: Option<String>,
        default: Option<Shape>,
    },
    Base(Shape),
    Return(Shape),
}

pub(super) enum HeaderEvent {
    Metadata(HeaderMetadata),
    Visit(CallObservation),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HeaderGroup {
    PositionalOnlyAnnotations,
    PositionalAnnotations,
    VarargAnnotation,
    KeywordOnlyAnnotations,
    KeywordDefaults,
    KwargAnnotation,
    PositionalDefaults,
    Bases,
    Keywords,
    Decorators,
    ReturnAnnotation,
    TypeParameters,
}

impl HeaderGroup {
    pub(super) const fn index(self) -> usize {
        self as usize
    }
}

#[derive(Default)]
pub(super) struct HeaderSchedule {
    metadata: Vec<Option<HeaderMetadata>>,
    // Arguments/bases/keywords precede Body; decorators/return/type params follow.
    // Capture is not syntax admission; each frontend owns its header-call guards.
    visits: [Vec<CallObservation>; 12],
    state: u8,
    kind: Option<DefinitionKind>,
}

impl HeaderSchedule {
    pub(super) fn stage(
        &mut self,
        metadata: HeaderMetadata,
        text: usize,
        limit: usize,
        work: &mut impl FnMut(usize) -> Result<()>,
    ) -> Result<usize> {
        work(0)?;
        if self.state != 0 {
            return Err(Error::Unsupported);
        }
        if self.metadata.len() >= limit {
            return Err(Error::WorkingLimit);
        }
        // Lowering has already charged the incoming owned text. Retain that
        // charge (or a conservative replacement) plus container growth/transients.
        let charge = (4 * std::mem::size_of::<Option<HeaderMetadata>>())
            .checked_add(text)
            .ok_or(Error::WorkingLimit)?;
        work(charge)?;
        self.metadata.push(Some(metadata));
        Ok(charge)
    }

    pub(super) fn capture_visit(
        &mut self,
        group: HeaderGroup,
        call: CallObservation,
        limit: usize,
        work: &mut impl FnMut(usize) -> Result<()>,
    ) -> Result<()> {
        work(0)?;
        if self.state != 0 {
            return Err(Error::Unsupported);
        }
        let count = self
            .visits
            .iter()
            .try_fold(0usize, |n, group| n.checked_add(group.len()))
            .ok_or(Error::WorkingLimit)?;
        if count >= limit || call.value.len() > 512 || call.callee.len() > 512 {
            return Err(Error::WorkingLimit);
        }
        // Incoming text is already charged by lowering; this additional fixed
        // allowance conservatively double-charges its bounded retained strings.
        let charge = (4 * std::mem::size_of::<CallObservation>())
            .checked_add(1024)
            .ok_or(Error::WorkingLimit)?;
        work(charge)?;
        self.visits[group.index()].push(call);
        Ok(())
    }

    pub(super) fn before(
        &mut self,
        kind: DefinitionKind,
        check: &mut impl FnMut() -> Result<()>,
        emit: &mut impl FnMut(HeaderEvent) -> Result<()>,
    ) -> Result<()> {
        use HeaderGroup::*;
        check()?;
        if self.state != 0 {
            return Err(Error::Unsupported);
        }
        // Poison before consuming: a failed drain cannot be resumed as a prefix.
        self.state = 3;
        self.kind = Some(kind);
        let invalid = match kind {
            DefinitionKind::Function => {
                !self.visits[Bases.index()].is_empty() || !self.visits[Keywords.index()].is_empty()
            }
            DefinitionKind::Class => {
                self.visits[..Bases.index()]
                    .iter()
                    .any(|group| !group.is_empty())
                    || !self.visits[ReturnAnnotation.index()].is_empty()
            }
        };
        if invalid {
            return Err(Error::Unsupported);
        }
        for order in 0..=4 {
            for item in &mut self.metadata {
                check()?;
                let rank = match item {
                    Some(HeaderMetadata::Parameter { order, .. })
                        if kind == DefinitionKind::Function && *order <= 2 =>
                    {
                        *order
                    }
                    Some(HeaderMetadata::Base(_)) if kind == DefinitionKind::Class => 3,
                    Some(HeaderMetadata::Return(_)) if kind == DefinitionKind::Function => 4,
                    Some(_) => return Err(Error::Unsupported),
                    None => continue,
                };
                if rank == order {
                    emit(HeaderEvent::Metadata(item.take().unwrap()))?;
                }
            }
        }
        let groups: &[HeaderGroup] = match kind {
            // ast.arguments field order, distinct from parameter metadata order:
            // posonlyargs, args, vararg, kwonlyargs, kw_defaults, kwarg, defaults.
            DefinitionKind::Function => &[
                PositionalOnlyAnnotations,
                PositionalAnnotations,
                VarargAnnotation,
                KeywordOnlyAnnotations,
                KeywordDefaults,
                KwargAnnotation,
                PositionalDefaults,
            ],
            DefinitionKind::Class => &[Bases, Keywords],
        };
        for &group in groups {
            for call in std::mem::take(&mut self.visits[group.index()]) {
                check()?;
                emit(HeaderEvent::Visit(call))?;
            }
        }
        self.state = 1;
        Ok(())
    }

    pub(super) fn after(
        &mut self,
        kind: DefinitionKind,
        check: &mut impl FnMut() -> Result<()>,
        emit: &mut impl FnMut(HeaderEvent) -> Result<()>,
    ) -> Result<()> {
        use HeaderGroup::*;
        check()?;
        if self.state != 1 || self.kind != Some(kind) {
            return Err(Error::Unsupported);
        }
        self.state = 3;
        let groups: &[HeaderGroup] = match kind {
            DefinitionKind::Function => &[Decorators, ReturnAnnotation, TypeParameters],
            DefinitionKind::Class => &[Decorators, TypeParameters],
        };
        for &group in groups {
            for call in std::mem::take(&mut self.visits[group.index()]) {
                check()?;
                emit(HeaderEvent::Visit(call))?;
            }
        }
        self.state = 2;
        Ok(())
    }
}

#[cfg(test)]
impl DefinitionKind {
    // Metadata runs under the pushed owner, before generic_visit's field groups.
    pub(super) fn phases(self) -> impl Iterator<Item = Phase> {
        use Phase::*;
        let groups: &'static [Phase] = match self {
            Self::Function => &[
                Metadata,
                Arguments,
                Body,
                Decorators,
                Return,
                TypeParameters,
            ],
            Self::Class => &[Metadata, Bases, Keywords, Body, Decorators, TypeParameters],
        };
        groups.iter().copied()
    }
}

// Intrusive call chains have no queue allocation or renderer edges. A parent
// concatenates child chains in helper order, each edge a constant number of times.
#[derive(Clone, Copy, Default)]
pub(super) struct Calls {
    pub(super) first: Option<usize>,
    pub(super) last: Option<usize>,
}

impl Calls {
    pub(super) fn append(&mut self, other: Self, mut link: impl FnMut(usize, Option<usize>)) {
        if let Some(first) = other.first {
            if let Some(last) = self.last {
                link(last, Some(first));
            } else {
                self.first = Some(first);
            }
            self.last = other.last;
            link(other.last.unwrap(), None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unlimited(_: usize) -> Result<()> {
        Ok(())
    }

    fn target<'a>(p: &'a Projection, scope: ScopeId, head: &str) -> Option<&'a str> {
        p.lookup(scope, head, &mut unlimited)
            .unwrap()
            .map(|id| p.target(id))
    }

    #[test]
    fn projection_path_aliases_nearest_overwrite_once_and_restore() {
        let mut p = Projection::default();
        p.root(&mut unlimited).unwrap();
        let root = ScopeId::default();
        p.import(root, "probe", "outer", "eval", &mut unlimited)
            .unwrap();
        let owner = p.enter(root, "Owner", 4, &mut unlimited).unwrap();
        let inner = p.enter(owner, "inner", 4, &mut unlimited).unwrap();
        assert_eq!(target(&p, inner, "probe"), Some("outer.eval"));
        p.import(owner, "probe", "safe", "read", &mut unlimited)
            .unwrap();
        assert_eq!(target(&p, inner, "probe"), Some("safe.read"));
        assert_eq!(target(&p, root, "probe"), Some("outer.eval"));
        p.import(inner, "probe", "", "other", &mut unlimited)
            .unwrap();
        p.import(inner, "other", "nested", "exec", &mut unlimited)
            .unwrap();
        assert_eq!(target(&p, inner, "probe"), Some("other"));
        assert_eq!(p.enter(root, "Owner", 4, &mut unlimited).unwrap(), owner);
        assert_eq!(p.enter(owner, "inner", 4, &mut unlimited).unwrap(), inner);
        assert_eq!(target(&p, owner, "probe"), Some("safe.read"));
        let long = format!("{}.eval", "path".repeat(600));
        p.import(root, "long", "", &long, &mut unlimited).unwrap();
        assert_eq!(target(&p, inner, "long"), Some(long.as_str()));
    }

    #[test]
    fn projection_bounds_precede_allocation_and_collisions_fail_closed() {
        let mut p = Projection::default();
        assert_eq!(
            p.root(&mut |_| Err(Error::WorkingLimit)),
            Err(Error::WorkingLimit)
        );
        assert!(p.scopes.is_empty());
        p.root(&mut unlimited).unwrap();
        let root = ScopeId::default();
        assert_eq!(
            p.enter(root, "child", 0, &mut unlimited),
            Err(Error::WorkingLimit)
        );
        assert_eq!(
            p.enter(root, "child", 4, &mut |_| Err(Error::Cancelled)),
            Err(Error::Cancelled)
        );
        let mut checkpoints = 0;
        assert_eq!(
            p.enter(root, "child", 4, &mut |n| {
                checkpoints += 1;
                if n > 0 {
                    Err(Error::WorkingLimit)
                } else {
                    Ok(())
                }
            }),
            Err(Error::WorkingLimit)
        );
        assert!(checkpoints >= 3);
        assert_eq!(p.scopes.len(), 1);
        assert!(p.scopes[0].children.is_empty());
        assert_eq!(
            p.import(root, "probe", "", "eval", &mut |n| {
                if n > 0 {
                    Err(Error::WorkingLimit)
                } else {
                    Ok(())
                }
            }),
            Err(Error::WorkingLimit)
        );
        assert!(p.scopes[0].aliases.is_empty());
        let hash = key("probe", &mut unlimited).unwrap();
        p.scopes[0]
            .aliases
            .insert(hash, ("different".into(), "eval".into()));
        assert!(matches!(
            p.lookup(root, "probe", &mut unlimited),
            Err(Error::Unsupported)
        ));
        assert_eq!(
            p.import(root, "probe", "", "read", &mut unlimited),
            Err(Error::Unsupported)
        );
        p.scopes[0]
            .children
            .insert(hash, ("different".into(), root));
        assert_eq!(
            p.enter(root, "probe", 4, &mut unlimited),
            Err(Error::Unsupported)
        );
    }

    #[test]
    fn projection_future_definition_group_trace_is_not_header_admission() {
        use Phase::*;
        let mut p = Projection::default();
        p.root(&mut unlimited).unwrap();
        let root = ScopeId::default();
        p.import(root, "probe", "before", "eval", &mut unlimited)
            .unwrap();
        let owner = p.enter(root, "same", 8, &mut unlimited).unwrap();
        let mut trace = Vec::new();
        // Hand-authored future groups: frontend header calls remain unsupported.
        for phase in DefinitionKind::Function.phases() {
            if phase == Body {
                p.import(owner, "probe", "after", "exec", &mut unlimited)
                    .unwrap();
                let child = p.enter(owner, "nested", 8, &mut unlimited).unwrap();
                p.import(child, "probe", "child", "read", &mut unlimited)
                    .unwrap();
                assert_eq!(target(&p, child, "probe"), Some("child.read"));
            }
            trace.push((phase, target(&p, owner, "probe").unwrap().to_owned()));
        }
        assert_eq!(
            trace,
            [
                (Metadata, "before.eval".into()),
                (Arguments, "before.eval".into()),
                (Body, "after.exec".into()),
                (Decorators, "after.exec".into()),
                (Return, "after.exec".into()),
                (TypeParameters, "after.exec".into()),
            ]
        );
        assert_eq!(target(&p, root, "probe"), Some("before.eval"));
        // Class/function identity is intentionally absent from path keys.
        let repeated = p.enter(root, "same", 8, &mut unlimited).unwrap();
        assert_eq!(target(&p, repeated, "probe"), Some("after.exec"));
        assert_eq!(
            DefinitionKind::Class.phases().collect::<Vec<_>>(),
            [Metadata, Bases, Keywords, Body, Decorators, TypeParameters]
        );
    }

    fn observation(group: HeaderGroup) -> CallObservation {
        let group = group.index();
        CallObservation {
            line: group + 1,
            end: group + 1,
            value: format!("group{group}()"),
            callee: "probe".into(),
        }
    }

    #[test]
    fn phase_batch_owned_groups_share_the_collector_driver_and_resolve_at_drain() {
        use HeaderGroup::*;
        for kind in [DefinitionKind::Function, DefinitionKind::Class] {
            let mut p = Projection::default();
            p.root(&mut unlimited).unwrap();
            let root = ScopeId::default();
            p.import(root, "probe", "early", "eval", &mut unlimited)
                .unwrap();
            let owner = p.enter(root, "repeat", 8, &mut unlimited).unwrap();
            let mut schedule = HeaderSchedule::default();
            let groups: &[HeaderGroup] = match kind {
                DefinitionKind::Function => &[
                    PositionalOnlyAnnotations,
                    PositionalAnnotations,
                    VarargAnnotation,
                    KeywordOnlyAnnotations,
                    KeywordDefaults,
                    KwargAnnotation,
                    PositionalDefaults,
                    Decorators,
                    ReturnAnnotation,
                    TypeParameters,
                ],
                DefinitionKind::Class => &[Bases, Keywords, Decorators, TypeParameters],
            };
            for &group in groups.iter().rev() {
                schedule
                    .capture_visit(group, observation(group), 20, &mut unlimited)
                    .unwrap();
            }
            let mut trace = Vec::new();
            schedule
                .before(kind, &mut || Ok(()), &mut |event| {
                    let HeaderEvent::Visit(call) = event else {
                        panic!("unexpected metadata")
                    };
                    let id = p.lookup(owner, &call.callee, &mut unlimited)?.unwrap();
                    trace.push((call.value, p.target(id).to_owned()));
                    Ok(())
                })
                .unwrap();
            // Body callback: nested imports must not affect the restored owner.
            p.import(owner, "probe", "late", "exec", &mut unlimited)
                .unwrap();
            let child = p.enter(owner, "nested", 8, &mut unlimited).unwrap();
            p.import(child, "probe", "child", "read", &mut unlimited)
                .unwrap();
            assert_eq!(target(&p, child, "probe"), Some("child.read"));
            schedule
                .after(kind, &mut || Ok(()), &mut |event| {
                    let HeaderEvent::Visit(call) = event else {
                        panic!("unexpected metadata")
                    };
                    let id = p.lookup(owner, &call.callee, &mut unlimited)?.unwrap();
                    trace.push((call.value, p.target(id).to_owned()));
                    Ok(())
                })
                .unwrap();
            assert_eq!(
                trace,
                groups
                    .iter()
                    .map(|&group| (
                        format!("group{}()", group.index()),
                        if group.index() < Decorators.index() {
                            "early.eval".into()
                        } else {
                            "late.exec".into()
                        }
                    ))
                    .collect::<Vec<_>>()
            );
            assert_eq!(target(&p, root, "probe"), Some("early.eval"));
            assert_eq!(p.enter(root, "repeat", 8, &mut unlimited).unwrap(), owner);
            assert_eq!(target(&p, owner, "probe"), Some("late.exec"));
            assert_eq!(
                schedule.before(kind, &mut || Ok(()), &mut |_| panic!("consumed twice")),
                Err(Error::Unsupported)
            );
            assert_eq!(
                schedule.after(kind, &mut || Ok(()), &mut |_| panic!("consumed twice")),
                Err(Error::Unsupported)
            );
            assert!(schedule.visits.iter().all(Vec::is_empty));
        }
    }

    #[test]
    fn phase_batch_owned_group_bounds_and_failed_drains_are_not_resumable() {
        use HeaderGroup::PositionalOnlyAnnotations as Group;
        let mut schedule = HeaderSchedule::default();
        assert_eq!(
            schedule.after(DefinitionKind::Function, &mut || Ok(()), &mut |_| Ok(())),
            Err(Error::Unsupported)
        );
        assert_eq!(
            schedule.capture_visit(Group, observation(Group), 0, &mut unlimited),
            Err(Error::WorkingLimit)
        );
        assert_eq!(
            schedule.capture_visit(Group, observation(Group), 2, &mut |_| Err(Error::Cancelled)),
            Err(Error::Cancelled)
        );
        assert_eq!(
            schedule.capture_visit(Group, observation(Group), 2, &mut |n| if n > 0 {
                Err(Error::WorkingLimit)
            } else {
                Ok(())
            }),
            Err(Error::WorkingLimit)
        );
        assert!(schedule.visits.iter().all(Vec::is_empty));
        schedule
            .capture_visit(Group, observation(Group), 2, &mut unlimited)
            .unwrap();
        schedule
            .capture_visit(Group, observation(Group), 2, &mut unlimited)
            .unwrap();
        assert_eq!(
            schedule.capture_visit(Group, observation(Group), 2, &mut unlimited),
            Err(Error::WorkingLimit)
        );
        let mut consumed = 0;
        assert_eq!(
            schedule.before(DefinitionKind::Function, &mut || Ok(()), &mut |_| {
                consumed += 1;
                Err(Error::Cancelled)
            }),
            Err(Error::Cancelled)
        );
        assert_eq!(consumed, 1);
        assert_eq!(
            schedule.before(DefinitionKind::Function, &mut || Ok(()), &mut |_| panic!(
                "resumed failed drain"
            )),
            Err(Error::Unsupported)
        );
        assert_eq!(
            schedule.after(DefinitionKind::Function, &mut || Ok(()), &mut |_| panic!(
                "resumed failed drain"
            )),
            Err(Error::Unsupported)
        );
        assert_eq!(
            schedule.stage(
                HeaderMetadata::Return(Shape {
                    line: 1,
                    end: 1,
                    value: "R".into()
                }),
                1,
                2,
                &mut unlimited
            ),
            Err(Error::Unsupported)
        );
    }

    #[test]
    fn phase_batch_metadata_order_is_distinct_from_generic_argument_order() {
        use HeaderGroup::PositionalOnlyAnnotations as Group;
        let mut schedule = HeaderSchedule::default();
        for (order, name) in [(0, "a"), (1, "v"), (0, "k"), (2, "w")] {
            schedule
                .stage(
                    HeaderMetadata::Parameter {
                        order,
                        name: name.into(),
                        line: 1,
                        end: 1,
                        annotation: None,
                        default: None,
                    },
                    name.len(),
                    8,
                    &mut unlimited,
                )
                .unwrap();
        }
        schedule
            .stage(
                HeaderMetadata::Return(Shape {
                    line: 1,
                    end: 1,
                    value: "R".into(),
                }),
                1,
                8,
                &mut unlimited,
            )
            .unwrap();
        schedule
            .capture_visit(Group, observation(Group), 8, &mut unlimited)
            .unwrap();
        let mut trace = Vec::new();
        schedule
            .before(DefinitionKind::Function, &mut || Ok(()), &mut |event| {
                trace.push(match event {
                    HeaderEvent::Metadata(HeaderMetadata::Parameter { name, .. }) => name,
                    HeaderEvent::Metadata(HeaderMetadata::Return(shape)) => shape.value,
                    HeaderEvent::Visit(call) => call.value,
                    _ => panic!("unexpected class metadata"),
                });
                Ok(())
            })
            .unwrap();
        assert_eq!(trace, ["a", "k", "v", "w", "R", "group0()"]);
        assert!(schedule.metadata.iter().all(Option::is_none));
        assert_eq!(
            schedule.capture_visit(Group, observation(Group), 8, &mut unlimited),
            Err(Error::Unsupported)
        );
        assert_eq!(
            schedule.stage(
                HeaderMetadata::Return(Shape {
                    line: 1,
                    end: 1,
                    value: "R".into()
                }),
                1,
                8,
                &mut unlimited
            ),
            Err(Error::Unsupported)
        );
    }
}
