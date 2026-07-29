// SPDX-License-Identifier: MIT OR Apache-2.0

use amari_discovery_macros::WireContract;
use schemars::JsonSchema;
use serde::Serialize;

include!("support.rs");

#[derive(JsonSchema, Serialize, WireContract)]
#[wire_contract(
    id = "amari.discovery/probe/shape-probe/input/v1",
    role = "input",
    compatibility = "additive_patch"
)]
#[serde(deny_unknown_fields)]
pub struct SupportedShapes {
    pub vector: [f64; 8],
    pub matrix: Vec<Vec<Option<f64>>>,
    pub maybe: Option<usize>,
    pub label: String,
    pub signed: i64,
    pub unsigned: u64,
    pub flag: bool,
}

fn main() {}
