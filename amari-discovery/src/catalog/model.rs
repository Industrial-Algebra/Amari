// SPDX-License-Identifier: MIT OR Apache-2.0

//! Serializable structural, semantic, and probe catalog records.

use serde::{Deserialize, Serialize};

use crate::{CapabilityId, ProbeId};

// ============================================================================
// Structural catalog — generated deterministically from workspace sources
// ============================================================================

/// A generated structural snapshot of selected Amari crates.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StructuralCatalog {
    /// Structural catalog schema version.
    pub schema_version: u32,
    /// Amari release version represented by the snapshot.
    pub version: String,
    /// Human-readable scope note for the snapshot.
    pub description: String,
    /// Deterministic SHA-256 hash of the canonical JSON bytes (excluding the
    /// `content_hash` field itself). Set by the generator for drift detection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    /// Structurally indexed crate records.
    pub crates: Vec<CrateRecord>,
    /// Declarative probe descriptors from `catalog/probes.toml`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub probe_descriptors: Vec<ProbeDescriptor>,
    /// WASM surface summary referencing the checked-in `generated-wasm.json`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wasm_surface: Option<WasmSurfaceRef>,
    /// Cross-crate dependency edges keyed by `(from_crate, to_crate)`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependency_edges: Vec<DependencyEdgeRecord>,
}

/// A lightweight reference to the checked-in WASM surface snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WasmSurfaceRef {
    /// Repository-relative path to `generated-wasm.json`.
    pub path: String,
    /// SHA-256 hex of the source `.d.ts` content (from `WasmSurface::source_hash`).
    pub source_hash: String,
    /// Number of exported classes.
    pub class_count: usize,
    /// Number of exported top-level functions.
    pub function_count: usize,
    /// Number of exported enums.
    pub enum_count: usize,
    /// Number of exported interfaces.
    pub interface_count: usize,
    /// Number of exported type aliases.
    pub type_alias_count: usize,
    /// Sorted, deduplicated capability mappings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capability_mappings: Vec<WasmCapabilityMappingRef>,
}

/// A WASM capability mapping record for the summary.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct WasmCapabilityMappingRef {
    /// `Class.method` qualified WASM export path.
    pub wasm_path: String,
    /// Validated Amari capability ID.
    pub capability_id: String,
}

/// A cross-crate dependency edge in the workspace dependency graph.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct DependencyEdgeRecord {
    /// Source crate name.
    pub from: String,
    /// Target crate name.
    pub to: String,
}

// ============================================================================
// Crate record — enriched with typed structural passes
// ============================================================================

/// Structural metadata for one Amari crate.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CrateRecord {
    /// Cargo package name.
    pub name: String,
    /// Cargo package version.
    pub version: String,
    /// Cargo package description.
    pub description: String,
    /// SPDX license expression.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub license: String,
    /// Rust edition.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub edition: String,
    /// Workspace-relative manifest path.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub manifest_path: String,
    /// Declared library output kinds (e.g. `["lib"]`, `["cdylib"]`,
    /// `["proc-macro"]`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub library_outputs: Vec<String>,
    /// Declared Cargo features included in this snapshot.
    pub features: Vec<FeatureRecord>,
    /// Package dependency declarations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<DependencyRecord>,
    /// Library, binary, and example targets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub targets: Vec<TargetRecord>,
    /// Public API items included in this snapshot (one per unique exported path).
    pub items: Vec<ItemRecord>,
    /// Exported crate macros (declarative and proc-macro).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub macros: Vec<MacroCatalogRecord>,
    /// Trait definitions exported from this crate.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trait_definitions: Vec<TraitDefinitionRecord>,
    /// Trait implementations declared in this crate.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trait_implementations: Vec<TraitImplementationRecord>,
    /// Per-surface cfg gates, including top-level, module, and associated items.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cfg_gates: Vec<CfgGateRecord>,
    /// Checked-in examples included in this snapshot.
    pub examples: Vec<ExampleRecord>,
    /// Module graph summary (public module paths).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modules: Vec<String>,
    /// README path when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readme: Option<String>,
}

// ============================================================================
// Feature and dependency records
// ============================================================================

/// A declared Cargo feature.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FeatureRecord {
    /// Feature name within its crate.
    pub name: String,
    /// Feature edges (sorted, deduplicated).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enables: Vec<String>,
}

