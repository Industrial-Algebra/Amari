// SPDX-License-Identifier: MIT OR Apache-2.0

//! The bounded MLP candidate ranker.
//!
//! The forward pass is Katakuri's architecture executed on borsalino-tensor's
//! CPU reference:
//!
//! ```text
//! features [rows, 64] --matmul--> [rows, h] --add(bias)--> relu --> ... -->
//! [rows, hidden] --matmul--> [rows, 1] --add(bias)--> scores [rows]
//! ```
//!
//! Three properties are contractual:
//!
//! - **Bit-identical rows.** borsalino-tensor's `matmul` accumulates each
//!   output element along ascending `k` in f32, so row `r` of a `rows`-row
//!   batch is bit-for-bit the same as scoring that row alone. Batching is
//!   therefore a throughput decision, never a numeric one.
//! - **Canonical order.** Candidates are sorted by their canonical
//!   alpha-invariant state digest *before* the batch is built, and the
//!   result is sorted by `(score, canonical digest)`. Request order can
//!   never leak into a ranking, so ties resolve identically everywhere.
//! - **Sanitized failure.** A backend rejection becomes
//!   [`NeuralError::BackendRejected`] naming the forward-pass stage; the
//!   backend's own error value is dropped, not re-wrapped, so no
//!   `borsalino_tensor` type or shape dump crosses this boundary.
//!
//! ## Weights are a caller-validated seam
//!
//! [`RankerWeights::from_tensors`] is the only way to build weights here.
//! It takes the *canonical* tensor set (Katakuri's
//! `expected_tensors` order, transposed to the row-major `[in, out]`
//! layout borsalino-tensor's `matmul` contracts), and rejects wrong
//! shapes, missing/duplicate/unknown tensors, non-finite values, and values
//! above [`RankerWeights::MAX_ABSOLUTE_WEIGHT`]. Task 43 fills this seam
//! from a validated checkpoint; nothing else needs to know a file format.
//!
//! Transposing at construction is a load-time step, not a compute step: it
//! is exact (no arithmetic), it happens once, and it keeps the scored path
//! to `matmul` + `add` + `relu`. borsalino-tensor has no transpose op in
//! T1, and it does not need one for this — layout is the consumer's
//! concern.

use borsalino_tensor::dtype::Dtype;
use borsalino_tensor::exec::{Executable, View};
use borsalino_tensor::layout::{Shape, TensorLayout};
use borsalino_tensor::ops::l0::Add;
use borsalino_tensor::ops::l2::MatMul;

use crate::inverse::SymbolicState;
use crate::neural::config::RankerConfig;
use crate::neural::encode::{CandidateInput, FeatureEncoder, FeatureVector};
use crate::neural::{DifferentiableRule, NeuralError, CANDIDATE_FEATURE_WIDTH};
use crate::relation::Sha256Digest;

/// Hard ceiling on candidates scored in one batch.
///
/// The design's neural row ceilings a training corpus at 65,536 examples;
/// one ranking call is bounded by the same number so a batch can never
/// outgrow the corpus it could have been trained on. It also keeps every
/// batch buffer inside the neural tensor budget: 65,536 rows x 256 hidden
/// units is exactly 16,777,216 elements, the same number
/// [`RankerConfig::MAX_TENSOR_ELEMENTS`] caps.
pub const MAX_RANKED_CANDIDATES: usize = 65_536;

/// One layer's parameters, in the transposed `[in, out]` layout.
#[derive(Clone, Debug, PartialEq)]
struct LayerWeights {
    /// `[in, out]` row-major: `weight_t[input * out + output]`.
    weight_t: Vec<f32>,
    /// `[out]` biases.
    bias: Vec<f32>,
    /// Input width.
    in_width: usize,
    /// Output width.
    out_width: usize,
}

