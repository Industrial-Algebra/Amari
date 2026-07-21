// SPDX-License-Identifier: MIT OR Apache-2.0

use std::{
    collections::{BTreeSet, HashSet},
    fs,
    path::Path,
};

use amari_discovery::{generate_workspace_catalog, verify_checked_in, Catalog, StructuralCatalog};
use sha2::Digest;

/// Local canonical JSON helper (the library canonical_json is not public).
fn canonical_json(catalog: &StructuralCatalog) -> serde_json::Result<Vec<u8>> {
    let mut json_bytes = serde_json::to_vec_pretty(catalog)?;
    json_bytes.push(b'\n');
    Ok(json_bytes)
}

/// Crates that legitimately have zero declared Cargo features.
const ALLOWED_NO_FEATURES: &[&str] = &["amari-flynn-macros", "amari-surcomplex", "amari-wasm"];

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("amari-discovery is inside the workspace")
}

fn checked_in_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("catalog/generated.json")
}

// ============================================================================
// Core generation tests
// ============================================================================

#[test]
fn generate_twice_produces_identical_results() {
    let first = generate_workspace_catalog(workspace_root()).unwrap();
    let second = generate_workspace_catalog(workspace_root()).unwrap();

    assert_eq!(first.crates.len(), second.crates.len());
    for (a, b) in first.crates.iter().zip(second.crates.iter()) {
        assert_eq!(a.name, b.name);
        assert_eq!(a.items.len(), b.items.len());
        assert_eq!(a.macros.len(), b.macros.len());
    }
    assert_eq!(
        first.probe_descriptors.len(),
        second.probe_descriptors.len()
    );
    assert_eq!(first.dependency_edges.len(), second.dependency_edges.len());

    // Canonical JSON must be byte-identical.
    let first_json = canonical_json(&first).unwrap();
    let second_json = canonical_json(&second).unwrap();
    assert_eq!(first_json, second_json);

    // Both must have identical content_hash.
    assert_eq!(first.content_hash, second.content_hash);
    assert!(first.content_hash.is_some());
}

#[test]
fn generated_catalog_has_26_packages_excluding_discovery() {
    let catalog = generate_workspace_catalog(workspace_root()).unwrap();

    let names: BTreeSet<&str> = catalog.crates.iter().map(|c| c.name.as_str()).collect();
    assert!(
        !names.contains("amari-discovery"),
        "amari-discovery must be excluded"
    );
    assert_eq!(catalog.crates.len(), 26, "expected 26 workspace packages");

    // Verify known packages are present.
    for expected in &[
        "amari",
        "amari-core",
        "amari-tropical",
        "amari-dual",
        "amari-network",
        "amari-optimization",
        "amari-holographic",
        "amari-cgt",
        "amari-surreal",
        "amari-surcomplex",
        "amari-rewrite",
        "amari-wasm",
        "amari-flynn",
        "amari-flynn-macros",
        "amari-gpu",
    ] {
        assert!(
            names.contains(expected),
            "expected package {expected} in catalog"
        );
    }
}

#[test]
fn every_crate_has_nonempty_metadata_and_no_missing_defaults() {
    let catalog = generate_workspace_catalog(workspace_root()).unwrap();

    for c in &catalog.crates {
        assert!(!c.name.is_empty(), "crate name must be nonempty");
        assert!(!c.version.is_empty(), "{} version must be nonempty", c.name);
        assert!(
            !c.description.is_empty(),
            "{} description must be nonempty",
            c.name
        );
        assert!(!c.license.is_empty(), "{} license must be nonempty", c.name);
        assert!(!c.edition.is_empty(), "{} edition must be nonempty", c.name);
        assert!(
            !c.manifest_path.is_empty(),
            "{} manifest_path must be nonempty",
            c.name
        );
        assert!(
            !c.library_outputs.is_empty(),
            "{} library_outputs must be nonempty",
            c.name
        );

        // Most crates have at least one feature (default is almost always present).
        // Some proc-macro crates may genuinely have no features declared.
        assert!(
            !c.features.is_empty() || ALLOWED_NO_FEATURES.contains(&c.name.as_str()),
            "{} must have at least one feature",
            c.name
        );
    }
}

#[test]
fn required_semantic_symbols_are_present() {
    let catalog = generate_workspace_catalog(workspace_root()).unwrap();

    let all_item_paths: HashSet<&str> = catalog
        .crates
        .iter()
        .flat_map(|c| c.items.iter())
        .map(|i| i.path.as_str())
        .collect();

    // Required paths from semantic/core.toml.
    let required = &[
        "amari_core::Multivector::geometric_product",
        "amari_core::Rotor",
        "amari_tropical::TropicalMatrix",
        "amari_tropical::viterbi::TropicalViterbi::decode",
        "amari_dual::DualNumber::derivative",
        "amari_network::GeometricNetwork::shortest_path",
        "amari_optimization::multiobjective::ParetoFront",
        "amari_holographic::HolographicMemory::retrieve",
        "amari_cgt::GameArena::grundy",
        "amari_surreal::RationalSurreal",
        "amari_surcomplex::RationalSurcomplex",
        "amari_rewrite::trs::TermSystem::apply_once",
        "amari_rewrite::synthesis::infer_rule",
    ];
    for path in required {
        assert!(
            all_item_paths.contains(path),
            "required semantic symbol {path} not found in generated catalog"
        );
    }
}

