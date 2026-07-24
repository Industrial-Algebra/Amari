// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tests for Task 5C2: exported declarative and procedural macro cataloguing.

use std::{fs, path::Path};

use amari_discovery::catalog::generator::{
    inventory_workspace, macro_catalog, module_graph, MacroCatalog, MacroKind, MacroRecord,
    MacroWarningReason,
};
use tempfile::TempDir;

/// Minimal Cargo.toml for a regular library test crate.
fn lib_toml() -> String {
    "[package]\n\
     name = \"test-crate\"\n\
     version = \"0.1.0\"\n\
     edition = \"2021\"\n\
     description = \"Test fixture\"\n\
     license = \"MIT OR Apache-2.0\"\n\
     \n\
     [workspace]\n\
     members = []\n\
     \n\
     [workspace.package]\n\
     version = \"0.1.0\"\n"
        .to_owned()
}

/// Minimal Cargo.toml for a proc-macro test crate.
fn proc_toml() -> String {
    "[package]\n\
     name = \"test-crate\"\n\
     version = \"0.1.0\"\n\
     edition = \"2021\"\n\
     description = \"Proc-macro test fixture\"\n\
     license = \"MIT OR Apache-2.0\"\n\
     \n\
     [lib]\n\
     proc-macro = true\n\
     \n\
     [workspace]\n\
     members = []\n\
     \n\
     [workspace.package]\n\
     version = \"0.1.0\"\n"
        .to_owned()
}

fn write_package(root: &Path, files: &[(&str, &str)]) {
    for (relative, source) in files {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, source).unwrap();
    }
}

/// Builds a macro catalog from a TempDir package. `source_path` is the library
/// entry point (e.g. `src/lib.rs`). The package name is always `test-crate`.
fn macros_for(root: &Path, source_path: &str) -> MacroCatalog {
    let inventory = inventory_workspace(root).unwrap();
    let graph = module_graph(root, source_path).unwrap();
    macro_catalog(&graph, &inventory, "test-crate", root).unwrap()
}

fn paths(catalog: &MacroCatalog) -> Vec<String> {
    catalog.records.iter().map(|r| r.path.clone()).collect()
}

// =========================================================================
// Declarative macro: private exclusion
// =========================================================================

#[test]
fn private_macro_rules_is_excluded() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &lib_toml()),
            (
                "src/lib.rs",
                "macro_rules! private_macro {\n\
                 \x20\x20\x20\x20() => {};\n\
                 }\n",
            ),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    assert!(
        catalog.records.is_empty(),
        "private macro_rules! must be excluded; got {:?}",
        paths(&catalog)
    );
    assert!(catalog.warnings.is_empty());
}

// =========================================================================
// Declarative macro: #[macro_export] at crate root
// =========================================================================

#[test]
fn macro_export_is_at_crate_root_regardless_of_declaration_module() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &lib_toml()),
            ("src/lib.rs", "pub mod nested;\n"),
            (
                "src/nested.rs",
                "#[macro_export]\n\
                 macro_rules! geo {\n\
                 \x20\x20\x20\x20($a:expr, $b:expr) => { $a.outer_product(&$b) };\n\
                 }\n",
            ),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    assert!(
        catalog.records.iter().any(|r| r.path == "crate::geo"),
        "must export at crate root: {:?}",
        paths(&catalog)
    );
    assert!(
        !catalog
            .records
            .iter()
            .any(|r| r.path.contains("nested::geo")),
        "must not export at declaration module path"
    );
    assert_eq!(catalog.warnings.len(), 0);
}

#[test]
fn macro_export_signature_preserves_declaration_surface() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &lib_toml()),
            (
                "src/lib.rs",
                "#[macro_export]\n\
                 macro_rules! wedge {\n\
                 \x20\x20\x20\x20($a:expr, $b:expr) => { $a.outer_product(&$b) };\n\
                 }\n",
            ),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    let record = catalog
        .records
        .iter()
        .find(|r| r.name == "wedge")
        .expect("wedge macro must exist");
    assert_eq!(record.kind, MacroKind::Declarative);
    assert!(record.signature.contains("macro_rules ! wedge"));
    assert!(record.signature.contains("$ a : expr"));
    assert_eq!(record.source.module, "crate");
    assert_eq!(record.source.source_path, "src/lib.rs");
}

// =========================================================================
// Declarative macro: same-name cfg variants retained
// =========================================================================

#[test]
fn cfg_variant_macros_with_same_name_are_both_retained() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &lib_toml()),
            ("src/lib.rs", "pub mod unix_v;\npub mod win_v;\n"),
            (
                "src/unix_v.rs",
                "#[cfg(unix)]\n\
                 #[macro_export]\n\
                 macro_rules! platform_macro {\n\
                 \x20\x20\x20\x20() => { \"unix\" };\n\
                 }\n",
            ),
            (
                "src/win_v.rs",
                "#[cfg(windows)]\n\
                 #[macro_export]\n\
                 macro_rules! platform_macro {\n\
                 \x20\x20\x20\x20() => { \"windows\" };\n\
                 }\n",
            ),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    let count = catalog
        .records
        .iter()
        .filter(|r| r.name == "platform_macro")
        .count();
    assert_eq!(
        count,
        2,
        "cfg-deferred variants must both be retained: {:?}",
        paths(&catalog)
    );
}

