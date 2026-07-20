// SPDX-License-Identifier: MIT OR Apache-2.0

//! Agent-first discovery and planning for the Amari mathematical ecosystem.
//!
//! `amari-discovery` provides the typed engine behind the `amari` command.
//! Its runtime authority is read-only: it may inspect projects, recommend
//! capabilities, construct plans, and run registered bounded probes.
//!
//! ## Features
//!
//! - `standard-probes` (default): compiles registered deterministic probe
//!   adapters, beginning with bounded tropical Viterbi decoding.
//! - `ai`: reserves the provider-neutral AI adapter contract. It does not
//!   enable network access or an external provider transport.

#![deny(missing_docs)]

pub mod capabilities;
pub mod catalog;
pub mod cli;
pub mod commands;
pub mod error;
pub mod inspect;
pub mod planner;
mod probes;
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
pub use inspect::{
    inspect_cargo_platform, inspect_cargo_project, inspect_npm_project,
    inspect_npm_typescript_project, inspect_project, inspect_project_envelope,
    inspect_rust_project, inspect_rust_sources, inspect_typescript_sources,
    AmariDependencyEvidence, BenchmarkEvidence, BenchmarkStatus, CargoBench, CargoBuildSettings,
    CargoDependencyRecord, CargoInspection, CargoInspectionWarning, CargoLock, CargoPackage,
    CargoPlatformInspection, CargoPlatformWarning, CargoTargetKey, CargoTargetSettings,
    ConfigInputProvenance, ConfigSetting, ConfigSettingIssue, ConfigSource, ConfiguredLinker,
    ConfiguredRunner, CustomTargetEvidence, DependencyKind, InspectionLimit, InspectionLimits,
    LockedPackage, ManifestSource, NativeLink, NativeRequirement, NoStdEvidence,
    NoStdPackageEvidence, NpmDependencyEvidence, NpmDependencyKind, NpmInspection,
    NpmInspectionWarning, NpmLock, NpmLockedPackage, NpmPackage, NpmSource, ProjectInspector,
    ProjectKind, ProjectSignal, ProjectSnapshot, RustCfgEvidence, RustCrateAttribute, RustFileKind,
    RustInspectionWarning, RustSourceInspection, RustUsage, RustUsageKind, RustflagCategory,
    RustflagCategoryCount, RustflagsEvidence, RustflagsScope, SnapshotState, SourceLocation,
    SystemDependencyKind, SystemDependencySignal, TargetCfgConstraint, TargetCfgSource,
    TypeScriptCapabilityEvidence, TypeScriptDeclarationExport, TypeScriptExportKind,
    TypeScriptFileContext, TypeScriptFileRole, TypeScriptImport, TypeScriptImportKind,
    TypeScriptInspection, TypeScriptInspectionWarning, TypeScriptRuntimeEvidence,
    TypeScriptRuntimeSignal, TypeScriptVocabularyEvidence, VocabularyEvidence, WasmTargetEvidence,
    WasmTargetOrigin, WorkspaceDependencyBase, WorkspaceMeta,
};
pub use planner::{
    BlockedCandidate, CandidateRanker, CandidateRetriever, CapabilityGraphExpander,
    GraphConstraints, GraphExpansion, GraphExpansionState, GraphLimit, GraphLimits, GraphPath,
    GraphStep, NormalizationLimits, PlanGenerator, PlanNormalizer, RankedCandidate,
    RankingComponents, RankingContext, RankingProvenance, RankingResult, RankingSignal,
    RankingSignalKind, RecallConfig, RelationCostPolicy, RetrievalSource, RetrievedCandidate,
    RANKING_OBJECTIVE_ORDER,
};
pub use probes::{
    Cl3ProductOutput, Cl3ProductRequest, NetworkPath, NetworkShortestPathOutput,
    NetworkShortestPathRequest, ObjectiveDirection, ParetoFrontOutput, ParetoFrontRequest,
    ParetoPoint, PolynomialDerivativeOutput, PolynomialDerivativeRequest, ProbeEngine,
    ProbeEngineLimits, ProbeExecution, ProbeIsolation, TropicalViterbiOutput,
    TropicalViterbiRequest,
};
pub use protocol::{
    CandidatePlan, CapabilityId, CatalogIdentity, Compatibility, DiscoveryOutcome, Envelope,
    Evidence, GoalSpec, NormalizationTrace, PlanCompatibility, PlanNormalization, PlanStep,
    PlanTestTarget, PlanningContext, ProbeBackend, ProbeId, ProbeReplayHash, ProbeResult,
    Provenance, Recommendation, RecommendationScore, RecommendationScoreComponents, ReplayMetadata,
    ResourceObservations, SchemaVersion, SCHEMA_V1,
};
