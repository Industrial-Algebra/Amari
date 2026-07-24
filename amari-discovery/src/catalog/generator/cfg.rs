// SPDX-License-Identifier: MIT OR Apache-2.0

//! Typed `#[cfg]` gate recording for publicly reachable surfaces.
//!
//! [`cfg_gates`] composes the Task 5B1 [`ModuleGraph`], the Task 5B2
//! [`ExportGraph`], the Task 5B3 [`SignatureCatalog`], and a
//! [`PackageInventoryRecord`] to associate conservative, normalized cfg gates
//! with every publicly reachable exported declaration.
//!
//! # Model
//!
//! Four expression forms cover the subset of `#[cfg]` that can be reasoned
//! about offline:
//!
//! - [`CfgExpr::Feature`]: `feature = "name"`.
//! - [`CfgExpr::All`]: `all(...)` — commutative conjunction.
//! - [`CfgExpr::Any`]: `any(...)` — commutative disjunction.
//! - `CfgExpr::Not`: `not(...)`.
//!
//! Any predicate that cannot be represented by these forms (for example
//! `target_os`, `target_arch`, `unix`, `cfg_attr`, or malformed attribute
//! shapes) is recorded as [`CfgExpr::UnknownCfg`], preserving the normalized
//! source text of the unsupported predicate so no information is lost.
//!
//! # Gate construction
//!
//! For each exported item the effective gate is the conjunction of:
//!
//! 1. The item's own `#[cfg]` attributes.
//! 2. The `#[cfg]` attributes of every enclosing inline module and of the
//!    module declaration that introduced the item's containing file-backed
//!    module, transitively up to the crate root.
//!
//! Multiple `#[cfg]` attributes on the same item or module are AND-ed
//! together (this matches rustc semantics: `#[cfg(A)] #[cfg(B)]` is
//! equivalent to `#[cfg(all(A, B))]`).
//!
//! # Unknown predicates
//!
//! When an unsupported predicate appears anywhere in the effective gate, it
//! is partitioned out: the remaining known sub-expressions are retained in a
//! [`CfgGate::Conditional`] with an explicit set of unknown predicates, and
//! evaluation returns [`CfgStatus::Unknown`]. If the entire gate consists
//! solely of unsupported predicates the gate is [`CfgGate::Unknown`].
//!
//! # Default-feature evaluation
//!
//! [`feature_default_closure`] computes the local set of features enabled
//! under `default` by following feature-to-feature edges transitively,
//! ignoring `dep:` and dependency feature edges. A declared feature in the
//! closure is Enabled-by-default; a known feature not in the closure is
//! Disabled-by-default; unknown predicates and ungated surfaces are
//! Always/Enabled.
//!
//! # Same-file cfg variants
//!
//! Two declarations that share a canonical module and identifier but are
//! guarded by different `#[cfg]` attributes in the same source file produce
//! distinct [`CfgGateRecord`]s distinguished by [`CfgGateRecord::source_ordinal`],
//! a deterministic AST-preorder position shared with the
//! [`TraitCatalog`](super::traits::TraitCatalog) `source_ordinal` field.
//!   Both modules count every top-level named API declaration and impl block
//!   (Struct, Enum, Union, Fn, Const, Static, Trait, Type, Impl) in source
//!   preorder, so ordinals are cross-catalog comparable.
//!
//! # Limitations
//!
//! - **Macros and generated code** are not extracted (Task 5C2 and 5C3).

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use quote::ToTokens;
use syn::parse::Parser;
use syn::{Attribute, Item};

use crate::{DiscoveryError, DiscoveryResult};

use super::exports::{ExportGraph, ExportSource};
use super::inventory::{FeatureInventoryRecord, PackageInventoryRecord};
use super::modules::{ModuleGraph, ModuleKind, ModuleRecord};
use super::signatures::{AssociatedKind, SignatureCatalog, SignatureKind};

/// Map: (canonical_module, ident) → decl variants with (source_path, ordinal, gate).
type DeclCfgMap = BTreeMap<(String, String), Vec<(String, usize, CfgGate)>>;
/// Map: (declaring_module, re-export leaf name) → re-export-site gate.
/// For glob re-exports the leaf name is the empty string.
type ReexportCfgMap = BTreeMap<(String, String), CfgGate>;
/// Map: (owner_module, owner_ident, member_name, member_kind) → Vec<(source_path, member_ordinal, gate)>
/// for associated items (inherent methods, trait items).
/// Key uses kind (not ordinal) for robust matching against SignatureCatalog alphabetical order.
/// Duplicate cfg variants are preserved as distinct entries under the same key.
type AssociatedCfgMap =
    BTreeMap<(String, String, String, AssociatedKind), Vec<(String, usize, CfgGate)>>;

// ============================================================================
// Public types
// ============================================================================

/// What kind of API surface a [`CfgGateRecord`] represents.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum CfgSurfaceKind {
    /// A top-level item (struct, enum, function, etc.).
    TopLevel,
    /// A reachable module path.
    Module,
    /// An inherent method on a type.
    InherentMethod,
    /// An inherent associated constant on a type.
    InherentConst,
    /// A trait associated method.
    TraitMethod,
    /// A trait associated constant.
    TraitConst,
    /// A trait associated type.
    TraitType,
}

/// A subset of `#[cfg]` predicates that can be reasoned about offline.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub enum CfgExpr {
    /// `feature = "name"`.
    Feature(String),
    /// `all(...)` — commutative conjunction.
    All(Vec<CfgExpr>),
    /// `any(...)` — commutative disjunction.
    Any(Vec<CfgExpr>),
    /// `not(...)`.
    Not(Box<CfgExpr>),
    /// An unsupported predicate, preserving the normalized source text.
    UnknownCfg(String),
}

impl std::fmt::Display for CfgExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Feature(name) => write!(f, "feature({name:?})"),
            Self::All(children) => {
                write!(f, "all(")?;
                for (i, child) in children.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{child}")?;
                }
                write!(f, ")")
            }
            Self::Any(children) => {
                write!(f, "any(")?;
                for (i, child) in children.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{child}")?;
                }
                write!(f, ")")
            }
            Self::Not(inner) => write!(f, "not({inner})"),
            Self::UnknownCfg(text) => write!(f, "unknown_cfg({text})"),
        }
    }
}

impl CfgExpr {
    /// Constructs a normalized `all(...)` expression with sorted, deduplicated
    /// children.
    pub fn all(children: Vec<CfgExpr>) -> Self {
        let mut sorted = children;
        sorted.sort();
        sorted.dedup();
        Self::All(sorted)
    }

    /// Constructs a normalized `any(...)` expression with sorted, deduplicated
    /// children.
    pub fn any(children: Vec<CfgExpr>) -> Self {
        let mut sorted = children;
        sorted.sort();
        sorted.dedup();
        Self::Any(sorted)
    }

    /// Returns whether this expression contains any [`UnknownCfg`](CfgExpr::UnknownCfg).
    fn has_unknown(&self) -> bool {
        match self {
            Self::UnknownCfg(_) => true,
            Self::All(children) | Self::Any(children) => children.iter().any(|c| c.has_unknown()),
            Self::Not(inner) => inner.has_unknown(),
            Self::Feature(_) => false,
        }
    }
}

