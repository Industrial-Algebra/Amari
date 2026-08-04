// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded geometric-network shortest-path probe adapter.

use serde::{Deserialize, Serialize};

#[cfg(feature = "standard-probes")]
use amari_core::Vector;
#[cfg(feature = "standard-probes")]
use amari_network::GeometricNetwork;
#[cfg(feature = "standard-probes")]
use serde_json::Value;

#[cfg(feature = "standard-probes")]
use super::registry::{AdapterOutput, AdapterRegistration, EffectiveProbeLimits};
#[cfg(feature = "standard-probes")]
use crate::{DiscoveryError, DiscoveryResult, ProbeLimits, ResourceObservations, SideEffectPolicy};

#[cfg(feature = "standard-probes")]
const MAX_NODES: u64 = 128;

/// Typed directed adjacency-matrix request for a shortest path.
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
    id = "amari.discovery/probe/network-shortest-path/input/v1",
    role = "input",
    compatibility = "additive_patch",
    constraints(
        adjacency_node_limit = "the adjacency matrix contains at most 128 nodes",
        adjacency_nonempty = "the adjacency matrix contains at least one node",
        adjacency_square = "every adjacency row has one entry per node",
        endpoint_indices_in_bounds = "source and target are valid node indices",
        finite_nonnegative_weights = "every present edge weight is finite and nonnegative"
    ),
    example(
        label = "two_node_path",
        value = "{\"adjacency\":[[0.0,1.0],[null,0.0]],\"source\":0,\"target\":1}"
    )
)]
pub struct NetworkShortestPathRequest {
    /// Square directed adjacency matrix, where `None` means no edge.
    pub adjacency: Vec<Vec<Option<f64>>>,
    /// Source node index.
    pub source: usize,
    /// Target node index.
    pub target: usize,
}

/// One reachable shortest path and its total edge weight.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NetworkPath {
    /// Node indices from source through target, inclusive.
    pub nodes: Vec<usize>,
    /// Sum of the directed edge weights along the path.
    pub total_weight: f64,
}

/// Typed output from geometric-network shortest-path evaluation.
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
    id = "amari.discovery/probe/network-shortest-path/output/v1",
    role = "output",
    compatibility = "additive_patch",
    constraints(
        finite_total_weight = "a reachable path has a finite total weight",
        optional_path_shape = "path is null exactly when the target is unreachable",
        path_nodes_within_node_count = "every returned path node is a valid input node index"
    ),
    example(
        label = "two_node_path",
        value = "{\"path\":{\"nodes\":[0,1],\"total_weight\":1.0}}"
    )
)]
pub struct NetworkShortestPathOutput {
    /// The shortest path, or `None` when the target is unreachable.
    pub path: Option<NetworkPath>,
}

