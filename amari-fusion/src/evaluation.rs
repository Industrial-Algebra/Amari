//! LLM evaluation metrics using Tropical-Dual-Clifford algebra
#![allow(missing_docs)]

use crate::TropicalDualClifford;
use alloc::vec::Vec;
use amari_dual::{functions::cross_entropy_loss, DualNumber};
use amari_tropical::TropicalNumber;
use core::fmt;
use num_traits::Float;

/// Comprehensive LLM evaluation metrics
#[derive(Clone, Debug)]
pub struct EvaluationMetrics<T: Float> {
    /// Perplexity using tropical approximation
    pub tropical_perplexity: TropicalNumber<T>,
    /// Exact perplexity with gradients
    pub dual_perplexity: DualNumber<T>,
    /// Geometric coherence score
    pub clifford_coherence: T,
    /// BLEU score components
    pub bleu_components: BleuComponents<T>,
    /// Attention pattern quality
    pub attention_quality: T,
    /// Semantic consistency
    pub semantic_consistency: T,
    /// Computational efficiency metrics
    pub efficiency_metrics: EfficiencyMetrics<T>,
}

impl<T: Float> EvaluationMetrics<T> {
    /// Create zero-initialized metrics
    pub fn zero() -> Self {
        Self {
            tropical_perplexity: TropicalNumber::neg_infinity(),
            dual_perplexity: DualNumber::constant(T::infinity()),
            clifford_coherence: T::zero(),
            bleu_components: BleuComponents::zero(),
            attention_quality: T::zero(),
            semantic_consistency: T::zero(),
            efficiency_metrics: EfficiencyMetrics::zero(),
        }
    }

    /// Compute overall quality score
    pub fn overall_score(&self) -> T {
        let perplexity_score = T::one() / (T::one() + self.dual_perplexity.real);
        let coherence_score = self.clifford_coherence;
        let bleu_score = self.bleu_components.overall_score();
        let attention_score = self.attention_quality;
        let semantic_score = self.semantic_consistency;

        (perplexity_score + coherence_score + bleu_score + attention_score + semantic_score)
            / T::from(5.0).unwrap()
    }

    /// Check if metrics indicate good model performance
    pub fn is_good_performance(&self) -> bool {
        let perplexity_ok = self.dual_perplexity.real < T::from(50.0).unwrap();
        let coherence_ok = self.clifford_coherence > T::from(0.7).unwrap();
        let bleu_ok = self.bleu_components.overall_score() > T::from(0.3).unwrap();
        let attention_ok = self.attention_quality > T::from(0.6).unwrap();

        perplexity_ok && coherence_ok && bleu_ok && attention_ok
    }
}

/// BLEU score components for translation evaluation
#[derive(Clone, Debug)]
pub struct BleuComponents<T: Float> {
    /// Unigram precision
    pub precision_1gram: T,
    /// Bigram precision
    pub precision_2gram: T,
    /// Trigram precision
    pub precision_3gram: T,
    /// 4-gram precision
    pub precision_4gram: T,
    /// Brevity penalty for short translations
    pub brevity_penalty: T,
    /// Ratio of candidate to reference length
    pub length_ratio: T,
}

impl<T: Float> BleuComponents<T> {
    /// Create zero-initialized BLEU components
    pub fn zero() -> Self {
        Self {
            precision_1gram: T::zero(),
            precision_2gram: T::zero(),
            precision_3gram: T::zero(),
            precision_4gram: T::zero(),
            brevity_penalty: T::one(),
            length_ratio: T::one(),
        }
    }

    /// Compute overall BLEU score from components
    pub fn overall_score(&self) -> T {
        let geometric_mean = (self.precision_1gram
            * self.precision_2gram
            * self.precision_3gram
            * self.precision_4gram)
            .powf(T::from(0.25).unwrap());
        geometric_mean * self.brevity_penalty
    }
}

/// Computational efficiency tracking
#[derive(Clone, Debug)]
pub struct EfficiencyMetrics<T: Float> {
    /// Speedup from tropical algebra optimizations
    pub tropical_speedup: T,
    /// Overhead from dual number computations
    pub dual_overhead: T,
    /// Complexity factor from Clifford algebra operations
    pub clifford_complexity: T,
    /// Memory usage in normalized units
    pub memory_usage: T,
    /// Cache hit ratio
    pub cache_efficiency: T,
}

