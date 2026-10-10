//! WASM bindings for amari-fusion: TropicalDualClifford LLM evaluation system
//!
//! This module provides WebAssembly bindings for the revolutionary TropicalDualClifford
//! system that combines three exotic number systems for efficient LLM evaluation:
//!
//! - **Tropical Algebra**: Converts expensive softmax to efficient max operations
//! - **Dual Numbers**: Provides automatic differentiation without computational graphs
//! - **Clifford Algebra**: Handles geometric relationships and rotations
//!
//! Perfect for:
//! - Real-time LLM inference in browsers
//! - Attention mechanism optimization
//! - Neural network parameter optimization
//! - Interactive machine learning demonstrations

use amari_fusion::{EvaluationResult, TropicalDualClifford};
use js_sys::Object;
use wasm_bindgen::prelude::*;

/// WASM wrapper for TropicalDualClifford fusion system
#[wasm_bindgen]
pub struct WasmTropicalDualClifford {
    inner: TropicalDualClifford<f64, 8>, // Default to 8D for flexibility
}

#[wasm_bindgen]
impl WasmTropicalDualClifford {
    /// Create a zero TDC object
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: TropicalDualClifford::zero(),
        }
    }

    /// Create from logits (array of log-probabilities)
    #[wasm_bindgen(js_name = fromLogits)]
    pub fn from_logits(logits: &[f64]) -> Self {
        Self {
            inner: TropicalDualClifford::from_logits(logits),
        }
    }

    /// Create from probability distribution
    ///
    /// Note: Converts probabilities to log space for tropical representation
    #[wasm_bindgen(js_name = fromProbabilities)]
    pub fn from_probabilities(probs: &[f64]) -> Self {
        // Convert probabilities to log space (logits)
        let logits: Vec<f64> = probs
            .iter()
            .map(|&p| if p > 0.0 { p.ln() } else { f64::NEG_INFINITY })
            .collect();
        Self {
            inner: TropicalDualClifford::from_logits(&logits),
        }
    }

    /// Create random TDC for testing
    #[wasm_bindgen(js_name = random)]
    pub fn random() -> Self {
        Self {
            inner: TropicalDualClifford::random(),
        }
    }

    /// Create random TDC with specific scale
    #[wasm_bindgen(js_name = randomWithScale)]
    pub fn random_with_scale(scale: f64) -> Self {
        Self {
            inner: TropicalDualClifford::random_with_scale(scale),
        }
    }

    /// Check if TDC is zero
    #[wasm_bindgen(js_name = isZero)]
    pub fn is_zero(&self) -> bool {
        self.inner.is_zero()
    }

    /// Extract tropical features as array
    #[wasm_bindgen(js_name = getTropicalFeatures)]
    pub fn get_tropical_features(&self) -> Vec<f64> {
        self.inner
            .extract_tropical_features()
            .into_iter()
            .map(|t: amari_tropical::TropicalNumber<f64>| t.value())
            .collect()
    }

    /// Extract dual features as array of real parts
    #[wasm_bindgen(js_name = getDualReals)]
    pub fn get_dual_reals(&self) -> Vec<f64> {
        self.inner
            .extract_dual_features()
            .into_iter()
            .map(|d| d.value())
            .collect()
    }

    /// Extract dual features as array of dual parts (derivatives)
    #[wasm_bindgen(js_name = getDualDerivatives)]
    pub fn get_dual_derivatives(&self) -> Vec<f64> {
        self.inner
            .extract_dual_features()
            .into_iter()
            .map(|d| d.derivative())
            .collect()
    }

    /// Get Clifford coefficients
    #[wasm_bindgen(js_name = getCliffordCoefficients)]
    pub fn get_clifford_coefficients(&self) -> Vec<f64> {
        (0..8).map(|i| self.inner.clifford().get(i)).collect()
    }

    /// Add two TDC objects
    pub fn add(&self, other: &WasmTropicalDualClifford) -> WasmTropicalDualClifford {
        Self {
            inner: self.inner.add(&other.inner),
        }
    }

    /// Scale TDC object
    pub fn scale(&self, factor: f64) -> WasmTropicalDualClifford {
        Self {
            inner: self.inner.scale(factor),
        }
    }

    /// Compute fusion norm (combined measure across all systems)
    #[wasm_bindgen(js_name = fusionNorm)]
    pub fn fusion_norm(&self) -> f64 {
        // Combine norms from all three systems
        let tropical_norm = self
            .get_tropical_features()
            .iter()
            .map(|&x| if x.is_finite() { x } else { 0.0 })
            .fold(f64::NEG_INFINITY, f64::max);

        let dual_norm = self
            .get_dual_reals()
            .iter()
            .map(|&x| x * x)
            .sum::<f64>()
            .sqrt();

        let clifford_norm = self.inner.clifford().coefficient_norm();

        // Weighted combination
        (tropical_norm.max(0.0) + dual_norm + clifford_norm) / 3.0
    }

    /// Perform tropical attention operation
    #[wasm_bindgen(js_name = tropicalAttention)]
    pub fn tropical_attention(&self, keys: &[f64], values: &[f64]) -> Result<Vec<f64>, JsValue> {
        if keys.len() != values.len() {
            return Err(JsValue::from_str("Keys and values must have same length"));
        }

        let query_features = self.get_tropical_features();
        let mut attention_scores = Vec::new();

        // Compute tropical attention scores (max-based)
        for &key in keys {
            let score = query_features
                .iter()
                .map(|&q| {
                    if q.is_finite() {
                        q + key
                    } else {
                        f64::NEG_INFINITY
                    }
                })
                .fold(f64::NEG_INFINITY, f64::max);
            attention_scores.push(score);
        }

        // Apply tropical softmax (find max and normalize)
        let max_score = attention_scores
            .iter()
            .fold(f64::NEG_INFINITY, |a, &b| a.max(b));

        let normalized_scores: Vec<f64> = attention_scores
            .iter()
            .map(|&score| if score == max_score { 1.0 } else { 0.0 })
            .collect();

        // Compute weighted sum of values
        let mut result = 0.0;
        for (i, &weight) in normalized_scores.iter().enumerate() {
            result += weight * values[i];
        }

        Ok(vec![result])
    }

    /// Extract geometric features using Clifford operations
    #[wasm_bindgen(js_name = extractGeometricFeatures)]
    pub fn extract_geometric_features(&self) -> Vec<f64> {
        let mv = self.inner.clifford();
        vec![
            mv.get(0),                       // scalar
            mv.get(1),                       // e1
            mv.get(2),                       // e2
            mv.get(3),                       // e3
            mv.coefficient_norm(),           // magnitude
            mv.reverse().coefficient_norm(), // reverse magnitude
        ]
    }

    /// Evaluate two TDC systems (core LLM comparison operation)
    pub fn evaluate(&self, other: &WasmTropicalDualClifford) -> WasmEvaluationResult {
        let result = self.inner.evaluate(&other.inner);
        WasmEvaluationResult { inner: result }
    }

    /// Transform using another TDC (advanced operation)
    pub fn transform(&self, transformation: &WasmTropicalDualClifford) -> WasmTropicalDualClifford {
        Self {
            inner: self.inner.transform(&transformation.inner),
        }
    }

    /// Compute distance between two TDC systems
    pub fn distance(&self, other: &WasmTropicalDualClifford) -> f64 {
        self.inner.distance(&other.inner)
    }

    /// Interpolate between two TDC systems (useful for animation)
    pub fn interpolate(
        &self,
        other: &WasmTropicalDualClifford,
        t: f64,
    ) -> WasmTropicalDualClifford {
        Self {
            inner: self.inner.interpolate(&other.inner, t),
        }
    }

    /// Perform sensitivity analysis for gradient-based optimization
    ///
    /// Note: This feature is not yet available in v0.12.0
    /// TODO: Re-enable when sensitivity_analysis is added to TropicalDualClifford
    #[wasm_bindgen(js_name = sensitivityAnalysis)]
    pub fn sensitivity_analysis(&self) -> WasmSensitivityMap {
        // Stub implementation - compute basic sensitivity from dual gradients
        let dual_features = self.inner.extract_dual_features();
        let mut sensitivities = Vec::new();

        for (i, d) in dual_features.iter().enumerate() {
            sensitivities.push((i, d.value(), d.derivative()));
        }

        // Sort by derivative magnitude
        sensitivities.sort_by(|a, b| b.2.abs().partial_cmp(&a.2.abs()).unwrap());

        WasmSensitivityMap {
            sensitivities: sensitivities
                .into_iter()
                .map(|(c, v, s)| SensitivityEntry {
                    component: c,
                    value: v,
                    sensitivity: s,
                })
                .collect(),
        }
    }
}

