//! Framework-neutral router context DTOs and the compiled adapter entry point.
use serde::Serialize;
use serde_json::Value;

pub use crate::adapters::fastapi::derive;

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    pub status: String,
    pub candidates: Vec<Candidate>,
    pub evidence_ids: Vec<String>,
    pub source_refs: Vec<Value>,
    pub basis: String,
    pub warnings: Vec<String>,
    pub unknown_status: String,
    pub converter_projection: ConverterEligibility,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConverterEligibility {
    pub status: String,
    pub framework: String,
    pub profile: String,
    pub reason: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub prefix: String,
    pub composed_path: String,
    pub full_path_proven: bool,
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::analysis::{Evidence, Fact};
    use crate::catalog::{self, Item, Kind};
    use serde_json::json;
    use std::path::Path;

    fn value(f: &Fact) -> Value {
        serde_json::from_str(&f.value).unwrap_or(Value::Null)
    }

    pub(crate) const DIRECT: &str = "from fastapi import APIRouter\nrouter = APIRouter(prefix='/caps')\nraise RuntimeError('TARGET_MUST_NOT_EXECUTE')\n@router.get('/items')\ndef handler():\n    return 'PRIVATE_PAYLOAD'\n";

    pub(crate) fn extract(source: &str) -> Option<(Evidence, catalog::Catalog, Item)> {
        extract_files(&[("app.py", source)])
    }

    #[test]
    fn saved_generic_facts_keep_direct_context_without_legacy_recipe_interpretation() {
        let source = "@decorate(kind='PRIVATE_METADATA')\nclass SyntheticContext:\n    def configure(self):\n        @self.router.get('/items')\n        def handler(): pass\n";
        let Some((mut evidence, _, item)) = extract(source) else {
            return;
        };
        evidence.extractor_version = "python-static-6-router-context".into();
        for kind in [
            "router_registration",
            "router_class_wiring",
            "router_registry_rule",
            "router_factory_rule",
        ] {
            evidence.facts.push(Fact {
                id: format!("legacy_{kind}"),
                kind: kind.into(),
                path: "app.py".into(),
                symbol: "app.SyntheticContext".into(),
                line: 1,
                end_line: 6,
                value: r#"{"supported":true}"#.into(),
            });
        }
        let saved = serde_json::to_string(&evidence).unwrap();
        assert!(!saved.contains("PRIVATE_METADATA"));
        let loaded: Evidence = serde_json::from_str(&saved).unwrap();
        let context = derive(&loaded, &item);
        assert_eq!(context.status, "unknown");
        assert!(context.candidates.is_empty());
        assert!(context
            .evidence_ids
            .iter()
            .all(|id| !id.starts_with("legacy_")));
        let catalog = catalog::build(&loaded, "synthetic").unwrap();
        let rendered = crate::render::endpoint::build(
            &catalog,
            &loaded,
            &item,
            &crate::render::endpoint::View::Services,
            None,
            0,
            200,
        )
        .unwrap();
        assert_eq!(rendered.legend["routerContext"]["status"], "unknown");

        let Some((mut evidence, _, item)) = extract(DIRECT) else {
            return;
        };
        for version in [
            "python-static-6-router-context",
            "python-static-7-call-targets",
            crate::analysis::EXTRACTOR_VERSION,
        ] {
            evidence.extractor_version = version.into();
            let loaded: Evidence =
                serde_json::from_str(&serde_json::to_string(&evidence).unwrap()).unwrap();
            let context = derive(&loaded, &item);
            assert_eq!(context.status, "supported");
            assert_eq!(context.candidates[0].composed_path, "/caps/items");
            assert_eq!(
                context.converter_projection.status,
                if version == "python-static-6-router-context" {
                    "unknown"
                } else {
                    "eligible"
                }
            );
        }
    }

    #[test]
    fn converter_guard_complex_calls_reexports_and_class_aliases_keep_budgeted_proof() {
        let direct = "from fastapi import APIRouter\nrouter = APIRouter(prefix='/caps')\n@router.get('/{name:path}')\ndef handler(): pass\n";
        let row = format!("[{}]", vec!["0"; 16].join(","));
        let matrix = format!("[{}]", vec![row; 16].join(","));
        let cases = [
            vec![("app.py", format!("{direct}\nfrom starlette.convertors import register_url_convertor as reg\nreg('path', Custom({matrix}))\n"))],
            vec![("registration.py", "from starlette.convertors import register_url_convertor as install\n".into()), ("app.py", format!("{direct}\nfrom registration import install\ninstall('path', Custom())\n"))],
            vec![("registration.py", "from starlette.convertors import register_url_convertor as install\ndef install(*args): pass\n".into()), ("app.py", format!("{direct}\nfrom registration import install\ninstall('path', Custom())\n"))],
            vec![("app.py", format!("{direct}\nfrom starlette.convertors import register_url_convertor as reg\nclass Registration:\n    install = reg\nRegistration.install('path', Custom())\n"))],
            vec![("pkg/__init__.py", "".into()), ("pkg/registration.py", "from starlette.convertors import register_url_convertor as reg\nclass Registration:\n    install = reg\n".into()), ("pkg/reexport.py", "from .registration import Registration as Exported\n".into()), ("app.py", format!("{direct}\nimport pkg.reexport as exposed\nexposed.Exported.install('path', Custom())\n"))],
        ];
        for files in cases {
            let borrowed: Vec<_> = files.iter().map(|(p, s)| (*p, s.as_str())).collect();
            let Some((mut e, catalog, item)) = extract_files(&borrowed) else {
                return;
            };
            let context = derive(&e, &item);
            assert_eq!(context.converter_projection.status, "blocked", "{files:?}");
            for f in e.facts.iter().filter(|f| {
                matches!(
                    f.kind.as_str(),
                    "import" | "from_import" | "class" | "binding"
                )
            }) {
                // The router constructor binding is represented by router_prefix;
                // all registration-chain facts must be retained independently.
                if f.kind == "binding" && value(f)["target"] == "router" {
                    continue;
                }
                assert!(
                    context.evidence_ids.contains(&f.id),
                    "missing {} {}: {files:?}",
                    f.kind,
                    f.symbol
                );
                assert!(context.source_refs.iter().any(|r| r["factId"] == f.id));
            }
            let contract = tempfile::NamedTempFile::new().unwrap();
            std::fs::write(contract.path(), br#"{"openapi":"3.1.0","paths":{"/caps/{name}":{"get":{}},"/caps/{name:path}":{"get":{}}}}"#).unwrap();
            let doc = crate::openapi::load(&contract.path().canonicalize().unwrap()).unwrap();
            let mut projected_only = doc.clone();
            projected_only
                .operations
                .retain(|op| !op.path.contains(":path"));
            assert_eq!(
                crate::openapi::match_item_with_context(&projected_only, &item, false, &context)
                    ["status"],
                "context-mismatch"
            );
            let full = crate::render::endpoint::build(
                &catalog,
                &e,
                &item,
                &crate::render::endpoint::View::Services,
                Some(&doc),
                0,
                200,
            )
            .unwrap();
            assert_eq!(full.legend["contractMatch"]["status"], "candidate");
            assert_eq!(
                full.legend["contractMatch"]["candidates"][0]["path"],
                "/caps/{name:path}"
            );
            for id in &context.evidence_ids {
                assert!(full.legend["sourceRefs"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|r| r["factId"] == *id));
            }
            let limited = crate::render::endpoint::build(
                &catalog,
                &e,
                &item,
                &crate::render::endpoint::View::Services,
                Some(&doc),
                0,
                item.evidence_ids.len() + 1,
            )
            .unwrap();
            assert_eq!(limited.legend["routerContext"]["status"], "budget-unknown");
            assert_eq!(limited.legend["contractMatch"]["status"], "unknown");
            // Inflate a guard-chain import, not a route/focus anchor. Its full
            // bytes must be charged before projection or literal narrowing.
            let import = e
                .facts
                .iter_mut()
                .find(|f| f.kind == "from_import" && value(f)["module"] == "starlette.convertors")
                .unwrap();
            let mut metadata = value(import);
            metadata["padding"] = json!("x".repeat(crate::packet::MAX_INPUT_BYTES));
            import.value = metadata.to_string();
            let limited = crate::render::endpoint::build(
                &catalog,
                &e,
                &item,
                &crate::render::endpoint::View::Services,
                Some(&doc),
                0,
                200,
            )
            .unwrap();
            assert_eq!(limited.legend["routerContext"]["status"], "budget-unknown");
            assert_eq!(limited.legend["contractMatch"]["status"], "unknown");
            assert!(limited.legend["routerContext"]["sourceRefs"]
                .as_array()
                .unwrap()
                .is_empty());
        }
    }

    #[test]
    fn converter_alias_expansion_limit_fails_closed_with_citations() {
        let mut source = "from fastapi import APIRouter\nfrom starlette.convertors import register_url_convertor as alias0\nrouter = APIRouter(prefix='/caps')\n@router.get('/{name:path}')\ndef handler(): pass\n".to_owned();
        for n in 1..80 {
            source.push_str(&format!("alias{n} = alias{}\n", n - 1));
        }
        let Some((e, _, item)) = extract(&source) else {
            return;
        };
        let context = derive(&e, &item);
        assert_eq!(context.converter_projection.status, "blocked");
        let binding = e
            .facts
            .iter()
            .find(|f| f.kind == "binding" && value(f)["target"] == "alias1")
            .unwrap();
        assert!(context.evidence_ids.contains(&binding.id));
        assert!(context
            .source_refs
            .iter()
            .any(|r| r["factId"] == binding.id));
    }

    #[test]
    fn legacy_call_facts_require_rediscovery_only_for_projection() {
        let Some((mut e, _, item)) = extract(&DIRECT.replace("'/items'", "'/{name:path}'")) else {
            return;
        };
        e.extractor_version = "python-static-6-router-context".into();
        let context = derive(&e, &item);
        assert_eq!(context.status, "supported");
        assert_eq!(context.converter_projection.status, "unknown");
        assert!(context
            .converter_projection
            .reason
            .contains("Rediscovery required"));
        let contract = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(contract.path(), br#"{"openapi":"3.1.0","paths":{"/caps/{name}":{"get":{}},"/caps/{name:path}":{"get":{}}}}"#).unwrap();
        let doc = crate::openapi::load(&contract.path().canonicalize().unwrap()).unwrap();
        let matched = crate::openapi::match_item_with_context(&doc, &item, false, &context);
        assert_eq!(matched["status"], "candidate");
        assert_eq!(matched["candidates"][0]["path"], "/caps/{name:path}");
    }

    #[test]
    fn converter_eligibility_and_registration_alias_proofs_are_source_backed() {
        let direct = "from fastapi import APIRouter\nrouter = APIRouter(prefix='/caps')\n@router.get('/{name:path}')\ndef handler(): pass\n";
        for source in [
            direct.to_owned(),
            DIRECT.replace("'/items'", "'/{name:path}'"),
        ] {
            let Some((e, _, item)) = extract(&source) else {
                return;
            };
            let context = derive(&e, &item);
            assert_eq!(context.converter_projection.status, "eligible");
            assert!(context
                .source_refs
                .iter()
                .any(|r| r["kind"] == "from_import"));
            assert_eq!(context.candidates[0].composed_path, "/caps/{name:path}");
        }
        for registration in [
            "from starlette.convertors import register_url_convertor as reg\nreg('path', Custom())\n",
            "import starlette as st\nst.convertors.register_url_convertor('int', Custom())\n",
            "import starlette.convertors as cv\nreg = cv.register_url_convertor\nagain = reg\nagain('uuid', Custom())\n",
            "from starlette.convertors import register_url_convertor as reg\ndef unknown_scope(reg):\n    reg(dynamic, Custom())\n",
            "reg = unknown.register_url_convertor\nreg('str', Custom())\n",
        ] {
            let Some((e, _, item)) = extract_files(&[("app.py", direct), ("registration.py", registration)]) else { return; };
            let context = derive(&e, &item);
            assert_eq!(context.status, "supported");
            assert_eq!(context.converter_projection.status, "blocked", "{registration}");
            for f in e.facts.iter().filter(|f| f.path == "registration.py" && matches!(f.kind.as_str(), "import" | "from_import" | "binding")) {
                assert!(context.evidence_ids.contains(&f.id), "missing {}: {registration}", f.kind);
                assert!(context.source_refs.iter().any(|r| r["factId"] == f.id));
            }
            assert!(context.source_refs.iter().any(|r| r["path"] == "registration.py" && r["kind"] == "call"));
        }
        let Some((e, _, item)) = extract(&format!("{direct}\nordinary_call()\n")) else {
            return;
        };
        assert_eq!(derive(&e, &item).converter_projection.status, "eligible");
    }

    fn extract_files(files: &[(&str, &str)]) -> Option<(Evidence, catalog::Catalog, Item)> {
        let python = ["python3.12", "python3.13", "python3"]
            .into_iter()
            .find(|p| {
                std::process::Command::new(p)
                    .args([
                        "-I",
                        "-S",
                        "-c",
                        "import sys;sys.exit(sys.version_info < (3,12))",
                    ])
                    .status()
                    .is_ok_and(|s| s.success())
            });
        let Some(python) = python else {
            eprintln!("SKIP: router context extraction requires Python >=3.12");
            return None;
        };
        let root = tempfile::tempdir().unwrap();
        for (path, source) in files {
            let path = root.path().join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, source).unwrap();
        }
        let e = crate::analysis::extract(root.path(), Path::new(python)).unwrap();
        assert!(e.complete);
        let c = catalog::build(&e, "synthetic").unwrap();
        let i = c
            .items
            .iter()
            .find(|i| i.kind == Kind::Api)
            .unwrap()
            .clone();
        Some((e, c, i))
    }

    #[test]
    fn p2_direct_receiver_rejects_intervening_imports_and_declarations() {
        let source = "from fastapi import APIRouter\nrouter = APIRouter(prefix='/caps')\ntails = APIRouter(prefix='/tails')\ndef configure():\n    from app import tails as router\n    @router.get('/items')\n    def handler(): pass\n";
        for source in [
            source.to_owned(),
            source.replace("from app import tails as router", "import app as router"),
            source.replace("from app import tails as router", "def router(): pass"),
            source.replace(
                "from app import tails as router",
                "async def router(): pass",
            ),
            source.replace("from app import tails as router", "class router: pass"),
            source.replace(
                "    @router.get('/items')\n    def handler(): pass",
                "    def build():\n        @router.get('/items')\n        def handler(): pass",
            ),
        ] {
            let Some((e, _, item)) = extract(&source) else {
                return;
            };
            assert_p2_context_cannot_select_caps(&e, &item);
        }
    }

    #[test]
    fn ordinary_alias_escapes_invalidate_direct_router_identity() {
        for addition in [
            "alias = router\n",
            "alias = router\nalias.prefix = dynamic\n",
        ] {
            let Some((e, _, item)) = extract(&format!("{DIRECT}{addition}")) else {
                return;
            };
            assert_p2_context_cannot_select_caps(&e, &item);
        }
    }

    fn assert_p2_context_cannot_select_caps(e: &Evidence, item: &Item) {
        let context = derive(e, item);
        assert_ne!(context.status, "supported");
        let contract = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(
            contract.path(),
            br#"{"openapi":"3.1.0","paths":{"/caps/items":{"get":{}},"/tails/items":{"get":{}}}}"#,
        )
        .unwrap();
        let document = crate::openapi::load(&contract.path().canonicalize().unwrap()).unwrap();
        let association = crate::openapi::match_item_with_context(&document, item, false, &context);
        assert_eq!(association["status"], "ambiguous");
        assert_eq!(association["candidates"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn direct_imported_prefix_include_and_mutations() {
        let source = "from fastapi import APIRouter as Router, FastAPI\napp = FastAPI()\nrouter = Router(prefix='/caps')\napp.include_router(router, prefix='/v1')\n@router.get('')\ndef handler(): pass\n";
        // Constructor aliases are resolved by identity, not by their spelling.
        let Some((e, _, item)) = extract(source) else {
            return;
        };
        let c = derive(&e, &item);
        assert_eq!(c.status, "supported");
        assert_eq!(c.candidates[0].composed_path, "/v1/caps");
        for addition in [
            "router.prefix = dynamic\n",
            "router = other\n",
            "router.prefix += dynamic\n",
            "del router.prefix\n",
        ] {
            let Some((e, _, item)) = extract(&format!("{source}{addition}")) else {
                return;
            };
            assert_ne!(derive(&e, &item).status, "supported");
        }
        for source in [
            source.replace("prefix='/caps'", "prefix=dynamic"),
            source.replace("prefix='/caps'", "prefix='/caps', **options"),
            source.replace("prefix='/v1'", "prefix=dynamic"),
            source.replace("from fastapi", "from impostor"),
            format!("{source}\nRouter = other\n"),
        ] {
            let Some((e, _, item)) = extract(&source) else {
                return;
            };
            assert_ne!(derive(&e, &item).status, "supported");
        }
    }
}
