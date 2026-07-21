// SPDX-License-Identifier: MIT OR Apache-2.0

//! End-to-end npm/TypeScript recommendation through the installed `amari` binary.

use std::fs;
use std::path::{Path, PathBuf};

use amari_discovery::{
    inspect_npm_typescript_project, Catalog, InspectionLimits, ProbeBackend, ProbeResult,
    ResourceObservations,
};
use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::{json, Value};
use tempfile::{NamedTempFile, TempDir};
use walkdir::WalkDir;

const DECLARATION_PATH: &str = "node_modules/@justinelliottcobb/amari-wasm/amari_wasm.d.ts";
const GOAL: &str = "compute the geometric product of multivectors in browser WebAssembly";
const CORE_CAPABILITY: &str = "amari:amari-core:product:geometric-product";
const CORE_PROBE: &str = "amari-probe:core:geometric-product:v1";
const WASM_PACKAGE: &str = "@justinelliottcobb/amari-wasm";

fn fixture_source() -> &'static Path {
    Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/ts-project"
    ))
}

fn goal_fixture() -> &'static Path {
    Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/goals/geometric-product.json"
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
        let text = name.to_string_lossy();
        let source = entry.path();
        if source.is_dir() {
            copy_and_transform(&source, &dst.join(name), version);
        } else if text == "amari_wasm.d.ts.fixture" {
            let declaration = dst.join(DECLARATION_PATH);
            fs::create_dir_all(declaration.parent().unwrap()).unwrap();
            fs::copy(source, declaration).unwrap();
        } else if let Some(base) = text.strip_suffix(".in") {
            let contents = fs::read_to_string(source).unwrap();
            fs::write(
                dst.join(base),
                contents.replace("__AMARI_VERSION__", version),
            )
            .unwrap();
        } else {
            fs::copy(source, dst.join(name)).unwrap();
        }
    }
}

fn tree_contents(root: &Path) -> Vec<(PathBuf, Option<Vec<u8>>)> {
    let mut entries = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .map(Result::unwrap)
        .map(|entry| {
            let relative = entry.path().strip_prefix(root).unwrap().to_owned();
            let contents = entry
                .file_type()
                .is_file()
                .then(|| fs::read(entry.path()).unwrap());
            (relative, contents)
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    entries
}

fn recommend_json(
    root: &Path,
    inline_goal: Option<&str>,
    goal_file: Option<&Path>,
    probe_results: Option<&Path>,
) -> Value {
    let mut command = Command::cargo_bin("amari").unwrap();
    command.arg("recommend").arg(root);
    if let Some(goal) = inline_goal {
        command.args(["--goal", goal]);
    }
    if let Some(path) = goal_file {
        command.arg("--goal-file").arg(path);
    }
    if let Some(path) = probe_results {
        command.arg("--probe-results").arg(path);
    }
    let output = command
        .arg("--json")
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).unwrap()
}

fn recommendation(value: &Value) -> &Value {
    assert_eq!(value["data"]["status"], "recommended");
    &value["data"]["data"]
}

fn score<'a>(recommendation: &'a Value, capability: &str) -> &'a Value {
    recommendation["scores"]
        .as_array()
        .unwrap()
        .iter()
        .find(|score| score["capability_id"] == capability)
        .unwrap()
}