/// The consolidated cfg gate for a publicly reachable surface.
///
/// Variants represent the three possible states after parsing:
///
/// - [`Always`](CfgGate::Always): no `#[cfg]` attributes apply.
/// - [`Known`](CfgGate::Known): all predicates are understood and captured.
/// - [`Conditional`](CfgGate::Conditional): a mix of known and unknown
///   predicates; the known part is preserved alongside the list of unknown
///   predicates.
/// - [`Unknown`](CfgGate::Unknown): the entire gate consists of unsupported
///   predicates.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum CfgGate {
    /// No cfg gate — always available.
    Always,
    /// Fully understood cfg gate.
    Known(CfgExpr),
    /// Partially understood: known expression plus unknown predicates.
    Conditional {
        /// The known sub-expression.
        expr: CfgExpr,
        /// Texts of unknown predicates.
        unknowns: Vec<String>,
    },
    /// Entirely unknown predicates.
    Unknown(Vec<String>),
}

impl CfgGate {
    /// Evaluates this gate against a map of known feature statuses.
    ///
    /// For [`Known`](CfgGate::Known) gates, evaluates the expression directly.
    /// For [`Conditional`](CfgGate::Conditional) gates, evaluates the full
    /// expression (including any embedded [`UnknownCfg`](CfgExpr::UnknownCfg)
    /// nodes) with Kleene three-valued short-circuit semantics:
    /// `all(Disabled, Unknown) → Disabled`, `any(Enabled, Unknown) → Enabled`,
    /// `not(Unknown) → Unknown`. A pure unknown gate always returns
    /// [`CfgStatus::Unknown`].
    pub fn evaluate(&self, defaults: &BTreeMap<String, CfgStatus>) -> CfgStatus {
        match self {
            Self::Always => CfgStatus::Enabled,
            Self::Unknown(_) => CfgStatus::Unknown,
            Self::Known(expr) => evaluate_expr(expr, defaults),
            Self::Conditional { expr, .. } => {
                // Evaluate the full expression (including UnknownCfg nodes)
                // with Kleene short-circuit semantics:
                //   all(Disabled, Unknown) → Disabled
                //   any(Enabled, Unknown) → Enabled
                //   not(Unknown) → Unknown
                evaluate_expr(expr, defaults)
            }
        }
    }

    /// Combines two gates as a conjunction (AND).
    pub fn and(self, other: CfgGate) -> CfgGate {
        match (self, other) {
            (CfgGate::Always, gate) | (gate, CfgGate::Always) => gate,
            (CfgGate::Known(a), CfgGate::Known(b)) => CfgGate::Known(CfgExpr::all(vec![a, b])),
            (
                CfgGate::Known(expr),
                CfgGate::Conditional {
                    expr: cexpr,
                    unknowns,
                },
            )
            | (
                CfgGate::Conditional {
                    expr: cexpr,
                    unknowns,
                },
                CfgGate::Known(expr),
            ) => {
                let mut merged = unknowns;
                merged.sort();
                merged.dedup();
                CfgGate::Conditional {
                    expr: CfgExpr::all(vec![expr, cexpr]),
                    unknowns: merged,
                }
            }
            (CfgGate::Known(expr), CfgGate::Unknown(mut unknowns))
            | (CfgGate::Unknown(mut unknowns), CfgGate::Known(expr)) => {
                unknowns.sort();
                unknowns.dedup();
                let full_expr = CfgExpr::all(
                    std::iter::once(expr)
                        .chain(unknowns.iter().map(|u| CfgExpr::UnknownCfg(u.clone())))
                        .collect(),
                );
                CfgGate::Conditional {
                    expr: full_expr,
                    unknowns,
                }
            }
            (
                CfgGate::Conditional {
                    expr: a,
                    unknowns: mut ua,
                },
                CfgGate::Conditional {
                    expr: b,
                    unknowns: ub,
                },
            ) => {
                ua.extend(ub);
                ua.sort();
                ua.dedup();
                CfgGate::Conditional {
                    expr: CfgExpr::all(vec![a, b]),
                    unknowns: ua,
                }
            }
            (CfgGate::Unknown(mut ua), CfgGate::Unknown(ub)) => {
                ua.extend(ub);
                ua.sort();
                ua.dedup();
                CfgGate::Unknown(ua)
            }
            (
                CfgGate::Conditional {
                    expr,
                    unknowns: mut ua,
                },
                CfgGate::Unknown(ub),
            )
            | (
                CfgGate::Unknown(ub),
                CfgGate::Conditional {
                    expr,
                    unknowns: mut ua,
                },
            ) => {
                // Only add unknowns not already present in the expression tree.
                let new_unknowns: Vec<String> =
                    ub.iter().filter(|u| !ua.contains(u)).cloned().collect();
                let full_expr = if new_unknowns.is_empty() {
                    expr
                } else {
                    let mut children: Vec<CfgExpr> = Vec::with_capacity(1 + new_unknowns.len());
                    children.push(expr);
                    for u in &new_unknowns {
                        children.push(CfgExpr::UnknownCfg(u.clone()));
                    }
                    CfgExpr::all(children)
                };
                ua.extend(ub);
                ua.sort();
                ua.dedup();
                CfgGate::Conditional {
                    expr: full_expr,
                    unknowns: ua,
                }
            }
        }
    }

    /// Builds a gate from a single known expression, partitioning out
    /// any unknown sub-expressions.
    /// Builds a gate from a single known expression, partitioning out
    /// any unknown sub-expressions.
    pub fn from_expr(expr: CfgExpr) -> Self {
        if !expr.has_unknown() {
            return CfgGate::Known(expr);
        }
        // Collect unknown predicate texts for metadata, but keep the full
        // expression structure (with UnknownCfg nodes) so Kleene evaluation
        // can short-circuit correctly.
        let unknowns = collect_unknown_texts(&expr);
        if unknowns.is_empty() {
            return CfgGate::Known(expr);
        }
        // Check if expr consists ENTIRELY of unknown predicates.
        let non_unknown_count = count_known_nodes(&expr);
        if non_unknown_count == 0 {
            CfgGate::Unknown(unknowns)
        } else {
            CfgGate::Conditional { expr, unknowns }
        }
    }
}

/// Three-valued evaluation of a cfg gate under known feature defaults.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum CfgStatus {
    /// The gate is definitively satisfied.
    Enabled,
    /// The gate is definitively not satisfied.
    Disabled,
    /// The gate cannot be evaluated (contains unknown predicates).
    Unknown,
}

/// A cfg gate attached to a single exported surface.
///
/// Two records may share the same [`path`](Self::path) when mutually-exclusive
/// cfg variants produce different source declarations for the same exported
/// name; they are distinguished by [`source_path`](Self::source_path) and
/// [`source_ordinal`](Self::source_ordinal).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CfgGateRecord {
    /// Exported path, e.g. `crate::module::Name`.
    pub path: String,
    /// Package-root-relative source path, e.g. `src/lib.rs`.
    pub source_path: String,
    /// Deterministic AST preorder ordinal for same-file cfg variants.
    /// Zero when no ordinal differentiation was necessary.
    pub source_ordinal: usize,
    /// The consolidated cfg gate for this surface.
    pub gate: CfgGate,
    /// Evaluation of the gate under the package's local default features.
    pub status: CfgStatus,
    /// What kind of surface this record represents.
    pub surface_kind: CfgSurfaceKind,
}

