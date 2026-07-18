// SPDX-License-Identifier: MIT OR Apache-2.0

//! Typed Rust-project recommendation pipeline.

use std::fs;
use std::io::Read;
use std::path::Path;

use crate::inspect::{
    inspect_rust_project, nofollow_open_readonly, snapshot_compatibility, InspectionLimits,
    NofollowResult,
};
use crate::{
    CandidateRanker, CandidateRetriever, CapabilityGraphExpander, Catalog, CatalogIdentity,
    DiscoveryError, DiscoveryOutcome, DiscoveryResult, Envelope, Evidence, GoalSpec,
    GraphConstraints, GraphExpansionState, PlanCompatibility, PlanGenerator, PlanStep,
    PlanningContext, ProbeId, ProbeResult, ProjectKind, RankedCandidate, RankingContext,
    RecallConfig, Recommendation, RecommendationScore, RecommendationScoreComponents,
    ReplayMetadata, SnapshotState, SCHEMA_V1,
};

const MAX_PROBE_RESULTS_BYTES: u64 = 1_048_576;
const MAX_PROBE_RESULTS: usize = 256;
const MAX_ALTERNATIVES: usize = 7;

/// Reads a bounded JSON array of saved [`ProbeResult`] values.
///
/// The selected path must be a regular file no larger than one mebibyte.
/// Project files are never modified and probe code is never executed.
///
/// # Errors
///
/// Returns [`DiscoveryError::InvalidInput`] for non-regular or oversized
/// inputs, an I/O error when the file cannot be read, or a serialization error
/// for malformed JSON.
pub fn read_probe_results(path: &Path) -> DiscoveryResult<Vec<ProbeResult>> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() {
        return Err(DiscoveryError::InvalidInput(
            "probe-results input must be a regular file".to_owned(),
        ));
    }
    if metadata.len() > MAX_PROBE_RESULTS_BYTES {
        return Err(DiscoveryError::LimitExceeded(format!(
            "probe-results bytes {} exceed limit {MAX_PROBE_RESULTS_BYTES}",
            metadata.len()
        )));
    }
    let mut file = match nofollow_open_readonly(path)? {
        NofollowResult::Opened(file) => file,
        NofollowResult::SymlinkOrRace => {
            return Err(DiscoveryError::InvalidInput(
                "probe-results input must not be a symlink or replaced file".to_owned(),
            ));
        }
    };
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.by_ref()
        .take(MAX_PROBE_RESULTS_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_PROBE_RESULTS_BYTES {
        return Err(DiscoveryError::LimitExceeded(format!(
            "probe-results bytes {} exceed limit {MAX_PROBE_RESULTS_BYTES}",
            bytes.len()
        )));
    }
    let results: Vec<ProbeResult> = serde_json::from_slice(&bytes)?;
    if results.len() > MAX_PROBE_RESULTS {
        return Err(DiscoveryError::LimitExceeded(format!(
            "probe-results count {} exceeds limit {MAX_PROBE_RESULTS}",
            results.len()
        )));
    }
    Ok(results)
}

