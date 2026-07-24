// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tests for Task 5B3: normalized signature and associated-item extraction.

use std::{fs, path::Path};

use amari_discovery::catalog::generator::{
    export_graph, module_graph, signature_catalog, AggregateShape, AssociatedItem, AssociatedKind,
    FieldLabel, SignatureCatalog, SignatureKind, VariantData,
};
use amari_discovery::DiscoveryError;
use tempfile::TempDir;

fn write_package(root: &Path, files: &[(&str, &str)]) {
    for (relative, source) in files {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, source).unwrap();
    }
}

/// Builds the module graph, export graph, and signature catalog for a package.
fn catalog_for(root: &Path, source_path: &str) -> SignatureCatalog {
    let graph = module_graph(root, source_path).unwrap();
    let exports = export_graph(&graph, root).unwrap();
    signature_catalog(&graph, &exports, root).unwrap()
}

/// Tokens of a normalized signature, whitespace-collapsed for stable matching.
fn tokens(signature: &str) -> Vec<&str> {
    signature.split_whitespace().collect()
}

/// Returns the single record projected at `path`, panicking if not exactly one.
fn one<'a>(
    catalog: &'a SignatureCatalog,
    path: &str,
) -> &'a amari_discovery::catalog::generator::SignatureRecord {
    let matches: Vec<_> = catalog.records.iter().filter(|r| r.path == path).collect();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one record at `{path}`, got {matches:?}"
    );
    matches[0]
}

fn associated_named<'a>(items: &'a [AssociatedItem], name: &str) -> &'a AssociatedItem {
    items
        .iter()
        .find(|item| item.name == name)
        .unwrap_or_else(|| panic!("no associated item named `{name}`"))
}

// ---------------------------------------------------------------------------
// Function signatures: generics, where clauses, qualifiers, return types.
// ---------------------------------------------------------------------------

#[test]
fn function_signature_preserves_generics_where_and_return_without_body() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub fn map<T>(x: T) -> Option<T>\n\
             where\n\
             \x20\x20\x20\x20T: Clone,\n\
             {\n\
             \x20\x20\x20\x20None\n\
             }\n",
        )],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");
    let record = one(&catalog, "crate::map");
    assert_eq!(record.kind, SignatureKind::Function);
    let toks = tokens(&record.signature);
    assert!(toks.starts_with(&["pub", "fn", "map"]), "got {toks:?}");
    assert!(toks.contains(&"->"), "return arrow missing: {toks:?}");
    assert!(toks.contains(&"Option"), "return type missing: {toks:?}");
    assert!(
        toks.windows(2).any(|w| w == ["where", "T"]),
        "where missing: {toks:?}"
    );
    assert!(toks.contains(&"Clone"), "bound missing: {toks:?}");
    // No implementation body leaks into the signature.
    assert!(!toks.contains(&"None"), "body leaked: {toks:?}");
    assert!(!toks.contains(&"{"), "brace leaked: {toks:?}");
    assert!(record.shape.is_none());
    assert!(record.associated.is_empty());
}

#[test]
fn function_qualifiers_const_async_unsafe_extern_are_preserved() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("src/a.rs", "pub async fn asy() {}\n"),
            ("src/b.rs", "pub unsafe fn uns() {}\n"),
            ("src/c.rs", "pub extern \"C\" fn ext() {}\n"),
            (
                "src/lib.rs",
                "pub mod a;\npub mod b;\npub mod c;\npub const fn cst() -> u8 { 0 }\n",
            ),
        ],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");
    let cst = tokens(&one(&catalog, "crate::cst").signature);
    assert!(
        cst.starts_with(&["pub", "const", "fn", "cst"]),
        "const qualifier missing: {cst:?}"
    );
    assert!(cst.contains(&"u8"));
    assert!(tokens(&one(&catalog, "crate::a::asy").signature).contains(&"async"));
    assert!(tokens(&one(&catalog, "crate::b::uns").signature).contains(&"unsafe"));
    let ext = tokens(&one(&catalog, "crate::c::ext").signature);
    assert!(ext.contains(&"extern"), "extern missing: {ext:?}");
    assert!(ext.contains(&"\"C\""), "abi missing: {ext:?}");
}

// ---------------------------------------------------------------------------
// Struct / union / enum public API shape.
// ---------------------------------------------------------------------------