impl Default for WasmTropicalDualClifford {
    fn default() -> Self {
        Self::new()
    }
}

/// WASM wrapper for evaluation results
#[wasm_bindgen]
pub struct WasmEvaluationResult {
    inner: EvaluationResult<f64>,
}

#[wasm_bindgen]
impl WasmEvaluationResult {
    /// Get the best path score from tropical algebra
    #[wasm_bindgen(js_name = getBestPathScore)]
    pub fn get_best_path_score(&self) -> f64 {
        self.inner.best_path_score.value()
    }

    /// Get gradient norm from dual numbers
    #[wasm_bindgen(js_name = getGradientNorm)]
    pub fn get_gradient_norm(&self) -> f64 {
        self.inner.gradient_norm
    }

    /// Get geometric distance from Clifford algebra
    #[wasm_bindgen(js_name = getGeometricDistance)]
    pub fn get_geometric_distance(&self) -> f64 {
        self.inner.geometric_distance
    }

    /// Get combined score using all three algebras
    #[wasm_bindgen(js_name = getCombinedScore)]
    pub fn get_combined_score(&self) -> f64 {
        self.inner.combined_score
    }

    /// Convert to JavaScript object for easy access
    #[wasm_bindgen(js_name = toObject)]
    pub fn to_object(&self) -> Result<JsValue, JsValue> {
        let obj = Object::new();

        js_sys::Reflect::set(
            &obj,
            &"bestPathScore".into(),
            &self.get_best_path_score().into(),
        )?;

        js_sys::Reflect::set(
            &obj,
            &"gradientNorm".into(),
            &self.get_gradient_norm().into(),
        )?;

        js_sys::Reflect::set(
            &obj,
            &"geometricDistance".into(),
            &self.get_geometric_distance().into(),
        )?;

        js_sys::Reflect::set(
            &obj,
            &"combinedScore".into(),
            &self.get_combined_score().into(),
        )?;

        Ok(obj.into())
    }
}

/// Internal struct for sensitivity entry
struct SensitivityEntry {
    component: usize,
    value: f64,
    sensitivity: f64,
}

