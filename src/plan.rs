//! Human software planning against a saved proposal. No I/O or execution authority.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Plan {
    pub schema_version: String,
    pub id: String,
    pub title: String,
    pub question: String,
    pub architecture: Architecture,
    pub scope: Scope,
    pub references: Vec<Reference>,
    pub observations: Vec<Observation>,
    pub requirements: Vec<Requirement>,
    pub questions: Vec<Question>,
    pub options: Vec<DesignOption>,
    pub decisions: Vec<Decision>,
    pub recommendation: Option<Recommendation>,
    pub work_items: Vec<WorkItem>,
    pub verification: Vec<Verification>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Architecture {
    pub run_directory: String,
    pub snapshot_id: String,
    pub proposal_sha256: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Scope {
    pub in_scope: Vec<String>,
    pub out_of_scope: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceKind {
    Source,
    TestSource,
    Constraint,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Reference {
    pub id: String,
    pub kind: ReferenceKind,
    pub path: String,
    pub line: usize,
    pub end_line: usize,
    pub note: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Observation {
    pub id: String,
    pub statement: String,
    pub fact_ids: Vec<String>,
    pub reference_ids: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RequirementStatus {
    Proposed,
    Confirmed,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Requirement {
    pub id: String,
    pub statement: String,
    pub status: RequirementStatus,
    pub confirmed_by: Option<String>,
    pub observation_ids: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QuestionStatus {
    Open,
    Resolved,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Question {
    pub id: String,
    pub question: String,
    pub status: QuestionStatus,
    pub answer: Option<String>,
    pub reference_ids: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesignOption {
    pub id: String,
    pub title: String,
    pub description: String,
    pub pros: Vec<String>,
    pub cons: Vec<String>,
    pub reference_ids: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Decision {
    pub id: String,
    pub question: String,
    pub option_ids: Vec<String>,
    pub selected_option_id: Option<String>,
    pub selected_by: Option<String>,
    pub rationale: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Recommendation {
    pub option_id: String,
    pub rationale: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkItem {
    pub id: String,
    pub title: String,
    pub description: String,
    pub component_ids: Vec<String>,
    pub requirement_ids: Vec<String>,
    pub decision_ids: Vec<String>,
    pub depends_on: Vec<String>,
    pub verification_ids: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VerificationKind {
    SourceReview,
    AutomatedTest,
    EnvironmentCheck,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Verification {
    pub id: String,
    pub kind: VerificationKind,
    pub check: String,
    pub expected: String,
    pub reference_ids: Vec<String>,
    pub requirement_ids: Vec<String>,
}

pub struct ArchitectureContext {
    pub snapshot_id: String,
    pub proposal_sha256: String,
    pub status: String,
    pub component_ids: BTreeSet<String>,
    pub facts: BTreeMap<String, FactReference>,
}

pub struct FactReference {
    pub path: String,
    pub line: usize,
    pub end_line: usize,
    pub kind: String,
    pub symbol: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Review {
    pub valid: bool,
    pub ready_for_plan_review: bool,
    pub errors: Vec<String>,
    pub blockers: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn parse(source: &str) -> Result<Plan, String> {
    serde_json::from_str(source).map_err(|e| format!("Invalid plan JSON: {e}"))
}

fn safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id.bytes().enumerate().all(|(i, c)| {
            c.is_ascii_alphabetic() || c == b'_' || (i > 0 && (c.is_ascii_digit() || c == b'-'))
        })
}

fn links(ids: &[String], targets: &BTreeSet<&str>, label: &str, errors: &mut Vec<String>) {
    for id in ids {
        if !safe_id(id) || !targets.contains(id.as_str()) {
            errors.push(format!("{label}: unknown or invalid ID {id:?}"));
        }
    }
}

// Walk only the fixed serialized schema to apply uniform text and collection limits.
fn bounds(value: &serde_json::Value, path: &str, errors: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) if s.trim().is_empty() || s.chars().count() > 16_384 => {
            errors.push(format!(
                "{path}: text must be nonblank and at most 16384 characters"
            ));
        }
        serde_json::Value::Array(items) => {
            if items.len() > 128 {
                errors.push(format!("{path}: at most 128 items allowed"));
                return;
            }
            for (i, item) in items.iter().enumerate() {
                bounds(item, &format!("{path}[{i}]"), errors);
            }
        }
        serde_json::Value::Object(fields) => {
            for (key, item) in fields {
                bounds(item, &format!("{path}.{key}"), errors);
            }
        }
        _ => {}
    }
}

pub fn review(plan: &Plan, context: &ArchitectureContext) -> Review {
    let mut result = Review {
        valid: false,
        ready_for_plan_review: false,
        errors: vec![],
        blockers: vec![],
        warnings: vec![],
    };
    let errors = &mut result.errors;
    bounds(
        &serde_json::to_value(plan).expect("fixed plan schema is serializable"),
        "plan",
        errors,
    );
    if !errors.is_empty() {
        return result;
    }
    if plan.schema_version != "ideomancer.plan.v1" {
        errors.push("Unsupported schemaVersion".into());
    }
    if !safe_id(&plan.id) {
        errors.push("Plan id must be a safe ASCII identifier of at most 128 characters".into());
    }
    if plan.architecture.snapshot_id != context.snapshot_id {
        errors.push("Architecture snapshotId mismatch".into());
    }
    if plan.architecture.proposal_sha256 != context.proposal_sha256 {
        errors.push("Architecture proposalSha256 mismatch".into());
    }
    if !matches!(context.status.as_str(), "proposal_ready" | "accepted") {
        errors.push("Architecture status must be proposal_ready or accepted".into());
    } else if context.status == "proposal_ready" {
        result.warnings.push("Architecture proposal is not accepted; design discussion is allowed, implementation is not authorized.".into());
    }
    let refs: BTreeSet<_> = plan.references.iter().map(|x| x.id.as_str()).collect();
    let obs: BTreeSet<_> = plan.observations.iter().map(|x| x.id.as_str()).collect();
    let reqs: BTreeSet<_> = plan.requirements.iter().map(|x| x.id.as_str()).collect();
    let options: BTreeSet<_> = plan.options.iter().map(|x| x.id.as_str()).collect();
    let decisions: BTreeSet<_> = plan.decisions.iter().map(|x| x.id.as_str()).collect();
    let work: BTreeSet<_> = plan.work_items.iter().map(|x| x.id.as_str()).collect();
    let gates: BTreeSet<_> = plan.verification.iter().map(|x| x.id.as_str()).collect();
    let facts: BTreeSet<_> = context.facts.keys().map(String::as_str).collect();
    let components: BTreeSet<_> = context.component_ids.iter().map(String::as_str).collect();
    let mut all = BTreeSet::new();
    // Iterate the original collections, not sets, so within-collection duplicates survive.
    for id in plan
        .references
        .iter()
        .map(|x| &x.id)
        .chain(plan.observations.iter().map(|x| &x.id))
        .chain(plan.requirements.iter().map(|x| &x.id))
        .chain(plan.questions.iter().map(|x| &x.id))
        .chain(plan.options.iter().map(|x| &x.id))
        .chain(plan.decisions.iter().map(|x| &x.id))
        .chain(plan.work_items.iter().map(|x| &x.id))
        .chain(plan.verification.iter().map(|x| &x.id))
    {
        if !safe_id(id) {
            errors.push(format!(
                "Invalid local ID {id:?}: use at most 128 safe ASCII characters"
            ));
        }
        if !all.insert(id) {
            errors.push(format!("Duplicate local ID {id:?}"));
        }
    }
    for r in &plan.references {
        if r.line == 0 || r.end_line < r.line {
            errors.push(format!("{}: invalid reference line range", r.id));
        }
    }
    if !plan.references.is_empty() {
        result.warnings.push("Manual citations are unverified by this command: cited paths were not opened, cited source/test definitions were not read, and no tests were executed. Author review is separate; constraints are not automatic truth or runtime proof.".into());
    }
    for o in &plan.observations {
        links(&o.fact_ids, &facts, &format!("{} factIds", o.id), errors);
        links(
            &o.reference_ids,
            &refs,
            &format!("{} referenceIds", o.id),
            errors,
        );
        if o.fact_ids.is_empty() && o.reference_ids.is_empty() {
            errors.push(format!(
                "{}: observation needs a fact or manual citation",
                o.id
            ));
        }
    }
    for r in &plan.requirements {
        links(
            &r.observation_ids,
            &obs,
            &format!("{} observationIds", r.id),
            errors,
        );
        match r.status {
            RequirementStatus::Confirmed if r.confirmed_by.is_none() => {
                errors.push(format!("{}: confirmed requirement needs confirmedBy", r.id))
            }
            RequirementStatus::Proposed => {
                if r.confirmed_by.is_some() {
                    errors.push(format!(
                        "{}: proposed requirement cannot contain confirmedBy",
                        r.id
                    ));
                }
                result.blockers.push(format!(
                    "{}: requirement is proposed, not confirmed and attributed",
                    r.id
                ));
            }
            _ => {}
        }
    }
    for q in &plan.questions {
        links(
            &q.reference_ids,
            &refs,
            &format!("{} referenceIds", q.id),
            errors,
        );
        if q.status == QuestionStatus::Resolved && q.answer.is_none() {
            errors.push(format!("{}: resolved question needs an answer", q.id));
        }
        if q.status == QuestionStatus::Open {
            result
                .blockers
                .push(format!("{}: question remains open", q.id));
        }
    }
    for o in &plan.options {
        links(
            &o.reference_ids,
            &refs,
            &format!("{} referenceIds", o.id),
            errors,
        );
    }
    for d in &plan.decisions {
        links(
            &d.option_ids,
            &options,
            &format!("{} optionIds", d.id),
            errors,
        );
        if d.option_ids.is_empty() {
            errors.push(format!("{}: decision needs at least one option", d.id));
        }
        if let Some(selected) = &d.selected_option_id {
            links(
                std::slice::from_ref(selected),
                &options,
                &format!("{} selectedOptionId", d.id),
                errors,
            );
            if !d.option_ids.contains(selected) {
                errors.push(format!("{}: selection is not in optionIds", d.id));
            }
            if d.selected_by.is_none() || d.rationale.is_none() {
                errors.push(format!(
                    "{}: selection needs selectedBy and rationale",
                    d.id
                ));
            }
        } else {
            if d.selected_by.is_some() || d.rationale.is_some() {
                errors.push(format!(
                    "{}: unselected decision cannot contain selectedBy or rationale",
                    d.id
                ));
            }
            result
                .blockers
                .push(format!("{}: decision has no selection", d.id));
        }
    }
    if let Some(r) = &plan.recommendation {
        links(
            std::slice::from_ref(&r.option_id),
            &options,
            "Recommendation optionId",
            errors,
        );
    }
    for w in &plan.work_items {
        links(
            &w.component_ids,
            &components,
            &format!("{} componentIds", w.id),
            errors,
        );
        links(
            &w.requirement_ids,
            &reqs,
            &format!("{} requirementIds", w.id),
            errors,
        );
        links(
            &w.decision_ids,
            &decisions,
            &format!("{} decisionIds", w.id),
            errors,
        );
        links(&w.depends_on, &work, &format!("{} dependsOn", w.id), errors);
        links(
            &w.verification_ids,
            &gates,
            &format!("{} verificationIds", w.id),
            errors,
        );
        for (name, missing) in [
            ("requirement", w.requirement_ids.is_empty()),
            ("decision", w.decision_ids.is_empty()),
            ("verification gate", w.verification_ids.is_empty()),
        ] {
            if missing {
                result
                    .blockers
                    .push(format!("{}: work item needs at least one {name}", w.id));
            }
        }
    }
    // Bounded dependency elimination detects self-dependencies and longer cycles.
    let mut remaining: BTreeSet<_> = work.clone();
    loop {
        let removable: Vec<_> = plan
            .work_items
            .iter()
            .filter(|w| {
                remaining.contains(w.id.as_str())
                    && w.depends_on.iter().all(|d| !remaining.contains(d.as_str()))
            })
            .map(|w| w.id.as_str())
            .collect();
        if removable.is_empty() {
            break;
        }
        for id in removable {
            remaining.remove(id);
        }
    }
    if !remaining.is_empty() {
        errors
            .push("Work dependencies contain a cycle (including possible self-dependency)".into());
    }
    for v in &plan.verification {
        links(
            &v.reference_ids,
            &refs,
            &format!("{} referenceIds", v.id),
            errors,
        );
        links(
            &v.requirement_ids,
            &reqs,
            &format!("{} requirementIds", v.id),
            errors,
        );
    }
    for (section, empty) in [
        ("observations", plan.observations.is_empty()),
        ("requirements", reqs.is_empty()),
        ("options", options.is_empty()),
        ("decisions", decisions.is_empty()),
        ("workItems", work.is_empty()),
        ("verification", gates.is_empty()),
    ] {
        if empty {
            result
                .blockers
                .push(format!("Missing planning section: {section}"));
        }
    }
    result.valid = result.errors.is_empty();
    result.ready_for_plan_review = result.valid && result.blockers.is_empty();
    result
}

// Keep prose and paths readable; escape markup, HTML, protocol/directive colons,
// and controls so user text cannot introduce links, blocks, or table rows.
struct Escaped<'a>(&'a str);

fn escape(text: &str) -> Escaped<'_> {
    Escaped(text)
}

impl std::fmt::Display for Escaped<'_> {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut whitespace_prefix = true;
        let mut numeric_prefix = true;
        let mut has_digit = false;
        let mut trailing_whitespace = false;
        for c in self.0.chars() {
            if matches!(c, '<' | '>' | '&' | ':') || c.is_control() {
                write!(out, "&#{};", c as u32)?;
            } else {
                let block_marker = matches!(c, '-' | '+' | '=') && whitespace_prefix;
                let ordered_marker = c == '.' && numeric_prefix && has_digit;
                if matches!(
                    c,
                    '\\' | '`' | '*' | '_' | '[' | ']' | '!' | '#' | '|' | '(' | ')' | '~'
                ) || block_marker
                    || ordered_marker
                {
                    out.write_char('\\')?;
                }
                out.write_char(c)?;
            }
            // Equivalent to testing the trimmed prefix, without rescanning it.
            if !c.is_whitespace() {
                whitespace_prefix = false;
                numeric_prefix &= c.is_ascii_digit() && !trailing_whitespace;
                has_digit |= c.is_ascii_digit();
            } else if !whitespace_prefix {
                // Trailing whitespace is ignored by trim; an ensuing digit
                // makes that whitespace internal and invalidates the prefix.
                trailing_whitespace = true;
            }
        }
        Ok(())
    }
}

struct Listed<'a>(&'a [String]);

fn listed(items: &[String]) -> Listed<'_> {
    Listed(items)
}

impl std::fmt::Display for Listed<'_> {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0.is_empty() {
            return out.write_str("(none)");
        }
        for (i, item) in self.0.iter().enumerate() {
            if i > 0 {
                out.write_str("; ")?;
            }
            write!(out, "{}", escape(item))?;
        }
        Ok(())
    }
}

const MAX_MARKDOWN_BYTES: usize = 8 * 1024 * 1024;

#[derive(Default)]
struct MarkdownBuffer {
    text: String,
}

impl std::fmt::Write for MarkdownBuffer {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        if text.len() > MAX_MARKDOWN_BYTES - self.text.len() {
            return Err(std::fmt::Error);
        }
        let needed = self.text.len() + text.len();
        if needed > self.text.capacity() {
            let capacity = needed
                .max(self.text.capacity().max(1024) * 2)
                .min(MAX_MARKDOWN_BYTES);
            self.text.reserve_exact(capacity - self.text.len());
        }
        self.text.push_str(text);
        Ok(())
    }
}

pub fn markdown(
    plan: &Plan,
    context: &ArchitectureContext,
    review: &Review,
) -> Result<String, String> {
    let mut out = MarkdownBuffer::default();
    render_markdown(&mut out, plan, context, review)
        .map_err(|_| "Markdown exceeds the 8 MiB output limit".to_string())?;
    Ok(out.text)
}

fn render_markdown(
    out: &mut MarkdownBuffer,
    plan: &Plan,
    context: &ArchitectureContext,
    review: &Review,
) -> std::fmt::Result {
    out.write_str("# DRAFT SOFTWARE PLAN\n\n**No implementation is authorized.** A valid draft is reviewable even when unresolved choices block implementation-plan readiness. The ready-for-plan-review flag reports planning completeness, never approval or permission to execute work.\n\n")?;
    writeln!(
        out,
        "## {}\n\nPlan: {}\n\n**Planning question:** {}\n\nValid: {}. Ready for plan review: {}.\n",
        escape(&plan.title),
        escape(&plan.id),
        escape(&plan.question),
        review.valid,
        review.ready_for_plan_review
    )?;
    for (title, entries) in [
        ("Structural Errors", &review.errors),
        ("Review Blockers", &review.blockers),
        ("Warnings", &review.warnings),
    ] {
        writeln!(out, "## {title}\n")?;
        if entries.is_empty() {
            out.write_str("None.\n\n")?;
        }
        for entry in entries {
            writeln!(out, "- {}", escape(entry))?;
        }
        out.write_char('\n')?;
    }
    out.write_str("## Open Questions and Answers\n\n")?;
    for q in &plan.questions {
        writeln!(
            out,
            "- {} [{}]: {}. Answer: {}. Manual citations: {}",
            escape(&q.id),
            if q.status == QuestionStatus::Open {
                "OPEN"
            } else {
                "RESOLVED"
            },
            escape(&q.question),
            escape(q.answer.as_deref().unwrap_or("(none)")),
            listed(&q.reference_ids)
        )?;
    }
    writeln!(out, "\n## Scope\n\nIn scope: {}\n\nOut of scope: {}\n\n## Saved Architecture\n\nRun directory (citation text): {}\n\nSnapshot: {}\n\nProposal SHA256: {}\n\nLoaded snapshot: {}\n\nLoaded proposal SHA256: {}\n\nArchitecture status: {}. Acceptance is not implementation authorization.\n", listed(&plan.scope.in_scope), listed(&plan.scope.out_of_scope), escape(&plan.architecture.run_directory), escape(&plan.architecture.snapshot_id), escape(&plan.architecture.proposal_sha256), escape(&context.snapshot_id), escape(&context.proposal_sha256), escape(&context.status))?;
    out.write_str("## Current Observations\n\nStatements below are evidence-linked observations, not approved desired behavior or runtime proof. Saved packet facts are source evidence only.\n\n")?;
    for o in &plan.observations {
        writeln!(out, "### {}\n\n{}\n", escape(&o.id), escape(&o.statement))?;
        for id in &o.fact_ids {
            if let Some(f) = context.facts.get(id) {
                writeln!(
                    out,
                    "- Saved fact {}: {} lines {}-{}; kind {}; symbol {}",
                    escape(id),
                    escape(&f.path),
                    f.line,
                    f.end_line,
                    escape(&f.kind),
                    escape(&f.symbol)
                )?;
            } else {
                writeln!(out, "- Missing saved fact: {}", escape(id))?;
            }
        }
        writeln!(out, "\nManual citations: {}\n", listed(&o.reference_ids))?;
    }
    out.write_str("## Manual Citations\n\nUNVERIFIED citation text only. This command opens no citation paths or source/test definitions; no tests executed. Any separate source review is the author's attribution, not a validator result. Constraints are human assertions, not automatic truth.\n\n")?;
    for r in &plan.references {
        let kind = match r.kind {
            ReferenceKind::Source => "source",
            ReferenceKind::TestSource => "test_source",
            ReferenceKind::Constraint => "constraint",
        };
        writeln!(
            out,
            "- {} [{}]: {} lines {}-{}. Note: {}",
            escape(&r.id),
            kind,
            escape(&r.path),
            r.line,
            r.end_line,
            escape(&r.note)
        )?;
    }
    out.write_str("\n## Requirements\n\nConfirmation records human attribution only, not implementation approval.\n\n")?;
    for r in &plan.requirements {
        writeln!(
            out,
            "- {} [{}]: {}. Confirmed by: {}. Observations: {}",
            escape(&r.id),
            if r.status == RequirementStatus::Proposed {
                "PROPOSED"
            } else {
                "CONFIRMED"
            },
            escape(&r.statement),
            escape(r.confirmed_by.as_deref().unwrap_or("(none)")),
            listed(&r.observation_ids)
        )?;
    }
    out.write_str("\n## Alternatives and Tradeoffs\n\n| Option | Description | Pros | Cons | Manual citations |\n| --- | --- | --- | --- | --- |\n")?;
    for o in &plan.options {
        writeln!(
            out,
            "| {}: {} | {} | {} | {} | {} |",
            escape(&o.id),
            escape(&o.title),
            escape(&o.description),
            listed(&o.pros),
            listed(&o.cons),
            listed(&o.reference_ids)
        )?;
    }
    out.write_str("\n## Recommendation (Not a Selection)\n\n")?;
    if let Some(r) = &plan.recommendation {
        writeln!(
            out,
            "Advisory only; does not select an option: {}. Rationale: {}\n",
            escape(&r.option_id),
            escape(&r.rationale)
        )?;
    } else {
        out.write_str("None.\n\n")?;
    }
    out.write_str("## Decisions\n\n")?;
    for d in &plan.decisions {
        writeln!(
            out,
            "### {}\n\n{}\n\nOptions: {}\n\nSelection: {}\n\nSelected by: {}\n\nRationale: {}\n",
            escape(&d.id),
            escape(&d.question),
            listed(&d.option_ids),
            escape(
                d.selected_option_id
                    .as_deref()
                    .unwrap_or("OPEN (unselected)")
            ),
            escape(d.selected_by.as_deref().unwrap_or("(none)")),
            escape(d.rationale.as_deref().unwrap_or("(none)"))
        )?;
    }
    out.write_str("## Proposed Work Items\n\nListed in authored order; dependency IDs specify prerequisites, not execution permission. Nothing has been run.\n\n")?;
    for (i, w) in plan.work_items.iter().enumerate() {
        writeln!(out, "### {}. {}: {}\n\n{}\n\nDependencies: {}\n\nTarget components: {}\n\nRequirements: {}\n\nDecisions: {}\n\nPlanned verification gates: {}\n", i + 1, escape(&w.id), escape(&w.title), escape(&w.description), listed(&w.depends_on), listed(&w.component_ids), listed(&w.requirement_ids), listed(&w.decision_ids), listed(&w.verification_ids))?;
    }
    out.write_str("## Planned Verification (Not Results)\n\nAll checks are PLANNED, not passed or executed. Expected outcomes are proposals, not observed runtime behavior.\n\n")?;
    for v in &plan.verification {
        let kind = match v.kind {
            VerificationKind::SourceReview => "source_review",
            VerificationKind::AutomatedTest => "automated_test",
            VerificationKind::EnvironmentCheck => "environment_check",
        };
        writeln!(out, "### {} [PLANNED: {}]\n\nCheck: {}\n\nExpected: {}\n\nManual citations: {}\n\nRequirements: {}\n", escape(&v.id), kind, escape(&v.check), escape(&v.expected), listed(&v.reference_ids), listed(&v.requirement_ids))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn fixture() -> (Plan, ArchitectureContext) {
        let plan = parse(&json!({
            "schemaVersion":"ideomancer.plan.v1", "id":"example", "title":"Failure boundary", "question":"Where should a failed check-in stop?",
            "architecture":{"runDirectory":"saved/run", "snapshotId":"snapshot", "proposalSha256":"digest"},
            "scope":{"inScope":["Define the boundary"], "outOfScope":["Executing changes"]},
            "references":[{"id":"citation", "kind":"test_source", "path":"not-a-real-file", "line":1, "endLine":2, "note":"Synthetic test definition citation"}],
            "observations":[{"id":"observation", "statement":"A boundary is described in source", "factIds":["fact"], "referenceIds":["citation"]}],
            "requirements":[{"id":"requirement", "statement":"Make the boundary explicit", "status":"confirmed", "confirmedBy":"Product owner", "observationIds":["observation"]}],
            "questions":[{"id":"question", "question":"Is scope limited to design?", "status":"resolved", "answer":"Yes, design only", "referenceIds":[]}],
            "options":[{"id":"option", "title":"Explicit boundary", "description":"Describe the boundary", "pros":["Clarity"], "cons":["Design effort"], "referenceIds":["citation"]}],
            "decisions":[{"id":"decision", "question":"Which design?", "optionIds":["option"], "selectedOptionId":"option", "selectedBy":"Designer", "rationale":"Clarity"}],
            "recommendation":{"optionId":"option", "rationale":"Worth discussing"},
            "workItems":[{"id":"work", "title":"Describe boundary", "description":"Propose a source-level change", "componentIds":["component"], "requirementIds":["requirement"], "decisionIds":["decision"], "dependsOn":[], "verificationIds":["gate"]}],
            "verification":[{"id":"gate", "kind":"source_review", "check":"Review the proposed boundary", "expected":"Boundary is explicit", "referenceIds":["citation"], "requirementIds":["requirement"]}]
        }).to_string()).unwrap();
        let context = ArchitectureContext {
            snapshot_id: "snapshot".into(),
            proposal_sha256: "digest".into(),
            status: "proposal_ready".into(),
            component_ids: BTreeSet::from(["component".into()]),
            facts: BTreeMap::from([(
                "fact".into(),
                FactReference {
                    path: "synthetic.rs".into(),
                    line: 3,
                    end_line: 4,
                    kind: "function".into(),
                    symbol: "boundary".into(),
                },
            )]),
        };
        (plan, context)
    }

    #[test]
    fn strict_schema_and_enums() {
        let (p, _) = fixture();
        let value = serde_json::to_value(p).unwrap();
        for path in [
            "",
            "/architecture",
            "/scope",
            "/references/0",
            "/observations/0",
            "/requirements/0",
            "/questions/0",
            "/options/0",
            "/decisions/0",
            "/recommendation",
            "/workItems/0",
            "/verification/0",
        ] {
            let mut v = value.clone();
            v.pointer_mut(path)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("approval".into(), json!(true));
            assert!(parse(&v.to_string()).is_err(), "{path}");
        }
        for path in [
            "/references/0/kind",
            "/requirements/0/status",
            "/questions/0/status",
            "/verification/0/kind",
        ] {
            let mut v = value.clone();
            *v.pointer_mut(path).unwrap() = json!("unknown");
            assert!(parse(&v.to_string()).is_err(), "{path}");
        }
    }

    #[test]
    fn ready_is_not_authority_and_draft_is_valid() {
        let (mut p, c) = fixture();
        let r = review(&p, &c);
        assert!(r.valid && r.ready_for_plan_review);
        assert!(!r.warnings.is_empty()); // Unaccepted architecture is not a blocker.
        assert!(markdown(&p, &c, &r)
            .unwrap()
            .contains("No implementation is authorized"));
        p.requirements[0].status = RequirementStatus::Proposed;
        p.requirements[0].confirmed_by = None;
        p.questions[0].status = QuestionStatus::Open;
        p.questions[0].answer = None;
        p.decisions[0].selected_option_id = None;
        p.decisions[0].selected_by = None;
        p.decisions[0].rationale = None;
        let r = review(&p, &c);
        assert!(r.valid && !r.ready_for_plan_review);
        assert_eq!(r.blockers.len(), 3); // Recommendation never selects.
        let rendered = markdown(&p, &c, &r).unwrap();
        assert!(rendered.contains("OPEN \\(unselected\\)"));
        assert!(rendered.contains("PLANNED, not passed or executed"));
        assert_eq!(rendered, markdown(&p, &c, &review(&p, &c)).unwrap());
        assert_eq!(
            serde_json::to_value(r).unwrap()["readyForPlanReview"],
            false
        );
    }

    #[test]
    fn duplicate_ids_and_typed_references() {
        let (p, c) = fixture();
        let base = serde_json::to_value(p).unwrap();
        for (path, replacement) in [
            ("/references/0/id", json!("observation")),
            ("/observations/0/factIds", json!(["invented"])),
            ("/observations/0/referenceIds", json!(["fact"])),
            ("/requirements/0/observationIds", json!(["option"])),
            ("/questions/0/referenceIds", json!(["requirement"])),
            ("/options/0/referenceIds", json!(["fact"])),
            ("/decisions/0/optionIds", json!(["requirement"])),
            ("/recommendation/optionId", json!("decision")),
            ("/workItems/0/componentIds", json!(["invented"])),
            ("/workItems/0/requirementIds", json!(["observation"])),
            ("/workItems/0/decisionIds", json!(["option"])),
            ("/workItems/0/verificationIds", json!(["work"])),
            ("/workItems/0/dependsOn", json!(["gate"])),
            ("/verification/0/referenceIds", json!(["fact"])),
            ("/verification/0/requirementIds", json!(["decision"])),
        ] {
            let mut v = base.clone();
            *v.pointer_mut(path).unwrap() = replacement;
            assert!(!review(&parse(&v.to_string()).unwrap(), &c).valid, "{path}");
        }
        let mut v = base;
        let duplicate = v["references"][0].clone();
        v["references"].as_array_mut().unwrap().push(duplicate);
        assert!(review(&parse(&v.to_string()).unwrap(), &c)
            .errors
            .iter()
            .any(|s| s.contains("Duplicate")));

        let (mut p, c) = fixture();
        let mut alternative: DesignOption =
            serde_json::from_value(serde_json::to_value(&p.options[0]).unwrap()).unwrap();
        alternative.id = "alternative".into();
        p.options.push(alternative);
        p.decisions[0].selected_option_id = Some("alternative".into());
        assert!(review(&p, &c)
            .errors
            .iter()
            .any(|s| s.contains("not in optionIds")));
        p.decisions[0].option_ids.push("alternative".into());
        assert!(review(&p, &c).valid);
    }

    #[test]
    fn pins_status_cycles_and_missing_attribution() {
        let (p, c) = fixture();
        let base = serde_json::to_value(p).unwrap();
        for (path, replacement) in [
            ("/architecture/snapshotId", json!("other")),
            ("/architecture/proposalSha256", json!("other")),
            ("/requirements/0/confirmedBy", Value::Null),
            ("/requirements/0/confirmedBy", json!("  ")),
            ("/requirements/0/status", json!("proposed")),
            ("/questions/0/answer", Value::Null),
            ("/decisions/0/selectedBy", Value::Null),
            ("/decisions/0/rationale", Value::Null),
            ("/decisions/0/selectedOptionId", Value::Null),
            ("/workItems/0/dependsOn", json!(["work"])),
            ("/references/0/line", json!(0)),
            ("/references/0/endLine", json!(0)),
        ] {
            let mut v = base.clone();
            *v.pointer_mut(path).unwrap() = replacement;
            assert!(!review(&parse(&v.to_string()).unwrap(), &c).valid, "{path}");
        }
        let mut p = parse(&base.to_string()).unwrap();
        let mut second: WorkItem = serde_json::from_value(base["workItems"][0].clone()).unwrap();
        second.id = "second".into();
        second.depends_on = vec!["work".into()];
        p.work_items[0].depends_on = vec!["second".into()];
        p.work_items.push(second);
        assert!(!review(&p, &c).valid);
        p.work_items[0].depends_on.clear();
        assert!(review(&p, &c).valid);
        let mut c = c;
        c.status = "accepted".into();
        assert!(review(&p, &c).ready_for_plan_review);
        c.status = "failed".into();
        assert!(!review(&p, &c).valid);
    }

    #[test]
    fn drafts_bounds_and_grounding() {
        let (mut p, c) = fixture();
        p.work_items[0].requirement_ids.clear();
        p.work_items[0].decision_ids.clear();
        p.work_items[0].verification_ids.clear();
        let r = review(&p, &c);
        assert!(r.valid && !r.ready_for_plan_review && r.blockers.len() == 3);
        p.observations[0].fact_ids.clear();
        assert!(review(&p, &c).valid); // Manual citation is sufficient grounding.
        p.observations[0].reference_ids.clear();
        assert!(!review(&p, &c).valid);
        let (mut p, c) = fixture();
        p.title = "x".repeat(16_385);
        assert!(!review(&p, &c).valid);
        p.title = "x".into();
        p.id = "a".repeat(129);
        assert!(!review(&p, &c).valid);
        p.id = "1-invalid".into();
        assert!(!review(&p, &c).valid);
        p.id = "valid".into();
        p.work_items[0].verification_ids = vec!["gate".into(); 129];
        assert!(!review(&p, &c).valid);
        p.work_items[0].verification_ids = vec!["gate".into()];
        p.scope.in_scope = vec!["item".into(); 129];
        assert!(!review(&p, &c).valid);
        p.scope.in_scope.clear();
        p.references.clear();
        p.observations.clear();
        p.requirements.clear();
        p.questions.clear();
        p.options.clear();
        p.decisions.clear();
        p.recommendation = None;
        p.work_items.clear();
        p.verification.clear();
        let r = review(&p, &c);
        assert!(r.valid && !r.ready_for_plan_review && r.blockers.len() == 6);
    }

    #[test]
    fn readable_punctuation_and_bounded_errors() {
        let (mut p, c) = fixture();
        let prose = "A check-in is field-first, next? See src/services/file.py.";
        assert_eq!(escape(prose).to_string(), prose);
        assert_eq!(
            escape("plan_id (draft)").to_string(),
            "plan\\_id \\(draft\\)"
        );
        assert_eq!(escape("- injected").to_string(), "\\- injected");
        assert_eq!(escape("1. injected").to_string(), "1\\. injected");
        p.title = prose.into();
        p.references[0].path = "src/services/file.py".into();
        let rendered = markdown(&p, &c, &review(&p, &c)).unwrap();
        assert!(rendered.contains(prose));
        assert!(rendered.contains("src/services/file.py lines"));
        assert!(rendered.contains("A valid draft is reviewable"));
        assert!(!rendered.contains("&#45;"));
        assert!(!rendered.contains("&#47;"));

        // One limit error, not thousands of blank-text or unresolved-ID errors.
        p.work_items[0].verification_ids = vec![String::new(); 10_000];
        let r = review(&p, &c);
        assert!(!r.valid && !r.ready_for_plan_review);
        assert_eq!(r.errors.len(), 1);
        assert!(r.errors[0].contains("at most 128 items"));
        p.work_items[0].verification_ids = vec!["unknown".into()];
        p.title = " ".into();
        let r = review(&p, &c);
        assert_eq!(r.errors.len(), 1);
        assert!(r.errors[0].contains("text must be nonblank"));
    }

    #[test]
    fn escaping_prefix_state_is_linear_and_preserves_trim_semantics() {
        let spaces = " ".repeat(8192);
        for marker in ['-', '+', '='] {
            let tail = marker.to_string().repeat(8192);
            assert_eq!(
                escape(&format!("{spaces}{tail}")).to_string(),
                format!("{spaces}\\{tail}")
            );
        }
        let periods = ".".repeat(8192);
        assert_eq!(
            escape(&format!("{spaces}{periods}")).to_string(),
            format!("{spaces}{periods}")
        );
        for (text, expected) in [
            (" 12 .", " 12 \\."),
            (" 1 2.", " 1 2."),
            (" 12\t.", " 12&#9;\\."),
            (" \t-", " &#9;\\-"),
            (" 12\u{2003}.", " 12\u{2003}\\."),
            ("12.3.", "12\\.3."),
        ] {
            assert_eq!(escape(text).to_string(), expected);
        }
    }

    #[test]
    fn markdown_limit_is_inclusive_and_accounts_for_escaping_and_fixed_text() {
        let (p, mut c) = fixture();
        let r = review(&p, &c);
        c.facts.get_mut("fact").unwrap().symbol.clear();
        let fixed_bytes = markdown(&p, &c, &r).unwrap().len();
        // '<' is five output bytes, '_' is two, and non-ASCII counts as UTF-8 bytes.
        let suffix = "<_é";
        let escaped_bytes = escape(suffix).to_string().len();
        let fact = c.facts.get_mut("fact").unwrap();
        fact.symbol = "x".repeat(MAX_MARKDOWN_BYTES - fixed_bytes - escaped_bytes);
        fact.symbol.push_str(suffix);
        assert_eq!(markdown(&p, &c, &r).unwrap().len(), MAX_MARKDOWN_BYTES);
        c.facts.get_mut("fact").unwrap().symbol.push('x');
        assert!(markdown(&p, &c, &r).is_err());

        let mut out = MarkdownBuffer::default();
        out.write_str(&"x".repeat(MAX_MARKDOWN_BYTES - 2)).unwrap();
        write!(out, "{}", escape("_")).unwrap();
        assert_eq!(out.text.len(), MAX_MARKDOWN_BYTES);
        assert!(out.write_char('x').is_err());
        assert_eq!(out.text.len(), MAX_MARKDOWN_BYTES);
        assert!(out.text.capacity() <= MAX_MARKDOWN_BYTES);
    }

    #[test]
    fn large_context_and_repeated_facts_stop_at_the_output_budget() {
        let (mut p, mut c) = fixture();
        c.facts.get_mut("fact").unwrap().symbol = "x".repeat(32 * 1024);
        p.observations = (0..64)
            .map(|i| Observation {
                id: format!("observation{i}"),
                statement: "Synthetic source observation".into(),
                fact_ids: vec!["fact".into(); 128],
                reference_ids: vec![],
            })
            .collect();
        p.requirements[0].observation_ids = vec!["observation0".into()];
        let r = review(&p, &c);
        assert!(r.valid);
        assert!(serde_json::to_vec(&p).unwrap().len() < 1024 * 1024);
        let mut out = MarkdownBuffer::default();
        assert!(render_markdown(&mut out, &p, &c, &r).is_err());
        assert_eq!(out.text.len(), MAX_MARKDOWN_BYTES);
        assert!(out.text.capacity() <= MAX_MARKDOWN_BYTES);
        // Failure occurs during the first few observations, not after all 8192 facts.
        assert!(out.text.contains("### observation1\n"));
        assert!(!out.text.contains("### observation2\n"));
        assert!(markdown(&p, &c, &r).is_err());

        // One oversized field must also stream, rather than allocate its escaped copy.
        c.facts.get_mut("fact").unwrap().symbol = "<".repeat(MAX_MARKDOWN_BYTES + 1);
        let mut out = MarkdownBuffer::default();
        assert!(render_markdown(&mut out, &p, &c, &r).is_err());
        assert!(out.text.len() <= MAX_MARKDOWN_BYTES);
        assert!(out.text.capacity() <= MAX_MARKDOWN_BYTES);
        assert!(!out.text.contains("### observation1\n"));
    }

    #[test]
    fn manual_paths_are_text_and_all_rendered_text_is_escaped() {
        let (mut p, mut c) = fixture();
        let hostile = "<script>alert(1)</script>\n![image](https://invalid/x)\n```\n:::include /private\n| [x](file:///private)";
        p.title = hostile.into();
        p.references[0].path = hostile.into();
        p.references[0].note = hostile.into();
        c.facts.get_mut("fact").unwrap().path = hostile.into();
        c.facts.get_mut("fact").unwrap().symbol = hostile.into();
        let r = review(&p, &c);
        assert!(r.valid && r.ready_for_plan_review);
        let rendered = markdown(&p, &c, &r).unwrap();
        for raw in [
            "<script>",
            "![image]",
            "https://",
            "```",
            ":::include",
            "file://",
        ] {
            assert!(!rendered.contains(raw), "{raw}");
        }
        assert!(rendered.contains("&#60;script&#62;"));
        assert!(rendered.contains("UNVERIFIED citation text only"));
    }
}
