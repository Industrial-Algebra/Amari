// SPDX-License-Identifier: MIT OR Apache-2.0

//! Deterministic holographic trace recall (0.25, Task 47).
//!
//! A **recall memory** for inverse rewriting: replay-validated inverse traces
//! are encoded into one holographic vector each, and a query trace is ranked
//! against them by vector similarity, returning transparent scored evidence.
//!
//! This is the `holographic-guidance` surface, and it is only that. In 0.25 the
//! evidence is advisory: nothing here selects, prunes, or generates a
//! transition, and no consumer is required to act on it.
//!
//! ## The surface in one paragraph
//!
//! [`RecallTrace::validate`] is the single minting path: it verifies a
//! [`ReplayCertificate`](crate::inverse::ReplayCertificate) against a term
//! system and reads out the certified goal and step terms. A
//! [`TraceCorpus`] admits those traces (bounded, duplicates rejected) and
//! [`TraceCorpus::recall`] scores every admitted trace against a query,
//! returning [`TraceMatch`]es ordered by descending similarity with the
//! decomposition exposed.
//!
//! ## Consuming validated output, never generating it
//!
//! The corpus is caller-provided and replay-validated **only**. A
//! [`RecallTrace`] cannot be assembled from terms: it has private fields and no
//! public constructor, and the only path to one replays a certificate through
//! the system (`ReplayCertificate::verify`). The recall memory therefore
//! consumes inverse-search output — the same D12 doctrine as the neural and
//! geometric surfaces — and cannot manufacture a trace, a transition, or a
//! predecessor.
//!
//! ## Additive superposition, not bundling
//!
//! A trace vector is `Σ position(token(term))` accumulated with the canonical
//! [`BindingAlgebra::superpose`](amari_holographic::BindingAlgebra::superpose).
//! `bundle` is deliberately *not* used: bundling is an attention operation
//! that normalizes (soft `beta`) or selects (`beta = ∞`) between candidates, so
//! it keeps the magnitude bounded and lets earlier contributions decay.
//! `superpose` is the trait's additive accumulation — `T = Σ keyᵢ ⊛ valueᵢ`,
//! coefficient-wise addition with no normalization — which is what a trace
//! memory needs: repeated evidence accumulates, the trace's own content keeps
//! dominating its stored vector, and similarity is a stable inner product. The
//! contract test pins the difference: superposing a trace with itself doubles
//! it, bundling it with itself returns it.
//!
//! ## Similarity evidence and its direction
//!
//! Each match exposes three independently measured lanes (`combined`,
//! `goal_alignment`, `path_alignment`), each the canonical
//! [`BindingAlgebra::similarity`](amari_holographic::BindingAlgebra::similarity)
//! of one pair of published vector parts. **Preferred is HIGHER** — see
//! [`SCORE_DIRECTION`] — and recall ranks descending by `combined`, with the
//! trace identity as a total tiebreak. The lanes are evidence, not an additive
//! score: no weighting and no hybrid scoring exists on this surface.
//!
//! ## Determinism
//!
//! The encoder is frozen and integer-exact: bipolar `±1` tokens derived from
//! framed SHA-256 content hashes, positions bound by the canonical
//! permutation, addition and IEEE-754 square roots only — no transcendental
//! function, no learned component, no hash-map iteration. Identical inputs
//! therefore produce bit-identical vectors, scores, and orders across runs and
//! across admission orders.
//!
//! ## Purity
//!
//! The module is pure: no filesystem, no environment, no process globals, no
//! project-memory lookup. The only thing a recall can see is the corpus the
//! caller admitted plus the query the caller supplied.
//!
//! ## Feature isolation
//!
//! Everything here is behind `holographic-guidance`; a default build exposes
//! no holographic surface at all.

mod encode;
mod recall;

pub use encode::{
    HolographicEncoder, TraceEncoding, MAX_TRACE_DIMENSION, MAX_TRACE_TERMS, TRACE_SCHEMA,
};
pub use recall::{
    RecallConfig, RecallTrace, TraceCorpus, TraceMatch, TraceRecall, TraceSimilarity,
    MAX_CORPUS_TRACES, MAX_RECALL_RESULTS, SCORE_DIRECTION,
};

use crate::error::RewriteError;

/// Typed rejection from the holographic recall surface.
///
/// Variants carry a static resource name, a count or ceiling, a static reason,
/// or the replay error that refused provenance. No term text, path, or corpus
/// content is carried, so a rejection is not a content oracle.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum HolographicError {
    /// A ceiling rejected the request before any work was done.
    #[error("holographic limit exceeded: {resource} (value {value}, ceiling {limit})")]
    LimitExceeded {
        /// The rejected resource.
        resource: &'static str,
        /// The offending value.
        value: usize,
        /// The applied ceiling.
        limit: usize,
    },
    /// A corpus entry is not a well-formed trace.
    #[error("malformed trace: {reason}")]
    MalformedTrace {
        /// The static reason.
        reason: &'static str,
    },
    /// Recall was asked to answer from a corpus with no admitted trace.
    #[error("recall corpus is empty")]
    EmptyCorpus,
    /// A certificate did not replay, so no trace could be minted from it.
    ///
    /// Carries the replay rejection (a typed [`RewriteError`]) because the
    /// certificate and system are the caller's own; nothing about other callers
    /// or other corpora is revealed.
    #[error("trace provenance is not replay-validated: {0}")]
    UnvalidatedProvenance(RewriteError),
    /// Internal composition failure. Same-dimension operands cannot reach it;
    /// it is reported rather than assumed away.
    #[error("holographic composition failed: {reason}")]
    Composition {
        /// The static reason.
        reason: &'static str,
    },
}