impl<T: Float> EfficiencyMetrics<T> {
    /// Create unit-initialized efficiency metrics
    pub fn zero() -> Self {
        Self {
            tropical_speedup: T::one(),
            dual_overhead: T::one(),
            clifford_complexity: T::one(),
            memory_usage: T::zero(),
            cache_efficiency: T::zero(),
        }
    }

    pub fn efficiency_score(&self) -> T {
        let speedup_score =
            self.tropical_speedup.min(T::from(10.0).unwrap()) / T::from(10.0).unwrap();
        let overhead_score = T::one() / (T::one() + self.dual_overhead);
        let complexity_score = T::one() / (T::one() + self.clifford_complexity);
        let memory_score = T::one() / (T::one() + self.memory_usage);
        let cache_score = self.cache_efficiency;

        (speedup_score + overhead_score + complexity_score + memory_score + cache_score)
            / T::from(5.0).unwrap()
    }
}

/// LLM evaluator using all three algebraic systems
pub struct LLMEvaluator<T: Float> {
    /// Vocabulary size for perplexity calculations
    pub vocab_size: usize,
    /// Sequence length for attention analysis
    pub seq_length: usize,
    /// Evaluation thresholds
    pub thresholds: EvaluationThresholds<T>,
}

impl<T: Float> LLMEvaluator<T> {
    /// Create new evaluator
    pub fn new(vocab_size: usize, seq_length: usize) -> Self {
        Self {
            vocab_size,
            seq_length,
            thresholds: EvaluationThresholds::default(),
        }
    }

    /// Evaluate model predictions using all three systems
    pub fn evaluate_predictions(
        &self,
        predictions: &[TropicalDualClifford<T, 8>],
        targets: &[usize],
        attention_weights: Option<&[T]>,
    ) -> EvaluationMetrics<T> {
        let mut metrics = EvaluationMetrics::zero();

        // Phase 1: Fast perplexity approximation using tropical algebra
        metrics.tropical_perplexity = self.compute_tropical_perplexity(predictions, targets);

        // Phase 2: Exact perplexity with gradients using dual numbers
        metrics.dual_perplexity = self.compute_dual_perplexity(predictions, targets);

        // Phase 3: Geometric coherence using Clifford algebra
        metrics.clifford_coherence = self.compute_clifford_coherence(predictions);

        // Attention analysis if provided
        if let Some(weights) = attention_weights {
            metrics.attention_quality = self.analyze_attention_quality(weights);
        }

        // Semantic consistency across predictions
        metrics.semantic_consistency = self.compute_semantic_consistency(predictions);

        // BLEU components for generation tasks
        metrics.bleu_components = self.compute_bleu_components(predictions, targets);

        // Efficiency tracking
        metrics.efficiency_metrics = self.compute_efficiency_metrics(predictions);

        metrics
    }

    /// Compute perplexity using tropical algebra (fast approximation)
    fn compute_tropical_perplexity(
        &self,
        predictions: &[TropicalDualClifford<T, 8>],
        targets: &[usize],
    ) -> TropicalNumber<T> {
        let mut total_log_prob = TropicalNumber::zero();
        let mut count = 0;

        for (pred, &target) in predictions.iter().zip(targets.iter()) {
            if target < self.vocab_size {
                // Extract tropical probabilities
                let tropical_logits = pred.extract_tropical_features();

                // Tropical softmax (efficient max-based operation)
                let max_logit = tropical_logits
                    .iter()
                    .fold(TropicalNumber::neg_infinity(), |acc, &x| {
                        acc.tropical_add(&x)
                    });

                // Target probability in tropical space
                if target < tropical_logits.len() {
                    let target_prob = tropical_logits[target]
                        .tropical_mul(&TropicalNumber::new(-max_logit.value()));
                    total_log_prob = total_log_prob.tropical_add(&target_prob);
                    count += 1;
                }
            }
        }

        if count > 0 {
            // Average and convert to perplexity
            let count_ln = T::from((count as f64).ln()).unwrap_or(T::zero());
            let avg_log_prob = TropicalNumber::new(total_log_prob.value() - count_ln);
            TropicalNumber::new(-avg_log_prob.value()) // exp(-avg_log_prob)
        } else {
            TropicalNumber::neg_infinity()
        }
    }

