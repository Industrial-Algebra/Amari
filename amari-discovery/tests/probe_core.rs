// SPDX-License-Identifier: MIT OR Apache-2.0

//! Core Cl(3,0,0) geometric-product probe parity and limits.

#![cfg(feature = "standard-probes")]

use amari_core::Multivector;
use amari_discovery::{
    Cl3ProductOutput, Cl3ProductRequest, DiscoveryError, ProbeEngine, ProbeEngineLimits,
};
use serde_json::Value;

const CORE_PRODUCT: &str = "amari-probe:core:geometric-product:v1";

fn execute(engine: &ProbeEngine, request: Cl3ProductRequest) -> Cl3ProductOutput {
    let execution = engine
        .execute(
            &CORE_PRODUCT.parse().unwrap(),
            &serde_json::to_value(request).unwrap(),
        )
        .unwrap();
    serde_json::from_value(execution.output).unwrap()
}

#[test]
fn output_matches_direct_cl3_geometric_product() {
    let request = Cl3ProductRequest {
        left: [1.0, 2.0, -3.0, 4.0, 0.5, -0.25, 2.5, -1.5],
        right: [0.5, -1.0, 2.0, 3.0, -4.0, 1.25, 0.75, 2.0],
    };
    let expected = Multivector::<3, 0, 0>::from_slice(&request.left)
        .geometric_product(&Multivector::from_slice(&request.right));
    let output = execute(&ProbeEngine::new().unwrap(), request);

    assert_eq!(output.coefficients.as_slice(), expected.as_slice());
}

#[test]
fn scalar_identity_is_preserved_on_both_sides() {
    let coefficients = [1.0, -2.0, 3.0, -4.0, 5.0, -6.0, 7.0, -8.0];
    let identity = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    let engine = ProbeEngine::new().unwrap();

    assert_eq!(
        execute(
            &engine,
            Cl3ProductRequest {
                left: identity,
                right: coefficients,
            },
        )
        .coefficients,
        coefficients
    );
    assert_eq!(
        execute(
            &engine,
            Cl3ProductRequest {
                left: coefficients,
                right: identity,
            },
        )
        .coefficients,
        coefficients
    );
}

#[test]
fn non_finite_or_malformed_coefficients_are_rejected_before_product() {
    let mut input = serde_json::to_value(Cl3ProductRequest {
        left: [0.0; 8],
        right: [0.0; 8],
    })
    .unwrap();
    input["left"][3] = Value::Null;

    assert!(matches!(
        ProbeEngine::new()
            .unwrap()
            .execute(&CORE_PRODUCT.parse().unwrap(), &input),
        Err(DiscoveryError::InvalidInput(message)) if message.contains("finite")
    ));
}

#[test]
fn cooperative_input_work_node_iteration_and_output_limits_are_enforced() {
    let input = serde_json::to_value(Cl3ProductRequest {
        left: [1.0; 8],
        right: [2.0; 8],
    })
    .unwrap();
    for (limits, expected) in [
        (
            ProbeEngineLimits {
                max_input_bytes: 8,
                ..ProbeEngineLimits::default()
            },
            "input bytes",
        ),
        (
            ProbeEngineLimits {
                max_operations: 63,
                ..ProbeEngineLimits::default()
            },
            "operations",
        ),
        (
            ProbeEngineLimits {
                max_nodes: 23,
                ..ProbeEngineLimits::default()
            },
            "nodes",
        ),
        (
            ProbeEngineLimits {
                max_iterations: 63,
                ..ProbeEngineLimits::default()
            },
            "iterations",
        ),
        (
            ProbeEngineLimits {
                max_output_bytes: 1,
                ..ProbeEngineLimits::default()
            },
            "output bytes",
        ),
    ] {
        let error = ProbeEngine::with_limits(limits)
            .unwrap()
            .execute(&CORE_PRODUCT.parse().unwrap(), &input)
            .unwrap_err();
        assert!(
            matches!(error, DiscoveryError::LimitExceeded(ref message) if message.contains(expected)),
            "unexpected error for {expected}: {error}"
        );
    }
}

#[test]
fn non_finite_product_is_a_typed_probe_failure() {
    let request = Cl3ProductRequest {
        left: [1.7e308, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        right: [2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    };

    assert!(matches!(
        ProbeEngine::new().unwrap().execute(
            &CORE_PRODUCT.parse().unwrap(),
            &serde_json::to_value(request).unwrap()
        ),
        Err(DiscoveryError::ProbeFailed(message)) if message.contains("non-finite")
    ));
}