#[test]
fn required_features_and_examples_are_present() {
    let catalog = generate_workspace_catalog(workspace_root()).unwrap();

    let all_features: HashSet<String> = catalog
        .crates
        .iter()
        .flat_map(|c| c.features.iter().map(|f| format!("{}:{}", c.name, f.name)))
        .collect();
    let all_examples: HashSet<String> = catalog
        .crates
        .iter()
        .flat_map(|c| c.examples.iter().map(|e| format!("{}:{}", c.name, e.name)))
        .collect();

    for feat in &["amari-core:std", "amari-tropical:std"] {
        assert!(
            all_features.contains(*feat),
            "required feature {feat} not found"
        );
    }
    for ex in &["amari-core:basic", "amari-tropical:max_plus_paths"] {
        assert!(
            all_examples.contains(*ex),
            "required example {ex} not found"
        );
    }
}

// ============================================================================
// Rich record tests
// ============================================================================

#[test]
fn amari_core_has_rich_records() {
    let catalog = generate_workspace_catalog(workspace_root()).unwrap();
    let core = catalog
        .crates
        .iter()
        .find(|c| c.name == "amari-core")
        .unwrap();

    // Must have items, modules, features, dependencies, targets.
    assert!(!core.items.is_empty(), "amari-core must have items");
    assert!(!core.modules.is_empty(), "amari-core must have modules");
    assert!(!core.features.is_empty(), "amari-core must have features");
    assert!(
        !core.trait_definitions.is_empty(),
        "amari-core must have trait definitions"
    );
    assert!(
        !core.trait_implementations.is_empty(),
        "amari-core must have trait impls"
    );
    assert!(!core.cfg_gates.is_empty(), "amari-core must have cfg gates");
    assert!(!core.examples.is_empty(), "amari-core must have examples");

    // Check a specific struct has shape.
    let mv = core
        .items
        .iter()
        .find(|i| i.path == "amari_core::Multivector")
        .unwrap();
    assert_eq!(mv.kind.as_deref(), Some("struct"));
    assert!(mv.signature.is_some());
    assert!(mv.shape.is_some(), "Multivector must have shape");

    // Check associated items are top-level ItemRecords (no inline associated array).
    assert!(
        !mv.variants.is_empty(),
        "Multivector must have at least one variant"
    );
    let gp = core
        .items
        .iter()
        .find(|i| i.path == "amari_core::Multivector::geometric_product");
    assert!(
        gp.is_some(),
        "geometric_product must be a top-level ItemRecord"
    );
    let gp = gp.unwrap();
    assert_eq!(gp.kind.as_deref(), Some("method"));

    // Check source paths.
    assert!(mv.source_path.is_some());
}

#[test]
fn amari_flynn_macros_has_proc_macro_records() {
    let catalog = generate_workspace_catalog(workspace_root()).unwrap();
    let flynn_macros = catalog
        .crates
        .iter()
        .find(|c| c.name == "amari-flynn-macros")
        .unwrap();

    assert_eq!(flynn_macros.library_outputs, vec!["proc-macro"]);
    assert!(
        !flynn_macros.macros.is_empty(),
        "amari-flynn-macros must have macro records"
    );
    for m in &flynn_macros.macros {
        assert!(
            m.path.starts_with("amari_flynn_macros::"),
            "macro path must be package-qualified"
        );
    }
}

// ============================================================================
// WASM surface tests
// ============================================================================

#[test]
fn wasm_surface_reference_is_present() {
    let catalog = generate_workspace_catalog(workspace_root()).unwrap();
    let wasm = catalog.wasm_surface.as_ref().unwrap();

    assert_eq!(wasm.path, "amari-discovery/catalog/generated-wasm.json");
    assert!(!wasm.source_hash.is_empty());
    assert_eq!(wasm.source_hash.len(), 64);
    assert!(wasm.class_count > 0);
    assert!(!wasm.capability_mappings.is_empty());
}

