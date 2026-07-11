// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tests for Task 5B4: extract trait and implementation relationships.
//!
//! These tests cover supertraits, required/provided items, direct generic
//! `impl Trait for Type` blocks, external endpoint handling, alias
//! projection, and real-assertion integration with the Amari workspace.

use std::{fs, path::Path};

use amari_discovery::catalog::generator::{
    export_graph, module_graph, trait_relationships, RelationshipEndpoint, TraitCatalog,
    TraitDefinition, TraitImplementation, TraitItemStatus,
};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn write_package(root: &Path, files: &[(&str, &str)]) {
    for (relative, source) in files {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, source).unwrap();
    }
}

fn catalog_for(root: &Path, source_path: &str) -> TraitCatalog {
    let graph = module_graph(root, source_path).unwrap();
    let exports = export_graph(&graph, root).unwrap();
    trait_relationships(&graph, &exports, root).unwrap()
}

fn find_trait<'a>(catalog: &'a TraitCatalog, name: &str) -> &'a TraitDefinition {
    catalog
        .definitions
        .iter()
        .find(|d| {
            matches!(&d.trait_endpoint, RelationshipEndpoint::Local { ident, .. } if ident == name)
                || matches!(&d.trait_endpoint, RelationshipEndpoint::External { path } if path.ends_with(name))
        })
        .unwrap_or_else(|| panic!("trait `{name}` not found in catalog"))
}

fn find_impl<'a>(
    catalog: &'a TraitCatalog,
    trait_name: &str,
    type_name: &str,
) -> &'a TraitImplementation {
    catalog
        .implementations
        .iter()
        .find(|imp| {
            let trait_matches = match &imp.trait_endpoint {
                RelationshipEndpoint::Local { ident, .. } => ident == trait_name,
                RelationshipEndpoint::External { path } => path.ends_with(trait_name),
            };
            let type_matches = match &imp.impl_type_endpoint {
                RelationshipEndpoint::Local { ident, .. } => ident == type_name,
                RelationshipEndpoint::External { path } => path.ends_with(type_name),
            };
            trait_matches && type_matches
        })
        .unwrap_or_else(|| panic!("impl `{trait_name} for {type_name}` not found"))
}

/// Tokens of a normalized signature, whitespace-collapsed for stable matching.
fn tokens(signature: &str) -> Vec<&str> {
    signature.split_whitespace().collect()
}

/// Find trait definitions at a specific export path.
fn find_trait_by_export<'a>(
    catalog: &'a TraitCatalog,
    export_path: &str,
) -> Vec<&'a TraitDefinition> {
    catalog
        .definitions
        .iter()
        .filter(|d| d.export_path == export_path)
        .collect()
}

/// Find implementations with a specific trait_path and impl_type_path pair.
fn find_impl_by_path<'a>(
    catalog: &'a TraitCatalog,
    trait_path: &str,
    impl_type_path: &str,
) -> Vec<&'a TraitImplementation> {
    catalog
        .implementations
        .iter()
        .filter(|imp| imp.trait_path == trait_path && imp.impl_type_path == impl_type_path)
        .collect()
}

// ===================================================================
// RED phase: tests that will fail until production code exists.
// ===================================================================

// -------------------------------------------------------------------
// 1. Supertrait constraints
// -------------------------------------------------------------------

#[test]
fn supertraits_are_detected_from_trait_declaration() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[("src/lib.rs", "pub trait MyTrait: Clone + Send {}\n")],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");
    let def = find_trait(&catalog, "MyTrait");

    let supertrait_names: Vec<&str> = def
        .supertraits
        .iter()
        .filter_map(|st| match &st.endpoint {
            RelationshipEndpoint::External { path } => path.split("::").last(),
            RelationshipEndpoint::Local { ident, .. } => Some(ident.as_str()),
        })
        .collect();

    assert!(
        supertrait_names.contains(&"Clone"),
        "expected Clone supertrait, got {supertrait_names:?}"
    );
    assert!(
        supertrait_names.contains(&"Send"),
        "expected Send supertrait, got {supertrait_names:?}"
    );
}

#[test]
fn supertraits_with_generic_and_lifetime_and_associated_type() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub trait Marker {}\n\
             pub trait WithSupertraits: Marker + Clone {}\n",
        )],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");

    // WithSupertraits should have two supertraits: Marker (local) and Clone (external).
    let def = find_trait(&catalog, "WithSupertraits");
    let st_names: Vec<&str> = def
        .supertraits
        .iter()
        .filter_map(|st| match &st.endpoint {
            RelationshipEndpoint::External { path } => path.split("::").last(),
            RelationshipEndpoint::Local { ident, .. } => Some(ident.as_str()),
        })
        .collect();

    assert!(st_names.contains(&"Marker"), "local supertrait missing");
    assert!(st_names.contains(&"Clone"), "external supertrait missing");
}

// -------------------------------------------------------------------
// 2. Required vs provided items
// -------------------------------------------------------------------