/// Validated ranker weights, in Katakuri's canonical parameter order.
///
/// Construction validates every tensor; the type has no other constructor,
/// so "the weights are finite and bounded" is a property of holding one.
#[derive(Clone, Debug, PartialEq)]
pub struct RankerWeights {
    layers: Vec<LayerWeights>,
    head: LayerWeights,
    /// Digest frame for [`Self::digest`].
    digest: Sha256Digest,
    element_count: usize,
}

impl RankerWeights {
    /// Absolute ceiling on one weight's magnitude.
    ///
    /// This is a ceiling, never the effective bound: the bound a given
    /// architecture can carry is
    /// [`RankerConfig::max_weight_magnitude`], which composes the layer
    /// widths into an activation bound and is always `<= this` value (PR
    /// #353 round 1 P1b — a fixed ceiling cannot hold eight 256-wide layers
    /// finite, because each of them multiplies the activation by
    /// `256 * M`).
    ///
    /// It stays meaningful for two reasons: it caps the architectures whose
    /// derived bound would otherwise be absurdly loose (`1e6` is already
    /// three orders of magnitude above real weights, which live around
    /// `1e-2 .. 1e1`), and it is the single number the derived formula
    /// saturates at, so "no weight above 1e6, ever" remains true.
    pub const MAX_ABSOLUTE_WEIGHT: f32 = 1_000_000.0;

    /// Digest frame for [`Self::digest`].
    pub const FRAME: &'static str = "amari.neural.ranker-weights/v1";

    /// Validate a canonical tensor set against a configuration.
    ///
    /// `tensors` are `(name, row-major f32 elements)` in any order; names
    /// must match [`RankerConfig::expected_tensors`] exactly, each appearing
    /// once.
    ///
    /// # Errors
    ///
    /// [`NeuralError::InvalidWeights`] on an unknown, missing, or duplicate
    /// tensor name, a length that disagrees with the declared shape, a
    /// non-finite value, or a magnitude above
    /// [`Self::MAX_ABSOLUTE_WEIGHT`].
    pub fn from_tensors(
        config: &RankerConfig,
        tensors: &[(&str, &[f32])],
    ) -> Result<Self, NeuralError> {
        let expected = config.expected_tensors();
        let magnitude_bound = config.max_weight_magnitude();
        // Reject an unknown name on first sight, rendering a bounded form of
        // the caller's name: a diagnostic must never copy an unbounded input
        // (PR #353 round 3 P2b).
        for (name, _) in tensors {
            if !expected.iter().any(|(known, _)| known == name) {
                return Err(NeuralError::InvalidWeights {
                    message: format!(
                        "unknown tensor `{}` for this ranker spec",
                        render_tensor_name(name)
                    ),
                });
            }
        }
        let mut found: Vec<(&str, &[f32])> = Vec::with_capacity(expected.len());
        for (name, shape) in &expected {
            // Count occurrences without collecting them: a million duplicate
            // names must cost a counter, not a vector (PR #353 round 3 P2b).
            let mut count = 0usize;
            let mut occurrence: Option<&[f32]> = None;
            for (other, values) in tensors {
                if other != name {
                    continue;
                }
                count += 1;
                if count == 1 {
                    occurrence = Some(values);
                }
            }
            if count != 1 {
                return Err(NeuralError::InvalidWeights {
                    message: format!("tensor `{name}` must appear exactly once, found {count}"),
                });
            }
            let Some(values) = occurrence else {
                return Err(NeuralError::InvalidWeights {
                    message: format!("tensor `{name}` is missing"),
                });
            };
            let elements: usize = shape.iter().product();
            if values.len() != elements {
                return Err(NeuralError::InvalidWeights {
                    message: format!(
                        "tensor `{name}` declares {elements} elements (shape {shape:?}), got {}",
                        values.len()
                    ),
                });
            }
            for (index, value) in values.iter().enumerate() {
                if !value.is_finite() {
                    return Err(NeuralError::InvalidWeights {
                        message: format!("tensor `{name}` element {index} is not finite"),
                    });
                }
                if value.abs() > magnitude_bound {
                    return Err(NeuralError::InvalidWeights {
                        message: format!(
                            "tensor `{name}` element {index} is {} in magnitude, above this \
                             architecture's bound of {magnitude_bound} (weights {:?})",
                            value.abs(),
                            config.hidden_widths()
                        ),
                    });
                }
            }
            found.push((name.as_str(), values));
        }

        let mut layers = Vec::with_capacity(config.spec().hidden.len());
        let mut in_width = RankerConfig::FEATURE_WIDTH as usize;
        for (index, layer) in config.spec().hidden.iter().enumerate() {
            let out_width = layer.width as usize;
            let weight = tensor(&found, &format!("hidden.{index}.weight"))?;
            let bias = tensor(&found, &format!("hidden.{index}.bias"))?;
            layers.push(LayerWeights {
                weight_t: transpose(weight, out_width, in_width),
                bias: bias.to_vec(),
                in_width,
                out_width,
            });
            in_width = out_width;
        }
        let head_weight = tensor(&found, "head.weight")?;
        let head_bias = tensor(&found, "head.bias")?;
        let head = LayerWeights {
            weight_t: transpose(head_weight, 1, in_width),
            bias: head_bias.to_vec(),
            in_width,
            out_width: 1,
        };

        let element_count = found.iter().map(|(_, values)| values.len()).sum();
        let digest = weights_digest(&expected, &found);
        Ok(Self {
            layers,
            head,
            digest,
            element_count,
        })
    }

