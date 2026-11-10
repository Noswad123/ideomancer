//! Public CLI contracts for offline, operator-trusted JavaScript/TypeScript views.
#![cfg(unix)]

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;
use std::time::SystemTime;
use tempfile::{tempdir, TempDir};

const SECRET: &str = "PRIVATE_JS_LITERAL_MUST_NOT_LEAVE_SOURCE";
const SOURCE: &str = r#"
import { imported } from './unavailable';
import axios from 'axios';
function localHelper() { return 'PRIVATE_JS_LITERAL_MUST_NOT_LEAVE_SOURCE'; }
export function $run(injected) {
  localHelper(); imported(); this.$execute(); Service.prototype.$execute(); injected.lookup();
  axios.get('PRIVATE_JS_LITERAL_MUST_NOT_LEAVE_SOURCE'); fetch('PRIVATE_JS_LITERAL_MUST_NOT_LEAVE_SOURCE');
}
export async function $async() { return localHelper(); }
export class Service extends ExternalBase {
  constructor(private dependency: Unknown) {}
  private hidden() { localHelper(); }
  protected guarded() { localHelper(); }
  #secret() { localHelper(); }
  public $execute() { localHelper(); this.hidden(); }
  signature(): void;
}
interface Shape { noBody(): void; }
declare function declarationOnly(): void;
function overloadOnly(value: string): string;
function overloadOnly(value: number): number;
"#;
const ROUTES: &str = r#"
module.exports = (router, deps) => {
  router.post('/lookup', req => req.contextService.lookup(req.body));
};
"#;

struct Tools {
    node: PathBuf,
    compiler: PathBuf,
}

fn tools() -> Option<&'static Tools> {
    static TOOLS: OnceLock<Option<Tools>> = OnceLock::new();
    let tools = TOOLS.get_or_init(|| {
        let compiler = PathBuf::from(std::env::var_os("IDEOMANCER_TEST_TYPESCRIPT")?);
        if !compiler.is_absolute() || !compiler.is_file() {
            return None;
        }
        let executable = std::env::var_os("IDEOMANCER_TEST_NODE").unwrap_or_else(|| "node".into());
        let output = Command::new(executable)
            .args(["--eval", "process.stdout.write(process.execPath)"])
            .env_remove("NODE_OPTIONS")
            .env_remove("NODE_PATH")
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let node = PathBuf::from(String::from_utf8(output.stdout).ok()?);
        if !node.is_absolute() || !node.is_file() {
            return None;
        }
        Some(Tools {
            node,
            compiler: fs::canonicalize(compiler).ok()?,
        })
    });
    if tools.is_none() {
        eprintln!("SKIP javascript_views: set IDEOMANCER_TEST_TYPESCRIPT to an absolute trusted typescript/lib/typescript.js outside fixtures and IDEOMANCER_TEST_NODE to trusted Node (default: node)");
    }
    tools.as_ref()
}

fn success(output: Output) -> Value {
    assert!(output.status.success(), "{output:?}");
    serde_json::from_slice(&output.stdout).expect("one public CLI JSON report")
}

fn incomplete(output: Output) -> Value {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let report: Value =
        serde_json::from_slice(&output.stdout).expect("saved incomplete catalog report");
    assert_eq!(report["complete"], false);
    assert_eq!(report["page"]["complete"], false);
    report
}

fn rejected(output: Output, term: &str) {
    assert!(!output.status.success(), "unexpected success: {output:?}");
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(text.to_lowercase().contains(&term.to_lowercase()), "{text}");
    for unexpected in ["FORBIDDEN_TOOL_EXECUTED", "failed to run", SECRET] {
        assert!(
            !text.contains(unexpected),
            "not a safe local rejection: {text}"
        );
    }
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn artifacts(root: &Path) -> BTreeMap<PathBuf, (Vec<u8>, SystemTime)> {
    let mut result = BTreeMap::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            for (name, value) in artifacts(&path) {
                result.insert(path.strip_prefix(root).unwrap().join(name), value);
            }
        } else {
            result.insert(
                path.strip_prefix(root).unwrap().to_owned(),
                (
                    fs::read(&path).unwrap(),
                    fs::metadata(&path).unwrap().modified().unwrap(),
                ),
            );
        }
    }
    result
}

fn item<'a>(saved: &'a Value, kind: &str, suffix: &str) -> &'a Value {
    let items: Vec<_> = saved["page"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["kind"] == kind && i["symbol"].as_str().unwrap().ends_with(suffix))
        .collect();
    assert_eq!(items.len(), 1, "{kind} {suffix}: {saved}");
    items[0]
}

fn catalog(saved: &Value) -> &Path {
    Path::new(saved["catalogDirectory"].as_str().unwrap())
}

fn artifact(report: &Value, field: &str) -> String {
    let path = Path::new(report[field].as_str().unwrap());
    assert!(path.is_absolute() && path.starts_with(report["viewDirectory"].as_str().unwrap()));
    fs::read_to_string(path).unwrap()
}

fn legend(report: &Value) -> Value {
    serde_json::from_str(&artifact(report, "legend")).unwrap()
}

struct Fixture {
    _temp: TempDir,
    cwd: PathBuf,
    root: PathBuf,
    tools: &'static Tools,
    forbidden: PathBuf,
}

impl Fixture {
    fn new() -> Option<Self> {
        let tools = tools()?;
        let temp = tempdir().unwrap();
        let cwd = fs::canonicalize(temp.path()).unwrap();
        let root = cwd.join("source app");
        fs::create_dir(&root).unwrap();
        assert!(!tools.node.starts_with(&root) && !tools.compiler.starts_with(&root));
        let forbidden = cwd.join("forbidden tool");
        fs::write(&forbidden, "#!/bin/sh\n: > \"$JS_TOOL_MARKER\"\nprintf 'FORBIDDEN_TOOL_EXECUTED\\n' >&2\nexit 97\n").unwrap();
        fs::set_permissions(&forbidden, fs::Permissions::from_mode(0o755)).unwrap();
        let f = Self {
            _temp: temp,
            cwd,
            root,
            tools,
            forbidden,
        };
        let source = format!(
            "require('fs').writeFileSync({}, 'executed');\n{SOURCE}",
            json!(f.cwd.join("source.executed"))
        );
        fs::write(f.root.join("app.ts"), source).unwrap();
        fs::write(f.root.join("routes.js"), ROUTES).unwrap();
        Some(f)
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_ideomancer"));
        command
            .current_dir(&self.cwd)
            .env("IDEOMANCER_NODE", &self.forbidden)
            .env(
                "IDEOMANCER_TYPESCRIPT",
                self.cwd.join("missing compiler.js"),
            )
            .env("IDEOMANCER_PYTHON", &self.forbidden)
            .env("IDEOMANCER_AGENT", &self.forbidden)
            .env("IDEOMANCER_MMDC", &self.forbidden)
            .env("JS_TOOL_MARKER", self.cwd.join("tool.executed"))
            .env_remove("IDEOMANCER_MODEL")
            .env_remove("NODE_OPTIONS")
            .env_remove("NODE_PATH");
        command
    }

