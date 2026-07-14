// SPDX-License-Identifier: MIT OR Apache-2.0

//! Agent-first discovery and planning for the Amari mathematical ecosystem.
//!
//! `amari-discovery` provides the typed engine behind the `amari` command.
//! Its runtime authority is read-only: it may inspect projects, recommend
//! capabilities, construct plans, and run registered bounded probes.
//!
//! ## Features
//!
//! - `standard-probes` (default): compiles the standard dual-number, game,
//!   surreal, and surcomplex probe dependencies. Probe execution is added in
//!   later discovery runtime phases.
//! - `ai`: reserves the provider-neutral AI adapter contract. It does not
//!   enable network access or an external provider transport.

#![deny(missing_docs)]

pub mod capabilities;
pub mod catalog;
pub mod cli;
pub mod commands;
pub mod error;
pub mod protocol;
mod render;

pub use capabilities::{
    AiAdapterStatus, Capabilities, CatalogStatus, FeatureGate, PlatformInfo, ResourceLimits,
    RuntimeCapabilityState,
};
pub use catalog::generator::wasm::{
    default_capability_mappings, parse_wasm_surface, validate_capability_mappings,
    WasmCapabilityMapping, WasmClass, WasmEnum, WasmEnumVariant, WasmFunction, WasmGetter,
    WasmInterface, WasmInterfaceMember, WasmMethod, WasmSurface, WasmSurfaceWarning, WasmTypeAlias,
};
pub use catalog::generator::{generate_workspace_catalog, verify_checked_in};
pub use catalog::{
    AssociatedItemRecord, CapabilityRecord, CapabilityRelation, Catalog, CfgGateRecord, CostHint,
    CrateRecord, DependencyEdgeRecord, DependencyRecord, ExampleRecord, FeatureRecord, FieldRecord,
    ItemRecord, ItemShape, ItemVariantRecord, MacroCatalogRecord, ProbeDescriptor, ProbeLimits,
    ProbeManifest, RelationshipEndpointRecord, SemanticCatalog, SideEffectPolicy, StabilityTier,
    StructuralCatalog, SuperTraitConstraintRecord, TargetRecord, TraitDefinitionRecord,
    TraitImplementationRecord, TraitItemRecord, VariantDataRecord, VariantFieldRecord,
    VariantRecord, WasmCapabilityMappingRef, WasmSurfaceRef,
};
pub use commands::discover::{
    DiscoveredExample, ExampleResult, GraphRelationItem, GraphResult, SearchResultItem,
    SearchResults,
};
pub use error::{DiscoveryError, DiscoveryResult};
pub use protocol::{
    CapabilityId, CatalogIdentity, Compatibility, DiscoveryOutcome, Envelope, Evidence,
    ProbeBackend, ProbeId, ProbeResult, Provenance, ReplayMetadata, ResourceObservations,
    SchemaVersion, SCHEMA_V1,
};
