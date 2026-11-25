// SPDX-License-Identifier: MIT OR Apache-2.0

//! Hybrid inverse search guidance: one transparent combined order over the
//! symbolic, neural, geometric, and holographic-recall evidence lanes.
//!
//! The 0.25 backends each rank an expansion round in their own units and their
//! own direction. This module is the *only* place they are combined, and it is
//! a ranking bridge exactly like its parts: the symbolic engine builds the
//! round's candidates, [`HybridStrategy`] orders them, and the guidance mode
//! ([`crate::inverse::GuidanceMode`]) still owns pruning. The strategy never
//! generates, validates, invalidates, or drops a candidate.
//!
//! ## Score direction is explicit, and normalized before any combination
//!
//! | backend | native direction | native statistic |
//! |---|---|---|
//! | symbolic | LOWER is preferred | the candidate's position in the engine's lexicographic symbolic order |
//! | neural | LOWER is preferred | the ranker's score |
//! | geometric | LOWER is preferred | the seven-lane transparent total |
//! | recall | HIGHER is preferred | the maximum `combined` similarity of a remembered trace that contains the candidate's term |
//!
//! Raw lanes are never added together. Each *available* backend ranks the round
//! in its own native direction with its own content tie-break, and the hybrid
//! derives one **ordinal-fraction** normalization from that ranking:
//!
//! ```text
//! normalized[i] = position_of_i_in_that_backend's_order / (n - 1)   (0 when n = 1)
//! contribution  = weight * normalized
//! combined      = Σ contribution, summed in HybridBackend::ALL declaration order
//! ```
//!
//! The normalization therefore *is* the documented direction conversion: recall
//! prefers higher similarity, and a higher-similarity candidate receives a
//! **lower** ordinal, so `lower normalized is more preferred` holds for every
//! backend at once. Every normalized value is finite and inside `[0, 1]` for
//! every admitted input — there is no division that can produce `NaN` or an
//! infinity (the single-candidate denominator is guarded) — and no raw lane
//! value ever enters the sum, so units are never mixed. [`NORMALIZATION_CONVENTION`]
//! is the published spelling of this rule and [`normalization_hash`] pins it.
//!
//! ## Typed finite weights
//!
//! [`HybridWeights`] has private fields and one validating constructor: every
//! weight must be finite and inside the documented `[0, 1]` domain *before* any
//! search, allocation, or backend call. `NaN`, `±∞`, and out-of-domain values
//! are typed rejections ([`HybridConfigError::NonFiniteWeight`],
//! [`HybridConfigError::WeightOutOfRange`]); nothing is silently clamped and no
//! configured authority is widened. `-0.0` is a zero and is stored as `+0.0`, so
//! a contribution is never a negative zero.
//!
//! The zero and one extremes are pinned per backend and end to end:
//!
//! - weight `0`: the backend is still evaluated and still fully reported
//!   (availability, raw, normalized), but its contribution is exactly `+0.0`, so
//!   it has **no ordering effect**;
//! - weight `1`: for a single enabled backend the combined score *is* that
//!   backend's normalized score, so the hybrid's order is that backend's order
//!   exactly;
//! - **all weights zero** is defined, not undefined: every contribution is
//!   `+0.0`, every combined score is `+0.0`, and the round is ordered by the
//!   canonical content key alone. That is a pure, documented tie-break, never a
//!   hidden fallback score.
//!
//! ## Exhaustive component evidence
//!
//! [`HybridStrategy::evaluate`] answers with a [`HybridEvaluation`]: the combined
//! order, and for every candidate one [`HybridComponent`] per backend in
//! [`HybridBackend::ALL`] order, each carrying its availability, native raw
//! evidence, normalized score, weight, and contribution. Availability is typed:
//!
//! - [`HybridAvailability::Available`] — the backend produced this round's
//!   evidence;
//! - [`HybridAvailability::Missing`] — the backend is not supplied by this
//!   hybrid (or is not compiled into this build). `raw` and `normalized` are
//!   `None`, never an invented zero;
//! - [`HybridAvailability::Failed`] — the backend was supplied but could not
//!   score the round (a ceiling, a rejected stage, a non-finite score, an empty
//!   recall corpus, …). The failure is the guidance seam's own typed, sanitized
//!   vocabulary plus the round meter ([`BackendFailure`]), `raw` and `normalized`
//!   are `None`, and the contribution is exactly `+0.0`.
//!
//! A failure is therefore explicit evidence *and* the round still ranks from the
//! remaining evidence — the hybrid answers with what it actually has and never
//! claims a failed backend's score. No component ever disappears because another
//! backend scored well, and no component's evidence is omitted from the answer.
//!
//! ## Ordering authority, completeness authority, and identity
//!
//! Ordering only: complete mode consumes the full permutation, so a fully
//! exhausted closure keeps the unguided explored set and evidence hash; any
//! truncation stays inside [`crate::inverse::GuidanceMode::HeuristicPruning`],
//! which applies it to the *combined* order and counts what it dropped. A
//! budget-truncated run stays `Partial`.
//!
//! A round's answer is a function of `(configuration, backends, candidates)`
//! alone: no mutable global, no model cache, no RNG, no filesystem, and no
//! environment access. [`HybridStrategy::replay_hash`] is the identity folded
//! into replay identity through [`crate::inverse::scorer_hash_with`], and it
//! covers the hybrid schema version, the normalization convention, every
//! configured weight's IEEE-754 bits, the effective admission ceiling, and each
//! supplied backend's own identity — a present backend and an absent one never
//! share an identity, even at weight zero.
//!
//! ## Provenance
//!
//! The hybrid consumes validated backend output and never manufactures it. The
//! neural lane is a `NeuralStrategy`, which exists only for a ranker validated
//! against the frozen feature schema (the Task 43 checkpoint path); no training
//! trace is constructed, replayed, or synthesized here. The recall lane is a
//! `crate::holographic::RecallTrace`, which can only be minted by replaying a
//! certificate through a term system, and `RecallLane` additionally rejects a
//! trace or query whose recorded `system_hash` is not the **original** system's,
//! so a trace replayed through a different system is a typed rejection
//! ([`HybridConfigError::ForeignTraceProvenance`]). Nothing here synthesizes a
//! trace from the hybrid scorer.
//!
//! ## Feature isolation
//!
//! The module is compiled only with at least one backend (`neural` or
//! `holographic-guidance`; `network` implies `neural`). A default build exposes
//! no hybrid surface at all, and a build with a single backend reports the
//! unavailable ones as [`HybridAvailability::Missing`] evidence rather than
//! failing to compile or silently skipping them.

