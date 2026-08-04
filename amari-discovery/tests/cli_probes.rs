// SPDX-License-Identifier: MIT OR Apache-2.0

//! Probe CLI discovery, dry-run, and isolated execution contract.

use std::fs;

#[cfg(feature = "standard-probes")]
use amari_discovery::ProbeEngine;
use amari_discovery::{
    CandidatePlan, Catalog, CatalogIdentity, Compatibility, Envelope, PlanCompatibility,
    PlanNormalization, PlanStep, ReplayMetadata,
};
#[cfg(feature = "standard-probes")]
use amari_discovery::{
    PolynomialDerivativeOutput, PolynomialDerivativeRequest, ProbeSchemaDocument,
};
use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::{json, Value};
use tempfile::TempDir;

const VITERBI: &str = "amari-probe:tropical:viterbi:v1";
const DUAL: &str = "amari-probe:dual:polynomial-derivative:v1";

fn command_json(arguments: &[&str]) -> Value {
    let output = Command::cargo_bin("amari")
        .unwrap()
        .args(arguments)
        .arg("--json")
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).unwrap()
}

fn viterbi_input() -> Value {
    json!({
        "transitions": [[-1.0, -2.0], [-2.0, -1.0]],
        "emissions": [[-1.0, -3.0], [-3.0, -1.0]],
        "observations": [0, 1, 0]
    })
}

fn write_json_file(temporary: &TempDir, name: &str, value: &Value) -> std::path::PathBuf {
    let path = temporary.path().join(name);
    fs::write(&path, serde_json::to_vec(value).unwrap()).unwrap();
    path
}

fn plan_envelope(probe_id: &str) -> Envelope<CandidatePlan> {
    let catalog = Catalog::embedded().unwrap();
    let descriptor = catalog
        .probes()
        .iter()
        .find(|probe| probe.id.to_string() == probe_id)
        .unwrap();
    let identity = CatalogIdentity {
        version: catalog.version().to_owned(),
        hash: catalog.content_hash().to_owned(),
    };
    Envelope::new(
        CandidatePlan {
            capability_id: descriptor.capability_id.clone(),
            prerequisite_order: vec![descriptor.capability_id.clone()],
            steps: vec![PlanStep::Probe {
                capability_id: descriptor.capability_id.clone(),
                probe_id: descriptor.id.clone(),
            }],
            compatibility: PlanCompatibility {
                catalog: identity.clone(),
                project_hash: "fixture-project-hash".to_owned(),
                input_hash: "fixture-plan-input-hash".to_owned(),
                probe_results: Vec::new(),
            },
            normalization: PlanNormalization {
                normalized: true,
                max_rewrites: 1,
                trace: Vec::new(),
            },
            plan_hash: "0".repeat(64),
        },
        identity,
        Compatibility {
            status: "compatible".to_owned(),
            reasons: Vec::new(),
        },
        ReplayMetadata {
            replayable: true,
            required_hashes: vec!["catalog_hash".to_owned(), "project_hash".to_owned()],
            reasons: Vec::new(),
        },
    )
}

#[cfg(feature = "standard-probes")]
#[test]
fn schema_returns_complete_input_and_output_documents() {
    let input_document =
        ProbeSchemaDocument::from_contract::<PolynomialDerivativeRequest>().unwrap();
    let output_document =
        ProbeSchemaDocument::from_contract::<PolynomialDerivativeOutput>().unwrap();

    let input = command_json(&["probe", "schema", DUAL, "--direction", "input"]);
    let output = command_json(&["probe", "schema", DUAL, "--direction", "output"]);

    assert_eq!(
        input["data"]["document"],
        input_document.exported_value().unwrap()
    );
    assert_eq!(
        output["data"]["document"],
        output_document.exported_value().unwrap()
    );
    assert_eq!(
        input["data"]["document"]["$id"],
        "amari.discovery/probe/dual-polynomial-derivative/input/v1"
    );
    assert_eq!(
        output["data"]["document"]["$id"],
        "amari.discovery/probe/dual-polynomial-derivative/output/v1"
    );
    assert_eq!(
        input["data"]["hash"],
        input_document.canonical_hash().unwrap()
    );
    assert_eq!(
        output["data"]["hash"],
        output_document.canonical_hash().unwrap()
    );
    assert_eq!(input["data"]["hash"].as_str().unwrap().len(), 64);
    assert_eq!(
        input["data"]["hash"],
        command_json(&["probe", "schema", DUAL, "--direction", "input"])["data"]["hash"]
    );
}