#[test]
fn struct_shape_keeps_public_fields_and_drops_private_and_restricted() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub struct Mixed {\n\
             \x20\x20\x20\x20pub visible: u8,\n\
             \x20\x20\x20\x20hidden: u32,\n\
             \x20\x20\x20\x20pub(crate) restricted: u16,\n\
             }\n",
        )],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");
    let record = one(&catalog, "crate::Mixed");
    assert_eq!(record.kind, SignatureKind::Struct);
    assert_eq!(record.signature, "pub struct Mixed");
    let AggregateShape::Struct { fields } = record.shape.as_ref().unwrap() else {
        panic!("expected struct shape: {:?}", record.shape);
    };
    let names: Vec<&str> = fields
        .iter()
        .filter_map(|f| match &f.label {
            FieldLabel::Named(name) => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(names, vec!["visible"]);
    assert_eq!(fields[0].ty, "u8");
    // No private or restricted field name appears anywhere in the shape.
    assert!(
        fields.iter().all(|f| match &f.label {
            FieldLabel::Named(n) => n != "hidden" && n != "restricted",
            _ => true,
        }),
        "private/restricted fields leaked: {fields:?}"
    );
}

#[test]
fn tuple_struct_shape_keeps_only_public_positional_fields() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[("src/lib.rs", "pub struct Pair(pub u8, u32);\n")],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");
    let record = one(&catalog, "crate::Pair");
    let AggregateShape::Struct { fields } = record.shape.as_ref().unwrap() else {
        panic!("expected struct shape");
    };
    assert_eq!(fields.len(), 1);
    assert!(matches!(fields[0].label, FieldLabel::Positional(0)));
    assert_eq!(fields[0].ty, "u8");
}

#[test]
fn unit_struct_has_empty_field_shape() {
    let temp = TempDir::new().unwrap();
    write_package(temp.path(), &[("src/lib.rs", "pub struct Empty;\n")]);

    let catalog = catalog_for(temp.path(), "src/lib.rs");
    let record = one(&catalog, "crate::Empty");
    let AggregateShape::Struct { fields } = record.shape.as_ref().unwrap() else {
        panic!("expected struct shape");
    };
    assert!(fields.is_empty());
}

#[test]
fn enum_shape_records_unit_tuple_and_struct_variants() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub enum Msg {\n\
             \x20\x20\x20\x20Quit,\n\
             \x20\x20\x20\x20Move { x: u8, y: u8 },\n\
             \x20\x20\x20\x20Write(String),\n\
             }\n",
        )],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");
    let record = one(&catalog, "crate::Msg");
    assert_eq!(record.kind, SignatureKind::Enum);
    let AggregateShape::Enum { variants } = record.shape.as_ref().unwrap() else {
        panic!("expected enum shape");
    };
    let by_name: std::collections::HashMap<&str, &VariantData> = variants
        .iter()
        .map(|v| (v.name.as_str(), &v.data))
        .collect();
    assert!(matches!(by_name["Quit"], VariantData::Unit));
    match by_name["Move"] {
        VariantData::Struct(fields) => {
            let names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
            assert_eq!(names, vec!["x", "y"]);
        }
        other => panic!("Move should be struct variant: {other:?}"),
    }
    match by_name["Write"] {
        VariantData::Tuple(tys) => assert_eq!(tys, &["String".to_owned()]),
        other => panic!("Write should be tuple variant: {other:?}"),
    }
}

#[test]
fn union_shape_keeps_only_public_fields() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[("src/lib.rs", "pub union U { pub hi: u8, lo: u8 }\n")],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");
    let record = one(&catalog, "crate::U");
    assert_eq!(record.kind, SignatureKind::Union);
    let AggregateShape::Union { fields } = record.shape.as_ref().unwrap() else {
        panic!("expected union shape");
    };
    let names: Vec<&str> = fields
        .iter()
        .filter_map(|f| match &f.label {
            FieldLabel::Named(n) => Some(n.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(names, vec!["hi"]);
}

// ---------------------------------------------------------------------------
// Generics with lifetimes and const generics.
// ---------------------------------------------------------------------------

#[test]
fn struct_generics_preserve_lifetimes_and_const_generics() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub struct Buf<'a, const N: usize> { pub data: &'a [u8; N] }\n",
        )],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");
    let record = one(&catalog, "crate::Buf");
    let toks = tokens(&record.signature);
    assert!(toks.contains(&"'a"), "lifetime missing: {toks:?}");
    assert!(toks.contains(&"const"), "const generic missing: {toks:?}");
    assert!(toks.contains(&"N"), "const generic name missing: {toks:?}");
    let AggregateShape::Struct { fields } = record.shape.as_ref().unwrap() else {
        panic!("expected struct shape");
    };
    assert_eq!(fields[0].ty, "& 'a [u8 ; N]");
}

