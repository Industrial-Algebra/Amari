// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded expansion through the semantic capability graph.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use amari_core::Multivector;
use amari_network::GeometricNetwork;
use amari_tropical::verified::{MinPlus, VerifiedTropicalNumber};
use serde::{Deserialize, Serialize};

use crate::{CapabilityId, Catalog, DiscoveryError, DiscoveryResult};

type MinPlusNumber = VerifiedTropicalNumber<f64, MinPlus>;

/// Resource limits for capability-graph expansion.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphLimits {
    /// Maximum number of distinct capability nodes retained.
    pub max_nodes: usize,
    /// Maximum number of relationship edges from any seed.
    pub max_depth: usize,
}

impl GraphLimits {
    /// Hard ceiling for retained capability nodes, independent of caller input.
    pub const MAX_ALLOWED_NODES: usize = 64;
    /// Hard ceiling for semantic relationship traversal depth.
    pub const MAX_ALLOWED_DEPTH: usize = 16;

    fn validate(self) -> DiscoveryResult<()> {
        if self.max_nodes == 0 {
            return Err(DiscoveryError::InvalidInput(
                "capability graph max_nodes must be greater than zero".to_owned(),
            ));
        }
        if self.max_nodes > Self::MAX_ALLOWED_NODES {
            return Err(DiscoveryError::LimitExceeded(format!(
                "capability graph max_nodes {} exceeds hard ceiling {}",
                self.max_nodes,
                Self::MAX_ALLOWED_NODES
            )));
        }
        if self.max_depth > Self::MAX_ALLOWED_DEPTH {
            return Err(DiscoveryError::LimitExceeded(format!(
                "capability graph max_depth {} exceeds hard ceiling {}",
                self.max_depth,
                Self::MAX_ALLOWED_DEPTH
            )));
        }
        Ok(())
    }
}

impl Default for GraphLimits {
    fn default() -> Self {
        Self {
            max_nodes: 64,
            max_depth: 4,
        }
    }
}

/// Project-derived constraints that invalidate capability candidates.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct GraphConstraints {
    /// Capability IDs that must not appear in expansion paths.
    pub blocked_capabilities: BTreeSet<CapabilityId>,
}

/// Deterministic nonnegative costs assigned to relationship kinds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RelationCostPolicy {
    /// Cost used for relationship kinds without an explicit override.
    pub default_cost: f64,
    /// Per-kind cost overrides keyed by semantic relationship kind.
    pub relation_costs: BTreeMap<String, f64>,
}

impl Default for RelationCostPolicy {
    fn default() -> Self {
        Self {
            default_cost: 2.0,
            relation_costs: BTreeMap::from([
                ("alternative_to".to_owned(), 2.0),
                ("composes_with".to_owned(), 1.5),
                ("produces_input_for".to_owned(), 1.0),
                ("supports".to_owned(), 1.0),
            ]),
        }
    }
}

impl RelationCostPolicy {
    fn cost(&self, kind: &str) -> f64 {
        self.relation_costs
            .get(kind)
            .copied()
            .unwrap_or(self.default_cost)
    }

    fn validate(&self) -> DiscoveryResult<()> {
        validate_cost("default", self.default_cost)?;
        for (kind, cost) in &self.relation_costs {
            if kind.is_empty() {
                return Err(DiscoveryError::InvalidInput(
                    "relationship cost kind must not be empty".to_owned(),
                ));
            }
            validate_cost(kind, *cost)?;
        }
        Ok(())
    }
}

/// A resource boundary that truncated graph expansion.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GraphLimit {
    /// The retained-node limit was reached.
    NodeCount {
        /// Configured maximum retained nodes.
        max: usize,
        /// Bounded attempted count (`max.saturating_add(1)`).
        observed: usize,
    },
    /// The traversal-depth limit left an unvisited frontier.
    Depth {
        /// Configured maximum edge depth.
        max: usize,
        /// Bounded attempted depth (`max + 1`).
        observed: usize,
    },
}

