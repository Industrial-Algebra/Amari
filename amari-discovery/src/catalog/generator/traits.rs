// SPDX-License-Identifier: MIT OR Apache-2.0

//! Typed trait and implementation relationship extraction for catalog source
//! generation.
//!
//! [`trait_relationships`] composes the Task 5B1 [`ModuleGraph`], the Task 5B2
//! [`ExportGraph`], and the Task 5B3 declaration indexing to extract a typed
//! relationship catalog of trait definitions (supertraits and required/provided
//! items) and explicit trait implementations (`impl Trait for Type`).
//!
//! # Model
//!
//! Each trait relationship has one of two endpoint forms:
//!
//! - [`RelationshipEndpoint::Local`]: a trait or type whose source identity
//!   (canonical module + identifier) is resolvable within the workspace. Two
//!   local endpoints are equal when they point to the same source declaration.
//! - [`RelationshipEndpoint::External`]: a trait that cannot be traced to local
//!   source — for example `Default`, `Clone`, `Sized`, `Send`, or `Sync` from
//!   `core`/`std`. These are recorded as a best-effort path string.
//!
//! # Endpoint resolution
//!
//! Trait and self-type paths are resolved through the same deterministic,
//! cycle-guarded mechanism used by Task 5B3: `crate`/`self`/`super` prefixes,
//! named imports, and glob imports. A path whose first segment is absent from
//! the local namespace is treated as an external (unresolvable) endpoint rather
//! than a corruption error.
//!
//! # Generics and qualifiers
//!
//! Normalized generic parameter lists and `where` clauses are preserved on
//! [`TraitImplementation`] so that `impl<T: Float> Default for ParetoFront<T>`
//! and `impl<const D: usize> BindingAlgebra for FHRRAlgebra<D>` carry their
//! bounds. The [`unsafe_trait`](TraitImplementation::unsafe_trait) and
//! [`negative`](TraitImplementation::negative) flags distinguish
//! `unsafe impl Trait for Type` and `impl !Trait for Type` respectively.
//!
//! # Derive handling
//!
//! Derive macros are not expanded. A `#[derive(Default, Clone)]` attribute on a
//! type or struct is detected syntactically and recorded as a set of
//! derive-introduced relationships on [`TraitImplementation`] with the
//! [`is_derived`](TraitImplementation::is_derived) flag set. The actual
//! generated impl bodies are never extracted.
//!
//! # cfg deferral
//!
//! `#[cfg]` evaluation remains deferred (as in Tasks 5B1/5B2/5B3). Mutually
//! exclusive cfg variants that declare the same trait or implementation in
//! different source files are retained as distinct records.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use quote::ToTokens;
use syn::{Attribute, Item, ItemImpl, ItemTrait, PathArguments, TraitItem, Type, TypePath};

use crate::{DiscoveryError, DiscoveryResult};

use super::exports::ExportGraph;
use super::modules::{ModuleGraph, ModuleKind};
use super::signatures::AssociatedKind;

// ============================================================================
// Public types
// ============================================================================

/// An endpoint in a trait relationship — either a local item traceable to
/// source, or an external/unresolved reference.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RelationshipEndpoint {
    /// A local item whose source identity is known.
    Local {
        /// Canonical module path, e.g. `crate::algebra::ga`.
        module: String,
        /// Item identifier exactly as written in source.
        ident: String,
    },
    /// An external or unresolved reference (e.g. `Default`, `Clone`, `Send`).
    External {
        /// The unresolved path as written in source, e.g. `Default`,
        /// `Clone`, `std::default::Default`.
        path: String,
    },
}

/// A supertrait constraint on a trait definition.
///
/// For `pub trait A: B + C`, each of `B` and `C` becomes a
/// [`SuperTraitConstraint`].
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SuperTraitConstraint {
    /// The supertrait endpoint (the bound after `:`).
    pub endpoint: RelationshipEndpoint,
}

/// The required/provided status of a trait associated item.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TraitItemStatus {
    /// The item has no default — implementors must provide it.
    Required,
    /// The item has a default definition — implementors may override it.
    Provided,
}

/// A single associated item of a trait definition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitAssociatedItem {
    /// Item name.
    pub name: String,
    /// Item kind (Method, Const, or Type).
    pub kind: AssociatedKind,
    /// Normalized declaration signature (no body).
    pub signature: String,
    /// Whether this item is required or has a default.
    pub status: TraitItemStatus,
}

