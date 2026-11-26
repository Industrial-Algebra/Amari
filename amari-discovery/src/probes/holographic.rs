// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded holographic MAP256 probe adapters.

use serde::{Deserialize, Serialize};

#[cfg(feature = "standard-probes")]
use amari_holographic::{algebra::map::MAP256, AlgebraConfig, BindingAlgebra, HolographicMemory};
#[cfg(feature = "standard-probes")]
use serde_json::Value;

#[cfg(feature = "standard-probes")]
use super::registry::{AdapterOutput, AdapterRegistration, EffectiveProbeLimits};
#[cfg(feature = "standard-probes")]
use crate::{DiscoveryError, DiscoveryResult, ProbeLimits, ResourceObservations, SideEffectPolicy};

#[cfg(feature = "standard-probes")]
const MAP_DIMENSION: u64 = 256;
#[cfg(feature = "standard-probes")]
const MAX_SUPERPOSITION_SEEDS: u64 = 256;
#[cfg(feature = "standard-probes")]
const MAX_RECALL_ENTRIES: u64 = 32;
#[cfg(feature = "standard-probes")]
const RECALL_PASSES_PER_ENTRY: u64 = 11;
#[cfg(feature = "standard-probes")]
// v2 validates the query separately for raw recall, exact membership and nearest score.
const RECALL_FIXED_PASSES: u64 = 18;

/// Typed request for deterministic additive MAP256 superposition.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[serde(deny_unknown_fields)]
#[wire_contract(
    id = "amari.discovery/probe/holographic-superposition/input/v1",
    role = "input",
    compatibility = "additive_patch",
    constraints(
        integer_seeds = "every seed is an unsigned 64-bit integer",
        nonempty_seeds = "at least one seed is required",
        seed_count_limit = "at most 256 seeds are accepted"
    ),
    example(label = "two_seeds", value = "{\"seeds\":[1,2]}")
)]
pub struct HolographicSuperpositionRequest {
    /// Seeds converted deterministically with `MAP256::from_seed`.
    pub seeds: Vec<u64>,
}

/// Typed output from additive MAP256 superposition.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[wire_contract(
    id = "amari.discovery/probe/holographic-superposition/output/v1",
    role = "output",
    compatibility = "additive_patch",
    constraints(
        finite_coefficients = "every returned MAP coefficient is finite",
        map256_dimension = "the coefficient vector contains exactly 256 MAP components"
    )
)]
pub struct HolographicSuperpositionOutput {
    /// Additive trace coefficients in MAP component order.
    pub coefficients: Vec<f64>,
}

/// One deterministic key-value entry for holographic memory.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HolographicEntry {
    /// Seed for the MAP256 key.
    pub key_seed: u64,
    /// Seed for the MAP256 value.
    pub value_seed: u64,
}

/// Typed request for MAP256 associative-memory retrieval.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[serde(deny_unknown_fields)]
#[wire_contract(
    id = "amari.discovery/probe/holographic-recall/input/v2",
    role = "input",
    compatibility = "additive_patch",
    constraints(
        entry_count_limit = "at most 32 key-value entries are accepted",
        integer_seeds = "entry and query seeds are unsigned 64-bit integers",
        nonempty_entries = "at least one key-value entry is required"
    ),
    example(
        label = "one_entry",
        value = "{\"entries\":[{\"key_seed\":1,\"value_seed\":2}],\"query_seed\":1}"
    )
)]
pub struct HolographicRecallRequest {
    /// Ordered key-value entries stored with existing memory semantics.
    pub entries: Vec<HolographicEntry>,
    /// Seed for the query key.
    pub query_seed: u64,
}

/// Exact tracked-key evidence; absence of tracking is never reported as absence of a key.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HolographicMembership {
    /// All exact matches in storage order.
    Present {
        /// Zero-based positions of all exact matches in ingestion order.
        indices: Vec<usize>,
    },
    /// No match in a fully tracked trace.
    Absent,
    /// The trace did not retain sufficient evidence.
    Unknown,
}

/// Capacity metrics reported by `HolographicMemory<MAP256>`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HolographicCapacity {
    /// Number of stored entries.
    pub item_count: usize,
    /// Theoretical MAP256 capacity estimate.
    pub theoretical_capacity: usize,
    /// Current estimated signal-to-noise ratio.
    pub estimated_snr: f64,
    /// Recommended minimum signal-to-noise ratio.
    pub snr_threshold: f64,
    /// Whether storage exceeds half the theoretical capacity.
    pub near_capacity: bool,
}

