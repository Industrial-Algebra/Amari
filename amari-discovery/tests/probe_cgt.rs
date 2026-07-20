// SPDX-License-Identifier: MIT OR Apache-2.0

//! Combinatorial-game nim-sum probe parity and limits.

#![cfg(feature = "standard-probes")]

use amari_cgt::GameArena;
use amari_discovery::{
    CgtNimSumOutput, CgtNimSumRequest, DiscoveryError, ProbeEngine, ProbeEngineLimits,
    ProbeExecution,
};
use serde_json::json;

const NIM_SUM: &str = "amari-probe:cgt:nim-sum:v1";

fn request() -> CgtNimSumRequest {
    CgtNimSumRequest {
        heaps: vec![3, 4, 5],
    }
}

fn execute(engine: &ProbeEngine, request: CgtNimSumRequest) -> ProbeExecution {
    engine
        .execute(
            &NIM_SUM.parse().unwrap(),
            &serde_json::to_value(request).unwrap(),
        )
        .unwrap()
}

fn direct_grundy_values(heaps: &[u32]) -> Vec<u32> {
    let mut arena = GameArena::new();
    heaps
        .iter()
        .map(|size| {
            let heap = arena.nim_heap(*size).unwrap();
            arena.grundy(heap).unwrap().0
        })
        .collect()
}

#[test]
fn output_matches_direct_per_heap_grundy_and_xor() {
    let request = request();
    let expected = direct_grundy_values(&request.heaps);
    let engine = ProbeEngine::new().unwrap();
    let first = execute(&engine, request.clone());
    let second = execute(&engine, request);
    let output: CgtNimSumOutput = serde_json::from_value(first.output.clone()).unwrap();

    assert_eq!(first, second);
    assert_eq!(output.grundy_values, expected);
    assert_eq!(output.grundy_values, vec![3, 4, 5]);
    assert_eq!(output.nim_sum, 3 ^ 4 ^ 5);
    assert!(first.resources.operations > 0);
    assert!(first.resources.nodes > 0);
    assert!(first.resources.iterations > 0);
}

#[test]
fn empty_heaps_have_the_additive_identity_nim_sum() {
    let output: CgtNimSumOutput = serde_json::from_value(
        execute(
            &ProbeEngine::new().unwrap(),
            CgtNimSumRequest { heaps: Vec::new() },
        )
        .output,
    )
    .unwrap();

    assert!(output.grundy_values.is_empty());
    assert_eq!(output.nim_sum, 0);
}

#[test]
fn heap_count_and_value_are_bounded_before_arena_allocation() {
    let too_many = json!({ "heaps": vec![0; 257] });
    assert!(matches!(
        ProbeEngine::new()
            .unwrap()
            .execute(&NIM_SUM.parse().unwrap(), &too_many),
        Err(DiscoveryError::LimitExceeded(message))
            if message.contains("heap count 257 exceeds limit 256")
    ));

    let too_large = json!({ "heaps": [65] });
    assert!(matches!(
        ProbeEngine::new()
            .unwrap()
            .execute(&NIM_SUM.parse().unwrap(), &too_large),
        Err(DiscoveryError::LimitExceeded(message))
            if message.contains("heap value 65 exceeds limit 64")
    ));
}

#[test]
fn malformed_and_overflowing_heap_values_are_rejected() {
    for input in [
        json!({ "heaps": [-1] }),
        json!({ "heaps": [4294967296_u64] }),
    ] {
        assert!(matches!(
            ProbeEngine::new()
                .unwrap()
                .execute(&NIM_SUM.parse().unwrap(), &input),
            Err(DiscoveryError::InvalidInput(message)) if message.contains("heap")
        ));
    }
}

#[test]
fn maximum_heap_value_is_accepted_with_checked_option_accounting() {
    let execution = execute(
        &ProbeEngine::new().unwrap(),
        CgtNimSumRequest { heaps: vec![64] },
    );
    let output: CgtNimSumOutput = serde_json::from_value(execution.output).unwrap();

    assert_eq!(output.grundy_values, vec![64]);
    assert_eq!(output.nim_sum, 64);
    assert!(execution.resources.operations <= 10_000);
    assert_eq!(execution.resources.nodes, 65);
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
                max_nodes: 5,
                ..ProbeEngineLimits::default()
            },
            "nodes",
        ),
        (
            ProbeEngineLimits {
                max_iterations: 7,
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
            .execute(&NIM_SUM.parse().unwrap(), &input)
            .unwrap_err();
        assert!(
            matches!(error, DiscoveryError::LimitExceeded(ref message) if message.contains(expected)),
            "unexpected error for {expected}: {error}"
        );
    }
}
