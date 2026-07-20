// SPDX-License-Identifier: MIT OR Apache-2.0

//! Multi-objective Pareto-front probe parity and limits.

#![cfg(feature = "standard-probes")]

use amari_discovery::{
    DiscoveryError, ObjectiveDirection, ParetoFrontOutput, ParetoFrontRequest, ParetoPoint,
    ProbeEngine, ProbeEngineLimits, ProbeExecution,
};
use amari_optimization::multiobjective::{Individual, ParetoFront};
use serde_json::json;

const PARETO_FRONT: &str = "amari-probe:optimization:pareto-front:v1";

fn request() -> ParetoFrontRequest {
    ParetoFrontRequest {
        objectives: vec![
            vec![1.0, 1.0],
            vec![2.0, 3.0],
            vec![0.5, 0.5],
            vec![3.0, 0.0],
        ],
        directions: vec![ObjectiveDirection::Minimize, ObjectiveDirection::Maximize],
    }
}

fn execute(engine: &ProbeEngine, request: ParetoFrontRequest) -> ProbeExecution {
    engine
        .execute(
            &PARETO_FRONT.parse().unwrap(),
            &serde_json::to_value(request).unwrap(),
        )
        .unwrap()
}

fn direct_front(request: &ParetoFrontRequest) -> Vec<usize> {
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
        .map(|individual| individual.variables[0] as usize)
        .collect::<Vec<_>>();
    indices.sort_unstable();
    indices
}

#[test]
fn output_matches_direct_individual_and_pareto_front_parity() {
    let request = request();
    let expected = direct_front(&request);
    let engine = ProbeEngine::new().unwrap();
    let first = execute(&engine, request.clone());
    let second = execute(&engine, request.clone());
    let output: ParetoFrontOutput = serde_json::from_value(first.output.clone()).unwrap();

    assert_eq!(first, second);
    assert_eq!(
        output
            .solutions
            .iter()
            .map(|point| point.index)
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(
        output.solutions,
        vec![
            ParetoPoint {
                index: 0,
                objectives: vec![1.0, 1.0],
            },
            ParetoPoint {
                index: 1,
                objectives: vec![2.0, 3.0],
            },
            ParetoPoint {
                index: 2,
                objectives: vec![0.5, 0.5],
            },
        ]
    );
    assert_eq!(first.resources.operations, 32);
    assert_eq!(first.resources.nodes, 8);
    assert_eq!(first.resources.iterations, 4);
}

#[test]
fn maximize_direction_is_transformed_to_minimization() {
    let output: ParetoFrontOutput = serde_json::from_value(
        execute(
            &ProbeEngine::new().unwrap(),
            ParetoFrontRequest {
                objectives: vec![vec![1.0], vec![3.0], vec![2.0]],
                directions: vec![ObjectiveDirection::Maximize],
            },
        )
        .output,
    )
    .unwrap();

    assert_eq!(
        output.solutions,
        vec![ParetoPoint {
            index: 1,
            objectives: vec![3.0],
        }]
    );
}

#[test]
fn empty_ragged_and_non_finite_objectives_are_rejected() {
    for (input, expected) in [
        (
            json!({ "objectives": [], "directions": ["minimize"] }),
            "population",
        ),
        (
            json!({ "objectives": [[1.0]], "directions": [] }),
            "dimension",
        ),
        (
            json!({ "objectives": [[1.0], [2.0, 3.0]], "directions": ["minimize"] }),
            "dimension",
        ),
        (
            json!({ "objectives": [[null]], "directions": ["minimize"] }),
            "finite",
        ),
    ] {
        assert!(matches!(
            ProbeEngine::new()
                .unwrap()
                .execute(&PARETO_FRONT.parse().unwrap(), &input),
            Err(DiscoveryError::InvalidInput(message)) if message.contains(expected)
        ));
    }
}

#[test]
fn population_dimension_and_combined_work_are_bounded() {
    let too_many_candidates = json!({
        "objectives": vec![vec![0]; 257],
        "directions": ["minimize"],
    });
    assert!(matches!(
        ProbeEngine::new()
            .unwrap()
            .execute(&PARETO_FRONT.parse().unwrap(), &too_many_candidates),
        Err(DiscoveryError::LimitExceeded(message))
            if message.contains("population 257 exceeds limit 256")
    ));

    let too_many_dimensions = json!({
        "objectives": [vec![0; 33]],
        "directions": vec!["minimize"; 33],
    });
    assert!(matches!(
        ProbeEngine::new()
            .unwrap()
            .execute(&PARETO_FRONT.parse().unwrap(), &too_many_dimensions),
        Err(DiscoveryError::LimitExceeded(message))
            if message.contains("dimensions 33 exceeds limit 32")
    ));

    let too_much_work = json!({
        "objectives": vec![vec![0; 32]; 64],
        "directions": vec!["minimize"; 32],
    });
    assert!(matches!(
        ProbeEngine::new()
            .unwrap()
            .execute(&PARETO_FRONT.parse().unwrap(), &too_much_work),
        Err(DiscoveryError::LimitExceeded(message)) if message.contains("operations")
    ));
}

#[test]
fn cooperative_input_work_node_iteration_and_output_limits_are_enforced() {
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
                max_operations: 31,
                ..ProbeEngineLimits::default()
            },
            "operations",
        ),
        (
            ProbeEngineLimits {
                max_nodes: 7,
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
            .execute(&PARETO_FRONT.parse().unwrap(), &input)
            .unwrap_err();
        assert!(
            matches!(error, DiscoveryError::LimitExceeded(ref message) if message.contains(expected)),
            "unexpected error for {expected}: {error}"
        );
    }
}