/// Typed output from MAP256 associative-memory retrieval.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    amari_discovery_macros::WireContract,
)]
#[wire_contract(
    id = "amari.discovery/probe/holographic-recall/output/v2",
    role = "output",
    compatibility = "additive_patch",
    constraints(
        bounded_warnings = "warnings are deterministic and bounded to known capacity conditions",
        capacity_metrics_consistent = "capacity item count, theoretical capacity, and near_capacity reflect the stored entries",
        finite_metrics = "all returned coefficients and reported scores are finite",
        map256_dimension = "raw recall contains exactly 256 MAP components",
        membership_evidence = "present indices identify exact tracked-key matches, independently of nearest_key_similarity"
    )
)]
pub struct HolographicRecallOutput {
    /// Raw left unbinding of the additive memory trace, without cleanup.
    pub raw_coefficients: Vec<f64>,
    /// Exact-key evidence, not a calibrated probability.
    pub membership_evidence: HolographicMembership,
    /// Nearest tracked-key score, or null if no comparison is possible.
    pub nearest_key_similarity: Option<f64>,
    /// Capacity metrics from the underlying memory.
    pub capacity: HolographicCapacity,
    /// Deterministic bounded warnings.
    pub warnings: Vec<String>,
}

#[cfg(feature = "standard-probes")]
pub(super) fn recall_registration() -> DiscoveryResult<AdapterRegistration> {
    Ok(AdapterRegistration {
        id: "amari-probe:holographic:recall:v2".parse()?,
        capability_id: "amari:amari-holographic:memory:retrieval".parse()?,
        input_schema: "amari.discovery/probe/holographic-recall/input/v2".to_owned(),
        output_schema: "amari.discovery/probe/holographic-recall/output/v2".to_owned(),
        required_features: vec!["standard-probes".to_owned()],
        limits: ProbeLimits {
            max_input_bytes: 65_536,
            max_output_bytes: 65_536,
            max_operations: 100_000,
            timeout_millis: 2_000,
        },
        deterministic: true,
        side_effects: SideEffectPolicy::None,
        network: false,
        execute: execute_recall,
    })
}

#[cfg(feature = "standard-probes")]
pub(super) fn superposition_registration() -> DiscoveryResult<AdapterRegistration> {
    Ok(AdapterRegistration {
        id: "amari-probe:holographic:superposition:v1".parse()?,
        capability_id: "amari:amari-holographic:algebra:superposition".parse()?,
        input_schema: "amari.discovery/probe/holographic-superposition/input/v1".to_owned(),
        output_schema: "amari.discovery/probe/holographic-superposition/output/v1".to_owned(),
        required_features: vec!["standard-probes".to_owned()],
        limits: ProbeLimits {
            max_input_bytes: 65_536,
            max_output_bytes: 65_536,
            max_operations: 100_000,
            timeout_millis: 2_000,
        },
        deterministic: true,
        side_effects: SideEffectPolicy::None,
        network: false,
        execute: execute_superposition,
    })
}

#[cfg(feature = "standard-probes")]
fn execute_recall(input: &Value, limits: &EffectiveProbeLimits) -> DiscoveryResult<AdapterOutput> {
    let request: HolographicRecallRequest =
        serde_json::from_value(input.clone()).map_err(|error| {
            DiscoveryError::InvalidInput(format!(
                "holographic recall request requires bounded entries with integer seeds: {error}"
            ))
        })?;
    let resources = validate_recall(&request, limits)?;

    let mut memory = HolographicMemory::<MAP256>::with_key_tracking(AlgebraConfig::default());
    let failed =
        |e: amari_holographic::HolographicError| DiscoveryError::ProbeFailed(e.to_string());
    for entry in request.entries {
        memory
            .store(
                &MAP256::from_seed(entry.key_seed),
                &MAP256::from_seed(entry.value_seed),
            )
            .map_err(failed)?;
    }
    let query = MAP256::from_seed(request.query_seed);
    let retrieval = memory.retrieve(&query).map_err(failed)?;
    use amari_holographic::memory::MembershipEvidence;
    let membership_evidence = match memory.membership(&query).map_err(failed)? {
        MembershipEvidence::Present { indices } => HolographicMembership::Present { indices },
        MembershipEvidence::Absent => HolographicMembership::Absent,
        MembershipEvidence::Unknown => HolographicMembership::Unknown,
    };
    let nearest_key_similarity = memory
        .nearest_key(&query)
        .map_err(failed)?
        .map(|k| k.similarity);
    let capacity = memory.capacity_info();
    let raw_coefficients = retrieval.raw_value.components().to_vec();
    if raw_coefficients.iter().any(|c| !c.is_finite())
        || nearest_key_similarity.is_some_and(|s| !s.is_finite())
        || !capacity.estimated_snr.is_finite()
        || !capacity.snr_threshold.is_finite()
    {
        return Err(DiscoveryError::ProbeFailed(
            "MAP256 recall produced a non-finite result".to_owned(),
        ));
    }
    let warnings = if capacity.near_capacity {
        vec!["MAP256 memory is near or above recommended capacity".to_owned()]
    } else {
        Vec::new()
    };

    Ok(AdapterOutput {
        resources,
        output: serde_json::to_value(HolographicRecallOutput {
            raw_coefficients,
            membership_evidence,
            nearest_key_similarity,
            capacity: HolographicCapacity {
                item_count: capacity.item_count,
                theoretical_capacity: capacity.theoretical_capacity,
                estimated_snr: capacity.estimated_snr,
                snr_threshold: capacity.snr_threshold,
                near_capacity: capacity.near_capacity,
            },
            warnings,
        })?,
    })
}

