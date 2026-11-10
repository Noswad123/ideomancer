use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde_json::{json, Value};
use tempfile::tempdir;

const YAML: &str = r#"schemaVersion: 0.1.0
id: sample
name: Sample Project
nodes:
  - id: z
    label: Zed
  - id: a
    label: Alpha
edges:
  - from: z
    to: a
    label: uses
  - from: a
    to: z
"#;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ideomancer"))
}

fn run(args: &[&str], input: &str) -> Output {
    let mut child = cli()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn ideomancer");
    // The CLI may reject arguments before reading stdin.
    let _ = child.stdin.take().unwrap().write_all(input.as_bytes());
    child.wait_with_output().unwrap()
}

fn success(output: &Output) {
    assert!(
        output.status.success(),
        "status: {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn failure(output: &Output) {
    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn report(output: &Output, valid: bool) -> Value {
    if valid {
        success(output);
    } else {
        failure(output);
    }
    let value: Value = serde_json::from_slice(&output.stdout).expect("JSON diagnostics on stdout");
    assert_eq!(value["valid"], valid);
    let errors = value["errors"].as_array().expect("errors array");
    assert!(errors
        .iter()
        .all(|error| error.as_str().is_some_and(|s| !s.is_empty())));
    assert_eq!(errors.is_empty(), valid);
    value
}

fn starter(bytes: &[u8], format: &str, id: &str) {
    let manifest: Value = match format {
        "json" => serde_json::from_slice(bytes).expect("JSON starter"),
        _ => {
            assert!(
                serde_json::from_slice::<Value>(bytes).is_err(),
                "default must be YAML"
            );
            serde_yaml::from_slice(bytes).expect("YAML starter")
        }
    };
    assert_eq!(manifest["schemaVersion"], "0.1.0");
    assert_eq!(manifest["name"], "My Project 42");
    assert_eq!(manifest["id"], id);
    let nodes = manifest["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0]["id"], "app");
    assert_eq!(nodes[0]["label"], "My Project 42");
    assert!(manifest["edges"].as_array().unwrap().is_empty());
    report(
        &run(
            &["validate", "-", "--json"],
            std::str::from_utf8(bytes).unwrap(),
        ),
        true,
    );
}

#[test]
fn init_stdout_yaml_and_json_roundtrip() {
    let output = run(&["init", "--name", "My Project 42"], "");
    success(&output);
    starter(&output.stdout, "yaml", "my-project-42");

    let output = run(
        &[
            "init",
            "--name",
            "My Project 42",
            "--id",
            "chosen-id",
            "--format",
            "json",
        ],
        "",
    );
    success(&output);
    starter(&output.stdout, "json", "chosen-id");
}

#[test]
fn init_file_inference_and_explicit_format_override_roundtrip() {
    let dir = tempdir().unwrap();
    for (filename, explicit, expected) in [
        ("project.json", None, "json"),
        ("project.yaml", None, "yaml"),
        ("project.yml", None, "yaml"),
        ("json-in-yaml.yaml", Some("json"), "json"),
        ("yaml-in-json.json", Some("yaml"), "yaml"),
    ] {
        let path = dir.path().join(filename);
        let mut command = cli();
        command
            .args(["init", "--name", "My Project 42", "--out"])
            .arg(&path);
        if let Some(format) = explicit {
            command.args(["--format", format]);
        }
        let output = command.output().unwrap();
        success(&output);
        assert!(output.stdout.is_empty());
        starter(&fs::read(&path).unwrap(), expected, "my-project-42");
        report(
            &cli()
                .arg("validate")
                .arg(&path)
                .arg("--json")
                .output()
                .unwrap(),
            true,
        );
    }
}

#[test]
fn init_refuses_existing_paths_and_does_not_create_parent_directories() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("existing.yaml");
    for original in [b"keep this file".as_slice(), b"".as_slice()] {
        fs::write(&path, original).unwrap();
        let output = cli()
            .args(["init", "--name", "Example", "--out"])
            .arg(&path)
            .output()
            .unwrap();
        failure(&output);
        assert!(!output.stderr.is_empty());
        assert_eq!(fs::read(&path).unwrap(), original);
    }
    let output = cli()
        .args(["init", "--name", "Example", "--out"])
        .arg(dir.path())
        .output()
        .unwrap();
    failure(&output);
    let missing = dir.path().join("missing/project.yaml");
    let output = cli()
        .args(["init", "--name", "Example", "--out"])
        .arg(&missing)
        .output()
        .unwrap();
    failure(&output);
    assert!(!missing.parent().unwrap().exists());
}

#[cfg(unix)]
#[test]
fn init_does_not_follow_existing_or_dangling_symlinks() {
    use std::os::unix::fs::symlink;
    let dir = tempdir().unwrap();
    for existing in [false, true] {
        let target = dir.path().join(format!("target-{existing}"));
        if existing {
            fs::write(&target, "untouched").unwrap();
        }
        let link = dir.path().join(format!("link-{existing}.yaml"));
        symlink(&target, &link).unwrap();
        failure(
            &cli()
                .args(["init", "--name", "Example", "--out"])
                .arg(&link)
                .output()
                .unwrap(),
        );
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        if existing {
            assert_eq!(fs::read_to_string(&target).unwrap(), "untouched");
        } else {
            assert!(!target.exists());
        }
    }
}

#[test]
fn init_rejects_blank_names_and_ids_without_creating_output() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("project.yaml");
    for args in [
        vec!["init", "--name", ""],
        vec!["init", "--name", " \t\n"],
        vec!["init", "--name", "Example", "--id", ""],
        vec!["init", "--name", "Example", "--id", " \t\n"],
    ] {
        let output = cli().args(args).arg("--out").arg(&path).output().unwrap();
        failure(&output);
        assert!(!output.stderr.is_empty());
        assert!(!path.exists());
    }
}

#[test]
fn validate_accepts_default_stdin_explicit_stdin_and_paths() {
    for args in [vec!["validate"], vec!["validate", "-"]] {
        let output = run(&args, YAML);
        success(&output);
        assert!(!output.stdout.is_empty(), "human success message");
    }
    report(&run(&["validate", "--json"], YAML), true);
    let dir = tempdir().unwrap();
    for (filename, source) in [
        ("manifest.yaml", YAML.to_string()),
        (
            "manifest.json",
            serde_json::to_string(&serde_yaml::from_str::<Value>(YAML).unwrap()).unwrap(),
        ),
    ] {
        let path = dir.path().join(filename);
        fs::write(&path, source).unwrap();
        success(&cli().arg("validate").arg(&path).output().unwrap());
        report(
            &cli()
                .arg("validate")
                .arg(&path)
                .arg("--json")
                .output()
                .unwrap(),
            true,
        );
    }
}

#[test]
fn validate_returns_json_for_parse_errors_and_unknown_fields() {
    for source in ["nodes: [", "{\"nodes\":", "{}", "null"] {
        report(&run(&["validate", "-", "--json"], source), false);
    }
    let base: Value = serde_yaml::from_str(YAML).unwrap();
    for pointer in ["", "/nodes/0", "/edges/0"] {
        let mut manifest = base.clone();
        manifest
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unexpected".into(), json!(true));
        for source in [
            serde_json::to_string(&manifest).unwrap(),
            serde_yaml::to_string(&manifest).unwrap(),
        ] {
            let diagnostics = report(&run(&["validate", "-", "--json"], &source), false);
            assert!(diagnostics["errors"]
                .as_array()
                .unwrap()
                .iter()
                .any(|error| error.as_str().unwrap().contains("unexpected")));
        }
    }
}

#[test]
fn validate_rejects_unsupported_versions_blank_metadata_and_bad_graphs() {
    let base: Value = serde_yaml::from_str(YAML).unwrap();
    for (pointer, value) in [
        ("/schemaVersion", json!("0.2.0")),
        ("/schemaVersion", json!("0.1.0-extra")),
        ("/schemaVersion", json!("")),
        ("/name", json!(" \t")),
        ("/id", json!(" \t")),
        ("/nodes/1/id", json!("z")),
        ("/edges/0/from", json!("missing_source")),
        ("/edges/0/to", json!("missing_target")),
    ] {
        let mut manifest = base.clone();
        *manifest.pointer_mut(pointer).unwrap() = value;
        let source = serde_json::to_string(&manifest).unwrap();
        report(&run(&["validate", "-", "--json"], &source), false);
        let output = run(&["validate", "-"], &source);
        failure(&output);
        assert!(!output.stdout.is_empty() || !output.stderr.is_empty());
    }
}

#[test]
fn validate_enforces_ascii_node_id_grammar() {
    for id in ["", " ", "1node", "has-dash", "has space", "a.b", "é", "aé"] {
        let source = json!({"schemaVersion":"0.1.0", "id":"sample", "name":"Sample", "nodes":[{"id":id}], "edges":[]}).to_string();
        report(&run(&["validate", "--json"], &source), false);
    }
    for id in ["_", "_private", "A", "a0_B"] {
        let source = json!({"schemaVersion":"0.1.0", "id":"sample", "name":"Sample", "nodes":[{"id":id}], "edges":[]}).to_string();
        report(&run(&["validate", "--json"], &source), true);
    }
}

#[test]
fn input_io_errors_use_stderr_even_in_json_mode() {
    let dir = tempdir().unwrap();
    for path in [dir.path().join("missing.yaml"), dir.path().to_path_buf()] {
        let output = cli()
            .arg("validate")
            .arg(path)
            .arg("--json")
            .output()
            .unwrap();
        failure(&output);
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
}

#[test]
fn required_strings_are_not_coerced_from_other_scalar_types() {
    let base: Value = serde_yaml::from_str(YAML).unwrap();
    for pointer in [
        "/schemaVersion",
        "/id",
        "/name",
        "/nodes/0/id",
        "/edges/0/from",
        "/edges/0/to",
    ] {
        for value in [Value::Null, json!(true), json!(42), json!(1.25)] {
            let mut manifest = base.clone();
            *manifest.pointer_mut(pointer).unwrap() = value;
            for source in [
                serde_json::to_string(&manifest).unwrap(),
                serde_yaml::to_string(&manifest).unwrap(),
            ] {
                report(&run(&["validate", "--json"], &source), false);
            }
        }
    }
    for pointer in ["/nodes/0/label", "/nodes/0/kind", "/edges/0/label"] {
        for value in [json!(true), json!(42)] {
            let mut manifest = base.clone();
            manifest
                .pointer_mut(pointer.rsplit_once('/').unwrap().0)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(pointer.rsplit_once('/').unwrap().1.into(), value);
            report(&run(&["validate", "--json"], &manifest.to_string()), false);
        }
    }
}

#[test]
fn optional_strings_accept_null_and_edges_can_be_omitted() {
    let source = json!({"schemaVersion":"0.1.0", "id":"sample", "name":"Sample", "nodes":[{"id":"app", "label":null, "kind":null}]}).to_string();
    report(&run(&["validate", "--json"], &source), true);
    let output = run(&["emit", "-"], &source);
    success(&output);
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("n_app[\"app\"]"));
}

#[test]
fn init_handles_unicode_names_and_explains_unusable_slugs() {
    let output = run(&["init", "--name", "日本語", "--format", "json"], "");
    success(&output);
    let manifest: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(manifest["id"], "日本語");
    report(
        &run(
            &["validate", "--json"],
            std::str::from_utf8(&output.stdout).unwrap(),
        ),
        true,
    );
    let output = run(&["init", "--name", "!!!"], "");
    failure(&output);
    assert!(String::from_utf8_lossy(&output.stderr).contains("--id"));
    success(&run(&["init", "--name", "!!!", "--id", "explicit"], ""));
}

#[test]
fn shipped_examples_validate_and_emit() {
    for source in [
        include_str!("../examples/project.idman.yaml"),
        include_str!("../examples/ideomancer.idman.yaml"),
        include_str!("../examples/manifest.json"),
    ] {
        report(&run(&["validate", "--json"], source), true);
        success(&run(&["emit", "-"], source));
    }
}

#[test]
#[ignore = "requires Mermaid CLI (mmdc) and its browser runtime"]
fn real_mermaid_renders_escaped_labels_as_literal_text() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("escaping.json");
    let output = dir.path().join("escaping.svg");
    let label = "Quote \" slash \\ literal #34; &amp; <b> | []{}() `code` 日本語\r\nNext line";
    let manifest = json!({
        "schemaVersion":"0.1.0", "id":"escaping", "name":"Escaping",
        "nodes":[{"id":"end", "label":label}, {"id":"graph"}],
        "edges":[{"from":"end", "to":"graph", "label":label}]
    });
    fs::write(&input, manifest.to_string()).unwrap();
    success(
        &cli()
            .arg("render")
            .arg(&input)
            .arg("--out")
            .arg(&output)
            .output()
            .unwrap(),
    );
    let svg = fs::read_to_string(output)
        .unwrap()
        .replace("<br />", "<br/>");
    assert!(svg.contains("<svg"));
    // XML entity escaping is intentional: both labels must show the literal
    // input, not interpret HTML, Mermaid entities or Markdown supplied by users.
    let literal =
        "Quote \" slash \\ literal #34; &amp;amp; &lt;b&gt; | []{}() `code` 日本語<br/>Next line";
    assert_eq!(
        svg.matches(literal).count(),
        2,
        "node and edge labels: {svg}"
    );
}

