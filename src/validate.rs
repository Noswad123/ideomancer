use std::collections::HashSet;

use crate::manifest::{Manifest, SCHEMA_VERSION};

pub fn validate_manifest(manifest: &Manifest) -> Vec<String> {
    let mut errors = Vec::new();

    if manifest.schema_version != SCHEMA_VERSION {
        errors.push(format!(
            "unsupported schemaVersion {:?}; expected {SCHEMA_VERSION:?} (legacy manifests are not supported)",
            manifest.schema_version
        ));
    }
    require_non_empty(&manifest.id, "id", &mut errors);
    require_non_empty(&manifest.name, "name", &mut errors);

    if manifest.nodes.is_empty() {
        errors.push("nodes must contain at least one node".to_string());
    }

    let mut node_ids = HashSet::new();
    for node in &manifest.nodes {
        if node.id.trim().is_empty() {
            errors.push("node id must not be empty".to_string());
            continue;
        }

        if !is_mermaid_safe_id(&node.id) {
            errors.push(format!(
                "node id {:?} is invalid; use letters, digits, and underscores, and start with a letter or underscore",
                node.id
            ));
        }

        if !node_ids.insert(node.id.as_str()) {
            errors.push(format!("duplicate node id {:?}", node.id));
        }

        if matches!(node.kind.as_deref(), Some(kind) if kind.trim().is_empty()) {
            errors.push(format!(
                "node {:?} kind must not be empty when provided",
                node.id
            ));
        }
    }

    for edge in &manifest.edges {
        if !node_ids.contains(edge.from.as_str()) {
            errors.push(format!(
                "edge from {:?} references a node that does not exist",
                edge.from
            ));
        }

        if !node_ids.contains(edge.to.as_str()) {
            errors.push(format!(
                "edge to {:?} references a node that does not exist",
                edge.to
            ));
        }
    }

    errors
}

fn require_non_empty(value: &str, field: &str, errors: &mut Vec<String>) {
    if value.trim().is_empty() {
        errors.push(format!("{field} must not be empty"));
    }
}

fn is_mermaid_safe_id(id: &str) -> bool {
    let mut chars = id.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }

    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

#[cfg(test)]
mod tests {
    use crate::manifest::{Edge, Manifest, Node};

    use super::validate_manifest;

    #[test]
    fn accepts_valid_manifest() {
        let manifest = Manifest {
            schema_version: "0.1.0".to_string(),
            id: "ideomancer".to_string(),
            name: "Ideomancer".to_string(),
            nodes: vec![
                Node {
                    id: "cli".to_string(),
                    label: Some("CLI".to_string()),
                    kind: None,
                },
                Node {
                    id: "renderer".to_string(),
                    label: Some("Renderer".to_string()),
                    kind: None,
                },
            ],
            edges: vec![Edge {
                from: "cli".to_string(),
                to: "renderer".to_string(),
                label: None,
            }],
        };

        assert!(validate_manifest(&manifest).is_empty());
    }

    #[test]
    fn rejects_missing_edge_target() {
        let manifest = Manifest {
            schema_version: "0.1.0".to_string(),
            id: "ideomancer".to_string(),
            name: "Ideomancer".to_string(),
            nodes: vec![Node {
                id: "cli".to_string(),
                label: None,
                kind: None,
            }],
            edges: vec![Edge {
                from: "cli".to_string(),
                to: "missing".to_string(),
                label: None,
            }],
        };

        let errors = validate_manifest(&manifest);
        assert!(errors.iter().any(|error| error.contains("missing")));
    }
}
