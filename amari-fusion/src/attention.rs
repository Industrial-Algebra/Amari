//! Attention mechanisms using Tropical-Dual-Clifford algebra

use crate::TropicalDualClifford;
use alloc::vec::Vec;
use amari_core::Multivector;
use amari_dual::{functions::softmax, DualNumber};
use amari_tropical::TropicalNumber;
use num_traits::Float;

/// Attention head using all three algebraic systems
#[derive(Clone, Debug)]
pub struct AttentionHead<T: Float> {
    /// Query projection weights
    pub w_q: TropicalDualClifford<T, 8>,
    /// Key projection weights  
    pub w_k: TropicalDualClifford<T, 8>,
    /// Value projection weights
    pub w_v: TropicalDualClifford<T, 8>,
    /// Output projection weights
    pub w_o: TropicalDualClifford<T, 8>,
    /// Attention temperature
    pub temperature: DualNumber<T>,
    /// Head dimension
    pub d_head: usize,
}

impl<T: Float> AttentionHead<T> {
    /// Create new attention head with random initialization
    pub fn new(d_model: usize, d_head: usize) -> Self {
        let scale = T::from(1.0 / (d_model as f64).sqrt()).unwrap();

        Self {
            w_q: TropicalDualClifford::random_with_scale(scale),
            w_k: TropicalDualClifford::random_with_scale(scale),
            w_v: TropicalDualClifford::random_with_scale(scale),
            w_o: TropicalDualClifford::random_with_scale(scale),
            temperature: DualNumber::variable(T::from(1.0 / (d_head as f64).sqrt()).unwrap()),
            d_head,
        }
    }

    /// Compute attention scores using tropical algebra for efficiency
    pub fn compute_attention_tropical(
        &self,
        queries: &[TropicalNumber<T>],
        keys: &[TropicalNumber<T>],
        mask: Option<&[bool]>,
    ) -> Vec<TropicalNumber<T>> {
        let seq_len = queries.len().min(keys.len());
        let mut scores = Vec::with_capacity(seq_len * seq_len);

        // Compute Q·K^T in tropical space (becomes addition)
        for i in 0..seq_len {
            for j in 0..seq_len {
                let score = if let Some(mask) = mask {
                    if mask[i * seq_len + j] {
                        queries[i].tropical_mul(&keys[j])
                    } else {
                        TropicalNumber::neg_infinity()
                    }
                } else {
                    queries[i].tropical_mul(&keys[j])
                };
                scores.push(score);
            }
        }

        // Apply tropical softmax (efficient max-based operation)
        self.tropical_softmax(&scores, seq_len)
    }

    /// Tropical softmax using max-plus algebra
    fn tropical_softmax(
        &self,
        logits: &[TropicalNumber<T>],
        seq_len: usize,
    ) -> Vec<TropicalNumber<T>> {
        let mut result = Vec::with_capacity(logits.len());

        for i in 0..seq_len {
            let start_idx = i * seq_len;
            let row = &logits[start_idx..start_idx + seq_len];

            // Find max in tropical space (becomes standard max)
            let max_val = row.iter().fold(TropicalNumber::neg_infinity(), |acc, &x| {
                acc.tropical_add(&x)
            });

            // Normalize by subtracting max (tropical division)
            for &score in row {
                let normalized = TropicalNumber::new(score.value() - max_val.value());
                result.push(normalized);
            }
        }

        result
    }

