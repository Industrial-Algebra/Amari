// SPDX-License-Identifier: MIT OR Apache-2.0

//! Holographic MAP256 probe parity and limits.

#![cfg(feature = "standard-probes")]

use amari_discovery::{
    DiscoveryError, HolographicAttribution, HolographicCapacity, HolographicEntry,
    HolographicRecallOutput, HolographicRecallRequest, HolographicSuperpositionOutput,
    HolographicSuperpositionRequest, ProbeEngine, ProbeEngineLimits, ProbeExecution,
};
use amari_holographic::{algebra::map::MAP256, AlgebraConfig, BindingAlgebra, HolographicMemory};
use serde_json::json;

const RECALL: &str = "amari-probe:holographic:recall:v1";
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

fn execute_recall(engine: &ProbeEngine, request: HolographicRecallRequest) -> ProbeExecution {
    engine
        .execute(
            &RECALL.parse().unwrap(),
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
fn recall_matches_direct_holographic_memory_retrieval() {
    let request = HolographicRecallRequest {
        entries: vec![
            HolographicEntry {
                key_seed: 3,
                value_seed: 103,
            },
            HolographicEntry {
                key_seed: 5,
                value_seed: 105,
            },
            HolographicEntry {
                key_seed: 8,
                value_seed: 108,
            },
        ],
        query_seed: 5,
    };
    let mut memory = HolographicMemory::<MAP256>::with_key_tracking(AlgebraConfig::default());
    for entry in &request.entries {
        memory.store(
            &MAP256::from_seed(entry.key_seed),
            &MAP256::from_seed(entry.value_seed),
        );
    }
    let expected = memory.retrieve(&MAP256::from_seed(request.query_seed));
    let capacity = memory.capacity_info();
    let engine = ProbeEngine::new().unwrap();
    let first = execute_recall(&engine, request.clone());
    let second = execute_recall(&engine, request);
    let output: HolographicRecallOutput = serde_json::from_value(first.output.clone()).unwrap();

    assert_eq!(first, second);
    assert_eq!(output.value_coefficients, expected.value.components());
    assert_eq!(output.raw_coefficients, expected.raw_value.components());
    assert_eq!(output.confidence, expected.confidence);
    assert_eq!(output.query_similarity, expected.query_similarity);
    assert_eq!(
        output.attribution,
        expected
            .attribution
            .into_iter()
            .map(|(index, weight)| HolographicAttribution { index, weight })
            .collect::<Vec<_>>()
    );
    assert_eq!(
        output.capacity,
        HolographicCapacity {
            item_count: capacity.item_count,
            theoretical_capacity: capacity.theoretical_capacity,
            estimated_snr: capacity.estimated_snr,
            snr_threshold: capacity.snr_threshold,
            near_capacity: capacity.near_capacity,
        }
    );
    assert!(output.warnings.is_empty());
    assert_eq!(first.resources.operations, 9_984);
    assert_eq!(first.resources.nodes, 2_307);
    assert_eq!(first.resources.iterations, 7);
}

#[test]
fn recall_capacity_warning_is_deterministic() {
    let entries = (0..24)
        .map(|seed| HolographicEntry {
            key_seed: seed,
            value_seed: seed + 1_000,
        })
        .collect();
    let request = HolographicRecallRequest {
        entries,
        query_seed: 0,
    };
    let engine = ProbeEngine::new().unwrap();
    let first = execute_recall(&engine, request.clone());
    let second = execute_recall(&engine, request);
    let output: HolographicRecallOutput = serde_json::from_value(first.output.clone()).unwrap();

    assert_eq!(first, second);
    assert!(output.capacity.near_capacity);
    assert_eq!(output.capacity.item_count, 24);
    assert_eq!(output.capacity.theoretical_capacity, 46);
    assert_eq!(output.warnings.len(), 1);
    assert!(output.warnings[0].contains("capacity"));
}

#[test]
fn recall_entries_and_shape_are_bounded() {
    for (input, expected) in [
        (
            json!({ "entries": [], "query_seed": 0 }),
            "at least one entry",
        ),
        (
            json!({
                "entries": vec![json!({ "key_seed": 0, "value_seed": 1 }); 33],
                "query_seed": 0,
            }),
            "entry count 33 exceeds limit 32",
        ),
        (
            json!({ "entries": [{ "key_seed": "bad", "value_seed": 1 }], "query_seed": 0 }),
            "integer seeds",
        ),
    ] {
        let error = ProbeEngine::new()
            .unwrap()
            .execute(&RECALL.parse().unwrap(), &input)
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
fn recall_cooperative_limits_are_enforced() {
    let input = serde_json::to_value(HolographicRecallRequest {
        entries: vec![
            HolographicEntry {
                key_seed: 3,
                value_seed: 103,
            },
            HolographicEntry {
                key_seed: 5,
                value_seed: 105,
            },
            HolographicEntry {
                key_seed: 8,
                value_seed: 108,
            },
        ],
        query_seed: 5,
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
                max_operations: 9_983,
                ..ProbeEngineLimits::default()
            },
            "operations",
        ),
        (
            ProbeEngineLimits {
                max_nodes: 2_306,
                ..ProbeEngineLimits::default()
            },
            "nodes",
        ),
        (
            ProbeEngineLimits {
                max_iterations: 6,
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
            .execute(&RECALL.parse().unwrap(), &input)
            .unwrap_err();
        assert!(
            matches!(error, DiscoveryError::LimitExceeded(ref message) if message.contains(expected)),
            "unexpected error for {expected}: {error}"
        );
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
