//! Common probability distributions
//!
//! Two sampling APIs per distribution (audit D5(a)):
//! - `sample()` — convenience: draws from the thread-local rng. NOT
//!   reproducible across runs.
//! - `sample_with_rng(&mut R)` — reproducible: the caller owns the rng
//!   (seed it with `rand::SeedableRng` for deterministic verification
//!   runs).
//!
//! The `Prob` wrapper on samples carries the probability OF THE OUTCOME
//! where it is well-defined (Bernoulli: `p` for `true`, `1 - p` for
//! `false`), and unit weight for continuous distributions where a single
//! sample has density, not mass (audit F9 — documented per type).

use crate::prob::Prob;
use rand::{Rng, RngExt};
use rand_distr::{Bernoulli as RandBernoulli, Distribution, Exp, Normal as RandNormal};

/// Uniform distribution over a range
#[derive(Clone, Copy, Debug)]
pub struct Uniform<T> {
    min: T,
    max: T,
}

impl Uniform<i32> {
    /// Create uniform distribution over `[min, max)`
    ///
    /// # Panics
    ///
    /// Panics if `min >= max` at CONSTRUCTION (audit F6: the historical
    /// version deferred the panic to sample time, far from the cause).
    pub fn new(min: i32, max: i32) -> Self {
        assert!(min < max, "Uniform requires min < max, got [{min}, {max})");
        Self { min, max }
    }

    /// Sample from the distribution (thread-local rng; not reproducible).
    pub fn sample(&self) -> Prob<i32> {
        let mut rng = rand::rng();
        self.sample_with_rng(&mut rng)
    }

    /// Sample with a caller-owned rng (reproducible; D5(a)).
    pub fn sample_with_rng<R: Rng>(&self, rng: &mut R) -> Prob<i32> {
        let value = rng.random_range(self.min..self.max);
        // A uniform draw over n values: each outcome has mass 1/n. Widen
        // BEFORE subtracting: (i32::MAX - i32::MIN) overflows i32 (R1 P1).
        let n = (i64::from(self.max) - i64::from(self.min)) as f64;
        Prob::with_probability(1.0 / n, value)
    }
}

/// Bernoulli distribution (binary outcome)
#[derive(Clone, Copy, Debug)]
pub struct Bernoulli {
    p: f64,
}

impl Bernoulli {
    /// Create Bernoulli distribution with probability p
    ///
    /// # Panics
    ///
    /// Panics if `p` is not in [0, 1].
    pub fn new(p: f64) -> Self {
        assert!((0.0..=1.0).contains(&p), "Probability must be in [0, 1]");
        Self { p }
    }

    /// Sample from the distribution (thread-local rng; not reproducible).
    pub fn sample(&self) -> Prob<bool> {
        let mut rng = rand::rng();
        self.sample_with_rng(&mut rng)
    }

    /// Sample with a caller-owned rng (reproducible; D5(a)).
    ///
    /// The `Prob` label is the probability OF THE SAMPLED OUTCOME
    /// (audit F9): `p` when the value is `true`, `1 - p` when `false`
    /// (the historical version labeled both outcomes with `p`).
    pub fn sample_with_rng<R: Rng>(&self, rng: &mut R) -> Prob<bool> {
        let dist = RandBernoulli::new(self.p).expect("p validated at construction");
        let value = dist.sample(rng);
        let outcome_probability = if value { self.p } else { 1.0 - self.p };
        Prob::with_probability(outcome_probability, value)
    }
}

/// Normal (Gaussian) distribution
#[derive(Clone, Copy, Debug)]
pub struct Normal {
    mean: f64,
    std_dev: f64,
}

impl Normal {
    /// Create normal distribution with given mean and standard deviation
    ///
    /// # Panics
    ///
    /// Panics if `std_dev` is not positive.
    pub fn new(mean: f64, std_dev: f64) -> Self {
        assert!(std_dev > 0.0, "Standard deviation must be positive");
        Self { mean, std_dev }
    }

    /// Sample from the distribution (thread-local rng; not reproducible).
    pub fn sample(&self) -> Prob<f64> {
        let mut rng = rand::rng();
        self.sample_with_rng(&mut rng)
    }

