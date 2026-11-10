use crate::manifest::Manifest;

pub fn render(manifest: &Manifest) -> String {
    let mut output = String::from("flowchart LR\n");

    for node in &manifest.nodes {
        let label = node.label.as_deref().unwrap_or(&node.id);
        output.push_str(&format!("  n_{}[\"{}\"]\n", node.id, escape_label(label)));
    }

    if !manifest.nodes.is_empty() && !manifest.edges.is_empty() {
        output.push('\n');
    }

    for edge in &manifest.edges {
        match edge
            .label
            .as_deref()
            .filter(|label| !label.trim().is_empty())
        {
            Some(label) => output.push_str(&format!(
                "  n_{} -->|\"{}\"| n_{}\n",
                edge.from,
                escape_label(label),
                edge.to
            )),
            None => output.push_str(&format!("  n_{} --> n_{}\n", edge.from, edge.to)),
        }
    }

    output
}

// Mermaid uses #NN; entities, not backslash escaping. Encode user-supplied
// markup as well as syntax delimiters; only our own line breaks become HTML.
fn escape_label(label: &str) -> String {
    let mut escaped = String::new();
    for ch in label.replace("\r\n", "\n").chars() {
        match ch {
            '\n' | '\r' => escaped.push_str("<br/>"),
            '#' | '&' | '"' | '<' | '>' | '|' | '[' | ']' | '{' | '}' | '(' | ')' | '`' | '\\' => {
                escaped.push_str(&format!("#{};", ch as u32))
            }
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use crate::manifest::{Edge, Manifest, Node};

    use super::{escape_label, render};

    #[test]
    fn escapes_mermaid_syntax_and_user_markup() {
        assert_eq!(
            escape_label("\"\\#34; &amp; <script>|[]{}()` 日本語\r\nnext\rlast"),
            "#34;#92;#35;34; #38;amp; #60;script#62;#124;#91;#93;#123;#125;#40;#41;#96; 日本語<br/>next<br/>last"
        );
    }

    #[test]
    fn escapes_both_node_and_edge_labels() {
        let manifest = Manifest {
            schema_version: "0.1.0".into(),
            id: "test".into(),
            name: "Test".into(),
            nodes: vec![Node {
                id: "end".into(),
                label: Some("\"<b>|\n".into()),
                kind: None,
            }],
            edges: vec![Edge {
                from: "end".into(),
                to: "end".into(),
                label: Some("\"<b>|\n".into()),
            }],
        };
        let source = render(&manifest);
        assert!(source.contains("n_end[\"#34;#60;b#62;#124;<br/>\"]"));
        assert!(source.contains("n_end -->|\"#34;#60;b#62;#124;<br/>\"| n_end"));
    }

    #[test]
    fn renders_mermaid_flowchart() {
        let manifest = Manifest {
            schema_version: "0.1.0".to_string(),
            id: "ideomancer".to_string(),
            name: "Ideomancer".to_string(),
            nodes: vec![
                Node {
                    id: "cli".to_string(),
                    label: Some("CLI".to_string()),
                    kind: Some("interface".to_string()),
                },
                Node {
                    id: "renderer".to_string(),
                    label: Some("Renderer".to_string()),
                    kind: Some("component".to_string()),
                },
            ],
            edges: vec![Edge {
                from: "cli".to_string(),
                to: "renderer".to_string(),
                label: Some("renders".to_string()),
            }],
        };

        assert_eq!(
            render(&manifest),
            "flowchart LR\n  n_cli[\"CLI\"]\n  n_renderer[\"Renderer\"]\n\n  n_cli -->|\"renders\"| n_renderer\n"
        );
    }
}
