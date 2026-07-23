// SPDX-License-Identifier: MIT OR Apache-2.0

//! Portable functional budgets for discovery's agent-facing command surface.

use std::{
    fs,
    time::{Duration, Instant},
};

use assert_cmd::Command;
use tempfile::TempDir;

const MAX_CAPABILITIES_JSON_BYTES: usize = 8 * 1024;
const MAX_SEARCH_JSON_BYTES: usize = 4 * 1024;
const MAX_RECOMMENDATION_JSON_BYTES: usize = 32 * 1024;
const MAX_SMALL_INSPECTION_DURATION: Duration = Duration::from_secs(30);

fn command_output(arguments: &[&str]) -> Vec<u8> {
    let output = Command::cargo_bin("amari")
        .expect("amari test binary")
        .args(arguments)
        .output()
        .expect("run amari command");
    assert!(
        output.status.success(),
        "command {arguments:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn small_project() -> TempDir {
    let temporary = tempfile::tempdir().expect("temporary project");
    fs::create_dir(temporary.path().join("src")).expect("create source directory");
    let manifest = format!(
        r#"[package]
name = "discovery-budget-fixture"
version = "0.1.0"
edition = "2021"

[dependencies]
amari-core = "{}"
"#,
        env!("CARGO_PKG_VERSION")
    );
    fs::write(temporary.path().join("Cargo.toml"), manifest).expect("write manifest");
    fs::write(
        temporary.path().join("src/lib.rs"),
        "use amari_core::Multivector;\npub fn scalar() -> Multivector<3, 0, 0> { Multivector::scalar(1.0) }\n",
    )
    .expect("write source");
    temporary
}

fn assert_single_compact_record(bytes: &[u8]) {
    assert_eq!(
        bytes.iter().filter(|byte| **byte == b'\n').count(),
        1,
        "machine output must remain one compact record"
    );
    assert_eq!(bytes.last(), Some(&b'\n'));
}

fn approximate_tokens(bytes: usize) -> usize {
    bytes.div_ceil(4)
}

#[test]
fn capabilities_json_stays_within_agent_context_budget() {
    let bytes = command_output(&["capabilities", "--json"]);
    assert_single_compact_record(&bytes);
    assert!(
        bytes.len() <= MAX_CAPABILITIES_JSON_BYTES,
        "capabilities output grew to {} bytes (~{} tokens), above {} bytes",
        bytes.len(),
        approximate_tokens(bytes.len()),
        MAX_CAPABILITIES_JSON_BYTES
    );
}

#[test]
fn compact_search_stays_within_agent_context_budget() {
    let bytes = command_output(&["discover", "search", "tropical", "--json"]);
    assert_single_compact_record(&bytes);
    assert!(
        bytes.len() <= MAX_SEARCH_JSON_BYTES,
        "compact search output grew to {} bytes (~{} tokens), above {} bytes",
        bytes.len(),
        approximate_tokens(bytes.len()),
        MAX_SEARCH_JSON_BYTES
    );
}

#[test]
fn small_fixture_inspection_has_a_coarse_portable_deadline() {
    let project = small_project();
    let started = Instant::now();
    let bytes = command_output(&[
        "inspect",
        project.path().to_str().expect("UTF-8 project path"),
        "--json",
    ]);
    let elapsed = started.elapsed();

    assert_single_compact_record(&bytes);
    assert!(
        elapsed <= MAX_SMALL_INSPECTION_DURATION,
        "small fixture inspection took {elapsed:?}, above the coarse {MAX_SMALL_INSPECTION_DURATION:?} budget"
    );
}

#[test]
fn recommendation_is_byte_deterministic_and_bounded() {
    let project = small_project();
    let project_path = project.path().to_str().expect("UTF-8 project path");
    let arguments = [
        "recommend",
        project_path,
        "--goal",
        "compute a geometric product",
        "--json",
    ];
    let first = command_output(&arguments);
    let second = command_output(&arguments);

    assert_single_compact_record(&first);
    assert_eq!(first, second, "fixed-input recommendation output drifted");
    assert!(
        first.len() <= MAX_RECOMMENDATION_JSON_BYTES,
        "recommendation output grew to {} bytes (~{} tokens), above {} bytes",
        first.len(),
        approximate_tokens(first.len()),
        MAX_RECOMMENDATION_JSON_BYTES
    );
}
