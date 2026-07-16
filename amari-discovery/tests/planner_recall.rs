// SPDX-License-Identifier: MIT OR Apache-2.0

//! Deterministic holographic candidate recall for the discovery planner.

use std::collections::HashSet;

use amari_discovery::{
    CandidateRetriever, Catalog, ProjectKind, ProjectSnapshot, RecallConfig, RetrievalSource,
    RustSourceInspection, SnapshotState, VocabularyEvidence,
};

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

fn snapshot_with_rust_vocabulary(term: &str) -> ProjectSnapshot {
    let mut snapshot = empty_snapshot("rust-vocabulary-project");
    snapshot.project_kind = ProjectKind::RustCargo;
    snapshot.rust = Some(RustSourceInspection {
        usages: Vec::new(),
        file_kinds: Vec::new(),
        crate_attributes: Vec::new(),
        cfg_evidence: Vec::new(),
        vocabulary: vec![VocabularyEvidence {
            path: "README.md".to_owned(),
            term: term.to_owned(),
            source: None,
        }],
        warnings: Vec::new(),
        input_hash: "rust-input".to_owned(),
        state: SnapshotState::Complete,
        inspected_file_count: 1,
        total_bytes: 0,
        input_files: Vec::new(),
    });
    snapshot
}

fn capability_ids(catalog: &Catalog) -> HashSet<String> {
    catalog
        .capabilities()
        .iter()
        .map(|capability| capability.id.to_string())
        .collect()
}

#[test]
fn exact_concepts_retrieve_expected_capability_first() {
    let catalog = Catalog::embedded().unwrap();
    let retriever = CandidateRetriever::default();

    let candidates = retriever
        .retrieve(
            &catalog,
            &empty_snapshot("project-a"),
            "Clifford product with geometric algebra multivectors",
        )
        .unwrap();

    assert_eq!(
        candidates.first().unwrap().capability_id.to_string(),
        "amari:amari-core:product:geometric-product"
    );
    assert!(candidates[0].holographic_score.is_finite());
    assert!(!candidates[0].matched_evidence.is_empty());
}

#[test]
fn related_vocabulary_retrieves_nonliteral_path_capability() {
    let catalog = Catalog::embedded().unwrap();
    let retriever = CandidateRetriever::default();
    let query = "routing under path costs";

    let candidates = retriever
        .retrieve(&catalog, &empty_snapshot("project-b"), query)
        .unwrap();
    let first = candidates.first().unwrap();

    assert!(
        matches!(
            first.capability_id.to_string().as_str(),
            "amari:amari-tropical:paths:shortest-path"
                | "amari:amari-network:paths:geometric-shortest-path"
        ),
        "unexpected first candidate: {}",
        first.capability_id
    );
    assert!(!first
        .matched_evidence
        .iter()
        .any(|evidence| evidence == query));
}

#[test]
fn same_seed_catalog_snapshot_and_goal_are_byte_identical() {
    let catalog = Catalog::embedded().unwrap();
    let retriever = CandidateRetriever::new(RecallConfig {
        seed: 0xA11CE,
        ..RecallConfig::default()
    });
    let snapshot = empty_snapshot("stable-project-hash");

    let first = retriever
        .retrieve(&catalog, &snapshot, "exact arithmetic")
        .unwrap();
    let second = retriever
        .retrieve(&catalog, &snapshot, "exact arithmetic")
        .unwrap();

    assert_eq!(
        serde_json::to_vec(&first).unwrap(),
        serde_json::to_vec(&second).unwrap()
    );
}

#[test]
fn typed_snapshot_vocabulary_contributes_to_recall() {
    let catalog = Catalog::embedded().unwrap();
    let snapshot = snapshot_with_rust_vocabulary("automatic_differentiation");

    let candidates = CandidateRetriever::default()
        .retrieve(&catalog, &snapshot, "")
        .unwrap();

    assert_eq!(
        candidates.first().unwrap().capability_id.to_string(),
        "amari:amari-dual:autodiff:forward-derivative"
    );
}

#[test]
fn holographic_recall_only_returns_catalog_ids() {
    let catalog = Catalog::embedded().unwrap();
    let known_ids = capability_ids(&catalog);

    let candidates = CandidateRetriever::default()
        .retrieve(
            &catalog,
            &empty_snapshot("project-c"),
            "memory graphs rewriting derivatives",
        )
        .unwrap();

    assert!(!candidates.is_empty());
    assert!(candidates
        .iter()
        .all(|candidate| known_ids.contains(&candidate.capability_id.to_string())));
}

#[test]
fn lexical_fallback_works_below_holographic_confidence_threshold() {
    let catalog = Catalog::embedded().unwrap();
    let retriever = CandidateRetriever::new(RecallConfig {
        minimum_holographic_score: 1.1,
        ..RecallConfig::default()
    });

    let candidates = retriever
        .retrieve(
            &catalog,
            &empty_snapshot("project-d"),
            "forward-mode AD with dual numbers",
        )
        .unwrap();

    assert_eq!(
        candidates.first().unwrap().capability_id.to_string(),
        "amari:amari-dual:autodiff:forward-derivative"
    );
    assert_eq!(candidates[0].source, RetrievalSource::LexicalFallback);
    assert!(candidates[0].lexical_score > 0.0);
}

#[test]
fn lexical_fallback_does_not_fabricate_unmatched_candidates() {
    let catalog = Catalog::embedded().unwrap();
    let retriever = CandidateRetriever::new(RecallConfig {
        minimum_holographic_score: 1.1,
        ..RecallConfig::default()
    });

    let candidates = retriever
        .retrieve(&catalog, &empty_snapshot("project-e"), "quokka zephyr")
        .unwrap();

    assert!(candidates.is_empty());
}
