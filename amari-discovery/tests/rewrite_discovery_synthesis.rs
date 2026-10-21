// SPDX-License-Identifier: MIT OR Apache-2.0

//! CLI/catalog parity tests for the Cohort 7 synthesis probes
//! (0.25 Task 39).

#![cfg(feature = "standard-probes")]

use amari_discovery::{
    Catalog, ProbeSchemaDocument, RewriteInverseSynthesisOutput, RewriteInverseSynthesisRequest,
    RewriteSpecializeOutput, RewriteSpecializeRequest,
};
use assert_cmd::Command;
use serde_json::json;

const SPECIALIZE: &str = "amari-probe:rewrite:specialize:v1";
const SYNTHESIZE_INVERSE: &str = "amari-probe:rewrite:synthesize-inverse:v1";
const SPECIALIZE_CAPABILITY: &str = "amari:amari-rewrite:synthesis:specialize";
const INVERSE_CAPABILITY: &str = "amari:amari-rewrite:synthesis:inverse";
const SYNTAX_CAPABILITY: &str = "amari:amari-rewrite:macros:relational-syntax";

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

fn relation_targets(source: &str) -> Vec<String> {
    Catalog::embedded()
        .unwrap()
        .relations()
        .iter()
        .filter(|relation| relation.from.to_string() == source)
        .map(|relation| relation.to.to_string())
        .collect()
}

#[test]
fn semantic_catalog_surfaces_synthesis_capabilities() {
    let specialize = capability(SPECIALIZE_CAPABILITY);
    assert!(
        specialize
            .probe_refs
            .iter()
            .any(|probe| probe.to_string() == SPECIALIZE),
        "the specialization capability does not reference {SPECIALIZE}"
    );
    assert!(
        specialize
            .symbol_refs
            .iter()
            .any(|symbol| symbol.contains("specialize_rules")),
        "the specialization capability lost its specialize_rules symbol ref"
    );
    assert!(
        specialize
            .symbol_refs
            .iter()
            .any(|symbol| symbol.contains("CoverageReport")),
        "the specialization capability must name its coverage evidence"
    );

    let inverse = capability(INVERSE_CAPABILITY);
    assert!(
        inverse
            .probe_refs
            .iter()
            .any(|probe| probe.to_string() == SYNTHESIZE_INVERSE),
        "the inverse-synthesis capability does not reference {SYNTHESIZE_INVERSE}"
    );
    assert!(
        inverse
            .symbol_refs
            .iter()
            .any(|symbol| symbol.contains("synthesize_inverse")),
        "the inverse-synthesis capability lost its synthesize_inverse symbol ref"
    );
    for symbol in ["InverseClause", "ResidualSchema"] {
        assert!(
            inverse
                .symbol_refs
                .iter()
                .any(|reference| reference.contains(symbol)),
            "the inverse-synthesis capability must name {symbol}"
        );
    }

    // The relational syntax capability names the checked macro that
    // builds forward/inverse clauses declaratively.
    let syntax = capability(SYNTAX_CAPABILITY);
    assert!(
        syntax
            .symbol_refs
            .iter()
            .any(|symbol| symbol == "amari_rewrite_macros::relation"),
        "the relational syntax capability lost its relation macro symbol ref"
    );
    assert!(
        syntax.probe_refs.is_empty(),
        "the relational syntax macro is not probe-executable"
    );
}

