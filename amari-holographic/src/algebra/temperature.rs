//! Checked two-term logarithmic reductions. Beta is an inverse temperature.
use super::{AlgebraError, AlgebraResult};

fn validate(a: f64, b: f64, beta: f64) -> AlgebraResult<()> {
    if !a.is_finite() || !b.is_finite() {
        return Err(AlgebraError::NonFinite);
    }
    if beta.is_nan() || beta < 0. {
        return Err(AlgebraError::Beta);
    }
    Ok(())
}

// beta * |a-b|, even when the unscaled difference overflows.
fn gap(a: f64, b: f64, beta: f64) -> f64 {
    let d = (a - b).abs();
    if d.is_finite() {
        beta * d
    } else {
        ((a * 0.5 - b * 0.5).abs() * beta) * 2.
    }
}

/// `ln(exp(βa)+exp(βb))/β`. Diverges as β approaches zero from above.
/// Infinite positive β gives the maximum; zero β has no finite result.
pub fn log_sum_exp(a: f64, b: f64, beta: f64) -> AlgebraResult<f64> {
    validate(a, b, beta)?;
    if beta == 0. {
        return Err(AlgebraError::Beta);
    }
    let hi = a.max(b);
    if beta == f64::INFINITY {
        return Ok(hi);
    }
    let log_tail = (-gap(a, b, beta)).exp().ln_1p();
    let correction = log_tail / beta;
    let y = if correction.is_finite() {
        hi + correction
    } else {
        beta.mul_add(hi, log_tail) / beta
    };
    if y.is_finite() {
        Ok(y)
    } else {
        Err(AlgebraError::Unrepresentable)
    }
}

/// `ln((exp(βa)+exp(βb))/2)/β`. At β=0 returns the arithmetic mean.
/// Equal inputs return that input exactly, including at infinite β.
pub fn log_mean_exp(a: f64, b: f64, beta: f64) -> AlgebraResult<f64> {
    validate(a, b, beta)?;
    if a == b {
        return Ok(a);
    }
    let mid = a.midpoint(b);
    if beta == 0. {
        return Ok(mid);
    }
    let hi = a.max(b);
    if beta == f64::INFINITY {
        return Ok(hi);
    }
    let d = gap(a, b, beta);
    let half_span = if (a - b).is_finite() {
        (a - b).abs() * 0.5
    } else {
        (a * 0.5 - b * 0.5).abs()
    };
    // log(cosh(z))/z ~ z/2: retain the mean and its leading correction
    // when subtracting ln(2) would lose the entire answer.
    let z = d * 0.5;
    let y = if z < 1e-4 {
        mid + half_span * (z * 0.5 * (1. - z * z / 6.))
    } else if z < 1. {
        mid + (2. * (z * 0.5).sinh().powi(2)).ln_1p() / beta
    } else {
        hi + (((-d).exp().ln_1p() - core::f64::consts::LN_2) / beta)
    };
    if y.is_finite() {
        Ok(y)
    } else {
        Err(AlgebraError::Unrepresentable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finite_and_hard_limits() {
        for (a, b) in [(0., 10.), (10., 0.)] {
            let expected = ((1. + 10_f64.exp()) / 2.).ln();
            assert!((log_mean_exp(a, b, 1.).unwrap() - expected).abs() < 1e-12);
            assert!((log_sum_exp(a, b, 1.).unwrap() - (1. + 10_f64.exp()).ln()).abs() < 1e-12);
            assert_eq!(log_mean_exp(a, b, f64::INFINITY), Ok(10.));
            assert_eq!(log_sum_exp(a, b, f64::INFINITY), Ok(10.));
            assert!((log_mean_exp(a, b, 1e-15).unwrap() - 5.).abs() < 1e-13);
            assert!((log_mean_exp(a, b, 1e8).unwrap() - 10.).abs() < 1e-8);
        }
    }
    #[test]
    fn large_range_and_invalid_inputs() {
        for f in [log_mean_exp, log_sum_exp] {
            for beta in [-1., f64::NEG_INFINITY, f64::NAN] {
                assert_eq!(f(0., 0., beta), Err(AlgebraError::Beta));
            }
            assert_eq!(f(f64::NAN, 1., 1.), Err(AlgebraError::NonFinite));
            assert_eq!(f(1., f64::INFINITY, 1.), Err(AlgebraError::NonFinite));
        }
        assert_eq!(
            log_sum_exp(0., 0., f64::from_bits(1)),
            Err(AlgebraError::Unrepresentable)
        );
        let v = log_mean_exp(-1e308, 1e308, 1e-308).unwrap();
        assert!((v / 1e308 - 1_f64.cosh().ln()).abs() < 1e-14);
    }
}
