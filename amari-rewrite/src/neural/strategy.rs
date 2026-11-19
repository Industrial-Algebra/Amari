// SPDX-License-Identifier: MIT OR Apache-2.0

//! The neural candidate strategy: learned ordering over the symbolic engine's
//! already-valid inverse transitions (Task 44).
//!
//! [`NeuralStrategy`] is the only bridge between the Task 40 ranker and the
//! Task 16 guidance seam, and it is a *ranking* bridge:
//!
//! - the symbolic engine is the only transition generator; the strategy
//!   receives each expansion round's engine-built candidates, encodes them,
//!   scores them, and returns a permutation of their indices. It has no access
//!   to the term system, so it cannot generate, validate, or invalidate a
//!   candidate;
//! - the guidance *mode* still owns pruning
//!   ([`GuidanceMode::HeuristicPruning`] truncates the returned order and
//!   counts what it dropped). The strategy never drops anything, so
//!   completeness authority stays where the Task 34 tri-state doctrine put it;
//! - in complete mode the same permutation is consumed in full, so the explored
//!   set — and the certified exhaustion/replay hash — is identical with and
//!   without the strategy. Only visit order changes, which is how the strategy
//!   "steers" without claiming authority.
//!
//! ## Determinism
//!
//! A round's ranking is a function of `(schema, weights, spec, candidates)`
//! alone:
//!
//! - the feature schema digest is recorded at construction and must match
//!   [`FeatureSchemaV1`] — a declared digest that does not match is a typed
//!   [`StrategyError::FeatureSchemaMismatch`], never a silent
//!   reinterpretation. (A Task 43 checkpoint records exactly this digest in
//!   `META_FEATURE_SCHEMA_HASH`, so a checkpoint's recorded value is what a
//!   caller declares here; the checkpoint loader has already rejected a
//!   mismatched artifact before any ranker exists.);
//! - candidates are encoded through [`FeatureEncoder`], whose lanes are exact
//!   IEEE-754 arithmetic, and scored one row at a time. Task 40's contract
//!   makes a row of an `n`-row batch bit-identical to the same row scored
//!   alone, so the single-row path here is the batched path's arithmetic;
//! - the returned order is `(score ascending, content key ascending)`, the
//!   same total order [`MlpRanker::rank`] produces. The content key is Task
//!   40's: it covers the canonical state digest, provenance, and search
//!   context, so ties are settled by content and never by request order.
//!
//! ## Failure is an outcome, not a reorder
//!
//! An encoder or ranker failure mid-search becomes a
//! [`GuidanceFailure`]: typed, sanitized (a stage or a rule name, never a
//! backend value or a path), and metered (how many of the round's candidates
//! were scored before it failed). The search reports it as
//! [`GuidedSearchOutcome::StrategyFailed`]; there is no fallback to the
//! symbolic order, because a round ordered by a strategy that did not answer is
//! a different search wearing the same name.
//!
//! ## Bounded admission
//!
//! The round's candidate set is admitted against
//! [`NeuralStrategyConfig::max_candidates`] *before* anything is encoded or
//! scored, and that ceiling can only be lowered: it is clamped by
//! [`MAX_RANKED_CANDIDATES`], Task 40's batch ceiling. Nothing here allocates
//! from search state beyond one entry per admitted candidate.

use crate::inverse::{
    GuidanceCandidate, GuidanceFailure, GuidanceFailureKind, GuidanceRequest, GuidanceStrategy,
    SymbolicScore,
};
use crate::neural::encode::{CandidateInput, FeatureEncoder, FeatureSchemaV1, SearchContext};
use crate::neural::model::{canonical_digest, content_key};
use crate::neural::{MlpRanker, NeuralError, MAX_RANKED_CANDIDATES};
use crate::relation::Sha256Digest;

/// A validated bound on one ranking round.
///
/// The default is Task 40's batch ceiling; a caller may tighten it, never widen
/// it (the fixed ceiling is the ranker's, and a strategy may not claim more
/// admission than the scorer can serve).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeuralStrategyConfig {
    max_candidates: usize,
}

impl NeuralStrategyConfig {
    /// The default ceiling: one ranking round may not exceed
    /// [`MAX_RANKED_CANDIDATES`].
    pub const DEFAULT_MAX_CANDIDATES: usize = MAX_RANKED_CANDIDATES;

    /// Validate a candidate ceiling. Zero or above the fixed ceiling is a typed
    /// error: limits bound work, they never disable or expand it.
    ///
    /// # Errors
    ///
    /// [`StrategyError::LimitExceeded`] when `max_candidates` is zero or above
    /// [`MAX_RANKED_CANDIDATES`].
    pub fn new(max_candidates: usize) -> Result<Self, StrategyError> {
        if max_candidates == 0 || max_candidates > MAX_RANKED_CANDIDATES {
            return Err(StrategyError::LimitExceeded {
                resource: "neural strategy candidates",
                value: max_candidates,
                limit: MAX_RANKED_CANDIDATES,
            });
        }
        Ok(Self { max_candidates })
    }