use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

use crate::inverse::guidance::{scorer_hash, validate_ranking};
use crate::inverse::{
    GuidanceCandidate, GuidanceFailure, GuidanceFailureKind, GuidanceRequest, GuidanceStrategy,
};
use crate::relation::Sha256Digest;

/// Frame of the hybrid scorer identity.
pub const HYBRID_FRAME: &str = "amari.inverse.hybrid/v1";

/// Frame of the normalization convention identity.
pub const NORMALIZATION_FRAME: &str = "amari.inverse.hybrid-normalization/v1";

/// The hybrid schema version. Bumped only by a deliberate change to the
/// evidence layout, the normalization, or the combination rule.
pub const HYBRID_SCHEMA_VERSION: u32 = 1;

/// The published normalization convention, pinned by [`normalization_hash`].
///
/// It is a constant because it is part of the scorer identity: two hybrids that
/// normalize differently must never share a replay identity. It is also the
/// contract's spelling of "lower is more preferred" (pinned by the suite).
pub const NORMALIZATION_CONVENTION: &str = "ordinal-fraction-v1: each available backend ranks the round \
     in its own native direction (symbolic, neural, and geometric prefer lower; recall prefers higher) \
     with its own content tie-break, and each candidate's normalized score is its position in that \
     order divided by (n - 1), or 0 when n = 1; lower normalized is more preferred";

/// Hard ceiling on the candidates one hybrid ranking round may admit.
///
/// It mirrors the neural batch ceiling; a supplied backend's own ceiling can
/// only tighten the effective admission ceiling
/// ([`HybridStrategy::max_candidates`]), never widen it.
pub const MAX_HYBRID_CANDIDATES: usize = 65_536;

/// Frame of the hybrid recall-memory identity.
#[cfg(feature = "holographic-guidance")]
pub const RECALL_LANE_FRAME: &str = "amari.inverse.hybrid-recall/v1";

/// Frame of the candidate-term projection key recall evidence is matched by.
#[cfg(feature = "holographic-guidance")]
const TERM_PROJECTION_FRAME: &str = "amari.inverse.hybrid-candidate-term/v1";

/// One evidence lane the hybrid can combine.
///
/// The declaration order is contractual: components are reported, and
/// contributions accumulated, in this order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HybridBackend {
    /// The transparent symbolic cost (the engine's own lexicographic order).
    Symbolic,
    /// The learned neural ranker (`neural`).
    Neural,
    /// The transparent geometric score over one graph snapshot (`network`).
    Geometric,
    /// The deterministic holographic trace recall (`holographic-guidance`).
    Recall,
}

impl HybridBackend {
    /// Every backend, in evidence and accumulation order.
    pub const ALL: [Self; 4] = [Self::Symbolic, Self::Neural, Self::Geometric, Self::Recall];

    /// The stable name of this backend in diagnostics and evidence.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Symbolic => "symbolic",
            Self::Neural => "neural",
            Self::Geometric => "geometric",
            Self::Recall => "recall",
        }
    }

    /// This backend's **native** direction: `true` when lower is preferred.
    ///
    /// The hybrid converts every lane to lower-is-preferred through the ordinal
    /// normalization; this flag is the published statement of the raw direction,
    /// never a switch the combination depends on silently.
    pub const fn prefers_lower(self) -> bool {
        match self {
            Self::Symbolic | Self::Neural | Self::Geometric => true,
            // Recall ranks by descending similarity: higher is more similar.
            Self::Recall => false,
        }
    }

    /// Zero-based position of this backend in [`Self::ALL`].
    const fn slot(self) -> usize {
        match self {
            Self::Symbolic => 0,
            Self::Neural => 1,
            Self::Geometric => 2,
            Self::Recall => 3,
        }
    }
}

impl fmt::Display for HybridBackend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

/// Digest of the published normalization convention.
///
/// Part of [`HybridStrategy::replay_hash`]: a hybrid that normalizes differently
/// is a different scorer.
pub fn normalization_hash() -> Sha256Digest {
    let mut payload = Vec::with_capacity(8 + NORMALIZATION_CONVENTION.len());
    payload.extend_from_slice(&HYBRID_SCHEMA_VERSION.to_le_bytes());
    payload.extend_from_slice(&(NORMALIZATION_CONVENTION.len() as u32).to_le_bytes());
    payload.extend_from_slice(NORMALIZATION_CONVENTION.as_bytes());
    Sha256Digest::framed(NORMALIZATION_FRAME, &payload)
}

/// Typed, validated combination weights.
///
/// Private fields: the only way to obtain a value is [`Self::new`] or
/// [`Self::single`], both of which validate every lane finite and inside
/// [`Self::MIN`]`..=`[`Self::MAX`] *before* any search exists. A weight is never
/// clamped and never widened.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HybridWeights {
    lanes: [f64; 4],
}

impl HybridWeights {
    /// The smallest admissible weight.
    pub const MIN: f64 = 0.0;

    /// The largest admissible weight.
    pub const MAX: f64 = 1.0;