// ============================================================================
// Three-valued evaluation
// ============================================================================

/// Public three-valued evaluation of a cfg expression against known defaults.
pub fn evaluate_expr(expr: &CfgExpr, defaults: &BTreeMap<String, CfgStatus>) -> CfgStatus {
    match expr {
        CfgExpr::Feature(name) => defaults
            .get(name.as_str())
            .copied()
            .unwrap_or(CfgStatus::Disabled),
        CfgExpr::All(children) => {
            let mut result = CfgStatus::Enabled;
            for child in children {
                match evaluate_expr(child, defaults) {
                    CfgStatus::Disabled => return CfgStatus::Disabled,
                    CfgStatus::Unknown => result = CfgStatus::Unknown,
                    CfgStatus::Enabled => {}
                }
            }
            result
        }
        CfgExpr::Any(children) => {
            let mut result = CfgStatus::Disabled;
            for child in children {
                match evaluate_expr(child, defaults) {
                    CfgStatus::Enabled => return CfgStatus::Enabled,
                    CfgStatus::Unknown => result = CfgStatus::Unknown,
                    CfgStatus::Disabled => {}
                }
            }
            result
        }
        CfgExpr::Not(inner) => match evaluate_expr(inner, defaults) {
            CfgStatus::Enabled => CfgStatus::Disabled,
            CfgStatus::Disabled => CfgStatus::Enabled,
            CfgStatus::Unknown => CfgStatus::Unknown,
        },
        CfgExpr::UnknownCfg(_) => CfgStatus::Unknown,
    }
}

/// Collects all unknown predicate texts from an expression (for metadata).
fn collect_unknown_texts(expr: &CfgExpr) -> Vec<String> {
    let mut texts = Vec::new();
    collect_unknown_inner(expr, &mut texts);
    texts.sort();
    texts.dedup();
    texts
}

fn collect_unknown_inner(expr: &CfgExpr, texts: &mut Vec<String>) {
    match expr {
        CfgExpr::UnknownCfg(text) => texts.push(text.clone()),
        CfgExpr::All(children) | CfgExpr::Any(children) => {
            for child in children {
                collect_unknown_inner(child, texts);
            }
        }
        CfgExpr::Not(inner) => collect_unknown_inner(inner, texts),
        CfgExpr::Feature(_) => {}
    }
}

/// Counts non-UnknownCfg nodes at any depth. Returns 0 when the expression
/// is entirely unknown predicates.
fn count_known_nodes(expr: &CfgExpr) -> usize {
    match expr {
        CfgExpr::UnknownCfg(_) => 0,
        CfgExpr::Feature(_) => 1,
        CfgExpr::Not(inner) => count_known_nodes(inner),
        CfgExpr::All(children) | CfgExpr::Any(children) => {
            children.iter().map(count_known_nodes).sum()
        }
    }
}

// ============================================================================
// Feature default closure
// ============================================================================

/// Computes the set of feature names enabled by `default` through local
/// feature-to-feature edges, ignoring `dep:` and dependency feature edges.
///
/// `features` must be the [`FeatureInventoryRecord`] list from a
/// [`PackageInventoryRecord`].
///
/// The closure includes every feature transitively reachable from the
/// `default` feature via non-dep feature edges. It does *not* inspect
/// dependency features.
pub fn feature_default_closure(features: &[FeatureInventoryRecord]) -> BTreeSet<String> {
    let feature_map: BTreeMap<&str, &[String]> = features
        .iter()
        .map(|f| (f.name.as_str(), f.enables.as_slice()))
        .collect();

    let mut closure = BTreeSet::new();
    let mut stack: Vec<&str> = vec!["default"];

    while let Some(current) = stack.pop() {
        let Some(enables) = feature_map.get(current) else {
            continue;
        };
        for edge in *enables {
            if edge.starts_with("dep:") {
                continue;
            }
            if edge.contains('/') {
                continue;
            }
            if closure.insert(edge.clone()) {
                stack.push(edge.as_str());
            }
        }
    }

    closure
}

/// Computes default feature status map from package inventory.
fn default_status_map(inventory: &PackageInventoryRecord) -> BTreeMap<String, CfgStatus> {
    let closure = feature_default_closure(&inventory.features);

    let all_features: BTreeSet<&str> = inventory.features.iter().map(|f| f.name.as_str()).collect();

    let mut map = BTreeMap::new();
    for feature in &all_features {
        let status = if closure.contains(*feature) {
            CfgStatus::Enabled
        } else {
            CfgStatus::Disabled
        };
        map.insert(feature.to_string(), status);
    }
    map
}

// ============================================================================
// Public entry point
// ============================================================================

