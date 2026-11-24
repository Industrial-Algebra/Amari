// SPDX-License-Identifier: MIT OR Apache-2.0

//! The frozen deterministic trace encoder (`holographic-trace-v1`).
//!
//! The encoder turns a replay-validated [`RecallTrace`] into one
//! holographic vector. It is frozen, integer/bipolar, and free of learned
//! components, transcendental functions, and hash-map iteration:
//!
//! - **token** — a term's token is a bipolar unit vector: component `i` is
//!   `+1` when bit `i` (LSB-first within each digest byte) of the term's
//!   framed SHA-256 content hash is set, and `-1` otherwise. The imaginary
//!   part is identically zero, so the token is a real phasor with
//!   `±1` components. Tokens are *alpha-invariant*: the content hash is the
//!   canonical term digest, so `X` and `Y` are the same token.
//! - **position** — a token is bound to its position by the canonical
//!   permutation, [`BindingAlgebra::permute`] (a cyclic shift). Position is
//!   taken modulo the dimension, so a trace longer than `D` aliases its
//!   positions deliberately and visibly.
//! - **trace** — the goal sits at position 0 (one replay-validated context
//!   exists per trace, so a fixed slot is unambiguous) and the derivation's
//!   step terms follow at positions 1.. in derivation order (goal side
//!   first). The trace vector is the **additive superposition** of those
//!   positioned tokens, accumulated in order with
//!   [`BindingAlgebra::superpose`].
//!
//! ## Why `superpose` and not `bundle`
//!
//! `bundle` is an attention operation: it normalizes (soft) or selects
//! (hard, `beta = ∞`), so it deliberately keeps the magnitude bounded and
//! lets earlier contributions decay as new ones arrive. A trace memory needs
//! the opposite property — repeated evidence must accumulate so that a
//! trace's own content still dominates its stored vector — and
//! `superpose` is exactly the trait's additive accumulation
//! (`T = Σ keyᵢ ⊛ valueᵢ`, coefficient-wise addition, no normalization).
//! Bundling a trace with itself returns that trace; superposing it with
//! itself doubles it. The corpus therefore pins `superpose`.
//!
//! ## Identity
//!
//! Two independent content hashes exist, and both are pinned:
//!
//! - the **encoder config digest** ([`HolographicEncoder::digest`]) covers the
//!   schema string, the seed, the dimension, and the walk limits, so two
//!   encoders can never share an identity while producing different vectors;
//! - the **trace identity** ([`RecallTrace::trace_hash`]) is content only —
//!   the ordered alpha-canonical digests of the goal and steps — so the same
//!   replayed content has one identity under every seed and every dimension.
//!
//! ## Bounds
//!
//! [`MAX_TRACE_DIMENSION`] and [`MAX_TRACE_TERMS`] are hard ceilings, and every
//! admitted term is measured against the encoder's
//! [`RelationLimits`](crate::relation::RelationLimits) — nodes and depth — with
//! an explicit worklist *before* the token derivation hashes anything. A
//! rejected term is never copied, sorted, or hashed.

use alloc::vec::Vec;

use amari_holographic::{BindingAlgebra, FHRRAlgebra};

use crate::holographic::{HolographicError, RecallTrace};
use crate::relation::{RelationLimits, Sha256Digest};
use crate::trs::Term;

/// Hard ceiling on the holographic dimension of one encoder.
///
/// The derivation costs one content hash block per 256 components, and the
/// trace vector is `D` coefficients on the stack, so the ceiling bounds both.
pub const MAX_TRACE_DIMENSION: usize = 512;

/// Hard ceiling on the terms of one trace: the goal plus its steps.
pub const MAX_TRACE_TERMS: usize = 64;

/// Schema tag of the frozen encoder, covered by
/// [`HolographicEncoder::digest`].
pub const TRACE_SCHEMA: &str = "holographic-trace-v1";

/// Frame of the per-term content digest a token is derived from.
const TERM_TOKEN_FRAME: &str = "amari.holographic.term-token/v1";

/// Frame of one 256-component token derivation block.
const TOKEN_FRAME: &str = "amari.holographic.token/v1";

/// Frame of the frozen trace vector digest.
const VECTOR_FRAME: &str = "amari.holographic.vector/v1";

/// Frame of the encoder config digest.
const ENCODER_FRAME: &str = "amari.holographic.encoder/v1";

/// Bits of token derivation one digest block provides.
const TOKEN_BLOCK_BITS: usize = 256;

/// A frozen deterministic encoder of replay-validated traces.
///
/// `D` is the holographic dimension. Construction validates it against
/// [`MAX_TRACE_DIMENSION`]; the seed and the walk ceilings are covered by
/// [`Self::digest`], so the encoder's identity is a content hash of its
/// configuration.
#[derive(Clone, Debug)]
pub struct HolographicEncoder<const D: usize> {
    seed: u64,
    limits: RelationLimits,
}

