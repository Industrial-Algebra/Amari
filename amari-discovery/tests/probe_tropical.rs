// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tropical Viterbi proof-slice parity and cooperative limits.

#![cfg(feature = "standard-probes")]

use amari_discovery::{
    DiscoveryError, ProbeEngine, ProbeEngineLimits, TropicalViterbiOutput, TropicalViterbiRequest,
};
use amari_tropical::viterbi::TropicalViterbi;
use serde_json::{json, Value};

const VITERBI: &str = "amari-probe:tropical:viterbi:v1";

fn request() -> TropicalViterbiRequest {
    TropicalViterbiRequest {
        transitions: vec![vec![-1.0, -2.0], vec![-2.0, -1.0]],
        emissions: vec![vec![-1.0, -3.0], vec![-3.0, -1.0]],
        observations: vec![0, 1, 0],
    }
}

fn execute(engine: &ProbeEngine, request: &TropicalViterbiRequest) -> TropicalViterbiOutput {
    let execution = engine
        .execute(
            &VITERBI.parse().unwrap(),
            &serde_json::to_value(request).unwrap(),
        )
        .unwrap();
    serde_json::from_value(execution.output).unwrap()
}

fn invalid(engine: &ProbeEngine, input: Value, expected: &str) {
    let error = engine
        .execute(&VITERBI.parse().unwrap(), &input)
        .unwrap_err();
    assert!(
        matches!(error, DiscoveryError::InvalidInput(ref message) if message.contains(expected)),
        "unexpected error: {error}"
    );
}

#[test]
fn viterbi_output_matches_direct_amari_tropical_decode() {
    let request = request();
    let direct = TropicalViterbi::new(request.transitions.clone(), request.emissions.clone())
        .decode(&request.observations);
    let output = execute(&ProbeEngine::new().unwrap(), &request);

    assert_eq!(output.path, direct.0);
    assert_eq!(output.score, direct.1.value());
}

#[test]
fn matrix_shape_state_and_observation_validation_precedes_execution() {
    let engine = ProbeEngine::new().unwrap();
    let mut empty_states = request();
    empty_states.transitions.clear();
    invalid(
        &engine,
        serde_json::to_value(empty_states).unwrap(),
        "at least one state",
    );

    let mut non_square = request();
    non_square.transitions[0].pop();
    invalid(
        &engine,
        serde_json::to_value(non_square).unwrap(),
        "square transition",
    );

    let mut emission_rows = request();
    emission_rows.emissions.pop();
    invalid(
        &engine,
        serde_json::to_value(emission_rows).unwrap(),
        "emission row per state",
    );

    let mut ragged_emissions = request();
    ragged_emissions.emissions[0].pop();
    invalid(
        &engine,
        serde_json::to_value(ragged_emissions).unwrap(),
        "equal nonzero width",
    );

    let mut no_observations = request();
    no_observations.observations.clear();
    invalid(
        &engine,
        serde_json::to_value(no_observations).unwrap(),
        "at least one observation",
    );

    let mut bad_observation = request();
    bad_observation.observations[1] = 2;
    invalid(
        &engine,
        serde_json::to_value(bad_observation).unwrap(),
        "observation index",
    );

    let mut non_finite = serde_json::to_value(request()).unwrap();
    non_finite["transitions"][0][0] = Value::Null;
    invalid(&engine, non_finite, "request schema");
}

#[test]
fn state_and_observation_count_ceilings_are_enforced() {
    let engine = ProbeEngine::new().unwrap();
    let states = 65;
    let too_many_states = TropicalViterbiRequest {
        transitions: vec![vec![0.0; states]; states],
        emissions: vec![vec![0.0]; states],
        observations: vec![0],
    };
    invalid(
        &engine,
        serde_json::to_value(too_many_states).unwrap(),
        "state count",
    );

    let mut too_many_observations = request();
    too_many_observations.observations = vec![0; 4097];
    invalid(
        &engine,
        serde_json::to_value(too_many_observations).unwrap(),
        "observation count",
    );
}

#[test]
fn request_operation_node_iteration_and_output_byte_limits_are_enforced() {
    let input = serde_json::to_value(request()).unwrap();
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
                max_operations: 1,
                ..ProbeEngineLimits::default()
            },
            "operations",
        ),
        (
            ProbeEngineLimits {
                max_nodes: 1,
                ..ProbeEngineLimits::default()
            },
            "nodes",
        ),
        (
            ProbeEngineLimits {
                max_iterations: 1,
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
        let engine = ProbeEngine::with_limits(limits).unwrap();
        let error = engine
            .execute(&VITERBI.parse().unwrap(), &input)
            .unwrap_err();
        assert!(
            matches!(error, DiscoveryError::LimitExceeded(ref message) if message.contains(expected)),
            "unexpected error for {expected}: {error}"
        );
    }
}

#[test]
fn caller_limits_cannot_loosen_descriptor_work_ceiling() {
    let states = 64;
    let request = TropicalViterbiRequest {
        transitions: vec![vec![0.0; states]; states],
        emissions: vec![vec![0.0]; states],
        observations: vec![0; 30],
    };
    let engine = ProbeEngine::with_limits(ProbeEngineLimits {
        max_input_bytes: u64::MAX,
        max_output_bytes: u64::MAX,
        max_operations: u64::MAX,
        max_nodes: u64::MAX,
        max_iterations: u64::MAX,
    })
    .unwrap();

    assert!(matches!(
        engine.execute(
            &VITERBI.parse().unwrap(),
            &serde_json::to_value(request).unwrap()
        ),
        Err(DiscoveryError::LimitExceeded(message)) if message.contains("operations") && message.contains("100000")
    ));
}

#[test]
fn non_finite_algorithm_result_is_a_typed_probe_failure() {
    let request = TropicalViterbiRequest {
        transitions: vec![vec![1.7e308]],
        emissions: vec![vec![1.7e308]],
        observations: vec![0, 0],
    };

    assert!(matches!(
        ProbeEngine::new().unwrap().execute(
            &VITERBI.parse().unwrap(),
            &serde_json::to_value(request).unwrap()
        ),
        Err(DiscoveryError::ProbeFailed(message)) if message.contains("non-finite score")
    ));
}

#[test]
fn zero_cooperative_limits_are_rejected() {
    assert!(matches!(
        ProbeEngine::with_limits(ProbeEngineLimits {
            max_iterations: 0,
            ..ProbeEngineLimits::default()
        }),
        Err(DiscoveryError::InvalidInput(message)) if message.contains("greater than zero")
    ));
}

#[test]
fn malformed_json_shape_is_a_typed_input_error() {
    invalid(
        &ProbeEngine::new().unwrap(),
        json!({"transitions": [], "unexpected": true}),
        "request schema",
    );
}
