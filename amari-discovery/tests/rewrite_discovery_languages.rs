// SPDX-License-Identifier: MIT OR Apache-2.0

//! CLI/catalog parity tests for the regular-language rewrite probe
//! (0.25 Cohort 4, Task 22).

#![cfg(feature = "standard-probes")]

use amari_discovery::{
    Catalog, ProbeSchemaDocument, RewriteLanguagesOutput, RewriteLanguagesRequest,
};
use assert_cmd::Command;

const LANGUAGES: &str = "amari-probe:rewrite:languages:v1";

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
fn semantic_catalog_surfaces_language_capabilities() {
    let catalog = Catalog::embedded().unwrap();
    let capabilities: Vec<String> = catalog
        .capabilities()
        .iter()
        .map(|capability| capability.id.to_string())
        .collect();
    for id in [
        "amari:amari-rewrite:language:automata",
        "amari:amari-rewrite:language:grammars",
    ] {
        assert!(capabilities.iter().any(|known| known == id), "missing {id}");
    }
    // The automata capability owns and references the languages
    // probe; the grammars capability is exercised through grammar
    // text inputs to that same probe (probe_refs are owner-only).
    let automata = catalog
        .capabilities()
        .iter()
        .find(|capability| capability.id.to_string() == "amari:amari-rewrite:language:automata")
        .expect("automata capability");
    assert!(
        automata
            .probe_refs
            .iter()
            .any(|probe| probe.to_string() == LANGUAGES),
        "automata capability does not reference {LANGUAGES}"
    );
    let grammars = catalog
        .capabilities()
        .iter()
        .find(|capability| capability.id.to_string() == "amari:amari-rewrite:language:grammars")
        .expect("grammars capability");
    assert!(
        grammars
            .symbol_refs
            .iter()
            .any(|symbol| symbol.contains("RegularTreeGrammar")),
        "grammars capability lost its symbol ref"
    );
}

#[test]
fn catalog_descriptors_cover_the_languages_probe() {
    let catalog = Catalog::embedded().unwrap();
    let ids: Vec<String> = catalog
        .probes()
        .iter()
        .map(|descriptor| descriptor.id.to_string())
        .collect();
    assert!(ids.iter().any(|id| id == LANGUAGES));
    // Descriptor count is locked: the languages probe extends the
    // existing nineteen.
    assert_eq!(ids.len(), 20);
}

#[test]
fn cli_schema_parity_for_language_contracts() {
    let input_document = ProbeSchemaDocument::from_contract::<RewriteLanguagesRequest>()
        .unwrap()
        .exported_value()
        .unwrap();
    let output_document = ProbeSchemaDocument::from_contract::<RewriteLanguagesOutput>()
        .unwrap()
        .exported_value()
        .unwrap();
    let input = command_json(&["probe", "schema", LANGUAGES, "--direction", "input"]);
    let output = command_json(&["probe", "schema", LANGUAGES, "--direction", "output"]);
    assert_eq!(input["data"]["document"], input_document);
    assert_eq!(output["data"]["document"], output_document);
}