// =========================================================================
// Declarative macro: local re-exports and aliases
// =========================================================================

#[test]
fn pub_use_reexports_known_macro_under_new_name() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &lib_toml()),
            (
                "src/lib.rs",
                "#[macro_export]\n\
                 macro_rules! original {\n\
                 \x20\x20\x20\x20() => {};\n\
                 }\n\
                 pub use crate::original as aliased;\n",
            ),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    let original = catalog
        .records
        .iter()
        .find(|r| r.name == "original")
        .expect("original must exist");
    let aliased = catalog
        .records
        .iter()
        .find(|r| r.path == "crate::aliased")
        .expect("aliased re-export must exist");

    assert_eq!(aliased.name, "aliased");
    assert_eq!(aliased.source.module, original.source.module);
    assert_eq!(aliased.source.source_path, original.source.source_path);
}

#[test]
fn grouped_pub_use_reexports_multiple_macros() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &lib_toml()),
            (
                "src/lib.rs",
                "#[macro_export]\n\
                 macro_rules! first { () => {}; }\n\
                 #[macro_export]\n\
                 macro_rules! second { () => {}; }\n\
                 pub use crate::{first as f1, second as s2};\n",
            ),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    assert!(
        catalog.records.iter().any(|r| r.path == "crate::f1"),
        "grouped alias f1 missing: {:?}",
        paths(&catalog)
    );
    assert!(
        catalog.records.iter().any(|r| r.path == "crate::s2"),
        "grouped alias s2 missing: {:?}",
        paths(&catalog)
    );
}

// =========================================================================
// Declarative macro: external re-export warnings
// =========================================================================

#[test]
fn external_macro_reexport_is_typed_warning_not_record() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &lib_toml()),
            (
                "src/lib.rs",
                "pub use serde::Serialize;\n\
                 pub use std::io::Read;\n",
            ),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    assert!(
        catalog.records.iter().all(|r| r.path != "crate::Serialize"),
        "external re-export must not fabricate a local record"
    );
    assert!(
        catalog.records.iter().all(|r| r.path != "crate::Read"),
        "external re-export must not fabricate a local record"
    );
    assert!(
        catalog.warnings.len() >= 2,
        "must warn about external re-exports: {:?}",
        catalog.warnings
    );
    let ext_targets: Vec<String> = catalog
        .warnings
        .iter()
        .filter_map(|w| match &w.reason {
            MacroWarningReason::ExternalReexport { target } => Some(target.clone()),
            _ => None,
        })
        .collect();
    assert!(ext_targets.contains(&"serde::Serialize".to_owned()));
    assert!(ext_targets.contains(&"std::io::Read".to_owned()));
}

// =========================================================================
// Procedural macro: function-like proc_macro
// =========================================================================

#[test]
fn proc_macro_function_is_recorded_in_proc_macro_target() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &proc_toml()),
            (
                "src/lib.rs",
                "use proc_macro::TokenStream;\n\
                 #[proc_macro]\n\
                 pub fn my_macro(input: TokenStream) -> TokenStream {\n\
                 \x20\x20\x20\x20input\n\
                 }\n",
            ),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    let record = catalog
        .records
        .iter()
        .find(|r| r.name == "my_macro")
        .expect("proc_macro function must be recorded");
    assert_eq!(record.kind, MacroKind::ProcMacro);
    assert_eq!(record.path, "crate::my_macro");
    assert!(record.signature.contains("pub fn my_macro"));
    assert!(record.signature.contains("TokenStream"));
    assert!(!record.signature.contains("input\n"));
    assert!(record.helpers.is_empty());
}

#[test]
fn proc_macro_attribute_is_recorded() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &proc_toml()),
            (
                "src/lib.rs",
                "use proc_macro::TokenStream;\n\
                 #[proc_macro_attribute]\n\
                 pub fn my_attr(attr: TokenStream, item: TokenStream) -> TokenStream {\n\
                 \x20\x20\x20\x20item\n\
                 }\n",
            ),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    let record = catalog
        .records
        .iter()
        .find(|r| r.name == "my_attr")
        .expect("proc_macro_attribute must be recorded");
    assert_eq!(record.kind, MacroKind::ProcMacroAttribute);
    assert_eq!(record.path, "crate::my_attr");
    assert!(record.signature.contains("pub fn my_attr"));
}

#[test]
fn proc_macro_derive_with_helpers_is_recorded() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &proc_toml()),
            (
                "src/lib.rs",
                "use proc_macro::TokenStream;\n\
                 #[proc_macro_derive(MyTrait, attributes(helper_a, helper_b))]\n\
                 pub fn derive_my_trait(input: TokenStream) -> TokenStream {\n\
                 \x20\x20\x20\x20input\n\
                 }\n",
            ),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    let record = catalog
        .records
        .iter()
        .find(|r| r.name == "MyTrait")
        .expect("proc_macro_derive must record the derive name");
    assert_eq!(record.kind, MacroKind::ProcMacroDerive);
    assert_eq!(record.path, "crate::MyTrait");
    assert_eq!(record.helpers, vec!["helper_a", "helper_b"]);
    assert!(record.signature.contains("pub fn derive_my_trait"));
}