    /// Sample with a caller-owned rng (reproducible; D5(a)).
    ///
    /// Continuous distribution: a single sample has DENSITY, not mass —
    /// the `Prob` wrapper carries the value with unit weight (audit F9;
    /// documented, not a probability claim).
    pub fn sample_with_rng<R: Rng>(&self, rng: &mut R) -> Prob<f64> {
        let dist =
            RandNormal::new(self.mean, self.std_dev).expect("std_dev validated at construction");
        let value = dist.sample(rng);
        Prob::new(value)
    }
}

/// Exponential distribution
#[derive(Clone, Copy, Debug)]
pub struct Exponential {
    lambda: f64,
}

impl Exponential {
    /// Create exponential distribution with rate parameter lambda
    ///
    /// # Panics
    ///
    /// Panics if `lambda` is not positive.
    pub fn new(lambda: f64) -> Self {
        assert!(lambda > 0.0, "Lambda must be positive");
        Self { lambda }
    }

    /// Sample from the distribution (thread-local rng; not reproducible).
    pub fn sample(&self) -> Prob<f64> {
        let mut rng = rand::rng();
        self.sample_with_rng(&mut rng)
    }

    /// Sample with a caller-owned rng (reproducible; D5(a)).
    ///
    /// Continuous distribution: density, not mass (see [`Normal`]).
    pub fn sample_with_rng<R: Rng>(&self, rng: &mut R) -> Prob<f64> {
        let dist = Exp::new(self.lambda).expect("lambda validated at construction");
        let value = dist.sample(rng);
        Prob::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    #[test]
    fn uniform_rejects_empty_ranges_at_construction() {
        assert!(std::panic::catch_unwind(|| Uniform::new(5, 5)).is_err());
        assert!(std::panic::catch_unwind(|| Uniform::new(7, 3)).is_err());
    }

    #[test]
    fn test_uniform_range() {
        let dist = Uniform::new(1, 7);
        for _ in 0..100 {
            let sample = dist.sample().into_inner();
            assert!((1..7).contains(&sample));
        }
    }

    #[test]
    fn uniform_labels_each_outcome_with_its_mass() {
        let dist = Uniform::new(0, 4);
        let sample = dist.sample();
        assert!((sample.probability() - 0.25).abs() < 1e-15);
    }

    #[test]
    fn full_i32_range_does_not_overflow() {
        // R1 P1: (i32::MAX - i32::MIN) overflows i32 — mass must be
        // 1/4294967295, not a panic.
        let dist = Uniform::new(i32::MIN, i32::MAX);
        let sample = dist.sample();
        assert!((sample.probability() - 1.0 / 4294967295.0).abs() < 1e-20);
        let value = sample.into_inner();
        assert!((i32::MIN..i32::MAX).contains(&value));
    }

    #[test]
    fn test_bernoulli() {
        let dist = Bernoulli::new(0.5);
        let sample = dist.sample();
        // Outcome probability: 0.5 either way.
        assert_eq!(sample.probability(), 0.5);
    }

    #[test]
    fn bernoulli_labels_the_outcome_probability() {
        let dist = Bernoulli::new(0.3);
        for _ in 0..200 {
            let s = dist.sample();
            let expected = if s.into_inner() { 0.3 } else { 0.7 };
            assert!((s.probability() - expected).abs() < 1e-15);
        }
    }

    #[test]
    fn test_normal() {
        let dist = Normal::new(0.0, 1.0);
        let sample = dist.sample();
        // Continuous: unit weight, documented as density-not-mass.
        assert_eq!(sample.probability(), 1.0);
    }

    #[test]
    fn seeded_sampling_is_reproducible() {
        let a = {
            let mut rng = StdRng::seed_from_u64(99);
            Normal::new(0.0, 1.0).sample_with_rng(&mut rng).into_inner()
        };
        let b = {
            let mut rng = StdRng::seed_from_u64(99);
            Normal::new(0.0, 1.0).sample_with_rng(&mut rng).into_inner()
        };
        assert_eq!(a.to_bits(), b.to_bits());
        let c = {
            let mut rng = StdRng::seed_from_u64(100);
            Normal::new(0.0, 1.0).sample_with_rng(&mut rng).into_inner()
        };
        assert_ne!(a.to_bits(), c.to_bits());
    }
}
