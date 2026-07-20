// SPDX-License-Identifier: MIT OR Apache-2.0

//! Holographic MAP256 probe parity and limits.

#![cfg(feature = "standard-probes")]

use amari_discovery::{
    DiscoveryError, HolographicSuperpositionOutput, HolographicSuperpositionRequest, ProbeEngine,
    ProbeEngineLimits, ProbeExecution,
};
use amari_holographic::{algebra::map::MAP256, BindingAlgebra};
use serde_json::json;

const SUPERPOSITION: &str = "amari-probe:holographic:superposition:v1";

fn execute_superposition(
    engine: &ProbeEngine,
    request: HolographicSuperpositionRequest,
) -> ProbeExecution {
    engine
        .execute(
            &SUPERPOSITION.parse().unwrap(),
            &serde_json::to_value(request).unwrap(),
        )
        .unwrap()
}

fn direct_superposition(seeds: &[u64]) -> MAP256 {
    seeds.iter().fold(MAP256::zero(), |trace, seed| {
        trace.superpose(&MAP256::from_seed(*seed)).unwrap()
    })
}

#[test]
fn superposition_matches_repeated_binding_algebra_addition() {
    let request = HolographicSuperpositionRequest {
        seeds: vec![7, 11, 29],
    };
    let expected = direct_superposition(&request.seeds);
    let engine = ProbeEngine::new().unwrap();
    let first = execute_superposition(&engine, request.clone());
    let second = execute_superposition(&engine, request);
    let output: HolographicSuperpositionOutput =
        serde_json::from_value(first.output.clone()).unwrap();

    assert_eq!(first, second);
    assert_eq!(output.coefficients, expected.components().to_vec());
    assert_eq!(output.coefficients.len(), 256);
    assert_eq!(first.resources.operations, 768);
    assert_eq!(first.resources.nodes, 1_024);
    assert_eq!(first.resources.iterations, 3);
}

#[test]
fn additive_superposition_is_not_attention_style_bundle_cleanup() {
    let seeds = vec![3, 5, 8];
    let elements = seeds
        .iter()
        .map(|seed| MAP256::from_seed(*seed))
        .collect::<Vec<_>>();
    let bundled = <MAP256 as BindingAlgebra>::bundle_all(&elements, 1.0).unwrap();
    let output: HolographicSuperpositionOutput = serde_json::from_value(
        execute_superposition(
            &ProbeEngine::new().unwrap(),
            HolographicSuperpositionRequest { seeds },
        )
        .output,
    )
    .unwrap();

    assert_ne!(output.coefficients, bundled.components().to_vec());
    assert!(output
        .coefficients
        .iter()
        .any(|coefficient| coefficient.abs() > 1.0));
}

#[test]
fn empty_and_oversized_seed_sets_are_rejected() {
    for (input, expected) in [
        (json!({ "seeds": [] }), "at least one seed"),
        (
            json!({ "seeds": vec![0; 257] }),
            "seed count 257 exceeds limit 256",
        ),
    ] {
        let error = ProbeEngine::new()
            .unwrap()
            .execute(&SUPERPOSITION.parse().unwrap(), &input)
            .unwrap_err();
        assert!(matches!(
            error,
            DiscoveryError::InvalidInput(ref message)
                | DiscoveryError::LimitExceeded(ref message)
                if message.contains(expected)
        ));
    }
}

#[test]
fn superposition_cooperative_limits_are_enforced() {
    let input = serde_json::to_value(HolographicSuperpositionRequest {
        seeds: vec![7, 11, 29],
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
                max_operations: 767,
                ..ProbeEngineLimits::default()
            },
            "operations",
        ),
        (
            ProbeEngineLimits {
                max_nodes: 1_023,
                ..ProbeEngineLimits::default()
            },
            "nodes",
        ),
        (
            ProbeEngineLimits {
                max_iterations: 2,
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
            .execute(&SUPERPOSITION.parse().unwrap(), &input)
            .unwrap_err();
        assert!(
            matches!(error, DiscoveryError::LimitExceeded(ref message) if message.contains(expected)),
            "unexpected error for {expected}: {error}"
        );
    }
}