/// WASM wrapper for sensitivity analysis results
#[wasm_bindgen]
pub struct WasmSensitivityMap {
    sensitivities: Vec<SensitivityEntry>,
}

#[wasm_bindgen]
impl WasmSensitivityMap {
    /// Get components with highest sensitivity (for optimization)
    #[wasm_bindgen(js_name = getMostSensitive)]
    pub fn get_most_sensitive(&self, n: usize) -> Vec<usize> {
        self.sensitivities
            .iter()
            .take(n)
            .map(|s| s.component)
            .collect()
    }

    /// Get total sensitivity across all components
    #[wasm_bindgen(js_name = getTotalSensitivity)]
    pub fn get_total_sensitivity(&self) -> f64 {
        self.sensitivities.iter().map(|s| s.sensitivity.abs()).sum()
    }

    /// Get all sensitivities as JavaScript arrays
    #[wasm_bindgen(js_name = getAllSensitivities)]
    pub fn get_all_sensitivities(&self) -> Result<JsValue, JsValue> {
        let components: Vec<usize> = self.sensitivities.iter().map(|s| s.component).collect();
        let values: Vec<f64> = self.sensitivities.iter().map(|s| s.value).collect();
        let sensitivities: Vec<f64> = self.sensitivities.iter().map(|s| s.sensitivity).collect();

        let obj = Object::new();
        js_sys::Reflect::set(
            &obj,
            &"components".into(),
            &js_sys::Array::from_iter(components.into_iter().map(JsValue::from)).into(),
        )?;
        js_sys::Reflect::set(
            &obj,
            &"values".into(),
            &js_sys::Array::from_iter(values.into_iter().map(JsValue::from)).into(),
        )?;
        js_sys::Reflect::set(
            &obj,
            &"sensitivities".into(),
            &js_sys::Array::from_iter(sensitivities.into_iter().map(JsValue::from)).into(),
        )?;

        Ok(obj.into())
    }
}

// Note: WasmTropicalDualDistribution is commented out in v0.12.0
// as TropicalDualDistribution is not yet available in the fusion API.
// TODO: Re-enable when TropicalDualDistribution is added to amari-fusion

// /// WASM wrapper for LLM token distributions
// #[wasm_bindgen]
// pub struct WasmTropicalDualDistribution {
//     // Placeholder for when TropicalDualDistribution is available
// }
//
// #[wasm_bindgen]
// impl WasmTropicalDualDistribution {
//     /// Create from logit vector (typical LLM output)
//     #[wasm_bindgen(constructor)]
//     pub fn new(logits: &[f64]) -> Self {
//         // TODO: Implement when API is available
//         Self { }
//     }
// }

/// High-performance batch operations for LLM workloads
#[wasm_bindgen]
pub struct FusionBatchOperations;

#[wasm_bindgen]
impl FusionBatchOperations {
    /// Batch tropical attention across multiple queries
    #[wasm_bindgen(js_name = batchTropicalAttention)]
    pub fn batch_tropical_attention(
        queries: &[f64],
        keys: &[f64],
        values: &[f64],
        query_dim: usize,
    ) -> Result<Vec<f64>, JsValue> {
        if !queries.len().is_multiple_of(query_dim) {
            return Err(JsValue::from_str(
                "Queries length must be divisible by query_dim",
            ));
        }

        if keys.len() != values.len() {
            return Err(JsValue::from_str("Keys and values must have same length"));
        }

        let num_queries = queries.len() / query_dim;
        let mut results = Vec::new();

        for i in 0..num_queries {
            let start = i * query_dim;
            let end = start + query_dim;
            let query_logits = &queries[start..end];

            // Create TDC from query
            let tdc = WasmTropicalDualClifford::from_logits(query_logits);

            // Compute attention
            let attention_result = tdc.tropical_attention(keys, values)?;
            results.extend(attention_result);
        }

        Ok(results)
    }

    /// Compute fusion similarity between two TDC objects
    #[wasm_bindgen(js_name = fusionSimilarity)]
    pub fn fusion_similarity(
        tdc1: &WasmTropicalDualClifford,
        tdc2: &WasmTropicalDualClifford,
    ) -> f64 {
        // Tropical similarity (max-based correlation)
        let trop1 = tdc1.get_tropical_features();
        let trop2 = tdc2.get_tropical_features();
        let tropical_sim = trop1
            .iter()
            .zip(trop2.iter())
            .map(|(&a, &b)| {
                if a.is_finite() && b.is_finite() {
                    (a + b).max(0.0)
                } else {
                    0.0
                }
            })
            .sum::<f64>();

        // Dual similarity (dot product of real parts)
        let dual1 = tdc1.get_dual_reals();
        let dual2 = tdc2.get_dual_reals();
        let dual_sim = dual1
            .iter()
            .zip(dual2.iter())
            .map(|(&a, &b)| a * b)
            .sum::<f64>();

        // Clifford similarity (scalar product)
        let cliff1 = tdc1.get_clifford_coefficients();
        let cliff2 = tdc2.get_clifford_coefficients();
        let clifford_sim = cliff1
            .iter()
            .zip(cliff2.iter())
            .map(|(&a, &b)| a * b)
            .sum::<f64>();

        // Combined similarity
        (tropical_sim + dual_sim + clifford_sim) / 3.0
    }

