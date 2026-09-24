//! CLI/catalog parity tests for the inverse-search rewrite probes
//! (0.25 Cohort 3, Task 17).

#![cfg(feature = "standard-probes")]

use std::fs;

use amari_discovery::{
    Catalog, ProbeSchemaDocument, RewriteBackwardSearchOutput, RewriteBackwardSearchRequest,
    RewriteBidirectionalSearchOutput, RewriteBidirectionalSearchRequest,
};
use assert_cmd::Command;
use serde_json::json;
use tempfile::TempDir;

const BACKWARD: &str = "amari-probe:rewrite:backward-search:v1";
const BIDIRECTIONAL: &str = "amari-probe:rewrite:bidirectional-search:v1";

fn command_json(arguments: &[&str]) -> serde_json::Value {
    let output = Command::cargo_bin("amari")
        .unwrap()
        .args(arguments)
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).unwrap()
}

fn backward_witness_input() -> serde_json::Value {
    json!({
        "target": {"kind": "symbol", "name": "a", "arguments": []},
        "goal": {"kind": "symbol", "name": "add", "arguments": [
            {"kind": "symbol", "name": "a", "arguments": []},
            {"kind": "symbol", "name": "zero", "arguments": []}
        ]},
        "rules": [{
            "lhs": {"kind": "symbol", "name": "add", "arguments": [
                {"kind": "variable", "name": "X"},
                {"kind": "symbol", "name": "zero", "arguments": []}
            ]},
            "rhs": {"kind": "variable", "name": "X"}
        }],
        "mode": "breadth_first",
        "guidance": {"kind": "complete_within_limits"},
        "max_depth": 8
    })
}

#[test]
fn semantic_catalog_surfaces_inverse_search_capabilities_and_relations() {
    let catalog = Catalog::embedded().unwrap();
    let capabilities: Vec<String> = catalog
        .capabilities()
        .iter()
        .map(|capability| capability.id.to_string())
        .collect();
    for id in [
        "amari:amari-rewrite:inverse:backward-search",
        "amari:amari-rewrite:inverse:bidirectional-search",
    ] {
        assert!(capabilities.iter().any(|known| known == id), "missing {id}");
    }
    // The legacy predecessor capability links to both successors.
    let relations: Vec<(String, String)> = catalog
        .relations()
        .iter()
        .map(|relation| (relation.from.to_string(), relation.to.to_string()))
        .collect();
    for successor in [
        "amari:amari-rewrite:inverse:backward-search",
        "amari:amari-rewrite:inverse:bidirectional-search",
    ] {
        assert!(
            relations.iter().any(
                |(from, to)| from == "amari:amari-rewrite:inverse:predecessors" && to == successor
            ),
            "missing relation from legacy predecessors to {successor}"
        );
    }
}

#[test]
fn catalog_descriptors_cover_both_inverse_search_probes() {
    let catalog = Catalog::embedded().unwrap();
    let ids: Vec<String> = catalog
        .probes()
        .iter()
        .map(|descriptor| descriptor.id.to_string())
        .collect();
    assert!(ids.iter().any(|id| id == BACKWARD));
    assert!(ids.iter().any(|id| id == BIDIRECTIONAL));
    // Descriptor count is locked: Task 22 extended the set again.
    assert_eq!(ids.len(), 20);
}

#[test]
fn cli_schema_parity_for_inverse_search_contracts() {
    let cases: [(&str, serde_json::Value, serde_json::Value); 2] = [
        (
            BACKWARD,
            ProbeSchemaDocument::from_contract::<RewriteBackwardSearchRequest>()
                .unwrap()
                .exported_value()
                .unwrap(),
            ProbeSchemaDocument::from_contract::<RewriteBackwardSearchOutput>()
                .unwrap()
                .exported_value()
                .unwrap(),
        ),
        (
            BIDIRECTIONAL,
            ProbeSchemaDocument::from_contract::<RewriteBidirectionalSearchRequest>()
                .unwrap()
                .exported_value()
                .unwrap(),
            ProbeSchemaDocument::from_contract::<RewriteBidirectionalSearchOutput>()
                .unwrap()
                .exported_value()
                .unwrap(),
        ),
    ];
    for (probe, input_document, output_document) in cases {
        let input = command_json(&["probe", "schema", probe, "--direction", "input"]);
        let output = command_json(&["probe", "schema", probe, "--direction", "output"]);
        assert_eq!(input["data"]["document"], input_document);
        assert_eq!(output["data"]["document"], output_document);
    }
}