    /// The number of parameters in this weight set.
    pub fn element_count(&self) -> usize {
        self.element_count
    }

    /// Content digest over the canonical parameter order: name, shape-free
    /// element bytes. Two weight sets agree iff they agree bit for bit in
    /// canonical order.
    pub fn digest(&self) -> Sha256Digest {
        self.digest
    }

    /// The `(input_width, output_width)` pair of every layer, head last.
    fn shape_signature(&self) -> Vec<(usize, usize)> {
        let mut signature: Vec<(usize, usize)> = self
            .layers
            .iter()
            .map(|layer| (layer.in_width, layer.out_width))
            .collect();
        signature.push((self.head.in_width, self.head.out_width));
        signature
    }
}

/// One candidate's canonical identity, frozen features, and ranker score.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RankedCandidate {
    digest: Sha256Digest,
    features: FeatureVector,
    score: f32,
}

impl RankedCandidate {
    /// The candidate's joint alpha-canonical state digest.
    pub fn canonical_digest(&self) -> Sha256Digest {
        self.digest
    }

    /// The frozen schema's feature vector for this candidate.
    pub fn features(&self) -> &FeatureVector {
        &self.features
    }

    /// The ranker's score. Lower is better: the result order is ascending.
    pub fn score(&self) -> f32 {
        self.score
    }
}

/// The bounded MLP ranker: validated spec plus validated weights.
#[derive(Clone, Debug, PartialEq)]
pub struct MlpRanker {
    config: RankerConfig,
    weights: RankerWeights,
}

impl MlpRanker {
    /// Bind validated weights to a compatible configuration.
    ///
    /// # Errors
    ///
    /// [`NeuralError::InvalidWeights`] when the weight tensor shapes do not
    /// match the configuration's layers (only reachable when the pair was
    /// assembled from two different specs).
    pub fn new(config: RankerConfig, weights: RankerWeights) -> Result<Self, NeuralError> {
        let mut expected: Vec<(usize, usize)> = Vec::new();
        let mut in_width = RankerConfig::FEATURE_WIDTH as usize;
        for layer in config.spec().hidden.iter() {
            expected.push((in_width, layer.width as usize));
            in_width = layer.width as usize;
        }
        expected.push((in_width, 1));
        if weights.shape_signature() != expected {
            return Err(NeuralError::InvalidWeights {
                message: format!(
                    "weight layer shapes {:?} do not match the spec's {:?}",
                    weights.shape_signature(),
                    expected
                ),
            });
        }
        Ok(Self { config, weights })
    }

    /// The bound configuration.
    pub fn config(&self) -> &RankerConfig {
        &self.config
    }

