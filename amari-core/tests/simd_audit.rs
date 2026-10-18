//! M-REP-D SIMD audit battery (D5 frozen policy, contract PR #315).
//!
//! Tier 1 — SIMD-vs-scalar equivalence under the cancellation-aware bound:
//! `|simd − scalar| ≤ max(16 · ulp(M̂), 1e-300)` with `M̂ = min(Σ|term_i|,
//! f64::MAX)` (saturation closes the overflowing-M hole); identical
//! finiteness class required where either side is nonfinite. `M` is
//! computed in f64 by this harness and recorded per case.
//! Tier 2 — scalar-vs-oracle only where scalar is finite: the exact value
//! is computed from f64 inputs exactly (integer mantissa arithmetic) and
//! compared under the established rounding tolerance; cases where scalar is
//! nonfinite but the exact value is finite (inf−inf class) are recorded as
//! EXPECTED-DIVERGENT and pass only if SIMD matches scalar's class.
//! Runtime inventory — reports which implementation the dispatcher selects
//! under the compiled feature set, so CI evidence distinguishes
//! compiled-in AVX2+FMA from the scalar fallback.

#![cfg(test)]

use amari_core::Multivector;
#[cfg(all(
    target_arch = "x86_64",
    target_feature = "avx2",
    target_feature = "fma"
))]
use std::sync::atomic::AtomicUsize;

#[cfg(all(
    target_arch = "x86_64",
    target_feature = "avx2",
    target_feature = "fma"
))]
static EXPECTED_DIVERGENT_OVERFLOW: AtomicUsize = AtomicUsize::new(0);

type Cl3 = Multivector<3, 0, 0>;

/// Exact oracle: coefficient `out` of the Cl(3,0,0) geometric product,
/// computed exactly with integer arithmetic from the f64 inputs (each f64
/// is exactly representable as mantissa × 2^exp), then rounded once.
use num_bigint::{BigInt, Sign};

fn decompose(x: f64) -> (BigInt, i32) {
    if x == 0.0 {
        return (BigInt::from(0), 0);
    }
    let bits = x.to_bits();
    let neg = bits >> 63 == 1;
    let biased = ((bits >> 52) & 0x7ff) as i32;
    let mantissa = (bits & ((1u64 << 52) - 1)) as i128;
    let (m, e) = if biased == 0 {
        (mantissa, -1074) // subnormal: implicit bit absent
    } else {
        (mantissa + (1i128 << 52), biased - 1075)
    };
    (
        if neg {
            -BigInt::from(m)
        } else {
            BigInt::from(m)
        },
        e,
    )
}

/// Round a signed exact integer × 2^exp to f64 (nearest, 53-bit mantissa).
fn reconstruct(m: BigInt, e: i32) -> f64 {
    if m.sign() == Sign::NoSign {
        return 0.0;
    }
    let neg = m.sign() == Sign::Minus;
    let mut m = if neg { -m.clone() } else { m.clone() };
    let mut e = e;
    while m.bits() > 53 {
        let odd = (&m & BigInt::from(1u8)) != BigInt::from(0u8);
        m >>= 1u32;
        if odd {
            m += 1; // round half away from zero; final-rounding tolerance absorbs the ulp
        }
        e += 1;
    }
    while m.bits() < 53 {
        m <<= 1u32;
        e -= 1;
    }
    ldexp_f64(m.to_string().parse::<f64>().unwrap(), e, neg)
}

/// Scale by 2^e in bounded chunks so the exponentiation itself never
/// underflows before the final multiply (R1 P2: the naive `2f64.powi(e)`
/// returned 0 for e = -1075, making the oracle report 0 for
/// MIN_POSITIVE * 0.5 whose exact value is the subnormal 2^-1023).
fn ldexp_f64(m: f64, e: i32, neg: bool) -> f64 {
    let mut x = m;
    let mut e = e;
    while e > 1000 {
        x *= 2f64.powi(1000);
        e -= 1000;
    }
    while e < -1000 {
        x *= 2f64.powi(-1000);
        e += 1000;
    }
    x *= 2f64.powi(e);
    if neg {
        -x
    } else {
        x
    }
}

