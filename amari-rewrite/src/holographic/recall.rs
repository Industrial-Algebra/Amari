// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded holographic trace recall over caller-provided, replay-validated
//! traces.
//!
//! ## What the memory consumes
//!
//! The corpus holds **replay-validated inverse traces and nothing else**. A
//! [`RecallTrace`] has private fields and no public constructor; the one
//! minting path is [`RecallTrace::validate`], which verifies a
//! [`ReplayCertificate`] against the supplied [`TermSystem`] — every binding
//! hash recomputes, the recorded resources fit the configuration ceilings, and
//! the derivation replays through the system to the recorded query. A trace
//! therefore *is* validated inverse-search output (the same D12 doctrine as the
//! neural and geometric surfaces: the recall memory consumes, it never
//! generates), and admission validates provenance before encoding simply
//! because no other value exists to admit.
//!
//! ## What the memory returns
//!
//! Recall returns scored trace references with the similarity decomposition
//! exposed, and nothing else: the module can rank evidence and cannot select,
//! prune, or generate a transition. The evidence is advisory in this task.
//!
//! ## Similarity evidence
//!
//! For a query encoding `q` and a corpus encoding `t` (see
//! [`crate::holographic::encode`]):
//!
//! ```text
//! combined        = sim(q.vector, t.vector)
//! goal_alignment  = sim(q.goal,   t.goal)
//! path_alignment  = sim(q.path,   t.path)
//! ```
//!
//! where `sim` is the canonical [`BindingAlgebra::similarity`] — the normalized
//! real inner product — **clamped to `[-1, 1]`**. The clamped value is the
//! public formula: mathematically identical operands have similarity `1`, but
//! the same value computed in floating point can land at
//! `0.9999999999999998` (or overshoot to `1.0000000000000002`), so a lane is not
//! documented as bit-exactly `1.0` for identical traces. `0.0` is orthogonal and
//! `0.0` is also the result for a zero-norm operand. Each lane is recomputable
//! by any caller from the published vectors and the same clamp.
//!
//! The three lanes are **independent evidence, not an additive score**: they
//! are measured against different parts with different norms, so
//! `combined ≠ goal_alignment + path_alignment`. The single ranked scalar is
//! `combined`; there is no weighting table and no hybrid score in this task.
//!
//! ## Direction convention (pinned)
//!
//! **Preferred is HIGHER**: a larger `combined` is more similar, so recall
//! ranks descending by `combined` and breaks ties by ascending trace identity
//! (the content hash), which makes the order total and content-only. The
//! convention is stated in [`SCORE_DIRECTION`] and pinned end to end by the
//! contract tests (Cohort-8 lesson: a score direction is declared *and*
//! verified, never implied by a sort call).
//!
//! ## Purity
//!
//! The module reads exactly one thing: the corpus the caller admitted. There is
//! no filesystem access, no environment read, no process-global state, and no
//! project-memory lookup anywhere on this surface, so recall cannot see a trace
//! the caller did not hand it.
//!
//! ## Bounds
//!
//! [`RecallConfig`] bounds the corpus (traces per corpus, terms per trace,
//! results per recall) with hard ceilings [`MAX_CORPUS_TRACES`],
//! [`MAX_TRACE_TERMS`], and [`MAX_RECALL_RESULTS`]. Every ceiling is validated
//! at construction, a caller may tighten one and never widen it, and every
//! check precedes the work it bounds: the corpus allocates its entry vector at
//! the configured ceiling, admission checks the corpus size, the trace length,
//! and the identity set *before* encoding, and recall allocates at most the
//! admitted entry count.
//!
//! Minting is bounded the same way, on borrowed data: [`RecallTrace::validate`]
//! checks the certificate's borrowed step count against [`MAX_TRACE_TERMS`] and
//! measures the shape of every borrowed term *before* the certificate is
//! replayed and before any term is cloned (F3/F5, PR #384 round 1).
//!
//! ## Similarity interval and ties
//!
//! Every lane is documented to lie in `[-1, 1]`, and that interval is a stated
//! invariant of this surface rather than an approximation: a lane is clamped to
//! it (F6, PR #384 round 1), because the canonical inner product is a
//! floating-point value that can overshoot by one unit in the last place (an
//! identical trace at `D = 3` scores `1.0000000000000002`).
//!
//! The clamp is **nondecreasing, not strictly order preserving**. Clamping
//! cannot invert a comparison — a lane that was greater before clamping is never
//! less after it — but it CAN collapse a strict comparison into a tie: two lanes
//! that were strictly ordered can be clamped onto the same interval endpoint.
//! After a tie, the trace identity (ascending content hash) determines the order,
//! which is what makes the returned order total and content-only. There is no
//! claim that a clamped lane is bit-exactly `1.0` for every identical operand,
//! and no claim that distinct or strictly ordered lanes stay distinct or strictly
//! ordered (F22, PR #384 round 4).

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use amari_holographic::BindingAlgebra;