/// A trait definition with its supertraits, required/provided items, and
/// the public export path plus source-identity fields needed to distinguish
/// alias projections and alternate cfg-deferred declaration variants.
///
/// A local trait exported under multiple aliases produces one
/// [`TraitDefinition`] per export path so that alias-projection mappings are
/// observable. Two cfg-deferred variants of the same trait declared in
/// different source files produce distinct [`TraitDefinition`] entries with
/// different [`source_path`](TraitDefinition::source_path) values. When two
/// variants occupy the same file (different `#[cfg]` gates), the
/// [`source_ordinal`](TraitDefinition::source_ordinal) field distinguishes
/// them by deterministic AST preorder position.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitDefinition {
    /// The public exported path under which this definition is reachable,
    /// e.g. `crate::algebra::BindingAlgebra`.
    pub export_path: String,
    /// Package-root-relative source path where the trait is declared,
    /// using `/` separators.
    pub source_path: String,
    /// Deterministic AST preorder ordinal for distinguishing same-file
    /// cfg-gated declaration variants. Task 5C1 uses this to correlate
    /// gates after the fact.
    pub source_ordinal: usize,
    /// The trait endpoint (local source or external).
    pub trait_endpoint: RelationshipEndpoint,
    /// Super trait constraints.
    pub supertraits: Vec<SuperTraitConstraint>,
    /// Items without default definitions (required).
    pub required_items: Vec<TraitAssociatedItem>,
    /// Items with default definitions (provided).
    pub provided_items: Vec<TraitAssociatedItem>,
}

/// An explicit or derived trait implementation for a type.
///
/// Records one leg of `impl Trait for Type`, preserving normalized generics,
/// unsafe/negative markers, the derivation flag, the explicit projectable
/// trait and type paths, the source path for cfg-deferred identity, and a
/// deterministic AST ordinal for same-file cfg-gated variants.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitImplementation {
    /// Normalized trait path for this relationship.
    ///
    /// For external traits this is the best-effort path string (e.g.
    /// `"Default"`). For local traits this is the export alias path under
    /// which the trait is reachable (e.g. `"crate::MyTrait"`).
    pub trait_path: String,
    /// Normalized type path for this relationship.
    ///
    /// For external types this is the best-effort path string. For local
    /// types this is the export alias path under which the type is reachable.
    pub impl_type_path: String,
    /// Package-root-relative source path where the impl block (or the derive
    /// attribute's host declaration) originates, using `/` separators.
    pub source_path: String,
    /// Deterministic AST preorder ordinal for distinguishing same-file
    /// cfg-gated declaration variants of the same trait/type pair.
    pub source_ordinal: usize,
    /// The trait being implemented.
    pub trait_endpoint: RelationshipEndpoint,
    /// The type the trait is implemented for.
    pub impl_type_endpoint: RelationshipEndpoint,
    /// Normalized generic parameters clause (e.g. `<T: Float>` or
    /// `<const D: usize>`).
    pub generics: String,
    /// Whether the impl block is marked `unsafe`.
    pub unsafe_trait: bool,
    /// Whether this is a negative impl (`impl !Trait for Type`).
    pub negative: bool,
    /// Whether this implementation was introduced by a `#[derive(...)]`
    /// attribute rather than an explicit `impl` block.
    ///
    /// Derive attributes are detected syntactically without macro expansion;
    /// the actual generated impl bodies are never extracted.
    pub is_derived: bool,
}

/// The complete trait relationship index for one target source tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitCatalog {
    /// Trait definitions, sorted by endpoint.
    pub definitions: Vec<TraitDefinition>,
    /// Trait implementations, sorted by (trait, type).
    pub implementations: Vec<TraitImplementation>,
}

// ============================================================================
// Public entry point
// ============================================================================

