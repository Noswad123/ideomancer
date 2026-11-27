//! Shared observable Python output, independent of any parser or admission policy.
use super::python_scalars::{AsciiJson, JsonError};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::io::{self, Write};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(super) struct Fact {
    pub(super) id: String,
    pub(super) kind: String,
    pub(super) path: String,
    pub(super) line: usize,
    #[serde(rename = "endLine")]
    pub(super) end_line: usize,
    pub(super) symbol: String,
    pub(super) value: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(super) struct Warning {
    pub(super) code: &'static str,
    pub(super) path: String,
    pub(super) message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Failure {
    Cancelled,
    FactLimit,
    WarningLimit,
    OutputLimit,
    WorkingLimit,
}

impl From<JsonError> for Failure {
    fn from(error: JsonError) -> Self {
        match error {
            JsonError::Cancelled => Self::Cancelled,
            JsonError::OutputLimit | JsonError::Io(_) => Self::OutputLimit,
        }
    }
}

fn checkpoint(check: &mut impl FnMut() -> bool) -> Result<(), Failure> {
    if check() {
        Err(Failure::Cancelled)
    } else {
        Ok(())
    }
}

struct HashWriter<'a>(&'a mut Sha256);
impl Write for HashWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(super) fn ascii_json(
    text: &str,
    hash: &mut Sha256,
    check: &mut impl FnMut() -> bool,
) -> Result<(), Failure> {
    AsciiJson::new(HashWriter(hash), usize::MAX, check)
        .text(text)
        .map_err(Into::into)
}

#[derive(Clone, Copy, Serialize)]
pub(super) struct FactInput<'a> {
    pub(super) path: &'a str,
    pub(super) kind: &'a str,
    pub(super) line: usize,
    #[serde(rename = "endLine")]
    pub(super) end_line: usize,
    pub(super) symbol: &'a str,
    pub(super) value: &'a str,
}

/// No identity buffer and no digest on error. The caller must latch cancellation
/// for the entire file/run; a subsequent successful callback cannot redeem it.
pub(super) fn fact_id(
    fact: FactInput<'_>,
    check: &mut impl FnMut() -> bool,
) -> Result<String, Failure> {
    checkpoint(check)?;
    let mut hash = Sha256::new();
    if fact.kind == "module" {
        for chunk in fact.path.as_bytes().chunks(256) {
            checkpoint(check)?;
            hash.update(chunk);
        }
    } else {
        let mut writer = AsciiJson::new(HashWriter(&mut hash), usize::MAX, &mut *check);
        writer.raw(b"[")?;
        writer.text(fact.path)?;
        writer.raw(b", ")?;
        writer.text(fact.kind)?;
        writer.raw(format!(", {}, {}, ", fact.line, fact.end_line).as_bytes())?;
        writer.text(fact.symbol)?;
        writer.raw(b", ")?;
        writer.text(fact.value)?;
        writer.raw(b"]")?;
    }
    checkpoint(check)?;
    let prefix = if fact.kind == "module" {
        "module"
    } else {
        "fact"
    };
    Ok(format!("{prefix}_{:x}", hash.finalize()))
}

pub(super) struct Signature<'a> {
    pub(super) positional: &'a [String],
    pub(super) positional_only: usize,
    pub(super) keyword: &'a [String],
    pub(super) vararg: Option<&'a str>,
    pub(super) kwarg: Option<&'a str>,
}

