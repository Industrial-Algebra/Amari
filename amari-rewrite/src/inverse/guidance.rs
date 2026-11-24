// SPDX-License-Identifier: MIT OR Apache-2.0

//! Transparent guidance for inverse search.
//!
//! Guidance never creates transitions: the symbolic engine is the
//! only transition generator. A scorer receives already valid
//! candidates and orders (or, in explicit heuristic mode, prunes)
//! them. The 0.25 baseline scorer is the transparent symbolic cost.
//!
//! ## Candidate strategies (Task 44)
//!
//! A [`GuidanceStrategy`] is the seam a learned or geometric scalar plugs
//! into. It is a *ranking* seam, not a search seam:
//!
//! - the engine produces the round's candidates ([`GuidanceCandidate`]s,
//!   each an engine-built state plus the transition that produced it);
//! - the strategy returns indices, i.e. a permutation of the candidates it
//!   was given. [`validate_ranking`] enforces that it is a permutation — a
//!   strategy can neither add, remove, validate, nor invalidate a candidate,
//!   and a ranking that is not a permutation is a typed failure, never a
//!   silently accepted partial order;
//! - the guidance *mode* still owns pruning ([`GuidanceMode::HeuristicPruning`]
//!   truncates the strategy's order and counts what it dropped), so
//!   ordering authority and completeness authority stay separate;
//! - complete mode consumes the same permutation and drops nothing, so the
//!   invariant is *per-round candidate preservation*: a strategy never adds,
//!   removes, or alters a candidate within a round, it only permutes it;
//! - what follows from that invariant is closure-relative parity, not
//!   unconditional parity. A fully **exhausted** closure admits every candidate
//!   of every round, so its explored set — and therefore its
//!   exhaustion/replay hash — is identical with and without a strategy, and
//!   only the visit order differs. A run cut short by a resource limit is
//!   different: the order decides which candidates are admitted before the
//!   budget runs out, so a budget-truncated run has an **order-dependent
//!   explored set**. Both runs remain `Partial` — ordering can change what a
//!   cut-off search saw, never whether it may claim a certificate.
//!
//! The strategy's identity enters replay identity through the scorer hash
//! ([`scorer_hash_with`]): [`scorer_hash`] alone is the transparent symbolic
//! scorer, and a strategy-composed digest names both the symbolic scorer and
//! the strategy. Guidance-mode hashes are unchanged, because the mode (not
//! the strategy) is what the guidance hash binds.

use alloc::vec::Vec;
use core::fmt;

use crate::error::{RewriteError, RewriteResult};
use crate::inverse::outcome::BackwardSearchOutcome;
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
///
/// This is the identity of a search whose only scorer is the symbolic cost,
/// i.e. exactly what [`scorer_hash_with`]`(None)` returns.
pub fn scorer_hash() -> Sha256Digest {
    Sha256Digest::framed("amari.inverse.scorer/symbolic-v1", &[])
}

/// Digest of the scorer identity that actually ranked a search.
///
/// `None` is bit-for-bit [`scorer_hash`], so an unguided search's replay
/// identity is unchanged by the strategy seam. `Some(strategy)` folds the
/// symbolic scorer and the strategy's own identity into one composite digest
/// (both length-framed inside the payload), which is what a strategy-guided
/// outcome records: the same symbolic engine under a different checkpoint must
/// not replay as the same scorer.
pub fn scorer_hash_with(strategy: Option<&Sha256Digest>) -> Sha256Digest {
    let Some(strategy) = strategy else {
        return scorer_hash();
    };
    let mut encoding = Vec::with_capacity(64);
    encoding.extend_from_slice(scorer_hash().as_bytes());
    encoding.extend_from_slice(strategy.as_bytes());
    Sha256Digest::framed("amari.inverse.scorer/composite-v1", &encoding)
}