#[test]
fn invalid_input_does_not_modify_existing_output() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("bad.yaml");
    let output = dir.path().join("existing.svg");
    for source in ["nodes: [".to_string(), YAML.replace("to: a", "to: missing")] {
        fs::write(&input, source).unwrap();
        for command in ["emit", "render"] {
            fs::write(&output, "keep original").unwrap();
            failure(
                &cli()
                    .arg(command)
                    .arg(&input)
                    .arg("--out")
                    .arg(&output)
                    .output()
                    .unwrap(),
            );
            assert_eq!(fs::read_to_string(&output).unwrap(), "keep original");
        }
    }
}

#[test]
fn emit_is_deterministic_preserves_order_and_overwrites_output() {
    let expected = [
        "flowchart LR",
        "n_z[\"Zed\"]",
        "n_a[\"Alpha\"]",
        "n_z -->|\"uses\"| n_a",
        "n_a --> n_z",
    ];
    let first = run(&["emit", "-"], YAML);
    success(&first);
    let actual: Vec<_> = std::str::from_utf8(&first.stdout)
        .unwrap()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    assert_eq!(actual, expected);
    let json = serde_json::to_string(&serde_yaml::from_str::<Value>(YAML).unwrap()).unwrap();
    let second = run(&["emit", "-", "--format", "mermaid"], &json);
    success(&second);
    assert_eq!(second.stdout, first.stdout);
    let dir = tempdir().unwrap();
    let input = dir.path().join("input.json");
    let output = dir.path().join("output.mmd");
    fs::write(&input, json).unwrap();
    fs::write(&output, "old content that must be replaced").unwrap();
    let result = cli()
        .arg("emit")
        .arg(&input)
        .arg("--out")
        .arg(&output)
        .output()
        .unwrap();
    success(&result);
    assert!(result.stdout.is_empty());
    assert_eq!(fs::read(&output).unwrap(), first.stdout);
}

