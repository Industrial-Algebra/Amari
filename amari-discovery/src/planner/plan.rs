// SPDX-License-Identifier: MIT OR Apache-2.0

//! Deterministic construction and replay validation for candidate plans.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use sha2::{Digest, Sha256};

use super::normalize::{NormalizationLimits, PlanNormalizer};
use crate::{
    CandidatePlan, CapabilityId, Catalog, CatalogIdentity, DiscoveryError, DiscoveryResult,
    PlanCompatibility, PlanNormalization, PlanStep, PlanTestTarget, PlanningContext,
    ProbeReplayHash, ProjectKind, RankedCandidate,
};

const AMARI_WASM_PACKAGE: &str = "@justinelliottcobb/amari-wasm";

/// Deterministic catalog-backed candidate-plan generator.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlanGenerator {
    normalizer: PlanNormalizer,
}

impl PlanGenerator {
    /// Creates a plan generator with explicit normalization limits.
    ///
    /// # Errors
    ///
    /// Returns [`DiscoveryError::InvalidInput`] when either limit is zero.
    pub fn new(limits: NormalizationLimits) -> DiscoveryResult<Self> {
        Ok(Self {
            normalizer: PlanNormalizer::new(limits)?,
        })
    }

    /// Generates and normalizes a replayable plan for one ranked candidate.
    ///
    /// Every dependency version, feature, symbol, example, and probe comes
    /// directly from the validated catalog. A static package test step is
    /// emitted for each referenced crate. No project command is executed.
    ///
    /// # Errors
    ///
    /// Returns [`DiscoveryError::InvalidInput`] for an empty goal or malformed
    /// candidate path, [`DiscoveryError::CatalogCorruption`] for unresolved
    /// catalog references, and a bounded normalization error when the generated
    /// plan cannot reach a fixed point within default limits.
    pub fn generate(
        &self,
        catalog: &Catalog,
        context: &PlanningContext,
        candidate: &RankedCandidate,
    ) -> DiscoveryResult<CandidatePlan> {
        context.goal.validate()?;
        let prerequisite_order = prerequisite_order(candidate)?;
        let capabilities: BTreeMap<_, _> = catalog
            .capabilities()
            .iter()
            .map(|record| (record.id.clone(), record))
            .collect();
        let crates: BTreeMap<_, _> = catalog
            .crates()
            .iter()
            .map(|record| (record.name.as_str(), record))
            .collect();

        let rust_project = matches!(
            context.snapshot.project_kind,
            ProjectKind::RustCargo | ProjectKind::Mixed
        );
        let npm_project = matches!(
            context.snapshot.project_kind,
            ProjectKind::NpmTypeScript | ProjectKind::Mixed
        );
        if !rust_project && !npm_project {
            return Err(DiscoveryError::InvalidInput(
                "candidate plan requires Rust/Cargo or npm project evidence".to_owned(),
            ));
        }

        let mut steps = Vec::new();
        for capability_id in &prerequisite_order {
            let capability = capabilities.get(capability_id).ok_or_else(|| {
                DiscoveryError::InvalidInput(format!(
                    "candidate path references unknown capability `{capability_id}`"
                ))
            })?;
            let mut actionable = false;
            if rust_project {
                let previous_len = steps.len();
                append_rust_steps(&mut steps, capability_id, capability, &crates)?;
                actionable |= steps.len() > previous_len;
            }
            if npm_project {
                let wasm_paths = context
                    .snapshot
                    .typescript
                    .as_ref()
                    .into_iter()
                    .flat_map(|typescript| &typescript.capabilities)
                    .filter(|evidence| evidence.capability_id == *capability_id)
                    .map(|evidence| evidence.wasm_path.clone())
                    .collect::<BTreeSet<_>>();
                if !wasm_paths.is_empty() {
                    append_npm_steps(&mut steps, capability_id, catalog.version(), wasm_paths);
                    actionable = true;
                }
            }
            if actionable {
                for probe_id in &capability.probe_refs {
                    steps.push(PlanStep::Probe {
                        capability_id: capability_id.clone(),
                        probe_id: probe_id.clone(),
                    });
                }
            }
        }

        let compatibility = PlanCompatibility::from_context(catalog, context)?;
        let draft = CandidatePlan {
            capability_id: candidate.capability_id.clone(),
            prerequisite_order,
            steps,
            compatibility,
            normalization: PlanNormalization::default(),
            plan_hash: String::new(),
        };
        self.normalizer.normalize(&draft)
    }
}