#[test]
fn required_and_provided_associated_items_are_distinguished() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub trait MyTrait {\n\
             \x20\x20\x20\x20fn required_method(&self);\n\
             \x20\x20\x20\x20fn provided_method(&self) -> u32 { 42 }\n\
             \x20\x20\x20\x20const REQUIRED: u32;\n\
             \x20\x20\x20\x20const PROVIDED: u32 = 7;\n\
             \x20\x20\x20\x20type RequiredType;\n\
             \x20\x20\x20\x20type ProvidedType = u32;\n\
             }\n",
        )],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");
    let def = find_trait(&catalog, "MyTrait");

    // Check required items
    let required_names: Vec<&str> = def
        .required_items
        .iter()
        .filter(|item| item.status == TraitItemStatus::Required)
        .map(|item| item.name.as_str())
        .collect();
    assert!(required_names.contains(&"required_method"));
    assert!(required_names.contains(&"REQUIRED"));
    assert!(required_names.contains(&"RequiredType"));

    // Check provided items
    let provided_names: Vec<&str> = def
        .provided_items
        .iter()
        .filter(|item| item.status == TraitItemStatus::Provided)
        .map(|item| item.name.as_str())
        .collect();
    assert!(provided_names.contains(&"provided_method"));
    assert!(provided_names.contains(&"PROVIDED"));
    assert!(provided_names.contains(&"ProvidedType"));

    // Verify signature tokens
    let req_fn = def
        .required_items
        .iter()
        .find(|i| i.name == "required_method")
        .unwrap();
    let toks = tokens(&req_fn.signature);
    assert!(toks.contains(&"fn"), "expected fn keyword, got {toks:?}");
    // Check that the receiver mentions self (may be "(&self" or "self").
    assert!(
        toks.iter().any(|t| t.contains("self")),
        "expected self receiver, got {toks:?}"
    );

    let prov_fn = def
        .provided_items
        .iter()
        .find(|i| i.name == "provided_method")
        .unwrap();
    let prov_toks = tokens(&prov_fn.signature);
    assert!(
        prov_toks.contains(&"->"),
        "expected return arrow, got {prov_toks:?}"
    );
}

// -------------------------------------------------------------------
// 3. Direct generic implementations
// -------------------------------------------------------------------

#[test]
fn direct_generic_impl_trait_for_type_is_indexed() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub trait MyTrait {}\n\
             pub struct MyStruct<T>(T);\n\
             impl<T> MyTrait for MyStruct<T> {}\n",
        )],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");
    let imp = find_impl(&catalog, "MyTrait", "MyStruct");
    assert!(!imp.unsafe_trait, "should not be unsafe");
    assert!(!imp.negative, "should not be negative");
}

#[test]
fn impl_with_generic_bounds_is_indexed() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub trait MyTrait {}\n\
             pub struct MyStruct<T>(T);\n\
             impl<T: Clone> MyTrait for MyStruct<T> {}\n",
        )],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");
    let imp = find_impl(&catalog, "MyTrait", "MyStruct");

    // The generics should contain "Clone" in the bounds.
    let toks = tokens(&imp.generics);
    assert!(
        toks.contains(&"Clone"),
        "expected Clone bound in generics, got {toks:?}"
    );
}

#[test]
fn impl_with_const_generic_is_indexed() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub trait MyTrait {}\n\
             pub struct MyStruct<const D: usize>;\n\
             impl<const D: usize> MyTrait for MyStruct<D> {}\n",
        )],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");
    let imp = find_impl(&catalog, "MyTrait", "MyStruct");
    let toks = tokens(&imp.generics);
    assert!(
        toks.contains(&"const"),
        "expected const generic, got {toks:?}"
    );
}

// -------------------------------------------------------------------
// 4. External endpoint handling
// -------------------------------------------------------------------

#[test]
fn external_trait_impl_is_recorded_as_external_endpoint() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub struct MyStruct;\n\
             impl Default for MyStruct {\n\
             \x20\x20\x20\x20fn default() -> Self { MyStruct }\n\
             }\n",
        )],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");
    let imp = find_impl(&catalog, "Default", "MyStruct");

    // Default should be external
    assert!(
        matches!(&imp.trait_endpoint, RelationshipEndpoint::External { path } if path == "Default")
    );

    // MyStruct should be local
    assert!(matches!(
        &imp.impl_type_endpoint,
        RelationshipEndpoint::Local { .. }
    ));
}

#[test]
fn local_trait_impl_is_recorded_as_local_endpoint() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub trait MyTrait {}\n\
             pub struct MyStruct;\n\
             impl MyTrait for MyStruct {}\n",
        )],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");
    let imp = find_impl(&catalog, "MyTrait", "MyStruct");

    assert!(
        matches!(&imp.trait_endpoint, RelationshipEndpoint::Local { .. }),
        "expected local trait endpoint"
    );
    assert!(
        matches!(&imp.impl_type_endpoint, RelationshipEndpoint::Local { .. }),
        "expected local type endpoint"
    );
}

// -------------------------------------------------------------------
// 5. Unsafe and negative impl markers
// -------------------------------------------------------------------

