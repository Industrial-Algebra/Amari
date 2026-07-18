// SPDX-License-Identifier: MIT OR Apache-2.0

//! Cross-process replay of saved recommendation candidates.

use std::fs;
use std::path::{Path, PathBuf};

use amari_discovery::{
    inspect_rust_project, CandidatePlan, CapabilityId, Catalog, InspectionLimits,
    PlanCompatibility, PlanStep, ProbeBackend, ProbeResult, ResourceObservations,
};
use assert_cmd::Command;
use predicates::prelude::*;
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
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

fn recommendation_json(root: &Path, probe_results: Option<&Path>) -> Value {
    recommendation_json_for_goal(root, GOAL, probe_results)
}

fn recommendation_json_for_goal(root: &Path, goal: &str, probe_results: Option<&Path>) -> Value {
    let mut command = Command::cargo_bin("amari").unwrap();
    command.arg("recommend").arg(root).args(["--goal", goal]);
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

fn recompute_plan_hash(plan: &mut Value) {
    #[derive(Serialize)]
    struct PlanHashView<'a> {
        capability_id: &'a CapabilityId,
        prerequisite_order: &'a [CapabilityId],
        steps: &'a [PlanStep],
        compatibility: &'a PlanCompatibility,
    }

    let typed: CandidatePlan = serde_json::from_value(plan.clone()).unwrap();
    let bytes = serde_json::to_vec(&PlanHashView {
        capability_id: &typed.capability_id,
        prerequisite_order: &typed.prerequisite_order,
        steps: &typed.steps,
        compatibility: &typed.compatibility,
    })
    .unwrap();
    plan["plan_hash"] = json!(hex::encode(Sha256::digest(bytes)));
}

fn save_json(value: &Value) -> NamedTempFile {
    let file = NamedTempFile::new().unwrap();
    serde_json::to_writer(file.as_file(), value).unwrap();
    file
}

fn plan_output(
    root: &Path,
    recommendation: &Path,
    candidate_id: &str,
    json_output: bool,
) -> Vec<u8> {
    let mut command = Command::cargo_bin("amari").unwrap();
    command
        .arg("plan")
        .arg(candidate_id)
        .arg("--recommendation")
        .arg(recommendation)
        .arg("--project")
        .arg(root);
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

fn write_probe_results(root: &Path) -> NamedTempFile {
    let catalog = Catalog::embedded().unwrap();
    let snapshot = inspect_rust_project(root, &InspectionLimits::default()).unwrap();
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
    let file = NamedTempFile::new().unwrap();
    serde_json::to_writer(file.as_file(), &vec![probe]).unwrap();
    file
}

#[test]
fn saved_preferred_candidate_replays_with_plan_and_provenance_parity() {
    let fixture = materialize_fixture();
    let before = tree_contents(fixture.path());
    let recommendation = recommendation_json(fixture.path(), None);
    let artifact = save_json(&recommendation);

    let first_bytes = plan_output(fixture.path(), artifact.path(), DUAL_CAPABILITY, true);
    let first: Value = serde_json::from_slice(&first_bytes).unwrap();
    let second = plan_output(fixture.path(), artifact.path(), DUAL_CAPABILITY, true);

    assert_eq!(first["schema_version"], recommendation["schema_version"]);
    assert_eq!(first["provenance"], recommendation["provenance"]);
    assert_eq!(first["data"], recommendation["data"]["data"]["preferred"]);
    assert_eq!(first["data"]["normalization"]["normalized"], true);
    assert_eq!(first_bytes, second);
    assert_eq!(tree_contents(fixture.path()), before);
    assert!(!serde_json::to_string(&first)
        .unwrap()
        .contains(fixture.path().to_str().unwrap()));
}

#[test]
fn saved_alternative_candidate_replays_with_exact_plan_parity() {
    let fixture = materialize_fixture();
    let recommendation =
        recommendation_json_for_goal(fixture.path(), "compute geometric algebra products", None);
    let alternative = recommendation["data"]["data"]["alternatives"]
        .as_array()
        .and_then(|alternatives| alternatives.first())
        .expect("fixture goal must retain a Pareto alternative");
    let candidate_id = alternative["capability_id"].as_str().unwrap();
    let artifact = save_json(&recommendation);

    let replay: Value = serde_json::from_slice(&plan_output(
        fixture.path(),
        artifact.path(),
        candidate_id,
        true,
    ))
    .unwrap();

    assert_eq!(&replay["data"], alternative);
    assert_eq!(replay["provenance"], recommendation["provenance"]);
}

#[test]
fn self_rehashed_non_catalog_plan_steps_are_rejected() {
    let fixture = materialize_fixture();
    let mut recommendation = recommendation_json(fixture.path(), None);
    let selected = &mut recommendation["data"]["data"]["preferred"];
    let dependency = selected["steps"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|step| step["kind"] == "dependency")
        .unwrap();
    dependency["package"] = json!("amari-evil");
    dependency["version"] = json!("99.0.0");
    recompute_plan_hash(selected);
    let artifact = save_json(&recommendation);

    Command::cargo_bin("amari")
        .unwrap()
        .arg("plan")
        .arg(DUAL_CAPABILITY)
        .arg("--recommendation")
        .arg(artifact.path())
        .arg("--project")
        .arg(fixture.path())
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("plan_steps"));
}

