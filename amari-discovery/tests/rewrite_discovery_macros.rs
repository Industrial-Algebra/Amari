// SPDX-License-Identifier: MIT OR Apache-2.0

//! Discovery surfacing for the rewrite syntax macros (0.25 Cohort 1,
//! Task 5): semantic capability records are searchable, detailed, and
//! graph-connected with correct feature refs; the structural catalog
//! lists the macro package exactly once and never indexes
//! amari-discovery itself.

use assert_cmd::Command;
use serde_json::Value;

fn discover_json(args: &[&str]) -> Value {
    let output = Command::cargo_bin("amari")
        .unwrap()
        .args(args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).unwrap()
}

fn result_ids(value: &Value) -> Vec<String> {
    value["data"]["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["id"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn search_finds_term_rule_macro_capability() {
    let value = discover_json(&["discover", "search", "term macro", "--json"]);
    let ids = result_ids(&value);
    assert!(
        ids.contains(&"amari:amari-rewrite:macros:term-rule-constructors".to_string()),
        "term macro search should find the syntax capability: {ids:?}"
    );
}

#[test]
fn search_finds_derive_capability_by_alias() {
    let value = discover_json(&["discover", "search", "rewritable derive", "--json"]);
    let ids = result_ids(&value);
    assert!(
        ids.contains(&"amari:amari-rewrite:macros:derive-rewritable".to_string()),
        "alias search should find the derive capability: {ids:?}"
    );
}

#[test]
fn detail_shows_macro_feature_refs() {
    let value = discover_json(&[
        "discover",
        "detail",
        "amari:amari-rewrite:macros:term-rule-constructors",
        "--json",
    ]);
    let features = value["data"]["feature_refs"].as_array().unwrap();
    assert!(
        features.iter().any(|f| f == "amari-rewrite:macros"),
        "detail must reference the macros feature: {features:?}"
    );
}

#[test]
fn graph_connects_macro_capability_to_crate() {
    let value = discover_json(&[
        "discover",
        "graph",
        "amari:amari-rewrite:macros:term-rule-constructors",
        "--json",
    ]);
    let relations = value["data"]["relations"].as_array().unwrap();
    assert!(
        relations
            .iter()
            .any(|relation| { relation["to"] == "amari:amari-rewrite:trs:normalization" }),
        "graph should relate the macros capability to TRS normalization: \
         {relations:?}"
    );
}

#[test]
fn structural_catalog_lists_macro_package_exactly_once() {
    // Split layout (CORE-CAT1): compose the embedded manifest + per-crate
    // bodies instead of reading the removed monolith.
    mod generated_index {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/catalog/index.rs"));
    }
    let structural = amari_discovery::compose_structural(
        generated_index::MANIFEST_JSON,
        generated_index::CRATE_JSON,
    )
    .unwrap();
    let raw = serde_json::to_string(&structural).unwrap();
    let catalog: Value = serde_json::from_str(&raw).unwrap();
    let crates = catalog["crates"].as_array().unwrap();
    let macro_packages = crates
        .iter()
        .filter(|c| c["name"] == "amari-rewrite-macros")
        .count();
    assert_eq!(macro_packages, 1, "macro package must appear exactly once");
    assert!(
        crates.iter().all(|c| c["name"] != "amari-discovery"),
        "amari-discovery must never index itself"
    );
}