#[test]
fn proc_macro_derive_helpers_are_sorted_and_deduped() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &proc_toml()),
            (
                "src/lib.rs",
                "use proc_macro::TokenStream;\n\
                 #[proc_macro_derive(Deduped, attributes(zebra, apple, zebra))]\n\
                 pub fn derive_deduped(input: TokenStream) -> TokenStream {\n\
                 \x20\x20\x20\x20input\n\
                 }\n",
            ),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    let record = catalog
        .records
        .iter()
        .find(|r| r.name == "Deduped")
        .expect("derive must be recorded");
    assert_eq!(record.helpers, vec!["apple", "zebra"]);
}

#[test]
fn proc_macro_derive_without_helpers_has_empty_vec() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &proc_toml()),
            (
                "src/lib.rs",
                "use proc_macro::TokenStream;\n\
                 #[proc_macro_derive(NoHelpers)]\n\
                 pub fn derive_no_helpers(input: TokenStream) -> TokenStream {\n\
                 \x20\x20\x20\x20input\n\
                 }\n",
            ),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    let record = catalog
        .records
        .iter()
        .find(|r| r.name == "NoHelpers")
        .expect("derive without helpers must be recorded");
    assert!(record.helpers.is_empty());
}

// =========================================================================
// Procedural macro: non-proc-macro target exclusion
// =========================================================================

#[test]
fn proc_macro_attrs_in_non_proc_macro_target_are_not_fabricated() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &lib_toml()),
            (
                "src/lib.rs",
                "use proc_macro::TokenStream;\n\
                 #[proc_macro]\n\
                 pub fn not_a_proc(input: TokenStream) -> TokenStream {\n\
                 \x20\x20\x20\x20input\n\
                 }\n\
                 #[proc_macro_attribute]\n\
                 pub fn not_an_attr(attr: TokenStream, item: TokenStream) -> TokenStream {\n\
                 \x20\x20\x20\x20item\n\
                 }\n",
            ),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    assert!(
        !catalog.records.iter().any(|r| r.name == "not_a_proc"),
        "must not fabricate proc_macro in non-proc-macro target"
    );
    assert!(
        !catalog.records.iter().any(|r| r.name == "not_an_attr"),
        "must not fabricate proc_macro_attribute in non-proc-macro target"
    );
}

// =========================================================================
// Warnings: malformed proc macro attributes
// =========================================================================

#[test]
fn malformed_proc_macro_derive_is_warning() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &proc_toml()),
            (
                "src/lib.rs",
                "use proc_macro::TokenStream;\n\
                 #[proc_macro_derive]\n\
                 pub fn missing_name(input: TokenStream) -> TokenStream {\n\
                 \x20\x20\x20\x20input\n\
                 }\n",
            ),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    assert!(
        !catalog.records.iter().any(|r| r.name == "missing_name"),
        "malformed derive must not be recorded as a macro"
    );
    let has_malformed = catalog
        .warnings
        .iter()
        .any(|w| matches!(&w.reason, MacroWarningReason::MalformedProcMacro { .. }));
    assert!(
        has_malformed,
        "must warn about malformed derive: {:?}",
        catalog.warnings
    );
}

// =========================================================================
// Deterministic ordering
// =========================================================================

#[test]
fn macro_catalog_is_deterministic() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &proc_toml()),
            (
                "src/lib.rs",
                "#[macro_export]\n\
                 macro_rules! zebra { () => {}; }\n\
                 #[macro_export]\n\
                 macro_rules! apple { () => {}; }\n\
                 use proc_macro::TokenStream;\n\
                 #[proc_macro]\n\
                 pub fn mango(input: TokenStream) -> TokenStream { input }\n\
                 #[proc_macro_attribute]\n\
                 pub fn banana(attr: TokenStream, item: TokenStream) -> TokenStream { item }\n\
                 #[proc_macro_derive(Beta, attributes(h2, h1))]\n\
                 pub fn derive_beta(input: TokenStream) -> TokenStream { input }\n\
                 #[proc_macro_derive(Alpha, attributes(a1))]\n\
                 pub fn derive_alpha(input: TokenStream) -> TokenStream { input }\n",
            ),
        ],
    );

    let first = macros_for(temp.path(), "src/lib.rs");
    let second = macros_for(temp.path(), "src/lib.rs");
    assert_eq!(first, second);
}

// =========================================================================
// Issue 1: Unique deterministic file preorder ordinals
// =========================================================================