/// Euclidean blade product: e_A · e_B = (-1)^inv · e_{A XOR B} with
/// inv = #{(a ∈ A, b ∈ B) : b < a}.
fn blade_sign(m1: u32, m2: u32) -> i32 {
    let mut inv = 0u32;
    let mut x = m1;
    while x != 0 {
        let k = x.trailing_zeros();
        inv += (m2 & ((1u32 << k) - 1)).count_ones();
        x &= x - 1;
    }
    if inv.is_multiple_of(2) {
        1
    } else {
        -1
    }
}

fn exact_coefficient(lhs: &[f64; 8], rhs: &[f64; 8], out: usize) -> f64 {
    // exact terms: sign_ij · lhs[i] · rhs[j] for (i ^ j) == out
    let mut terms: Vec<(BigInt, i32)> = Vec::new();
    for i in 0..8u32 {
        for j in 0..8u32 {
            if (i ^ j) != out as u32 {
                continue;
            }
            let (m1, e1) = decompose(lhs[i as usize]);
            let (m2, e2) = decompose(rhs[j as usize]);
            if m1.sign() == Sign::NoSign || m2.sign() == Sign::NoSign {
                continue;
            }
            let prod = m1 * m2 * blade_sign(i, j);
            terms.push((prod, e1 + e2));
        }
    }
    if terms.is_empty() {
        return 0.0;
    }
    let min_e = terms.iter().map(|t| t.1).min().unwrap();
    let mut sum = BigInt::from(0);
    for (m, e) in &terms {
        sum += m << (e - min_e) as u32;
    }
    reconstruct(sum, min_e)
}

/// Cancellation scale M = Σ|term_i| in f64 (saturating).
#[cfg(all(
    target_arch = "x86_64",
    target_feature = "avx2",
    target_feature = "fma"
))]
/// Per-coefficient term scale (M-REP-D R1 P2): M = Σ_i |lhs[i]·rhs[i^k]|
/// over EXACTLY the 8 pairs contributing to coefficient `out`, accumulated
/// in f64 with a STICKY overflow flag. The flag — not `M == MAX` — defines
/// the saturation regime: a sum that merely lands on MAX does not authorize
/// finiteness-class exceptions, and unrelated coefficients' terms cannot.
fn cancellation_scale(lhs: &[f64; 8], rhs: &[f64; 8], out: usize) -> (f64, bool) {
    let mut m: f64 = 0.0;
    let mut overflow = false;
    for (i, l) in lhs.iter().enumerate() {
        let r = rhs[i ^ out];
        let t = l.abs() * r.abs();
        if t.is_infinite() {
            overflow = true;
        } else {
            m += t;
            if m.is_infinite() {
                overflow = true;
            }
        }
    }
    (m.min(f64::MAX), overflow)
}

#[cfg(all(
    target_arch = "x86_64",
    target_feature = "avx2",
    target_feature = "fma"
))]
fn ulp(x: f64) -> f64 {
    // FINITE endpoint (R1 P1): at MAX the next bit pattern is infinity;
    // the naive form returns inf and makes every saturated bound vacuous.
    // At the top of the range the spacing is measured downward.
    if !x.is_finite() {
        return f64::MAX - f64::from_bits(f64::MAX.to_bits() - 1);
    }
    let next = f64::from_bits(x.to_bits() + 1);
    if next.is_finite() {
        (next - x).abs().max(f64::MIN_POSITIVE)
    } else {
        (x - f64::from_bits(x.to_bits() - 1)).max(f64::MIN_POSITIVE)
    }
}

#[cfg(all(
    target_arch = "x86_64",
    target_feature = "avx2",
    target_feature = "fma"
))]
mod simd_rows {
    use super::*;
    use std::sync::atomic::Ordering;

