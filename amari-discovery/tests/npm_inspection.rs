// SPDX-License-Identifier: MIT OR Apache-2.0

//! End-to-end npm/JavaScript/TypeScript inspection through `amari inspect`.

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::TempDir;

const DECLARATION_PATH: &str = "node_modules/@justinelliottcobb/amari-wasm/amari_wasm.d.ts";

fn fixture_source() -> &'static Path {
    Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/ts-project"
    ))
}

fn materialize_fixture(version: &str) -> TempDir {
    let temp = TempDir::new().unwrap();
    copy_and_transform(fixture_source(), temp.path(), version);
    temp
}

fn copy_and_transform(src: &Path, dst: &Path, version: &str) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        let name_text = name.to_string_lossy();
        let source = entry.path();
        if source.is_dir() {
            copy_and_transform(&source, &dst.join(name), version);
        } else if name_text == "amari_wasm.d.ts.fixture" {
            let declaration = dst.join(DECLARATION_PATH);
            fs::create_dir_all(declaration.parent().unwrap()).unwrap();
            fs::copy(source, declaration).unwrap();
        } else if let Some(base) = name_text.strip_suffix(".in") {
            let content = fs::read_to_string(source).unwrap();
            fs::write(
                dst.join(base),
                content.replace("__AMARI_VERSION__", version),
            )
            .unwrap();
        } else {
            fs::copy(source, dst.join(name)).unwrap();
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
fn current_npm_project_emits_composed_snapshot_envelope() {
    let version = amari_discovery::Catalog::embedded()
        .unwrap()
        .version()
        .to_string();
    let temp = materialize_fixture(&version);
    let value = inspect_json(temp.path());

    assert_eq!(value["schema_version"], "amari.discovery/v1");
    assert_eq!(value["data"]["project_kind"], "npm_type_script");
    assert_eq!(
        value["provenance"]["project_hash"],
        value["data"]["project_hash"]
    );
    assert_eq!(value["provenance"]["compatibility"]["status"], "applicable");
    assert_eq!(value["provenance"]["replay"]["replayable"], true);

    let npm = &value["data"]["npm"];
    assert_eq!(npm["package"]["name"], "amari-typescript-project");
    assert!(npm["package"]["dependencies"]
        .as_array()
        .is_some_and(|dependencies| dependencies.iter().any(|dependency| {
            dependency["package_name"] == "@justinelliottcobb/amari-wasm"
                && dependency["resolved_version"] == version
                && dependency["compatibility"]["status"] == "applicable"
        })));

    let typescript = &value["data"]["typescript"];
    assert!(typescript["imports"]
        .as_array()
        .is_some_and(|imports| !imports.is_empty()));
    assert!(typescript["declaration_exports"]
        .as_array()
        .is_some_and(|exports| !exports.is_empty()));
    assert!(typescript["capabilities"]
        .as_array()
        .is_some_and(|capabilities| capabilities.iter().any(|evidence| {
            evidence["wasm_path"] == "WasmMultivector300.geometricProduct"
                && evidence["capability_id"] == "amari:amari-core:product:geometric-product"
        })));

    let required_hashes = value["provenance"]["replay"]["required_hashes"]
        .as_array()
        .unwrap();
    assert!(required_hashes.iter().any(|hash| hash == "npm.input_hash"));
    assert!(required_hashes
        .iter()
        .any(|hash| hash == "typescript.input_hash"));

    let serialized = serde_json::to_string(&value).unwrap();
    assert!(!serialized.contains(temp.path().to_str().unwrap()));
    assert!(!serialized.contains("Browser-side Amari WASM integration"));
    assert!(!serialized.contains("AMARI_RUNTIME"));
}

#[test]
fn stale_npm_project_reports_unknown_version_without_failing() {
    let temp = materialize_fixture("0.19.0");
    let value = inspect_json(temp.path());

    assert_eq!(
        value["provenance"]["compatibility"]["status"],
        "unknown_version"
    );
    assert!(value["data"]["npm"]["package"]["dependencies"]
        .as_array()
        .unwrap()
        .iter()
        .any(|dependency| {
            dependency["resolved_version"] == "0.19.0"
                && dependency["compatibility"]["status"] == "unknown_version"
        }));
}

#[test]
fn npm_partial_state_propagates_through_shared_snapshot() {
    use amari_discovery::{inspect_npm_typescript_project, InspectionLimits, SnapshotState};

    let version = amari_discovery::Catalog::embedded()
        .unwrap()
        .version()
        .to_string();
    let temp = materialize_fixture(&version);
    let limits = InspectionLimits {
        max_inspection_bytes: 800,
        ..InspectionLimits::default()
    };
    let snapshot = inspect_npm_typescript_project(temp.path(), &limits).unwrap();

    assert!(snapshot.npm.is_some());
    assert!(snapshot.typescript.is_some());
    assert!(matches!(
        snapshot.state,
        SnapshotState::LimitExceeded { .. }
    ));
}

#[test]
fn human_npm_inspection_matches_json_evidence_categories() {
    let version = amari_discovery::Catalog::embedded()
        .unwrap()
        .version()
        .to_string();
    let temp = materialize_fixture(&version);
    let value = inspect_json(temp.path());
    let dependencies = value["data"]["npm"]["package"]["dependencies"]
        .as_array()
        .unwrap()
        .len();
    let imports = value["data"]["typescript"]["imports"]
        .as_array()
        .unwrap()
        .len();
    let capabilities = value["data"]["typescript"]["capabilities"]
        .as_array()
        .unwrap()
        .len();

    Command::cargo_bin("amari")
        .unwrap()
        .arg("inspect")
        .arg(temp.path())
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains("npm/TypeScript project"))
        .stdout(predicate::str::contains(format!(
            "Amari npm dependencies: {dependencies}"
        )))
        .stdout(predicate::str::contains(format!(
            "JS/TS imports: {imports}"
        )))
        .stdout(predicate::str::contains(format!(
            "WASM capabilities: {capabilities}"
        )))
        .stdout(predicate::str::contains("schema_version").not())
        .stdout(predicate::str::contains(temp.path().to_string_lossy().as_ref()).not());
}