/// Completion state for bounded graph expansion.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GraphExpansionState {
    /// Every reachable unblocked capability fit within the configured limits.
    Complete,
    /// Deterministic partial paths are available despite one or more limits.
    Partial {
        /// Limits reached, in stable node-count then depth order.
        limits: Vec<GraphLimit>,
    },
}

/// One semantic relationship traversed by a capability path.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphStep {
    /// Capability at the start of this step.
    pub from: CapabilityId,
    /// Capability at the end of this step.
    pub to: CapabilityId,
    /// Curated relationship kind.
    pub relation_kind: String,
    /// Whether traversal reverses the curated relation to include a prerequisite
    /// or the other side of a symmetric composition.
    pub reverse: bool,
    /// Validated finite nonnegative edge cost.
    pub cost: f64,
}

/// Lowest-cost integration path from a seed to one expanded capability.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphPath {
    /// Expanded capability reached by this path.
    pub target: CapabilityId,
    /// Seed candidate from which this path starts.
    pub source_seed: CapabilityId,
    /// Ordered capabilities, including source and target.
    pub capabilities: Vec<CapabilityId>,
    /// Ordered relationship steps between adjacent capabilities.
    pub steps: Vec<GraphStep>,
    /// Min-plus sum of step costs.
    pub total_cost: f64,
}

/// Result of bounded capability-graph expansion.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphExpansion {
    /// Lowest-cost path for each retained capability, sorted by target ID.
    pub paths: Vec<GraphPath>,
    /// Blocked capabilities encountered or supplied as blocked seeds.
    pub blocked_capabilities: Vec<CapabilityId>,
    /// Complete or typed partial expansion state.
    pub state: GraphExpansionState,
}

/// Deterministic bounded semantic graph expander.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CapabilityGraphExpander {
    limits: GraphLimits,
    costs: RelationCostPolicy,
}

impl CapabilityGraphExpander {
    /// Creates an expander with explicit bounds and relationship costs.
    ///
    /// # Errors
    ///
    /// Returns [`DiscoveryError::InvalidInput`] for a zero node limit or an
    /// invalid relationship cost. Returns [`DiscoveryError::LimitExceeded`]
    /// when caller-provided graph limits exceed the fixed hard ceilings.
    pub fn new(limits: GraphLimits, costs: RelationCostPolicy) -> DiscoveryResult<Self> {
        limits.validate()?;
        costs.validate()?;
        Ok(Self { limits, costs })
    }

    /// Expands seed candidates through bounded semantic relationships.
    ///
    /// `supports` and `produces_input_for` relations may be traversed in
    /// reverse to include prerequisites. `composes_with` and `alternative_to`
    /// are traversed in both directions. Other relation kinds retain their
    /// curated direction. Invalidating relation kinds are never traversal
    /// edges; project-specific invalidation is supplied through `constraints`.
    ///
    /// # Errors
    ///
    /// Returns [`DiscoveryError::InvalidInput`] for seed or blocked IDs absent
    /// from the supplied catalog. Network or min-plus parity failures are
    /// reported as typed internal errors.
    pub fn expand(
        &self,
        catalog: &Catalog,
        seeds: &[CapabilityId],
        constraints: &GraphConstraints,
    ) -> DiscoveryResult<GraphExpansion> {
        let catalog_ids: BTreeSet<_> = catalog
            .capabilities()
            .iter()
            .map(|capability| capability.id.clone())
            .collect();
        validate_ids("seed", seeds.iter(), &catalog_ids)?;
        validate_ids(
            "blocked capability",
            constraints.blocked_capabilities.iter(),
            &catalog_ids,
        )?;

        let edges = traversal_edges(catalog, &self.costs);
        let adjacency = adjacency(&edges);
        let (visited, blocked, state) = self.bounded_reachability(seeds, constraints, &adjacency);
        let paths = shortest_paths(&visited, seeds, &edges, self.limits.max_depth)?;

        Ok(GraphExpansion {
            paths,
            blocked_capabilities: blocked.into_iter().collect(),
            state,
        })
    }