/// A resolved dependency declaration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DependencyRecord {
    /// Local dependency key used by Rust source.
    pub alias: String,
    /// Actual Cargo package name after `package = ...` renaming.
    pub package: String,
    /// Dependency table kind: `normal`, `build`, or `development`.
    pub kind: String,
    /// Optional target selector from `[target.'...']`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Resolved version requirement, when declared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Resolved manifest path text, when declared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Whether the dependency is optional.
    #[serde(default)]
    pub optional: bool,
    /// Whether Cargo default features are enabled.
    #[serde(default = "default_true")]
    pub default_features: bool,
    /// Explicit dependency features, sorted and deduplicated.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
}

fn default_true() -> bool {
    true
}

// ============================================================================
// Target records
// ============================================================================

/// A classified Cargo package target.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TargetRecord {
    /// Cargo target name.
    pub name: String,
    /// Target kind: `library`, `binary`, or `example`.
    pub kind: String,
    /// Manifest-relative source path.
    pub path: String,
    /// Features required to build the target.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_features: Vec<String>,
    /// Library crate types, empty for binary and example targets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub crate_types: Vec<String>,
}

// ============================================================================
// Item record — public API items
// ============================================================================

/// A public structural API item.
///
/// When exactly one source variant exists, the canonical summary fields
/// (`kind`, `signature`, `shape`, `source_path`, `source_module`,
/// `is_reexport`) duplicate the single variant's data for backward
/// compatibility. When multiple cfg/source variants exist for the same
/// exported path, every distinct variant is recorded in `variants` and
/// the canonical summary fields are omitted.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ItemRecord {
    /// Fully-qualified stable Rust path (package-qualified, `::` separated).
    pub path: String,
    /// Canonical summary: structural item kind (`struct`, `enum`, `fn`, etc.).
    /// Present only when a single variant exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Canonical summary: normalized source-level signature.
    /// Present only when a single variant exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    /// Canonical summary: public structural shape for aggregate types.
    /// Present only when a single variant exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<ItemShape>,
    /// Canonical summary: source file declaring this item.
    /// Present only when a single variant exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    /// Canonical summary: whether this is a re-export.
    /// Present only when a single variant exists.
    #[serde(default)]
    pub is_reexport: bool,
    /// Canonical summary: declaration source module path.
    /// Present only when a single variant exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_module: Option<String>,
    /// Every distinct source/cfg variant for this exported path.
    /// Always present. When len == 1, the canonical summary fields above
    /// duplicate the variant's data for backward compatibility.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<ItemVariantRecord>,
}

/// One source/cfg variant of an exported item at a given public path.
///
/// Each variant captures the kind, signature, shape, declaration source
/// identity, and re-export flag for one concrete declaration variant.
/// When mutually exclusive `#[cfg]` gates produce different
/// implementations for the same exported path, every variant is preserved.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ItemVariantRecord {
    /// Structural item kind.
    pub kind: String,
    /// Normalized source-level signature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    /// Public structural shape for aggregate types.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<ItemShape>,
    /// Workspace-relative source file declaring this variant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    /// Package-qualified declaration source module path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_module: Option<String>,
    /// Whether this variant is a re-export (export path differs from
    /// declaration source identity).
    #[serde(default)]
    pub is_reexport: bool,
    /// Canonical module path where the item is declared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declaration_module: Option<String>,
    /// Item identifier exactly as written in source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declaration_ident: Option<String>,
}

/// Public structural shape of an aggregate type.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ItemShape {
    /// Struct shape with its public fields.
    #[serde(rename = "struct")]
    Struct {
        /// Public fields in declaration order.
        fields: Vec<FieldRecord>,
    },
    /// Enum shape with its variants.
    #[serde(rename = "enum")]
    Enum {
        /// Variants in declaration order.
        variants: Vec<VariantRecord>,
    },
    /// Union shape with its public fields.
    #[serde(rename = "union")]
    Union {
        /// Public fields in declaration order.
        fields: Vec<FieldRecord>,
    },
}

/// A single public field of a struct or union.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FieldRecord {
    /// Field label (name or positional index).
    pub label: String,
    /// Normalized field type.
    pub ty: String,
}

/// A single enum variant.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct VariantRecord {
    /// Variant name.
    pub name: String,
    /// Variant field data.
    pub data: VariantDataRecord,
}

