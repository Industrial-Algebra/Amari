// SPDX-License-Identifier: MIT OR Apache-2.0

//! Normalized declaration signatures and associated items for catalog source
//! generation.
//!
//! [`signature_catalog`] composes the Task 5B1 [`ModuleGraph`] and the Task 5B2
//! [`ExportGraph`] to extract a normalized declaration signature for every
//! publicly reachable local item, projected under each reachable export alias.
//! It preserves both the exported path and the local source identity of the
//! declaration, and attaches associated items (inherent methods, trait
//! associated types/constants/functions) under every alias of their owning type
//! or trait.
//!
//! # Normalization
//!
//! Signatures are produced by re-emitting the relevant `syn` syntax-tree
//! nodes through `quote::ToTokens` and collapsing the resulting token stream
//! to a stable, whitespace-normalized form (runs of whitespace become a single
//! space). This is a deterministic, syntactic normalization: it preserves
//! generics, lifetimes, const generics, `where` clauses, qualifiers
//! (`const`/`async`/`unsafe`/`extern`), receiver forms, and return types
//! exactly as written, while dropping implementation bodies. Multi-character
//! operators (`->`, `::`) and delimiter/punctuation adjacency are preserved as
//! the token renderer emits them, so the form is stable across source
//! reformatting but is not a one-token-per-space canonicalization. No
//! `rustfmt`, `rustc`, or `cargo` execution is performed.
//!
//! # Public API shape
//!
//! For aggregate types the head signature (e.g. `pub struct Foo<T>`) is
//! complemented by a structured [`AggregateShape`] that records the public
//! fields or enum variants. Private and restricted (`pub(crate)` and friends)
//! struct fields are never recorded — only fully `pub` fields appear — so the
//! shape never leaks private structure. Enum variants are always public API
//! (variants inherit their enum's visibility) and are recorded in full.
//!
//! # Associated items
//!
//! Public inherent methods and public inherent associated constants are
//! extracted from `impl Type {}` blocks and attached to every exported alias of
//! their owner. Trait associated types/constants/functions are extracted from
//! the trait body and attached to every exported alias of the trait; they are
//! recorded with a [`AssociatedItem::has_default`] flag so Task 5B4 can later
//! classify required versus provided items.
//!
//! Associated items are projected under each reachable export alias:
//! `crate::Alias` backed by `private::Thing` yields the item
//! `crate::Alias::method`, while the owning local type identity
//! (`crate::private::Thing`) is retained on the record.
//!
//! `impl Trait for Type` blocks are parsed only to be excluded: recording
//! trait-implementation relationships is Task 5B4. cfg evaluation remains
//! deferred (as in Tasks 5B1/5B2): mutually exclusive declaration variants that
//! share a canonical module or exported path are retained as distinct records
//! keyed by their source file, so neither is silently dropped.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use quote::ToTokens;
use syn::{
    Fields, FieldsNamed, Item, ItemConst, ItemEnum, ItemFn, ItemImpl, ItemStatic, ItemStruct,
    ItemTrait, ItemType, ItemUnion, TraitItem, Type, TypePath, Visibility,
};

use crate::{DiscoveryError, DiscoveryResult};

use super::exports::{ExportGraph, ExportItemKind, ExportSource};
use super::modules::{ModuleGraph, ModuleKind};

/// Local source identity of the declaration backing a signature.
///
/// `module` is the canonical module path where the item is declared,
/// `source_path` is the package-relative file that physically hosts the
/// declaration (the file itself for file modules, or the host file for inline
/// modules), and `ident` is the item identifier exactly as written in source.
/// `source_path` distinguishes cfg-deferred declaration variants that share a
/// canonical module path but live in different files.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SignatureSource {
    /// Canonical module path of the declaration, e.g. `crate::algebra::ga`.
    pub module: String,
    /// Package-relative source file hosting the declaration, e.g. `src/lib.rs`.
    pub source_path: String,
    /// Item identifier exactly as written in source.
    pub ident: String,
}

/// Coarse declaration kind of an exported signature.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SignatureKind {
    /// `pub fn`.
    Function,
    /// `pub struct`.
    Struct,
    /// `pub enum`.
    Enum,
    /// `pub union`.
    Union,
    /// `pub const`.
    Constant,
    /// `pub static`.
    Static,
    /// `pub trait`.
    Trait,
    /// `pub type`.
    TypeAlias,
}

/// How an aggregate field is addressed in source.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FieldLabel {
    /// A named field: `pub name: T`.
    Named(String),
    /// A positional tuple field: `pub T` at index `n`.
    Positional(usize),
}

/// A single public field of a struct or union.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct FieldShape {
    /// How the field is addressed.
    pub label: FieldLabel,
    /// Normalized field type.
    pub ty: String,
}

