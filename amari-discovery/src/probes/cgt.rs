// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded combinatorial-game nim-sum probe adapter.

use serde::{Deserialize, Serialize};

#[cfg(feature = "standard-probes")]
use amari_cgt::GameArena;
#[cfg(feature = "standard-probes")]
use serde_json::Value;

#[cfg(feature = "standard-probes")]
use super::registry::{AdapterOutput, AdapterRegistration, EffectiveProbeLimits};
#[cfg(feature = "standard-probes")]
use crate::{DiscoveryError, DiscoveryResult, ProbeLimits, ResourceObservations, SideEffectPolicy};

#[cfg(feature = "standard-probes")]
const MAX_HEAPS: u64 = 256;
#[cfg(feature = "standard-probes")]
const MAX_HEAP_VALUE: u64 = 64;
#[cfg(feature = "standard-probes")]
const OPERATIONS_PER_OPTION_ENTRY: u64 = 4;
#[cfg(feature = "standard-probes")]
const OPERATIONS_PER_REQUESTED_HEAP: u64 = 2;

/// Typed input for exact nim-sum evaluation through Sprague-Grundy values.
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
    id = "amari.discovery/probe/cgt-nim-sum/input/v1",
    role = "input",
    compatibility = "additive_patch",
    constraints(
        heap_count_limit = "at most 256 Nim heaps are accepted per request",
        heap_value_limit = "each Nim heap value must be no greater than 64"
    ),
    example(label = "three_heaps", value = "{\"heaps\":[1,2,3]}")
)]
pub struct CgtNimSumRequest {
    /// Sizes of the independent Nim heaps.
    pub heaps: Vec<u32>,
}

/// Typed output from exact nim-sum evaluation.
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
#[wire_contract(
    id = "amari.discovery/probe/cgt-nim-sum/output/v1",
    role = "output",
    compatibility = "additive_patch",
    constraints(
        grundy_values_align_with_heaps = "grundy_values has exactly one entry for each requested heap, in request order",
        nim_sum_is_xor = "nim_sum is the bitwise XOR of every grundy_values entry"
    ),
    example(
        label = "three_heaps",
        value = "{\"grundy_values\":[1,2,3],\"nim_sum\":0}"
    )
)]
pub struct CgtNimSumOutput {
    /// Sprague-Grundy value computed directly for each requested heap.
    pub grundy_values: Vec<u32>,
    /// Bitwise XOR of all per-heap Sprague-Grundy values.
    pub nim_sum: u32,
}

#[cfg(feature = "standard-probes")]
pub(super) fn registration() -> DiscoveryResult<AdapterRegistration> {
    Ok(AdapterRegistration {
        id: "amari-probe:cgt:nim-sum:v1".parse()?,
        capability_id: "amari:amari-cgt:nim:grundy-sum".parse()?,
        input_schema: "amari.discovery/probe/cgt-nim-sum/input/v1".to_owned(),
        output_schema: "amari.discovery/probe/cgt-nim-sum/output/v1".to_owned(),
        required_features: vec!["standard-probes".to_owned()],
        limits: ProbeLimits {
            max_input_bytes: 16_384,
            max_output_bytes: 4_096,
            max_operations: 10_000,
            timeout_millis: 1_000,
        },
        deterministic: true,
        side_effects: SideEffectPolicy::None,
        network: false,
        execute,
    })
}

#[cfg(feature = "standard-probes")]
fn execute(input: &Value, limits: &EffectiveProbeLimits) -> DiscoveryResult<AdapterOutput> {
    let request: CgtNimSumRequest = serde_json::from_value(input.clone()).map_err(|error| {
        DiscoveryError::InvalidInput(format!(
            "nim-sum request requires unsigned 32-bit heap values: {error}"
        ))
    })?;
    let resources = validate_request(&request, limits)?;

    let mut arena = GameArena::new();
    let mut grundy_values = Vec::with_capacity(request.heaps.len());
    let mut nim_sum = 0_u32;
    for size in request.heaps {
        let heap = arena.nim_heap(size).map_err(|_| {
            DiscoveryError::ProbeFailed("bounded Nim heap construction failed".to_owned())
        })?;
        let grundy = arena.grundy(heap).map_err(|_| {
            DiscoveryError::ProbeFailed("bounded Nim heap Grundy evaluation failed".to_owned())
        })?;
        grundy_values.push(grundy.0);
        nim_sum ^= grundy.0;
    }

    Ok(AdapterOutput {
        resources,
        output: serde_json::to_value(CgtNimSumOutput {
            grundy_values,
            nim_sum,
        })?,
    })
}

#[cfg(feature = "standard-probes")]
fn validate_request(
    request: &CgtNimSumRequest,
    limits: &EffectiveProbeLimits,
) -> DiscoveryResult<ResourceObservations> {
    let heap_count = u64::try_from(request.heaps.len())
        .map_err(|_| DiscoveryError::LimitExceeded("nim heap count overflow".to_owned()))?;
    enforce("heap count", heap_count, MAX_HEAPS)?;

    let maximum_heap = request.heaps.iter().copied().max().unwrap_or(0);
    enforce("heap value", u64::from(maximum_heap), MAX_HEAP_VALUE)?;

    // Building every cached heap from zero through `maximum_heap` creates one
    // logical option set of sizes 1..=maximum_heap. Count it before calling
    // `GameArena::nim_heap`, which is the first operation that allocates those
    // recursively constructed option vectors.
    let maximum_heap = u64::from(maximum_heap);
    let option_entries = maximum_heap
        .checked_mul(maximum_heap.checked_add(1).ok_or_else(|| {
            DiscoveryError::LimitExceeded("nim option-entry count overflow".to_owned())
        })?)
        .and_then(|value| value.checked_div(2))
        .ok_or_else(|| {
            DiscoveryError::LimitExceeded("nim option-entry count overflow".to_owned())
        })?;
    let operations = option_entries
        .checked_mul(OPERATIONS_PER_OPTION_ENTRY)
        .and_then(|value| {
            heap_count
                .checked_mul(OPERATIONS_PER_REQUESTED_HEAP)
                .and_then(|heap_work| value.checked_add(heap_work))
        })
        .ok_or_else(|| DiscoveryError::LimitExceeded("nim operation count overflow".to_owned()))?;
    let nodes = if request.heaps.is_empty() {
        0
    } else {
        maximum_heap.checked_add(1).ok_or_else(|| {
            DiscoveryError::LimitExceeded("nim arena node count overflow".to_owned())
        })?
    };
    let iterations = nodes
        .checked_add(heap_count)
        .ok_or_else(|| DiscoveryError::LimitExceeded("nim iteration count overflow".to_owned()))?;

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
fn enforce(kind: &str, observed: u64, maximum: u64) -> DiscoveryResult<()> {
    if observed <= maximum {
        Ok(())
    } else {
        Err(DiscoveryError::LimitExceeded(format!(
            "nim-sum {kind} {observed} exceeds limit {maximum}"
        )))
    }
}