impl<const D: usize> HolographicEncoder<D> {
    /// Build an encoder for dimension `D`.
    ///
    /// # Errors
    ///
    /// [`HolographicError::LimitExceeded`] when `D` is zero or above
    /// [`MAX_TRACE_DIMENSION`].
    pub fn new(seed: u64, limits: RelationLimits) -> Result<Self, HolographicError> {
        if D == 0 || D > MAX_TRACE_DIMENSION {
            return Err(HolographicError::LimitExceeded {
                resource: "trace dimension",
                value: D,
                limit: MAX_TRACE_DIMENSION,
            });
        }
        Ok(Self {
            seed,
            // Defence in depth: a limits value built outside the validating
            // constructor cannot widen a hard ceiling here.
            limits: limits.clamped(),
        })
    }

    /// The encoder's seed.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// The encoder's dimension.
    pub fn dimension(&self) -> usize {
        D
    }

    /// The walk ceilings every admitted term is measured against.
    pub fn limits(&self) -> RelationLimits {
        self.limits
    }

    /// Content digest of this encoder's configuration.
    ///
    /// Covers the schema string, the seed, the dimension, and the four walk
    /// ceilings. Two encoders that differ in any of them have different
    /// digests, and equal digests imply equal vectors for equal traces.
    pub fn digest(&self) -> Sha256Digest {
        let mut payload = Vec::with_capacity(64);
        let schema_len = u32::try_from(TRACE_SCHEMA.len()).unwrap_or(u32::MAX);
        payload.extend_from_slice(&schema_len.to_le_bytes());
        payload.extend_from_slice(TRACE_SCHEMA.as_bytes());
        payload.extend_from_slice(&self.seed.to_le_bytes());
        payload.extend_from_slice(&(D as u64).to_le_bytes());
        for value in [
            self.limits.max_term_nodes(),
            self.limits.max_term_depth(),
            self.limits.max_constraints(),
            self.limits.max_operations(),
        ] {
            payload.extend_from_slice(&(value as u64).to_le_bytes());
        }
        Sha256Digest::framed(ENCODER_FRAME, &payload)
    }

    /// The frozen token of `term`: a bipolar vector at position 0.
    ///
    /// # Errors
    ///
    /// [`HolographicError::LimitExceeded`] when the term exceeds the encoder's
    /// node or depth ceiling. The term is measured with an explicit worklist
    /// before anything is hashed.
    pub fn encode_token(&self, term: &Term) -> Result<FHRRAlgebra<D>, HolographicError> {
        self.token(term)
    }

    /// Encode a replay-validated trace.
    ///
    /// The vector is the additive superposition of the goal's token at
    /// position 0 and every step's token at position `index + 1`, accumulated
    /// in derivation order. The returned encoding also carries the two
    /// similarity lanes' parts and the frozen vector digest.
    ///
    /// # Errors
    ///
    /// [`HolographicError::LimitExceeded`] when the trace has more than
    /// [`MAX_TRACE_TERMS`] terms or any term exceeds the walk ceilings.
    pub fn encode(&self, trace: &RecallTrace) -> Result<TraceEncoding<D>, HolographicError> {
        let terms = trace.terms();
        if terms.len() > MAX_TRACE_TERMS {
            return Err(HolographicError::LimitExceeded {
                resource: "trace terms",
                value: terms.len(),
                limit: MAX_TRACE_TERMS,
            });
        }
        // Bounded admission before allocation: measure every term first, so a
        // rejected trace never reaches the derivation.
        for term in &terms {
            self.measure(term)?;
        }

        let goal = self.token(trace.goal())?;
        // Accumulate goal-then-steps in one order, so the vector is exactly the
        // documented superposition and not a differently associated sum. The
        // path part is the same accumulation without the goal slot.
        let mut vector = <FHRRAlgebra<D> as BindingAlgebra>::zero();
        vector = <FHRRAlgebra<D> as BindingAlgebra>::superpose(&vector, &goal).map_err(|_| {
            HolographicError::Composition {
                reason: "goal superposition",
            }
        })?;
        let mut path = <FHRRAlgebra<D> as BindingAlgebra>::zero();
        for (index, step) in trace.steps().iter().enumerate() {
            let positioned = self.positioned(step, index + 1)?;
            path = <FHRRAlgebra<D> as BindingAlgebra>::superpose(&path, &positioned).map_err(
                |_| HolographicError::Composition {
                    reason: "step superposition",
                },
            )?;
            vector = <FHRRAlgebra<D> as BindingAlgebra>::superpose(&vector, &positioned).map_err(
                |_| HolographicError::Composition {
                    reason: "step superposition",
                },
            )?;
        }

        Ok(TraceEncoding {
            digest: self.vector_digest(&vector),
            trace_hash: trace.trace_hash(),
            vector,
            goal,
            path,
        })
    }