/// Fields of an enum variant.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum VariantData {
    /// A unit variant: `Quit`.
    Unit,
    /// A tuple variant: `Write(String)`. Holds the normalized field types.
    Tuple(Vec<String>),
    /// A struct variant: `Move { x: u8 }`.
    Struct(Vec<VariantField>),
}

/// A named field of a struct variant.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct VariantField {
    /// Field name.
    pub name: String,
    /// Normalized field type.
    pub ty: String,
}

/// A single enum variant.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct VariantShape {
    /// Variant name.
    pub name: String,
    /// Variant field data.
    pub data: VariantData,
}

/// Public structural shape of an aggregate type.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AggregateShape {
    /// Struct shape with its public fields.
    Struct {
        /// Public fields in declaration order.
        fields: Vec<FieldShape>,
    },
    /// Enum shape with its variants.
    Enum {
        /// Variants in declaration order.
        variants: Vec<VariantShape>,
    },
    /// Union shape with its public fields.
    Union {
        /// Public fields in declaration order.
        fields: Vec<FieldShape>,
    },
}

/// Kind of an associated item attached to a type or trait.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AssociatedKind {
    /// A method (`fn`), inherent or trait.
    Method,
    /// An associated constant (`const`).
    Const,
    /// An associated type (`type`).
    Type,
}

/// An associated item projected under an exported type or trait.
///
/// For trait items, [`has_default`](Self::has_default) records whether the item
/// has a default definition (provided) versus being purely required, which
/// Task 5B4 uses to classify trait obligations. Inherent methods always report
/// `false`.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct AssociatedItem {
    /// Item name.
    pub name: String,
    /// Item kind.
    pub kind: AssociatedKind,
    /// Normalized declaration signature (no body).
    pub signature: String,
    /// Whether a trait item has a default definition. Always `false` for
    /// inherent items.
    pub has_default: bool,
}

/// Normalized declaration signature of one exported item under one exported
/// path.
///
/// Because cfg evaluation is deferred and because re-export aliases map
/// multiple exported paths to a single declaration, more than one record may
/// share the same declaration while differing in `path` (alias projection) or
/// in [`source`](Self::source) (cfg source variants).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignatureRecord {
    /// Exported path this signature is projected under, e.g. `crate::Alias` or
    /// `crate::Alias::method`'s owner. Associated items are reachable as
    /// `{path}::{item.name}`.
    pub path: String,
    /// Local source identity of the backing declaration.
    pub source: SignatureSource,
    /// Coarse declaration kind.
    pub kind: SignatureKind,
    /// Normalized declaration head (no body).
    pub signature: String,
    /// Public structural shape for aggregate types, when applicable.
    pub shape: Option<AggregateShape>,
    /// Associated items projected under `path`, for types and traits.
    pub associated: Vec<AssociatedItem>,
}

/// Complete normalized signature index for one target source tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignatureCatalog {
    /// Every signature record, sorted by `(path, source)` with duplicates
    /// removed.
    pub records: Vec<SignatureRecord>,
}

impl SignatureCatalog {
    /// Returns every record whose exported path equals `path`.
    pub fn at(&self, path: &str) -> Vec<&SignatureRecord> {
        self.records
            .iter()
            .filter(|record| record.path == path)
            .collect()
    }
}

