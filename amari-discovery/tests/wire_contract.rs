// SPDX-License-Identifier: MIT OR Apache-2.0

use amari_discovery::wire::{
    ProbeSchemaDocument, ProbeSchemaSummary, WireCompatibility, WireExample, WireSchemaRole,
    WireSemanticConstraint,
};
use amari_discovery::SCHEMA_V1;

const INPUT_ID: &str = "amari.discovery/probe/dual-polynomial-derivative/input/v1";
const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

#[test]
fn schema_role_serializes_as_input_or_output() {
    assert_eq!(
        serde_json::to_value(WireSchemaRole::Input).unwrap(),
        "input"
    );
    assert_eq!(
        serde_json::to_value(WireSchemaRole::Output).unwrap(),
        "output"
    );
}

#[test]
fn compatibility_class_serializes_as_stable_snake_case() {
    assert_eq!(
        serde_json::to_value(WireCompatibility::AdditivePatch).unwrap(),
        "additive_patch"
    );
    assert_eq!(
        serde_json::to_value(WireCompatibility::VersionedChange).unwrap(),
        "versioned_change"
    );
}

#[test]
fn schema_summary_rejects_malformed_hash() {
    let summary = ProbeSchemaSummary::new(
        INPUT_ID,
        WireSchemaRole::Input,
        WireCompatibility::AdditivePatch,
        HASH,
    )
    .expect("valid summary must be accepted");
    assert_eq!(summary.id(), INPUT_ID);
    assert_eq!(summary.role(), WireSchemaRole::Input);
    assert_eq!(summary.hash(), HASH);

    let error = ProbeSchemaSummary::new(
        INPUT_ID,
        WireSchemaRole::Input,
        WireCompatibility::AdditivePatch,
        "ABC123",
    )
    .expect_err("uppercase short hash must be rejected");
    assert_eq!(error.kind(), "invalid_input");
}

#[test]
fn canonical_document_json_is_pretty_with_trailing_newline() {
    let document = ProbeSchemaDocument::new(
        INPUT_ID,
        WireSchemaRole::Input,
        SCHEMA_V1,
        serde_json::json!({
            "type": "object",
            "required": ["coefficients"],
            "properties": {
                "coefficients": {"type": "array", "items": {"type": "number"}}
            },
            "additionalProperties": false
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
    .expect("document must validate");

    let canonical = document.canonical_json().expect("canonical JSON");
    assert!(canonical.ends_with('\n'));
    assert!(canonical.contains("\n  \"$id\": "));
    assert_eq!(document.canonical_hash().unwrap().len(), 64);
    assert!(document
        .canonical_hash()
        .unwrap()
        .chars()
        .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_uppercase()));
}