/// Extracts trait definitions and implementations for a target source tree.
///
/// `graph` and `exports` must have been built for the same `package_root`.
/// The function re-reads each file-backed module source to collect trait
/// declarations and `impl Trait for Type` blocks; inline modules are indexed
/// from their host file. It performs no Cargo, rustc, or network access.
///
/// # Errors
///
/// Returns [`DiscoveryError::CatalogCorruption`] when the package root cannot
/// be resolved or a recorded source file cannot be read or parsed.
pub fn trait_relationships(
    graph: &ModuleGraph,
    exports: &ExportGraph,
    package_root: &Path,
) -> DiscoveryResult<TraitCatalog> {
    let canonical_root = fs::canonicalize(package_root).map_err(|error| {
        DiscoveryError::CatalogCorruption(format!(
            "cannot resolve package root {}: {error}",
            package_root.display()
        ))
    })?;
    let index = build_trait_index(graph, &canonical_root)?;

    // Build export-path lookup from export graph.
    // Maps (module, ident) -> set of export paths.
    let mut trait_exports: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
    let mut type_exports: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();

    for export in &exports.exports {
        let super::exports::ExportSource::Local {
            module,
            ident,
            kind,
        } = &export.source
        else {
            continue;
        };
        let map = match kind {
            super::exports::ExportItemKind::Trait => &mut trait_exports,
            super::exports::ExportItemKind::Struct
            | super::exports::ExportItemKind::Enum
            | super::exports::ExportItemKind::Union
            | super::exports::ExportItemKind::TypeAlias => &mut type_exports,
            _ => continue,
        };
        map.entry((module.clone(), ident.clone()))
            .or_default()
            .insert(export.path.clone());
    }

    // -------------------------------------------------------------------
    // 1. Trait definitions — one per (export_path × declaration_variant)
    // -------------------------------------------------------------------
    let mut definitions: Vec<TraitDefinition> = Vec::new();
    let mut seen_defs: BTreeSet<(String, String, usize, String)> = BTreeSet::new();

    for export in &exports.exports {
        let super::exports::ExportSource::Local {
            module,
            ident,
            kind,
        } = &export.source
        else {
            continue;
        };
        if !matches!(kind, super::exports::ExportItemKind::Trait) {
            continue;
        }
        let Some(variants) = index.traits.get(&(module.clone(), ident.clone())) else {
            continue;
        };
        let local_ep = RelationshipEndpoint::Local {
            module: module.clone(),
            ident: ident.clone(),
        };

        for variant in variants {
            let def = TraitDefinition {
                export_path: export.path.clone(),
                source_path: variant.source_path.clone(),
                source_ordinal: variant.source_ordinal,
                trait_endpoint: local_ep.clone(),
                supertraits: variant.data.supertraits.clone(),
                required_items: variant.data.required_items.clone(),
                provided_items: variant.data.provided_items.clone(),
            };
            // Dedup key includes export_path AND source_path AND
            // source_ordinal so that distinct cfg variants in the same
            // file are each retained.
            let item_hash: String = def
                .required_items
                .iter()
                .map(|i| i.name.clone())
                .collect::<Vec<_>>()
                .join(",");
            let dedup_key = (
                def.export_path.clone(),
                def.source_path.clone(),
                def.source_ordinal,
                item_hash,
            );
            if seen_defs.insert(dedup_key) {
                definitions.push(def);
            }
        }
    }

    // -------------------------------------------------------------------
    // 2. Explicit trait implementations — projected by alias and variant
    // -------------------------------------------------------------------
    let mut implementations: Vec<TraitImplementation> = Vec::new();
    let mut seen_impls: BTreeSet<(String, String, String, usize)> = BTreeSet::new();

    for raw_impl in &index.impl_blocks {
        let mut visited = BTreeSet::new();
        let trait_ep = resolve_trait_path(
            &index,
            &raw_impl.module,
            &raw_impl.trait_segments,
            &mut visited,
        );
        let type_ep = match &raw_impl.self_type {
            RawSelfType::Path { segments } | RawSelfType::Reference { segments, .. } => {
                resolve_type_path(&index, &raw_impl.module, segments, &mut visited)
            }
            RawSelfType::External { display } => RelationshipEndpoint::External {
                path: display.clone(),
            },
        };

        // Resolve trait export paths.
        let trait_paths: Vec<String> = match &trait_ep {
            RelationshipEndpoint::Local { module, ident } => trait_exports
                .get(&(module.clone(), ident.clone()))
                .map(|paths| paths.iter().cloned().collect())
                .unwrap_or_default(),
            RelationshipEndpoint::External { path } => vec![path.clone()],
        };
        // Do NOT fabricate canonical module::ident paths for local
        // endpoints without reachable exports. If there are no export
        // paths, skip this relationship entirely.
        if trait_paths.is_empty() {
            continue;
        }

        // Resolve type export paths.
        let type_paths: Vec<String> = match &type_ep {
            RelationshipEndpoint::Local { module, ident } => type_exports
                .get(&(module.clone(), ident.clone()))
                .map(|paths| {
                    paths
                        .iter()
                        .map(|path| project_self_type_path(&raw_impl.self_type, path))
                        .collect()
                })
                .unwrap_or_default(),
            RelationshipEndpoint::External { path } => {
                vec![project_self_type_path(&raw_impl.self_type, path)]
            }
        };
        // Same rule: no fabricated paths for non-exported local types.
        if type_paths.is_empty() {
            continue;
        }

        for tp in &trait_paths {
            for itp in &type_paths {
                let imp = TraitImplementation {
                    trait_path: tp.clone(),
                    impl_type_path: itp.clone(),
                    source_path: raw_impl.source_path.clone(),
                    source_ordinal: raw_impl.source_ordinal,
                    trait_endpoint: trait_ep.clone(),
                    impl_type_endpoint: type_ep.clone(),
                    generics: raw_impl.generics.clone(),
                    unsafe_trait: raw_impl.unsafe_trait,
                    negative: raw_impl.negative,
                    is_derived: false,
                };
                let dedup_key = (
                    imp.trait_path.clone(),
                    imp.impl_type_path.clone(),
                    imp.source_path.clone(),
                    imp.source_ordinal,
                );
                if seen_impls.insert(dedup_key) {
                    implementations.push(imp);
                }
            }
        }
    }

    // -------------------------------------------------------------------
    // 3. Derive-introduced implementations
    // -------------------------------------------------------------------
    for export in &exports.exports {
        let super::exports::ExportSource::Local {
            module,
            ident,
            kind,
        } = &export.source
        else {
            continue;
        };
        if !matches!(
            kind,
            super::exports::ExportItemKind::Struct
                | super::exports::ExportItemKind::Enum
                | super::exports::ExportItemKind::Union
        ) {
            continue;
        }
        let Some(variants) = index.declarations.get(&(module.clone(), ident.clone())) else {
            continue;
        };
        let type_ep = RelationshipEndpoint::Local {
            module: module.clone(),
            ident: ident.clone(),
        };
        // The export path(s) for this type.
        let type_paths: Vec<String> = type_exports
            .get(&(module.clone(), ident.clone()))
            .map(|paths| paths.iter().cloned().collect())
            .unwrap_or_default();
        // Do not fabricate paths for non-exported local types.
        if type_paths.is_empty() {
            continue;
        }

        for variant in variants {
            for derive_trait in &variant.derive_traits {
                for itp in &type_paths {
                    let imp = TraitImplementation {
                        trait_path: derive_trait.clone(),
                        impl_type_path: itp.clone(),
                        source_path: variant.source_path.clone(),
                        source_ordinal: variant.source_ordinal,
                        trait_endpoint: RelationshipEndpoint::External {
                            path: derive_trait.clone(),
                        },
                        impl_type_endpoint: type_ep.clone(),
                        generics: variant.generics.clone(),
                        unsafe_trait: false,
                        negative: false,
                        is_derived: true,
                    };
                    let dedup_key = (
                        imp.trait_path.clone(),
                        imp.impl_type_path.clone(),
                        imp.source_path.clone(),
                        imp.source_ordinal,
                    );
                    if seen_impls.insert(dedup_key) {
                        implementations.push(imp);
                    }
                }
            }
        }
    }

    // Sort for determinism.
    definitions.sort_by(|a, b| {
        a.export_path
            .cmp(&b.export_path)
            .then_with(|| a.source_path.cmp(&b.source_path))
            .then_with(|| a.source_ordinal.cmp(&b.source_ordinal))
    });
    implementations.sort_by(|a, b| {
        a.trait_path
            .cmp(&b.trait_path)
            .then_with(|| a.impl_type_path.cmp(&b.impl_type_path))
            .then_with(|| a.source_path.cmp(&b.source_path))
            .then_with(|| a.source_ordinal.cmp(&b.source_ordinal))
    });

    Ok(TraitCatalog {
        definitions,
        implementations,
    })
}