/// Extracts normalized signatures and associated items for a target tree.
///
/// `graph` and `exports` must have been built for the same `package_root`.
/// The function re-reads each file-backed module source to collect
/// declarations and `impl` blocks; inline modules are indexed from their host
/// file. It performs no Cargo, rustc, or network access.
///
/// # Errors
///
/// Returns [`DiscoveryError::CatalogCorruption`] when the package root cannot
/// be resolved or a recorded source file cannot be read or parsed.
pub fn signature_catalog(
    graph: &ModuleGraph,
    exports: &ExportGraph,
    package_root: &Path,
) -> DiscoveryResult<SignatureCatalog> {
    let canonical_root = fs::canonicalize(package_root).map_err(|error| {
        DiscoveryError::CatalogCorruption(format!(
            "cannot resolve package root {}: {error}",
            package_root.display()
        ))
    })?;
    let index = build_index(graph, &canonical_root)?;

    // Owners of exported aggregate types (struct/enum/union) whose inherent
    // methods must be collected. Traits host their own associated items, so they
    // are excluded from the inherent-method owner set.
    let mut exported_type_owners: BTreeSet<(String, String)> = BTreeSet::new();
    for export in &exports.exports {
        if let ExportSource::Local {
            module,
            ident,
            kind,
        } = &export.source
        {
            if matches!(
                kind,
                ExportItemKind::Struct | ExportItemKind::Enum | ExportItemKind::Union
            ) {
                exported_type_owners.insert((module.clone(), ident.clone()));
            }
        }
    }

    let associated_by_owner = resolve_inherent_items(&index, &exported_type_owners);

    let mut records: Vec<SignatureRecord> = Vec::new();
    for export in &exports.exports {
        let ExportSource::Local {
            module,
            ident,
            kind,
        } = &export.source
        else {
            continue;
        };
        let Some(signature_kind) = map_kind(*kind) else {
            continue;
        };
        let declarations = index.declarations.get(&(module.clone(), ident.clone()));
        let Some(declarations) = declarations else {
            // The export resolved to a source that this syntactic pass could
            // not index (for example a declaration introduced by a macro). Skip
            // it rather than fabricating an empty signature.
            continue;
        };
        for (host_file, data) in declarations {
            let associated = match signature_kind {
                SignatureKind::Trait => data.associated.clone(),
                SignatureKind::Struct | SignatureKind::Enum | SignatureKind::Union => {
                    associated_by_owner
                        .get(&(module.clone(), ident.clone()))
                        .cloned()
                        .unwrap_or_default()
                }
                _ => Vec::new(),
            };
            records.push(SignatureRecord {
                path: export.path.clone(),
                source: SignatureSource {
                    module: module.clone(),
                    source_path: host_file.clone(),
                    ident: ident.clone(),
                },
                kind: signature_kind,
                signature: data.signature.clone(),
                shape: data.shape.clone(),
                associated,
            });
        }
    }

    // Deterministic order and deduplication. Records may legitimately share a
    // `path` (alias projection, cfg variants), so they are keyed by the full
    // `(path, source)` identity.
    let mut seen: BTreeSet<(String, SignatureSource, SignatureKind, String)> = BTreeSet::new();
    records.retain(|record| {
        let key = (
            record.path.clone(),
            record.source.clone(),
            record.kind,
            record.signature.clone(),
        );
        seen.insert(key)
    });
    records.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.source.cmp(&right.source))
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.signature.cmp(&right.signature))
    });

    Ok(SignatureCatalog { records })
}

// ---------------------------------------------------------------------------
// Source indexing: declarations, impl blocks, and type-namespace scopes.
// ---------------------------------------------------------------------------

/// A parsed declaration with its normalized head, optional shape, and (for
/// traits) associated items drawn from the trait body.
#[derive(Clone, Debug)]
struct DeclarationData {
    signature: String,
    shape: Option<AggregateShape>,
    associated: Vec<AssociatedItem>,
}

/// A raw inherent-impl item awaiting owner resolution.
#[derive(Clone, Debug)]
struct RawImplItem {
    is_public: bool,
    item: AssociatedItem,
}

/// An inherent `impl Type {}` block awaiting owner resolution.
#[derive(Clone, Debug)]
struct RawImpl {
    /// Canonical module where the impl block is declared.
    module: String,
    /// Path segments of the implemented self type (e.g. `["Foo"]` or
    /// `["crate", "bar", "Foo"]`).
    self_segments: Vec<String>,
    /// Public and private items; visibility filtering happens after owner
    /// resolution.
    items: Vec<RawImplItem>,
}

/// One use-import binding contributing to type-namespace name resolution.
#[derive(Clone, Debug)]
struct ImportEntry {
    local_name: Option<String>,
    path: Vec<String>,
    glob: bool,
}

/// Type-namespace scope for one canonical module.
#[derive(Clone, Debug, Default)]
struct TypeScope {
    /// Locally declared type names mapped to `(module, ident)` owner identity.
    locals: BTreeMap<String, (String, String)>,
    /// `use` bindings in this module.
    imports: Vec<ImportEntry>,
    /// Canonical path of the parent module, if any.
    parent: Option<String>,
    /// Child module names mapped to canonical child paths.
    children: BTreeMap<String, String>,
}

/// Indexed source: declarations, impl blocks, and scopes.
#[derive(Clone, Debug, Default)]
struct SourceIndex {
    /// Declarations keyed by `(canonical module, ident)`, each with the list of
    /// host files that define it (cfg variants).
    declarations: BTreeMap<(String, String), Vec<(String, DeclarationData)>>,
    /// Inherent impl blocks.
    impls: Vec<RawImpl>,
    /// Type-namespace scopes keyed by canonical module path.
    scopes: BTreeMap<String, TypeScope>,
}

