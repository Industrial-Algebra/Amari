// SPDX-License-Identifier: MIT OR Apache-2.0

//! Discovery surfacing for neural inverse guidance (0.25 Cohort 8, Task 45):
//! the encoder, ranker, trainer, checkpoint, and strategy capabilities are
//! curated, marked experimental, searchable, detailed, graph-connected, and
//! recommended — as metadata only, with no probes and no neural dependency
//! added to `amari-discovery`.
//!
//! Gating note: the neural backend itself is `amari-rewrite`'s non-default
//! `neural` feature. Discovery never enables it; the capabilities advertise
//! that feature through `feature_refs` so a consumer learns the gate without
//! this crate ever linking it. The test target is gated exactly like the
//! other `rewrite_discovery_*` targets (`standard-probes`, the default).

#![cfg(feature = "standard-probes")]

use std::collections::HashSet;

use amari_discovery::{
    CandidateRetriever, CapabilityRecord, Catalog, ProjectKind, ProjectSnapshot, SnapshotState,
    StabilityTier,
};
use assert_cmd::Command;
use serde_json::Value;

const ENCODER: &str = "amari:amari-rewrite:neural:candidate-encoding";
const RANKER: &str = "amari:amari-rewrite:neural:ranker";
const TRAINER: &str = "amari:amari-rewrite:neural:training";
const CHECKPOINT: &str = "amari:amari-rewrite:neural:checkpoint";
const STRATEGY: &str = "amari:amari-rewrite:neural:strategy";
const NEURAL_FEATURE: &str = "amari-rewrite:neural";
const INVERSE_PREDECESSORS: &str = "amari:amari-rewrite:inverse:predecessors";
const BACKWARD_SEARCH: &str = "amari:amari-rewrite:inverse:backward-search";

const CAPABILITIES: [&str; 5] = [ENCODER, RANKER, TRAINER, CHECKPOINT, STRATEGY];