/// One already-valid candidate offered to a guidance strategy.
///
/// Every field is the symbolic engine's own output: the state, the transition
/// that produced it (with provenance), and the transparent symbolic score the
/// round was ranked by. A strategy may reorder these and nothing else.
#[derive(Clone, Copy, Debug)]
pub struct GuidanceCandidate<'a> {
    /// The candidate's alpha-canonical child state.
    pub state: &'a SymbolicState,
    /// The engine transition that produced the candidate.
    pub predecessor: &'a SymbolicPredecessor,
    /// The transparent symbolic score for this round.
    pub score: &'a SymbolicScore,
}

/// Everything a guidance strategy may inspect for one expansion round.
///
/// The request borrows the engine's round: building a request clones nothing
/// and allocates nothing, so a strategy call is bounded by what the search
/// already holds.
#[derive(Clone, Copy, Debug)]
pub struct GuidanceRequest<'a> {
    /// The state being expanded.
    pub parent: &'a SymbolicState,
    /// Backward depth of the parent (the candidates are one deeper).
    pub depth: u64,
    /// The guidance mode the round runs under. A strategy orders only; the
    /// mode decides whether the order is truncated.
    pub mode: &'a GuidanceMode,
    /// The round's candidates, in the engine's own order.
    pub candidates: &'a [GuidanceCandidate<'a>],
}

/// Which part of the strategy seam produced a [`GuidanceFailure`].
///
/// Every variant is a static rule name, a ceiling, or a stage: no path, no
/// artifact text, and no backend error value is ever carried, so a failure
/// diagnostic cannot become an allocation channel or a path oracle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuidanceFailureKind {
    /// A ceiling rejected the round before it was scored.
    LimitExceeded {
        /// The rejected resource.
        resource: &'static str,
        /// The offending value.
        value: usize,
        /// The applied ceiling.
        limit: usize,
    },
    /// A metered budget ran out while scoring the round.
    BudgetExhausted {
        /// The exhausted resource.
        resource: &'static str,
        /// The configured limit.
        limit: usize,
    },
    /// The strategy's configuration failed validation.
    InvalidConfig,
    /// The strategy's parameters failed validation.
    InvalidParameters,
    /// A feature vector failed validation.
    InvalidFeatures,
    /// The numeric backend rejected a stage. The stage is named; the
    /// backend's own error value is deliberately dropped.
    BackendRejected {
        /// The pipeline stage that was rejected.
        stage: &'static str,
    },
    /// The strategy produced a non-finite score.
    NonFiniteScore {
        /// The pipeline stage that produced it.
        stage: &'static str,
    },
    /// The strategy could not answer (a non-scoring surface failure).
    Unavailable {
        /// The surface that was unavailable.
        stage: &'static str,
    },
    /// The strategy's ranking was not a permutation of the candidates.
    MalformedRanking,
}

impl fmt::Display for GuidanceFailureKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LimitExceeded {
                resource,
                value,
                limit,
            } => write!(formatter, "{resource} limit {limit} (got {value})"),
            Self::BudgetExhausted { resource, limit } => {
                write!(formatter, "{resource} budget {limit}")
            }
            Self::InvalidConfig => formatter.write_str("configuration"),
            Self::InvalidParameters => formatter.write_str("parameters"),
            Self::InvalidFeatures => formatter.write_str("features"),
            Self::BackendRejected { stage } => write!(formatter, "backend stage {stage}"),
            Self::NonFiniteScore { stage } => write!(formatter, "non-finite score at {stage}"),
            Self::Unavailable { stage } => write!(formatter, "unavailable surface {stage}"),
            Self::MalformedRanking => formatter.write_str("ranking"),
        }
    }
}

/// A typed, sanitized, metered guidance-strategy failure.
///
/// The search turns this into [`GuidedSearchOutcome::StrategyFailed`]: it is
/// an outcome, never a panic and never a silent fallback to the symbolic
/// order (a reordered round that the strategy did not authorize would be a
/// silently different search).
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error(
    "guidance strategy failed at the {kind} stage after scoring {scored} of {candidates} \
     candidates"
)]
pub struct GuidanceFailure {
    /// What went wrong.
    pub kind: GuidanceFailureKind,
    /// Candidates in the round that failed (the meter's denominator).
    pub candidates: usize,
    /// Candidates fully scored before the failure (the meter's numerator).
    pub scored: usize,
}