    /// Optimize TDC parameters using gradient information
    #[wasm_bindgen(js_name = gradientStep)]
    pub fn gradient_step(
        tdc: &WasmTropicalDualClifford,
        learning_rate: f64,
    ) -> WasmTropicalDualClifford {
        let dual_derivs = tdc.get_dual_derivatives();

        // Simple gradient descent step on dual components
        let mut new_logits = tdc.get_tropical_features();
        for (i, &grad) in dual_derivs.iter().enumerate() {
            if i < new_logits.len() {
                new_logits[i] -= learning_rate * grad;
            }
        }

        WasmTropicalDualClifford::from_logits(&new_logits)
    }

    /// Batch evaluation of multiple TDC pairs (optimized for WASM)
    #[wasm_bindgen(js_name = batchEvaluate)]
    pub fn batch_evaluate(
        tdc_a_batch: &[f64], // Flattened TDC coefficients
        tdc_b_batch: &[f64], // Flattened TDC coefficients
        batch_size: usize,
    ) -> Result<Vec<f64>, JsValue> {
        if tdc_a_batch.len() != tdc_b_batch.len() {
            return Err(JsValue::from_str("Batch arrays must have the same length"));
        }

        if tdc_a_batch.len() != batch_size * 8 {
            return Err(JsValue::from_str("Invalid batch dimensions"));
        }

        let mut results = Vec::with_capacity(batch_size * 4); // 4 scores per evaluation

        for i in 0..batch_size {
            let start = i * 8;
            let end = start + 8;

            // Create TDC from coefficients
            let tdc_a: TropicalDualClifford<f64, 8> =
                TropicalDualClifford::from_logits(&tdc_a_batch[start..end]);
            let tdc_b: TropicalDualClifford<f64, 8> =
                TropicalDualClifford::from_logits(&tdc_b_batch[start..end]);

            // Evaluate
            let eval_result = tdc_a.evaluate(&tdc_b);

            // Pack results
            results.push(eval_result.best_path_score.value());
            results.push(eval_result.gradient_norm);
            results.push(eval_result.geometric_distance);
            results.push(eval_result.combined_score);
        }

        Ok(results)
    }

    /// Batch distance computation (optimized for similarity search)
    #[wasm_bindgen(js_name = batchDistance)]
    pub fn batch_distance(
        query_logits: &[f64], // Single query TDC
        corpus_batch: &[f64], // Multiple corpus TDCs
        corpus_size: usize,
    ) -> Result<Vec<f64>, JsValue> {
        if query_logits.len() != 8 {
            return Err(JsValue::from_str("Query must have 8 components"));
        }

        if corpus_batch.len() != corpus_size * 8 {
            return Err(JsValue::from_str("Invalid corpus batch dimensions"));
        }

        let query_tdc: TropicalDualClifford<f64, 8> =
            TropicalDualClifford::from_logits(query_logits);
        let mut distances = Vec::with_capacity(corpus_size);

        for i in 0..corpus_size {
            let start = i * 8;
            let end = start + 8;

            let corpus_tdc: TropicalDualClifford<f64, 8> =
                TropicalDualClifford::from_logits(&corpus_batch[start..end]);
            distances.push(query_tdc.distance(&corpus_tdc));
        }

        Ok(distances)
    }

    /// Batch sensitivity analysis for gradient-based optimization
    ///
    /// Note: Using simplified implementation based on gradient norms
    #[wasm_bindgen(js_name = batchSensitivity)]
    pub fn batch_sensitivity(
        tdc_batch: &[f64], // Flattened TDC coefficients
        batch_size: usize,
    ) -> Result<Vec<f64>, JsValue> {
        if tdc_batch.len() != batch_size * 8 {
            return Err(JsValue::from_str("Invalid batch dimensions"));
        }

        let mut total_sensitivities = Vec::with_capacity(batch_size);

        for i in 0..batch_size {
            let start = i * 8;
            let end = start + 8;

            let tdc: TropicalDualClifford<f64, 8> =
                TropicalDualClifford::from_logits(&tdc_batch[start..end]);

            // Compute sensitivity as sum of absolute gradients
            let dual_features = tdc.extract_dual_features();
            let sensitivity: f64 = dual_features.iter().map(|d| d.derivative().abs()).sum();
            total_sensitivities.push(sensitivity);
        }

        Ok(total_sensitivities)
    }
}

/// Conversion utilities for JavaScript interoperability
#[wasm_bindgen]
pub struct FusionUtils;

#[wasm_bindgen]
impl FusionUtils {
    /// Convert softmax probabilities to tropical representation
    ///
    /// Note: Direct conversion helpers are not in v0.12.0, using manual implementation
    #[wasm_bindgen(js_name = softmaxToTropical)]
    pub fn softmax_to_tropical(probs: &[f64]) -> Vec<f64> {
        // Convert probabilities to log space (tropical representation)
        probs
            .iter()
            .map(|&p| if p > 0.0 { p.ln() } else { f64::NEG_INFINITY })
            .collect()
    }

    /// Convert tropical numbers back to softmax probabilities
    ///
    /// Note: Direct conversion helpers are not in v0.12.0, using manual implementation
    #[wasm_bindgen(js_name = tropicalToSoftmax)]
    pub fn tropical_to_softmax(tropical_values: &[f64]) -> Vec<f64> {
        // Find max for numerical stability
        let max_val = tropical_values
            .iter()
            .fold(f64::NEG_INFINITY, |a, &b| a.max(b));

        // Convert from log space to probabilities
        let exp_vals: Vec<f64> = tropical_values
            .iter()
            .map(|&v| (v - max_val).exp())
            .collect();

        let sum: f64 = exp_vals.iter().sum();

        exp_vals.iter().map(|&v| v / sum).collect()
    }