/// Records cfg gates for every publicly reachable exported declaration.
///
/// `graph`, `exports`, and `sigs` must have been built for the same
/// `package_root`. `inventory` supplies the declared feature set needed for
/// default-feature evaluation.
///
/// The function re-reads each file-backed module source to collect `#[cfg]`
/// attributes. It performs no Cargo, rustc, or network access.
///
/// `sigs` must have been built for the same `package_root`; its
/// associated-item list is used to emit per-member [`CfgGateRecord`]s under
/// alias-projected paths.
///
/// # Limitations
///
/// - **Macros and generated code** are not extracted.
///
/// # Errors
///
/// Returns [`crate::DiscoveryError::CatalogCorruption`] when a recorded source
/// file cannot be read or parsed, or when the package root cannot be resolved.
pub fn cfg_gates(
    graph: &ModuleGraph,
    exports: &ExportGraph,
    sigs: &SignatureCatalog,
    inventory: &PackageInventoryRecord,
    package_root: &Path,
) -> DiscoveryResult<Vec<CfgGateRecord>> {
    let canonical_root = fs::canonicalize(package_root).map_err(|error| {
        DiscoveryError::CatalogCorruption(format!(
            "cannot resolve package root {}: {error}",
            package_root.display()
        ))
    })?;

    let default_map = default_status_map(inventory);

    // -------------------------------------------------------------------
    // 1. Collect item and module cfg gates from source files.
    // -------------------------------------------------------------------
    // Map: (canonical_module, ident) → Vec<(source_path, ordinal, item_cfg)>
    let mut item_cfgs: DeclCfgMap = BTreeMap::new();
    // Map: canonical_module_path → module_cfg (the #[cfg] on the mod decl)
    let mut module_cfgs: BTreeMap<String, CfgGate> = BTreeMap::new();

    let mut reexport_cfgs: ReexportCfgMap = BTreeMap::new();
    let mut associated_cfgs: AssociatedCfgMap = BTreeMap::new();
    collect_cfgs(
        graph,
        &canonical_root,
        &mut item_cfgs,
        &mut module_cfgs,
        &mut reexport_cfgs,
        &mut associated_cfgs,
    )?;

    // -------------------------------------------------------------------
    // 2. Build the inherited module gate for each canonical module.
    // -------------------------------------------------------------------
    let inherited = inherited_module_gates(graph, &module_cfgs);

    // -------------------------------------------------------------------
    // 3. Build a map: export_path → set of (module, ident) source identities.
    // -------------------------------------------------------------------
    let mut path_sources: BTreeMap<String, BTreeSet<(String, String)>> = BTreeMap::new();
    for export in &exports.exports {
        if let ExportSource::Local { module, ident, .. } = &export.source {
            path_sources
                .entry(export.path.clone())
                .or_default()
                .insert((module.clone(), ident.clone()));
        }
    }

    // -------------------------------------------------------------------
    // 4. For each (export_path, source_identity), emit one CfgGateRecord
    //    per declaration cfg variant.
    // -------------------------------------------------------------------
    let mut records: Vec<CfgGateRecord> = Vec::new();

    for (export_path, source_ids) in &path_sources {
        for (module, ident) in source_ids {
            let decl_key = (module.clone(), ident.clone());
            let variants = item_cfgs.get(&decl_key);

            let module_inherited = inherited.get(module).cloned().unwrap_or(CfgGate::Always);

            match variants {
                Some(variants) if !variants.is_empty() => {
                    for (source_path, ordinal, item_gate) in variants {
                        let combined = item_gate.clone().and(module_inherited.clone());
                        // Apply re-export-site gate if this is a re-export.
                        let combined =
                            apply_reexport_gate(&combined, export_path, module, &reexport_cfgs);
                        let status = combined.evaluate(&default_map);
                        records.push(CfgGateRecord {
                            path: export_path.clone(),
                            source_path: source_path.clone(),
                            source_ordinal: *ordinal,
                            gate: combined,
                            status,
                            surface_kind: CfgSurfaceKind::TopLevel,
                        });
                    }
                }
                _ => {
                    // Declaration not found in our cfg index (possibly
                    // macro-generated). Emit Unknown rather than Always.
                    // Still check for re-export-site gate.
                    let source_path = graph
                        .modules
                        .iter()
                        .find(|m| &m.path == module)
                        .and_then(|m| m.source_path.clone())
                        .unwrap_or_default();
                    let gate = CfgGate::Unknown(vec!["declaration_cfg_unavailable".to_owned()]);
                    let gate = apply_reexport_gate(&gate, export_path, module, &reexport_cfgs);
                    let status = gate.evaluate(&default_map);
                    records.push(CfgGateRecord {
                        path: export_path.clone(),
                        source_path,
                        source_ordinal: 0,
                        gate,
                        status,
                        surface_kind: CfgSurfaceKind::TopLevel,
                    });
                }
            }
        }
    }

    // -------------------------------------------------------------------
    // 5. Emit CfgGateRecord for associated items (inherent methods/consts,
    //    trait associated items) at alias-projected paths.
    // -------------------------------------------------------------------
    emit_associated_item_records(
        sigs,
        &item_cfgs,
        &associated_cfgs,
        &inherited,
        &default_map,
        &reexport_cfgs,
        &mut records,
    );

    // -------------------------------------------------------------------
    // 6. Also record gates for module-export paths.
    // -------------------------------------------------------------------
    for export in &exports.exports {
        if records.iter().any(|r| r.path == export.path) {
            continue;
        }
        if let ExportSource::Module { module } = &export.source {
            let gate = inherited.get(module).cloned().unwrap_or(CfgGate::Always);
            let status = gate.evaluate(&default_map);
            let source_path = graph
                .modules
                .iter()
                .find(|m| m.path == *module)
                .and_then(|m| m.source_path.clone())
                .unwrap_or_default();
            records.push(CfgGateRecord {
                path: export.path.clone(),
                source_path,
                source_ordinal: 0,
                gate,
                status,
                surface_kind: CfgSurfaceKind::Module,
            });
        }
    }

    // -------------------------------------------------------------------
    // 5. Sort for determinism.
    // -------------------------------------------------------------------
    records.sort_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then_with(|| a.source_path.cmp(&b.source_path))
            .then_with(|| a.source_ordinal.cmp(&b.source_ordinal))
    });

    Ok(records)
}

// ============================================================================
// Module inheritance
// ============================================================================

fn inherited_module_gates(
    graph: &ModuleGraph,
    module_cfgs: &BTreeMap<String, CfgGate>,
) -> BTreeMap<String, CfgGate> {
    let parent_map: BTreeMap<&str, Option<&str>> = graph
        .modules
        .iter()
        .map(|m| (m.path.as_str(), m.parent.as_deref()))
        .collect();

    let mut result = BTreeMap::new();

    for record in &graph.modules {
        let mut gates: Vec<CfgGate> = Vec::new();
        let mut current: Option<&str> = Some(record.path.as_str());
        while let Some(path) = current {
            if path != "crate" {
                if let Some(gate) = module_cfgs.get(path) {
                    gates.push(gate.clone());
                }
            }
            current = parent_map.get(path).copied().flatten();
        }
        let combined = gates.into_iter().fold(CfgGate::Always, |acc, g| acc.and(g));
        result.insert(record.path.clone(), combined);
    }

    result
}

/// Emits [`CfgGateRecord`] entries for associated items from the
/// [`SignatureCatalog`].
///
/// Each associated item in the catalog is matched by `(module, owner_ident,
/// name, kind)` against the cfg-indexed associated-item gates. When multiple
/// cfg variants exist for the same member (e.g. the same method name appears
/// in distinct cfg-gated impl blocks), every variant is emitted as a separate
/// [`CfgGateRecord`] under each applicable owner alias.
fn emit_associated_item_records(
    sigs: &SignatureCatalog,
    item_cfgs: &DeclCfgMap,
    associated_cfgs: &AssociatedCfgMap,
    inherited: &BTreeMap<String, CfgGate>,
    default_map: &BTreeMap<String, CfgStatus>,
    reexport_cfgs: &ReexportCfgMap,
    records: &mut Vec<CfgGateRecord>,
) {
    for sig_rec in &sigs.records {
        if sig_rec.associated.is_empty() {
            continue;
        }
        // Look up the owner's declaration cfg gate.
        let owner_key = (sig_rec.source.module.clone(), sig_rec.source.ident.clone());
        let owner_module_inherited = inherited
            .get(&sig_rec.source.module)
            .cloned()
            .unwrap_or(CfgGate::Always);

        // Collect all owner cfg variants to apply to each member.
        let owner_cfg_variants: Vec<(String, usize, CfgGate)> =
            item_cfgs.get(&owner_key).cloned().unwrap_or_default();

        // If no declaration-level cfgs, the item's declared gate is Always.
        let owner_gate_variants: Vec<CfgGate> = if owner_cfg_variants.is_empty() {
            vec![CfgGate::Always]
        } else {
            owner_cfg_variants
                .iter()
                .map(|(_, _, g)| g.clone())
                .collect()
        };

        for assoc in &sig_rec.associated {
            // Determine surface kind.
            let surface_kind = match sig_rec.kind {
                SignatureKind::Trait => match assoc.kind {
                    AssociatedKind::Method => CfgSurfaceKind::TraitMethod,
                    AssociatedKind::Const => CfgSurfaceKind::TraitConst,
                    AssociatedKind::Type => CfgSurfaceKind::TraitType,
                },
                SignatureKind::Struct | SignatureKind::Enum | SignatureKind::Union => {
                    match assoc.kind {
                        AssociatedKind::Method => CfgSurfaceKind::InherentMethod,
                        AssociatedKind::Const => CfgSurfaceKind::InherentConst,
                        AssociatedKind::Type => {
                            // Inherent associated types are not a thing in
                            // stable Rust; fall back to TopLevel.
                            CfgSurfaceKind::TopLevel
                        }
                    }
                }
                _ => CfgSurfaceKind::TopLevel,
            };

            // Look up cfg variants for this member by (module, owner, name, kind).
            // Each variant may come from a distinct impl block or cfg bifurcation.
            let member_key = (
                sig_rec.source.module.clone(),
                sig_rec.source.ident.clone(),
                assoc.name.clone(),
                assoc.kind,
            );
            let associated_variants = associated_cfgs.get(&member_key);
            let member_gate_variants: Vec<(String, usize, CfgGate)> = match associated_variants {
                Some(variants) if !variants.is_empty() => variants.clone(),
                _ => vec![(sig_rec.source.source_path.clone(), 0, CfgGate::Always)],
            };

            // Emit one record per (owner_gate × member_variant) combination.
            for owner_gate in &owner_gate_variants {
                for (member_source_path, member_ord, member_gate) in &member_gate_variants {
                    let combined = owner_gate
                        .clone()
                        .and(owner_module_inherited.clone())
                        .and(member_gate.clone());
                    // Apply re-export gate for the owner's path.
                    let combined = apply_reexport_gate(
                        &combined,
                        &sig_rec.path,
                        &sig_rec.source.module,
                        reexport_cfgs,
                    );
                    let status = combined.evaluate(default_map);

                    let member_path = format!("{}::{}", sig_rec.path, assoc.name);
                    records.push(CfgGateRecord {
                        path: member_path,
                        source_path: member_source_path.clone(),
                        source_ordinal: *member_ord,
                        gate: combined,
                        status,
                        surface_kind,
                    });
                }
            }
        }
    }
}

