// SPDX-License-Identifier: MIT OR Apache-2.0

//! Cross-command JSON/NDJSON agent output contract before shell integration.

use std::{fs, path::Path};

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::TempDir;

fn output(arguments: &[&str], mode: &str) -> Vec<u8> {
    Command::cargo_bin("amari")
        .unwrap()
        .args(arguments)
        .arg(mode)
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .get_output()
        .stdout
        .clone()
}

fn machine_parity(arguments: &[&str]) -> Value {
    let json_bytes = output(arguments, "--json");
    let ndjson_bytes = output(arguments, "--ndjson");
    assert_eq!(
        ndjson_bytes.iter().filter(|byte| **byte == b'\n').count(),
        1
    );
    let json: Value = serde_json::from_slice(&json_bytes).unwrap();
    let ndjson: Value = serde_json::from_slice(&ndjson_bytes).unwrap();
    assert_eq!(json, ndjson, "machine mode drift for {arguments:?}");
    assert_eq!(json["schema_version"], "amari.discovery/v1");
    assert!(json["provenance"]["catalog"]["hash"].is_string());
    json
}

fn project() -> TempDir {
    let temporary = tempfile::tempdir().unwrap();
    fs::create_dir(temporary.path().join("src")).unwrap();
    fs::write(
        temporary.path().join("Cargo.toml"),
        r#"[package]
name = "agent-contract-fixture"
version = "0.1.0"
edition = "2021"

[dependencies]
amari-core = "0.23.0"
"#,
    )
    .unwrap();
    fs::write(
        temporary.path().join("src/lib.rs"),
        "use amari_core::Multivector;\npub fn scalar() -> Multivector<3,0,0> { Multivector::scalar(1.0) }\n",
    )
    .unwrap();
    temporary
}

fn save_recommendation(project: &Path) -> (TempDir, String, std::path::PathBuf) {
    let artifact_dir = tempfile::tempdir().unwrap();
    let recommendation = machine_parity(&[
        "recommend",
        project.to_str().unwrap(),
        "--goal",
        "compute a geometric product",
    ]);
    let candidate = recommendation["data"]["data"]["preferred"]["capability_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let path = artifact_dir.path().join("recommendation.json");
    fs::write(&path, serde_json::to_vec(&recommendation).unwrap()).unwrap();
    (artifact_dir, candidate, path)
}

#[test]
fn all_schema_selections_have_human_json_and_ndjson_contracts() {
    for kind in ["request", "response", "goal", "plan", "probe"] {
        let value = machine_parity(&["schema", kind]);
        assert_eq!(value["data"]["kind"], kind);
        assert_eq!(value["data"]["protocol_version"], "amari.discovery/v1");
        assert!(value["data"]["document"]["$id"].is_string());

        Command::cargo_bin("amari")
            .unwrap()
            .args(["schema", kind])
            .assert()
            .success()
            .stderr(predicate::str::is_empty())
            .stdout(predicate::str::contains("Schema:"))
            .stdout(predicate::str::contains("industrialalgebra.com/schemas"));
    }

    let listed = machine_parity(&["schema"]);
    assert_eq!(listed["data"]["schemas"].as_array().unwrap().len(), 5);
}

#[test]
fn existing_one_shot_command_families_share_json_ndjson_envelopes() {
    let project = project();
    machine_parity(&["capabilities"]);
    machine_parity(&["discover", "search", "tropical"]);
    machine_parity(&["inspect", project.path().to_str().unwrap()]);
    machine_parity(&["probe", "list"]);
    machine_parity(&["schema", "request"]);

    let (_artifact_dir, candidate, recommendation) = save_recommendation(project.path());
    machine_parity(&[
        "plan",
        &candidate,
        "--recommendation",
        recommendation.to_str().unwrap(),
        "--project",
        project.path().to_str().unwrap(),
    ]);
}

#[test]
fn capabilities_advertise_all_three_output_modes() {
    let value = machine_parity(&["capabilities"]);
    assert_eq!(
        value["data"]["output_modes"],
        serde_json::json!(["human", "json", "ndjson"])
    );
}

#[test]
fn machine_errors_are_single_structured_stderr_records_with_stable_codes() {
    let mut expected = None;
    for mode in ["--json", "--ndjson"] {
        let output = Command::cargo_bin("amari")
            .unwrap()
            .args(["discover", "detail", "amari:unknown:missing:value", mode])
            .assert()
            .failure()
            .code(2)
            .stdout(predicate::str::is_empty())
            .get_output()
            .stderr
            .clone();
        assert_eq!(output.iter().filter(|byte| **byte == b'\n').count(), 1);
        let error: Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(error["kind"], "invalid_id");
        assert_eq!(error["details"]["exit_code"], 2);
        if let Some(expected) = &expected {
            assert_eq!(&error, expected);
        } else {
            expected = Some(error);
        }
    }

    let capabilities = machine_parity(&["capabilities"]);
    assert_eq!(capabilities["data"]["exit_codes"]["invalid_input"], 2);
    assert_eq!(capabilities["data"]["exit_codes"]["probe_failed"], 6);
    assert_eq!(capabilities["data"]["exit_codes"]["internal"], 70);
}

#[test]
fn json_and_ndjson_are_mutually_exclusive_and_shell_remains_deferred() {
    Command::cargo_bin("amari")
        .unwrap()
        .args(["capabilities", "--json", "--ndjson"])
        .assert()
        .failure()
        .code(2)
        .stdout(predicate::str::is_empty());

    for mode in ["--json", "--ndjson"] {
        let output = Command::cargo_bin("amari")
            .unwrap()
            .args(["shell", mode])
            .assert()
            .failure()
            .code(69)
            .stdout(predicate::str::is_empty())
            .get_output()
            .stderr
            .clone();
        let error: Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(error["kind"], "not_implemented");
        assert_eq!(error["details"]["exit_code"], 69);
    }
}