    fn bounded_reachability(
        &self,
        seeds: &[CapabilityId],
        constraints: &GraphConstraints,
        adjacency: &BTreeMap<CapabilityId, Vec<TraversalEdge>>,
    ) -> (
        BTreeMap<CapabilityId, usize>,
        BTreeSet<CapabilityId>,
        GraphExpansionState,
    ) {
        let mut visited = BTreeMap::new();
        let mut blocked = BTreeSet::new();
        let mut queue = VecDeque::new();
        let mut node_limited = false;
        let mut depth_limited = false;

        let unique_seeds: BTreeSet<_> = seeds.iter().cloned().collect();
        for seed in unique_seeds {
            if constraints.blocked_capabilities.contains(&seed) {
                blocked.insert(seed);
            } else if visited.len() < self.limits.max_nodes {
                visited.insert(seed.clone(), 0);
                queue.push_back(seed);
            } else {
                node_limited = true;
            }
        }

        while let Some(current) = queue.pop_front() {
            let depth = visited[&current];
            let neighbors = adjacency.get(&current).map(Vec::as_slice).unwrap_or(&[]);
            for edge in neighbors {
                if constraints.blocked_capabilities.contains(&edge.to) {
                    blocked.insert(edge.to.clone());
                    continue;
                }
                if visited.contains_key(&edge.to) {
                    continue;
                }
                if depth >= self.limits.max_depth {
                    depth_limited = true;
                    continue;
                }
                if visited.len() >= self.limits.max_nodes {
                    node_limited = true;
                    continue;
                }
                visited.insert(edge.to.clone(), depth + 1);
                queue.push_back(edge.to.clone());
            }
        }

        let mut limits = Vec::new();
        if node_limited {
            limits.push(GraphLimit::NodeCount {
                max: self.limits.max_nodes,
                observed: self.limits.max_nodes.saturating_add(1),
            });
        }
        if depth_limited {
            limits.push(GraphLimit::Depth {
                max: self.limits.max_depth,
                observed: self.limits.max_depth.saturating_add(1),
            });
        }
        let state = if limits.is_empty() {
            GraphExpansionState::Complete
        } else {
            GraphExpansionState::Partial { limits }
        };
        (visited, blocked, state)
    }
}

#[derive(Clone, Debug)]
struct TraversalEdge {
    from: CapabilityId,
    to: CapabilityId,
    relation_kind: String,
    reverse: bool,
    cost: f64,
}

fn traversal_edges(catalog: &Catalog, costs: &RelationCostPolicy) -> Vec<TraversalEdge> {
    let mut edges = Vec::new();
    for relation in catalog.relations() {
        if is_invalidating(&relation.kind) {
            continue;
        }
        let cost = costs.cost(&relation.kind);
        edges.push(TraversalEdge {
            from: relation.from.clone(),
            to: relation.to.clone(),
            relation_kind: relation.kind.clone(),
            reverse: false,
            cost,
        });
        if is_reversible(&relation.kind) {
            edges.push(TraversalEdge {
                from: relation.to.clone(),
                to: relation.from.clone(),
                relation_kind: relation.kind.clone(),
                reverse: true,
                cost,
            });
        }
    }
    edges.sort_by(|left, right| {
        (&left.from, &left.to, &left.relation_kind, left.reverse).cmp(&(
            &right.from,
            &right.to,
            &right.relation_kind,
            right.reverse,
        ))
    });
    edges
}

fn adjacency(edges: &[TraversalEdge]) -> BTreeMap<CapabilityId, Vec<TraversalEdge>> {
    let mut adjacency = BTreeMap::<CapabilityId, Vec<TraversalEdge>>::new();
    for edge in edges {
        adjacency
            .entry(edge.from.clone())
            .or_default()
            .push(edge.clone());
    }
    adjacency
}