#[cfg(feature = "standard-probes")]
#[test]
fn schema_rejects_invalid_direction_and_unknown_probe_as_typed_errors() {
    let invalid_direction = Command::cargo_bin("amari")
        .unwrap()
        .args(["probe", "schema", DUAL, "--direction", "sideways", "--json"])
        .assert()
        .failure()
        .code(2)
        .stdout(predicate::str::is_empty())
        .get_output()
        .stderr
        .clone();
    let invalid_direction: Value = serde_json::from_slice(&invalid_direction).unwrap();
    assert_eq!(invalid_direction["kind"], "invalid_input");
    assert!(invalid_direction["message"]
        .as_str()
        .unwrap()
        .contains("direction"));

    let unknown_probe = Command::cargo_bin("amari")
        .unwrap()
        .args([
            "probe",
            "schema",
            "amari-probe:dual:does-not-exist:v1",
            "--direction",
            "input",
            "--json",
        ])
        .assert()
        .failure()
        .code(2)
        .stdout(predicate::str::is_empty())
        .get_output()
        .stderr
        .clone();
    let unknown_probe: Value = serde_json::from_slice(&unknown_probe).unwrap();
    assert_eq!(unknown_probe["kind"], "invalid_input");
    assert!(unknown_probe["message"]
        .as_str()
        .unwrap()
        .contains("unknown probe"));
}

#[test]
fn readme_schema_docs() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme = fs::read_to_string(manifest_dir.join("README.md")).unwrap();
    let guide = fs::read_to_string(manifest_dir.join("../docs/guide/amari-discovery.md")).unwrap();

    for document in [&readme, &guide] {
        assert!(document.contains(
            "amari probe schema amari-probe:dual:polynomial-derivative:v1 --direction input --json"
        ));
        assert!(document.contains("schema_hashes"));
        assert!(document.contains("Structural JSON Schema"));
        assert!(document.contains("semantic Rust validation"));
        assert!(document.contains("amari.discovery/v1"));
    }

    assert!(readme.contains("amari.discovery/probe/dual-polynomial-derivative/input/v1"));
    assert!(readme.contains("x-amari-semantic-constraints"));
    assert!(readme.contains("canonical SHA-256 hash"));
    assert!(guide.contains(
        "required field, field type, unknown-field, semantic constraint, or output meaning"
    ));
    assert!(guide.contains("v1") && guide.contains("v2"));
    assert!(guide.contains("additive optional metadata"));
}

#[test]
fn list_reports_every_catalog_probe_with_dynamic_execution_state() {
    let listed = command_json(&["probe", "list"]);
    let capabilities = command_json(&["capabilities"]);
    let probes = listed["data"]["probes"].as_array().unwrap();
    let states = capabilities["data"]["known_probes"].as_array().unwrap();
    let catalog = Catalog::embedded().unwrap();

    assert_eq!(probes.len(), catalog.probes().len());
    assert_eq!(probes.len(), states.len());
    for (probe, state) in probes.iter().zip(states) {
        assert_eq!(probe["id"], state["id"]);
        assert_eq!(probe["known"], true);
        assert_eq!(probe["available"], state["available"]);
        assert_eq!(probe["executable"], state["executable"]);
        assert!(probe.get("descriptor").is_none());
        assert!(probe.get("schema_hashes").is_none());
    }
}

#[test]
fn describe_returns_the_catalog_contract_and_process_guarantees() {
    let value = command_json(&["probe", "describe", VITERBI]);

    assert_eq!(value["data"]["descriptor"]["id"], VITERBI);
    assert_eq!(
        value["data"]["descriptor"]["input_schema"],
        "amari.discovery/probe/tropical-viterbi/input/v1"
    );
    assert_eq!(value["data"]["known"], true);
    assert_eq!(
        value["data"]["executable"],
        cfg!(feature = "standard-probes")
    );
    assert_eq!(value["data"]["isolation"], "process");
    assert_eq!(value["data"]["hard_timeout"], true);
    assert_eq!(value["data"]["crash_isolation"], true);

    #[cfg(feature = "standard-probes")]
    {
        let input_document =
            ProbeSchemaDocument::from_contract::<amari_discovery::TropicalViterbiRequest>()
                .unwrap();
        let output_document =
            ProbeSchemaDocument::from_contract::<amari_discovery::TropicalViterbiOutput>().unwrap();
        let hashes = &value["data"]["schema_hashes"];
        assert_eq!(hashes["probe_id"], VITERBI);
        assert_eq!(hashes["state"], "resolved");
        assert_eq!(
            hashes["input_summary"],
            serde_json::to_value(input_document.summary().unwrap()).unwrap()
        );
        assert_eq!(
            hashes["output_summary"],
            serde_json::to_value(output_document.summary().unwrap()).unwrap()
        );
    }

    #[cfg(not(feature = "standard-probes"))]
    {
        let hashes = &value["data"]["schema_hashes"];
        assert_eq!(hashes["probe_id"], VITERBI);
        assert_eq!(hashes["state"], "declared");
        assert!(hashes["input_summary"].is_null());
        assert!(hashes["output_summary"].is_null());
    }
}

