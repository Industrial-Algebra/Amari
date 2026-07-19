// SPDX-License-Identifier: MIT OR Apache-2.0

//! Dual-number polynomial derivative probe parity and limits.

#![cfg(feature = "standard-probes")]

use amari_discovery::{
    DiscoveryError, PolynomialDerivativeOutput, PolynomialDerivativeRequest, ProbeEngine,
    ProbeEngineLimits, ProbeExecution,
};
use amari_dual::DualNumber;
use serde_json::{json, Value};

const POLYNOMIAL_DERIVATIVE: &str = "amari-probe:dual:polynomial-derivative:v1";

fn execute(engine: &ProbeEngine, request: PolynomialDerivativeRequest) -> ProbeExecution {
    engine
        .execute(
            &POLYNOMIAL_DERIVATIVE.parse().unwrap(),
            &serde_json::to_value(request).unwrap(),
        )
        .unwrap()
}

fn direct_horner(request: &PolynomialDerivativeRequest) -> DualNumber<f64> {
    let point = DualNumber::variable(request.at);
    request
        .coefficients
        .iter()
        .copied()
        .fold(DualNumber::constant(0.0), |accumulator, coefficient| {
            accumulator * point + DualNumber::constant(coefficient)
        })
}

#[test]
fn output_matches_direct_dual_horner_value_and_derivative() {
    let request = PolynomialDerivativeRequest {
        // 2x^3 - 3x^2 + 5x - 7, in descending-power order.
        coefficients: vec![2.0, -3.0, 5.0, -7.0],
        at: 2.0,
    };
    let expected = direct_horner(&request);
    let execution = execute(&ProbeEngine::new().unwrap(), request);
    let output: PolynomialDerivativeOutput = serde_json::from_value(execution.output).unwrap();

    assert_eq!(output.value, expected.value());
    assert_eq!(output.derivative, expected.derivative());
    assert_eq!(output.value, 7.0);
    assert_eq!(output.derivative, 17.0);
    assert_eq!(execution.resources.operations, 8);
    assert_eq!(execution.resources.nodes, 7);
    assert_eq!(execution.resources.iterations, 4);
}

#[test]
fn constant_polynomial_has_zero_derivative() {
    let execution = execute(
        &ProbeEngine::new().unwrap(),
        PolynomialDerivativeRequest {
            coefficients: vec![7.5],
            at: -123.0,
        },
    );
    let output: PolynomialDerivativeOutput = serde_json::from_value(execution.output).unwrap();

    assert_eq!(output.value, 7.5);
    assert_eq!(output.derivative, 0.0);
}

#[test]
fn empty_polynomial_is_rejected_before_evaluation() {
    let request = PolynomialDerivativeRequest {
        coefficients: Vec::new(),
        at: 1.0,
    };

    assert!(matches!(
        ProbeEngine::new().unwrap().execute(
            &POLYNOMIAL_DERIVATIVE.parse().unwrap(),
            &serde_json::to_value(request).unwrap()
        ),
        Err(DiscoveryError::InvalidInput(message)) if message.contains("at least one coefficient")
    ));
}

#[test]
fn malformed_non_finite_values_are_rejected_before_evaluation() {
    for input in [
        json!({ "coefficients": [1.0, Value::Null], "at": 2.0 }),
        json!({ "coefficients": [1.0, 2.0], "at": Value::Null }),
    ] {
        assert!(matches!(
            ProbeEngine::new()
                .unwrap()
                .execute(&POLYNOMIAL_DERIVATIVE.parse().unwrap(), &input),
            Err(DiscoveryError::InvalidInput(message)) if message.contains("finite")
        ));
    }
}

#[test]
fn coefficient_count_is_bounded_by_descriptor_work_limit() {
    let input = json!({
        "coefficients": vec![0; 5_001],
        "at": 0,
    });

    assert!(matches!(
        ProbeEngine::new()
            .unwrap()
            .execute(&POLYNOMIAL_DERIVATIVE.parse().unwrap(), &input),
        Err(DiscoveryError::LimitExceeded(message))
            if message.contains("coefficient count 5001 exceeds limit 5000")
    ));
}

#[test]
fn cooperative_input_work_node_iteration_and_output_limits_are_enforced() {
    let input = serde_json::to_value(PolynomialDerivativeRequest {
        coefficients: vec![2.0, -3.0, 5.0, -7.0],
        at: 2.0,
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
                max_operations: 7,
                ..ProbeEngineLimits::default()
            },
            "operations",
        ),
        (
            ProbeEngineLimits {
                max_nodes: 6,
                ..ProbeEngineLimits::default()
            },
            "nodes",
        ),
        (
            ProbeEngineLimits {
                max_iterations: 3,
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
            .execute(&POLYNOMIAL_DERIVATIVE.parse().unwrap(), &input)
            .unwrap_err();
        assert!(
            matches!(error, DiscoveryError::LimitExceeded(ref message) if message.contains(expected)),
            "unexpected error for {expected}: {error}"
        );
    }
}

#[test]
fn non_finite_horner_result_is_a_typed_probe_failure() {
    let request = PolynomialDerivativeRequest {
        coefficients: vec![1.0, 1.0, 1.0],
        at: 1.7e308,
    };

    assert!(matches!(
        ProbeEngine::new().unwrap().execute(
            &POLYNOMIAL_DERIVATIVE.parse().unwrap(),
            &serde_json::to_value(request).unwrap()
        ),
        Err(DiscoveryError::ProbeFailed(message)) if message.contains("non-finite")
    ));
}
