// SPDX-License-Identifier: MIT OR Apache-2.0

//! Deterministic capability planning primitives.
//!
//! Planning starts with bounded candidate retrieval from the embedded semantic
//! catalog. Later stages expand these candidates through capability relations,
//! rank trade-offs, and normalize replayable plans.

mod graph;
mod normalize;
mod plan;
mod rank;
mod recall;

pub use graph::{
    CapabilityGraphExpander, GraphConstraints, GraphExpansion, GraphExpansionState, GraphLimit,
    GraphLimits, GraphPath, GraphStep, RelationCostPolicy,
};
pub use normalize::{NormalizationLimits, PlanNormalizer};
pub(crate) use plan::catalog_plan_steps;
pub use plan::PlanGenerator;
pub use rank::{
    BlockedCandidate, CandidateRanker, RankedCandidate, RankingComponents, RankingContext,
    RankingProvenance, RankingResult, RankingSignal, RankingSignalKind, RANKING_OBJECTIVE_ORDER,
};
pub use recall::{CandidateRetriever, RecallConfig, RetrievalSource, RetrievedCandidate};