/// Fields of an enum variant.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum VariantDataRecord {
    /// A unit variant: `Quit`.
    #[serde(rename = "unit")]
    Unit,
    /// A tuple variant: `Write(String)`. Holds the normalized field types.
    #[serde(rename = "tuple")]
    Tuple {
        /// Normalized field types in declaration order.
        types: Vec<String>,
    },
    /// A struct variant: `Move { x: u8 }`.
    #[serde(rename = "struct")]
    Struct {
        /// Named fields in declaration order.
        fields: Vec<VariantFieldRecord>,
    },
}

/// A named field of a struct variant.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct VariantFieldRecord {
    /// Field name.
    pub name: String,
    /// Normalized field type.
    pub ty: String,
}

/// An associated item projected under an exported type or trait.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssociatedItemRecord {
    /// Item name.
    pub name: String,
    /// Item kind: `method`, `const`, or `type`.
    pub kind: String,
    /// Normalized declaration signature (no body).
    pub signature: String,
    /// Whether a trait item has a default definition.
    #[serde(default)]
    pub has_default: bool,
}

// ============================================================================
// Macro catalog records
// ============================================================================

/// One macro exported from a crate.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MacroCatalogRecord {
    /// Exported package-qualified path.
    pub path: String,
    /// Exported macro name.
    pub name: String,
    /// Macro kind: `declarative`, `proc_macro`, `proc_macro_attribute`,
    /// `proc_macro_derive`.
    pub kind: String,
    /// Package-relative source file.
    pub source_path: String,
    /// Normalized declaration signature.
    pub signature: String,
    /// For proc-macro-derive, helper attributes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub helpers: Vec<String>,
}

// ============================================================================
// Trait relationship records
// ============================================================================

/// A trait definition exported from a crate.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TraitDefinitionRecord {
    /// The public exported path under which this definition is reachable.
    pub export_path: String,
    /// Package-root-relative source path.
    pub source_path: String,
    /// Deterministic AST preorder ordinal.
    #[serde(default)]
    pub source_ordinal: usize,
    /// The trait endpoint.
    pub trait_endpoint: RelationshipEndpointRecord,
    /// Super trait constraints.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supertraits: Vec<SuperTraitConstraintRecord>,
    /// Items without default definitions (required).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_items: Vec<TraitItemRecord>,
    /// Items with default definitions (provided).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provided_items: Vec<TraitItemRecord>,
}

/// A trait implementation declared in a crate.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TraitImplementationRecord {
    /// Normalized trait path for this relationship.
    pub trait_path: String,
    /// Normalized type path for this relationship.
    pub impl_type_path: String,
    /// Package-root-relative source path.
    pub source_path: String,
    /// Deterministic AST preorder ordinal.
    #[serde(default)]
    pub source_ordinal: usize,
    /// The trait being implemented.
    pub trait_endpoint: RelationshipEndpointRecord,
    /// The type the trait is implemented for.
    pub impl_type_endpoint: RelationshipEndpointRecord,
    /// Normalized generic parameters clause.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub generics: String,
    /// Whether the impl block is marked `unsafe`.
    #[serde(default)]
    pub unsafe_trait: bool,
    /// Whether this is a negative impl (`impl !Trait for Type`).
    #[serde(default)]
    pub negative: bool,
    /// Whether this implementation was introduced by a `#[derive(...)]``.
    #[serde(default)]
    pub is_derived: bool,
}

/// An endpoint in a trait relationship.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum RelationshipEndpointRecord {
    /// A local item whose source identity is known.
    #[serde(rename = "local")]
    Local {
        /// Canonical module path, e.g. `crate::algebra::ga`.
        module: String,
        /// Item identifier exactly as written in source.
        ident: String,
    },
    /// An external or unresolved reference.
    #[serde(rename = "external")]
    External {
        /// The unresolved path as written in source.
        path: String,
    },
}

/// A supertrait constraint on a trait definition.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SuperTraitConstraintRecord {
    /// The supertrait endpoint.
    pub endpoint: RelationshipEndpointRecord,
}

/// A single associated item of a trait definition.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TraitItemRecord {
    /// Item name.
    pub name: String,
    /// Item kind: `method`, `const`, or `type`.
    pub kind: String,
    /// Normalized declaration signature (no body).
    pub signature: String,
    /// Whether this item is required or has a default.
    pub status: String,
}

// ============================================================================
// cfg gate records
// ============================================================================