/// If `export_path` is a re-export (its namespace prefix differs from the
/// source module), look up the re-export-site cfg gate and AND it with the
/// existing gate. Returns the existing gate unchanged otherwise.
fn apply_reexport_gate(
    gate: &CfgGate,
    export_path: &str,
    source_module: &str,
    reexport_cfgs: &ReexportCfgMap,
) -> CfgGate {
    // Extract the namespace prefix (everything before the last ::) and the leaf.
    let (namespace, leaf) = match export_path.rsplit_once("::") {
        Some((prefix, leaf)) => (prefix, leaf),
        None => return gate.clone(), // Crate root exports are not re-exports.
    };

    // If the namespace matches the source module, it's a direct path, not a re-export.
    if namespace == source_module {
        return gate.clone();
    }

    // Look up the re-export-site gate. Try named re-export first, then glob.
    let named_key = (namespace.to_owned(), leaf.to_owned());
    let glob_key = (namespace.to_owned(), String::new());

    if let Some(reexport_gate) = reexport_cfgs.get(&named_key) {
        gate.clone().and(reexport_gate.clone())
    } else if let Some(reexport_gate) = reexport_cfgs.get(&glob_key) {
        // For glob re-exports, provenance is uncertain — emit Conditional
        // if the glob has a cfg gate. Combine the gate with an Unknown
        // flag to indicate reduced certainty.
        let combined = gate.clone().and(reexport_gate.clone());
        let unknown_token = format!("glob_reexport_from_{}", namespace.replace("::", "_"));
        match combined {
            CfgGate::Always => combined,
            CfgGate::Known(expr) => CfgGate::Conditional {
                expr,
                unknowns: vec![unknown_token],
            },
            CfgGate::Conditional { expr, mut unknowns } => {
                unknowns.push(unknown_token);
                unknowns.sort();
                unknowns.dedup();
                CfgGate::Conditional { expr, unknowns }
            }
            CfgGate::Unknown(mut unknowns) => {
                unknowns.push(unknown_token);
                unknowns.sort();
                unknowns.dedup();
                CfgGate::Unknown(unknowns)
            }
        }
    } else {
        gate.clone()
    }
}

// ============================================================================
// Source-walking: collecting item and module cfg gates
// ============================================================================

fn collect_cfgs(
    graph: &ModuleGraph,
    package_root: &Path,
    item_cfgs: &mut DeclCfgMap,
    module_cfgs: &mut BTreeMap<String, CfgGate>,
    reexport_cfgs: &mut ReexportCfgMap,
    associated_cfgs: &mut AssociatedCfgMap,
) -> DiscoveryResult<()> {
    // Group non-inline modules by source_path so we parse each file once.
    let mut file_modules: BTreeMap<&str, Vec<&ModuleRecord>> = BTreeMap::new();
    for record in &graph.modules {
        if let Some(source_path) = &record.source_path {
            if record.kind != ModuleKind::Inline {
                file_modules
                    .entry(source_path.as_str())
                    .or_default()
                    .push(record);
            }
        }
    }

    for source_path in file_modules.keys() {
        let file = package_root.join(source_path);
        let source = fs::read_to_string(&file).map_err(|error| {
            DiscoveryError::CatalogCorruption(format!(
                "cannot read module {}: {error}",
                file.display()
            ))
        })?;
        let ast = syn::parse_file(&source).map_err(|error| {
            DiscoveryError::CatalogCorruption(format!("cannot parse {}: {error}", file.display()))
        })?;

        // Use the module's canonical path, not hardcoded "crate".
        // Multiple cfg-deferred variants may share the same source_path;
        // they all have the same canonical path by construction. Use the
        // first variant's path.
        let canonical = file_modules[source_path]
            .first()
            .map(|rec| rec.path.as_str())
            .unwrap_or("crate");

        let mut ordinal = 0usize;
        collect_cfg_reexports(&ast.items, canonical, reexport_cfgs);
        collect_from_items(
            &ast.items,
            canonical,
            source_path,
            item_cfgs,
            module_cfgs,
            &mut ordinal,
        );
        collect_associated_cfgs(&ast.items, canonical, source_path, associated_cfgs);
    }

    Ok(())
}

/// Collects `#[cfg]` gates on `pub use` statements for re-export resolution.
///
/// For each `pub use` leaf, records the re-export-site gate in `reexport_cfgs`
/// keyed by `(canonical_module, leaf_name)`. Glob re-exports use the empty
/// string as the leaf name. Inline modules are recursed into.
fn collect_cfg_reexports(items: &[Item], canonical: &str, reexport_cfgs: &mut ReexportCfgMap) {
    for item in items {
        if let Item::Use(item_use) = item {
            // Only record `pub use` re-exports.
            if !matches!(&item_use.vis, syn::Visibility::Public(_)) {
                continue;
            }
            let reexport_gate = attrs_to_gate(&item_use.attrs);
            if reexport_gate == CfgGate::Always {
                continue; // No cfg on this re-export.
            }
            for entry in expand_use_tree_for_cfg(&item_use.tree) {
                let key = if entry.glob {
                    (canonical.to_owned(), String::new())
                } else {
                    (canonical.to_owned(), entry.local_name.unwrap_or_default())
                };
                reexport_cfgs.entry(key).or_insert(reexport_gate.clone());
            }
        }
        // Recurse into inline modules.
        if let Item::Mod(item_mod) = item {
            if let Some((_, content)) = &item_mod.content {
                let child_canonical = format!("{canonical}::{}", item_mod.ident);
                collect_cfg_reexports(content, &child_canonical, reexport_cfgs);
            }
        }
    }
}

