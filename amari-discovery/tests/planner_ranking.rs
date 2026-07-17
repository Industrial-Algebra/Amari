// SPDX-License-Identifier: MIT OR Apache-2.0

//! Transparent Pareto ranking for expanded planner candidates.

use std::collections::BTreeSet;

use amari_discovery::{
    CandidateRanker, CapabilityGraphExpander, CapabilityId, Catalog, GraphConstraints,
    ProbeBackend, ProbeResult, ProjectKind, ProjectSnapshot, RankingComponents, RankingContext,
    RankingSignal, RankingSignalKind, ResourceObservations, RetrievalSource, RetrievedCandidate,
    SnapshotState,
};

fn id(value: &str) -> CapabilityId {
    value.parse().unwrap()
}

fn snapshot(hash: &str) -> ProjectSnapshot {
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

fn recalled(capability_id: CapabilityId, score: f64) -> RetrievedCandidate {
    RetrievedCandidate {
        capability_id,
        retrieval_score: score,
        holographic_score: score,
        lexical_score: score,
        matched_evidence: vec!["matched".to_owned()],
        source: RetrievalSource::Holographic,
    }
}

fn expansion(catalog: &Catalog, seeds: &[CapabilityId]) -> amari_discovery::GraphExpansion {
    CapabilityGraphExpander::default()
        .expand(catalog, seeds, &GraphConstraints::default())
        .unwrap()
}

fn ranked<'a>(
    result: &'a amari_discovery::RankingResult,
    capability_id: &CapabilityId,
) -> &'a amari_discovery::RankedCandidate {
    result
        .pareto
        .iter()
        .chain(&result.dominated)
        .find(|candidate| &candidate.capability_id == capability_id)
        .unwrap()
}

fn probe(
    catalog: &Catalog,
    project_hash: &str,
    validated: &[&str],
    refuted: &[&str],
) -> ProbeResult {
    ProbeResult {
        probe_id: "amari-probe:dual:polynomial-derivative:v1".parse().unwrap(),
        backend: ProbeBackend::Cpu,
        duration_micros: 10,
        resources: ResourceObservations {
            operations: 1,
            ..ResourceObservations::default()
        },
        seed: Some(7),
        project_hash: Some(project_hash.to_owned()),
        catalog_hash: catalog.content_hash().to_owned(),
        input_hash: "probe-input".to_owned(),
        validated_assumptions: validated.iter().map(|value| (*value).to_owned()).collect(),
        refuted_assumptions: refuted.iter().map(|value| (*value).to_owned()).collect(),
        warnings: Vec::new(),
        output: serde_json::json!({"ok": true}),
    }
}

#[test]
fn components_expose_canonical_all_minimization_vector() {
    let components = RankingComponents {
        applicability: 0.9,
        evidence: 0.8,
        effort: 0.3,
        maturity: 0.7,
        runtime: 0.4,
        platform: 0.6,
        verification: 0.5,
        risk: 0.2,
    };

    assert_eq!(
        components.minimization_objectives(),
        [0.1, 0.2, 0.3, 0.3, 0.4, 0.4, 0.5, 0.2]
    );
}

#[test]
fn satisfying_prerequisites_improves_and_unblocks_same_id() {
    let catalog = Catalog::embedded().unwrap();
    let target = id("amari:amari-core:rotor:rotation");
    let graph = expansion(&catalog, std::slice::from_ref(&target));
    let recalled = [recalled(target.clone(), 0.6)];
    let project = snapshot("project-prerequisite");

    let baseline = CandidateRanker::default()
        .rank(
            &catalog,
            &graph,
            &recalled,
            &project,
            &RankingContext::default(),
        )
        .unwrap();
    let satisfied = CandidateRanker::default()
        .rank(
            &catalog,
            &graph,
            &recalled,
            &project,
            &RankingContext {
                prerequisites_satisfied: BTreeSet::from([target.clone()]),
                ..RankingContext::default()
            },
        )
        .unwrap();
    let blocked = CandidateRanker::default()
        .rank(
            &catalog,
            &graph,
            &recalled,
            &project,
            &RankingContext {
                prerequisites_blocked: BTreeSet::from([target.clone()]),
                ..RankingContext::default()
            },
        )
        .unwrap();

    assert!(
        ranked(&satisfied, &target).components.applicability
            > ranked(&baseline, &target).components.applicability
    );
    assert!(blocked
        .blocked
        .iter()
        .any(|candidate| candidate.capability_id == target));
    assert!(blocked
        .pareto
        .iter()
        .chain(&blocked.dominated)
        .all(|candidate| candidate.capability_id != target));
}

#[test]
fn removing_evidence_never_increases_confidence() {
    let catalog = Catalog::embedded().unwrap();
    let target = id("amari:amari-dual:autodiff:forward-derivative");
    let graph = expansion(&catalog, std::slice::from_ref(&target));
    let project = snapshot("project-evidence");
    let recalled = [recalled(target.clone(), 0.3)];

    let with_evidence = CandidateRanker::default()
        .rank(
            &catalog,
            &graph,
            &recalled,
            &project,
            &RankingContext {
                signals: vec![RankingSignal {
                    capability_id: target.clone(),
                    kind: RankingSignalKind::Evidence,
                    strength: 0.9,
                    summary: "typed project evidence".to_owned(),
                }],
                ..RankingContext::default()
            },
        )
        .unwrap();
    let without_evidence = CandidateRanker::default()
        .rank(
            &catalog,
            &graph,
            &recalled,
            &project,
            &RankingContext::default(),
        )
        .unwrap();

    assert!(
        ranked(&without_evidence, &target).confidence <= ranked(&with_evidence, &target).confidence
    );
}