/// A cfg gate attached to a single exported surface.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CfgGateRecord {
    /// Exported path, e.g. `crate::module::Name`.
    pub path: String,
    /// Package-root-relative source path, e.g. `src/lib.rs`.
    pub source_path: String,
    /// Deterministic AST preorder ordinal.
    #[serde(default)]
    pub source_ordinal: usize,
    /// The consolidated cfg gate as a display string.
    pub gate: String,
    /// Evaluation of the gate under the package's local default features:
    /// `enabled`, `disabled`, or `unknown`.
    pub status: String,
    /// What kind of surface this record represents.
    pub surface_kind: String,
}

// ============================================================================
// Example records
// ============================================================================

/// A checked-in crate example.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExampleRecord {
    /// Cargo example target name.
    pub name: String,
    /// Workspace-relative source path.
    pub path: String,
    /// Features required to build this example.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_features: Vec<String>,
}

// ============================================================================
// Semantic catalog
// ============================================================================

/// Curated semantic capability and relationship records.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SemanticCatalog {
    /// Amari release version targeted by the overlay.
    pub catalog_version: String,
    /// Curated capabilities.
    pub capabilities: Vec<CapabilityRecord>,
    /// Directed semantic relationships.
    pub relations: Vec<CapabilityRelation>,
}

/// A curated Amari capability.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CapabilityRecord {
    /// Stable machine capability ID.
    pub id: CapabilityId,
    /// Concise display name.
    pub name: String,
    /// Human-readable purpose and problem shape.
    pub description: String,
    /// Alternative names used in search and inspection.
    pub aliases: Vec<String>,
    /// Mathematical and software concepts associated with the capability.
    pub concepts: Vec<String>,
    /// Referenced structural crate names.
    pub crate_refs: Vec<String>,
    /// Referenced features in `<crate>:<feature>` form.
    pub feature_refs: Vec<String>,
    /// Referenced fully-qualified structural item paths.
    pub symbol_refs: Vec<String>,
    /// Referenced examples in `<crate>:<example>` form.
    pub example_refs: Vec<String>,
    /// Known bounded probes relevant to this capability.
    pub probe_refs: Vec<ProbeId>,
    /// API stability tier.
    pub stability: StabilityTier,
    /// Expected relative runtime or integration cost.
    pub cost: CostHint,
}

/// A directed relationship between two curated capabilities.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CapabilityRelation {
    /// Source capability.
    pub from: CapabilityId,
    /// Target capability.
    pub to: CapabilityId,
    /// Stable relationship kind such as `composes_with` or `alternative_to`.
    pub kind: String,
}

/// Stability tier for a curated capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StabilityTier {
    /// Stable public API suitable for production use.
    Stable,
    /// Public API that may evolve during the current release series.
    Experimental,
    /// Research-facing capability with intentionally limited guarantees.
    Research,
}

/// Relative execution or integration cost.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CostHint {
    /// Small bounded CPU or integration cost.
    Low,
    /// Moderate bounded CPU or integration cost.
    Moderate,
    /// High cost that warrants explicit planning.
    High,
}

// ============================================================================
// Probe manifest
// ============================================================================

/// Declarative manifest of known probes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProbeManifest {
    /// Amari release version targeted by the manifest.
    pub catalog_version: String,
    /// Known probe descriptors, whether executable or not.
    pub probes: Vec<ProbeDescriptor>,
}

/// Contract for a known bounded Amari probe.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProbeDescriptor {
    /// Stable versioned probe ID.
    pub id: ProbeId,
    /// Capability validated or demonstrated by the probe.
    pub capability_id: CapabilityId,
    /// Versioned request schema ID.
    pub input_schema: String,
    /// Versioned response schema ID.
    pub output_schema: String,
    /// `amari-discovery` features required by a future adapter.
    pub required_features: Vec<String>,
    /// Relative probe cost.
    pub cost: CostHint,
    /// Whether identical validated inputs produce identical mathematical output.
    pub deterministic: bool,
    /// Declared side-effect authority.
    pub side_effects: SideEffectPolicy,
    /// Hard and cooperative resource ceilings.
    pub limits: ProbeLimits,
}

/// Side-effect authority granted to a probe.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SideEffectPolicy {
    /// Pure computation with no side effects.
    None,
    /// Bounded read-only access to validated project evidence.
    ReadOnly,
}

/// Resource ceilings declared by a probe descriptor.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProbeLimits {
    /// Maximum canonical request bytes.
    pub max_input_bytes: u64,
    /// Maximum canonical response bytes.
    pub max_output_bytes: u64,
    /// Maximum domain operations.
    pub max_operations: u64,
    /// Wall-clock timeout in milliseconds for isolated CLI execution.
    pub timeout_millis: u64,
}
