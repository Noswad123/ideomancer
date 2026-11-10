//! Static JS/TS extraction using an explicitly selected, operator-trusted compiler.
//! The child is isolated from target loaders/configuration, not OS-sandboxed.

use crate::analysis::{self, Diagnostic, Evidence, Fact, LanguageMetadata};
use serde::Deserialize;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const EXTRACTOR_VERSION: &str = "javascript-typescript-static-6-generic-facts";
pub const CLASS_MODULE_EXTRACTOR_VERSION: &str = "javascript-typescript-static-5-class-modules";
pub const CLASS_RELATION_EXTRACTOR_VERSION: &str = "javascript-typescript-static-4-class-relations";
pub const OPERATION_EXTRACTOR_VERSION: &str = "javascript-typescript-static-3-operations";
pub const MODULE_EXTRACTOR_VERSION: &str = "javascript-typescript-static-2-modules";
pub const LEGACY_EXTRACTOR_VERSION: &str = "javascript-typescript-static-1";

pub fn supported_version(version: &str) -> bool {
    matches!(
        version,
        EXTRACTOR_VERSION
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
            | CLASS_MODULE_EXTRACTOR_VERSION
            | CLASS_RELATION_EXTRACTOR_VERSION
            | OPERATION_EXTRACTOR_VERSION
            | MODULE_EXTRACTOR_VERSION
    )
}

pub fn supports_class_relations(version: &str) -> bool {
    matches!(
        version,
        EXTRACTOR_VERSION | CLASS_MODULE_EXTRACTOR_VERSION | CLASS_RELATION_EXTRACTOR_VERSION
    )
}

pub fn supports_class_module_targets(version: &str) -> bool {
    matches!(version, EXTRACTOR_VERSION | CLASS_MODULE_EXTRACTOR_VERSION)
}

/// Structural factory/return receipts were introduced in v6. Legacy evidence
/// remains generically readable, but cannot supply this capability retroactively.
pub fn supports_structural_facts(version: &str) -> bool {
    version == EXTRACTOR_VERSION
}
// This is a fixed source policy, never loaded from target configuration.
pub const SCAN_POLICY: &str = "javascript-typescript-scope-1-nofollow:src-else-root;js,jsx,ts,tsx;exclude=operator-dirs,dotfolders,node_modules,bower_components,assets,vendor,generated,coverage,init,migrations,config,configs,environments;skip=*.spec.*,*.test.*,*.min.js,*.bundle.js,*.d.ts;modules=root-relative-path-with-extension-dotjoined";
const HELPER: &str = include_str!("javascript_extract.cjs");
const MAX_COMPILER: u64 = 32 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Response {
    protocol_version: String,
    node_version: String,
    typescript_version: String,
    typescript_sha256: String,
    facts: Vec<Fact>,
    diagnostics: Vec<Diagnostic>,
    complete: bool,
}

fn trust_boundary(source_root: &Path) -> Result<PathBuf, String> {
    for ancestor in source_root.ancestors() {
        // Git worktree files and even dangling marker symlinks count. Never
        // follow the marker or read repository metadata/configuration.
        match fs::symlink_metadata(ancestor.join(".git")) {
            Ok(_) => return Ok(ancestor.into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("Inspect target repository boundary: {error}")),
        }
    }
    Ok(source_root.into())
}

fn outside(path: &Path, root: &Path, label: &str) -> Result<PathBuf, String> {
    let resolved = fs::canonicalize(path)
        .map_err(|e| format!("Cannot resolve {label} {}: {e}", path.display()))?;
    if resolved.starts_with(root) {
        return Err(format!(
            "{label} must be outside the target repository/source root (including symlink destinations)"
        ));
    }
    if !fs::metadata(&resolved)
        .map_err(|e| e.to_string())?
        .is_file()
    {
        return Err(format!("{label} must be a regular file"));
    }
    Ok(resolved)
}

fn runtime(node: &Path, root: &Path) -> Result<PathBuf, String> {
    let candidate = if node.components().count() == 1 && node.is_relative() {
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
            .map(|dir| dir.join(node))
            .find(|p| {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    fs::metadata(p)
                        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
                }
                #[cfg(not(unix))]
                {
                    p.is_file()
                }
            })
            .ok_or("Cannot find Node on the operator PATH; select an installed Node executable")?
    } else {
        node.into()
    };
    // Resolve before changing cwd. A trusted global shim is allowed, but no
    // executable (or symlink destination) within the target is ever launched.
    outside(&candidate, root, "Node executable")
}

