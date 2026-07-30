// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded Cl(3,0,0) geometric-product probe adapter.

use serde::{Deserialize, Serialize};

#[cfg(feature = "standard-probes")]
use amari_core::Multivector;
#[cfg(feature = "standard-probes")]
use serde_json::Value;

#[cfg(feature = "standard-probes")]
use super::registry::{AdapterOutput, AdapterRegistration, EffectiveProbeLimits};
#[cfg(feature = "standard-probes")]
use crate::{DiscoveryError, DiscoveryResult, ProbeLimits, ResourceObservations, SideEffectPolicy};

#[cfg(feature = "standard-probes")]
const COEFFICIENT_COUNT: u64 = 8;
#[cfg(feature = "standard-probes")]
const PRODUCT_OPERATIONS: u64 = COEFFICIENT_COUNT * COEFFICIENT_COUNT;
#[cfg(feature = "standard-probes")]
const PRODUCT_NODES: u64 = COEFFICIENT_COUNT * 3;
#[cfg(feature = "standard-probes")]
const PRODUCT_ITERATIONS: u64 = PRODUCT_OPERATIONS;

/// Typed input for a geometric product in Euclidean Cl(3,0,0).
#[derive(
    Clone,
    Debug,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[serde(deny_unknown_fields)]
#[wire_contract(
    id = "amari.discovery/probe/core-geometric-product/input/v1",
    role = "input",
    compatibility = "additive_patch",
    constraints(
        finite_numbers = "all Cl(3,0,0) input coefficients must be finite",
        fixed_coefficient_length = "each operand must contain exactly eight coefficients in binary basis-blade order"
    ),
    example(
        label = "basis_product",
        value = "{\"left\":[0.0,1.0,0.0,0.0,0.0,0.0,0.0,0.0],\"right\":[0.0,0.0,1.0,0.0,0.0,0.0,0.0,0.0]}"
    )
)]
pub struct Cl3ProductRequest {
    /// Left coefficients in binary basis-blade order.
    pub left: [f64; 8],
    /// Right coefficients in binary basis-blade order.
    pub right: [f64; 8],
}

/// Typed output from a Euclidean Cl(3,0,0) geometric product.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[wire_contract(
    id = "amari.discovery/probe/core-geometric-product/output/v1",
    role = "output",
    compatibility = "additive_patch",
    constraints(
        finite_numbers = "all Cl(3,0,0) output coefficients are finite",
        fixed_coefficient_length = "the result contains exactly eight coefficients in binary basis-blade order"
    ),
    example(
        label = "basis_product",
        value = "{\"coefficients\":[0.0,0.0,0.0,1.0,0.0,0.0,0.0,0.0]}"
    )
)]
pub struct Cl3ProductOutput {
    /// Product coefficients in binary basis-blade order.
    pub coefficients: [f64; 8],
}

#[cfg(feature = "standard-probes")]
pub(super) fn registration() -> DiscoveryResult<AdapterRegistration> {
    Ok(AdapterRegistration {
        id: "amari-probe:core:geometric-product:v1".parse()?,
        capability_id: "amari:amari-core:product:geometric-product".parse()?,
        input_schema: "amari.discovery/probe/core-geometric-product/input/v1".to_owned(),
        output_schema: "amari.discovery/probe/core-geometric-product/output/v1".to_owned(),
        required_features: vec!["standard-probes".to_owned()],
        limits: ProbeLimits {
            max_input_bytes: 4_096,
            max_output_bytes: 4_096,
            max_operations: 1_024,
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
    let request: Cl3ProductRequest = serde_json::from_value(input.clone()).map_err(|error| {
        DiscoveryError::InvalidInput(format!(
            "Cl(3,0,0) product request requires exactly eight finite coefficients per operand: {error}"
        ))
    })?;
    if request
        .left
        .iter()
        .chain(&request.right)
        .any(|coefficient| !coefficient.is_finite())
    {
        return Err(DiscoveryError::InvalidInput(
            "Cl(3,0,0) product coefficients must be finite".to_owned(),
        ));
    }
    enforce("operations", PRODUCT_OPERATIONS, limits.max_operations)?;
    enforce("nodes", PRODUCT_NODES, limits.max_nodes)?;
    enforce("iterations", PRODUCT_ITERATIONS, limits.max_iterations)?;

    let left = Multivector::<3, 0, 0>::from_slice(&request.left);
    let right = Multivector::<3, 0, 0>::from_slice(&request.right);
    let product = left.geometric_product(&right);
    if product
        .as_slice()
        .iter()
        .any(|coefficient| !coefficient.is_finite())
    {
        return Err(DiscoveryError::ProbeFailed(
            "Cl(3,0,0) geometric product produced a non-finite coefficient".to_owned(),
        ));
    }
    let coefficients = product.as_slice().try_into().map_err(|_| {
        DiscoveryError::Internal(
            "Cl(3,0,0) product returned an unexpected coefficient count".to_owned(),
        )
    })?;
    Ok(AdapterOutput {
        resources: ResourceObservations {
            operations: PRODUCT_OPERATIONS,
            nodes: PRODUCT_NODES,
            iterations: PRODUCT_ITERATIONS,
            bytes: 0,
        },
        output: serde_json::to_value(Cl3ProductOutput { coefficients })?,
    })
}

#[cfg(feature = "standard-probes")]
fn enforce(kind: &str, observed: u64, maximum: u64) -> DiscoveryResult<()> {
    if observed <= maximum {
        Ok(())
    } else {
        Err(DiscoveryError::LimitExceeded(format!(
            "Cl(3,0,0) product {kind} {observed} exceeds limit {maximum}"
        )))
    }
}
