// SPDX-License-Identifier: MIT OR Apache-2.0

use std::str::FromStr;

use amari_discovery::{
    CapabilityId, CatalogIdentity, Compatibility, DiscoveryError, DiscoveryOutcome, Envelope,
    Evidence, ProbeBackend, ProbeId, ProbeResult, ReplayMetadata, ResourceObservations,
    SchemaVersion,
};
use serde_json::{json, Value};

fn catalog() -> CatalogIdentity {
    CatalogIdentity {
        version: "0.23.0".into(),
        hash: "fixture-catalog-hash".into(),
    }
}

fn compatible() -> Compatibility {
    Compatibility {
        status: "compatible".into(),
        reasons: vec![],
    }
}

fn non_replayable() -> ReplayMetadata {
    ReplayMetadata {
        replayable: false,
        required_hashes: vec![],
        reasons: vec!["fixture response".into()],
    }
}

fn sample_probe_result() -> ProbeResult {
    ProbeResult {
        probe_id: ProbeId::from_str("amari-probe:tropical:shortest-path:v1").unwrap(),
        backend: ProbeBackend::Cpu,
        duration_micros: 42,
        resources: ResourceObservations {
            operations: 5,
            nodes: 3,
            iterations: 2,
            bytes: 128,
        },
        seed: None,
        project_hash: None,
        catalog_hash: "fixture-catalog-hash".into(),
        input_hash: "fixture-input-hash".into(),
        validated_assumptions: vec!["weights are finite".into()],
        refuted_assumptions: vec![],
        warnings: vec![],
        output: json!({"distance": 3.0}),
    }
}

#[test]
fn capability_and_probe_ids_use_stable_namespaces() {
    assert!(CapabilityId::from_str("amari:amari-tropical:paths:shortest-path").is_ok());
    assert!(CapabilityId::from_str("amari:crate:module:symbol:method").is_ok());
    assert!(ProbeId::from_str("amari-probe:tropical:shortest-path:v1").is_ok());

    for invalid in [
        "shortest-path",
        "other:amari-tropical:paths:shortest-path",
        "amari:too-short",
        "amari:crate::symbol",
        "amari:crate:bad segment:symbol",
        "amari:crate:Module:symbol",
        "amari:crate:-module:symbol",
    ] {
        assert!(
            CapabilityId::from_str(invalid).is_err(),
            "accepted {invalid}"
        );
    }

    for invalid in [
        "shortest-path",
        "amari-probe:tropical:shortest-path",
        "amari-probe::shortest-path:v1",
        "amari-probe:tropical:shortest-path:1",
        "amari-probe:tropical:shortest-path:vx",
        "amari-probe:tropical:shortest-path:v0",
        "amari-probe:tropical:shortest-path:v+1",
        "amari-probe:tropical:shortest-path:v01",
        "amari-probe:tropical:bad operation:v1",
    ] {
        assert!(ProbeId::from_str(invalid).is_err(), "accepted {invalid}");
    }
}

#[test]
fn identifiers_round_trip_through_display_and_json() {
    let capability = CapabilityId::from_str("amari:amari-tropical:paths:shortest-path").unwrap();
    let probe = ProbeId::from_str("amari-probe:tropical:shortest-path:v1").unwrap();

    assert_eq!(
        capability.to_string(),
        "amari:amari-tropical:paths:shortest-path"
    );
    assert_eq!(probe.to_string(), "amari-probe:tropical:shortest-path:v1");
    assert_eq!(
        serde_json::to_value(capability).unwrap(),
        json!("amari:amari-tropical:paths:shortest-path")
    );
    assert_eq!(
        serde_json::to_value(probe).unwrap(),
        json!("amari-probe:tropical:shortest-path:v1")
    );
}

#[test]
fn envelope_serializes_schema_and_provenance() {
    let envelope = Envelope::new(
        json!({"ok": true}),
        catalog(),
        compatible(),
        non_replayable(),
    );
    let json = serde_json::to_value(envelope).unwrap();

    assert_eq!(json["schema_version"], SchemaVersion::V1.as_str());
    assert!(json["provenance"]["tool_version"].is_string());
    assert_eq!(json["provenance"]["catalog"]["version"], "0.23.0");
    assert_eq!(
        json["provenance"]["catalog"]["hash"],
        "fixture-catalog-hash"
    );
    assert_eq!(json["provenance"]["compatibility"]["status"], "compatible");
    assert!(json["provenance"]["compatibility"]["reasons"].is_array());
    assert!(json["provenance"]["replay"]["replayable"].is_boolean());
    assert!(json["provenance"]["replay"]["required_hashes"].is_array());
    assert!(json["provenance"]["replay"]["reasons"].is_array());
    for key in ["project_hash", "input_hash", "seed"] {
        assert!(json["provenance"].get(key).is_some(), "missing {key}");
    }
    assert!(json["warnings"].is_array());
    assert_eq!(json["data"]["ok"], true);
}

