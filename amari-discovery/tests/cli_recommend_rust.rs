// SPDX-License-Identifier: MIT OR Apache-2.0

//! End-to-end Rust recommendation through the installed `amari` binary.

use std::fs;
use std::path::{Path, PathBuf};

use amari_discovery::{
    inspect_rust_project, Catalog, GoalSpec, InspectionLimits, PlanCompatibility, PlanningContext,
    ProbeBackend, ProbeResult, RecallConfig, ResourceObservations,
};
use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::{json, Value};
use tempfile::{NamedTempFile, TempDir};
use walkdir::WalkDir;

const GOAL: &str = "differentiate a scalar polynomial with forward dual numbers";
const DUAL_CAPABILITY: &str = "amari:amari-dual:autodiff:forward-derivative";
const DUAL_PROBE: &str = "amari-probe:dual:polynomial-derivative:v1";

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

fn copy_and_transform(src: &Path, dst: &Path, version: &str) {
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        let text = name.to_string_lossy();
        let source = entry.path();
        if source.is_dir() {
            let target = dst.join(&name);
            fs::create_dir_all(&target).unwrap();
            copy_and_transform(&source, &target, version);
        } else if text.ends_with(".in") {
            let target = dst.join(text.trim_end_matches(".in"));
            let contents = fs::read_to_string(source).unwrap();
            fs::write(target, contents.replace("__AMARI_VERSION__", version)).unwrap();
        } else if (text == "Cargo.toml" || text == "Cargo.lock")
            && src.join(format!("{text}.in")).exists()
        {
            continue;
        } else {
            fs::copy(source, dst.join(name)).unwrap();
        }
    }
}

fn tree_contents(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut entries = WalkDir::new(root)
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
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    entries
}

fn recommend_output(root: &Path, probe_results: Option<&Path>, json_output: bool) -> Vec<u8> {
    let mut command = Command::cargo_bin("amari").unwrap();
    command.arg("recommend").arg(root).args(["--goal", GOAL]);
    if let Some(path) = probe_results {
        command.arg("--probe-results").arg(path);
    }
    if json_output {
        command.arg("--json");
    }
    command
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .get_output()
        .stdout
        .clone()
}

fn recommend_json(root: &Path, probe_results: Option<&Path>) -> Value {
    serde_json::from_slice(&recommend_output(root, probe_results, true)).unwrap()
}

fn recommendation(value: &Value) -> &Value {
    assert_eq!(value["data"]["status"], "recommended");
    &value["data"]["data"]
}

fn score<'a>(recommendation: &'a Value, capability_id: &str) -> &'a Value {
    recommendation["scores"]
        .as_array()
        .unwrap()
        .iter()
        .find(|score| score["capability_id"] == capability_id)
        .unwrap()
}