#[test]
fn list_and_describe_human_output_are_concise() {
    Command::cargo_bin("amari")
        .unwrap()
        .args(["probe", "list"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains("Registered probes"))
        .stdout(predicate::str::contains(VITERBI));

    Command::cargo_bin("amari")
        .unwrap()
        .args(["probe", "describe", VITERBI])
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains("Process isolation"))
        .stdout(predicate::str::contains("Hard timeout: yes"))
        .stdout(predicate::str::contains("Input schema hash:"))
        .stdout(predicate::str::contains("Output schema hash:"))
        .stdout(predicate::str::contains(
            "amari probe schema amari-probe:tropical:viterbi:v1 --direction input",
        ));
}

#[cfg(feature = "standard-probes")]
#[test]
fn explicit_input_routes_through_process_supervisor_with_math_parity() {
    let temporary = tempfile::tempdir().unwrap();
    let input = viterbi_input();
    let input_path = write_json_file(&temporary, "input.json", &input);
    let path = input_path.to_str().unwrap();
    let value = command_json(&["probe", "run", VITERBI, "--input", path]);
    let direct = ProbeEngine::new()
        .unwrap()
        .execute(&VITERBI.parse().unwrap(), &input)
        .unwrap();

    assert_eq!(value["data"]["result"]["output"], direct.output);
    assert_eq!(
        value["data"]["result"]["resources"],
        serde_json::to_value(direct.resources).unwrap()
    );
    assert_eq!(value["data"]["result"]["backend"], "cpu");
    assert_eq!(value["data"]["isolation"], "process");
    assert_eq!(value["data"]["hard_timeout"], true);
    assert_eq!(value["data"]["crash_isolation"], true);
    assert_eq!(value["data"]["timeout_millis"], 5_000);
    assert_eq!(
        value["provenance"]["catalog"]["hash"],
        value["data"]["result"]["catalog_hash"]
    );
    assert_eq!(
        value["provenance"]["input_hash"],
        value["data"]["result"]["input_hash"]
    );
    assert_eq!(
        value["data"]["result"]["input_hash"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
    assert_eq!(
        direct.isolation,
        amari_discovery::ProbeIsolation::Cooperative
    );
}

#[cfg(feature = "standard-probes")]
#[test]
fn explicit_input_human_output_reports_process_guarantees() {
    let temporary = tempfile::tempdir().unwrap();
    let input_path = write_json_file(&temporary, "input.json", &viterbi_input());

    Command::cargo_bin("amari")
        .unwrap()
        .args([
            "probe",
            "run",
            VITERBI,
            "--input",
            input_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains("Isolation: process"))
        .stdout(predicate::str::contains("Hard timeout: yes"));
}

#[test]
fn plan_dry_run_is_compatibility_only_and_never_requires_probe_input() {
    let temporary = tempfile::tempdir().unwrap();
    let plan = serde_json::to_value(plan_envelope(VITERBI)).unwrap();
    let plan_path = write_json_file(&temporary, "plan.json", &plan);
    let value = command_json(&[
        "probe",
        "run",
        VITERBI,
        "--plan",
        plan_path.to_str().unwrap(),
        "--dry-run",
    ]);

    assert_eq!(value["data"]["probe_id"], VITERBI);
    assert_eq!(value["data"]["compatible"], true);
    assert_eq!(value["data"]["would_execute"], false);
    assert_eq!(value["data"]["plan_hash"], "0".repeat(64));
    assert_eq!(value["data"]["planned_isolation"], "process");
    assert!(value["data"].get("output").is_none());
}

#[test]
fn plan_without_dry_run_is_rejected_with_explicit_input_guidance() {
    let temporary = tempfile::tempdir().unwrap();
    let plan_path = write_json_file(
        &temporary,
        "plan.json",
        &serde_json::to_value(plan_envelope(VITERBI)).unwrap(),
    );

    Command::cargo_bin("amari")
        .unwrap()
        .args([
            "probe",
            "run",
            VITERBI,
            "--plan",
            plan_path.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("provide explicit typed input"));
}

#[test]
fn json_probe_errors_are_structured_on_stderr() {
    let output = Command::cargo_bin("amari")
        .unwrap()
        .args([
            "probe",
            "describe",
            "amari-probe:tropical:does-not-exist:v1",
            "--json",
        ])
        .assert()
        .failure()
        .code(2)
        .stdout(predicate::str::is_empty())
        .get_output()
        .stderr
        .clone();
    let error: Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(error["kind"], "invalid_input");
    assert!(error["message"].as_str().unwrap().contains("unknown probe"));
    assert_eq!(error["details"]["exit_code"], 2);
}

#[test]
fn dry_run_requires_plan_and_unknown_probe_is_typed() {
    let temporary = tempfile::tempdir().unwrap();
    let input_path = write_json_file(&temporary, "input.json", &viterbi_input());
    Command::cargo_bin("amari")
        .unwrap()
        .args([
            "probe",
            "run",
            VITERBI,
            "--input",
            input_path.to_str().unwrap(),
            "--dry-run",
        ])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("dry-run requires --plan"));

    Command::cargo_bin("amari")
        .unwrap()
        .args(["probe", "describe", "amari-probe:tropical:unknown:v1"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("unknown probe"));
}