// ---------------------------------------------------------------------------
// Type aliases, consts, statics.
// ---------------------------------------------------------------------------

#[test]
fn type_alias_signature_includes_target_type() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[("src/lib.rs", "pub type Ptr<T> = Option<T>;\n")],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");
    let record = one(&catalog, "crate::Ptr");
    assert_eq!(record.kind, SignatureKind::TypeAlias);
    assert_eq!(record.signature, "pub type Ptr < T > = Option < T >");
}

#[test]
fn const_and_static_signatures_drop_values_but_keep_types() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub const N: u8 = 9;\npub static G: u8 = 0;\npub static mut M: u8 = 0;\n",
        )],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");
    let n = one(&catalog, "crate::N");
    assert_eq!(n.kind, SignatureKind::Constant);
    assert_eq!(n.signature, "pub const N : u8");
    assert!(!tokens(&n.signature).contains(&"9"), "value leaked");

    let g = one(&catalog, "crate::G");
    assert_eq!(g.kind, SignatureKind::Static);
    assert_eq!(g.signature, "pub static G : u8");

    let m = one(&catalog, "crate::M");
    let mt = tokens(&m.signature);
    assert!(
        mt.contains(&"static") && mt.contains(&"mut"),
        "mut static: {mt:?}"
    );
}

// ---------------------------------------------------------------------------
// Trait associated items: required vs provided distinction.
// ---------------------------------------------------------------------------

#[test]
fn trait_associated_items_record_required_and_provided() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub trait Container {\n\
             \x20\x20\x20\x20type Item;\n\
             \x20\x20\x20\x20const SIZE: usize;\n\
             \x20\x20\x20\x20fn required(&self) -> u8;\n\
             \x20\x20\x20\x20fn provided(&self) -> u8 { 0 }\n\
             }\n",
        )],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");
    let record = one(&catalog, "crate::Container");
    assert_eq!(record.kind, SignatureKind::Trait);
    assert_eq!(record.signature, "pub trait Container");

    let item = associated_named(&record.associated, "Item");
    assert_eq!(item.kind, AssociatedKind::Type);
    assert!(!item.has_default, "Item should be required");

    let size = associated_named(&record.associated, "SIZE");
    assert_eq!(size.kind, AssociatedKind::Const);
    assert!(!size.has_default, "SIZE should be required");

    let required = associated_named(&record.associated, "required");
    assert_eq!(required.kind, AssociatedKind::Method);
    assert!(!required.has_default, "required should be required");

    let provided = associated_named(&record.associated, "provided");
    assert_eq!(provided.kind, AssociatedKind::Method);
    assert!(provided.has_default, "provided should have default");
    // The default body must not leak into the signature.
    assert!(!tokens(&provided.signature).contains(&"0"));
}

// ---------------------------------------------------------------------------
// Trait associated type bounds, defaults, and trait associated const default.
// ---------------------------------------------------------------------------