// ============================================================================
// Internal indexing: traits, impl blocks, derive attributes
// ============================================================================

/// Parsed trait data for one local trait.
#[derive(Clone, Debug, Default)]
struct TraitData {
    supertraits: Vec<SuperTraitConstraint>,
    required_items: Vec<TraitAssociatedItem>,
    provided_items: Vec<TraitAssociatedItem>,
}

/// A variant of a trait declaration, keyed by its source identity.
#[derive(Clone, Debug)]
struct TraitDataVariant {
    /// Package-root-relative source path.
    source_path: String,
    /// Deterministic AST preorder ordinal for same-file cfg-gated variants.
    source_ordinal: usize,
    /// The trait data.
    data: TraitData,
}

/// A parsed `impl Trait for Type` block awaiting endpoint resolution.
#[derive(Clone, Debug)]
struct RawImplBlock {
    /// Canonical module path where the impl block is declared.
    module: String,
    /// Path segments of the trait being implemented (e.g. `["Default"]` or
    /// `["crate", "MyTrait"]`).
    trait_segments: Vec<String>,
    /// Parsed self-type shape, retaining reference/composite syntax while
    /// allowing nominal local types to resolve through public aliases.
    self_type: RawSelfType,
    /// Normalized generics clause.
    generics: String,
    /// Whether the impl block is `unsafe`.
    unsafe_trait: bool,
    /// Whether this is a negative impl (`impl !Trait`).
    negative: bool,
    /// Package-root-relative source path of the impl block.
    source_path: String,
    /// Deterministic AST preorder ordinal for same-file cfg-gated variants.
    source_ordinal: usize,
}

/// Self-type syntax needed to project a trait implementation publicly.
#[derive(Clone, Debug)]
enum RawSelfType {
    /// A nominal path such as `MyStruct<T>`.
    Path { segments: Vec<String> },
    /// A reference to a nominal path. The endpoint remains the referenced
    /// local type, while the projected display preserves `&`, lifetime, and
    /// mutability.
    Reference {
        segments: Vec<String>,
        lifetime: Option<String>,
        mutable: bool,
    },
    /// A composite or otherwise non-projectable type such as `(A, B)`.
    External { display: String },
}

/// One variant of a type/enum/union declaration, including its derives
/// and the normalized generics from the declaration.
#[derive(Clone, Debug)]
struct TypeDeclVariant {
    /// Package-root-relative source path.
    source_path: String,
    /// Deterministic AST preorder ordinal for same-file cfg-gated variants.
    source_ordinal: usize,
    /// Trait paths from `#[derive(...)]` attributes.
    derive_traits: Vec<String>,
    /// Normalized generics clause from the type declaration itself
    /// (used for derive-introduced implementations).
    generics: String,
}

/// The complete trait-relevant index for a package.
#[derive(Clone, Debug, Default)]
struct TraitIndex {
    /// Trait data variants keyed by `(canonical module, ident)`.
    /// Each key may have multiple variants (cfg-deferred).
    traits: BTreeMap<(String, String), Vec<TraitDataVariant>>,
    /// Raw impl blocks.
    impl_blocks: Vec<RawImplBlock>,
    /// Type declaration variants keyed by `(canonical module, ident)`.
    /// Each key may have multiple variants (cfg-deferred).
    declarations: BTreeMap<(String, String), Vec<TypeDeclVariant>>,
    /// Type-namespace scopes for path resolution.
    scopes: BTreeMap<String, TypeScope>,
    /// Module-canonical-path to source-path mapping.
    source_map: BTreeMap<String, String>,
}