fn build_index(graph: &ModuleGraph, package_root: &Path) -> DiscoveryResult<SourceIndex> {
    let mut index = SourceIndex::default();

    // Seed structural edges from the module graph so path navigation works even
    // for modules whose source has no parseable declarations.
    for record in &graph.modules {
        let scope = index.scopes.entry(record.path.clone()).or_default();
        if scope.parent.is_none() {
            scope.parent = record.parent.clone();
        }
    }
    for record in &graph.modules {
        for child_path in &record.children {
            let child_name = child_path
                .rsplit("::")
                .next()
                .unwrap_or(child_path)
                .to_owned();
            index
                .scopes
                .entry(record.path.clone())
                .or_default()
                .children
                .insert(child_name, child_path.clone());
        }
    }

    // Parse each file-backed module variant once and index declarations, impls,
    // and scopes for that module and its inline descendants.
    for record in &graph.modules {
        let Some(source_rel) = &record.source_path else {
            continue;
        };
        if record.kind == ModuleKind::Inline {
            continue;
        }
        let file = package_root.join(source_rel);
        let source = fs::read_to_string(&file).map_err(|error| {
            DiscoveryError::CatalogCorruption(format!(
                "cannot read module {}: {error}",
                file.display()
            ))
        })?;
        let ast = syn::parse_file(&source).map_err(|error| {
            DiscoveryError::CatalogCorruption(format!("cannot parse {}: {error}", file.display()))
        })?;
        index_file_items(&ast.items, &record.path, source_rel, &mut index);
    }

    Ok(index)
}

/// Indexes items of `canonical` (and recurses into its inline child modules),
/// attributing every declaration and impl to `host_file`.
fn index_file_items(items: &[Item], canonical: &str, host_file: &str, index: &mut SourceIndex) {
    for item in items {
        index_named_item(item, canonical, host_file, index);
    }
    for item in items {
        if let Item::Mod(item_mod) = item {
            if let Some((_, content)) = &item_mod.content {
                let child_canonical = format!("{canonical}::{}", item_mod.ident);
                index_file_items(content, &child_canonical, host_file, index);
            }
        }
    }
}

fn index_named_item(item: &Item, canonical: &str, host_file: &str, index: &mut SourceIndex) {
    let scope = index.scopes.entry(canonical.to_owned()).or_default();
    match item {
        Item::Struct(item_struct) => {
            register_local_type(scope, canonical, &item_struct.ident);
            let data = struct_data(item_struct);
            index
                .declarations
                .entry((canonical.to_owned(), item_struct.ident.to_string()))
                .or_default()
                .push((host_file.to_owned(), data));
        }
        Item::Enum(item_enum) => {
            register_local_type(scope, canonical, &item_enum.ident);
            let data = enum_data(item_enum);
            index
                .declarations
                .entry((canonical.to_owned(), item_enum.ident.to_string()))
                .or_default()
                .push((host_file.to_owned(), data));
        }
        Item::Union(item_union) => {
            register_local_type(scope, canonical, &item_union.ident);
            let data = union_data(item_union);
            index
                .declarations
                .entry((canonical.to_owned(), item_union.ident.to_string()))
                .or_default()
                .push((host_file.to_owned(), data));
        }
        Item::Trait(item_trait) => {
            register_local_type(scope, canonical, &item_trait.ident);
            let data = trait_data(item_trait);
            index
                .declarations
                .entry((canonical.to_owned(), item_trait.ident.to_string()))
                .or_default()
                .push((host_file.to_owned(), data));
        }
        Item::Type(item_type) => {
            register_local_type(scope, canonical, &item_type.ident);
            let data = type_alias_data(item_type);
            index
                .declarations
                .entry((canonical.to_owned(), item_type.ident.to_string()))
                .or_default()
                .push((host_file.to_owned(), data));
        }
        Item::Fn(item_fn) => {
            let data = function_data(item_fn);
            index
                .declarations
                .entry((canonical.to_owned(), item_fn.sig.ident.to_string()))
                .or_default()
                .push((host_file.to_owned(), data));
        }
        Item::Const(item_const) => {
            let data = const_data(item_const);
            index
                .declarations
                .entry((canonical.to_owned(), ident_string(&item_const.ident)))
                .or_default()
                .push((host_file.to_owned(), data));
        }
        Item::Static(item_static) => {
            let data = static_data(item_static);
            index
                .declarations
                .entry((canonical.to_owned(), item_static.ident.to_string()))
                .or_default()
                .push((host_file.to_owned(), data));
        }
        Item::Impl(item_impl) => {
            if let Some(raw) = inherent_impl_data(canonical, item_impl) {
                index.impls.push(raw);
            }
        }
        Item::Use(item_use) => {
            for entry in expand_use_tree(&item_use.tree) {
                scope.imports.push(entry);
            }
        }
        _ => {}
    }
}

fn register_local_type(scope: &mut TypeScope, module: &str, ident: &syn::Ident) {
    scope
        .locals
        .insert(ident.to_string(), (module.to_owned(), ident.to_string()));
}