/// A flattened `use` entry for re-export cfg collection.
#[derive(Clone, Debug)]
struct CfgUseEntry {
    local_name: Option<String>,
    glob: bool,
}

fn expand_use_tree_for_cfg(tree: &syn::UseTree) -> Vec<CfgUseEntry> {
    let mut entries = Vec::new();
    flatten_use_tree_cfg(tree, &mut entries);
    entries
}

fn flatten_use_tree_cfg(tree: &syn::UseTree, entries: &mut Vec<CfgUseEntry>) {
    use syn::UseTree;
    match tree {
        UseTree::Path(path) => flatten_use_tree_cfg(&path.tree, entries),
        UseTree::Name(name) => {
            entries.push(CfgUseEntry {
                local_name: Some(name.ident.to_string()),
                glob: false,
            });
        }
        UseTree::Rename(rename) => {
            entries.push(CfgUseEntry {
                local_name: Some(rename.rename.to_string()),
                glob: false,
            });
        }
        UseTree::Glob(_) => {
            entries.push(CfgUseEntry {
                local_name: None,
                glob: true,
            });
        }
        UseTree::Group(group) => {
            for inner in &group.items {
                flatten_use_tree_cfg(inner, entries);
            }
        }
    }
}

/// Collects `#[cfg]` gates on associated items (inherent methods/consts and
/// trait associated items).
///
/// Keys are `(owner_module, owner_ident, member_name, member_kind)` so lookup
/// is robust against SignatureCatalog alphabetical sort order. The impl
/// block's own `#[cfg]` attributes are AND-ed with each member's gate.
/// Every associated item is stored (including Always) to preserve distinct
/// records when the same method name appears in multiple cfg-variant impl
/// blocks.
/// Inline modules are recursed into.
fn collect_associated_cfgs(
    items: &[Item],
    canonical: &str,
    source_path: &str,
    associated_cfgs: &mut AssociatedCfgMap,
) {
    for item in items {
        match item {
            Item::Impl(impl_item) => {
                // Inherent impl (no trait reference).
                if impl_item.trait_.is_some() {
                    continue;
                }
                let owner_ident = type_path_last_ident(&impl_item.self_ty);
                let Some(owner_ident) = owner_ident else {
                    continue;
                };
                let impl_gate = attrs_to_gate(&impl_item.attrs);
                let mut member_ordinal = 0usize;
                for impl_item in &impl_item.items {
                    let (member_name, member_gate, member_kind) = match impl_item {
                        syn::ImplItem::Fn(f) => (
                            f.sig.ident.to_string(),
                            attrs_to_gate(&f.attrs),
                            AssociatedKind::Method,
                        ),
                        syn::ImplItem::Const(c) => (
                            c.ident.to_string(),
                            attrs_to_gate(&c.attrs),
                            AssociatedKind::Const,
                        ),
                        _ => continue,
                    };
                    let combined = member_gate.and(impl_gate.clone());
                    associated_cfgs
                        .entry((
                            canonical.to_owned(),
                            owner_ident.clone(),
                            member_name,
                            member_kind,
                        ))
                        .or_default()
                        .push((source_path.to_owned(), member_ordinal, combined));
                    member_ordinal += 1;
                }
            }
            Item::Trait(trait_item) => {
                let owner_ident = trait_item.ident.to_string();
                let mut member_ordinal = 0usize;
                for trait_member in &trait_item.items {
                    let (member_name, member_gate, member_kind) = match trait_member {
                        syn::TraitItem::Fn(f) => (
                            f.sig.ident.to_string(),
                            attrs_to_gate(&f.attrs),
                            AssociatedKind::Method,
                        ),
                        syn::TraitItem::Const(c) => (
                            c.ident.to_string(),
                            attrs_to_gate(&c.attrs),
                            AssociatedKind::Const,
                        ),
                        syn::TraitItem::Type(t) => (
                            t.ident.to_string(),
                            attrs_to_gate(&t.attrs),
                            AssociatedKind::Type,
                        ),
                        _ => continue,
                    };
                    associated_cfgs
                        .entry((
                            canonical.to_owned(),
                            owner_ident.clone(),
                            member_name,
                            member_kind,
                        ))
                        .or_default()
                        .push((source_path.to_owned(), member_ordinal, member_gate));
                    member_ordinal += 1;
                }
            }
            Item::Mod(item_mod) => {
                if let Some((_, content)) = &item_mod.content {
                    let child_canonical = format!("{canonical}::{}", item_mod.ident);
                    collect_associated_cfgs(
                        content,
                        &child_canonical,
                        source_path,
                        associated_cfgs,
                    );
                }
            }
            _ => {}
        }
    }
}

/// Extracts the last identifier segment from a type path for ownership lookup.
fn type_path_last_ident(ty: &syn::Type) -> Option<String> {
    let syn::Type::Path(type_path) = ty else {
        return None;
    };
    if type_path.qself.is_some() {
        return None;
    }
    type_path
        .path
        .segments
        .last()
        .map(|seg| seg.ident.to_string())
}

/// Recursively walks items from a source file, collecting item-level and
/// module-level cfg gates. Inline modules are recursed into with their
/// canonical path.
fn collect_from_items(
    items: &[Item],
    canonical: &str,
    source_path: &str,
    item_cfgs: &mut DeclCfgMap,
    module_cfgs: &mut BTreeMap<String, CfgGate>,
    ordinal: &mut usize,
) {
    for item in items {
        let item_cfg = attrs_to_gate(item.attrs(item));
        match item {
            Item::Struct(s) => {
                let ident = s.ident.to_string();
                item_cfgs
                    .entry((canonical.to_owned(), ident))
                    .or_default()
                    .push((source_path.to_owned(), *ordinal, item_cfg));
                *ordinal += 1;
            }
            Item::Enum(e) => {
                let ident = e.ident.to_string();
                item_cfgs
                    .entry((canonical.to_owned(), ident))
                    .or_default()
                    .push((source_path.to_owned(), *ordinal, item_cfg));
                *ordinal += 1;
            }
            Item::Union(u) => {
                let ident = u.ident.to_string();
                item_cfgs
                    .entry((canonical.to_owned(), ident))
                    .or_default()
                    .push((source_path.to_owned(), *ordinal, item_cfg));
                *ordinal += 1;
            }
            Item::Fn(fl) => {
                let ident = fl.sig.ident.to_string();
                item_cfgs
                    .entry((canonical.to_owned(), ident))
                    .or_default()
                    .push((source_path.to_owned(), *ordinal, item_cfg));
                *ordinal += 1;
            }
            Item::Const(c) => {
                let ident = c.ident.to_string();
                item_cfgs
                    .entry((canonical.to_owned(), ident))
                    .or_default()
                    .push((source_path.to_owned(), *ordinal, item_cfg));
                *ordinal += 1;
            }
            Item::Static(st) => {
                let ident = st.ident.to_string();
                item_cfgs
                    .entry((canonical.to_owned(), ident))
                    .or_default()
                    .push((source_path.to_owned(), *ordinal, item_cfg));
                *ordinal += 1;
            }
            Item::Trait(t) => {
                let ident = t.ident.to_string();
                item_cfgs
                    .entry((canonical.to_owned(), ident))
                    .or_default()
                    .push((source_path.to_owned(), *ordinal, item_cfg));
                *ordinal += 1;
            }
            Item::Type(t) => {
                let ident = t.ident.to_string();
                item_cfgs
                    .entry((canonical.to_owned(), ident))
                    .or_default()
                    .push((source_path.to_owned(), *ordinal, item_cfg));
                *ordinal += 1;
            }
            Item::Impl(_impl_item) => {
                // Impl blocks advance the ordinal (shared convention with
                // traits.rs) but don't produce a cfg-gate record themselves.
                *ordinal += 1;
            }
            Item::Mod(item_mod) => {
                // Record the module's own cfg gate.
                let child_canonical = format!("{canonical}::{}", item_mod.ident);
                let module_gate = attrs_to_gate(item.attrs(item));
                // Only record the first (most permissive) gate for a module.
                module_cfgs
                    .entry(child_canonical.clone())
                    .or_insert(module_gate.clone());

                if let Some((_, content)) = &item_mod.content {
                    // Inline module — recurse into its items.
                    collect_from_items(
                        content,
                        &child_canonical,
                        source_path,
                        item_cfgs,
                        module_cfgs,
                        ordinal,
                    );
                }
            }
            _ => {}
        }
    }
}

