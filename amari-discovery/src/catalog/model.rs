// SPDX-License-Identifier: MIT OR Apache-2.0

//! Serializable structural, semantic, and probe catalog records.

use serde::{Deserialize, Serialize};

use crate::{CapabilityId, ProbeId};

/// A generated structural snapshot of selected Amari crates.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StructuralCatalog {
    /// Structural catalog schema version.
    pub schema_version: u32,
    /// Amari release version represented by the snapshot.
    pub version: String,
    /// Human-readable scope note for the snapshot.
    pub description: String,
    /// Structurally indexed crate records.
    pub crates: Vec<CrateRecord>,
}

/// Structural metadata for one Amari crate.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CrateRecord {
    /// Cargo package name.
    pub name: String,
    /// Cargo package version.
    pub version: String,
    /// Cargo package description.
    pub description: String,
    /// Declared Cargo features included in this snapshot.
    pub features: Vec<FeatureRecord>,
    /// Public API items included in this snapshot.
    pub items: Vec<ItemRecord>,
    /// Checked-in examples included in this snapshot.
    pub examples: Vec<ExampleRecord>,
}

/// A declared Cargo feature.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FeatureRecord {
    /// Feature name within its crate.
    pub name: String,
}

/// A public structural API item.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ItemRecord {
    /// Fully-qualified stable Rust path.
    pub path: String,
    /// Structural item kind such as `struct`, `method`, or `function`.
    pub kind: String,
    /// Source-level signature when captured by the generator.
    pub signature: Option<String>,
}

/// A checked-in crate example.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExampleRecord {
    /// Cargo example target name.
    pub name: String,
    /// Repository-relative source path.
    pub path: String,
}

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
