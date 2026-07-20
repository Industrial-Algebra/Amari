// SPDX-License-Identifier: MIT OR Apache-2.0

//! Geometric-network shortest-path probe parity and limits.

#![cfg(feature = "standard-probes")]

use amari_core::Vector;
use amari_discovery::{
    DiscoveryError, NetworkPath, NetworkShortestPathOutput, NetworkShortestPathRequest,
    ProbeEngine, ProbeEngineLimits, ProbeExecution,
};
use amari_network::GeometricNetwork;
use serde_json::json;

const SHORTEST_PATH: &str = "amari-probe:network:shortest-path:v1";

fn request() -> NetworkShortestPathRequest {
    NetworkShortestPathRequest {
        adjacency: vec![
            vec![None, Some(1.0), Some(5.0), None],
            vec![None, None, Some(1.0), Some(10.0)],
            vec![None, None, None, Some(2.0)],
            vec![None, None, None, None],
        ],
        source: 0,
        target: 3,
    }
}

fn execute(engine: &ProbeEngine, request: NetworkShortestPathRequest) -> ProbeExecution {
    engine
        .execute(
            &SHORTEST_PATH.parse().unwrap(),
            &serde_json::to_value(request).unwrap(),
        )
        .unwrap()
}

fn direct_shortest_path(request: &NetworkShortestPathRequest) -> Option<(Vec<usize>, f64)> {
    let mut network = GeometricNetwork::<3, 0, 0>::with_capacity(
        request.adjacency.len(),
        request
            .adjacency
            .iter()
            .flatten()
            .filter(|weight| weight.is_some())
            .count(),
    );
    for index in 0..request.adjacency.len() {
        network.add_node(Vector::from_components(index as f64, 0.0, 0.0).mv);
    }
    for (source, row) in request.adjacency.iter().enumerate() {
        for (target, weight) in row.iter().enumerate() {
            if let Some(weight) = weight {
                network.add_edge(source, target, *weight).unwrap();
            }
        }
    }
    network
        .shortest_path(request.source, request.target)
        .unwrap()
}

#[test]
fn output_matches_direct_geometric_network_shortest_path() {
    let request = request();
    let expected = direct_shortest_path(&request).map(|(nodes, total_weight)| NetworkPath {
        nodes,
        total_weight,
    });
    let engine = ProbeEngine::new().unwrap();
    let first = execute(&engine, request.clone());
    let second = execute(&engine, request);
    let output: NetworkShortestPathOutput = serde_json::from_value(first.output.clone()).unwrap();

    assert_eq!(first, second);
    assert_eq!(output.path, expected);
    assert_eq!(
        output.path,
        Some(NetworkPath {
            nodes: vec![0, 1, 2, 3],
            total_weight: 4.0,
        })
    );
    assert_eq!(first.resources.operations, 21);
    assert_eq!(first.resources.nodes, 4);
    assert_eq!(first.resources.iterations, 4);
}

#[test]
fn empty_and_non_square_adjacency_are_rejected() {
    for (input, expected) in [
        (
            json!({ "adjacency": [], "source": 0, "target": 0 }),
            "non-empty",
        ),
        (
            json!({ "adjacency": [[null, 1.0], [null]], "source": 0, "target": 1 }),
            "square",
        ),
    ] {
        assert!(matches!(
            ProbeEngine::new()
                .unwrap()
                .execute(&SHORTEST_PATH.parse().unwrap(), &input),
            Err(DiscoveryError::InvalidInput(message)) if message.contains(expected)
        ));
    }
}

#[test]
fn malformed_negative_and_non_finite_weights_are_rejected() {
    for input in [
        json!({ "adjacency": [[null, -1.0], [null, null]], "source": 0, "target": 1 }),
        json!({ "adjacency": [[null, "NaN"], [null, null]], "source": 0, "target": 1 }),
    ] {
        assert!(matches!(
            ProbeEngine::new()
                .unwrap()
                .execute(&SHORTEST_PATH.parse().unwrap(), &input),
            Err(DiscoveryError::InvalidInput(message))
                if message.contains("finite nonnegative")
        ));
    }
}

#[test]
fn node_count_and_endpoint_indices_are_bounded() {
    let too_many_nodes = json!({
        "adjacency": vec![Vec::<Option<f64>>::new(); 129],
        "source": 0,
        "target": 0,
    });
    assert!(matches!(
        ProbeEngine::new()
            .unwrap()
            .execute(&SHORTEST_PATH.parse().unwrap(), &too_many_nodes),
        Err(DiscoveryError::LimitExceeded(message))
            if message.contains("node count 129 exceeds limit 128")
    ));

    for (source, target, expected) in [(2, 0, "source index 2"), (0, 2, "target index 2")] {
        let input = json!({
            "adjacency": [[null, 1.0], [null, null]],
            "source": source,
            "target": target,
        });
        assert!(matches!(
            ProbeEngine::new()
                .unwrap()
                .execute(&SHORTEST_PATH.parse().unwrap(), &input),
            Err(DiscoveryError::InvalidInput(message)) if message.contains(expected)
        ));
    }
}

#[test]
fn unreachable_target_is_a_successful_typed_outcome() {
    let execution = execute(
        &ProbeEngine::new().unwrap(),
        NetworkShortestPathRequest {
            adjacency: vec![vec![None, None], vec![None, None]],
            source: 0,
            target: 1,
        },
    );
    let output: NetworkShortestPathOutput = serde_json::from_value(execution.output).unwrap();

    assert_eq!(output.path, None);
}

#[test]
fn reachable_path_whose_weight_overflows_is_a_typed_probe_failure() {
    let request = NetworkShortestPathRequest {
        adjacency: vec![
            vec![None, Some(f64::MAX), None],
            vec![None, None, Some(f64::MAX)],
            vec![None, None, None],
        ],
        source: 0,
        target: 2,
    };

    assert!(matches!(
        ProbeEngine::new().unwrap().execute(
            &SHORTEST_PATH.parse().unwrap(),
            &serde_json::to_value(request).unwrap()
        ),
        Err(DiscoveryError::ProbeFailed(message)) if message.contains("non-finite")
    ));
}

#[test]
fn source_equal_to_target_returns_the_zero_length_path() {
    let execution = execute(
        &ProbeEngine::new().unwrap(),
        NetworkShortestPathRequest {
            adjacency: vec![vec![None, Some(1.0)], vec![None, None]],
            source: 1,
            target: 1,
        },
    );
    let output: NetworkShortestPathOutput = serde_json::from_value(execution.output).unwrap();

    assert_eq!(
        output.path,
        Some(NetworkPath {
            nodes: vec![1],
            total_weight: 0.0,
        })
    );
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
                max_operations: 20,
                ..ProbeEngineLimits::default()
            },
            "operations",
        ),
        (
            ProbeEngineLimits {
                max_nodes: 3,
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
            .execute(&SHORTEST_PATH.parse().unwrap(), &input)
            .unwrap_err();
        assert!(
            matches!(error, DiscoveryError::LimitExceeded(ref message) if message.contains(expected)),
            "unexpected error for {expected}: {error}"
        );
    }
}