impl PlanCompatibility {
    /// Computes canonical catalog, project, goal, and saved-probe replay hashes.
    ///
    /// This constructor is shared by recommendation envelopes and generated
    /// plans so every outcome uses one stable `input_hash` definition.
    ///
    /// # Errors
    ///
    /// Returns a serialization error if typed goal or probe evidence cannot be
    /// encoded for deterministic hashing.
    pub fn from_context(catalog: &Catalog, context: &PlanningContext) -> DiscoveryResult<Self> {
        compatibility(catalog, context)
    }
}

impl CandidatePlan {
    /// Validates all replay-required hashes against current inputs.
    ///
    /// # Errors
    ///
    /// Returns [`DiscoveryError::ReplayDrift`] naming the first mismatched
    /// catalog, project, input, or saved-probe hash field.
    pub fn verify_replay(&self, actual: &PlanCompatibility) -> DiscoveryResult<()> {
        compare_replay(
            "catalog_version",
            &self.compatibility.catalog.version,
            &actual.catalog.version,
        )?;
        compare_replay(
            "catalog_hash",
            &self.compatibility.catalog.hash,
            &actual.catalog.hash,
        )?;
        compare_replay(
            "project_hash",
            &self.compatibility.project_hash,
            &actual.project_hash,
        )?;
        compare_replay(
            "input_hash",
            &self.compatibility.input_hash,
            &actual.input_hash,
        )?;
        if self.compatibility.probe_results != actual.probe_results {
            return Err(DiscoveryError::ReplayDrift {
                field: "probe_results".to_owned(),
                expected: hash_serializable(&self.compatibility.probe_results)?,
                actual: hash_serializable(&actual.probe_results)?,
            });
        }
        Ok(())
    }
}

fn prerequisite_order(candidate: &RankedCandidate) -> DiscoveryResult<Vec<CapabilityId>> {
    if candidate.path.capabilities.is_empty()
        || candidate.path.target != candidate.capability_id
        || candidate.path.capabilities.last() != Some(&candidate.capability_id)
    {
        return Err(DiscoveryError::InvalidInput(
            "ranked candidate requires a nonempty path ending at its capability ID".to_owned(),
        ));
    }
    let mut seen = BTreeSet::new();
    Ok(candidate
        .path
        .capabilities
        .iter()
        .filter(|capability| seen.insert((*capability).clone()))
        .cloned()
        .collect())
}

fn append_rust_steps(
    steps: &mut Vec<PlanStep>,
    capability_id: &CapabilityId,
    capability: &crate::CapabilityRecord,
    crates: &BTreeMap<&str, &crate::CrateRecord>,
) -> DiscoveryResult<()> {
    for package in &capability.crate_refs {
        let record = crates.get(package.as_str()).ok_or_else(|| {
            DiscoveryError::CatalogCorruption(format!(
                "capability `{capability_id}` references unknown crate `{package}`"
            ))
        })?;
        steps.push(PlanStep::Dependency {
            capability_id: capability_id.clone(),
            package: package.clone(),
            version: record.version.clone(),
        });
    }
    for reference in &capability.feature_refs {
        let (package, feature) = split_catalog_reference(reference, "feature")?;
        steps.push(PlanStep::Feature {
            capability_id: capability_id.clone(),
            package: package.to_owned(),
            feature: feature.to_owned(),
        });
    }
    for path in &capability.symbol_refs {
        steps.push(PlanStep::Symbol {
            capability_id: capability_id.clone(),
            path: path.clone(),
        });
    }
    for reference in &capability.example_refs {
        let (package, example) = split_catalog_reference(reference, "example")?;
        steps.push(PlanStep::Example {
            capability_id: capability_id.clone(),
            package: package.to_owned(),
            example: example.to_owned(),
        });
    }
    for package in &capability.crate_refs {
        steps.push(PlanStep::Test {
            capability_id: capability_id.clone(),
            package: package.clone(),
            target: PlanTestTarget::AllTargets,
        });
    }
    Ok(())
}