    /// The bound weights.
    pub fn weights(&self) -> &RankerWeights {
        &self.weights
    }

    /// Score one already-encoded feature vector.
    ///
    /// # Errors
    ///
    /// [`NeuralError::BackendRejected`] when the tensor backend rejects a
    /// stage, [`NeuralError::NonFiniteScore`] when the head produces a
    /// non-finite score (unreachable under the validated bounds).
    pub fn score_features(&self, features: &FeatureVector) -> Result<f32, NeuralError> {
        let scores = self.forward(features.lanes(), 1)?;
        let score = scores[0];
        if !score.is_finite() {
            return Err(NeuralError::NonFiniteScore { stage: "head" });
        }
        Ok(score)
    }

    /// Encode and score one candidate.
    ///
    /// # Errors
    ///
    /// Any encoder error, plus the forward-pass errors of
    /// [`Self::score_features`].
    pub fn score_candidate(
        &self,
        encoder: &FeatureEncoder,
        candidate: &CandidateInput<'_>,
    ) -> Result<RankedCandidate, NeuralError> {
        let prepared = self.prepare(encoder, candidate)?;
        let score = self.score_features(&prepared.features)?;
        Ok(RankedCandidate {
            digest: prepared.digest,
            features: prepared.features,
            score,
        })
    }

    /// Encode, canonically order, and score a batch of candidates.
    ///
    /// The returned order is `(score ascending, content key ascending)`.
    /// The content key starts with the canonical state digest, so distinct
    /// states order by digest; candidates for the same state are separated
    /// by their provenance and search context, which are exactly the
    /// differences the feature vector encodes. The resulting ranking is
    /// therefore a function of the candidates' content: permuting the
    /// request cannot change it, and duplicates are not collapsed.
    ///
    /// Admission precedes canonicalization: each candidate is encoded
    /// (bounded, iterative) before its digest is derived (which clones the
    /// term), so an over-large term returns a typed limit error rather than
    /// aborting inside `Term::clone`.
    ///
    /// # Errors
    ///
    /// [`NeuralError::LimitExceeded`] when the batch exceeds
    /// [`MAX_RANKED_CANDIDATES`], any encoder error, plus the forward-pass
    /// errors of [`Self::score_features`].
    pub fn rank(
        &self,
        encoder: &FeatureEncoder,
        candidates: &[CandidateInput<'_>],
    ) -> Result<Vec<RankedCandidate>, NeuralError> {
        if candidates.len() > MAX_RANKED_CANDIDATES {
            return Err(NeuralError::LimitExceeded {
                resource: "ranked candidates",
                value: candidates.len(),
                limit: MAX_RANKED_CANDIDATES,
            });
        }
        if candidates.is_empty() {
            return Ok(Vec::new());
        }
        let mut prepared: Vec<Prepared> = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            prepared.push(self.prepare(encoder, candidate)?);
        }
        // Canonicalize before batching: the batch is a pure throughput
        // device, so its row order must not be the request's.
        prepared.sort_by(|left, right| left.key.cmp(&right.key));

        let mut batch = Vec::with_capacity(prepared.len() * CANDIDATE_FEATURE_WIDTH);
        for prepared in &prepared {
            batch.extend_from_slice(prepared.features.lanes());
        }
        let scores = self.forward(&batch, prepared.len())?;

        let mut scored: Vec<(Prepared, f32)> = prepared.into_iter().zip(scores).collect();
        if scored.iter().any(|(_, score)| !score.is_finite()) {
            return Err(NeuralError::NonFiniteScore { stage: "head" });
        }
        // Total order: score, then the candidate's content key. Two
        // candidates can tie only if every input the ranker observed is
        // identical, in which case any order is the same order.
        scored.sort_by(|left, right| {
            left.1
                .total_cmp(&right.1)
                .then_with(|| left.0.key.cmp(&right.0.key))
        });
        Ok(scored
            .into_iter()
            .map(|(prepared, score)| RankedCandidate {
                digest: prepared.digest,
                features: prepared.features,
                score,
            })
            .collect())
    }