/// Caller reserves 2 * 16384 + 17 working bytes before invoking this encoder.
pub(super) fn signature(
    input: Signature<'_>,
    check: &mut impl FnMut() -> bool,
) -> Result<String, Failure> {
    checkpoint(check)?;
    if input.positional.len().saturating_add(input.keyword.len()) > 128 {
        return Ok("{\"truncated\":true}".into());
    }
    let mut writer = AsciiJson::new(Vec::with_capacity(16384), 16384, check);
    let result = (|| {
        writer.raw(b"{\"keywordOnly\":[")?;
        for (i, name) in input.keyword.iter().enumerate() {
            if i != 0 {
                writer.raw(b",")?;
            }
            writer.text(name)?;
        }
        writer.raw(b"]")?;
        if let Some(name) = input.kwarg {
            writer.raw(b",\"kwarg\":")?;
            writer.text(name)?;
        }
        writer.raw(b",\"positional\":[")?;
        for (i, name) in input.positional.iter().enumerate() {
            if i != 0 {
                writer.raw(b",")?;
            }
            writer.text(name)?;
        }
        writer.raw(b"],\"positionalOnly\":")?;
        writer.raw(input.positional_only.to_string().as_bytes())?;
        if let Some(name) = input.vararg {
            writer.raw(b",\"vararg\":")?;
            writer.text(name)?;
        }
        writer.raw(b"}")
    })();
    match result {
        Ok(()) => Ok(
            String::from_utf8(writer.into_inner().into_boxed_slice().into_vec())
                .expect("ASCII signature"),
        ),
        Err(JsonError::OutputLimit) => Ok("{\"truncated\":true}".into()),
        Err(error) => Err(error.into()),
    }
}

pub(super) struct ImportAlias<'a> {
    pub(super) name: &'a str,
    pub(super) asname: Option<&'a str>,
}

pub(super) struct ImportMetadata<'a> {
    pub(super) module: &'a str,
    pub(super) level: usize,
    pub(super) names: &'a [ImportAlias<'a>],
}

/// Run first with CountWriter, charge the measured buffer before allocation,
/// then run with Vec::with_capacity(count). No JSON Value or alias copies.
pub(super) fn write_import<W: Write>(
    sink: W,
    input: &ImportMetadata<'_>,
    limit: usize,
    check: &mut impl FnMut() -> bool,
) -> Result<W, Failure> {
    let mut writer = AsciiJson::new(sink, limit, check);
    writer.raw(b"{\"level\":")?;
    writer.raw(input.level.to_string().as_bytes())?;
    writer.raw(b",\"module\":")?;
    writer.text(input.module)?;
    writer.raw(b",\"names\":[")?;
    for (i, alias) in input.names.iter().enumerate() {
        if i != 0 {
            writer.raw(b",")?;
        }
        writer.raw(b"{\"asname\":")?;
        if let Some(name) = alias.asname {
            writer.text(name)?;
        } else {
            writer.raw(b"null")?;
        }
        writer.raw(b",\"name\":")?;
        writer.text(alias.name)?;
        writer.raw(b"}")?;
    }
    writer.raw(b"]}")?;
    Ok(writer.into_inner())
}

pub(super) struct CountWriter {
    count: usize,
    limit: usize,
}
impl CountWriter {
    pub(super) fn new(limit: usize) -> Self {
        Self { count: 0, limit }
    }
    pub(super) fn count(&self) -> usize {
        self.count
    }
}
impl Write for CountWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = self
            .count
            .checked_add(bytes.len())
            .filter(|&n| n <= self.limit)
            .ok_or_else(|| io::Error::other("output limit"))?;
        self.count = count;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Measures the complete client report, including identity/files/private fields.
/// This is mandatory before publication, even when sink reservations succeeded.
pub(super) fn measure(
    value: &impl Serialize,
    limit: usize,
    check: &mut impl FnMut() -> bool,
) -> Result<usize, Failure> {
    struct Checked<'a, C> {
        count: CountWriter,
        check: &'a mut C,
        cancelled: bool,
    }
    impl<C: FnMut() -> bool> Write for Checked<'_, C> {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            for _ in bytes.chunks(256) {
                if (self.check)() {
                    self.cancelled = true;
                    return Err(io::Error::other("cancelled"));
                }
            }
            self.count.write(bytes)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    checkpoint(check)?;
    let mut writer = Checked {
        count: CountWriter::new(limit),
        check,
        cancelled: false,
    };
    let result = serde_json::to_writer(&mut writer, value);
    if writer.cancelled {
        return Err(Failure::Cancelled);
    }
    result.map_err(|_| Failure::OutputLimit)?;
    checkpoint(writer.check)?;
    Ok(writer.count.count())
}

