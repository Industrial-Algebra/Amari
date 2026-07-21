// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::{json, Value};
use tempfile::TempDir;

fn request(command: &str, arguments: Value) -> Value {
    json!({
        "schema_version": "amari.discovery/v1",
        "command": command,
        "arguments": arguments,
    })
}

fn one_shot(arguments: &[&str]) -> Value {
    let bytes = Command::cargo_bin("amari")
        .unwrap()
        .args(arguments)
        .arg("--json")
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&bytes).unwrap()
}

fn rust_project() -> TempDir {
    let project = tempfile::tempdir().unwrap();
    fs::create_dir(project.path().join("src")).unwrap();
    fs::write(
        project.path().join("Cargo.toml"),
        r#"[package]
name = "shell-agent-rust"
version = "0.1.0"
edition = "2021"

[dependencies]
amari-core = "0.23.0"
"#,
    )
    .unwrap();
    fs::write(
        project.path().join("src/lib.rs"),
        "use amari_core::Multivector;\npub fn scalar() -> Multivector<3,0,0> { Multivector::scalar(1.0) }\n",
    )
    .unwrap();
    project
}

fn npm_project() -> TempDir {
    let project = tempfile::tempdir().unwrap();
    fs::write(
        project.path().join("package.json"),
        r#"{"name":"shell-agent-npm","version":"0.1.0","dependencies":{"@justinelliottcobb/amari-wasm":"0.23.0"}}"#,
    )
    .unwrap();
    project
}

#[test]
fn json_mode_pairs_one_typed_request_with_the_one_shot_response() {
    let input = serde_json::to_vec(&request("capabilities", json!({}))).unwrap();
    let output = Command::cargo_bin("amari")
        .unwrap()
        .args(["shell", "--json"])
        .write_stdin(input)
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .get_output()
        .stdout
        .clone();

    assert_eq!(output.iter().filter(|byte| **byte == b'\n').count(), 1);
    assert_eq!(
        serde_json::from_slice::<Value>(&output).unwrap(),
        one_shot(&["capabilities"])
    );
}

#[test]
fn ndjson_mode_emits_exactly_one_shared_envelope_per_request_line() {
    let requests = [
        request("capabilities", json!({})),
        request("discover.search", json!({"query": "tropical"})),
        request("probe.list", json!({})),
    ];
    let mut input = requests
        .iter()
        .map(|value| serde_json::to_string(value).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    input.push('\n');
    let output = Command::cargo_bin("amari")
        .unwrap()
        .args(["shell", "--ndjson"])
        .write_stdin(input)
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .get_output()
        .stdout
        .clone();

    let records: Vec<Value> = output
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).unwrap())
        .collect();
    assert_eq!(records.len(), requests.len());
    assert_eq!(records[0], one_shot(&["capabilities"]));
    assert_eq!(records[1], one_shot(&["discover", "search", "tropical"]));
    assert_eq!(records[2], one_shot(&["probe", "list"]));
}

#[test]
fn shell_modes_share_session_project_semantics_and_allow_explicit_override() {
    let rust = rust_project();
    let npm = npm_project();
    let lines = [
        request("inspect", json!({})),
        request("inspect", json!({"path": npm.path().to_str().unwrap()})),
    ];
    let input = lines
        .iter()
        .map(|line| serde_json::to_string(line).unwrap())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let output = Command::cargo_bin("amari")
        .unwrap()
        .args([
            "shell",
            "--project",
            rust.path().to_str().unwrap(),
            "--ndjson",
        ])
        .write_stdin(input)
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .get_output()
        .stdout
        .clone();
    let records: Vec<Value> = output
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).unwrap())
        .collect();
    assert_eq!(records[0]["data"]["project_kind"], "rust_cargo");
    assert_eq!(records[1]["data"]["project_kind"], "npm_type_script");

    let json = Command::cargo_bin("amari")
        .unwrap()
        .args([
            "shell",
            "--project",
            rust.path().to_str().unwrap(),
            "--json",
        ])
        .write_stdin(serde_json::to_vec(&request("inspect", json!({}))).unwrap())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&json).unwrap();
    assert_eq!(json, records[0]);

    let human = Command::cargo_bin("amari")
        .unwrap()
        .args(["shell", "--project", rust.path().to_str().unwrap()])
        .write_stdin("inspect\nexit\n")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let human = String::from_utf8(human).unwrap();
    assert!(human.contains("Rust/Cargo project"));
    assert!(human.contains(json["data"]["project_hash"].as_str().unwrap()));
}

#[test]
fn machine_errors_stay_on_stderr_with_stable_exit_semantics() {
    let input = request("inspect", json!({"unknown": true}));
    let output = Command::cargo_bin("amari")
        .unwrap()
        .args(["shell", "--ndjson"])
        .write_stdin(serde_json::to_string(&input).unwrap() + "\n")
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .get_output()
        .stderr
        .clone();
    assert_eq!(output.iter().filter(|byte| **byte == b'\n').count(), 1);
    let error: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(error["kind"], "invalid_input");
    assert_eq!(error["details"]["exit_code"], 2);

    let missing = tempfile::tempdir().unwrap().path().join("missing");
    let output = Command::cargo_bin("amari")
        .unwrap()
        .args(["shell", "--project", missing.to_str().unwrap(), "--json"])
        .write_stdin(serde_json::to_vec(&request("inspect", json!({}))).unwrap())
        .assert()
        .code(4)
        .stdout(predicate::str::is_empty())
        .get_output()
        .stderr
        .clone();
    let error: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(error["kind"], "inspection_failure");
    assert_eq!(error["details"]["exit_code"], 4);
}

#[test]
fn json_mode_rejects_unpaired_trailing_requests_and_machine_modes_have_no_prompt() {
    let encoded = serde_json::to_string(&request("capabilities", json!({}))).unwrap();
    Command::cargo_bin("amari")
        .unwrap()
        .args(["shell", "--json"])
        .write_stdin(format!("{encoded}\n{encoded}\n"))
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("invalid_input"));

    let output = Command::cargo_bin("amari")
        .unwrap()
        .args(["shell", "--ndjson"])
        .write_stdin(serde_json::to_string(&request("capabilities", json!({}))).unwrap() + "\n")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(!String::from_utf8(output).unwrap().contains("amari>"));
}
