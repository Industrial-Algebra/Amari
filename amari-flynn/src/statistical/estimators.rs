//! Probability estimation utilities

/// Estimate probability from sample data
///
/// Returns 0.0 when `total` is zero (no information; callers should treat
/// zero-sample estimates separately).
pub fn estimate_probability(successes: usize, total: usize) -> f64 {
    if total == 0 {
        return 0.0;
    }
    successes as f64 / total as f64
}

/// Compute Wilson score confidence interval for a probability estimate
///
/// Returns `(lower_bound, upper_bound)` at the requested confidence level.
/// The z-score is computed for EVERY level via the Acklam inverse-normal
/// approximation (absolute error < 1.15e-9); the common levels use exact
/// tabulated values. There is no silent substitution — a 0.999 request
/// produces a 0.999 interval (audit finding F4).
///
/// # Panics
///
/// Panics if `confidence` is not in (0, 1).
pub fn confidence_interval(successes: usize, total: usize, confidence: f64) -> (f64, f64) {
    if total == 0 {
        return (0.0, 1.0);
    }

    let p = estimate_probability(successes, total);
    let n = total as f64;

    // Wilson score interval (better for edge cases than normal approximation)
    let z = confidence_z_score(confidence);
    let z_sq = z * z;

    let denominator = 1.0 + z_sq / n;
    let center = (p + z_sq / (2.0 * n)) / denominator;
    let margin = (z / denominator) * ((p * (1.0 - p) / n) + (z_sq / (4.0 * n * n))).sqrt();

    let lower = (center - margin).max(0.0);
    let upper = (center + margin).min(1.0);

    (lower, upper)
}

/// Compute required sample size for desired precision (worst case p = 0.5)
///
/// # Panics
///
/// Panics if `precision` is not positive or `confidence` is not in (0, 1).
pub fn sample_size_for_precision(precision: f64, confidence: f64) -> usize {
    assert!(
        precision > 0.0,
        "precision must be positive, got {precision}"
    );
    let z = confidence_z_score(confidence);
    let n = (z * z) / (4.0 * precision * precision);
    n.ceil() as usize
}

/// Exact z-scores for the most common confidence levels.
const EXACT_Z_TABLE: &[(f64, f64)] = &[
    (0.90, 1.644_853_626_951_472_2_f64),
    (0.95, 1.959_963_984_540_054_5),
    (0.98, 2.326_347_874_040_840_8),
    (0.99, 2.575_829_303_548_900_4),
    (0.995, 2.807_033_768_337_246),
    (0.999, 3.290_526_731_491_925_5),
];

/// Get z-score for a confidence level
///
/// Exact for the tabulated levels; otherwise the Acklam rational
/// approximation of the inverse standard normal CDF, with absolute error
/// below 1.15e-9 (documented bound; the refinement pass is applied).
///
/// # Panics
///
/// Panics if `confidence` is not in (0, 1) — never silently substituted
/// (audit finding F4).
pub fn confidence_z_score(confidence: f64) -> f64 {
    assert!(
        confidence > 0.0 && confidence < 1.0,
        "confidence must be in (0, 1), got {confidence}"
    );
    // TWO-SIDED coverage quantile throughout (consistent with the Wilson
    // interval and the exact table): z satisfies P(-z <= Z <= z) =
    // confidence, i.e. z = Phi^{-1}((1 + confidence) / 2). The historical
    // Acklam path returned the ONE-SIDED quantile Phi^{-1}(confidence),
    // so 0.8 gave 0.8416 instead of 1.2816 (R1 P1).
    for &(level, z) in EXACT_Z_TABLE {
        if confidence == level {
            return z;
        }
    }
    // STABLE lower-tail inversion (R2 P1): computing (1 + c) / 2 first
    // loses the tail to rounding — for the largest f64 confidence below
    // 1.0, 1 + c rounds UP to exactly 2.0 (ties-to-even), the central
    // mass becomes 1.0, and ln(1 - 1) = -inf yields a NaN z-score (and a
    // zero sample size). Alpha = (1 - c) / 2 is exact by construction;
    // z = -Phi^{-1}(alpha) is the same two-sided quantile with no
    // cancellation anywhere.
    let alpha = 0.5 * (1.0 - confidence);
    -acklam_inverse_normal(alpha)
}