#[cfg(not(unix))]
pub fn extract(_root: &Path, _node: &Path, _typescript: &Path) -> Result<Evidence, String> {
    Err(
        "JavaScript extraction requires Unix descriptor-relative no-follow filesystem support"
            .into(),
    )
}

#[cfg(unix)]
pub fn extract(root: &Path, node: &Path, typescript: &Path) -> Result<Evidence, String> {
    if !typescript.is_absolute() {
        return Err("Select a trusted compiler with --typescript ABSFILE (typescript/lib/typescript.js); target resolution, installation, and fallback are disabled".into());
    }
    let scan = analysis::scan_javascript(root)?;
    let canonical_root =
        fs::canonicalize(&scan.root).map_err(|e| format!("Resolve target root: {e}"))?;
    let boundary = trust_boundary(&canonical_root)?;
    let compiler = outside(
        typescript,
        &boundary,
        "TypeScript compiler (--typescript ABSFILE)",
    )?;
    let executable = runtime(node, &boundary)?;
    let mut compiler_bytes = Vec::new();
    let anchor = fs::File::open("/").map_err(|e| e.to_string())?;
    let compiler_file = analysis::open_anchored(
        &anchor,
        compiler.strip_prefix("/").map_err(|e| e.to_string())?,
        false,
    )
    .map_err(|e| format!("Open trusted TypeScript compiler without following symlinks: {e}"))?;
    let metadata = compiler_file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > MAX_COMPILER {
        return Err(
            "Trusted TypeScript compiler must be a regular file no larger than 32 MiB".into(),
        );
    }
    compiler_file
        .take(MAX_COMPILER + 1)
        .read_to_end(&mut compiler_bytes)
        .map_err(|e| format!("Read trusted TypeScript compiler: {e}"))?;
    if compiler_bytes.len() as u64 > MAX_COMPILER {
        return Err("Trusted TypeScript compiler exceeds the 32 MiB limit".into());
    }
    let compiler_hash = analysis::hash(&compiler_bytes);
    let neutral = tempfile::tempdir().map_err(|e| format!("Create neutral Node cwd: {e}"))?;
    if fs::canonicalize(neutral.path())
        .map_err(|e| e.to_string())?
        .starts_with(&boundary)
    {
        return Err(
            "Neutral Node cwd must be outside the target repository/source root; select an external TMPDIR".into(),
        );
    }
    let mut command = Command::new(executable);
    command
        .args(["--no-addons", "--max-old-space-size=512", "--eval", HELPER])
        .arg(&compiler)
        .arg(&compiler_hash)
        .current_dir(neutral.path())
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for key in ["PATH", "SYSTEMROOT", "WINDIR"] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    let input = serde_json::to_vec(&serde_json::json!({ "sources": scan.sources }))
        .map_err(|e| e.to_string())?;
    let output = analysis::run_bounded(command, input, "JavaScript/TypeScript")?;
    let response: Response = serde_json::from_slice(&output)
        .map_err(|_| "Invalid JavaScript/TypeScript helper response (body withheld)".to_string())?;
    if response.protocol_version != "ideomancer.js-extractor.v1"
        || response.typescript_sha256 != compiler_hash
        || response.node_version.is_empty()
        || response.typescript_version.is_empty()
    {
        return Err(
            "JavaScript/TypeScript helper protocol or trusted compiler fingerprint mismatch".into(),
        );
    }
    let language = LanguageMetadata {
        name: "javascript-typescript".into(),
        parser: "typescript".into(),
        parser_version: response.typescript_version,
        parser_sha256: compiler_hash,
        runtime: "node".into(),
        runtime_version: response.node_version,
        scan_policy: SCAN_POLICY.into(),
    };
    let snapshot = serde_json::to_vec(&(
        SCAN_POLICY,
        EXTRACTOR_VERSION,
        analysis::hash(HELPER.as_bytes()),
        &language,
        &scan.files,
    ))
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

    #[test]
    fn capabilities_preserve_legacy_generic_evidence() {
        for (version, modules, relations, class_modules, structural) in [
            (LEGACY_EXTRACTOR_VERSION, false, false, false, false),
            (MODULE_EXTRACTOR_VERSION, true, false, false, false),
            (OPERATION_EXTRACTOR_VERSION, true, false, false, false),
            (CLASS_RELATION_EXTRACTOR_VERSION, true, true, false, false),
            (CLASS_MODULE_EXTRACTOR_VERSION, true, true, true, false),
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
        let Some((node, compiler)) = tools() else {
            return;
        };
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
            let evidence = extract(root.path(), &node, &compiler).unwrap();
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
        let Some((node, compiler)) = tools() else {
            return;
        };
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
        let evidence = extract(root.path(), &node, &compiler).unwrap();
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
        let Some((node, compiler)) = tools() else {
            return;
        };
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
        let evidence = extract(root.path(), &node, &compiler).unwrap();
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
                parser: "typescript".into(),
                parser_version: "5.8.3".into(),
                parser_sha256: "a".repeat(64),
                runtime: "node".into(),
                runtime_version: "v24.11.1".into(),
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

    fn tools() -> Option<(PathBuf, PathBuf)> {
        let Some(compiler) = std::env::var_os("IDEOMANCER_TEST_TYPESCRIPT") else {
            eprintln!("SKIP: set IDEOMANCER_TEST_TYPESCRIPT to a trusted compiler ABSFILE");
            return None;
        };
        let compiler = PathBuf::from(compiler);
        assert!(
            compiler.is_absolute() && compiler.is_file(),
            "IDEOMANCER_TEST_TYPESCRIPT must name an existing absolute compiler file"
        );
        let node = std::env::var_os("IDEOMANCER_TEST_NODE")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("node"));
        Some((node, compiler))
    }

    #[test]
    fn compiler_swap_between_rust_hash_and_helper_load_never_executes() {
        use std::os::unix::fs::PermissionsExt;
        let Some((node, _)) = tools() else {
            return;
        };
        let source = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        let node = runtime(&node, &fs::canonicalize(source.path()).unwrap()).unwrap();
        let compiler = external.path().join("compiler.js");
        let marker = external.path().join("executed");
        let original = b"module.exports = {};";
        fs::write(&compiler, original).unwrap();
        let replacement = format!("require('node:fs').writeFileSync({:?}, 'COMPILER_EXECUTED'); throw new Error('changed compiler executed');", marker.to_str().unwrap());
        fs::write(external.path().join("compiler.js.swap"), &replacement).unwrap();
        // This operator-trusted shim runs only after Rust has read the compiler.
        // Keep every Node argument intact while replacing the file before load.
        let shim = external.path().join("node-shim");
        let quoted_node = format!("'{}'", node.to_str().unwrap().replace('\'', "'\\''"));
        fs::write(&shim, format!("#!/bin/sh\ncp \"$5.swap\" \"$5\" || exit 90\nprintf '%s' \"$6\" >\"$5.expected\"\nexec {quoted_node} \"$@\"\n")).unwrap();
        fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).unwrap();
        let error = extract(source.path(), &shim, &compiler).unwrap_err();
        assert!(error.contains("fingerprint mismatch"), "{error}");
        assert_eq!(
            fs::read_to_string(external.path().join("compiler.js.expected")).unwrap(),
            analysis::hash(original)
        );
        assert_eq!(fs::read_to_string(&compiler).unwrap(), replacement);
        assert!(!marker.exists());
    }

    #[test]
    fn missing_compiler_is_actionable_and_target_tools_are_rejected() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let error = extract(root.path(), Path::new("node"), Path::new("")).unwrap_err();
        assert!(error.contains("--typescript ABSFILE"));
        let parser = root.path().join("compiler.js");
        fs::write(&parser, "throw new Error('must not execute')").unwrap();
        assert!(extract(root.path(), Path::new("node"), &parser)
            .unwrap_err()
            .contains("outside the target"));
        let external = tempfile::tempdir().unwrap();
        let link = external.path().join("compiler.js");
        symlink(&parser, &link).unwrap();
        assert!(extract(root.path(), Path::new("node"), &link)
            .unwrap_err()
            .contains("outside the target"));
        assert!(runtime(&parser, &fs::canonicalize(root.path()).unwrap()).is_err());
    }

    #[test]
    fn monorepo_sibling_compiler_and_node_are_rejected_before_execution() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let repo = tempfile::tempdir().unwrap();
        fs::create_dir(repo.path().join(".git")).unwrap();
        let source = repo.path().join("apps/server");
        fs::create_dir_all(&source).unwrap();
        let client = repo.path().join("client/node_modules/typescript/lib");
        fs::create_dir_all(&client).unwrap();
        let marker = repo.path().join("executed");
        let compiler = client.join("typescript.js");
        fs::write(
            &compiler,
            format!(
                "require('fs').writeFileSync({:?}, 'executed')",
                marker.to_str().unwrap()
            ),
        )
        .unwrap();
        let node = repo.path().join("client/node");
        fs::write(&node, format!("#!/bin/sh\ntouch '{}'\n", marker.display())).unwrap();
        fs::set_permissions(&node, fs::Permissions::from_mode(0o755)).unwrap();
        let external = tempfile::tempdir().unwrap();
        let external_compiler = external.path().join("compiler.js");
        fs::write(&external_compiler, "throw new Error('must not execute')").unwrap();
        let compiler_link = external.path().join("compiler-link.js");
        let node_link = external.path().join("node-link");
        symlink(&compiler, &compiler_link).unwrap();
        symlink(&node, &node_link).unwrap();
        for parser in [&compiler, &compiler_link] {
            let error = extract(&source, Path::new("node"), parser).unwrap_err();
            assert!(error.contains("TypeScript compiler"));
            assert!(error.contains("outside the target repository/source root"));
        }
        for executable in [&node, &node_link] {
            let error = extract(&source, executable, &external_compiler).unwrap_err();
            assert!(error.contains("Node executable"));
            assert!(error.contains("outside the target repository/source root"));
        }
        assert!(!marker.exists());
    }

    #[test]
    fn repository_boundary_uses_nearest_marker_without_following_it() {
        use std::os::unix::fs::symlink;
        let repo = tempfile::tempdir().unwrap();
        let repo = fs::canonicalize(repo.path()).unwrap();
        fs::create_dir(repo.join(".git")).unwrap();
        let nested = repo.join("nested");
        let source = nested.join("apps/server");
        fs::create_dir_all(&source).unwrap();
        assert_eq!(trust_boundary(&source).unwrap(), repo);
        fs::write(nested.join(".git"), "gitdir: DO_NOT_READ").unwrap();
        assert_eq!(trust_boundary(&source).unwrap(), nested);
        fs::remove_file(nested.join(".git")).unwrap();
        symlink(
            repo.join("nonexistent-private-metadata"),
            nested.join(".git"),
        )
        .unwrap();
        assert_eq!(trust_boundary(&source).unwrap(), nested);
    }

    #[test]
    fn standalone_source_allows_explicit_external_tools() {
        let source = tempfile::tempdir().unwrap();
        let source = fs::canonicalize(source.path()).unwrap();
        assert_eq!(trust_boundary(&source).unwrap(), source);
        let external = tempfile::tempdir().unwrap();
        let compiler = external.path().join("compiler.js");
        let node = external.path().join("node");
        fs::write(&compiler, "trusted fixture").unwrap();
        fs::write(&node, "trusted fixture").unwrap();
        assert_eq!(
            outside(&compiler, &source, "TypeScript compiler").unwrap(),
            fs::canonicalize(&compiler).unwrap()
        );
        assert_eq!(
            runtime(&node, &source).unwrap(),
            fs::canonicalize(&node).unwrap()
        );
    }

    #[test]
    fn node_environment_loaders_never_execute() {
        let Some((node, compiler)) = tools() else {
            return;
        };
        if let Some(root) = std::env::var_os("IDEOMANCER_NODE_ENV_TEST_ROOT") {
            let root = PathBuf::from(root);
            let evidence = extract(&root, &node, &compiler).unwrap();
            assert!(evidence.complete);
            assert!(!root.join("executed").exists());
            return;
        }
        // A subprocess avoids mutating the environment of parallel tests.
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("app.ts"), "export class App {}").unwrap();
        let loader = root.path().join("loader.cjs");
        fs::write(
            &loader,
            format!(
                "require('fs').writeFileSync({:?}, 'LOADER_EXECUTED')",
                root.path().join("executed").to_str().unwrap()
            ),
        )
        .unwrap();
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "javascript::tests::node_environment_loaders_never_execute",
                "--nocapture",
            ])
            .env("IDEOMANCER_NODE_ENV_TEST_ROOT", root.path())
            .env("NODE_OPTIONS", format!("--require={}", loader.display()))
            .env("NODE_PATH", root.path())
            .env("HOME", root.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!root.path().join("executed").exists());
    }

    #[test]
    fn static_identity_and_language_metadata_are_deterministic() {
        let Some((node, compiler)) = tools() else {
            return;
        };
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
        let first = extract(root.path(), &node, &compiler).unwrap();
        let second = extract(root.path(), &node, &compiler).unwrap();
        assert_eq!(first.snapshot_id, second.snapshot_id);
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
            analysis::hash(&fs::read(&compiler).unwrap())
        );
        fs::write(root.path().join("app.ts"), "export class Changed {}").unwrap();
        assert_ne!(
            first.snapshot_id,
            extract(root.path(), &node, &compiler).unwrap().snapshot_id
        );
        let mut invalid = first;
        invalid.python_version = "24.11.1".into();
        assert!(invalid.validate_language().is_err());
        assert!(!invalid.is_javascript());
        invalid.python_version.clear();
        invalid.language.as_mut().unwrap().name = "python".into();
        assert!(invalid.validate_language().is_err());
    }
}