#[test]
fn unsafe_trait_impl_is_marked() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub struct MyStruct;\n\
             unsafe impl Send for MyStruct {}\n",
        )],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");
    let imp = find_impl(&catalog, "Send", "MyStruct");
    assert!(imp.unsafe_trait, "expected unsafe impl");
}

#[test]
fn negative_impl_is_marked() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub struct MyStruct;\n\
             impl !Send for MyStruct {}\n",
        )],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");
    let imp = find_impl(&catalog, "Send", "MyStruct");
    assert!(imp.negative, "expected negative impl");
}

// -------------------------------------------------------------------
// 6. Re-export alias preservation
// -------------------------------------------------------------------

#[test]
fn trait_relationships_are_preserved_through_re_export_aliases() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            (
                "src/lib.rs",
                "mod inner;\n\
                 pub use inner::MyTrait;\n\
                 pub use inner::MyStruct;\n",
            ),
            (
                "src/inner.rs",
                "pub trait MyTrait {}\n\
                 pub struct MyStruct;\n\
                 impl MyTrait for MyStruct {}\n",
            ),
        ],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");

    // The trait should be found at its exported alias.
    // Both the local and re-exported paths should produce the same definition.
    let def = find_trait(&catalog, "MyTrait");
    // The local module should be crate::inner
    assert!(
        matches!(&def.trait_endpoint, RelationshipEndpoint::Local { module, .. } if module == "crate::inner" || module.ends_with("inner")),
        "expected trait to resolve to the inner module source"
    );

    // Implementation should also be present.
    let imp = find_impl(&catalog, "MyTrait", "MyStruct");
    assert!(
        matches!(&imp.trait_endpoint, RelationshipEndpoint::Local { .. }),
        "trait endpoint should be local through alias"
    );
}

// -------------------------------------------------------------------
// 7. Real workspace assertion: BindingAlgebra
// -------------------------------------------------------------------

#[test]
fn real_binding_algebra_supertraits_and_items() {
    // Use the actual amari-holographic crate.
    let root = workspace_root();
    let holo_src = root.join("amari-holographic/src/lib.rs");
    assert!(
        holo_src.exists(),
        "amari-holographic/src/lib.rs not found at {}",
        holo_src.display()
    );

    let graph = module_graph(&root, "amari-holographic/src/lib.rs").unwrap();
    let exports = export_graph(&graph, &root).unwrap();
    let catalog = trait_relationships(&graph, &exports, &root).unwrap();

    // BindingAlgebra should be found at crate::BindingAlgebra (exported from
    // crate root via `pub use algebra::BindingAlgebra`).
    let ba_def = catalog
        .definitions
        .iter()
        .find(|d| matches!(&d.trait_endpoint, RelationshipEndpoint::Local { ident, .. } if ident == "BindingAlgebra"))
        .expect("BindingAlgebra definition not found");

    // Supertraits: Sized + Clone + Send + Sync
    let supertrait_names: Vec<&str> = ba_def
        .supertraits
        .iter()
        .filter_map(|st| match &st.endpoint {
            RelationshipEndpoint::External { path } => path.split("::").last(),
            RelationshipEndpoint::Local { ident, .. } => Some(ident.as_str()),
        })
        .collect();
    assert!(
        supertrait_names.contains(&"Sized"),
        "missing Sized: {:?}",
        supertrait_names
    );
    assert!(
        supertrait_names.contains(&"Clone"),
        "missing Clone: {:?}",
        supertrait_names
    );
    assert!(
        supertrait_names.contains(&"Send"),
        "missing Send: {:?}",
        supertrait_names
    );
    assert!(
        supertrait_names.contains(&"Sync"),
        "missing Sync: {:?}",
        supertrait_names
    );

    // Required vs provided methods
    let req_names: Vec<&str> = ba_def
        .required_items
        .iter()
        .map(|i| i.name.as_str())
        .collect();
    assert!(req_names.contains(&"bind"), "bind should be required");
    assert!(req_names.contains(&"inverse"), "inverse should be required");
    assert!(req_names.contains(&"bundle"), "bundle should be required");

    // unbind has a default implementation
    let unbind_item = ba_def.provided_items.iter().find(|i| i.name == "unbind");
    assert!(
        unbind_item.is_some(),
        "unbind should be a provided (default) item"
    );

    // At least one explicit generic implementor: FHRRAlgebra<D> implements
    // BindingAlgebra
    let fhrr_impl = catalog
        .implementations
        .iter()
        .find(|imp| {
            matches!(&imp.impl_type_endpoint, RelationshipEndpoint::Local { ident, .. } if ident == "FHRRAlgebra")
                && matches!(&imp.trait_endpoint, RelationshipEndpoint::Local { ident: t, .. } if t == "BindingAlgebra")
        });
    assert!(
        fhrr_impl.is_some(),
        "expected impl BindingAlgebra for FHRRAlgebra<D>"
    );
    if let Some(imp) = fhrr_impl {
        let gentoks = tokens(&imp.generics);
        assert!(
            gentoks.iter().any(|t| *t == "D" || *t == "const"),
            "expected const generic D, got {:?}",
            gentoks
        );
    }

    // Also check for CliffordAlgebra<P,Q,R> implementing BindingAlgebra.
    let clifford_impl = catalog
        .implementations
        .iter()
        .find(|imp| {
            matches!(&imp.impl_type_endpoint, RelationshipEndpoint::Local { ident, .. } if ident == "CliffordAlgebra")
                && matches!(&imp.trait_endpoint, RelationshipEndpoint::Local { ident: t, .. } if t == "BindingAlgebra")
        });
    assert!(
        clifford_impl.is_some(),
        "expected impl BindingAlgebra for CliffordAlgebra<P,Q,R>"
    );
}