/// Type-namespace scope for one canonical module.
#[derive(Clone, Debug, Default)]
struct TypeScope {
    /// Locally declared type/trait names mapped to `(module, ident)`.
    locals: BTreeMap<String, (String, String)>,
    /// `use` bindings in this module.
    imports: Vec<ImportEntry>,
    /// Parent module canonical path.
    parent: Option<String>,
    /// Child modules by name.
    children: BTreeMap<String, String>,
}

/// One use-import binding.
#[derive(Clone, Debug)]
struct ImportEntry {
    local_name: Option<String>,
    path: Vec<String>,
    glob: bool,
}

fn build_trait_index(graph: &ModuleGraph, package_root: &Path) -> DiscoveryResult<TraitIndex> {
    let mut index = TraitIndex::default();

    // Seed structural scopes from the module graph.
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

    // Build source_path map from module graph, inheriting parent source
    // for inline modules.
    for record in &graph.modules {
        if let Some(sp) = &record.source_path {
            index
                .source_map
                .entry(record.path.clone())
                .or_insert_with(|| sp.clone());
        }
    }
    // Fill inline modules by walking to the nearest ancestor with a source.
    for record in &graph.modules {
        if index.source_map.contains_key(&record.path) {
            continue;
        }
        // Walk up the parent chain from the graph directly.
        let mut ancestor: Option<&str> = record.parent.as_deref();
        while let Some(p) = ancestor {
            if let Some(src) = index.source_map.get(p).cloned() {
                index.source_map.insert(record.path.clone(), src);
                break;
            }
            // Walk further up.
            let scope = index.scopes.get(p);
            ancestor = scope.and_then(|s| s.parent.as_deref());
        }
    }

    // Parse each file-backed module.
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
        let mut ordinal = 0usize;
        index_file_items(
            &ast.items,
            &record.path,
            source_rel,
            &mut index,
            &mut ordinal,
        );
    }

    Ok(index)
}

fn register_type_name(scope: &mut TypeScope, module: &str, ident: &syn::Ident) {
    scope
        .locals
        .insert(ident.to_string(), (module.to_owned(), ident.to_string()));
}

// ---------------------------------------------------------------------------
// Trait data extraction
// ---------------------------------------------------------------------------

