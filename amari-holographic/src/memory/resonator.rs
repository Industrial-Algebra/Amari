//! Resonator networks for cleanup/factorization of noisy retrievals.
//!
//! Given a set of known valid states (codebook), resonators iteratively project
//! a noisy input toward the nearest valid state.

use alloc::vec::Vec;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use super::error::{HolographicError, HolographicResult};
use super::domain::{finite,key_inverse,cosine,MemoryElement};

fn certificate<A:MemoryElement>(observed:&A,candidate:&A)->HolographicResult<(bool,f64)>{let obs=finite(observed)?;let val=finite(candidate)?;if obs.len()!=val.len(){return Err(HolographicError::DimensionMismatch{expected:obs.len(),actual:val.len()});}let(mut accepted,mut max)=(true,0_f64);for(x,y)in obs.iter().zip(val){let err=(x-y).abs();accepted &= err<=1e-10+1e-8*x.abs();max=max.max(err);}Ok((accepted,max))}
use crate::algebra::BindingAlgebra;

/// Configuration for resonator cleanup.
#[derive(Clone, Debug)]
pub struct ResonatorConfig {
    /// Maximum iterations before giving up
    pub max_iterations: usize,
    /// Convergence threshold (similarity between iterations)
    pub convergence_threshold: f64,
    /// Temperature annealing: start soft, end hard
    pub initial_beta: f64,
    /// Final temperature (usually large for hard cleanup)
    pub final_beta: f64,
}

impl Default for ResonatorConfig {
    fn default() -> Self {
        Self {
            max_iterations: 50,
            convergence_threshold: 0.999,
            initial_beta: 1.0,
            final_beta: 100.0,
        }
    }
}

/// Result of a cleanup operation.
#[derive(Clone, Debug)]
pub struct CleanupResult<A: BindingAlgebra> {
    /// The cleaned-up representation
    pub cleaned: A,
    /// Number of iterations performed
    pub iterations: usize,
    /// Whether the resonator converged
    pub converged: bool,
    /// Final similarity to best match
    pub final_similarity: f64,
    /// Index in codebook of best match
    pub best_match_index: usize,
}

/// Result of a factorization operation.
#[derive(Clone, Debug)]
pub struct FactorizationResult<A: BindingAlgebra> {
    /// First factor
    pub factor_a: A,
    /// Second factor
    pub factor_b: A,
    /// Number of iterations performed
    pub iterations: usize,
    /// Whether every reconstructed coefficient meets the frozen observation certificate.
    pub accepted: bool,
    /// Maximum absolute reconstruction residual.
    pub residual: f64,
}

/// A resonator network for cleaning up noisy retrievals.
///
/// Given a set of known valid states (codebook), iteratively projects
/// a noisy input toward the nearest valid state.
///
/// For holographic memory, the codebook is typically the stored keys or values.
#[derive(Clone)]
pub struct Resonator<A: BindingAlgebra> {
    /// The codebook of valid states
    codebook: Vec<A>,
    /// Configuration for cleanup
    config: ResonatorConfig,
}

impl<A: MemoryElement> Resonator<A> {
    /// Create a resonator with the given codebook.
    ///
    /// # Errors
    ///
    /// Returns `HolographicError::EmptyCodebook` if the codebook is empty.
    pub fn new(codebook: Vec<A>, config: ResonatorConfig) -> HolographicResult<Self> {
        if codebook.is_empty() {
            return Err(HolographicError::EmptyCodebook);
        }

        for word in &codebook { finite(word)?; }
        Ok(Self { codebook, config })
    }

    /// Clean up a noisy input by resonating against the codebook.
    pub fn cleanup(&self, noisy: &A) -> CleanupResult<A> {
        let mut current = noisy.clone();
        let mut iterations = 0;
        let mut converged = false;
        let mut prev_similarity = 0.0;

        for iter in 0..self.config.max_iterations {
            iterations = iter + 1;

            // Compute temperature for this iteration (annealing)
            let t = iter as f64 / self.config.max_iterations.max(1) as f64;
            let beta =
                self.config.initial_beta + t * (self.config.final_beta - self.config.initial_beta);

            // Compute similarities to all codebook items
            #[cfg(feature = "parallel")]
            let similarities: Vec<f64> = self
                .codebook
                .par_iter()
                .map(|c| current.similarity(c))
                .collect();

            #[cfg(not(feature = "parallel"))]
            let similarities: Vec<f64> = self
                .codebook
                .iter()
                .map(|c| current.similarity(c))
                .collect();

            // Find best match
            let (best_idx, best_sim) = similarities
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal))
                .map(|(i, s)| (i, *s))
                .unwrap_or((0, 0.0));

            // Check for convergence
            if (best_sim - prev_similarity).abs() < 1.0 - self.config.convergence_threshold {
                converged = true;
                return CleanupResult {
                    cleaned: self.codebook[best_idx].clone(),
                    iterations,
                    converged,
                    final_similarity: best_sim,
                    best_match_index: best_idx,
                };
            }
            prev_similarity = best_sim;

