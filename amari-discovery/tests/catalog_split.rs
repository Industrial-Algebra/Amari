// SPDX-License-Identifier: MIT OR Apache-2.0
//! Split-layout contracts for the structural catalog (CORE-CAT1).
//!
//! The catalog is stored as `catalog/manifest.json` plus one JSON file per
//! crate under `catalog/crates/`. These tests pin the two properties the
//! split exists for:
//!
//! 1. **Round-trip byte equality** — composing the split files must
//!    reproduce, byte for byte, the canonical serialization of the
//!    in-memory catalog. This keeps the `content_hash` recipe and all
//!    validation semantics unchanged from the monolithic layout.
//! 2. **Loader equivalence** — the embedded (compile-time) catalog built
//!    from the split files must equal the freshly generated one.

use amari_discovery::{
    compose_structural, generate_workspace_catalog, split_catalog, StructuralCatalog,
};
use std::path::Path;

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("amari-discovery is inside the workspace")
}

fn canonical_bytes(catalog: &StructuralCatalog) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(catalog).unwrap();
    bytes.push(b'\n');
    bytes
}

#[test]
fn split_then_compose_reproduces_monolith_bytes() {
    let catalog = generate_workspace_catalog(workspace_root()).unwrap();
    let (manifest, bodies) = split_catalog(&catalog).unwrap();

    let manifest_json = serde_json::to_string_pretty(&manifest).unwrap();
    let refs: Vec<(&str, &str)> = bodies
        .iter()
        .map(|(n, b)| (n.as_str(), b.as_str()))
        .collect();
    let composed = compose_structural(&manifest_json, &refs).unwrap();

    assert_eq!(canonical_bytes(&composed), canonical_bytes(&catalog));
}

#[test]
fn split_manifest_covers_every_crate_exactly_once() {
    let catalog = generate_workspace_catalog(workspace_root()).unwrap();
    let (manifest, bodies) = split_catalog(&catalog).unwrap();

    assert_eq!(manifest.crates.len(), catalog.crates.len());
    assert_eq!(bodies.len(), catalog.crates.len());
    let names: std::collections::HashSet<&str> =
        manifest.crates.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names.len(), manifest.crates.len(), "duplicate crate refs");
    for record in &catalog.crates {
        assert!(
            names.contains(record.name.as_str()),
            "missing {}",
            record.name
        );
    }
}

#[test]
fn compose_rejects_mismatched_crate_bodies() {
    let catalog = generate_workspace_catalog(workspace_root()).unwrap();
    let (manifest, bodies) = split_catalog(&catalog).unwrap();
    let manifest_json = serde_json::to_string_pretty(&manifest).unwrap();

    // Drop one body: composition must fail, not silently produce a
    // truncated catalog.
    let truncated = &bodies[..bodies.len() - 1];
    let refs: Vec<(&str, &str)> = truncated
        .iter()
        .map(|(n, b)| (n.as_str(), b.as_str()))
        .collect();
    assert!(compose_structural(&manifest_json, &refs).is_err());
}