#[test]
fn trait_associated_type_bounds_and_defaults_and_const_default() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub trait BoundsAndDefaults {\n\
             \x20\x20\x20\x20type Bare;\n\
             \x20\x20\x20\x20type Bounded: Clone;\n\
             \x20\x20\x20\x20type WithDefault = u32;\n\
             \x20\x20\x20\x20type BoundedDefault: std::fmt::Debug = String;\n\
             \x20\x20\x20\x20const REQUIRED: u8;\n\
             \x20\x20\x20\x20const WITH_DEFAULT: u8 = 42;\n\
             }\n",
        )],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");
    let record = one(&catalog, "crate::BoundsAndDefaults");
    assert_eq!(record.kind, SignatureKind::Trait);

    // Bare type -- no bounds, no default.
    let bare = associated_named(&record.associated, "Bare");
    assert_eq!(bare.kind, AssociatedKind::Type);
    assert!(!bare.has_default, "Bare should be required");
    assert_eq!(bare.signature, "type Bare");
    assert!(!bare.signature.contains(':'), "unexpected bound");

    // Bounded type -- has bounds, no default.
    let bounded = associated_named(&record.associated, "Bounded");
    assert_eq!(bounded.kind, AssociatedKind::Type);
    assert!(!bounded.has_default, "Bounded should be required");
    let bt = tokens(&bounded.signature);
    assert!(
        bt.contains(&"Clone"),
        "bounds missing from signature: {bt:?}"
    );

    // Type with default, no bounds.
    let wd = associated_named(&record.associated, "WithDefault");
    assert_eq!(wd.kind, AssociatedKind::Type);
    assert!(wd.has_default, "WithDefault should have default");
    // Default value must not leak into the signature.
    assert_eq!(wd.signature, "type WithDefault");

    // Bounded type with default.
    let bd = associated_named(&record.associated, "BoundedDefault");
    assert_eq!(bd.kind, AssociatedKind::Type);
    assert!(bd.has_default, "BoundedDefault should have default");
    let bdt = tokens(&bd.signature);
    assert!(bdt.contains(&"Debug"), "bounds missing: {bdt:?}");
    assert!(!bdt.contains(&"="), "equals leaked: {bdt:?}");
    assert!(!bdt.contains(&"String"), "default value leaked: {bdt:?}");

    // Const required -- no default.
    let required = associated_named(&record.associated, "REQUIRED");
    assert_eq!(required.kind, AssociatedKind::Const);
    assert!(!required.has_default, "REQUIRED should be required");
    let rt = tokens(&required.signature);
    assert!(
        rt.contains(&"const") && rt.contains(&"REQUIRED") && rt.contains(&"u8"),
        "signature malformed: {rt:?}"
    );
    assert!(!rt.contains(&"="), "value leaked: {rt:?}");

    // Const with default.
    let wdc = associated_named(&record.associated, "WITH_DEFAULT");
    assert_eq!(wdc.kind, AssociatedKind::Const);
    assert!(wdc.has_default, "WITH_DEFAULT should have default");
    let wdct = tokens(&wdc.signature);
    assert!(
        wdct.contains(&"const") && wdct.contains(&"WITH_DEFAULT") && wdct.contains(&"u8"),
        "signature malformed: {wdct:?}"
    );
    assert!(!wdct.contains(&"42"), "value leaked: {wdct:?}");
}

// ---------------------------------------------------------------------------
// Trait declaration supertrait bounds preserved in normalized signature.
// 5B4 owns relationship analysis; 5B3 only preserves what was written.
// ---------------------------------------------------------------------------

#[test]
fn trait_signature_retains_supertrait_bounds() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub trait Super { fn base(&self); }\n\
             pub trait Sub: Super { fn extra(&self); }\n\
             pub trait Multi: Super + Clone { }\n",
        )],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");

    // Base trait -- no supertrait bounds, no colon.
    let sup = one(&catalog, "crate::Super");
    assert_eq!(sup.kind, SignatureKind::Trait);
    assert_eq!(sup.signature, "pub trait Super");
    assert!(sup.associated.iter().any(|i| i.name == "base"));

    // Sub trait -- supertrait bound preserved in signature.
    let sub = one(&catalog, "crate::Sub");
    assert_eq!(sub.kind, SignatureKind::Trait);
    let st = tokens(&sub.signature);
    assert!(st.contains(&":"), "colon missing: {st:?}");
    assert!(st.contains(&"Super"), "supertrait bound missing: {st:?}");
    // No relationship analysis: only the normalized signature is stored.
    // The associated items are the trait's own items, not inherited.
    assert!(
        sub.associated.iter().any(|i| i.name == "extra"),
        "own item missing"
    );
    assert!(
        !sub.associated.iter().any(|i| i.name == "base"),
        "inherited item must not appear before 5B4"
    );

    // Multi-super trait -- all bounds preserved.
    let multi = one(&catalog, "crate::Multi");
    let mt = tokens(&multi.signature);
    assert!(mt.contains(&"Super"), "Super missing: {mt:?}");
    assert!(mt.contains(&"Clone"), "Clone missing: {mt:?}");
}

// ---------------------------------------------------------------------------
// Inherent methods: public only, receiver forms, private excluded.
// ---------------------------------------------------------------------------