    /// Validate four lane weights.
    ///
    /// # Errors
    ///
    /// [`HybridConfigError::NonFiniteWeight`] for `NaN` or an infinity, and
    /// [`HybridConfigError::WeightOutOfRange`] for a value outside
    /// `[MIN, MAX]`.
    pub fn new(
        symbolic: f64,
        neural: f64,
        geometric: f64,
        recall: f64,
    ) -> Result<Self, HybridConfigError> {
        let mut lanes = [0.0f64; 4];
        for (slot, weight) in [symbolic, neural, geometric, recall]
            .into_iter()
            .enumerate()
        {
            let backend = HybridBackend::ALL[slot];
            if !weight.is_finite() {
                return Err(HybridConfigError::NonFiniteWeight { backend });
            }
            if weight < Self::MIN || weight > Self::MAX {
                return Err(HybridConfigError::WeightOutOfRange { backend, weight });
            }
            // A zero is stored as `+0.0` so a contribution is never `-0.0`.
            lanes[slot] = if weight == 0.0 { 0.0 } else { weight };
        }
        Ok(Self { lanes })
    }

    /// One backend at `weight`, every other lane at zero.
    ///
    /// # Errors
    ///
    /// As [`Self::new`].
    pub fn single(backend: HybridBackend, weight: f64) -> Result<Self, HybridConfigError> {
        let mut lanes = [0.0f64; 4];
        lanes[backend.slot()] = weight;
        Self::new(lanes[0], lanes[1], lanes[2], lanes[3])
    }

    /// The validated weight for one backend.
    pub fn get(&self, backend: HybridBackend) -> f64 {
        self.lanes[backend.slot()]
    }

    /// True when every lane is zero: the documented content-only configuration.
    pub fn is_zero(&self) -> bool {
        self.lanes.iter().all(|weight| *weight == 0.0)
    }
}

impl Default for HybridWeights {
    /// The transparent baseline: symbolic only, weight `1`, everything else `0`.
    fn default() -> Self {
        Self {
            lanes: [1.0, 0.0, 0.0, 0.0],
        }
    }
}

/// A validated hybrid configuration: typed weights plus an admission ceiling.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HybridConfig {
    weights: HybridWeights,
    max_candidates: usize,
}

impl HybridConfig {
    /// The default per-round admission ceiling ([`MAX_HYBRID_CANDIDATES`]).
    pub const DEFAULT_MAX_CANDIDATES: usize = MAX_HYBRID_CANDIDATES;

    /// Validate a configuration.
    ///
    /// # Errors
    ///
    /// [`HybridConfigError::LimitExceeded`] when `max_candidates` is zero or
    /// above [`MAX_HYBRID_CANDIDATES`]. A supplied backend's own ceiling can
    /// tighten the effective ceiling afterwards; nothing can widen it.
    pub fn new(weights: HybridWeights, max_candidates: usize) -> Result<Self, HybridConfigError> {
        if max_candidates == 0 || max_candidates > MAX_HYBRID_CANDIDATES {
            return Err(HybridConfigError::LimitExceeded {
                resource: "hybrid strategy candidates",
                value: max_candidates,
                limit: MAX_HYBRID_CANDIDATES,
            });
        }
        Ok(Self {
            weights,
            max_candidates,
        })
    }

    /// The validated weights.
    pub fn weights(&self) -> &HybridWeights {
        &self.weights
    }

    /// The configured admission ceiling (before backend tightening).
    pub fn max_candidates(&self) -> usize {
        self.max_candidates
    }
}

impl Default for HybridConfig {
    fn default() -> Self {
        Self {
            weights: HybridWeights::default(),
            max_candidates: Self::DEFAULT_MAX_CANDIDATES,
        }
    }
}

/// Typed rejection of a hybrid's construction inputs.
///
/// Every variant is a static name, a count, a ceiling, or a value: nothing here
/// carries a path, a trace, or a backend error value.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum HybridConfigError {
    /// A weight was `NaN` or infinite. It is rejected, never clamped.
    #[error("hybrid weight for backend {backend} is not finite")]
    NonFiniteWeight {
        /// The rejected backend lane.
        backend: HybridBackend,
    },
    /// A weight was outside the documented `[0, 1]` domain.
    #[error("hybrid weight for backend {backend} is outside [0, 1]: {weight}")]
    WeightOutOfRange {
        /// The rejected backend lane.
        backend: HybridBackend,
        /// The offending value.
        weight: f64,
    },
    /// A ceiling was zero or above its fixed maximum.
    #[error("invalid hybrid limit: {resource} (value {value}, ceiling {limit})")]
    LimitExceeded {
        /// The rejected resource.
        resource: &'static str,
        /// The offending value.
        value: usize,
        /// The applied ceiling.
        limit: usize,
    },
    /// A recall trace or query was not replayed against the original system.
    #[error("recall provenance is not the original system's: {role}")]
    ForeignTraceProvenance {
        /// Which input was rejected (`recall query` or `recall trace`).
        role: &'static str,
    },
    /// The recall corpus refused an admitted trace.
    #[cfg(feature = "holographic-guidance")]
    #[error("recall corpus rejected a trace: {0}")]
    RecallAdmission(crate::holographic::HolographicError),
}

/// A backend's typed, sanitized, metered round failure.
///
/// The kind is the guidance seam's own vocabulary, so a hybrid failure needs no
/// parallel taxonomy; the meter says how many of the round's candidates the
/// backend fully scored before it failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BackendFailure {
    /// What went wrong.
    pub kind: GuidanceFailureKind,
    /// Candidates fully scored before the failure.
    pub scored: usize,
}

impl fmt::Display for BackendFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "backend failed at the {} stage after scoring {} candidates",
            self.kind, self.scored
        )
    }
}

/// Whether — and why not — a backend supplied this round's evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HybridAvailability {
    /// The backend produced this round's evidence.
    Available,
    /// The backend is not supplied (or not compiled into this build).
    Missing,
    /// The backend was supplied but could not score this round.
    Failed(BackendFailure),
}

