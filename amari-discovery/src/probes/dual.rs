// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded dual-number polynomial derivative probe adapter.

use serde::{Deserialize, Serialize};

#[cfg(feature = "standard-probes")]
use amari_dual::DualNumber;
#[cfg(feature = "standard-probes")]
use serde_json::Value;

#[cfg(feature = "standard-probes")]
use super::registry::{AdapterOutput, AdapterRegistration, EffectiveProbeLimits};
#[cfg(feature = "standard-probes")]
use crate::{DiscoveryError, DiscoveryResult, ProbeLimits, ResourceObservations, SideEffectPolicy};

#[cfg(feature = "standard-probes")]
const MAX_OPERATIONS: u64 = 10_000;
#[cfg(feature = "standard-probes")]
const OPERATIONS_PER_COEFFICIENT: u64 = 2;
#[cfg(feature = "standard-probes")]
const MAX_COEFFICIENTS: u64 = MAX_OPERATIONS / OPERATIONS_PER_COEFFICIENT;

/// Typed input for dual-number evaluation of a scalar polynomial and derivative.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolynomialDerivativeRequest {
    /// Polynomial coefficients in descending-power order.
    pub coefficients: Vec<f64>,
    /// Point at which to evaluate the polynomial and its first derivative.
    pub at: f64,
}

/// Typed output from dual-number polynomial evaluation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PolynomialDerivativeOutput {
    /// Polynomial value at the requested point.
    pub value: f64,
    /// Exact forward-mode first derivative at the requested point.
    pub derivative: f64,
}

#[cfg(feature = "standard-probes")]
pub(super) fn registration() -> DiscoveryResult<AdapterRegistration> {
    Ok(AdapterRegistration {
        id: "amari-probe:dual:polynomial-derivative:v1".parse()?,
        capability_id: "amari:amari-dual:autodiff:forward-derivative".parse()?,
        input_schema: "amari.discovery/probe/dual-polynomial-derivative/input/v1".to_owned(),
        output_schema: "amari.discovery/probe/dual-polynomial-derivative/output/v1".to_owned(),
        required_features: vec!["standard-probes".to_owned()],
        limits: ProbeLimits {
            max_input_bytes: 16_384,
            max_output_bytes: 4_096,
            max_operations: MAX_OPERATIONS,
            timeout_millis: 1_000,
        },
        deterministic: true,
        side_effects: SideEffectPolicy::None,
        network: false,
        execute,
    })
}

#[cfg(feature = "standard-probes")]
fn execute(input: &Value, limits: &EffectiveProbeLimits) -> DiscoveryResult<AdapterOutput> {
    let request: PolynomialDerivativeRequest =
        serde_json::from_value(input.clone()).map_err(|error| {
            DiscoveryError::InvalidInput(format!(
                "polynomial derivative request requires finite coefficients and evaluation point: {error}"
            ))
        })?;
    let resources = validate_request(&request, limits)?;

    let point = DualNumber::variable(request.at);
    let result = request
        .coefficients
        .iter()
        .copied()
        .fold(DualNumber::constant(0.0), |accumulator, coefficient| {
            accumulator * point + DualNumber::constant(coefficient)
        });
    if !result.value().is_finite() || !result.derivative().is_finite() {
        return Err(DiscoveryError::ProbeFailed(
            "dual Horner evaluation produced a non-finite value or derivative".to_owned(),
        ));
    }

    Ok(AdapterOutput {
        resources,
        output: serde_json::to_value(PolynomialDerivativeOutput {
            value: result.value(),
            derivative: result.derivative(),
        })?,
    })
}

#[cfg(feature = "standard-probes")]
fn validate_request(
    request: &PolynomialDerivativeRequest,
    limits: &EffectiveProbeLimits,
) -> DiscoveryResult<ResourceObservations> {
    if request.coefficients.is_empty() {
        return Err(DiscoveryError::InvalidInput(
            "polynomial requires at least one coefficient".to_owned(),
        ));
    }
    let coefficient_count = u64::try_from(request.coefficients.len()).map_err(|_| {
        DiscoveryError::LimitExceeded("polynomial coefficient count overflow".to_owned())
    })?;
    if coefficient_count > MAX_COEFFICIENTS {
        return Err(DiscoveryError::LimitExceeded(format!(
            "polynomial coefficient count {coefficient_count} exceeds limit {MAX_COEFFICIENTS}"
        )));
    }
    if !request.at.is_finite()
        || request
            .coefficients
            .iter()
            .any(|coefficient| !coefficient.is_finite())
    {
        return Err(DiscoveryError::InvalidInput(
            "polynomial coefficients and evaluation point must be finite".to_owned(),
        ));
    }

    let operations = coefficient_count
        .checked_mul(OPERATIONS_PER_COEFFICIENT)
        .ok_or_else(|| {
            DiscoveryError::LimitExceeded("dual Horner operation count overflow".to_owned())
        })?;
    let nodes = coefficient_count.checked_add(3).ok_or_else(|| {
        DiscoveryError::LimitExceeded("dual Horner node count overflow".to_owned())
    })?;
    enforce("operations", operations, limits.max_operations)?;
    enforce("nodes", nodes, limits.max_nodes)?;
    enforce("iterations", coefficient_count, limits.max_iterations)?;

    Ok(ResourceObservations {
        operations,
        nodes,
        iterations: coefficient_count,
        bytes: 0,
    })
}

#[cfg(feature = "standard-probes")]
fn enforce(kind: &str, observed: u64, maximum: u64) -> DiscoveryResult<()> {
    if observed <= maximum {
        Ok(())
    } else {
        Err(DiscoveryError::LimitExceeded(format!(
            "dual Horner {kind} {observed} exceeds limit {maximum}"
        )))
    }
}
