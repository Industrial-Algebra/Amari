// SPDX-License-Identifier: MIT OR Apache-2.0

//! Geometric inverse search guidance: a frozen eight-blade state descriptor,
//! a bounded [`amari_network::GeometricNetwork`] over valid inverse
//! transitions, and a transparent ranking score over the engine's candidates.
//!
//! Rewrite systems naturally form directed graphs: terms are nodes and rewrite
//! steps are edges. This module is the 0.25 geometric surface, and it is a
//! *guidance* surface:
//!
//! - the symbolic engine is the only transition generator. [`InverseSearchGraph`]
//!   is built from states the caller already holds and from transitions the
//!   engine already validated; it has no access to a term system and no method
//!   that returns a term, a state, or a candidate, so it can reorder evidence
//!   and never produce it;
//! - node identity is the canonical state digest,
//!   [`crate::inverse::SymbolicState::canonical_digest`], and **never**
//!   embedding equality. [`InverseStateEmbeddingV1`] is a heuristic,
//!   non-isometric descriptor whose components deliberately collide; a
//!   colliding pair stays two nodes;
//! - ranking authority and completeness authority stay separate.
//!   [`GeometricStrategy`] returns a permutation of the round's candidates
//!   (enforced by [`crate::inverse::validate_ranking`]), and the guidance mode
//!   still owns pruning;
//! - every score is transparent: seven documented lanes in `[0, 1]` and a
//!   frozen weight table whose digest is pinned. Preferred is LOWER.
//!
//! ## What this module does not claim
//!
//! A geometric distance is **evidence, not geometry**: the descriptor is a
//! normalized summary of a state's shape and content sketch, so "closer in the
//! embedding" says the two states look alike, never that they are semantically
//! closer, that one precedes the other in a rewrite relation, or that a
//! candidate is nearer a solution. No claim on this surface is a correctness
//! claim, and an edge records a transition the engine already derived — it does
//! not propose one.
//!
//! ## Bounds
//!
//! [`InverseGraphConfig`] bounds the graph's nodes and edges as well as every
//! descriptor walk, and [`GeometricStrategyConfig`] bounds a ranking round's
//! candidate count. Every ceiling is validated at construction and admitted
//! before allocation; callers may tighten one and never widen it.
//!
//! ## Determinism
//!
//! Identical inputs produce identical descriptors, graph content, scores, and
//! rankings. The descriptor uses integer counts, integer bit-lengths, and
//! IEEE-754 multiply/divide (no transcendental and no hash iteration); the
//! nearest-node scan settles ties by index; and rounds sort by
//! `(total ascending, content key ascending)`, so a permutation of the request
//! permutes the indices but not the state order.

mod config;
mod graph;
mod strategy;

pub use config::{
    GeometricStrategyConfig, InverseGraphConfig, NetworkError, MAX_EMBEDDING_DISTANCE,
    MAX_GEOMETRIC_CANDIDATES, MAX_GRAPH_EDGES, MAX_GRAPH_NODES,
};
pub use graph::{
    EmbeddingComponent, GraphNodeInput, InverseSearchGraph, InverseStateEmbeddingV1,
    EMBEDDING_WIDTH,
};
pub use strategy::{
    GeometricScore, GeometricScoreTable, GeometricStrategy, ScoreLane, SCORE_LANES,
};

/// Summary of a rewrite trace as a graph path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RewriteGraphSummary {
    /// Number of terms in the trace.
    pub nodes: usize,
    /// Number of rewrite transitions in the trace.
    pub edges: usize,
}

impl RewriteGraphSummary {
    /// Summarize a linear rewrite trace.
    pub fn from_trace<T>(trace: &[T]) -> Self {
        Self {
            nodes: trace.len(),
            edges: trace.len().saturating_sub(1),
        }
    }
}

/// Marker proving this module was compiled with the optional `amari-network`
/// dependency available.
pub fn network_bridge_enabled() -> bool {
    let _ = core::mem::size_of::<Option<amari_network::NodeMetadata>>();
    true
}