impl HybridAvailability {
    /// True only for [`Self::Available`].
    pub fn is_available(&self) -> bool {
        matches!(self, Self::Available)
    }

    /// The typed failure, when the backend failed.
    pub fn failure(&self) -> Option<&BackendFailure> {
        match self {
            Self::Failed(failure) => Some(failure),
            _ => None,
        }
    }
}

/// One backend's evidence for one candidate.
///
/// `raw` is the backend's native statistic in its native direction, `normalized`
/// is the lower-is-better ordinal fraction, `weight` is the configured lane
/// weight, and `contribution` is exactly `weight * normalized` — or exactly
/// `+0.0` when the backend did not supply this round.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HybridComponent {
    /// Which lane this is.
    pub backend: HybridBackend,
    /// Whether the lane supplied evidence.
    pub availability: HybridAvailability,
    /// Native-direction raw evidence, when available.
    pub raw: Option<f64>,
    /// Lower-is-better normalized score in `[0, 1]`, when available.
    pub normalized: Option<f64>,
    /// The configured weight for this lane.
    pub weight: f64,
    /// `weight * normalized`, exactly `+0.0` when unavailable.
    pub contribution: f64,
}

/// One candidate's exhaustive hybrid evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct HybridCandidate {
    /// Index into the request's candidate slice.
    pub index: usize,
    /// One component per backend, in [`HybridBackend::ALL`] order.
    pub components: Vec<HybridComponent>,
    /// The sum of the contributions, in [`HybridBackend::ALL`] order.
    pub combined: f64,
}

impl HybridCandidate {
    /// The component for one backend.
    pub fn component(&self, backend: HybridBackend) -> Option<&HybridComponent> {
        self.components
            .iter()
            .find(|component| component.backend == backend)
    }
}

/// One ranked round: the combined order plus every component's evidence.
///
/// `order` is a permutation of the request's candidate indices, most preferred
/// first. `candidates` holds the evidence in *request* order, so a caller can
/// join it to the request without assuming the ordering.
#[derive(Clone, Debug, PartialEq)]
pub struct HybridEvaluation {
    /// The combined order: a permutation of the round's indices.
    pub order: Vec<usize>,
    /// Per-candidate evidence, in request order.
    pub candidates: Vec<HybridCandidate>,
}

impl HybridEvaluation {
    /// The availability of one backend for this round.
    pub fn availability(&self, backend: HybridBackend) -> Option<HybridAvailability> {
        self.candidates
            .first()
            .and_then(|candidate| candidate.component(backend))
            .map(|component| component.availability)
    }

    /// The candidate evidence for one request index.
    pub fn candidate(&self, index: usize) -> Option<&HybridCandidate> {
        self.candidates
            .get(index)
            .filter(|candidate| candidate.index == index)
    }
}

/// One backend's answer for one round, before weights are applied.
#[derive(Clone, Debug)]
struct BackendRound {
    availability: HybridAvailability,
    /// Native raws, indexed by request position.
    raws: Option<Vec<f64>>,
    /// The backend's own preference order (a permutation).
    order: Option<Vec<usize>>,
}

impl BackendRound {
    fn missing() -> Self {
        Self {
            availability: HybridAvailability::Missing,
            raws: None,
            order: None,
        }
    }

    fn failed(failure: GuidanceFailure) -> Self {
        Self {
            availability: HybridAvailability::Failed(BackendFailure {
                kind: failure.kind,
                scored: failure.scored,
            }),
            raws: None,
            order: None,
        }
    }

    fn available(raws: Vec<f64>, order: Vec<usize>) -> Self {
        Self {
            availability: HybridAvailability::Available,
            raws: Some(raws),
            order: Some(order),
        }
    }
}

/// A validated memory of replay-validated inverse traces, as the hybrid
/// consumes it.
///
/// The trait exists so the hybrid stays non-generic over the holographic
/// dimension: [`RecallLane`] implements it for one `D`, and a caller holds the
/// lane behind a borrow.
#[cfg(feature = "holographic-guidance")]
pub trait RecallBackend {
    /// This memory's identity: the encoder, the original system, the query, the
    /// admitted trace identities, and the ceilings that can change an answer.
    fn recall_hash(&self) -> Sha256Digest;

