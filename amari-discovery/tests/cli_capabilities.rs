// SPDX-License-Identifier: MIT OR Apache-2.0

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;

fn capabilities_json() -> Value {
    let output = Command::cargo_bin("amari")
        .unwrap()
        .args(["capabilities", "--json"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).unwrap()
}

#[test]
fn capabilities_json_self_describes_schema_and_exit_codes() {
    let value = capabilities_json();

    assert_eq!(value["schema_version"], "amari.discovery/v1");
    assert_eq!(value["data"]["binary"], "amari");
    assert!(value["data"]["tool_version"].is_string());
    assert_eq!(value["data"]["protocol_versions"][0], "amari.discovery/v1");
    assert_eq!(
        value["data"]["exit_codes"],
        serde_json::json!({
            "invalid_id": 2,
            "invalid_input": 2,
            "catalog_corruption": 3,
            "inspection_failure": 4,
            "probe_unavailable": 5,
            "probe_failed": 6,
            "limit_exceeded": 7,
            "io": 8,
            "serialization": 9,
            "not_implemented": 69,
            "internal": 70
        })
    );
    assert!(value["data"]["host"]["os"].is_string());
    assert!(value["data"]["host"]["source"].is_string());
    assert!(value["data"]["target"]["arch"].is_string());
    assert!(value["data"]["target"]["triple"].is_string());
    assert_eq!(value["data"]["target"]["source"], "cargo-target");
    assert!(value["data"]["feature_gates"].is_array());
    assert_eq!(
        value["data"]["ai_adapter"]["contract_compiled"],
        cfg!(feature = "ai")
    );
    assert_eq!(value["data"]["ai_adapter"]["provider_configured"], false);
    assert_eq!(value["data"]["ai_adapter"]["executable"], false);
}

#[test]
fn bootstrap_capabilities_do_not_forecast_catalog_or_probe_execution() {
    let value = capabilities_json();

    assert_eq!(value["data"]["catalog"]["version"], "bootstrap");
    assert_eq!(value["data"]["catalog"]["available"], false);
    assert_eq!(
        value["provenance"]["catalog"]["hash"],
        value["data"]["catalog"]["hash"]
    );
    assert_eq!(value["data"]["known_probes"].as_array().unwrap().len(), 0);

    for inspector in value["data"]["project_inspectors"].as_array().unwrap() {
        assert!(inspector["known"].is_boolean());
        assert_eq!(inspector["available"], false);
        assert_eq!(inspector["executable"], false);
    }

    assert_eq!(
        value["data"]["output_modes"],
        serde_json::json!(["human", "json"])
    );
    assert!(value["data"]["resource_limits"]["max_inspection_files"].is_number());
    assert!(value["data"]["resource_limits"]["max_probe_output_bytes"].is_number());
}

#[test]
fn feature_gates_report_the_compiled_binary() {
    let value = capabilities_json();
    let features = value["data"]["feature_gates"].as_array().unwrap();
    let standard = features
        .iter()
        .find(|feature| feature["name"] == "standard-probes")
        .unwrap();
    let ai = features
        .iter()
        .find(|feature| feature["name"] == "ai")
        .unwrap();

    assert_eq!(standard["compiled"], cfg!(feature = "standard-probes"));
    assert_eq!(ai["compiled"], cfg!(feature = "ai"));
}

#[test]
fn capabilities_human_output_is_concise() {
    Command::cargo_bin("amari")
        .unwrap()
        .arg("capabilities")
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains("Amari Discovery"))
        .stdout(predicate::str::contains("Project inspectors"))
        .stdout(predicate::str::contains("Catalog: bootstrap"))
        .stdout(predicate::str::contains("schema_version").not());
}

#[test]
fn help_exposes_the_approved_command_families() {
    for command in [
        "capabilities",
        "discover",
        "inspect",
        "recommend",
        "plan",
        "probe",
        "shell",
        "schema",
    ] {
        Command::cargo_bin("amari")
            .unwrap()
            .args([command, "--help"])
            .assert()
            .success();
    }

    for command in ["search", "detail", "graph", "example"] {
        Command::cargo_bin("amari")
            .unwrap()
            .args(["discover", command, "--help"])
            .assert()
            .success();
    }
    for command in ["list", "describe", "run"] {
        Command::cargo_bin("amari")
            .unwrap()
            .args(["probe", command, "--help"])
            .assert()
            .success();
    }
}

#[test]
fn unavailable_commands_use_a_typed_non_internal_failure() {
    Command::cargo_bin("amari")
        .unwrap()
        .args(["discover", "search", "tropical", "--json"])
        .assert()
        .code(69)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("not_implemented"))
        .stderr(predicate::str::contains("internal failure").not());
}

#[test]
fn clap_enforces_required_replay_and_input_arguments() {
    for args in [
        vec!["recommend"],
        vec!["plan", "candidate"],
        vec!["probe", "run", "amari-probe:test:test:v1"],
    ] {
        Command::cargo_bin("amari")
            .unwrap()
            .args(args)
            .assert()
            .code(2);
    }
}