    /// Compute exact perplexity with automatic differentiation
    fn compute_dual_perplexity(
        &self,
        predictions: &[TropicalDualClifford<T, 8>],
        targets: &[usize],
    ) -> DualNumber<T> {
        let mut total_loss = DualNumber::constant(T::zero());
        let mut count = 0;

        for (pred, &target) in predictions.iter().zip(targets.iter()) {
            if target < self.vocab_size {
                // Extract dual logits with gradients
                let dual_logits = pred.extract_dual_features();

                // Create target distribution (one-hot)
                let dist_len = dual_logits.len().min(self.vocab_size);
                let mut target_dist = Vec::with_capacity(dist_len);
                for _ in 0..dist_len {
                    target_dist.push(T::zero());
                }
                if target < target_dist.len() {
                    target_dist[target] = T::one();
                }

                // Compute cross-entropy loss with gradients
                let loss = cross_entropy_loss(&dual_logits[..target_dist.len()], &target_dist);
                total_loss = total_loss + loss;
                count += 1;
            }
        }

        if count > 0 {
            let avg_loss = total_loss / DualNumber::constant(T::from(count).unwrap());
            avg_loss.exp() // Perplexity = exp(cross_entropy)
        } else {
            DualNumber::constant(T::infinity())
        }
    }

    /// Compute geometric coherence using Clifford algebra
    fn compute_clifford_coherence(&self, predictions: &[TropicalDualClifford<T, 8>]) -> T {
        if predictions.len() < 2 {
            return T::one();
        }

        let mut total_coherence = T::zero();
        let mut count = 0;

        // Analyze geometric relationships between consecutive predictions
        for i in 1..predictions.len() {
            let prev_mv = predictions[i - 1].clifford();
            let curr_mv = predictions[i].clifford();

            // Compute geometric similarity using scalar product
            let similarity = prev_mv.scalar_product(curr_mv);
            let norm_product = prev_mv.coefficient_norm() * curr_mv.coefficient_norm();

            // Coherence based on normalized similarity
            let coherence = if norm_product > 0.0 {
                T::from(similarity / norm_product).unwrap_or(T::zero())
            } else {
                T::zero()
            };
            total_coherence = total_coherence + coherence.abs();
            count += 1;
        }

        if count > 0 {
            total_coherence / T::from(count).unwrap()
        } else {
            T::one()
        }
    }

    /// Analyze attention pattern quality
    fn analyze_attention_quality(&self, attention_weights: &[T]) -> T {
        let entropy = self.compute_attention_entropy(attention_weights);
        let sparsity = self.compute_attention_sparsity(attention_weights);
        let locality = self.compute_attention_locality(attention_weights);

        // Combine metrics for overall quality
        let entropy_score = if entropy > T::from(1.0).unwrap() && entropy < T::from(3.0).unwrap() {
            T::one()
        } else {
            T::from(0.5).unwrap()
        };

        let sparsity_score = if sparsity > T::from(0.1).unwrap() && sparsity < T::from(0.7).unwrap()
        {
            T::one()
        } else {
            T::from(0.5).unwrap()
        };

        (entropy_score + sparsity_score + locality) / T::from(3.0).unwrap()
    }

    fn compute_attention_entropy(&self, weights: &[T]) -> T {
        weights
            .iter()
            .filter(|&&w| w > T::from(1e-10).unwrap())
            .map(|&w| -w * w.ln())
            .fold(T::zero(), |acc, x| acc + x)
    }

    fn compute_attention_sparsity(&self, weights: &[T]) -> T {
        let max_weight = weights.iter().copied().fold(T::zero(), T::max);
        let threshold = max_weight * T::from(0.1).unwrap();
        let active_count = weights.iter().filter(|&&w| w > threshold).count();
        T::from(active_count).unwrap() / T::from(weights.len()).unwrap()
    }

    fn compute_attention_locality(&self, weights: &[T]) -> T {
        if weights.len() < self.seq_length {
            return T::one();
        }

        let mut locality_score = T::zero();
        let mut count = 0;

        // Check local attention patterns
        for i in 0..self.seq_length {
            let start_idx = i * self.seq_length;
            if start_idx + self.seq_length <= weights.len() {
                let row = &weights[start_idx..start_idx + self.seq_length];

                // Compute local vs. global attention ratio
                let local_range = 5.min(self.seq_length / 4);
                let local_start = i.saturating_sub(local_range);
                let local_end = (i + local_range).min(self.seq_length);

                let local_sum: T = (local_start..local_end)
                    .map(|j| row.get(j).copied().unwrap_or(T::zero()))
                    .fold(T::zero(), |acc, x| acc + x);

                let total_sum: T = row.iter().copied().fold(T::zero(), |acc, x| acc + x);

                if total_sum > T::zero() {
                    locality_score = locality_score + (local_sum / total_sum);
                    count += 1;
                }
            }
        }

        if count > 0 {
            locality_score / T::from(count).unwrap()
        } else {
            T::zero()
        }
    }

