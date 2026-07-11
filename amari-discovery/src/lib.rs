// SPDX-License-Identifier: MIT OR Apache-2.0

//! Agent-first discovery and planning for the Amari mathematical ecosystem.
//!
//! `amari-discovery` provides the typed engine behind the `amari` command.
//! Its runtime authority is read-only: it may inspect projects, recommend
//! capabilities, construct plans, and run registered bounded probes.

#![deny(missing_docs)]

pub mod capabilities;
pub mod catalog;
pub mod cli;
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
pub use catalog::{
    CapabilityRecord, CapabilityRelation, Catalog, CostHint, CrateRecord, ExampleRecord,
    FeatureRecord, ItemRecord, ProbeDescriptor, ProbeLimits, ProbeManifest, SemanticCatalog,
    SideEffectPolicy, StabilityTier, StructuralCatalog,
};
pub use error::{DiscoveryError, DiscoveryResult};
pub use protocol::{
    CapabilityId, CatalogIdentity, Compatibility, DiscoveryOutcome, Envelope, Evidence,
    ProbeBackend, ProbeId, ProbeResult, Provenance, ReplayMetadata, ResourceObservations,
    SchemaVersion, SCHEMA_V1,
};
