//! Public consumer contracts. All sources and claims are synthetic.
use ideomancer_core::engine::v1::*;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

fn fixture() -> (tempfile::TempDir, CatalogRef, FactsRef) {
    fixture_source("class Target: pass\nclass Other: pass\nclass Child(External): pass\n")
}

fn fixture_source(source: &str) -> (tempfile::TempDir, CatalogRef, FactsRef) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("source");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("model.py"), source).unwrap();
    let report = discover(&DiscoverRequest {
        source_root: root,
        output_container: temp.path().join("catalogs"),
        toolchain: Toolchain::Python {
            python: "python3".into(),
        },
    })
    .unwrap();
    let catalog = open_catalog(report.catalog_directory).unwrap();
    let facts = open_facts(&catalog, &FactsRequest::default()).unwrap();
    (temp, catalog, facts)
}
fn set(facts: &FactsRef) -> ClaimSet {
    let view = facts.view();
    let site = &view.relationship_sites[0];
    let target = view.classes.iter().find(|c| c.name == "Target").unwrap();
    let mut evidence = site.evidence_ids.clone();
    evidence.push(target.declaration_id.clone());
    ClaimSet {
        schema_version: "ideomancer.claims.v1".into(),
        facts: view.identity.clone(),
        adapter: AdapterIdentity {
            id: "synthetic.adapter".into(),
            version: "1".into(),
            implementation: "synthetic-build-1".into(),
        },
        contract: ContractIdentity {
            id: "synthetic.contract".into(),
            version: "1".into(),
            spec_hash: "a".repeat(64),
        },
        configuration_hash: "b".repeat(64),
        claims: vec![ClassClaim {
            id: "claim-1".into(),
            site_id: site.id.clone(),
            source_class_id: site.source_class_id.clone(),
            target_class_id: target.id.clone(),
            kind: ClaimKind::Extends,
            basis: ClaimBasis::FrameworkContract,
            evidence_ids: evidence,
            rule_id: "base-1".into(),
            assumptions: vec!["Approved by the consumer".into()],
        }],
        diagnostics: vec![],
    }
}
#[test]
fn composition_persistence_and_offline_reopen() {
    let (temp, catalog, facts) = fixture();
    let set = set(&facts);
    let evidence_before = fs::read(catalog.directory().join("evidence.json")).unwrap();
    let mut second = set.clone();
    second.adapter.id = "synthetic.second".into();
    let validated = validate_claims(&facts, &[set, second]).unwrap();
    let saved = temp.path().join("claims");
    save_claims(&validated, &saved).unwrap();
    fs::remove_dir_all(catalog.source_root()).unwrap();
    let reopened = load_claims(&facts, &saved).unwrap();
    let report = class_diagrams_augmented(
        &catalog,
        &ClassSelection::All,
        &reopened,
        &RenderMode::SourceOnly,
    )
    .unwrap();
    assert_eq!(report.augmentation.origins.len(), 2);
    assert!(report.report.coverage.all_selected_classes_represented);
    let legend: Value = serde_json::from_slice(&fs::read(&report.report.legend).unwrap()).unwrap();
    assert_eq!(
        legend["relationships"][0]["claimOrigins"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        legend["groups"][0]["drawnEdges"].as_array().unwrap().len(),
        1
    );
    assert!(fs::read_to_string(&report.report.diagrams[0].mermaid)
        .unwrap()
        .contains("framework contract"));
    assert_eq!(
        evidence_before,
        fs::read(catalog.directory().join("evidence.json")).unwrap()
    );
    assert!(
        class_diagrams_augmented(
            &catalog,
            &ClassSelection::All,
            &reopened,
            &RenderMode::SourceOnly
        )
        .unwrap()
        .report
        .reused
    );
    let mut wire: Value =
        serde_json::from_slice(&fs::read(saved.join("claims.json")).unwrap()).unwrap();
    wire["sets"][0]["configurationHash"] = json!("c".repeat(64));
    fs::write(
        saved.join("claims.json"),
        serde_json::to_vec(&wire).unwrap(),
    )
    .unwrap();
    assert!(load_claims(&facts, &saved).is_err());
}
#[test]
fn rejects_ungrounded_conflicting_and_stale_claims() {
    let (_temp, catalog, facts) = fixture();
    let original = set(&facts);
    let mut bad = original.clone();
    bad.claims[0].basis = ClaimBasis::Hypothesis;
    assert!(validate_claims(&facts, &[bad])
        .unwrap_err()
        .message
        .contains("Hypotheses"));
    let mut bad = original.clone();
    bad.claims[0].evidence_ids.clear();
    assert!(validate_claims(&facts, &[bad]).is_err());
    let mut bad = original.clone();
    bad.facts.facts_hash = "0".repeat(64);
    assert!(validate_claims(&facts, &[bad]).is_err());
    assert!(validate_claims(&facts, &vec![original.clone(); 5]).is_err());
    let mut bad = original.clone();
    bad.claims[0].evidence_ids.push("unknown-fact".into());
    assert!(validate_claims(&facts, &[bad]).is_err());
    let mut bad = original.clone();
    bad.claims[0].kind = ClaimKind::DependsOn;
    assert!(validate_claims(&facts, &[bad]).is_err());
    let mut bad = original.clone();
    bad.claims[0].target_class_id = bad.claims[0].source_class_id.clone();
    assert!(validate_claims(&facts, &[bad]).is_err());
    let mut conflict = original.clone();
    let other = facts
        .view()
        .classes
        .iter()
        .find(|c| c.name == "Other")
        .unwrap();
    conflict.claims[0].target_class_id = other.id.clone();
    conflict.claims[0]
        .evidence_ids
        .push(other.declaration_id.clone());
    assert!(validate_claims(&facts, &[original.clone(), conflict]).is_err());
    let validated = validate_claims(&facts, std::slice::from_ref(&original)).unwrap();
    let evidence = catalog.directory().join("evidence.json");
    let mut wire: Value = serde_json::from_slice(&fs::read(&evidence).unwrap()).unwrap();
    wire["facts"][0]["value"] = json!("changed metadata");
    fs::write(&evidence, serde_json::to_vec_pretty(&wire).unwrap()).unwrap();
    assert!(validate_claims(&facts, &[original]).is_err());
    assert!(class_diagrams_augmented(
        &catalog,
        &ClassSelection::All,
        &validated,
        &RenderMode::SourceOnly
    )
    .is_err());
}
#[test]
fn strict_dtos_and_default_completeness() {
    let schema: Value =
        serde_json::from_str(include_str!("../docs/adapter-contract.schema.json")).unwrap();
    assert_eq!(schema["$defs"]["ClaimSet"]["additionalProperties"], false);
    assert!(serde_json::from_value::<FactsRequest>(json!({"unexpected":true})).is_err());
    assert!(
        serde_json::from_value::<FactsRequest>(json!({}))
            .unwrap()
            .require_complete
    );
    let (_temp, _catalog, facts) = fixture();
    let mut wire = serde_json::to_value(set(&facts)).unwrap();
    wire["claims"][0]["rawSource"] = json!("not permitted");
    assert!(serde_json::from_value::<ClaimSet>(wire).is_err());
    assert_eq!(extension_capabilities().max_claims, 2000);
}

#[test]
fn empty_claim_sets_still_invalidate_derived_cache() {
    let (_temp, catalog, facts) = fixture();
    let mut empty = set(&facts);
    empty.claims.clear();
    let first = validate_claims(&facts, std::slice::from_ref(&empty)).unwrap();
    let first = class_diagrams_augmented(
        &catalog,
        &ClassSelection::All,
        &first,
        &RenderMode::SourceOnly,
    )
    .unwrap();
    let old_legend = fs::read(&first.report.legend).unwrap();
    empty.configuration_hash = "c".repeat(64);
    let second = validate_claims(&facts, &[empty]).unwrap();
    let second = class_diagrams_augmented(
        &catalog,
        &ClassSelection::All,
        &second,
        &RenderMode::SourceOnly,
    )
    .unwrap();
    assert!(!second.report.reused);
    assert_ne!(old_legend, fs::read(second.report.legend).unwrap());
}

#[test]
fn removed_profile_is_actionable_before_catalog_access() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_ideomancer"))
        .args([
            "diagram",
            "/missing/catalog",
            "--kind",
            "class",
            "--js-profile",
            "any-retired-profile",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("removed"), "{error}");
    assert!(!error.contains("any-retired-profile"));
}

#[test]
fn javascript_factories_use_catalog_class_ids() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("source");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("factory.js"), "module.exports = function(Base, Dependency) { class Product extends Base { constructor(value) { super(); this.value = value; } run() { return Dependency(); } } return Product; };\n").unwrap();
    let discovered = discover(&DiscoverRequest {
        source_root: root,
        output_container: temp.path().join("catalogs"),
        toolchain: Toolchain::NativeJavaScriptTypeScript {},
    })
    .unwrap();
    let catalog = open_catalog(discovered.catalog_directory).unwrap();
    let facts = open_facts(&catalog, &FactsRequest::default()).unwrap();
    assert!(facts.view().coverage.exported_factory_structure);
    let factory = &facts.view().factories[0];
    assert!(factory.safe);
    assert_eq!(factory.parameters.len(), 2);
    assert!(facts
        .view()
        .classes
        .iter()
        .any(|c| Some(&c.id) == factory.returned_class_id.as_ref() && c.id != c.declaration_id));
    assert!(facts
        .view()
        .relationship_sites
        .iter()
        .any(|s| s.kind == RelationshipSiteKind::Extends));
    fs::remove_dir_all(catalog.source_root()).unwrap();
    assert_eq!(
        open_facts(&catalog, &FactsRequest::default())
            .unwrap()
            .view(),
        facts.view()
    );
}