    /// The admitted candidates per round.
    pub fn max_candidates(&self) -> usize {
        self.max_candidates
    }
}

impl Default for NeuralStrategyConfig {
    fn default() -> Self {
        Self {
            max_candidates: Self::DEFAULT_MAX_CANDIDATES,
        }
    }
}

/// Typed rejection of a strategy's construction inputs.
///
/// Both variants are static-or-digest: nothing here carries a path, a
/// checkpoint's text, or a backend error value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum StrategyError {
    /// The ranker declares a feature schema this crate does not implement, so
    /// its features and its weights no longer mean the same thing.
    #[error(
        "neural strategy was declared for feature schema {recorded}, but this crate's frozen \
         schema is {expected}"
    )]
    FeatureSchemaMismatch {
        /// This crate's frozen schema digest ([`FeatureSchemaV1`]).
        expected: Sha256Digest,
        /// The digest the caller/checkpoint declared.
        recorded: Sha256Digest,
    },
    /// A strategy ceiling was zero or above its fixed maximum.
    #[error("invalid neural strategy limit: {resource} (value {value}, ceiling {limit})")]
    LimitExceeded {
        /// The rejected resource.
        resource: &'static str,
        /// The offending value.
        value: usize,
        /// The fixed ceiling.
        limit: usize,
    },
}

/// A validated ranker plus the frozen schema it was built for: the neural
/// ranking adapter over already-valid inverse candidates.
///
/// Construction is the schema gate ([`Self::new`]); [`Self::from_ranker`] is
/// the accept-the-frozen-schema convenience. Holding a `NeuralStrategy` means
/// "these weights were validated for exactly this feature schema", and
/// [`Self::schema_digest`] is the recorded digest a replay certificate or an
/// outcome can be bound to.
#[derive(Clone, Debug)]
pub struct NeuralStrategy {
    ranker: MlpRanker,
    encoder: FeatureEncoder,
    schema: Sha256Digest,
    config: NeuralStrategyConfig,
}

impl NeuralStrategy {
    /// Digest frame for [`Self::replay_hash`].
    pub const FRAME: &'static str = "amari.neural.strategy/v1";

    /// Bind a validated ranker to the candidate strategy.
    ///
    /// `declared_schema` is the feature-schema digest the ranker was trained
    /// and checkpointed against (a Task 43 artifact records
    /// [`FeatureSchemaV1::digest`] in `META_FEATURE_SCHEMA_HASH`).
    ///
    /// # Errors
    ///
    /// [`StrategyError::FeatureSchemaMismatch`] when `declared_schema` is not
    /// this crate's frozen schema digest.
    pub fn new(
        ranker: MlpRanker,
        encoder: FeatureEncoder,
        declared_schema: Sha256Digest,
        config: NeuralStrategyConfig,
    ) -> Result<Self, StrategyError> {
        let expected = FeatureSchemaV1.digest();
        if declared_schema != expected {
            return Err(StrategyError::FeatureSchemaMismatch {
                expected,
                recorded: declared_schema,
            });
        }
        Ok(Self {
            ranker,
            encoder,
            schema: expected,
            config,
        })
    }

    /// Bind a ranker that was validated against the frozen schema directly.
    pub fn from_ranker(ranker: MlpRanker, encoder: FeatureEncoder) -> Self {
        Self {
            ranker,
            encoder,
            schema: FeatureSchemaV1.digest(),
            config: NeuralStrategyConfig::default(),
        }
    }

    /// The ranker this strategy scores with.
    pub fn ranker(&self) -> &MlpRanker {
        &self.ranker
    }

    /// The admission gate every candidate is encoded through.
    pub fn encoder(&self) -> &FeatureEncoder {
        &self.encoder
    }

    /// The recorded (and validated) feature-schema digest.
    pub fn schema_digest(&self) -> Sha256Digest {
        self.schema
    }

    /// The validated round ceiling.
    pub fn config(&self) -> &NeuralStrategyConfig {
        &self.config
    }

    /// This strategy's replay identity: the frozen schema digest, the ranker's
    /// canonical spec and weight digests, the encoder's limits, and the round
    /// ceiling.
    ///
    /// It is the value [`GuidanceStrategy::strategy_hash`] returns, and the
    /// value that is folded into [`crate::inverse::scorer_hash_with`]. Two
    /// checkpoints never share it, and a changed encoder limit or round ceiling
    /// is a changed strategy — because either one can change the ranking.
    pub fn replay_hash(&self) -> Sha256Digest {
        let mut payload = Vec::with_capacity(160);
        payload.extend_from_slice(self.schema.as_bytes());
        payload.extend_from_slice(self.ranker.config().digest().as_bytes());
        payload.extend_from_slice(self.ranker.weights().digest().as_bytes());
        let limits = self.encoder.limits();
        for limit in [
            limits.max_term_nodes(),
            limits.max_term_depth(),
            limits.max_constraints(),
            limits.max_operations(),
        ] {
            payload.extend_from_slice(&(limit as u64).to_le_bytes());
        }
        payload.extend_from_slice(&(self.config.max_candidates() as u64).to_le_bytes());
        Sha256Digest::framed(Self::FRAME, &payload)
    }

