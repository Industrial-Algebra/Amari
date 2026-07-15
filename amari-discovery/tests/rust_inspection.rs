// SPDX-License-Identifier: MIT OR Apache-2.0

//! End-to-end Rust/Cargo inspection through the installed `amari` binary.

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::TempDir;

fn fixture_source() -> &'static Path {
    Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/rust-project"
    ))
}

fn materialize_fixture(version: &str) -> TempDir {
    let temp = TempDir::new().unwrap();
    copy_and_transform(fixture_source(), temp.path(), version);
    temp
}

fn copy_and_transform(src: &Path, dst: &Path, version: &str) {
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();
        let src_path = entry.path();
        if src_path.is_dir() {
            let dst_sub = dst.join(&*file_name);
            fs::create_dir_all(&dst_sub).unwrap();
            copy_and_transform(&src_path, &dst_sub, version);
        } else if name.ends_with(".in") {
            let base = name.trim_end_matches(".in");
            let content = fs::read_to_string(&src_path).unwrap();
            fs::write(
                dst.join(base),
                content.replace("__AMARI_VERSION__", version),
            )
            .unwrap();
        } else if (name == "Cargo.toml" || name == "Cargo.lock")
            && src.join(format!("{name}.in")).exists()
        {
            // Generated fixture files are never authoritative.
            continue;
        } else {
            fs::copy(&src_path, dst.join(&*file_name)).unwrap();
        }
    }
}

fn inspect_json(root: &Path) -> Value {
    let output = Command::cargo_bin("amari")
        .unwrap()
        .arg("inspect")
        .arg(root)
        .arg("--json")
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).unwrap()
}

#[test]
fn current_rust_project_emits_composed_snapshot_envelope() {
    let version = amari_discovery::Catalog::embedded()
        .unwrap()
        .version()
        .to_string();
    let temp = materialize_fixture(&version);
    let value = inspect_json(temp.path());

    assert_eq!(value["schema_version"], "amari.discovery/v1");
    assert_eq!(value["data"]["project_kind"], "rust_cargo");
    assert_eq!(
        value["provenance"]["project_hash"],
        value["data"]["project_hash"]
    );
    assert_eq!(value["provenance"]["compatibility"]["status"], "applicable");
    assert_eq!(value["provenance"]["replay"]["replayable"], true);

    let cargo = &value["data"]["cargo"];
    assert_eq!(cargo["root_package"]["name"], "rust-project");
    assert!(cargo["root_package"]["dependencies"]
        .as_array()
        .is_some_and(|dependencies| !dependencies.is_empty()));

    let rust = &value["data"]["rust"];
    assert!(rust["usages"]
        .as_array()
        .is_some_and(|usages| !usages.is_empty()));
    assert!(rust["vocabulary"]
        .as_array()
        .is_some_and(|terms| !terms.is_empty()));

    let platform = &value["data"]["platform"];
    assert!(platform["benchmarks"]
        .as_array()
        .is_some_and(|benches| benches.iter().any(|bench| bench["name"] == "speed_bench")));
    assert!(platform["wasm_targets"]
        .as_array()
        .is_some_and(|targets| targets
            .iter()
            .any(|target| target["target"] == "wasm32-unknown-unknown")));

    let serialized = serde_json::to_string(&value).unwrap();
    assert!(!serialized.contains(temp.path().to_str().unwrap()));
    assert!(!serialized.contains("secret-native-flag"));
    assert!(!serialized.contains("--tool=memcheck"));
}

#[test]
fn stale_rust_project_reports_unknown_version_without_failing() {
    let temp = materialize_fixture("0.19.0");
    let value = inspect_json(temp.path());

    assert_eq!(
        value["provenance"]["compatibility"]["status"],
        "unknown_version"
    );
    let dependencies = value["data"]["cargo"]["root_package"]["dependencies"]
        .as_array()
        .unwrap();
    assert!(dependencies.iter().any(|dependency| {
        dependency["resolved_version"] == "0.19.0"
            && dependency["compatibility"]["status"] == "unknown_version"
    }));
}

#[test]
fn composed_snapshot_propagates_semantic_partial_state() {
    use amari_discovery::{inspect_rust_project, InspectionLimits, SnapshotState};

    let version = amari_discovery::Catalog::embedded()
        .unwrap()
        .version()
        .to_string();
    let temp = materialize_fixture(&version);
    let config_path = temp.path().join(".cargo").join("config.toml");
    let mut config = fs::read_to_string(&config_path).unwrap();
    config.push_str(&"# bounded platform input\n".repeat(400));
    fs::write(config_path, config).unwrap();

    let limits = InspectionLimits {
        max_per_file_bytes: 4_096,
        ..InspectionLimits::default()
    };
    let snapshot = inspect_rust_project(temp.path(), &limits).unwrap();

    assert!(matches!(
        snapshot.platform.as_ref().map(|platform| &platform.state),
        Some(SnapshotState::LimitExceeded { .. })
    ));
    assert!(matches!(
        snapshot.state,
        SnapshotState::LimitExceeded { .. }
    ));
}

#[test]
fn human_rust_inspection_summarizes_typed_evidence() {
    let version = amari_discovery::Catalog::embedded()
        .unwrap()
        .version()
        .to_string();
    let temp = materialize_fixture(&version);

    Command::cargo_bin("amari")
        .unwrap()
        .arg("inspect")
        .arg(temp.path())
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains("Rust/Cargo project"))
        .stdout(predicate::str::contains("Amari dependencies"))
        .stdout(predicate::str::contains("API usages"))
        .stdout(predicate::str::contains("Benchmarks"))
        .stdout(predicate::str::contains("WASM targets"))
        .stdout(predicate::str::contains("schema_version").not())
        .stdout(predicate::str::contains(temp.path().to_string_lossy().as_ref()).not());
}
