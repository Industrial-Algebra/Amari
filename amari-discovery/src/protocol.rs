// SPDX-License-Identifier: MIT OR Apache-2.0

//! Stable identifiers and machine-readable discovery protocol types.

use std::{fmt, str::FromStr};

use serde::{de::Error as _, Deserialize, Deserializer, Serialize};

use crate::{DiscoveryError, ProjectSnapshot};

fn is_canonical_id_segment(segment: &str) -> bool {
    let bytes = segment.as_bytes();
    let is_alphanumeric = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();

    !bytes.is_empty()
        && is_alphanumeric(bytes[0])
        && is_alphanumeric(bytes[bytes.len() - 1])
        && bytes
            .iter()
            .all(|byte| is_alphanumeric(*byte) || matches!(*byte, b'-' | b'_'))
}

/// The stable schema identifier for the first discovery protocol.
pub const SCHEMA_V1: &str = "amari.discovery/v1";

/// A supported discovery protocol schema version.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SchemaVersion {
    /// The `amari.discovery/v1` protocol.
    #[serde(rename = "amari.discovery/v1")]
    V1,
}

impl SchemaVersion {
    /// Returns the stable string representation of this schema version.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::V1 => SCHEMA_V1,
        }
    }
}

/// A stable identifier for an Amari capability.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize)]
#[serde(transparent)]
pub struct CapabilityId(String);

impl FromStr for CapabilityId {
    type Err = DiscoveryError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let segments: Vec<_> = value.split(':').collect();
        if segments.len() < 4 || segments.first() != Some(&"amari") {
            return Err(DiscoveryError::invalid_id(
                value,
                "capability IDs require `amari:<crate>:<module>:<symbol>`",
            ));
        }
        if !segments
            .iter()
            .all(|segment| is_canonical_id_segment(segment))
        {
            return Err(DiscoveryError::invalid_id(
                value,
                "capability ID segments must use lowercase ASCII letters, digits, hyphens, or underscores and begin and end with a letter or digit",
            ));
        }
        Ok(Self(value.to_owned()))
    }
}

impl<'de> Deserialize<'de> for CapabilityId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

impl fmt::Display for CapabilityId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A stable identifier for a bounded capability probe.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize)]
#[serde(transparent)]
pub struct ProbeId(String);

impl FromStr for ProbeId {
    type Err = DiscoveryError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let segments: Vec<_> = value.split(':').collect();
        if segments.len() != 4 || segments.first() != Some(&"amari-probe") {
            return Err(DiscoveryError::invalid_id(
                value,
                "probe IDs require `amari-probe:<domain>:<operation>:vN`",
            ));
        }
        if !segments[1..=2]
            .iter()
            .all(|segment| is_canonical_id_segment(segment))
        {
            return Err(DiscoveryError::invalid_id(
                value,
                "probe domain and operation must use canonical lowercase ASCII segments",
            ));
        }
        let version_is_canonical = segments[3].strip_prefix('v').is_some_and(|number| {
            !number.is_empty()
                && !number.starts_with('0')
                && number.bytes().all(|byte| byte.is_ascii_digit())
        });
        if !version_is_canonical {
            return Err(DiscoveryError::invalid_id(
                value,
                "probe version must be a positive `vN` integer",
            ));
        }
        Ok(Self(value.to_owned()))
    }
}

impl<'de> Deserialize<'de> for ProbeId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

impl fmt::Display for ProbeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// The catalog version and content hash used to produce a response.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CatalogIdentity {
    /// The Amari catalog version.
    pub version: String,
    /// The deterministic catalog content hash.
    pub hash: String,
}

/// Compatibility status and the evidence supporting it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Compatibility {
    /// A stable compatibility status such as `compatible` or `unknown_version`.
    pub status: String,
    /// Human-readable reasons for the status.
    pub reasons: Vec<String>,
}

/// Requirements for replaying a discovery response.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReplayMetadata {
    /// Whether the response contains sufficient provenance for replay.
    pub replayable: bool,
    /// Hash fields that must match before replay.
    pub required_hashes: Vec<String>,
    /// Reasons replay is unavailable or constrained.
    pub reasons: Vec<String>,
}