#[test]
fn emit_prefixes_mermaid_reserved_ids_and_already_prefixed_ids() {
    let source = "schemaVersion: 0.1.0\nid: sample\nname: Sample\nnodes:\n  - id: end\n  - id: n_end\nedges:\n  - from: end\n    to: n_end\n    label: uses\n";
    let output = run(&["emit", "-"], source);
    success(&output);
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("n_end[\"end\"]"));
    assert!(text.contains("n_n_end[\"n_end\"]"));
    assert!(text.contains("n_end -->|\"uses\"| n_n_end"));
}

#[cfg(unix)]
mod renderer {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn stub(path: &Path, exit_code: i32) {
        fs::write(
            path,
            format!(
                r#"#!/bin/sh
set -eu
test "$#" -eq 4
test "$1" = '-i'
test "$3" = '-o'
printf '%s\n' "$2" > "$SOURCE_LOG"
test -f "$2"
cp "$2" "$SOURCE_COPY"
if test {exit_code} -ne 0; then
    printf 'renderer deliberately failed\n' >&2
    exit {exit_code}
fi
printf 'stub diagram\n' > "$4"
"#
            ),
        )
        .unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn command(
        input: &Path,
        out: &Path,
        executable: &Path,
        temp: &Path,
        log: &Path,
        copy: &Path,
    ) -> Command {
        let mut command = cli();
        command
            .arg("render")
            .arg(input)
            .arg("--out")
            .arg(out)
            .arg("--mmdc")
            .arg(executable)
            .env("TMPDIR", temp)
            .env("TMP", temp)
            .env("TEMP", temp)
            .env("SOURCE_LOG", log)
            .env("SOURCE_COPY", copy);
        command
    }