#[test]
fn proc_macros_inline_and_top_level_have_no_ordinal_collisions() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &proc_toml()),
            (
                "src/lib.rs",
                "use proc_macro::TokenStream;\n\
                 #[proc_macro]\n\
                 pub fn top_a(input: TokenStream) -> TokenStream { input }\n\
                 mod inline_a {\n\
                     #[proc_macro]\n\
                     pub fn nested_a(input: TokenStream) -> TokenStream { input }\n\
                     #[proc_macro_derive(NestedB)]\n\
                     pub fn derive_nested_b(input: TokenStream) -> TokenStream { input }\n\
                 }\n\
                 #[proc_macro]\n\
                 pub fn top_after(input: TokenStream) -> TokenStream { input }\n",
            ),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");

    // Every macro must be found.
    for expected_name in &["top_a", "nested_a", "NestedB", "top_after"] {
        assert!(
            catalog.records.iter().any(|r| r.name == *expected_name),
            "macro {expected_name} must be recorded: {:?}",
            paths(&catalog)
        );
    }

    // Ordinals across the same source file must be unique (no collisions).
    let source_path = "src/lib.rs";
    let ordinals: Vec<u64> = catalog
        .records
        .iter()
        .filter(|r| r.source.source_path == source_path)
        .map(|r| r.source.ordinal)
        .collect();
    let mut sorted = ordinals.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        ordinals.len(),
        sorted.len(),
        "ordinals must be unique across top-level and inline items: {ordinals:?}"
    );

    // Ordinals must be contiguous starting from 0 for proc-macro items.
    // All proc-macro records in this file should have ordinals reflecting
    // file-order positions (use at pos 0, mod at pos 2 consume slots).
    let proc_records: Vec<&MacroRecord> = catalog
        .records
        .iter()
        .filter(|r| r.source.source_path == source_path)
        .collect();
    assert_eq!(proc_records.len(), 4, "expected 4 proc macro records");
    let mut proc_ordinals: Vec<u64> = proc_records.iter().map(|r| r.source.ordinal).collect();
    proc_ordinals.sort();
    assert_eq!(
        proc_ordinals,
        vec![1, 2, 3, 5],
        "ordinals must be file-order positions (use=0, mod decl consumes slot after children)"
    );
}

#[test]
fn mixed_declarative_and_proc_macro_ordinals_are_contiguous_and_unique() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &proc_toml()),
            (
                "src/lib.rs",
                "use proc_macro::TokenStream;\n\
                 #[proc_macro]\n\
                 pub fn first_proc(input: TokenStream) -> TokenStream { input }\n\
                 #[macro_export]\n\
                 macro_rules! mid_macro { () => {}; }\n\
                 #[proc_macro_attribute]\n\
                 pub fn last_proc(attr: TokenStream, item: TokenStream) -> TokenStream { item }\n",
            ),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    let source_path = "src/lib.rs";

    // Verify all 3 items exist.
    for name in &["first_proc", "mid_macro", "last_proc"] {
        assert!(
            catalog.records.iter().any(|r| r.name == *name),
            "{name} must be recorded: {:?}",
            paths(&catalog)
        );
    }

    let mut ordinals: Vec<u64> = catalog
        .records
        .iter()
        .filter(|r| r.source.source_path == source_path)
        .map(|r| r.source.ordinal)
        .collect();
    ordinals.sort();
    assert_eq!(
        ordinals,
        vec![1, 2, 3],
        "mixed declarative and proc macro ordinals must be file-order positions (use at pos 0 consumes ordinal 0)"
    );
}

// =========================================================================
// Issue 2: Arbitrary inline module depth
// =========================================================================

#[test]
fn macro_export_at_inline_depth_2_is_indexed_at_crate_root() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &lib_toml()),
            (
                "src/lib.rs",
                "mod outer_inline {\n\
                     pub mod inner_inline {\n\
                         #[macro_export]\n\
                         macro_rules! deep_macro { () => {}; }\n\
                     }\n\
                 }\n",
            ),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    let record = catalog
        .records
        .iter()
        .find(|r| r.name == "deep_macro")
        .expect("macro_export at inline depth 2 must be recorded at crate root");
    assert_eq!(record.path, "crate::deep_macro");
    assert_eq!(record.source.module, "crate");
    assert_eq!(record.source.source_path, "src/lib.rs");
}

#[test]
fn proc_macro_fns_nested_in_inline_module_depth_2_are_visited() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &proc_toml()),
            (
                "src/lib.rs",
                "use proc_macro::TokenStream;\n\
                 mod level1 {\n\
                     mod level2 {\n\
                         #[proc_macro]\n\
                         pub fn deep_proc(input: TokenStream) -> TokenStream { input }\n\
                         #[proc_macro_derive(DeepDerive)]\n\
                         pub fn derive_deep(input: TokenStream) -> TokenStream { input }\n\
                     }\n\
                 }\n",
            ),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    assert!(
        catalog.records.iter().any(|r| r.name == "deep_proc"),
        "proc_macro at inline depth 2 must be recorded: {:?}",
        paths(&catalog)
    );
    assert!(
        catalog.records.iter().any(|r| r.name == "DeepDerive"),
        "proc_macro_derive at inline depth 2 must be recorded: {:?}",
        paths(&catalog)
    );

    // Verify source module reflects declaration site.
    for name in &["deep_proc", "DeepDerive"] {
        let record = catalog.records.iter().find(|r| r.name == *name).unwrap();
        assert!(
            record.source.module.contains("level1::level2")
                || record.source.module.contains("level1"),
            "deep proc macro {} source.module should reflect nesting: got {}",
            name,
            record.source.module
        );
    }
}