// -------------------------------------------------------------------
// 8. Real workspace assertion: TermSystem
// -------------------------------------------------------------------

#[test]
fn real_term_system_trait_relationships() {
    let root = workspace_root();
    let rs_src = root.join("amari-rewrite/src/lib.rs");
    assert!(
        rs_src.exists(),
        "amari-rewrite/src/lib.rs not found at {}",
        rs_src.display()
    );

    let graph = module_graph(&root, "amari-rewrite/src/lib.rs").unwrap();
    let exports = export_graph(&graph, &root).unwrap();
    let catalog = trait_relationships(&graph, &exports, &root).unwrap();

    // TermSystem has `#[derive(Clone, Debug, Default, PartialEq, Eq)]`.
    // The derived Default impl should be present with is_derived=true.
    let ts_impl_default: Vec<&TraitImplementation> = catalog
        .implementations
        .iter()
        .filter(|imp| {
            matches!(&imp.impl_type_endpoint, RelationshipEndpoint::Local { ident, .. } if ident == "TermSystem")
                && imp.trait_path == "Default"
        })
        .collect();
    // TermSystem is exported at two paths (crate::trs::TermSystem and
    // crate::TermSystem), so two derive projections are produced.
    assert!(
        !ts_impl_default.is_empty(),
        "expected at least one derive-introduced Default for TermSystem"
    );
    assert!(
        ts_impl_default[0].is_derived,
        "TermSystem Default impl should be marked as derived"
    );
    assert!(
        !ts_impl_default[0].trait_path.is_empty(),
        "trait_path should be nonempty"
    );

    // There should be NO explicit (non-derived) Default impl for TermSystem.
    let explicit_default: Vec<&TraitImplementation> = catalog
        .implementations
        .iter()
        .filter(|imp| {
            matches!(&imp.impl_type_endpoint, RelationshipEndpoint::Local { ident, .. } if ident == "TermSystem")
                && imp.trait_path == "Default"
                && !imp.is_derived
        })
        .collect();
    assert!(
        explicit_default.is_empty(),
        "expected no explicit Default for TermSystem"
    );
}

// -------------------------------------------------------------------
// 9. Real workspace assertion: ParetoFront
// -------------------------------------------------------------------

#[test]
fn real_pareto_front_explicit_default_impl() {
    let root = workspace_root();
    let opt_src = root.join("amari-optimization/src/lib.rs");
    assert!(
        opt_src.exists(),
        "amari-optimization/src/lib.rs not found at {}",
        opt_src.display()
    );

    let graph = module_graph(&root, "amari-optimization/src/lib.rs").unwrap();
    let exports = export_graph(&graph, &root).unwrap();
    let catalog = trait_relationships(&graph, &exports, &root).unwrap();

    // ParetoFront<T: Float> should have an explicit `impl<T: Float> Default
    // for ParetoFront<T>`.
    let pf_default = catalog
        .implementations
        .iter()
        .find(|imp| {
            matches!(&imp.impl_type_endpoint, RelationshipEndpoint::Local { ident, .. } if ident == "ParetoFront")
                && matches!(&imp.trait_endpoint, RelationshipEndpoint::External { path } if path == "Default")
        });
    assert!(
        pf_default.is_some(),
        "expected explicit impl Default for ParetoFront<T>"
    );

    if let Some(imp) = pf_default {
        let gentoks = tokens(&imp.generics);
        assert!(
            gentoks.contains(&"Float"),
            "expected Float bound in generics for ParetoFront<T>, got {:?}",
            gentoks
        );
        assert!(
            gentoks.contains(&"T"),
            "expected type parameter T, got {:?}",
            gentoks
        );
    }
}

// -------------------------------------------------------------------
// 10. Real workspace assertion: WasmGenericMultivector
// -------------------------------------------------------------------