#[test]
fn inherent_methods_keep_public_receivers_and_drop_private() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub struct Svc;\n\
             impl Svc {\n\
             \x20\x20\x20\x20pub fn new() -> Self { Self }\n\
             \x20\x20\x20\x20pub fn borrow(&self) {}\n\
             \x20\x20\x20\x20pub fn alter(&mut self) {}\n\
             \x20\x20\x20\x20fn hidden() {}\n\
             \x20\x20\x20\x20pub(crate) fn crate_only() {}\n\
             }\n",
        )],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");
    let record = one(&catalog, "crate::Svc");
    let names: Vec<&str> = record.associated.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, vec!["alter", "borrow", "new"]);

    let borrow = associated_named(&record.associated, "borrow");
    assert!(
        borrow.signature.contains('&') && borrow.signature.contains("self"),
        "ref receiver: {}",
        borrow.signature
    );

    let alter = associated_named(&record.associated, "alter");
    assert!(
        alter.signature.contains("mut") && alter.signature.contains("self"),
        "mut receiver: {}",
        alter.signature
    );

    let new = associated_named(&record.associated, "new");
    assert!(
        !new.signature.contains("self"),
        "static fn should have no self"
    );
}

// ---------------------------------------------------------------------------
// Inherent associated const: public only, value stripped, private excluded.
// ---------------------------------------------------------------------------

#[test]
fn inherent_associated_const_strips_value_and_excludes_private_and_restricted() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub struct Bucket;\n\
             impl Bucket {\n\
             \x20\x20\x20\x20pub const CAP: usize = 1024;\n\
             \x20\x20\x20\x20const HIDDEN: u8 = 0;\n\
             \x20\x20\x20\x20pub(crate) const RESTRICTED: u8 = 0;\n\
             }\n",
        )],
    );

    // Inherent associated types are not stable in edition 2021 / rust-version
    // 1.75, so this test covers only associated consts.

    let catalog = catalog_for(temp.path(), "src/lib.rs");
    let record = one(&catalog, "crate::Bucket");
    let names: Vec<&str> = record.associated.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["CAP"],
        "only public const should appear: {names:?}"
    );

    let cap = associated_named(&record.associated, "CAP");
    assert_eq!(cap.kind, AssociatedKind::Const);
    assert!(
        !cap.has_default,
        "inherent items always report has_default=false"
    );
    // Signature includes the type but strips the default value.
    let toks = tokens(&cap.signature);
    assert!(toks.contains(&"const"), "missing const keyword: {toks:?}");
    assert!(toks.contains(&"CAP"), "missing name: {toks:?}");
    assert!(toks.contains(&"usize"), "missing type: {toks:?}");
    assert!(!toks.contains(&"1024"), "default value leaked: {toks:?}");
    assert!(!toks.contains(&"="), "equals leaked: {toks:?}");

    // Private and restricted consts must be excluded.
    assert!(
        record.associated.iter().all(|i| i.name != "HIDDEN"),
        "private const leaked"
    );
    assert!(
        record.associated.iter().all(|i| i.name != "RESTRICTED"),
        "restricted const leaked"
    );
}

// ---------------------------------------------------------------------------
// Alias projection: associated items under every reachable export alias.
// ---------------------------------------------------------------------------

#[test]
fn struct_methods_project_under_alias_and_original_path() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            (
                "src/lib.rs",
                "pub mod defs;\npub use defs::Real as Alias;\n",
            ),
            (
                "src/defs.rs",
                "pub struct Real;\nimpl Real { pub fn hello(&self) {} }\n",
            ),
        ],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");
    let alias = one(&catalog, "crate::Alias");
    assert_eq!(alias.kind, SignatureKind::Struct);
    assert_eq!(
        alias.signature, "pub struct Real",
        "alias receives source declaration signature"
    );
    assert_eq!(alias.source.module, "crate::defs");
    assert_eq!(alias.source.source_path, "src/defs.rs");
    assert_eq!(alias.source.ident, "Real");
    assert!(alias.associated.iter().any(|i| i.name == "hello"));

    let original = one(&catalog, "crate::defs::Real");
    assert!(original.associated.iter().any(|i| i.name == "hello"));
}