/// Provenance shared by every discovery response.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Provenance {
    /// The `amari-discovery` package version.
    pub tool_version: String,
    /// The catalog used to produce the response.
    pub catalog: CatalogIdentity,
    /// Compatibility between inspected inputs and the catalog.
    pub compatibility: Compatibility,
    /// Replay requirements for the response.
    pub replay: ReplayMetadata,
    /// Hash of the inspected project, when project context exists.
    pub project_hash: Option<String>,
    /// Hash of explicit command input, when applicable.
    pub input_hash: Option<String>,
    /// Deterministic seed, when the operation uses one.
    pub seed: Option<u64>,
}

/// A versioned discovery response envelope.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Envelope<T> {
    /// The machine protocol schema version.
    pub schema_version: String,
    /// Tool, catalog, compatibility, and replay provenance.
    pub provenance: Provenance,
    /// Non-fatal warnings accumulated while producing the response.
    pub warnings: Vec<String>,
    /// The typed response payload.
    pub data: T,
}

impl<T> Envelope<T> {
    /// Creates an envelope with explicit catalog, compatibility, and replay data.
    pub fn new(
        data: T,
        catalog: CatalogIdentity,
        compatibility: Compatibility,
        replay: ReplayMetadata,
    ) -> Self {
        Self {
            schema_version: SCHEMA_V1.to_owned(),
            provenance: Provenance {
                tool_version: env!("CARGO_PKG_VERSION").to_owned(),
                catalog,
                compatibility,
                replay,
                project_hash: None,
                input_hash: None,
                seed: None,
            },
            warnings: Vec::new(),
            data,
        }
    }
}

/// The execution backend used by a probe.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeBackend {
    /// Deterministic CPU execution.
    Cpu,
}

/// Resource usage observed during a bounded probe.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ResourceObservations {
    /// Domain operations performed.
    pub operations: u64,
    /// Graph or term nodes visited.
    pub nodes: u64,
    /// Iterative steps performed.
    pub iterations: u64,
    /// Input and output bytes accounted by the probe.
    pub bytes: u64,
}

/// A typed result returned by a registered bounded probe.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProbeResult {
    /// The stable probe identifier.
    pub probe_id: ProbeId,
    /// The backend that performed the calculation.
    pub backend: ProbeBackend,
    /// Elapsed execution time in microseconds.
    pub duration_micros: u64,
    /// Typed resource observations.
    pub resources: ResourceObservations,
    /// Deterministic seed, when the probe uses one.
    pub seed: Option<u64>,
    /// Inspected project hash, when project context exists.
    pub project_hash: Option<String>,
    /// Catalog hash used to select and validate the probe.
    pub catalog_hash: String,
    /// Canonical input hash.
    pub input_hash: String,
    /// Assumptions supported by the result.
    pub validated_assumptions: Vec<String>,
    /// Assumptions contradicted by the result.
    pub refuted_assumptions: Vec<String>,
    /// Non-fatal probe warnings.
    pub warnings: Vec<String>,
    /// Probe-specific JSON output.
    pub output: serde_json::Value,
}

/// A discrete piece of evidence used by discovery and planning.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    /// Stable evidence category.
    pub kind: String,
    /// Concise human-readable description.
    pub summary: String,
    /// Optional source path or catalog record.
    pub source: Option<String>,
    /// Relative evidence weight used by ranking.
    pub weight: f64,
}

/// A normalized user goal supplied to the discovery planner.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GoalSpec {
    /// Concise statement of the mathematical or integration goal.
    pub statement: String,
    /// Explicit deterministic constraints applied to the goal.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<String>,
}

/// Typed project, goal, and saved-probe inputs used to construct plans.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanningContext {
    /// Sanitized read-only project snapshot.
    pub snapshot: ProjectSnapshot,
    /// Explicit planner goal.
    pub goal: GoalSpec,
    /// Saved bounded probe results considered during planning.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub probe_results: Vec<ProbeResult>,
}

/// Replay hash for one saved probe result consumed by planning.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct ProbeReplayHash {
    /// Registered probe identifier.
    pub probe_id: ProbeId,
    /// Canonical probe input hash.
    pub input_hash: String,
    /// SHA-256 hash of the serialized saved result.
    pub result_hash: String,
}

/// Provenance that must match before a plan can be replayed.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PlanCompatibility {
    /// Catalog version and content hash used to construct the plan.
    pub catalog: CatalogIdentity,
    /// Inspected project hash used to construct the plan.
    pub project_hash: String,
    /// Canonical hash of the goal and saved probe inputs.
    pub input_hash: String,
    /// Canonical saved-probe result hashes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub probe_results: Vec<ProbeReplayHash>,
}