    /// Score one round: per-candidate native similarity (**higher is better**),
    /// in request order.
    ///
    /// # Errors
    ///
    /// A typed, sanitized, metered [`GuidanceFailure`] when the memory cannot
    /// answer (an empty corpus, a ceiling, or a rejected stage).
    fn similarities(
        &self,
        candidates: &[GuidanceCandidate<'_>],
    ) -> Result<Vec<f64>, GuidanceFailure>;
}

/// A bounded holographic recall memory bound to one original term system.
///
/// The traces must be [`crate::holographic::RecallTrace`] values, i.e. results
/// of replaying a certificate against a term system; construction additionally
/// requires that recorded system to be `original`, so provenance is checked
/// rather than assumed. The lane projects recall evidence onto candidates by
/// **canonical term identity**: a candidate's native similarity is the highest
/// `combined` similarity among the recalled traces whose content contains the
/// candidate's state term, and `0.0` (the neutral native value) when no
/// remembered trace does. Constraint sets are not part of a trace's content and
/// are therefore not projected.
#[cfg(feature = "holographic-guidance")]
#[derive(Clone, Debug)]
pub struct RecallLane<const D: usize> {
    corpus: crate::holographic::TraceCorpus<D>,
    query: crate::holographic::RecallTrace,
    /// `(trace identity, canonical term keys of its content)`, sorted by
    /// identity so a match resolves by binary search.
    traces: Vec<(Sha256Digest, Vec<Sha256Digest>)>,
    limit: usize,
    system: Sha256Digest,
}

#[cfg(feature = "holographic-guidance")]
impl<const D: usize> RecallLane<D> {
    /// Build a lane over the traces this system's own search produced.
    ///
    /// The corpus is constructed here from exactly the supplied traces, so the
    /// scored memory and the projected content can never disagree.
    ///
    /// # Errors
    ///
    /// - [`HybridConfigError::ForeignTraceProvenance`] when the query or any
    ///   trace records a `system_hash` other than `original`'s — a trace
    ///   replayed through a different system is never accepted;
    /// - [`HybridConfigError::RecallAdmission`] when the corpus refuses a trace
    ///   (its ceilings, an empty step list, or a duplicate identity).
    pub fn new(
        original: &crate::trs::TermSystem,
        encoder: crate::holographic::HolographicEncoder<D>,
        config: crate::holographic::RecallConfig,
        query: crate::holographic::RecallTrace,
        traces: Vec<crate::holographic::RecallTrace>,
    ) -> Result<Self, HybridConfigError> {
        let system = crate::inverse::system_hash(original);
        if query.system_hash() != system {
            return Err(HybridConfigError::ForeignTraceProvenance {
                role: "recall query",
            });
        }
        for trace in &traces {
            if trace.system_hash() != system {
                return Err(HybridConfigError::ForeignTraceProvenance {
                    role: "recall trace",
                });
            }
        }
        let limit = config.max_results();
        let mut corpus = crate::holographic::TraceCorpus::new(encoder, config);
        let mut projected = Vec::with_capacity(traces.len());
        for trace in &traces {
            corpus
                .admit(trace)
                .map_err(HybridConfigError::RecallAdmission)?;
            let keys: Vec<Sha256Digest> = trace.terms().into_iter().map(term_projection).collect();
            projected.push((trace.trace_hash(), keys));
        }
        projected.sort_by_key(|entry| entry.0);
        Ok(Self {
            corpus,
            query,
            traces: projected,
            limit,
            system,
        })
    }

    /// The original system's recorded identity.
    pub fn system(&self) -> Sha256Digest {
        self.system
    }

    /// Number of admitted traces.
    pub fn trace_count(&self) -> usize {
        self.traces.len()
    }

    /// The recall result limit every round is scored under.
    pub fn limit(&self) -> usize {
        self.limit
    }

    /// The query's recorded trace identity.
    pub fn query_trace_hash(&self) -> Sha256Digest {
        self.query.trace_hash()
    }

    /// The candidate's native similarity: the highest `combined` similarity
    /// among matched traces whose content contains its term.
    fn similarity_of(&self, recall: &crate::holographic::TraceRecall, key: &Sha256Digest) -> f64 {
        for matched in &recall.matches {
            let Ok(found) = self
                .traces
                .binary_search_by(|(identity, _)| identity.cmp(&matched.trace_hash))
            else {
                continue;
            };
            if self.traces[found].1.contains(key) {
                // Matches arrive in descending similarity order, so the first
                // containing trace is the most similar one.
                return matched.score.combined;
            }
        }
        // No remembered trace contains this term: the native neutral value.
        0.0
    }
}

#[cfg(feature = "holographic-guidance")]
impl<const D: usize> RecallBackend for RecallLane<D> {
    fn recall_hash(&self) -> Sha256Digest {
        let mut payload = Vec::with_capacity(176 + self.traces.len() * 32);
        payload.extend_from_slice(&(D as u64).to_le_bytes());
        payload.extend_from_slice(self.corpus.encoder_digest().as_bytes());
        payload.extend_from_slice(self.system.as_bytes());
        payload.extend_from_slice(self.query.trace_hash().as_bytes());
        payload.extend_from_slice(self.query.query_hash().as_bytes());
        payload.extend_from_slice(&(self.limit as u64).to_le_bytes());
        payload.extend_from_slice(&(self.traces.len() as u64).to_le_bytes());
        for (identity, _) in &self.traces {
            payload.extend_from_slice(identity.as_bytes());
        }
        Sha256Digest::framed(RECALL_LANE_FRAME, &payload)
    }

    fn similarities(
        &self,
        candidates: &[GuidanceCandidate<'_>],
    ) -> Result<Vec<f64>, GuidanceFailure> {
        let total = candidates.len();
        let recall = self
            .corpus
            .recall(&self.query, self.limit)
            .map_err(|error| GuidanceFailure::new(recall_kind(&error), total, 0))?;
        let mut similarities = Vec::with_capacity(total);
        for candidate in candidates {
            let key = term_projection(candidate.state.term());
            similarities.push(self.similarity_of(&recall, &key));
        }
        Ok(similarities)
    }
}

/// The canonical term identity recall evidence is projected through.
#[cfg(feature = "holographic-guidance")]
fn term_projection(term: &crate::trs::Term) -> Sha256Digest {
    Sha256Digest::canonical_term(TERM_PROJECTION_FRAME, term)
}

/// Map one recall-surface rejection onto the guidance seam's typed vocabulary.
#[cfg(feature = "holographic-guidance")]
fn recall_kind(error: &crate::holographic::HolographicError) -> GuidanceFailureKind {
    use crate::holographic::HolographicError;
    match error {
        HolographicError::LimitExceeded {
            resource,
            value,
            limit,
        } => GuidanceFailureKind::LimitExceeded {
            resource,
            value: *value,
            limit: *limit,
        },
        HolographicError::MalformedTrace { .. } => GuidanceFailureKind::InvalidParameters,
        // A corpus with no admitted trace never answers "nothing similar".
        HolographicError::EmptyCorpus => GuidanceFailureKind::Unavailable {
            stage: "recall corpus",
        },
        // The traces were validated at construction, so a replay rejection here
        // cannot be a scoring cause; it is reported as an unavailable surface.
        HolographicError::UnvalidatedProvenance(_) => GuidanceFailureKind::Unavailable {
            stage: "recall provenance",
        },
        HolographicError::Composition { .. } => GuidanceFailureKind::BackendRejected {
            stage: "recall composition",
        },
    }
}

/// The hybrid ranking adapter over the configured backends.
///
/// Holding a `HybridStrategy` means "these weights, these backend snapshots,
/// under this ceiling", and [`Self::replay_hash`] is that identity. Nothing here
/// is mutable or global: a round is a pure function of the borrowed backends and
/// the engine's candidates.
pub struct HybridStrategy<'a> {
    config: HybridConfig,
    #[cfg(feature = "neural")]
    neural: Option<&'a crate::neural::NeuralStrategy>,
    #[cfg(feature = "network")]
    geometric: Option<&'a crate::network::GeometricStrategy>,
    #[cfg(feature = "holographic-guidance")]
    recall: Option<&'a dyn RecallBackend>,
}

impl<'a> HybridStrategy<'a> {
    /// A hybrid with the validated configuration and no optional backend.
    ///
    /// The symbolic lane is always present; the others are added with the
    /// `with_*` builders and report [`HybridAvailability::Missing`] until then.
    pub fn new(config: HybridConfig) -> Self {
        Self {
            config,
            #[cfg(feature = "neural")]
            neural: None,
            #[cfg(feature = "network")]
            geometric: None,
            #[cfg(feature = "holographic-guidance")]
            recall: None,
        }
    }

