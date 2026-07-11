// SPDX-License-Identifier: MIT OR Apache-2.0

//! Fixture and snapshot tests for the WASM/TypeScript surface parser.
//!
//! # Organization
//!
//! * Fixture tests: parse a curated `.d.ts` covering classes, methods,
//!   constructors, static methods, getters, enums, interfaces, type aliases,
//!   re-export aliases, and nested generic/tuple types.
//!
//! * Real snapshot tests: parse the authoritative `amari_wasm.d.ts` generated
//!   by `wasm-pack` and assert exact classes, methods, and capability
//!   mappings against the checked-in `generated-wasm.json`.

use std::collections::HashSet;

use amari_discovery::{
    default_capability_mappings, parse_wasm_surface, validate_capability_mappings, CapabilityId,
    WasmCapabilityMapping, WasmSurface,
};

const FIXTURE_DTS: &str = include_str!("fixtures/wasm-surface/test.d.ts");
const GENERATED_WASM_JSON: &str = include_str!("../catalog/generated-wasm.json");

// ---------------------------------------------------------------------------
// Fixture tests – parser exhaustiveness
// ---------------------------------------------------------------------------

fn fixture_surface() -> WasmSurface {
    parse_wasm_surface(FIXTURE_DTS).expect("fixture .d.ts must parse")
}

#[test]
fn fixture_parses_correct_class_count() {
    let surface = fixture_surface();
    // WasmMultivector300, WasmGenericMultivector, WasmGenericRotor,
    // WasmRotor300, WasmCountingMeasure, EmptyShell, NestedContainer
    assert_eq!(surface.classes.len(), 7);
}

#[test]
fn fixture_multivector300_has_geometric_product() {
    let surface = fixture_surface();
    let cls = surface
        .classes
        .iter()
        .find(|c| c.name == "WasmMultivector300")
        .expect("WasmMultivector300 must be present");
    assert!(
        cls.methods.iter().any(|m| m.name == "geometricProduct"),
        "geometricProduct must be present"
    );
    assert!(cls.has_free);
    assert!(cls.has_dispose);
}

#[test]
fn fixture_multivector300_has_constructor_and_static_methods() {
    let surface = fixture_surface();
    let cls = surface
        .classes
        .iter()
        .find(|c| c.name == "WasmMultivector300")
        .unwrap();
    // Explicit public constructor (no `private constructor()`)
    assert!(!cls.private_constructor);
    assert!(cls.constructor_signature.as_ref().unwrap().contains("()"));
    let static_names: Vec<_> = cls.static_methods.iter().map(|m| m.name.as_str()).collect();
    assert!(static_names.contains(&"basisVector"));
    assert!(static_names.contains(&"fromCoefficients"));
    assert!(static_names.contains(&"scalar"));
}

#[test]
fn fixture_multivector300_has_getter() {
    let surface = fixture_surface();
    let cls = surface
        .classes
        .iter()
        .find(|c| c.name == "WasmMultivector300")
        .unwrap();
    let getter_names: Vec<_> = cls.getters.iter().map(|g| g.name.as_str()).collect();
    assert!(getter_names.contains(&"dim"));
}

#[test]
fn fixture_generic_multivector_has_geometric_product_and_constructor_with_params() {
    let surface = fixture_surface();
    let cls = surface
        .classes
        .iter()
        .find(|c| c.name == "WasmGenericMultivector")
        .expect("WasmGenericMultivector must be present");
    assert!(
        cls.methods.iter().any(|m| m.name == "geometricProduct"),
        "geometricProduct on generic multivector"
    );
    // Explicit public constructor with (p, q, r) parameters
    assert!(!cls.private_constructor);
    let sig = cls.constructor_signature.as_ref().unwrap();
    assert!(sig.contains("p: number, q: number, r: number"), "{sig}");
}

#[test]
fn fixture_rotor300_has_private_constructor_and_apply() {
    let surface = fixture_surface();
    let cls = surface
        .classes
        .iter()
        .find(|c| c.name == "WasmRotor300")
        .expect("WasmRotor300 must be present");
    assert!(cls.private_constructor);
    assert!(
        cls.static_methods.iter().any(|m| m.name == "fromBivector"),
        "fromBivector static method"
    );
    assert!(
        cls.methods.iter().any(|m| m.name == "apply"),
        "apply method"
    );
}