#[test]
fn human_plan_output_projects_the_selected_normalized_plan() {
    let fixture = materialize_fixture();
    let recommendation = recommendation_json(fixture.path(), None);
    let artifact = save_json(&recommendation);
    let selected = &recommendation["data"]["data"]["preferred"];

    let human = String::from_utf8(plan_output(
        fixture.path(),
        artifact.path(),
        DUAL_CAPABILITY,
        false,
    ))
    .unwrap();

    assert!(human.contains(DUAL_CAPABILITY));
    assert!(human.contains(selected["plan_hash"].as_str().unwrap()));
    assert!(human.contains("amari-dual"));
    assert!(human.contains(DUAL_PROBE));
}

#[test]
fn unknown_candidate_is_rejected_without_project_mutation() {
    let fixture = materialize_fixture();
    let before = tree_contents(fixture.path());
    let artifact = save_json(&recommendation_json(fixture.path(), None));

    Command::cargo_bin("amari")
        .unwrap()
        .arg("plan")
        .arg("amari:unknown:module:symbol")
        .arg("--recommendation")
        .arg(artifact.path())
        .arg("--project")
        .arg(fixture.path())
        .arg("--json")
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("unknown candidate"));

    assert_eq!(tree_contents(fixture.path()), before);
}

#[test]
fn changed_project_is_rejected_as_project_hash_drift() {
    let fixture = materialize_fixture();
    let artifact = save_json(&recommendation_json(fixture.path(), None));
    fs::write(
        fixture.path().join("src/replay-drift.rs"),
        "pub fn changed_after_recommendation() {}\n",
    )
    .unwrap();
    let before_replay = tree_contents(fixture.path());

    Command::cargo_bin("amari")
        .unwrap()
        .arg("plan")
        .arg(DUAL_CAPABILITY)
        .arg("--recommendation")
        .arg(artifact.path())
        .arg("--project")
        .arg(fixture.path())
        .arg("--json")
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("project_hash"));

    assert_eq!(tree_contents(fixture.path()), before_replay);
}

#[test]
fn catalog_and_input_hash_drift_are_rejected() {
    let fixture = materialize_fixture();
    let recommendation = recommendation_json(fixture.path(), None);

    let mut catalog_drift = recommendation.clone();
    catalog_drift["provenance"]["catalog"]["hash"] = json!("0".repeat(64));
    let catalog_artifact = save_json(&catalog_drift);
    Command::cargo_bin("amari")
        .unwrap()
        .arg("plan")
        .arg(DUAL_CAPABILITY)
        .arg("--recommendation")
        .arg(catalog_artifact.path())
        .arg("--project")
        .arg(fixture.path())
        .assert()
        .code(2)
        .stderr(predicate::str::contains("catalog_hash"));

    let mut input_drift = recommendation;
    input_drift["data"]["data"]["goal"]["statement"] = json!("a different goal");
    let input_artifact = save_json(&input_drift);
    Command::cargo_bin("amari")
        .unwrap()
        .arg("plan")
        .arg(DUAL_CAPABILITY)
        .arg("--recommendation")
        .arg(input_artifact.path())
        .arg("--project")
        .arg(fixture.path())
        .assert()
        .code(2)
        .stderr(predicate::str::contains("input_hash"));
}

