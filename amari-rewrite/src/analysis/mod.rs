// SPDX-License-Identifier: MIT OR Apache-2.0

//! Analysis infrastructure for rewrite systems.
//!
//! Currently holds the deterministic first-order unifier shared by
//! critical pairs, inverse frontier meeting, constraint solving, and
//! synthesis, plus the critical-pair generator, the bounded
//! local-confluence analysis, and the LPO termination checker.
//! Completion lands in a later cohort.

mod confluence;
mod critical_pairs;
mod inverse;
mod lpo;
mod unify;

pub use confluence::{local_confluence, ConfluenceReport, JoinabilityWitness, PairJoinability};
pub use critical_pairs::{critical_pairs, CriticalPair, CriticalPairReport};
pub use inverse::{
    BackwardKind, BranchingEstimate, InverseAnalyzer, InverseReport, ReversibilityClass,
    RuleAnalysis,
};
pub use lpo::{lpo_greater, prove_terminating, Precedence, TerminationProof, TerminationReport};

// The module and its primary entry point share the name `unify`;
// they live in different namespaces, so `analysis::unify(...)` calls
// the function while `analysis::unify::unify` remains the module path.
pub use self::unify::{unify, unify_with};