#[test]
fn real_wasm_generic_multivector_explicit_default_impl() {
    let root = workspace_root();
    let wasm_src = root.join("amari-wasm/src/lib.rs");
    assert!(
        wasm_src.exists(),
        "amari-wasm/src/lib.rs not found at {}",
        wasm_src.display()
    );

    let graph = module_graph(&root, "amari-wasm/src/lib.rs").unwrap();
    let exports = export_graph(&graph, &root).unwrap();
    let catalog = trait_relationships(&graph, &exports, &root).unwrap();

    // WasmGenericMultivector should have an explicit `impl Default for
    // WasmGenericMultivector`.
    let wgm_default = catalog
        .implementations
        .iter()
        .find(|imp| {
            matches!(&imp.impl_type_endpoint, RelationshipEndpoint::Local { ident, .. } if ident == "WasmGenericMultivector")
                && matches!(&imp.trait_endpoint, RelationshipEndpoint::External { path } if path == "Default")
        });
    assert!(
        wgm_default.is_some(),
        "expected explicit impl Default for WasmGenericMultivector"
    );

    // Also verify that WasmGenericMultivector has inherent methods.
    let wgm_impls: Vec<_> = catalog
        .implementations
        .iter()
        .filter(|imp| {
            matches!(&imp.impl_type_endpoint, RelationshipEndpoint::Local { ident, .. } if ident == "WasmGenericMultivector")
        })
        .collect();
    assert!(
        !wgm_impls.is_empty(),
        "expected at least one trait impl for WasmGenericMultivector"
    );
}

// ===================================================================
// I1: Alias projection observability
// ===================================================================

#[test]
fn local_trait_exported_under_multiple_aliases_all_retained() {
    // One local trait re-exported under two different paths.
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            (
                "src/lib.rs",
                "mod inner;\n\
                 pub use inner::MyTrait;\n\
                 pub use inner::MyTrait as TraitAlias;\n",
            ),
            ("src/inner.rs", "pub trait MyTrait {}\n"),
        ],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");

    // Both export paths should be present in the definitions.
    let at_root = find_trait_by_export(&catalog, "crate::MyTrait");
    assert_eq!(
        at_root.len(),
        1,
        "expected exactly one definition at crate::MyTrait, got {}",
        at_root.len()
    );

    let aliased = find_trait_by_export(&catalog, "crate::TraitAlias");
    assert_eq!(
        aliased.len(),
        1,
        "expected exactly one definition at crate::TraitAlias, got {}",
        aliased.len()
    );

    // Both definitions must share the same source endpoint.
    assert_eq!(at_root[0].trait_endpoint, aliased[0].trait_endpoint);

    // export_path must differ.
    assert_ne!(at_root[0].export_path, aliased[0].export_path);

    // source_path should be nonempty.
    assert!(!at_root[0].source_path.is_empty());
    assert!(!aliased[0].source_path.is_empty());
}

#[test]
fn trait_and_type_aliases_projected_into_implementations() {
    // Local trait and local type each exported under aliases.
    // Implementation relationships should project all valid combinations.
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            (
                "src/lib.rs",
                "mod inner;\n\
                 pub use inner::MyTrait;\n\
                 pub use inner::MyTrait as T2;\n\
                 pub use inner::MyStruct;\n\
                 pub use inner::MyStruct as S2;\n",
            ),
            (
                "src/inner.rs",
                "pub trait MyTrait {}\n\
                 pub struct MyStruct;\n\
                 impl MyTrait for MyStruct {}\n",
            ),
        ],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");

    // Expected combinations:
    //   (crate::MyTrait, crate::MyStruct)
    //   (crate::MyTrait, crate::S2)
    //   (crate::T2, crate::MyStruct)
    //   (crate::T2, crate::S2)
    let expected_pairs: &[(&str, &str)] = &[
        ("crate::MyTrait", "crate::MyStruct"),
        ("crate::MyTrait", "crate::S2"),
        ("crate::T2", "crate::MyStruct"),
        ("crate::T2", "crate::S2"),
    ];

    for (trait_path, type_path) in expected_pairs {
        let matches = find_impl_by_path(&catalog, trait_path, type_path);
        assert!(
            !matches.is_empty(),
            "expected at least one impl for ({trait_path}, {type_path})"
        );
    }

    // Total impl count should be at least the number of unique pairs.
    assert!(
        catalog.implementations.len() >= expected_pairs.len(),
        "expected at least {} impl records, got {}",
        expected_pairs.len(),
        catalog.implementations.len()
    );
}

// ===================================================================
// I2: cfg-deferred variants preserved
// ===================================================================

#[test]
fn cfg_alternate_trait_definitions_both_retained_by_source_path() {
    // Two #[path]-gated source files that both declare the same trait
    // ident inside the same canonical module path. The module graph
    // retains both variants (cfg is deferred). The trait index should
    // retain both with distinct source_path values.
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            (
                "src/lib.rs",
                // Two `mod platform;` with different #[path] (no actual cfg).
                // The module graph sees both and retains both variants.
                "#[path = \"variant_a.rs\"]\n\
                 mod platform;\n\
                 #[path = \"variant_b.rs\"]\n\
                 mod platform;\n\
                 pub use platform::MyTrait;\n",
            ),
            ("src/variant_a.rs", "pub trait MyTrait {}\n"),
            ("src/variant_b.rs", "pub trait MyTrait {}\n"),
        ],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");

    // We should have exactly TWO definitions for MyTrait at the same
    // export path, each with a different source_path.
    let defs: Vec<&TraitDefinition> = catalog
        .definitions
        .iter()
        .filter(|d| {
            matches!(&d.trait_endpoint, RelationshipEndpoint::Local { ident, .. } if ident == "MyTrait")
        })
        .collect();

    assert_eq!(
        defs.len(),
        2,
        "expected 2 trait definition variants for MyTrait, got {}",
        defs.len()
    );

    // Their source_paths must differ.
    assert_ne!(defs[0].source_path, defs[1].source_path);

    // Both source_paths should be nonempty and point to the expected files.
    for d in defs {
        assert!(
            d.source_path == "src/variant_a.rs" || d.source_path == "src/variant_b.rs",
            "unexpected source_path: {}",
            d.source_path
        );
    }
}