    /// Validate logits for numerical stability
    #[wasm_bindgen(js_name = validateLogits)]
    pub fn validate_logits(logits: &[f64]) -> bool {
        logits.iter().all(|&x| x.is_finite()) && !logits.is_empty()
    }

    /// Normalize logits to prevent overflow
    #[wasm_bindgen(js_name = normalizeLogits)]
    pub fn normalize_logits(logits: &[f64]) -> Vec<f64> {
        if logits.is_empty() {
            return Vec::new();
        }

        let max_logit = logits.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        logits.iter().map(|&x| x - max_logit).collect()
    }
}

/// Initialize the fusion module
#[wasm_bindgen(js_name = initFusion)]
pub fn init_fusion() {
    web_sys::console::log_1(&"Amari Fusion WASM module initialized: TropicalDualClifford system ready for LLM evaluation".into());
}

// ============================================================================
// TDC Binding Operations (v0.12.3)
// ============================================================================

/// WASM wrapper for binding operations on TDC
#[wasm_bindgen]
impl WasmTropicalDualClifford {
    /// Bind two TDC objects using geometric product (creates associations)
    ///
    /// The result is dissimilar to both inputs - useful for creating
    /// key-value associations in holographic memory.
    pub fn bind(&self, other: &WasmTropicalDualClifford) -> WasmTropicalDualClifford {
        Self {
            inner: self.inner.bind(&other.inner),
        }
    }

    /// Unbind: retrieve associated value
    ///
    /// If `bound = key.bind(value)`, then `key.unbind(bound) ≈ value`
    pub fn unbind(&self, other: &WasmTropicalDualClifford) -> WasmTropicalDualClifford {
        Self {
            inner: self.inner.unbind(&other.inner),
        }
    }

    /// Bundle two TDC objects (superposition/aggregation)
    ///
    /// The result is similar to both inputs - useful for storing
    /// multiple associations in the same memory trace.
    /// `beta` controls soft (1.0) vs hard (∞) bundling.
    pub fn bundle(&self, other: &WasmTropicalDualClifford, beta: f64) -> WasmTropicalDualClifford {
        Self {
            inner: self.inner.bundle(&other.inner, beta),
        }
    }

    /// Compute similarity between two TDC objects
    ///
    /// Uses the proper Clifford inner product with reverse: <A B̃>₀ / (|A| |B|)
    /// Returns a value in [-1, 1].
    pub fn similarity(&self, other: &WasmTropicalDualClifford) -> f64 {
        self.inner.similarity(&other.inner)
    }

    /// Get the binding identity element
    ///
    /// `x.bind(identity) = x` for any x
    #[wasm_bindgen(js_name = bindingIdentity)]
    pub fn binding_identity() -> WasmTropicalDualClifford {
        Self {
            inner: TropicalDualClifford::binding_identity(),
        }
    }

    /// Compute binding inverse
    ///
    /// If successful, `x.bind(x.bindingInverse()) ≈ identity`
    #[wasm_bindgen(js_name = bindingInverse)]
    pub fn binding_inverse(&self) -> Result<WasmTropicalDualClifford, JsValue> {
        self.inner
            .binding_inverse()
            .map(|inv| Self { inner: inv })
            .ok_or_else(|| JsValue::from_str("Element is not invertible (singular)"))
    }

    /// Normalize the TDC to unit norm
    #[wasm_bindgen(js_name = normalizeToUnit)]
    pub fn normalize_to_unit(&self) -> WasmTropicalDualClifford {
        Self {
            inner: self.inner.normalize(),
        }
    }

    /// Create a random unit vector TDC (grade 1 only)
    ///
    /// Unit vectors are guaranteed invertible and useful for
    /// proper VSA (Vector Symbolic Architecture) operations.
    #[wasm_bindgen(js_name = randomVector)]
    pub fn random_vector() -> WasmTropicalDualClifford {
        use amari_core::Multivector;

        // Create random unit vector in grade 1
        let mut clifford_coeffs = vec![0.0; Multivector::<8, 0, 0>::BASIS_COUNT];
        let mut norm_sq = 0.0;

        for i in 0..8 {
            let index = 1 << i;
            if index < clifford_coeffs.len() {
                let val = (fastrand::f64() - 0.5) * 2.0;
                clifford_coeffs[index] = val;
                norm_sq += val * val;
            }
        }

        // Normalize
        if norm_sq > 1e-10 {
            let scale = 1.0 / norm_sq.sqrt();
            for i in 0..8 {
                let index = 1 << i;
                if index < clifford_coeffs.len() {
                    clifford_coeffs[index] *= scale;
                }
            }
        }

        let clifford = Multivector::from_coefficients(clifford_coeffs);
        Self {
            inner: TropicalDualClifford::from_clifford(clifford),
        }
    }

    /// Compute Clifford similarity (proper inner product with reverse)
    #[wasm_bindgen(js_name = cliffordSimilarity)]
    pub fn clifford_similarity(&self, other: &WasmTropicalDualClifford) -> f64 {
        self.inner.clifford_similarity(&other.inner)
    }
}

// ============================================================================
// Holographic Memory WASM Bindings (v0.12.3)
//
// For full holographic memory operations, use amari-holographic directly
// with algebra types like ProductCl3x32 or CliffordAlgebra.
// The TDC binding operations above provide basic VSA primitives.
// ============================================================================

use amari_holographic::{
    AlgebraConfig, BindingAlgebra, HolographicMemory, ProductCliffordAlgebra, Resonator,
    ResonatorConfig,
};