#[test]
fn fixture_generic_rotor_has_apply_and_compose() {
    let surface = fixture_surface();
    let cls = surface
        .classes
        .iter()
        .find(|c| c.name == "WasmGenericRotor")
        .expect("WasmGenericRotor must be present");
    assert!(cls.private_constructor);
    let method_names: Vec<_> = cls.methods.iter().map(|m| m.name.as_str()).collect();
    assert!(method_names.contains(&"apply"));
    assert!(method_names.contains(&"compose"));
    assert!(method_names.contains(&"inverse"));
}

#[test]
fn fixture_empty_class_has_no_methods_but_has_free_and_dispose() {
    let surface = fixture_surface();
    let cls = surface
        .classes
        .iter()
        .find(|c| c.name == "EmptyShell")
        .expect("EmptyShell must be present");
    assert!(cls.private_constructor);
    assert!(cls.methods.is_empty());
    assert!(cls.static_methods.is_empty());
    assert!(cls.getters.is_empty());
    assert!(cls.has_free);
    assert!(cls.has_dispose);
}

#[test]
fn fixture_nested_container_has_nested_types_in_signatures() {
    let surface = fixture_surface();
    let cls = surface
        .classes
        .iter()
        .find(|c| c.name == "NestedContainer")
        .expect("NestedContainer must be present");
    let map_method = cls
        .methods
        .iter()
        .find(|m| m.name == "mapTransform")
        .expect("mapTransform must be present");
    assert!(map_method
        .signature
        .contains("Map<string, Array<Float64Array>>"));
    let tuple_method = cls
        .methods
        .iter()
        .find(|m| m.name == "tupleResult")
        .expect("tupleResult must be present");
    assert!(tuple_method.signature.contains("[number, string, boolean]"));
}

#[test]
fn fixture_parses_enum_with_variants() {
    let surface = fixture_surface();
    let enum_ = surface
        .enums
        .iter()
        .find(|e| e.name == "WasmIntegrationMethod")
        .expect("WasmIntegrationMethod enum must be present");
    assert_eq!(enum_.variants.len(), 3);
    let variant_names: Vec<_> = enum_.variants.iter().map(|v| v.name.as_str()).collect();
    assert!(variant_names.contains(&"Riemann"));
    assert!(variant_names.contains(&"MonteCarlo"));
    assert!(variant_names.contains(&"Trapezoidal"));
    assert_eq!(
        enum_
            .variants
            .iter()
            .find(|v| v.name == "Riemann")
            .unwrap()
            .value,
        0
    );
    assert_eq!(
        enum_
            .variants
            .iter()
            .find(|v| v.name == "MonteCarlo")
            .unwrap()
            .value,
        1
    );
}

#[test]
fn fixture_parses_top_level_functions() {
    let surface = fixture_surface();
    assert_eq!(surface.functions.len(), 3);
    let names: Vec<_> = surface.functions.iter().map(|f| f.name.as_str()).collect();
    assert!(names.contains(&"init"));
    assert!(names.contains(&"expectation"));
    assert!(names.contains(&"velocity_to_gamma"));
}

#[test]
fn fixture_parses_type_aliases() {
    let surface = fixture_surface();
    assert_eq!(surface.type_aliases.len(), 3); // InitInput, SyncInitInput, GenericRotor alias
    let names: Vec<_> = surface
        .type_aliases
        .iter()
        .map(|a| a.name.as_str())
        .collect();
    assert!(names.contains(&"InitInput"));
    assert!(names.contains(&"SyncInitInput"));
    assert!(names.contains(&"GenericRotor"));
}

#[test]
fn fixture_parses_re_export_alias() {
    let surface = fixture_surface();
    let alias = surface
        .type_aliases
        .iter()
        .find(|a| a.name == "GenericRotor")
        .expect("GenericRotor re-export alias must be present");
    assert_eq!(alias.target, "WasmGenericRotor");
}

#[test]
fn fixture_parses_interface_with_readonly_members() {
    let surface = fixture_surface();
    let iface = surface
        .interfaces
        .iter()
        .find(|i| i.name == "InitOutput")
        .expect("InitOutput interface must be present");
    assert!(!iface.members.is_empty());
}

#[test]
fn fixture_produces_deterministic_hash() {
    let surface1 = parse_wasm_surface(FIXTURE_DTS).unwrap();
    let surface2 = parse_wasm_surface(FIXTURE_DTS).unwrap();
    assert_eq!(surface1.source_hash, surface2.source_hash);
    assert!(!surface1.source_hash.is_empty());
    assert_eq!(surface1.source_hash.len(), 64);
}

