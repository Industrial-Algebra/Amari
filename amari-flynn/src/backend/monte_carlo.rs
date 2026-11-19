//! Monte Carlo statistical verification

use crate::contracts::VerificationResult;
use crate::statistical::bounds::hoeffding_epsilon;
use rand::Rng;

/// Default tail probability for verification verdicts (95% confidence).
pub const DEFAULT_DELTA: f64 = 0.05;

/// Monte Carlo verification backend
///
/// Uses statistical sampling to verify probability bounds under the two
/// explicit semantic frames (audit contract D1(c)):
///
/// - [`MonteCarloVerifier::verify_probability_at_least`]: the predicate is
///   the event whose HOLD probability must be at least a threshold.
/// - [`MonteCarloVerifier::verify_violation_bound`]: the predicate is the
///   VIOLATION event whose probability must be at most a bound.
///
/// The historical `verify_probability_bound(pred, bound)` checked
/// `P(pred) <= bound` regardless of which frame the caller intended; it is
/// removed (audit finding F1). Pass a violation predicate to
/// [`MonteCarloVerifier::verify_violation_bound`] for the old behavior —
/// with the frame now stated by the method name.
pub struct MonteCarloVerifier {
    samples: usize,
    delta: f64,
}

impl MonteCarloVerifier {
    /// Create verifier with specified sample count at the default
    /// confidence (95%, [`DEFAULT_DELTA`]).
    ///
    /// # Panics
    ///
    /// Panics if `samples` is zero (audit finding F6).
    pub fn new(samples: usize) -> Self {
        Self::with_confidence(samples, DEFAULT_DELTA)
    }

    /// Create verifier with specified sample count and tail probability
    /// `delta` (confidence `1 - delta`).
    ///
    /// # Panics
    ///
    /// Panics if `samples` is zero or `delta` is not in (0, 1].
    pub fn with_confidence(samples: usize, delta: f64) -> Self {
        assert!(samples >= 1, "sample count must be at least 1");
        assert!(
            delta > 0.0 && delta <= 1.0,
            "delta must be in (0, 1], got {delta}"
        );
        Self { samples, delta }
    }

    /// Number of samples per verification run.
    pub fn samples(&self) -> usize {
        self.samples
    }

    /// Tail probability in use.
    pub fn delta(&self) -> f64 {
        self.delta
    }

    /// Verify that `P(predicate) >= threshold` (hold frame).
    ///
    /// `predicate` is the event that should occur. Verdicts at confidence
    /// `1 - delta` via Hoeffding:
    /// - `Verified` when the lower confidence bound on P(predicate) is at
    ///   least `threshold`;
    /// - `Violated` when the upper confidence bound is below `threshold`;
    /// - `Inconclusive` when the interval straddles `threshold`.
    pub fn verify_probability_at_least<F>(&self, predicate: F, threshold: f64) -> VerificationResult
    where
        F: Fn() -> bool,
    {
        let (_estimate, lower, upper) = self.estimate_probability(predicate);
        if lower >= threshold {
            VerificationResult::Verified
        } else if upper < threshold {
            VerificationResult::Violated
        } else {
            VerificationResult::Inconclusive
        }
    }

    /// Verify that `P(violation) <= bound` (violation frame).
    ///
    /// `violation` is the BAD event whose probability must stay under
    /// `bound`. Verdicts at confidence `1 - delta`:
    /// - `Verified` when the upper confidence bound on P(violation) is at
    ///   most `bound`;
    /// - `Violated` when the lower confidence bound exceeds `bound`;
    /// - `Inconclusive` when the interval straddles `bound`.
    pub fn verify_violation_bound<F>(&self, violation: F, bound: f64) -> VerificationResult
    where
        F: Fn() -> bool,
    {
        let (_estimate, lower, upper) = self.estimate_probability(violation);
        if upper <= bound {
            VerificationResult::Verified
        } else if lower > bound {
            VerificationResult::Violated
        } else {
            VerificationResult::Inconclusive
        }
    }

    /// Seeded hold-frame verification: one caller-provided rng is threaded
    /// through every sample, so runs are reproducible (audit D5(a)).
    pub fn verify_probability_at_least_with_rng<R, F>(
        &self,
        rng: &mut R,
        predicate: F,
        threshold: f64,
    ) -> VerificationResult
    where
        R: Rng,
        F: FnMut(&mut R) -> bool,
    {
        let (_, lower, upper) = self.estimate_probability_with_rng(rng, predicate);
        if lower >= threshold {
            VerificationResult::Verified
        } else if upper < threshold {
            VerificationResult::Violated
        } else {
            VerificationResult::Inconclusive
        }
    }

    /// Seeded violation-frame verification (see
    /// [`Self::verify_violation_bound`] and D5(a)).
    pub fn verify_violation_bound_with_rng<R, F>(
        &self,
        rng: &mut R,
        violation: F,
        bound: f64,
    ) -> VerificationResult
    where
        R: Rng,
        F: FnMut(&mut R) -> bool,
    {
        let (_, lower, upper) = self.estimate_probability_with_rng(rng, violation);
        if upper <= bound {
            VerificationResult::Verified
        } else if lower > bound {
            VerificationResult::Violated
        } else {
            VerificationResult::Inconclusive
        }
    }