// =========================================================================
// Issue 3: Warning classification for bare multi-segment paths
// =========================================================================

#[test]
fn pub_use_local_module_path_is_not_external_when_module_is_local() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &lib_toml()),
            (
                "src/lib.rs",
                "pub mod geo_mod;\npub use geo_mod::Something;\n",
            ),
            ("src/geo_mod.rs", "pub struct Something;\n"),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");

    // Must not emit ExternalReexport for a path whose first segment is a local module.
    let ext_targets: Vec<String> = catalog
        .warnings
        .iter()
        .filter_map(|w| match &w.reason {
            MacroWarningReason::ExternalReexport { target } => Some(target.clone()),
            _ => None,
        })
        .collect();
    assert!(
        !ext_targets.contains(&"geo_mod::Something".to_owned()),
        "local module path must NOT be ExternalReexport: {:?}",
        ext_targets
    );

    // If the path resolves to a known non-macro local item (Issue 5),
    // the macro warning is suppressed entirely.
    // If it were truly unresolved (the item doesn't exist), we'd get
    // UnresolvedReexport.
}

#[test]
fn genuinely_external_path_still_typed_external() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &lib_toml()),
            (
                "src/lib.rs",
                "pub use serde::Serialize;\n\
                 pub use std::io::Read;\n",
            ),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    let ext_targets: Vec<String> = catalog
        .warnings
        .iter()
        .filter_map(|w| match &w.reason {
            MacroWarningReason::ExternalReexport { target } => Some(target.clone()),
            _ => None,
        })
        .collect();
    assert!(ext_targets.contains(&"serde::Serialize".to_owned()));
    assert!(ext_targets.contains(&"std::io::Read".to_owned()));
}

#[test]
fn crate_self_super_segments_are_not_treated_as_external() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &lib_toml()),
            (
                "src/lib.rs",
                "pub use crate::not_a_macro;\n\
                 pub use self::also_not;\n\
                 pub use super::should_not_be_external;\n",
            ),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    // No ExternalReexport warnings for crate/self/super paths.
    let ext_targets: Vec<String> = catalog
        .warnings
        .iter()
        .filter_map(|w| match &w.reason {
            MacroWarningReason::ExternalReexport { target } => Some(target.clone()),
            _ => None,
        })
        .collect();
    assert!(
        ext_targets.is_empty(),
        "crate/self/super paths must not be ExternalReexport: {ext_targets:?}"
    );
    // Instead, they should be UnresolvedReexport.
    assert!(
        catalog
            .warnings
            .iter()
            .all(|w| matches!(&w.reason, MacroWarningReason::UnresolvedReexport { .. })),
        "crate/self/super paths must be UnresolvedReexport: {:?}",
        catalog.warnings
    );
}

// =========================================================================
// Issue 4: Multiple proc_macro_derive attributes
// =========================================================================

#[test]
fn multiple_proc_macro_derive_attrs_on_one_fn_emit_all_distinct_records() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &proc_toml()),
            (
                "src/lib.rs",
                "use proc_macro::TokenStream;\n\
                 #[proc_macro_derive(DeriveA, attributes(helper_a))]\n\
                 #[proc_macro_derive(DeriveB)]\n\
                 pub fn multi_derive(input: TokenStream) -> TokenStream { input }\n",
            ),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");

    let derive_a = catalog
        .records
        .iter()
        .find(|r| r.name == "DeriveA")
        .expect("DeriveA must be recorded");
    assert_eq!(derive_a.kind, MacroKind::ProcMacroDerive);
    assert_eq!(derive_a.path, "crate::DeriveA");
    assert_eq!(derive_a.helpers, vec!["helper_a"]);
    assert!(derive_a.signature.contains("pub fn multi_derive"));

    let derive_b = catalog
        .records
        .iter()
        .find(|r| r.name == "DeriveB")
        .expect("DeriveB must be recorded");
    assert_eq!(derive_b.kind, MacroKind::ProcMacroDerive);
    assert_eq!(derive_b.path, "crate::DeriveB");
    assert!(derive_b.helpers.is_empty());
    assert!(derive_b.signature.contains("pub fn multi_derive"));

    // Both derive records share the same source ordinal and signature.
    assert_eq!(derive_a.source.ordinal, derive_b.source.ordinal);
    assert_eq!(derive_a.signature, derive_b.signature);
}

#[test]
fn multiple_derive_attrs_with_one_malformed_still_emits_valid_and_warns() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &proc_toml()),
            (
                "src/lib.rs",
                "use proc_macro::TokenStream;\n\
                 #[proc_macro_derive(GoodDerive)]\n\
                 #[proc_macro_derive]\n\
                 pub fn partial(input: TokenStream) -> TokenStream { input }\n",
            ),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    // The valid derive must still be emitted.
    assert!(
        catalog.records.iter().any(|r| r.name == "GoodDerive"),
        "valid derive must be emitted alongside malformed: {:?}",
        paths(&catalog)
    );
    // The malformed derive must produce a warning.
    let malformed_count = catalog
        .warnings
        .iter()
        .filter(|w| matches!(&w.reason, MacroWarningReason::MalformedProcMacro { .. }))
        .count();
    assert_eq!(
        malformed_count, 1,
        "one malformed derive warning expected: {:?}",
        catalog.warnings
    );
}