#[test]
fn rust_recommendation_contains_preferred_alternatives_scores_and_actions() {
    let fixture = materialize_fixture();
    let value = recommend_json(fixture.path(), None);
    let recommendation = recommendation(&value);
    let preferred = &recommendation["preferred"];

    assert_eq!(value["schema_version"], "amari.discovery/v1");
    assert_eq!(preferred["capability_id"], DUAL_CAPABILITY);
    assert_eq!(preferred["normalization"]["normalized"], true);
    assert_eq!(preferred["plan_hash"].as_str().unwrap().len(), 64);
    assert!(recommendation["alternatives"].is_array());

    let preferred_score = score(recommendation, DUAL_CAPABILITY);
    assert!(preferred_score["confidence"].as_f64().is_some());
    assert!(preferred_score["components"]["applicability"]
        .as_f64()
        .is_some());
    assert_eq!(preferred_score["objectives"].as_array().unwrap().len(), 8);
    assert!(preferred_score["evidence"]
        .as_array()
        .is_some_and(|evidence| !evidence.is_empty()));

    assert!(recommendation["missing_information"]
        .as_array()
        .is_some_and(|missing| !missing.is_empty()));
    assert!(recommendation["suggested_probes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|probe| probe == DUAL_PROBE));
    assert!(recommendation["suggested_tests"]
        .as_array()
        .unwrap()
        .iter()
        .any(|step| step["kind"] == "test" && step["package"] == "amari-dual"));

    assert_eq!(
        value["provenance"]["project_hash"],
        preferred["compatibility"]["project_hash"]
    );
    assert_eq!(
        value["provenance"]["input_hash"],
        preferred["compatibility"]["input_hash"]
    );
    assert_eq!(value["provenance"]["replay"]["replayable"], true);
    assert_eq!(
        value["provenance"]["seed"],
        json!(RecallConfig::default().seed)
    );
    let snapshot = inspect_rust_project(fixture.path(), &InspectionLimits::default()).unwrap();
    let expected_compatibility = PlanCompatibility::from_context(
        &Catalog::embedded().unwrap(),
        &PlanningContext {
            snapshot,
            goal: GoalSpec {
                statement: GOAL.to_owned(),
                constraints: Vec::new(),
            },
            probe_results: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(
        value["provenance"]["input_hash"],
        expected_compatibility.input_hash
    );
    assert!(!serde_json::to_string(&value)
        .unwrap()
        .contains(fixture.path().to_str().unwrap()));
}

#[test]
fn matching_saved_probe_improves_the_same_candidate_score() {
    let fixture = materialize_fixture();
    let baseline = recommend_json(fixture.path(), None);
    let baseline_recommendation = recommendation(&baseline);
    let baseline_verification = score(baseline_recommendation, DUAL_CAPABILITY)["components"]
        ["verification"]
        .as_f64()
        .unwrap();

    let catalog = Catalog::embedded().unwrap();
    let snapshot = inspect_rust_project(fixture.path(), &InspectionLimits::default()).unwrap();
    let probe = ProbeResult {
        probe_id: DUAL_PROBE.parse().unwrap(),
        backend: ProbeBackend::Cpu,
        duration_micros: 10,
        resources: ResourceObservations {
            operations: 3,
            nodes: 0,
            iterations: 1,
            bytes: 64,
        },
        seed: Some(7),
        project_hash: Some(snapshot.project_hash),
        catalog_hash: catalog.content_hash().to_owned(),
        input_hash: "saved-dual-input".to_owned(),
        validated_assumptions: vec!["derivative_matches".to_owned()],
        refuted_assumptions: Vec::new(),
        warnings: Vec::new(),
        output: json!({"matches": true}),
    };
    let probe_file = NamedTempFile::new().unwrap();
    serde_json::to_writer(probe_file.as_file(), &vec![probe]).unwrap();

    let with_probe = recommend_json(fixture.path(), Some(probe_file.path()));
    let with_probe_recommendation = recommendation(&with_probe);
    let probe_score = score(with_probe_recommendation, DUAL_CAPABILITY);

    assert!(probe_score["components"]["verification"].as_f64().unwrap() > baseline_verification);
    assert!(probe_score["validated_assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|assumption| assumption == "derivative_matches"));
    assert!(!with_probe_recommendation["missing_information"]
        .as_array()
        .unwrap()
        .iter()
        .any(|missing| missing
            .as_str()
            .is_some_and(|text| text.contains(DUAL_PROBE))));
}

#[test]
fn fixed_seed_recommendation_is_byte_deterministic_and_read_only() {
    let fixture = materialize_fixture();
    let before = tree_contents(fixture.path());

    let first = recommend_output(fixture.path(), None, true);
    let second = recommend_output(fixture.path(), None, true);

    assert_eq!(first, second);
    assert_eq!(tree_contents(fixture.path()), before);
}

#[test]
fn human_output_is_a_projection_of_the_json_recommendation() {
    let fixture = materialize_fixture();
    let value = recommend_json(fixture.path(), None);
    let recommendation = recommendation(&value);
    let human = String::from_utf8(recommend_output(fixture.path(), None, false)).unwrap();

    assert!(human.contains(GOAL));
    assert!(human.contains(
        recommendation["preferred"]["capability_id"]
            .as_str()
            .unwrap()
    ));
    assert!(human.contains(recommendation["preferred"]["plan_hash"].as_str().unwrap()));
    assert!(human.contains(DUAL_PROBE));
    assert!(human.contains("amari-dual: all targets"));
    for evidence in recommendation["evidence"].as_array().unwrap() {
        assert!(human.contains(evidence["summary"].as_str().unwrap()));
    }
}

#[cfg(unix)]
#[test]
fn symlinked_probe_result_file_is_rejected_without_project_mutation() {
    use std::os::unix::fs::symlink;

    let fixture = materialize_fixture();
    let before = tree_contents(fixture.path());
    let probe_file = NamedTempFile::new().unwrap();
    serde_json::to_writer(probe_file.as_file(), &Vec::<ProbeResult>::new()).unwrap();
    let link_dir = TempDir::new().unwrap();
    let link = link_dir.path().join("probe-results.json");
    symlink(probe_file.path(), &link).unwrap();

    Command::cargo_bin("amari")
        .unwrap()
        .arg("recommend")
        .arg(fixture.path())
        .args(["--goal", GOAL, "--probe-results"])
        .arg(link)
        .arg("--json")
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::starts_with("invalid_input:"));

    assert_eq!(tree_contents(fixture.path()), before);
}

#[test]
fn malformed_probe_result_file_is_rejected_without_project_mutation() {
    let fixture = materialize_fixture();
    let before = tree_contents(fixture.path());
    let probe_file = NamedTempFile::new().unwrap();
    fs::write(probe_file.path(), b"not-json").unwrap();

    Command::cargo_bin("amari")
        .unwrap()
        .arg("recommend")
        .arg(fixture.path())
        .args(["--goal", GOAL, "--probe-results"])
        .arg(probe_file.path())
        .arg("--json")
        .assert()
        .code(9)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::starts_with("serialization:"));

    assert_eq!(tree_contents(fixture.path()), before);
}
