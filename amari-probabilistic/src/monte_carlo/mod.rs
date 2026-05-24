//! Monte Carlo methods for geometric algebra
//!
//! This module provides Monte Carlo integration and variance reduction
//! techniques for computing expectations on multivector spaces.
//!
//! # Techniques
//!
//! - **Basic Monte Carlo**: Simple averaging of samples
//! - **Importance Sampling**: Reweight samples from proposal distribution
//! - **Control Variates**: Reduce variance using correlated variables
//! - **Antithetic Variates**: Reduce variance using negatively correlated pairs
//!
//! # Example
//!
//! ```ignore
//! use amari_probabilistic::monte_carlo::MonteCarloEstimator;
//! use amari_probabilistic::distribution::GaussianMultivector;
//!
//! let dist = GaussianMultivector::<3, 0, 0>::standard();
//! let estimator = MonteCarloEstimator::new(&dist);
//!
//! // Estimate E[||X||²]
//! let estimate = estimator.estimate(|x| x.norm_squared(), 10000)?;
//! ```

use crate::distribution::Distribution;
use crate::error::{ProbabilisticError, Result};
use amari_core::Multivector;

/// Monte Carlo estimator for expectations
#[derive(Debug, Clone)]
pub struct MonteCarloEstimator<'a, const P: usize, const Q: usize, const R: usize, D>
where
    D: Distribution<Multivector<P, Q, R>>,
{
    /// Distribution to sample from
    distribution: &'a D,
}

impl<'a, const P: usize, const Q: usize, const R: usize, D> MonteCarloEstimator<'a, P, Q, R, D>
where
    D: Distribution<Multivector<P, Q, R>>,
{
    /// Create a new Monte Carlo estimator
    pub fn new(distribution: &'a D) -> Self {
        Self { distribution }
    }

    /// Estimate E[f(X)] using simple Monte Carlo
    ///
    /// # Arguments
    ///
    /// * `f` - Function to compute expectation of
    /// * `num_samples` - Number of samples to use
    pub fn estimate<F>(&self, f: F, num_samples: usize) -> Result<f64>
    where
        F: Fn(&Multivector<P, Q, R>) -> f64,
    {
        if num_samples == 0 {
            return Err(ProbabilisticError::insufficient_samples(1, 0));
        }

        let mut rng = rand::rng();
        let sum: f64 = (0..num_samples)
            .map(|_| {
                let x = self.distribution.sample(&mut rng);
                f(&x)
            })
            .sum();

        Ok(sum / num_samples as f64)
    }

    /// Estimate E[f(X)] with variance estimate
    ///
    /// Returns (mean, variance_of_mean)
    pub fn estimate_with_variance<F>(&self, f: F, num_samples: usize) -> Result<(f64, f64)>
    where
        F: Fn(&Multivector<P, Q, R>) -> f64,
    {
        if num_samples < 2 {
            return Err(ProbabilisticError::insufficient_samples(2, num_samples));
        }

        let mut rng = rand::rng();
        let values: Vec<f64> = (0..num_samples)
            .map(|_| {
                let x = self.distribution.sample(&mut rng);
                f(&x)
            })
            .collect();

        let n = num_samples as f64;
        let mean = values.iter().sum::<f64>() / n;
        let variance = values.iter().map(|&v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0);
        let variance_of_mean = variance / n;

        Ok((mean, variance_of_mean))
    }

    /// Estimate using antithetic variates
    ///
    /// For symmetric distributions, pairs (X, -X) have negative correlation,
    /// reducing variance.
    pub fn estimate_antithetic<F>(&self, f: F, num_pairs: usize) -> Result<f64>
    where
        F: Fn(&Multivector<P, Q, R>) -> f64,
    {
        if num_pairs == 0 {
            return Err(ProbabilisticError::insufficient_samples(1, 0));
        }

        let _dim = 1 << (P + Q + R);
        let mut rng = rand::rng();
        let mut sum = 0.0;

        for _ in 0..num_pairs {
            let x = self.distribution.sample(&mut rng);
            let fx = f(&x);

            // Create antithetic variate (negated)
            let neg_coeffs: Vec<f64> = x.to_vec().iter().map(|&c| -c).collect();
            let neg_x = Multivector::from_coefficients(neg_coeffs);
            let f_neg_x = f(&neg_x);

            // Average the pair
            sum += 0.5 * (fx + f_neg_x);
        }

        Ok(sum / num_pairs as f64)
    }
}

/// Importance sampling estimator
pub struct ImportanceSampler<'a, const P: usize, const Q: usize, const R: usize, Target, Proposal>
where
    Target: Distribution<Multivector<P, Q, R>>,
    Proposal: Distribution<Multivector<P, Q, R>>,
{
    /// Target distribution
    target: &'a Target,
    /// Proposal distribution
    proposal: &'a Proposal,
}

impl<'a, const P: usize, const Q: usize, const R: usize, Target, Proposal>
    ImportanceSampler<'a, P, Q, R, Target, Proposal>
