// SPDX-License-Identifier: MIT OR Apache-2.0

//! CLI/catalog parity tests for the Cohort 6 analysis and completion
//! probes (0.25 Task 35).

#![cfg(feature = "standard-probes")]

use amari_discovery::{
    Catalog, ProbeSchemaDocument, RewriteAnalysisOutput, RewriteAnalysisRequest,
    RewriteCompletionOutput, RewriteCompletionRequest,
};
use assert_cmd::Command;
use serde_json::json;

const ANALYSIS: &str = "amari-probe:rewrite:analysis:v1";
const COMPLETION: &str = "amari-probe:rewrite:completion:v1";
const CRITICAL_PAIRS_CAPABILITY: &str = "amari:amari-rewrite:analysis:critical-pairs";
const CONFLUENCE_CAPABILITY: &str = "amari:amari-rewrite:analysis:local-confluence";
const TERMINATION_CAPABILITY: &str = "amari:amari-rewrite:analysis:termination";
const COMPLETION_CAPABILITY: &str = "amari:amari-rewrite:completion:knuth-bendix";

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

fn capability(id: &str) -> amari_discovery::CapabilityRecord {
    Catalog::embedded()
        .unwrap()
        .capabilities()
        .iter()
        .find(|capability| capability.id.to_string() == id)
        .unwrap_or_else(|| panic!("missing capability {id}"))
        .clone()
}

fn relation_sources(target: &str) -> Vec<String> {
    Catalog::embedded()
        .unwrap()
        .relations()
        .iter()
        .filter(|relation| relation.to.to_string() == target)
        .map(|relation| relation.from.to_string())
        .collect()
}

#[test]
fn semantic_catalog_surfaces_analysis_and_completion_capabilities() {
    let confluence = capability(CONFLUENCE_CAPABILITY);
    assert!(
        confluence
            .probe_refs
            .iter()
            .any(|probe| probe.to_string() == ANALYSIS),
        "the confluence capability does not reference {ANALYSIS}"
    );
    assert!(
        confluence
            .symbol_refs
            .iter()
            .any(|symbol| symbol.contains("local_confluence")),
        "the confluence capability lost its local_confluence symbol ref"
    );

    let completion = capability(COMPLETION_CAPABILITY);
    assert!(
        completion
            .probe_refs
            .iter()
            .any(|probe| probe.to_string() == COMPLETION),
        "the completion capability does not reference {COMPLETION}"
    );
    assert!(
        completion
            .symbol_refs
            .iter()
            .any(|symbol| symbol.contains("completion::complete")),
        "the completion capability lost its complete symbol ref"
    );
    assert!(
        completion
            .feature_refs
            .iter()
            .any(|feature| feature == "amari-rewrite:completion"),
        "the completion capability must name the non-default completion feature"
    );

    let critical_pairs = capability(CRITICAL_PAIRS_CAPABILITY);
    assert!(
        critical_pairs
            .symbol_refs
            .iter()
            .any(|symbol| symbol.contains("critical_pairs")),
        "the critical-pairs capability lost its symbol ref"
    );
    let termination = capability(TERMINATION_CAPABILITY);
    assert!(
        termination
            .symbol_refs
            .iter()
            .any(|symbol| symbol.contains("prove_terminating")),
        "the termination capability lost its symbol ref"
    );
}

#[test]
fn analysis_capabilities_connect_to_inverse_and_preimage_systems() {
    // Completion consumes critical pairs, termination, and
    // local-confluence evidence.
    let completion_sources = relation_sources(COMPLETION_CAPABILITY);
    for source in [
        CRITICAL_PAIRS_CAPABILITY,
        CONFLUENCE_CAPABILITY,
        TERMINATION_CAPABILITY,
    ] {
        assert!(
            completion_sources.iter().any(|id| id == source),
            "missing completion relation from {source}"
        );
    }

    // Completion-guided search composes with the existing inverse and
    // preimage systems without replacing their authority.
    let catalog = Catalog::embedded().unwrap();
    let outgoing: Vec<String> = catalog
        .relations()
        .iter()
        .filter(|relation| relation.from.to_string() == COMPLETION_CAPABILITY)
        .map(|relation| relation.to.to_string())
        .collect();
    for target in [
        "amari:amari-rewrite:inverse:predecessors",
        "amari:amari-rewrite:language:preimages",
    ] {
        assert!(
            outgoing.iter().any(|id| id == target),
            "missing completion relation to {target}"
        );
    }

    // Local confluence is analysis support for normalization, not a
    // replacement for it.
    let confluence_sources = relation_sources("amari:amari-rewrite:trs:normalization");
    assert!(
        confluence_sources
            .iter()
            .any(|id| id == CONFLUENCE_CAPABILITY),
        "local confluence must support normalization"
    );
}