    /// Bind the neural ranker.
    #[cfg(feature = "neural")]
    pub fn with_neural(mut self, neural: &'a crate::neural::NeuralStrategy) -> Self {
        self.neural = Some(neural);
        self
    }

    /// Bind the geometric graph snapshot.
    #[cfg(feature = "network")]
    pub fn with_geometric(mut self, geometric: &'a crate::network::GeometricStrategy) -> Self {
        self.geometric = Some(geometric);
        self
    }

    /// Bind the holographic recall memory.
    #[cfg(feature = "holographic-guidance")]
    pub fn with_recall(mut self, recall: &'a dyn RecallBackend) -> Self {
        self.recall = Some(recall);
        self
    }

    /// The validated configuration.
    pub fn config(&self) -> &HybridConfig {
        &self.config
    }

    /// The effective per-round admission ceiling.
    ///
    /// The configured ceiling tightened by every supplied backend's own ceiling:
    /// a hybrid never admits a round a bound backend would refuse, and nothing
    /// here widens a configured authority.
    pub fn max_candidates(&self) -> usize {
        let configured = self.config.max_candidates;
        #[cfg(feature = "neural")]
        let configured = match self.neural {
            Some(neural) => configured.min(neural.config().max_candidates()),
            None => configured,
        };
        #[cfg(feature = "network")]
        let configured = match self.geometric {
            Some(geometric) => configured.min(geometric.config().max_candidates()),
            None => configured,
        };
        configured
    }

    /// This strategy's replay identity.
    ///
    /// It covers the hybrid schema version, the published normalization
    /// convention, every configured weight's IEEE-754 bits, the effective
    /// admission ceiling, and each supplied backend's own identity (a present
    /// backend and an absent one never share an identity, even at weight zero).
    /// Two hybrids with identical behavior share it; any change that can change
    /// a ranking or an admission does not.
    pub fn replay_hash(&self) -> Sha256Digest {
        let mut payload = Vec::with_capacity(320);
        payload.extend_from_slice(&HYBRID_SCHEMA_VERSION.to_le_bytes());
        payload.extend_from_slice(normalization_hash().as_bytes());
        for backend in HybridBackend::ALL {
            payload.extend_from_slice(&self.config.weights.get(backend).to_bits().to_le_bytes());
        }
        payload.extend_from_slice(&(self.max_candidates() as u64).to_le_bytes());
        for backend in HybridBackend::ALL {
            match self.backend_identity(backend) {
                Some(identity) => {
                    payload.push(0x01);
                    payload.extend_from_slice(identity.as_bytes());
                }
                None => payload.push(0x00),
            }
        }
        Sha256Digest::framed(HYBRID_FRAME, &payload)
    }

    /// One supplied backend's own identity, when it is supplied.
    fn backend_identity(&self, backend: HybridBackend) -> Option<Sha256Digest> {
        match backend {
            HybridBackend::Symbolic => Some(scorer_hash()),
            HybridBackend::Neural => {
                #[cfg(feature = "neural")]
                {
                    self.neural.map(crate::neural::NeuralStrategy::replay_hash)
                }
                #[cfg(not(feature = "neural"))]
                {
                    None
                }
            }
            HybridBackend::Geometric => {
                #[cfg(feature = "network")]
                {
                    self.geometric
                        .map(crate::network::GeometricStrategy::replay_hash)
                }
                #[cfg(not(feature = "network"))]
                {
                    None
                }
            }
            HybridBackend::Recall => {
                #[cfg(feature = "holographic-guidance")]
                {
                    self.recall.map(RecallBackend::recall_hash)
                }
                #[cfg(not(feature = "holographic-guidance"))]
                {
                    None
                }
            }
        }
    }

