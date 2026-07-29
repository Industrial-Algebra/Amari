// SPDX-License-Identifier: MIT OR Apache-2.0

use amari_discovery_macros::WireContract;
use schemars::JsonSchema;
use serde::Serialize;

pub mod wire {
    #[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
    #[serde(rename_all = "snake_case")]
    pub enum WireSchemaRole {
        Input,
        Output,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
    #[serde(rename_all = "snake_case")]
    pub enum WireCompatibility {
        AdditivePatch,
    }

    #[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
    pub struct WireSemanticConstraint;

    #[derive(Clone, Debug, PartialEq, serde::Serialize)]
    pub struct WireExample;

    pub trait WireContract {
        fn schema_id(&self) -> &'static str;
        fn schema_role(&self) -> WireSchemaRole;
        fn structural_schema(&self) -> serde_json::Value;
        fn semantic_constraints(&self) -> &'static [WireSemanticConstraint];
        fn examples(&self) -> &'static [WireExample];
        fn compatibility(&self) -> WireCompatibility;
    }
}

#[derive(JsonSchema, Serialize, WireContract)]
#[wire_contract(
    id = "amari.discovery/probe/test-probe/input/v1",
    role = "input",
    compatibility = "additive_patch"
)]
pub struct TestRequest {
    pub value: f64,
    pub labels: Vec<String>,
}

fn main() {}