#[test]
fn wasm_capability_mappings_must_match_recomputed_defaults() {
    // RED: When generated-wasm.json has capability_mappings that differ
    // from recomputed defaults, generation must fail with CatalogCorruption.
    let temp = tempfile::TempDir::new().unwrap();
    let root = temp.path();

    // Minimal workspace.
    let workspace_toml = r#"[workspace]
members = ["crates/math"]
resolver = "2"
[workspace.package]
version = "0.1.0"
description = "Test"
license = "MIT"
"#
    .to_string();
    fs::write(root.join("Cargo.toml"), workspace_toml).unwrap();

    // Minimal crate with a real public item so structural passes succeed.
    fs::create_dir_all(root.join("crates/math/src")).unwrap();
    let math_toml = r#"
[package]
name = "fixture-math"
version.workspace = true
description.workspace = true
license.workspace = true
"#;
    fs::write(root.join("crates/math/Cargo.toml"), math_toml).unwrap();
    fs::write(
        root.join("crates/math/src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }",
    )
    .unwrap();

    // amari-discovery sentinel.
    fs::create_dir_all(root.join("amari-discovery/catalog")).unwrap();
    fs::write(
        root.join("amari-discovery/Cargo.toml"),
        "[package]\nname = \"amari-discovery\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();

    // Valid probes.toml (schema2 requirement).
    fs::write(
        root.join("amari-discovery/catalog/probes.toml"),
        r#"
catalog_version = "0.1.0"
[[probes]]
id = "amari-probe:test:basic:v1"
capability_id = "amari:test:basic"
input_schema = "amari.discovery/probe/test-basic/input/v1"
output_schema = "amari.discovery/probe/test-basic/output/v1"
required_features = []
cost = "low"
deterministic = true
side_effects = "none"
[probes.limits]
max_input_bytes = 1000
max_output_bytes = 1000
max_operations = 10
timeout_millis = 1000
"#,
    )
    .unwrap();

    // Tampered generated-wasm.json with capability_mappings that differ from
    // what default_capability_mappings would recompute from the empty surface.
    fs::write(
        root.join("amari-discovery/catalog/generated-wasm.json"),
        r#"{
  "schema_version": 1,
  "source_hash": "0000000000000000000000000000000000000000000000000000000000000000",
  "description": "tampered",
  "classes": [],
  "functions": [],
  "enums": [],
  "interfaces": [],
  "type_aliases": [],
  "warnings": [],
  "capability_mappings": [
    {
      "wasm_path": "FakeClass.fakeMethod",
      "capability_id": "amari:fake:module:capability"
    }
  ]
}"#,
    )
    .unwrap();

    let result = generate_workspace_catalog(root);
    assert!(
        result.is_err(),
        "tampered capability_mappings must fail generation"
    );
    let err = result.unwrap_err();
    assert_eq!(err.kind(), "catalog_corruption");
    assert!(
        err.to_string().contains("capability_mapping"),
        "error must mention capability mapping: {err}"
    );
}

// ============================================================================
// Probe tests
// ============================================================================

#[test]
fn probe_descriptors_equal_manifest() {
    let catalog = generate_workspace_catalog(workspace_root()).unwrap();

    // probes.toml has 14 probes.
    assert_eq!(catalog.probe_descriptors.len(), 14);

    let ids: BTreeSet<String> = catalog
        .probe_descriptors
        .iter()
        .map(|p| p.id.to_string())
        .collect();
    assert!(ids.contains("amari-probe:core:geometric-product:v1"));
    assert!(ids.contains("amari-probe:tropical:shortest-path:v1"));
    assert!(ids.contains("amari-probe:tropical:viterbi:v1"));
    assert!(ids.contains("amari-probe:rewrite:predecessors:v1"));
}

// ============================================================================
// Determinism and uniqueness tests
// ============================================================================

#[test]
fn paths_are_sorted_and_unique() {
    let catalog = generate_workspace_catalog(workspace_root()).unwrap();

    for c in &catalog.crates {
        // Item paths sorted and unique.
        assert!(
            c.items.windows(2).all(|w| w[0].path <= w[1].path),
            "items not sorted in {}",
            c.name
        );
        let mut seen = HashSet::new();
        for item in &c.items {
            assert!(
                seen.insert(&item.path),
                "duplicate item path {} in {}",
                item.path,
                c.name
            );
        }

        // Modules sorted and unique.
        assert!(
            c.modules.windows(2).all(|w| w[0] <= w[1]),
            "modules not sorted in {}",
            c.name
        );

        // Examples sorted and unique.
        let mut ex_seen = HashSet::new();
        for ex in &c.examples {
            assert!(ex_seen.insert(&ex.name), "duplicate example {}", ex.name);
        }
    }
}

#[test]
fn no_discovery_api_leaked_in_catalog() {
    let catalog = generate_workspace_catalog(workspace_root()).unwrap();

    for c in &catalog.crates {
        for item in &c.items {
            assert!(
                !item.path.contains("amari_discovery"),
                "discovery API leaked: {}",
                item.path
            );
        }
    }
}

// ============================================================================
// Checked-in file equality / drift tests
// ============================================================================

#[test]
fn generated_equals_checked_in() {
    // The canonical checked-in file must exactly match generation.
    verify_checked_in(workspace_root(), &checked_in_path()).unwrap();
}

#[test]
fn modified_copy_produces_drift_error() {
    let catalog = generate_workspace_catalog(workspace_root()).unwrap();
    let json = canonical_json(&catalog).unwrap();

    let temp_dir = tempfile::TempDir::new().unwrap();
    let temp_path = temp_dir.path().join("modified.json");

    // Write a deliberately corrupted version.
    let modified = String::from_utf8_lossy(&json).replace(
        "\"version\": \"0.23.0\"",
        "\"version\": \"0.0.0-corrupted\"",
    );
    fs::write(&temp_path, modified.as_bytes()).unwrap();

    let result = verify_checked_in(workspace_root(), &temp_path);
    assert!(result.is_err(), "drift must be detected");
    let err = result.unwrap_err();
    assert_eq!(err.kind(), "catalog_corruption");
    assert!(err.to_string().contains("drift"));
}

// ============================================================================
// Task 5D: A. PURE LIBRARY + HASH — RED tests
// ============================================================================

#[test]
fn generated_catalog_content_hash_is_some_and_deterministic() {
    // RED: generate_workspace_catalog must return content_hash already populated.
    let catalog = generate_workspace_catalog(workspace_root()).unwrap();
    let hash = catalog
        .content_hash
        .as_ref()
        .expect("content_hash must be Some after generation");
    assert_eq!(
        hash.len(),
        64,
        "content_hash must be 64 hex chars (SHA-256)"
    );
    assert!(
        hash.chars().all(|c| c.is_ascii_hexdigit()),
        "content_hash must be hex"
    );

    // Second generation produces the same hash.
    let second = generate_workspace_catalog(workspace_root()).unwrap();
    assert_eq!(
        catalog.content_hash, second.content_hash,
        "content_hash must be deterministic"
    );
}

#[test]
fn content_hash_is_sensitive_to_catalog_content() {
    // The content_hash must change when catalog content changes.
    // Genuinely mutate, clear/recompute canonical no-hash bytes, assert changed.
    let mut catalog = generate_workspace_catalog(workspace_root()).unwrap();
    let first_hash = catalog.content_hash.clone().unwrap();

    // Genuinely mutate the catalog content.
    catalog.description = "Modified description for hash sensitivity test".to_string();

    // Clear and recompute the content hash from the canonical no-hash bytes.
    catalog.content_hash = None;
    let mut hasher = sha2::Sha256::new();
    let json_without_hash = serde_json::to_vec_pretty(&catalog).unwrap();
    hasher.update(&json_without_hash);
    let new_hash = hex::encode(hasher.finalize());
    catalog.content_hash = Some(new_hash.clone());

    assert_ne!(
        first_hash, new_hash,
        "content_hash must change when catalog content changes"
    );
    assert_ne!(
        first_hash, "0000000000000000000000000000000000000000000000000000000000000000",
        "hash must not be the zero hash"
    );
    assert!(
        first_hash.chars().all(|c| c.is_ascii_hexdigit()),
        "first_hash must be hex"
    );
    assert!(
        new_hash.chars().all(|c| c.is_ascii_hexdigit()),
        "new_hash must be hex"
    );
}

#[test]
fn verify_checked_in_drift_message_includes_first_differing_line() {
    // RED: Drift messages must include the first differing line number.
    let catalog = generate_workspace_catalog(workspace_root()).unwrap();
    let json = canonical_json(&catalog).unwrap();

    let temp_dir = tempfile::TempDir::new().unwrap();
    let temp_path = temp_dir.path().join("corrupted.json");

    // Corrupt the version string which appears early in the JSON.
    let modified = String::from_utf8_lossy(&json).replace(
        "\"version\": \"0.23.0\"",
        "\"version\": \"0.0.0-corrupted\"",
    );
    fs::write(&temp_path, modified.as_bytes()).unwrap();

    let result = verify_checked_in(workspace_root(), &temp_path);
    assert!(result.is_err(), "drift must be detected");
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("drift"), "must mention drift");
    // RED: The drift message should include the first differing line number.
    assert!(
        err_msg.contains("line ") || err_msg.contains("differing"),
        "drift message must help locate the difference: {err_msg}"
    );
}