    fn discover(&self) -> Command {
        let mut command = self.command();
        command
            .arg("discover")
            .arg(&self.root)
            .args(["--lang", "javascript-typescript", "--json", "--limit", "30"])
            .arg("--out")
            .arg(self.cwd.join("catalog output"))
            .arg("--node")
            .arg(&self.tools.node)
            .arg("--typescript")
            .arg(&self.tools.compiler);
        command
    }

    fn saved(&self) -> Value {
        let saved = success(self.discover().output().unwrap());
        self.safe(&saved);
        saved
    }

    fn diagram(&self, saved: &Value, selected: &Value, view: &str) -> Command {
        let mut command = self.command();
        command.arg("diagram").arg(catalog(saved)).args([
            "--entry",
            selected["id"].as_str().unwrap(),
            "--view",
            view,
            "--no-render",
            "--focus-facts",
            "200",
        ]);
        command
    }

    fn safe(&self, saved: &Value) {
        for name in [
            "source.executed",
            "tool.executed",
            "loader.executed",
            "plugin.executed",
            "module.executed",
            ".ideomancer/analyses",
        ] {
            assert!(!self.cwd.join(name).exists(), "unexpected {name}");
        }
        for (path, (bytes, _)) in artifacts(catalog(saved)) {
            assert!(
                !String::from_utf8_lossy(&bytes).contains(SECRET),
                "literal leaked: {path:?}"
            );
            assert!(
                !path.components().any(|p| p.as_os_str() == "attempts"),
                "agent attempt: {path:?}"
            );
        }
    }
}