where
    Target: Distribution<Multivector<P, Q, R>>,
    Proposal: Distribution<Multivector<P, Q, R>>,
{
    /// Create a new importance sampler
    pub fn new(target: &'a Target, proposal: &'a Proposal) -> Self {
        Self { target, proposal }
    }

    /// Estimate E_target[f(X)] using importance sampling
    pub fn estimate<F>(&self, f: F, num_samples: usize) -> Result<f64>
    where
        F: Fn(&Multivector<P, Q, R>) -> f64,
    {
        self.estimate_with_rng(f, num_samples, &mut rand::rng())
    }

    /// Estimate E_target[f(X)] using a caller-provided RNG (for deterministic tests).
    pub fn estimate_with_rng<F, Rng: rand::Rng>(
        &self,
        f: F,
        num_samples: usize,
        rng: &mut Rng,
    ) -> Result<f64>
    where
        F: Fn(&Multivector<P, Q, R>) -> f64,
    {
        if num_samples == 0 {
            return Err(ProbabilisticError::insufficient_samples(1, 0));
        }

        let mut weighted_sum = 0.0;
        let mut weight_sum = 0.0;

        for _ in 0..num_samples {
            let x = self.proposal.sample(rng);

            let log_target = self.target.log_prob(&x).unwrap_or(f64::NEG_INFINITY);
            let log_proposal = self.proposal.log_prob(&x).unwrap_or(f64::NEG_INFINITY);

            if log_target.is_finite() && log_proposal.is_finite() {
                let log_weight = log_target - log_proposal;
                let weight = log_weight.exp();

                weighted_sum += weight * f(&x);
                weight_sum += weight;
            }
        }

        if weight_sum == 0.0 {
            return Err(ProbabilisticError::numerical(
                "importance_sampling",
                "All weights are zero - proposal may not cover target",
            ));
        }

        Ok(weighted_sum / weight_sum)
    }

    /// Compute effective sample size (ESS)
    pub fn effective_sample_size(&self, num_samples: usize) -> Result<f64> {
        let mut rng = rand::rng();
        let mut weights = Vec::with_capacity(num_samples);

        for _ in 0..num_samples {
            let x = self.proposal.sample(&mut rng);

            let log_target = self.target.log_prob(&x).unwrap_or(f64::NEG_INFINITY);
            let log_proposal = self.proposal.log_prob(&x).unwrap_or(f64::NEG_INFINITY);

            if log_target.is_finite() && log_proposal.is_finite() {
                weights.push((log_target - log_proposal).exp());
            }
        }

        if weights.is_empty() {
            return Ok(0.0);
        }

        // Normalize weights
        let sum_w: f64 = weights.iter().sum();
        let normalized: Vec<f64> = weights.iter().map(|&w| w / sum_w).collect();

        // ESS = 1 / sum(w_i²)
        let sum_sq: f64 = normalized.iter().map(|&w| w * w).sum();
        Ok(1.0 / sum_sq)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distribution::GaussianMultivector;

    #[test]
    fn test_monte_carlo_estimator() {
        let dist = GaussianMultivector::<2, 0, 0>::standard();
        let estimator = MonteCarloEstimator::new(&dist);

        // Estimate E[scalar_part] - should be ~0 for standard Gaussian
        let estimate = estimator.estimate(|x| x.get(0), 10000).unwrap();
        assert!(estimate.abs() < 0.1);
    }

    #[test]
    fn test_monte_carlo_with_variance() {
        let dist = GaussianMultivector::<2, 0, 0>::standard();
        let estimator = MonteCarloEstimator::new(&dist);

        let (_mean, var) = estimator
            .estimate_with_variance(|x| x.get(0), 1000)
            .unwrap();

        // Variance of mean should be ~1/n = 0.001
        assert!(var < 0.1);
    }

    #[test]
    fn test_antithetic_variates() {
        let dist = GaussianMultivector::<2, 0, 0>::standard();
        let estimator = MonteCarloEstimator::new(&dist);

        // For symmetric function like x², antithetic should give same result
        let estimate = estimator
            .estimate_antithetic(|x| x.get(0) * x.get(0), 5000)
            .unwrap();

        // E[X²] = 1 for standard Gaussian
        assert!((estimate - 1.0).abs() < 0.2);
    }

    #[test]
    fn test_importance_sampling() {
        use rand::SeedableRng;

        let target =
            GaussianMultivector::<2, 0, 0>::isotropic(Multivector::scalar(2.0), 1.0).unwrap();
        let proposal = GaussianMultivector::<2, 0, 0>::standard();
        let sampler = ImportanceSampler::new(&target, &proposal);

        // Use a seeded RNG for deterministic results.
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let estimate = sampler
            .estimate_with_rng(|x| x.get(0), 10000, &mut rng)
            .unwrap();
        assert!((estimate - 2.0).abs() < 0.5);
    }
}