#[test]
fn cli_run_matches_engine_output_for_backward_search() {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("backward.json");
    let input = backward_witness_input();
    fs::write(&path, serde_json::to_vec(&input).unwrap()).unwrap();
    // Direct CLI JSON.
    let cli = command_json(&["probe", "run", BACKWARD, "--input", path.to_str().unwrap()]);
    let direct = amari_discovery::ProbeEngine::new()
        .unwrap()
        .execute(&BACKWARD.parse().unwrap(), &input)
        .unwrap();
    assert_eq!(cli["data"]["result"]["output"], direct.output);
    assert_eq!(cli["data"]["isolation"], json!("process"));
    assert_eq!(cli["data"]["crash_isolation"], json!(true));
    let output: RewriteBackwardSearchOutput =
        serde_json::from_value(cli["data"]["result"]["output"].clone()).unwrap();
    assert_eq!(output.outcome, "witness");
    assert_eq!(output.steps.len(), 1);
    // NDJSON framing carries the identical output.
    let ndjson = Command::cargo_bin("amari")
        .unwrap()
        .args(["shell", "--ndjson"])
        .write_stdin(
            serde_json::to_string(&json!({
                "schema_version": "amari.discovery/v1",
                "command": "probe.run",
                "arguments": {"probe_id": BACKWARD, "input": path.to_str().unwrap()}
            }))
            .unwrap()
                + "\n",
        )
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let line = String::from_utf8(ndjson).unwrap();
    let frame: serde_json::Value = serde_json::from_str(line.lines().next().unwrap()).unwrap();
    assert_eq!(frame["data"]["result"]["output"], direct.output);
    // Human mode succeeds and names the outcome.
    let human = Command::cargo_bin("amari")
        .unwrap()
        .args(["probe", "run", BACKWARD, "--input", path.to_str().unwrap()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let human = String::from_utf8(human).unwrap();
    assert!(human.contains("witness"), "human output: {human}");
}

#[test]
fn cli_run_matches_engine_output_for_bidirectional_search() {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("bidirectional.json");
    let input = json!({
        "source": {"kind": "symbol", "name": "add", "arguments": [
            {"kind": "symbol", "name": "a", "arguments": []},
            {"kind": "symbol", "name": "zero", "arguments": []}
        ]},
        "goal": {"kind": "symbol", "name": "a", "arguments": []},
        "rules": [{
            "lhs": {"kind": "symbol", "name": "add", "arguments": [
                {"kind": "variable", "name": "X"},
                {"kind": "symbol", "name": "zero", "arguments": []}
            ]},
            "rhs": {"kind": "variable", "name": "X"}
        }],
        "max_depth": 8
    });
    fs::write(&path, serde_json::to_vec(&input).unwrap()).unwrap();
    let cli = command_json(&[
        "probe",
        "run",
        BIDIRECTIONAL,
        "--input",
        path.to_str().unwrap(),
    ]);
    let direct = amari_discovery::ProbeEngine::new()
        .unwrap()
        .execute(&BIDIRECTIONAL.parse().unwrap(), &input)
        .unwrap();
    assert_eq!(cli["data"]["result"]["output"], direct.output);
    let output: RewriteBidirectionalSearchOutput =
        serde_json::from_value(cli["data"]["result"]["output"].clone()).unwrap();
    assert_eq!(output.outcome, "witness");
    assert_eq!(output.forward_steps.len(), 1);
}