use crate::holographic::encode::{HolographicEncoder, TraceEncoding, MAX_TRACE_TERMS};
use crate::holographic::HolographicError;
use crate::inverse::{CertificateRejection, ReplayCertificate, ReplayDerivation, ReplayQuery};
use crate::relation::{RelationLimits, RelationResources, Sha256Digest};
use crate::trs::{Term, TermSystem};

/// Hard ceiling on the traces one corpus may hold.
pub const MAX_CORPUS_TRACES: usize = 256;

/// Hard ceiling on the matches one recall may return.
pub const MAX_RECALL_RESULTS: usize = 64;

/// The pinned score direction: **higher** similarity is preferred.
///
/// Recall ranks by descending `combined` similarity, so the best match is the
/// first entry with the largest value. This string is the contract's spelling
/// of that convention and is pinned by the tests.
pub const SCORE_DIRECTION: &str = "higher-is-better";

/// Frame of one term's alpha-shared digest inside a trace identity.
const TRACE_TERM_FRAME: &str = "amari.holographic.trace-term/v1";

/// Frame of the trace identity itself.
const TRACE_FRAME: &str = "amari.holographic.trace/v1";

/// A replay-validated inverse trace: the certified goal, the derivation's step
/// terms in order, and the identities they bind.
///
/// Private fields, no public constructor. Possession of a `RecallTrace` is
/// proof that [`RecallTrace::validate`] replayed a certificate through a term
/// system to exactly this content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecallTrace {
    goal: Term,
    steps: Vec<Term>,
    trace_hash: Sha256Digest,
    query_hash: Sha256Digest,
    system_hash: Sha256Digest,
}

