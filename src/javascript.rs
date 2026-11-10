//! Native static JS/TS extraction from the scanner's captured source bytes.
//! No target code, runtime, configuration, or compiler plugins are executed.

use crate::analysis::{self, Evidence, LanguageMetadata};
use crate::javascript_native::{self, NativeSource};
use std::path::Path;

pub const EXTRACTOR_VERSION: &str = "javascript-typescript-tree-sitter-1";
pub const STRUCTURAL_EXTRACTOR_VERSION: &str = "javascript-typescript-static-6-generic-facts";
pub const PARSER_VERSION: &str = "tree-sitter-0.25.10;javascript-0.25.0;typescript-0.23.2";
pub const CLASS_MODULE_EXTRACTOR_VERSION: &str = "javascript-typescript-static-5-class-modules";
pub const CLASS_RELATION_EXTRACTOR_VERSION: &str = "javascript-typescript-static-4-class-relations";
pub const OPERATION_EXTRACTOR_VERSION: &str = "javascript-typescript-static-3-operations";
pub const MODULE_EXTRACTOR_VERSION: &str = "javascript-typescript-static-2-modules";
pub const LEGACY_EXTRACTOR_VERSION: &str = "javascript-typescript-static-1";

pub fn supported_version(version: &str) -> bool {
    matches!(
        version,
        EXTRACTOR_VERSION
            | STRUCTURAL_EXTRACTOR_VERSION
            | CLASS_MODULE_EXTRACTOR_VERSION
            | CLASS_RELATION_EXTRACTOR_VERSION
            | OPERATION_EXTRACTOR_VERSION
            | MODULE_EXTRACTOR_VERSION
            | LEGACY_EXTRACTOR_VERSION
    )
}

pub fn supports_modules(version: &str) -> bool {
    matches!(
        version,
        EXTRACTOR_VERSION
            | STRUCTURAL_EXTRACTOR_VERSION
            | CLASS_MODULE_EXTRACTOR_VERSION
            | CLASS_RELATION_EXTRACTOR_VERSION
            | OPERATION_EXTRACTOR_VERSION
            | MODULE_EXTRACTOR_VERSION
    )
}

pub fn supports_class_relations(version: &str) -> bool {
    matches!(
        version,
        EXTRACTOR_VERSION
            | STRUCTURAL_EXTRACTOR_VERSION
            | CLASS_MODULE_EXTRACTOR_VERSION
            | CLASS_RELATION_EXTRACTOR_VERSION
    )
}

pub fn supports_class_module_targets(version: &str) -> bool {
    matches!(
        version,
        EXTRACTOR_VERSION | STRUCTURAL_EXTRACTOR_VERSION | CLASS_MODULE_EXTRACTOR_VERSION
    )
}

/// Structural factory/return receipts were introduced in v6. Legacy evidence
/// remains generically readable, but cannot supply this capability retroactively.
pub fn supports_structural_facts(version: &str) -> bool {
    matches!(version, STRUCTURAL_EXTRACTOR_VERSION | EXTRACTOR_VERSION)
}
// This is a fixed source policy, never loaded from target configuration.
pub const SCAN_POLICY: &str = "javascript-typescript-scope-1-nofollow:src-else-root;js,jsx,ts,tsx;exclude=operator-dirs,dotfolders,node_modules,bower_components,assets,vendor,generated,coverage,init,migrations,config,configs,environments;skip=*.spec.*,*.test.*,*.min.js,*.bundle.js,*.d.ts;modules=root-relative-path-with-extension-dotjoined";