#[test]
fn alias_of_private_source_projects_methods_and_keeps_source_identity() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            (
                "src/lib.rs",
                "mod private_thing;\npub use private_thing::Thing as Alias;\n",
            ),
            (
                "src/private_thing.rs",
                "pub struct Thing;\nimpl Thing { pub fn act(&self) {} }\n",
            ),
        ],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");
    // The private module itself is not exported.
    assert!(catalog
        .records
        .iter()
        .all(|r| r.path != "crate::private_thing"));
    assert!(
        catalog
            .records
            .iter()
            .all(|r| r.path != "crate::private_thing::Thing"),
        "private-module path must not be exported"
    );
    // But its public type is reachable through the alias, with its source
    // identity retained and associated methods projected under the alias.
    let alias = one(&catalog, "crate::Alias");
    assert_eq!(alias.kind, SignatureKind::Struct);
    assert_eq!(alias.source.module, "crate::private_thing");
    assert_eq!(alias.source.source_path, "src/private_thing.rs");
    assert_eq!(alias.source.ident, "Thing");
    assert!(alias.associated.iter().any(|i| i.name == "act"));
}

#[test]
fn trait_associated_items_project_under_alias() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("src/lib.rs", "pub mod defs;\npub use defs::Tr as AliasT;\n"),
            ("src/defs.rs", "pub trait Tr { fn m(&self); }\n"),
        ],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");
    let alias = one(&catalog, "crate::AliasT");
    assert_eq!(alias.kind, SignatureKind::Trait);
    assert!(alias.associated.iter().any(|i| i.name == "m"));
}

// ---------------------------------------------------------------------------
// Exclusions: private types and trait-impl items stay out of 5B3.
// ---------------------------------------------------------------------------

#[test]
fn private_type_impls_are_excluded() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "struct Hidden;\nimpl Hidden { pub fn leak() {} }\npub fn keep() {}\n",
        )],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");
    assert!(
        catalog.records.iter().all(|r| r.path != "crate::Hidden"),
        "private type must not get a signature record"
    );
    assert!(
        catalog
            .records
            .iter()
            .all(|r| !r.associated.iter().any(|i| i.name == "leak")),
        "private type method must not leak into any record"
    );
    // The exported function is still indexed.
    assert_eq!(one(&catalog, "crate::keep").kind, SignatureKind::Function);
}

#[test]
fn trait_impl_items_are_not_emitted_as_inherent() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub trait Tr { fn m(&self); }\n\
             pub struct S;\n\
             impl Tr for S { fn m(&self) {} }\n\
             impl S { pub fn inherent(&self) {} }\n",
        )],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");
    let s = one(&catalog, "crate::S");
    let names: Vec<&str> = s.associated.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["inherent"],
        "trait-impl method must not appear: {names:?}"
    );

    // The trait's own body item is still recorded under the trait.
    let tr = one(&catalog, "crate::Tr");
    assert!(tr.associated.iter().any(|i| i.name == "m"));
}

// ---------------------------------------------------------------------------
// Transitive glob/named import chains: impl self-type resolution via
// multi-hop glob re-exports.
// ---------------------------------------------------------------------------

#[test]
fn transitive_glob_import_chain_resolves_impl_owner() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            (
                "src/lib.rs",
                "pub mod origin;\n\
                 pub mod prelude;\n\
                 pub mod user;\n\
                 pub use prelude::Thing;\n",
            ),
            ("src/origin.rs", "pub struct Thing;\n"),
            ("src/prelude.rs", "pub use crate::origin::*;\n"),
            (
                "src/user.rs",
                "use crate::prelude::*;\n\
                 impl Thing {\n\
                 \x20\x20\x20\x20pub fn user_method(&self) {}\n\
                 }\n",
            ),
        ],
    );

    // This is the RED test: the method from `user.rs`'s impl must appear
    // on every reachable export alias of `Thing`, even though the type
    // reaches `user` through a transitive glob chain
    // (origin → prelude glob → user glob).
    let catalog = catalog_for(temp.path(), "src/lib.rs");

    // The direct re-export `prelude::Thing` from lib.rs
    let thing = one(&catalog, "crate::Thing");
    assert!(
        thing.associated.iter().any(|i| i.name == "user_method"),
        "user_method must appear on crate::Thing (direct re-export from prelude)"
    );

    // `crate::origin::Thing` is also exported (public module, public item)
    let origin = one(&catalog, "crate::origin::Thing");
    assert!(
        origin.associated.iter().any(|i| i.name == "user_method"),
        "user_method must appear on crate::origin::Thing"
    );

    // `crate::prelude::Thing` is reachable through prelude's glob re-export
    let prelude = one(&catalog, "crate::prelude::Thing");
    assert!(
        prelude.associated.iter().any(|i| i.name == "user_method"),
        "user_method must appear on crate::prelude::Thing"
    );
}

