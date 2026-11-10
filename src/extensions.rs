//! Typed, snapshot-bound structural facts and externally approved relationship claims.
use crate::{
    analysis::hash,
    discovery,
    engine::v1::{CatalogRef, ClassDiagramReport, ClassSelection, RenderMode},
    workflow,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::Path,
};

const LIMIT: usize = 1024 * 1024;
macro_rules! dto {
    ($name:ident { $($field:ident : $ty:ty),* $(,)? }) => {
        #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        pub struct $name { $(pub $field: $ty),* }
    };
}
dto!(ExtensionCapabilities {
    schema_version: String,
    exported_factory_structure: bool,
    class_relationship_sites: bool,
    max_claim_sets: usize,
    max_claims: usize,
    max_claim_bytes: usize
});
dto!(FactsIdentity {
    source_root_id: String,
    snapshot_id: String,
    extractor_version: String,
    inventory_hash: String,
    facts_hash: String
});
dto!(FactCoverage {
    class_relationship_sites: bool,
    exported_factory_structure: bool,
    unsupported_shapes: usize
});
dto!(FactDiagnostic {
    code: String,
    path: String,
    message: String
});
dto!(FactFile {
    path: String,
    sha256: String
});
dto!(SourceCitation {
    path: String,
    line: usize,
    end_line: usize,
    kind: String,
    symbol: String
});
dto!(ClassFact {
    id: String,
    declaration_id: String,
    name: String,
    symbol: String,
    path: String,
    line: usize,
    end_line: usize
});
dto!(ParameterFact { id: String, binding_id: Option<String>, name: String, position: usize, optional: bool, rest: bool, has_default: bool });
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FactoryResultKind {
    Class,
    Instance,
    Unsupported,
    Ambiguous,
}
dto!(FactoryFact { id: String, callable_id: String, module_path: String, parameters: Vec<ParameterFact>, result_kind: FactoryResultKind, returned_class_id: Option<String>, return_fact_ids: Vec<String>, evidence_ids: Vec<String>, safe: bool, reason: Option<String> });
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipSiteKind {
    Extends,
    Dependency,
    Construction,
}
dto!(RelationshipSite { id: String, source_class_id: String, owner_id: String, owner_kind: String, binding_id: Option<String>, binding_name: Option<String>, binding_origin: Option<String>, kind: RelationshipSiteKind, member_name: Option<String>, target_class_id: Option<String>, evidence_ids: Vec<String> });
dto!(FactsView { schema_version: String, identity: FactsIdentity, complete: bool, coverage: FactCoverage, diagnostics: Vec<FactDiagnostic>, files: Vec<FactFile>, classes: Vec<ClassFact>, factories: Vec<FactoryFact>, relationship_sites: Vec<RelationshipSite>, sources: BTreeMap<String, SourceCitation> });
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct FactsRequest {
    pub require_complete: bool,
}
impl Default for FactsRequest {
    fn default() -> Self {
        Self {
            require_complete: true,
        }
    }
}
dto!(AdapterIdentity {
    id: String,
    version: String,
    implementation: String
});
dto!(ContractIdentity {
    id: String,
    version: String,
    spec_hash: String
});
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClaimKind {
    Extends,
    DependsOn,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClaimBasis {
    SourceSyntax,
    FrameworkContract,
    ExplicitConfiguration,
    Hypothesis,
}
dto!(ClassClaim { id: String, site_id: String, source_class_id: String, target_class_id: String, kind: ClaimKind, basis: ClaimBasis, evidence_ids: Vec<String>, rule_id: String, assumptions: Vec<String> });
dto!(ClaimSet { schema_version: String, facts: FactsIdentity, adapter: AdapterIdentity, contract: ContractIdentity, configuration_hash: String, claims: Vec<ClassClaim>, diagnostics: Vec<FactDiagnostic> });
dto!(AugmentationOrigin {
    adapter: AdapterIdentity,
    contract: ContractIdentity,
    configuration_hash: String,
    claim_set_hash: String
});
dto!(AugmentationReport { schema_version: String, composition_policy: String, facts: FactsIdentity, origins: Vec<AugmentationOrigin>, claim_count: usize });
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AugmentedClassDiagramReport {
    pub schema_version: String,
    pub report: ClassDiagramReport,
    pub augmentation: AugmentationReport,
}
#[derive(Clone, Debug, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[error("{message}")]
pub struct ExtensionError {
    pub message: String,
}
impl From<String> for ExtensionError {
    fn from(message: String) -> Self {
        Self { message }
    }
}
fn invalid(message: &str) -> ExtensionError {
    message.to_string().into()
}
#[derive(Clone, Debug)]
pub struct FactsRef {
    catalog: CatalogRef,
    view: FactsView,
    receipt_hash: String,
}
impl FactsRef {
    pub fn view(&self) -> &FactsView {
        &self.view
    }
}
#[derive(Clone, Debug)]
pub struct ValidatedClaimsRef {
    facts: FactsRef,
    sets: Vec<ClaimSet>,
    augmentation: AugmentationReport,
}
impl ValidatedClaimsRef {
    pub fn augmentation(&self) -> &AugmentationReport {
        &self.augmentation
    }
    pub(crate) fn augmentation_evidence(&self) -> impl Iterator<Item = &str> {
        self.sets
            .iter()
            .flat_map(|s| &s.claims)
            .flat_map(|c| &c.evidence_ids)
            .map(String::as_str)
    }
}

pub fn extension_capabilities() -> ExtensionCapabilities {
    ExtensionCapabilities {
        schema_version: "ideomancer.extensions.v1".into(),
        exported_factory_structure: true,
        class_relationship_sites: true,
        max_claim_sets: 4,
        max_claims: 2000,
        max_claim_bytes: LIMIT,
    }
}
fn digest<T: Serialize>(value: &T) -> Result<String, ExtensionError> {
    // Value uses sorted object keys, independent of DTO field order.
    let value = serde_json::to_value(value).map_err(|e| invalid(&e.to_string()))?;
    Ok(hash(
        &serde_json::to_vec(&value).map_err(|e| invalid(&e.to_string()))?,
    ))
}
fn receipt(store: &discovery::Store) -> Result<String, ExtensionError> {
    digest(&(&store.catalog, &store.evidence))
}
fn string(v: &Value, key: &str) -> Result<String, ExtensionError> {
    v[key]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| invalid("Invalid structural fact string"))
}
fn ids(v: &Value, key: &str) -> Result<Vec<String>, ExtensionError> {
    v[key]
        .as_array()
        .ok_or_else(|| invalid("Invalid structural fact IDs"))?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or_else(|| invalid("Invalid structural fact ID"))
        })
        .collect()
}
pub fn open_facts(
    catalog: &CatalogRef,
    request: &FactsRequest,
) -> Result<FactsRef, ExtensionError> {
    let store = discovery::load(catalog.directory())?;
    if Path::new(&store.catalog.source_root) != catalog.source_root()
        || store.catalog.snapshot_id != catalog.snapshot_id()
    {
        return Err(invalid("Catalog handle is stale"));
    }
    if request.require_complete && !store.catalog.complete {
        return Err(invalid(
            "Complete facts required; rediscover unavailable sources",
        ));
    }
    let bundle = crate::render::class_uml::build(&store.catalog, &store.evidence, None)?;
    let evidence = &store.evidence;
    let by_id: BTreeMap<_, _> = evidence.facts.iter().map(|f| (f.id.as_str(), f)).collect();
    let mut sources = BTreeMap::new();
    for f in &evidence.facts {
        if f.line == 0
            || f.end_line < f.line
            || !evidence.files.iter().any(|file| file.path == f.path)
            || sources
                .insert(
                    f.id.clone(),
                    SourceCitation {
                        path: f.path.clone(),
                        line: f.line,
                        end_line: f.end_line,
                        kind: f.kind.clone(),
                        symbol: f.symbol.clone(),
                    },
                )
                .is_some()
        {
            return Err(invalid("Invalid source fact inventory"));
        }
    }
    let classes: Vec<_> = store
        .catalog
        .items
        .iter()
        .filter(|i| i.kind == crate::catalog::Kind::Class)
        .map(|i| ClassFact {
            id: i.id.clone(),
            declaration_id: i.fact_id.clone(),
            name: i.symbol.rsplit('.').next().unwrap_or(&i.symbol).into(),
            symbol: i.symbol.clone(),
            path: i.path.clone(),
            line: i.line,
            end_line: i.end_line,
        })
        .collect();
    let mut sites = Vec::new();
    let mut site_ids = BTreeSet::new();
    for r in bundle.legend["relationships"]
        .as_array()
        .ok_or_else(|| invalid("Missing relationship model"))?
    {
        let evidence_ids = ids(r, "sourceFactIds")?;
        let id = string(r, "siteFactId")?;
        let origin = by_id
            .get(id.as_str())
            .ok_or_else(|| invalid("Missing relationship source site"))?;
        let source_class_id = string(r, "classId")?;
        let class = classes
            .iter()
            .find(|c| c.id == source_class_id)
            .ok_or_else(|| invalid("Unknown site source class"))?;
        if !site_ids.insert(id.clone()) || !evidence_ids.contains(&id) || origin.path != class.path
        {
            return Err(invalid("Duplicate or ungrounded relationship site"));
        }
        match origin.kind.as_str() {
            "js_class_relation" => {
                let raw: Value = serde_json::from_str(&origin.value)
                    .map_err(|_| invalid("Invalid relationship site"))?;
                if raw["classFactId"] != class.declaration_id
                    || raw["relationship"] != r["relationship"]
                    || raw["ownerFactId"] != r["ownerFactId"]
                {
                    return Err(invalid("Relationship site identity mismatch"));
                }
            }
            "base"
                if r["relationship"] == "extends"
                    && origin.symbol == class.symbol
                    && class.line <= origin.line
                    && origin.end_line <= class.end_line => {}
            _ => return Err(invalid("Incompatible relationship source site")),
        }
        let owner_id = string(r, "ownerFactId")?;
        sites.push(RelationshipSite {
            id,
            source_class_id,
            owner_kind: sources
                .get(&owner_id)
                .ok_or_else(|| invalid("Unknown relationship owner"))?
                .kind
                .clone(),
            owner_id,
            binding_id: r["bindingGuardFactId"].as_str().map(str::to_owned),
            binding_name: r["expression"].as_str().map(str::to_owned),
            binding_origin: r["bindingKind"].as_str().map(str::to_owned),
            kind: match r["relationship"].as_str() {
                Some("extends") => RelationshipSiteKind::Extends,
                Some("creates") => RelationshipSiteKind::Construction,
                _ => RelationshipSiteKind::Dependency,
            },
            member_name: r["memberName"].as_str().map(str::to_owned),
            target_class_id: r["targetClassId"].as_str().map(str::to_owned),
            evidence_ids,
        });
    }
    let factory_capability =
        crate::javascript::supports_structural_facts(&evidence.extractor_version);
    let mut factories = Vec::new();
    if factory_capability {
        let index = crate::js_focus::Index::new(evidence.facts.iter().collect())?;
        for f in evidence.facts.iter().filter(|f| f.kind == "js_factory") {
            let v: Value =
                serde_json::from_str(&f.value).map_err(|_| invalid("Invalid factory receipt"))?;
            if v["policy"] != "javascript-structural-facts-1"
                || v["modulePath"] != f.path
                || !matches!(
                    v["exportKind"].as_str(),
                    Some("commonjs_default" | "esm_named")
                )
            {
                return Err(invalid("Invalid factory policy or module"));
            }
            let callable_id = string(&v, "factoryFactId")?;
            let callable = by_id
                .get(callable_id.as_str())
                .ok_or_else(|| invalid("Unknown factory callable"))?;
            let module_id = string(&v, "moduleFactId")?;
            let module = by_id
                .get(module_id.as_str())
                .ok_or_else(|| invalid("Unknown factory module"))?;
            if !crate::js_focus::callable(callable)
                || callable.path != f.path
                || module.path != f.path
                || module.kind != "module"
                || f.line < callable.line
                || f.end_line > callable.end_line
            {
                return Err(invalid("Invalid factory ownership"));
            }
            let ci = index
                .facts
                .iter()
                .position(|f| f.id == callable_id)
                .ok_or_else(|| invalid("Missing factory declaration"))?;
            if index
                .owners
                .get(&ci)
                .is_none_or(|i| index.facts[*i].id != module.id)
            {
                return Err(invalid("Factory declaration is not module-owned"));
            }
            let mut evidence_ids = ids(&v, "supportingFactIds")?;
            evidence_ids.extend([f.id.clone(), callable_id.clone(), module_id]);
            let parameter_prefix = format!("{}.", callable.symbol);
            let baseline_parameters: Vec<_> = evidence
                .facts
                .iter()
                .filter(|p| {
                    p.kind == "parameter"
                        && p.path == callable.path
                        && p.line >= callable.line
                        && p.end_line <= callable.end_line
                        && p.symbol
                            .strip_prefix(&parameter_prefix)
                            .is_some_and(|name| !name.contains('.'))
                })
                .collect();
            let mut parameters = Vec::new();
            let mut factory_scope = None;
            for p in v["parameters"]
                .as_array()
                .ok_or_else(|| invalid("Invalid factory parameters"))?
            {
                let id = string(p, "parameterFactId")?;
                let parameter = by_id
                    .get(id.as_str())
                    .ok_or_else(|| invalid("Unknown parameter"))?;
                let binding_id = p["bindingGuardFactId"].as_str().map(str::to_owned);
                if parameter.path != callable.path
                    || parameter.line < callable.line
                    || parameter.end_line > callable.end_line
                    || !matches!(parameter.kind.as_str(), "parameter" | "js_parameter")
                    || parameter.symbol != format!("{}.{}", callable.symbol, string(p, "name")?)
                    || baseline_parameters
                        .get(parameters.len())
                        .map(|p| p.id.as_str())
                        != Some(id.as_str())
                {
                    return Err(invalid("Invalid factory parameter ownership"));
                }
                if p["position"].as_u64() != Some(parameters.len() as u64)
                    || parameters.iter().any(|p: &ParameterFact| p.id == id)
                {
                    return Err(invalid("Duplicate or unordered factory parameters"));
                }
                if let Some(id) = &binding_id {
                    let guard = by_id
                        .get(id.as_str())
                        .ok_or_else(|| invalid("Missing parameter guard"))?;
                    let gv: Value = serde_json::from_str(&guard.value)
                        .map_err(|_| invalid("Invalid parameter guard"))?;
                    if guard.kind != "js_binding_guard"
                        || guard.path != callable.path
                        || guard.line < callable.line
                        || guard.end_line > callable.end_line
                        || guard.line != parameter.line
                        || guard.end_line != parameter.end_line
                        || gv["bindingKind"] != "parameter"
                        || gv["scopeKind"] != "function"
                        || !gv["declarationFactId"].is_null()
                        || gv["name"] != p["name"]
                        || gv["declarationCount"] != 1
                        || gv["writes"].as_u64().is_none()
                    {
                        return Err(invalid("Invalid factory parameter guard"));
                    }
                    let scope = (
                        string(&gv, "scopeId")?,
                        gv["scopeStart"]
                            .as_u64()
                            .ok_or_else(|| invalid("Invalid parameter scope"))?,
                        gv["scopeEnd"]
                            .as_u64()
                            .ok_or_else(|| invalid("Invalid parameter scope"))?,
                    );
                    if scope.1 >= scope.2
                        || scope.2 > 1024 * 1024
                        || factory_scope
                            .as_ref()
                            .is_some_and(|previous| previous != &scope)
                    {
                        return Err(invalid("Factory parameter scope mismatch"));
                    }
                    for sf in evidence
                        .facts
                        .iter()
                        .filter(|sf| sf.kind == "js_class_scope" && sf.path == callable.path)
                    {
                        let sv: Value = serde_json::from_str(&sf.value)
                            .map_err(|_| invalid("Invalid parameter scope receipt"))?;
                        if sv["scopeId"] == scope.0
                            && (sv["scopeKind"] != "function"
                                || sv["start"] != scope.1
                                || sv["end"] != scope.2
                                || sf.line != callable.line
                                || sf.end_line != callable.end_line)
                        {
                            return Err(invalid(
                                "Factory parameter scope is not the callable scope",
                            ));
                        }
                    }
                    factory_scope = Some(scope);
                }
                evidence_ids.push(id.clone());
                if let Some(id) = &binding_id {
                    evidence_ids.push(id.clone());
                }
                parameters.push(ParameterFact {
                    id,
                    binding_id,
                    name: string(p, "name")?,
                    position: p["position"]
                        .as_u64()
                        .ok_or_else(|| invalid("Invalid parameter position"))?
                        as usize,
                    optional: p["optional"]
                        .as_bool()
                        .ok_or_else(|| invalid("Invalid parameter flags"))?,
                    rest: p["rest"]
                        .as_bool()
                        .ok_or_else(|| invalid("Invalid parameter flags"))?,
                    has_default: p["hasDefault"]
                        .as_bool()
                        .ok_or_else(|| invalid("Invalid parameter flags"))?,
                });
            }
            if parameters.len() != baseline_parameters.len() {
                return Err(invalid("Factory parameter inventory mismatch"));
            }
            let return_fact_ids = ids(&v, "returnFactIds")?;
            for id in &return_fact_ids {
                let r = by_id
                    .get(id.as_str())
                    .ok_or_else(|| invalid("Unknown factory return"))?;
                let rv: Value = serde_json::from_str(&r.value)
                    .map_err(|_| invalid("Invalid factory return receipt"))?;
                if r.kind != "js_factory_return"
                    || r.path != callable.path
                    || r.line < callable.line
                    || r.end_line > callable.end_line
                    || rv["factoryFactId"] != callable_id
                {
                    return Err(invalid("Invalid factory return ownership"));
                }
                if rv["policy"] != "javascript-structural-facts-1"
                    || !rv
                        .get("bindingGuardFactId")
                        .is_some_and(|v| v.is_null() || v.as_str().is_some_and(text))
                    || !matches!(
                        rv["valueKind"].as_str(),
                        Some("class" | "instance" | "unsupported")
                    )
                {
                    return Err(invalid("Invalid factory return policy or kind"));
                }
                let value_form = string(&rv, "valueForm")?;
                if !matches!(
                    value_form.as_str(),
                    "class_expression"
                        | "new_class_expression"
                        | "identifier"
                        | "new_identifier"
                        | "unsupported"
                ) || !rv
                    .get("valueStart")
                    .is_some_and(|v| v.is_null() || v.as_u64().is_some())
                    || !rv
                        .get("valueEnd")
                        .is_some_and(|v| v.is_null() || v.as_u64().is_some())
                    || !(matches!((rv["valueStart"].as_u64(), rv["valueEnd"].as_u64()), (Some(start), Some(end)) if start < end && end <= 1024 * 1024)
                        || (rv["valueKind"] == "unsupported"
                            && value_form == "unsupported"
                            && rv["valueStart"].is_null()
                            && rv["valueEnd"].is_null()))
                {
                    return Err(invalid("Invalid factory return value form or positions"));
                }
                for id in ids(&rv, "supportingFactIds")? {
                    let proof = by_id
                        .get(id.as_str())
                        .ok_or_else(|| invalid("Missing return proof"))?;
                    if proof.path != callable.path {
                        return Err(invalid("Cross-file factory proof"));
                    }
                    if proof.kind == "js_binding_guard" && rv["valueKind"] != "unsupported" {
                        let gv: Value = serde_json::from_str(&proof.value)
                            .map_err(|_| invalid("Invalid return guard"))?;
                        if gv["declarationCount"] != 1 || gv["writes"] != 0 {
                            return Err(invalid("Unstable returned class binding"));
                        }
                    }
                }
                evidence_ids.extend(ids(&rv, "supportingFactIds")?);
                if rv["valueKind"] != "unsupported" {
                    let returned_id = string(&rv, "classFactId")?;
                    let returned = by_id
                        .get(returned_id.as_str())
                        .ok_or_else(|| invalid("Unknown returned class proof"))?;
                    let ri = index
                        .facts
                        .iter()
                        .position(|f| f.id == returned_id)
                        .ok_or_else(|| invalid("Unknown returned declaration"))?;
                    let (_, metadata) = index
                        .declarations
                        .get(&ri)
                        .ok_or_else(|| invalid("Missing returned class metadata"))?;
                    if returned.kind != "class" || index.owners.get(&ri) != Some(&ci) {
                        return Err(invalid("Returned class is not factory-owned"));
                    }
                    let class_form = string(metadata, "classForm")?;
                    let class_start = metadata["start"]
                        .as_u64()
                        .ok_or_else(|| invalid("Missing returned class start"))?;
                    let class_end = metadata["end"]
                        .as_u64()
                        .ok_or_else(|| invalid("Missing returned class end"))?;
                    if !matches!(
                        class_form.as_str(),
                        "class_declaration" | "class_expression"
                    ) || class_start >= class_end
                        || class_end > 1024 * 1024
                        || !matches!(
                            (rv["valueKind"].as_str(), value_form.as_str()),
                            (Some("class"), "class_expression" | "identifier")
                                | (Some("instance"), "new_class_expression" | "new_identifier")
                        )
                    {
                        return Err(invalid(
                            "Returned class form does not match the return value",
                        ));
                    }
                    let value_start = rv["valueStart"]
                        .as_u64()
                        .ok_or_else(|| invalid("Missing supported return start"))?;
                    let value_end = rv["valueEnd"]
                        .as_u64()
                        .ok_or_else(|| invalid("Missing supported return end"))?;
                    if let Some(id) = rv["bindingGuardFactId"].as_str() {
                        let guard = by_id
                            .get(id)
                            .ok_or_else(|| invalid("Unknown returned binding guard"))?;
                        let gv: Value = serde_json::from_str(&guard.value)
                            .map_err(|_| invalid("Invalid returned binding guard"))?;
                        let supporting = ids(&rv, "supportingFactIds")?;
                        let class_scopes: Vec<_> = index
                            .guards
                            .iter()
                            .filter(|(i, proof)| {
                                index.facts[*i].path == returned.path
                                    && proof["scopeKind"] == "class"
                                    && proof["name"] == metadata["name"]
                                    && index.facts[*i].line == returned.line
                                    && index.facts[*i].end_line == returned.end_line
                                    && supporting.iter().any(|id| id == &index.facts[*i].id)
                            })
                            .collect();
                        if guard.kind != "js_binding_guard"
                            || !matches!(value_form.as_str(), "identifier" | "new_identifier")
                            || guard.path != callable.path
                            || !matches!(
                                gv["bindingKind"].as_str(),
                                Some("class" | "const" | "let" | "var")
                            )
                            || gv["scopeKind"] != "function"
                            || gv["scopeId"].as_str().is_none_or(|id| !text(id))
                            || !matches!((gv["scopeStart"].as_u64(), gv["scopeEnd"].as_u64(), gv["initializationEnd"].as_u64()), (Some(start), Some(end), Some(initialized)) if start < initialized && initialized <= end && end <= 1024 * 1024)
                            || gv["initializationEnd"].as_u64().is_none_or(|initialized| {
                                class_end > initialized || initialized > value_start
                            })
                            || gv["declarationCount"] != 1
                            || gv["writes"] != 0
                            || index
                                .guards
                                .iter()
                                .filter(|(i, proof)| {
                                    index.facts[*i].path == guard.path
                                        && proof["scopeId"] == gv["scopeId"]
                                        && proof["name"] == gv["name"]
                                })
                                .count()
                                != 1
                            || (gv["name"] != metadata["name"] && class_scopes.is_empty())
                            || class_scopes.len() > 1
                            || class_scopes.first().is_some_and(|(_, proof)| {
                                proof["scopeEnd"] != gv["initializationEnd"]
                            })
                            || guard.line > returned.line
                            || guard.end_line < returned.end_line
                            || guard.line < callable.line
                            || guard.end_line > callable.end_line
                            || (!gv["declarationFactId"].is_null()
                                && gv["declarationFactId"] != returned.id)
                            || !supporting.iter().any(|proof| proof == id)
                            || factory_scope.as_ref().is_some_and(|scope| {
                                gv["scopeId"] != scope.0
                                    || gv["scopeStart"] != scope.1
                                    || gv["scopeEnd"] != scope.2
                            })
                        {
                            return Err(invalid("Returned class binding guard mismatch"));
                        }
                        for sf in evidence
                            .facts
                            .iter()
                            .filter(|sf| sf.kind == "js_class_scope" && sf.path == callable.path)
                        {
                            let sv: Value = serde_json::from_str(&sf.value)
                                .map_err(|_| invalid("Invalid returned class scope receipt"))?;
                            if sv["scopeId"] == gv["scopeId"]
                                && (sv["scopeKind"] != "function"
                                    || sv["start"] != gv["scopeStart"]
                                    || sv["end"] != gv["scopeEnd"]
                                    || sf.line != callable.line
                                    || sf.end_line != callable.end_line)
                            {
                                return Err(invalid(
                                    "Returned binding scope is not the factory callable",
                                ));
                            }
                        }
                        evidence_ids.push(id.into());
                    } else {
                        // The declaration receipt independently locates the class inside
                        // this return value; same-named parameters are unrelated bindings.
                        if !matches!(
                            value_form.as_str(),
                            "class_expression" | "new_class_expression"
                        ) || class_form != "class_expression"
                            || class_start < value_start
                            || class_end > value_end
                            || returned.line < r.line
                            || returned.end_line > r.end_line
                        {
                            return Err(invalid(
                                "Unguarded return is not a direct class expression",
                            ));
                        }
                    }
                } else if !rv["bindingGuardFactId"].is_null() {
                    return Err(invalid(
                        "Unsupported return cannot carry a class binding guard",
                    ));
                }
                if let Some(id) = rv["classFactId"].as_str() {
                    evidence_ids.push(id.into());
                }
            }
            evidence_ids.extend(return_fact_ids.clone());
            evidence_ids.sort();
            evidence_ids.dedup();
            if evidence_ids.iter().any(|id| {
                by_id
                    .get(id.as_str())
                    .is_none_or(|proof| proof.path != f.path)
            }) {
                return Err(invalid("Unknown or cross-file factory proof ID"));
            }
            let result_kind: FactoryResultKind = serde_json::from_value(v["resultKind"].clone())
                .map_err(|_| invalid("Invalid factory result"))?;
            let returned_class_id = v["returnedClassFactId"]
                .as_str()
                .map(|id| {
                    classes
                        .iter()
                        .find(|c| c.declaration_id == id)
                        .map(|c| c.id.clone())
                        .ok_or_else(|| invalid("Unknown returned class"))
                })
                .transpose()?;
            let safe = v["safe"]
                .as_bool()
                .ok_or_else(|| invalid("Invalid factory safety"))?;
            if safe
                && (!matches!(
                    result_kind,
                    FactoryResultKind::Class | FactoryResultKind::Instance
                ) || returned_class_id.is_none()
                    || return_fact_ids.len() != 1)
            {
                return Err(invalid("Invalid unique factory result"));
            }
            if safe {
                let exports: Vec<_> = evidence_ids
                    .iter()
                    .filter_map(|id| by_id.get(id.as_str()).copied())
                    .filter(|proof| proof.kind == "js_module_export")
                    .collect();
                if exports.len() != 1 {
                    return Err(invalid("Unique factory result requires one export proof"));
                }
                let export: Value = serde_json::from_str(&exports[0].value)
                    .map_err(|_| invalid("Invalid factory export proof"))?;
                if export["ownerFactId"] != module.id
                    || export["targetFactId"] != callable_id
                    || export["targetKind"] != "function"
                    || export["exported"] != v["exportedName"]
                    || (v["exportKind"] == "commonjs_default"
                        && (export["mode"] != "commonjs"
                            || export["initializedBeforeExport"] != true))
                    || (v["exportKind"] == "esm_named" && export["mode"] != "esm")
                {
                    return Err(invalid("Factory export proof mismatch"));
                }
                let r = by_id[return_fact_ids[0].as_str()];
                let rv: Value = serde_json::from_str(&r.value)
                    .map_err(|_| invalid("Invalid unique factory return"))?;
                if rv["classFactId"] != v["returnedClassFactId"]
                    || rv["valueKind"] != v["resultKind"]
                {
                    return Err(invalid("Conflicting unique factory result"));
                }
                let returned = by_id
                    .get(
                        v["returnedClassFactId"]
                            .as_str()
                            .ok_or_else(|| invalid("Missing returned class"))?,
                    )
                    .ok_or_else(|| invalid("Missing returned class"))?;
                if returned.path != callable.path
                    || returned.line < callable.line
                    || returned.end_line > callable.end_line
                {
                    return Err(invalid("Returned class is not factory-owned"));
                }
                let ri = index
                    .facts
                    .iter()
                    .position(|f| f.id == returned.id)
                    .ok_or_else(|| invalid("Missing returned declaration"))?;
                if index.owners.get(&ri) != Some(&ci) {
                    return Err(invalid("Returned class declaration owner mismatch"));
                }
            }
            factories.push(FactoryFact {
                id: f.id.clone(),
                callable_id,
                module_path: f.path.clone(),
                parameters,
                result_kind,
                returned_class_id,
                return_fact_ids,
                evidence_ids,
                safe,
                reason: v["reason"].as_str().map(str::to_owned),
            });
        }
    }
    let mut view = FactsView {
        schema_version: "ideomancer.facts.v1".into(),
        identity: FactsIdentity {
            source_root_id: hash(store.catalog.source_root.as_bytes()),
            snapshot_id: catalog.snapshot_id().into(),
            extractor_version: evidence.extractor_version.clone(),
            inventory_hash: digest(&evidence.files)?,
            facts_hash: String::new(),
        },
        complete: evidence.complete,
        coverage: FactCoverage {
            class_relationship_sites: (!evidence.is_javascript()
                || crate::javascript::supports_class_relations(&evidence.extractor_version))
                && !evidence
                    .diagnostics
                    .iter()
                    .any(|d| d.code == "javascript_class_relations_omitted"),
            exported_factory_structure: factory_capability
                && !evidence
                    .diagnostics
                    .iter()
                    .any(|d| d.code == "javascript_structural_facts_omitted"),
            unsupported_shapes: factories.iter().filter(|f| !f.safe).count(),
        },
        diagnostics: evidence
            .diagnostics
            .iter()
            .take(100)
            .map(|d| FactDiagnostic {
                code: d.code.chars().take(128).collect(),
                path: d.path.clone(),
                message: d.message.chars().take(512).collect(),
            })
            .collect(),
        files: evidence
            .files
            .iter()
            .map(|f| FactFile {
                path: f.path.clone(),
                sha256: f.sha256.clone(),
            })
            .collect(),
        classes,
        factories,
        relationship_sites: sites,
        sources,
    };
    if evidence.is_javascript() && !view.coverage.exported_factory_structure {
        view.diagnostics.truncate(99);
        view.diagnostics.push(FactDiagnostic {
            code: "exported_factory_structure_unavailable".into(),
            path: String::new(),
            message: "Saved extraction lacks complete generic factory facts; rediscover to obtain this capability".into(),
        });
    }
    let receipt_hash = receipt(&store)?;
    view.identity.facts_hash = digest(&(&view, &receipt_hash))?;
    if serde_json::to_vec(&view)
        .map_err(|e| invalid(&e.to_string()))?
        .len()
        > 8 * LIMIT
    {
        return Err(invalid("Facts metadata exceeds 8 MiB"));
    }
    Ok(FactsRef {
        catalog: catalog.clone(),
        view,
        receipt_hash,
    })
}
fn fresh(facts: &FactsRef) -> Result<(), ExtensionError> {
    let store = discovery::load(facts.catalog.directory())?;
    if receipt(&store)? != facts.receipt_hash {
        return Err(invalid("Facts handle is stale: saved evidence changed"));
    }
    Ok(())
}
pub(crate) fn check_store(
    claims: &ValidatedClaimsRef,
    store: &discovery::Store,
) -> Result<(), String> {
    if receipt(store).map_err(|e| e.message)? != claims.facts.receipt_hash {
        return Err("Validated claims are stale".into());
    }
    Ok(())
}
fn text(s: &str) -> bool {
    !s.trim().is_empty() && s.len() <= 512 && !s.chars().any(char::is_control)
}
fn sha(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub fn validate_claims(
    facts: &FactsRef,
    sets: &[ClaimSet],
) -> Result<ValidatedClaimsRef, ExtensionError> {
    if sets
        .iter()
        .flat_map(|s| &s.claims)
        .any(|c| c.basis == ClaimBasis::Hypothesis)
    {
        return Err(invalid("Hypotheses are not authoritative claims"));
    }
    if sets.len() > 4
        || sets.iter().map(|s| s.claims.len()).sum::<usize>() > 2000
        || serde_json::to_vec(sets)
            .map_err(|e| invalid(&e.to_string()))?
            .len()
            > LIMIT
    {
        return Err(invalid("Claim resource limit exceeded"));
    }
    fresh(facts)?;
    if !facts.view.complete || !facts.view.coverage.class_relationship_sites {
        return Err(invalid("Complete relationship facts required; rediscover"));
    }
    let mut resolutions = BTreeMap::new();
    let mut origins = Vec::new();
    for set in sets {
        if set.schema_version != "ideomancer.claims.v1"
            || set.facts != facts.view.identity
            || ![
                &set.adapter.id,
                &set.adapter.version,
                &set.adapter.implementation,
                &set.contract.id,
                &set.contract.version,
            ]
            .iter()
            .all(|s| text(s))
            || !sha(&set.contract.spec_hash)
            || !sha(&set.configuration_hash)
            || set.diagnostics.len() > 100
            || set
                .diagnostics
                .iter()
                .any(|d| !text(&d.code) || d.path.len() > 4096 || !text(&d.message))
        {
            return Err(invalid("Invalid claim set identity or metadata"));
        }
        let mut claim_ids = BTreeSet::new();
        for c in &set.claims {
            if !text(&c.id)
                || !text(&c.rule_id)
                || !claim_ids.insert(&c.id)
                || c.assumptions.len() > 32
                || c.assumptions.iter().any(|s| !text(s))
            {
                return Err(invalid("Invalid claim ID, rule or assumptions"));
            }
            let site = facts
                .view
                .relationship_sites
                .iter()
                .find(|s| s.id == c.site_id)
                .ok_or_else(|| invalid("Unknown claim site"))?;
            let target = facts
                .view
                .classes
                .iter()
                .find(|t| t.id == c.target_class_id)
                .ok_or_else(|| invalid("Unknown target class"))?;
            if c.basis == ClaimBasis::SourceSyntax
                && site.target_class_id.as_ref() != Some(&c.target_class_id)
            {
                return Err(invalid(
                    "Source syntax claims require an independently resolved syntax target",
                ));
            }
            if c.source_class_id == c.target_class_id
                || site.source_class_id != c.source_class_id
                || !facts.view.classes.iter().any(|s| s.id == c.source_class_id)
                || (c.kind == ClaimKind::Extends) != (site.kind == RelationshipSiteKind::Extends)
                || site
                    .target_class_id
                    .as_ref()
                    .is_some_and(|t| t != &c.target_class_id)
            {
                return Err(invalid(
                    "Incompatible claim endpoint, site kind or syntax target",
                ));
            }
            if c.evidence_ids
                .iter()
                .any(|id| !facts.view.sources.contains_key(id))
                || !c.evidence_ids.contains(&target.declaration_id)
                || !site
                    .evidence_ids
                    .iter()
                    .all(|id| c.evidence_ids.contains(id))
            {
                return Err(invalid(
                    "Claim must cite every site proof and target declaration",
                ));
            }
            if !facts.view.coverage.exported_factory_structure
                && c.evidence_ids.iter().any(|id| {
                    facts.view.sources.get(id).is_some_and(|source| {
                        matches!(source.kind.as_str(), "js_factory" | "js_factory_return")
                    })
                })
            {
                return Err(invalid(
                    "Claim requires unavailable exported factory structure",
                ));
            }
            if resolutions
                .insert(&c.site_id, &c.target_class_id)
                .is_some_and(|old| old != &c.target_class_id)
            {
                return Err(invalid("Conflicting claim resolutions"));
            }
        }
        origins.push(AugmentationOrigin {
            adapter: set.adapter.clone(),
            contract: set.contract.clone(),
            configuration_hash: set.configuration_hash.clone(),
            claim_set_hash: digest(set)?,
        });
    }
    Ok(ValidatedClaimsRef {
        facts: facts.clone(),
        sets: sets.to_vec(),
        augmentation: AugmentationReport {
            schema_version: "ideomancer.augmentation.v1".into(),
            composition_policy: "class-claims-1".into(),
            facts: facts.view.identity.clone(),
            origins,
            claim_count: sets.iter().map(|s| s.claims.len()).sum(),
        },
    })
}
pub fn class_diagrams_augmented(
    catalog: &CatalogRef,
    selection: &ClassSelection,
    claims: &ValidatedClaimsRef,
    render: &RenderMode,
) -> Result<AugmentedClassDiagramReport, ExtensionError> {
    if catalog.directory() != claims.facts.catalog.directory()
        || catalog.source_root() != claims.facts.catalog.source_root()
        || catalog.snapshot_id() != claims.facts.catalog.snapshot_id()
    {
        return Err(invalid("Claims belong to a different catalog"));
    }
    let report =
        crate::engine::v1::class_diagrams_internal(catalog, selection, render, Some(claims))
            .map_err(|e| invalid(&e.message))?;
    Ok(AugmentedClassDiagramReport {
        schema_version: "ideomancer.augmented-class-diagram.v1".into(),
        report,
        augmentation: claims.augmentation.clone(),
    })
}

pub(crate) fn compose(claims: &ValidatedClaimsRef, relations: &mut [Value]) -> Result<(), String> {
    for r in relations {
        let Some(site) = claims.facts.view.relationship_sites.iter().find(|s| {
            s.source_class_id == r["classId"]
                && r["siteFactId"] == s.id
                && s.kind
                    == match r["relationship"].as_str() {
                        Some("extends") => RelationshipSiteKind::Extends,
                        Some("creates") => RelationshipSiteKind::Construction,
                        _ => RelationshipSiteKind::Dependency,
                    }
        }) else {
            continue;
        };
        let matching: Vec<_> = claims
            .sets
            .iter()
            .enumerate()
            .flat_map(|(i, set)| {
                set.claims
                    .iter()
                    .filter(|c| c.site_id == site.id)
                    .map(move |c| (i, c))
            })
            .collect();
        if let Some((_, first)) = matching.first() {
            if r["targetClassId"]
                .as_str()
                .is_some_and(|target| target != first.target_class_id)
                || (first.basis == ClaimBasis::SourceSyntax
                    && r["targetClassId"].as_str() != Some(first.target_class_id.as_str()))
            {
                return Err("Claim composition cannot replace an established syntax target".into());
            }
            r["sourceResolution"] = r["resolution"].clone();
            r["sourceTargetClassId"] = r["targetClassId"].clone();
            r["targetClassId"] = json!(first.target_class_id);
            r["claimOrigins"] = json!(matching
                .iter()
                .map(|(i, c)| json!({"setIndex":i,"claim":c}))
                .collect::<Vec<_>>());
            r["augmentationBases"] =
                json!(matching.iter().map(|(_, c)| &c.basis).collect::<Vec<_>>());
        }
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SavedClaims {
    schema_version: String,
    sets: Vec<ClaimSet>,
    sets_hash: String,
}
pub fn save_claims(claims: &ValidatedClaimsRef, directory: &Path) -> Result<(), ExtensionError> {
    fresh(&claims.facts)?;
    if directory.starts_with(claims.facts.catalog.source_root())
        || directory.starts_with(claims.facts.catalog.directory())
    {
        return Err(invalid(
            "Claims storage must be separate from source and catalog evidence",
        ));
    }
    let parent = directory
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let base = crate::analyze::create_storage(parent)?;
    let name = directory
        .file_name()
        .ok_or_else(|| invalid("Claims directory requires a name"))?;
    let destination = base.join(name);
    if destination.starts_with(claims.facts.catalog.source_root())
        || destination.starts_with(claims.facts.catalog.directory())
    {
        return Err(invalid(
            "Claims storage must be separate from source and catalog evidence",
        ));
    }
    let previous = workflow::tree_snapshot(&destination)?;
    if previous.is_some() {
        read_saved(&destination)?;
    }
    let saved = SavedClaims {
        schema_version: "ideomancer.saved-claims.v1".into(),
        sets: claims.sets.clone(),
        sets_hash: digest(&claims.sets)?,
    };
    let stage = tempfile::Builder::new()
        .prefix(".claims-init-")
        .tempdir_in(&base)
        .map_err(|e| invalid(&e.to_string()))?;
    let bytes = serde_json::to_vec_pretty(&saved).map_err(|e| invalid(&e.to_string()))?;
    if bytes.len() > 2 * LIMIT {
        return Err(invalid("Saved claims exceed resource limit"));
    }
    std::fs::write(stage.path().join("claims.json"), bytes).map_err(|e| invalid(&e.to_string()))?;
    read_saved(stage.path())?;
    fresh(&claims.facts)?;
    let staged = stage.path().to_path_buf();
    workflow::publish_current(stage, &staged, &destination, previous.as_ref())?;
    Ok(())
}
fn read_saved(directory: &Path) -> Result<SavedClaims, ExtensionError> {
    let before = workflow::tree_snapshot(directory)?
        .ok_or_else(|| invalid("Claims directory unavailable"))?;
    let entries: Vec<_> = std::fs::read_dir(directory)
        .map_err(|e| invalid(&e.to_string()))?
        .collect::<Result<_, _>>()
        .map_err(|e| invalid(&e.to_string()))?;
    if entries.len() != 1 || entries[0].file_name() != "claims.json" {
        return Err(invalid("Unmanaged claims directory"));
    }
    let path = directory.join("claims.json");
    let metadata = std::fs::symlink_metadata(&path).map_err(|e| invalid(&e.to_string()))?;
    if !metadata.is_file() || metadata.len() > (2 * LIMIT) as u64 {
        return Err(invalid("Unsafe or oversized claims file"));
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(
            (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32,
        );
    }
    let file = options.open(&path).map_err(|e| invalid(&e.to_string()))?;
    if !file
        .metadata()
        .map_err(|e| invalid(&e.to_string()))?
        .is_file()
    {
        return Err(invalid("Claims must be a regular file"));
    }
    let mut bytes = Vec::new();
    file.take((2 * LIMIT + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| invalid(&e.to_string()))?;
    if bytes.len() > 2 * LIMIT {
        return Err(invalid("Saved claims exceed resource limit"));
    }
    let saved: SavedClaims = serde_json::from_slice(&bytes).map_err(|e| invalid(&e.to_string()))?;
    if saved.schema_version != "ideomancer.saved-claims.v1"
        || saved.sets_hash != digest(&saved.sets)?
        || workflow::tree_snapshot(directory)? != Some(before)
    {
        return Err(invalid("Saved claims identity mismatch"));
    }
    Ok(saved)
}
pub fn load_claims(
    facts: &FactsRef,
    directory: &Path,
) -> Result<ValidatedClaimsRef, ExtensionError> {
    let saved = read_saved(directory)?;
    validate_claims(facts, &saved.sets)
}