    /// Admit, encode, and identify one candidate.
    ///
    /// The order is contractual, not incidental: [`FeatureEncoder::encode`]
    /// is the bounded, fully iterative admission pass, and everything after
    /// it is safe only because admission succeeded. The canonical digest
    /// clones the term (`SymbolicState::new`) and canonicalization walks it
    /// recursively, so deriving the identity first would let an over-large
    /// term abort the process inside `Term::clone` instead of returning a
    /// typed limit error (PR #353 round 1 P1a).
    fn prepare(
        &self,
        encoder: &FeatureEncoder,
        candidate: &CandidateInput<'_>,
    ) -> Result<Prepared, NeuralError> {
        let features = encoder.encode(candidate)?;
        let digest = canonical_digest(candidate);
        let key = content_key(candidate, digest);
        Ok(Prepared {
            key,
            digest,
            features,
        })
    }

    /// The forward pass over `rows` feature rows of width 64.
    ///
    /// Every stage's magnitude is bounded by the derivation behind
    /// [`RankerConfig::max_weight_magnitude`], which is what makes
    /// [`NeuralError::BackendRejected`] a defence-in-depth branch rather
    /// than an expected path.
    fn forward(&self, features: &[f32], rows: usize) -> Result<Vec<f32>, NeuralError> {
        let mut activations = features.to_vec();
        let mut width = RankerConfig::FEATURE_WIDTH as usize;
        for layer in &self.weights.layers {
            let hidden = matmul(
                &activations,
                [rows, width],
                &layer.weight_t,
                [width, layer.out_width],
                "hidden matmul",
            )?;
            let mut biased = vec![0.0f32; hidden.len()];
            add(
                &hidden,
                [rows, layer.out_width],
                &layer.bias,
                [layer.out_width],
                &mut biased,
                "hidden bias add",
            )?;
            relu(&mut biased);
            activations = biased;
            width = layer.out_width;
        }
        let scores = matmul(
            &activations,
            [rows, width],
            &self.weights.head.weight_t,
            [width, 1],
            "head matmul",
        )?;
        let mut out = vec![0.0f32; scores.len()];
        add(
            &scores,
            [rows, 1],
            &self.weights.head.bias,
            [1],
            &mut out,
            "head bias add",
        )?;
        Ok(out)
    }
}

/// The ranker as a differentiable rule over feature/score vectors.
///
/// `forward` maps one frozen feature vector (length
/// [`CANDIDATE_FEATURE_WIDTH`]) to its one-element score vector, and `loss`
/// is the squared error of two score vectors — the differentiable surrogate
/// Task 42's pairwise trainer consumes. `Gradient` is the unit placeholder
/// because this task ships the forward pass only; backpropagation belongs to
/// the trainer.
impl DifferentiableRule<Vec<f32>> for MlpRanker {
    type Parameters = RankerWeights;
    type Gradient = ();
    type Error = NeuralError;

    fn forward(&self, state: &Vec<f32>) -> Result<Vec<f32>, NeuralError> {
        let lanes: [f32; CANDIDATE_FEATURE_WIDTH] =
            state
                .as_slice()
                .try_into()
                .map_err(|_| NeuralError::InvalidFeatures {
                    message: "ranker input must be one feature vector of the frozen width",
                })?;
        let features = FeatureVector::new(lanes)?;
        Ok(Vec::from([self.score_features(&features)?]))
    }

    fn loss(&self, predicted: &Vec<f32>, target: &Vec<f32>) -> Result<f64, NeuralError> {
        if predicted.len() != 1 || target.len() != 1 {
            return Err(NeuralError::InvalidFeatures {
                message: "ranker loss expects one-element score vectors",
            });
        }
        if !predicted[0].is_finite() || !target[0].is_finite() {
            return Err(NeuralError::InvalidFeatures {
                message: "ranker loss expects finite scores",
            });
        }
        let difference = f64::from(predicted[0]) - f64::from(target[0]);
        Ok(difference * difference)
    }
}