#[test]
fn multiple_derive_attrs_dedup_and_sort_helpers_per_attr() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &proc_toml()),
            (
                "src/lib.rs",
                "use proc_macro::TokenStream;\n\
                 #[proc_macro_derive(Zebra, attributes(zz, aa, zz))]\n\
                 #[proc_macro_derive(Apple, attributes(b, a, c))]\n\
                 pub fn dual_derive(input: TokenStream) -> TokenStream { input }\n",
            ),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    let zebra = catalog.records.iter().find(|r| r.name == "Zebra").unwrap();
    let apple = catalog.records.iter().find(|r| r.name == "Apple").unwrap();
    assert_eq!(zebra.helpers, vec!["aa", "zz"]);
    assert_eq!(apple.helpers, vec!["a", "b", "c"]);
}

// =========================================================================
// Issue 5: Conservative pub use macro warnings
// =========================================================================

#[test]
fn pub_use_of_resolved_local_item_suppresses_macro_warning() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &lib_toml()),
            (
                "src/lib.rs",
                "mod types_mod { pub struct KnownType; }\n\
                 pub use types_mod::KnownType;\n\
                 mod fn_mod { pub fn known_fn() {} }\n\
                 pub use fn_mod::known_fn;\n",
            ),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    // These pub use statements refer to known non-macro local items (struct and fn).
    // They should NOT emit macro warnings.
    assert!(
        !catalog
            .warnings
            .iter()
            .any(|w| w.path == "crate::KnownType" || w.path == "crate::known_fn"),
        "pub use of known non-macro local items should suppress macro warnings: {:?}",
        catalog.warnings
    );
}

#[test]
fn genuinely_unresolved_pub_use_still_warns() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &lib_toml()),
            ("src/lib.rs", "pub use completely::unknown::path;\n"),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    // If the path's first segment is not a local module and not external, it should warn.
    assert!(
        !catalog.warnings.is_empty(),
        "genuinely unresolved paths must still warn"
    );
}

// =========================================================================
// Bug 1: Private #[proc_macro_derive] must not emit derive record
// =========================================================================

#[test]
fn private_proc_macro_derive_must_not_emit_record() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &proc_toml()),
            (
                "src/lib.rs",
                "use proc_macro::TokenStream;\n\
                 #[proc_macro_derive(SecretDerive)]\n\
                 fn secret_derive(_input: TokenStream) -> TokenStream {\n\
                 \x20\x20\x20\x20unimplemented!()\n\
                 }\n",
            ),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    // Must NOT emit a derive record for a non-pub function.
    assert!(
        !catalog.records.iter().any(|r| r.name == "SecretDerive"),
        "private fn with #[proc_macro_derive] must not emit a record: {:?}",
        paths(&catalog)
    );
    // Should emit a malformed-proc-macro warning.
    let malformed_count = catalog
        .warnings
        .iter()
        .filter(|w| matches!(&w.reason, MacroWarningReason::MalformedProcMacro { .. }))
        .count();
    assert_eq!(
        malformed_count, 1,
        "private #[proc_macro_derive] must emit one MalformedProcMacro warning: {:?}",
        catalog.warnings
    );
}

#[test]
fn private_proc_macro_derive_with_valid_derive_alongside_still_suppressed() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &proc_toml()),
            (
                "src/lib.rs",
                "use proc_macro::TokenStream;\n\
                 #[proc_macro]\n\
                 pub fn ok_macro(input: TokenStream) -> TokenStream { input }\n\
                 #[proc_macro_derive(HiddenDerive)]\n\
                 fn hidden_derive(_input: TokenStream) -> TokenStream {\n\
                 \x20\x20\x20\x20unimplemented!()\n\
                 }\n",
            ),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    // The public proc_macro should still be recorded.
    assert!(
        catalog.records.iter().any(|r| r.name == "ok_macro"),
        "public proc_macro must still be recorded: {:?}",
        paths(&catalog)
    );
    // The private derive must not be recorded.
    assert!(
        !catalog.records.iter().any(|r| r.name == "HiddenDerive"),
        "private #[proc_macro_derive] must not emit a record"
    );
    // One malformed warning for the private derive.
    let malformed_count = catalog
        .warnings
        .iter()
        .filter(|w| matches!(&w.reason, MacroWarningReason::MalformedProcMacro { .. }))
        .count();
    assert_eq!(
        malformed_count, 1,
        "private proc_macro_derive must emit one warning: {:?}",
        catalog.warnings
    );
}

// =========================================================================
// Bug 2: pub use macro_name as _ creates no record or warning
// =========================================================================

#[test]
fn pub_use_as_underscore_is_silent_noop() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &lib_toml()),
            (
                "src/lib.rs",
                "#[macro_export]\n\
                 macro_rules! known { () => {}; }\n\
                 pub use crate::known as _;\n",
            ),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    // The underlying macro is still recorded.
    assert!(
        catalog.records.iter().any(|r| r.name == "known"),
        "underlying declared macro must still be recorded"
    );
    // No record with name `_` or path ending in `_`.
    assert!(
        !catalog.records.iter().any(|r| r.name == "_"),
        "as _ must not create a record named '_': {:?}",
        paths(&catalog)
    );
    // No warning for the as _ discard.
    assert!(
        catalog.warnings.is_empty(),
        "as _ must not emit warnings: {:?}",
        catalog.warnings
    );
}