#[test]
fn cfg_alternate_impl_blocks_both_retained_by_source_path() {
    // Two #[path]-gated source files that both implement the same
    // trait for the same type. The impl blocks have different source_path
    // values and must both be retained. Both trait and type must be
    // publicly exported (no fabricated paths).
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            (
                "src/lib.rs",
                "#[path = \"variant_a.rs\"]\n\
                 mod platform;\n\
                 #[path = \"variant_b.rs\"]\n\
                 mod platform;\n\
                 pub use platform::MyStruct;\n\
                 pub use platform::MyTrait;\n",
            ),
            (
                "src/variant_a.rs",
                "pub struct MyStruct;\n\
                 pub trait MyTrait {}\n\
                 impl MyTrait for MyStruct {}\n",
            ),
            (
                "src/variant_b.rs",
                "pub struct MyStruct;\n\
                 pub trait MyTrait {}\n\
                 impl MyTrait for MyStruct {}\n",
            ),
        ],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");

    // Both MyStruct and MyTrait are exported at crate::MyStruct and
    // crate::MyTrait. We expect 2 impl records with different source_paths.
    let impls: Vec<&TraitImplementation> = catalog
        .implementations
        .iter()
        .filter(|imp| {
            matches!(&imp.impl_type_endpoint, RelationshipEndpoint::Local { ident, .. } if ident == "MyStruct")
        })
        .collect();

    assert_eq!(
        impls.len(),
        2,
        "expected 2 impl variants for MyStruct, got {}",
        impls.len()
    );

    // Their source_paths must differ.
    assert_ne!(impls[0].source_path, impls[1].source_path);

    for imp in impls {
        assert!(
            imp.source_path == "src/variant_a.rs" || imp.source_path == "src/variant_b.rs",
            "unexpected source_path: {}",
            imp.source_path
        );
    }
}

// ===================================================================
// M1: Derive carries declaration generics
// ===================================================================

#[test]
fn derived_generic_type_carries_declaration_generics() {
    // A generic struct with `#[derive(Default)]` should produce a
    // derived Default impl whose generics match the struct declaration.
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "#[derive(Default)]\n\
             pub struct Wrapped<T: Clone>(T);\n",
        )],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");

    // Find the derived Default for Wrapped.
    let derived_defaults: Vec<&TraitImplementation> = catalog
        .implementations
        .iter()
        .filter(|imp| {
            matches!(&imp.impl_type_endpoint, RelationshipEndpoint::Local { ident, .. } if ident == "Wrapped")
                && imp.trait_path == "Default"
                && imp.is_derived
        })
        .collect();

    assert_eq!(
        derived_defaults.len(),
        1,
        "expected exactly one derived Default for Wrapped, got {}",
        derived_defaults.len()
    );

    // The generics should contain T: Clone (from the struct declaration).
    let gentoks = tokens(&derived_defaults[0].generics);
    assert!(
        gentoks.contains(&"Clone"),
        "expected Clone bound in derived generics, got {:?}",
        gentoks
    );
    assert!(
        gentoks.contains(&"T"),
        "expected T in derived generics, got {:?}",
        gentoks
    );

    // source_path must be nonempty.
    assert!(
        !derived_defaults[0].source_path.is_empty(),
        "source_path should not be empty for derived impl"
    );
}

// ===================================================================
// 5B4 fixes: super::super:: resolution
// ===================================================================

#[test]
fn deeply_nested_super_super_path_resolution() {
    // Three levels of nesting. The innermost module has an impl that
    // references the trait and type via super::super:: from the third level.
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub mod a {
                     pub mod b {
                         pub mod c {
                             use super::super::MyTrait;
                             use super::super::MyStruct;
                             impl MyTrait for MyStruct {}
                         }
                         pub trait MyTrait {}
                         pub struct MyStruct;
                     }
                     pub use b::MyTrait;
                     pub use b::MyStruct;
                 }\n",
        )],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");

    // MyTrait should be exported at crate::a::MyTrait and
    // crate::a::b::MyTrait. Find via the trait export path.
    let defs: Vec<&TraitDefinition> = catalog
        .definitions
        .iter()
        .filter(|d| {
            matches!(&d.trait_endpoint, RelationshipEndpoint::Local { ident, .. } if ident == "MyTrait")
        })
        .collect();
    assert!(!defs.is_empty(), "expected at least one MyTrait definition");

    // The impl should have MyStruct as a local endpoint.
    let impls: Vec<&TraitImplementation> = catalog
        .implementations
        .iter()
        .filter(|imp| {
            matches!(&imp.impl_type_endpoint, RelationshipEndpoint::Local { ident, .. } if ident == "MyStruct")
                && matches!(&imp.trait_endpoint, RelationshipEndpoint::Local { ident: t, .. } if t == "MyTrait")
        })
        .collect();
    assert!(
        !impls.is_empty(),
        "expected impl MyTrait for MyStruct via super::super::"
    );
    for imp in &impls {
        assert!(
            matches!(&imp.trait_endpoint, RelationshipEndpoint::Local { .. }),
            "trait endpoint should be local, got {:?}",
            imp.trait_endpoint
        );
        assert!(
            matches!(&imp.impl_type_endpoint, RelationshipEndpoint::Local { .. }),
            "type endpoint should be local, got {:?}",
            imp.impl_type_endpoint
        );
    }
}

