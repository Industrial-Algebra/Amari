// SPDX-License-Identifier: MIT OR Apache-2.0

//! Golden contract for the five curated v1 machine schemas.

use std::{collections::BTreeMap, fs, path::Path};

use amari_discovery::{protocol_schema, SchemaKind, SCHEMA_V1};
use serde_json::Value;

fn expected() -> BTreeMap<SchemaKind, (&'static str, &'static [&'static str])> {
    BTreeMap::from([
        (
            SchemaKind::Request,
            (
                "https://industrialalgebra.com/schemas/amari.discovery/request/v1",
                &["schema_version", "command", "arguments"] as &[_],
            ),
        ),
        (
            SchemaKind::Response,
            (
                "https://industrialalgebra.com/schemas/amari.discovery/response/v1",
                &["schema_version", "provenance", "warnings", "data"] as &[_],
            ),
        ),
        (
            SchemaKind::Goal,
            (
                "https://industrialalgebra.com/schemas/amari.discovery/goal/v1",
                &["statement"] as &[_],
            ),
        ),
        (
            SchemaKind::Plan,
            (
                "https://industrialalgebra.com/schemas/amari.discovery/plan/v1",
                &[
                    "capability_id",
                    "prerequisite_order",
                    "steps",
                    "compatibility",
                    "normalization",
                    "plan_hash",
                ] as &[_],
            ),
        ),
        (
            SchemaKind::Probe,
            (
                "https://industrialalgebra.com/schemas/amari.discovery/probe/v1",
                &[
                    "probe_id",
                    "backend",
                    "duration_micros",
                    "resources",
                    "catalog_hash",
                    "input_hash",
                    "validated_assumptions",
                    "refuted_assumptions",
                    "warnings",
                    "output",
                ] as &[_],
            ),
        ),
    ])
}

#[test]
fn every_schema_has_stable_id_protocol_and_required_fields() {
    assert_eq!(SchemaKind::ALL.len(), 5);
    for (kind, (expected_id, expected_required)) in expected() {
        let schema = protocol_schema(kind).unwrap();
        assert_eq!(schema.kind, kind);
        assert_eq!(schema.id, expected_id);
        assert_eq!(schema.protocol_version, SCHEMA_V1);
        assert_eq!(
            schema.document["$schema"],
            "https://json-schema.org/draft/2020-12/schema"
        );
        assert_eq!(schema.document["$id"], expected_id);
        assert_eq!(schema.document["x-amari-protocol-version"], SCHEMA_V1);
        let required = schema.document["required"].as_array().unwrap();
        for field in expected_required {
            assert!(
                required.iter().any(|value| value == field),
                "{kind:?} missing {field}"
            );
        }
    }
}

#[test]
fn request_and_response_lock_the_envelope_protocol_version() {
    for kind in [SchemaKind::Request, SchemaKind::Response] {
        let schema = protocol_schema(kind).unwrap();
        assert_eq!(
            schema.document["properties"]["schema_version"]["const"],
            SCHEMA_V1
        );
        assert_eq!(schema.document["additionalProperties"], false);
    }
}

#[test]
fn curated_schema_bytes_match_checked_in_goldens() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for kind in SchemaKind::ALL {
        let schema = protocol_schema(kind).unwrap();
        let golden = fs::read(
            root.join("tests/golden/schemas")
                .join(format!("{}-v1.json", kind.as_str())),
        )
        .unwrap();
        assert_eq!(
            schema.canonical_json().unwrap(),
            golden,
            "{} schema drift",
            kind.as_str()
        );
        let parsed: Value = serde_json::from_slice(&golden).unwrap();
        assert_eq!(parsed, schema.document);
    }
}

#[test]
fn plan_and_probe_schemas_reject_unbounded_or_ambiguous_core_shapes() {
    let plan = protocol_schema(SchemaKind::Plan).unwrap();
    assert_eq!(plan.document["properties"]["steps"]["maxItems"], 64);
    assert!(plan.document["$defs"]["plan_step"]["oneOf"].is_array());
    assert_eq!(
        plan.document["properties"]["plan_hash"]["pattern"],
        "^[0-9a-f]{64}$"
    );
    assert_eq!(
        plan.document["properties"]["compatibility"]["$ref"],
        "#/$defs/compatibility"
    );
    assert_eq!(
        plan.document["$defs"]["compatibility"]["properties"]["probe_results"]["items"]["$ref"],
        "#/$defs/probe_replay_hash"
    );
    assert_eq!(
        plan.document["properties"]["normalization"]["properties"]["trace"]["items"]["$ref"],
        "#/$defs/normalization_trace"
    );
    for side in ["before", "after"] {
        assert_eq!(
            plan.document["$defs"]["normalization_trace"]["properties"][side]["items"]["$ref"],
            "#/$defs/plan_step"
        );
        assert_eq!(
            plan.document["$defs"]["normalization_trace"]["properties"][side]["maxItems"],
            64
        );
    }

    let probe = protocol_schema(SchemaKind::Probe).unwrap();
    assert_eq!(
        probe.document["properties"]["input_hash"]["pattern"],
        "^[0-9a-f]{64}$"
    );
    assert_eq!(
        probe.document["properties"]["validated_assumptions"]["maxItems"],
        256
    );
    assert_eq!(
        probe.document["properties"]["refuted_assumptions"]["maxItems"],
        256
    );
}
