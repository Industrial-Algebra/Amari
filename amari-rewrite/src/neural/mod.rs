// SPDX-License-Identifier: MIT OR Apache-2.0

//! Neural and differentiable rewrite traits, a frozen candidate feature
//! schema, and a bounded MLP candidate ranker.
//!
//! The module has two halves, and the split is load-bearing:
//!
//! - **Traits** ([`DifferentiableRule`]) stay framework-free. They are
//!   the seam every learned rule and ranker integrates through, so
//!   nothing in the rewrite architecture depends on a tensor backend.
//! - **The 0.25 inverse ranker** ([`FeatureEncoder`], [`MlpRanker`]) is a
//!   concrete, opt-in backend behind the non-default `neural` feature. It
//!   is built from a Katakuri `MlpSpec` (the architecture and the
//!   canonical parameter order) and computes on the `borsalino-tensor`
//!   CPU reference (`matmul` / `add`). Katakuri never computes, and
//!   neither does this module's architecture layer: the arithmetic is
//!   borsalino's, the identity is Katakuri's.
//!
//! ## What the ranker is allowed to do
//!
//! Guidance never creates transitions — the symbolic engine is the only
//! transition generator (see [`crate::inverse`]). The ranker therefore
//! only *orders already-valid candidates*: it consumes encoded
//! [`CandidateInput`]s, returns a score, and never prunes, synthesizes, or
//! certifies anything. Ordering authority belongs to the caller.
//!
//! ## Determinism
//!
//! Every score is a function of `(schema, weights, candidate)` alone:
//!
//! - the feature schema is frozen and versioned ([`FeatureSchemaV1`]
//!   carries a content digest, so any encoding change is visible),
//! - the encoder uses exact IEEE-754 arithmetic (add/sub/mul/div of counts
//!   and integer bit-lengths), never a transcendental, so lane values are
//!   bit-identical on every platform,
//! - borsalino-tensor's CPU reference accumulates each output element with
//!   a strict ascending `k` loop in f32, so a batch of `n` rows produces
//!   the same bits per row as scoring that row alone,
//! - the batch is canonicalized before scoring and the result is sorted by
//!   `(score, content key)`, where the key encodes the state digest and the
//!   candidate's provenance and search context, so the ranking is a
//!   function of content alone and no permutation of the request can change
//!   it.
//!
//! ## Admission before canonicalization
//!
//! [`FeatureEncoder::encode`] is the admission gate: it is bounded, fully
//! iterative, and runs *before* anything that clones or canonicalizes the
//! term. That order is contractual — the canonical digest clones the term,
//! and `Term::clone` recurses, so an over-large term must be rejected by
//! the encoder before the digest is derived (PR #353 round 1 P1a).
//!
//! ## Non-finite values are impossible by construction, and rejected anyway
//!
//! Features are ratios in `[0, 1]`. Weights are validated finite and
//! bounded by [`RankerConfig::max_weight_magnitude`], which composes the
//! architecture's widths into an activation bound: for weights and biases
//! bounded by `M`, each layer multiplies the previous bound by
//! `width * M` and adds `M`, so the derived bound is the largest `M` whose
//! composed bound stays inside a fixed safety headroom below `f32::MAX`.
//! It is a per-architecture bound, not a global constant — eight 256-wide
//! layers can carry far smaller weights than one (PR #353 round 1 P1b). The
//! encoder still refuses out-of-range lanes and the ranker still refuses a
//! non-finite score, because a bound argued for only in a comment is not a
//! contract.
//!
//! ## Errors
//!
//! [`NeuralError`] is the only error type the module exposes. Backend
//! failures are replaced by a static, stage-named message: no
//! `borsalino_tensor::TensorError`, no shape dump, and no backend op name
//! ever reaches a caller.

mod config;
mod data;
mod encode;
mod model;

/// Byte cap on caller-supplied text quoted into a diagnostic.
///
/// A rejection must never copy an unbounded input: quoting a 16 MB tensor
/// name or a 16 MB spec fragment verbatim would allocate that much to say
/// "rejected". Diagnostics therefore render a bounded excerpt — truncated
/// on a UTF-8 boundary and length-stamped — while the rejection itself is
/// carried by the typed error (PR #353 rounds 3 P2b and 4 P2a).
pub(crate) const MAX_DIAGNOSTIC_EXCERPT_BYTES: usize = 64;