#[derive(Clone, Copy)]
pub(super) struct SinkLimits {
    pub(super) facts: usize,
    pub(super) warnings: usize,
    pub(super) output: usize,
    pub(super) working: usize,
}
impl Default for SinkLimits {
    fn default() -> Self {
        Self {
            facts: 100_000,
            warnings: 100_000,
            output: 32 * 1024 * 1024,
            working: 32 * 1024 * 1024,
        }
    }
}

/// Marks are file-local and must be used in reverse order, before finalization.
/// Establish them only after reserving every source's module baseline.
#[derive(Clone, Copy)]
pub(super) struct FileMark {
    facts: usize,
    warnings: usize,
    used: usize,
    working: usize,
}

pub(super) struct FactSink {
    facts: Vec<Fact>,
    warnings: Vec<Warning>,
    limits: SinkLimits,
    used: usize,
    working: usize,
    // Container high-water charges survive rollback; record text is output-bound.
    storage: usize,
    fact_slots: usize,
    warning_slots: usize,
    baselines: usize,
}

impl FactSink {
    pub(super) fn new(limits: SinkLimits) -> Self {
        Self {
            facts: Vec::new(),
            warnings: Vec::new(),
            limits,
            used: 0,
            working: 0,
            storage: 0,
            fact_slots: 0,
            warning_slots: 0,
            baselines: 0,
        }
    }

    pub(super) fn facts(&self) -> &[Fact] {
        &self.facts
    }
    pub(super) fn warnings(&self) -> &[Warning] {
        &self.warnings
    }
    pub(super) fn working(&self) -> usize {
        self.working
    }
    pub(super) fn remaining_output(&self) -> usize {
        self.limits.output.saturating_sub(self.used)
    }

    /// Reserve wrapper/file/identity overhead before any baselines or parsing.
    /// Include array punctuation; records additionally reserve one comma each.
    pub(super) fn reserve_output_baseline(&mut self, bytes: usize) -> Result<(), Failure> {
        if bytes > self.remaining_output() {
            return Err(Failure::OutputLimit);
        }
        self.used += bytes;
        Ok(())
    }

    /// Pending normalization/scopes/renderers/temporaries only, not total RSS.
    /// Owned symbol/value/message must already be charged before construction.
    pub(super) fn charge(
        &mut self,
        bytes: usize,
        check: &mut impl FnMut() -> bool,
    ) -> Result<(), Failure> {
        checkpoint(check)?;
        self.working = self
            .working
            .checked_add(bytes)
            .filter(|&n| n <= self.limits.working.saturating_sub(self.storage))
            .ok_or(Failure::WorkingLimit)?;
        Ok(())
    }

    pub(super) fn release(&mut self, bytes: usize) {
        self.working = self
            .working
            .checked_sub(bytes)
            .expect("balanced working charge");
    }

    pub(super) fn mark(&self) -> FileMark {
        FileMark {
            facts: self.facts.len(),
            warnings: self.warnings.len(),
            used: self.used,
            working: self.working,
        }
    }

    pub(super) fn rollback(&mut self, mark: FileMark) {
        assert!(mark.facts >= self.baselines && mark.facts <= self.facts.len());
        assert!(mark.warnings <= self.warnings.len());
        self.facts.truncate(mark.facts);
        self.warnings.truncate(mark.warnings);
        self.used = mark.used;
        self.working = mark.working;
    }

    /// Call for every admitted path before taking any per-file mark. No parser
    /// status affects these labels. Failure means the batch cannot be admitted.
    pub(super) fn module_baseline(
        &mut self,
        path: &str,
        module: &str,
        check: &mut impl FnMut() -> bool,
    ) -> Result<(), Failure> {
        assert_eq!(self.facts.len(), self.baselines);
        let temporary = module.len().checked_mul(2).ok_or(Failure::WorkingLimit)?;
        self.charge(temporary, check)?;
        let result = (|| {
            // Bound the copies before constructing the owned incoming payload.
            self.preview(
                FactInput {
                    path,
                    kind: "module",
                    line: 1,
                    end_line: 1,
                    symbol: module,
                    value: module,
                },
                check,
            )?;
            let symbol = copy_text(module, check)?;
            let value = copy_text(module, check)?;
            self.emit(path, "module", 1, 1, symbol, value, check)
        })();
        self.release(temporary);
        result?;
        self.baselines += 1;
        Ok(())
    }