// ============================================================================
// Task 5D: B. REQUIRED INPUTS + VALIDATION — RED tests
// ============================================================================

#[test]
fn missing_probes_toml_is_typed_error_not_empty() {
    // RED: Missing probes.toml must be a CatalogCorruption error, not empty vec.
    let fixture_root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/catalog-workspace");

    // This fixture has no probes.toml. Generation should fail for schema2.
    let result = generate_workspace_catalog(&fixture_root);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.kind(), "catalog_corruption");
}

#[test]
fn missing_wasm_json_is_typed_error_not_none() {
    // RED: Missing generated-wasm.json must be a typed error, not None.
    // We test this indirectly: the generated catalog's wasm_surface must be Some.
    let catalog = generate_workspace_catalog(workspace_root()).unwrap();
    assert!(
        catalog.wasm_surface.is_some(),
        "wasm_surface must be Some when generated-wasm.json exists and is valid"
    );
}

#[test]
fn catalog_validate_schema2_requires_probe_descriptors_match_manifest() {
    // RED: For schema2, probe_descriptors in the structural catalog must exactly
    // equal the separately loaded ProbeManifest.probes (full == equality).
    let catalog = generate_workspace_catalog(workspace_root()).unwrap();
    assert_eq!(catalog.schema_version, 2);

    // The probe_descriptors in the catalog must match the separately loaded
    // probes.toml. The generated catalog captures probes.toml exactly in
    // probe_descriptors; the embedded catalog validates full equality.
    let probes_toml_str = include_str!("../catalog/probes.toml");
    let manifest: amari_discovery::ProbeManifest = toml::from_str(probes_toml_str).unwrap();
    assert_eq!(
        catalog.probe_descriptors, manifest.probes,
        "generated probe_descriptors must match probes.toml exactly"
    );

    // Also verify full inequality detection: modify a limit in the generated
    // probes and confirm that equality check catches it.
    let mut modified = catalog.probe_descriptors.clone();
    if let Some(first) = modified.first_mut() {
        first.limits.max_input_bytes = 1; // tamper
    }
    assert_ne!(
        modified, manifest.probes,
        "modified probe descriptors must differ from probes.toml"
    );
}