impl RecallTrace {
    /// Validate a replay certificate and mint the trace it attests to.
    ///
    /// The certificate is verified against `system` first
    /// ([`ReplayCertificate::verify`]): every binding hash must recompute, the
    /// recorded resources must fit the configuration ceilings, and the
    /// derivation must replay to the recorded query — including the *recorded
    /// intermediate terms*, each of which must equal the state the replay
    /// reaches (see [`crate::inverse`]). Only then is the trace's content read
    /// out — the certified goal, and the recorded step terms in derivation
    /// order (goal side first). For a bidirectional certificate the recorded
    /// inverse half is the trace; forward steps carry hashes but no terms, so
    /// nothing is reconstructed.
    ///
    /// The borrowed data is admitted *before* that replay and before any term
    /// is cloned, because the replay is the expensive part and because a
    /// certificate is caller-supplied:
    ///
    /// 1. every cheap ceiling is decided on integers first: the borrowed step
    ///    count against [`MAX_TRACE_TERMS`], and — through the SAME
    ///    crate-internal preflight [`ReplayCertificate::verify`] uses — the
    ///    embedded configuration and guidance mode, the derivation against the
    ///    embedded search's producer-derived step, state, transition, and
    ///    operation authority, and the recorded resource observation
    ///    (F18/F20/F21, PR #384 round 4; F27, PR #384 round 5). An oversize
    ///    derivation, an invalid configuration or guidance mode, and an
    ///    over-ceiling observation are therefore all rejected before any term is
    ///    walked, cloned, or replayed;
    /// 2. the shape of every borrowed term the replay must reproduce — the
    ///    certified goal, the query's other endpoint, each recorded step term,
    ///    and (for a bidirectional certificate) both meeting terms — is
    ///    measured against the crate's hard relation ceilings
    ///    ([`RelationLimits::default`]: node and depth), and the recorded
    ///    meeting substitution's ranges are admitted under ONE shared relation
    ///    meter, so the aggregate admission is bounded by the operation ceiling
    ///    instead of growing with the number of bindings. This measurement
    ///    meters the replay's own intermediate terms too, because each replayed
    ///    intermediate state must equal its measured recorded step term;
    /// 3. only then does the replay run (through the crate-internal
    ///    continuation that does NOT re-admit the borrowed data), and only then
    ///    is the content cloned. A valid certificate's borrowed terms are
    ///    therefore walked exactly once on the whole path.
    ///
    /// # Errors
    ///
    /// - [`HolographicError::LimitExceeded`] when the encoded trace exceeds
    ///   [`MAX_TRACE_TERMS`], a derivation half exceeds its embedded search
    ///   step/state/transition/operation ceiling, a borrowed term exceeds the
    ///   node or depth ceiling, or the shared borrowed-admission walk exhausts
    ///   its aggregate operation budget (reported with the first inadmissible
    ///   value);
    /// - [`HolographicError::UnvalidatedProvenance`] when the certificate's
    ///   embedded configuration, guidance mode, or recorded observation is
    ///   invalid (including rejections before borrowed admission), when its
    ///   hashes do not match `system`, or when replay itself returns a typed
    ///   error (including a tampered intermediate term or invented derivation);
    /// - [`HolographicError::MalformedTrace`] when the query kind and the
    ///   derivation kind disagree.
    pub fn validate(
        system: &TermSystem,
        certificate: &ReplayCertificate,
    ) -> Result<Self, HolographicError> {
        // Borrowed admission: read the certificate's terms by reference only,
        // so every check below costs one bounded walk and never a clone.
        let (goal, steps) = match (&certificate.query, &certificate.derivation) {
            (ReplayQuery::Backward { goal, .. }, ReplayDerivation::Backward(derivation)) => {
                (goal, derivation.steps.as_slice())
            }
            (
                ReplayQuery::Bidirectional { goal, .. },
                ReplayDerivation::Bidirectional(derivation),
            ) => (goal, derivation.backward_steps.as_slice()),
            _ => {
                return Err(HolographicError::MalformedTrace {
                    reason: "query kind does not match the derivation kind",
                });
            }
        };
        // Every cheap CEILING is decided on integers before any borrowed walk:
        // the encoded trace-term count, then the embedded search's step, state,
        // transition, and operation authority. A certificate carrying 100,000
        // unused unifier bindings and an over-deep half is therefore rejected
        // without walking a single binding (F21, PR #384 round 4).
        let terms = steps.len() + 1;
        if terms > MAX_TRACE_TERMS {
            return Err(HolographicError::LimitExceeded {
                resource: "trace terms",
                value: terms,
                limit: MAX_TRACE_TERMS,
            });
        }
        // Each derivation half is replayed, not encoded, so it is bounded by the
        // *embedded search configuration* rather than by the encoded trace-term
        // ceiling above (F12, PR #384 round 3; producer-derived retained-state
        // and operation bounds, F18/F20, PR #384 round 4).
        //
        // F27 (PR #384 round 5): the rest of the cheap preflight (configuration,
        // guidance, derivation ceilings, recorded observation) and the aggregate
        // borrowed admission are the SAME crate-internal pass
        // `ReplayCertificate::verify` runs, under one meter. Reusing it is what
        // puts every integer ceiling before the first range walk and keeps a
        // valid certificate from being walked twice.
        let limits = RelationLimits::default();
        let mut meter = RelationResources::new(&limits);
        let admission = crate::inverse::preflight_and_admit(certificate, &mut meter)
            .map_err(admission_rejection)?;
        // Every borrowed term is admitted and the certificate is preflight-clean,
        // so only the hashes and the replay remain: no second admission walk.
        certificate
            .verify_admitted(system, &admission)
            .map_err(HolographicError::UnvalidatedProvenance)?;
        // The trace content is read out from the certified pair, never from
        // caller text.
        let goal = goal.clone();
        let steps = steps
            .iter()
            .map(|step| step.term.clone())
            .collect::<Vec<_>>();
        let trace_hash = trace_identity(&goal, &steps);
        Ok(Self {
            goal,
            steps,
            trace_hash,
            query_hash: certificate.query_hash,
            system_hash: certificate.system_hash,
        })
    }

    /// The certified goal.
    pub fn goal(&self) -> &Term {
        &self.goal
    }

    /// The recorded step terms, in derivation order (goal side first).
    pub fn steps(&self) -> &[Term] {
        &self.steps
    }

    /// The trace's terms: the goal followed by the steps.
    pub fn terms(&self) -> Vec<&Term> {
        let mut terms = Vec::with_capacity(self.steps.len() + 1);
        terms.push(&self.goal);
        terms.extend(self.steps.iter());
        terms
    }

