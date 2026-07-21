// SPDX-License-Identifier: MIT OR Apache-2.0

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn discover_json(args: &[&str]) -> Value {
    let output = Command::cargo_bin("amari")
        .unwrap()
        .args(args)
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).unwrap()
}

fn discover_fails(args: &[&str], expected_code: i32, expected_kind: &str) {
    let stderr = Command::cargo_bin("amari")
        .unwrap()
        .args(args)
        .assert()
        .code(expected_code)
        .stdout(predicate::str::is_empty())
        .get_output()
        .stderr
        .clone();
    let error: Value = serde_json::from_slice(&stderr).unwrap();
    assert_eq!(error["kind"], expected_kind);
    assert_eq!(error["details"]["exit_code"], expected_code);
    assert!(!error["message"]
        .as_str()
        .unwrap()
        .contains("internal failure"));
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

#[test]
fn search_tropical_json_returns_compact_results() {
    let value = discover_json(&["discover", "search", "tropical", "--json"]);

    assert_eq!(value["schema_version"], "amari.discovery/v1");
    let data = &value["data"];
    assert!(data["results"].is_array());
    let results = data["results"].as_array().unwrap();
    assert!(
        results.len() >= 2,
        "tropical should match multiple capabilities"
    );

    for result in results {
        // Compact: has id, name, description, aliases, concepts, stability, cost
        assert!(result["id"].is_string());
        assert!(result["name"].is_string());
        assert!(result["description"].is_string());
        assert!(result["aliases"].is_array());
        assert!(result["concepts"].is_array());
        assert!(result["stability"].is_string());
        assert!(result["cost"].is_string());

        // Compact: must NOT duplicate full detail fields
        assert!(
            result.get("crate_refs").is_none(),
            "search results must not include crate_refs"
        );
        assert!(
            result.get("feature_refs").is_none(),
            "search results must not include feature_refs"
        );
        assert!(
            result.get("symbol_refs").is_none(),
            "search results must not include symbol_refs"
        );
        assert!(
            result.get("example_refs").is_none(),
            "search results must not include example_refs"
        );
        assert!(
            result.get("probe_refs").is_none(),
            "search results must not include probe_refs"
        );
    }

    // Provenance: catalog identity present, no project context
    assert_eq!(value["provenance"]["catalog"]["version"], "0.23.0");
    assert!(value["provenance"]["catalog"]["hash"].is_string());
    assert_eq!(value["provenance"]["project_hash"], Value::Null);
    assert_eq!(value["provenance"]["input_hash"], Value::Null);
    assert_eq!(value["provenance"]["compatibility"]["status"], "compatible");
    assert_eq!(value["provenance"]["replay"]["replayable"], false);
}

#[test]
fn search_exact_capability_name_ranks_first() {
    // "Tropical shortest paths" is the exact name of one capability
    let value = discover_json(&["discover", "search", "Tropical shortest paths", "--json"]);
    let results = value["data"]["results"].as_array().unwrap();
    assert!(!results.is_empty());
    // First result should be the exact name match
    assert_eq!(results[0]["id"], "amari:amari-tropical:paths:shortest-path");
    assert_eq!(results[0]["name"], "Tropical shortest paths");
}

#[test]
fn search_empty_results_returns_empty_array() {
    let value = discover_json(&["discover", "search", "zzz_nonexistent_xyzzy", "--json"]);
    let results = value["data"]["results"].as_array().unwrap();
    assert!(results.is_empty());
}

#[test]
fn search_human_output_is_concise() {
    Command::cargo_bin("amari")
        .unwrap()
        .args(["discover", "search", "tropical"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains("Tropical"))
        .stdout(predicate::str::contains("schema_version").not())
        .stdout(predicate::str::contains("crate_refs").not());
}

#[test]
fn search_by_alias_finds_capability() {
    // "min-plus paths" is an alias of the tropical shortest paths capability
    let value = discover_json(&["discover", "search", "min-plus paths", "--json"]);
    let results = value["data"]["results"].as_array().unwrap();
    assert!(!results.is_empty());
    let ids: Vec<&str> = results.iter().map(|r| r["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&"amari:amari-tropical:paths:shortest-path"));
}

#[test]
fn search_by_concept_finds_capability() {
    // "geometric algebra" is a concept on multiple capabilities
    let value = discover_json(&["discover", "search", "geometric algebra", "--json"]);
    let results = value["data"]["results"].as_array().unwrap();
    assert!(!results.is_empty());
    let ids: Vec<&str> = results.iter().map(|r| r["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&"amari:amari-core:product:geometric-product"));
}

#[test]
fn search_by_description_finds_capability() {
    // "multivectors" appears in the geometric product description
    let value = discover_json(&["discover", "search", "multivectors", "--json"]);
    let results = value["data"]["results"].as_array().unwrap();
    assert!(!results.is_empty());
    let ids: Vec<&str> = results.iter().map(|r| r["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&"amari:amari-core:product:geometric-product"));
}

#[test]
fn search_by_crate_name_finds_capability() {
    let value = discover_json(&["discover", "search", "amari-tropical", "--json"]);
    let results = value["data"]["results"].as_array().unwrap();
    assert!(!results.is_empty());
    let ids: Vec<&str> = results.iter().map(|r| r["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&"amari:amari-tropical:paths:shortest-path"));
}

#[test]
fn search_ranking_is_deterministic() {
    let a = discover_json(&["discover", "search", "tropical", "--json"]);
    let b = discover_json(&["discover", "search", "tropical", "--json"]);
    let ids_a: Vec<&str> = a["data"]["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    let ids_b: Vec<&str> = b["data"]["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids_a, ids_b,
        "search results must be deterministically ordered"
    );
}

// ---------------------------------------------------------------------------
// Detail (symbol_ref resolution)
// ---------------------------------------------------------------------------

#[test]
fn detail_by_exact_symbol_ref_resolves_capability() {
    // amari_tropical::TropicalMatrix is a symbol_ref on the tropical
    // shortest paths capability, not its name or alias.
    let value = discover_json(&[
        "discover",
        "detail",
        "amari_tropical::TropicalMatrix",
        "--json",
    ]);
    assert_eq!(
        value["data"]["id"],
        "amari:amari-tropical:paths:shortest-path"
    );
    assert_eq!(value["data"]["name"], "Tropical shortest paths");
}

#[test]
fn detail_by_capability_exact_symbol_ref_resolves_capability() {
    // amari_core::Rotor is the sole symbol_ref on the rotor capability.
    let value = discover_json(&["discover", "detail", "amari_core::Rotor", "--json"]);
    assert_eq!(value["data"]["id"], "amari:amari-core:rotor:rotation");
    assert_eq!(value["data"]["name"], "Rotor transformations");
}

// ---------------------------------------------------------------------------
// Detail
// ---------------------------------------------------------------------------

#[test]
fn detail_known_capability_returns_full_record() {
    let value = discover_json(&[
        "discover",
        "detail",
        "amari:amari-tropical:paths:shortest-path",
        "--json",
    ]);

    assert_eq!(value["schema_version"], "amari.discovery/v1");
    let data = &value["data"];

    assert_eq!(data["id"], "amari:amari-tropical:paths:shortest-path");
    assert_eq!(data["name"], "Tropical shortest paths");
    assert!(data["description"].is_string());
    assert!(data["aliases"].is_array());
    assert!(data["concepts"].is_array());

    // Detail is complete: includes all fields
    assert!(data["crate_refs"].is_array());
    assert!(data["symbol_refs"].is_array());
    assert!(data["probe_refs"].is_array());
    assert!(data["stability"].is_string());
    assert!(data["cost"].is_string());

    // Crate refs must reference known crates
    let crate_refs: Vec<&str> = data["crate_refs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(crate_refs.contains(&"amari-tropical"));
}

#[test]
fn detail_unknown_capability_id_returns_structured_error() {
    discover_fails(
        &[
            "discover",
            "detail",
            "amari:amari-nonexistent:fake:not-real",
            "--json",
        ],
        2,
        "invalid_id",
    );
}

#[test]
fn detail_bare_string_not_a_valid_id_returns_structured_error() {
    // "not-an-id" doesn't start with "amari:" so it can't be a CapabilityId
    // and it doesn't match any capability name or alias
    discover_fails(
        &["discover", "detail", "not-an-id", "--json"],
        2,
        "invalid_id",
    );
}

#[test]
fn detail_human_output_is_concise() {
    Command::cargo_bin("amari")
        .unwrap()
        .args([
            "discover",
            "detail",
            "amari:amari-tropical:paths:shortest-path",
        ])
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains("Tropical shortest paths"))
        .stdout(predicate::str::contains("schema_version").not());
}

#[test]
fn detail_human_output_renders_feature_refs() {
    // Detail human output must include feature_refs.
    Command::cargo_bin("amari")
        .unwrap()
        .args([
            "discover",
            "detail",
            "amari:amari-tropical:paths:shortest-path",
        ])
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains("Features:"))
        .stdout(predicate::str::contains("amari-tropical:std"));
}

// ---------------------------------------------------------------------------
// Graph (symbol_ref resolution)
// ---------------------------------------------------------------------------

#[test]
fn graph_by_exact_symbol_ref_resolves_capability() {
    let value = discover_json(&[
        "discover",
        "graph",
        "amari_tropical::TropicalMatrix",
        "--json",
    ]);
    assert_eq!(
        value["data"]["capability_id"],
        "amari:amari-tropical:paths:shortest-path"
    );
}

// ---------------------------------------------------------------------------
// Graph
// ---------------------------------------------------------------------------

#[test]
fn graph_known_capability_returns_relationships() {
    let value = discover_json(&[
        "discover",
        "graph",
        "amari:amari-tropical:paths:shortest-path",
        "--json",
    ]);

    assert_eq!(value["schema_version"], "amari.discovery/v1");
    let data = &value["data"];

    assert_eq!(
        data["capability_id"],
        "amari:amari-tropical:paths:shortest-path"
    );
    assert!(data["capability_name"].is_string());
    assert!(data["relations"].is_array());

    // Every relationship must have valid endpoint IDs and a kind
    let relations = data["relations"].as_array().unwrap();
    assert!(
        !relations.is_empty(),
        "tropical shortest path has known relations"
    );
    for relation in relations {
        assert!(relation["from"].is_string());
        assert!(relation["to"].is_string());
        assert!(relation["kind"].is_string());

        // Endpoint IDs must parse as valid capability IDs
        let from = relation["from"].as_str().unwrap();
        let to = relation["to"].as_str().unwrap();
        assert!(
            from.starts_with("amari:"),
            "graph from must be a CapabilityId: {from}"
        );
        assert!(
            to.starts_with("amari:"),
            "graph to must be a CapabilityId: {to}"
        );

        // Kind must be non-empty
        assert!(!relation["kind"].as_str().unwrap().is_empty());
    }
}

#[test]
fn graph_rotor_has_inbound_supports_relation() {
    // Rotor has an inbound "supports" relation from geometric-product.
    // The graph command must include both inbound and outbound relations.
    let value = discover_json(&[
        "discover",
        "graph",
        "amari:amari-core:rotor:rotation",
        "--json",
    ]);

    let relations = value["data"]["relations"].as_array().unwrap();
    assert_eq!(relations.len(), 1, "rotor must have one inbound relation");
    let first = &relations[0];
    assert_eq!(first["from"], "amari:amari-core:product:geometric-product");
    assert_eq!(first["to"], "amari:amari-core:rotor:rotation");
    assert_eq!(first["kind"], "supports");
}

#[test]
fn graph_capability_without_relations_returns_empty_array() {
    // "Tropical Viterbi decoding" has no relations in the semantic catalog.
    let value = discover_json(&[
        "discover",
        "graph",
        "amari:amari-tropical:sequence:viterbi",
        "--json",
    ]);

    let relations = value["data"]["relations"].as_array().unwrap();
    assert!(relations.is_empty());
}

#[test]
fn graph_unknown_id_returns_structured_error() {
    discover_fails(
        &[
            "discover",
            "graph",
            "amari:amari-nonexistent:fake:not-real",
            "--json",
        ],
        2,
        "invalid_id",
    );
}

#[test]
fn graph_human_output_is_concise() {
    Command::cargo_bin("amari")
        .unwrap()
        .args([
            "discover",
            "graph",
            "amari:amari-tropical:paths:shortest-path",
        ])
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains("shortest-path"))
        .stdout(predicate::str::contains("schema_version").not());
}

#[test]
fn graph_human_output_labels_relation_direction() {
    // Rotor has one inbound relation from geometric-product.
    // The human output must label it [inbound].
    Command::cargo_bin("amari")
        .unwrap()
        .args(["discover", "graph", "amari:amari-core:rotor:rotation"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains("[inbound]"))
        .stdout(predicate::str::contains("[outbound]").not());
}

#[test]
fn graph_human_output_labels_outbound_relation() {
    // Geometric product has one outbound relation to rotor.
    // The human output must label it [outbound].
    Command::cargo_bin("amari")
        .unwrap()
        .args([
            "discover",
            "graph",
            "amari:amari-core:product:geometric-product",
        ])
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains("[outbound]"))
        .stdout(predicate::str::contains("[inbound]").not());
}

// ---------------------------------------------------------------------------
// Example (symbol_ref resolution)
// ---------------------------------------------------------------------------

#[test]
fn example_by_exact_symbol_ref_resolves_capability() {
    let value = discover_json(&[
        "discover",
        "example",
        "amari_tropical::TropicalMatrix",
        "--json",
    ]);
    assert_eq!(
        value["data"]["capability_id"],
        "amari:amari-tropical:paths:shortest-path"
    );
    let example_names: Vec<&str> = value["data"]["examples"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["example_name"].as_str().unwrap())
        .collect();
    assert!(example_names.contains(&"max_plus_paths"));
}

// ---------------------------------------------------------------------------
// Example
// ---------------------------------------------------------------------------

#[test]
fn example_known_capability_returns_example_info() {
    let value = discover_json(&[
        "discover",
        "example",
        "amari:amari-tropical:paths:shortest-path",
        "--json",
    ]);

    assert_eq!(value["schema_version"], "amari.discovery/v1");
    let data = &value["data"];

    assert_eq!(
        data["capability_id"],
        "amari:amari-tropical:paths:shortest-path"
    );
    assert_eq!(data["capability_name"], "Tropical shortest paths");
    assert!(data["examples"].is_array());

    let examples = data["examples"].as_array().unwrap();
    assert!(
        !examples.is_empty(),
        "tropical shortest path has example refs"
    );

    let example_names: Vec<&str> = examples
        .iter()
        .map(|e| e["example_name"].as_str().unwrap())
        .collect();
    assert!(example_names.contains(&"max_plus_paths"));

    // Each example record must have crate_name, example_name, path
    for example in examples {
        assert!(example["crate_name"].is_string());
        assert!(example["example_name"].is_string());
        assert!(example["path"].is_string());
        assert!(!example["path"].as_str().unwrap().is_empty());
    }
}

#[test]
fn example_capability_without_examples_returns_typed_error() {
    // "Tropical Viterbi decoding" has empty example_refs
    discover_fails(
        &[
            "discover",
            "example",
            "amari:amari-tropical:sequence:viterbi",
            "--json",
        ],
        2,
        "invalid_input",
    );
}

#[test]
fn example_unknown_id_returns_structured_error() {
    discover_fails(
        &[
            "discover",
            "example",
            "amari:amari-nonexistent:fake:not-real",
            "--json",
        ],
        2,
        "invalid_id",
    );
}

#[test]
fn example_human_output_is_concise() {
    Command::cargo_bin("amari")
        .unwrap()
        .args([
            "discover",
            "example",
            "amari:amari-tropical:paths:shortest-path",
        ])
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains("max_plus_paths"))
        .stdout(predicate::str::contains("schema_version").not());
}

// ---------------------------------------------------------------------------
// Provenance: consistent across discover commands
// ---------------------------------------------------------------------------

#[test]
fn discover_provenance_identifies_embedded_catalog_and_non_project() {
    // Use BTreeSet for deterministic ordering across all four discover commands.
    use std::collections::BTreeSet;
    let args_sets: BTreeSet<Vec<&str>> = BTreeSet::from([
        vec!["discover", "search", "tropical", "--json"],
        vec![
            "discover",
            "detail",
            "amari:amari-tropical:paths:shortest-path",
            "--json",
        ],
        vec![
            "discover",
            "graph",
            "amari:amari-tropical:paths:shortest-path",
            "--json",
        ],
        vec![
            "discover",
            "example",
            "amari:amari-tropical:paths:shortest-path",
            "--json",
        ],
    ]);
    for args in &args_sets {
        let value = discover_json(args);
        assert_eq!(value["provenance"]["catalog"]["version"], "0.23.0");
        assert!(value["provenance"]["catalog"]["hash"].is_string());
        assert_eq!(value["provenance"]["project_hash"], Value::Null);
        assert_eq!(value["provenance"]["compatibility"]["status"], "compatible");
        assert_eq!(value["provenance"]["replay"]["replayable"], false);
        assert!(!value["provenance"]["replay"]["reasons"]
            .as_array()
            .unwrap()
            .is_empty());
    }
}

// ---------------------------------------------------------------------------
// Search: case-insensitive name matching
// ---------------------------------------------------------------------------

#[test]
fn search_by_lowercase_name_ranks_same_as_exact() {
    // "tropical shortest paths" (lowercase) should match the same capability
    // with the same ranking as the exact-name "Tropical shortest paths".
    let exact = discover_json(&["discover", "search", "Tropical shortest paths", "--json"]);
    let lower = discover_json(&["discover", "search", "tropical shortest paths", "--json"]);
    let ids_exact: Vec<&str> = exact["data"]["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    let ids_lower: Vec<&str> = lower["data"]["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert!(!ids_exact.is_empty());
    assert_eq!(
        ids_exact, ids_lower,
        "case-insensitive name match must rank identically"
    );
}

#[test]
fn search_by_symbol_path_finds_capability() {
    // Searching for an exact symbol path like amari_tropical::TropicalMatrix
    // must find the owning capability.
    let value = discover_json(&[
        "discover",
        "search",
        "amari_tropical::TropicalMatrix",
        "--json",
    ]);
    let results = value["data"]["results"].as_array().unwrap();
    assert!(!results.is_empty());
    let ids: Vec<&str> = results.iter().map(|r| r["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&"amari:amari-tropical:paths:shortest-path"));
}

#[test]
fn search_exact_id_still_case_sensitive() {
    // Canonical capability IDs are lowercase. An uppercase ID variant must
    // not match via rank-0 exact ID.
    let value = discover_json(&[
        "discover",
        "search",
        "AMARI:AMARI-TROPICAL:PATHS:SHORTEST-PATH",
        "--json",
    ]);
    let results = value["data"]["results"].as_array().unwrap();
    // The uppercase form should not match via exact ID; it might match via
    // case-insensitive substring in name/concepts but should not have rank-0.
    // If no results, that's fine — the key is the exact ID is case-sensitive.
    if !results.is_empty() {
        assert_ne!(
            results[0]["id"], "amari:amari-tropical:paths:shortest-path",
            "uppercase ID variant must not get exact-ID rank-0 match"
        );
    }
}

// ---------------------------------------------------------------------------
// Error: stable format, exit codes, clean stdout
// ---------------------------------------------------------------------------

#[test]
fn error_stderr_contains_stable_structured_kind() {
    // Every structured error must print "{kind}: {message}" to stderr.
    for (args, expected_kind) in [
        (
            [
                "discover",
                "detail",
                "amari:amari-nonexistent:fake:not-real",
                "--json",
            ]
            .as_slice(),
            "invalid_id",
        ),
        (
            ["discover", "detail", "not-a-valid-ref", "--json"].as_slice(),
            "invalid_id",
        ),
        (
            [
                "discover",
                "example",
                "amari:amari-tropical:sequence:viterbi",
                "--json",
            ]
            .as_slice(),
            "invalid_input",
        ),
    ] {
        discover_fails(args, 2, expected_kind);
    }
}

#[test]
fn error_stdout_is_empty_and_exit_code_is_documented() {
    // Exit code 2 is the documented code for invalid_id and invalid_input.
    // Verify clean stdout for every error path.
    for (args, expected_code) in [
        (
            [
                "discover",
                "detail",
                "amari:amari-nonexistent:fake:not-real",
                "--json",
            ]
            .as_slice(),
            2,
        ),
        (
            [
                "discover",
                "graph",
                "amari:amari-nonexistent:fake:not-real",
                "--json",
            ]
            .as_slice(),
            2,
        ),
        (
            [
                "discover",
                "example",
                "amari:amari-nonexistent:fake:not-real",
                "--json",
            ]
            .as_slice(),
            2,
        ),
    ] {
        Command::cargo_bin("amari")
            .unwrap()
            .args(args)
            .assert()
            .code(expected_code)
            .stdout(predicate::str::is_empty());
    }
}

// ---------------------------------------------------------------------------
// Error: help still works after discover is implemented
// ---------------------------------------------------------------------------

#[test]
fn discover_help_still_exposes_subcommands() {
    for command in ["search", "detail", "graph", "example"] {
        Command::cargo_bin("amari")
            .unwrap()
            .args(["discover", command, "--help"])
            .assert()
            .success();
    }
}