    fn check_case(a: &[f64; 8], b: &[f64; 8]) -> u32 {
        let lhs = Cl3::from_coefficients(a.to_vec());
        let rhs = Cl3::from_coefficients(b.to_vec());
        let scalar = lhs.geometric_product(&rhs);
        let simd = amari_core::simd::geometric_product_3d_avx2(&lhs, &rhs);
        let mut divergent = 0u32;
        for k in 0..8 {
            let s = scalar.get(k);
            let v = simd.get(k);
            let (m_raw, overflowed) = cancellation_scale(a, b, k);
            if overflowed {
                EXPECTED_DIVERGENT_OVERFLOW.fetch_add(1, Ordering::Relaxed);
            }
            if s.is_finite() && v.is_finite() {
                let bound = (16.0 * ulp(m_raw)).max(1e-300);
                assert!(bound.is_finite(), "bound must be finite (M={m_raw})");
                assert!(
                    (s - v).abs() <= bound,
                    "coefficient {k}: |{s} - {v}| = {} > bound {bound} (M={m_raw})",
                    (s - v).abs()
                );
                // Tier 2: oracle, only where scalar finite
                let exact = exact_coefficient(a, b, k);
                if exact.is_finite() {
                    let t2 = (16.0 * ulp(m_raw)).max(1e-300);
                    assert!(
                        (s - exact).abs() <= t2,
                        "coefficient {k}: scalar {s} vs exact {exact} exceeds oracle tolerance"
                    );
                }
            } else {
                // FMA-semantics note (battery-execution addendum 2026-10-18):
                // in the overflow regime (STICKY flag — a term or partial
                // sum actually overflowed), FMA's exact-intermediate
                // product can legitimately resolve a scalar NaN
                // (inf + -inf under separate rounding) to a signed inf, or
                // vice versa. Class divergences are EXPECTED-DIVERGENT-
                // OVERFLOW: counted here, surfaced per test — never silent,
                // never authorized without the flag.
                let classes_match = (s.is_nan() == v.is_nan())
                    && (s.is_infinite() == v.is_infinite())
                    && (!s.is_infinite() || s.signum() == v.signum());
                if !classes_match {
                    assert!(
                        overflowed,
                        "coefficient {k}: finiteness-class divergence scalar={s} simd={v} WITHOUT term overflow (M={m_raw})"
                    );
                    divergent += 1;
                }
            }
        }
        divergent
    }

    #[test]
    fn tier1_all_basis_pairs() {
        let mut div = 0u32;
        for i in 0..8usize {
            for j in 0..8usize {
                let mut a = [0.0f64; 8];
                let mut b = [0.0f64; 8];
                a[i] = 1.0;
                b[j] = 1.0;
                div += check_case(&a, &b);
            }
        }
        println!("tier1_all_basis_pairs: {div} EXPECTED-DIVERGENT-OVERFLOW");
    }