fn shortest_paths(
    visited: &BTreeMap<CapabilityId, usize>,
    seeds: &[CapabilityId],
    edges: &[TraversalEdge],
    max_depth: usize,
) -> DiscoveryResult<Vec<GraphPath>> {
    if visited.is_empty() {
        return Ok(Vec::new());
    }

    let ids: Vec<_> = visited.keys().cloned().collect();
    let indices: BTreeMap<_, _> = ids
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, id)| (id, index))
        .collect();
    let mut network = GeometricNetwork::<1, 0, 0>::with_capacity(ids.len(), edges.len());
    for index in 0..ids.len() {
        network.add_node(Multivector::scalar(index as f64));
    }

    let mut selected_edges = BTreeMap::<(CapabilityId, CapabilityId), TraversalEdge>::new();
    for edge in edges {
        let (Some(from_depth), Some(to_depth)) = (visited.get(&edge.from), visited.get(&edge.to))
        else {
            continue;
        };
        // Restrict path selection to the bounded BFS layers. Otherwise an
        // unconstrained cheaper route can exceed max_depth and hide a valid
        // shallower route already retained by expansion.
        if *to_depth != from_depth.saturating_add(1) {
            continue;
        }
        let key = (edge.from.clone(), edge.to.clone());
        match selected_edges.get(&key) {
            Some(existing) if !edge_precedes(edge, existing) => {}
            _ => {
                selected_edges.insert(key, edge.clone());
            }
        }
    }
    for edge in selected_edges.values() {
        network
            .add_edge(indices[&edge.from], indices[&edge.to], edge.cost)
            .map_err(|error| {
                DiscoveryError::Internal(format!("capability network construction failed: {error}"))
            })?;
    }

    let available_seeds: BTreeSet<_> = seeds
        .iter()
        .filter(|seed| visited.contains_key(*seed))
        .cloned()
        .collect();
    let mut paths = Vec::with_capacity(ids.len());
    for target in &ids {
        let mut best: Option<GraphPath> = None;
        for seed in &available_seeds {
            let Some((indices_path, network_cost)) = network
                .shortest_path(indices[seed], indices[target])
                .map_err(|error| {
                    DiscoveryError::Internal(format!("capability shortest path failed: {error}"))
                })?
            else {
                continue;
            };
            if indices_path.len().saturating_sub(1) > max_depth {
                continue;
            }
            let capabilities: Vec<_> = indices_path
                .iter()
                .map(|index| ids[*index].clone())
                .collect();
            let steps = graph_steps(&capabilities, &selected_edges)?;
            let total_cost = min_plus_path_cost(&steps);
            if (network_cost - total_cost).abs() > 1e-9 {
                return Err(DiscoveryError::Internal(
                    "network and min-plus path costs diverged".to_owned(),
                ));
            }
            let candidate = GraphPath {
                target: target.clone(),
                source_seed: seed.clone(),
                capabilities,
                steps,
                total_cost,
            };
            let replaces_best = match &best {
                Some(current) => path_precedes(&candidate, current),
                None => true,
            };
            if replaces_best {
                best = Some(candidate);
            }
        }
        if let Some(path) = best {
            paths.push(path);
        }
    }
    Ok(paths)
}

fn graph_steps(
    capabilities: &[CapabilityId],
    edges: &BTreeMap<(CapabilityId, CapabilityId), TraversalEdge>,
) -> DiscoveryResult<Vec<GraphStep>> {
    capabilities
        .windows(2)
        .map(|pair| {
            let edge = edges
                .get(&(pair[0].clone(), pair[1].clone()))
                .ok_or_else(|| {
                    DiscoveryError::Internal("shortest path contains an unknown edge".to_owned())
                })?;
            Ok(GraphStep {
                from: edge.from.clone(),
                to: edge.to.clone(),
                relation_kind: edge.relation_kind.clone(),
                reverse: edge.reverse,
                cost: edge.cost,
            })
        })
        .collect()
}

