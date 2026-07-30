// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded tropical Viterbi probe adapter.

use serde::{Deserialize, Serialize};

#[cfg(feature = "standard-probes")]
use amari_tropical::viterbi::TropicalViterbi;
#[cfg(feature = "standard-probes")]
use serde_json::Value;

#[cfg(feature = "standard-probes")]
use super::registry::{AdapterOutput, AdapterRegistration, EffectiveProbeLimits};
#[cfg(feature = "standard-probes")]
use crate::{DiscoveryError, DiscoveryResult, ProbeLimits, ResourceObservations, SideEffectPolicy};

#[cfg(feature = "standard-probes")]
const MAX_STATES: usize = 64;
#[cfg(feature = "standard-probes")]
const MAX_EMISSION_SYMBOLS: usize = 4_096;
#[cfg(feature = "standard-probes")]
const MAX_OBSERVATIONS: usize = 4_096;

/// Typed input for the tropical Viterbi proof-slice probe.
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
    id = "amari.discovery/probe/tropical-viterbi/input/v1",
    role = "input",
    compatibility = "additive_patch",
    constraints(
        emission_rows_match_states = "there is exactly one emission row per state",
        emission_width_limit = "emission rows have equal nonzero width at most 4096",
        finite_weights = "all transition and emission weights are finite",
        nonempty_observations = "at least one observation is required",
        observation_count_limit = "at most 4096 observations are accepted",
        observation_indices_in_bounds = "every observation index is below the emission width",
        square_transitions = "the transition matrix is square",
        state_limit = "at most 64 states are accepted",
        states_nonempty = "at least one state is required"
    ),
    example(
        label = "two_state_decode",
        value = "{\"transitions\":[[0.0,1.0],[1.0,0.0]],\"emissions\":[[0.0,1.0],[1.0,0.0]],\"observations\":[0,1]}"
    )
)]
pub struct TropicalViterbiRequest {
    /// Square state-transition matrix of tropical log weights.
    pub transitions: Vec<Vec<f64>>,
    /// State-by-symbol emission matrix of tropical log weights.
    pub emissions: Vec<Vec<f64>>,
    /// Emission symbol indices to decode.
    pub observations: Vec<usize>,
}

/// Typed output from tropical Viterbi decoding.
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
    id = "amari.discovery/probe/tropical-viterbi/output/v1",
    role = "output",
    compatibility = "additive_patch",
    constraints(
        finite_score = "the decoded tropical score is finite",
        path_length_matches_observations = "the returned path has exactly one state per observation",
        path_states_within_state_count = "every returned path state is below the input state count"
    ),
    example(label = "two_state_decode", value = "{\"path\":[0,1],\"score\":1.0}")
)]
pub struct TropicalViterbiOutput {
    /// Most likely state index at each observation.
    pub path: Vec<usize>,
    /// Tropical score returned by `TropicalViterbi::decode`.
    pub score: f64,
}

#[cfg(feature = "standard-probes")]
pub(super) fn registration() -> DiscoveryResult<AdapterRegistration> {
    Ok(AdapterRegistration {
        id: "amari-probe:tropical:viterbi:v1".parse()?,
        capability_id: "amari:amari-tropical:sequence:viterbi".parse()?,
        input_schema: "amari.discovery/probe/tropical-viterbi/input/v1".to_owned(),
        output_schema: "amari.discovery/probe/tropical-viterbi/output/v1".to_owned(),
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
        execute,
    })
}

#[cfg(feature = "standard-probes")]
fn execute(input: &Value, limits: &EffectiveProbeLimits) -> DiscoveryResult<AdapterOutput> {
    let request: TropicalViterbiRequest =
        serde_json::from_value(input.clone()).map_err(|error| {
            DiscoveryError::InvalidInput(format!(
                "tropical Viterbi request does not match the versioned request schema: {error}"
            ))
        })?;
    let shape = validate_request(&request, limits)?;
    let decoder = TropicalViterbi::new(request.transitions, request.emissions);
    let (path, score) = decoder.decode(&request.observations);
    let score = score.value();
    if !score.is_finite() {
        return Err(DiscoveryError::ProbeFailed(
            "tropical Viterbi produced a non-finite score".to_owned(),
        ));
    }
    Ok(AdapterOutput {
        resources: ResourceObservations {
            operations: shape.operations,
            nodes: shape.nodes,
            iterations: shape.iterations,
            bytes: 0,
        },
        output: serde_json::to_value(TropicalViterbiOutput { path, score })?,
    })
}

