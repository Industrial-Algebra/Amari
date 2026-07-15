// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::HashSet;

use amari_discovery::{Catalog, SideEffectPolicy, StructuralCatalog};

const STRUCTURAL: &str = include_str!("../catalog/generated.json");
const SEMANTIC: &str = include_str!("../catalog/semantic/core.toml");
const PROBES: &str = include_str!("../catalog/probes.toml");

#[test]
fn embedded_catalog_has_unique_valid_capabilities() {
    let catalog = Catalog::embedded().unwrap();
    catalog.validate().unwrap();

    assert!(!catalog.crates().is_empty());
    assert!(catalog.capabilities().len() >= 8);
    let ids: HashSet<_> = catalog
        .capabilities()
        .iter()
        .map(|capability| capability.id.to_string())
        .collect();
    assert_eq!(ids.len(), catalog.capabilities().len());
}

#[test]
fn semantic_capabilities_distinguish_planned_and_implemented_surfaces() {
    let catalog = Catalog::embedded().unwrap();
    assert!(catalog.capabilities().iter().all(|capability| {
        !capability.symbol_refs.is_empty()
            || !capability.example_refs.is_empty()
            || capability
                .description
                .to_ascii_lowercase()
                .contains("planned")
    }));

    let superposition = catalog
        .capabilities()
        .iter()
        .find(|capability| {
            capability.id.to_string() == "amari:amari-holographic:algebra:superposition"
        })
        .expect("implemented superposition capability must be discoverable");
    assert!(!superposition
        .description
        .to_ascii_lowercase()
        .contains("planned"));
    assert_eq!(
        superposition.symbol_refs,
        [
            "amari_holographic::BindingAlgebra::scale",
            "amari_holographic::BindingAlgebra::superpose",
        ]
    );

    let rational = catalog
        .capabilities()
        .iter()
        .find(|capability| {
            capability.id.to_string() == "amari:amari-surreal:rational:exact-arithmetic"
        })
        .expect("rational surreal capability must exist");
    assert!(!rational
        .example_refs
        .iter()
        .any(|example| example == "amari-surreal:dyadic_arithmetic"));
}

#[test]
fn semantic_references_resolve_to_structural_or_probe_records() {
    let catalog = Catalog::embedded().unwrap();
    let crate_names: HashSet<_> = catalog
        .crates()
        .iter()
        .map(|record| record.name.as_str())
        .collect();
    let item_paths: HashSet<_> = catalog
        .crates()
        .iter()
        .flat_map(|record| record.items.iter())
        .map(|item| item.path.as_str())
        .collect();
    let feature_refs: HashSet<_> = catalog
        .crates()
        .iter()
        .flat_map(|record| {
            record
                .features
                .iter()
                .map(|feature| format!("{}:{}", record.name, feature.name))
        })
        .collect();
    let example_refs: HashSet<_> = catalog
        .crates()
        .iter()
        .flat_map(|record| {
            record
                .examples
                .iter()
                .map(|example| format!("{}:{}", record.name, example.name))
        })
        .collect();
    let probe_ids: HashSet<_> = catalog
        .probes()
        .iter()
        .map(|probe| probe.id.to_string())
        .collect();

    for capability in catalog.capabilities() {
        assert!(capability
            .crate_refs
            .iter()
            .all(|name| crate_names.contains(name.as_str())));
        assert!(capability
            .feature_refs
            .iter()
            .all(|reference| feature_refs.contains(reference)));
        assert!(capability
            .symbol_refs
            .iter()
            .all(|path| item_paths.contains(path.as_str())));
        assert!(capability
            .example_refs
            .iter()
            .all(|reference| example_refs.contains(reference)));
        assert!(capability
            .probe_refs
            .iter()
            .all(|id| probe_ids.contains(&id.to_string())));
    }
}

