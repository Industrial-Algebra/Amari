// SPDX-License-Identifier: MIT OR Apache-2.0

use amari_discovery::{
    ProbeSchemaDocument, WireCompatibility, WireExample, WireSchemaRole, WireSemanticConstraint,
    SCHEMA_V1,
};
use sha2::{Digest, Sha256};

const INPUT_ID: &str = "amari.discovery/probe/dual-polynomial-derivative/input/v1";

fn structural_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "required": ["coefficients"],
        "properties": {
            "coefficients": {
                "type": "array",
                "items": {"type": "number"}
            }
        },
        "additionalProperties": false
    })
}

fn document() -> ProbeSchemaDocument {
    ProbeSchemaDocument::new(
        INPUT_ID,
        WireSchemaRole::Input,
        SCHEMA_V1,
        structural_schema(),
        vec![WireSemanticConstraint::new(
            "non_empty_coefficients",
            "coefficients must contain at least one finite coefficient",
        )],
        vec![WireExample::new(
            "quadratic",
            serde_json::json!({"coefficients": [1.0, 2.0, 3.0]}),
        )],
        WireCompatibility::AdditivePatch,
    )
    .expect("valid schema document")
}

#[test]
fn exported_document_contains_structural_and_amari_metadata() {
    let exported = document().exported_value().unwrap();
    assert_eq!(exported["$id"], INPUT_ID);
    assert_eq!(
        exported["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(exported["additionalProperties"], false);
    assert_eq!(exported["x-amari-schema-role"], "input");
    assert_eq!(exported["x-amari-protocol-version"], SCHEMA_V1);
    assert_eq!(
        exported["x-amari-semantic-constraints"][0]["id"],
        "non_empty_coefficients"
    );
    assert_eq!(exported["x-amari-examples"][0]["label"], "quadratic");
    assert_eq!(exported["x-amari-compatibility"], "additive_patch");
}

#[test]
fn canonical_bytes_are_stable_across_schema_construction_order() {
    let first = document();
    let reordered = ProbeSchemaDocument::new(
        INPUT_ID,
        WireSchemaRole::Input,
        SCHEMA_V1,
        serde_json::json!({
            "additionalProperties": false,
            "properties": {
                "coefficients": {
                    "items": {"type": "number"},
                    "type": "array"
                }
            },
            "required": ["coefficients"],
            "type": "object"
        }),
        vec![WireSemanticConstraint::new(
            "non_empty_coefficients",
            "coefficients must contain at least one finite coefficient",
        )],
        vec![WireExample::new(
            "quadratic",
            serde_json::json!({"coefficients": [1.0, 2.0, 3.0]}),
        )],
        WireCompatibility::AdditivePatch,
    )
    .unwrap();

    assert_eq!(
        first.canonical_json().unwrap(),
        reordered.canonical_json().unwrap()
    );
    assert_eq!(
        first.canonical_hash().unwrap(),
        reordered.canonical_hash().unwrap()
    );
}

#[test]
fn canonical_hash_is_sha256_over_exact_exported_bytes() {
    let document = document();
    let canonical = document.canonical_json().unwrap();
    let expected = hex::encode(Sha256::digest(canonical.as_bytes()));
    assert_eq!(document.canonical_hash().unwrap(), expected);
    assert_eq!(document.summary().unwrap().hash(), expected);
}

#[test]
fn malformed_schema_version_and_protocol_are_rejected() {
    for id in [
        "amari.discovery/probe/dual-polynomial-derivative/input/v0",
        "amari.discovery/probe/dual-polynomial-derivative/input/vx",
    ] {
        let error = ProbeSchemaDocument::new(
            id,
            WireSchemaRole::Input,
            SCHEMA_V1,
            structural_schema(),
            Vec::new(),
            Vec::new(),
            WireCompatibility::AdditivePatch,
        )
        .expect_err("malformed schema version must be rejected");
        assert_eq!(error.kind(), "invalid_input");
    }

    let error = ProbeSchemaDocument::new(
        INPUT_ID,
        WireSchemaRole::Input,
        "amari.discovery/v2",
        structural_schema(),
        Vec::new(),
        Vec::new(),
        WireCompatibility::AdditivePatch,
    )
    .expect_err("unknown protocol version must be rejected");
    assert_eq!(error.kind(), "invalid_input");
}