#[cfg(feature = "standard-probes")]
struct RequestShape {
    operations: u64,
    nodes: u64,
    iterations: u64,
}

#[cfg(feature = "standard-probes")]
fn validate_request(
    request: &TropicalViterbiRequest,
    limits: &EffectiveProbeLimits,
) -> DiscoveryResult<RequestShape> {
    let states = request.transitions.len();
    if states == 0 {
        return invalid("tropical Viterbi requires at least one state");
    }
    if states > MAX_STATES {
        return invalid(format!(
            "tropical Viterbi state count {states} exceeds limit {MAX_STATES}"
        ));
    }
    if request.transitions.iter().any(|row| row.len() != states) {
        return invalid("tropical Viterbi requires a square transition matrix");
    }
    if request.emissions.len() != states {
        return invalid("tropical Viterbi requires one emission row per state");
    }
    let symbols = request.emissions.first().map_or(0, Vec::len);
    if symbols == 0
        || symbols > MAX_EMISSION_SYMBOLS
        || request.emissions.iter().any(|row| row.len() != symbols)
    {
        return invalid(format!(
            "tropical Viterbi emission rows require equal nonzero width at most {MAX_EMISSION_SYMBOLS}"
        ));
    }
    if request.observations.is_empty() {
        return invalid("tropical Viterbi requires at least one observation");
    }
    if request.observations.len() > MAX_OBSERVATIONS {
        return invalid(format!(
            "tropical Viterbi observation count {} exceeds limit {MAX_OBSERVATIONS}",
            request.observations.len()
        ));
    }
    if request
        .observations
        .iter()
        .any(|observation| *observation >= symbols)
    {
        return invalid(format!(
            "tropical Viterbi observation index must be below emission width {symbols}"
        ));
    }
    if request
        .transitions
        .iter()
        .chain(&request.emissions)
        .flatten()
        .any(|weight| !weight.is_finite())
    {
        return invalid("tropical Viterbi matrix weights must be finite");
    }

    let states = u64::try_from(states).map_err(|_| limit("state count overflow"))?;
    let symbols = u64::try_from(symbols).map_err(|_| limit("symbol count overflow"))?;
    let observations = u64::try_from(request.observations.len())
        .map_err(|_| limit("observation count overflow"))?;
    let transition_operations = observations
        .saturating_sub(1)
        .checked_mul(states)
        .and_then(|value| value.checked_mul(states))
        .ok_or_else(|| limit("tropical Viterbi operations overflow"))?;
    let operations = states
        .checked_add(transition_operations)
        .and_then(|value| value.checked_add(states))
        .and_then(|value| value.checked_add(observations.saturating_sub(1)))
        .ok_or_else(|| limit("tropical Viterbi operations overflow"))?;
    let nodes = states
        .checked_mul(states)
        .and_then(|value| value.checked_add(states.checked_mul(symbols)?))
        .and_then(|value| value.checked_add(states.checked_mul(observations)?))
        .ok_or_else(|| limit("tropical Viterbi nodes overflow"))?;

    enforce("operations", operations, limits.max_operations)?;
    enforce("nodes", nodes, limits.max_nodes)?;
    enforce("iterations", observations, limits.max_iterations)?;
    Ok(RequestShape {
        operations,
        nodes,
        iterations: observations,
    })
}

#[cfg(feature = "standard-probes")]
fn enforce(kind: &str, observed: u64, maximum: u64) -> DiscoveryResult<()> {
    if observed <= maximum {
        Ok(())
    } else {
        Err(limit(format!(
            "tropical Viterbi {kind} {observed} exceeds limit {maximum}"
        )))
    }
}

#[cfg(feature = "standard-probes")]
fn invalid<T>(message: impl Into<String>) -> DiscoveryResult<T> {
    Err(DiscoveryError::InvalidInput(message.into()))
}

#[cfg(feature = "standard-probes")]
fn limit(message: impl Into<String>) -> DiscoveryError {
    DiscoveryError::LimitExceeded(message.into())
}