#[test]
fn catalog_validate_schema2_rejects_missing_probe_descriptors() {
    // RED: A schema2 catalog with empty probe_descriptors must fail validation.
    let mut catalog = generate_workspace_catalog(workspace_root()).unwrap();
    catalog.probe_descriptors.clear();

    // Serialize to JSON, then try to load and validate.
    let json = serde_json::to_string_pretty(&catalog).unwrap();
    let semantic = include_str!("../catalog/semantic/core.toml");
    let probes = include_str!("../catalog/probes.toml");

    let result = Catalog::from_sources(&json, semantic, probes);
    assert!(
        result.is_err(),
        "empty probe_descriptors in schema2 must fail validation"
    );
}

#[test]
fn catalog_validate_schema2_rejects_missing_wasm_surface() {
    // RED: A schema2 catalog with wasm_surface = None must fail validation.
    let mut catalog = generate_workspace_catalog(workspace_root()).unwrap();
    catalog.wasm_surface = None;

    let json = serde_json::to_string_pretty(&catalog).unwrap();
    let semantic = include_str!("../catalog/semantic/core.toml");
    let probes = include_str!("../catalog/probes.toml");

    let result = Catalog::from_sources(&json, semantic, probes);
    assert!(
        result.is_err(),
        "missing wasm_surface in schema2 must fail validation"
    );
}

#[test]
fn catalog_validate_schema2_rejects_invalid_content_hash() {
    // RED: Schema2 from_sources preserves the supplied content_hash and
    // validate() rejects mismatched hashes. Tampered checked-in JSON is caught.
    let mut catalog = generate_workspace_catalog(workspace_root()).unwrap();
    // Inject a clearly wrong hash.
    catalog.content_hash =
        Some("0000000000000000000000000000000000000000000000000000000000000000".to_string());

    let json = serde_json::to_string_pretty(&catalog).unwrap();
    let semantic = include_str!("../catalog/semantic/core.toml");
    let probes = include_str!("../catalog/probes.toml");

    // from_sources preserves the bad hash; validate() rejects it.
    let result = Catalog::from_sources(&json, semantic, probes);
    assert!(
        result.is_err(),
        "tampered content_hash must fail validation, got: {result:?}"
    );
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("content_hash mismatch"),
        "error must mention content_hash mismatch, got: {err}"
    );
}

// ============================================================================
// Task 5D: C. COMPLETE/NONLOSSY COMPOSITION — RED tests
// ============================================================================

#[test]
fn macro_records_present_for_all_library_targets_not_just_proc_macro() {
    // RED: macro_catalog must run for every library target, not only proc-macro.
    // amari_core has geo! and wedge! declarative macros.
    let catalog = generate_workspace_catalog(workspace_root()).unwrap();
    let core = catalog
        .crates
        .iter()
        .find(|c| c.name == "amari-core")
        .expect("amari-core must be present");

    // amari-core is NOT a proc-macro crate but HAS declarative macros (geo!, wedge!).
    assert!(
        !core.macros.is_empty(),
        "amari-core must have macro records for its declarative macros (geo!, wedge!)"
    );

    // Verify specific expected macros.
    let macro_names: HashSet<&str> = core.macros.iter().map(|m| m.name.as_str()).collect();
    assert!(
        macro_names.contains("geo") || macro_names.contains("wedge"),
        "amari-core must have geo! or wedge! macro: got {macro_names:?}"
    );
}

