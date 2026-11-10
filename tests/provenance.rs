//! Synthetic source-only factory union regression through the public CLI.
#![cfg(unix)]
use serde_json::Value;
use std::fs;
use std::process::{Command, Output};

fn success(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn direct_factory_unions_remain_unselected_cited_alternatives_offline() {
    let python = std::env::var_os("IDEOMANCER_TEST_PYTHON").unwrap_or_else(|| "python3.12".into());
    let Ok(probe) = Command::new(&python)
        .args([
            "-I",
            "-c",
            "import sys; assert sys.version_info >= (3,12); print(sys.executable)",
        ])
        .output()
    else {
        eprintln!("SKIP: Python >=3.12 required");
        return;
    };
    if !probe.status.success() {
        eprintln!("SKIP: Python >=3.12 required");
        return;
    }
    let python = String::from_utf8(probe.stdout).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let mut previous = None;
    let annotations = [
        (0, "SDK | LocalClient", 1),
        (1, "LocalClient | SDK", 1),
        (2, "SDK | OtherSDK", 2),
        (3, "SDK | LocalClient | list[SDK]", 0),
        (4, "'SDK | LocalClient'", 0),
        (5, "SDK | Any", 0),
    ];
    let initializers = [
        (
            "factory.create(select([env], mode=\")[{\"), config=[env])",
            1,
            false,
        ),
        ("factory.create(env).as_local_adapter()", 0, true),
        ("factory.create(env)[0]", 0, true),
        ("factory.create(env).helper", 0, true),
        ("factory.create(env)()", 0, true),
        ("SDK(config=select([env], mode=\")[{\"))", 1, false),
        ("SDK().helper()", 0, true),
        ("SDK()[0]", 0, true),
        ("SDK().helper", 0, true),
        ("SDK()()", 0, true),
        ("factory.create(a,b,c,d,e,f,g,h,i,j,k,l,m,n,o,p,q)", 0, true),
        ("factory.unknown(env)", 0, false),
        ("UnknownSDK(select(env))", 0, false),
    ];
    let package_cases = [
        ("sibling", 1),
        ("package_init", 1),
        ("parent", 1),
        ("alias", 1),
        ("escaped", 0),
        ("missing", 0),
        ("duplicate", 0),
        ("shadow", 0),
        ("wrong_case", 0),
        ("local_sdk", 0),
        ("relative_sdk_missing", 0),
        ("relative_sdk_local", 0),
    ];
    let cases = annotations
        .into_iter()
        .map(|(number, annotation, groups)| (number, annotation, "factory.create()", groups, false))
        .chain(
            initializers
                .into_iter()
                .enumerate()
                .map(|(n, (initializer, groups, bad_shape))| {
                    (
                        n + annotations.len(),
                        "SDK | LocalClient",
                        initializer,
                        groups,
                        bad_shape,
                    )
                }),
        )
        .chain(std::iter::once((
            annotations.len() + initializers.len(),
            "SDK | LocalClient",
            "factory.create()",
            1,
            false,
        )))
        .chain(
            package_cases
                .into_iter()
                .enumerate()
                .map(|(n, (case, groups))| {
                    (
                        annotations.len() + initializers.len() + 1 + n,
                        if case == "alias" {
                            "SDK | Wrapped"
                        } else {
                            "SDK | LocalClient"
                        },
                        "factory.create()",
                        groups,
                        false,
                    )
                }),
        );
    for (number, annotation, initializer, groups, bad_shape) in cases {
        let big_case = number == annotations.len() + initializers.len();
        let package_case = number
            .checked_sub(annotations.len() + initializers.len() + 1)
            .and_then(|n| package_cases.get(n))
            .map(|(case, _)| *case);
        let filler: String = if big_case {
            (0..500)
                .map(|n| format!("    detail_{n} = unknown_{}\n", "x".repeat(400)))
                .collect()
        } else {
            String::new()
        };
        let cwd = temp.path().join(number.to_string());
        let root = cwd.join("source");
        fs::create_dir_all(&root).unwrap();
        if package_case.is_some() {
            fs::create_dir_all(root.join("pkg/factories")).unwrap();
            fs::write(root.join("pkg/__init__.py"), "").unwrap();
            fs::write(root.join("pkg/factories/__init__.py"), "").unwrap();
        }
        let factory_import = match package_case {
            Some("package_init") => "from pkg.factories import Factory",
            Some(_) => "from pkg.factories.factory import Factory",
            None => "from factory import Factory",
        };
        let marker = cwd.join("EXECUTED");
        fs::write(
            root.join("app.py"),
            format!(
                r#"
from pathlib import Path
Path({marker:?}).touch()
raise RuntimeError('source must never execute')
from unavailable_framework import FastAPI
{factory_import}
from benchling_sdk.benchling import Benchling as SDK
app = FastAPI()
class Context:
    def __init__(self, factory: Factory):
        self.benchling = {initializer}
def service(ctx: Context):
    ctx.benchling.containers.checkin()
{filler}
@app.post('/checkin')
def endpoint(ctx: Context):
    service(ctx)
"#,
                marker = marker.to_str().unwrap()
            ),
        )
        .unwrap();
        let factory_path = match package_case {
            Some("package_init") => "pkg/factories/__init__.py",
            Some(_) => "pkg/factories/factory.py",
            None => "factory.py",
        };
        let wrapper_import = match package_case {
            Some("parent") => "from ..local_client import ClientSlot, LocalClient",
            Some("escaped") => "from ....local_client import LocalClient",
            Some("missing") => "from .missing import LocalClient",
            Some("alias") => "from .local_client import LocalClient as Wrapped",
            Some("shadow") => "from .local_client import LocalClient\nLocalClient = replacement",
            Some("wrong_case") => "from .local_client import localClient",
            Some(_) => "from .local_client import ClientSlot, LocalClient",
            None => "from wrapper import LocalClient",
        };
        let sdk_import = if matches!(
            package_case,
            Some("relative_sdk_missing" | "relative_sdk_local")
        ) {
            "from .benchling_sdk.benchling import Benchling as SDK"
        } else {
            "from benchling_sdk.benchling import Benchling as SDK"
        };
        fs::write(
            root.join(factory_path),
            format!(
                r#"
{sdk_import}
from benchling_api_client import Benchling as OtherSDK
{wrapper_import}
from typing import Any
class Factory:
    def create(self) -> {annotation}:
        raise RuntimeError('not executed')
"#
            ),
        )
        .unwrap();
        let wrapper_path = match package_case {
            Some("parent") => "pkg/local_client.py",
            Some(_) => "pkg/factories/local_client.py",
            None => "wrapper.py",
        };
        let duplicate = if package_case == Some("duplicate") {
            "\nclass LocalClient:\n    pass\n"
        } else {
            ""
        };
        fs::write(root.join(wrapper_path), format!("from typing import Any\nclass ClientSlot:\n    pass\nclass LocalClient:\n    def __getattr__(self, name) -> Any:\n        raise RuntimeError('never unwrap')\n{duplicate}")).unwrap();
        if package_case == Some("local_sdk") {
            fs::create_dir(root.join("benchling_sdk")).unwrap();
            fs::write(root.join("benchling_sdk/__init__.py"), "").unwrap();
            fs::write(
                root.join("benchling_sdk/benchling.py"),
                "class Benchling:\n    pass\n",
            )
            .unwrap();
        }
        if package_case == Some("relative_sdk_local") {
            fs::create_dir(root.join("pkg/factories/benchling_sdk")).unwrap();
            fs::write(root.join("pkg/factories/benchling_sdk/__init__.py"), "").unwrap();
            fs::write(
                root.join("pkg/factories/benchling_sdk/benchling.py"),
                "class Benchling:\n    pass\n",
            )
            .unwrap();
        }
        let command = || {
            let mut c = Command::new(env!("CARGO_BIN_EXE_ideomancer"));
            c.current_dir(&cwd)
                .env("IDEOMANCER_PYTHON", cwd.join("missing-python"))
                .env("IDEOMANCER_AGENT", cwd.join("missing-agent"))
                .env("IDEOMANCER_MMDC", cwd.join("missing-renderer"))
                .env_remove("IDEOMANCER_MODEL");
            c
        };
        let saved = success(
            command()
                .arg("discover")
                .arg(&root)
                .env("IDEOMANCER_PYTHON", python.trim())
                .arg("--out")
                .arg(cwd.join("catalog"))
                .args(["--json", "--kind", "api"])
                .output()
                .unwrap(),
        );
        let entry = saved["page"]["items"][0]["id"].as_str().unwrap();
        for view in ["network", "services"] {
            let report = success(
                command()
                    .arg("diagram")
                    .arg(saved["catalogDirectory"].as_str().unwrap())
                    .args([
                        "--entry",
                        entry,
                        "--view",
                        view,
                        "--no-render",
                        "--focus-hops",
                        if big_case {
                            "4"
                        } else if package_case.is_some() {
                            "3"
                        } else {
                            "2"
                        },
                        "--focus-facts",
                        if big_case || package_case.is_some() {
                            "600"
                        } else {
                            "200"
                        },
                    ])
                    .output()
                    .unwrap(),
            );
            let legend: Value =
                serde_json::from_slice(&fs::read(report["legend"].as_str().unwrap()).unwrap())
                    .unwrap();
            if big_case {
                assert_eq!(legend["limits"]["maxFacts"], 600);
                assert_eq!(legend["limits"]["hops"], 4);
                let cap = legend["limits"]["focusMaxBytes"].as_u64().unwrap();
                assert_eq!(legend["coverage"]["maxBytes"], cap);
                assert!(legend["coverage"]["inputBytes"].as_u64().unwrap() <= cap);
                assert_eq!(
                    cap + legend["limits"]["reservedBytes"].as_u64().unwrap(),
                    196608
                );
                assert!(legend["limits"]["retainedInputBytes"].as_u64().unwrap() <= 196608);
                assert_eq!(legend["focus"]["truncated"], true);
                assert!(legend["focus"]["boundaries"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|b| b.as_str().unwrap().starts_with("byte_limit:")));
            }
            let boundary = legend["boundaries"]
                .as_array()
                .unwrap()
                .iter()
                .find(|b| {
                    b["callFactId"].as_str().is_some_and(|id| {
                        legend["calls"].as_array().unwrap().iter().any(|c| {
                            c["callFactId"] == id
                                && c["label"] == "ctx.benchling.containers.checkin"
                        })
                    })
                })
                .unwrap();
            if groups > 0 && initializer.starts_with("factory.") {
                assert_eq!(boundary["branches"].as_array().unwrap().len(), 2);
                assert!(boundary["classification"]
                    .as_str()
                    .unwrap()
                    .contains("runtime selection UNKNOWN"));
                assert!(boundary["sourceRefs"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|r| r["kind"] == "return_annotation"));
                if package_case.is_some() {
                    let alternative = boundary["branches"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|b| {
                            b["member"]
                                == if package_case == Some("alias") {
                                    "Wrapped"
                                } else {
                                    "LocalClient"
                                }
                        })
                        .unwrap();
                    assert_eq!(alternative["classification"], "UNKNOWN");
                    let qualified = if package_case == Some("parent") {
                        "pkg.local_client.LocalClient"
                    } else {
                        "pkg.factories.local_client.LocalClient"
                    };
                    assert_eq!(alternative["qualifiedName"], qualified);
                    let refs = alternative["sourceRefs"].as_array().unwrap();
                    assert!(refs
                        .iter()
                        .any(|r| r["kind"] == "from_import" && r["path"] == factory_path));
                    assert!(refs.iter().any(|r| r["kind"] == "class"
                        && r["symbol"] == qualified
                        && r["path"] == wrapper_path));
                    assert!(!refs.iter().any(|r| r["kind"] == "return_annotation"
                        && r["symbol"].as_str().unwrap().ends_with(".__getattr__")));
                    assert_eq!(boundary["resolutionTrace"]["branchCoverage"], "complete");
                }
                if number < 2 {
                    let alternative = boundary["branches"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|b| b["member"] == "LocalClient")
                        .unwrap();
                    assert_eq!(alternative["classification"], "UNKNOWN");
                }
            } else if groups == 0 {
                assert_eq!(boundary["classification"], "UNKNOWN unresolved destination");
                assert!(
                    boundary["resolutionTrace"]["stoppingReason"].is_string()
                        || boundary["branches"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|b| b["stoppingReason"].is_string())
                );
                if matches!(
                    package_case,
                    Some(
                        "escaped"
                            | "missing"
                            | "duplicate"
                            | "shadow"
                            | "wrong_case"
                            | "relative_sdk_missing"
                    )
                ) {
                    assert_eq!(boundary["resolutionTrace"]["branchCoverage"], "incomplete");
                    assert!(boundary["resolutionTrace"]["stoppingReason"].is_string());
                }
                if bad_shape {
                    assert!(
                        boundary["resolutionTrace"]["stoppingReason"]
                            .as_str()
                            .unwrap()
                            .contains("complete direct callee call"),
                        "{initializer}: {boundary}"
                    );
                }
            } else {
                assert_eq!(
                    boundary["classification"],
                    "Benchling SDK boundary candidate"
                );
                assert_eq!(boundary["branches"], serde_json::json!([]));
            }
            if view == "network" {
                assert_eq!(legend["counts"]["networkGroups"], groups, "{legend}");
                let normalized: Vec<_> = boundary["branches"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|b| {
                        (
                            b["member"].clone(),
                            b["qualifiedName"].clone(),
                            b["classification"].clone(),
                        )
                    })
                    .collect();
                if number == 0 {
                    previous = Some(normalized);
                } else if number == 1 {
                    assert_eq!(previous.as_ref().unwrap(), &normalized);
                }
            }
        }
        assert!(!marker.exists());
        assert!(!cwd.join(".ideomancer/analyses").exists());
    }
}