#[test]
fn pub_use_of_unresolved_as_underscore_is_also_noop() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &lib_toml()),
            ("src/lib.rs", "pub use nonexistent_macro as _;\n"),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    assert!(
        catalog.records.is_empty(),
        "as _ with unresolved path must not fabricate a record"
    );
    // It's a genuine discard — not even a warning is emitted.
    assert!(
        catalog.warnings.is_empty(),
        "as _ with unresolved path must not warn: {:?}",
        catalog.warnings
    );
}

// =========================================================================
// Bug 3: Conclusive non-macro suppression handles arbitrary-depth paths
// =========================================================================

#[test]
fn conclusive_suppression_handles_crate_qualified_arbitrary_depth() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &lib_toml()),
            (
                "src/lib.rs",
                "pub mod alpha;\n\
                 pub use crate::alpha::beta::Gamma;\n",
            ),
            ("src/alpha.rs", "pub mod beta;\n"),
            ("src/alpha/beta.rs", "pub struct Gamma;\n"),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    // Gamma is a known struct in crate::alpha::beta — suppress the macro warning.
    assert!(
        !catalog.warnings.iter().any(|w| w.path == "crate::Gamma"),
        "crate::alpha::beta::Gamma must be conclusively non-macro (struct): {:?}",
        catalog.warnings
    );
}

#[test]
fn conclusive_suppression_handles_self_qualified_path() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &lib_toml()),
            (
                "src/lib.rs",
                "mod inner {\n\
                     pub enum Color { Red, Green }\n\
                     pub use self::Color;\n\
                 }\n",
            ),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    // self::Color in module crate::inner resolves to crate::inner::Color (an enum).
    // No macro warning should be emitted for it.
    assert!(
        !catalog.warnings.iter().any(|w| w.path.contains("Color")),
        "self::Color must be conclusively non-macro: {:?}",
        catalog.warnings
    );
}

#[test]
fn conclusive_suppression_handles_super_paths() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &lib_toml()),
            (
                "src/lib.rs",
                "pub fn root_fn() {}\n\
                 mod child {\n\
                     pub use super::root_fn;\n\
                 }\n",
            ),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    // super::root_fn resolves to crate::root_fn (a fn) — suppress.
    assert!(
        !catalog.warnings.iter().any(|w| w.path.contains("root_fn")),
        "super::root_fn must be conclusively non-macro: {:?}",
        catalog.warnings
    );
}

#[test]
fn conclusive_suppression_handles_repeated_super_paths() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &lib_toml()),
            (
                "src/lib.rs",
                "pub struct TopStruct;\n\
                 mod level1 {\n\
                     mod level2 {\n\
                         pub use super::super::TopStruct;\n\
                     }\n\
                 }\n",
            ),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    // super::super::TopStruct in crate::level1::level2 resolves to crate::TopStruct.
    assert!(
        !catalog
            .warnings
            .iter()
            .any(|w| w.path.contains("TopStruct")),
        "repeated super::super::TopStruct must be conclusively non-macro: {:?}",
        catalog.warnings
    );
}

#[test]
fn conclusive_suppression_handles_single_segment_same_scope_item() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &lib_toml()),
            (
                "src/lib.rs",
                "pub const ANSWER: u32 = 42;\n\
                 pub use ANSWER;\n",
            ),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    // ANSWER is a known const in the same scope — suppress.
    assert!(
        !catalog.warnings.iter().any(|w| w.path == "crate::ANSWER"),
        "single-segment same-scope ANSWER must be conclusively non-macro: {:?}",
        catalog.warnings
    );
}

#[test]
fn conclusive_suppression_in_external_file_module_with_nesting() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &lib_toml()),
            ("src/lib.rs", "pub mod outer;\n"),
            (
                "src/outer.rs",
                "pub mod inner;\n\
                 pub use crate::outer::inner::NestedType;\n",
            ),
            ("src/outer/inner.rs", "pub struct NestedType;\n"),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    // NestedType in crate::outer must be suppressed — it's a known struct.
    assert!(
        !catalog
            .warnings
            .iter()
            .any(|w| w.path == "crate::outer::NestedType"),
        "external file module path must be conclusively non-macro: {:?}",
        catalog.warnings
    );
}

#[test]
fn conclusive_suppression_in_nested_inline_module_with_mixed_paths() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &lib_toml()),
            (
                "src/lib.rs",
                "mod a {\n\
                     mod b {\n\
                         pub type Alias = i32;\n\
                         pub use crate::a::b::Alias;\n\
                     }\n\
                 }\n",
            ),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    // Alias is a known type alias — suppress.
    assert!(
        !catalog.warnings.iter().any(|w| w.path.contains("Alias")),
        "nested inline module crate::a::b::Alias must be conclusively non-macro: {:?}",
        catalog.warnings
    );
}