#[test]
fn relations_and_probe_descriptors_are_complete_and_unique() {
    let catalog = Catalog::embedded().unwrap();
    let capability_ids: HashSet<_> = catalog
        .capabilities()
        .iter()
        .map(|capability| capability.id.to_string())
        .collect();
    let mut probe_ids = HashSet::new();

    for relation in catalog.relations() {
        assert!(capability_ids.contains(&relation.from.to_string()));
        assert!(capability_ids.contains(&relation.to.to_string()));
        assert!(!relation.kind.is_empty());
    }

    for probe in catalog.probes() {
        assert!(probe_ids.insert(probe.id.to_string()));
        assert!(capability_ids.contains(&probe.capability_id.to_string()));
        assert!(probe.input_schema.starts_with("amari.discovery/probe/"));
        assert!(probe.input_schema.ends_with("/v1"));
        assert!(probe.output_schema.starts_with("amari.discovery/probe/"));
        assert!(probe.output_schema.ends_with("/v1"));
        assert!(probe.limits.max_input_bytes > 0);
        assert!(probe.limits.max_output_bytes > 0);
        assert!(probe.limits.max_operations > 0);
        assert!(probe.limits.timeout_millis > 0);
        assert!(matches!(probe.side_effects, SideEffectPolicy::None));
        let _declared_cost = probe.cost;
        let _declared_determinism = probe.deterministic;
    }
}

#[test]
fn catalog_hash_is_deterministic_and_content_sensitive() {
    let first = Catalog::embedded().unwrap();
    let second = Catalog::embedded().unwrap();
    assert_eq!(first.content_hash(), second.content_hash());
    assert_eq!(first.content_hash().len(), 64);
    assert!(first
        .content_hash()
        .bytes()
        .all(|byte| byte.is_ascii_hexdigit()));

    // Modify the structural content AND recompute the content_hash so
    // validation passes. The composite Catalog.content_hash must differ.
    let mut changed_structural: StructuralCatalog = serde_json::from_str(STRUCTURAL).unwrap();
    changed_structural.description = "Changed structural catalog for Amari".to_string();
    // Recompute content_hash for the modified structural JSON.
    let mut for_hash = changed_structural.clone();
    for_hash.content_hash = None;
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    let json_without_hash = serde_json::to_vec_pretty(&for_hash).unwrap();
    hasher.update(&json_without_hash);
    changed_structural.content_hash = Some(hex::encode(hasher.finalize()));

    let changed_json = serde_json::to_string_pretty(&changed_structural).unwrap();
    let changed = Catalog::from_sources(&changed_json, SEMANTIC, PROBES).unwrap();
    assert_ne!(first.content_hash(), changed.content_hash());
}

#[test]
fn validation_rejects_wasm_mapping_to_unknown_semantic_capability() {
    let mut structural: StructuralCatalog = serde_json::from_str(STRUCTURAL).unwrap();
    let wasm = structural
        .wasm_surface
        .as_mut()
        .expect("schema 2 fixture must contain WASM summary");
    wasm.capability_mappings[0].capability_id = "amari:missing:module:capability".parse().unwrap();

    let mut for_hash = structural.clone();
    for_hash.content_hash = None;
    use sha2::{Digest, Sha256};
    let canonical = serde_json::to_vec_pretty(&for_hash).unwrap();
    structural.content_hash = Some(hex::encode(Sha256::digest(canonical)));
    let modified = serde_json::to_string_pretty(&structural).unwrap();

    let error = Catalog::from_sources(&modified, SEMANTIC, PROBES)
        .expect_err("unknown semantic mapping must be rejected");
    assert_eq!(error.kind(), "catalog_corruption");
    assert!(error.to_string().contains("unknown capability"));
}

#[test]
fn validation_rejects_dangling_semantic_and_relationship_references() {
    let bad_crate = SEMANTIC.replace(
        "crate_refs = [\"amari-core\"]",
        "crate_refs = [\"amari-missing\"]",
    );
    assert!(Catalog::from_sources(STRUCTURAL, &bad_crate, PROBES).is_err());

    let bad_feature = SEMANTIC.replace("amari-core:std", "amari-core:missing");
    assert!(Catalog::from_sources(STRUCTURAL, &bad_feature, PROBES).is_err());

    let bad_example = SEMANTIC.replace("amari-core:basic", "amari-core:missing");
    assert!(Catalog::from_sources(STRUCTURAL, &bad_example, PROBES).is_err());

    let bad_symbol = SEMANTIC.replace(
        "amari_core::Multivector::geometric_product",
        "amari_core::Missing::operation",
    );
    assert!(Catalog::from_sources(STRUCTURAL, &bad_symbol, PROBES).is_err());

    let bad_probe = SEMANTIC.replace(
        "amari-probe:core:geometric-product:v1",
        "amari-probe:core:missing:v1",
    );
    assert!(Catalog::from_sources(STRUCTURAL, &bad_probe, PROBES).is_err());

    let bad_relation = SEMANTIC.replace(
        "to = \"amari:amari-core:rotor:rotation\"",
        "to = \"amari:missing:module:capability\"",
    );
    assert!(Catalog::from_sources(STRUCTURAL, &bad_relation, PROBES).is_err());
}