// ============================================================================
// Attribute parsing
// ============================================================================

/// Extracts and normalizes cfg from all `#[cfg(...)]` attributes on an item
/// or module.
///
/// Multiple `#[cfg(A)] #[cfg(B)]` are AND-ed together. The result may be
/// [`CfgGate::Always`] if no cfg attributes are present.
fn attrs_to_gate(attrs: &[Attribute]) -> CfgGate {
    let mut gates: Vec<CfgExpr> = Vec::new();
    for attr in attrs {
        if !attr.path().is_ident("cfg") {
            continue;
        }
        let meta = &attr.meta;
        if let syn::Meta::List(list) = meta {
            if let Some(expr) = parse_cfg_tokens(&list.tokens.to_string()) {
                gates.push(expr);
            }
        }
    }

    if gates.is_empty() {
        return CfgGate::Always;
    }

    let combined = if gates.len() == 1 {
        gates.into_iter().next().unwrap()
    } else {
        CfgExpr::all(gates)
    };

    CfgGate::from_expr(combined)
}

/// Parses the contents of a `#[cfg(...)]` attribute into a [`CfgExpr`].
///
/// Supported forms:
/// - `feature = "name"`
/// - `all(...)`
/// - `any(...)`
/// - `not(...)`
///
/// Everything else (including `target_os`, `target_arch`, `unix`,
/// key-value other than feature, malformed shapes) becomes
/// [`CfgExpr::UnknownCfg`].
fn parse_cfg_tokens(source: &str) -> Option<CfgExpr> {
    let trimmed = source.trim();

    if trimmed.is_empty() {
        return None;
    }

    // Try parsing as syn::Meta (for top-level predicates like `feature = "x"`,
    // `unix`, `target_os = "linux"`)
    if let Ok(meta) = syn::parse_str::<syn::Meta>(trimmed) {
        return Some(parse_cfg_meta(&meta));
    }

    // Try parsing as a macro-style call for all(...) / any(...) / not(...).
    if let Ok(mac) = syn::parse_str::<syn::Macro>(trimmed) {
        let name = mac.path.segments.last().map(|s| s.ident.to_string());
        let mac_source = mac.tokens.to_string();
        match name.as_deref() {
            Some("all") => {
                let children = parse_comma_separated_metas(&mac_source);
                return Some(CfgExpr::all(children));
            }
            Some("any") => {
                let children = parse_comma_separated_metas(&mac_source);
                return Some(CfgExpr::any(children));
            }
            Some("not") => {
                let children = parse_comma_separated_metas(&mac_source);
                if children.len() == 1 {
                    return Some(CfgExpr::Not(Box::new(children.into_iter().next().unwrap())));
                }
                return Some(CfgExpr::UnknownCfg(normalize_cfg_text(trimmed)));
            }
            _ => return Some(CfgExpr::UnknownCfg(normalize_cfg_text(trimmed))),
        }
    }

    Some(CfgExpr::UnknownCfg(normalize_cfg_text(trimmed)))
}

/// Parses a `syn::Meta` into a [`CfgExpr`].
fn parse_cfg_meta(meta: &syn::Meta) -> CfgExpr {
    match meta {
        syn::Meta::Path(path) => CfgExpr::UnknownCfg(normalize_cfg_text(&path_to_string(path))),
        syn::Meta::NameValue(name_value) => {
            let key = path_to_string(&name_value.path);
            let value = expr_to_string(&name_value.value);
            if key == "feature" {
                CfgExpr::Feature(value)
            } else {
                CfgExpr::UnknownCfg(format!("{key} = {value:?}"))
            }
        }
        syn::Meta::List(list) => {
            let name = path_to_string(&list.path);
            let tokens_str = list.tokens.to_string();
            match name.as_str() {
                "all" => {
                    let children = parse_comma_separated_metas(&tokens_str);
                    CfgExpr::all(children)
                }
                "any" => {
                    let children = parse_comma_separated_metas(&tokens_str);
                    CfgExpr::any(children)
                }
                "not" => {
                    let children = parse_comma_separated_metas(&tokens_str);
                    if children.len() == 1 {
                        CfgExpr::Not(Box::new(children.into_iter().next().unwrap()))
                    } else {
                        CfgExpr::UnknownCfg(format!("not({})", tokens_str.trim()))
                    }
                }
                _ => CfgExpr::UnknownCfg(normalize_cfg_text(&list.tokens.to_string())),
            }
        }
    }
}

/// Parses a comma-separated list of `syn::Meta` expressions from a source
/// string (the content inside `all(...)`, `any(...)`, or `not(...)`).
fn parse_comma_separated_metas(source: &str) -> Vec<CfgExpr> {
    // Try parsing as a Punctuated list.
    if let Ok(punctuated) =
        syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated.parse_str(source)
    {
        return punctuated.iter().map(parse_cfg_meta).collect();
    }
    // Fallback: split on commas.
    let mut results = Vec::new();
    for part in source.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(meta) = syn::parse_str::<syn::Meta>(trimmed) {
            results.push(parse_cfg_meta(&meta));
        } else {
            results.push(CfgExpr::UnknownCfg(normalize_cfg_text(trimmed)));
        }
    }
    results
}