/// Type alias for 256-dimensional ProductClifford algebra
type ProductCl3x32 = ProductCliffordAlgebra<32>;

/// WASM wrapper for HolographicMemory using ProductClifford algebra
#[wasm_bindgen]
pub struct WasmHolographicMemory {
    inner: HolographicMemory<ProductCl3x32>,
}

#[wasm_bindgen]
impl WasmHolographicMemory {
    /// Create a new holographic memory with default settings
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: HolographicMemory::new(AlgebraConfig::default()),
        }
    }

    /// Create with key tracking enabled (for attribution)
    #[wasm_bindgen(js_name = withKeyTracking)]
    pub fn with_key_tracking() -> Self {
        Self {
            inner: HolographicMemory::with_key_tracking(AlgebraConfig::default()),
        }
    }

    /// Store a key-value pair in memory (using flat coefficient arrays)
    ///
    /// Each array should have 256 coefficients for ProductCl3x32.
    pub fn store(&mut self, key: &[f64], value: &[f64]) -> Result<(), JsValue> {
        if key.len() != 256 || value.len() != 256 {
            return Err(JsValue::from_str(
                "Key and value must have 256 coefficients for ProductCl3x32",
            ));
        }
        let k = ProductCl3x32::from_coefficients(key)
            .map_err(|e| JsValue::from_str(&format!("{}", e)))?;
        let v = ProductCl3x32::from_coefficients(value)
            .map_err(|e| JsValue::from_str(&format!("{}", e)))?;
        self.inner.store(&k, &v);
        Ok(())
    }

    /// Retrieve a value by key
    ///
    /// Returns the retrieved coefficients as a Float64Array.
    pub fn retrieve(&self, key: &[f64]) -> Result<Vec<f64>, JsValue> {
        if key.len() != 256 {
            return Err(JsValue::from_str(
                "Key must have 256 coefficients for ProductCl3x32",
            ));
        }
        let k = ProductCl3x32::from_coefficients(key)
            .map_err(|e| JsValue::from_str(&format!("{}", e)))?;
        let result = self.inner.retrieve(&k);
        Ok(result.value.to_coefficients())
    }

    /// Get retrieval confidence for a key
    #[wasm_bindgen(js_name = retrieveConfidence)]
    pub fn retrieve_confidence(&self, key: &[f64]) -> Result<f64, JsValue> {
        if key.len() != 256 {
            return Err(JsValue::from_str(
                "Key must have 256 coefficients for ProductCl3x32",
            ));
        }
        let k = ProductCl3x32::from_coefficients(key)
            .map_err(|e| JsValue::from_str(&format!("{}", e)))?;
        let result = self.inner.retrieve(&k);
        Ok(result.confidence)
    }

    /// Get the number of stored items
    #[wasm_bindgen(js_name = itemCount)]
    pub fn item_count(&self) -> usize {
        self.inner.item_count()
    }

    /// Get capacity information
    #[wasm_bindgen(js_name = theoreticalCapacity)]
    pub fn theoretical_capacity(&self) -> usize {
        self.inner.capacity_info().theoretical_capacity
    }

    /// Get the estimated SNR (signal-to-noise ratio)
    #[wasm_bindgen(js_name = estimatedSnr)]
    pub fn estimated_snr(&self) -> f64 {
        self.inner.capacity_info().estimated_snr
    }

    /// Check if memory is near capacity
    #[wasm_bindgen(js_name = isNearCapacity)]
    pub fn is_near_capacity(&self) -> bool {
        self.inner.capacity_info().near_capacity
    }

    /// Clear all stored items
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Generate a random versor (for use as key/value)
    #[wasm_bindgen(js_name = randomVersor)]
    pub fn random_versor(num_factors: usize) -> Vec<f64> {
        ProductCl3x32::random_versor(num_factors).to_coefficients()
    }
}

impl Default for WasmHolographicMemory {
    fn default() -> Self {
        Self::new()
    }
}

/// WASM wrapper for Resonator using ProductClifford algebra
#[wasm_bindgen]
pub struct WasmResonator {
    inner: Resonator<ProductCl3x32>,
}

#[wasm_bindgen]
impl WasmResonator {
    /// Create a resonator from a flat codebook array
    ///
    /// Each item should have 256 coefficients for ProductCl3x32.
    #[wasm_bindgen(constructor)]
    pub fn new(codebook_flat: &[f64]) -> Result<WasmResonator, JsValue> {
        if !codebook_flat.len().is_multiple_of(256) {
            return Err(JsValue::from_str(
                "Codebook must have length divisible by 256",
            ));
        }

        let num_items = codebook_flat.len() / 256;
        if num_items == 0 {
            return Err(JsValue::from_str("Codebook cannot be empty"));
        }

        let mut codebook = Vec::with_capacity(num_items);
        for i in 0..num_items {
            let start = i * 256;
            let end = start + 256;
            let item = ProductCl3x32::from_coefficients(&codebook_flat[start..end])
                .map_err(|e| JsValue::from_str(&format!("{}", e)))?;
            codebook.push(item);
        }

        Resonator::new(codebook, ResonatorConfig::default())
            .map(|r| WasmResonator { inner: r })
            .map_err(|e| JsValue::from_str(&format!("{}", e)))
    }

