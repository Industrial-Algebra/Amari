// SPDX-License-Identifier: MIT OR Apache-2.0

use amari_discovery_macros::WireContract;
use schemars::JsonSchema;
use serde::Serialize;

#[derive(JsonSchema, Serialize, WireContract)]
#[wire_contract(
    id = "amari.discovery/probe/test-probe/input/v1",
    role = "input"
)]
pub struct GenericDto<T> {
    pub value: T,
}

fn main() {}