fn min_plus_path_cost(steps: &[GraphStep]) -> f64 {
    steps
        .iter()
        .fold(MinPlusNumber::tropical_one(), |cost, step| {
            cost.tropical_mul(MinPlusNumber::new(step.cost))
        })
        .value()
}

fn path_precedes(candidate: &GraphPath, current: &GraphPath) -> bool {
    match candidate.total_cost.total_cmp(&current.total_cost) {
        Ordering::Less => {
            let selected = MinPlusNumber::new(candidate.total_cost)
                .tropical_add(MinPlusNumber::new(current.total_cost))
                .value();
            selected == candidate.total_cost
        }
        Ordering::Greater => false,
        Ordering::Equal => {
            (&candidate.source_seed, &candidate.capabilities)
                < (&current.source_seed, &current.capabilities)
        }
    }
}

fn edge_precedes(candidate: &TraversalEdge, current: &TraversalEdge) -> bool {
    candidate.cost.total_cmp(&current.cost) == Ordering::Less
        || (candidate.cost.total_cmp(&current.cost) == Ordering::Equal
            && (&candidate.relation_kind, candidate.reverse)
                < (&current.relation_kind, current.reverse))
}

fn validate_ids<'a>(
    label: &str,
    ids: impl Iterator<Item = &'a CapabilityId>,
    catalog_ids: &BTreeSet<CapabilityId>,
) -> DiscoveryResult<()> {
    for id in ids {
        if !catalog_ids.contains(id) {
            return Err(DiscoveryError::InvalidInput(format!(
                "{label} `{id}` is absent from the catalog"
            )));
        }
    }
    Ok(())
}

fn validate_cost(kind: &str, cost: f64) -> DiscoveryResult<()> {
    if !cost.is_finite() || cost < 0.0 {
        return Err(DiscoveryError::InvalidInput(format!(
            "relationship cost for `{kind}` must be finite and nonnegative"
        )));
    }
    Ok(())
}

fn is_reversible(kind: &str) -> bool {
    matches!(
        kind,
        "supports" | "produces_input_for" | "composes_with" | "alternative_to"
    )
}

fn is_invalidating(kind: &str) -> bool {
    matches!(
        kind,
        "invalid_when" | "invalidates" | "conflicts_with" | "incompatible_with"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: &str) -> CapabilityId {
        value.parse().unwrap()
    }

    fn edge(from: &str, to: &str, cost: f64) -> TraversalEdge {
        TraversalEdge {
            from: id(from),
            to: id(to),
            relation_kind: "supports".to_owned(),
            reverse: false,
            cost,
        }
    }

    #[test]
    fn design_invalid_when_relation_is_not_traversable() {
        assert!(is_invalidating("invalid_when"));
    }

    #[test]
    fn bounded_path_retains_shallow_route_when_cheaper_route_exceeds_depth() {
        let start = id("amari:test:graph:start");
        let first = id("amari:test:graph:first");
        let second = id("amari:test:graph:second");
        let target = id("amari:test:graph:target");
        let visited = BTreeMap::from([
            (start.clone(), 0),
            (first, 1),
            (second, 2),
            (target.clone(), 1),
        ]);
        let edges = vec![
            edge("amari:test:graph:start", "amari:test:graph:target", 5.0),
            edge("amari:test:graph:start", "amari:test:graph:first", 1.0),
            edge("amari:test:graph:first", "amari:test:graph:second", 1.0),
            edge("amari:test:graph:second", "amari:test:graph:target", 1.0),
        ];

        let paths = shortest_paths(&visited, &[start], &edges, 2).unwrap();
        let target_path = paths.iter().find(|path| path.target == target).unwrap();

        assert_eq!(target_path.capabilities.len(), 2);
        assert_eq!(target_path.total_cost, 5.0);
    }
}