fn append_npm_steps(
    steps: &mut Vec<PlanStep>,
    capability_id: &CapabilityId,
    version: &str,
    wasm_paths: BTreeSet<String>,
) {
    steps.push(PlanStep::Dependency {
        capability_id: capability_id.clone(),
        package: AMARI_WASM_PACKAGE.to_owned(),
        version: version.to_owned(),
    });
    steps.extend(wasm_paths.into_iter().map(|path| PlanStep::Symbol {
        capability_id: capability_id.clone(),
        path,
    }));
    steps.push(PlanStep::Test {
        capability_id: capability_id.clone(),
        package: AMARI_WASM_PACKAGE.to_owned(),
        target: PlanTestTarget::NpmPackage,
    });
}

fn split_catalog_reference<'a>(
    reference: &'a str,
    kind: &str,
) -> DiscoveryResult<(&'a str, &'a str)> {
    reference
        .split_once(':')
        .filter(|(package, item)| !package.is_empty() && !item.is_empty())
        .ok_or_else(|| {
            DiscoveryError::CatalogCorruption(format!("malformed {kind} reference `{reference}`"))
        })
}

fn compatibility(
    catalog: &Catalog,
    context: &PlanningContext,
) -> DiscoveryResult<PlanCompatibility> {
    let mut probe_results = context
        .probe_results
        .iter()
        .map(|result| {
            Ok(ProbeReplayHash {
                probe_id: result.probe_id.clone(),
                input_hash: result.input_hash.clone(),
                result_hash: hash_serializable(result)?,
            })
        })
        .collect::<DiscoveryResult<Vec<_>>>()?;
    probe_results.sort();
    probe_results.dedup();

    let mut constraints = context.goal.constraints.clone();
    constraints.sort();
    constraints.dedup();
    let canonical_goal = CanonicalGoal {
        statement: context.goal.statement.trim(),
        constraints: &constraints,
        probe_results: &probe_results,
    };

    Ok(PlanCompatibility {
        catalog: CatalogIdentity {
            version: catalog.version().to_owned(),
            hash: catalog.content_hash().to_owned(),
        },
        project_hash: context.snapshot.project_hash.clone(),
        input_hash: hash_serializable(&canonical_goal)?,
        probe_results,
    })
}

#[derive(Serialize)]
struct CanonicalGoal<'a> {
    statement: &'a str,
    constraints: &'a [String],
    probe_results: &'a [ProbeReplayHash],
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

pub(super) fn hash_serializable(value: &impl Serialize) -> DiscoveryResult<String> {
    let bytes = serde_json::to_vec(value)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
}

#[derive(Serialize)]
struct PlanHashView<'a> {
    capability_id: &'a CapabilityId,
    prerequisite_order: &'a [CapabilityId],
    steps: &'a [PlanStep],
    compatibility: &'a PlanCompatibility,
}

pub(super) fn compute_plan_hash(plan: &CandidatePlan) -> DiscoveryResult<String> {
    hash_serializable(&PlanHashView {
        capability_id: &plan.capability_id,
        prerequisite_order: &plan.prerequisite_order,
        steps: &plan.steps,
        compatibility: &plan.compatibility,
    })
}