    fn preview(
        &self,
        input: FactInput<'_>,
        check: &mut impl FnMut() -> bool,
    ) -> Result<(usize, usize), Failure> {
        checkpoint(check)?;
        if self.facts.len() >= self.limits.facts {
            return Err(Failure::FactLimit);
        }
        #[derive(Serialize)]
        struct Preview<'a> {
            id: &'a str,
            #[serde(flatten)]
            input: FactInput<'a>,
        }
        let id = if input.kind == "module" {
            concat!(
                "module_",
                "0000000000000000000000000000000000000000000000000000000000000000"
            )
        } else {
            concat!(
                "fact_",
                "0000000000000000000000000000000000000000000000000000000000000000"
            )
        };
        let growth = if self.facts.len() == self.fact_slots {
            4 * std::mem::size_of::<Fact>()
        } else {
            0
        };
        if growth
            > self
                .limits
                .working
                .saturating_sub(self.storage)
                .saturating_sub(self.working)
        {
            return Err(Failure::WorkingLimit);
        }
        let available = self
            .remaining_output()
            .checked_sub(1)
            .ok_or(Failure::OutputLimit)?;
        Ok((
            measure(&Preview { id, input }, available, check)? + 1,
            growth,
        ))
    }

    /// Caller retains its symbol/value working charges until this returns, then
    /// releases them whether accepted or rejected. Charge their capacities, not
    /// just lengths. Compact retained strings so persistent text is output-bound.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn emit(
        &mut self,
        path: &str,
        kind: &str,
        line: usize,
        end_line: usize,
        symbol: String,
        value: String,
        check: &mut impl FnMut() -> bool,
    ) -> Result<(), Failure> {
        let input = FactInput {
            path,
            kind,
            line,
            end_line,
            symbol: &symbol,
            value: &value,
        };
        let (size, growth) = self.preview(input, check)?;
        let id = fact_id(input, check)?;
        let path = copy_text(path, check)?;
        let kind = copy_text(kind, check)?;
        let symbol = symbol.into_boxed_str().into_string();
        let value = value.into_boxed_str().into_string();
        checkpoint(check)?;
        self.facts.push(Fact {
            id,
            kind,
            path,
            line,
            end_line,
            symbol,
            value,
        });
        self.used += size;
        self.storage += growth;
        self.fact_slots = self.fact_slots.max(self.facts.len());
        Ok(())
    }

    /// As with emit, the incoming message capacity is caller-precharged until
    /// return. The persistent compact message/path are bounded by encoded output.
    pub(super) fn warn(
        &mut self,
        code: &'static str,
        path: &str,
        message: String,
        check: &mut impl FnMut() -> bool,
    ) -> Result<(), Failure> {
        checkpoint(check)?;
        if self.warnings.len() >= self.limits.warnings {
            return Err(Failure::WarningLimit);
        }
        #[derive(Serialize)]
        struct Preview<'a> {
            code: &'static str,
            path: &'a str,
            message: &'a str,
        }
        let growth = if self.warnings.len() == self.warning_slots {
            4 * std::mem::size_of::<Warning>()
        } else {
            0
        };
        if growth
            > self
                .limits
                .working
                .saturating_sub(self.storage)
                .saturating_sub(self.working)
        {
            return Err(Failure::WorkingLimit);
        }
        let available = self
            .remaining_output()
            .checked_sub(1)
            .ok_or(Failure::OutputLimit)?;
        let size = measure(
            &Preview {
                code,
                path,
                message: &message,
            },
            available,
            check,
        )? + 1;
        let path = copy_text(path, check)?;
        let message = message.into_boxed_str().into_string();
        checkpoint(check)?;
        self.warnings.push(Warning {
            code,
            path,
            message,
        });
        self.used += size;
        self.storage += growth;
        self.warning_slots = self.warning_slots.max(self.warnings.len());
        Ok(())
    }

    /// In-place sorting needs no scratch. ID dedup scratch is charged separately,
    /// with borrowed IDs (never a second fact array). An error invalidates the
    /// whole run, not merely the current file; do not publish partially sorted data.
    pub(super) fn finalize(&mut self, check: &mut impl FnMut() -> bool) -> Result<(), Failure> {
        let scratch = self
            .facts
            .len()
            .checked_mul(4 * (std::mem::size_of::<&str>() + 64) + 1)
            .ok_or(Failure::WorkingLimit)?;
        self.charge(scratch, check)?;
        let result = (|| {
            let mut ids = HashSet::new();
            let mut keep = Vec::with_capacity(self.facts.len());
            for fact in &self.facts {
                checkpoint(check)?;
                // IDs are bounded output, but hashing still needs scan checkpoints.
                for _ in fact.id.as_bytes().chunks(256) {
                    checkpoint(check)?;
                }
                keep.push(ids.insert(fact.id.as_str()));
            }
            drop(ids);
            let mut keep = keep.into_iter();
            let mut cancelled = false;
            self.facts.retain(|_| {
                cancelled |= check();
                let retain = keep.next().unwrap();
                cancelled || retain
            });
            if cancelled {
                return Err(Failure::Cancelled);
            }
            heap_sort(&mut self.facts, check, |a, b, check| {
                let order = compare_text(&a.path, &b.path, check)?;
                if !order.is_eq() {
                    return Ok(order);
                }
                let order = (a.line, a.end_line).cmp(&(b.line, b.end_line));
                if !order.is_eq() {
                    return Ok(order);
                }
                for (a, b) in [
                    (&a.kind, &b.kind),
                    (&a.symbol, &b.symbol),
                    (&a.value, &b.value),
                    (&a.id, &b.id),
                ] {
                    let order = compare_text(a, b, check)?;
                    if !order.is_eq() {
                        return Ok(order);
                    }
                }
                Ok(Ordering::Equal)
            })?;
            heap_sort(&mut self.warnings, check, |a, b, check| {
                for (a, b) in [
                    (a.path.as_str(), b.path.as_str()),
                    (a.code, b.code),
                    (a.message.as_str(), b.message.as_str()),
                ] {
                    let order = compare_text(a, b, check)?;
                    if !order.is_eq() {
                        return Ok(order);
                    }
                }
                Ok(Ordering::Equal)
            })
        })();
        self.release(scratch);
        result
    }

    pub(super) fn into_records(self) -> (Vec<Fact>, Vec<Warning>) {
        (self.facts, self.warnings)
    }

    /// Failure cleanup after a run-wide cancellation/sort error. Marks cannot be
    /// used after sorting: the original prefix may have moved. This bounded scan
    /// deliberately ignores an expired callback and retains only admitted source
    /// labels, in path order. No application-derived values survive this cleanup.
    pub(super) fn retain_baselines(&mut self) {
        self.facts.retain(|fact| fact.kind == "module");
        self.warnings.clear();
        self.facts.sort_unstable_by(|a, b| a.path.cmp(&b.path));
        // Reservations stay conservative; measure the final fallback report too.
    }
}