/// One admitted candidate: its ordering key, canonical identity, and
/// features.
struct Prepared {
    /// Deterministic content key: the total order's tie-break.
    key: Vec<u8>,
    /// Joint alpha-canonical state identity.
    digest: Sha256Digest,
    /// The frozen schema's feature vector.
    features: FeatureVector,
}

/// The joint alpha-canonical identity the ranking is keyed by.
///
/// Only call this on an admitted candidate: it clones the term, and
/// `Term::clone` recurses, so an over-large term must already have been
/// rejected by the encoder (see [`MlpRanker::prepare`]).
fn canonical_digest(candidate: &CandidateInput<'_>) -> Sha256Digest {
    SymbolicState::new(candidate.term.clone(), candidate.constraints.clone()).canonical_digest()
}

/// The deterministic content key that breaks every remaining tie.
///
/// The state digest identifies the canonical state, but two candidates for
/// the same state can differ in search context (depth, branch factor,
/// growth, existentials, rule priority) and in provenance (rule, position,
/// scope, target/predecessor hashes). Those differences are observable —
/// they are exactly what the search-context and position lanes encode — so
/// the total order must cover them. With this key the returned ranking is a
/// function of the candidates' content alone; permuting the request cannot
/// change it (PR #353 round 1 P2b).
///
/// The key starts with the state digest, so distinct states keep the
/// documented digest-ascending order among equal scores.
fn content_key(candidate: &CandidateInput<'_>, digest: Sha256Digest) -> Vec<u8> {
    let mut key = Vec::with_capacity(160);
    key.extend_from_slice(digest.as_bytes());
    let provenance = candidate.provenance;
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
    let context = candidate.context;
    key.extend_from_slice(&context.depth.to_le_bytes());
    key.extend_from_slice(&context.branch_factor.to_le_bytes());
    key.push(u8::from(context.cycle));
    key.extend_from_slice(&context.term_growth.to_le_bytes());
    key.extend_from_slice(&context.constraint_growth.to_le_bytes());
    key.extend_from_slice(&(context.existentials as u64).to_le_bytes());
    key.extend_from_slice(&context.rule_priority.to_le_bytes());
    key
}

/// Byte cap on a caller-supplied name quoted into a diagnostic.
///
/// A rejection message must never copy an unbounded input: quoting a 16 MB
/// tensor name verbatim would allocate 16 MB to say "unknown tensor". The
/// cap truncates on a UTF-8 boundary and always states the real length, so
/// the message stays recognisable and bounded (PR #353 round 3 P2b).
const MAX_DIAGNOSTIC_NAME_BYTES: usize = 64;

/// Render a caller-supplied tensor name for a diagnostic: bounded, lossy
/// beyond [`MAX_DIAGNOSTIC_NAME_BYTES`], and always length-stamped.
fn render_tensor_name(name: &str) -> String {
    if name.len() <= MAX_DIAGNOSTIC_NAME_BYTES {
        return name.to_string();
    }
    let mut end = MAX_DIAGNOSTIC_NAME_BYTES;
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}... ({} bytes)", &name[..end], name.len())
}

/// Tensor values by canonical name (already validated present).
fn tensor<'a>(found: &[(&str, &'a [f32])], name: &str) -> Result<&'a [f32], NeuralError> {
    found
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(_, values)| *values)
        .ok_or_else(|| NeuralError::InvalidWeights {
            message: format!("tensor `{name}` is missing"),
        })
}

/// Reinterpret `[out, in]` row-major weights as `[in, out]` row-major.
///
/// Exact: a permutation of elements, never an arithmetic step.
fn transpose(weight: &[f32], out_width: usize, in_width: usize) -> Vec<f32> {
    let mut transposed = vec![0.0f32; weight.len()];
    for output in 0..out_width {
        for input in 0..in_width {
            transposed[input * out_width + output] = weight[output * in_width + input];
        }
    }
    transposed
}