    /// Compute exact attention using dual numbers for gradients
    pub fn compute_attention_dual(
        &self,
        queries: &[DualNumber<T>],
        keys: &[DualNumber<T>],
        values: &[DualNumber<T>],
        mask: Option<&[bool]>,
    ) -> Vec<DualNumber<T>> {
        let seq_len = queries.len();
        let mut attention_scores = Vec::with_capacity(seq_len * seq_len);

        // Compute scaled dot-product attention scores
        for i in 0..seq_len {
            for j in 0..seq_len {
                let mut score = DualNumber::constant(T::zero());

                // Dot product in dual space (preserves gradients)
                for k in 0..self.d_head.min(queries.len()) {
                    let q_idx = i * self.d_head + k;
                    let k_idx = j * self.d_head + k;

                    if q_idx < queries.len() && k_idx < keys.len() {
                        score = score + queries[q_idx] * keys[k_idx];
                    }
                }

                // Scale by temperature
                score = score * self.temperature;

                // Apply mask if provided
                if let Some(mask) = mask {
                    if !mask[i * seq_len + j] {
                        score = DualNumber::constant(T::neg_infinity());
                    }
                }

                attention_scores.push(score);
            }
        }

        // Apply softmax with automatic differentiation
        let mut result = Vec::with_capacity(values.len());
        for i in 0..seq_len {
            let start_idx = i * seq_len;
            let row_scores = &attention_scores[start_idx..start_idx + seq_len];
            let attention_weights = softmax(row_scores);

            // Compute weighted sum of values
            let mut weighted_output = DualNumber::constant(T::zero());
            for (j, &weight) in attention_weights.iter().enumerate() {
                if j < values.len() {
                    weighted_output = weighted_output + weight * values[j];
                }
            }
            result.push(weighted_output);
        }

        result
    }

    /// Geometric attention using Clifford algebra for rotational invariance
    pub fn compute_attention_clifford<const DIM: usize>(
        &self,
        query_mv: &Multivector<DIM, 0, 0>,
        key_mv: &Multivector<DIM, 0, 0>,
        value_mv: &Multivector<DIM, 0, 0>,
    ) -> Multivector<DIM, 0, 0> {
        // Simplified geometric attention using scalar product
        let alignment = query_mv.scalar_product(key_mv);
        let norm_product = query_mv.coefficient_norm() * key_mv.coefficient_norm();

        let attention_strength = if norm_product > 0.0 {
            (alignment / norm_product).clamp(0.0, 1.0)
        } else {
            0.0
        };

        // Scale value by attention strength
        value_mv.clone() * attention_strength
    }

    /// Multi-head self-attention using all three systems
    pub fn multi_head_attention(
        &self,
        input: &TropicalDualClifford<T, 8>,
        mask: Option<&[bool]>,
    ) -> TropicalDualClifford<T, 8> {
        // Phase 1: Fast approximation using tropical algebra
        let tropical_queries = input.extract_tropical_features();
        let tropical_keys = tropical_queries.clone();
        let tropical_attention =
            self.compute_attention_tropical(&tropical_queries, &tropical_keys, mask);

        // Phase 2: Exact computation with gradients using dual numbers
        let dual_queries = input.extract_dual_features();
        let dual_keys = dual_queries.clone();
        let dual_values = dual_queries.clone();
        let dual_attention =
            self.compute_attention_dual(&dual_queries, &dual_keys, &dual_values, mask);

        // Phase 3: Geometric refinement using Clifford algebra
        let query_mv = input.clifford().clone();
        let key_mv = input.clifford().clone();
        let value_mv = input.clifford().clone();
        let clifford_attention = self.compute_attention_clifford(&query_mv, &key_mv, &value_mv);

        // Combine results from all three systems
        TropicalDualClifford::from_components(
            tropical_attention.into_iter().take(8).collect(),
            dual_attention.into_iter().take(8).collect(),
            clifford_attention,
        )
    }

    /// Attention pattern analysis using geometric structure
    pub fn analyze_attention_patterns(&self, attention_weights: &[T]) -> AttentionAnalysis<T> {
        let seq_len = (attention_weights.len() as f64).sqrt() as usize;

        // Compute entropy (information content)
        let entropy = self.compute_attention_entropy(attention_weights);

        // Compute sparsity (concentration)
        let sparsity = self.compute_attention_sparsity(attention_weights);

        // Analyze geometric patterns
        let geometric_coherence = self.compute_geometric_coherence(attention_weights, seq_len);

        AttentionAnalysis {
            entropy,
            sparsity,
            geometric_coherence,
            peak_attention: attention_weights.iter().copied().fold(T::zero(), T::max),
            effective_range: self.compute_effective_range(attention_weights),
        }
    }