    /// The trace's content identity: independent of the encoder's seed and
    /// dimension, so identical replay content has one identity everywhere.
    pub fn trace_hash(&self) -> Sha256Digest {
        self.trace_hash
    }

    /// The certified query's binding hash.
    pub fn query_hash(&self) -> Sha256Digest {
        self.query_hash
    }

    /// The certified system's binding hash.
    pub fn system_hash(&self) -> Sha256Digest {
        self.system_hash
    }
}

/// Content identity of a trace: the ordered alpha-canonical term digests of the
/// goal and steps, length-prefixed and framed.
///
/// Variables are renumbered across the whole trace, so cross-term variable
/// identity is preserved and alpha-variants share an identity.
fn trace_identity(goal: &Term, steps: &[Term]) -> Sha256Digest {
    let mut terms: Vec<&Term> = Vec::with_capacity(steps.len() + 1);
    terms.push(goal);
    terms.extend(steps.iter());
    let digests = Sha256Digest::canonical_terms(TRACE_TERM_FRAME, &terms);
    let mut payload = Vec::with_capacity(4 + 32 * digests.len());
    payload.extend_from_slice(&(digests.len() as u32).to_le_bytes());
    for digest in digests {
        payload.extend_from_slice(digest.as_bytes());
    }
    Sha256Digest::framed(TRACE_FRAME, &payload)
}

/// The transparent similarity evidence of one match.
///
/// Each lane is the canonical [`BindingAlgebra::similarity`] of one pair of
/// published parts (see the module documentation for the formula), **clamped to
/// the documented `[-1, 1]` interval**: the canonical value is a floating-point
/// inner product that can overshoot the interval by one unit in the last place,
/// and the interval is a stated invariant of this surface rather than an
/// approximation. The clamped value is the public number: mathematically
/// identical operands have similarity `1`, but the exposed lane may be
/// `0.9999999999999998` (or clamp to `1.0` from above) and is never documented
/// as bit-exactly `1.0`. Clamping is nondecreasing and therefore cannot invert a
/// comparison, but it is NOT strict-order preserving: a strict comparison can be
/// clamped into a tie, in which case the recall order falls back to the
/// trace-identity tiebreak. **Higher is more similar**;
/// `combined` is the ranked scalar and the other two are independent evidence,
/// never summed into it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TraceSimilarity {
    /// Similarity of the full trace vectors — the ranked scalar.
    pub combined: f64,
    /// Similarity of the position-0 goal tokens.
    pub goal_alignment: f64,
    /// Similarity of the superposed step parts (zero when either side has no
    /// step).
    pub path_alignment: f64,
}

/// One scored trace reference.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TraceMatch {
    /// The matched trace's content identity.
    pub trace_hash: Sha256Digest,
    /// The similarity evidence.
    pub score: TraceSimilarity,
}

/// One recall's evidence: the query identity, the encoder identity, how many
/// admitted traces were considered, and the ranked matches.
#[derive(Clone, Debug, PartialEq)]
pub struct TraceRecall {
    /// The query trace's content identity.
    pub query_hash: Sha256Digest,
    /// The encoder config digest the evidence was produced under.
    pub encoder_digest: Sha256Digest,
    /// Number of admitted traces scored.
    pub considered: usize,
    /// The ranked matches, best (highest `combined`) first, truncated to the
    /// requested limit.
    pub matches: Vec<TraceMatch>,
}

/// Validated ceilings for one recall corpus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecallConfig {
    max_traces: usize,
    max_terms: usize,
    max_results: usize,
}

impl RecallConfig {
    /// Tighten the corpus ceilings.
    ///
    /// # Errors
    ///
    /// [`HolographicError::LimitExceeded`] when any value is zero or above its
    /// hard ceiling ([`MAX_CORPUS_TRACES`], [`MAX_TRACE_TERMS`],
    /// [`MAX_RECALL_RESULTS`]).
    pub fn new(
        max_traces: usize,
        max_terms: usize,
        max_results: usize,
    ) -> Result<Self, HolographicError> {
        check("recall traces", max_traces, MAX_CORPUS_TRACES)?;
        check("trace terms", max_terms, MAX_TRACE_TERMS)?;
        check("recall results", max_results, MAX_RECALL_RESULTS)?;
        Ok(Self {
            max_traces,
            max_terms,
            max_results,
        })
    }