            // Update current estimate using weighted bundle
            // Weights are softmax of similarities at temperature beta
            let weights = softmax_weights(&similarities, beta);

            current = self.weighted_bundle(&weights);
        }

        // Did not converge, return best match
        let (best_idx, best_sim) = self
            .codebook
            .iter()
            .enumerate()
            .map(|(i, c)| (i, current.similarity(c)))
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal))
            .unwrap_or((0, 0.0));

        CleanupResult {
            cleaned: self.codebook[best_idx].clone(),
            iterations,
            converged,
            final_similarity: best_sim,
            best_match_index: best_idx,
        }
    }

    /// Factorize a bound pair: given `A⊛B`, find `A` and `B` from codebooks.
    pub fn factorize(&self, bound: &A, other_codebook: &Resonator<A>) -> HolographicResult<FactorizationResult<A>> {
        finite(bound)?;
        // Reject singular candidates, even if the search would not visit them.
        let left=self.codebook.iter().map(key_inverse).collect::<HolographicResult<Vec<_>>>()?;
        let right=other_codebook.codebook.iter().map(key_inverse).collect::<HolographicResult<Vec<_>>>()?;
        let mut a=0;
        for iterations in 1..=self.config.max_iterations {
            let b=other_codebook.best_index(&left[a].bind(bound))?;
            let next=self.best_index(&bound.bind(&right[b]))?;
            let (accepted,residual)=certificate(bound,&self.codebook[next].bind(&other_codebook.codebook[b]))?;
            if accepted || next==a || iterations==self.config.max_iterations {return Ok(FactorizationResult{factor_a:self.codebook[next].clone(),factor_b:other_codebook.codebook[b].clone(),iterations,accepted,residual});} a=next;
        }
        Err(HolographicError::ResonatorDidNotConverge(0))
    }

    fn best_index(&self,input:&A)->HolographicResult<usize>{
        let obs=finite(input)?;let mut best=0;let mut max=f64::NEG_INFINITY;let mut best_word=Vec::new();
        for(i,word)in self.codebook.iter().enumerate(){let c=finite(word)?;let score=cosine(&obs,&c)?;
            if score>max || (score==max && c<best_word){best=i;max=score;best_word=c;}}
        Ok(best)
    }

    /// Compute weighted bundle of codebook items.
    fn weighted_bundle(&self, weights: &[f64]) -> A {
        if self.codebook.is_empty() {
            return A::zero();
        }

        // Use bundle_all with soft bundling
        // For now, just return the item with highest weight
        let best_idx = weights
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal))
            .map(|(i, _)| i)
            .unwrap_or(0);

        self.codebook[best_idx].clone()
    }

    /// Get the codebook size.
    pub fn codebook_size(&self) -> usize {
        self.codebook.len()
    }

    /// Get a reference to a codebook item.
    pub fn get_codebook_item(&self, index: usize) -> Option<&A> {
        self.codebook.get(index)
    }
}

/// Compute softmax weights with temperature.
pub fn softmax_weights(similarities: &[f64], beta: f64) -> Vec<f64> {
    if similarities.is_empty() {
        return Vec::new();
    }

    // Find max for numerical stability
    let max_sim = similarities
        .iter()
        .cloned()
        .fold(f64::NEG_INFINITY, f64::max);

    // Compute exp(beta * (x - max))
    let exps: Vec<f64> = similarities
        .iter()
        .map(|&s| (beta * (s - max_sim)).exp())
        .collect();

    // Normalize
    let sum: f64 = exps.iter().sum();
    if sum > 1e-10 {
        exps.iter().map(|e| e / sum).collect()
    } else {
        // Uniform weights if sum is too small
        let uniform = 1.0 / similarities.len() as f64;
        alloc::vec![uniform; similarities.len()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_softmax_weights() {
        let sims = alloc::vec![1.0, 2.0, 3.0];
        let weights = softmax_weights(&sims, 1.0);

        // Weights should sum to 1
        let sum: f64 = weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-10);

        // Larger similarity should have larger weight
        assert!(weights[2] > weights[1]);
        assert!(weights[1] > weights[0]);
    }

    #[test]
    fn test_softmax_high_temperature() {
        let sims = alloc::vec![1.0, 2.0, 3.0];
        let weights = softmax_weights(&sims, 100.0);

        // At high temperature, should be nearly winner-take-all
        assert!(weights[2] > 0.99);
    }

    #[test]
    fn test_softmax_low_temperature() {
        let sims = alloc::vec![1.0, 2.0, 3.0];
        let weights = softmax_weights(&sims, 0.01);

        // At low temperature, should be nearly uniform
        let diff = (weights[0] - weights[2]).abs();
        assert!(diff < 0.1);
    }

    #[test]
    fn test_resonator_config_default() {
        let config = ResonatorConfig::default();
        assert_eq!(config.max_iterations, 50);
        assert_eq!(config.initial_beta, 1.0);
        assert_eq!(config.final_beta, 100.0);
    }
}
