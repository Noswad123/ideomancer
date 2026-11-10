//! Pure, deterministic presentation of a proposal. This does not classify evidence
//! or sanitize renderer output; the viewer embeds the trusted renderer's SVG as an image.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

// Also bump when shared viewer markup/assets change: verified older bundles can refresh.
pub const VERSION: &str = "readable-v2";
const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_EDGES: usize = 160;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    pub title: String,
    pub snapshot_id: String,
    pub proposal_sha256: String,
    pub components: Vec<Component>,
    pub relationships: Vec<Relationship>,
    pub uncertainties: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Component {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub rationale: String,
    pub evidence: Vec<EvidenceRef>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Relationship {
    pub from: String,
    pub to: String,
    pub label: String,
    pub basis: String,
    pub rationale: String,
    pub evidence: Vec<EvidenceRef>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceRef {
    pub id: String,
    pub path: String,
    pub kind: String,
    pub symbol: String,
    pub summary: String,
    pub line: usize,
    pub end_line: usize,
}

#[derive(Debug)]
pub struct Documents {
    pub mermaid: String,
    pub legend: Value,
    pub html: String,
}

pub fn build(input: &Input) -> Result<Documents, String> {
    if input.relationships.len() > MAX_EDGES {
        return Err(format!(
            "presentation supports at most {MAX_EDGES} relationships"
        ));
    }
    let mut ids = BTreeSet::new();
    for node in &input.components {
        if node.id.is_empty()
            || !node
                .id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_')
            || !ids.insert(node.id.as_str())
        {
            return Err(
                "component IDs must be unique, nonempty ASCII letters/digits/underscores".into(),
            );
        }
        if node.label.trim().is_empty() {
            return Err("component labels must not be blank".into());
        }
    }
    for edge in &input.relationships {
        if !ids.contains(edge.from.as_str()) || !ids.contains(edge.to.as_str()) {
            return Err("relationship refers to an unknown component".into());
        }
        if !matches!(edge.basis.as_str(), "observed" | "inferred") {
            return Err("relationship basis must be observed or inferred".into());
        }
        if edge.label.trim().is_empty() {
            return Err("relationship labels must not be blank".into());
        }
    }

    // Sort the shared HTML index, but never sort the proposal's nodes or edges.
    let mut evidence = BTreeMap::new();
    for citation in input
        .components
        .iter()
        .flat_map(|n| &n.evidence)
        .chain(input.relationships.iter().flat_map(|e| &e.evidence))
    {
        if let Some(previous) = evidence.insert(citation.id.as_str(), citation) {
            if previous != citation {
                return Err("conflicting evidence records share an ID".into());
            }
        }
    }
    let evidence_index: BTreeMap<_, _> = evidence
        .keys()
        .enumerate()
        .map(|(index, id)| (*id, index + 1))
        .collect();

    let title = shorten(&format!("DRAFT: {}", input.title), 100, usize::MAX);
    let title_yaml = serde_json::to_string(&title)
        .map_err(|e| e.to_string())?
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026");
    let mut mermaid = format!(
        "---\ntitle: {title_yaml}\nconfig:\n  securityLevel: strict\n  themeVariables:\n    fontSize: 14px\n  flowchart:\n    rankSpacing: 100\n    nodeSpacing: 60\n    wrappingWidth: 180\n    curve: linear\n---\nflowchart LR\n"
    );
    for node in &input.components {
        writeln!(
            mermaid,
            "  n_{}[\"{}\"]",
            node.id,
            escape_label(&shorten(&node.label, 64, usize::MAX))
        )
        .unwrap();
    }
    let mut edges = Vec::with_capacity(input.relationships.len());
    for (index, edge) in input.relationships.iter().enumerate() {
        let key = format!("E{:03}", index + 1);
        let label = escape_label(&format!("{key}: {}", shorten(&edge.label, 32, 5)));
        if edge.basis == "observed" {
            writeln!(
                mermaid,
                "  n_{} -->|\"{}\"| n_{}",
                edge.from, label, edge.to
            )
            .unwrap();
        } else {
            writeln!(
                mermaid,
                "  n_{} -. \"{}\" .-> n_{}",
                edge.from, label, edge.to
            )
            .unwrap();
        }
        let mut record = serde_json::to_value(edge).map_err(|e| e.to_string())?;
        record
            .as_object_mut()
            .unwrap()
            .insert("key".into(), json!(key));
        edges.push(record);
    }
    let legend = json!({
        "schemaVersion": "ideomancer.presentation.v1",
        "presentationVersion": VERSION,
        "title": input.title,
        "status": "DRAFT",
        "snapshotId": input.snapshot_id,
        "proposalSha256": input.proposal_sha256,
        "nodes": input.components,
        "edges": edges,
        "uncertainties": input.uncertainties,
    });

    let mut html = String::from(r#"<header><span class="draft">DRAFT</span><h1>"#);
    html.push_str(&escape_html(&input.title));
    html.push_str("</h1><p>Draft presentation only. Proposal acceptance is tracked separately; this view does not accept or modify the architecture.</p><dl class=\"metadata\"><dt>Snapshot</dt><dd>");
    html.push_str(&escape_html(&input.snapshot_id));
    html.push_str("</dd><dt>Proposal SHA-256</dt><dd><code>");
    html.push_str(&escape_html(&input.proposal_sha256));
    write!(
        html,
        "</code></dd><dt>Presentation version</dt><dd>{VERSION}</dd></dl></header>"
    )
    .unwrap();
    html.push_str(r#"<nav aria-label="Artifacts"><a href="diagram.svg">Open SVG</a><a href="diagram.mmd">Diagram source</a><a href="legend.json">Full JSON legend</a></nav>"#);
    html.push_str(&super::viewer::diagram("DRAFT proposal diagram; full text and citations follow", "Short diagram labels are excerpts, not paraphrases. E001 and subsequent keys refer to the relationship details below."));
    html.push_str(r#"<section aria-labelledby="key"><h2 id="key">Diagram Key</h2><p><strong>Solid arrow: observed.</strong> A relationship reported from source syntax, not runtime proof.</p><p><strong>Dashed arrow: inferred.</strong> A proposed interpretation, not directly established by source syntax and not runtime proof.</p><p>Direction and basis are preserved from the proposal. This presentation does not reclassify relationships.</p></section>
<section><h2>Components</h2>"#);
    for node in &input.components {
        html.push_str("<article class=\"card\"><h3>");
        html.push_str(&escape_html(&node.label));
        html.push_str("</h3><dl><dt>ID</dt><dd>");
        html.push_str(&escape_html(&node.id));
        html.push_str("</dd><dt>Kind</dt><dd>");
        html.push_str(&escape_html(&node.kind));
        html.push_str("</dd><dt>Rationale</dt><dd>");
        html.push_str(&escape_html(&node.rationale));
        html.push_str("</dd></dl>");
        citations_html(&mut html, &node.evidence, &evidence_index);
        html.push_str("</article>");
    }
    html.push_str("</section><section><h2>Relationships</h2>");
    for (index, edge) in input.relationships.iter().enumerate() {
        let key = format!("E{:03}", index + 1);
        write!(html, "<article class=\"card\" id=\"{key}\"><h3>{key}: ").unwrap();
        html.push_str(&escape_html(&edge.label));
        html.push_str("</h3><dl><dt>From</dt><dd>");
        html.push_str(&escape_html(&edge.from));
        html.push_str("</dd><dt>To</dt><dd>");
        html.push_str(&escape_html(&edge.to));
        html.push_str("</dd><dt>Basis</dt><dd>");
        html.push_str(&escape_html(&edge.basis));
        html.push_str("</dd><dt>Rationale</dt><dd>");
        html.push_str(&escape_html(&edge.rationale));
        html.push_str("</dd></dl>");
        citations_html(&mut html, &edge.evidence, &evidence_index);
        html.push_str("</article>");
    }
    html.push_str("</section><section><h2>Evidence Index</h2>");
    for (id, citation) in &evidence {
        write!(
            html,
            "<details id=\"evidence-{}\"><summary>",
            evidence_index[id]
        )
        .unwrap();
        html.push_str(&escape_html(id));
        html.push_str("</summary><dl><dt>Path</dt><dd>");
        html.push_str(&escape_html(&citation.path));
        write!(
            html,
            "</dd><dt>Lines</dt><dd>{}-{}</dd><dt>Kind</dt><dd>",
            citation.line, citation.end_line
        )
        .unwrap();
        html.push_str(&escape_html(&citation.kind));
        html.push_str("</dd><dt>Symbol</dt><dd>");
        html.push_str(&escape_html(&citation.symbol));
        html.push_str("</dd><dt>Summary</dt><dd>");
        html.push_str(&escape_html(&citation.summary));
        html.push_str("</dd></dl></details>");
    }
    html.push_str("</section><section><h2>Model Uncertainties</h2>");
    if input.uncertainties.is_empty() {
        html.push_str("<p>No uncertainties were supplied in this proposal.</p>");
    } else {
        html.push_str("<ul>");
        for uncertainty in &input.uncertainties {
            html.push_str("<li class=\"text\">");
            html.push_str(&escape_html(uncertainty));
            html.push_str("</li>");
        }
        html.push_str("</ul>");
    }
    html.push_str("</section>");
    let html = super::viewer::document(&format!("DRAFT: {}", input.title), &html);
    check_size("diagram.mmd", mermaid.len())?;
    check_size(
        "legend.json",
        serde_json::to_vec_pretty(&legend)
            .map_err(|e| e.to_string())?
            .len(),
    )?;
    check_size("index.html", html.len())?;
    Ok(Documents {
        mermaid,
        legend,
        html,
    })
}

fn check_size(name: &str, bytes: usize) -> Result<(), String> {
    if bytes > MAX_BYTES {
        Err(format!(
            "{name} exceeds the 8 MiB presentation limit ({bytes} bytes)"
        ))
    } else {
        Ok(())
    }
}

fn citations_html(html: &mut String, citations: &[EvidenceRef], index: &BTreeMap<&str, usize>) {
    html.push_str("<p>Citations: ");
    if citations.is_empty() {
        html.push_str("none supplied");
    }
    for (position, citation) in citations.iter().enumerate() {
        if position > 0 {
            html.push_str(", ");
        }
        write!(
            html,
            "<a href=\"#evidence-{}\">{}</a>",
            index[citation.id.as_str()],
            escape_html(&citation.id)
        )
        .unwrap();
    }
    html.push_str("</p>");
}

fn shorten(text: &str, max_chars: usize, max_words: usize) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let words = normalized
        .split_whitespace()
        .take(max_words)
        .collect::<Vec<_>>()
        .join(" ");
    if words == normalized && normalized.chars().count() <= max_chars {
        return normalized;
    }
    let mut result: String = words.chars().take(max_chars.saturating_sub(1)).collect();
    result.truncate(result.trim_end().len());
    result.push('\u{2026}');
    result
}

// Same entity rules as render::mermaid, kept private so this module is standalone.
fn escape_label(label: &str) -> String {
    let mut escaped = String::new();
    for ch in label.replace("\r\n", "\n").chars() {
        match ch {
            '\n' | '\r' => escaped.push_str("<br/>"),
            '#' | '&' | '"' | '<' | '>' | '|' | '[' | ']' | '{' | '}' | '(' | ')' | '`' | '\\' => {
                write!(escaped, "#{};", ch as u32).unwrap();
            }
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn escape_html(text: &str) -> String {
    let mut result = String::new();
    for ch in text.chars() {
        match ch {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '"' => result.push_str("&quot;"),
            '\'' => result.push_str("&#39;"),
            _ => result.push(ch),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Input {
        serde_json::from_value(json!({
            "title": "A project", "snapshotId": "snapshot", "proposalSha256": "digest",
            "components": [
                {"id": "end", "label": "Client", "kind": "interface", "rationale": "Entry point", "evidence": []},
                {"id": "service", "label": "Service", "kind": "module", "rationale": "Worker", "evidence": []}
            ],
            "relationships": [
                {"from": "end", "to": "service", "label": "calls worker", "basis": "observed", "rationale": "Call syntax", "evidence": []},
                {"from": "service", "to": "end", "label": "may return data", "basis": "inferred", "rationale": "Interpretation", "evidence": []}
            ], "uncertainties": ["Runtime behavior unknown"]
        })).unwrap()
    }

    #[test]
    fn deterministic_keys_and_topology() {
        let input = fixture();
        let a = build(&input).unwrap();
        let b = build(&input).unwrap();
        assert_eq!(a.mermaid, b.mermaid);
        assert_eq!(a.html, b.html);
        assert_eq!(a.legend, b.legend);
        assert!(a
            .mermaid
            .contains("n_end -->|\"E001: calls worker\"| n_service"));
        assert!(a
            .mermaid
            .contains("n_service -. \"E002: may return data\" .-> n_end"));
        assert_eq!(a.mermaid.lines().filter(|s| s.contains("[\"")).count(), 2);
        assert_eq!(a.mermaid.matches("-->").count(), 1);
        assert_eq!(a.mermaid.matches(".->").count(), 1);
        assert_eq!(a.legend["edges"][0]["key"], "E001");
        assert_eq!(a.legend["edges"][1]["key"], "E002");
        assert!(a.html.contains("id=\"E001\""));
        assert!(a.html.contains("Solid arrow: observed."));
        assert!(a.html.contains("Dashed arrow: inferred."));
        assert!(a.html.contains("source syntax, not runtime proof"));
    }

    #[test]
    fn unicode_excerpts_preserve_full_originals() {
        assert_eq!(
            shorten("one two three four five six", 32, 5),
            "one two three four five\u{2026}"
        );
        assert_eq!(
            shorten(&"界".repeat(33), 32, 5),
            format!("{}\u{2026}", "界".repeat(31))
        );
        assert_eq!(shorten("a\n b\t c", 32, 5), "a b c");
        let mut input = fixture();
        input.relationships[0].label = "一 二 三 四 五 六 with full original detail".into();
        input.components[0].label = "界".repeat(80);
        input.title = "長".repeat(150);
        let docs = build(&input).unwrap();
        assert_eq!(
            docs.legend["edges"][0]["label"],
            input.relationships[0].label
        );
        assert_eq!(docs.legend["nodes"][0]["label"], input.components[0].label);
        assert!(docs.html.contains(&input.components[0].label));
        assert!(docs.html.contains(&input.relationships[0].label));
        assert!(docs.html.contains(&input.title));
        let title: String = serde_json::from_str(
            docs.mermaid
                .lines()
                .nth(1)
                .unwrap()
                .strip_prefix("title: ")
                .unwrap(),
        )
        .unwrap();
        assert!(title.starts_with("DRAFT: "));
        assert_eq!(title.chars().count(), 100);
        assert!(docs.html.contains(">DRAFT</span>"));
        assert_eq!(docs.legend["schemaVersion"], "ideomancer.presentation.v1");
        assert_eq!(docs.legend["presentationVersion"], VERSION);
        assert_eq!(docs.legend["snapshotId"], input.snapshot_id);
        assert_eq!(docs.legend["proposalSha256"], input.proposal_sha256);
        assert_eq!(docs.legend["uncertainties"][0], input.uncertainties[0]);
    }

    #[test]
    fn escapes_untrusted_text_without_making_links_or_scripts() {
        let mut input = fixture();
        let attack = "\"><script>alert(1)</script><img src=https://evil.invalid onerror='x'> & #34; |[]{}()`\\\n---\nflowchart TD";
        input.title = attack.into();
        input.snapshot_id = attack.into();
        input.proposal_sha256 = attack.into();
        input.components[0].label = attack.into();
        input.components[0].kind = attack.into();
        input.components[0].rationale = attack.into();
        input.relationships[0].label = attack.into();
        input.relationships[0].rationale = attack.into();
        input.uncertainties = vec![attack.into()];
        input.components[0].evidence.push(EvidenceRef {
            id: attack.into(),
            path: "javascript:alert(1)".into(),
            kind: attack.into(),
            symbol: attack.into(),
            summary: attack.into(),
            line: 1,
            end_line: 2,
        });
        let docs = build(&input).unwrap();
        assert_eq!(docs.html.matches("<script>").count(), 1);
        assert!(docs.html.contains("script-src 'sha256-"));
        assert!(docs.html.contains("data-viewer-viewport"));
        assert!(!docs.html.contains("<svg"));
        assert!(!docs.html.contains("onerror='x'"));
        assert!(docs.html.contains("&lt;script&gt;"));
        assert_eq!(docs.html.matches("<img ").count(), 1);
        assert_eq!(docs.html.matches("src=\"").count(), 1);
        for link in docs.html.split("href=\"").skip(1) {
            let target = link.split('"').next().unwrap();
            assert!(
                matches!(target, "diagram.svg" | "diagram.mmd" | "legend.json")
                    || target.starts_with("#evidence-")
            );
        }
        assert!(docs.html.contains("default-src 'none'"));
        assert!(docs.html.contains("base-uri 'none'; form-action 'none'"));
        assert!(!docs.mermaid.contains("<script"));
        assert!(!docs.mermaid.contains("\nflowchart TD"));
        assert!(docs.mermaid.contains("#34;#62;#60;script#62;"));
        assert_eq!(docs.legend["nodes"][0]["rationale"], attack);
        assert_eq!(escape_label("\"\\#34; &amp; <script>|[]{}()` 日本語\r\nnext\rlast"),
            "#34;#92;#35;34; #38;amp; #60;script#62;#124;#91;#93;#123;#125;#40;#41;#96; 日本語<br/>next<br/>last");
    }

    #[test]
    fn validates_ids_references_basis_labels_and_edge_limit() {
        for id in ["", "a-b", "é", "a\nflowchart", "x\"]"] {
            let mut input = fixture();
            input.components[0].id = id.into();
            assert!(build(&input).is_err());
        }
        let mut input = fixture();
        input.components[1].id = input.components[0].id.clone();
        assert!(build(&input).is_err());
        let mut input = fixture();
        input.relationships[0].to = "missing".into();
        assert!(build(&input).is_err());
        let mut input = fixture();
        input.relationships[0].basis = "Observed".into();
        assert!(build(&input).is_err());
        let mut input = fixture();
        input.components[0].label = " \n\t".into();
        assert!(build(&input).is_err());
        let mut input = fixture();
        input.relationships[0].label = " ".into();
        assert!(build(&input).is_err());
        let mut input = fixture();
        let edge = serde_json::to_value(&input.relationships[0]).unwrap();
        input.relationships = (0..160)
            .map(|_| serde_json::from_value(edge.clone()).unwrap())
            .collect();
        assert_eq!(build(&input).unwrap().legend["edges"][159]["key"], "E160");
        input
            .relationships
            .push(serde_json::from_value(edge).unwrap());
        assert!(build(&input).unwrap_err().contains("160"));
    }

    #[test]
    fn shared_evidence_is_deduplicated_only_in_html() {
        let mut input = fixture();
        let citation = json!({"id": "e1", "path": "src/lib.rs", "kind": "call", "symbol": "go", "summary": "Full summary", "line": 12, "endLine": 14});
        input.components[0]
            .evidence
            .push(serde_json::from_value(citation.clone()).unwrap());
        input.relationships[0]
            .evidence
            .push(serde_json::from_value(citation.clone()).unwrap());
        let docs = build(&input).unwrap();
        assert_eq!(docs.html.matches("<details id=\"evidence-").count(), 1);
        assert_eq!(docs.html.matches("href=\"#evidence-1\"").count(), 2);
        assert_eq!(docs.legend["nodes"][0]["evidence"][0], citation);
        assert_eq!(docs.legend["edges"][0]["evidence"][0], citation);
        assert!(docs.html.contains("12-14"));
        input.relationships[0].evidence[0].summary = "Different".into();
        assert!(build(&input).unwrap_err().contains("conflicting evidence"));
    }

    #[test]
    fn byte_caps_measure_utf8_and_escaped_output() {
        assert!(check_size("test", MAX_BYTES).is_ok());
        assert!(check_size("test", MAX_BYTES + 1).is_err());
        let mut input = fixture();
        input.components[0].rationale = "界".repeat(MAX_BYTES / 3 + 1);
        let error = build(&input).unwrap_err();
        assert!(error.contains("legend.json") && error.contains("8 MiB"));
        input.components[0].rationale = "&".repeat(MAX_BYTES / 5);
        assert!(build(&input).unwrap_err().contains("index.html"));
        input.components[0].rationale = "&".repeat(1000);
        let docs = build(&input).unwrap();
        assert!(docs.html.len() <= MAX_BYTES);
        assert!(docs.mermaid.len() <= MAX_BYTES);
        assert!(serde_json::to_vec_pretty(&docs.legend).unwrap().len() <= MAX_BYTES);
    }

    #[test]
    #[ignore = "requires locally installed mmdc and Chromium; no downloads"]
    fn local_mmdc_accepts_both_arrow_styles() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("diagram.mmd");
        let svg = directory.path().join("diagram.svg");
        let mut input = fixture();
        input.components[0].label = "Client \"quoted\" <tag> # & 日本語".into();
        input.relationships[1].label = "inferred \"quoted\" | []".into();
        std::fs::write(&source, build(&input).unwrap().mermaid).unwrap();
        let output = std::process::Command::new("mmdc")
            .arg("-i")
            .arg(&source)
            .arg("-o")
            .arg(&svg)
            .output()
            .expect("mmdc must be installed locally");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let rendered = std::fs::read_to_string(svg).unwrap();
        assert!(rendered.contains("<svg"));
        assert!(rendered.contains("E001"));
        assert!(rendered.contains("E002"));
        assert!(rendered.contains("DRAFT"));
    }
}