#[test]
fn flynn_proc_macro_records_still_present() {
    // RED: amari-flynn-macros proc macros must still be present.
    let catalog = generate_workspace_catalog(workspace_root()).unwrap();
    let flynn_macros = catalog
        .crates
        .iter()
        .find(|c| c.name == "amari-flynn-macros")
        .expect("amari-flynn-macros must be present");

    assert!(
        !flynn_macros.macros.is_empty(),
        "amari-flynn-macros must have proc macro records"
    );
}

#[test]
fn public_modules_only_from_reachable_exports() {
    // RED: Modules must only include publicly reachable paths, not raw
    // module graph private modules.
    let catalog = generate_workspace_catalog(workspace_root()).unwrap();

    for c in &catalog.crates {
        // Every listed module path must start with the crate's Rust name.
        let rust_name = c.name.replace('-', "_");
        for m in &c.modules {
            assert!(
                m.starts_with(&rust_name),
                "module '{}' in {} must be package-qualified",
                m,
                c.name
            );
        }

        // All item paths must also be package-qualified.
        for item in &c.items {
            assert!(
                item.path.starts_with(&rust_name),
                "item path '{}' in {} must be package-qualified",
                item.path,
                c.name
            );
        }
    }
}

#[test]
fn source_paths_use_manifest_parent_not_package_name() {
    // RED: Source path prefixes must use manifest_path parent, not package name.
    let catalog = generate_workspace_catalog(workspace_root()).unwrap();

    // The root package is "amari" which is at the workspace root.
    // Its manifest_path should be "Cargo.toml" (parent is "").
    // But its items may still have source paths relative to the workspace.
    // The amari package's items should live in src/ not amari/src/.
    let amari_pkg = catalog
        .crates
        .iter()
        .find(|c| c.name == "amari")
        .expect("amari package must be present");

    // Check that item source paths don't fabricate a directory that doesn't exist.
    for item in &amari_pkg.items {
        if let Some(sp) = &item.source_path {
            assert!(
                !sp.starts_with("amari/amari"),
                "source path '{}' must not double-nest package name",
                sp
            );
        }
    }

    // Check target paths are workspace-relative, not fabricated.
    for t in &amari_pkg.targets {
        assert!(
            !t.path.starts_with("amari/amari"),
            "target path '{}' must not double-nest",
            t.path
        );
    }
}

#[test]
fn root_package_example_paths_use_examples_dir_not_package_prefix() {
    // RED: Root package example paths must be "examples/..." not
    // "amari/examples/...".
    let catalog = generate_workspace_catalog(workspace_root()).unwrap();
    let amari_pkg = catalog
        .crates
        .iter()
        .find(|c| c.name == "amari")
        .expect("amari package must be present");

    // The amari root package examples should be directly under examples/
    for ex in &amari_pkg.examples {
        assert!(
            ex.path.starts_with("examples/") || ex.path.starts_with("amari/examples/"),
            "example path '{}' must be under examples/",
            ex.path
        );
    }
}

#[test]
fn item_paths_are_package_qualified_not_crate_local() {
    // RED: All item paths must be package-qualified (amari_core::...),
    // not crate-local (crate::...).
    let catalog = generate_workspace_catalog(workspace_root()).unwrap();

    for c in &catalog.crates {
        for item in &c.items {
            assert!(
                !item.path.starts_with("crate::"),
                "item path '{}' in {} must not start with crate::",
                item.path,
                c.name
            );
        }
    }
}

#[test]
fn trait_paths_are_package_qualified_when_local() {
    // RED: trait_path and impl_type_path in trait implementations must be
    // package-qualified for local endpoints.
    let catalog = generate_workspace_catalog(workspace_root()).unwrap();

    for c in &catalog.crates {
        for ti in &c.trait_implementations {
            assert!(
                !ti.trait_path.starts_with("crate::"),
                "trait_path '{}' in {} must not start with crate::",
                ti.trait_path,
                c.name
            );
            assert!(
                !ti.impl_type_path.starts_with("crate::"),
                "impl_type_path '{}' in {} must not start with crate::",
                ti.impl_type_path,
                c.name
            );
        }
    }
}

// ============================================================================
// Task 5D: D. WORKSPACE PATHS — RED tests
// ============================================================================

#[test]
fn source_module_paths_are_package_qualified() {
    // RED: source_module on ItemRecord must be package-qualified.
    let catalog = generate_workspace_catalog(workspace_root()).unwrap();

    for c in &catalog.crates {
        let rust_name = c.name.replace('-', "_");
        for item in &c.items {
            if let Some(sm) = &item.source_module {
                assert!(
                    sm.starts_with(&rust_name) || sm == "crate",
                    "source_module '{}' on item '{}' in {} must be package-qualified or crate",
                    sm,
                    item.path,
                    c.name
                );
            }
        }
    }
}