    #[test]
    fn render_success_and_nonzero_exit_remove_source_tempfiles() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("input.yaml");
        let executable = dir.path().join("fake mmdc");
        let temp = dir.path().join("temp");
        let log = dir.path().join("source.log");
        let copy = dir.path().join("source.mmd");
        fs::create_dir(&temp).unwrap();
        fs::write(&input, YAML).unwrap();
        let emitted = run(&["emit", "-"], YAML);
        success(&emitted);
        for (extension, exit_code) in [("svg", 0), ("png", 0), ("pdf", 0), ("svg", 7)] {
            stub(&executable, exit_code);
            let out = dir.path().join(format!("diagram-{exit_code}.{extension}"));
            let result = command(&input, &out, &executable, &temp, &log, &copy)
                .output()
                .unwrap();
            if exit_code == 0 {
                success(&result);
                assert_eq!(fs::read_to_string(&out).unwrap(), "stub diagram\n");
            } else {
                failure(&result);
                assert!(String::from_utf8_lossy(&result.stderr)
                    .contains("renderer deliberately failed"));
                assert!(!out.exists());
            }
            let source = fs::read_to_string(&log).expect("stub invoked with source path");
            assert!(
                !Path::new(source.trim_end()).exists(),
                "temporary source leaked"
            );
            assert_eq!(fs::read(&copy).unwrap(), emitted.stdout);
            assert_eq!(fs::read_dir(&temp).unwrap().count(), 0);
        }
    }