// ---------------------------------------------------------------------------
// cfg-deferred source variants: both declarations retained per exported path.
// ---------------------------------------------------------------------------

#[test]
fn cfg_deferred_source_variants_produce_distinct_signature_records() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            (
                "src/lib.rs",
                "#[cfg(unix)]\n#[path = \"u.rs\"]\nmod sys;\n\
                 #[cfg(windows)]\n#[path = \"w.rs\"]\nmod sys;\n\
                 pub use sys::Handle;\n",
            ),
            ("src/u.rs", "pub struct Handle;\n"),
            ("src/w.rs", "pub struct Handle;\n"),
        ],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");
    let handles: Vec<_> = catalog
        .records
        .iter()
        .filter(|r| r.path == "crate::Handle")
        .collect();
    assert_eq!(
        handles.len(),
        2,
        "both cfg source variants must be retained: {handles:?}"
    );
    let source_paths: Vec<&str> = handles
        .iter()
        .map(|r| r.source.source_path.as_str())
        .collect();
    assert!(source_paths.contains(&"src/u.rs"));
    assert!(source_paths.contains(&"src/w.rs"));
    // Both share the exported path and local owning module.
    assert!(handles.iter().all(|r| r.source.module == "crate::sys"));
}

// ---------------------------------------------------------------------------
// Inline module: exported items point to host file, not a separate source.
// ---------------------------------------------------------------------------

#[test]
fn inline_module_item_has_host_file_source_path() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub mod inner {\n             pub fn greet() -> & 'static str { \"hi\" }\n             }\n",
        )],
    );

    let catalog = catalog_for(temp.path(), "src/lib.rs");
    let record = one(&catalog, "crate::inner::greet");
    assert_eq!(record.kind, SignatureKind::Function);
    // source.source_path points to the host file, not a separate module file.
    assert_eq!(
        record.source.source_path, "src/lib.rs",
        "inline module item should claim host file"
    );
    assert_eq!(record.source.module, "crate::inner");
    assert_eq!(record.source.ident, "greet");
    // Body must not leak into the signature.
    let toks = tokens(&record.signature);
    assert!(!toks.contains(&"hi"), "body leaked: {toks:?}");
}

// ---------------------------------------------------------------------------
// Determinism and typed errors.
// ---------------------------------------------------------------------------

#[test]
fn signature_catalog_is_deterministic() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            (
                "src/lib.rs",
                "pub mod defs;\npub use defs::Real as Alias;\n",
            ),
            (
                "src/defs.rs",
                "pub struct Real { pub x: u8 }\nimpl Real { pub fn hello(&self) {} }\n",
            ),
        ],
    );

    let first = catalog_for(temp.path(), "src/lib.rs");
    let second = catalog_for(temp.path(), "src/lib.rs");
    assert_eq!(first, second);
}

#[test]
fn missing_package_root_is_a_typed_error() {
    let temp = TempDir::new().unwrap();
    write_package(temp.path(), &[("src/lib.rs", "pub fn f() {}\n")]);
    let graph = module_graph(temp.path(), "src/lib.rs").unwrap();
    let exports = export_graph(&graph, temp.path()).unwrap();
    let bogus = temp.path().join("does-not-exist");
    let result = signature_catalog(&graph, &exports, &bogus);
    assert!(result.is_err());
}

#[test]
fn source_file_removed_before_signature_catalog_returns_catalog_corruption() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[("src/lib.rs", "pub fn removed() -> u8 { 0 }\n")],
    );

    let graph = module_graph(temp.path(), "src/lib.rs").unwrap();
    let exports = export_graph(&graph, temp.path()).unwrap();

    // Remove the source file after the graphs are built so
    // signature_catalog cannot re-read it.
    fs::remove_file(temp.path().join("src/lib.rs")).unwrap();

    let result = signature_catalog(&graph, &exports, temp.path());
    match result {
        Err(DiscoveryError::CatalogCorruption(msg)) => {
            assert!(
                msg.contains("cannot read module"),
                "error should be about reading: {msg}"
            );
        }
        other => panic!("expected CatalogCorruption, got {other:?}"),
    }
}