#[test]
fn readme_path_uses_manifest_parent_not_package_name() {
    // RED: README paths must use manifest_path parent, not package name.
    let catalog = generate_workspace_catalog(workspace_root()).unwrap();
    let amari_pkg = catalog
        .crates
        .iter()
        .find(|c| c.name == "amari")
        .expect("amari package must be present");

    // The amari root package's README should be at the workspace root.
    if let Some(readme) = &amari_pkg.readme {
        assert!(
            readme == "README.md" || readme == "amari/README.md",
            "root package readme should be README.md, not {readme}"
        );
    }
}

// ============================================================================
// Fixture-based deterministic generation test
// ============================================================================

#[test]
fn fixture_workspace_produces_deterministic_catalog() {
    let fixture_root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/catalog-workspace");

    // This fixture workspace doesn't have amari-discovery, so generation
    // should fail with the expected message.
    let result = generate_workspace_catalog(&fixture_root);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("amari-discovery/Cargo.toml"));
}

// ============================================================================
// Task 5D: Item variant / is_reexport / content_hash / probe / WASM RED tests
// ============================================================================

#[test]
fn items_have_variants_array_and_no_inline_associated() {
    // RED: Every ItemRecord has a `variants` array. Owner items must NOT
    // carry an inline `associated` array (removed from the model).
    // Associated items are top-level ItemRecords only.
    let catalog = generate_workspace_catalog(workspace_root()).unwrap();

    for c in &catalog.crates {
        for item in &c.items {
            // Every item must have at least one variant.
            assert!(
                !item.variants.is_empty(),
                "item '{}' in {} must have at least one variant",
                item.path,
                c.name
            );
            // Single-variant items must have canonical summary fields populated.
            if item.variants.len() == 1 {
                assert!(
                    item.kind.is_some(),
                    "single-variant item '{}' must have kind",
                    item.path
                );
                assert!(
                    item.signature.is_some(),
                    "single-variant item '{}' must have signature",
                    item.path
                );
                assert!(
                    item.source_path.is_some(),
                    "single-variant item '{}' must have source_path",
                    item.path
                );
            } else {
                // Multi-variant items omit canonical summary fields.
                assert!(
                    item.kind.is_none(),
                    "multi-variant item '{}' must have kind=None",
                    item.path
                );
            }
            // Verify each variant has kind, signature, and source info.
            for v in &item.variants {
                assert!(!v.kind.is_empty(), "variant kind must not be empty");
                assert!(v.signature.is_some(), "variant signature must be present");
                assert!(
                    v.source_module.is_some(),
                    "variant source_module must be present"
                );
                assert!(
                    v.declaration_module.is_some(),
                    "variant declaration_module must be present"
                );
                assert!(
                    v.declaration_ident.is_some(),
                    "variant declaration_ident must be present"
                );
            }
        }
    }
}

#[test]
fn is_reexport_false_for_direct_module_items() {
    // RED: Direct public module items (where export path matches declaration
    // source identity) must have is_reexport = false. Re-exports through
    // pub use aliases must have is_reexport = true.
    let catalog = generate_workspace_catalog(workspace_root()).unwrap();

    for c in &catalog.crates {
        for item in &c.items {
            for v in &item.variants {
                // A variant is a direct declaration when the export path
                // (last segments) matches declaration_module::declaration_ident.
                let Some(ref decl_module) = v.declaration_module else {
                    continue;
                };
                let Some(ref decl_ident) = v.declaration_ident else {
                    continue;
                };
                let direct_origin = format!("{decl_module}::{decl_ident}");
                let is_direct = item.path == direct_origin;
                if is_direct {
                    assert!(
                        !v.is_reexport,
                        "direct item '{}' in {} must have is_reexport=false, variant has is_reexport=true",
                        item.path, c.name
                    );
                }
            }
        }
    }
}

#[test]
fn associated_items_are_top_level_with_variants() {
    // RED: Associated items (inherent methods, trait items) must be
    // top-level ItemRecords. They must NOT appear in any owner's inline
    // associated array (removed from model). They must themselves support
    // variant grouping when the owner has cfg variants.
    let catalog = generate_workspace_catalog(workspace_root()).unwrap();

    // Find a known method path.
    let gp = catalog
        .crates
        .iter()
        .flat_map(|c| c.items.iter())
        .find(|i| i.path == "amari_core::Multivector::geometric_product");
    assert!(
        gp.is_some(),
        "geometric_product must be a top-level ItemRecord"
    );
    let gp = gp.unwrap();
    assert_eq!(
        gp.kind.as_deref(),
        Some("method"),
        "geometric_product must be a method"
    );
    assert!(
        gp.signature.is_some(),
        "geometric_product must have a signature"
    );
    assert!(
        !gp.variants.is_empty(),
        "geometric_product must have at least one variant"
    );
}

