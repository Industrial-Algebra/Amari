// SPDX-License-Identifier: MIT OR Apache-2.0

//! Provenance, privacy, and cross-process planner replay hardening.

use std::fs;
use std::path::{Path, PathBuf};

use amari_discovery::{Catalog, RecallConfig};
use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::{json, Value};
use tempfile::{NamedTempFile, TempDir};
use walkdir::WalkDir;

const GOAL: &str = "differentiate a scalar polynomial with forward dual numbers";
const CAPABILITY: &str = "amari:amari-dual:autodiff:forward-derivative";
const SECRET: &str = "AKIA_REPLAY_PRIVATE_7f42d119";

fn fixture_source() -> &'static Path {
    Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/rust-project"
    ))
}

fn materialize_fixture() -> TempDir {
    let temp = TempDir::new().unwrap();
    let version = Catalog::embedded().unwrap().version().to_owned();
    copy_and_transform(fixture_source(), temp.path(), &version);
    temp
}

fn copy_and_transform(source: &Path, target: &Path, version: &str) {
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        let text = name.to_string_lossy();
        let source_path = entry.path();
        if source_path.is_dir() {
            let target_path = target.join(&name);
            fs::create_dir_all(&target_path).unwrap();
            copy_and_transform(&source_path, &target_path, version);
        } else if text.ends_with(".in") {
            let target_path = target.join(text.trim_end_matches(".in"));
            let contents = fs::read_to_string(source_path).unwrap();
            fs::write(target_path, contents.replace("__AMARI_VERSION__", version)).unwrap();
        } else if (text == "Cargo.toml" || text == "Cargo.lock")
            && source.join(format!("{text}.in")).exists()
        {
            continue;
        } else {
            fs::copy(source_path, target.join(name)).unwrap();
        }
    }
}

