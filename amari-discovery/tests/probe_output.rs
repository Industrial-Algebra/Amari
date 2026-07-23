// SPDX-License-Identifier: MIT OR Apache-2.0

//! Golden probe success/error output contracts across human and machine modes.

#![cfg(feature = "standard-probes")]

use std::{fs, process::Output};

use assert_cmd::Command;
use serde_json::{json, Value};
use tempfile::TempDir;

const VITERBI: &str = "amari-probe:tropical:viterbi:v1";
const UNKNOWN: &str = "amari-probe:tropical:does-not-exist:v1";

const RUN_JSON: &str = include_str!("golden/probe-output/probe-run.json");
const RUN_HUMAN: &str = include_str!("golden/probe-output/probe-run-human.txt");
const INVALID_JSON: &str = include_str!("golden/probe-output/probe-error-invalid.json");
const INVALID_HUMAN: &str = include_str!("golden/probe-output/probe-error-invalid-human.txt");
const FAILED_JSON: &str = include_str!("golden/probe-output/probe-error-failed.json");
const FAILED_HUMAN: &str = include_str!("golden/probe-output/probe-error-failed-human.txt");

fn viterbi_input() -> Value {
    json!({
        "transitions": [[-1.0, -2.0], [-2.0, -1.0]],
        "emissions": [[-1.0, -3.0], [-3.0, -1.0]],
        "observations": [0, 1, 0]
    })
}

fn non_finite_result_input() -> Value {
    json!({
        "transitions": [[1.7e308]],
        "emissions": [[1.7e308]],
        "observations": [0, 0]
    })
}

fn write_input(directory: &TempDir, name: &str, value: &Value) -> std::path::PathBuf {
    let path = directory.path().join(name);
    fs::write(&path, serde_json::to_vec(value).unwrap()).unwrap();
    path
}

fn probe_run(path: &std::path::Path, mode: Option<&str>) -> Output {
    let mut command = Command::cargo_bin("amari").unwrap();
    command.args(["probe", "run", VITERBI, "--input"]).arg(path);
    if let Some(mode) = mode {
        command.arg(mode);
    }
    command.output().unwrap()
}

fn probe_describe_unknown(mode: Option<&str>) -> Output {
    let mut command = Command::cargo_bin("amari").unwrap();
    command.args(["probe", "describe", UNKNOWN]);
    if let Some(mode) = mode {
        command.arg(mode);
    }
    command.output().unwrap()
}

fn pretty_json(bytes: &[u8]) -> String {
    let value: Value = serde_json::from_slice(bytes).unwrap();
    format!("{}\n", serde_json::to_string_pretty(&value).unwrap())
}

fn normalize_success_json(bytes: &[u8]) -> (String, Value) {
    let mut value: Value = serde_json::from_slice(bytes).unwrap();
    value["provenance"]["tool_version"] = json!("<tool_version>");
    value["provenance"]["catalog"]["version"] = json!("<catalog_version>");
    value["provenance"]["catalog"]["hash"] = json!("<catalog_hash>");
    value["data"]["result"]["catalog_hash"] = json!("<catalog_hash>");
    value["data"]["result"]["duration_micros"] = json!(0);
    (
        format!("{}\n", serde_json::to_string_pretty(&value).unwrap()),
        value,
    )
}

fn normalize_human(bytes: &[u8]) -> String {
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    let mut normalized = String::new();
    for line in text.lines() {
        if line.starts_with("Duration (micros): ") {
            normalized.push_str("Duration (micros): <duration_micros>\n");
        } else if line.starts_with("Tool version: ") {
            normalized.push_str("Tool version: <tool_version>\n");
        } else if line.starts_with("Catalog version: ") {
            normalized.push_str("Catalog version: <catalog_version>\n");
        } else if line.starts_with("Catalog hash: ") {
            normalized.push_str("Catalog hash: <catalog_hash>\n");
        } else {
            normalized.push_str(line);
            normalized.push('\n');
        }
    }
    normalized
}

#[test]
fn successful_probe_human_json_and_ndjson_match_goldens_and_each_other() {
    let temporary = tempfile::tempdir().unwrap();
    let input = write_input(&temporary, "viterbi.json", &viterbi_input());

    let human = probe_run(&input, None);
    assert!(human.status.success());
    assert!(human.stderr.is_empty());
    assert_eq!(normalize_human(&human.stdout), RUN_HUMAN);

    let json_output = probe_run(&input, Some("--json"));
    let ndjson_output = probe_run(&input, Some("--ndjson"));
    for output in [&json_output, &ndjson_output] {
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        assert_eq!(
            output.stdout.iter().filter(|byte| **byte == b'\n').count(),
            1
        );
    }

    let (json_normalized, json_value) = normalize_success_json(&json_output.stdout);
    let (ndjson_normalized, ndjson_value) = normalize_success_json(&ndjson_output.stdout);
    assert_eq!(json_normalized, RUN_JSON);
    assert_eq!(ndjson_normalized, RUN_JSON);
    assert_eq!(json_value, ndjson_value);

    assert_eq!(json_value["schema_version"], "amari.discovery/v1");
    assert_eq!(json_value["data"]["isolation"], "process");
    assert_eq!(json_value["data"]["hard_timeout"], true);
    assert_eq!(json_value["data"]["crash_isolation"], true);
    assert!(json_value["provenance"]["catalog"]["hash"].is_string());
    assert!(json_value["provenance"]["input_hash"].is_string());
    assert!(json_value["data"]["result"]["validated_assumptions"].is_array());
    assert!(json_value["data"]["result"]["refuted_assumptions"].is_array());
}

#[test]
fn invalid_probe_error_has_golden_exit_stream_and_machine_contracts() {
    let human = probe_describe_unknown(None);
    assert_eq!(human.status.code(), Some(2));
    assert!(human.stdout.is_empty());
    assert_eq!(String::from_utf8(human.stderr).unwrap(), INVALID_HUMAN);

    for mode in ["--json", "--ndjson"] {
        let output = probe_describe_unknown(Some(mode));
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(
            output.stderr.iter().filter(|byte| **byte == b'\n').count(),
            1
        );
        assert_eq!(pretty_json(&output.stderr), INVALID_JSON);
    }
}

#[test]
fn failed_probe_error_has_golden_exit_stream_and_machine_contracts() {
    let temporary = tempfile::tempdir().unwrap();
    let input = write_input(
        &temporary,
        "non-finite-result.json",
        &non_finite_result_input(),
    );

    let human = probe_run(&input, None);
    assert_eq!(human.status.code(), Some(6));
    assert!(human.stdout.is_empty());
    assert_eq!(String::from_utf8(human.stderr).unwrap(), FAILED_HUMAN);

    for mode in ["--json", "--ndjson"] {
        let output = probe_run(&input, Some(mode));
        assert_eq!(output.status.code(), Some(6));
        assert!(output.stdout.is_empty());
        assert_eq!(
            output.stderr.iter().filter(|byte| **byte == b'\n').count(),
            1
        );
        assert_eq!(pretty_json(&output.stderr), FAILED_JSON);
    }
}