    /// The admitted traces per corpus.
    pub fn max_traces(&self) -> usize {
        self.max_traces
    }

    /// The admitted terms per trace.
    pub fn max_terms(&self) -> usize {
        self.max_terms
    }

    /// The admitted results per recall.
    pub fn max_results(&self) -> usize {
        self.max_results
    }
}

impl Default for RecallConfig {
    fn default() -> Self {
        Self {
            max_traces: MAX_CORPUS_TRACES,
            max_terms: MAX_TRACE_TERMS,
            max_results: MAX_RECALL_RESULTS,
        }
    }
}

/// A bounded holographic recall memory over replay-validated traces.
///
/// See the module documentation: admission validates provenance (by type) and
/// the corpus ceilings before encoding, recall reads only the admitted corpus,
/// and every order is content-only.
#[derive(Clone, Debug)]
pub struct TraceCorpus<const D: usize> {
    encoder: HolographicEncoder<D>,
    encoder_digest: Sha256Digest,
    config: RecallConfig,
    entries: Vec<TraceEncoding<D>>,
    index: BTreeMap<Sha256Digest, usize>,
}

impl<const D: usize> TraceCorpus<D> {
    /// Open an empty corpus with the configured ceilings.
    ///
    /// Allocation is bounded by the config: at most `max_traces` entry slots,
    /// whatever the caller later offers.
    pub fn new(encoder: HolographicEncoder<D>, config: RecallConfig) -> Self {
        let encoder_digest = encoder.digest();
        Self {
            encoder,
            encoder_digest,
            config,
            entries: Vec::with_capacity(config.max_traces()),
            index: BTreeMap::new(),
        }
    }

    /// The corpus's encoder.
    pub fn encoder(&self) -> &HolographicEncoder<D> {
        &self.encoder
    }

    /// The encoder config digest every entry was encoded under.
    pub fn encoder_digest(&self) -> Sha256Digest {
        self.encoder_digest
    }

    /// The corpus ceilings.
    pub fn config(&self) -> RecallConfig {
        self.config
    }

    /// Number of admitted traces.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when nothing has been admitted.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Admit one replay-validated trace.
    ///
    /// The checks run before any encoding work: the corpus ceiling, the
    /// requirement that the trace records at least one replay-validated step
    /// (a zero-step trace is a query shape, not a trace), the per-trace term
    /// ceiling, and the identity set. A rejected admission leaves the corpus
    /// exactly as it was.
    ///
    /// # Errors
    ///
    /// - [`HolographicError::LimitExceeded`] when the corpus is full or the
    ///   trace is longer than the configured term ceiling;
    /// - [`HolographicError::MalformedTrace`] for a step-less trace or a
    ///   duplicate identity.
    pub fn admit(&mut self, trace: &RecallTrace) -> Result<(), HolographicError> {
        if self.entries.len() >= self.config.max_traces {
            return Err(HolographicError::LimitExceeded {
                resource: "recall traces",
                value: self.entries.len() + 1,
                limit: self.config.max_traces,
            });
        }
        if trace.steps().is_empty() {
            return Err(HolographicError::MalformedTrace {
                reason: "trace has no replay-validated step",
            });
        }
        let terms = trace.steps().len() + 1;
        if terms > self.config.max_terms {
            return Err(HolographicError::LimitExceeded {
                resource: "trace terms",
                value: terms,
                limit: self.config.max_terms,
            });
        }
        if self.index.contains_key(&trace.trace_hash()) {
            return Err(HolographicError::MalformedTrace {
                reason: "duplicate trace identity",
            });
        }
        let encoding = self.encoder.encode(trace)?;
        self.index.insert(encoding.trace_hash, self.entries.len());
        self.entries.push(encoding);
        Ok(())
    }