#[cfg(feature = "standard-probes")]
fn execute_superposition(
    input: &Value,
    limits: &EffectiveProbeLimits,
) -> DiscoveryResult<AdapterOutput> {
    let request: HolographicSuperpositionRequest =
        serde_json::from_value(input.clone()).map_err(|error| {
            DiscoveryError::InvalidInput(format!(
                "holographic superposition request requires an array of integer seeds: {error}"
            ))
        })?;
    let resources = validate_superposition(&request, limits)?;

    let mut trace = MAP256::zero();
    for seed in request.seeds {
        trace = trace.superpose(&MAP256::from_seed(seed)).map_err(|error| {
            DiscoveryError::ProbeFailed(format!("MAP256 additive superposition failed: {error}"))
        })?;
    }
    let coefficients = trace.components().to_vec();
    if coefficients
        .iter()
        .any(|coefficient| !coefficient.is_finite())
    {
        return Err(DiscoveryError::ProbeFailed(
            "MAP256 additive superposition produced a non-finite coefficient".to_owned(),
        ));
    }

    Ok(AdapterOutput {
        resources,
        output: serde_json::to_value(HolographicSuperpositionOutput { coefficients })?,
    })
}

#[cfg(feature = "standard-probes")]
fn validate_recall(
    request: &HolographicRecallRequest,
    limits: &EffectiveProbeLimits,
) -> DiscoveryResult<ResourceObservations> {
    if request.entries.is_empty() {
        return Err(DiscoveryError::InvalidInput(
            "holographic recall requires at least one entry".to_owned(),
        ));
    }
    let entry_count = u64::try_from(request.entries.len()).map_err(|_| {
        DiscoveryError::LimitExceeded("holographic entry count overflow".to_owned())
    })?;
    if entry_count > MAX_RECALL_ENTRIES {
        return Err(DiscoveryError::LimitExceeded(format!(
            "holographic entry count {entry_count} exceeds limit {MAX_RECALL_ENTRIES}"
        )));
    }
    let operations = entry_count
        .checked_mul(RECALL_PASSES_PER_ENTRY)
        .and_then(|passes| passes.checked_add(RECALL_FIXED_PASSES))
        .and_then(|passes| passes.checked_mul(MAP_DIMENSION))
        .ok_or_else(|| {
            DiscoveryError::LimitExceeded("holographic recall operation count overflow".to_owned())
        })?;
    let nodes = entry_count
        .checked_mul(2)
        .and_then(|count| count.checked_add(3))
        .and_then(|count| count.checked_mul(MAP_DIMENSION))
        .and_then(|count| count.checked_add(entry_count))
        .ok_or_else(|| {
            DiscoveryError::LimitExceeded("holographic recall node count overflow".to_owned())
        })?;
    // Store each record, scan for exact matches, and scan separately for nearest.
    let iterations = entry_count
        .checked_mul(3)
        .and_then(|count| count.checked_add(1))
        .ok_or_else(|| {
            DiscoveryError::LimitExceeded("holographic recall iteration count overflow".to_owned())
        })?;
    enforce("operations", operations, limits.max_operations)?;
    enforce("nodes", nodes, limits.max_nodes)?;
    enforce("iterations", iterations, limits.max_iterations)?;

    Ok(ResourceObservations {
        operations,
        nodes,
        iterations,
        bytes: 0,
    })
}

#[cfg(feature = "standard-probes")]
fn validate_superposition(
    request: &HolographicSuperpositionRequest,
    limits: &EffectiveProbeLimits,
) -> DiscoveryResult<ResourceObservations> {
    if request.seeds.is_empty() {
        return Err(DiscoveryError::InvalidInput(
            "holographic superposition requires at least one seed".to_owned(),
        ));
    }
    let seed_count = u64::try_from(request.seeds.len())
        .map_err(|_| DiscoveryError::LimitExceeded("holographic seed count overflow".to_owned()))?;
    if seed_count > MAX_SUPERPOSITION_SEEDS {
        return Err(DiscoveryError::LimitExceeded(format!(
            "holographic seed count {seed_count} exceeds limit {MAX_SUPERPOSITION_SEEDS}"
        )));
    }
    let operations = seed_count.checked_mul(MAP_DIMENSION).ok_or_else(|| {
        DiscoveryError::LimitExceeded("holographic operation count overflow".to_owned())
    })?;
    let nodes = seed_count
        .checked_add(1)
        .and_then(|count| count.checked_mul(MAP_DIMENSION))
        .ok_or_else(|| {
            DiscoveryError::LimitExceeded("holographic node count overflow".to_owned())
        })?;
    enforce("operations", operations, limits.max_operations)?;
    enforce("nodes", nodes, limits.max_nodes)?;
    enforce("iterations", seed_count, limits.max_iterations)?;

    Ok(ResourceObservations {
        operations,
        nodes,
        iterations: seed_count,
        bytes: 0,
    })
}

#[cfg(feature = "standard-probes")]
fn enforce(kind: &str, observed: u64, maximum: u64) -> DiscoveryResult<()> {
    if observed <= maximum {
        Ok(())
    } else {
        Err(DiscoveryError::LimitExceeded(format!(
            "holographic {kind} {observed} exceeds limit {maximum}"
        )))
    }
}