impl GuidanceFailure {
    /// Build a failure from its kind and its round meter.
    pub fn new(kind: GuidanceFailureKind, candidates: usize, scored: usize) -> Self {
        Self {
            kind,
            candidates,
            scored,
        }
    }
}

/// A ranking seam over the symbolic engine's already-valid candidates.
///
/// Implementations score or order candidates; they never generate, validate,
/// or invalidate one. [`GuidanceStrategy::rank`] must return a permutation of
/// `request.candidates`' indices; [`validate_ranking`] is the search's
/// enforcement of that contract, and a violation is
/// [`GuidanceFailureKind::MalformedRanking`].
///
/// Determinism is the implementation's obligation: the same request and the
/// same strategy identity must yield the same permutation, with ties settled
/// by the candidates' canonical content (never by request order).
pub trait GuidanceStrategy {
    /// This strategy's identity, folded into replay identity through
    /// [`scorer_hash_with`]. Two different checkpoints must not share it.
    fn strategy_hash(&self) -> Sha256Digest;

    /// Order one round's candidates: a permutation of their indices, most
    /// preferred first.
    ///
    /// # Errors
    ///
    /// [`GuidanceFailure`] when the round cannot be scored. The failure is
    /// metered (how many candidates were scored first) and sanitized.
    fn rank(&self, request: &GuidanceRequest<'_>) -> Result<Vec<usize>, GuidanceFailure>;
}

/// Typed outcome of a strategy-aware backward search.
///
/// A completed search is the ordinary [`BackwardSearchOutcome`]; a strategy
/// that could not rank a round is reported as itself, so the caller can never
/// mistake a failed strategy for a symbolic result.
#[derive(Clone, Debug, PartialEq)]
pub enum GuidedSearchOutcome {
    /// The symbolic search ran to one of its settled outcomes.
    Completed(BackwardSearchOutcome),
    /// A guidance strategy failed mid-search; no search result exists.
    StrategyFailed(GuidanceFailure),
}

/// Enforce the strategy contract: `order` must be a permutation of
/// `0..candidates`.
///
/// This is the only validation a strategy's output receives — the candidates
/// themselves are the engine's and are never re-validated — and it is what
/// makes "pruning authority belongs to the mode" enforceable: a strategy that
/// returns a shortened, duplicated, or out-of-range order is rejected instead
/// of silently dropping candidates.
///
/// # Errors
///
/// [`GuidanceFailureKind::MalformedRanking`] for a length mismatch, a
/// duplicate index, or an out-of-range index. The meter records the round's
/// size and a full count, because a malformed ranking claims the whole round.
pub fn validate_ranking(order: &[usize], candidates: usize) -> Result<(), GuidanceFailure> {
    let malformed = |scored: usize| {
        Err(GuidanceFailure::new(
            GuidanceFailureKind::MalformedRanking,
            candidates,
            scored,
        ))
    };
    if order.len() != candidates {
        return malformed(0);
    }
    // A byte per candidate is the cheapest honest duplicate check: sorting a
    // copy of the indices would allocate `size_of::<usize>()` per candidate
    // from search state.
    let mut seen = alloc::vec![false; candidates];
    for index in order {
        let Some(slot) = seen.get_mut(*index) else {
            return malformed(0);
        };
        if *slot {
            return malformed(0);
        }
        *slot = true;
    }
    Ok(())
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

/// Engine-controlled witness minting (PR #382 round 2, F4).
///
/// [`InverseSearchGraph`](crate::network::InverseSearchGraph) deliberately has
/// no term system, so it cannot decide what the engine validated — and a
/// graph-side constructor that trusted a caller-assembled [`GuidanceCandidate`]
/// could not either, because every field of a guidance record is public. The
/// validating boundary therefore lives here, on the engine-owning inverse side,
/// and it is the only way to obtain a
/// [`TransitionWitness`](crate::network::TransitionWitness).
#[cfg(feature = "network")]
impl crate::network::TransitionWitness {
    /// Validate one engine transition and mint its witness.
    ///
    /// The engine is asked again: `parent`'s predecessor round is re-derived
    /// with [`crate::inverse::symbolic_predecessors`] at the freshening scope
    /// the candidate's own provenance records, and a witness is minted only
    /// when the engine derives that exact transition — same rule provenance,
    /// same freshened term, same existentials, same substitution — and the
    /// candidate's canonical state is the state that predecessor yields.
    ///
    /// Consequences, enforced here rather than documented as a caller
    /// obligation:
    ///
    /// - a genuine predecessor for one parent cannot mint a witness for an
    ///   unrelated parent, because the engine derives nothing for it;
    /// - a substituted candidate state cannot mint one, because the candidate
    ///   state's digest is re-derived from the predecessor, never taken on
    ///   trust;
    /// - a caller-assembled [`GuidanceRequest`] cannot mint one, so
    ///   [`observe_round`](crate::network::InverseSearchGraph::observe_round)
    ///   cannot record an edge from a fabricated round.
    ///
    /// Validation is deterministic and side-effect free: the same
    /// `(system, parent, candidate, limits)` either validates every time or
    /// never does, and nothing is admitted anywhere on the way.
    ///
    /// `limits` bounds the re-derivation. Passing the limits the round itself
    /// ran under reproduces it exactly; tighter limits can only fail closed.
    ///
    /// The checked step is the engine's own term-level backward step, which is
    /// what `symbolic_predecessors` derives; the witness additionally binds the
    /// parent's *full* canonical state digest as supplied, which is the identity
    /// the graph keys nodes on.
    ///
    /// # Errors
    ///
    /// [`NetworkError::UnwitnessedTransition`](crate::network::NetworkError::UnwitnessedTransition)
    /// when the engine does not derive the candidate's transition — including
    /// when the re-derivation itself exceeds `limits`, since a transition the
    /// engine cannot re-derive is not witnessed. The rejection carries no
    /// endpoint, digest, or term.
    #[cfg(feature = "network")]
    pub fn validate(
        system: &crate::trs::TermSystem,
        parent: &SymbolicState,
        candidate: &GuidanceCandidate<'_>,
        limits: &crate::relation::RelationLimits,
    ) -> Result<Self, crate::network::NetworkError> {
        let predecessor = candidate.predecessor;
        // The scope is bound to the record, not supplied by the caller: a
        // witness cannot re-scope a transition the engine derived elsewhere.
        let scope = predecessor.provenance.scope;
        let derived = crate::inverse::symbolic_predecessors(system, parent.term(), limits, scope)
            .map_err(|_| crate::network::NetworkError::UnwitnessedTransition)?;
        let mut validated = None;
        for entry in &derived {
            // Transition content, never the resource snapshot: the same round
            // need not be charged identically under different limits.
            if entry.term != predecessor.term
                || entry.existentials != predecessor.existentials
                || entry.constraints != predecessor.constraints
                || entry.substitution != predecessor.substitution
                || entry.provenance != predecessor.provenance
            {
                continue;
            }
            // The candidate's state is re-derived, not trusted.
            let state = SymbolicState::new(entry.term.clone(), entry.constraints.clone());
            if state.canonical_digest() != candidate.state.canonical_digest() {
                continue;
            }
            validated = Some(entry);
            break;
        }
        let entry = validated.ok_or(crate::network::NetworkError::UnwitnessedTransition)?;
        Ok(Self::mint(
            parent.canonical_digest(),
            candidate.state.canonical_digest(),
            crate::network::provenance_digest(entry),
        ))
    }
}