    /// Score one round exhaustively.
    ///
    /// # Errors
    ///
    /// [`GuidanceFailureKind::LimitExceeded`] when the round exceeds the
    /// effective admission ceiling. Nothing is scored and nothing is allocated
    /// per candidate in that case. Backend-level failures are *not* errors: they
    /// are reported as [`HybridAvailability::Failed`] evidence while the
    /// remaining evidence still ranks the round.
    pub fn evaluate(
        &self,
        request: &GuidanceRequest<'_>,
    ) -> Result<HybridEvaluation, GuidanceFailure> {
        let candidates = request.candidates;
        let total = candidates.len();
        let ceiling = self.max_candidates();
        if total > ceiling {
            return Err(GuidanceFailure::new(
                GuidanceFailureKind::LimitExceeded {
                    resource: "hybrid strategy candidates",
                    value: total,
                    limit: ceiling,
                },
                total,
                0,
            ));
        }
        // One canonical content key per candidate: the total tie-break, computed
        // once so every lane and the final order agree on it.
        let keys: Vec<Vec<u8>> = candidates.iter().map(content_key).collect();

        let symbolic = symbolic_round(candidates, &keys);
        #[cfg(feature = "neural")]
        let neural = match self.neural {
            Some(neural) => neural_round(neural, request),
            None => BackendRound::missing(),
        };
        #[cfg(not(feature = "neural"))]
        let neural = BackendRound::missing();
        #[cfg(feature = "network")]
        let geometric = match self.geometric {
            Some(geometric) => geometric_round(geometric, request),
            None => BackendRound::missing(),
        };
        #[cfg(not(feature = "network"))]
        let geometric = BackendRound::missing();
        #[cfg(feature = "holographic-guidance")]
        let recall = match self.recall {
            Some(recall) => recall_round(recall, request, &keys),
            None => BackendRound::missing(),
        };
        #[cfg(not(feature = "holographic-guidance"))]
        let recall = BackendRound::missing();

        let mut rounds = [symbolic, neural, geometric, recall];

        // The ordinal-fraction normalization, per available backend. A backend
        // whose answer is not a full permutation (or whose raw evidence does not
        // cover the round) is degraded to typed failure evidence: the invariant
        // `raw.is_some() <=> normalized.is_some()` holds for every component.
        let scale = if total <= 1 { 0.0 } else { (total - 1) as f64 };
        let mut normalized: [Option<Vec<f64>>; 4] = [None, None, None, None];
        for (slot, round) in rounds.iter_mut().enumerate() {
            let values = match (round.raws.as_ref(), round.order.as_ref()) {
                (Some(raws), Some(order)) if raws.len() == total => {
                    ordinal_normalization(order, total, scale)
                }
                (Some(_), Some(_)) => None,
                _ => continue,
            };
            match values {
                Some(values) => normalized[slot] = Some(values),
                None => {
                    round.availability = HybridAvailability::Failed(BackendFailure {
                        kind: GuidanceFailureKind::MalformedRanking,
                        scored: 0,
                    });
                    round.raws = None;
                    round.order = None;
                }
            }
        }

        let mut evidence = Vec::with_capacity(total);
        for index in 0..total {
            let mut combined = 0.0f64;
            let mut components = Vec::with_capacity(HybridBackend::ALL.len());
            for (slot, backend) in HybridBackend::ALL.into_iter().enumerate() {
                let round = &rounds[slot];
                let weight = self.config.weights.get(backend);
                let raw = round.raws.as_ref().map(|raws| raws[index]);
                let normalized = normalized[slot].as_ref().map(|values| values[index]);
                // Exactly `+0.0` when unavailable: an unavailable lane can never
                // move a combined score, and it is never reported as a score.
                let contribution = match normalized {
                    Some(value) => weight * value,
                    None => 0.0,
                };
                combined += contribution;
                components.push(HybridComponent {
                    backend,
                    availability: round.availability,
                    raw,
                    normalized,
                    weight,
                    contribution,
                });
            }
            evidence.push(HybridCandidate {
                index,
                components,
                combined,
            });
        }

        let mut order: Vec<usize> = (0..total).collect();
        order.sort_by(|&left, &right| {
            evidence[left]
                .combined
                .total_cmp(&evidence[right].combined)
                .then_with(|| keys[left].cmp(&keys[right]))
        });
        Ok(HybridEvaluation {
            order,
            candidates: evidence,
        })
    }
}

impl GuidanceStrategy for HybridStrategy<'_> {
    fn strategy_hash(&self) -> Sha256Digest {
        self.replay_hash()
    }

    fn rank(&self, request: &GuidanceRequest<'_>) -> Result<Vec<usize>, GuidanceFailure> {
        self.evaluate(request).map(|evaluation| evaluation.order)
    }
}

/// Map one backend's preference order onto the lower-is-better ordinal
/// fraction, or `None` when the order is not a permutation of the round.
fn ordinal_normalization(order: &[usize], total: usize, scale: f64) -> Option<Vec<f64>> {
    if order.len() != total || validate_ranking(order, total).is_err() {
        return None;
    }
    let mut positions = vec![0usize; total];
    for (position, &index) in order.iter().enumerate() {
        positions[index] = position;
    }
    Some(
        positions
            .into_iter()
            .map(|position| {
                if scale == 0.0 {
                    0.0
                } else {
                    position as f64 / scale
                }
            })
            .collect(),
    )
}