fn check_callable(report: &Value, selected: &Value, saved: &Value) -> Value {
    assert_eq!(report["kind"], "workflow");
    assert_eq!(report["snapshotId"], saved["snapshotId"]);
    assert!(report["svg"].is_null() && report["error"].is_null());
    let data = legend(report);
    assert_eq!(data["schemaVersion"], "ideomancer.callable-view.v1");
    assert_eq!(data["sourceModel"], "static-candidate");
    assert_eq!(data["item"], *selected);
    assert_eq!(data["contractMatch"]["status"], "not-applicable");
    assert_eq!(data["counts"]["networkGroups"], 0);
    assert!(!data["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|n| n["id"] == "O"));
    let refs: BTreeSet<_> = data["sourceRefs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["factId"].as_str().unwrap())
        .collect();
    for id in selected["evidenceIds"].as_array().unwrap() {
        assert!(
            refs.contains(id.as_str().unwrap()),
            "missing selected citation: {data}"
        );
    }
    assert!(artifact(report, "mermaid").starts_with("flowchart"));
    data
}

#[test]
fn discovery_is_stable_redacted_complete_and_explicitly_language_tagged() {
    let Some(f) = Fixture::new() else { return };
    let saved = f.saved();
    assert_eq!(saved["complete"], true);
    assert_eq!(
        saved["page"]["counts"],
        json!({"api": 1, "class": 1, "workflow": 6})
    );
    assert_eq!(saved["page"]["totalItems"], 8);
    let evidence = read_json(&catalog(&saved).join("evidence.json"));
    assert_eq!(evidence["schemaVersion"], "ideomancer.evidence.v2");
    assert_eq!(
        evidence["extractorVersion"],
        "javascript-typescript-static-6-generic-facts"
    );
    for fact in evidence["facts"].as_array().unwrap() {
        assert!(
            !matches!(
                fact["kind"].as_str(),
                Some("js_service_factory" | "js_service_request" | "js_model_query")
            ),
            "retired interpretation receipt: {fact}"
        );
    }
    assert_eq!(evidence["language"]["name"], "javascript-typescript");
    assert_eq!(evidence["language"]["parser"], "typescript");
    assert_eq!(evidence["language"]["runtime"], "node");
    assert_eq!(
        evidence["language"]["parserSha256"],
        format!("{:x}", Sha256::digest(fs::read(&f.tools.compiler).unwrap()))
    );
    for key in ["parserVersion", "runtimeVersion", "scanPolicy"] {
        assert!(!evidence["language"][key].as_str().unwrap().is_empty());
    }
    assert!(evidence.get("pythonVersion").is_none());
    assert_eq!(evidence["files"].as_array().unwrap().len(), 2);
    let diagnostics = evidence["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0]["code"], "javascript_unsupported_execution_boundary",
        "the declared external heritage is an explicit unsupported boundary"
    );
    let symbols: Vec<_> = saved["page"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["symbol"].as_str().unwrap())
        .collect();
    for excluded in [
        "hidden",
        "guarded",
        "#secret",
        "constructor",
        "signature",
        "noBody",
        "declarationOnly",
        "overloadOnly",
    ] {
        assert!(
            !symbols.iter().any(|s| s.ends_with(&format!(".{excluded}"))),
            "{symbols:?}"
        );
    }
    item(&saved, "workflow", ".$run");
    item(&saved, "workflow", ".Service.$execute");
    let before = artifacts(catalog(&saved));
    assert_eq!(f.saved(), saved);
    assert_eq!(artifacts(catalog(&saved)), before);
    let browse = success(
        f.command()
            .arg("catalog")
            .arg(catalog(&saved))
            .args(["--json", "--limit", "30"])
            .output()
            .unwrap(),
    );
    assert_eq!(browse, saved);
    f.safe(&saved);
}

#[test]
fn saved_callable_views_survive_deleted_source_and_all_extractors_disabled() {
    let Some(f) = Fixture::new() else { return };
    let saved = f.saved();
    fs::remove_dir_all(&f.root).unwrap();
    for suffix in [".$run", ".$async", ".Service.$execute"] {
        let selected = item(&saved, "workflow", suffix);
        for view in ["services", "network"] {
            let report = success(f.diagram(&saved, selected, view).output().unwrap());
            let data = check_callable(&report, selected, &saved);
            assert!(
                data["focus"]["visitedSymbols"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|s| s.as_str().unwrap().ends_with(".localHelper")),
                "{data}"
            );
            assert_eq!(
                data["focus"]["links"].as_array().unwrap().len(),
                1,
                "{data}"
            );
            assert_eq!(data["focus"]["links"][0]["kind"], "js_local_call_candidate");
            assert!(data["focus"]["visitedSymbols"]
                .as_array()
                .unwrap()
                .iter()
                .all(|s| !s.as_str().unwrap().ends_with(".hidden")));
            assert!(
                artifact(&report, "mermaid").contains("#36;"),
                "dollar identifiers must display, not interpolate"
            );
        }
    }
    f.safe(&saved);
}

#[test]
fn native_api_and_class_views_keep_factory_candidates_and_unknown_injected_receivers() {
    let Some(f) = Fixture::new() else { return };
    let saved = f.saved();
    let selected = saved["page"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["kind"] == "api")
        .unwrap();
    assert_eq!(selected["method"], "POST");
    assert_eq!(selected["routePath"], "/lookup");
    fs::remove_dir_all(&f.root).unwrap();
    for view in ["services", "network"] {
        let report = success(f.diagram(&saved, selected, view).output().unwrap());
        let data = legend(&report);
        assert_eq!(data["item"], *selected);
        assert_eq!(data["sourceModel"], "static-candidate");
        assert_eq!(data["routerContext"]["status"], "unsupported-language");
        assert_eq!(data["counts"]["networkGroups"], 0);
        let calls = data["calls"].as_array().unwrap();
        assert_eq!(
            calls.len(),
            1,
            "registration must not become callback body: {data}"
        );
        assert_eq!(calls[0]["ownerFactId"], selected["factId"]);
        assert!(
            calls[0].to_string().contains("req.contextService.lookup"),
            "{data}"
        );
        assert_ne!(
            calls[0]["targetCandidate"]["kind"],
            "annotated_receiver_candidate"
        );
        assert!(data["focus"]["links"].as_array().unwrap().is_empty());
        for invented in [
            "FastAPI",
            "HTTP client/import boundary candidate",
            "typed service",
        ] {
            assert!(!artifact(&report, "mermaid").contains(invented));
        }
    }
    for kind in ["api", "class"] {
        let report = success(
            f.command()
                .arg("diagram")
                .arg(catalog(&saved))
                // Exercise the focused UML projection; the unfiltered class
                // command now produces the complete multi-UML bundle instead.
                .args(["--kind", kind, "--limit", "12", "--no-render"])
                .output()
                .unwrap(),
        );
        let data = legend(&report);
        assert_eq!(data["sourceModel"], "static-candidate");
        if kind == "class" {
            assert_eq!(report["view"], "class-focus");
            let relations = data["relationships"].as_array().unwrap();
            assert_eq!(relations.len(), 2);
            let base = relations
                .iter()
                .find(|r| r["relationship"] == "extends")
                .unwrap();
            assert!(
                base["targetClassId"].is_null(),
                "external base must stay unresolved"
            );
            let store = relations
                .iter()
                .find(|r| r["relationship"] == "stores_parameter")
                .unwrap();
            assert_eq!(store["memberName"], "dependency");
            assert_eq!(store["bindingKind"], "constructor_parameter");
            assert!(store["targetClassId"].is_null());
            assert!(report["mermaid"].is_null() && report["svg"].is_null());
            assert_eq!(report["diagrams"].as_array().unwrap().len(), 1);
            let mmd =
                fs::read_to_string(report["diagrams"][0]["mermaid"].as_str().unwrap()).unwrap();
            assert!(
                mmd.starts_with("classDiagram\n")
                    && mmd.contains("#36;execute()")
                    && mmd.contains("-dependency")
                    && mmd.contains("<|--"),
                "{mmd}"
            );
            assert!(data["entries"][0]["attributes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|m| m["name"] == "dependency" && m["visibility"] == "private"));
        }
    }
    f.safe(&saved);
}

#[test]
fn javascript_no_view_and_kind_mismatches_fail_before_python_or_agent() {
    let Some(f) = Fixture::new() else { return };
    let saved = f.saved();
    let selected = item(&saved, "workflow", ".$run");
    fs::remove_dir_all(&f.root).unwrap();
    let before = artifacts(catalog(&saved));
    for extract_only in [false, true] {
        let mut cmd = f.command();
        cmd.arg("diagram").arg(catalog(&saved)).args([
            "--entry",
            selected["id"].as_str().unwrap(),
            "--no-render",
        ]);
        if extract_only {
            cmd.arg("--extract-only");
        }
        rejected(cmd.output().unwrap(), "JavaScript");
    }
    for kind in ["api", "class"] {
        rejected(
            f.diagram(&saved, selected, "services")
                .args(["--kind", kind])
                .output()
                .unwrap(),
            "kind",
        );
        rejected(
            f.command()
                .arg("diagram")
                .arg(catalog(&saved))
                .args([
                    "--entry",
                    selected["id"].as_str().unwrap(),
                    "--kind",
                    kind,
                    "--no-render",
                ])
                .output()
                .unwrap(),
            "kind",
        );
    }
    rejected(
        f.diagram(&saved, item(&saved, "class", ".Service"), "network")
            .output()
            .unwrap(),
        "requires",
    );
    assert_eq!(artifacts(catalog(&saved)), before);
    f.safe(&saved);
}

#[test]
fn repeat_views_preserve_mtime_and_new_selection_replaces_only_current_slot() {
    let Some(f) = Fixture::new() else { return };
    let saved = f.saved();
    let first = item(&saved, "workflow", ".$run");
    let second = item(&saved, "workflow", ".$async");
    fs::remove_dir_all(&f.root).unwrap();
    for view in ["services", "network"] {
        let report = success(f.diagram(&saved, first, view).output().unwrap());
        let before = artifacts(catalog(&saved));
        assert_eq!(
            success(f.diagram(&saved, first, view).output().unwrap()),
            report
        );
        assert_eq!(artifacts(catalog(&saved)), before);
        let next = success(f.diagram(&saved, second, view).output().unwrap());
        check_callable(&next, second, &saved);
        assert_eq!(next["viewDirectory"], report["viewDirectory"]);
        assert_eq!(
            Path::new(next["viewDirectory"].as_str().unwrap()),
            catalog(&saved).join("views").join(view)
        );
        assert!(!artifact(&next, "mermaid").contains("injected.lookup"));
        let current = artifacts(catalog(&saved));
        assert_eq!(
            success(f.diagram(&saved, second, view).output().unwrap()),
            next
        );
        assert_eq!(artifacts(catalog(&saved)), current);
    }
    let slots: BTreeSet<_> = fs::read_dir(catalog(&saved).join("views"))
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(
        slots,
        ["services", "network", "index.html"]
            .into_iter()
            .map(Into::into)
            .collect()
    );
    // A changed source replaces this application's current catalog, not a history slot.
    fs::create_dir(&f.root).unwrap();
    fs::write(
        f.root.join("app.ts"),
        "export function $fresh() { return 1; }",
    )
    .unwrap();
    let fresh = f.saved();
    assert_ne!(fresh["snapshotId"], saved["snapshotId"]);
    assert_eq!(fresh["catalogDirectory"], saved["catalogDirectory"]);
    assert!(
        !catalog(&fresh).join("views").exists(),
        "stale source views survived a new snapshot"
    );
    assert_eq!(
        fresh["page"]["counts"],
        json!({"api": 0, "class": 0, "workflow": 1})
    );
    let selected = item(&fresh, "workflow", ".$fresh");
    fs::remove_dir_all(&f.root).unwrap();
    let report = success(f.diagram(&fresh, selected, "services").output().unwrap());
    check_callable(&report, selected, &fresh);
    assert_eq!(
        Path::new(report["viewDirectory"].as_str().unwrap()),
        catalog(&fresh).join("views/services")
    );
    let current = artifacts(catalog(&fresh));
    assert_eq!(
        success(f.diagram(&fresh, selected, "services").output().unwrap()),
        report
    );
    assert_eq!(artifacts(catalog(&fresh)), current);
    f.safe(&saved);
}

#[test]
fn retired_profile_values_fail_before_catalog_or_tool_access() {
    let temp = tempdir().unwrap();
    for value in ["legacy", "invalidprofile", "", "generic"] {
        let output = Command::new(env!("CARGO_BIN_EXE_ideomancer"))
            .current_dir(temp.path())
            .args([
                "diagram",
                "missing-catalog",
                "--kind",
                "api",
                "--entry",
                "missing-entry",
                "--view",
                "services",
                "--js-profile",
                value,
            ])
            .env("IDEOMANCER_NODE", "/usr/bin/false")
            .env("IDEOMANCER_PYTHON", "/usr/bin/false")
            .env("IDEOMANCER_AGENT", "/usr/bin/false")
            .output()
            .unwrap();
        rejected(output, "removed");
    }
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
}

#[test]
fn missing_parser_is_actionable_and_target_tools_including_symlinks_are_rejected() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("app.js"), "export function run() {}").unwrap();
    let missing = Command::new(env!("CARGO_BIN_EXE_ideomancer"))
        .arg("discover")
        .arg(temp.path())
        .args(["--lang", "js", "--json"])
        .env_remove("IDEOMANCER_TYPESCRIPT")
        .env("IDEOMANCER_NODE", "/usr/bin/false")
        .output()
        .unwrap();
    rejected(missing, "--typescript");
    let Some(f) = Fixture::new() else { return };
    let target_node = f.root.join("node");
    fs::copy(&f.forbidden, &target_node).unwrap();
    fs::set_permissions(&target_node, fs::Permissions::from_mode(0o755)).unwrap();
    let target_parser = f.root.join("compiler.js");
    fs::write(
        &target_parser,
        format!(
            "require('fs').writeFileSync({}, 'executed');",
            json!(f.cwd.join("module.executed"))
        ),
    )
    .unwrap();
    for (flag, target) in [("--node", target_node), ("--typescript", target_parser)] {
        let link = f
            .cwd
            .join(format!("outside-{}", flag.trim_start_matches('-')));
        symlink(&target, &link).unwrap();
        for selected in [&target, &link] {
            let mut command = f.command();
            command
                .arg("discover")
                .arg(&f.root)
                .args(["--lang", "js", "--json"])
                .env("IDEOMANCER_NODE", &f.tools.node)
                .env("IDEOMANCER_TYPESCRIPT", &f.tools.compiler)
                .arg(flag)
                .arg(selected);
            rejected(command.output().unwrap(), "outside the target");
            assert!(!f.cwd.join("tool.executed").exists());
            assert!(!f.cwd.join("module.executed").exists());
            assert!(!f.cwd.join(".ideomancer/analyses").exists());
        }
    }
}

#[test]
fn node_loaders_module_paths_and_target_configuration_never_execute() {
    let Some(f) = Fixture::new() else { return };
    let baseline = f.saved();
    let loader = f.root.join("config/loader.cjs");
    fs::create_dir(loader.parent().unwrap()).unwrap();
    fs::write(
        &loader,
        format!(
            "require('fs').writeFileSync({}, 'executed');",
            json!(f.cwd.join("loader.executed"))
        ),
    )
    .unwrap();
    let plugin = f.root.join("config/plugin.js");
    fs::write(
        &plugin,
        format!(
            "require('fs').writeFileSync({}, 'executed');",
            json!(f.cwd.join("plugin.executed"))
        ),
    )
    .unwrap();
    fs::write(f.root.join("tsconfig.json"), json!({"compilerOptions": {"plugins": [{"name": plugin}], "baseUrl": ".", "paths": {"*": ["config/*"]}}, "extends": "./config/plugin.js"}).to_string()).unwrap();
    fs::write(
        f.root.join("package.json"),
        "MALFORMED CONFIG MUST NOT BE READ [",
    )
    .unwrap();
    let malicious = f.root.join("node_modules/typescript/lib");
    fs::create_dir_all(&malicious).unwrap();
    fs::write(
        malicious.join("typescript.js"),
        format!(
            "require('fs').writeFileSync({}, 'executed'); throw Error('target compiler executed');",
            json!(f.cwd.join("module.executed"))
        ),
    )
    .unwrap();
    let output = f
        .discover()
        .env("NODE_OPTIONS", format!("--require={}", loader.display()))
        .env("NODE_PATH", f.root.join("node_modules"))
        .output()
        .unwrap();
    assert_eq!(success(output), baseline);
    f.safe(&baseline);
}

#[test]
fn documented_source_scope_excludes_configs_dependencies_links_and_special_files() {
    let Some(f) = Fixture::new() else { return };
    fs::create_dir(f.root.join("src")).unwrap();
    fs::write(f.root.join("src/kept.ts"), "export function kept() {}").unwrap();
    for name in [
        "config/a.ts",
        "configs/a.js",
        "environments/a.ts",
        "tests/a.ts",
        "env/a.ts",
        "vendor/a.js",
        "node_modules/a.js",
        "bower_components/a.js",
        "generated/a.js",
        "coverage/a.js",
        ".hidden/a.js",
        "a.spec.ts",
        "a.test.js",
        "a.min.js",
        "a.bundle.js",
        "a.d.ts",
    ] {
        let path = f.root.join("src").join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "export function excludedScopeSentinel() {}").unwrap();
    }
    fs::write(f.root.join(".env"), SECRET).unwrap();
    let outside = f.cwd.join("external.ts");
    fs::write(&outside, "export function excludedLinkSentinel() {}").unwrap();
    symlink(&outside, f.root.join("src/link.ts")).unwrap();
    symlink(&f.cwd, f.root.join("src/dependency-link")).unwrap();
    let fifo = f.root.join("src/pipe.ts");
    assert!(Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .unwrap()
        .success());
    let saved = incomplete(f.discover().output().unwrap());
    let evidence = read_json(&catalog(&saved).join("evidence.json"));
    assert_eq!(
        evidence["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["path"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["src/kept.ts"]
    );
    assert_eq!(
        saved["page"]["counts"],
        json!({"api": 0, "class": 0, "workflow": 1})
    );
    item(&saved, "workflow", ".kept");
    assert!(!evidence.to_string().contains("excludedScopeSentinel"));
    assert!(!evidence.to_string().contains("excludedLinkSentinel"));
    f.safe(&saved);
}

#[test]
fn same_file_binding_guards_and_api_callback_ownership_are_not_position_guesses() {
    let Some(f) = Fixture::new() else { return };
    let guarded = r#"
import { importedHelper } from './external';
function helper() { return 1; }
export function direct() { helper(); }
export function external() { importedHelper(); }
export function shadow(helper) { helper(); }
export function blocked() { { let helper; helper(); } }
export function tdz() { helper(); const helper = () => 1; }
function mutable() { return 1; }
mutable = replacement;
export function reassigned() { mutable(); }
function duplicate() {}
function duplicate() {}
export function ambiguous() { duplicate(); }
"#;
    for extension in ["js", "ts"] {
        fs::write(f.root.join(format!("guards.{extension}")), guarded).unwrap();
    }
    fs::write(
        f.root.join("external.ts"),
        "export function importedHelper() { return 1; }",
    )
    .unwrap();
    fs::write(
        f.root.join("routes.js"),
        r#"
function named(req) { return profile(req.body); }
function profile(body) { return body; }
module.exports = (router, deps) => {
  router.post('/named', named);
  router.postAsync('/not-standard-http', named);
  function unrelated() { deps.wrong(); }
  router.post('/inline', req => { profile(req.body); req.contextService.lookup(req.body); });
  function following() { deps.alsoWrong(); }
};
"#,
    )
    .unwrap();
    let saved = f.saved();
    let evidence = read_json(&catalog(&saved).join("evidence.json"));
    let facts = evidence["facts"].as_array().unwrap();
    let apis: Vec<_> = saved["page"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["kind"] == "api")
        .collect();
    assert_eq!(apis.len(), 2);
    fs::remove_dir_all(&f.root).unwrap();
    for extension in ["js", "ts"] {
        for (name, count) in [
            ("direct", 1),
            ("external", 1),
            ("shadow", 0),
            ("blocked", 0),
            ("tdz", 0),
            ("reassigned", 0),
            ("ambiguous", 0),
        ] {
            let selected = item(&saved, "workflow", &format!("guards.{extension}.{name}"));
            let report = success(f.diagram(&saved, selected, "services").output().unwrap());
            let data = check_callable(&report, selected, &saved);
            assert_eq!(
                data["focus"]["links"].as_array().unwrap().len(),
                count,
                "{extension} {name}: {data}"
            );
            if count == 0 {
                assert!(
                    !data["focus"]["unresolved"].as_array().unwrap().is_empty(),
                    "{data}"
                );
            }
        }
    }
    for api in apis {
        let route = facts
            .iter()
            .find(|fact| {
                fact["kind"] == "api_route"
                    && api["evidenceIds"].as_array().unwrap().contains(&fact["id"])
            })
            .unwrap();
        let metadata: Value = serde_json::from_str(route["value"].as_str().unwrap()).unwrap();
        assert_eq!(metadata["handlerFactId"], api["factId"]);
        if api["routePath"] == "/named" {
            assert!(api["symbol"].as_str().unwrap().ends_with(".named"));
        }
        let report = success(f.diagram(&saved, api, "services").output().unwrap());
        let data = legend(&report);
        assert!(
            data["focus"]["visitedSymbols"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s.as_str().unwrap().ends_with(".profile")),
            "{data}"
        );
        assert_eq!(data["focus"]["links"].as_array().unwrap().len(), 1);
        assert_eq!(data["counts"]["networkGroups"], 0);
        let mmd = artifact(&report, "mermaid");
        assert!(
            !mmd.contains("alsoWrong") && !mmd.contains("deps.wrong"),
            "{mmd}"
        );
    }
    f.safe(&saved);
}

#[test]
fn malformed_sources_are_incomplete_without_fake_bodies_and_language_aliases_preserve_state() {
    let Some(f) = Fixture::new() else { return };
    let state = f.root.join(".ideomancer/analyses/accepted/state.json");
    fs::create_dir_all(state.parent().unwrap()).unwrap();
    fs::write(
        &state,
        b"{\"status\":\"accepted\",\"operatorOwned\":true}\n",
    )
    .unwrap();
    let original_state = (
        fs::read(&state).unwrap(),
        fs::metadata(&state).unwrap().modified().unwrap(),
    );
    fs::write(f.root.join("root-app.ts"), "export function rootApp() {}").unwrap();
    fs::write(
        f.root.join("bad.ts"),
        format!("export function fakeBody( {{ return '{SECRET}';"),
    )
    .unwrap();
    fs::write(
        f.root.join("invalid.js"),
        b"export function invalidUtf8() { return '\xff'; }",
    )
    .unwrap();
    let saved = incomplete(f.discover().output().unwrap());
    assert_eq!(saved["complete"], false);
    assert_eq!(saved["page"]["complete"], false);
    item(&saved, "workflow", ".rootApp");
    let evidence = read_json(&catalog(&saved).join("evidence.json"));
    assert_eq!(evidence["complete"], false);
    assert!(evidence["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["code"] == "javascript_parse_error"));
    assert!(evidence["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["path"] == "invalid.js"));
    for fact in evidence["facts"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|fact| fact["path"] == "bad.ts" || fact["path"] == "invalid.js")
    {
        assert_eq!(fact["kind"], "module", "no recovery-tree bodies: {fact}");
    }
    for diagnostic in evidence["diagnostics"].as_array().unwrap() {
        let message = diagnostic["message"].as_str().unwrap();
        assert!(
            !message.contains(SECRET)
                && !message.contains("return")
                && !message.contains("fakeBody"),
            "source snippet leaked: {diagnostic}"
        );
    }
    let before = artifacts(catalog(&saved));
    for alias in [
        "javascript-typescript",
        "js",
        "ts",
        "javascript",
        "typescript",
    ] {
        let report = incomplete(
            f.command()
                .arg("discover")
                .arg(&f.root)
                .args(["--language", alias, "--json", "--limit", "30"])
                .arg("--out")
                .arg(f.cwd.join("catalog output"))
                .env("IDEOMANCER_NODE", &f.tools.node)
                .env("IDEOMANCER_TYPESCRIPT", &f.tools.compiler)
                .output()
                .unwrap(),
        );
        assert_eq!(report, saved);
        assert_eq!(artifacts(catalog(&saved)), before);
    }
    // Language selection is explicit and invalid values never reach a runtime.
    rejected(
        f.command()
            .arg("discover")
            .arg(&f.root)
            .args(["--lang", "not-a-language", "--json"])
            .output()
            .unwrap(),
        "invalid value",
    );
    assert_eq!(
        (
            fs::read(&state).unwrap(),
            fs::metadata(&state).unwrap().modified().unwrap()
        ),
        original_state
    );
    // The pre-existing accepted-state fixture is intentionally not an agent run.
    assert!(!f.cwd.join("tool.executed").exists());
    f.safe(&saved);
}

// Every module case starts without the unrelated default app or router fixture.
fn module_fixture(files: &[(&str, &str)]) -> Option<Fixture> {
    let f = Fixture::new()?;
    fs::remove_file(f.root.join("app.ts")).unwrap();
    fs::remove_file(f.root.join("routes.js")).unwrap();
    for (name, source) in files {
        let path = f.root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, source).unwrap();
    }
    Some(f)
}

fn check_module_proof(data: &Value, evidence: &Value, link: &Value) {
    assert_eq!(link["kind"], "js_module_call_candidate");
    assert_eq!(link["resolutionPolicy"], "selected-relative-functions-1");
    let facts = evidence["facts"].as_array().unwrap();
    let proof: Vec<_> = link["evidenceIds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|id| facts.iter().find(|f| f["id"] == *id).unwrap())
        .collect();
    let refs = data["sourceRefs"].as_array().unwrap();
    for fact in &proof {
        assert!(
            refs.iter()
                .any(|r| r["factId"] == fact["id"] && r["path"] == fact["path"]),
            "missing citation: {fact}: {data}"
        );
        if matches!(
            fact["kind"].as_str(),
            Some(
                "js_module_import"
                    | "js_module_export"
                    | "js_module_summary"
                    | "js_declaration"
                    | "js_call"
                    | "js_binding_guard"
            )
        ) {
            let metadata: Value = serde_json::from_str(fact["value"].as_str().unwrap()).unwrap();
            for field in [
                "ownerFactId",
                "bindingGuardFactId",
                "targetFactId",
                "targetMetadataFactId",
                "declarationFactId",
                "callFactId",
                "importFactId",
            ] {
                if let Some(id) = metadata[field].as_str() {
                    assert!(
                        proof.iter().any(|f| f["id"] == id),
                        "incomplete {field} proof: {fact}"
                    );
                }
            }
            for id in metadata["supportingFactIds"]
                .as_array()
                .into_iter()
                .flatten()
            {
                assert!(
                    proof.iter().any(|f| f["id"] == *id),
                    "missing supporting proof: {fact}"
                );
            }
        }
    }
    for (kind, minimum) in [
        ("module", 2),
        ("js_module_summary", 2),
        ("js_module_import", 1),
        ("js_module_export", 1),
        ("js_binding_guard", 2),
        ("js_declaration", 2),
        ("js_call", 1),
        ("call", 1),
    ] {
        assert!(
            proof.iter().filter(|f| f["kind"] == kind).count() >= minimum,
            "missing {kind}: {proof:?}"
        );
    }
    for field in ["sourceFactId", "targetFactId"] {
        assert!(
            proof.iter().any(|f| f["id"] == link[field]),
            "{field}: {link}"
        );
    }
    let source = facts
        .iter()
        .find(|f| f["id"] == link["sourceFactId"])
        .unwrap();
    let target = facts
        .iter()
        .find(|f| f["id"] == link["targetFactId"])
        .unwrap();
    assert_ne!(source["path"], target["path"], "{link}");
    assert!(
        data["calls"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["callFactId"] == link["sourceFactId"] && c["targetCandidate"] == *link),
        "renderer lost candidate: {data}"
    );
}

#[test]
fn module_chains_keep_complete_cross_file_proofs_offline_and_repeat_without_writes() {
    for (app, work, leaf) in [
        (
            "import {work as task} from './work'; export function run(injected) { task(); injected.lookup(); this.work(); fetch('./private'); }",
            "import {leaf as next} from './leaf'; function work() { next(); local(); } function local() { return 1; } export {work};",
            "export function leaf() { return 1; }",
        ),
        (
            "const {work: task} = require('./work'); function run(injected) { task(); injected.lookup(); this.work(); fetch('./private'); } module.exports = {run};",
            "const {leaf: next} = require('./leaf'); function work() { next(); local(); } function local() { return 1; } module.exports = {work};",
            "function leaf() { return 1; } module.exports = {leaf};",
        ),
    ] {
        let Some(f) = module_fixture(&[("app.js", app), ("work.js", work), ("leaf.js", leaf)]) else { return };
        let saved = f.saved();
        let evidence = read_json(&catalog(&saved).join("evidence.json"));
        let selected = item(&saved, "workflow", ".run");
        fs::remove_dir_all(&f.root).unwrap();
        for view in ["services", "network"] {
            let report = success(f.diagram(&saved, selected, view).output().unwrap());
            let data = check_callable(&report, selected, &saved);
            let links = data["focus"]["links"].as_array().unwrap();
            assert_eq!(links.len(), 3, "{data}");
            let modules: Vec<_> = links.iter().filter(|l| l["kind"] == "js_module_call_candidate").collect();
            assert_eq!(modules.len(), 2, "{data}");
            let targets: BTreeSet<_> = modules.iter().map(|l| {
                check_module_proof(&data, &evidence, l);
                evidence["facts"].as_array().unwrap().iter().find(|f| f["id"] == l["targetFactId"]).unwrap()["path"].as_str().unwrap()
            }).collect();
            assert_eq!(targets, BTreeSet::from(["leaf.js", "work.js"]));
            assert!(links.iter().any(|l| l["kind"] == "js_local_call_candidate"));
            for name in ["injected.lookup", "this.work", "fetch"] {
                let call = data["calls"].as_array().unwrap().iter().find(|c| c["label"].as_str().unwrap().contains(name)).unwrap();
                assert!(call["targetCandidate"].is_null(), "invented target: {call}");
            }
            let before = artifacts(catalog(&saved));
            assert_eq!(success(f.diagram(&saved, selected, view).output().unwrap()), report);
            assert_eq!(artifacts(catalog(&saved)), before);
        }
        f.safe(&saved);
    }
}

#[test]
fn module_registry_factory_class_method_imports_helper_without_di_or_this_resolution() {
    let Some(f) = module_fixture(&[
        ("registry.js", "const {work: task} = require('./work'); module.exports = deps => { return class Registry { run() { task(); deps.lookup(); this.work(); } }; };"),
        ("work.js", "function local() { return 1; } function work() { local(); } module.exports = {work};"),
    ]) else { return };
    let saved = f.saved();
    let evidence = read_json(&catalog(&saved).join("evidence.json"));
    let selected = item(&saved, "workflow", ".Registry.run");
    let facts = evidence["facts"].as_array().unwrap();
    let factory = facts
        .iter()
        .find(|fact| fact["kind"] == "js_factory")
        .unwrap();
    let factory: Value = serde_json::from_str(factory["value"].as_str().unwrap()).unwrap();
    assert_eq!(factory["safe"], true);
    assert_eq!(factory["resultKind"], "class");
    assert!(factory["returnedClassFactId"].is_string());
    assert!(facts.iter().any(|fact| fact["kind"] == "js_factory_return"));
    assert!(facts.iter().all(|fact| !matches!(
        fact["kind"].as_str(),
        Some("js_service_factory" | "js_service_request" | "js_model_query")
    )));
    fs::remove_dir_all(&f.root).unwrap();
    for view in ["services", "network"] {
        let report = success(f.diagram(&saved, selected, view).output().unwrap());
        let data = check_callable(&report, selected, &saved);
        let links = data["focus"]["links"].as_array().unwrap();
        assert_eq!(links.len(), 2, "{data}");
        check_module_proof(
            &data,
            &evidence,
            links
                .iter()
                .find(|l| l["kind"] == "js_module_call_candidate")
                .unwrap(),
        );
        assert!(links.iter().any(|l| l["kind"] == "js_local_call_candidate"));
        for name in ["deps.lookup", "this.work"] {
            let call = data["calls"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["label"].as_str().unwrap().contains(name))
                .unwrap();
            assert!(call["targetCandidate"].is_null(), "{call}");
        }
    }
    f.safe(&saved);
}

macro_rules! unsupported_module_cases {
    ($($name:ident: [$($file:literal => $source:expr),+ $(,)?]),+ $(,)?) => {
        $(#[test]
        fn $name() {
            let Some(f) = module_fixture(&[$(($file, $source)),+]) else { return };
            let saved = f.saved();
            let selected = item(&saved, "workflow", ".run");
            fs::remove_dir_all(&f.root).unwrap();
            for view in ["services", "network"] {
                let report = success(f.diagram(&saved, selected, view).output().unwrap());
                let data = check_callable(&report, selected, &saved);
                assert!(data["focus"]["links"].as_array().unwrap().is_empty(), "unsupported link: {data}");
                assert!(data["calls"].as_array().unwrap().iter().all(|c| c["targetCandidate"].is_null()), "unsupported renderer candidate: {data}");
                assert!(!data["focus"]["unresolved"].as_array().unwrap().is_empty(), "{data}");
            }
            f.safe(&saved);
        })+
    };
}

unsupported_module_cases! {
    module_rejects_type_only_import: ["app.ts" => "import type {work} from './work'; export function run() { work(); }", "work.ts" => "export function work() {}"],
    module_rejects_type_only_import_specifier: ["app.ts" => "import {type work} from './work'; export function run() { work(); }", "work.ts" => "export function work() {}"],
    module_rejects_type_only_export: ["app.ts" => "import {work} from './work'; export function run() { work(); }", "work.ts" => "function work() {} export type {work};"],
    module_rejects_type_only_export_specifier: ["app.ts" => "import {work} from './work'; export function run() { work(); }", "work.ts" => "function work() {} export {type work};"],
    module_rejects_shadowed_parameter: ["app.js" => "import {work} from './work'; export function run(work) { work(); }", "work.js" => "export function work() {}"],
    module_rejects_rebound_import: ["app.js" => "import {work} from './work'; work = replacement; export function run() { work(); }", "work.js" => "export function work() {}"],
    module_rejects_rebound_export: ["app.js" => "import {work} from './work'; export function run() { work(); }", "work.js" => "export function work() {} work = replacement;"],
    module_rejects_ambiguous_extension: ["app.js" => "import {work} from './work'; export function run() { work(); }", "work.js" => "export function work() {}", "work.ts" => "export function work() {}"],
    module_rejects_directory_index: ["app.js" => "import {work} from './work'; export function run() { work(); }", "work/index.js" => "export function work() {}"],
    module_rejects_js_to_ts_remap: ["app.js" => "import {work} from './work.js'; export function run() { work(); }", "work.ts" => "export function work() {}"],
    module_rejects_out_of_scope: ["app.js" => "import {work} from './config/work'; export function run() { work(); }", "config/work.js" => "export function work() {}"],
    module_rejects_root_escape: ["app.js" => "import {work} from '../work'; export function run() { work(); }", "../work.js" => "export function work() {}"],
    module_rejects_mixed_importer: ["app.js" => "import {work} from './work'; export function run() { work(); } module.exports = {run};", "work.js" => "export function work() {}"],
    module_rejects_mixed_exporter: ["app.js" => "import {work} from './work'; export function run() { work(); }", "work.js" => "export function work() {} module.exports = {work};"],
    module_rejects_esm_to_cjs_interop: ["app.js" => "import {work} from './work'; export function run() { work(); }", "work.js" => "function work() {} module.exports = {work};"],
    module_rejects_cjs_to_esm_interop: ["app.js" => "const {work} = require('./work'); function run() { work(); } module.exports = {run};", "work.js" => "export function work() {}"],
    module_rejects_cycle: ["app.js" => "import {work} from './work'; export function run() { work(); }", "work.js" => "import {run} from './app'; export function work() { run(); }"],
    module_rejects_namespace_backedge_cycle: ["app.js" => "import {work} from './work'; export function run() { work(); }", "work.js" => "import * as back from './app'; export function work() { back.run(); }"],
    module_rejects_cjs_require_alias: ["app.js" => "const load = require; const {work} = load('./work'); function run() { work(); } module.exports = {run};", "work.js" => "function work() {} module.exports = {work};"],
    module_rejects_cjs_require_escape: ["app.js" => "escape(require); const {work} = require('./work'); function run() { work(); } module.exports = {run};", "work.js" => "function work() {} module.exports = {work};"],
    module_rejects_cjs_rebound_import: ["app.js" => "const {work} = require('./work'); work = replacement; function run() { work(); } module.exports = {run};", "work.js" => "function work() {} module.exports = {work};"],
    module_rejects_cjs_module_alias: ["app.js" => "const {work} = require('./work'); function run() { work(); } module.exports = {run};", "work.js" => "function work() {} const alias = module; alias.exports = {work};"],
    module_rejects_cjs_exports_escape: ["app.js" => "const {work} = require('./work'); function run() { work(); } module.exports = {run};", "work.js" => "function work() {} escape(exports); module.exports = {work};"],
    module_rejects_cjs_export_spread: ["app.js" => "const {work} = require('./work'); function run() { work(); } module.exports = {run};", "work.js" => "function work() {} module.exports = {...other, work};"],
    module_rejects_cjs_computed_export: ["app.js" => "const {work} = require('./work'); function run() { work(); } module.exports = {run};", "work.js" => "function work() {} module.exports = {['work']: work};"],
    module_rejects_cjs_destructure_rest: ["app.js" => "const {work, ...rest} = require('./work'); function run() { work(); } module.exports = {run};", "work.js" => "function work() {} module.exports = {work};"],
    module_rejects_cjs_computed_destructure: ["app.js" => "const {['work']: work} = require('./work'); function run() { work(); } module.exports = {run};", "work.js" => "function work() {} module.exports = {work};"],
    module_rejects_cjs_default_destructure: ["app.js" => "const {work = fallback} = require('./work'); function run() { work(); } module.exports = {run};", "work.js" => "function work() {} module.exports = {work};"],
    module_rejects_cjs_export_before_initialization: ["app.js" => "const {work} = require('./work'); function run() { work(); } module.exports = {run};", "work.js" => "module.exports = {work}; var work = () => 1;"],
}

#[test]
fn module_commonjs_initialized_variables_and_hoisted_functions_are_candidates() {
    for source in [
        "var work = () => 1; module.exports = {work};",
        "module.exports = {work}; function work() { return 1; }",
    ] {
        let Some(f) = module_fixture(&[
            ("app.js", "const {work: task} = require('./work'); function run() { task(); } module.exports = {run};"),
            ("work.js", source),
        ]) else { return };
        let saved = f.saved();
        let evidence = read_json(&catalog(&saved).join("evidence.json"));
        let selected = item(&saved, "workflow", ".run");
        fs::remove_dir_all(&f.root).unwrap();
        for view in ["services", "network"] {
            let report = success(f.diagram(&saved, selected, view).output().unwrap());
            let data = check_callable(&report, selected, &saved);
            let links = data["focus"]["links"].as_array().unwrap();
            assert_eq!(links.len(), 1, "{data}");
            check_module_proof(&data, &evidence, &links[0]);
        }
        f.safe(&saved);
    }
}

#[test]
fn module_dependencies_and_proofs_never_retain_argument_payloads_or_unsafe_specifiers() {
    let Some(f) = module_fixture(&[
        ("app.js", "require('node:fs').writeFileSync('./out','./PRIVATE_BODY'); const {work} = require('./work'); function run() { work(); } module.exports = {run};"),
        ("work.js", "function work() {} module.exports = {work};"),
        ("unsafe.ts", "import {bad} from './PRIVATE BODY'; export {bad} from './PRIVATE?QUERY'; export function unsafe() { bad(); }"),
        ("shadowed.js", "function shadowedDirect(require) { require('./PRIVATE_SHADOW_REQUIRE'); class C { field = require('./PRIVATE_SHADOW_OMITTED'); } } function shadowedReceivers(module, globalThis, global, window, self) { module.require('./PRIVATE_SHADOW_MODULE'); globalThis.require('./PRIVATE_SHADOW_GLOBAL_THIS'); global.require('./PRIVATE_SHADOW_GLOBAL'); window.require('./PRIVATE_SHADOW_WINDOW'); self.require('./PRIVATE_SHADOW_SELF'); }"),
        ("unproven.js", "globalThis.require('./PRIVATE_GLOBAL_REQUIRE'); global.require('./PRIVATE_GLOBAL_REQUIRE'); window.require('./PRIVATE_GLOBAL_REQUIRE'); self.require('./PRIVATE_GLOBAL_REQUIRE'); module.require('./PRIVATE_MODULE_REQUIRE');"),
        ("rebound.js", "require = loader; require('./PRIVATE_REBOUND_REQUIRE');"),
    ]) else { return };
    let saved = f.saved();
    assert!(!f.root.join("out").exists());
    let evidence = read_json(&catalog(&saved).join("evidence.json"));
    for fact in evidence["facts"].as_array().unwrap().iter().filter(|f| {
        ["js_module_summary", "js_module_import", "js_module_export"]
            .contains(&f["kind"].as_str().unwrap())
    }) {
        let text = fact.to_string();
        for payload in [
            "./out",
            "PRIVATE_BODY",
            "PRIVATE BODY",
            "PRIVATE?QUERY",
            "node:fs",
            "PRIVATE_SHADOW",
            "PRIVATE_GLOBAL_REQUIRE",
            "PRIVATE_MODULE_REQUIRE",
            "PRIVATE_REBOUND_REQUIRE",
        ] {
            assert!(
                !text.contains(payload),
                "module proof leaked {payload}: {fact}"
            );
        }
    }
    let summary = evidence["facts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["kind"] == "js_module_summary" && f["path"] == "app.js")
        .unwrap();
    let metadata: Value = serde_json::from_str(summary["value"].as_str().unwrap()).unwrap();
    assert_eq!(metadata["dependencies"], json!(["./work"]));
    for path in ["shadowed.js", "unproven.js", "rebound.js"] {
        let summary = evidence["facts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["kind"] == "js_module_summary" && f["path"] == path)
            .unwrap();
        let metadata: Value = serde_json::from_str(summary["value"].as_str().unwrap()).unwrap();
        assert_eq!(metadata["dependencies"], json!([]), "{summary}");
    }
    let selected = item(&saved, "workflow", ".run");
    fs::remove_dir_all(&f.root).unwrap();
    for view in ["services", "network"] {
        let report = success(f.diagram(&saved, selected, view).output().unwrap());
        let data = check_callable(&report, selected, &saved);
        assert!(
            data["focus"]["links"].as_array().unwrap().is_empty(),
            "unsupported require form must fail closed: {data}"
        );
        for field in ["legend", "mermaid", "viewer"] {
            let text = artifact(&report, field);
            assert!(
                !text.contains("PRIVATE_BODY")
                    && !text.contains("PRIVATE?QUERY")
                    && !text.contains("PRIVATE BODY"),
                "payload leaked in {field}"
            );
        }
    }
    for (path, (bytes, _)) in artifacts(catalog(&saved)) {
        let text = String::from_utf8_lossy(&bytes);
        for payload in [
            "PRIVATE_BODY",
            "PRIVATE BODY",
            "PRIVATE?QUERY",
            "PRIVATE_SHADOW",
            "PRIVATE_GLOBAL_REQUIRE",
            "PRIVATE_MODULE_REQUIRE",
            "PRIVATE_REBOUND_REQUIRE",
        ] {
            assert!(
                !text.contains(payload),
                "payload {payload} leaked in {path:?}"
            );
        }
    }
    f.safe(&saved);
}