/// Render caller-supplied text for a diagnostic: bounded and lossy beyond
/// [`MAX_DIAGNOSTIC_EXCERPT_BYTES`], always length-stamped when truncated.
pub(crate) fn bounded_excerpt(text: &str) -> String {
    if text.len() <= MAX_DIAGNOSTIC_EXCERPT_BYTES {
        return text.to_string();
    }
    let mut end = MAX_DIAGNOSTIC_EXCERPT_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}... ({} bytes)", &text[..end], text.len())
}

pub use config::RankerConfig;
pub use data::{
    Dataset, DatasetBuilder, DatasetConfig, DatasetManifest, LabelAuthority, PairwiseExample,
    DATASET_MANIFEST_SCHEMA_VERSION,
};
pub use encode::{
    CandidateInput, FeatureEncoder, FeatureSchemaV1, FeatureVector, Lane, SearchContext,
    CANDIDATE_FEATURE_WIDTH,
};
pub use model::{MlpRanker, RankedCandidate, RankerWeights, MAX_RANKED_CANDIDATES};

use thiserror::Error;

/// Errors returned by the neural feature and ranking surface.
///
/// Every variant is typed and local: the module never propagates a
/// `RewriteError`, a Katakuri error, or a borsalino-tensor error to its
/// caller, so guidance code never has to know which backend answered.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum NeuralError {
    /// A caller-tightened or fixed ceiling rejected the input.
    #[error("neural limit exceeded: {resource} (value {value}, ceiling {limit})")]
    LimitExceeded {
        /// The rejected resource.
        resource: &'static str,
        /// The offending value.
        value: usize,
        /// The applied ceiling.
        limit: usize,
    },
    /// The metered encoding budget was exhausted.
    #[error("neural encoding budget exhausted: {resource} (limit {limit})")]
    BudgetExhausted {
        /// The exhausted resource.
        resource: &'static str,
        /// The configured limit.
        limit: usize,
    },
    /// The ranker specification failed validation.
    #[error("invalid neural ranker configuration: {message}")]
    InvalidConfig {
        /// Validation failure detail.
        message: String,
    },
    /// The weight payload failed validation.
    #[error("invalid neural ranker weights: {message}")]
    InvalidWeights {
        /// Validation failure detail (tensor name and problem).
        message: String,
    },
    /// A feature vector failed validation (width, finiteness, range).
    #[error("invalid neural features: {message}")]
    InvalidFeatures {
        /// Validation failure detail.
        message: &'static str,
    },
    /// The tensor backend rejected a call. The stage is named; the
    /// backend's own error is deliberately dropped.
    #[error("tensor backend rejected the {stage} step")]
    BackendRejected {
        /// The forward-pass stage that was rejected.
        stage: &'static str,
    },
    /// A forward pass produced a non-finite score. Unreachable under the
    /// validated bounds; reported rather than returned.
    #[error("neural ranker produced a non-finite score at the {stage} step")]
    NonFiniteScore {
        /// The forward-pass stage that produced it.
        stage: &'static str,
    },
    /// A replayed trace contained no derivation steps, so nothing could be
    /// derived from it.
    #[error("trace produced no derivation steps")]
    EmptyTrace,
    /// A replayed trace did not verify against the supplied system.
    #[error("trace verification failed")]
    TraceVerificationFailed,
    /// The trace's chosen transition was not among the engine's re-derived
    /// valid candidates (trace/engine drift). A negative set is never
    /// fabricated from a drifted trace.
    #[error("trace drift: the chosen transition is not among the re-derived valid candidates")]
    TraceDrift,
    /// An approximate trace was rejected because the dataset configuration
    /// does not admit approximate evidence.
    #[error("approximate trace rejected: approximate admission is disabled")]
    ApproximateTraceRejected,
    /// Re-deriving a replayed state's candidates failed. Sanitized: no
    /// backend or relation detail is propagated.
    #[error("trace re-derivation failed")]
    TraceRederivation,
}

/// A differentiable or learned rewrite rule over a state representation.
///
/// Unchanged since 0.23.0 and still the seam every learned rule integrates
/// through: [`MlpRanker`] implements it for feature/score vectors, and
/// nothing in the trait depends on a tensor framework.
pub trait DifferentiableRule<State> {
    /// Rule parameters, if any.
    type Parameters;
    /// Gradient representation, if any.
    type Gradient;
    /// Rule evaluation error.
    type Error;

    /// Apply the rule forward.
    fn forward(&self, state: &State) -> Result<State, Self::Error>;

    /// Compute a scalar loss between a prediction and a target.
    fn loss(&self, predicted: &State, target: &State) -> Result<f64, Self::Error>;
}