/// Runs the deterministic Rust inspect → recall → graph → rank → plan pipeline.
///
/// The operation is read-only and offline. It does not invoke Cargo, rustc,
/// build scripts, project code, a shell, providers, probes, or the network.
/// Saved probe results are treated only as provenance-checked typed evidence.
///
/// # Errors
///
/// Returns a typed inspection, planner, catalog, or normalization error. npm-only
/// and unknown project roots are rejected until their dedicated vertical slice.
pub fn recommend_rust_envelope(
    catalog: &Catalog,
    project_root: &Path,
    goal: GoalSpec,
    probe_results: Vec<ProbeResult>,
    limits: &InspectionLimits,
) -> DiscoveryResult<Envelope<DiscoveryOutcome<Recommendation>>> {
    if goal.statement.trim().is_empty() {
        return Err(DiscoveryError::InvalidInput(
            "recommendation goal must not be empty".to_owned(),
        ));
    }

    let snapshot = inspect_rust_project(project_root, limits)?;
    if !matches!(
        snapshot.project_kind,
        ProjectKind::RustCargo | ProjectKind::Mixed
    ) {
        return Err(DiscoveryError::InvalidInput(
            "recommend currently requires a Rust/Cargo project".to_owned(),
        ));
    }
    let compatibility = snapshot_compatibility(&snapshot);
    let planning_context = PlanningContext {
        snapshot,
        goal: goal.clone(),
        probe_results,
    };
    let plan_compatibility = PlanCompatibility::from_context(catalog, &planning_context)?;
    let recall_config = RecallConfig::default();
    let recalled = CandidateRetriever::new(recall_config).retrieve(
        catalog,
        &planning_context.snapshot,
        &goal.statement,
    )?;

    if recalled.is_empty() {
        return Ok(outcome_envelope(
            catalog,
            &planning_context,
            compatibility,
            plan_compatibility.input_hash.clone(),
            recall_config.seed,
            Vec::new(),
            DiscoveryOutcome::InsufficientEvidence {
                missing: vec!["goal and project evidence matched no catalog capability".to_owned()],
            },
        ));
    }

    let seeds = recalled
        .iter()
        .map(|candidate| candidate.capability_id.clone())
        .collect::<Vec<_>>();
    let graph =
        CapabilityGraphExpander::default().expand(catalog, &seeds, &GraphConstraints::default())?;
    let ranking = CandidateRanker::default().rank(
        catalog,
        &graph,
        &recalled,
        &planning_context.snapshot,
        &RankingContext {
            probe_results: planning_context.probe_results.clone(),
            ..RankingContext::default()
        },
    )?;

    let Some(preferred_ranked) = ranking.preferred.as_ref() else {
        let warnings = ranking.warnings;
        let outcome = if ranking.blocked.is_empty() {
            DiscoveryOutcome::NoApplicableCapability {
                evidence: recalled
                    .iter()
                    .map(|candidate| Evidence {
                        kind: "candidate_recall".to_owned(),
                        summary: format!(
                            "{} recall score {:.12}",
                            candidate.capability_id, candidate.retrieval_score
                        ),
                        source: Some(candidate.capability_id.to_string()),
                        weight: candidate.retrieval_score,
                    })
                    .collect(),
            }
        } else {
            DiscoveryOutcome::Blocked {
                reasons: ranking
                    .blocked
                    .iter()
                    .flat_map(|candidate| {
                        candidate
                            .reasons
                            .iter()
                            .map(|reason| format!("{}:{reason}", candidate.capability_id))
                    })
                    .collect(),
            }
        };
        return Ok(outcome_envelope(
            catalog,
            &planning_context,
            compatibility,
            plan_compatibility.input_hash.clone(),
            recall_config.seed,
            warnings,
            outcome,
        ));
    };

    let generator = PlanGenerator::default();
    let preferred = generator.generate(catalog, &planning_context, preferred_ranked)?;
    let alternative_ranked = ranking
        .pareto
        .iter()
        .filter(|candidate| candidate.capability_id != preferred_ranked.capability_id)
        .take(MAX_ALTERNATIVES)
        .collect::<Vec<_>>();
    let alternatives = alternative_ranked
        .iter()
        .map(|candidate| generator.generate(catalog, &planning_context, candidate))
        .collect::<DiscoveryResult<Vec<_>>>()?;

    let mut selected_ranked = vec![preferred_ranked];
    selected_ranked.extend(alternative_ranked);
    let scores = selected_ranked
        .into_iter()
        .map(recommendation_score)
        .collect();
    let suggested_probes = preferred
        .steps
        .iter()
        .filter_map(|step| match step {
            PlanStep::Probe { probe_id, .. } => Some(probe_id.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let suggested_tests = preferred
        .steps
        .iter()
        .filter(|step| matches!(step, PlanStep::Test { .. }))
        .cloned()
        .collect::<Vec<_>>();
    let missing_information = missing_information(
        catalog,
        &planning_context,
        preferred_ranked,
        &suggested_probes,
        &graph.state,
    );
    let evidence = preferred_ranked.evidence.clone();
    let mut warnings = planning_context.snapshot.warnings.clone();
    warnings.extend(ranking.warnings.clone());
    if ranking.pareto.len().saturating_sub(1) > MAX_ALTERNATIVES {
        warnings.push(format!(
            "pareto alternatives truncated to {MAX_ALTERNATIVES}"
        ));
    }
    warnings.sort();
    warnings.dedup();

    let input_hash = plan_compatibility.input_hash;
    let recommendation = Recommendation {
        goal,
        preferred,
        alternatives,
        scores,
        evidence,
        missing_information,
        suggested_probes,
        suggested_tests,
        warnings: warnings.clone(),
    };
    Ok(outcome_envelope(
        catalog,
        &planning_context,
        compatibility,
        input_hash,
        recall_config.seed,
        warnings,
        DiscoveryOutcome::Recommended(recommendation),
    ))
}

fn recommendation_score(candidate: &RankedCandidate) -> RecommendationScore {
    RecommendationScore {
        capability_id: candidate.capability_id.clone(),
        components: RecommendationScoreComponents {
            applicability: candidate.components.applicability,
            evidence: candidate.components.evidence,
            effort: candidate.components.effort,
            maturity: candidate.components.maturity,
            runtime: candidate.components.runtime,
            platform: candidate.components.platform,
            verification: candidate.components.verification,
            risk: candidate.components.risk,
        },
        objectives: candidate.objectives,
        confidence: candidate.confidence,
        evidence: candidate.evidence.clone(),
        validated_assumptions: candidate.validated_assumptions.clone(),
    }
}

fn missing_information(
    catalog: &Catalog,
    context: &PlanningContext,
    preferred: &RankedCandidate,
    probes: &[ProbeId],
    graph_state: &GraphExpansionState,
) -> Vec<String> {
    let mut missing = Vec::new();
    if !matches!(context.snapshot.state, SnapshotState::Complete) {
        missing.push("project_inspection_partial".to_owned());
    }
    if !matches!(graph_state, GraphExpansionState::Complete) {
        missing.push("capability_graph_partial".to_owned());
    }
    for probe_id in probes {
        let matched = context.probe_results.iter().any(|result| {
            result.probe_id == *probe_id
                && result.catalog_hash == catalog.content_hash()
                && result.project_hash.as_deref() == Some(context.snapshot.project_hash.as_str())
        });
        if !matched {
            missing.push(format!("missing_probe_result:{probe_id}"));
        }
    }
    if preferred.components.verification < 1.0 {
        missing.push(format!("verification_partial:{}", preferred.capability_id));
    }
    missing.sort();
    missing.dedup();
    missing
}

fn outcome_envelope<T>(
    catalog: &Catalog,
    context: &PlanningContext,
    compatibility: crate::Compatibility,
    input_hash: String,
    seed: u64,
    mut warnings: Vec<String>,
    data: T,
) -> Envelope<T> {
    warnings.sort();
    warnings.dedup();
    Envelope {
        schema_version: SCHEMA_V1.to_owned(),
        provenance: crate::Provenance {
            tool_version: env!("CARGO_PKG_VERSION").to_owned(),
            catalog: CatalogIdentity {
                version: catalog.version().to_owned(),
                hash: catalog.content_hash().to_owned(),
            },
            compatibility,
            replay: ReplayMetadata {
                replayable: true,
                required_hashes: vec![
                    "catalog_hash".to_owned(),
                    "project_hash".to_owned(),
                    "input_hash".to_owned(),
                    "probe_results".to_owned(),
                ],
                reasons: Vec::new(),
            },
            project_hash: Some(context.snapshot.project_hash.clone()),
            input_hash: Some(input_hash),
            seed: Some(seed),
        },
        warnings,
        data,
    }
}