#[test]
fn saved_probe_hashes_replay_without_reembedding_probe_outputs() {
    let fixture = materialize_fixture();
    let probes = write_probe_results(fixture.path());
    let recommendation = recommendation_json(fixture.path(), Some(probes.path()));
    let artifact = save_json(&recommendation);

    let replay: Value = serde_json::from_slice(&plan_output(
        fixture.path(),
        artifact.path(),
        DUAL_CAPABILITY,
        true,
    ))
    .unwrap();

    assert_eq!(replay["data"], recommendation["data"]["data"]["preferred"]);
    assert!(replay["data"]["compatibility"]["probe_results"]
        .as_array()
        .is_some_and(|hashes| hashes.len() == 1));
    assert!(!serde_json::to_string(&replay).unwrap().contains("matches"));
}

#[test]
fn malformed_or_changed_probe_replay_hash_is_rejected() {
    let fixture = materialize_fixture();
    let probes = write_probe_results(fixture.path());
    let mut recommendation = recommendation_json(fixture.path(), Some(probes.path()));
    let mut malformed = recommendation.clone();
    malformed["data"]["data"]["preferred"]["compatibility"]["probe_results"][0]["result_hash"] =
        json!("not-a-sha256-hash");
    let artifact = save_json(&malformed);

    Command::cargo_bin("amari")
        .unwrap()
        .arg("plan")
        .arg(DUAL_CAPABILITY)
        .arg("--recommendation")
        .arg(artifact.path())
        .arg("--project")
        .arg(fixture.path())
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("probe_results"));

    recommendation["data"]["data"]["preferred"]["compatibility"]["probe_results"][0]
        ["result_hash"] = json!("0".repeat(64));
    let artifact = save_json(&recommendation);
    Command::cargo_bin("amari")
        .unwrap()
        .arg("plan")
        .arg(DUAL_CAPABILITY)
        .arg("--recommendation")
        .arg(artifact.path())
        .arg("--project")
        .arg(fixture.path())
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("input_hash"));
}

#[test]
fn replay_metadata_rejects_unknown_required_hashes() {
    let fixture = materialize_fixture();
    let mut recommendation = recommendation_json(fixture.path(), None);
    recommendation["provenance"]["replay"]["required_hashes"]
        .as_array_mut()
        .unwrap()
        .push(json!("future_unvalidated_hash"));
    let artifact = save_json(&recommendation);

    Command::cargo_bin("amari")
        .unwrap()
        .arg("plan")
        .arg(DUAL_CAPABILITY)
        .arg("--recommendation")
        .arg(artifact.path())
        .arg("--project")
        .arg(fixture.path())
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("must declare exactly"));
}

#[test]
fn changed_normalization_metadata_is_rejected() {
    let fixture = materialize_fixture();
    let mut recommendation = recommendation_json(fixture.path(), None);
    recommendation["data"]["data"]["preferred"]["normalization"]["max_rewrites"] = json!(1);
    let artifact = save_json(&recommendation);

    Command::cargo_bin("amari")
        .unwrap()
        .arg("plan")
        .arg(DUAL_CAPABILITY)
        .arg("--recommendation")
        .arg(artifact.path())
        .arg("--project")
        .arg(fixture.path())
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("normalization"));
}

#[test]
fn changed_plan_hash_is_rejected() {
    let fixture = materialize_fixture();
    let mut recommendation = recommendation_json(fixture.path(), None);
    recommendation["data"]["data"]["preferred"]["plan_hash"] = json!("0".repeat(64));
    let artifact = save_json(&recommendation);

    Command::cargo_bin("amari")
        .unwrap()
        .arg("plan")
        .arg(DUAL_CAPABILITY)
        .arg("--recommendation")
        .arg(artifact.path())
        .arg("--project")
        .arg(fixture.path())
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("plan_hash"));
}

#[cfg(unix)]
#[test]
fn symlinked_recommendation_artifact_is_rejected() {
    use std::os::unix::fs::symlink;

    let fixture = materialize_fixture();
    let artifact = save_json(&recommendation_json(fixture.path(), None));
    let link_dir = TempDir::new().unwrap();
    let link = link_dir.path().join("recommendation.json");
    symlink(artifact.path(), &link).unwrap();

    Command::cargo_bin("amari")
        .unwrap()
        .arg("plan")
        .arg(DUAL_CAPABILITY)
        .arg("--recommendation")
        .arg(link)
        .arg("--project")
        .arg(fixture.path())
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::starts_with("invalid_input:"));
}
