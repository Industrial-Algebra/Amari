// SPDX-License-Identifier: MIT OR Apache-2.0
//! Generates catalog/generated-wasm.json from a .d.ts file.
//!
//! Usage: cargo run -p amari-discovery --example generate_wasm_surface -- <path-to.d.ts>
//!
//! The output includes deterministic capability mappings attached by
//! [`amari_discovery::default_capability_mappings`] so that the checked-in
//! snapshot always encodes the known canonical semantic-IDs.

use std::env;
use std::fs;
use std::process;

use amari_discovery::{default_capability_mappings, parse_wasm_surface};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <path-to.d.ts>", args[0]);
        process::exit(1);
    }

    let src = fs::read_to_string(&args[1]).unwrap_or_else(|e| {
        eprintln!("Error reading {}: {}", args[1], e);
        process::exit(1);
    });

    let mut surface = parse_wasm_surface(&src).unwrap_or_else(|e| {
        eprintln!("Error parsing {}: {}", args[1], e);
        process::exit(1);
    });

    // --- enrichment: attach deterministic capability mappings ---
    surface.capability_mappings = default_capability_mappings(&surface).unwrap_or_else(|e| {
        eprintln!("Error generating capability mappings: {}", e);
        process::exit(1);
    });

    let json = serde_json::to_string_pretty(&surface).unwrap_or_else(|e| {
        eprintln!("Error serializing: {}", e);
        process::exit(1);
    });

    println!("{}", json);
    eprintln!(
        "Generated: {} classes, {} functions, {} enums, {} interfaces, {} type aliases, {} warnings, {} capability mappings",
        surface.classes.len(),
        surface.functions.len(),
        surface.enums.len(),
        surface.interfaces.len(),
        surface.type_aliases.len(),
        surface.warnings.len(),
        surface.capability_mappings.len(),
    );
}
