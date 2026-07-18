// SPDX-License-Identifier: MIT OR Apache-2.0

//! Validation and normalization of saved recommendation candidates.

use std::collections::BTreeSet;
use std::path::Path;

use super::recommend::read_bounded_input;
use crate::inspect::{inspect_supported_project, InspectionLimits};
use crate::planner::catalog_plan_steps;
use crate::{
    CandidatePlan, CapabilityId, Catalog, DiscoveryError, DiscoveryOutcome, DiscoveryResult,
    Envelope, PlanCompatibility, PlanNormalization, PlanNormalizer, ProjectKind, Recommendation,
    SCHEMA_V1,
};

const MAX_RECOMMENDATION_BYTES: u64 = 8 * 1_048_576;
const REQUIRED_REPLAY_HASHES: [&str; 4] = [
    "catalog_hash",
    "project_hash",
    "input_hash",
    "probe_results",
];

/// Reads a bounded saved recommendation artifact without following symlinks.
///
/// # Errors
///
/// Returns a typed I/O, limit, input, identifier, or serialization error for
/// an unreadable artifact, a non-regular path, or malformed protocol JSON.
pub fn read_recommendation(
    path: &Path,
) -> DiscoveryResult<Envelope<DiscoveryOutcome<Recommendation>>> {
    let bytes = read_bounded_input(path, MAX_RECOMMENDATION_BYTES, "recommendation")?;
    Ok(serde_json::from_slice(&bytes)?)
}

/// Validates and replays one candidate from a saved recommendation artifact.
///
/// Replay is read-only and offline. The current project is inspected again,
/// all catalog/project/input/probe hashes are validated, and the selected plan
/// is re-derived from the matched catalog and current snapshot before bounded
/// canonical normalization. No project command, probe, provider, shell, or
/// network operation is executed.
///
/// # Errors
///
/// Returns [`DiscoveryError::ReplayDrift`] for stale or internally inconsistent
/// provenance, [`DiscoveryError::InvalidInput`] for a non-replayable artifact
/// or unknown candidate, and typed inspection/normalization errors otherwise.
pub fn replay_plan_envelope(
    catalog: &Catalog,
    project_root: &Path,
    candidate_id: &str,
    artifact: Envelope<DiscoveryOutcome<Recommendation>>,
    limits: &InspectionLimits,
) -> DiscoveryResult<Envelope<CandidatePlan>> {
    let Envelope {
        schema_version,
        provenance,
        warnings,
        data,
    } = artifact;
    if schema_version != SCHEMA_V1 {
        return Err(DiscoveryError::InvalidInput(format!(
            "unsupported recommendation schema `{schema_version}`"
        )));
    }
    if !provenance.replay.replayable {
        return Err(DiscoveryError::InvalidInput(
            "recommendation artifact is not replayable".to_owned(),
        ));
    }
    let declared_hashes = provenance
        .replay
        .required_hashes
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let required_hashes = REQUIRED_REPLAY_HASHES.into_iter().collect::<BTreeSet<_>>();
    if declared_hashes != required_hashes
        || provenance.replay.required_hashes.len() != REQUIRED_REPLAY_HASHES.len()
    {
        return Err(DiscoveryError::InvalidInput(
            "recommendation replay metadata must declare exactly catalog_hash, project_hash, input_hash, and probe_results"
                .to_owned(),
        ));
    }

    let recommendation = match data {
        DiscoveryOutcome::Recommended(recommendation) => recommendation,
        DiscoveryOutcome::NoApplicableCapability { .. }
        | DiscoveryOutcome::InsufficientEvidence { .. }
        | DiscoveryOutcome::Blocked { .. } => {
            return Err(DiscoveryError::InvalidInput(
                "recommendation artifact does not contain candidate plans".to_owned(),
            ));
        }
    };
    recommendation.goal.validate()?;
    let candidate_id: CapabilityId = candidate_id.parse()?;
    let selected = select_candidate(&recommendation, &candidate_id)?;

    compare_replay(
        "catalog_version",
        catalog.version(),
        &provenance.catalog.version,
    )?;
    compare_replay(
        "catalog_hash",
        catalog.content_hash(),
        &provenance.catalog.hash,
    )?;
    compare_replay(
        "catalog_version",
        &provenance.catalog.version,
        &selected.compatibility.catalog.version,
    )?;
    compare_replay(
        "catalog_hash",
        &provenance.catalog.hash,
        &selected.compatibility.catalog.hash,
    )?;
    compare_optional_replay(
        "project_hash",
        provenance.project_hash.as_deref(),
        &selected.compatibility.project_hash,
    )?;
    compare_optional_replay(
        "input_hash",
        provenance.input_hash.as_deref(),
        &selected.compatibility.input_hash,
    )?;
    validate_sha256("plan_hash", &selected.plan_hash)?;
    for replay_hash in &selected.compatibility.probe_results {
        validate_sha256("probe_results", &replay_hash.result_hash)?;
    }

    let snapshot = inspect_supported_project(project_root, limits)?;
    if matches!(snapshot.project_kind, ProjectKind::Unknown) {
        return Err(DiscoveryError::InvalidInput(
            "plan replay requires a Rust/Cargo or npm/TypeScript project".to_owned(),
        ));
    }
    let actual = PlanCompatibility::from_replay_hashes(
        catalog,
        snapshot.project_hash.clone(),
        &recommendation.goal,
        selected.compatibility.probe_results.clone(),
    )?;
    selected.verify_replay(&actual)?;

    let expected_draft = CandidatePlan {
        capability_id: selected.capability_id.clone(),
        prerequisite_order: selected.prerequisite_order.clone(),
        steps: catalog_plan_steps(catalog, &snapshot, &selected.prerequisite_order)?,
        compatibility: selected.compatibility.clone(),
        normalization: PlanNormalization::default(),
        plan_hash: String::new(),
    };
    let expected = PlanNormalizer::default().normalize(&expected_draft)?;
    if expected.steps != selected.steps {
        return Err(DiscoveryError::ReplayDrift {
            field: "plan_steps".to_owned(),
            expected: expected.plan_hash,
            actual: selected.plan_hash,
        });
    }
    if expected.normalization != selected.normalization {
        return Err(DiscoveryError::ReplayDrift {
            field: "normalization".to_owned(),
            expected: "catalog-derived normalization trace".to_owned(),
            actual: "saved normalization trace differs".to_owned(),
        });
    }
    if expected.plan_hash != selected.plan_hash {
        return Err(DiscoveryError::ReplayDrift {
            field: "plan_hash".to_owned(),
            expected: expected.plan_hash,
            actual: selected.plan_hash,
        });
    }

    Ok(Envelope {
        schema_version,
        provenance,
        warnings,
        data: expected,
    })
}

