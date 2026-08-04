// SPDX-License-Identifier: MIT OR Apache-2.0

//! Hidden probe-worker framing and registry-only dispatch contract.

#![cfg(feature = "standard-probes")]

use amari_discovery::{Catalog, ProbeEngine, ProbeEngineLimits};
use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::{json, Value};

const MAX_FRAME_BYTES: u32 = 2 * 1024 * 1024;

fn request() -> Value {
    let catalog = Catalog::embedded().unwrap();
    json!({
        "probe_id": "amari-probe:tropical:viterbi:v1",
        "input": {
            "transitions": [[-1.0, -2.0], [-2.0, -1.0]],
            "emissions": [[-1.0, -3.0], [-3.0, -1.0]],
            "observations": [0, 1, 0]
        },
        "limits": ProbeEngineLimits::default(),
        "provenance": {
            "tool_version": env!("CARGO_PKG_VERSION"),
            "catalog": {
                "version": catalog.version(),
                "hash": catalog.content_hash()
            },
            "compatibility": {
                "status": "compatible",
                "reasons": []
            },
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

fn frame(value: &Value) -> Vec<u8> {
    let body = serde_json::to_vec(value).unwrap();
    let length = u32::try_from(body.len()).unwrap();
    let mut framed = length.to_be_bytes().to_vec();
    framed.extend_from_slice(&body);
    framed
}

fn run_worker(stdin: &[u8]) -> assert_cmd::assert::Assert {
    let mut command = Command::cargo_bin("amari").unwrap();
    command
        .arg("__probe-worker")
        .write_stdin(stdin.to_vec())
        .assert()
}

fn decode_single_frame(bytes: &[u8]) -> Value {
    assert!(bytes.len() >= 4, "response must contain a frame header");
    let length = u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize;
    assert_eq!(
        bytes.len(),
        length + 4,
        "response must be exactly one frame"
    );
    serde_json::from_slice(&bytes[4..]).unwrap()
}

#[test]
fn worker_request_has_only_typed_data_and_success_preserves_provenance() {
    let request = request();
    let mut keys = request
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    keys.sort();
    assert_eq!(keys, ["input", "limits", "probe_id", "provenance"]);
    let forbidden = [
        "path",
        "project_path",
        "handle",
        "executable",
        "command",
        "args",
        "shell",
    ];
    let encoded = serde_json::to_string(&request).unwrap();
    for field in forbidden {
        assert!(!encoded.contains(&format!("\"{field}\"")));
    }

    let output = run_worker(&frame(&request))
        .success()
        .stderr(predicate::str::is_empty())
        .get_output()
        .stdout
        .clone();
    let response = decode_single_frame(&output);

    assert_eq!(response["provenance"], request["provenance"]);
    assert_eq!(
        response["execution"]["probe_id"],
        "amari-probe:tropical:viterbi:v1"
    );
    assert_eq!(response["execution"]["isolation"], "cooperative");
    let direct = ProbeEngine::new()
        .unwrap()
        .execute(
            &"amari-probe:tropical:viterbi:v1".parse().unwrap(),
            &request["input"],
        )
        .unwrap();
    assert_eq!(response["execution"]["output"], direct.output);
    assert_eq!(
        response["execution"]["schema_hashes"],
        serde_json::to_value(direct.schema_hashes).unwrap()
    );
}

#[test]
fn worker_rejects_truncated_and_trailing_frames() {
    run_worker(&[0, 0, 0])
        .failure()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("invalid_input"));

    let mut truncated = 10_u32.to_be_bytes().to_vec();
    truncated.extend_from_slice(b"{}");
    run_worker(&truncated)
        .failure()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("invalid_input"));

    let mut trailing = frame(&request());
    trailing.push(0);
    run_worker(&trailing)
        .failure()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("invalid_input"));
}

#[test]
fn worker_rejects_oversized_frames_before_allocating_the_body() {
    run_worker(&(MAX_FRAME_BYTES + 1).to_be_bytes())
        .failure()
        .code(7)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("limit_exceeded"));
}

#[test]
fn worker_rejects_unknown_fields_and_unknown_probes() {
    let mut unknown_field = request();
    unknown_field["executable"] = json!("/tmp/not-allowed");
    run_worker(&frame(&unknown_field))
        .failure()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("serialization"));

    let mut unknown_probe = request();
    unknown_probe["probe_id"] = json!("amari-probe:tropical:not-registered:v1");
    run_worker(&frame(&unknown_probe))
        .failure()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("unknown probe"));
}

#[test]
fn worker_mode_is_hidden_from_public_help() {
    Command::cargo_bin("amari")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("__probe-worker").not());
}
