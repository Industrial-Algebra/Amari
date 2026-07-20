// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded holographic MAP256 probe adapters.

use serde::{Deserialize, Serialize};

#[cfg(feature = "standard-probes")]
use amari_holographic::{algebra::map::MAP256, BindingAlgebra};
#[cfg(feature = "standard-probes")]
use serde_json::Value;

#[cfg(feature = "standard-probes")]
use super::registry::{AdapterOutput, AdapterRegistration, EffectiveProbeLimits};
#[cfg(feature = "standard-probes")]
use crate::{DiscoveryError, DiscoveryResult, ProbeLimits, ResourceObservations, SideEffectPolicy};

#[cfg(feature = "standard-probes")]
const MAP_DIMENSION: u64 = 256;
#[cfg(feature = "standard-probes")]
const MAX_SUPERPOSITION_SEEDS: u64 = 256;

/// Typed request for deterministic additive MAP256 superposition.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HolographicSuperpositionRequest {
    /// Seeds converted deterministically with `MAP256::from_seed`.
    pub seeds: Vec<u64>,
}

/// Typed output from additive MAP256 superposition.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HolographicSuperpositionOutput {
    /// Additive trace coefficients in MAP component order.
    pub coefficients: Vec<f64>,
}

#[cfg(feature = "standard-probes")]
pub(super) fn superposition_registration() -> DiscoveryResult<AdapterRegistration> {
    Ok(AdapterRegistration {
        id: "amari-probe:holographic:superposition:v1".parse()?,
        capability_id: "amari:amari-holographic:algebra:superposition".parse()?,
        input_schema: "amari.discovery/probe/holographic-superposition/input/v1".to_owned(),
        output_schema: "amari.discovery/probe/holographic-superposition/output/v1".to_owned(),
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
        execute: execute_superposition,
    })
}

#[cfg(feature = "standard-probes")]
fn execute_superposition(
    input: &Value,
    limits: &EffectiveProbeLimits,
) -> DiscoveryResult<AdapterOutput> {
    let request: HolographicSuperpositionRequest =
        serde_json::from_value(input.clone()).map_err(|error| {
            DiscoveryError::InvalidInput(format!(
                "holographic superposition request requires an array of integer seeds: {error}"
            ))
        })?;
    let resources = validate_superposition(&request, limits)?;

    let mut trace = MAP256::zero();
    for seed in request.seeds {
        trace = trace.superpose(&MAP256::from_seed(seed)).map_err(|error| {
            DiscoveryError::ProbeFailed(format!("MAP256 additive superposition failed: {error}"))
        })?;
    }
    let coefficients = trace.components().to_vec();
    if coefficients
        .iter()
        .any(|coefficient| !coefficient.is_finite())
    {
        return Err(DiscoveryError::ProbeFailed(
            "MAP256 additive superposition produced a non-finite coefficient".to_owned(),
        ));
    }

    Ok(AdapterOutput {
        resources,
        output: serde_json::to_value(HolographicSuperpositionOutput { coefficients })?,
    })
}

#[cfg(feature = "standard-probes")]
fn validate_superposition(
    request: &HolographicSuperpositionRequest,
    limits: &EffectiveProbeLimits,
) -> DiscoveryResult<ResourceObservations> {
    if request.seeds.is_empty() {
        return Err(DiscoveryError::InvalidInput(
            "holographic superposition requires at least one seed".to_owned(),
        ));
    }
    let seed_count = u64::try_from(request.seeds.len())
        .map_err(|_| DiscoveryError::LimitExceeded("holographic seed count overflow".to_owned()))?;
    if seed_count > MAX_SUPERPOSITION_SEEDS {
        return Err(DiscoveryError::LimitExceeded(format!(
            "holographic seed count {seed_count} exceeds limit {MAX_SUPERPOSITION_SEEDS}"
        )));
    }
    let operations = seed_count.checked_mul(MAP_DIMENSION).ok_or_else(|| {
        DiscoveryError::LimitExceeded("holographic operation count overflow".to_owned())
    })?;
    let nodes = seed_count
        .checked_add(1)
        .and_then(|count| count.checked_mul(MAP_DIMENSION))
        .ok_or_else(|| {
            DiscoveryError::LimitExceeded("holographic node count overflow".to_owned())
        })?;
    enforce("operations", operations, limits.max_operations)?;
    enforce("nodes", nodes, limits.max_nodes)?;
    enforce("iterations", seed_count, limits.max_iterations)?;

    Ok(ResourceObservations {
        operations,
        nodes,
        iterations: seed_count,
        bytes: 0,
    })
}

#[cfg(feature = "standard-probes")]
fn enforce(kind: &str, observed: u64, maximum: u64) -> DiscoveryResult<()> {
    if observed <= maximum {
        Ok(())
    } else {
        Err(DiscoveryError::LimitExceeded(format!(
            "holographic {kind} {observed} exceeds limit {maximum}"
        )))
    }
}