#[cfg(feature = "standard-probes")]
pub(super) fn registration() -> DiscoveryResult<AdapterRegistration> {
    Ok(AdapterRegistration {
        id: "amari-probe:network:shortest-path:v1".parse()?,
        capability_id: "amari:amari-network:paths:geometric-shortest-path".parse()?,
        input_schema: "amari.discovery/probe/network-shortest-path/input/v1".to_owned(),
        output_schema: "amari.discovery/probe/network-shortest-path/output/v1".to_owned(),
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
    let request: NetworkShortestPathRequest =
        serde_json::from_value(input.clone()).map_err(|error| {
            DiscoveryError::InvalidInput(format!(
                "network request requires a square adjacency matrix of finite nonnegative weights and valid indices: {error}"
            ))
        })?;
    let shape = validate_request(&request, limits)?;

    let mut network =
        GeometricNetwork::<3, 0, 0>::with_capacity(request.adjacency.len(), shape.edge_count);
    for index in 0..request.adjacency.len() {
        network.add_node(deterministic_position(index));
    }
    for (source, row) in request.adjacency.iter().enumerate() {
        for (target, weight) in row.iter().enumerate() {
            if let Some(weight) = weight {
                network.add_edge(source, target, *weight).map_err(|error| {
                    DiscoveryError::Internal(format!(
                        "validated network edge could not be added: {error}"
                    ))
                })?;
            }
        }
    }

    let shortest_path = network
        .shortest_path(request.source, request.target)
        .map_err(|error| {
            DiscoveryError::ProbeFailed(format!("network shortest path failed: {error}"))
        })?;
    let mut resources = shape.resources;
    let path = match shortest_path {
        Some((nodes, total_weight)) => {
            if !total_weight.is_finite() {
                return Err(DiscoveryError::ProbeFailed(
                    "network shortest path produced a non-finite total weight".to_owned(),
                ));
            }
            Some(NetworkPath {
                nodes,
                total_weight,
            })
        }
        None => {
            account_reachability_fallback(&mut resources, shape.node_count, limits)?;
            if topologically_reachable(&request) {
                return Err(DiscoveryError::ProbeFailed(
                    "network shortest path has a non-finite accumulated weight".to_owned(),
                ));
            }
            None
        }
    };

    Ok(AdapterOutput {
        resources,
        output: serde_json::to_value(NetworkShortestPathOutput { path })?,
    })
}

#[cfg(feature = "standard-probes")]
struct RequestShape {
    node_count: u64,
    edge_count: usize,
    resources: ResourceObservations,
}

#[cfg(feature = "standard-probes")]
fn validate_request(
    request: &NetworkShortestPathRequest,
    limits: &EffectiveProbeLimits,
) -> DiscoveryResult<RequestShape> {
    if request.adjacency.is_empty() {
        return Err(DiscoveryError::InvalidInput(
            "network adjacency matrix must be non-empty".to_owned(),
        ));
    }
    let node_count = u64::try_from(request.adjacency.len())
        .map_err(|_| DiscoveryError::LimitExceeded("network node count overflow".to_owned()))?;
    if node_count > MAX_NODES {
        return Err(DiscoveryError::LimitExceeded(format!(
            "network node count {node_count} exceeds limit {MAX_NODES}"
        )));
    }
    if request
        .adjacency
        .iter()
        .any(|row| row.len() != request.adjacency.len())
    {
        return Err(DiscoveryError::InvalidInput(
            "network adjacency matrix must be square".to_owned(),
        ));
    }
    if request.source >= request.adjacency.len() {
        return Err(DiscoveryError::InvalidInput(format!(
            "network source index {} is outside node count {node_count}",
            request.source
        )));
    }
    if request.target >= request.adjacency.len() {
        return Err(DiscoveryError::InvalidInput(format!(
            "network target index {} is outside node count {node_count}",
            request.target
        )));
    }
    if request
        .adjacency
        .iter()
        .flatten()
        .flatten()
        .any(|weight| !weight.is_finite() || *weight < 0.0)
    {
        return Err(DiscoveryError::InvalidInput(
            "network edge weights must be finite nonnegative numbers".to_owned(),
        ));
    }

    let edge_count = request
        .adjacency
        .iter()
        .flatten()
        .filter(|weight| weight.is_some())
        .count();
    let edge_operations = u64::try_from(edge_count)
        .map_err(|_| DiscoveryError::LimitExceeded("network edge count overflow".to_owned()))?;
    let operations = node_count
        .checked_mul(node_count)
        .and_then(|value| value.checked_add(edge_operations))
        .ok_or_else(|| {
            DiscoveryError::LimitExceeded("network operation count overflow".to_owned())
        })?;
    enforce("operations", operations, limits.max_operations)?;
    enforce("nodes", node_count, limits.max_nodes)?;
    enforce("iterations", node_count, limits.max_iterations)?;

    Ok(RequestShape {
        node_count,
        edge_count,
        resources: ResourceObservations {
            operations,
            nodes: node_count,
            iterations: node_count,
            bytes: 0,
        },
    })
}

#[cfg(feature = "standard-probes")]
fn account_reachability_fallback(
    resources: &mut ResourceObservations,
    node_count: u64,
    limits: &EffectiveProbeLimits,
) -> DiscoveryResult<()> {
    let extra_operations = node_count.checked_mul(node_count).ok_or_else(|| {
        DiscoveryError::LimitExceeded("network reachability operation count overflow".to_owned())
    })?;
    resources.operations = resources
        .operations
        .checked_add(extra_operations)
        .ok_or_else(|| {
            DiscoveryError::LimitExceeded("network total operation count overflow".to_owned())
        })?;
    resources.iterations = resources
        .iterations
        .checked_add(node_count)
        .ok_or_else(|| {
            DiscoveryError::LimitExceeded("network total iteration count overflow".to_owned())
        })?;
    enforce("operations", resources.operations, limits.max_operations)?;
    enforce("iterations", resources.iterations, limits.max_iterations)
}

#[cfg(feature = "standard-probes")]
fn topologically_reachable(request: &NetworkShortestPathRequest) -> bool {
    let mut visited = vec![false; request.adjacency.len()];
    let mut pending = vec![request.source];
    visited[request.source] = true;

    while let Some(current) = pending.pop() {
        if current == request.target {
            return true;
        }
        for (neighbor, weight) in request.adjacency[current].iter().enumerate() {
            if weight.is_some() && !visited[neighbor] {
                visited[neighbor] = true;
                pending.push(neighbor);
            }
        }
    }
    false
}

#[cfg(feature = "standard-probes")]
fn deterministic_position(index: usize) -> amari_core::Multivector<3, 0, 0> {
    Vector::from_components(index as f64, 0.0, 0.0).mv
}

#[cfg(feature = "standard-probes")]
fn enforce(kind: &str, observed: u64, maximum: u64) -> DiscoveryResult<()> {
    if observed <= maximum {
        Ok(())
    } else {
        Err(DiscoveryError::LimitExceeded(format!(
            "network shortest-path {kind} {observed} exceeds limit {maximum}"
        )))
    }
}