#[test]
fn ambiguous_paths_still_warn() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &lib_toml()),
            ("src/lib.rs", "pub use crate::alpha::UnknownType;\n"),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");
    // UnknownType is not a known item anywhere — must still warn.
    assert!(
        catalog
            .warnings
            .iter()
            .any(|w| matches!(&w.reason, MacroWarningReason::UnresolvedReexport { .. })),
        "unknown path must still warn: {:?}",
        catalog.warnings
    );
}

// =========================================================================
// Bug 4: Exact canonical path matching (no loose ends_with)
// =========================================================================

#[test]
fn canonical_resolver_avoids_ends_with_module_name_collision() {
    // Two subtrees both have a module named `common` (`crate::group_a::common`
    // and `crate::group_b::common`). `KnownType` exists ONLY in group_a's common.
    // A `pub use common::KnownType` inside group_b must NOT resolve via group_a.
    // The current `ends_with("::common")` check matches both, causing false suppression.
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("Cargo.toml", &lib_toml()),
            (
                "src/lib.rs",
                "pub mod group_a;\n\
                 pub mod group_b;\n",
            ),
            ("src/group_a.rs", "pub mod common;\n"),
            (
                "src/group_a/common.rs",
                // KnownType exists here — only in group_a's common.
                "pub struct KnownType;\n",
            ),
            (
                "src/group_b.rs",
                "pub mod common;\n\
                 // common::KnownType relative to group_b resolves to\n\
                 // crate::group_b::common::KnownType which does NOT exist.\n\
                 // The ends_with bug would find crate::group_a::common::KnownType\n\
                 // and falsely suppress the warning.\n\
                 pub use common::KnownType;\n",
            ),
            (
                "src/group_b/common.rs",
                // This module does NOT have KnownType — only OtherType.
                "pub struct OtherType;\n",
            ),
        ],
    );

    let catalog = macros_for(temp.path(), "src/lib.rs");

    // common::KnownType inside group_b does not resolve to a known non-macro
    // item in crate::group_b::common. Must produce an UnresolvedReexport warning.
    let unresolved: Vec<&MacroWarningReason> = catalog.warnings.iter().map(|w| &w.reason).collect();
    assert!(
        catalog
            .warnings
            .iter()
            .any(|w| w.path == "crate::group_b::KnownType"
                && matches!(&w.reason, MacroWarningReason::UnresolvedReexport { .. })),
        "must warn about KnownType not existing in group_b::common (ends_with collision): {unresolved:?}"
    );
}

#[test]
fn real_amari_core_exports_geo_and_wedge_macros() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("amari-discovery is inside the workspace");

    let inventory = inventory_workspace(workspace_root).unwrap();
    let graph = module_graph(workspace_root, "amari-core/src/lib.rs").unwrap();
    let catalog = macro_catalog(&graph, &inventory, "amari-core", workspace_root).unwrap();

    let mut exported: Vec<String> = catalog.records.iter().map(|r| r.name.clone()).collect();
    exported.sort();
    exported.dedup();

    // amari-core unicode_ops.rs has #[macro_export] geo!, wedge!, etc.
    for expected in &["geo", "wedge", "dot", "lcon", "rcon", "dual", "rev"] {
        assert!(
            exported.contains(&expected.to_string()),
            "amari-core must export {expected}: got {exported:?}"
        );
    }

    // All amari-core macros should be Declarative.
    for record in &catalog.records {
        assert_eq!(
            record.kind,
            MacroKind::Declarative,
            "amari-core macro {} must be Declarative, got {:?}",
            record.name,
            record.kind
        );
    }

    // Geo and wedge should have source pointing to unicode_ops.rs.
    for name in &["geo", "wedge"] {
        let record = catalog
            .records
            .iter()
            .find(|r| &r.name == name)
            .expect("macro must exist");
        assert!(
            record.source.source_path.contains("unicode_ops"),
            "{name} source should be unicode_ops.rs, got {}",
            record.source.source_path
        );
    }
}

#[test]
fn real_amari_flynn_macros_has_attribute_and_derive_macros() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("amari-discovery is inside the workspace");

    let inventory = inventory_workspace(workspace_root).unwrap();
    let graph = module_graph(workspace_root, "amari-flynn-macros/src/lib.rs").unwrap();
    let catalog = macro_catalog(&graph, &inventory, "amari-flynn-macros", workspace_root).unwrap();

    let names: Vec<String> = catalog.records.iter().map(|r| r.name.clone()).collect();
    // amari-flynn-macros has: prob_requires, prob_ensures, ensures_expected
    // as #[proc_macro_attribute]
    for expected in &["prob_requires", "prob_ensures", "ensures_expected"] {
        let record = catalog
            .records
            .iter()
            .find(|r| &r.name == expected)
            .unwrap_or_else(|| panic!("{expected} must exist in flynn-macros: {names:?}"));
        assert_eq!(
            record.kind,
            MacroKind::ProcMacroAttribute,
            "{expected}: expected ProcMacroAttribute, got {:?}",
            record.kind
        );
        assert!(
            record.signature.contains("pub fn"),
            "{expected} signature missing `pub fn`: {}",
            record.signature
        );
    }
}