#[test]
fn fixture_sorted_output() {
    let surface = fixture_surface();
    // classes must be sorted by name
    let names: Vec<_> = surface.classes.iter().map(|c| c.name.as_str()).collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted, "classes must be sorted alphabetically");
    // methods within each class must be sorted
    for cls in &surface.classes {
        let method_names: Vec<_> = cls.methods.iter().map(|m| m.name.as_str()).collect();
        let mut sorted_methods = method_names.clone();
        sorted_methods.sort();
        assert_eq!(
            method_names, sorted_methods,
            "methods in {} must be sorted",
            cls.name
        );
    }
}

#[test]
fn fixture_methods_have_normalized_signatures() {
    let surface = fixture_surface();
    let cls = surface
        .classes
        .iter()
        .find(|c| c.name == "WasmMultivector300")
        .unwrap();
    let gp = cls
        .methods
        .iter()
        .find(|m| m.name == "geometricProduct")
        .unwrap();
    // Signature should be clean and normalized
    assert!(!gp.signature.contains("  "), "no double spaces");
    assert!(!gp.signature.contains("( "), "no space after paren");
    assert!(!gp.signature.contains(" )"), "no space before paren");
    assert!(gp
        .signature
        .contains("(other: WasmMultivector300): WasmMultivector300"));
}

#[test]
fn fixture_docs_attached_to_class() {
    let surface = fixture_surface();
    let cls = surface
        .classes
        .iter()
        .find(|c| c.name == "WasmMultivector300")
        .unwrap();
    assert!(cls.doc.as_ref().unwrap().contains("fast-path multivector"));
    assert!(cls.doc.as_ref().unwrap().contains("Cl(3,0,0)"));
}

#[test]
fn fixture_method_docs_attached() {
    let surface = fixture_surface();
    let cls = surface
        .classes
        .iter()
        .find(|c| c.name == "WasmMultivector300")
        .unwrap();
    let gp = cls
        .methods
        .iter()
        .find(|m| m.name == "geometricProduct")
        .unwrap();
    assert!(gp.doc.as_ref().unwrap().contains("geometric product"));
}

#[test]
fn parse_empty_string_is_error() {
    let err = parse_wasm_surface("").unwrap_err();
    assert!(err.to_string().contains("empty"));
}

#[test]
fn parse_only_comments_is_error() {
    let err = parse_wasm_surface("/* nothing */\n// also nothing\n").unwrap_err();
    assert!(err.to_string().contains("no recognizable"));
}

// ---------------------------------------------------------------------------
// Capability mapping validation
// ---------------------------------------------------------------------------

#[test]
fn capability_mapping_validation_keeps_valid_rejects_invalid() {
    let src = "export class Foo { bar(): number; }";
    let mut surface = parse_wasm_surface(src).unwrap();
    let valid: CapabilityId = "amari:amari-core:product:geometric-product"
        .parse()
        .unwrap();
    let invalid: CapabilityId = "amari:fake:bogus:symbol".parse().unwrap();
    surface.capability_mappings = vec![
        WasmCapabilityMapping {
            wasm_path: "Foo.bar".into(),
            capability_id: valid.clone(),
        },
        WasmCapabilityMapping {
            wasm_path: "Foo.bogus".into(),
            capability_id: invalid,
        },
    ];

    let validated = validate_capability_mappings(surface, &[valid]);
    assert_eq!(validated.capability_mappings.len(), 1);
    assert_eq!(validated.capability_mappings[0].wasm_path, "Foo.bar");
    assert!(
        validated
            .warnings
            .iter()
            .any(|w| w.kind == "invalid_capability_mapping"),
        "should warn about invalid mapping"
    );
}

#[test]
fn wasm_surface_serialization_roundtrips() {
    let surface = fixture_surface();
    let json = serde_json::to_string_pretty(&surface).unwrap();
    let parsed: WasmSurface = serde_json::from_str(&json).unwrap();
    assert_eq!(surface, parsed);
}

// ---------------------------------------------------------------------------
// Real snapshot tests — checked-in generated-wasm.json
// ---------------------------------------------------------------------------

fn generated_surface() -> WasmSurface {
    serde_json::from_str(GENERATED_WASM_JSON).expect("checked-in generated-wasm.json must parse")
}

