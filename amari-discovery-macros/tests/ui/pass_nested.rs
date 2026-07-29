// SPDX-License-Identifier: MIT OR Apache-2.0

use amari_discovery_macros::WireContract;
use schemars::JsonSchema;
use serde::Serialize;

include!("support.rs");

#[derive(JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Nested {
    pub numerator: String,
    pub denominator: String,
}

#[derive(JsonSchema, Serialize, WireContract)]
#[wire_contract(
    id = "amari.discovery/probe/nested-probe/output/v1",
    role = "output",
    compatibility = "versioned_change"
)]
#[serde(deny_unknown_fields)]
pub struct NestedOutput {
    pub exact: Nested,
    pub warnings: Vec<String>,
}

fn main() {}