    #[test]
    fn render_spawn_failure_cleans_up_tempfile() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("input.yaml");
        let temp = dir.path().join("temp");
        fs::create_dir(&temp).unwrap();
        fs::write(&input, YAML).unwrap();
        let out = dir.path().join("diagram.svg");
        let result = command(
            &input,
            &out,
            &dir.path().join("missing-mmdc"),
            &temp,
            &dir.path().join("log"),
            &dir.path().join("copy"),
        )
        .output()
        .unwrap();
        failure(&result);
        assert!(!result.stderr.is_empty());
        assert!(!out.exists());
        assert_eq!(
            fs::read_dir(&temp).unwrap().count(),
            0,
            "spawn failure leaked temporary source"
        );
    }

    #[test]
    fn render_rejects_unsupported_extensions_before_invoking_renderer() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("input.yaml");
        let executable = dir.path().join("fake-mmdc");
        let temp = dir.path().join("temp");
        let log = dir.path().join("source.log");
        fs::create_dir(&temp).unwrap();
        fs::write(&input, YAML).unwrap();
        stub(&executable, 0);
        for filename in ["diagram.txt", "diagram.mmd", "diagram"] {
            let out = dir.path().join(filename);
            let result = command(
                &input,
                &out,
                &executable,
                &temp,
                &log,
                &dir.path().join("copy"),
            )
            .output()
            .unwrap();
            failure(&result);
            assert!(!result.stderr.is_empty());
            assert!(
                !log.exists(),
                "renderer must not run for unsupported output"
            );
            assert!(!out.exists());
            assert_eq!(fs::read_dir(&temp).unwrap().count(), 0);
        }
    }
}
