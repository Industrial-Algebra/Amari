// SPDX-License-Identifier: MIT OR Apache-2.0

//! CLI/catalog parity tests for the regular-language preimage probe
//! (0.25 Cohort 5, Task 29).

#![cfg(feature = "standard-probes")]

use std::fs;

use amari_discovery::{
    Catalog, ProbeEngine, ProbeSchemaDocument, RewritePreimagesOutput, RewritePreimagesRequest,
};
use assert_cmd::Command;
use serde_json::json;
use tempfile::TempDir;

const PREIMAGES: &str = "amari-probe:rewrite:preimages:v1";
const CAPABILITY: &str = "amari:amari-rewrite:language:preimages";

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
fn semantic_catalog_surfaces_preimage_capabilities() {
    let catalog = Catalog::embedded().unwrap();
    let capability = catalog
        .capabilities()
        .iter()
        .find(|capability| capability.id.to_string() == CAPABILITY)
        .expect("preimages capability");
    assert!(
        capability
            .probe_refs
            .iter()
            .any(|probe| probe.to_string() == PREIMAGES),
        "preimages capability does not reference {PREIMAGES}"
    );
    assert!(
        capability
            .symbol_refs
            .iter()
            .any(|symbol| symbol.contains("classify_and_query")),
        "preimages capability lost its symbol ref"
    );
    // Relations connect automata, grammars, and inverse search to the
    // preimage capability.
    let related: Vec<String> = catalog
        .relations()
        .iter()
        .filter(|relation| relation.to.to_string() == CAPABILITY)
        .map(|relation| relation.from.to_string())
        .collect();
    for source in [
        "amari:amari-rewrite:language:automata",
        "amari:amari-rewrite:language:grammars",
        "amari:amari-rewrite:inverse:predecessors",
    ] {
        assert!(related.iter().any(|id| id == source), "missing {source}");
    }
}

#[test]
fn catalog_descriptors_cover_the_preimages_probe() {
    let catalog = Catalog::embedded().unwrap();
    let ids: Vec<String> = catalog
        .probes()
        .iter()
        .map(|descriptor| descriptor.id.to_string())
        .collect();
    assert!(ids.iter().any(|id| id == PREIMAGES));
    // Descriptor count is locked: the preimages probe extends the
    // existing twenty.
    assert_eq!(ids.len(), 21);
    let descriptor = catalog
        .probes()
        .iter()
        .find(|descriptor| descriptor.id.to_string() == PREIMAGES)
        .unwrap();
    assert_eq!(descriptor.capability_id.to_string(), CAPABILITY);
    assert_eq!(
        descriptor.input_schema,
        "amari.discovery/probe/rewrite-preimages/input/v1"
    );
    assert_eq!(
        descriptor.output_schema,
        "amari.discovery/probe/rewrite-preimages/output/v1"
    );
}

#[test]
fn cli_schema_parity_for_preimage_contracts() {
    let input_document = ProbeSchemaDocument::from_contract::<RewritePreimagesRequest>()
        .unwrap()
        .exported_value()
        .unwrap();
    let output_document = ProbeSchemaDocument::from_contract::<RewritePreimagesOutput>()
        .unwrap()
        .exported_value()
        .unwrap();
    let input = command_json(&["probe", "schema", PREIMAGES, "--direction", "input"]);
    let output = command_json(&["probe", "schema", PREIMAGES, "--direction", "output"]);
    assert_eq!(input["data"]["document"], input_document);
    assert_eq!(output["data"]["document"], output_document);
}

#[test]
fn cli_run_matches_engine_output_for_preimages() {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("preimages.json");
    let input = json!({
        "operation": "membership",
        "preimage": "one_step",
        "rules": [{
            "lhs": {"kind": "symbol", "name": "a", "arguments": []},
            "rhs": {"kind": "symbol", "name": "b", "arguments": []}
        }],
        "language": {
            "alphabet": [{"name": "a", "arity": 0}, {"name": "b", "arity": 0}],
            "states": ["q0"],
            "transitions": [{"symbol": "b", "children": [], "parent": "q0"}],
            "finals": ["q0"]
        },
        "term": {"kind": "symbol", "name": "a", "arguments": []}
    });
    fs::write(&path, serde_json::to_vec(&input).unwrap()).unwrap();
    let cli = command_json(&["probe", "run", PREIMAGES, "--input", path.to_str().unwrap()]);
    let direct = amari_discovery::ProbeEngine::new()
        .unwrap()
        .execute(&PREIMAGES.parse().unwrap(), &input)
        .unwrap();
    assert_eq!(cli["data"]["result"]["output"], direct.output);
    assert_eq!(cli["data"]["isolation"], json!("process"));
    assert_eq!(cli["data"]["crash_isolation"], json!(true));
    let output: RewritePreimagesOutput =
        serde_json::from_value(cli["data"]["result"]["output"].clone()).unwrap();
    assert_eq!(output.verdict.as_deref(), Some("proven"));
}

/// Runs `probe run` expecting failure; returns (exit code, stderr).
fn command_run_error(input: &serde_json::Value) -> (i32, String) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("request.json");
    std::fs::write(&path, serde_json::to_vec(input).unwrap()).unwrap();
    let output = Command::cargo_bin("amari")
        .unwrap()
        .args(["probe", "run", PREIMAGES, "--input", path.to_str().unwrap()])
        .arg("--json")
        .assert()
        .failure()
        .get_output()
        .clone();
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn cli_preserves_typed_domain_errors() {
    // The process-isolated worker surfaces typed domain errors with
    // the same kind and exit code the in-process engine reports
    // (PR #286 round 1: worker framing must not flatten them).
    let cases = [
        // Unknown top-level field.
        json!({"operation": "classify", "rules": [], "bogus": true}),
        // Unknown NESTED DTO field.
        json!({
            "operation": "classify",
            "rules": [{
                "lhs": {"kind": "symbol", "name": "a", "arguments": [], "bogus": true},
                "rhs": {"kind": "symbol", "name": "b", "arguments": []}
            }]
        }),
        // Over-ceiling horizon.
        json!({"operation": "classify", "rules": [], "horizon": 5}),
    ];
    for input in &cases {
        let direct = ProbeEngine::new()
            .unwrap()
            .execute(&PREIMAGES.parse().unwrap(), input)
            .unwrap_err();
        let (code, stderr) = command_run_error(input);
        assert_eq!(
            code,
            i32::from(direct.exit_code()),
            "exit-code parity for {input}: {stderr}"
        );
        let rendered: serde_json::Value = serde_json::from_str(stderr.trim()).unwrap();
        assert_eq!(
            rendered["kind"].as_str().unwrap(),
            direct.kind(),
            "kind parity for {input}: {stderr}"
        );
    }
}