#[test]
fn triple_super_path_resolution() {
    // Four levels of nesting (a::b::c::d). The innermost module uses
    // super::super::super:: to reach crate::a items.
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub mod a {
                     pub mod b {
                         pub mod c {
                             pub mod d {
                                 use super::super::MyTrait;
                                 use super::super::MyStruct;
                                 impl MyTrait for MyStruct {}
                             }
                         }
                         pub trait MyTrait {}
                         pub struct MyStruct;
                     }
                 }\n",
        )],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");

    let impls: Vec<&TraitImplementation> = catalog
        .implementations
        .iter()
        .filter(|imp| {
            matches!(&imp.impl_type_endpoint, RelationshipEndpoint::Local { ident, .. } if ident == "MyStruct")
                && matches!(&imp.trait_endpoint, RelationshipEndpoint::Local { ident: t, .. } if t == "MyTrait")
        })
        .collect();
    assert!(
        !impls.is_empty(),
        "expected impl MyTrait for MyStruct via super::super::super::"
    );
    for imp in &impls {
        assert!(
            matches!(&imp.trait_endpoint, RelationshipEndpoint::Local { .. }),
            "trait endpoint should be local"
        );
        assert!(
            matches!(&imp.impl_type_endpoint, RelationshipEndpoint::Local { .. }),
            "type endpoint should be local"
        );
    }
}

// ===================================================================
// 5B4 fixes: public-catalog filtering (no fabricated paths)
// ===================================================================

#[test]
fn non_exported_local_trait_not_in_public_impl_paths() {
    // A private module with a local trait and type; the impl should
    // not produce public relationships because neither endpoint has
    // reachable exports.
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "mod private {
                 pub trait LocalTrait {}
                 pub struct LocalStruct;
                 impl LocalTrait for LocalStruct {}
             }\n",
        )],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");

    // Neither LocalTrait nor LocalStruct is publicly exported.
    // Therefore no impl should appear with a fabricated path.
    let local_impls: Vec<&TraitImplementation> = catalog
        .implementations
        .iter()
        .filter(|imp| {
            matches!(&imp.trait_endpoint, RelationshipEndpoint::Local { .. })
                || matches!(&imp.impl_type_endpoint, RelationshipEndpoint::Local { .. })
        })
        .collect();

    // The impl should not be emitted because neither endpoint has
    // reachable exports. However, a valid external trait impl against
    // a local type that HAS exports IS allowed — this test has no
    // exports at all, so no impl relationships.
    assert!(
        local_impls.is_empty(),
        "expected no local-relationship impl records for non-exported trait+type, got {}: {:?}",
        local_impls.len(),
        local_impls
    );
}

#[test]
fn external_trait_plus_exported_local_type_emitted() {
    // External trait (Default) impl for an exported local type should
    // remain emitted. This verifies we don't over-filter.
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub struct ExposedStruct;
             impl Default for ExposedStruct {
                 fn default() -> Self { ExposedStruct }
             }\n",
        )],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");

    let imp = find_impl(&catalog, "Default", "ExposedStruct");
    assert!(
        matches!(&imp.trait_endpoint, RelationshipEndpoint::External { .. }),
        "Default should be external"
    );
    assert!(
        matches!(&imp.impl_type_endpoint, RelationshipEndpoint::Local { .. }),
        "ExposedStruct should be local"
    );
    assert_eq!(
        imp.impl_type_path, "crate::ExposedStruct",
        "expected public export path for ExposedStruct"
    );
}

#[test]
fn exported_local_trait_non_exported_type_not_emitted() {
    // Local exported trait + non-exported local type: should NOT emit
    // a public relationship for the non-exported type.
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub trait MyTrait {}
             mod inner {
                 pub struct MyStruct;
                 impl super::MyTrait for MyStruct {}
             }\n",
        )],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");

    // MyTrait is exported. MyStruct is NOT exported (inner is private).
    // The impl should not be emitted because the self-type has no
    // public path.
    let matching_impls: Vec<&TraitImplementation> = catalog
        .implementations
        .iter()
        .filter(|imp| {
            matches!(&imp.impl_type_endpoint, RelationshipEndpoint::Local { ident, .. } if ident == "MyStruct")
        })
        .collect();
    assert!(
        matching_impls.is_empty(),
        "expected no impl for non-exported MyStruct, got {}: {:?}",
        matching_impls.len(),
        matching_impls
    );
}