fn javascript_fixture(source: &str) -> Option<(tempfile::TempDir, CatalogRef)> {
    javascript_files(&[("factory.js", source)])
}

fn javascript_files(files: &[(&str, &str)]) -> Option<(tempfile::TempDir, CatalogRef)> {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("source");
    fs::create_dir(&root).unwrap();
    for (path, source) in files {
        fs::write(root.join(path), source).unwrap();
    }
    let discovered = discover(&DiscoverRequest {
        source_root: root,
        output_container: temp.path().join("catalogs"),
        toolchain: Toolchain::NativeJavaScriptTypeScript {},
    })
    .unwrap();
    Some((temp, open_catalog(discovered.catalog_directory).unwrap()))
}

fn assert_exported_citations(catalog: &CatalogRef, view: &FactsView) {
    let mut required = BTreeSet::new();
    for c in &view.classes {
        required.insert(c.declaration_id.as_str());
    }
    for f in &view.factories {
        required.extend([f.id.as_str(), f.callable_id.as_str()]);
        required.extend(
            f.evidence_ids
                .iter()
                .chain(&f.return_fact_ids)
                .map(String::as_str),
        );
        for p in &f.parameters {
            required.insert(p.id.as_str());
            required.extend(p.binding_id.as_deref());
        }
        assert!(f
            .returned_class_id
            .as_ref()
            .is_none_or(|id| view.classes.iter().any(|c| &c.id == id)));
    }
    for s in &view.relationship_sites {
        required.extend([s.id.as_str(), s.owner_id.as_str()]);
        required.extend(s.binding_id.as_deref());
        required.extend(s.evidence_ids.iter().map(String::as_str));
        assert!(view.classes.iter().any(|c| c.id == s.source_class_id));
        assert!(s
            .target_class_id
            .as_ref()
            .is_none_or(|id| view.classes.iter().any(|c| &c.id == id)));
    }
    assert_eq!(required, view.sources.keys().map(String::as_str).collect());
    let evidence: Value =
        serde_json::from_slice(&fs::read(catalog.directory().join("evidence.json")).unwrap())
            .unwrap();
    let by_id: BTreeMap<_, _> = evidence["facts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| (f["id"].as_str().unwrap(), f))
        .collect();
    for (id, citation) in &view.sources {
        let f = by_id[id.as_str()];
        assert_eq!(
            serde_json::to_value(citation).unwrap(),
            json!({"path":f["path"],"line":f["line"],"endLine":f["endLine"],"kind":f["kind"],"symbol":f["symbol"]})
        );
    }
}

#[test]
fn typed_facts_do_not_require_oversized_member_documents() {
    let mut files = vec![("factory.js".to_string(), "module.exports = function(Base) { class Target {} class Product extends Base {} return Product; };".to_string())];
    for n in 0..8 {
        let mut source = format!("export class Bulk{n} {{\n");
        for field in 0..1000 {
            source.push_str(&format!("state{field}: {};\n", "Declared".repeat(64)));
        }
        source.push_str("}\n");
        files.push((format!("bulk{n}.ts"), source));
    }
    let files: Vec<_> = files
        .iter()
        .map(|(p, s)| (p.as_str(), s.as_str()))
        .collect();
    let (_temp, catalog) = javascript_files(&files).unwrap();
    let error =
        class_diagrams(&catalog, &ClassSelection::All, &RenderMode::SourceOnly).unwrap_err();
    assert!(error.message.contains("class UML exceeds"), "{error}");
    fs::remove_dir_all(catalog.source_root()).unwrap();
    let facts = open_facts(&catalog, &FactsRequest::default()).unwrap();
    assert_exported_citations(&catalog, facts.view());
    assert!(serde_json::to_vec(facts.view()).unwrap().len() < 8 * 1024 * 1024);
    assert!(!catalog.directory().join("views").exists());
    validate_claims(&facts, &[set(&facts)]).unwrap();
    assert_eq!(
        facts.view(),
        open_facts(&catalog, &FactsRequest::default())
            .unwrap()
            .view()
    );
    rewrite_saved(&catalog, |evidence, _| {
        for f in evidence["facts"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .filter(|f| f["kind"] == "js_member")
        {
            let mut raw: Value = serde_json::from_str(f["value"].as_str().unwrap()).unwrap();
            if raw["memberKind"] == "field" {
                raw["type"] = json!("WideType".repeat(256));
                f["value"] = json!(raw.to_string());
            }
        }
    });
    let error =
        class_diagrams(&catalog, &ClassSelection::All, &RenderMode::SourceOnly).unwrap_err();
    assert!(error.message.contains("working budget exceeded"), "{error}");
    let enlarged = open_facts(&catalog, &FactsRequest::default()).unwrap();
    assert_eq!(enlarged.view().sources, facts.view().sources);
    assert_ne!(
        enlarged.view().identity.facts_hash,
        facts.view().identity.facts_hash
    );
}

#[test]
fn unrelated_call_flood_is_validated_but_not_exported_and_hashes_are_deterministic() {
    let (_temp, catalog) = javascript_fixture("module.exports = function(Base) { class Target {} class Product extends Base { constructor(value) { this.state = value; } } return Product; };").unwrap();
    let initial = open_facts(&catalog, &FactsRequest::default()).unwrap();
    rewrite_saved(&catalog, |evidence, _| {
        let facts = evidence["facts"].as_array_mut().unwrap();
        let module = facts
            .iter()
            .find(|f| f["kind"] == "module")
            .unwrap()
            .clone();
        for n in 0..4500 {
            let id = format!("unrelated-call-{n}");
            let mut call = module.clone();
            call["id"] = json!(id);
            call["kind"] = json!("call");
            call["symbol"] = json!(format!("app.{}", "neutral_".repeat(140)));
            call["value"] = json!("external()");
            facts.push(call.clone());
            call["id"] = json!(format!("unrelated-metadata-{n}"));
            call["kind"] = json!("js_call");
            call["value"] = json!(json!({"callFactId":id,"ownerFactId":module["id"],"callee":"external","targetFactId":null,"supportingFactIds":[],"lookupScopeIds":[]}).to_string());
            facts.push(call);
        }
        // Citation symbols alone would exceed the public facts-document cap.
        assert!(
            facts
                .iter()
                .map(|f| f["symbol"].as_str().unwrap().len())
                .sum::<usize>()
                > 8 * 1024 * 1024
        );
    });
    assert!(validate_claims(&initial, &[set(&initial)]).is_err());
    let facts = open_facts(&catalog, &FactsRequest::default()).unwrap();
    assert_eq!(facts.view().sources, initial.view().sources);
    assert_ne!(
        facts.view().identity.facts_hash,
        initial.view().identity.facts_hash
    );
    assert_exported_citations(&catalog, facts.view());
    let mut arbitrary = set(&facts);
    arbitrary.claims[0]
        .evidence_ids
        .push("unrelated-call-0".into());
    assert!(validate_claims(&facts, &[arbitrary]).is_err());
    let valid = set(&facts);
    validate_claims(&facts, std::slice::from_ref(&valid)).unwrap();
    rewrite_saved(&catalog, |evidence, _| {
        evidence["facts"].as_array_mut().unwrap().reverse()
    });
    let permuted = open_facts(&catalog, &FactsRequest::default()).unwrap();
    assert_eq!(facts.view(), permuted.view());
    // Raw saved-store changes still invalidate existing handles, even permutations.
    assert!(validate_claims(&facts, &[valid]).is_err());
    rewrite_saved(&catalog, |evidence, _| {
        let call = evidence["facts"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|f| f["id"] == "unrelated-call-0")
            .unwrap();
        call["value"] = json!("different saved call metadata");
    });
    let changed = open_facts(&catalog, &FactsRequest::default()).unwrap();
    assert_eq!(changed.view().sources, permuted.view().sources);
    assert_ne!(
        changed.view().identity.facts_hash,
        permuted.view().identity.facts_hash
    );
}

#[test]
fn omitted_member_and_call_citations_do_not_skip_global_proof_validation() {
    let source = "module.exports = function(Base) { class Target {} class Product extends Base {} return Product; }; class Auxiliary { constructor(value) { this.state = value; } run(input) { external(input); } }";
    for mutation in [
        "signature",
        "duplicate_signature",
        "constructor",
        "call",
        "span",
        "duplicate_fact",
    ] {
        let (_temp, catalog) = javascript_fixture(source).unwrap();
        let facts = open_facts(&catalog, &FactsRequest::default()).unwrap();
        assert_exported_citations(&catalog, facts.view());
        rewrite_saved(&catalog, |evidence, _| {
            let all = evidence["facts"].as_array_mut().unwrap();
            let kind = match mutation {
                "signature" | "duplicate_signature" => "js_callable_signature",
                "constructor" => "js_constructor_field",
                "call" => "js_call",
                _ => "call",
            };
            let pos = all
                .iter()
                .position(|f| {
                    f["kind"] == kind && f["symbol"].as_str().unwrap().contains("Auxiliary")
                })
                .unwrap();
            assert!(!facts
                .view()
                .sources
                .contains_key(all[pos]["id"].as_str().unwrap()));
            if mutation == "duplicate_signature" || mutation == "duplicate_fact" {
                let mut duplicate = all[pos].clone();
                if mutation == "duplicate_signature" {
                    duplicate["id"] = json!("duplicate-signature");
                }
                all.push(duplicate);
            } else if mutation == "span" {
                all[pos]["line"] = json!(0);
            } else {
                let mut raw: Value =
                    serde_json::from_str(all[pos]["value"].as_str().unwrap()).unwrap();
                match mutation {
                    "signature" => raw["returnType"] = json!(false),
                    "constructor" => raw["sourceType"] = json!("not allowed"),
                    "call" => raw["supportingFactIds"] = json!(["missing-proof"]),
                    _ => unreachable!(),
                }
                all[pos]["value"] = json!(raw.to_string());
            }
        });
        assert!(
            open_facts(&catalog, &FactsRequest::default()).is_err(),
            "accepted {mutation}"
        );
    }
}

#[test]
fn typed_relationship_targets_and_proofs_match_full_and_focused_uml() {
    let (_python_temp, python, _) =
        fixture_source("class Target: pass\nclass Child(Target): pass\n");
    let (_js_temp, javascript) = javascript_files(&[
        ("base.js", "export class Target {}"),
        ("factory.js", "import { Target } from './base'; export function factory(Dependency) { return class Product extends Target { constructor(value) { super(); this.state = value; } make() { return new Target(); } run() { return Dependency(); } }; }"),
    ]).unwrap();
    for catalog in [&python, &javascript] {
        let facts = open_facts(catalog, &FactsRequest::default()).unwrap();
        assert_exported_citations(catalog, facts.view());
        let child = facts
            .view()
            .classes
            .iter()
            .find(|c| c.name == "Child" || c.name == "Product")
            .unwrap();
        for selection in [
            ClassSelection::All,
            ClassSelection::Entry {
                id: child.id.clone(),
            },
        ] {
            let report = class_diagrams(catalog, &selection, &RenderMode::SourceOnly).unwrap();
            let legend: Value = serde_json::from_slice(&fs::read(report.legend).unwrap()).unwrap();
            for site in facts
                .view()
                .relationship_sites
                .iter()
                .filter(|s| selection == ClassSelection::All || s.source_class_id == child.id)
            {
                let r = legend["relationships"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|r| r["siteFactId"] == site.id)
                    .unwrap();
                assert_eq!(r["classId"], site.source_class_id);
                assert_eq!(r["ownerFactId"], site.owner_id);
                assert_eq!(r["targetClassId"], json!(site.target_class_id));
                assert_eq!(r["sourceFactIds"], json!(site.evidence_ids));
            }
        }
    }
}

#[test]
fn genuinely_oversized_exported_facts_still_fail_the_eight_mib_cap() {
    let (_temp, catalog, _) = fixture_source("class Widget: pass\n");
    rewrite_saved(&catalog, |evidence, items| {
        let symbol = format!("model.{}", "W".repeat(3 * 1024 * 1024));
        evidence["facts"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|f| f["kind"] == "class")
            .unwrap()["symbol"] = json!(symbol);
        let class = items["items"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|i| i["kind"] == "class")
            .unwrap();
        class["symbol"] = json!(symbol);
        class["label"] = json!(symbol);
    });
    let error = open_facts(&catalog, &FactsRequest::default()).unwrap_err();
    assert_eq!(error.message, "Facts metadata exceeds 8 MiB");
    assert!(!catalog.directory().join("views").exists());
}

// Re-sign synthetic metadata so regressions exercise grounding, not receipt tamper checks.
fn rewrite_saved(catalog: &CatalogRef, edit: impl FnOnce(&mut Value, &mut Value)) {
    use sha2::{Digest, Sha256};
    let directory = catalog.directory();
    let mut evidence: Value =
        serde_json::from_slice(&fs::read(directory.join("evidence.json")).unwrap()).unwrap();
    let mut items: Value =
        serde_json::from_slice(&fs::read(directory.join("catalog.json")).unwrap()).unwrap();
    edit(&mut evidence, &mut items);
    for item in items["items"].as_array_mut().unwrap() {
        let mut typed: CatalogItem = serde_json::from_value(item.clone()).unwrap();
        typed.id.clear();
        typed.evidence_ids.sort();
        typed.evidence_ids.dedup();
        let digest = format!("{:x}", Sha256::digest(serde_json::to_vec(&typed).unwrap()));
        let kind = match typed.kind {
            ItemKind::Api => "api",
            ItemKind::Class => "class",
            ItemKind::Workflow => "workflow",
        };
        typed.id = format!("{kind}_{}", &digest[..16]);
        *item = serde_json::to_value(typed).unwrap();
    }
    let mut receipt: Value =
        serde_json::from_slice(&fs::read(directory.join("receipt.json")).unwrap()).unwrap();
    for (name, key, value) in [
        ("evidence.json", "evidenceSha256", evidence),
        ("catalog.json", "catalogSha256", items),
    ] {
        let bytes = serde_json::to_vec_pretty(&value).unwrap();
        receipt[key] = json!(format!("{:x}", Sha256::digest(&bytes)));
        fs::write(directory.join(name), bytes).unwrap();
    }
    fs::write(
        directory.join("receipt.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .unwrap();
}

#[test]
fn source_syntax_requires_an_independently_resolved_target() {
    let (_temp, _catalog, facts) = fixture();
    let mut claim = set(&facts);
    claim.claims[0].basis = ClaimBasis::SourceSyntax;
    assert!(validate_claims(&facts, &[claim])
        .unwrap_err()
        .message
        .contains("independently resolved"));
    let (_temp, catalog, facts) =
        fixture_source("class Target: pass\nclass Other: pass\nclass Child(Target): pass\n");
    let mut syntax = set(&facts);
    syntax.claims[0].basis = ClaimBasis::SourceSyntax;
    assert_eq!(
        facts.view().relationship_sites[0].target_class_id.as_ref(),
        Some(&syntax.claims[0].target_class_id)
    );
    let mut contract = syntax.clone();
    contract.adapter.id = "other.adapter".into();
    contract.claims[0].basis = ClaimBasis::FrameworkContract;
    let claims = validate_claims(&facts, &[syntax, contract]).unwrap();
    let report = class_diagrams_augmented(
        &catalog,
        &ClassSelection::All,
        &claims,
        &RenderMode::SourceOnly,
    )
    .unwrap();
    let legend: Value = serde_json::from_slice(&fs::read(report.report.legend).unwrap()).unwrap();
    assert_eq!(
        legend["relationships"][0]["claimOrigins"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        legend["groups"][0]["drawnEdges"].as_array().unwrap().len(),
        1
    );
}

#[test]
fn relationship_sites_are_origins_not_the_first_supporting_citation() {
    let Some((_temp, catalog)) = javascript_fixture("module.exports = function(Base) { class Target {} class Actual {} class Child extends Base { make() { return new Actual(); } } return Child; };\n") else { return };
    rewrite_saved(&catalog, |evidence, items| {
        let base_id = evidence["facts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["kind"] == "base")
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned();
        *evidence = serde_json::from_str(
            &serde_json::to_string(evidence)
                .unwrap()
                .replace(&base_id, "aaa-base-proof"),
        )
        .unwrap();
        *items = serde_json::from_str(
            &serde_json::to_string(items)
                .unwrap()
                .replace(&base_id, "aaa-base-proof"),
        )
        .unwrap();
        for fact in evidence["facts"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .filter(|f| f["kind"] == "js_class_relation")
        {
            let mut raw: Value = serde_json::from_str(fact["value"].as_str().unwrap()).unwrap();
            if raw["relationship"] == "creates" {
                raw["supportingFactIds"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!("aaa-base-proof"));
            }
            fact["value"] = json!(serde_json::to_string(&raw).unwrap());
        }
    });
    let facts = open_facts(&catalog, &FactsRequest::default()).unwrap();
    let extends = facts
        .view()
        .relationship_sites
        .iter()
        .find(|s| s.kind == RelationshipSiteKind::Extends)
        .unwrap();
    let creates = facts
        .view()
        .relationship_sites
        .iter()
        .find(|s| s.kind == RelationshipSiteKind::Construction)
        .unwrap();
    assert_ne!(extends.id, creates.id);
    assert_ne!(creates.id, "aaa-base-proof");
    assert_eq!(creates.evidence_ids[0], "aaa-base-proof");
    let mut claim = set(&facts);
    claim.claims[0].site_id = extends.id.clone();
    claim.claims[0].source_class_id = extends.source_class_id.clone();
    claim.claims[0].evidence_ids = extends.evidence_ids.clone();
    let target = facts
        .view()
        .classes
        .iter()
        .find(|c| c.name == "Target")
        .unwrap();
    claim.claims[0].target_class_id = target.id.clone();
    claim.claims[0]
        .evidence_ids
        .push(target.declaration_id.clone());
    let validated = validate_claims(&facts, &[claim]).unwrap();
    let report = class_diagrams_augmented(
        &catalog,
        &ClassSelection::All,
        &validated,
        &RenderMode::SourceOnly,
    )
    .unwrap();
    let legend: Value = serde_json::from_slice(&fs::read(report.report.legend).unwrap()).unwrap();
    let construction = legend["relationships"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["siteFactId"] == creates.id)
        .unwrap();
    assert_eq!(
        construction["targetClassId"],
        json!(creates.target_class_id)
    );
    assert!(construction["claimOrigins"].is_null());
}

#[test]
fn factory_parameters_and_return_guards_cannot_borrow_nested_or_unrelated_proofs() {
    let source = "module.exports = function(Input, Next) { class Other {} class Product { run(Input) {} } return Product; };\n";
    for mutation in [
        "nested_parameter",
        "parameter_order",
        "wrong_kind",
        "unrelated_class",
        "missing_guard",
    ] {
        let Some((_temp, catalog)) = javascript_fixture(source) else {
            return;
        };
        open_facts(&catalog, &FactsRequest::default()).unwrap();
        rewrite_saved(&catalog, |evidence, _| {
            let facts = evidence["facts"].as_array_mut().unwrap();
            let factory = facts
                .iter()
                .position(|f| f["kind"] == "js_factory")
                .unwrap();
            let ret = facts
                .iter()
                .position(|f| f["kind"] == "js_factory_return")
                .unwrap();
            let mut fv: Value =
                serde_json::from_str(facts[factory]["value"].as_str().unwrap()).unwrap();
            let mut rv: Value =
                serde_json::from_str(facts[ret]["value"].as_str().unwrap()).unwrap();
            match mutation {
                "nested_parameter" => {
                    let nested = facts
                        .iter()
                        .find(|f| {
                            f["kind"] == "parameter"
                                && f["symbol"].as_str().unwrap().contains(".Product.run.Input")
                        })
                        .unwrap()["id"]
                        .clone();
                    fv["parameters"][0]["parameterFactId"] = nested;
                }
                "parameter_order" => {
                    fv["parameters"].as_array_mut().unwrap().swap(0, 1);
                    fv["parameters"][0]["position"] = json!(0);
                    fv["parameters"][1]["position"] = json!(1);
                }
                "wrong_kind" => rv["bindingGuardFactId"] = rv["classFactId"].clone(),
                "unrelated_class" => {
                    let other = facts
                        .iter()
                        .find(|f| {
                            f["kind"] == "js_binding_guard"
                                && serde_json::from_str::<Value>(f["value"].as_str().unwrap())
                                    .unwrap()["name"]
                                    == "Other"
                        })
                        .unwrap()["id"]
                        .clone();
                    rv["bindingGuardFactId"] = other.clone();
                    rv["supportingFactIds"].as_array_mut().unwrap().push(other);
                }
                "missing_guard" => rv["bindingGuardFactId"] = Value::Null,
                _ => unreachable!(),
            }
            facts[factory]["value"] = json!(serde_json::to_string(&fv).unwrap());
            facts[ret]["value"] = json!(serde_json::to_string(&rv).unwrap());
        });
        assert!(
            open_facts(&catalog, &FactsRequest::default()).is_err(),
            "{mutation}"
        );
    }
}

#[test]
fn direct_returned_class_expressions_need_no_local_binding_guard() {
    for source in [
        "module.exports = () => class Product {};\n",
        "module.exports = function() { return class Product {}; };\n",
        "module.exports = function() { class Product {} return Product; };\n",
        "module.exports = function() { const Alias = class Product {}; return Alias; };\n",
        "module.exports = function(Product) { return class Product {}; };\n",
        "module.exports = function(Product) {\n  return class Product {\n    constructor() {}\n  };\n};\n",
        "module.exports = function(Product) { return new (class Product {})(); };\n",
        "module.exports = function(Product) {\n  return new (class Product {\n    constructor() {}\n  })();\n};\n",
    ] {
        let Some((_temp, catalog)) = javascript_fixture(source) else {
            return;
        };
        let facts = open_facts(&catalog, &FactsRequest::default()).unwrap();
        assert!(facts.view().factories[0].safe);
    }
}

#[test]
fn deleted_return_guards_cannot_turn_separate_classes_into_direct_expressions() {
    for source in [
        "module.exports = function() { class Product {} return Product; };\n",
        "module.exports = function() { const Product = class Product {}; return Product; };\n",
    ] {
        for forge_form in [false, true] {
            let Some((_temp, catalog)) = javascript_fixture(source) else {
                return;
            };
            open_facts(&catalog, &FactsRequest::default()).unwrap();
            rewrite_saved(&catalog, |evidence, _| {
                let ret = evidence["facts"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|f| f["kind"] == "js_factory_return")
                    .unwrap();
                let mut raw: Value = serde_json::from_str(ret["value"].as_str().unwrap()).unwrap();
                raw["bindingGuardFactId"] = Value::Null;
                if forge_form {
                    raw["valueForm"] = json!("class_expression");
                }
                ret["value"] = json!(serde_json::to_string(&raw).unwrap());
            });
            let error = open_facts(&catalog, &FactsRequest::default()).unwrap_err();
            assert!(
                error.message.contains("direct class expression"),
                "{source}: {error}"
            );
        }
    }
}

#[test]
fn generic_returns_require_independent_form_and_position_receipts() {
    for field in [
        "valueForm",
        "valueStart",
        "valueEnd",
        "classForm",
        "start",
        "end",
    ] {
        let Some((_temp, catalog)) = javascript_fixture(
            "module.exports = function(Product) { return class Product {}; };\n",
        ) else {
            return;
        };
        rewrite_saved(&catalog, |evidence, _| {
            let facts = evidence["facts"].as_array_mut().unwrap();
            let kind = if field.starts_with("value") {
                "js_factory_return"
            } else {
                "js_declaration"
            };
            let fact = facts
                .iter_mut()
                .find(|f| {
                    f["kind"] == kind
                        && (kind != "js_declaration"
                            || serde_json::from_str::<Value>(f["value"].as_str().unwrap()).unwrap()
                                ["kind"]
                                == "class")
                })
                .unwrap();
            let mut raw: Value = serde_json::from_str(fact["value"].as_str().unwrap()).unwrap();
            raw.as_object_mut().unwrap().remove(field);
            fact["value"] = json!(serde_json::to_string(&raw).unwrap());
        });
        assert!(
            open_facts(&catalog, &FactsRequest::default()).is_err(),
            "{field}"
        );
    }
}

#[test]
fn omission_markers_disable_capabilities_even_beyond_the_public_diagnostic_limit() {
    for marker in [
        "javascript_structural_facts_omitted",
        "javascript_class_relations_omitted",
    ] {
        let Some((_temp, catalog)) = javascript_fixture("module.exports = function(Base) { class Target {} class Child extends Base {} return Child; };\n") else { return };
        rewrite_saved(&catalog, |evidence, items| {
            let diagnostics = evidence["diagnostics"].as_array_mut().unwrap();
            for _ in 0..110 {
                diagnostics.push(json!({"code":"synthetic_warning", "path":"", "message":"bounded synthetic diagnostic"}));
            }
            diagnostics.push(
                json!({"code":marker, "path":"", "message":"line 1: optional facts omitted"}),
            );
            items["warnings"].as_array_mut().unwrap().push(json!("Source diagnostics are present; extraction or dynamic discovery may be unresolved. Consult the source evidence diagnostics."));
            items["warnings"]
                .as_array_mut()
                .unwrap()
                .sort_by(|a, b| a.as_str().cmp(&b.as_str()));
            items["warnings"].as_array_mut().unwrap().dedup();
        });
        let facts = open_facts(&catalog, &FactsRequest::default()).unwrap();
        assert!(facts.view().complete);
        assert!(facts.view().diagnostics.len() <= 100);
        let mut claims = set(&facts);
        if marker == "javascript_structural_facts_omitted" {
            assert!(!facts.view().coverage.exported_factory_structure);
            assert!(facts.view().coverage.class_relationship_sites);
            claims.claims[0]
                .evidence_ids
                .push(facts.view().factories[0].id.clone());
        } else {
            assert!(!facts.view().coverage.class_relationship_sites);
        }
        assert!(validate_claims(&facts, &[claims]).is_err());
    }
}