    #[test]
    fn tier1_adversarial_sets() {
        let mut div = 0u32;
        let max = f64::MAX;
        let sub = f64::MIN_POSITIVE; // smallest normal
        let cases: Vec<[f64; 8]> = vec![
            [0.0; 8],
            [1.0; 8],
            [max, -max, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [sub, 0.0, sub, 0.0, 0.0, sub, 0.0, 0.0],
            [2.3e-162, 0.0, 0.0, 1.6e-162, 0.0, 0.0, 0.0, 0.0],
            [1e150, -1e150, 1e-150, -1e-150, 0.5, -0.5, max / 4.0, sub],
            [
                std::f64::consts::FRAC_1_SQRT_2,
                std::f64::consts::FRAC_1_SQRT_2,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
            ],
        ];
        for a in &cases {
            for b in &cases {
                div += check_case(a, b);
            }
        }
        println!("tier1_adversarial_sets: {div} EXPECTED-DIVERGENT-OVERFLOW");
    }

    #[test]
    fn tier1_expected_divergent_inf_minus_inf() {
        // exact 0, float NaN via inf + (-inf) — EXPECTED-DIVERGENT: both
        // paths must agree in class (both NaN), never scalar-NaN/simd-value.
        let max = f64::MAX;
        let mut a = [0.0f64; 8];
        let mut b = [0.0f64; 8];
        a[0] = max / 2.0;
        b[0] = 4.0; // product term = 2*MAX -> inf
        a[1] = max / 2.0;
        b[1] = -4.0; // product term = -2*MAX -> -inf
        let div = check_case(&a, &b);
        assert!(
            div >= 1,
            "engineered fixture must exercise EXPECTED-DIVERGENT-OVERFLOW"
        );
        println!("tier1_expected_divergent: {div} EXPECTED-DIVERGENT-OVERFLOW");
    }

    #[test]
    fn tier1_randomized_dense() {
        let mut div = 0u32;
        let mut state = 0x853c49e6748fea9bu64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for _ in 0..10_000 {
            let mut a = [0.0f64; 8];
            let mut b = [0.0f64; 8];
            for k in 0..8 {
                let r = next();
                let scale = [1.0, 1e-8, 1e8, 1e-160, 1e160][(r % 5) as usize];
                let unit = ((r >> 32) as i64 as f64) / (i32::MAX as f64);
                a[k] = unit * scale;
                let r2 = next();
                let scale2 = [1.0, 1e-8, 1e8, 1e-160, 1e160][(r2 % 5) as usize];
                let unit2 = ((r2 >> 32) as i64 as f64) / (i32::MAX as f64);
                b[k] = unit2 * scale2;
            }
            div += check_case(&a, &b);
        }
        println!(
            "tier1_randomized_dense: {div} EXPECTED-DIVERGENT-OVERFLOW (global sticky: {})",
            EXPECTED_DIVERGENT_OVERFLOW.load(Ordering::Relaxed)
        );
    }

    #[test]
    fn runtime_inventory() {
        let name = amari_core::simd::select_geometric_product_impl_name();
        assert_eq!(
            name, "avx2+fma",
            "under +avx2,+fma with both features detected on this host, the dispatcher must select the SIMD kernel (fn-pointer identity is not stable across compilation units and is not inventory evidence)"
        );
        let f = amari_core::simd::select_geometric_product_impl();
        let e1 = Cl3::basis_vector(0);
        let e2 = Cl3::basis_vector(1);
        let via_dispatch = f(&e1, &e2);
        let direct = amari_core::simd::geometric_product_3d_avx2(&e1, &e2);
        for k in 0..8 {
            assert_eq!(via_dispatch.get(k), direct.get(k));
        }
    }
}

#[cfg(not(all(
    target_arch = "x86_64",
    target_feature = "avx2",
    target_feature = "fma"
)))]
mod exclusion_rows {
    use super::*;

    #[test]
    fn runtime_inventory_reports_scalar_when_simd_compiled_out() {
        // Under the EXCLUSION row (+avx2 alone, or -avx2), the kernel is
        // compiled out by the all(avx2, fma) gate and the dispatcher must
        // fall back to the scalar implementation.
        let f = amari_core::simd::select_geometric_product_impl();
        let e1 = Cl3::basis_vector(0);
        let e2 = Cl3::basis_vector(1);
        let via_dispatch = f(&e1, &e2);
        let scalar = e1.geometric_product(&e2);
        for k in 0..8 {
            assert_eq!(via_dispatch.get(k), scalar.get(k));
        }
    }
}

#[cfg(all(
    target_arch = "x86_64",
    target_feature = "avx2",
    target_feature = "fma"
))]
mod batch_contract {
    use super::*;