    /// Digest of one frozen trace vector: the dimension and every real
    /// coefficient's IEEE-754 bit pattern, in index order.
    ///
    /// The imaginary part is identically zero by construction (bipolar
    /// tokens, additive superposition, cyclic shifts), so it is not hashed
    /// twice.
    pub fn vector_digest(&self, vector: &FHRRAlgebra<D>) -> Sha256Digest {
        let mut payload = Vec::with_capacity(8 + 8 * D);
        payload.extend_from_slice(&(D as u64).to_le_bytes());
        for index in 0..D {
            let bits = vector.real(index).unwrap_or(0.0).to_bits();
            payload.extend_from_slice(&bits.to_le_bytes());
        }
        Sha256Digest::framed(VECTOR_FRAME, &payload)
    }

    /// The frozen token of `term`, measured first.
    fn token(&self, term: &Term) -> Result<FHRRAlgebra<D>, HolographicError> {
        self.measure(term)?;
        let content = Sha256Digest::canonical_term(TERM_TOKEN_FRAME, term);
        let blocks = D.div_ceil(TOKEN_BLOCK_BITS);
        let mut bits = Vec::with_capacity(blocks * TOKEN_BLOCK_BITS);
        for block in 0..blocks {
            let mut payload = Vec::with_capacity(8 + 8 + 4 + 32);
            payload.extend_from_slice(&self.seed.to_le_bytes());
            payload.extend_from_slice(&(D as u64).to_le_bytes());
            payload.extend_from_slice(&(block as u32).to_le_bytes());
            payload.extend_from_slice(content.as_bytes());
            let digest = Sha256Digest::framed(TOKEN_FRAME, &payload);
            for byte in digest.as_bytes() {
                for bit in 0..8 {
                    bits.push((byte >> bit) & 1 == 1);
                }
            }
        }
        let mut real = [0.0f64; D];
        for (index, slot) in real.iter_mut().enumerate() {
            *slot = if bits.get(index).copied().unwrap_or(false) {
                1.0
            } else {
                -1.0
            };
        }
        Ok(FHRRAlgebra::new(real, [0.0f64; D]))
    }

    /// A token bound to `position` by the canonical permutation.
    fn positioned(&self, term: &Term, position: usize) -> Result<FHRRAlgebra<D>, HolographicError> {
        let token = self.token(term)?;
        let shift = i32::try_from(position).unwrap_or(i32::MAX);
        Ok(<FHRRAlgebra<D> as BindingAlgebra>::permute(&token, shift))
    }

    /// Measure a term against the walk ceilings with an explicit worklist.
    ///
    /// The worklist starts with one frame and holds at most the admitted node
    /// count, and the counters are charged before anything else happens, so an
    /// over-limit term is rejected without recursive descent or allocation
    /// proportional to its size.
    fn measure(&self, term: &Term) -> Result<(), HolographicError> {
        let mut stack: Vec<(&Term, usize)> = Vec::with_capacity(8);
        stack.push((term, 1));
        let mut nodes = 0usize;
        while let Some((node, depth)) = stack.pop() {
            nodes += 1;
            if nodes > self.limits.max_term_nodes() {
                return Err(HolographicError::LimitExceeded {
                    resource: "trace term nodes",
                    value: nodes,
                    limit: self.limits.max_term_nodes(),
                });
            }
            if depth > self.limits.max_term_depth() {
                return Err(HolographicError::LimitExceeded {
                    resource: "trace term depth",
                    value: depth,
                    limit: self.limits.max_term_depth(),
                });
            }
            if let Term::Sym(_, args) = node {
                for arg in args {
                    stack.push((arg, depth + 1));
                }
            }
        }
        Ok(())
    }
}

/// One encoded trace: the frozen vector and the parts its evidence decomposes
/// into.
///
/// `vector` is the full superposition, `goal` is the position-0 token, and
/// `path` is the superposition of the positioned step tokens (zero for a
/// zero-step trace). The lanes are measured against these parts, never
/// reconstructed from one another.
#[derive(Clone, Debug)]
pub struct TraceEncoding<const D: usize> {
    /// The full trace vector: goal at position 0, steps at positions 1..
    pub vector: FHRRAlgebra<D>,
    /// The goal's token at position 0.
    pub goal: FHRRAlgebra<D>,
    /// The superposition of the positioned step tokens (zero when there is
    /// no step).
    pub path: FHRRAlgebra<D>,
    /// Frozen digest of [`Self::vector`].
    pub digest: Sha256Digest,
    /// The trace's content identity.
    pub trace_hash: Sha256Digest,
}