/// Static test scope emitted by a plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanTestTarget {
    /// Run every target belonging to the selected package.
    AllTargets,
}

/// One read-only integration instruction in a candidate plan.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlanStep {
    /// Add an exact Amari package dependency.
    Dependency {
        /// Capability requiring this step.
        capability_id: CapabilityId,
        /// Cargo package name.
        package: String,
        /// Exact catalog package version.
        version: String,
    },
    /// Enable one exact package feature.
    Feature {
        /// Capability requiring this step.
        capability_id: CapabilityId,
        /// Cargo package name.
        package: String,
        /// Cargo feature name.
        feature: String,
    },
    /// Integrate one exact public symbol.
    Symbol {
        /// Capability requiring this step.
        capability_id: CapabilityId,
        /// Fully qualified public symbol path.
        path: String,
    },
    /// Consult or reproduce one checked-in example.
    Example {
        /// Capability requiring this step.
        capability_id: CapabilityId,
        /// Cargo package name.
        package: String,
        /// Cargo example target name.
        example: String,
    },
    /// Run one registered bounded probe.
    Probe {
        /// Capability requiring this step.
        capability_id: CapabilityId,
        /// Registered probe identifier.
        probe_id: ProbeId,
    },
    /// Verify one exact package test scope.
    Test {
        /// Capability requiring this step.
        capability_id: CapabilityId,
        /// Cargo package name.
        package: String,
        /// Static package test scope.
        target: PlanTestTarget,
    },
}

impl PlanStep {
    /// Returns the capability that requires this integration step.
    pub const fn capability_id(&self) -> &CapabilityId {
        match self {
            Self::Dependency { capability_id, .. }
            | Self::Feature { capability_id, .. }
            | Self::Symbol { capability_id, .. }
            | Self::Example { capability_id, .. }
            | Self::Probe { capability_id, .. }
            | Self::Test { capability_id, .. } => capability_id,
        }
    }
}

/// One deterministic rewrite applied during plan normalization.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NormalizationTrace {
    /// Plan steps immediately before the rewrite.
    pub before: Vec<PlanStep>,
    /// Plan steps immediately after the rewrite.
    pub after: Vec<PlanStep>,
}

/// Completion and trace metadata for plan normalization.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PlanNormalization {
    /// Whether the plan reached a rewrite fixed point within its limits.
    pub normalized: bool,
    /// Maximum rewrites allowed for this normalization attempt.
    pub max_rewrites: usize,
    /// Applied rewrites in deterministic order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trace: Vec<NormalizationTrace>,
}

/// A deterministic replayable plan for one ranked capability path.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CandidatePlan {
    /// Preferred catalog capability implemented by this plan.
    pub capability_id: CapabilityId,
    /// Canonical prerequisite-first capability order.
    pub prerequisite_order: Vec<CapabilityId>,
    /// Canonical deduplicated integration instructions.
    pub steps: Vec<PlanStep>,
    /// Required replay provenance.
    pub compatibility: PlanCompatibility,
    /// Bounded rewrite-normalization metadata.
    pub normalization: PlanNormalization,
    /// SHA-256 hash of canonical plan content excluding trace metadata.
    pub plan_hash: String,
}

/// A preferred replayable plan and its Pareto alternatives.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Recommendation {
    /// Goal used to generate the recommendation.
    pub goal: GoalSpec,
    /// Deterministically preferred plan.
    pub preferred: CandidatePlan,
    /// Other retained Pareto plans.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<CandidatePlan>,
    /// Evidence shared by the recommendation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<Evidence>,
    /// Non-fatal planner warnings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

/// A successful typed domain outcome.
///
/// Non-recommendation variants communicate expected domain conditions and do
/// not imply a nonzero process exit code.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", content = "data", rename_all = "snake_case")]
pub enum DiscoveryOutcome<T> {
    /// A capability or plan can be recommended.
    Recommended(T),
    /// Available evidence rules out all catalog capabilities.
    NoApplicableCapability {
        /// Evidence supporting the outcome.
        evidence: Vec<Evidence>,
    },
    /// More project or goal evidence is required.
    InsufficientEvidence {
        /// Missing evidence descriptions.
        missing: Vec<String>,
    },
    /// Preconditions explicitly prevent a recommendation.
    Blocked {
        /// Blocking conditions.
        reasons: Vec<String>,
    },
}