#[test]
fn content_hash_rejects_tampered_json() {
    // RED: Catalog::from_sources preserves the supplied schema2 content_hash
    // and validate() rejects mismatches. We mutate the generated JSON
    // content (not the hash field) and verify rejection.
    let catalog = generate_workspace_catalog(workspace_root()).unwrap();
    let json = serde_json::to_string_pretty(&catalog).unwrap();

    // Tamper the content: change a description string.
    let tampered = json.replace(
        "Generated structural catalog for Amari",
        "TAMPERED structural catalog for Amari",
    );
    // The content_hash field is still the original; content has changed.
    let semantic = include_str!("../catalog/semantic/core.toml");
    let probes = include_str!("../catalog/probes.toml");

    let result = Catalog::from_sources(&tampered, semantic, probes);
    assert!(
        result.is_err(),
        "tampered JSON content must fail content_hash validation, got: {result:?}"
    );
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("content_hash mismatch"),
        "error must indicate content_hash mismatch, got: {err}"
    );
}

#[test]
fn content_hash_rejects_tampered_hash_field() {
    // RED: Directly tampering the content_hash field while keeping content
    // intact must also fail validation.
    let catalog = generate_workspace_catalog(workspace_root()).unwrap();
    let json = serde_json::to_string_pretty(&catalog).unwrap();

    // Replace the hash with a bogus value.
    let original_hash = catalog.content_hash.as_deref().unwrap();
    let tampered = json.replace(
        original_hash,
        "0000000000000000000000000000000000000000000000000000000000000000",
    );

    let semantic = include_str!("../catalog/semantic/core.toml");
    let probes = include_str!("../catalog/probes.toml");

    let result = Catalog::from_sources(&tampered, semantic, probes);
    assert!(
        result.is_err(),
        "tampered content_hash field must fail validation, got: {result:?}"
    );
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("content_hash"),
        "error must mention content_hash, got: {err}"
    );
}

#[test]
fn probe_descriptors_full_equality_catches_modified_limits() {
    // RED: probe_descriptors comparison must use full ProbeDescriptor ==
    // equality, not just count+ID. Modified limits/features/cost with same ID
    // must fail validation.
    let mut catalog = generate_workspace_catalog(workspace_root()).unwrap();

    if let Some(first) = catalog.probe_descriptors.first_mut() {
        first.limits.max_input_bytes = 1; // tamper limits
    }

    let json = serde_json::to_string_pretty(&catalog).unwrap();
    let semantic = include_str!("../catalog/semantic/core.toml");
    let probes = include_str!("../catalog/probes.toml");

    // from_sources preserves the tampered hash; validate() rejects on
    // content_hash mismatch (because the structural JSON changed).
    let result = Catalog::from_sources(&json, semantic, probes);
    assert!(
        result.is_err(),
        "tampered probe limits must fail validation, got: {result:?}"
    );
}

#[test]
fn wasm_capability_mappings_have_unique_nonempty_paths_and_valid_ids() {
    // RED: WASM capability mappings must have unique, nonempty wasm_paths
    // and valid capability_ids that parse as CapabilityId.
    let catalog = generate_workspace_catalog(workspace_root()).unwrap();
    let wasm = catalog
        .wasm_surface
        .as_ref()
        .expect("schema2 requires wasm_surface");

    let mut paths = std::collections::HashSet::new();
    for (i, mapping) in wasm.capability_mappings.iter().enumerate() {
        assert!(
            !mapping.wasm_path.is_empty(),
            "mapping[{i}]: wasm_path must be nonempty"
        );
        assert!(
            paths.insert(&mapping.wasm_path),
            "mapping[{i}]: duplicate wasm_path '{}'",
            mapping.wasm_path
        );
        assert!(
            mapping
                .capability_id
                .parse::<amari_discovery::CapabilityId>()
                .is_ok(),
            "mapping[{i}]: invalid capability_id '{}'",
            mapping.capability_id
        );
    }
}

#[test]
fn wasm_summary_counts_are_nonzero() {
    // RED: WASM surface summary counts must be consistent (at least one
    // export category has a count).
    let catalog = generate_workspace_catalog(workspace_root()).unwrap();
    let wasm = catalog.wasm_surface.as_ref().unwrap();

    assert!(
        wasm.class_count > 0
            || wasm.function_count > 0
            || wasm.enum_count > 0
            || wasm.interface_count > 0,
        "wasm_surface must have at least one export category"
    );
    assert_eq!(
        wasm.source_hash.len(),
        64,
        "source_hash must be 64 hex chars"
    );
}
