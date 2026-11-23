// SPDX-License-Identifier: MIT OR Apache-2.0

//! Validated ceilings for the geometric inverse guidance surface.
//!
//! Every ceiling here is a hard maximum. A caller may tighten one through a
//! validated constructor, and may never widen one: limits bound work, they do
//! not disable or expand it (cohort doctrine). Admission is checked *before*
//! anything proportional to the admitted set is allocated — the graph's node
//! and edge capacity, a descriptor's walk, and a ranking round's candidate
//! list all use these values rather than the request's own size.

use crate::relation::RelationLimits;

/// Hard ceiling on the nodes of one inverse search graph.
///
/// The graph is a bounded witness structure, not a search: it holds at most
/// this many symbolic states, and a graph asked for one more rejects the
/// insertion instead of growing.
pub const MAX_GRAPH_NODES: usize = 4_096;

/// Hard ceiling on the directed edges of one inverse search graph.
pub const MAX_GRAPH_EDGES: usize = 16_384;

/// Hard ceiling on the candidates one geometric ranking round may admit.
///
/// Scoring a round costs one bounded descriptor per candidate plus one
/// nearest-node scan, so the round's cost is this ceiling times the graph's
/// node ceiling.
pub const MAX_GEOMETRIC_CANDIDATES: usize = 256;

/// Hard ceiling on a descriptor distance before normalization.
///
/// The frozen descriptor's components are seven values in `[0, 1]` and one in
/// `[-1, 1]`, so the largest possible distance is `sqrt(11) ≈ 3.317`: this
/// ceiling sits above the descriptor ball's diameter, which keeps the score's
/// distance lane in `[0, 1]` with headroom, makes the lane finite for every
/// input, and keeps scores comparable across rounds. It is a normalization
/// ceiling, never a correctness claim.
pub const MAX_EMBEDDING_DISTANCE: f64 = 8.0;

/// Typed rejection from the geometric network surface.
///
/// Every variant is a static resource name, a count, or a ceiling: no path, no
/// term text, and no backend error value is carried, so a rejection cannot
/// become an allocation channel or a content oracle.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum NetworkError {
    /// A ceiling rejected the request before any work was done.
    #[error("network limit exceeded: {resource} (value {value}, ceiling {limit})")]
    LimitExceeded {
        /// The rejected resource.
        resource: &'static str,
        /// The offending value.
        value: usize,
        /// The applied ceiling.
        limit: usize,
    },
    /// The metered descriptor budget was exhausted.
    #[error("network budget exhausted: {resource} (limit {limit})")]
    BudgetExhausted {
        /// The exhausted resource.
        resource: &'static str,
        /// The configured limit.
        limit: usize,
    },
    /// A node index was not in the graph.
    #[error("network node index {index} is out of range ({nodes} nodes)")]
    UnknownNode {
        /// The rejected index.
        index: usize,
        /// The graph's node count.
        nodes: usize,
    },
}

/// Validated ceilings for one inverse search graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InverseGraphConfig {
    max_nodes: usize,
    max_edges: usize,
    limits: RelationLimits,
}

impl InverseGraphConfig {
    /// Tighten the graph's ceilings.
    ///
    /// `limits` bounds every descriptor walk (term nodes, term depth,
    /// constraints, operations), so a graph can never encode a state its own
    /// limits would not admit.
    ///
    /// # Errors
    ///
    /// [`NetworkError::LimitExceeded`] when `max_nodes` or `max_edges` is zero
    /// or above its hard ceiling.
    pub fn new(
        max_nodes: usize,
        max_edges: usize,
        limits: RelationLimits,
    ) -> Result<Self, NetworkError> {
        check("graph nodes", max_nodes, MAX_GRAPH_NODES)?;
        check("graph edges", max_edges, MAX_GRAPH_EDGES)?;
        Ok(Self {
            max_nodes,
            max_edges,
            limits,
        })
    }

    /// The admitted node count.
    pub fn max_nodes(&self) -> usize {
        self.max_nodes
    }

    /// The admitted edge count.
    pub fn max_edges(&self) -> usize {
        self.max_edges
    }

    /// The descriptor walk's limits.
    pub fn limits(&self) -> &RelationLimits {
        &self.limits
    }
}

impl Default for InverseGraphConfig {
    fn default() -> Self {
        Self {
            max_nodes: MAX_GRAPH_NODES,
            max_edges: MAX_GRAPH_EDGES,
            limits: RelationLimits::default(),
        }
    }
}

/// Validated ceilings for one geometric ranking round.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeometricStrategyConfig {
    max_candidates: usize,
}

impl GeometricStrategyConfig {
    /// The default round ceiling: [`MAX_GEOMETRIC_CANDIDATES`].
    pub const DEFAULT_MAX_CANDIDATES: usize = MAX_GEOMETRIC_CANDIDATES;

    /// Tighten the round's candidate admission.
    ///
    /// # Errors
    ///
    /// [`NetworkError::LimitExceeded`] when `max_candidates` is zero or above
    /// [`MAX_GEOMETRIC_CANDIDATES`].
    pub fn new(max_candidates: usize) -> Result<Self, NetworkError> {
        check(
            "geometric strategy candidates",
            max_candidates,
            MAX_GEOMETRIC_CANDIDATES,
        )?;
        Ok(Self { max_candidates })
    }

    /// The admitted candidates per round.
    pub fn max_candidates(&self) -> usize {
        self.max_candidates
    }
}

impl Default for GeometricStrategyConfig {
    fn default() -> Self {
        Self {
            max_candidates: Self::DEFAULT_MAX_CANDIDATES,
        }
    }
}

/// Reject a zero or above-ceiling value.
fn check(resource: &'static str, value: usize, limit: usize) -> Result<(), NetworkError> {
    if value == 0 || value > limit {
        return Err(NetworkError::LimitExceeded {
            resource,
            value,
            limit,
        });
    }
    Ok(())
}