/// Acklam's rational approximation to the inverse normal CDF.
///
/// Relative error < 1.15e-9 over the central region; the Halley refinement
/// step is applied, which brings the result to within one ULP of the true
/// quantile for all practical inputs.
fn acklam_inverse_normal(p: f64) -> f64 {
    // Coefficients in rational approximation for inverse normal CDF.
    const A: [f64; 6] = [
        -3.969_683_028_665_376e1,
        2.209_460_984_245_187e2,
        -2.759_285_104_469_687e2,
        1.383_577_518_672_69e2,
        -3.066_479_806_614_716e1,
        2.506_628_277_459_239_f64,
    ];
    const B: [f64; 5] = [
        -5.447_609_879_822_406e1,
        1.615_858_368_580_409e2,
        -1.556_989_798_598_866e2,
        6.680_131_188_771_972e1,
        -1.328_068_155_288_572e1,
    ];
    const C: [f64; 6] = [
        -7.784_894_002_430_293e-3,
        -3.223_964_580_411_365e-1,
        -2.400_758_277_161_838,
        -2.549_732_539_343_734,
        4.374_664_141_464_968,
        2.938_163_982_698_783,
    ];
    const D: [f64; 4] = [
        7.784_695_709_041_462e-3,
        3.224_671_290_700_398e-1,
        2.445_134_137_142_996,
        3.754_408_661_907_416,
    ];
    const P_LOW: f64 = 0.024_25;
    const P_HIGH: f64 = 1.0 - P_LOW;

    if p < P_LOW {
        // Rational approximation for lower region.
        let q = (-2.0 * p.ln()).sqrt();
        let num = (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q) + C[5];
        let den = (((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0;
        refine(num / den, p)
    } else if p <= P_HIGH {
        // Rational approximation for central region.
        let q = p - 0.5;
        let r = q * q;
        let num = (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q;
        let den = ((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0;
        refine(num / den, p)
    } else {
        // Rational approximation for upper region.
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        let num = (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q) + C[5];
        let den = ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q) + 1.0;
        refine(-(num / den), p)
    }
}

/// One Halley refinement step against the normal CDF (erf-based).
fn refine(z: f64, p: f64) -> f64 {
    let e = 0.5 * erfc(-z / core::f64::consts::SQRT_2) - p;
    let u = e * (2.0 * core::f64::consts::PI).sqrt() * (z * z / 2.0).exp();
    z - u / (1.0 + z * u / 2.0)
}

/// Complementary error function, via `libm` (accurate to <1 ulp; works on
/// no_std targets, keeping the refinement step identical everywhere).
fn erfc(x: f64) -> f64 {
    libm::erfc(x)
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_probability() {
        assert_eq!(estimate_probability(5, 10), 0.5);
        assert_eq!(estimate_probability(0, 10), 0.0);
        assert_eq!(estimate_probability(10, 10), 1.0);
    }

    #[test]
    fn z_scores_are_exact_for_tabulated_levels() {
        for &(level, z) in EXACT_Z_TABLE {
            let got = confidence_z_score(level);
            assert!(
                (got - z).abs() < 1e-12,
                "level {level}: got {got}, want {z}"
            );
        }
    }

    #[test]
    fn z_scores_approximate_untabulated_levels_accurately() {
        // TWO-SIDED coverage: z = Phi^{-1}((1 + c) / 2).
        // 0.80 -> Phi^{-1}(0.90) = 1.2815515655446004
        let z80 = confidence_z_score(0.8);
        assert!((z80 - 1.281_551_565_544_600_4).abs() < 1e-9, "got {z80}");
        // 0.975 -> Phi^{-1}(0.9875) = 2.241402727...
        let z975 = confidence_z_score(0.975);
        assert!((z975 - 2.241_402_73).abs() < 1e-6, "got {z975}");
        // 0.9999 -> Phi^{-1}(0.99995) = 3.890591885...
        let z9999 = confidence_z_score(0.9999);
        assert!((z9999 - 3.890_591_88).abs() < 1e-6, "got {z9999}");
        // 0.5 -> Phi^{-1}(0.75) = 0.6744897501960817 (interval no longer
        // collapses — the historical one-sided path gave 0.0).
        let z50 = confidence_z_score(0.5);
        assert!((z50 - 0.674_489_750_196_081_7).abs() < 1e-9, "got {z50}");
        // Continuity across the table edge: 0.98 (tabulated 2.3263...) and
        // 0.9799 (approximated) must nearly agree.
        let a = confidence_z_score(0.98);
        let b = confidence_z_score(0.979_9);
        assert!((a - b).abs() < 6e-3); // ~38x z/c slope near 0.98
    }

    #[test]
    fn extreme_confidence_is_finite_and_useful() {
        // R2 P1 anchor: the largest f64 confidence below 1.0 previously
        // produced NaN (1 + c rounded to 2.0) and a zero sample size.
        let c = 0.999_999_999_999_999_9_f64; // largest f64 < 1.0
        assert!(c < 1.0);
        let z = confidence_z_score(c);
        assert!(z.is_finite(), "z must be finite, got {z}");
        assert!(z > 8.0, "extreme-confidence z should be large, got {z}");
        let n = sample_size_for_precision(0.01, c);
        assert!(n > 100_000, "sample size must be huge, got {n}");
        // Interval at extreme confidence still widens monotonically.
        let (_, hi_max) = confidence_interval(50, 100, c);
        let (_, hi_99) = confidence_interval(50, 100, 0.99);
        assert!(hi_max > hi_99);
    }

    #[test]
    fn no_silent_substitution_outside_valid_range() {
        assert!(std::panic::catch_unwind(|| confidence_z_score(0.0)).is_err());
        assert!(std::panic::catch_unwind(|| confidence_z_score(1.0)).is_err());
        assert!(std::panic::catch_unwind(|| confidence_z_score(1.5)).is_err());
    }

    #[test]
    fn a_999_request_returns_a_999_interval() {
        // F4 anchor: the 99.9% interval must be WIDER than the 99% one and
        // contain the true quantile behavior (z=3.2905 exact entry).
        let (_, hi99) = confidence_interval(50, 100, 0.99);
        let (_, hi999) = confidence_interval(50, 100, 0.999);
        assert!(
            hi999 > hi99,
            "99.9% upper {hi999} must exceed 99% upper {hi99}"
        );
    }

    #[test]
    fn test_confidence_interval() {
        let (lower, upper) = confidence_interval(50, 100, 0.95);
        assert!(lower < 0.5);
        assert!(upper > 0.5);
        assert!(lower >= 0.0);
        assert!(upper <= 1.0);
    }

    #[test]
    fn test_sample_size() {
        let n = sample_size_for_precision(0.05, 0.95);
        assert!(n > 0);
        assert!(n < 1000);
    }
}