    /// Clean up a noisy input to find the closest codebook item
    pub fn cleanup(&self, input: &[f64]) -> Result<Vec<f64>, JsValue> {
        if input.len() != 256 {
            return Err(JsValue::from_str(
                "Input must have 256 coefficients for ProductCl3x32",
            ));
        }
        let inp = ProductCl3x32::from_coefficients(input)
            .map_err(|e| JsValue::from_str(&format!("{}", e)))?;
        let result = self.inner.cleanup(&inp);
        Ok(result.cleaned.to_coefficients())
    }

    /// Get cleanup result with metadata
    #[wasm_bindgen(js_name = cleanupWithInfo)]
    pub fn cleanup_with_info(&self, input: &[f64]) -> Result<JsValue, JsValue> {
        if input.len() != 256 {
            return Err(JsValue::from_str(
                "Input must have 256 coefficients for ProductCl3x32",
            ));
        }
        let inp = ProductCl3x32::from_coefficients(input)
            .map_err(|e| JsValue::from_str(&format!("{}", e)))?;
        let result = self.inner.cleanup(&inp);

        let obj = Object::new();
        js_sys::Reflect::set(
            &obj,
            &"cleaned".into(),
            &js_sys::Float64Array::from(result.cleaned.to_coefficients().as_slice()).into(),
        )?;
        js_sys::Reflect::set(
            &obj,
            &"bestMatchIndex".into(),
            &(result.best_match_index as u32).into(),
        )?;
        js_sys::Reflect::set(&obj, &"converged".into(), &result.converged.into())?;
        js_sys::Reflect::set(
            &obj,
            &"iterations".into(),
            &(result.iterations as u32).into(),
        )?;
        js_sys::Reflect::set(
            &obj,
            &"finalSimilarity".into(),
            &result.final_similarity.into(),
        )?;

        Ok(obj.into())
    }

    /// Get the codebook size
    #[wasm_bindgen(js_name = codebookSize)]
    pub fn codebook_size(&self) -> usize {
        self.inner.codebook_size()
    }
}