/// Extracts supertraits and required/provided items from a trait declaration.
fn extract_trait_data(item: &ItemTrait, canonical: &str, index: &TraitIndex) -> TraitData {
    let supertraits: Vec<SuperTraitConstraint> = item
        .supertraits
        .iter()
        .map(|bound| {
            let endpoint = resolve_type_bound(bound, canonical, index);
            SuperTraitConstraint { endpoint }
        })
        .collect();

    let mut required_items = Vec::new();
    let mut provided_items = Vec::new();

    for trait_item in &item.items {
        match trait_item {
            TraitItem::Const(tc) => {
                let ident = &tc.ident;
                let ty = &tc.ty;
                let generics = &tc.generics;
                let signature = normalize(quote::quote!(const #ident #generics : #ty));
                let item = TraitAssociatedItem {
                    name: ident.to_string(),
                    kind: AssociatedKind::Const,
                    signature,
                    status: if tc.default.is_some() {
                        TraitItemStatus::Provided
                    } else {
                        TraitItemStatus::Required
                    },
                };
                if tc.default.is_some() {
                    provided_items.push(item);
                } else {
                    required_items.push(item);
                }
            }
            TraitItem::Fn(tf) => {
                let sig = &tf.sig;
                let signature = normalize(quote::quote!(#sig));
                let item = TraitAssociatedItem {
                    name: tf.sig.ident.to_string(),
                    kind: AssociatedKind::Method,
                    signature,
                    status: if tf.default.is_some() {
                        TraitItemStatus::Provided
                    } else {
                        TraitItemStatus::Required
                    },
                };
                if tf.default.is_some() {
                    provided_items.push(item);
                } else {
                    required_items.push(item);
                }
            }
            TraitItem::Type(tt) => {
                let ident = &tt.ident;
                let generics = &tt.generics;
                let colon_token = &tt.colon_token;
                let bounds = &tt.bounds;
                let signature =
                    normalize(quote::quote!(type #ident #generics #colon_token #bounds));
                let item = TraitAssociatedItem {
                    name: ident.to_string(),
                    kind: AssociatedKind::Type,
                    signature,
                    status: if tt.default.is_some() {
                        TraitItemStatus::Provided
                    } else {
                        TraitItemStatus::Required
                    },
                };
                if tt.default.is_some() {
                    provided_items.push(item);
                } else {
                    required_items.push(item);
                }
            }
            _ => {}
        }
    }

    TraitData {
        supertraits,
        required_items,
        provided_items,
    }
}

// ---------------------------------------------------------------------------
// Impl block indexing
// ---------------------------------------------------------------------------

fn index_impl_block(
    item: &ItemImpl,
    module: &str,
    source_path: &str,
    index: &mut TraitIndex,
    source_ordinal: usize,
) {
    // Extract trait path, if this is a trait impl.
    let trait_path = item.trait_.as_ref().map(|(_, path, _)| path);
    let Some(trait_path) = trait_path else {
        return; // Inherent impl; not a trait relationship.
    };

    // Check for negative impl (`impl !Trait for Type`).
    // `item.trait_` is `Option<(Option<Token![!]>, Path, Token![for])>`.
    let negative = item
        .trait_
        .as_ref()
        .is_some_and(|(neg, _, _)| neg.is_some());

    // Normalize generics.
    let generics_str = if item.generics.params.is_empty() && item.generics.where_clause.is_none() {
        String::new()
    } else {
        normalize(item.generics.to_token_stream())
    };

    // Extract trait path segments (strip generic arguments for resolution).
    let trait_segments: Vec<String> = trait_path
        .segments
        .iter()
        .map(|seg| seg.ident.to_string())
        .collect();

    let self_type = parse_self_type(&item.self_ty);

    index.impl_blocks.push(RawImplBlock {
        module: module.to_owned(),
        trait_segments,
        self_type,
        generics: generics_str,
        unsafe_trait: item.unsafety.is_some(),
        negative,
        source_path: source_path.to_owned(),
        source_ordinal,
    });
}

/// Parses a self type without collapsing references or tuples to the
/// containing module.
fn parse_self_type(self_ty: &Type) -> RawSelfType {
    match self_ty {
        Type::Path(TypePath { qself: None, path }) => RawSelfType::Path {
            segments: path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect(),
        },
        Type::Reference(reference) => match reference.elem.as_ref() {
            Type::Path(TypePath { qself: None, path }) => RawSelfType::Reference {
                segments: path
                    .segments
                    .iter()
                    .map(|segment| segment.ident.to_string())
                    .collect(),
                lifetime: reference.lifetime.as_ref().map(ToString::to_string),
                mutable: reference.mutability.is_some(),
            },
            _ => RawSelfType::External {
                display: normalize_type_display(self_ty),
            },
        },
        _ => RawSelfType::External {
            display: normalize_type_display(self_ty),
        },
    }
}

fn normalize_type_display(self_ty: &Type) -> String {
    normalize(self_ty).replace(" ,", ",")
}

fn project_self_type_path(self_type: &RawSelfType, resolved_path: &str) -> String {
    match self_type {
        RawSelfType::Path { .. } => resolved_path.to_owned(),
        RawSelfType::Reference {
            lifetime, mutable, ..
        } => {
            let lifetime = lifetime.as_deref().unwrap_or("");
            let mutable = if *mutable { "mut " } else { "" };
            if lifetime.is_empty() {
                format!("&{mutable}{resolved_path}")
            } else {
                format!("&{lifetime} {mutable}{resolved_path}")
            }
        }
        RawSelfType::External { display } => display.clone(),
    }
}

// ---------------------------------------------------------------------------
// Derive detection
// ---------------------------------------------------------------------------

/// Extract trait paths from `#[derive(...)]` attributes.
fn extract_derive_traits(attrs: &[Attribute]) -> Vec<String> {
    let mut traits = Vec::new();
    for attr in attrs {
        if !attr.path().is_ident("derive") {
            continue;
        }
        let meta = &attr.meta;
        let syn::Meta::List(list) = meta else {
            continue;
        };
        // Parse the tokens inside derive(...) as a comma-separated list of paths.
        // syn 2.x does not implement Parse for Punctuated<Path, Comma> directly,
        // so we use a lightweight manual parse: split on commas and parse each
        // path individually.
        let token_str = list.tokens.to_string();
        for part in token_str.split(',') {
            let trimmed = part.trim();
            if !trimmed.is_empty() {
                if let Ok(path) = syn::parse_str::<syn::Path>(trimmed) {
                    traits.push(path_to_string(&path));
                }
            }
        }
    }
    traits
}

/// Converts a `syn::Path` to a simple string representation.
fn path_to_string(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|seg| {
            let ident = seg.ident.to_string();
            match &seg.arguments {
                PathArguments::None => ident,
                PathArguments::AngleBracketed(args) => {
                    let inner: Vec<String> = args.args.iter().map(|_| "_".to_string()).collect();
                    format!("{}<{}>", ident, inner.join(", "))
                }
                PathArguments::Parenthesized(args) => {
                    let inputs: Vec<String> = args.inputs.iter().map(|_| "_".to_string()).collect();
                    format!("{}({}) -> _", ident, inputs.join(", "))
                }
            }
        })
        .collect::<Vec<_>>()
        .join("::")
}

// ---------------------------------------------------------------------------
// Path resolution
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Eq, PartialEq)]
enum Resolved {
    Item((String, String)),
    Module(String),
    NotFound,
    Unresolved,
}

/// Resolves a trait path to a local or external endpoint.
fn resolve_trait_path(
    index: &TraitIndex,
    current: &str,
    segments: &[String],
    visited: &mut BTreeSet<String>,
) -> RelationshipEndpoint {
    match resolve_generic_path(index, current, segments, visited) {
        Some(Resolved::Item((module, ident))) => RelationshipEndpoint::Local { module, ident },
        Some(Resolved::Module(_)) => {
            // A trait path that resolves to a module is unusual; record as
            // unresolved external.
            RelationshipEndpoint::External {
                path: segments.join("::"),
            }
        }
        Some(Resolved::NotFound) | Some(Resolved::Unresolved) | None => {
            // If the first segment is not found locally, it's external.
            RelationshipEndpoint::External {
                path: segments.join("::"),
            }
        }
    }
}

/// Resolves a self-type path to a local or external endpoint.
fn resolve_type_path(
    index: &TraitIndex,
    current: &str,
    segments: &[String],
    visited: &mut BTreeSet<String>,
) -> RelationshipEndpoint {
    match resolve_generic_path(index, current, segments, visited) {
        Some(Resolved::Item((module, ident))) => RelationshipEndpoint::Local { module, ident },
        Some(Resolved::Module(namespace)) => RelationshipEndpoint::External { path: namespace },
        Some(Resolved::NotFound) | Some(Resolved::Unresolved) | None => {
            RelationshipEndpoint::External {
                path: segments.join("::"),
            }
        }
    }
}

/// Resolves a type bound (supertrait path) to an endpoint.
fn resolve_type_bound(
    bound: &syn::TypeParamBound,
    current: &str,
    index: &TraitIndex,
) -> RelationshipEndpoint {
    match bound {
        syn::TypeParamBound::Trait(trait_bound) => {
            let path = &trait_bound.path;
            let segments: Vec<String> = path
                .segments
                .iter()
                .map(|seg| seg.ident.to_string())
                .collect();
            let mut visited = BTreeSet::new();
            resolve_trait_path(index, current, &segments, &mut visited)
        }
        // Lifetime bounds (e.g. `'a`) and other bounds are not trait
        // endpoints. Skip them.
        _ => RelationshipEndpoint::External {
            path: bound
                .to_token_stream()
                .to_string()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
        },
    }
}

/// Indexes items of `canonical` and recurses into inline child modules.
/// `source_path` is the package-root-relative path of the current file.
/// `ordinal` is a mutable counter providing deterministic AST-preorder
/// declaration identity for cfg-gated same-file variants.
fn index_file_items(
    items: &[Item],
    canonical: &str,
    source_path: &str,
    index: &mut TraitIndex,
    ordinal: &mut usize,
) {
    for item in items {
        index_named_item(item, canonical, source_path, index, ordinal);
    }
    for item in items {
        if let Item::Mod(item_mod) = item {
            if let Some((_, content)) = &item_mod.content {
                let child_canonical = format!("{canonical}::{}", item_mod.ident);
                // Inline modules inherit the host file's source path.
                index_file_items(content, &child_canonical, source_path, index, ordinal);
            }
        }
    }
}

fn index_named_item(
    item: &Item,
    canonical: &str,
    source_path: &str,
    index: &mut TraitIndex,
    ordinal: &mut usize,
) {
    let scope = index.scopes.entry(canonical.to_owned()).or_default();
    match item {
        Item::Trait(item_trait) => {
            register_type_name(scope, canonical, &item_trait.ident);
            let data = extract_trait_data(item_trait, canonical, index);
            index
                .traits
                .entry((canonical.to_owned(), item_trait.ident.to_string()))
                .or_default()
                .push(TraitDataVariant {
                    source_path: source_path.to_owned(),
                    source_ordinal: *ordinal,
                    data,
                });
            *ordinal += 1;
        }
        Item::Struct(item_struct) => {
            register_type_name(scope, canonical, &item_struct.ident);
            let generics = normalize(item_struct.generics.to_token_stream());
            index
                .declarations
                .entry((canonical.to_owned(), item_struct.ident.to_string()))
                .or_default()
                .push(TypeDeclVariant {
                    source_path: source_path.to_owned(),
                    source_ordinal: *ordinal,
                    derive_traits: extract_derive_traits(&item_struct.attrs),
                    generics,
                });
            *ordinal += 1;
        }
        Item::Enum(item_enum) => {
            register_type_name(scope, canonical, &item_enum.ident);
            let generics = normalize(item_enum.generics.to_token_stream());
            index
                .declarations
                .entry((canonical.to_owned(), item_enum.ident.to_string()))
                .or_default()
                .push(TypeDeclVariant {
                    source_path: source_path.to_owned(),
                    source_ordinal: *ordinal,
                    derive_traits: extract_derive_traits(&item_enum.attrs),
                    generics,
                });
            *ordinal += 1;
        }
        Item::Union(item_union) => {
            register_type_name(scope, canonical, &item_union.ident);
            let generics = normalize(item_union.generics.to_token_stream());
            index
                .declarations
                .entry((canonical.to_owned(), item_union.ident.to_string()))
                .or_default()
                .push(TypeDeclVariant {
                    source_path: source_path.to_owned(),
                    source_ordinal: *ordinal,
                    derive_traits: extract_derive_traits(&item_union.attrs),
                    generics,
                });
            *ordinal += 1;
        }
        Item::Fn(_) => {
            *ordinal += 1;
        }
        Item::Const(_) => {
            *ordinal += 1;
        }
        Item::Static(_) => {
            *ordinal += 1;
        }
        Item::Type(item_type) => {
            register_type_name(scope, canonical, &item_type.ident);
            *ordinal += 1;
        }
        Item::Impl(item_impl) => {
            index_impl_block(item_impl, canonical, source_path, index, *ordinal);
            *ordinal += 1;
        }
        Item::Use(item_use) => {
            for entry in expand_use_tree(&item_use.tree) {
                scope.imports.push(entry);
            }
        }
        _ => {}
    }
}

/// Resolves a generic path through the type namespace.
fn resolve_generic_path(
    index: &TraitIndex,
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
            // Consume all leading "super" segments iteratively so that
            // super::super::... chains resolve correctly.
            let mut current_module = current.to_owned();
            let mut idx = 0;
            for seg in segments {
                if seg != "super" {
                    break;
                }
                let parent = index
                    .scopes
                    .get(&current_module)
                    .and_then(|s| s.parent.clone());
                match parent {
                    Some(p) => current_module = p,
                    None => return Some(Resolved::Unresolved),
                }
                idx += 1;
            }
            navigate(index, &current_module, &segments[idx..], visited)
        }
        _ => {
            let looked_up = lookup_name(index, current, first, visited);
            match looked_up {
                Resolved::NotFound => {
                    // Genuinely not found locally => it's a bare external
                    // name. Return it as NotFound so the caller can decide.
                    Some(Resolved::NotFound)
                }
                Resolved::Unresolved => Some(Resolved::Unresolved),
                other => descend(index, other, rest, visited),
            }
        }
    }
}

fn navigate(
    index: &TraitIndex,
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
    index: &TraitIndex,
    target: Resolved,
    rest: &[String],
    visited: &mut BTreeSet<String>,
) -> Option<Resolved> {
    match target {
        Resolved::Item(owner) => {
            if rest.is_empty() {
                Some(Resolved::Item(owner))
            } else {
                // Items do not expose sub-paths.
                Some(Resolved::Unresolved)
            }
        }
        Resolved::Module(module) => navigate(index, &module, rest, visited),
        Resolved::NotFound | Resolved::Unresolved => Some(target),
    }
}

fn lookup_name(
    index: &TraitIndex,
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

    // Cycle guard for re-export chains.
    let mut cycle_key = String::from(module);
    cycle_key.push('\0');
    cycle_key.push_str(name);
    if !visited.insert(cycle_key.clone()) {
        return Resolved::Unresolved;
    }

    // Named imports.
    let mut distinct: Vec<Resolved> = Vec::new();
    for entry in &scope.imports {
        if entry.glob {
            continue;
        }
        if entry.local_name.as_deref() == Some(name) {
            match resolve_generic_path(index, module, &entry.path, visited) {
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

    // Glob imports.
    let mut glob_owner: Option<(String, String)> = None;
    for entry in &scope.imports {
        if !entry.glob {
            continue;
        }
        if let Some(Resolved::Module(target)) =
            resolve_generic_path(index, module, &entry.path, visited)
        {
            let found = index
                .scopes
                .get(&target)
                .and_then(|target_scope| target_scope.locals.get(name))
                .cloned();
            if let Some(owner) = found {
                if glob_owner.is_some() && glob_owner.as_ref() != Some(&owner) {
                    visited.remove(&cycle_key);
                    return Resolved::Unresolved;
                }
                glob_owner = Some(owner);
            } else {
                // Try recursive resolution through target's imports.
                match lookup_name(index, &target, name, visited) {
                    Resolved::Item(owner) => {
                        if glob_owner.is_some() && glob_owner.as_ref() != Some(&owner) {
                            visited.remove(&cycle_key);
                            return Resolved::Unresolved;
                        }
                        glob_owner = Some(owner);
                    }
                    Resolved::Unresolved => {
                        visited.remove(&cycle_key);
                        return Resolved::Unresolved;
                    }
                    _ => {}
                }
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
// Helpers: use-tree expansion, normalization
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

/// Normalizes a token stream into a deterministic single-space-separated form.
fn normalize(stream: impl ToTokens) -> String {
    stream
        .to_token_stream()
        .to_string()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_collapses_whitespace() {
        let ty: Type = syn::parse_quote!(Option<T>);
        assert_eq!(normalize(ty.to_token_stream()), "Option < T >");
    }

    #[test]
    fn derive_traits_extracted_from_attributes() {
        let attrs: Vec<Attribute> = syn::parse_quote! {
            #[derive(Clone, Debug, Default)]
        };
        let traits = extract_derive_traits(&attrs);
        assert_eq!(traits, vec!["Clone", "Debug", "Default"]);
    }

    #[test]
    fn derive_traits_empty_for_no_derive() {
        let attrs: Vec<Attribute> = syn::parse_quote! {
            #[inline]
        };
        let traits = extract_derive_traits(&attrs);
        assert!(traits.is_empty());
    }

    #[test]
    fn path_to_string_simple_and_generic() {
        let path: syn::Path = syn::parse_quote!(Default);
        assert_eq!(path_to_string(&path), "Default");

        let path: syn::Path = syn::parse_quote!(std::fmt::Debug);
        assert_eq!(path_to_string(&path), "std::fmt::Debug");
    }
}