/// The symbolic lane: the engine's own lexicographic order, in content order.
fn symbolic_round(candidates: &[GuidanceCandidate<'_>], keys: &[Vec<u8>]) -> BackendRound {
    let total = candidates.len();
    let mut order: Vec<usize> = (0..total).collect();
    order.sort_by(|&left, &right| {
        candidates[left]
            .score
            .cmp(candidates[right].score)
            .then_with(|| keys[left].cmp(&keys[right]))
    });
    let mut positions = vec![0usize; total];
    for (position, &index) in order.iter().enumerate() {
        positions[index] = position;
    }
    // The symbolic preference is lexicographic, so its native statistic is the
    // candidate's position in that order.
    let raws = positions
        .into_iter()
        .map(|position| position as f64)
        .collect();
    BackendRound::available(raws, order)
}

/// The neural lane: the ranker's own order, with its own tie-break.
#[cfg(feature = "neural")]
fn neural_round(
    neural: &crate::neural::NeuralStrategy,
    request: &GuidanceRequest<'_>,
) -> BackendRound {
    let total = request.candidates.len();
    let order = match neural.rank(request) {
        Ok(order) => order,
        Err(failure) => return BackendRound::failed(failure),
    };
    let mut raws = Vec::with_capacity(total);
    for (index, candidate) in request.candidates.iter().enumerate() {
        match neural_raw(neural, candidate) {
            Ok(raw) if raw.is_finite() => raws.push(raw),
            Ok(_) => {
                return BackendRound::failed(GuidanceFailure::new(
                    GuidanceFailureKind::NonFiniteScore {
                        stage: "neural score",
                    },
                    total,
                    index,
                ))
            }
            Err(_) => {
                return BackendRound::failed(GuidanceFailure::new(
                    GuidanceFailureKind::Unavailable {
                        stage: "neural evidence",
                    },
                    total,
                    index,
                ))
            }
        }
    }
    BackendRound::available(raws, order)
}

/// One candidate's native neural score, through the ranker's own admission gate.
#[cfg(feature = "neural")]
fn neural_raw(
    neural: &crate::neural::NeuralStrategy,
    candidate: &GuidanceCandidate<'_>,
) -> Result<f64, crate::neural::NeuralError> {
    use crate::neural::{CandidateInput, SearchContext};
    let score = candidate.score;
    // The context mirrors the neural strategy's own encoding contract: the
    // quantities are the round's symbolic score, supplied rather than
    // recomputed, so the two adapters cannot disagree about depth or growth.
    let context = SearchContext::new(
        score.depth,
        score.branch_factor,
        score.cycle,
        score.term_growth,
        score.constraint_growth,
        candidate.predecessor.existentials.len(),
        score.rule_priority,
    );
    let input = CandidateInput::new(
        candidate.state.term(),
        candidate.state.constraints(),
        &candidate.predecessor.provenance,
        context,
    );
    let features = neural.encoder().encode(&input)?;
    Ok(f64::from(neural.ranker().score_features(&features)?))
}

/// The geometric lane: the transparent total's own order.
#[cfg(feature = "network")]
fn geometric_round(
    geometric: &crate::network::GeometricStrategy,
    request: &GuidanceRequest<'_>,
) -> BackendRound {
    let total = request.candidates.len();
    let order = match geometric.rank(request) {
        Ok(order) => order,
        Err(failure) => return BackendRound::failed(failure),
    };
    let mut raws = Vec::with_capacity(total);
    for (index, candidate) in request.candidates.iter().enumerate() {
        match geometric.score(candidate) {
            Ok(score) if score.total().is_finite() => raws.push(score.total()),
            Ok(_) => {
                return BackendRound::failed(GuidanceFailure::new(
                    GuidanceFailureKind::NonFiniteScore {
                        stage: "geometric score",
                    },
                    total,
                    index,
                ))
            }
            Err(_) => {
                return BackendRound::failed(GuidanceFailure::new(
                    GuidanceFailureKind::Unavailable {
                        stage: "geometric evidence",
                    },
                    total,
                    index,
                ))
            }
        }
    }
    BackendRound::available(raws, order)
}

/// The recall lane: descending similarity (**higher is better**), then the
/// canonical content key.
#[cfg(feature = "holographic-guidance")]
fn recall_round(
    recall: &dyn RecallBackend,
    request: &GuidanceRequest<'_>,
    keys: &[Vec<u8>],
) -> BackendRound {
    let total = request.candidates.len();
    let similarities = match recall.similarities(request.candidates) {
        Ok(similarities) => similarities,
        Err(failure) => return BackendRound::failed(failure),
    };
    if similarities.len() != total {
        return BackendRound::failed(GuidanceFailure::new(
            GuidanceFailureKind::MalformedRanking,
            total,
            0,
        ));
    }
    if let Some(index) = similarities.iter().position(|value| !value.is_finite()) {
        return BackendRound::failed(GuidanceFailure::new(
            GuidanceFailureKind::NonFiniteScore {
                stage: "recall similarity",
            },
            total,
            index,
        ));
    }
    // The direction inversion happens here, explicitly: a larger similarity is a
    // smaller ordinal, so every lane reaches the combination lower-is-preferred.
    let mut order: Vec<usize> = (0..total).collect();
    order.sort_by(|&left, &right| {
        similarities[right]
            .total_cmp(&similarities[left])
            .then_with(|| keys[left].cmp(&keys[right]))
    });
    BackendRound::available(similarities, order)
}

/// The canonical content key that settles every remaining tie.
///
/// It is the same total-order convention the existing strategies use: the
/// candidate's canonical state digest, the transition's provenance, and the
/// round's symbolic context. The returned ordering is therefore a function of
/// the candidates' content and never of request order.
fn content_key(candidate: &GuidanceCandidate<'_>) -> Vec<u8> {
    let mut key = Vec::with_capacity(160);
    key.extend_from_slice(candidate.state.canonical_digest().as_bytes());
    let provenance = &candidate.predecessor.provenance;
    key.extend_from_slice(provenance.rule_id.digest().as_bytes());
    let position = provenance.position.as_slice();
    let position_len = u32::try_from(position.len()).unwrap_or(u32::MAX);
    key.extend_from_slice(&position_len.to_le_bytes());
    for index in position {
        key.extend_from_slice(&(*index as u64).to_le_bytes());
    }
    key.extend_from_slice(&provenance.scope.to_le_bytes());
    key.extend_from_slice(provenance.target_hash.as_bytes());
    key.extend_from_slice(provenance.predecessor_hash.as_bytes());
    let score = candidate.score;
    key.extend_from_slice(&score.depth.to_le_bytes());
    key.extend_from_slice(&score.branch_factor.to_le_bytes());
    key.push(u8::from(score.cycle));
    key.extend_from_slice(&score.term_growth.to_le_bytes());
    key.extend_from_slice(&score.constraint_growth.to_le_bytes());
    key.extend_from_slice(&(candidate.predecessor.existentials.len() as u64).to_le_bytes());
    key.extend_from_slice(&score.rule_priority.to_le_bytes());
    key
}