    fn compute_attention_entropy(&self, weights: &[T]) -> T {
        weights
            .iter()
            .filter(|&&w| w > T::zero())
            .map(|&w| {
                let log_w = w.ln();
                -w * log_w
            })
            .fold(T::zero(), |acc, x| acc + x)
    }

    fn compute_attention_sparsity(&self, weights: &[T]) -> T {
        let max_weight = weights.iter().copied().fold(T::zero(), T::max);
        let threshold = max_weight * T::from(0.1).unwrap();
        let active_count = weights.iter().filter(|&&w| w > threshold).count();
        T::from(active_count).unwrap() / T::from(weights.len()).unwrap()
    }

    fn compute_geometric_coherence(&self, weights: &[T], seq_len: usize) -> T {
        let mut coherence = T::zero();
        let mut count = T::zero();

        for i in 0..seq_len {
            for j in 1..seq_len {
                if i + j < seq_len {
                    let idx1 = i * seq_len + j;
                    let idx2 = i * seq_len + (j - 1);
                    if idx1 < weights.len() && idx2 < weights.len() {
                        let diff = (weights[idx1] - weights[idx2]).abs();
                        coherence = coherence + diff;
                        count = count + T::one();
                    }
                }
            }
        }

        if count > T::zero() {
            T::one() - (coherence / count)
        } else {
            T::zero()
        }
    }

    fn compute_effective_range(&self, weights: &[T]) -> T {
        let total_weight: T = weights.iter().copied().fold(T::zero(), |acc, x| acc + x);
        let threshold = total_weight * T::from(0.95).unwrap();

        let mut cumulative = T::zero();
        let mut range = 0;

        for &weight in weights {
            cumulative = cumulative + weight;
            range += 1;
            if cumulative >= threshold {
                break;
            }
        }

        T::from(range).unwrap() / T::from(weights.len()).unwrap()
    }
}

/// Analysis results for attention patterns
#[derive(Clone, Debug)]
pub struct AttentionAnalysis<T: Float> {
    /// Shannon entropy of attention distribution
    pub entropy: T,
    /// Sparsity measure (fraction of near-zero weights)
    pub sparsity: T,
    /// Coherence with geometric structure
    pub geometric_coherence: T,
    /// Maximum attention weight
    pub peak_attention: T,
    /// Effective range of attention window
    pub effective_range: T,
}

impl<T: Float> AttentionAnalysis<T> {
    /// Check if attention pattern is healthy
    pub fn is_healthy(&self) -> bool {
        let entropy_ok =
            self.entropy > T::from(0.5).unwrap() && self.entropy < T::from(3.0).unwrap();
        let sparsity_ok =
            self.sparsity > T::from(0.1).unwrap() && self.sparsity < T::from(0.8).unwrap();
        let coherence_ok = self.geometric_coherence > T::from(0.3).unwrap();

        entropy_ok && sparsity_ok && coherence_ok
    }

    /// Get overall attention quality score
    pub fn quality_score(&self) -> T {
        let entropy_score =
            if self.entropy > T::from(1.0).unwrap() && self.entropy < T::from(2.0).unwrap() {
                T::one()
            } else {
                T::from(0.5).unwrap()
            };

        let sparsity_score =
            if self.sparsity > T::from(0.2).unwrap() && self.sparsity < T::from(0.6).unwrap() {
                T::one()
            } else {
                T::from(0.5).unwrap()
            };

        (entropy_score + self.geometric_coherence + sparsity_score) / T::from(3.0).unwrap()
    }
}