#[test]
fn generated_has_valid_schema_version() {
    let surface = generated_surface();
    assert_eq!(surface.schema_version, 1);
}

#[test]
fn generated_has_source_hash() {
    let surface = generated_surface();
    assert_eq!(surface.source_hash.len(), 64);
    // Must be valid hex
    hex::decode(&surface.source_hash).expect("source_hash must be valid hex");
}

#[test]
fn generated_has_wams_multivector300_with_geometric_product() {
    let surface = generated_surface();
    let cls = surface
        .classes
        .iter()
        .find(|c| c.name == "WasmMultivector300")
        .expect("WasmMultivector300 must be in the authoritative surface");
    assert!(
        cls.methods.iter().any(|m| m.name == "geometricProduct"),
        "WasmMultivector300.geometricProduct must be present"
    );
}

#[test]
fn generated_has_wams_generic_multivector_with_geometric_product() {
    let surface = generated_surface();
    let cls = surface
        .classes
        .iter()
        .find(|c| c.name == "WasmGenericMultivector")
        .expect("WasmGenericMultivector must be in the authoritative surface");
    assert!(
        cls.methods.iter().any(|m| m.name == "geometricProduct"),
        "WasmGenericMultivector.geometricProduct must be present"
    );
}

#[test]
fn generated_has_all_fast_path_multivector_classes() {
    let surface = generated_surface();
    let expected: HashSet<_> = [
        "WasmMultivector030",
        "WasmMultivector110",
        "WasmMultivector200",
        "WasmMultivector210",
        "WasmMultivector300",
        "WasmMultivector310",
        "WasmMultivector410",
        "WasmMultivector500",
    ]
    .iter()
    .copied()
    .collect();
    let actual: HashSet<_> = surface
        .classes
        .iter()
        .filter(|c| c.name.starts_with("WasmMultivector"))
        .map(|c| c.name.as_str())
        .collect();
    for name in &expected {
        assert!(
            actual.contains(name),
            "{name} must be in the authoritative surface"
        );
    }
    // NOTE: each fast-path class should have geometricProduct
    for cls in surface
        .classes
        .iter()
        .filter(|c| expected.contains(c.name.as_str()))
    {
        assert!(
            cls.methods.iter().any(|m| m.name == "geometricProduct"),
            "{} must have geometricProduct",
            cls.name
        );
    }
}

#[test]
fn generated_has_rotor_fast_path_classes() {
    let surface = generated_surface();
    let expected: HashSet<_> = [
        "WasmRotor030",
        "WasmRotor110",
        "WasmRotor200",
        "WasmRotor210",
        "WasmRotor300",
        "WasmRotor310",
        "WasmRotor410",
        "WasmRotor500",
    ]
    .iter()
    .copied()
    .collect();
    let actual: HashSet<_> = surface
        .classes
        .iter()
        .filter(|c| c.name.starts_with("WasmRotor"))
        .map(|c| c.name.as_str())
        .collect();
    for name in &expected {
        assert!(
            actual.contains(name),
            "{name} must be in the authoritative surface"
        );
    }
    // Rotor classes should have private constructors + apply method
    for cls in surface
        .classes
        .iter()
        .filter(|c| expected.contains(c.name.as_str()))
    {
        assert!(
            cls.private_constructor,
            "{} should have a private constructor",
            cls.name
        );
        assert!(
            cls.methods.iter().any(|m| m.name == "apply"),
            "{} should have an apply method",
            cls.name
        );
    }
}

#[test]
fn generated_has_wams_generic_rotor() {
    let surface = generated_surface();
    let cls = surface
        .classes
        .iter()
        .find(|c| c.name == "WasmGenericRotor")
        .expect("WasmGenericRotor must be in the authoritative surface");
    assert!(cls.private_constructor);
    assert!(cls.methods.iter().any(|m| m.name == "apply"));
    assert!(cls.methods.iter().any(|m| m.name == "compose"));
    assert!(cls.methods.iter().any(|m| m.name == "inverse"));
}

