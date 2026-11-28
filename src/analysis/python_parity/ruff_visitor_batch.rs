//! Finite module-domain visitor witnesses, not grammar or production cutover proof.
use super::ruff_facts_batch::source;
use super::*;
use serde::Deserialize;
use std::collections::BTreeSet;

const PROFILE: &str = "cpython-3.12.11-module-visitor-v1";
const WITNESSES: &str = include_str!("../../../docs/python-visitor-witnesses.json");
const REQUIRED_IDS: [&str; 14] = [
    "g2_declarations",
    "g2_statements",
    "g2_expressions",
    "g2_patterns",
    "g2_markers",
    "g2_type_params",
    "g2_ignores",
    "g2_argument_order",
    "g2_comprehensions",
    "g2_optional_children",
    "g2_fstrings",
    "g2_interleaved_args",
    "g2_render_caps",
    "g3_metadata_order",
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    profile: String,
    cases: Vec<Witness>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Witness {
    id: String,
    source: String,
}

fn witnesses() -> Vec<Witness> {
    assert!(WITNESSES.len() <= 128 * 1024);
    let corpus: Corpus = serde_json::from_str(WITNESSES).unwrap();
    assert_eq!(corpus.profile, PROFILE);
    assert!((REQUIRED_IDS.len()..=30).contains(&corpus.cases.len()));
    let mut ids = BTreeSet::new();
    for case in &corpus.cases {
        assert!(case.id.starts_with("g2_") || case.id.starts_with("g3_"));
        assert!((4..=80).contains(&case.id.len()));
        assert!(case
            .id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'));
        assert!(ids.insert(case.id.as_str()), "duplicate {}", case.id);
        assert!(!case.source.is_empty(), "empty {}", case.id);
        assert!(case.source.len() <= 32 * 1024, "oversize {}", case.id);
        assert!(!case.source.contains('\0'), "NUL in {}", case.id);
    }
    for id in REQUIRED_IDS {
        assert!(ids.contains(id), "missing {id}");
    }
    corpus.cases
}

#[test]
fn visitor_witness_schema_is_closed() {
    assert!(serde_json::from_str::<Corpus>(r#"{"profile":"x","cases":[],"metadata":{}}"#).is_err());
    assert!(serde_json::from_str::<Corpus>(
        r#"{"profile":"x","cases":[{"id":"g2_x","source":"pass","metadata":{}}]}"#
    )
    .is_err());
    assert!(serde_json::from_str::<Corpus>(r#"{"profile":"x","cases":[{"id":"g2_x"}]}"#).is_err());
    assert!(!witnesses().is_empty());
}

#[test]
fn visitor_witnesses_are_accepted_and_redacted() {
    for case in witnesses() {
        let inputs = [source(&case.source)];
        let report = extract_fact_candidates(&inputs).unwrap();
        assert_eq!(report.identity.policy, FACT_POLICY, "{}", case.id);
        assert_eq!(report.files.len(), 1, "{}", case.id);
        assert_eq!(
            report.files[0].status,
            FactStatus::AcceptedFacts,
            "{}: {:?}",
            case.id,
            report.files[0]
        );
        assert!(
            !serde_json::to_string(&report)
                .unwrap()
                .contains("REDACTION_PAYLOAD_71"),
            "literal leaked in {}",
            case.id
        );
    }
}

#[cfg(unix)]
#[test]
#[ignore = "requires explicitly selected trusted Python 3.12; complete arrays and raw visitor order"]
fn visitor_witnesses_match_trusted_ast_oracle() {
    let cases = witnesses();
    eprintln!(
        "Visitor witnesses: {}",
        cases
            .iter()
            .map(|case| case.id.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    super::ruff_facts_batch::assert_oracle_sources(
        cases.iter().map(|case| source(&case.source)).collect(),
    );
}
