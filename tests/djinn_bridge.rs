//! Optional real-executable bridge, with a loopback-only synthetic provider.
//! Run with IDEOMANCER_DJINN_BIN=/absolute/path/to/built/djinn
//! cargo test --locked --test djinn_bridge -- --ignored
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime};
use tempfile::tempdir;

fn cli(temp: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ideomancer"));
    // No inherited credentials, proxy settings, or provider configuration.
    command
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", temp.join("home"))
        .env("XDG_CONFIG_HOME", temp.join("config"))
        .env("XDG_DATA_HOME", temp.join("data"))
        .current_dir(temp)
        .stdin(Stdio::null());
    command
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

fn json_success(output: &Output) -> Value {
    success(output);
    serde_json::from_slice(&output.stdout).expect("one JSON status document")
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn proposal(evidence: &Value) -> Value {
    let modules: Vec<_> = evidence["facts"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|fact| fact["kind"] == "module")
        .map(|fact| fact["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        modules.len(),
        2,
        "expected the two synthetic source modules"
    );
    json!({
        "schemaVersion": "ideomancer.architecture.v1",
        "snapshotId": evidence["snapshotId"],
        "name": "Synthetic Bridge Architecture",
        "components": [
            {"id": "api", "label": "API", "kind": "component",
             "evidenceIds": [modules[0]], "rationale": "Synthetic module grouping."},
            {"id": "storage", "label": "Storage", "kind": "component",
             "evidenceIds": [modules[1]], "rationale": "Synthetic module grouping."}
        ],
        "relationships": [
            {"from": "api", "to": "storage", "label": "uses", "basis": "inferred",
             "evidenceIds": modules, "rationale": "Synthetic grouping, not a runtime trace."}
        ],
        "uncertainties": ["This proposal is a synthetic bridge fixture."]
    })
}

fn artifacts(run: &Path) -> BTreeMap<PathBuf, (Vec<u8>, SystemTime)> {
    let mut result = BTreeMap::new();
    let mut pending = vec![run.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                assert!(path.is_file(), "unexpected artifact: {path:?}");
                let bytes = fs::read(&path).unwrap();
                let modified = fs::metadata(&path).unwrap().modified().unwrap();
                result.insert(path, (bytes, modified));
            }
        }
    }
    result
}

#[test]
#[ignore = "requires IDEOMANCER_DJINN_BIN pointing to a built Djinn with execute support and trusted Python >=3.12"]
fn real_djinn_bridge_is_offline_reviewed_and_idempotent() {
    real_djinn_bridge(false, false, false);
}

#[test]
#[ignore = "requires IDEOMANCER_DJINN_BIN pointing to a built Djinn with execute support and trusted Python >=3.12"]
fn real_djinn_analyze_drafts_without_automatic_acceptance() {
    real_djinn_bridge(true, false, false);
}

#[test]
#[ignore = "requires IDEOMANCER_DJINN_BIN pointing to a built Djinn with stored OAuth/Codex SSE support and trusted Python >=3.12"]
fn real_djinn_analyze_uses_global_opencode_oauth_and_codex_sse() {
    real_djinn_bridge(true, true, false);
}

#[test]
#[ignore = "requires IDEOMANCER_DJINN_BIN pointing to a built Djinn with execute support and trusted Python >=3.12"]
fn real_djinn_focused_analyze_sends_saved_packet_and_caches_draft() {
    real_djinn_bridge(true, false, true);
}

fn real_djinn_bridge(analyze: bool, oauth: bool, focused: bool) {
    let djinn = PathBuf::from(std::env::var_os("IDEOMANCER_DJINN_BIN").expect(
        "set IDEOMANCER_DJINN_BIN=/absolute/path/to/built/djinn, then run \
         cargo test --locked --test djinn_bridge -- --ignored; this test never builds Djinn",
    ));
    assert!(
        djinn.is_absolute() && djinn.is_file(),
        "IDEOMANCER_DJINN_BIN must point to an existing absolute built Djinn executable: {djinn:?}"
    );
    let temp = tempdir().unwrap();
    for directory in ["home", "config/djinn", "config/opencode", "data/opencode"] {
        fs::create_dir_all(temp.path().join(directory)).unwrap();
    }
    let auth = json!({"openai": {"type": "oauth", "access": "fixture-access",
        "refresh": "fixture-refresh", "expires": 4102444800000_i64,
        "accountId": "fixture-account"}});
    if oauth {
        fs::write(
            temp.path().join("data/opencode/auth.json"),
            auth.to_string(),
        )
        .unwrap();
        fs::write(
            temp.path().join("config/opencode/opencode.jsonc"),
            r#"{
            // The analyze default overrides this synthetic global model selection.
            "default_agent": "fixture",
            "agent": {"fixture": {"model": "openai/fixture-codex-model"}},
        }"#,
        )
        .unwrap();
    }
    let python = std::env::var_os("IDEOMANCER_TEST_PYTHON").unwrap_or_else(|| "python3.12".into());
    let checked = Command::new(python)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", temp.path().join("home"))
        .env("XDG_CONFIG_HOME", temp.path().join("config"))
        .env("XDG_DATA_HOME", temp.path().join("data"))
        .current_dir(temp.path())
        .args([
            "-I",
            "-c",
            "import sys; assert sys.version_info >= (3, 12); print(sys.executable)",
        ])
        .output()
        .expect("install trusted Python >=3.12 or set IDEOMANCER_TEST_PYTHON");
    success(&checked);
    let python = PathBuf::from(String::from_utf8(checked.stdout).unwrap().trim());
    assert!(python.is_absolute());

    let root = temp.path().join("synthetic source");
    let run = temp.path().join("run");
    let marker = temp.path().join("SOURCE_MUST_NOT_EXECUTE");
    fs::create_dir(&root).unwrap();
    fs::write(
        root.join("api.py"),
        format!(
            "from pathlib import Path\nPath({}).write_text('executed')\n\
             raise RuntimeError('source module must never execute')\n\
             import storage\ndef serve():\n    return {}\n{}",
            serde_json::to_string(marker.to_str().unwrap()).unwrap(),
            if focused {
                "helper()"
            } else {
                "storage.load()"
            },
            if focused {
                "def helper():\n    return storage.load()\ndef unrelated():\n    return 99\n"
            } else {
                ""
            }
        ),
    )
    .unwrap();
    fs::write(root.join("storage.py"), "def load():\n    return 42\n").unwrap();
    if !analyze {
        let extracted = json_success(
            &cli(temp.path())
                .arg("inspect")
                .arg(&root)
                .arg("--out")
                .arg(&run)
                .arg("--python")
                .arg(&python)
                .output()
                .unwrap(),
        );
        assert_eq!(extracted["status"], "extracted");
        assert_eq!(extracted["complete"], true);
        assert!(!marker.exists(), "inspect executed source");
    }

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
    listener.set_nonblocking(true).unwrap();
    let (stop, stopped) = mpsc::channel();
    let server = thread::spawn(move || {
        const HEADER_CAP: usize = 16 * 1024;
        const BODY_CAP: usize = 1024 * 1024;
        let deadline = Instant::now() + Duration::from_secs(45);
        let mut requests = Vec::new();
        // Keep SSE sockets open until the CLI exits: terminal events, not EOF, finish responses.
        let mut open_streams = Vec::new();
        loop {
            match listener.accept() {
                Ok((mut stream, peer)) => {
                    assert!(peer.ip().is_loopback());
                    stream.set_nonblocking(false).unwrap();
                    stream
                        .set_read_timeout(Some(Duration::from_secs(5)))
                        .unwrap();
                    stream
                        .set_write_timeout(Some(Duration::from_secs(5)))
                        .unwrap();
                    let mut header = Vec::new();
                    while !header.ends_with(b"\r\n\r\n") {
                        assert!(header.len() < HEADER_CAP, "HTTP headers exceeded cap");
                        let mut byte = [0];
                        stream
                            .read_exact(&mut byte)
                            .expect("bounded HTTP header read");
                        header.push(byte[0]);
                    }
                    let header = String::from_utf8(header).unwrap();
                    let mut lines = header.lines();
                    assert_eq!(
                        lines.next(),
                        Some(if oauth {
                            "POST /v1/codex HTTP/1.1"
                        } else {
                            "POST /v1/chat/completions HTTP/1.1"
                        })
                    );
                    let mut length = None;
                    let mut authorization = None;
                    let mut account = None;
                    let mut originator = None;
                    for line in lines.filter(|line| !line.is_empty()) {
                        let (name, value) = line.split_once(':').expect("HTTP header field");
                        if name.eq_ignore_ascii_case("content-length") {
                            assert!(length.is_none(), "duplicate Content-Length");
                            length = Some(value.trim().parse::<usize>().unwrap());
                        }
                        assert!(!name.eq_ignore_ascii_case("transfer-encoding"));
                        if name.eq_ignore_ascii_case("authorization") {
                            authorization = Some(value.trim());
                        }
                        if name.eq_ignore_ascii_case("chatgpt-account-id") {
                            account = Some(value.trim());
                        }
                        if name.eq_ignore_ascii_case("originator") {
                            originator = Some(value.trim());
                        }
                    }
                    assert_eq!(
                        authorization,
                        Some(if oauth {
                            "Bearer fixture-access"
                        } else {
                            "Bearer synthetic-bridge-key"
                        })
                    );
                    if oauth {
                        assert_eq!(account, Some("fixture-account"));
                        assert_eq!(originator, Some("opencode"));
                    }
                    let length = length.expect("Content-Length required");
                    assert!((1..=BODY_CAP).contains(&length), "HTTP body exceeded cap");
                    let mut body = vec![0; length];
                    stream
                        .read_exact(&mut body)
                        .expect("bounded HTTP body read");
                    let request: Value = serde_json::from_slice(&body).unwrap();
                    assert_eq!(
                        request["model"],
                        if oauth { "gpt-6.1-sol" } else { "bridge-test" }
                    );
                    assert!(
                        request
                            .get("tools")
                            .is_none_or(|tools| { tools.as_array().is_some_and(Vec::is_empty) }),
                        "execution must be tool-free"
                    );
                    if oauth {
                        assert_eq!(request["tools"], json!([]));
                        assert_eq!(request["tool_choice"], "none");
                        assert_eq!(request["store"], false);
                        assert_eq!(request["stream"], true);
                        let input = request["input"].as_array().unwrap();
                        assert_eq!(input.len(), 1);
                        assert_eq!(input[0]["role"], "user");
                        let content = input[0]["content"].as_array().unwrap();
                        assert_eq!(content.len(), 1);
                        assert_eq!(content[0]["type"], "input_text");
                        let evidence: Value =
                            serde_json::from_str(content[0]["text"].as_str().unwrap()).unwrap();
                        let text = proposal(&evidence).to_string();
                        let events = [
                            json!({"type": "response.metadata", "response": {"model": "actual-fixture-codex"}}),
                            json!({"type": "response.output_text.delta", "output_index": 0, "content_index": 0, "delta": text}),
                            json!({"type": "response.output_text.done", "output_index": 0, "content_index": 0, "text": text}),
                            json!({"type": "response.completed", "response": {"status": "completed", "model": "actual-fixture-codex", "output": []}}),
                        ];
                        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: keep-alive\r\n\r\n").unwrap();
                        for event in events {
                            write!(
                                stream,
                                "event: {}\ndata: {event}\n\n",
                                event["type"].as_str().unwrap()
                            )
                            .unwrap();
                        }
                        stream.flush().unwrap();
                        requests.push(request);
                        open_streams.push(stream);
                        continue;
                    }
                    let users: Vec<_> = request["messages"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter(|message| message["role"] == "user")
                        .collect();
                    assert_eq!(users.len(), 1);
                    let input: Value =
                        serde_json::from_str(users[0]["content"].as_str().unwrap()).unwrap();
                    let response = json!({
                        "model": "mock-bridge-v1",
                        "choices": [{"index": 0, "finish_reason": "stop",
                            "message": {"role": "assistant", "content": proposal(&input).to_string()}}]
                    }).to_string();
                    requests.push(request);
                    write!(stream,
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        response.len(), response).unwrap();
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    // Drain pending connections before honoring shutdown so extra calls count.
                    match stopped.try_recv() {
                        Ok(()) | Err(mpsc::TryRecvError::Disconnected) => break,
                        Err(mpsc::TryRecvError::Empty) => {}
                    }
                    assert!(Instant::now() < deadline, "bridge server deadline elapsed");
                    thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("loopback accept: {error}"),
            }
        }
        requests
    });
    let interpret = |run: &Path| {
        let mut command = cli(temp.path());
        if analyze {
            command
                .arg("analyze")
                .arg(&root)
                .arg("--out")
                .arg(run)
                .arg("--python")
                .arg(&python)
                .arg("--no-render");
            if focused {
                command.args(["--focus", "serve"]);
            }
        } else {
            command.arg("interpret").arg(run);
        }
        if oauth {
            command.env("DJINN_CODEX_API_ENDPOINT", format!("{endpoint}/codex"));
        } else {
            command
                .args(["--model", "bridge-test"])
                .env("OPENAI_API_KEY", "synthetic-bridge-key")
                .env("OPENAI_BASE_URL", &endpoint);
        }
        command
            .arg("--agent")
            .arg(&djinn)
            .args(["--timeout-seconds", "10"])
            .output()
            .unwrap()
    };
    let output = interpret(&run);
    for secret in ["fixture-access", "fixture-refresh", "fixture-account"] {
        assert!(
            !String::from_utf8_lossy(&output.stdout).contains(secret),
            "stdout leaked fixture credential"
        );
        assert!(
            !String::from_utf8_lossy(&output.stderr).contains(secret),
            "stderr leaked fixture credential"
        );
    }
    let ready = json_success(&output);
    let base = run;
    let run = if analyze {
        let run = PathBuf::from(ready["runDirectory"].as_str().unwrap());
        assert!(run.is_absolute());
        assert!(run.starts_with(fs::canonicalize(&base).unwrap()));
        assert_ne!(run, fs::canonicalize(&base).unwrap());
        assert_eq!(ready["requiresAcceptance"], true);
        assert_eq!(ready["complete"], true);
        for (field, name) in [
            ("draftManifest", "presentation/draft.architecture.json"),
            ("draftMermaid", "presentation/diagram.mmd"),
            ("draftLegend", "presentation/legend.json"),
            ("draftViewer", "presentation/index.html"),
        ] {
            assert_eq!(ready[field], run.join(name).to_str().unwrap());
            assert!(run.join(name).is_file());
        }
        let draft = read_json(&run.join("presentation/draft.architecture.json"));
        assert!(draft["name"].as_str().unwrap().starts_with("DRAFT"));
        assert_eq!(ready["presentationVersion"], "readable-v2");
        let mermaid = fs::read_to_string(ready["draftMermaid"].as_str().unwrap()).unwrap();
        let (yaml, body) = mermaid
            .strip_prefix("---\n")
            .unwrap()
            .split_once("\n---\n")
            .unwrap();
        let metadata: Value = serde_yaml::from_str(yaml).unwrap();
        assert!(metadata["title"].as_str().unwrap().contains("DRAFT"));
        assert!(body.contains("flowchart"));
        assert!(ready["draftSvg"].is_null());
        assert!(!run.join("draft.svg").exists());
        run
    } else {
        base.clone()
    };
    let evidence = read_json(&run.join("evidence.json"));
    let input = if focused {
        read_json(&run.join("evidence-packet.json"))
    } else {
        evidence.clone()
    };
    let expected = proposal(&input);
    let state = read_json(&run.join("state.json"));
    assert_eq!(ready["status"], "proposal_ready");
    assert_eq!(state["status"], "proposal_ready");
    assert_eq!(
        state["modelRequested"],
        if oauth {
            json!("openai/gpt-6.1-sol")
        } else {
            json!("bridge-test")
        }
    );
    assert_eq!(
        state["modelReported"],
        if oauth {
            "actual-fixture-codex"
        } else {
            "mock-bridge-v1"
        }
    );
    assert_eq!(state["attempts"], 1);
    let request = read_json(&run.join("attempt-0001.request.json"));
    let receipt = read_json(&run.join("attempt-0001.result.json"));
    if focused {
        assert_eq!(request["protocolVersion"], "agent-execution.v1");
        assert_eq!(request["input"], input);
        assert_ne!(input, evidence, "focused input must not be full inventory");
        let options = json!({"symbol": "serve", "hops": 3, "maxFacts": 600});
        assert_eq!(state["focus"], options);
        assert_eq!(ready["focus"], options);
        assert_eq!(state["packetPolicyVersion"], "architecture-focus.v1");
        let skill = cli(temp.path())
            .args(["skill", "--focus"])
            .output()
            .unwrap();
        success(&skill);
        assert_eq!(
            request["instructions"],
            String::from_utf8(skill.stdout).unwrap()
        );
        let focus = &request["input"]["focus"];
        assert_eq!(focus["policyVersion"], "architecture-focus.v1");
        assert_eq!(focus["requestedSymbol"], "serve");
        assert_eq!(focus["maxHops"], 3);
        assert_eq!(focus["maxFacts"], 600);
        let facts = input["facts"].as_array().unwrap();
        assert!(!facts.is_empty() && facts.len() <= 600);
        assert!(facts.len() < evidence["facts"].as_array().unwrap().len());
        assert!(serde_json::to_vec(&input).unwrap().len() <= 192 * 1024);
        let anchor = facts
            .iter()
            .find(|fact| fact["id"] == focus["anchor"]["id"])
            .unwrap();
        assert_eq!(anchor["kind"], "function");
        assert_eq!(anchor["symbol"], "api.serve");
        assert_eq!(anchor["path"], "api.py");
        for field in ["id", "symbol", "path", "line", "endLine"] {
            assert_eq!(focus["anchor"][field], anchor[field]);
        }
        assert_eq!(
            focus["visitedSymbols"],
            json!(["api.helper", "api.serve", "storage.load"])
        );
        assert_eq!(focus["truncated"], false);
        assert!(focus["boundaries"].is_array());
        assert_eq!(focus["unresolved"], json!([]));
        assert!(!facts.iter().any(|fact| fact["symbol"] == "api.unrelated"));
        for fact in facts {
            assert!(evidence["facts"].as_array().unwrap().contains(fact));
        }
        // Candidate links describe static source evidence, not executed dispatch.
        let links = focus["links"].as_array().unwrap();
        assert_eq!(links.len(), 2);
        for link in links {
            assert_eq!(link["kind"], "local_call_candidate");
            for field in ["sourceFactId", "targetFactId"] {
                assert!(facts.iter().any(|fact| fact["id"] == link[field]));
            }
            for id in link["evidenceIds"].as_array().unwrap() {
                assert!(facts.iter().any(|fact| fact["id"] == *id));
            }
        }
        for (source, target) in [("api.serve", "api.helper"), ("api.helper", "storage.load")] {
            assert!(links.iter().any(|link| {
                facts.iter().any(|fact| {
                    fact["id"] == link["sourceFactId"]
                        && fact["kind"] == "call"
                        && fact["symbol"] == source
                }) && facts.iter().any(|fact| {
                    fact["id"] == link["targetFactId"]
                        && fact["kind"] == "function"
                        && fact["symbol"] == target
                })
            }));
        }
    }
    assert_eq!(request["timeoutMs"], 9000);
    assert_eq!(state["agentTimeoutMs"], request["timeoutMs"]);
    assert_eq!(receipt["timeoutMs"], request["timeoutMs"]);
    assert!(state["elapsedMs"].as_u64().is_some());
    assert_eq!(receipt["elapsedMs"], state["elapsedMs"]);
    if !analyze {
        assert_eq!(ready, state);
    }
    assert!(!run.join("architecture.json").exists());
    assert!(!run.join("diagram.mmd").exists());

    // Review the entire known proposal before accepting its exact byte fingerprint.
    let proposal_bytes = fs::read(run.join("proposal.json")).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&proposal_bytes).unwrap(),
        expected
    );
    let fingerprint = format!("{:x}", Sha256::digest(&proposal_bytes));
    assert_eq!(ready["proposalSha256"], fingerprint);
    assert_eq!(state["proposalSha256"], fingerprint);
    if oauth {
        assert!(!marker.exists(), "analyze executed source module body");
        for (_, (bytes, _)) in artifacts(&run) {
            for secret in ["fixture-access", "fixture-refresh", "fixture-account"] {
                assert!(
                    !String::from_utf8_lossy(&bytes).contains(secret),
                    "artifact leaked fixture credential"
                );
            }
        }
        assert_eq!(
            read_json(&temp.path().join("data/opencode/auth.json")),
            auth
        );
        assert_eq!(
            fs::read_dir(temp.path().join("config/djinn"))
                .unwrap()
                .count(),
            0
        );
        stop.send(()).expect("loopback server failed");
        let requests = server.join().expect("loopback provider assertions failed");
        assert_eq!(
            requests.len(),
            1,
            "analyze must make exactly one provider POST"
        );
        assert_eq!(
            serde_json::from_str::<Value>(
                requests[0]["input"][0]["content"][0]["text"]
                    .as_str()
                    .unwrap()
            )
            .unwrap(),
            evidence
        );
        return;
    }
    let reviewed = artifacts(&run);
    assert_eq!(json_success(&interpret(&base)), ready);
    assert_eq!(artifacts(&run), reviewed, "cached proposal was rewritten");
    if focused {
        assert!(!run.join("architecture.json").exists());
        assert!(!run.join("diagram.mmd").exists());
        assert!(
            !marker.exists(),
            "focused analyze executed source module body"
        );
        stop.send(()).expect("loopback server failed");
        let requests = server.join().expect("loopback provider assertions failed");
        assert_eq!(
            requests.len(),
            1,
            "cached focused analyze made an extra provider POST"
        );
        let messages = requests[0]["messages"].as_array().unwrap();
        let user = messages
            .iter()
            .find(|message| message["role"] == "user")
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(user["content"].as_str().unwrap()).unwrap(),
            input
        );
        let system = messages
            .iter()
            .find(|message| message["role"] == "system")
            .unwrap();
        assert_eq!(system["content"], request["instructions"]);
        return;
    }
    let accept = || {
        cli(temp.path())
            .arg("accept")
            .arg(&run)
            .arg("--proposal-sha256")
            .arg(&fingerprint)
            .output()
            .unwrap()
    };
    let accepted = json_success(&accept());
    assert_eq!(accepted["status"], "accepted");
    let architecture = run.join("architecture.json");
    let manifest = read_json(&architecture);
    assert_eq!(manifest["schemaVersion"], "0.1.0");
    assert_eq!(manifest["name"], expected["name"]);
    assert_eq!(manifest["nodes"].as_array().unwrap().len(), 2);
    assert_eq!(manifest["edges"].as_array().unwrap().len(), 1);
    let validation = json_success(
        &cli(temp.path())
            .arg("validate")
            .arg(&architecture)
            .arg("--json")
            .output()
            .unwrap(),
    );
    assert_eq!(validation["valid"], true);
    assert_eq!(validation["errors"], json!([]));
    let emitted = cli(temp.path())
        .arg("emit")
        .arg(&architecture)
        .output()
        .unwrap();
    success(&emitted);
    assert_eq!(emitted.stdout, fs::read(run.join("diagram.mmd")).unwrap());
    let immutable = artifacts(&run);
    if !analyze {
        assert_eq!(json_success(&interpret(&base)), accepted);
    }
    assert_eq!(json_success(&accept()), accepted);
    assert_eq!(read_json(&run.join("state.json"))["status"], "accepted");
    assert_eq!(
        artifacts(&run),
        immutable,
        "accepted artifacts were rewritten"
    );
    assert!(!marker.exists(), "workflow executed source module body");
    stop.send(()).expect("loopback server failed");
    let requests = server.join().expect("loopback provider assertions failed");
    assert_eq!(
        requests.len(),
        1,
        "the entire workflow must make exactly one provider POST"
    );
    let input: Value = serde_json::from_str(
        requests[0]["messages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|message| message["role"] == "user")
            .unwrap()["content"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        input, evidence,
        "provider must receive extracted input evidence"
    );
}