#[test]
fn generated_does_not_contain_rust_only_legacy_aliases() {
    let surface = generated_surface();
    // WasmMultivector, WasmRotor are Rust-only type aliases that should
    // NOT appear in the authoritative .d.ts unless wasm-bindgen emits them.
    for cls in &surface.classes {
        assert_ne!(
            cls.name, "WasmMultivector",
            "WasmMultivector (Rust-only alias) must not appear in authoritative WASM surface"
        );
        assert_ne!(
            cls.name, "WasmRotor",
            "WasmRotor (Rust-only alias) must not appear in authoritative WASM surface"
        );
    }
    // Similarly for README-only constants
    for func in &surface.functions {
        assert_ne!(
            func.name, "GA",
            "GA constant must not appear in authoritative WASM surface unless generated"
        );
        assert_ne!(
            func.name, "ST",
            "ST constant must not appear in authoritative WASM surface unless generated"
        );
        assert_ne!(
            func.name, "MINK",
            "MINK constant must not appear in authoritative WASM surface unless generated"
        );
    }
}

// ---------------------------------------------------------------------------
// Snapshot tests — capability mappings
// ---------------------------------------------------------------------------

/// The canonical set of CapabilityIds that the built-in mapper is allowed
/// to emit.  This must stay in sync with [`default_capability_mappings`].
fn canonical_ids() -> Vec<CapabilityId> {
    vec![
        "amari:amari-core:product:geometric-product"
            .parse()
            .expect("invalid canonical ID in test"),
        "amari:amari-core:rotor:rotation"
            .parse()
            .expect("invalid canonical ID in test"),
    ]
}

#[test]
fn generated_capability_mappings_are_non_empty() {
    let surface = generated_surface();
    assert!(
        !surface.capability_mappings.is_empty(),
        "generated-wasm.json must contain non-empty capability_mappings"
    );
}

#[test]
fn generated_capability_mappings_have_every_geometric_product() {
    let surface = generated_surface();

    // Compute expected count from the surface itself
    let expected: Vec<_> = default_capability_mappings(&surface)
        .unwrap()
        .into_iter()
        .filter(|m| m.capability_id.to_string().contains("geometric-product"))
        .collect();

    assert!(
        !expected.is_empty(),
        "must map at least one geometricProduct export"
    );

    // Every expected mapping MUST appear in the snapshot
    let snapshot_paths: std::collections::HashSet<_> = surface
        .capability_mappings
        .iter()
        .map(|m| &m.wasm_path)
        .collect();

    for m in &expected {
        assert!(
            snapshot_paths.contains(&m.wasm_path),
            "snapshot missing expected mapping: {}",
            m.wasm_path
        );
    }

    // The snapshot should contain exactly the expected geometricProduct
    // mappings (not more, not less)
    let snapshot_gp_count = surface
        .capability_mappings
        .iter()
        .filter(|m| m.capability_id.to_string().contains("geometric-product"))
        .count();
    assert_eq!(
        snapshot_gp_count,
        expected.len(),
        "snapshot geometric-product count must match expected"
    );
}

#[test]
fn generated_capability_mappings_have_every_rotor_apply() {
    let surface = generated_surface();

    let expected: Vec<_> = default_capability_mappings(&surface)
        .unwrap()
        .into_iter()
        .filter(|m| m.capability_id.to_string().contains("rotation"))
        .collect();

    assert!(
        !expected.is_empty(),
        "must map at least one rotor apply export"
    );

    let snapshot_paths: std::collections::HashSet<_> = surface
        .capability_mappings
        .iter()
        .map(|m| &m.wasm_path)
        .collect();

    for m in &expected {
        assert!(
            snapshot_paths.contains(&m.wasm_path),
            "snapshot missing expected mapping: {}",
            m.wasm_path
        );
    }

    let snapshot_rot_count = surface
        .capability_mappings
        .iter()
        .filter(|m| m.capability_id.to_string().contains("rotation"))
        .count();
    assert_eq!(
        snapshot_rot_count,
        expected.len(),
        "snapshot rotation count must match expected"
    );
}

#[test]
fn generated_capability_mappings_only_use_canonical_ids() {
    let surface = generated_surface();
    let valid_ids: std::collections::HashSet<_> = canonical_ids().into_iter().collect();

    for mapping in &surface.capability_mappings {
        assert!(
            valid_ids.contains(&mapping.capability_id),
            "capability ID {} is not in the known canonical set",
            mapping.capability_id
        );
    }
}