    /// Estimate probability of predicate with confidence interval
    ///
    /// Returns `(estimate, lower_bound, upper_bound)` where the interval
    /// holds with confidence `1 - delta` (the exact inversion of this
    /// crate's [`crate::statistical::bounds::hoeffding_bound`]).
    pub fn estimate_probability<F>(&self, predicate: F) -> (f64, f64, f64)
    where
        F: Fn() -> bool,
    {
        let successes = (0..self.samples).filter(|_| predicate()).count();
        self.interval_from_successes(successes)
    }

    /// Seeded estimation: threads one rng through every sample (D5(a)).
    pub fn estimate_probability_with_rng<R, F>(&self, rng: &mut R, predicate: F) -> (f64, f64, f64)
    where
        R: Rng,
        F: FnMut(&mut R) -> bool,
    {
        let mut successes = 0usize;
        let mut predicate = predicate;
        for _ in 0..self.samples {
            if predicate(rng) {
                successes += 1;
            }
        }
        self.interval_from_successes(successes)
    }

    fn interval_from_successes(&self, successes: usize) -> (f64, f64, f64) {
        let estimate = successes as f64 / self.samples as f64;
        let epsilon = hoeffding_epsilon(self.samples, self.delta);
        let lower = (estimate - epsilon).max(0.0);
        let upper = (estimate + epsilon).min(1.0);
        (estimate, lower, upper)
    }
}

impl Default for MonteCarloVerifier {
    fn default() -> Self {
        Self::new(10000)
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::{RngExt, SeedableRng};
    use std::cell::Cell;

    #[test]
    fn zero_samples_rejected_at_construction() {
        assert!(std::panic::catch_unwind(|| MonteCarloVerifier::new(0)).is_err());
    }

    #[test]
    fn hold_frame_verifies_a_genuinely_holding_event() {
        // F1 anchor: P(cond)=0.99 with threshold 0.95 must be Verified,
        // not Violated as the inverted implementation reported.
        let verifier = MonteCarloVerifier::new(4000);
        let counter = Cell::new(0u32);
        let result = verifier.verify_probability_at_least(
            || {
                counter.set(counter.get().wrapping_add(1));
                !counter.get().is_multiple_of(100) // P(hold) = 0.99
            },
            0.95,
        );
        assert_eq!(result, VerificationResult::Verified);
    }

    #[test]
    fn hold_frame_violates_a_rarely_holding_event() {
        // F1 anchor: P(cond)=0.02 with threshold 0.95 must be Violated,
        // not Verified as the inverted implementation reported.
        let verifier = MonteCarloVerifier::new(4000);
        let counter = Cell::new(0u32);
        let result = verifier.verify_probability_at_least(
            || {
                counter.set(counter.get().wrapping_add(1));
                counter.get().is_multiple_of(50) // P(hold) = 0.02
            },
            0.95,
        );
        assert_eq!(result, VerificationResult::Violated);
    }

    #[test]
    fn violation_frame_matches_the_old_call_shape() {
        // Migration anchor for karpal-verify's verify_rare_event: an
        // always-false violation predicate must verify; an always-true
        // violation predicate must fail.
        let verifier = MonteCarloVerifier::new(2000);
        assert_eq!(
            verifier.verify_violation_bound(|| false, 0.05),
            VerificationResult::Verified
        );
        assert_eq!(
            verifier.verify_violation_bound(|| true, 0.05),
            VerificationResult::Violated
        );
    }

    #[test]
    fn seeded_runs_are_reproducible() {
        let verifier = MonteCarloVerifier::new(2000);
        let run = |seed: u64| {
            let mut rng = StdRng::seed_from_u64(seed);
            verifier
                .estimate_probability_with_rng(&mut rng, |r: &mut StdRng| r.random::<f64>() < 0.3)
        };
        let a = run(42);
        let b = run(42);
        assert_eq!(a, b);
        let c = run(7);
        assert_ne!(a.0, c.0);
    }

    #[test]
    fn interval_is_the_exact_hoeffding_inversion() {
        // At n=4000, delta=0.05 the half-width must be the exact
        // sqrt(ln(2/0.05)/(2*4000)) ~= 0.021474, not the historical
        // 1.802x-wider value.
        let verifier = MonteCarloVerifier::new(4000);
        let (est, lower, upper) = verifier.estimate_probability(|| false);
        assert_eq!(est, 0.0);
        // est = 0, so lower clamps to 0.0 and upper = epsilon exactly.
        let half_width = upper - lower;
        let expected = hoeffding_epsilon(4000, 0.05);
        assert!((half_width - expected).abs() < 1e-15);
        assert!(half_width < 0.022);
        assert_eq!(lower, 0.0);
    }

    #[test]
    fn boundary_behavior_is_inconclusive_or_correct() {
        // P=1.0 against threshold 1.0: lower bound < 1.0 (epsilon > 0), so
        // the strict reading gives Inconclusive; both verdicts are
        // acceptable but never Violated.
        let verifier = MonteCarloVerifier::new(1000);
        let result = verifier.verify_probability_at_least(|| true, 1.0);
        assert!(matches!(
            result,
            VerificationResult::Verified | VerificationResult::Inconclusive
        ));
    }

    #[test]
    fn custom_confidence_widens_the_interval() {
        let v95 = MonteCarloVerifier::with_confidence(2000, 0.05);
        let v999 = MonteCarloVerifier::with_confidence(2000, 0.001);
        let (_, _, hi95) = v95.estimate_probability(|| false);
        let (_, _, hi999) = v999.estimate_probability(|| false);
        assert!(hi999 > hi95);
    }
}
