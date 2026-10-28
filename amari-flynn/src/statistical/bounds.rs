//! Concentration inequalities and probability bounds

/// Hoeffding's inequality bound
///
/// For n independent samples of a [0, 1]-bounded random variable, the
/// probability that the sample mean deviates from the true mean by more
/// than `epsilon` is at most `2 * exp(-2 * n * epsilon^2)`.
pub fn hoeffding_bound(n: usize, epsilon: f64) -> f64 {
    2.0 * (-2.0 * n as f64 * epsilon * epsilon).exp()
}

/// Invert [`hoeffding_bound`]: the deviation bound that holds with
/// confidence `1 - delta`.
///
/// Solves `2 * exp(-2 * n * eps^2) <= delta` exactly:
/// `eps = sqrt(ln(2/delta) / (2n))`.
///
/// # Panics
///
/// Panics if `n` is zero or `delta` is not in (0, 1].
pub fn hoeffding_epsilon(n: usize, delta: f64) -> f64 {
    assert!(n >= 1, "sample count must be at least 1");
    assert!(
        delta > 0.0 && delta <= 1.0,
        "delta must be in (0, 1], got {delta}"
    );
    ((2.0_f64.ln() - delta.ln()) / (2.0 * n as f64)).sqrt()
}

/// Additive Chernoff bound for a sum of independent [0, 1]-bounded trials.
///
/// `mu` is the expectation of the sum. For relative deviation
/// `delta` in (0, 1], the probability that the sum exceeds
/// `mu * (1 + delta)` is at most `exp(-mu * delta^2 / 3)`.
///
/// The bound does not depend on the number of trials `n`; the parameter is
/// retained for signature stability and documentation.
///
/// # Panics
///
/// Panics if `delta` is not in (0, 1] or `mu` is negative — the implemented
/// additive form is only valid on that domain (audit finding F5).
pub fn chernoff_bound(n: usize, mu: f64, delta: f64) -> f64 {
    let _ = n;
    assert!(
        delta > 0.0 && delta <= 1.0,
        "delta must be in (0, 1] for the additive Chernoff form, got {delta}"
    );
    assert!(mu >= 0.0, "mu must be a non-negative expectation, got {mu}");
    let exp_term = -(mu * delta * delta) / 3.0;
    exp_term.exp()
}

/// Compute confidence level from sample statistics
///
/// Given samples and observed deviation, compute confidence that
/// the true probability is within epsilon of the observed estimate.
pub fn compute_confidence(samples: usize, epsilon: f64) -> f64 {
    let bound = hoeffding_bound(samples, epsilon);
    (1.0 - bound).clamp(0.0, 1.0)
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    #[test]
    fn test_hoeffding_bound() {
        let bound = hoeffding_bound(100, 0.1);
        assert!(bound > 0.0);
        assert!(bound < 1.0);

        // More samples should give tighter bound
        let bound2 = hoeffding_bound(1000, 0.1);
        assert!(bound2 < bound);
    }

    #[test]
    fn hoeffding_epsilon_is_the_exact_inversion() {
        // The defining property: hoeffding_bound(n, hoeffding_epsilon(n, delta))
        // must be <= delta (up to f64 rounding), and slack must be small.
        for &(n, delta) in &[
            (1usize, 0.05f64),
            (10, 0.05),
            (100, 0.05),
            (1000, 0.05),
            (1000, 0.01),
            (1000, 0.2),
        ] {
            let eps = hoeffding_epsilon(n, delta);
            let bound = hoeffding_bound(n, eps);
            assert!(
                bound <= delta * (1.0 + 1e-12),
                "inversion must not exceed delta: n={n} delta={delta} bound={bound}"
            );
            assert!(
                bound > delta * 0.999999,
                "inversion must be tight: n={n} delta={delta} bound={bound}"
            );
        }
    }

    #[test]
    fn hoeffding_epsilon_matches_closed_form() {
        // sqrt(ln(2/0.05) / (2*1000)) = sqrt(3.6888794541139363/2000)
        let eps = hoeffding_epsilon(1000, 0.05);
        let closed = (2.0_f64.ln() - 0.05_f64.ln()).sqrt() / (2.0 * 1000.0f64).sqrt();
        assert!((eps - closed).abs() < 1e-15);
        // The audit's historical value was 1.802x wider than this.
        assert!(eps < 0.043);
    }

    #[test]
    fn hoeffding_epsilon_rejects_degenerate_inputs() {
        assert!(std::panic::catch_unwind(|| hoeffding_epsilon(0, 0.05)).is_err());
        assert!(std::panic::catch_unwind(|| hoeffding_epsilon(10, 0.0)).is_err());
        assert!(std::panic::catch_unwind(|| hoeffding_epsilon(10, 1.5)).is_err());
    }

    #[test]
    fn test_chernoff_bound() {
        let bound = chernoff_bound(100, 50.0, 0.1);
        assert!(bound > 0.0);
        assert!(bound <= 1.0);
    }

    #[test]
    fn chernoff_bound_enforces_domain() {
        assert!(std::panic::catch_unwind(|| chernoff_bound(10, 5.0, 1.5)).is_err());
        assert!(std::panic::catch_unwind(|| chernoff_bound(10, 5.0, 0.0)).is_err());
        assert!(std::panic::catch_unwind(|| chernoff_bound(10, -1.0, 0.5)).is_err());
    }

    #[test]
    fn test_compute_confidence() {
        let conf = compute_confidence(1000, 0.05);
        assert!(conf > 0.9);
        assert!(conf <= 1.0);
    }
}
