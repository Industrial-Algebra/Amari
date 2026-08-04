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
//! - `ai`: compiles the provider-neutral AI validation contract. It does not
//!   enable network access, an external provider transport, or execution authority.

#![deny(missing_docs)]

#[cfg(feature = "ai")]
pub mod ai;
pub mod capabilities;
pub mod catalog;
pub mod cli;
pub mod commands;
pub mod error;
pub mod inspect;
pub mod ndjson;
pub mod planner;
mod probes;
pub mod protocol;
mod render;
pub mod schema;
mod shell;
pub mod wire;

#[cfg(feature = "ai")]
pub use ai::{
    AiContractLimits, AiExecutionRequest, GoalInterpretation, GoalInterpretationRequest,
    GoalInterpreter, ValidatedGoalInterpreter,
};
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
pub use commands::probe::{
    ProbeDescription, ProbeDryRun, ProbeList, ProbeListItem, ProbeRunResult, ProbeSchemaResolution,
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
pub use ndjson::{NdjsonWriter, DEFAULT_MAX_NDJSON_RECORD_BYTES};
pub use planner::{
    BlockedCandidate, CandidateRanker, CandidateRetriever, CapabilityGraphExpander,
    GraphConstraints, GraphExpansion, GraphExpansionState, GraphLimit, GraphLimits, GraphPath,
    GraphStep, NormalizationLimits, PlanGenerator, PlanNormalizer, RankedCandidate,
    RankingComponents, RankingContext, RankingProvenance, RankingResult, RankingSignal,
    RankingSignalKind, RecallConfig, RelationCostPolicy, RetrievalSource, RetrievedCandidate,
    RANKING_OBJECTIVE_ORDER,
};
pub use probes::{
    CgtNimSumOutput, CgtNimSumRequest, Cl3ProductOutput, Cl3ProductRequest, DecimalRational,
    DecimalSurcomplex, HolographicAttribution, HolographicCapacity, HolographicEntry,
    HolographicRecallOutput, HolographicRecallRequest, HolographicSuperpositionOutput,
    HolographicSuperpositionRequest, NetworkPath, NetworkShortestPathOutput,
    NetworkShortestPathRequest, ObjectiveDirection, ParetoFrontOutput, ParetoFrontRequest,
    ParetoPoint, PolynomialDerivativeOutput, PolynomialDerivativeRequest, ProbeEngine,
    ProbeEngineLimits, ProbeExecution, ProbeIsolation, RationalSurcomplexDivisionOutput,
    RationalSurcomplexDivisionRequest, RationalSurrealArithmeticOutput,
    RationalSurrealArithmeticRequest, RewriteExample, RewriteInferRuleOutput,
    RewriteInferRuleRequest, RewriteNormalizeOutput, RewriteNormalizeRequest,
    RewritePredecessorsOutput, RewritePredecessorsRequest, RewriteRule, RewriteTerm,
    TropicalViterbiOutput, TropicalViterbiRequest,
};
pub use protocol::{
    CandidatePlan, CapabilityId, CatalogIdentity, Compatibility, DiscoveryOutcome, Envelope,
    Evidence, GoalSpec, NormalizationTrace, PlanCompatibility, PlanNormalization, PlanStep,
    PlanTestTarget, PlanningContext, ProbeBackend, ProbeId, ProbeReplayHash, ProbeResult,
    Provenance, Recommendation, RecommendationScore, RecommendationScoreComponents, ReplayMetadata,
    ResourceObservations, SchemaVersion, SCHEMA_V1,
};
pub use schema::{
    protocol_schema, protocol_schema_catalog, ProtocolSchema, SchemaCatalog, SchemaKind,
    SchemaSummary,
};
pub use wire::{
    ProbeSchemaBinding, ProbeSchemaContractState, ProbeSchemaDocument, ProbeSchemaRegistration,
    ProbeSchemaSummary, ProbeWireSchemaRegistry, WireCompatibility, WireContract, WireExample,
    WireSchemaRole, WireSemanticConstraint,
};