#[test]
fn validation_rejects_probe_schema_mismatch_and_wrong_ownership() {
    let wrong_owner = SEMANTIC.replace(
        "probe_refs = [\"amari-probe:core:geometric-product:v1\"]",
        "probe_refs = [\"amari-probe:tropical:viterbi:v1\"]",
    );
    assert!(Catalog::from_sources(STRUCTURAL, &wrong_owner, PROBES).is_err());

    let missing_contract = PROBES.replace(
        "amari.discovery/probe/core-geometric-product/input/v1",
        "amari.discovery/probe/input/v1",
    );
    assert!(Catalog::from_sources(STRUCTURAL, SEMANTIC, &missing_contract).is_err());

    let mismatched_version = PROBES.replace(
        "amari.discovery/probe/core-geometric-product/output/v1",
        "amari.discovery/probe/core-geometric-product/output/v2",
    );
    assert!(Catalog::from_sources(STRUCTURAL, SEMANTIC, &mismatched_version).is_err());

    let mismatched_contract = PROBES.replace(
        "amari.discovery/probe/core-geometric-product/output/v1",
        "amari.discovery/probe/other-contract/output/v1",
    );
    assert!(Catalog::from_sources(STRUCTURAL, SEMANTIC, &mismatched_contract).is_err());
}

#[cfg(feature = "standard-probes")]
#[test]
fn seeded_structural_paths_resolve_to_current_public_apis() {
    type Mv = amari_core::Multivector<3, 0, 0>;
    let _: fn(&Mv, &Mv) -> Mv = Mv::geometric_product;
    let _ = amari_core::Rotor::<3, 0, 0>::identity;
    let _ = amari_tropical::viterbi::TropicalViterbi::<f64>::decode;
    let _: fn(&amari_dual::DualNumber<f64>) -> f64 = amari_dual::DualNumber::<f64>::derivative;
    let _ = amari_network::GeometricNetwork::<3, 0, 0>::shortest_path;
    let _ = amari_optimization::multiobjective::ParetoFront::<f64>::new;
    let _ = amari_holographic::HolographicMemory::<amari_holographic::MAPAlgebra<8>>::retrieve;
    let _: fn(
        &mut amari_cgt::GameArena,
        amari_cgt::GameId,
    ) -> amari_cgt::Result<amari_cgt::Nimber> = amari_cgt::GameArena::grundy;
    let _ = std::mem::size_of::<amari_surreal::RationalSurreal>();
    let _ = std::mem::size_of::<amari_surcomplex::RationalSurcomplex>();
    let _ = amari_rewrite::trs::TermSystem::apply_once;
    let _ = amari_rewrite::synthesis::infer_rule;
}

#[test]
fn validation_rejects_duplicate_capability_and_probe_ids() {
    let duplicate_capability = format!(
        "{SEMANTIC}\n[[capabilities]]\nid = \"amari:amari-core:product:geometric-product\"\nname = \"Duplicate\"\ndescription = \"Duplicate ID\"\naliases = []\nconcepts = []\ncrate_refs = [\"amari-core\"]\nfeature_refs = []\nsymbol_refs = []\nexample_refs = []\nprobe_refs = []\nstability = \"stable\"\ncost = \"low\"\n"
    );
    assert!(Catalog::from_sources(STRUCTURAL, &duplicate_capability, PROBES).is_err());

    let duplicate_probe = format!(
        "{PROBES}\n[[probes]]\nid = \"amari-probe:core:geometric-product:v1\"\ncapability_id = \"amari:amari-core:product:geometric-product\"\ninput_schema = \"amari.discovery/probe/core-geometric-product/input/v1\"\noutput_schema = \"amari.discovery/probe/core-geometric-product/output/v1\"\nrequired_features = []\ncost = \"low\"\ndeterministic = true\nside_effects = \"none\"\n[probes.limits]\nmax_input_bytes = 1\nmax_output_bytes = 1\nmax_operations = 1\ntimeout_millis = 1\n"
    );
    assert!(Catalog::from_sources(STRUCTURAL, SEMANTIC, &duplicate_probe).is_err());
}
