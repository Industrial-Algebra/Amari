// SPDX-License-Identifier: MIT OR Apache-2.0

use amari_discovery_macros::WireContract;
use schemars::JsonSchema;
use serde::Serialize;

include!("support.rs");

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
pub struct TestRequest {
    pub value: f64,
    pub labels: Vec<String>,
}

fn main() {}