fn ident_string(ident: &syn::Ident) -> String {
    ident.to_string()
}

// ---------------------------------------------------------------------------
// Declaration extraction by item kind.
// ---------------------------------------------------------------------------

fn function_data(item: &ItemFn) -> DeclarationData {
    // Re-emit only the visibility + signature so the implementation body never
    // leaks. The signature already orders the `where` clause after the return
    // type (see `syn::Signature` printing).
    let vis = &item.vis;
    let sig = &item.sig;
    let head = normalize(quote::quote!(#vis #sig));
    DeclarationData {
        signature: head,
        shape: None,
        associated: Vec::new(),
    }
}

fn const_data(item: &ItemConst) -> DeclarationData {
    let vis = &item.vis;
    let ident = &item.ident;
    let ty = &item.ty;
    let head = normalize(quote::quote!(#vis const #ident : #ty));
    DeclarationData {
        signature: head,
        shape: None,
        associated: Vec::new(),
    }
}

fn static_data(item: &ItemStatic) -> DeclarationData {
    let vis = &item.vis;
    let mutability = &item.mutability;
    let ident = &item.ident;
    let ty = &item.ty;
    let head = normalize(quote::quote!(#vis static #mutability #ident : #ty));
    DeclarationData {
        signature: head,
        shape: None,
        associated: Vec::new(),
    }
}

fn type_alias_data(item: &ItemType) -> DeclarationData {
    let vis = &item.vis;
    let ident = &item.ident;
    let generics = &item.generics;
    let ty = &item.ty;
    let head = normalize(quote::quote!(#vis type #ident #generics = #ty));
    DeclarationData {
        signature: head,
        shape: None,
        associated: Vec::new(),
    }
}

fn struct_data(item: &ItemStruct) -> DeclarationData {
    let vis = &item.vis;
    let ident = &item.ident;
    let generics = &item.generics;
    let where_clause = &item.generics.where_clause;
    let head = normalize(quote::quote!(#vis struct #ident #generics #where_clause));
    let fields = public_fields(&item.fields);
    DeclarationData {
        signature: head,
        shape: Some(AggregateShape::Struct { fields }),
        associated: Vec::new(),
    }
}

fn union_data(item: &ItemUnion) -> DeclarationData {
    let vis = &item.vis;
    let ident = &item.ident;
    let generics = &item.generics;
    let where_clause = &item.generics.where_clause;
    let head = normalize(quote::quote!(#vis union #ident #generics #where_clause));
    let fields = public_named_fields(&item.fields);
    DeclarationData {
        signature: head,
        shape: Some(AggregateShape::Union { fields }),
        associated: Vec::new(),
    }
}

fn enum_data(item: &ItemEnum) -> DeclarationData {
    let vis = &item.vis;
    let ident = &item.ident;
    let generics = &item.generics;
    let where_clause = &item.generics.where_clause;
    let head = normalize(quote::quote!(#vis enum #ident #generics #where_clause));
    let variants = item
        .variants
        .iter()
        .map(|variant| VariantShape {
            name: variant.ident.to_string(),
            data: variant_data(&variant.fields),
        })
        .collect();
    DeclarationData {
        signature: head,
        shape: Some(AggregateShape::Enum { variants }),
        associated: Vec::new(),
    }
}

fn trait_data(item: &ItemTrait) -> DeclarationData {
    let vis = &item.vis;
    let ident = &item.ident;
    let generics = &item.generics;
    let colon_token = &item.colon_token;
    let supertraits = &item.supertraits;
    let where_clause = &item.generics.where_clause;
    let head = normalize(quote::quote!(
        #vis trait #ident #generics #colon_token #supertraits #where_clause
    ));
    let associated = item
        .items
        .iter()
        .filter_map(trait_associated_item)
        .collect();
    DeclarationData {
        signature: head,
        shape: None,
        associated,
    }
}

fn variant_data(fields: &Fields) -> VariantData {
    match fields {
        Fields::Unit => VariantData::Unit,
        Fields::Unnamed(unnamed) => VariantData::Tuple(
            unnamed
                .unnamed
                .iter()
                .map(|field| normalize(field.ty.to_token_stream()))
                .collect(),
        ),
        Fields::Named(named) => VariantData::Struct(
            named
                .named
                .iter()
                .map(|field| VariantField {
                    name: field
                        .ident
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_default(),
                    ty: normalize(field.ty.to_token_stream()),
                })
                .collect(),
        ),
    }
}

/// Returns the public fields of a struct field list in declaration order.
fn public_fields(fields: &Fields) -> Vec<FieldShape> {
    match fields {
        Fields::Named(named) => public_named_fields(named),
        Fields::Unnamed(unnamed) => unnamed
            .unnamed
            .iter()
            .enumerate()
            .filter(|(_, field)| is_public(&field.vis))
            .map(|(index, field)| FieldShape {
                label: FieldLabel::Positional(index),
                ty: normalize(field.ty.to_token_stream()),
            })
            .collect(),
        Fields::Unit => Vec::new(),
    }
}

/// Returns the public named fields of a struct or union.
fn public_named_fields(named: &FieldsNamed) -> Vec<FieldShape> {
    named
        .named
        .iter()
        .filter(|field| is_public(&field.vis))
        .map(|field| FieldShape {
            label: FieldLabel::Named(
                field
                    .ident
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_default(),
            ),
            ty: normalize(field.ty.to_token_stream()),
        })
        .collect()
}

fn trait_associated_item(item: &TraitItem) -> Option<AssociatedItem> {
    match item {
        TraitItem::Const(item_const) => {
            let ident = &item_const.ident;
            let ty = &item_const.ty;
            let generics = &item_const.generics;
            let signature = normalize(quote::quote!(const #ident #generics : #ty));
            Some(AssociatedItem {
                name: ident_string(ident),
                kind: AssociatedKind::Const,
                signature,
                has_default: item_const.default.is_some(),
            })
        }
        TraitItem::Fn(item_fn) => {
            let sig = &item_fn.sig;
            let signature = normalize(quote::quote!(#sig));
            Some(AssociatedItem {
                name: item_fn.sig.ident.to_string(),
                kind: AssociatedKind::Method,
                signature,
                has_default: item_fn.default.is_some(),
            })
        }
        TraitItem::Type(item_type) => {
            let ident = &item_type.ident;
            let generics = &item_type.generics;
            let colon_token = &item_type.colon_token;
            let bounds = &item_type.bounds;
            let signature = normalize(quote::quote!(type #ident #generics #colon_token #bounds));
            Some(AssociatedItem {
                name: ident_string(ident),
                kind: AssociatedKind::Type,
                signature,
                has_default: item_type.default.is_some(),
            })
        }
        TraitItem::Macro(_) => None,
        TraitItem::Verbatim(_) => None,
        _ => None,
    }
}

fn inherent_impl_data(module: &str, item: &ItemImpl) -> Option<RawImpl> {
    // Trait impls (`impl Trait for Type`) record implementation relationships,
    // which is Task 5B4 territory; skip them here.
    if item.trait_.is_some() {
        return None;
    }
    let self_segments = type_path_segments(&item.self_ty)?;
    let mut items = Vec::new();
    for impl_item in &item.items {
        match impl_item {
            syn::ImplItem::Fn(item_fn) => {
                let is_public = is_public(&item_fn.vis);
                let vis = &item_fn.vis;
                let sig = &item_fn.sig;
                let signature = normalize(quote::quote!(#vis #sig));
                items.push(RawImplItem {
                    is_public,
                    item: AssociatedItem {
                        name: item_fn.sig.ident.to_string(),
                        kind: AssociatedKind::Method,
                        signature,
                        has_default: false,
                    },
                });
            }
            syn::ImplItem::Const(item_const) => {
                let is_public = is_public(&item_const.vis);
                let vis = &item_const.vis;
                let ident = &item_const.ident;
                let ty = &item_const.ty;
                let signature = normalize(quote::quote!(#vis const #ident : #ty));
                items.push(RawImplItem {
                    is_public,
                    item: AssociatedItem {
                        name: ident_string(ident),
                        kind: AssociatedKind::Const,
                        signature,
                        has_default: false,
                    },
                });
            }
            _ => {}
        }
    }
    Some(RawImpl {
        module: module.to_owned(),
        self_segments,
        items,
    })
}

/// Extracts the leading path segments of an inherent impl's self type.
///
/// Generic arguments on the final segment are stripped (e.g.
/// `Foo<T>` becomes `["Foo"]`); fully-qualified `<X as Y>::Z` self types and
/// non-path self types return `None` and are not associated.
fn type_path_segments(self_ty: &Type) -> Option<Vec<String>> {
    let Type::Path(TypePath { qself: None, path }) = self_ty else {
        return None;
    };
    Some(
        path.segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect(),
    )
}

// ---------------------------------------------------------------------------
// Type-namespace path resolution for impl self types.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq)]
enum Resolved {
    Item((String, String)),
    Module(String),
    NotFound,
    Unresolved,
}

fn resolve_inherent_items(
    index: &SourceIndex,
    exported_owners: &BTreeSet<(String, String)>,
) -> BTreeMap<(String, String), Vec<AssociatedItem>> {
    let mut by_owner: BTreeMap<(String, String), BTreeSet<AssociatedItem>> = BTreeMap::new();
    for raw in &index.impls {
        let mut visited = BTreeSet::new();
        let owner = match resolve_type_path(index, &raw.module, &raw.self_segments, &mut visited) {
            Some(Resolved::Item(owner)) => owner,
            _ => continue,
        };
        if !exported_owners.contains(&owner) {
            continue;
        }
        for raw_item in &raw.items {
            if !raw_item.is_public {
                continue;
            }
            by_owner
                .entry(owner.clone())
                .or_default()
                .insert(raw_item.item.clone());
        }
    }

    by_owner
        .into_iter()
        .map(|(owner, set)| {
            let mut items: Vec<AssociatedItem> = set.into_iter().collect();
            items.sort_by(|left, right| {
                left.name
                    .cmp(&right.name)
                    .then_with(|| left.kind.cmp(&right.kind))
            });
            (owner, items)
        })
        .collect()
}

fn resolve_type_path(
    index: &SourceIndex,
    current: &str,
    segments: &[String],
    visited: &mut BTreeSet<String>,
) -> Option<Resolved> {
    if segments.is_empty() {
        return Some(Resolved::Module(current.to_owned()));
    }
    let first = &segments[0];
    let rest = &segments[1..];
    match first.as_str() {
        "crate" | "$crate" => navigate(index, "crate", rest, visited),
        "self" => navigate(index, current, rest, visited),
        "super" => {
            let parent = index.scopes.get(current).and_then(|s| s.parent.clone());
            match parent {
                Some(parent) => navigate(index, &parent, rest, visited),
                None => Some(Resolved::Unresolved),
            }
        }
        _ => {
            let looked_up = lookup_name(index, current, first, visited);
            match looked_up {
                Resolved::NotFound => {
                    if rest.is_empty() {
                        None
                    } else {
                        Some(Resolved::NotFound)
                    }
                }
                Resolved::Unresolved => Some(Resolved::Unresolved),
                other => descend(index, other, rest, visited),
            }
        }
    }
}

fn navigate(
    index: &SourceIndex,
    module: &str,
    rest: &[String],
    visited: &mut BTreeSet<String>,
) -> Option<Resolved> {
    if rest.is_empty() {
        return Some(Resolved::Module(module.to_owned()));
    }
    let looked_up = lookup_name(index, module, &rest[0], visited);
    descend(index, looked_up, &rest[1..], visited)
}

fn descend(
    index: &SourceIndex,
    target: Resolved,
    rest: &[String],
    visited: &mut BTreeSet<String>,
) -> Option<Resolved> {
    match target {
        Resolved::Item((module, ident)) => {
            if rest.is_empty() {
                Some(Resolved::Item((module, ident)))
            } else {
                // Types do not expose addressable sub-paths in the type
                // namespace (generic arguments were already stripped).
                Some(Resolved::Unresolved)
            }
        }
        Resolved::Module(module) => navigate(index, &module, rest, visited),
        Resolved::NotFound | Resolved::Unresolved => Some(target),
    }
}

fn lookup_name(
    index: &SourceIndex,
    module: &str,
    name: &str,
    visited: &mut BTreeSet<String>,
) -> Resolved {
    let Some(scope) = index.scopes.get(module) else {
        return Resolved::Unresolved;
    };
    if let Some(owner) = scope.locals.get(name) {
        return Resolved::Item(owner.clone());
    }
    if let Some(child) = scope.children.get(name) {
        return Resolved::Module(child.clone());
    }

    let mut cycle_key = String::from(module);
    cycle_key.push('\0');
    cycle_key.push_str(name);
    if !visited.insert(cycle_key.clone()) {
        return Resolved::Unresolved;
    }

    // Named imports may rebind `name` to a local type path.
    let mut distinct: Vec<Resolved> = Vec::new();
    for entry in &scope.imports {
        if entry.glob {
            continue;
        }
        if entry.local_name.as_deref() == Some(name) {
            match resolve_type_path(index, module, &entry.path, visited) {
                Some(Resolved::Item(owner)) => {
                    if !distinct.contains(&Resolved::Item(owner.clone())) {
                        distinct.push(Resolved::Item(owner));
                    }
                }
                Some(other @ (Resolved::Module(_) | Resolved::NotFound | Resolved::Unresolved)) => {
                    let _ = other;
                }
                None => {}
            }
        }
    }
    if distinct.len() == 1 {
        visited.remove(&cycle_key);
        return distinct.into_iter().next().unwrap_or(Resolved::Unresolved);
    }
    if distinct.len() > 1 {
        visited.remove(&cycle_key);
        return Resolved::Unresolved;
    }

    // Glob imports may provide `name` from a target module's type namespace.
    // We first check the target's own locals (fast path) and then fall back
    // to recursively resolving through the target's own imports/globs,
    // handling transitive chains. The cycle guard above prevents infinite
    // recursion through mutually glob-importing modules.
    let mut glob_owner: Option<(String, String)> = None;
    for entry in &scope.imports {
        if !entry.glob {
            continue;
        }
        if let Some(Resolved::Module(target)) =
            resolve_type_path(index, module, &entry.path, visited)
        {
            let found = index
                .scopes
                .get(&target)
                .and_then(|target_scope| target_scope.locals.get(name));
            let owner = match found.cloned() {
                Some(owner) => Some(owner),
                None => {
                    // Not a direct local declaration in the target module;
                    // try recursive resolution through the target's own
                    // imports/globs (transitive chain).
                    match lookup_name(index, &target, name, visited) {
                        Resolved::Item(owner) => Some(owner),
                        Resolved::Unresolved => {
                            visited.remove(&cycle_key);
                            return Resolved::Unresolved;
                        }
                        _ => None,
                    }
                }
            };
            if let Some(owner) = owner {
                if glob_owner.is_some() && glob_owner.as_ref() != Some(&owner) {
                    visited.remove(&cycle_key);
                    return Resolved::Unresolved;
                }
                glob_owner = Some(owner);
            }
        }
    }
    visited.remove(&cycle_key);
    if let Some(owner) = glob_owner {
        return Resolved::Item(owner);
    }
    Resolved::NotFound
}

// ---------------------------------------------------------------------------
// Helpers: use-tree expansion, kind mapping, normalization, visibility.
// ---------------------------------------------------------------------------

fn expand_use_tree(tree: &syn::UseTree) -> Vec<ImportEntry> {
    let mut entries = Vec::new();
    flatten_use_tree(tree, Vec::new(), &mut entries);
    entries
}

fn flatten_use_tree(tree: &syn::UseTree, prefix: Vec<String>, entries: &mut Vec<ImportEntry>) {
    use syn::UseTree;
    match tree {
        UseTree::Path(path) => {
            let mut segments = prefix;
            segments.push(path.ident.to_string());
            flatten_use_tree(&path.tree, segments, entries);
        }
        UseTree::Name(name) => {
            let mut segments = prefix;
            let local = name.ident.to_string();
            segments.push(local.clone());
            entries.push(ImportEntry {
                local_name: Some(local),
                path: segments,
                glob: false,
            });
        }
        UseTree::Rename(rename) => {
            let mut segments = prefix;
            segments.push(rename.ident.to_string());
            entries.push(ImportEntry {
                local_name: Some(rename.rename.to_string()),
                path: segments,
                glob: false,
            });
        }
        UseTree::Glob(_) => {
            entries.push(ImportEntry {
                local_name: None,
                path: prefix,
                glob: true,
            });
        }
        UseTree::Group(group) => {
            for inner in &group.items {
                flatten_use_tree(inner, prefix.clone(), entries);
            }
        }
    }
}

fn map_kind(kind: ExportItemKind) -> Option<SignatureKind> {
    match kind {
        ExportItemKind::Function => Some(SignatureKind::Function),
        ExportItemKind::Struct => Some(SignatureKind::Struct),
        ExportItemKind::Enum => Some(SignatureKind::Enum),
        ExportItemKind::Union => Some(SignatureKind::Union),
        ExportItemKind::Constant => Some(SignatureKind::Constant),
        ExportItemKind::Static => Some(SignatureKind::Static),
        ExportItemKind::Trait => Some(SignatureKind::Trait),
        ExportItemKind::TypeAlias => Some(SignatureKind::TypeAlias),
        ExportItemKind::Module => None,
    }
}

/// Normalizes a token stream into a deterministic single-space-separated form.
fn normalize(stream: impl ToTokens) -> String {
    stream
        .to_token_stream()
        .to_string()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_public(visibility: &Visibility) -> bool {
    matches!(visibility, Visibility::Public(_))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_collapses_whitespace_deterministically() {
        let ty: Type = syn::parse_quote!(Option<T>);
        assert_eq!(normalize(ty.to_token_stream()), "Option < T >");
    }

    #[test]
    fn public_fields_filter_positional_and_named() {
        let named_struct: syn::ItemStruct = syn::parse_quote!(
            struct S {
                pub a: u8,
                _b: u8,
            }
        );
        let named = public_fields(&named_struct.fields);
        assert_eq!(named.len(), 1);
        assert!(matches!(named[0].label, FieldLabel::Named(ref n) if n == "a"));

        let tuple_struct: syn::ItemStruct = syn::parse_quote!(
            struct T(pub u8, u8);
        );
        let positional = public_fields(&tuple_struct.fields);
        assert_eq!(positional.len(), 1);
        assert!(matches!(positional[0].label, FieldLabel::Positional(0)));
    }
}