    /// Compute semantic consistency across predictions
    fn compute_semantic_consistency(&self, predictions: &[TropicalDualClifford<T, 8>]) -> T {
        if predictions.len() < 2 {
            return T::one();
        }

        let mut total_consistency = T::zero();
        let mut count = 0;

        // Compare semantic vectors using cosine similarity
        for i in 1..predictions.len() {
            let prev_features = predictions[i - 1].extract_dual_features();
            let curr_features = predictions[i].extract_dual_features();

            let cosine_sim = self.cosine_similarity(&prev_features, &curr_features);
            total_consistency = total_consistency + cosine_sim;
            count += 1;
        }

        if count > 0 {
            total_consistency / T::from(count).unwrap()
        } else {
            T::one()
        }
    }

    fn cosine_similarity(&self, a: &[DualNumber<T>], b: &[DualNumber<T>]) -> T {
        let min_len = a.len().min(b.len());
        if min_len == 0 {
            return T::zero();
        }

        let mut dot_product = T::zero();
        let mut norm_a = T::zero();
        let mut norm_b = T::zero();

        for i in 0..min_len {
            dot_product = dot_product + a[i].real * b[i].real;
            norm_a = norm_a + a[i].real * a[i].real;
            norm_b = norm_b + b[i].real * b[i].real;
        }

        let norm_product = norm_a.sqrt() * norm_b.sqrt();
        if norm_product > T::zero() {
            dot_product / norm_product
        } else {
            T::zero()
        }
    }

    /// Compute BLEU score components
    fn compute_bleu_components(
        &self,
        predictions: &[TropicalDualClifford<T, 8>],
        targets: &[usize],
    ) -> BleuComponents<T> {
        // Simplified BLEU computation for demonstration
        let mut components = BleuComponents::zero();

        if predictions.len() != targets.len() || predictions.is_empty() {
            return components;
        }

        // Convert predictions to token sequences (simplified)
        let pred_tokens: Vec<usize> = predictions
            .iter()
            .map(|pred| {
                let features = pred.extract_dual_features();
                if !features.is_empty() {
                    (features[0].real.abs() * T::from(self.vocab_size).unwrap())
                        .to_usize()
                        .unwrap_or(0)
                        % self.vocab_size
                } else {
                    0
                }
            })
            .collect();

        // Compute n-gram precisions
        components.precision_1gram = self.compute_ngram_precision(&pred_tokens, targets, 1);
        components.precision_2gram = self.compute_ngram_precision(&pred_tokens, targets, 2);
        components.precision_3gram = self.compute_ngram_precision(&pred_tokens, targets, 3);
        components.precision_4gram = self.compute_ngram_precision(&pred_tokens, targets, 4);

        // Compute brevity penalty
        let pred_len = T::from(pred_tokens.len()).unwrap();
        let ref_len = T::from(targets.len()).unwrap();
        components.length_ratio = pred_len / ref_len;

        components.brevity_penalty = if pred_len >= ref_len {
            T::one()
        } else {
            (T::one() - ref_len / pred_len).exp()
        };

        components
    }

    fn compute_ngram_precision(&self, predictions: &[usize], targets: &[usize], n: usize) -> T {
        if predictions.len() < n || targets.len() < n {
            return T::zero();
        }

        let mut matches = 0;
        let mut total = 0;

        // Extract n-grams from predictions
        for i in 0..=predictions.len() - n {
            let pred_ngram = &predictions[i..i + n];
            total += 1;

            // Check if this n-gram appears in targets
            for j in 0..=targets.len() - n {
                let target_ngram = &targets[j..j + n];
                if pred_ngram == target_ngram {
                    matches += 1;
                    break;
                }
            }
        }

        if total > 0 {
            T::from(matches).unwrap() / T::from(total).unwrap()
        } else {
            T::zero()
        }
    }

