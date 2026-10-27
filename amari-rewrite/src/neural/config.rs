// SPDX-License-Identifier: MIT OR Apache-2.0

//! The validated ranker specification.
//!
//! Katakuri owns the `ia-mlp` architecture grammar and the canonical
//! parameter order ([`katakuri::model::MlpSpec::expected_tensors`]); this
//! module owns the *Amari* half of the contract — the ceilings the design
//! fixes for the inverse ranker and the activation Amari can compute
//! bit-exactly.
//!
//! ## Why ReLU and nothing else
//!
//! [`katakuri::model::Activation`] names four semantics (`relu`, `tanh`,
//! `gelu_erf`, `gelu_tanh`) and deliberately computes none of them, because
//! the arithmetic belongs to the consumer. Amari implements `relu`:
//!
//! - **It is exactly reproducible.** `relu(x) = if x > 0 { x } else { 0 }`
//!   is comparison and selection — no rounding, no library call, the same
//!   bits on every target. `tanh` goes through the platform's libm, and
//!   `gelu_erf` through its `erf`; both can differ by an ulp between
//!   platforms and library versions, which would silently break the
//!   cross-platform bit-identical scoring contract that borsalino-tensor's
//!   fixed-order GEMM establishes. ReLU keeps the whole pipeline exact.
//! - **It preserves non-negativity**, so a hidden activation is bounded by
//!   the magnitude of its own pre-activation: the ceiling argument in
//!   [`crate::neural`] needs no extra clamp.
//!
//! A spec naming any other activation is rejected with a typed error rather
//! than approximated. When Amari ships an exact `tanh`/`gelu` — a pinned
//! polynomial with published error bounds, not a libm call — the supported
//! set grows here and the schema digest stays untouched.

use katakuri::model::{Activation, MlpSpec};

use crate::neural::{NeuralError, CANDIDATE_FEATURE_WIDTH};
use crate::relation::Sha256Digest;

/// A validated `ia-mlp` ranker specification.
///
/// Katakuri's parser already bounds the grammar; this type re-validates the
/// Amari-specific ceilings because [`MlpSpec`]'s fields are public and a
/// hand-built spec never went through the parser.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RankerConfig {
    spec: MlpSpec,
}

impl RankerConfig {
    /// Required input width: the frozen schema's width.
    pub const FEATURE_WIDTH: u32 = CANDIDATE_FEATURE_WIDTH as u32;

    /// Hard ceiling on one hidden layer's width (design resource table).
    pub const MAX_HIDDEN_WIDTH: u32 = 256;

    /// Hard ceiling on hidden layers (Katakuri's own `ia-mlp` ceiling).
    pub const MAX_HIDDEN_LAYERS: usize = 8;

    /// Hard ceiling on the derived parameter count (design resource table).
    pub const MAX_TENSOR_ELEMENTS: u64 = 16_777_216;

    /// Digest frame for [`Self::digest`].
    pub const FRAME: &'static str = "amari.neural.ranker-spec/v1";

    /// Validate a parsed `ia-mlp` specification.
    ///
    /// The checks run ceiling-first: the input width must match the frozen
    /// schema, the derived parameter count must fit the neural tensor
    /// budget, hidden layers must fit their width and count ceilings, and
    /// the activation must be the one Amari computes exactly.
    ///
    /// # Errors
    ///
    /// [`NeuralError::InvalidConfig`] naming the first violated ceiling.
    pub fn from_spec(spec: MlpSpec) -> Result<Self, NeuralError> {
        if spec.input_width != Self::FEATURE_WIDTH {
            return Err(NeuralError::InvalidConfig {
                message: format!(
                    "input_width must be {} (the frozen feature schema width), got {}",
                    Self::FEATURE_WIDTH,
                    spec.input_width
                ),
            });
        }
        let candidate = Self { spec };
        let elements = candidate.tensor_elements();
        if elements > Self::MAX_TENSOR_ELEMENTS {
            return Err(NeuralError::InvalidConfig {
                message: format!(
                    "the spec derives {elements} tensor elements, above the ceiling of {}",
                    Self::MAX_TENSOR_ELEMENTS
                ),
            });
        }
        if candidate.spec.hidden.len() > Self::MAX_HIDDEN_LAYERS {
            return Err(NeuralError::InvalidConfig {
                message: format!(
                    "{} hidden layers, above the ceiling of {}",
                    candidate.spec.hidden.len(),
                    Self::MAX_HIDDEN_LAYERS
                ),
            });
        }
        for (index, layer) in candidate.spec.hidden.iter().enumerate() {
            if layer.width == 0 || layer.width > Self::MAX_HIDDEN_WIDTH {
                return Err(NeuralError::InvalidConfig {
                    message: format!(
                        "hidden[{index}].width is {}, outside 1..={}",
                        layer.width,
                        Self::MAX_HIDDEN_WIDTH
                    ),
                });
            }
        }
        if candidate.spec.head.width != 1 {
            return Err(NeuralError::InvalidConfig {
                message: format!(
                    "head.width must be exactly 1 (a scalar score), got {}",
                    candidate.spec.head.width
                ),
            });
        }
        if candidate.spec.hidden_activation != Activation::Relu {
            return Err(NeuralError::InvalidConfig {
                message: format!(
                    "hidden_activation '{}' is not implemented: Amari computes \
                     'relu' exactly (no libm), so no other activation is \
                     accepted",
                    candidate.spec.hidden_activation.as_str()
                ),
            });
        }
        Ok(candidate)
    }

    /// Parse and validate an `ia-mlp` spec JSON document.
    ///
    /// # Errors
    ///
    /// [`NeuralError::InvalidConfig`] on a Katakuri parse rejection (with
    /// Katakuri's field-level detail) or on any Amari ceiling violation.
    pub fn from_spec_json(json: &str) -> Result<Self, NeuralError> {
        let spec = MlpSpec::from_json(json).map_err(|error| NeuralError::InvalidConfig {
            message: format!("rank spec rejected: {error}"),
        })?;
        Self::from_spec(spec)
    }

    /// The validated Katakuri specification.
    pub fn spec(&self) -> &MlpSpec {
        &self.spec
    }

    /// Hidden layer widths, in order.
    pub fn hidden_widths(&self) -> Vec<u32> {
        self.spec.hidden.iter().map(|layer| layer.width).collect()
    }

    /// The activation the forward pass applies.
    pub fn activation(&self) -> Activation {
        self.spec.hidden_activation
    }

    /// The expected tensors, in Katakuri's canonical parameter order.
    pub fn expected_tensors(&self) -> Vec<(String, Vec<usize>)> {
        self.spec.expected_tensors()
    }

    /// Total parameter count derived from the expected tensor shapes.
    pub fn tensor_elements(&self) -> u64 {
        self.expected_tensors()
            .iter()
            .map(|(_, shape)| {
                shape
                    .iter()
                    .fold(1u64, |product, &dim| product.saturating_mul(dim as u64))
            })
            .fold(0u64, |total, elements| total.saturating_add(elements))
    }

    /// Content digest of the specification: Katakuri's canonical JSON, so
    /// two semantically identical specs hash identically regardless of the
    /// input document's whitespace or key order.
    ///
    /// This is the identity Task 43 binds a checkpoint's weights to.
    pub fn digest(&self) -> Sha256Digest {
        Sha256Digest::framed(Self::FRAME, self.spec.canonical_json().as_bytes())
    }
}