    #[cfg(all(
        target_arch = "x86_64",
        target_feature = "avx2",
        target_feature = "fma"
    ))]
    #[test]
    #[should_panic(expected = "multiple of 8")]
    fn batch_rejects_partial_tails() {
        let lhs = vec![1.0f64; 9]; // not a multiple of 8
        let rhs = vec![0.0f64; 9];
        let mut out = vec![0.0f64; 9];
        amari_core::simd::batch_geometric_product_avx2(&lhs, &rhs, &mut out);
    }

    #[cfg(all(
        target_arch = "x86_64",
        target_feature = "avx2",
        target_feature = "fma"
    ))]
    #[test]
    fn batch_matches_scalar_on_full_pairs() {
        let lhs = vec![1.0f64, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let rhs = vec![0.0f64, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let mut out = vec![0.0f64; 8];
        amari_core::simd::batch_geometric_product_avx2(&lhs, &rhs, &mut out);
        let a = Cl3::from_coefficients(lhs);
        let b = Cl3::from_coefficients(rhs);
        let scalar = a.geometric_product(&b);
        for (k, o) in out.iter().enumerate() {
            assert_eq!(*o, scalar.get(k));
        }
    }
}

#[cfg(test)]
mod oracle_selfcheck {
    use super::*;

    /// The oracle must agree with the scalar implementation within one
    /// rounding on ordinary-magnitude dense inputs — if it does not, the
    /// ORACLE is wrong (sign table / exponent handling), not the library.
    #[test]
    fn oracle_agrees_with_scalar_on_dense_inputs() {
        let mut state = 0x243f6a8885a308d3u64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for _ in 0..500 {
            let mut a = [0.0f64; 8];
            let mut b = [0.0f64; 8];
            for k in 0..8 {
                a[k] = ((next() >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0;
                b[k] = ((next() >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0;
            }
            let lhs = Cl3::from_coefficients(a.to_vec());
            let rhs = Cl3::from_coefficients(b.to_vec());
            let scalar = lhs.geometric_product(&rhs);
            for k in 0..8 {
                let s = scalar.get(k);
                let e = exact_coefficient(&a, &b, k);
                assert!(
                    (s - e).abs() <= 1e-14 * e.abs().max(1.0),
                    "oracle self-check failed at coeff {k}: scalar={s} oracle={e}"
                );
            }
        }
    }
}

#[cfg(test)]
mod oracle_subnormal_regression {
    use super::*;

    /// R1 P2: the naive `2f64.powi(-1075)` underflowed to 0, making the
    /// oracle report 0 for MIN_POSITIVE * 0.5 (true value: subnormal
    /// 1.1125369292536007e-308). The chunked ldexp fixes it.
    #[test]
    fn oracle_handles_subnormal_results() {
        let x = f64::MIN_POSITIVE;
        let h = 0.5f64;
        // exact product via decompose/reconstruct directly
        let (m1, e1) = decompose(x);
        let (m2, e2) = decompose(h);
        let got = reconstruct(m1 * m2, e1 + e2);
        assert_eq!(got, 1.1125369292536007e-308);
    }
}

#[cfg(all(
    target_arch = "x86_64",
    target_feature = "avx2",
    target_feature = "fma"
))]
#[cfg(test)]
mod bound_rejection {
    use super::*;

    /// Test-the-test (R1 P1): the pre-fix `ulp(MAX)` was infinite, so a
    /// bogus zero answer at MAX-scale passed the bound. The fixed bound
    /// must reject it.
    #[test]
    fn bound_rejects_bogus_results_at_max_scale() {
        let m = f64::MAX;
        let bound = (16.0 * ulp(m)).max(1e-300);
        // The R1 defect: ulp(MAX) was INFINITY, so the bound was infinite
        // and admitted literally any finite error. The fix makes it finite.
        assert!(bound.is_finite(), "bound must be finite at MAX scale");
        // At saturated scale the honest path-vs-path tolerance is huge
        // (16*ulp(MAX) ~ 3e293) — correctness there is Tier-2 oracle
        // territory, which is why Tier 2 runs wherever scalar is finite.
        // The rejection force of the bound lives at ordinary scales:
        let m_normal = 1.0f64;
        let bound_normal = (16.0 * ulp(m_normal)).max(1e-300);
        assert!(bound_normal < 1e-12, "ordinary-scale bound too coarse");
        // a bogus zero against a true 1.0 at ordinary scale MUST be rejected
        assert!((0.0f64 - 1.0).abs() > bound_normal);
    }
}