fn tree_contents(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut files = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .map(Result::unwrap)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| {
            (
                entry.path().strip_prefix(root).unwrap().to_owned(),
                fs::read(entry.path()).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.0.cmp(&right.0));
    files
}

fn recommend(root: &Path) -> Value {
    let output = Command::cargo_bin("amari")
        .unwrap()
        .arg("recommend")
        .arg(root)
        .args(["--goal", GOAL, "--json"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).unwrap()
}

fn save(value: &Value) -> NamedTempFile {
    let file = NamedTempFile::new().unwrap();
    serde_json::to_writer(file.as_file(), value).unwrap();
    file
}

fn plan_command(root: &Path, artifact: &Path) -> Command {
    let mut command = Command::cargo_bin("amari").unwrap();
    command
        .arg("plan")
        .arg(CAPABILITY)
        .arg("--recommendation")
        .arg(artifact)
        .arg("--project")
        .arg(root)
        .arg("--json");
    command
}

fn replay(root: &Path, recommendation: &Value) -> Vec<u8> {
    let artifact = save(recommendation);
    plan_command(root, artifact.path())
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .get_output()
        .stdout
        .clone()
}

fn selected_mut(recommendation: &mut Value) -> &mut Value {
    &mut recommendation["data"]["data"]["preferred"]
}

#[test]
fn recommendation_replays_in_a_fresh_process_with_canonical_provenance() {
    let fixture = materialize_fixture();
    let before = tree_contents(fixture.path());
    let recommendation = recommend(fixture.path());
    let catalog = Catalog::embedded().unwrap();

    assert_eq!(
        recommendation["provenance"]["seed"],
        json!(RecallConfig::default().seed)
    );
    assert_eq!(
        recommendation["provenance"]["catalog"]["version"],
        catalog.version()
    );
    assert_eq!(
        recommendation["provenance"]["catalog"]["hash"],
        catalog.content_hash()
    );
    for pointer in [
        "/provenance/catalog/hash",
        "/provenance/project_hash",
        "/provenance/input_hash",
        "/data/data/preferred/plan_hash",
    ] {
        let hash = recommendation.pointer(pointer).unwrap().as_str().unwrap();
        assert_eq!(hash.len(), 64, "{pointer}");
        assert!(hash.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }

    let first = replay(fixture.path(), &recommendation);
    let second = replay(fixture.path(), &recommendation);
    let replayed: Value = serde_json::from_slice(&first).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        replayed["data"],
        recommendation["data"]["data"]["preferred"]
    );
    assert_eq!(tree_contents(fixture.path()), before);
}

#[test]
fn coherent_stale_hashes_are_rejected_against_current_authority() {
    let fixture = materialize_fixture();
    let recommendation = recommend(fixture.path());

    for (field, envelope_pointer, plan_pointer) in [
        (
            "catalog_hash",
            "/provenance/catalog/hash",
            "/data/data/preferred/compatibility/catalog/hash",
        ),
        (
            "project_hash",
            "/provenance/project_hash",
            "/data/data/preferred/compatibility/project_hash",
        ),
        (
            "input_hash",
            "/provenance/input_hash",
            "/data/data/preferred/compatibility/input_hash",
        ),
    ] {
        let mut stale = recommendation.clone();
        *stale.pointer_mut(envelope_pointer).unwrap() = json!("0".repeat(64));
        *stale.pointer_mut(plan_pointer).unwrap() = json!("0".repeat(64));
        let artifact = save(&stale);

        plan_command(fixture.path(), artifact.path())
            .assert()
            .code(2)
            .stdout(predicate::str::is_empty())
            .stderr(predicate::str::contains(field));
    }
}

#[test]
fn replay_rejects_seed_tool_and_compatibility_provenance_tampering() {
    let fixture = materialize_fixture();
    let recommendation = recommend(fixture.path());

    let cases = [
        ("seed", "/provenance/seed", json!(1)),
        ("seed", "/provenance/seed", Value::Null),
        (
            "tool_version",
            "/provenance/tool_version",
            json!("99.99.99"),
        ),
        (
            "compatibility",
            "/provenance/compatibility/reasons",
            json!([SECRET]),
        ),
    ];
    for (field, pointer, replacement) in cases {
        let mut tampered = recommendation.clone();
        *tampered.pointer_mut(pointer).unwrap() = replacement;
        let artifact = save(&tampered);
        plan_command(fixture.path(), artifact.path())
            .assert()
            .code(2)
            .stdout(predicate::str::is_empty())
            .stderr(predicate::str::contains(field).and(predicate::str::contains(SECRET).not()));
    }
}

#[test]
fn untrusted_artifact_warnings_are_not_reflected_by_replay() {
    let fixture = materialize_fixture();
    let mut recommendation = recommend(fixture.path());
    recommendation["warnings"] = json!([SECRET]);

    let output = replay(fixture.path(), &recommendation);
    let value: Value = serde_json::from_slice(&output).unwrap();

    assert!(!String::from_utf8(output).unwrap().contains(SECRET));
    assert!(!value["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|warning| warning == SECRET));
}

#[test]
fn replay_protocol_rejects_unknown_nested_fields() {
    let fixture = materialize_fixture();
    let mut recommendation = recommend(fixture.path());
    selected_mut(&mut recommendation)["compatibility"]["unbounded_authority"] = json!(SECRET);
    let artifact = save(&recommendation);

    plan_command(fixture.path(), artifact.path())
        .assert()
        .code(9)
        .stdout(predicate::str::is_empty())
        .stderr(
            predicate::str::contains("serialization").and(predicate::str::contains(SECRET).not()),
        );
}

#[test]
fn recommendation_and_replay_do_not_leak_sensitive_project_evidence() {
    let fixture = materialize_fixture();
    fs::write(
        fixture.path().join("src/private_evidence.rs"),
        format!("// credential marker: {SECRET}\npub fn harmless() {{}}\n"),
    )
    .unwrap();
    let before = tree_contents(fixture.path());

    let recommendation = recommend(fixture.path());
    let recommendation_bytes = serde_json::to_vec(&recommendation).unwrap();
    let replay_bytes = replay(fixture.path(), &recommendation);

    for bytes in [&recommendation_bytes, &replay_bytes] {
        let output = String::from_utf8(bytes.clone()).unwrap();
        assert!(!output.contains(SECRET));
        assert!(!output.contains(fixture.path().to_str().unwrap()));
    }
    assert_eq!(tree_contents(fixture.path()), before);
}

#[test]
fn replay_hash_constructor_rejects_malformed_project_and_probe_hashes() {
    let catalog = Catalog::embedded().unwrap();
    let goal = amari_discovery::GoalSpec {
        statement: GOAL.to_owned(),
        constraints: Vec::new(),
    };
    let malformed_probe = amari_discovery::ProbeReplayHash {
        probe_id: "amari-probe:dual:polynomial-derivative:v1".parse().unwrap(),
        input_hash: "not-a-hash".to_owned(),
        result_hash: "0".repeat(64),
    };

    for (project_hash, probes) in [
        ("not-a-hash".to_owned(), Vec::new()),
        ("0".repeat(64), vec![malformed_probe]),
    ] {
        assert!(amari_discovery::PlanCompatibility::from_replay_hashes(
            &catalog,
            project_hash,
            &goal,
            probes,
        )
        .is_err());
    }
}