    /// Compute efficiency metrics
    fn compute_efficiency_metrics(
        &self,
        predictions: &[TropicalDualClifford<T, 8>],
    ) -> EfficiencyMetrics<T> {
        let mut metrics = EfficiencyMetrics::zero();

        // Estimate computational complexity of each algebra
        let tropical_ops = predictions.len() * 2; // Max and add operations
        let dual_ops = predictions.len() * 8; // Dual arithmetic operations
        let clifford_ops = predictions.len() * 16; // Geometric product operations

        metrics.tropical_speedup =
            T::from(dual_ops).unwrap() / T::from(tropical_ops.max(1)).unwrap();
        metrics.dual_overhead =
            T::from(dual_ops).unwrap() / T::from(predictions.len().max(1)).unwrap();
        metrics.clifford_complexity =
            T::from(clifford_ops).unwrap() / T::from(predictions.len().max(1)).unwrap();

        // Estimate memory usage (simplified)
        let base_memory = predictions.len() * 8; // Base prediction storage
        let total_memory = base_memory * 3; // All three representations
        metrics.memory_usage =
            T::from(total_memory).unwrap() / T::from(base_memory.max(1)).unwrap();

        // Cache efficiency based on access patterns
        metrics.cache_efficiency = T::from(0.8).unwrap(); // Simplified estimate

        metrics
    }

    /// Evaluate model against multiple test cases
    pub fn batch_evaluate(&self, test_cases: &[LLMTestCase<T>]) -> Vec<EvaluationMetrics<T>> {
        test_cases
            .iter()
            .map(|test_case| {
                self.evaluate_predictions(
                    &test_case.predictions,
                    &test_case.targets,
                    test_case.attention_weights.as_deref(),
                )
            })
            .collect()
    }

    /// Compare two models using comprehensive metrics
    pub fn compare_models(
        &self,
        model_a_results: &[EvaluationMetrics<T>],
        model_b_results: &[EvaluationMetrics<T>],
    ) -> ModelComparison {
        assert_eq!(model_a_results.len(), model_b_results.len());

        let mut comparison = ModelComparison::new();

        for (a, b) in model_a_results.iter().zip(model_b_results.iter()) {
            comparison.perplexity_wins_a += if a.dual_perplexity.real < b.dual_perplexity.real {
                1
            } else {
                0
            };
            comparison.coherence_wins_a += if a.clifford_coherence > b.clifford_coherence {
                1
            } else {
                0
            };
            comparison.bleu_wins_a +=
                if a.bleu_components.overall_score() > b.bleu_components.overall_score() {
                    1
                } else {
                    0
                };
            comparison.efficiency_wins_a += if a.efficiency_metrics.efficiency_score()
                > b.efficiency_metrics.efficiency_score()
            {
                1
            } else {
                0
            };
        }

        comparison.total_cases = model_a_results.len();
        comparison
    }
}

/// Test case for LLM evaluation
#[derive(Clone, Debug)]
pub struct LLMTestCase<T: Float> {
    pub predictions: Vec<TropicalDualClifford<T, 8>>,
    pub targets: Vec<usize>,
    pub attention_weights: Option<Vec<T>>,
    pub task_type: TaskType,
}

/// Type of LLM task
#[derive(Clone, Debug, PartialEq)]
pub enum TaskType {
    LanguageModeling,
    Translation,
    Summarization,
    QuestionAnswering,
    TextGeneration,
}

/// Model comparison results
#[derive(Clone, Debug)]
pub struct ModelComparison {
    pub perplexity_wins_a: usize,
    pub coherence_wins_a: usize,
    pub bleu_wins_a: usize,
    pub efficiency_wins_a: usize,
    pub total_cases: usize,
}

impl Default for ModelComparison {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelComparison {
    pub fn new() -> Self {
        Self {
            perplexity_wins_a: 0,
            coherence_wins_a: 0,
            bleu_wins_a: 0,
            efficiency_wins_a: 0,
            total_cases: 0,
        }
    }