fn command_json(arguments: &[&str]) -> Value {
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

fn capability(id: &str) -> CapabilityRecord {
    Catalog::embedded()
        .unwrap()
        .capabilities()
        .iter()
        .find(|capability| capability.id.to_string() == id)
        .unwrap_or_else(|| panic!("missing capability {id}"))
        .clone()
}

fn result_ids(value: &Value) -> Vec<String> {
    value["data"]["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["id"].as_str().unwrap().to_string())
        .collect()
}

fn graph_relations(value: &Value) -> Vec<(String, String, String)> {
    value["data"]["relations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|relation| {
            (
                relation["from"].as_str().unwrap().to_string(),
                relation["to"].as_str().unwrap().to_string(),
                relation["kind"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn empty_snapshot(hash: &str) -> ProjectSnapshot {
    ProjectSnapshot {
        project_hash: hash.to_owned(),
        project_kind: ProjectKind::Unknown,
        signals: Vec::new(),
        cargo: None,
        rust: None,
        platform: None,
        npm: None,
        typescript: None,
        file_count: 0,
        total_bytes: 0,
        state: SnapshotState::Complete,
        warnings: Vec::new(),
        files: Vec::new(),
    }
}

#[test]
fn semantic_catalog_marks_neural_capabilities_experimental_and_probe_free() {
    let catalog = Catalog::embedded().unwrap();
    let known: HashSet<String> = catalog
        .capabilities()
        .iter()
        .map(|capability| capability.id.to_string())
        .collect();
    for id in CAPABILITIES {
        assert!(known.contains(id), "missing neural capability {id}");
    }

    for id in CAPABILITIES {
        let record = capability(id);
        assert_eq!(
            record.stability,
            StabilityTier::Experimental,
            "{id} must carry the experimental designation"
        );
        assert_eq!(
            record.crate_refs,
            vec!["amari-rewrite".to_string()],
            "{id} must name only the rewrite crate"
        );
        assert!(
            record
                .feature_refs
                .iter()
                .any(|feature| feature == NEURAL_FEATURE),
            "{id} must advertise the {NEURAL_FEATURE} gate"
        );
        assert!(
            record.probe_refs.is_empty(),
            "{id} must not claim a probe: neural guidance is metadata only"
        );
        assert!(
            !record.symbol_refs.is_empty(),
            "{id} must name the symbols it curates"
        );
    }

    // The neural additions are metadata only: the executable probe surface
    // is unchanged (Task 39 locked it at twenty-five).
    assert_eq!(catalog.probes().len(), 25, "Task 45 must not add probes");
}

#[test]
fn structural_catalog_resolves_every_neural_symbol_and_feature() {
    let catalog = Catalog::embedded().unwrap();
    let rewrite = catalog
        .crates()
        .iter()
        .find(|record| record.name == "amari-rewrite")
        .expect("amari-rewrite structural record");
    assert!(
        rewrite
            .features
            .iter()
            .any(|feature| feature.name == "neural"),
        "the structural catalog must record the non-default neural feature"
    );

    let item_paths: HashSet<&str> = rewrite
        .items
        .iter()
        .map(|item| item.path.as_str())
        .collect();
    for id in CAPABILITIES {
        for symbol in capability(id).symbol_refs {
            assert!(
                item_paths.contains(symbol.as_str()),
                "{id} references {symbol}, which the structural catalog does not record"
            );
        }
    }

    // Discovery never indexes itself, so its own dependency set can never
    // silently acquire the neural stack through catalog metadata.
    assert!(
        catalog
            .crates()
            .iter()
            .all(|record| record.name != "amari-discovery"),
        "amari-discovery must never index itself"
    );
}

#[test]
fn search_finds_each_neural_capability() {
    let queries = [
        (ENCODER, "candidate encoding"),
        (RANKER, "neural ranker"),
        (TRAINER, "ranker training"),
        (CHECKPOINT, "ranker checkpoint"),
        (STRATEGY, "neural guidance"),
    ];
    for (id, query) in queries {
        let ids = result_ids(&command_json(&["discover", "search", query]));
        assert!(
            ids.iter().any(|result| result == id),
            "search for `{query}` should find {id}: {ids:?}"
        );
    }
}

#[test]
fn detail_exposes_neural_feature_gate_and_experimental_tier() {
    for id in CAPABILITIES {
        let value = command_json(&["discover", "detail", id]);
        assert_eq!(value["data"]["id"], id);
        assert_eq!(
            value["data"]["stability"], "experimental",
            "{id} must render the experimental tier"
        );
        let features = value["data"]["feature_refs"].as_array().unwrap();
        assert!(
            features.iter().any(|feature| feature == NEURAL_FEATURE),
            "detail for {id} must expose {NEURAL_FEATURE}: {features:?}"
        );
        let probes = value["data"]["probe_refs"].as_array().unwrap();
        assert!(
            probes.is_empty(),
            "detail for {id} must not advertise a probe: {probes:?}"
        );
    }
}

#[test]
fn graph_connects_neural_pipeline_and_inverse_search() {
    let expected = [
        (ENCODER, RANKER, "produces_input_for"),
        (TRAINER, RANKER, "produces_input_for"),
        (CHECKPOINT, TRAINER, "supports"),
        (RANKER, STRATEGY, "composes_with"),
        (STRATEGY, INVERSE_PREDECESSORS, "composes_with"),
        (STRATEGY, BACKWARD_SEARCH, "composes_with"),
    ];
    for (from, to, kind) in expected {
        let relations = graph_relations(&command_json(&["discover", "graph", from]));
        assert!(
            relations.iter().any(|(source, target, actual)| {
                source == from && target == to && actual == kind
            }),
            "graph for {from} must carry {kind} -> {to}: {relations:?}"
        );
    }
}

#[test]
fn recommendation_recalls_neural_strategy_for_guidance_goal() {
    let catalog = Catalog::embedded().unwrap();
    let retriever = CandidateRetriever::default();
    let candidates = retriever
        .retrieve(
            &catalog,
            &empty_snapshot("neural-guidance-project"),
            "neural guidance for inverse rewrite search candidates",
        )
        .unwrap();
    let strategy = candidates
        .iter()
        .find(|candidate| candidate.capability_id.to_string() == STRATEGY)
        .unwrap_or_else(|| {
            panic!(
                "recommendation recall must surface {STRATEGY}: {:?}",
                candidates
                    .iter()
                    .map(|candidate| candidate.capability_id.to_string())
                    .collect::<Vec<_>>()
            )
        });
    assert!(
        strategy.retrieval_score.is_finite() && strategy.retrieval_score > 0.0,
        "recalled strategy must carry a positive retrieval score"
    );
    assert!(
        strategy
            .matched_evidence
            .iter()
            .any(|term| term == "neural"),
        "recalled strategy must match on the neural evidence: {:?}",
        strategy.matched_evidence
    );
}