/// Normalizes cfg source text: collapses whitespace, trims.
fn normalize_cfg_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Best-effort path to string, collapsing whitespace.
fn path_to_string(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|seg| seg.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

/// Best-effort expression to string.
fn expr_to_string(expr: &syn::Expr) -> String {
    match expr {
        syn::Expr::Lit(lit) => match &lit.lit {
            syn::Lit::Str(s) => s.value(),
            _ => lit.lit.to_token_stream().to_string(),
        },
        _ => expr.to_token_stream().to_string(),
    }
}

// ============================================================================
// Helper: getting attrs from an Item
// ============================================================================

trait ItemAttrs {
    fn attrs(&self, item: &Item) -> &[Attribute];
}

impl ItemAttrs for Item {
    fn attrs(&self, _item: &Item) -> &[Attribute] {
        match self {
            Item::Const(c) => &c.attrs,
            Item::Enum(e) => &e.attrs,
            Item::ExternCrate(ec) => &ec.attrs,
            Item::Fn(f) => &f.attrs,
            Item::ForeignMod(fm) => &fm.attrs,
            Item::Impl(i) => &i.attrs,
            Item::Macro(m) => &m.attrs,
            Item::Mod(m) => &m.attrs,
            Item::Static(s) => &s.attrs,
            Item::Struct(s) => &s.attrs,
            Item::Trait(t) => &t.attrs,
            Item::TraitAlias(ta) => &ta.attrs,
            Item::Type(t) => &t.attrs,
            Item::Union(u) => &u.attrs,
            Item::Use(u) => &u.attrs,
            Item::Verbatim(_v) => &[], // Verbatim has no attrs
            _ => &[],
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_display_is_quoted() {
        assert_eq!(
            CfgExpr::Feature("foo".to_owned()).to_string(),
            r#"feature("foo")"#
        );
    }

    #[test]
    fn not_display_nests() {
        let expr = CfgExpr::Not(Box::new(CfgExpr::Feature("bar".to_owned())));
        assert_eq!(expr.to_string(), r#"not(feature("bar"))"#);
    }

    #[test]
    fn unknown_cfg_display() {
        assert_eq!(
            CfgExpr::UnknownCfg(r#"target_os = "linux""#.to_owned()).to_string(),
            r#"unknown_cfg(target_os = "linux")"#
        );
    }

    #[test]
    fn all_normalization_sorts_and_deduplicates() {
        let expr = CfgExpr::all(vec![
            CfgExpr::Feature("b".to_owned()),
            CfgExpr::Feature("a".to_owned()),
            CfgExpr::Feature("a".to_owned()),
        ]);
        assert_eq!(
            expr,
            CfgExpr::All(vec![
                CfgExpr::Feature("a".to_owned()),
                CfgExpr::Feature("b".to_owned()),
            ])
        );
    }

    #[test]
    fn feature_gate_parse() {
        let attr: Attribute = syn::parse_quote!(#[cfg(feature = "serde")]);
        let gate = attrs_to_gate(&[attr]);
        assert_eq!(gate, CfgGate::Known(CfgExpr::Feature("serde".to_owned())));
    }

    #[test]
    fn all_gate_parse() {
        let attr: Attribute = syn::parse_quote!(#[cfg(all(feature = "a", feature = "b"))]);
        let gate = attrs_to_gate(&[attr]);
        assert_eq!(
            gate,
            CfgGate::Known(CfgExpr::all(vec![
                CfgExpr::Feature("a".to_owned()),
                CfgExpr::Feature("b".to_owned()),
            ]))
        );
    }

    #[test]
    fn any_gate_parse() {
        let attr: Attribute = syn::parse_quote!(#[cfg(any(feature = "x", feature = "y"))]);
        let gate = attrs_to_gate(&[attr]);
        assert_eq!(
            gate,
            CfgGate::Known(CfgExpr::any(vec![
                CfgExpr::Feature("x".to_owned()),
                CfgExpr::Feature("y".to_owned()),
            ]))
        );
    }

    #[test]
    fn not_gate_parse() {
        let attr: Attribute = syn::parse_quote!(#[cfg(not(feature = "z"))]);
        let gate = attrs_to_gate(&[attr]);
        assert_eq!(
            gate,
            CfgGate::Known(CfgExpr::Not(Box::new(CfgExpr::Feature("z".to_owned()))))
        );
    }

    #[test]
    fn unknown_predicate_is_unknown() {
        let attr: Attribute = syn::parse_quote!(#[cfg(target_os = "linux")]);
        let gate = attrs_to_gate(&[attr]);
        assert!(
            matches!(gate, CfgGate::Unknown(_)),
            "expected Unknown, got {gate:?}"
        );
    }

    #[test]
    fn bare_unix_is_unknown() {
        let attr: Attribute = syn::parse_quote!(#[cfg(unix)]);
        let gate = attrs_to_gate(&[attr]);
        assert!(
            matches!(gate, CfgGate::Unknown(_)),
            "expected Unknown, got {gate:?}"
        );
    }

    #[test]
    fn multiple_cfg_attrs_are_anded() {
        let attr1: Attribute = syn::parse_quote!(#[cfg(feature = "a")]);
        let attr2: Attribute = syn::parse_quote!(#[cfg(feature = "b")]);
        let gate = attrs_to_gate(&[attr1, attr2]);
        assert_eq!(
            gate,
            CfgGate::Known(CfgExpr::all(vec![
                CfgExpr::Feature("a".to_owned()),
                CfgExpr::Feature("b".to_owned()),
            ]))
        );
    }

    #[test]
    fn no_cfg_attrs_is_always() {
        let attr: Attribute = syn::parse_quote!(#[doc = "just docs"]);
        let gate = attrs_to_gate(&[attr]);
        assert_eq!(gate, CfgGate::Always);
    }

    #[test]
    fn gate_and_combines_correctly() {
        let a = CfgGate::Known(CfgExpr::Feature("a".to_owned()));
        let b = CfgGate::Known(CfgExpr::Feature("b".to_owned()));
        let combined = a.and(b);
        assert_eq!(
            combined,
            CfgGate::Known(CfgExpr::all(vec![
                CfgExpr::Feature("a".to_owned()),
                CfgExpr::Feature("b".to_owned()),
            ]))
        );

        assert_eq!(
            CfgGate::Always.and(CfgGate::Known(CfgExpr::Feature("x".to_owned()))),
            CfgGate::Known(CfgExpr::Feature("x".to_owned()))
        );
    }

    #[test]
    fn evaluate_feature_enabled_disabled() {
        let mut defaults = BTreeMap::new();
        defaults.insert("on".to_owned(), CfgStatus::Enabled);
        defaults.insert("off".to_owned(), CfgStatus::Disabled);

        let gate_on = CfgGate::Known(CfgExpr::Feature("on".to_owned()));
        assert_eq!(gate_on.evaluate(&defaults), CfgStatus::Enabled);

        let gate_off = CfgGate::Known(CfgExpr::Feature("off".to_owned()));
        assert_eq!(gate_off.evaluate(&defaults), CfgStatus::Disabled);

        let gate_unknown_feat = CfgGate::Known(CfgExpr::Feature("unknown_feat".to_owned()));
        assert_eq!(
            gate_unknown_feat.evaluate(&defaults),
            CfgStatus::Disabled,
            "unknown features default to Disabled"
        );
    }

    #[test]
    fn feature_default_closure_transitive() {
        let features = vec![
            FeatureInventoryRecord {
                name: "default".to_owned(),
                enables: vec!["std".to_owned()],
            },
            FeatureInventoryRecord {
                name: "std".to_owned(),
                enables: vec!["alloc".to_owned(), "dep:some_dep".to_owned()],
            },
            FeatureInventoryRecord {
                name: "alloc".to_owned(),
                enables: vec![],
            },
            FeatureInventoryRecord {
                name: "serde".to_owned(),
                enables: vec![],
            },
        ];

        let closure = feature_default_closure(&features);
        assert!(closure.contains("std"));
        assert!(closure.contains("alloc"));
        assert!(!closure.contains("some_dep"), "dep: excluded");
        assert!(!closure.contains("serde"));
        assert!(!closure.contains("default"));
    }
}