#[test]
fn irrelevant_signals_do_not_reorder_candidates() {
    let catalog = Catalog::embedded().unwrap();
    let seeds = [
        id("amari:amari-dual:autodiff:forward-derivative"),
        id("amari:amari-surreal:rational:exact-arithmetic"),
    ];
    let graph = expansion(&catalog, &seeds);
    let project = snapshot("project-irrelevant");
    let recalled = [
        recalled(seeds[0].clone(), 0.5),
        recalled(seeds[1].clone(), 0.5),
    ];

    let baseline = CandidateRanker::default()
        .rank(
            &catalog,
            &graph,
            &recalled,
            &project,
            &RankingContext::default(),
        )
        .unwrap();
    let with_irrelevant = CandidateRanker::default()
        .rank(
            &catalog,
            &graph,
            &recalled,
            &project,
            &RankingContext {
                signals: vec![RankingSignal {
                    capability_id: id("amari:amari-tropical:sequence:viterbi"),
                    kind: RankingSignalKind::Evidence,
                    strength: 1.0,
                    summary: "unrelated".to_owned(),
                }],
                ..RankingContext::default()
            },
        )
        .unwrap();

    assert_eq!(
        baseline
            .preferred
            .as_ref()
            .map(|candidate| &candidate.capability_id),
        with_irrelevant
            .preferred
            .as_ref()
            .map(|candidate| &candidate.capability_id)
    );
    assert_eq!(
        baseline
            .pareto
            .iter()
            .map(|candidate| &candidate.capability_id)
            .collect::<Vec<_>>(),
        with_irrelevant
            .pareto
            .iter()
            .map(|candidate| &candidate.capability_id)
            .collect::<Vec<_>>()
    );
}

#[test]
fn pareto_alternatives_survive_and_preference_is_deterministic() {
    let catalog = Catalog::embedded().unwrap();
    let seeds = [
        id("amari:amari-core:product:geometric-product"),
        id("amari:amari-surreal:rational:exact-arithmetic"),
        id("amari:amari-rewrite:synthesis:infer-rule"),
    ];
    let graph = expansion(&catalog, &seeds);
    let project = snapshot("project-pareto");
    let recalled: Vec<_> = seeds
        .iter()
        .cloned()
        .map(|capability_id| recalled(capability_id, 0.6))
        .collect();

    let first = CandidateRanker::default()
        .rank(
            &catalog,
            &graph,
            &recalled,
            &project,
            &RankingContext::default(),
        )
        .unwrap();
    let second = CandidateRanker::default()
        .rank(
            &catalog,
            &graph,
            &recalled,
            &project,
            &RankingContext::default(),
        )
        .unwrap();

    assert!(first.pareto.len() >= 2);
    assert_eq!(
        serde_json::to_vec(&first).unwrap(),
        serde_json::to_vec(&second).unwrap()
    );
    assert_eq!(
        first.preferred.as_ref().unwrap().capability_id,
        first.pareto[0].capability_id
    );
}

#[test]
fn matching_probe_improves_verification_and_mismatched_provenance_is_ignored() {
    let catalog = Catalog::embedded().unwrap();
    let target = id("amari:amari-dual:autodiff:forward-derivative");
    let graph = expansion(&catalog, std::slice::from_ref(&target));
    let project = snapshot("project-probe");
    let recalled = [recalled(target.clone(), 0.5)];

    let baseline = CandidateRanker::default()
        .rank(
            &catalog,
            &graph,
            &recalled,
            &project,
            &RankingContext::default(),
        )
        .unwrap();
    let matching = CandidateRanker::default()
        .rank(
            &catalog,
            &graph,
            &recalled,
            &project,
            &RankingContext {
                probe_results: vec![probe(
                    &catalog,
                    &project.project_hash,
                    &["derivative_matches"],
                    &[],
                )],
                ..RankingContext::default()
            },
        )
        .unwrap();
    let mismatched = CandidateRanker::default()
        .rank(
            &catalog,
            &graph,
            &recalled,
            &project,
            &RankingContext {
                probe_results: vec![probe(
                    &catalog,
                    "different-project",
                    &["derivative_matches"],
                    &[],
                )],
                ..RankingContext::default()
            },
        )
        .unwrap();

    assert!(
        ranked(&matching, &target).components.verification
            > ranked(&baseline, &target).components.verification
    );
    assert!(ranked(&matching, &target).confidence > ranked(&baseline, &target).confidence);
    assert_eq!(
        ranked(&mismatched, &target).components.verification,
        ranked(&baseline, &target).components.verification
    );
    assert!(mismatched
        .warnings
        .iter()
        .any(|warning| warning.contains("project_hash_mismatch")));
}

#[test]
fn refuted_probe_assumptions_block_only_the_same_capability() {
    let catalog = Catalog::embedded().unwrap();
    let target = id("amari:amari-dual:autodiff:forward-derivative");
    let other = id("amari:amari-optimization:multiobjective:pareto-front");
    let graph = expansion(&catalog, std::slice::from_ref(&target));
    let project = snapshot("project-refuted");

    let result = CandidateRanker::default()
        .rank(
            &catalog,
            &graph,
            &[recalled(target.clone(), 0.5)],
            &project,
            &RankingContext {
                probe_results: vec![probe(
                    &catalog,
                    &project.project_hash,
                    &[],
                    &["derivative_contract"],
                )],
                ..RankingContext::default()
            },
        )
        .unwrap();

    assert!(result
        .blocked
        .iter()
        .any(|candidate| candidate.capability_id == target));
    assert!(result
        .pareto
        .iter()
        .chain(&result.dominated)
        .any(|candidate| candidate.capability_id == other));
}
