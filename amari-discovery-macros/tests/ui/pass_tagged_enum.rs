// SPDX-License-Identifier: MIT OR Apache-2.0

use amari_discovery_macros::WireContract;
use schemars::JsonSchema;
use serde::Serialize;

include!("support.rs");

#[derive(JsonSchema, Serialize, WireContract)]
#[wire_contract(
    id = "amari.discovery/probe/term-probe/input/v1",
    role = "input",
    compatibility = "additive_patch"
)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Term {
    Variable { name: String },
    Symbol { name: String, arguments: Vec<Term> },
}

fn main() {}