pub fn extract(root: &Path) -> Result<Evidence, String> {
    let scan = analysis::scan_javascript(root)?;
    let sources: Vec<_> = scan
        .sources
        .into_iter()
        .map(|source| NativeSource {
            path: source.path,
            module: source.module,
            bytes: source.bytes,
        })
        .collect();
    let response = javascript_native::extract_sources(&sources)?;
    let language = LanguageMetadata {
        name: "javascript-typescript".into(),
        parser: "tree-sitter".into(),
        parser_version: PARSER_VERSION.into(),
        parser_sha256: javascript_native::fingerprint(),
        runtime: "native".into(),
        runtime_version: env!("CARGO_PKG_VERSION").into(),
        scan_policy: SCAN_POLICY.into(),
    };
    let snapshot = serde_json::to_vec(&(SCAN_POLICY, EXTRACTOR_VERSION, &language, &scan.files))
        .map_err(|e| e.to_string())?;
    let mut diagnostics = scan.diagnostics;
    diagnostics.extend(response.diagnostics);
    diagnostics.sort_by(|a, b| (&a.path, &a.code, &a.message).cmp(&(&b.path, &b.code, &b.message)));
    diagnostics.dedup_by(|a, b| a.path == b.path && a.code == b.code && a.message == b.message);
    let evidence = Evidence {
        schema_version: "ideomancer.evidence.v2".into(),
        snapshot_id: analysis::hash(&snapshot),
        extractor_version: EXTRACTOR_VERSION.into(),
        python_version: String::new(),
        language: Some(language),
        files: scan.files,
        facts: response.facts,
        diagnostics,
        complete: scan.complete && response.complete,
    };
    evidence.validate_language()?;
    Ok(evidence)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::analysis::Language;
    use std::fs;

    #[test]
    fn capabilities_preserve_legacy_generic_evidence() {
        for (version, modules, relations, class_modules, structural) in [
            (LEGACY_EXTRACTOR_VERSION, false, false, false, false),
            (MODULE_EXTRACTOR_VERSION, true, false, false, false),
            (OPERATION_EXTRACTOR_VERSION, true, false, false, false),
            (CLASS_RELATION_EXTRACTOR_VERSION, true, true, false, false),
            (CLASS_MODULE_EXTRACTOR_VERSION, true, true, true, false),
            (STRUCTURAL_EXTRACTOR_VERSION, true, true, true, true),
            (EXTRACTOR_VERSION, true, true, true, true),
        ] {
            assert!(supported_version(version));
            assert_eq!(supports_modules(version), modules);
            assert_eq!(supports_class_relations(version), relations);
            assert_eq!(supports_class_module_targets(version), class_modules);
            assert_eq!(supports_structural_facts(version), structural);
        }
        assert!(!supported_version("javascript-typescript-static-7-unknown"));
        assert!(!supports_structural_facts(
            "javascript-typescript-static-7-unknown"
        ));
    }

    #[test]
    fn capability_omissions_survive_diagnostic_floods() {
        let noise = "({[key]: 0});\n".repeat(5000);
        let factories = (0..8000)
            .map(|i| format!("export function f{i}() {{}}\n"))
            .collect::<String>();
        let methods = (0..12001)
            .map(|i| format!("m{i}() {{}}\n"))
            .collect::<String>();
        for (source, code, omitted_kinds) in [
            (
                factories,
                "javascript_structural_facts_omitted",
                &["js_factory", "js_factory_return"][..],
            ),
            (
                format!("export class C {{\n{methods}}}"),
                "javascript_class_relations_omitted",
                &["js_callable_signature", "js_class_relation"][..],
            ),
        ] {
            assert!(source.len() < 1024 * 1024);
            let root = tempfile::tempdir().unwrap();
            fs::write(root.path().join("noise.js"), &noise).unwrap();
            fs::write(root.path().join("z.ts"), source).unwrap();
            let evidence = extract(root.path()).unwrap();
            assert!(evidence.complete, "{:?}", evidence.diagnostics);
            assert!(evidence.diagnostics.len() <= 5000);
            assert_eq!(
                evidence
                    .diagnostics
                    .iter()
                    .filter(|d| d.code == code && d.path.is_empty())
                    .count(),
                1,
                "{code}"
            );
            assert!(!evidence
                .facts
                .iter()
                .any(|f| omitted_kinds.contains(&f.kind.as_str())));
            assert!(evidence
                .facts
                .iter()
                .any(|f| f.path == "z.ts" && f.kind == "js_module_export"));
        }
    }

    #[test]
    fn factory_return_forms_have_exact_utf16_identity() {
        let root = tempfile::tempdir().unwrap();
        let cases = [
            (
                "shadow",
                "module.exports = (Product) => { return class Product {}; };",
                "class_expression",
                Some("class Product {}"),
            ),
            (
                "new_shadow",
                "module.exports = (Product) => { return new class Product {}(); };",
                "new_class_expression",
                Some("new class Product {}()"),
            ),
            (
                "arrow",
                "module.exports = () => (class Product {});",
                "class_expression",
                Some("class Product {}"),
            ),
            (
                "new_arrow",
                "module.exports = () => (new (class Product {})());",
                "new_class_expression",
                Some("new (class Product {})()"),
            ),
            (
                "declaration",
                "module.exports = () => { class Product {} return Product; };",
                "identifier",
                Some("Product"),
            ),
            (
                "variable",
                "module.exports = () => { const Product = class {}; return Product; };",
                "identifier",
                Some("Product"),
            ),
            (
                "new_identifier",
                "module.exports = () => { class Product {} return new Product(); };",
                "new_identifier",
                Some("new Product()"),
            ),
            (
                "array",
                "module.exports = () => [1, 2];",
                "unsupported",
                Some("[1, 2]"),
            ),
            (
                "empty",
                "module.exports = () => { return; };",
                "unsupported",
                None,
            ),
        ];
        for (name, source, _, _) in cases {
            // An astral character before the declaration distinguishes UTF-16 offsets from bytes.
            fs::write(
                root.path().join(format!("{name}.js")),
                format!("// \u{1f600}\n{source}"),
            )
            .unwrap();
        }
        let evidence = extract(root.path()).unwrap();
        assert!(evidence.complete, "{:?}", evidence.diagnostics);
        for (name, source, form, expected) in cases {
            let path = format!("{name}.js");
            let text: Vec<u16> = format!("// \u{1f600}\n{source}").encode_utf16().collect();
            let rf = evidence
                .facts
                .iter()
                .find(|f| f.path == path && f.kind == "js_factory_return")
                .unwrap();
            let value: serde_json::Value = serde_json::from_str(&rf.value).unwrap();
            assert_eq!(value["valueForm"], form, "{name}");
            if let Some(expected) = expected {
                let start = value["valueStart"].as_u64().unwrap() as usize;
                let end = value["valueEnd"].as_u64().unwrap() as usize;
                assert_eq!(
                    String::from_utf16(&text[start..end]).unwrap(),
                    expected,
                    "{name}"
                );
            } else {
                assert!(value["valueStart"].is_null() && value["valueEnd"].is_null());
            }
            for f in evidence
                .facts
                .iter()
                .filter(|f| f.path == path && f.kind == "js_declaration")
            {
                let meta: serde_json::Value = serde_json::from_str(&f.value).unwrap();
                if meta["kind"] != "class" {
                    continue;
                }
                let start = meta["start"].as_u64().unwrap() as usize;
                let end = meta["end"].as_u64().unwrap() as usize;
                assert!(String::from_utf16(&text[start..end])
                    .unwrap()
                    .starts_with("class"));
                let direct = matches!(form, "class_expression" | "new_class_expression");
                let expression = direct || name == "variable";
                assert_eq!(
                    meta["classForm"],
                    if expression {
                        "class_expression"
                    } else {
                        "class_declaration"
                    }
                );
                if direct {
                    assert!(value["bindingGuardFactId"].is_null());
                    assert!(start >= value["valueStart"].as_u64().unwrap() as usize);
                    assert!(end <= value["valueEnd"].as_u64().unwrap() as usize);
                } else {
                    assert!(end <= value["valueStart"].as_u64().unwrap() as usize);
                    assert!(value["bindingGuardFactId"].is_string());
                }
            }
        }
    }

    #[test]
    fn generic_factory_results_are_grounded_and_literal_free() {
        let root = tempfile::tempdir().unwrap();
        let cases = [
            ("class", "module.exports = (Base, model) => { class C extends Base { constructor() { super(model); } method(x) { return model.call(x); } } return C; };", "class", true),
            ("instance", "module.exports = () => { class C {} return new C(); };", "instance", true),
            ("arrow", "module.exports = () => class {};", "class", true),
            ("arrow_instance", "module.exports = () => new class {};", "instance", true),
            ("multiple", "module.exports = x => { class C {} if (x) return C; return C; };", "ambiguous", false),
            ("conditional", "module.exports = x => { class C {} if (x) return C; };", "ambiguous", false),
            ("dynamic", "module.exports = () => build();", "unsupported", false),
            ("literal", "module.exports = () => 'PRIVATE_RETURN_VALUE';", "unsupported", false),
            ("alias", "module.exports = () => { class C {} const Alias = C; return Alias; };", "unsupported", false),
            ("written", "module.exports = () => { class C {} C = other; return C; };", "unsupported", false),
            ("early", "module.exports = () => { return new C(); class C {} };", "ambiguous", false),
            ("named", "export function build(Base, value = 'PRIVATE_DEFAULT_VALUE', ...rest) { class C extends Base {} return C; }", "class", true),
            ("reference", "const build = () => class {}; module.exports = build;", "class", true),
        ];
        for (name, text, _, _) in cases {
            fs::write(root.path().join(format!("{name}.ts")), text).unwrap();
        }
        fs::write(root.path().join("privacy.js"), "module.exports = (router, resolve, Model) => { router.use(resolve('PRIVATE_PROVIDER_VALUE')); router.get('/ok', () => Model.scope('PRIVATE_QUERY_VALUE').findAll({where: 'PRIVATE_WHERE_VALUE'})); router.getAsync('/PRIVATE_ASYNC_VALUE', () => {}); };").unwrap();
        let evidence = extract(root.path()).unwrap();
        assert!(evidence.complete, "{:?}", evidence.diagnostics);
        for (name, _, kind, safe) in cases {
            let receipts: Vec<_> = evidence
                .facts
                .iter()
                .filter(|f| f.path == format!("{name}.ts") && f.kind == "js_factory")
                .collect();
            assert_eq!(receipts.len(), 1, "{name}");
            let value: serde_json::Value = serde_json::from_str(&receipts[0].value).unwrap();
            assert_eq!(value["resultKind"], kind, "{name}");
            assert_eq!(value["safe"], safe, "{name}");
            for key in ["supportingFactIds", "returnFactIds"] {
                for id in value[key].as_array().unwrap() {
                    assert!(evidence
                        .facts
                        .iter()
                        .any(|f| Some(f.id.as_str()) == id.as_str()));
                }
            }
            for p in value["parameters"].as_array().unwrap() {
                for key in ["parameterFactId", "bindingGuardFactId"] {
                    if let Some(id) = p[key].as_str() {
                        assert!(evidence.facts.iter().any(|f| f.id == id));
                    }
                }
            }
        }
        let encoded = serde_json::to_string(&evidence).unwrap();
        for private in [
            "PRIVATE_RETURN_VALUE",
            "PRIVATE_DEFAULT_VALUE",
            "PRIVATE_PROVIDER_VALUE",
            "PRIVATE_QUERY_VALUE",
            "PRIVATE_WHERE_VALUE",
            "PRIVATE_ASYNC_VALUE",
        ] {
            assert!(!encoded.contains(private), "{private}");
        }
        let routes: Vec<_> = evidence
            .facts
            .iter()
            .filter(|f| f.kind == "api_route")
            .collect();
        assert_eq!(routes.len(), 1);
        let route: serde_json::Value = serde_json::from_str(&routes[0].value).unwrap();
        assert_eq!(route["methods"], serde_json::json!(["GET"]));
    }

    #[test]
    fn language_markers_fail_closed_without_running_a_runtime() {
        let valid = Evidence {
            schema_version: "ideomancer.evidence.v2".into(),
            extractor_version: EXTRACTOR_VERSION.into(),
            language: Some(LanguageMetadata {
                name: "javascript-typescript".into(),
                parser: "tree-sitter".into(),
                parser_version: PARSER_VERSION.into(),
                parser_sha256: "a".repeat(64),
                runtime: "native".into(),
                runtime_version: "0.1.0".into(),
                scan_policy: SCAN_POLICY.into(),
            }),
            ..Evidence::default()
        };
        assert!(valid.is_javascript());
        let encoded = serde_json::to_value(&valid).unwrap();
        for (key, value) in [
            ("name", "python"),
            ("parser", "unknown"),
            ("runtime", "python"),
            ("parserVersion", ""),
            ("runtimeVersion", ""),
            ("parserSha256", "not-a-hash"),
            ("scanPolicy", "unknown"),
        ] {
            let mut negative = encoded.clone();
            negative["language"][key] = value.into();
            let evidence: Evidence = serde_json::from_value(negative).unwrap();
            assert!(evidence.validate_language().is_err(), "{key}");
            assert!(!evidence.is_javascript());
        }
        for (key, value) in [
            ("schemaVersion", "unknown"),
            ("extractorVersion", "python-static-7-call-targets"),
            ("pythonVersion", "3.12"),
        ] {
            let mut negative = encoded.clone();
            negative[key] = value.into();
            assert!(serde_json::from_value::<Evidence>(negative)
                .unwrap()
                .validate_language()
                .is_err());
        }
        let mut python = Evidence {
            schema_version: "ideomancer.evidence.v1".into(),
            python_version: "3.12".into(),
            ..valid
        };
        assert!(python.validate_language().is_err());
        python.language = None;
        assert!(python.validate_language().is_err());
    }

    #[test]
    fn native_identity_and_language_metadata_are_deterministic() {
        let root = tempfile::tempdir().unwrap();
        let marker = root.path().join("executed");
        fs::write(
            root.path().join("app.ts"),
            format!(
                "require('fs').writeFileSync({:?}, 'executed'); export class App {{}}",
                marker.to_str().unwrap()
            ),
        )
        .unwrap();
        fs::write(
            root.path().join("tsconfig.json"),
            "INVALID CONFIG DO NOT READ",
        )
        .unwrap();
        fs::write(
            root.path().join("package.json"),
            "INVALID PACKAGE DO NOT READ",
        )
        .unwrap();
        fs::create_dir(root.path().join("config")).unwrap();
        fs::write(
            root.path().join("config/plugin.js"),
            format!(
                "require('fs').writeFileSync({:?}, 'plugin')",
                marker.to_str().unwrap()
            ),
        )
        .unwrap();
        let first = extract(root.path()).unwrap();
        let second = extract(root.path()).unwrap();
        assert!(first.complete, "{:?}", first.diagnostics);
        assert_eq!(first.extractor_version, EXTRACTOR_VERSION);
        assert_eq!(first.snapshot_id, second.snapshot_id);
        assert_eq!(
            serde_json::to_string(&first).unwrap(),
            serde_json::to_string(&second).unwrap()
        );
        assert_eq!(
            first.validate_language(),
            Ok(Language::JavaScriptTypeScript)
        );
        assert_eq!(first.files.len(), 1);
        assert!(first.python_version.is_empty());
        assert!(first.is_javascript());
        assert!(!marker.exists());
        let serialized = serde_json::to_string(&first).unwrap();
        assert!(!serialized.contains("pythonVersion"));
        assert_eq!(
            first.language.as_ref().unwrap().parser_sha256,
            javascript_native::fingerprint()
        );
        let language = first.language.as_ref().unwrap();
        assert_eq!(language.parser, "tree-sitter");
        assert_eq!(language.parser_version, PARSER_VERSION);
        assert_eq!(language.runtime, "native");
        assert_eq!(language.runtime_version, env!("CARGO_PKG_VERSION"));
        let snapshot = |version: &str, metadata: &LanguageMetadata| {
            analysis::hash(
                &serde_json::to_vec(&(SCAN_POLICY, version, metadata, &first.files)).unwrap(),
            )
        };
        assert_eq!(first.snapshot_id, snapshot(EXTRACTOR_VERSION, language));
        assert_ne!(
            first.snapshot_id,
            snapshot(STRUCTURAL_EXTRACTOR_VERSION, language)
        );
        let mut changed_metadata = language.clone();
        changed_metadata.parser_sha256 = "c".repeat(64);
        assert_ne!(
            first.snapshot_id,
            snapshot(EXTRACTOR_VERSION, &changed_metadata)
        );
        changed_metadata = language.clone();
        changed_metadata.parser_version.push_str(";changed-grammar");
        assert_ne!(
            first.snapshot_id,
            snapshot(EXTRACTOR_VERSION, &changed_metadata)
        );
        fs::write(root.path().join("app.ts"), "export class Changed {}").unwrap();
        assert_ne!(first.snapshot_id, extract(root.path()).unwrap().snapshot_id);
        let mut invalid = first;
        invalid.python_version = "24.11.1".into();
        assert!(invalid.validate_language().is_err());
        assert!(!invalid.is_javascript());
        invalid.python_version.clear();
        invalid.language.as_mut().unwrap().name = "python".into();
        assert!(invalid.validate_language().is_err());
    }

    #[test]
    fn jsx_and_tsx_use_native_grammars_without_configuration() {
        let root = tempfile::tempdir().unwrap();
        for (path, text) in [
            ("view.jsx", "export class View { render() { return <section><span /></section>; } }"),
            ("typed.tsx", "export class Typed { render(value: string): JSX.Element { return <section>{value}</section>; } }"),
        ] {
            fs::write(root.path().join(path), text).unwrap();
        }
        let evidence = extract(root.path()).unwrap();
        assert!(evidence.complete, "{:?}", evidence.diagnostics);
        assert_eq!(evidence.files.len(), 2);
        for path in ["view.jsx", "typed.tsx"] {
            assert!(
                evidence
                    .facts
                    .iter()
                    .any(|f| f.path == path && f.kind == "js_declaration"),
                "{path}"
            );
            assert!(
                evidence
                    .facts
                    .iter()
                    .any(|f| f.path == path && f.kind == "js_module_export"),
                "{path}"
            );
        }
    }

    #[test]
    fn native_parser_uses_captured_bytes_after_path_replacement() {
        let root = tempfile::tempdir().unwrap();
        let original = b"export class Captured {}";
        fs::write(root.path().join("app.js"), original).unwrap();
        let scan = analysis::scan_javascript(root.path()).unwrap();
        fs::write(root.path().join("app.js"), "INVALID REPLACEMENT SOURCE").unwrap();
        let sources: Vec<_> = scan
            .sources
            .into_iter()
            .map(|source| NativeSource {
                path: source.path,
                module: source.module,
                bytes: source.bytes,
            })
            .collect();
        assert_eq!(scan.files[0].sha256, analysis::hash(&sources[0].bytes));
        assert_eq!(sources[0].bytes, original);
        let result = javascript_native::extract_sources(&sources).unwrap();
        assert!(result.complete, "{:?}", result.diagnostics);
        assert!(result
            .facts
            .iter()
            .any(|f| f.kind == "js_declaration" && f.symbol.ends_with("Captured")));
    }

    #[test]
    fn invalid_utf8_and_parse_errors_fail_closed_without_source_leaks() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("bad.js"), b"// PRIVATE_INVALID_UTF8\n\xff").unwrap();
        fs::write(
            root.path().join("broken.ts"),
            "export class Broken { PRIVATE_PARSE_TEXT = ;",
        )
        .unwrap();
        fs::write(root.path().join("good.js"), "export class Good {}").unwrap();
        let evidence = extract(root.path()).unwrap();
        assert!(!evidence.complete);
        for path in ["bad.js", "broken.ts"] {
            assert!(
                evidence.diagnostics.iter().any(|d| d.path == path),
                "{path}"
            );
        }
        assert!(evidence
            .facts
            .iter()
            .filter(|f| f.path == "bad.js")
            .all(|f| f.kind == "module"));
        assert!(evidence
            .facts
            .iter()
            .any(|f| f.path == "good.js" && f.kind == "js_declaration"));
        let encoded = serde_json::to_string(&evidence).unwrap();
        assert!(!encoded.contains("PRIVATE_INVALID_UTF8"));
        assert!(!encoded.contains("PRIVATE_PARSE_TEXT"));
    }

    #[test]
    fn native_extraction_preserves_scanner_safety_and_resource_limits() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        fs::write(
            external.path().join("secret.js"),
            "export class PrivateOutside {}",
        )
        .unwrap();
        symlink(
            external.path().join("secret.js"),
            root.path().join("linked.js"),
        )
        .unwrap();
        let evidence = extract(root.path()).unwrap();
        assert!(!evidence.complete);
        assert!(evidence.files.is_empty());
        assert!(evidence.facts.is_empty());
        assert!(evidence
            .diagnostics
            .iter()
            .any(|d| d.code == "symlink_skipped"));
        fs::write(
            root.path().join("oversized.js"),
            vec![b' '; 1024 * 1024 + 1],
        )
        .unwrap();
        assert!(extract(root.path())
            .unwrap_err()
            .contains("1 MiB file limit"));
    }
}