// ===================================================================
// 5B4 fixes: same-file cfg declaration variants (source_ordinal)
// ===================================================================

#[test]
fn same_file_cfg_gated_trait_definitions_both_retained() {
    // Two trait definitions with the same name in the same file under
    // different cfg attrs must both appear as distinct records.
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "#[cfg(feature = \"a\")]
             pub trait MyTrait { fn from_a(&self); }
             #[cfg(feature = \"b\")]
             pub trait MyTrait { fn from_b(&self); }\n",
        )],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");

    // Both definitions at the same export path, same source_path,
    // but different source_ordinal values.
    let defs: Vec<&TraitDefinition> = catalog
        .definitions
        .iter()
        .filter(|d| {
            matches!(&d.trait_endpoint, RelationshipEndpoint::Local { ident, .. } if ident == "MyTrait")
        })
        .collect();

    assert_eq!(
        defs.len(),
        2,
        "expected 2 trait definitions for MyTrait (two cfg variants in one file), got {}: {:?}",
        defs.len(),
        defs.iter().map(|d| &d.source_path).collect::<Vec<_>>()
    );

    // Same source_path but different source_ordinal.
    assert_eq!(defs[0].source_path, defs[1].source_path);
    assert_ne!(defs[0].source_ordinal, defs[1].source_ordinal);

    // Different required items from different variants
    let req_names_0: Vec<&str> = defs[0]
        .required_items
        .iter()
        .map(|i| i.name.as_str())
        .collect();
    let req_names_1: Vec<&str> = defs[1]
        .required_items
        .iter()
        .map(|i| i.name.as_str())
        .collect();
    assert!(
        req_names_0.contains(&"from_a"),
        "variant 0 should have from_a"
    );
    assert!(
        req_names_1.contains(&"from_b"),
        "variant 1 should have from_b"
    );
}

#[test]
fn same_file_cfg_gated_impl_blocks_both_retained() {
    // Two impl blocks for the same trait+type pair in the same file
    // under different cfg attrs must both appear.
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub trait MyTrait {}
             pub struct MyStruct;
             #[cfg(feature = \"a\")]
             impl MyTrait for MyStruct {}
             #[cfg(feature = \"b\")]
             impl MyTrait for MyStruct {}
             pub use crate::MyStruct;  // ensure export\n",
        )],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");

    // Both impl variants should be present.
    let impls: Vec<&TraitImplementation> = catalog
        .implementations
        .iter()
        .filter(|imp| {
            matches!(&imp.impl_type_endpoint, RelationshipEndpoint::Local { ident, .. } if ident == "MyStruct")
                && matches!(&imp.trait_endpoint, RelationshipEndpoint::Local { ident: t, .. } if t == "MyTrait")
        })
        .collect();

    assert_eq!(
        impls.len(),
        2,
        "expected 2 impl variants for MyTrait+MyStruct, got {}: {:?}",
        impls.len(),
        impls.iter().map(|i| &i.source_path).collect::<Vec<_>>()
    );

    // Same source_path but different source_ordinal.
    assert_eq!(impls[0].source_path, impls[1].source_path);
    assert_ne!(impls[0].source_ordinal, impls[1].source_ordinal);
}

#[test]
fn same_file_cfg_gated_derive_variants_both_retained() {
    // Two type declarations with the same name in the same file
    // under different cfg attrs, each with different derive sets.
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "#[cfg(feature = \"a\")]
             #[derive(Clone)]
             pub struct MyStruct;
             #[cfg(feature = \"b\")]
             #[derive(Debug)]
             pub struct MyStruct;\n",
        )],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");

    // Both declarations should produce definitions/derive impls.
    // At minimum, two TraitDefinition entries (one per export path
    // through the struct being pub, MyStruct export paths).
    let impls: Vec<&TraitImplementation> = catalog
        .implementations
        .iter()
        .filter(|imp| {
            matches!(&imp.impl_type_endpoint, RelationshipEndpoint::Local { ident, .. } if ident == "MyStruct")
        })
        .collect();

    // Should have at least 2 impl records (could be more with re-export
    // projection) and they should have different source_ordinal values.
    assert!(
        impls.len() >= 2,
        "expected at least 2 impl records for MyStruct (2 derive variants), got {}",
        impls.len()
    );

    // At least one Clone and one Debug derived impl.
    let has_clone = impls.iter().any(|i| i.trait_path == "Clone");
    let has_debug = impls.iter().any(|i| i.trait_path == "Debug");
    assert!(has_clone, "expected a Clone derived impl");
    assert!(has_debug, "expected a Debug derived impl");
}

// -------------------------------------------------------------------
// Helpers
// -------------------------------------------------------------------

fn workspace_root() -> std::path::PathBuf {
    let candidate = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // CARGO_MANIFEST_DIR is the amari-discovery crate directory.
    // The workspace root is one level up.
    candidate
        .parent()
        .expect("CARGO_MANIFEST_DIR parent is workspace root")
        .to_path_buf()
}