    pub fn model_a_win_rate(&self) -> f64 {
        if self.total_cases == 0 {
            return 0.0;
        }

        let total_wins = self.perplexity_wins_a
            + self.coherence_wins_a
            + self.bleu_wins_a
            + self.efficiency_wins_a;
        let total_metrics = self.total_cases * 4;

        total_wins as f64 / total_metrics as f64
    }
}

/// Evaluation thresholds for quality assessment
#[derive(Clone, Debug)]
pub struct EvaluationThresholds<T: Float> {
    pub max_perplexity: T,
    pub min_coherence: T,
    pub min_bleu: T,
    pub min_attention_quality: T,
}

impl<T: Float> Default for EvaluationThresholds<T> {
    fn default() -> Self {
        Self {
            max_perplexity: T::from(100.0).unwrap(),
            min_coherence: T::from(0.6).unwrap(),
            min_bleu: T::from(0.2).unwrap(),
            min_attention_quality: T::from(0.5).unwrap(),
        }
    }
}

impl<T: Float> fmt::Display for EvaluationMetrics<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Evaluation Metrics:")?;
        writeln!(
            f,
            "  Perplexity: {:.2}",
            self.dual_perplexity.real.to_f64().unwrap_or(0.0)
        )?;
        writeln!(
            f,
            "  Coherence: {:.3}",
            self.clifford_coherence.to_f64().unwrap_or(0.0)
        )?;
        writeln!(
            f,
            "  BLEU: {:.3}",
            self.bleu_components.overall_score().to_f64().unwrap_or(0.0)
        )?;
        writeln!(
            f,
            "  Attention Quality: {:.3}",
            self.attention_quality.to_f64().unwrap_or(0.0)
        )?;
        write!(
            f,
            "  Overall Score: {:.3}",
            self.overall_score().to_f64().unwrap_or(0.0)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evaluator_creation() {
        let evaluator = LLMEvaluator::<f64>::new(1000, 64);
        assert_eq!(evaluator.vocab_size, 1000);
        assert_eq!(evaluator.seq_length, 64);
    }

    #[test]
    fn test_tropical_perplexity() {
        let evaluator = LLMEvaluator::<f64>::new(4, 4);
        let predictions = vec![
            TropicalDualClifford::random(),
            TropicalDualClifford::random(),
        ];
        let targets = vec![0, 1];

        let perplexity = evaluator.compute_tropical_perplexity(&predictions, &targets);
        assert!(!perplexity.is_infinity());
    }

    #[test]
    fn test_dual_perplexity() {
        let evaluator = LLMEvaluator::<f64>::new(4, 4);
        let predictions = vec![
            TropicalDualClifford::random(),
            TropicalDualClifford::random(),
        ];
        let targets = vec![0, 1];

        let perplexity = evaluator.compute_dual_perplexity(&predictions, &targets);
        assert!(perplexity.real > 0.0);
        assert!(perplexity.dual.abs() >= 0.0);
    }

    #[test]
    fn test_attention_quality() {
        let evaluator = LLMEvaluator::<f64>::new(4, 4);
        let weights = vec![0.5, 0.3, 0.15, 0.05];

        let quality = evaluator.analyze_attention_quality(&weights);
        assert!(quality >= 0.0);
        assert!(quality <= 1.0);
    }

    #[test]
    fn test_full_evaluation() {
        let evaluator = LLMEvaluator::<f64>::new(8, 4);
        let predictions = vec![
            TropicalDualClifford::random(),
            TropicalDualClifford::random(),
            TropicalDualClifford::random(),
        ];
        let targets = vec![0, 1, 2];
        let attention_weights = vec![0.4, 0.3, 0.2, 0.1];

        let metrics =
            evaluator.evaluate_predictions(&predictions, &targets, Some(&attention_weights));

        assert!(metrics.dual_perplexity.real > 0.0);
        assert!(metrics.clifford_coherence >= 0.0);
        assert!(metrics.attention_quality >= 0.0);
        assert!(metrics.overall_score() >= 0.0);
    }

    #[test]
    fn test_bleu_computation() {
        let evaluator = LLMEvaluator::<f64>::new(4, 4);
        let predictions = vec![
            TropicalDualClifford::random(),
            TropicalDualClifford::random(),
        ];
        let targets = vec![0, 1];

        let bleu = evaluator.compute_bleu_components(&predictions, &targets);
        assert!(bleu.precision_1gram >= 0.0);
        assert!(bleu.brevity_penalty > 0.0);
    }

    #[test]
    fn test_model_comparison() {
        let evaluator = LLMEvaluator::<f64>::new(4, 4);

        let metrics_a = vec![EvaluationMetrics::zero()];
        let metrics_b = vec![EvaluationMetrics::zero()];

        let comparison = evaluator.compare_models(&metrics_a, &metrics_b);
        assert_eq!(comparison.total_cases, 1);
    }
}