fn copy_text(text: &str, check: &mut impl FnMut() -> bool) -> Result<String, Failure> {
    let mut result = String::with_capacity(text.len());
    for (i, ch) in text.chars().enumerate() {
        if i % 256 == 0 {
            checkpoint(check)?;
        }
        result.push(ch);
    }
    Ok(result)
}

fn compare_text(a: &str, b: &str, check: &mut impl FnMut() -> bool) -> Result<Ordering, Failure> {
    for (a, b) in a.as_bytes().chunks(256).zip(b.as_bytes().chunks(256)) {
        checkpoint(check)?;
        let order = a.cmp(b);
        if !order.is_eq() {
            return Ok(order);
        }
    }
    Ok(a.len().cmp(&b.len()))
}

fn heap_sort<T, C: FnMut() -> bool>(
    values: &mut [T],
    check: &mut C,
    compare: impl Fn(&T, &T, &mut C) -> Result<Ordering, Failure>,
) -> Result<(), Failure> {
    fn sift<T, C: FnMut() -> bool>(
        values: &mut [T],
        mut root: usize,
        end: usize,
        check: &mut C,
        compare: &impl Fn(&T, &T, &mut C) -> Result<Ordering, Failure>,
    ) -> Result<(), Failure> {
        loop {
            checkpoint(check)?;
            let left = root * 2 + 1;
            if left >= end {
                return Ok(());
            }
            let mut child = left;
            if left + 1 < end && compare(&values[left], &values[left + 1], check)?.is_lt() {
                child += 1;
            }
            if !compare(&values[root], &values[child], check)?.is_lt() {
                return Ok(());
            }
            values.swap(root, child);
            root = child;
        }
    }
    checkpoint(check)?;
    let len = values.len();
    for root in (0..len / 2).rev() {
        sift(values, root, len, check, &compare)?;
    }
    for end in (1..values.len()).rev() {
        checkpoint(check)?;
        values.swap(0, end);
        sift(values, 0, end, check, &compare)?;
    }
    checkpoint(check)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input<'a>(kind: &'a str, value: &'a str) -> FactInput<'a> {
        FactInput {
            path: "sample.py",
            kind,
            line: 2,
            end_line: 4,
            symbol: "sample.f",
            value,
        }
    }

    #[test]
    fn shared_ids_preserve_spaced_ascii_identity_and_path_only_modules() {
        let value = "{\"keywordOnly\":[],\"positional\":[\"x\"],\"positionalOnly\":0}";
        assert_eq!(
            fact_id(input("signature", value), &mut || false).unwrap(),
            "fact_4c2230a6b0ddf75dd5b169675fd2b4c61806e40ff1239be8faf133a48b128511"
        );
        let module = fact_id(input("module", "ignored"), &mut || false).unwrap();
        assert_eq!(module, format!("module_{:x}", Sha256::digest(b"sample.py")));
        assert_eq!(
            module,
            fact_id(input("module", "changed"), &mut || false).unwrap()
        );
        let value = "\u{e9}\u{1f600}";
        assert_eq!(
            fact_id(input("call", value), &mut || false).unwrap(),
            format!(
                "fact_{:x}",
                Sha256::digest(br#"["sample.py", "call", 2, 4, "sample.f", "\u00e9\ud83d\ude00"]"#)
            )
        );
        let mut calls = 0;
        assert_eq!(
            fact_id(input("call", value), &mut || {
                calls += 1;
                calls == 8
            }),
            Err(Failure::Cancelled)
        );
    }

    #[test]
    fn shared_signature_sorted_ascii_optional_fields_and_limits() {
        let positional = ["z\u{e9}".into(), "a".into()];
        let keyword = ["k".into()];
        assert_eq!(
            signature(
                Signature {
                    positional: &positional,
                    positional_only: 1,
                    keyword: &keyword,
                    vararg: Some("v"),
                    kwarg: Some("w")
                },
                &mut || false
            )
            .unwrap(),
            r#"{"keywordOnly":["k"],"kwarg":"w","positional":["z\u00e9","a"],"positionalOnly":1,"vararg":"v"}"#
        );
        let positional = vec!["x".into(); 128];
        assert_ne!(
            signature(
                Signature {
                    positional: &positional,
                    positional_only: 0,
                    keyword: &[],
                    vararg: None,
                    kwarg: None
                },
                &mut || false
            )
            .unwrap(),
            "{\"truncated\":true}"
        );
        let keyword = ["extra".into()];
        assert_eq!(
            signature(
                Signature {
                    positional: &positional,
                    positional_only: 0,
                    keyword: &keyword,
                    vararg: None,
                    kwarg: None
                },
                &mut || false
            )
            .unwrap(),
            "{\"truncated\":true}"
        );
        let positional = ["x".repeat(16384)];
        assert_eq!(
            signature(
                Signature {
                    positional: &positional,
                    positional_only: 0,
                    keyword: &[],
                    vararg: None,
                    kwarg: None
                },
                &mut || false
            )
            .unwrap(),
            "{\"truncated\":true}"
        );
        let empty_name = [String::new()];
        let overhead = signature(
            Signature {
                positional: &empty_name,
                positional_only: 0,
                keyword: &[],
                vararg: None,
                kwarg: None,
            },
            &mut || false,
        )
        .unwrap()
        .len();
        let positional = ["x".repeat(16384 - overhead)];
        assert_eq!(
            signature(
                Signature {
                    positional: &positional,
                    positional_only: 0,
                    keyword: &[],
                    vararg: None,
                    kwarg: None
                },
                &mut || false
            )
            .unwrap()
            .len(),
            16384
        );
        assert_eq!(
            signature(
                Signature {
                    positional: &[],
                    positional_only: 0,
                    keyword: &[],
                    vararg: None,
                    kwarg: None
                },
                &mut || true
            ),
            Err(Failure::Cancelled)
        );
    }

    #[test]
    fn shared_import_compact_sorted_ascii_null_and_count_pass() {
        let names = [
            ImportAlias {
                name: "n\u{1f600}",
                asname: None,
            },
            ImportAlias {
                name: "other",
                asname: Some("a\u{e9}"),
            },
        ];
        let metadata = ImportMetadata {
            module: "p\u{e9}",
            level: 2,
            names: &names,
        };
        let expected = br#"{"level":2,"module":"p\u00e9","names":[{"asname":null,"name":"n\ud83d\ude00"},{"asname":"a\u00e9","name":"other"}]}"#;
        let count = write_import(CountWriter::new(4096), &metadata, 4096, &mut || false)
            .unwrap()
            .count();
        assert_eq!(count, expected.len());
        assert_eq!(
            write_import(Vec::with_capacity(count), &metadata, count, &mut || false).unwrap(),
            expected.as_slice()
        );
        assert!(matches!(
            write_import(Vec::new(), &metadata, count - 1, &mut || false),
            Err(Failure::OutputLimit)
        ));
        let mut calls = 0;
        assert!(matches!(
            write_import(CountWriter::new(4096), &metadata, 4096, &mut || {
                calls += 1;
                calls == 12
            }),
            Err(Failure::Cancelled)
        ));
    }

    fn emit(sink: &mut FactSink, line: usize, value: &str) -> Result<(), Failure> {
        sink.charge(value.len() + 1, &mut || false)?;
        let result = sink.emit(
            "sample.py",
            "call",
            line,
            line,
            "s".into(),
            value.into(),
            &mut || false,
        );
        sink.release(value.len() + 1);
        result
    }

    #[test]
    fn shared_sink_rollback_keeps_all_baselines_and_resets_pending() {
        let mut sink = FactSink::new(SinkLimits::default());
        sink.reserve_output_baseline(100).unwrap();
        sink.module_baseline("a.py", "a", &mut || false).unwrap();
        sink.module_baseline("b.py", "b", &mut || false).unwrap();
        let mark = sink.mark();
        let remaining = sink.remaining_output();
        sink.charge(500, &mut || false).unwrap();
        emit(&mut sink, 2, "f()").unwrap();
        sink.warn("dynamic", "a.py", "warning".into(), &mut || false)
            .unwrap();
        sink.rollback(mark);
        assert_eq!(sink.facts().len(), 2);
        assert!(sink.warnings().is_empty());
        assert_eq!(sink.working(), 0);
        assert_eq!(sink.remaining_output(), remaining);
        assert!(sink.storage > 0); // Capacity survives, so its charge must too.
    }

    #[test]
    fn shared_sink_dedups_ids_only_sorts_and_retains_duplicate_warnings() {
        let mut sink = FactSink::new(SinkLimits::default());
        sink.module_baseline("sample.py", "sample", &mut || false)
            .unwrap();
        emit(&mut sink, 3, "z()").unwrap();
        emit(&mut sink, 2, "b()").unwrap();
        emit(&mut sink, 2, "a()").unwrap();
        emit(&mut sink, 2, "a()").unwrap();
        for (path, code, message) in [
            ("z.py", "a", "a"),
            ("a.py", "b", "b"),
            ("a.py", "a", "z"),
            ("a.py", "a", "z"),
            ("a.py", "a", "a"),
        ] {
            sink.warn(code, path, message.into(), &mut || false)
                .unwrap();
        }
        sink.finalize(&mut || false).unwrap();
        assert_eq!(
            sink.facts()
                .iter()
                .map(|f| (f.line, f.value.as_str()))
                .collect::<Vec<_>>(),
            [(1, "sample"), (2, "a()"), (2, "b()"), (3, "z()")]
        );
        assert_eq!(
            sink.warnings()
                .iter()
                .map(|w| (w.path.as_str(), w.code, w.message.as_str()))
                .collect::<Vec<_>>(),
            [
                ("a.py", "a", "a"),
                ("a.py", "a", "z"),
                ("a.py", "a", "z"),
                ("a.py", "b", "b"),
                ("z.py", "a", "a")
            ]
        );
        assert_eq!(sink.working(), 0);
        let (facts, warnings) = sink.into_records();
        assert_eq!((facts.len(), warnings.len()), (4, 5));
    }

    #[test]
    fn shared_sink_bounds_refuse_before_record_growth() {
        let mut sink = FactSink::new(SinkLimits {
            facts: 1,
            warnings: 0,
            ..SinkLimits::default()
        });
        sink.module_baseline("sample.py", "sample", &mut || false)
            .unwrap();
        assert_eq!(emit(&mut sink, 1, "f()"), Err(Failure::FactLimit));
        assert_eq!(
            sink.warn("code", "p", "m".into(), &mut || false),
            Err(Failure::WarningLimit)
        );
        assert_eq!(sink.facts().len(), 1);
        let mut sink = FactSink::new(SinkLimits {
            output: 1,
            ..SinkLimits::default()
        });
        assert_eq!(emit(&mut sink, 1, "f()"), Err(Failure::OutputLimit));
        assert!(sink.facts().is_empty());
        assert_eq!(
            sink.reserve_output_baseline(usize::MAX),
            Err(Failure::OutputLimit)
        );
        let mut sink = FactSink::new(SinkLimits {
            working: 1,
            ..SinkLimits::default()
        });
        assert_eq!(
            sink.charge(usize::MAX, &mut || false),
            Err(Failure::WorkingLimit)
        );
        assert_eq!(sink.working(), 0);
        assert_eq!(
            sink.emit("p", "call", 1, 1, String::new(), String::new(), &mut || {
                false
            }),
            Err(Failure::WorkingLimit)
        );
        assert!(sink.facts().is_empty());
    }

    #[test]
    fn shared_sink_cancelled_sort_requires_baseline_fallback_and_releases_scratch() {
        let mut sink = FactSink::new(SinkLimits::default());
        sink.module_baseline("sample.py", "sample", &mut || false)
            .unwrap();
        for line in (2..20).rev() {
            emit(&mut sink, line, "f()").unwrap();
        }
        let mut calls = 0;
        assert_eq!(
            sink.finalize(&mut || {
                calls += 1;
                calls == 80
            }),
            Err(Failure::Cancelled)
        );
        assert_eq!(sink.working(), 0);
        sink.retain_baselines();
        assert_eq!(sink.facts().len(), 1);
        assert_eq!(sink.facts()[0].kind, "module");
        assert!(sink.warnings().is_empty());
        assert_eq!(
            sink.emit("p", "call", 1, 1, "s".into(), "f()".into(), &mut || true),
            Err(Failure::Cancelled)
        );
        assert_eq!(sink.facts().len(), 1);
    }

    #[test]
    fn shared_measure_includes_wrapper_overhead_and_cancelled_writes() {
        #[derive(Serialize)]
        struct Report<'a> {
            facts: &'a [Fact],
            private_evidence: &'a str,
        }
        let sink = FactSink::new(SinkLimits::default());
        let report = Report {
            facts: sink.facts(),
            private_evidence: "\"\u{e9}",
        };
        let expected = serde_json::to_vec(&report).unwrap().len();
        assert_eq!(measure(&report, expected, &mut || false), Ok(expected));
        assert_eq!(
            measure(&report, expected - 1, &mut || false),
            Err(Failure::OutputLimit)
        );
        let mut calls = 0;
        assert_eq!(
            measure(&report, expected, &mut || {
                calls += 1;
                calls == 4
            }),
            Err(Failure::Cancelled)
        );
    }
}