#[test]
fn replayable_and_non_replayable_envelopes_always_include_contract_fields() {
    let cases = [
        non_replayable(),
        ReplayMetadata {
            replayable: true,
            required_hashes: vec!["catalog_hash".into(), "input_hash".into()],
            reasons: vec![],
        },
    ];

    for replay in cases {
        let expected = replay.replayable;
        let value =
            serde_json::to_value(Envelope::new(json!(null), catalog(), compatible(), replay))
                .unwrap();
        assert_eq!(value["schema_version"], "amari.discovery/v1");
        assert!(value["provenance"]["catalog"]["version"].is_string());
        assert!(value["provenance"]["catalog"]["hash"].is_string());
        assert!(value["provenance"]["compatibility"]["status"].is_string());
        assert_eq!(value["provenance"]["replay"]["replayable"], expected);
        assert!(value["provenance"]["replay"]["required_hashes"].is_array());
        assert!(value["provenance"]["replay"]["reasons"].is_array());
        assert!(value["provenance"]["compatibility"]["reasons"].is_array());
    }
}

#[test]
fn probe_results_report_backend_resources_hashes_and_assumptions() {
    let json = serde_json::to_value(sample_probe_result()).unwrap();
    for key in [
        "probe_id",
        "backend",
        "duration_micros",
        "resources",
        "catalog_hash",
        "input_hash",
        "validated_assumptions",
        "refuted_assumptions",
        "output",
    ] {
        assert!(!json[key].is_null(), "missing {key}");
    }
    assert!(json.get("seed").is_some());
    assert!(json.get("project_hash").is_some());
    assert!(json["seed"].is_null());
    assert!(json["project_hash"].is_null());
    assert_eq!(json["backend"], "cpu");
    assert_eq!(json["resources"]["operations"], 5);
}

#[test]
fn project_seeded_probe_results_require_concrete_provenance() {
    let mut result = sample_probe_result();
    result.seed = Some(7);
    result.project_hash = Some("fixture-project-hash".into());

    let json = serde_json::to_value(result).unwrap();
    assert_eq!(json["seed"], 7);
    assert_eq!(json["project_hash"], "fixture-project-hash");
    assert!(json["catalog_hash"].is_string());
    assert!(json["input_hash"].is_string());
}

#[test]
fn discovery_errors_have_stable_kinds_and_exit_codes() {
    let json_error = serde_json::from_str::<Value>("{").unwrap_err();
    let cases = [
        (
            DiscoveryError::invalid_id("bad", "wrong namespace"),
            "invalid_id",
            2,
        ),
        (
            DiscoveryError::InvalidInput("bad input".into()),
            "invalid_input",
            2,
        ),
        (
            DiscoveryError::CatalogCorruption("bad catalog".into()),
            "catalog_corruption",
            3,
        ),
        (
            DiscoveryError::InspectionFailure("inspection failed".into()),
            "inspection_failure",
            4,
        ),
        (
            DiscoveryError::ProbeUnavailable("probe absent".into()),
            "probe_unavailable",
            5,
        ),
        (
            DiscoveryError::ProbeFailed("probe failed".into()),
            "probe_failed",
            6,
        ),
        (
            DiscoveryError::LimitExceeded("too much work".into()),
            "limit_exceeded",
            7,
        ),
        (
            DiscoveryError::Io(std::io::Error::other("io failed")),
            "io",
            8,
        ),
        (
            DiscoveryError::Serialization(json_error),
            "serialization",
            9,
        ),
        (
            DiscoveryError::Internal("invariant failed".into()),
            "internal",
            70,
        ),
    ];

    for (error, kind, exit_code) in cases {
        assert_eq!(error.kind(), kind);
        assert_eq!(error.exit_code(), exit_code);
        assert!(!error.to_string().is_empty());
    }
}

#[test]
fn domain_outcomes_are_successful_typed_responses() {
    let evidence = Evidence {
        kind: "manifest".into(),
        summary: "No Amari dependency is declared".into(),
        source: Some("Cargo.toml".into()),
        weight: 1.0,
    };
    let cases = [
        (
            DiscoveryOutcome::Recommended(json!({"capability_id": "amari:test:a:b"})),
            "recommended",
        ),
        (
            DiscoveryOutcome::<Value>::NoApplicableCapability {
                evidence: vec![evidence],
            },
            "no_applicable_capability",
        ),
        (
            DiscoveryOutcome::<Value>::InsufficientEvidence {
                missing: vec!["project language".into()],
            },
            "insufficient_evidence",
        ),
        (
            DiscoveryOutcome::<Value>::Blocked {
                reasons: vec!["incompatible target".into()],
            },
            "blocked",
        ),
    ];

    for (outcome, expected_status) in cases {
        let value = serde_json::to_value(outcome).unwrap();
        assert_eq!(value["status"], expected_status);
        assert!(!value["data"].is_null());
        match expected_status {
            "recommended" => assert!(value["data"]["capability_id"].is_string()),
            "no_applicable_capability" => assert!(value["data"]["evidence"].is_array()),
            "insufficient_evidence" => assert!(value["data"]["missing"].is_array()),
            "blocked" => assert!(value["data"]["reasons"].is_array()),
            _ => unreachable!(),
        }
    }
}

#[test]
fn deserialization_rejects_non_canonical_identifiers() {
    assert!(serde_json::from_str::<CapabilityId>(r#""amari:crate:bad segment:symbol""#).is_err());
    assert!(serde_json::from_str::<ProbeId>(r#""amari-probe:tropical:path:v+1""#).is_err());
}