#[test]
fn synthesis_capabilities_connect_to_inverse_and_relation_systems() {
    // Inverse synthesis consumes refined forward rules from
    // specialization.
    let inverse_sources = relation_sources(INVERSE_CAPABILITY);
    for source in [SPECIALIZE_CAPABILITY, SYNTAX_CAPABILITY] {
        assert!(
            inverse_sources.iter().any(|id| id == source),
            "missing inverse-synthesis relation from {source}"
        );
    }

    // Refined rules feed normalization, and specialization composes
    // with single-rule inference rather than replacing it.
    assert!(
        relation_targets(SPECIALIZE_CAPABILITY)
            .iter()
            .any(|id| id == "amari:amari-rewrite:trs:normalization"),
        "specialization must feed normalization"
    );
    assert!(
        relation_sources(SPECIALIZE_CAPABILITY)
            .iter()
            .any(|id| id == "amari:amari-rewrite:synthesis:infer-rule"),
        "single-rule inference must compose with specialization"
    );

    // Derived clauses compose with the reversible residual authority
    // and the existing bounded inverse search without replacing either.
    let inverse_targets = relation_targets(INVERSE_CAPABILITY);
    for target in [
        "amari:amari-rewrite:reversible:residual-replay",
        "amari:amari-rewrite:inverse:predecessors",
    ] {
        assert!(
            inverse_targets.iter().any(|id| id == target),
            "missing inverse-synthesis relation to {target}"
        );
    }

    // The relational syntax capability supports inverse synthesis and
    // does not claim probe execution authority.
    assert!(
        relation_targets(SYNTAX_CAPABILITY)
            .iter()
            .any(|id| id == INVERSE_CAPABILITY),
        "relational syntax must support inverse synthesis"
    );
    assert!(
        relation_targets(SYNTAX_CAPABILITY)
            .iter()
            .all(|id| id != "amari:amari-rewrite:inverse:predecessors"),
        "relational syntax must not claim inverse-search authority"
    );
}

#[test]
fn catalog_descriptors_cover_the_synthesis_probes() {
    let catalog = Catalog::embedded().unwrap();
    let ids: Vec<String> = catalog
        .probes()
        .iter()
        .map(|descriptor| descriptor.id.to_string())
        .collect();
    // Descriptor count is locked: Task 39 extends the existing
    // twenty-three with the specialization and inverse-synthesis
    // probes.
    assert_eq!(ids.len(), 25);
    assert!(ids.iter().any(|id| id == SPECIALIZE));
    assert!(ids.iter().any(|id| id == SYNTHESIZE_INVERSE));

    let specialize = catalog
        .probes()
        .iter()
        .find(|descriptor| descriptor.id.to_string() == SPECIALIZE)
        .expect("specialization descriptor");
    assert_eq!(specialize.capability_id.to_string(), SPECIALIZE_CAPABILITY);
    assert_eq!(
        specialize.input_schema,
        "amari.discovery/probe/rewrite-specialize/input/v1"
    );
    assert_eq!(
        specialize.output_schema,
        "amari.discovery/probe/rewrite-specialize/output/v1"
    );

    let inverse = catalog
        .probes()
        .iter()
        .find(|descriptor| descriptor.id.to_string() == SYNTHESIZE_INVERSE)
        .expect("inverse-synthesis descriptor");
    assert_eq!(inverse.capability_id.to_string(), INVERSE_CAPABILITY);
    assert_eq!(
        inverse.input_schema,
        "amari.discovery/probe/rewrite-synthesize-inverse/input/v1"
    );
    assert_eq!(
        inverse.output_schema,
        "amari.discovery/probe/rewrite-synthesize-inverse/output/v1"
    );
}

#[test]
fn cli_schema_parity_for_synthesis_contracts() {
    let specialize_input = ProbeSchemaDocument::from_contract::<RewriteSpecializeRequest>()
        .unwrap()
        .exported_value()
        .unwrap();
    let specialize_output = ProbeSchemaDocument::from_contract::<RewriteSpecializeOutput>()
        .unwrap()
        .exported_value()
        .unwrap();
    let inverse_input = ProbeSchemaDocument::from_contract::<RewriteInverseSynthesisRequest>()
        .unwrap()
        .exported_value()
        .unwrap();
    let inverse_output = ProbeSchemaDocument::from_contract::<RewriteInverseSynthesisOutput>()
        .unwrap()
        .exported_value()
        .unwrap();

    for (probe, input, output) in [
        (SPECIALIZE, specialize_input, specialize_output),
        (SYNTHESIZE_INVERSE, inverse_input, inverse_output),
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
fn cli_probe_list_marks_synthesis_probes_executable() {
    let listed = command_json(&["probe", "list"]);
    let probes = listed["data"]["probes"].as_array().unwrap();
    for probe in [SPECIALIZE, SYNTHESIZE_INVERSE] {
        let entry = probes
            .iter()
            .find(|entry| entry["id"] == json!(probe))
            .unwrap_or_else(|| panic!("missing probe {probe}"));
        assert_eq!(entry["executable"], true, "{probe} must execute");
        assert_eq!(entry["known"], true);
    }
}
