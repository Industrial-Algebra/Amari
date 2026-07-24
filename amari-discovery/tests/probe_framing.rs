// SPDX-License-Identifier: MIT OR Apache-2.0

//! Property-oriented hidden-worker framing and boundary hardening.

#![cfg(feature = "standard-probes")]

use amari_discovery::{Catalog, ProbeEngine, ProbeEngineLimits};
use assert_cmd::Command;
use predicates::prelude::*;
use proptest::prelude::*;
use serde_json::{json, Value};

const MAX_FRAME_BYTES: u32 = 2 * 1024 * 1024;
const VITERBI_PROBE: &str = "amari-probe:tropical:viterbi:v1";

fn input() -> Value {
    json!({
        "transitions": [[-1.0, -2.0], [-2.0, -1.0]],
        "emissions": [[-1.0, -3.0], [-3.0, -1.0]],
        "observations": [0, 1, 0]
    })
}

fn request_with_limits(limits: ProbeEngineLimits) -> Value {
    let catalog = Catalog::embedded().unwrap();
    json!({
        "probe_id": VITERBI_PROBE,
        "input": input(),
        "limits": limits,
        "provenance": {
            "tool_version": env!("CARGO_PKG_VERSION"),
            "catalog": {
                "version": catalog.version(),
                "hash": catalog.content_hash()
            },
            "compatibility": {"status": "compatible", "reasons": []},
            "replay": {
                "replayable": true,
                "required_hashes": ["catalog_hash", "input_hash"],
                "reasons": []
            },
            "project_hash": null,
            "input_hash": "fixture-worker-input-hash",
            "seed": null
        }
    })
}

fn frame_body(body: &[u8]) -> Vec<u8> {
    let length = u32::try_from(body.len()).unwrap();
    let mut frame = length.to_be_bytes().to_vec();
    frame.extend_from_slice(body);
    frame
}

fn frame(value: &Value) -> Vec<u8> {
    frame_body(&serde_json::to_vec(value).unwrap())
}

fn run_worker(stdin: &[u8]) -> assert_cmd::assert::Assert {
    let mut command = Command::cargo_bin("amari").unwrap();
    command
        .arg("__probe-worker")
        .write_stdin(stdin.to_vec())
        .assert()
}

fn decode_frame(bytes: &[u8]) -> Value {
    let length = u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize;
    assert_eq!(bytes.len(), length + 4);
    serde_json::from_slice(&bytes[4..]).unwrap()
}

fn malformed_frames() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        proptest::collection::vec(any::<u8>(), 0..4),
        proptest::collection::vec(any::<u8>(), 0..256).prop_map(|body| {
            let declared = u32::try_from(body.len() + 1).unwrap();
            let mut frame = declared.to_be_bytes().to_vec();
            frame.extend_from_slice(&body);
            frame
        }),
        proptest::collection::vec(any::<u8>(), 0..256).prop_map(|tail| {
            let mut body = vec![0xff];
            body.extend_from_slice(&tail);
            frame_body(&body)
        }),
        proptest::collection::vec(any::<u8>(), 1..64).prop_map(|trailing| {
            let mut frame = frame_body(b"{}");
            frame.extend_from_slice(&trailing);
            frame
        }),
        Just(0_u32.to_be_bytes().to_vec()),
        Just((MAX_FRAME_BYTES + 1).to_be_bytes().to_vec()),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn arbitrary_malformed_frames_fail_without_stdout(frame in malformed_frames()) {
        run_worker(&frame)
            .failure()
            .stdout(predicate::str::is_empty());
    }
}

#[test]
fn deeply_nested_and_oversized_json_are_rejected_with_typed_errors() {
    let mut nested = Value::Null;
    for _ in 0..160 {
        nested = Value::Array(vec![nested]);
    }
    let mut deep_request = request_with_limits(ProbeEngineLimits::default());
    deep_request["input"] = nested;
    run_worker(&frame(&deep_request))
        .failure()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("serialization"));

    let mut oversized_input = request_with_limits(ProbeEngineLimits::default());
    oversized_input["input"] = json!("x".repeat(1_048_576));
    run_worker(&frame(&oversized_input))
        .failure()
        .code(7)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("input bytes"));
}

#[test]
fn exact_operation_node_iteration_and_input_limits_pass_but_one_less_fails() {
    let direct = ProbeEngine::new()
        .unwrap()
        .execute(&VITERBI_PROBE.parse().unwrap(), &input())
        .unwrap();
    let input_bytes = u64::try_from(serde_json::to_vec(&input()).unwrap().len()).unwrap();
    assert!(direct.resources.operations > 0);
    assert!(direct.resources.nodes > 0);
    assert!(direct.resources.iterations > 0);

    let exact = ProbeEngineLimits {
        max_input_bytes: input_bytes,
        max_operations: direct.resources.operations,
        max_nodes: direct.resources.nodes,
        max_iterations: direct.resources.iterations,
        ..ProbeEngineLimits::default()
    };
    let output = run_worker(&frame(&request_with_limits(exact)))
        .success()
        .stderr(predicate::str::is_empty())
        .get_output()
        .stdout
        .clone();
    assert_eq!(
        decode_frame(&output)["execution"]["resources"],
        json!(direct.resources)
    );

    for tightened in 1_u8..16 {
        let limits = ProbeEngineLimits {
            max_input_bytes: input_bytes - u64::from(tightened & 0b0001 != 0),
            max_operations: direct.resources.operations - u64::from(tightened & 0b0010 != 0),
            max_nodes: direct.resources.nodes - u64::from(tightened & 0b0100 != 0),
            max_iterations: direct.resources.iterations - u64::from(tightened & 0b1000 != 0),
            ..exact
        };
        run_worker(&frame(&request_with_limits(limits)))
            .failure()
            .code(7)
            .stdout(predicate::str::is_empty())
            .stderr(predicate::str::contains("limit_exceeded"));
    }
}

#[test]
fn nested_limit_authority_and_unknown_probe_ids_are_rejected() {
    let mut unknown_limit = request_with_limits(ProbeEngineLimits::default());
    unknown_limit["limits"]["unbounded"] = json!(true);
    run_worker(&frame(&unknown_limit))
        .failure()
        .code(9)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("serialization"));

    let mut unknown_probe = request_with_limits(ProbeEngineLimits::default());
    unknown_probe["probe_id"] = json!("amari-probe:tropical:unknown:v1");
    run_worker(&frame(&unknown_probe))
        .failure()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("unknown probe"));
}

#[test]
fn repeated_malformed_requests_do_not_poison_a_later_valid_worker() {
    for _ in 0..8 {
        run_worker(&[0, 0, 0])
            .failure()
            .stdout(predicate::str::is_empty());
    }

    run_worker(&frame(&request_with_limits(ProbeEngineLimits::default())))
        .success()
        .stderr(predicate::str::is_empty());
}