#[test]
fn catalog_descriptors_cover_the_analysis_probes() {
    let catalog = Catalog::embedded().unwrap();
    let ids: Vec<String> = catalog
        .probes()
        .iter()
        .map(|descriptor| descriptor.id.to_string())
        .collect();
    // Descriptor count is locked: Task 39 extends the existing
    // twenty-three with the synthesis probes.
    assert_eq!(ids.len(), 25);
    assert!(ids.iter().any(|id| id == ANALYSIS));
    assert!(ids.iter().any(|id| id == COMPLETION));

    let analysis = catalog
        .probes()
        .iter()
        .find(|descriptor| descriptor.id.to_string() == ANALYSIS)
        .expect("analysis descriptor");
    assert_eq!(analysis.capability_id.to_string(), CONFLUENCE_CAPABILITY);
    assert_eq!(
        analysis.input_schema,
        "amari.discovery/probe/rewrite-analysis/input/v1"
    );
    assert_eq!(
        analysis.output_schema,
        "amari.discovery/probe/rewrite-analysis/output/v1"
    );

    let completion = catalog
        .probes()
        .iter()
        .find(|descriptor| descriptor.id.to_string() == COMPLETION)
        .expect("completion descriptor");
    assert_eq!(completion.capability_id.to_string(), COMPLETION_CAPABILITY);
    assert_eq!(
        completion.input_schema,
        "amari.discovery/probe/rewrite-completion/input/v1"
    );
    assert_eq!(
        completion.output_schema,
        "amari.discovery/probe/rewrite-completion/output/v1"
    );
}

#[test]
fn cli_schema_parity_for_analysis_contracts() {
    let analysis_input = ProbeSchemaDocument::from_contract::<RewriteAnalysisRequest>()
        .unwrap()
        .exported_value()
        .unwrap();
    let analysis_output = ProbeSchemaDocument::from_contract::<RewriteAnalysisOutput>()
        .unwrap()
        .exported_value()
        .unwrap();
    let completion_input = ProbeSchemaDocument::from_contract::<RewriteCompletionRequest>()
        .unwrap()
        .exported_value()
        .unwrap();
    let completion_output = ProbeSchemaDocument::from_contract::<RewriteCompletionOutput>()
        .unwrap()
        .exported_value()
        .unwrap();

    for (probe, input, output) in [
        (ANALYSIS, analysis_input, analysis_output),
        (COMPLETION, completion_input, completion_output),
    ] {
        let cli_input = command_json(&["probe", "schema", probe, "--direction", "input"]);
        let cli_output = command_json(&["probe", "schema", probe, "--direction", "output"]);
        assert_eq!(cli_input["data"]["document"], input, "input parity {probe}");
        assert_eq!(
            cli_output["data"]["document"], output,
            "output parity {probe}"
        );
    }
}

#[test]
fn cli_probe_list_marks_analysis_probes_executable() {
    let listed = command_json(&["probe", "list"]);
    let probes = listed["data"]["probes"].as_array().unwrap();
    for probe in [ANALYSIS, COMPLETION] {
        let entry = probes
            .iter()
            .find(|entry| entry["id"] == json!(probe))
            .unwrap_or_else(|| panic!("missing probe {probe}"));
        assert_eq!(entry["executable"], true, "{probe} must execute");
        assert_eq!(entry["known"], true);
    }
}
