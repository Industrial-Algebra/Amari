// SPDX-License-Identifier: MIT OR Apache-2.0

use amari_discovery_macros::WireContract;
use schemars::JsonSchema;
use serde::Serialize;

include!("ui/support.rs");

use wire::{WireCompatibility, WireContract as _, WireSchemaRole};

#[derive(JsonSchema, Serialize, WireContract)]
#[wire_contract(
    id = "amari.discovery/probe/test-probe/input/v1",
    role = "input",
    compatibility = "additive_patch",
    constraints(
        finite_numbers = "all numeric values must be finite",
        nonempty_labels = "labels must contain at least one entry"
    ),
    example(label = "simple", value = "{\"value\":1.0,\"labels\":[\"a\"]}")
)]
struct TestRequest {
    value: f64,
    labels: Vec<String>,
}

#[test]
fn derive_emits_structural_schema_and_semantic_metadata() {
    assert_eq!(
        TestRequest::SCHEMA_ID,
        "amari.discovery/probe/test-probe/input/v1"
    );
    assert_eq!(TestRequest::ROLE, WireSchemaRole::Input);
    assert_eq!(TestRequest::COMPATIBILITY, WireCompatibility::AdditivePatch);

    let schema = TestRequest::structural_schema();
    assert_eq!(schema["type"], "object");
    assert!(schema["properties"]["value"].is_object());
    assert!(schema["properties"]["labels"].is_object());

    let constraints = TestRequest::semantic_constraints();
    assert_eq!(constraints.len(), 2);
    assert_eq!(constraints[0].id, "finite_numbers");
    assert_eq!(
        constraints[1].description,
        "labels must contain at least one entry"
    );

    let examples = TestRequest::examples();
    assert_eq!(examples.len(), 1);
    assert_eq!(examples[0].label, "simple");
    assert_eq!(examples[0].value["labels"][0], "a");
}