#[test]
fn generated_capability_mappings_wasm_paths_exist_in_surface() {
    let surface = generated_surface();
    for mapping in &surface.capability_mappings {
        let parts: Vec<_> = mapping.wasm_path.split('.').collect();
        assert_eq!(
            parts.len(),
            2,
            "wasm_path must be Class.method; got: {}",
            mapping.wasm_path
        );
        let cls_name = parts[0];
        let method_name = parts[1];
        let cls = surface
            .classes
            .iter()
            .find(|c| c.name == cls_name)
            .unwrap_or_else(|| {
                panic!(
                    "mapped class {cls_name} must exist in surface (from {})",
                    mapping.wasm_path
                )
            });
        assert!(
            cls.methods.iter().any(|m| m.name == method_name),
            "mapped method {cls_name}.{method_name} must exist"
        );
    }
}

#[test]
fn generated_capability_mappings_are_sorted_and_deduped() {
    let surface = generated_surface();
    let paths: Vec<_> = surface
        .capability_mappings
        .iter()
        .map(|m| m.wasm_path.as_str())
        .collect();
    let mut sorted = paths.clone();
    sorted.sort();
    assert_eq!(paths, sorted, "capability mappings must be sorted");

    let unique: std::collections::HashSet<_> = paths.iter().collect();
    assert_eq!(
        paths.len(),
        unique.len(),
        "capability mappings must be deduplicated"
    );
}

#[test]
fn generated_capability_mappings_count_is_exact() {
    let surface = generated_surface();
    let expected = default_capability_mappings(&surface).unwrap();
    assert_eq!(
        surface.capability_mappings.len(),
        expected.len(),
        "snapshot mapping count must equal computed default count ({} vs {})",
        surface.capability_mappings.len(),
        expected.len(),
    );
}

// ---------------------------------------------------------------------------
// Fixture test — parser vs enrichment phase boundary
// ---------------------------------------------------------------------------

/// The fixture parser output is deliberately unmapped (the parser does not
/// enrich with capability mappings).  This test documents that boundary.
/// `default_capability_mappings` is the enrichment step called separately.
#[test]
fn fixture_capability_mappings_are_enrichment_not_parsing() {
    let surface = fixture_surface();
    // Parser never populates capability_mappings
    assert!(surface.capability_mappings.is_empty());

    // But enrichment recognises matching exports in the fixture
    let mappings = default_capability_mappings(&surface).unwrap();
    // The fixture has WasmMultivector300.geometricProduct and
    // WasmGenericMultivector.geometricProduct, plus WasmGenericRotor.apply
    // and WasmRotor300.apply — enrichment should find them
    assert!(!mappings.is_empty());

    let gp_paths: Vec<_> = mappings
        .iter()
        .filter(|m| m.capability_id.to_string().contains("geometric-product"))
        .map(|m| m.wasm_path.as_str())
        .collect();
    assert!(gp_paths.contains(&"WasmMultivector300.geometricProduct"));
    assert!(gp_paths.contains(&"WasmGenericMultivector.geometricProduct"));

    let rot_paths: Vec<_> = mappings
        .iter()
        .filter(|m| m.capability_id.to_string().contains("rotation"))
        .map(|m| m.wasm_path.as_str())
        .collect();
    assert!(rot_paths.contains(&"WasmRotor300.apply"));
    assert!(rot_paths.contains(&"WasmGenericRotor.apply"));
}

#[test]
fn generated_no_class_has_private_method_that_should_be_public() {
    let surface = generated_surface();
    // Basic sanity: no class should declare free() as a regular method
    for cls in &surface.classes {
        assert!(
            !cls.methods.iter().any(|m| m.name == "free"),
            "{} should not record free() as a regular method (it's parsed as a flag)",
            cls.name
        );
        assert!(
            !cls.methods.iter().any(|m| m.name == "constructor"),
            "constructor should not be a regular method"
        );
    }
}

#[test]
fn generated_surface_is_sorted_and_deduped() {
    let surface = generated_surface();
    // All arrays must be sorted
    let class_names: Vec<_> = surface.classes.iter().map(|c| c.name.as_str()).collect();
    let mut sorted_names = class_names.clone();
    sorted_names.sort();
    assert_eq!(class_names, sorted_names, "classes must be sorted");

    let func_names: Vec<_> = surface.functions.iter().map(|f| f.name.as_str()).collect();
    let mut sorted_funcs = func_names.clone();
    sorted_funcs.sort();
    assert_eq!(func_names, sorted_funcs, "functions must be sorted");

    // Warnings must be deduped
    let warns: Vec<_> = surface.warnings.iter().map(|w| &w.message).collect();
    let unique: HashSet<_> = warns.iter().collect();
    assert_eq!(warns.len(), unique.len(), "warnings must be deduplicated");
}