fn select_candidate(
    recommendation: &Recommendation,
    candidate_id: &CapabilityId,
) -> DiscoveryResult<CandidatePlan> {
    std::iter::once(&recommendation.preferred)
        .chain(&recommendation.alternatives)
        .find(|plan| plan.capability_id == *candidate_id)
        .cloned()
        .ok_or_else(|| {
            DiscoveryError::InvalidInput(format!(
                "unknown candidate `{candidate_id}` in recommendation artifact"
            ))
        })
}

fn compare_optional_replay(
    field: &str,
    artifact: Option<&str>,
    selected: &str,
) -> DiscoveryResult<()> {
    let artifact = artifact.ok_or_else(|| {
        DiscoveryError::InvalidInput(format!("recommendation provenance is missing `{field}`"))
    })?;
    compare_replay(field, artifact, selected)
}

fn compare_replay(field: &str, expected: &str, actual: &str) -> DiscoveryResult<()> {
    if expected == actual {
        Ok(())
    } else {
        Err(DiscoveryError::ReplayDrift {
            field: field.to_owned(),
            expected: expected.to_owned(),
            actual: actual.to_owned(),
        })
    }
}

fn validate_sha256(field: &str, value: &str) -> DiscoveryResult<()> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(DiscoveryError::ReplayDrift {
            field: field.to_owned(),
            expected: "64 lowercase hexadecimal SHA-256 characters".to_owned(),
            actual: value.to_owned(),
        })
    }
}