/// Multi-head attention using Tropical-Dual-Clifford algebra
#[derive(Clone, Debug)]
pub struct MultiHeadAttention<T: Float> {
    /// Individual attention heads
    pub heads: Vec<AttentionHead<T>>,
    /// Number of parallel attention heads
    pub num_heads: usize,
    /// Model embedding dimension
    pub d_model: usize,
    /// Per-head embedding dimension
    pub d_head: usize,
}

impl<T: Float> MultiHeadAttention<T> {
    /// Create new multi-head attention
    pub fn new(d_model: usize, num_heads: usize) -> Self {
        let d_head = d_model / num_heads;
        let mut heads = Vec::with_capacity(num_heads);

        for _ in 0..num_heads {
            heads.push(AttentionHead::new(d_model, d_head));
        }

        Self {
            heads,
            num_heads,
            d_model,
            d_head,
        }
    }

    /// Forward pass through all attention heads
    pub fn forward(
        &self,
        input: &TropicalDualClifford<T, 8>,
        mask: Option<&[bool]>,
    ) -> TropicalDualClifford<T, 8> {
        // Compute attention for each head
        let head_outputs: Vec<TropicalDualClifford<T, 8>> = self
            .heads
            .iter()
            .map(|head| head.multi_head_attention(input, mask))
            .collect();

        // Concatenate and project head outputs
        self.combine_heads(&head_outputs)
    }

    /// Combine outputs from multiple attention heads
    fn combine_heads(
        &self,
        head_outputs: &[TropicalDualClifford<T, 8>],
    ) -> TropicalDualClifford<T, 8> {
        // Simple average combination - could be more sophisticated
        let mut combined = TropicalDualClifford::zero();

        for output in head_outputs {
            combined = combined.add(output);
        }

        let scale = T::from(1.0 / self.num_heads as f64).unwrap();
        combined.scale(scale)
    }

    /// Analyze attention patterns across all heads
    pub fn analyze_all_heads(&self, attention_weights: &[Vec<T>]) -> Vec<AttentionAnalysis<T>> {
        self.heads
            .iter()
            .zip(attention_weights.iter())
            .map(|(head, weights)| head.analyze_attention_patterns(weights))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attention_head_creation() {
        let head = AttentionHead::<f64>::new(512, 64);
        assert_eq!(head.d_head, 64);
        assert!(head.temperature.real > 0.0);
    }

    #[test]
    fn test_tropical_attention() {
        let head = AttentionHead::<f64>::new(4, 2);
        let queries = vec![
            TropicalNumber::new(1.0),
            TropicalNumber::new(2.0),
            TropicalNumber::new(3.0),
        ];
        let keys = queries.clone();

        let result = head.compute_attention_tropical(&queries, &keys, None);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_dual_attention() {
        let head = AttentionHead::<f64>::new(4, 2);
        let queries = vec![DualNumber::variable(1.0), DualNumber::variable(2.0)];
        let keys = queries.clone();
        let values = queries.clone();

        let result = head.compute_attention_dual(&queries, &keys, &values, None);
        assert_eq!(result.len(), 2); // Should match sequence length

        // Both outputs should have gradients
        for output in &result {
            assert!(output.dual.abs() > 0.0);
        }
    }

    #[test]
    fn test_attention_analysis() {
        let head = AttentionHead::<f64>::new(4, 2);
        let weights = vec![0.7, 0.2, 0.1, 0.0];

        let analysis = head.analyze_attention_patterns(&weights);
        assert!(analysis.entropy > 0.0);
        assert!(analysis.sparsity > 0.0);
        assert!(analysis.peak_attention == 0.7);
    }

    #[test]
    fn test_multi_head_attention() {
        let mha = MultiHeadAttention::<f64>::new(512, 8);
        assert_eq!(mha.num_heads, 8);
        assert_eq!(mha.d_head, 64);

        let input = TropicalDualClifford::random();
        let output = mha.forward(&input, None);
        assert!(!output.is_zero());
    }
}
