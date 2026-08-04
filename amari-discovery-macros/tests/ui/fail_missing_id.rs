// SPDX-License-Identifier: MIT OR Apache-2.0

use amari_discovery_macros::WireContract;
use schemars::JsonSchema;
use serde::Serialize;

#[derive(JsonSchema, Serialize, WireContract)]
pub struct MissingId {
    pub value: f64,
}

fn main() {}
