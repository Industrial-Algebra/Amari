// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded multi-objective Pareto-front probe adapter.

use serde::{Deserialize, Serialize};

#[cfg(feature = "standard-probes")]
use amari_optimization::multiobjective::{Individual, ParetoFront};
#[cfg(feature = "standard-probes")]
use serde_json::Value;

#[cfg(feature = "standard-probes")]
use super::registry::{AdapterOutput, AdapterRegistration, EffectiveProbeLimits};
#[cfg(feature = "standard-probes")]
use crate::{DiscoveryError, DiscoveryResult, ProbeLimits, ResourceObservations, SideEffectPolicy};

#[cfg(feature = "standard-probes")]
const MAX_POPULATION: u64 = 256;
#[cfg(feature = "standard-probes")]
const MAX_DIMENSIONS: u64 = 32;

/// Optimization direction for one objective dimension.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectiveDirection {
    /// Prefer lower objective values.
    Minimize,
    /// Prefer higher objective values.
    Maximize,
}

/// Typed request for extracting a Pareto front from objective vectors.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParetoFrontRequest {
    /// Objective vector for each candidate.
    pub objectives: Vec<Vec<f64>>,
    /// Direction for each objective dimension.
    pub directions: Vec<ObjectiveDirection>,
}

/// One non-dominated point in the original objective convention.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParetoPoint {
    /// Zero-based candidate index from the request.
    pub index: usize,
    /// Original, untransformed objective values.
    pub objectives: Vec<f64>,
}

/// Typed output from Pareto-front extraction.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParetoFrontOutput {
    /// Non-dominated points ordered by original candidate index.
    pub solutions: Vec<ParetoPoint>,
}

#[cfg(feature = "standard-probes")]
pub(super) fn registration() -> DiscoveryResult<AdapterRegistration> {
    Ok(AdapterRegistration {
        id: "amari-probe:optimization:pareto-front:v1".parse()?,
        capability_id: "amari:amari-optimization:multiobjective:pareto-front".parse()?,
        input_schema: "amari.discovery/probe/optimization-pareto-front/input/v1".to_owned(),
        output_schema: "amari.discovery/probe/optimization-pareto-front/output/v1".to_owned(),
        required_features: vec!["standard-probes".to_owned()],
        limits: ProbeLimits {
            max_input_bytes: 65_536,
            max_output_bytes: 65_536,
            max_operations: 100_000,
            timeout_millis: 2_000,
        },
        deterministic: true,
        side_effects: SideEffectPolicy::None,
        network: false,
        execute,
    })
}

#[cfg(feature = "standard-probes")]
fn execute(input: &Value, limits: &EffectiveProbeLimits) -> DiscoveryResult<AdapterOutput> {
    let request: ParetoFrontRequest = serde_json::from_value(input.clone()).map_err(|error| {
        DiscoveryError::InvalidInput(format!(
            "Pareto request requires a finite rectangular population and objective directions: {error}"
        ))
    })?;
    let resources = validate_request(&request, limits)?;

    let mut front = ParetoFront::new();
    for (index, objectives) in request.objectives.iter().enumerate() {
        let mut individual = Individual::new(vec![index as f64]);
        individual.objectives = objectives
            .iter()
            .zip(&request.directions)
            .map(|(objective, direction)| match direction {
                ObjectiveDirection::Minimize => *objective,
                ObjectiveDirection::Maximize => -*objective,
            })
            .collect();
        front.add_if_non_dominated(individual);
    }

    let mut indices = front
        .solutions
        .iter()
        .map(|individual| {
            let marker = individual.variables.first().copied().ok_or_else(|| {
                DiscoveryError::Internal("Pareto result lost its candidate marker".to_owned())
            })?;
            let index = marker as usize;
            if index >= request.objectives.len() || index as f64 != marker {
                return Err(DiscoveryError::Internal(
                    "Pareto result contains an invalid candidate marker".to_owned(),
                ));
            }
            Ok(index)
        })
        .collect::<DiscoveryResult<Vec<_>>>()?;
    indices.sort_unstable();
    let solutions = indices
        .into_iter()
        .map(|index| ParetoPoint {
            index,
            objectives: request.objectives[index].clone(),
        })
        .collect();

    Ok(AdapterOutput {
        resources,
        output: serde_json::to_value(ParetoFrontOutput { solutions })?,
    })
}

#[cfg(feature = "standard-probes")]
fn validate_request(
    request: &ParetoFrontRequest,
    limits: &EffectiveProbeLimits,
) -> DiscoveryResult<ResourceObservations> {
    if request.objectives.is_empty() {
        return Err(DiscoveryError::InvalidInput(
            "Pareto population must be non-empty".to_owned(),
        ));
    }
    let population = u64::try_from(request.objectives.len()).map_err(|_| {
        DiscoveryError::LimitExceeded("Pareto population count overflow".to_owned())
    })?;
    if population > MAX_POPULATION {
        return Err(DiscoveryError::LimitExceeded(format!(
            "Pareto population {population} exceeds limit {MAX_POPULATION}"
        )));
    }
    if request.directions.is_empty() {
        return Err(DiscoveryError::InvalidInput(
            "Pareto objective dimension must be non-zero".to_owned(),
        ));
    }
    let dimensions = u64::try_from(request.directions.len())
        .map_err(|_| DiscoveryError::LimitExceeded("Pareto dimension count overflow".to_owned()))?;
    if dimensions > MAX_DIMENSIONS {
        return Err(DiscoveryError::LimitExceeded(format!(
            "Pareto dimensions {dimensions} exceeds limit {MAX_DIMENSIONS}"
        )));
    }
    if request
        .objectives
        .iter()
        .any(|objectives| objectives.len() != request.directions.len())
    {
        return Err(DiscoveryError::InvalidInput(
            "every Pareto candidate must match the objective dimension".to_owned(),
        ));
    }
    if request
        .objectives
        .iter()
        .flatten()
        .any(|objective| !objective.is_finite())
    {
        return Err(DiscoveryError::InvalidInput(
            "Pareto objectives must be finite".to_owned(),
        ));
    }

    let nodes = population.checked_mul(dimensions).ok_or_else(|| {
        DiscoveryError::LimitExceeded("Pareto objective cell count overflow".to_owned())
    })?;
    let operations = population.checked_mul(nodes).ok_or_else(|| {
        DiscoveryError::LimitExceeded("Pareto operation count overflow".to_owned())
    })?;
    enforce("operations", operations, limits.max_operations)?;
    enforce("nodes", nodes, limits.max_nodes)?;
    enforce("iterations", population, limits.max_iterations)?;

    Ok(ResourceObservations {
        operations,
        nodes,
        iterations: population,
        bytes: 0,
    })
}

#[cfg(feature = "standard-probes")]
fn enforce(kind: &str, observed: u64, maximum: u64) -> DiscoveryResult<()> {
    if observed <= maximum {
        Ok(())
    } else {
        Err(DiscoveryError::LimitExceeded(format!(
            "Pareto {kind} {observed} exceeds limit {maximum}"
        )))
    }
}