    /// The frozen schema's search context for one candidate.
    ///
    /// The quantities come from the round's [`SymbolicScore`] — which
    /// [`crate::inverse`] computed for the same candidates — so encoding can
    /// never disagree with the symbolic scorer about depth, growth, cycles, or
    /// rule priority.
    fn context(candidate: &GuidanceCandidate<'_>) -> SearchContext {
        let score: &SymbolicScore = candidate.score;
        SearchContext::new(
            score.depth,
            score.branch_factor,
            score.cycle,
            score.term_growth,
            score.constraint_growth,
            candidate.predecessor.existentials.len(),
            score.rule_priority,
        )
    }
}

impl GuidanceStrategy for NeuralStrategy {
    fn strategy_hash(&self) -> Sha256Digest {
        self.replay_hash()
    }

    fn rank(&self, request: &GuidanceRequest<'_>) -> Result<Vec<usize>, GuidanceFailure> {
        let candidates = request.candidates;
        let total = candidates.len();
        // Admission precedes everything: no encoding, no digest, and no
        // allocation proportional to a set that was never admitted.
        if total > self.config.max_candidates {
            return Err(GuidanceFailure::new(
                GuidanceFailureKind::LimitExceeded {
                    resource: "neural strategy candidates",
                    value: total,
                    limit: self.config.max_candidates,
                },
                total,
                0,
            ));
        }
        let mut scored: Vec<(usize, f32, Vec<u8>)> = Vec::with_capacity(total);
        for (index, candidate) in candidates.iter().enumerate() {
            let input = CandidateInput::new(
                candidate.state.term(),
                candidate.state.constraints(),
                &candidate.predecessor.provenance,
                Self::context(candidate),
            );
            // The encoder is the bounded, iterative admission gate and runs
            // before the digest, which clones the term (Task 40's order).
            let features = self
                .encoder
                .encode(&input)
                .map_err(|error| failure(&error, total, index))?;
            let digest = canonical_digest(&input);
            let key = content_key(&input, digest);
            let score = self
                .ranker
                .score_features(&features)
                .map_err(|error| failure(&error, total, index))?;
            scored.push((index, score, key));
        }
        // The same total order `MlpRanker::rank` produces: ascending score,
        // then the canonical content key. `sort_by` is stable, so two
        // candidates with identical content keep the engine's order (they are
        // interchangeable: every input the ranker observed is equal).
        scored.sort_by(|left, right| {
            left.1
                .total_cmp(&right.1)
                .then_with(|| left.2.cmp(&right.2))
        });
        Ok(scored.into_iter().map(|(index, _, _)| index).collect())
    }
}

/// Translate one scoring-surface failure into the guidance seam's typed,
/// sanitized, metered failure.
///
/// The mapping is deliberately lossy in one direction only: message text,
/// tensor names, and backend errors are dropped, while every rule name, stage,
/// and ceiling that a caller can act on is kept.
fn failure(error: &NeuralError, candidates: usize, scored: usize) -> GuidanceFailure {
    let kind = match error {
        NeuralError::LimitExceeded {
            resource,
            value,
            limit,
        } => GuidanceFailureKind::LimitExceeded {
            resource,
            value: *value,
            limit: *limit,
        },
        NeuralError::BudgetExhausted { resource, limit } => GuidanceFailureKind::BudgetExhausted {
            resource,
            limit: *limit,
        },
        NeuralError::InvalidConfig { .. } => GuidanceFailureKind::InvalidConfig,
        NeuralError::InvalidWeights { .. } => GuidanceFailureKind::InvalidParameters,
        NeuralError::InvalidFeatures { .. } => GuidanceFailureKind::InvalidFeatures,
        NeuralError::BackendRejected { stage } => GuidanceFailureKind::BackendRejected { stage },
        NeuralError::NonFiniteScore { stage } => GuidanceFailureKind::NonFiniteScore { stage },
        // The trace variants belong to the replay/dataset surface and cannot
        // arise while scoring a guidance round. Reported as an unavailable
        // surface rather than mapped onto a scoring cause that did not happen.
        _ => GuidanceFailureKind::Unavailable {
            stage: "trace replay",
        },
    };
    GuidanceFailure::new(kind, candidates, scored)
}