/// Digest over the canonical parameter order.
fn weights_digest(expected: &[(String, Vec<usize>)], found: &[(&str, &[f32])]) -> Sha256Digest {
    let mut payload = Vec::new();
    for (name, _) in expected {
        payload.extend_from_slice(&(name.len() as u32).to_le_bytes());
        payload.extend_from_slice(name.as_bytes());
        if let Some((_, values)) = found.iter().find(|(known, _)| known == name) {
            payload.extend_from_slice(&(values.len() as u32).to_le_bytes());
            for value in *values {
                payload.extend_from_slice(&value.to_bits().to_le_bytes());
            }
        }
    }
    Sha256Digest::framed(RankerWeights::FRAME, &payload)
}

/// ReLU: `max(x, 0)` for finite `x`, applied Amari-side because
/// borsalino-tensor T1 ships no activation op (and needs none: this is one
/// comparison per element, not a reduction).
fn relu(values: &mut [f32]) {
    for value in values.iter_mut() {
        if *value > 0.0 {
            continue;
        }
        *value = 0.0;
    }
}

/// `[rows, k] x [k, n]` -> `[rows, n]` on the borsalino-tensor CPU reference.
fn matmul(
    lhs: &[f32],
    lhs_row_major: [usize; 2],
    rhs: &[f32],
    rhs_row_major: [usize; 2],
    stage: &'static str,
) -> Result<Vec<f32>, NeuralError> {
    let lhs_shape = Shape::new(&[
        u32::try_from(lhs_row_major[0]).map_err(|_| rejected(stage))?,
        u32::try_from(lhs_row_major[1]).map_err(|_| rejected(stage))?,
    ])
    .map_err(|_| rejected(stage))?;
    let rhs_shape = Shape::new(&[
        u32::try_from(rhs_row_major[0]).map_err(|_| rejected(stage))?,
        u32::try_from(rhs_row_major[1]).map_err(|_| rejected(stage))?,
    ])
    .map_err(|_| rejected(stage))?;
    let lhs_layout = TensorLayout::row_major(Dtype::F32, lhs_shape).map_err(|_| rejected(stage))?;
    let rhs_layout = TensorLayout::row_major(Dtype::F32, rhs_shape).map_err(|_| rejected(stage))?;
    let mut out = vec![0.0f32; lhs_row_major[0].saturating_mul(rhs_row_major[1])];
    <MatMul as Executable>::execute(
        (View::new(&lhs_layout, lhs), View::new(&rhs_layout, rhs)),
        &mut out,
    )
    .map_err(|_| rejected(stage))?;
    Ok(out)
}

/// Elementwise `add` with borsalino-tensor's NumPy-style broadcasting (the
/// bias vector repeats across the batch axis).
fn add(
    lhs: &[f32],
    lhs_row_major: [usize; 2],
    rhs: &[f32],
    rhs_row_major: [usize; 1],
    out: &mut [f32],
    stage: &'static str,
) -> Result<(), NeuralError> {
    let lhs_shape = Shape::new(&[
        u32::try_from(lhs_row_major[0]).map_err(|_| rejected(stage))?,
        u32::try_from(lhs_row_major[1]).map_err(|_| rejected(stage))?,
    ])
    .map_err(|_| rejected(stage))?;
    let rhs_shape = Shape::new(&[u32::try_from(rhs_row_major[0]).map_err(|_| rejected(stage))?])
        .map_err(|_| rejected(stage))?;
    let lhs_layout = TensorLayout::row_major(Dtype::F32, lhs_shape).map_err(|_| rejected(stage))?;
    let rhs_layout = TensorLayout::row_major(Dtype::F32, rhs_shape).map_err(|_| rejected(stage))?;
    <Add as Executable>::execute(
        (View::new(&lhs_layout, lhs), View::new(&rhs_layout, rhs)),
        out,
    )
    .map_err(|_| rejected(stage))
}

/// A sanitized backend rejection: the stage is named, nothing else is.
fn rejected(stage: &'static str) -> NeuralError {
    NeuralError::BackendRejected { stage }
}
