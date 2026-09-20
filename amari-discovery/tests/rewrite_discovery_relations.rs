// SPDX-License-Identifier: MIT OR Apache-2.0

//! Discovery surfacing and CLI parity for the relation/reversibility
//! probes (0.25 Cohort 2, Task 12).

#![cfg(feature = "standard-probes")]

use std::fs;

use amari_discovery::{
    Catalog, ProbeSchemaDocument, RewriteInverseAnalysisOutput, RewriteInverseAnalysisRequest,
    RewriteResidualReplayOutput, RewriteResidualReplayRequest, RewriteSymbolicPredecessorsOutput,
    RewriteSymbolicPredecessorsRequest,
};
use assert_cmd::Command;
use serde_json::json;
use tempfile::TempDir;

const SYMBOLIC: &str = "amari-probe:rewrite:symbolic-predecessors:v1";
const ANALYSIS: &str = "amari-probe:rewrite:inverse-analysis:v1";
const REPLAY: &str = "amari-probe:rewrite:residual-replay:v1";

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

#[test]
fn semantic_catalog_surfaces_relation_and_reversibility_capabilities() {
    let catalog = Catalog::embedded().unwrap();
    let capabilities: Vec<String> = catalog
        .capabilities()
        .iter()
        .map(|capability| capability.id.to_string())
        .collect();
    for id in [
        "amari:amari-rewrite:inverse:symbolic-predecessors",
        "amari:amari-rewrite:inverse:analysis",
        "amari:amari-rewrite:reversible:residual-replay",
    ] {
        assert!(capabilities.iter().any(|known| known == id), "missing {id}");
    }
    // Each capability links its probe.
    let catalog_ref = &catalog;
    for (capability, probe) in [
        (
            "amari:amari-rewrite:inverse:symbolic-predecessors",
            SYMBOLIC,
        ),
        ("amari:amari-rewrite:inverse:analysis", ANALYSIS),
        ("amari:amari-rewrite:reversible:residual-replay", REPLAY),
    ] {
        let entry = catalog_ref
            .capabilities()
            .iter()
            .find(|entry| entry.id.to_string() == capability)
            .unwrap();
        assert!(
            entry.probe_refs.iter().any(|p| p.to_string() == probe),
            "{capability} does not reference {probe}"
        );
    }
}

#[test]
fn catalog_descriptors_cover_all_three_relation_probes() {
    let catalog = Catalog::embedded().unwrap();
    let probes: Vec<String> = catalog
        .probes()
        .iter()
        .map(|probe| probe.id.to_string())
        .collect();
    for id in [SYMBOLIC, ANALYSIS, REPLAY] {
        assert!(probes.iter().any(|known| known == id), "missing {id}");
    }
    // Descriptor count is locked: Task 22 extended the set again.
    assert_eq!(probes.len(), 20);
}

#[test]
fn probe_list_marks_relation_probes_executable() {
    let listed = command_json(&["probe", "list"]);
    let probes = listed["data"]["probes"].as_array().unwrap();
    for id in [SYMBOLIC, ANALYSIS, REPLAY] {
        let entry = probes
            .iter()
            .find(|probe| probe["id"] == json!(id))
            .unwrap_or_else(|| panic!("{id} not listed"));
        assert_eq!(entry["executable"], json!(true));
    }
}

#[test]
fn cli_schema_parity_for_relation_contracts() {
    let cases: [(&str, serde_json::Value, serde_json::Value); 3] = [
        (
            SYMBOLIC,
            ProbeSchemaDocument::from_contract::<RewriteSymbolicPredecessorsRequest>()
                .unwrap()
                .exported_value()
                .unwrap(),
            ProbeSchemaDocument::from_contract::<RewriteSymbolicPredecessorsOutput>()
                .unwrap()
                .exported_value()
                .unwrap(),
        ),
        (
            ANALYSIS,
            ProbeSchemaDocument::from_contract::<RewriteInverseAnalysisRequest>()
                .unwrap()
                .exported_value()
                .unwrap(),
            ProbeSchemaDocument::from_contract::<RewriteInverseAnalysisOutput>()
                .unwrap()
                .exported_value()
                .unwrap(),
        ),
        (
            REPLAY,
            ProbeSchemaDocument::from_contract::<RewriteResidualReplayRequest>()
                .unwrap()
                .exported_value()
                .unwrap(),
            ProbeSchemaDocument::from_contract::<RewriteResidualReplayOutput>()
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
fn cli_run_matches_engine_output_for_residual_replay() {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("residual.json");
    let input = json!({
        "source": {"kind": "symbol", "name": "f", "arguments": [
            {"kind": "symbol", "name": "a", "arguments": []},
            {"kind": "symbol", "name": "b", "arguments": []}
        ]},
        "rules": [{
            "lhs": {"kind": "symbol", "name": "f", "arguments": [
                {"kind": "variable", "name": "X"},
                {"kind": "variable", "name": "Y"}
            ]},
            "rhs": {"kind": "symbol", "name": "g", "arguments": [
                {"kind": "variable", "name": "X"}
            ]}
        }],
        "rule_index": 0,
        "path": []
    });
    fs::write(&path, serde_json::to_vec(&input).unwrap()).unwrap();
    let cli = command_json(&["probe", "run", REPLAY, "--input", path.to_str().unwrap()]);
    let direct = amari_discovery::ProbeEngine::new()
        .unwrap()
        .execute(&REPLAY.parse().unwrap(), &input)
        .unwrap();
    assert_eq!(cli["data"]["result"]["output"], direct.output);
    assert_eq!(cli["data"]["isolation"], json!("process"));
    assert_eq!(cli["data"]["crash_isolation"], json!(true));
    let output: RewriteResidualReplayOutput =
        serde_json::from_value(cli["data"]["result"]["output"].clone()).unwrap();
    assert!(output.matches_source);
    let name = match &output.target {
        amari_discovery::RewriteTerm::Symbol { name, .. } => name.clone(),
        other => panic!("expected symbol target, got {other:?}"),
    };
    assert_eq!(name, "g");
}
