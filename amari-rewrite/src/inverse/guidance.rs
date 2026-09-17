// SPDX-License-Identifier: MIT OR Apache-2.0

//! Transparent guidance for inverse search.
//!
//! Guidance never creates transitions: the symbolic engine is the
//! only transition generator. A scorer receives already valid
//! candidates and orders (or, in explicit heuristic mode, prunes)
//! them. The 0.25 scorer is the transparent symbolic cost only.

use alloc::vec::Vec;

use crate::error::{RewriteError, RewriteResult};
use crate::inverse::state::SymbolicState;
use crate::inverse::SymbolicPredecessor;
use crate::relation::Sha256Digest;

/// Transparent minimization dimensions for a candidate state.
///
/// Ordering is lexicographic in declaration order: depth, term
/// growth, constraint growth, unresolved existentials, residual
/// cost, branch factor, cycle/novelty, then stable rule priority as
/// the final tiebreak. Every dimension is caller-inspectable; there
/// is no opaque score.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct SymbolicScore {
    /// Search depth of the candidate.
    pub depth: u64,
    /// Node-count change relative to the parent state.
    pub term_growth: i64,
    /// Constraint-count change relative to the parent state.
    pub constraint_growth: u64,
    /// Unresolved existential variables introduced by the step.
    pub unresolved_existentials: u64,
    /// Residual evidence cost (always 0 in 0.25: residuals are free).
    pub residual_cost: u64,
    /// Number of valid candidates in this expansion round.
    pub branch_factor: u64,
    /// True when the candidate's canonical state was already seen.
    pub cycle: bool,
    /// Stable rule priority: the rule's index in the system.
    pub rule_priority: u64,
}

/// How guidance may influence the search.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub enum GuidanceMode {
    /// Ordering only: no candidate is dropped before a resource
    /// limit, so outcomes keep full authority.
    CompleteWithinLimits,
    /// Explicit beam pruning: at most `beam_width` candidates are
    /// kept per expansion round. Dropped candidates are counted and
    /// reported. When the beam actually drops candidates the outcome
    /// is `Approximate` and can never prove exhaustion or
    /// unreachability; a pruning run that drops NOTHING enumerated
    /// everything the beam admitted, so it retains full authority
    /// (`Witness`/`Exhausted`) — the beam made no approximation.
    HeuristicPruning { beam_width: u64 },
}

impl GuidanceMode {
    /// Validate a mode against the fixed ceilings. Beam width zero
    /// or above the state ceiling is a typed error.
    pub fn validate(&self, config: &crate::inverse::InverseSearchConfig) -> RewriteResult<()> {
        match self {
            Self::CompleteWithinLimits => Ok(()),
            Self::HeuristicPruning { beam_width } => {
                if *beam_width == 0 || *beam_width > crate::inverse::InverseSearchConfig::MAX_STATES
                {
                    return Err(RewriteError::InvalidLimit {
                        resource: "guidance beam width",
                        value: *beam_width as usize,
                        ceiling: crate::inverse::InverseSearchConfig::MAX_STATES as usize,
                    });
                }
                let _ = config;
                Ok(())
            }
        }
    }
}

/// Digest binding a search to its guidance mode.
pub fn guidance_hash(mode: &GuidanceMode) -> Sha256Digest {
    let mut encoding = Vec::new();
    match mode {
        GuidanceMode::CompleteWithinLimits => encoding.push(0x00),
        GuidanceMode::HeuristicPruning { beam_width } => {
            encoding.push(0x01);
            encoding.extend_from_slice(&beam_width.to_le_bytes());
        }
    }
    Sha256Digest::framed("amari.inverse.guidance/v1", &encoding)
}

/// Digest of the 0.25 transparent symbolic scorer identity.
pub fn scorer_hash() -> Sha256Digest {
    Sha256Digest::framed("amari.inverse.scorer/symbolic-v1", &[])
}

/// Score a valid backward candidate. Pure and total: same inputs,
/// same score.
pub fn score_candidate(
    parent: &SymbolicState,
    child: &SymbolicState,
    predecessor: &SymbolicPredecessor,
    depth: u64,
    branch_factor: u64,
    cycle: bool,
    rule_priority: u64,
) -> SymbolicScore {
    let parent_nodes = parent.term().positions().len() as i64;
    let child_nodes = child.term().positions().len() as i64;
    SymbolicScore {
        depth,
        term_growth: child_nodes - parent_nodes,
        constraint_growth: child
            .constraints()
            .len()
            .saturating_sub(parent.constraints().len()) as u64,
        unresolved_existentials: predecessor.existentials.len() as u64,
        residual_cost: 0,
        branch_factor,
        cycle,
        rule_priority,
    }
}