    /// Recall the admitted traces closest to `query`.
    ///
    /// Every admitted trace is scored, the matches are ordered by descending
    /// `combined` similarity with the trace identity as the tiebreak (so the
    /// order is total and content-only), and the result is truncated to
    /// `limit`. Admission order cannot change the answer.
    ///
    /// # Errors
    ///
    /// - [`HolographicError::LimitExceeded`] when `limit` is zero, above
    ///   [`MAX_RECALL_RESULTS`], or above the corpus's configured result
    ///   ceiling (a request may tighten a ceiling, never widen it);
    /// - [`HolographicError::EmptyCorpus`] when nothing has been admitted —
    ///   recall never answers "nothing similar" for a corpus it never saw;
    /// - [`HolographicError::LimitExceeded`] when the query exceeds the walk or
    ///   term ceilings.
    pub fn recall(
        &self,
        query: &RecallTrace,
        limit: usize,
    ) -> Result<TraceRecall, HolographicError> {
        check("recall results", limit, MAX_RECALL_RESULTS)?;
        if limit > self.config.max_results {
            return Err(HolographicError::LimitExceeded {
                resource: "recall results",
                value: limit,
                limit: self.config.max_results,
            });
        }
        if self.entries.is_empty() {
            return Err(HolographicError::EmptyCorpus);
        }
        let encoded = self.encoder.encode(query)?;
        let mut matches = Vec::with_capacity(self.entries.len());
        for entry in &self.entries {
            matches.push(TraceMatch {
                trace_hash: entry.trace_hash,
                score: TraceSimilarity {
                    combined: lane_similarity(&encoded.vector, &entry.vector),
                    goal_alignment: lane_similarity(&encoded.goal, &entry.goal),
                    path_alignment: lane_similarity(&encoded.path, &entry.path),
                },
            });
        }
        matches.sort_by(|left, right| {
            right
                .score
                .combined
                .total_cmp(&left.score.combined)
                .then_with(|| left.trace_hash.cmp(&right.trace_hash))
        });
        matches.truncate(limit);
        Ok(TraceRecall {
            query_hash: encoded.trace_hash,
            encoder_digest: self.encoder_digest,
            considered: self.entries.len(),
            matches,
        })
    }
}

/// Map one shared preflight/admission rejection onto the recall boundary's
/// public error type, preserving every mapping this boundary always had (F27,
/// PR #384 round 5):
///
/// - a certificate error (invalid embedded configuration, invalid guidance mode,
///   over-ceiling recorded observation, or disagreeing query/derivation kinds)
///   is unvalidated provenance;
/// - an integer derivation ceiling is an admission limit under its own resource
///   name;
/// - a borrowed shape is an admission limit, reported under the trace-specific
///   names the trace-term measurement has always used.
fn admission_rejection(rejection: CertificateRejection) -> HolographicError {
    match rejection {
        CertificateRejection::Certificate(crate::error::RewriteError::RelationLimitExceeded {
            resource: "operations",
            limit,
        }) => HolographicError::LimitExceeded {
            resource: "operations",
            value: limit.saturating_add(1),
            limit,
        },
        CertificateRejection::Certificate(error) => HolographicError::UnvalidatedProvenance(error),
        CertificateRejection::StepCeiling(ceiling) => HolographicError::LimitExceeded {
            resource: ceiling.resource,
            value: ceiling.value,
            limit: ceiling.limit,
        },
        CertificateRejection::Shape {
            resource,
            value,
            limit,
        } => HolographicError::LimitExceeded {
            resource: match resource {
                "term nodes" => "trace term nodes",
                "term depth" => "trace term depth",
                other => other,
            },
            value,
            limit,
        },
    }
}

/// Reject a zero or above-ceiling value.
fn check(resource: &'static str, value: usize, limit: usize) -> Result<(), HolographicError> {
    if value == 0 || value > limit {
        return Err(HolographicError::LimitExceeded {
            resource,
            value,
            limit,
        });
    }
    Ok(())
}

/// One similarity lane, clamped to the documented `[-1, 1]` interval.
///
/// [`BindingAlgebra::similarity`] is a floating-point inner product, so an
/// identical vector can score one ulp above `1.0` (`1.0000000000000002`) or one
/// ulp below it (`0.9999999999999998`). The module documents the interval, so the
/// interval is enforced here (F6, PR #384 round 1). Clamping is nondecreasing,
/// so clamping itself never reverses a pair of lanes. It is not strict-order
/// preserving, however: a strict comparison can be clamped into a tie, which the
/// trace-identity tiebreak then resolves. No bit-exact `1.0` is implied for
/// identical operands (F22, PR #384 round 4).
fn lane_similarity<const D: usize>(
    left: &amari_holographic::FHRRAlgebra<D>,
    right: &amari_holographic::FHRRAlgebra<D>,
) -> f64 {
    <amari_holographic::FHRRAlgebra<D> as BindingAlgebra>::similarity(left, right).clamp(-1.0, 1.0)
}