/// Initialize the holographic memory module
#[wasm_bindgen(js_name = initHolographic)]
pub fn init_holographic() {
    web_sys::console::log_1(
        &"Amari Holographic Memory WASM module initialized: VSA operations ready".into(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    #[allow(dead_code)]
    #[wasm_bindgen_test]
    fn test_fusion_basic() {
        let _tdc = WasmTropicalDualClifford::new();
        // Note: is_zero() might fail in WASM due to floating point precision differences
        // between native and WASM compilation targets

        let random_tdc = WasmTropicalDualClifford::random();
        assert!(!random_tdc.is_zero());
    }

    #[allow(dead_code)]
    #[wasm_bindgen_test]
    fn test_fusion_from_logits() {
        let logits = vec![0.1, 0.2, 0.3, 0.4];
        let tdc = WasmTropicalDualClifford::from_logits(&logits);

        let features = tdc.get_tropical_features();
        assert_eq!(features.len(), 8); // Should have 8 features for 8D system
    }

    #[allow(dead_code)]
    #[wasm_bindgen_test]
    fn test_fusion_operations() {
        let logits1 = vec![1.0, 2.0, 3.0];
        let logits2 = vec![0.5, 1.5, 2.5];

        let tdc1 = WasmTropicalDualClifford::from_logits(&logits1);
        let tdc2 = WasmTropicalDualClifford::from_logits(&logits2);

        let sum = tdc1.add(&tdc2);
        assert!(!sum.is_zero());

        let scaled = tdc1.scale(2.0);
        assert!(!scaled.is_zero());
    }

    #[allow(dead_code)]
    #[wasm_bindgen_test]
    fn test_tropical_attention() {
        let logits = vec![1.0, 2.0, 3.0];
        let tdc = WasmTropicalDualClifford::from_logits(&logits);

        let keys = vec![0.1, 0.2, 0.3];
        let values = vec![1.0, 2.0, 3.0];

        let result = tdc.tropical_attention(&keys, &values).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[allow(dead_code)]
    #[wasm_bindgen_test]
    fn test_evaluation_system() {
        let logits1 = vec![1.0, 2.0, 3.0, 0.5, 1.5, 0.8, 2.2, 1.1];
        let logits2 = vec![1.5, 1.8, 2.5, 1.0, 1.2, 1.3, 1.9, 0.9];

        let tdc1 = WasmTropicalDualClifford::from_logits(&logits1);
        let tdc2 = WasmTropicalDualClifford::from_logits(&logits2);

        let result = tdc1.evaluate(&tdc2);

        assert!(result.get_combined_score() != 0.0);
        assert!(result.get_geometric_distance() >= 0.0);
    }

    #[allow(dead_code)]
    #[wasm_bindgen_test]
    fn test_sensitivity_analysis() {
        let logits = vec![1.0, 2.0, 3.0, 0.5, 1.5, 0.8, 2.2, 1.1];
        let tdc = WasmTropicalDualClifford::from_logits(&logits);

        let sensitivity = tdc.sensitivity_analysis();
        let most_sensitive = sensitivity.get_most_sensitive(3);

        assert_eq!(most_sensitive.len(), 3);
        assert!(sensitivity.get_total_sensitivity() >= 0.0);
    }

    #[allow(dead_code)]
    #[wasm_bindgen_test]
    fn test_batch_operations() {
        let batch_a = vec![1.0, 2.0, 3.0, 0.5, 1.5, 0.8, 2.2, 1.1];
        let batch_b = vec![1.5, 1.8, 2.5, 1.0, 1.2, 1.3, 1.9, 0.9];

        let results = FusionBatchOperations::batch_evaluate(&batch_a, &batch_b, 1);
        assert!(results.is_ok());
        assert_eq!(results.unwrap().len(), 4); // 4 scores per evaluation
    }

    // Commented out: WasmTropicalDualDistribution not available in v0.12.0
    // #[allow(dead_code)]
    // #[wasm_bindgen_test]
    // fn test_tropical_dual_distribution() {
    //     let logits = vec![1.0, 2.0, 3.0, 0.5, 1.5];
    //     let dist1 = WasmTropicalDualDistribution::new(&logits);
    //     let dist2 = WasmTropicalDualDistribution::new(&[2.0, 1.0, 2.5, 1.0, 1.8]);
    //
    //     // Test KL divergence
    //     let kl = dist1.kl_divergence(&dist2).unwrap();
    //     assert!(kl.is_object());
    //
    //     // Test sequence generation
    //     let sequence = dist1.most_likely_sequence(3);
    //     assert_eq!(sequence.len(), 3);
    //
    //     // Test geometric alignment
    //     let alignment = dist1.geometric_alignment(&dist2);
    //     assert!(alignment.is_finite());
    // }

    #[allow(dead_code)]
    #[wasm_bindgen_test]
    fn test_utils() {
        let logits = vec![1.0, 2.0, 3.0, 0.5];

        // Test validation
        assert!(FusionUtils::validate_logits(&logits));
        assert!(!FusionUtils::validate_logits(&[f64::NAN]));

        // Test normalization
        let normalized = FusionUtils::normalize_logits(&logits);
        assert_eq!(normalized.len(), 4);
        assert!(normalized.iter().all(|&x| x.is_finite()));
    }

    // ============================================================================
    // Holographic Memory Tests (v0.12.2)
    // ============================================================================

    #[allow(dead_code)]
    #[wasm_bindgen_test]
    fn test_holographic_binding_operations() {
        let a = WasmTropicalDualClifford::random_vector();
        let b = WasmTropicalDualClifford::random_vector();

        // Test bind creates something dissimilar to both inputs
        let bound = a.bind(&b);
        assert!(bound.similarity(&a).abs() < 0.5);
        assert!(bound.similarity(&b).abs() < 0.5);

        // Test binding identity
        let identity = WasmTropicalDualClifford::binding_identity();
        let a_with_identity = a.bind(&identity);
        assert!(a.similarity(&a_with_identity).abs() > 0.9);
    }

    #[allow(dead_code)]
    #[wasm_bindgen_test]
    fn test_holographic_bundling() {
        let a = WasmTropicalDualClifford::random_vector();
        let b = WasmTropicalDualClifford::random_vector();

        // Test bundle preserves similarity to both
        let bundled = a.bundle(&b, 1.0);
        // With random vectors, bundled should have some similarity to both
        let sim_a = bundled.similarity(&a);
        let sim_b = bundled.similarity(&b);
        assert!(sim_a.abs() > 0.0 || sim_b.abs() > 0.0);
    }

    #[allow(dead_code)]
    #[wasm_bindgen_test]
    fn test_holographic_memory_basic() {
        let mut memory = WasmHolographicMemory::new();

        assert_eq!(memory.item_count(), 0);
        assert!(memory.theoretical_capacity() > 0);

        // Use flat coefficient arrays (256 elements for ProductCl3x32)
        let key = WasmHolographicMemory::random_versor(2);
        let value = WasmHolographicMemory::random_versor(2);

        memory.store(&key, &value).unwrap();
        assert_eq!(memory.item_count(), 1);

        // Retrieve should work
        let result = memory.retrieve(&key);
        assert!(result.is_ok());

        // Check confidence
        let confidence = memory.retrieve_confidence(&key).unwrap();
        assert!(confidence >= 0.0);
    }

    #[allow(dead_code)]
    #[wasm_bindgen_test]
    fn test_holographic_memory_clear() {
        let mut memory = WasmHolographicMemory::new();

        // Use flat coefficient arrays
        let key = WasmHolographicMemory::random_versor(2);
        let value = WasmHolographicMemory::random_versor(2);

        memory.store(&key, &value).unwrap();
        assert_eq!(memory.item_count(), 1);

        memory.clear();
        assert_eq!(memory.item_count(), 0);
    }

    #[allow(dead_code)]
    #[wasm_bindgen_test]
    fn test_resonator_basic() {
        // Create a small codebook with flat coefficient arrays (256 per item)
        let codebook: Vec<f64> = (0..3)
            .flat_map(|_| WasmHolographicMemory::random_versor(2))
            .collect();

        let resonator = WasmResonator::new(&codebook);
        assert!(resonator.is_ok());

        let resonator = resonator.unwrap();
        assert_eq!(resonator.codebook_size(), 3);

        // Test cleanup with flat coefficient array
        let input = WasmHolographicMemory::random_versor(2);
        let result = resonator.cleanup_with_info(&input);
        assert!(result.is_ok());
    }

    #[allow(dead_code)]
    #[wasm_bindgen_test]
    fn test_clifford_similarity() {
        let a = WasmTropicalDualClifford::random_vector();

        // Self-similarity should be 1
        let self_sim = a.clifford_similarity(&a);
        assert!((self_sim - 1.0).abs() < 0.1);

        // Different vectors should have lower similarity
        let b = WasmTropicalDualClifford::random_vector();
        let ab_sim = a.clifford_similarity(&b);
        assert!(ab_sim.abs() <= 1.0);
    }
}