#[test]
fn goal_file_recommends_exact_wasm_package_symbols_probes_and_tests() {
    let catalog = Catalog::embedded().unwrap();
    let fixture = materialize_fixture(catalog.version());
    let value = recommend_json(fixture.path(), None, Some(goal_fixture()), None);
    let recommendation = recommendation(&value);
    let preferred = &recommendation["preferred"];

    assert_eq!(preferred["capability_id"], CORE_CAPABILITY);
    assert_eq!(recommendation["goal"]["statement"], GOAL);
    assert_eq!(
        recommendation["goal"]["constraints"],
        json!(["browser", "wasm"])
    );
    assert_eq!(value["provenance"]["compatibility"]["status"], "applicable");
    assert!(preferred["steps"].as_array().unwrap().iter().any(|step| {
        step["kind"] == "dependency"
            && step["package"] == WASM_PACKAGE
            && step["version"] == catalog.version()
    }));
    assert!(preferred["steps"].as_array().unwrap().iter().any(|step| {
        step["kind"] == "symbol" && step["path"] == "WasmMultivector300.geometricProduct"
    }));
    assert!(recommendation["suggested_probes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|probe| probe == CORE_PROBE));
    assert!(recommendation["suggested_tests"]
        .as_array()
        .unwrap()
        .iter()
        .any(|step| {
            step["kind"] == "test"
                && step["package"] == WASM_PACKAGE
                && step["target"] == "npm_package"
        }));
    assert!(score(recommendation, CORE_CAPABILITY)["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .any(|evidence| evidence["kind"] == "ranking_signal:evidence"));
    let serialized = serde_json::to_string(&value).unwrap();
    assert!(!serialized.contains(fixture.path().to_str().unwrap()));
    assert!(!serialized.contains("Browser-side Amari WASM integration"));
}

#[test]
fn inline_and_goal_file_paths_use_the_same_typed_pipeline() {
    let version = Catalog::embedded().unwrap().version().to_owned();
    let fixture = materialize_fixture(&version);
    let inline = recommend_json(fixture.path(), Some(GOAL), None, None);
    let from_file = recommend_json(fixture.path(), None, Some(goal_fixture()), None);

    assert_eq!(
        recommendation(&inline)["preferred"]["capability_id"],
        recommendation(&from_file)["preferred"]["capability_id"]
    );
    assert_eq!(
        recommendation(&inline)["scores"],
        recommendation(&from_file)["scores"]
    );
    assert_ne!(
        inline["provenance"]["input_hash"], from_file["provenance"]["input_hash"],
        "goal-file constraints must participate in replay identity"
    );
}

#[test]
fn matching_saved_probe_improves_typescript_candidate_verification() {
    let catalog = Catalog::embedded().unwrap();
    let fixture = materialize_fixture(catalog.version());
    let baseline = recommend_json(fixture.path(), None, Some(goal_fixture()), None);
    let baseline_verification = score(recommendation(&baseline), CORE_CAPABILITY)["components"]
        ["verification"]
        .as_f64()
        .unwrap();
    let snapshot =
        inspect_npm_typescript_project(fixture.path(), &InspectionLimits::default()).unwrap();
    let probe = ProbeResult {
        probe_id: CORE_PROBE.parse().unwrap(),
        backend: ProbeBackend::Cpu,
        duration_micros: 12,
        resources: ResourceObservations {
            operations: 2,
            nodes: 2,
            iterations: 1,
            bytes: 96,
        },
        seed: Some(11),
        project_hash: Some(snapshot.project_hash),
        catalog_hash: catalog.content_hash().to_owned(),
        input_hash: "saved-geometric-product-input".to_owned(),
        validated_assumptions: vec!["product_matches".to_owned()],
        refuted_assumptions: Vec::new(),
        warnings: Vec::new(),
        output: json!({"matches": true}),
    };
    let probe_file = NamedTempFile::new().unwrap();
    serde_json::to_writer(probe_file.as_file(), &vec![probe]).unwrap();

    let with_probe = recommend_json(
        fixture.path(),
        None,
        Some(goal_fixture()),
        Some(probe_file.path()),
    );
    let probe_score = score(recommendation(&with_probe), CORE_CAPABILITY);
    assert!(probe_score["components"]["verification"].as_f64().unwrap() > baseline_verification);
    assert!(probe_score["validated_assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|assumption| assumption == "product_matches"));
}

#[test]
fn current_and_stale_wasm_versions_are_typed_and_still_deterministic() {
    let catalog = Catalog::embedded().unwrap();
    let current_fixture = materialize_fixture(catalog.version());
    let stale_fixture = materialize_fixture("0.19.0");

    let current = recommend_json(current_fixture.path(), None, Some(goal_fixture()), None);
    let stale_first = recommend_json(stale_fixture.path(), None, Some(goal_fixture()), None);
    let stale_second = recommend_json(stale_fixture.path(), None, Some(goal_fixture()), None);

    assert_eq!(
        current["provenance"]["compatibility"]["status"],
        "applicable"
    );
    assert_eq!(
        stale_first["provenance"]["compatibility"]["status"],
        "unknown_version"
    );
    assert_eq!(
        recommendation(&current)["preferred"]["capability_id"],
        CORE_CAPABILITY
    );
    assert_eq!(
        recommendation(&stale_first)["preferred"]["capability_id"],
        CORE_CAPABILITY
    );
    assert!(recommendation(&stale_first)["preferred"]["steps"]
        .as_array()
        .unwrap()
        .iter()
        .any(|step| {
            step["kind"] == "dependency"
                && step["package"] == WASM_PACKAGE
                && step["version"] == catalog.version()
        }));
    assert_eq!(stale_first, stale_second);
}

#[test]
fn typescript_human_output_projects_wasm_plan_actions() {
    let version = Catalog::embedded().unwrap().version().to_owned();
    let fixture = materialize_fixture(&version);
    let output = Command::cargo_bin("amari")
        .unwrap()
        .arg("recommend")
        .arg(fixture.path())
        .arg("--goal-file")
        .arg(goal_fixture())
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .get_output()
        .stdout
        .clone();
    let human = String::from_utf8(output).unwrap();

    assert!(human.contains(CORE_CAPABILITY));
    assert!(human.contains(CORE_PROBE));
    assert!(human.contains("@justinelliottcobb/amari-wasm: npm package tests"));
}

#[test]
fn recommendation_is_read_only_for_typescript_projects() {
    let version = Catalog::embedded().unwrap().version().to_owned();
    let fixture = materialize_fixture(&version);
    let before = tree_contents(fixture.path());

    let _ = recommend_json(fixture.path(), None, Some(goal_fixture()), None);

    assert_eq!(tree_contents(fixture.path()), before);
}

#[test]
fn clap_rejects_goal_and_goal_file_together() {
    let version = Catalog::embedded().unwrap().version().to_owned();
    let fixture = materialize_fixture(&version);

    Command::cargo_bin("amari")
        .unwrap()
        .arg("recommend")
        .arg(fixture.path())
        .args(["--goal", GOAL, "--goal-file"])
        .arg(goal_fixture())
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn malformed_and_semantically_empty_goal_files_are_typed_errors() {
    let version = Catalog::embedded().unwrap().version().to_owned();
    let fixture = materialize_fixture(&version);
    let malformed = NamedTempFile::new().unwrap();
    fs::write(malformed.path(), b"not-json").unwrap();
    let empty = NamedTempFile::new().unwrap();
    fs::write(empty.path(), br#"{"statement":"  ","constraints":[]}"#).unwrap();

    Command::cargo_bin("amari")
        .unwrap()
        .arg("recommend")
        .arg(fixture.path())
        .arg("--goal-file")
        .arg(malformed.path())
        .arg("--json")
        .assert()
        .code(9)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("\"kind\":\"serialization\""));

    Command::cargo_bin("amari")
        .unwrap()
        .arg("recommend")
        .arg(fixture.path())
        .arg("--goal-file")
        .arg(empty.path())
        .arg("--json")
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("\"kind\":\"invalid_input\""));
}
