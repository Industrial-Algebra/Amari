//! SIMD optimizations for geometric algebra operations
//!
//! This module provides vectorized implementations of critical geometric algebra
//! operations using CPU SIMD instruction sets (AVX2, SSE) for maximum performance.

#[cfg(all(
    target_arch = "x86_64",
    target_feature = "avx2",
    target_feature = "fma"
))]
use core::arch::x86_64::*;

use crate::Multivector;

/// SIMD-optimized geometric product for 3D Euclidean algebra (most common case)
#[cfg(all(
    target_arch = "x86_64",
    target_feature = "avx2",
    target_feature = "fma"
))]
#[inline(always)]
pub fn geometric_product_3d_avx2(
    lhs: &Multivector<3, 0, 0>,
    rhs: &Multivector<3, 0, 0>,
) -> Multivector<3, 0, 0> {
    /// Per-`i` sign tables: lane `k` of the contribution of `lhs[i]` carries
    /// `blade_sign(i, i ^ k)`, the Euclidean blade-product sign
    /// `(-1)^(#{(a ∈ i, b ∈ i^k) : b < a})`. Generated from the same blade
    /// algebra the audit oracle (tests/simd_audit.rs) uses and cross-checked
    /// there over all 64 basis pairs plus 10⁴ randomized dense products.
    ///
    /// M-REP-D S1 note: the previous hand-tuned pattern tables used a
    /// different coefficient-ordering convention than the library's bitmask
    /// order and computed wrong products for most basis pairs (188/448
    /// coefficient mismatches over the pair matrix); this formulation is
    /// correct by construction: output[k] = Σ_i sign(i, i^k)·lhs[i]·rhs[i^k].
    #[rustfmt::skip]
    const SIGN_LO: [[f64; 4]; 8] = [
        [1.0, 1.0, 1.0, 1.0],
        [1.0, 1.0, 1.0, 1.0],
        [1.0, -1.0, 1.0, -1.0],
        [-1.0, 1.0, -1.0, 1.0],
        [1.0, -1.0, -1.0, 1.0],
        [-1.0, 1.0, 1.0, -1.0],
        [-1.0, -1.0, 1.0, 1.0],
        [-1.0, -1.0, 1.0, 1.0],
    ];
    #[rustfmt::skip]
    const SIGN_HI: [[f64; 4]; 8] = [
        [1.0, 1.0, 1.0, 1.0],
        [1.0, 1.0, 1.0, 1.0],
        [1.0, -1.0, 1.0, -1.0],
        [-1.0, 1.0, -1.0, 1.0],
        [1.0, -1.0, -1.0, 1.0],
        [-1.0, 1.0, 1.0, -1.0],
        [-1.0, -1.0, 1.0, 1.0],
        [-1.0, -1.0, 1.0, 1.0],
    ];

    // SAFETY (M-REP-D S1): every gather index is `i ^ k` for i, k < 8 —
    // itself < 8 — so all reads stay inside the 8-f64 coefficient arrays of
    // `lhs`/`rhs`; their length is pinned to 2³ = 8 by every construction
    // path (`from_coefficients` asserts `len == BASIS_COUNT`, boxed slice
    // freezes it). Gathers take no alignment. The FMA intrinsics are
    // compiled only under `all(avx2, fma)` and dispatched only after both
    // are detected at runtime.
    unsafe {
        let lp = lhs.as_slice().as_ptr();
        let rp = rhs.as_slice().as_ptr();
        let mut acc_lo = _mm256_setzero_pd();
        let mut acc_hi = _mm256_setzero_pd();
        for i in 0usize..8 {
            let li = _mm256_set1_pd(*lp.add(i));
            // lanes k = 0..3 gather rhs[i ^ k] (k = 0 gives i itself)
            let idx_lo =
                _mm256_setr_epi64x(i as i64, (i ^ 1) as i64, (i ^ 2) as i64, (i ^ 3) as i64);
            let idx_hi = _mm256_setr_epi64x(
                (i ^ 4) as i64,
                (i ^ 5) as i64,
                (i ^ 6) as i64,
                (i ^ 7) as i64,
            );
            let g_lo = _mm256_i64gather_pd(rp, idx_lo, 8);
            let g_hi = _mm256_i64gather_pd(rp, idx_hi, 8);
            let s_lo = _mm256_loadu_pd(SIGN_LO[i].as_ptr());
            let s_hi = _mm256_loadu_pd(SIGN_HI[i].as_ptr());
            acc_lo = _mm256_fmadd_pd(li, _mm256_mul_pd(s_lo, g_lo), acc_lo);
            acc_hi = _mm256_fmadd_pd(li, _mm256_mul_pd(s_hi, g_hi), acc_hi);
        }
        let mut out = [0.0f64; 8];
        _mm256_storeu_pd(out.as_mut_ptr(), acc_lo);
        _mm256_storeu_pd(out.as_mut_ptr().add(4), acc_hi);
        Multivector::<3, 0, 0>::from_coefficients(out.to_vec())
    }
}

#[cfg(all(
    target_arch = "x86_64",
    target_feature = "avx2",
    target_feature = "fma"
))]
pub fn batch_geometric_product_avx2(
    lhs_batch: &[f64],
    rhs_batch: &[f64],
    result_batch: &mut [f64],
) {
    const COEFFS_PER_MV: usize = 8;
    assert!(
        lhs_batch.len().is_multiple_of(COEFFS_PER_MV)
            && rhs_batch.len().is_multiple_of(COEFFS_PER_MV)
            && result_batch.len().is_multiple_of(COEFFS_PER_MV),
        "batch_geometric_product_avx2: all batches must be a multiple of {COEFFS_PER_MV}          coefficients (8 per Cl(3,0,0) multivector); partial tails are rejected, not truncated"
    );
    let num_pairs = lhs_batch.len() / COEFFS_PER_MV;

    for i in 0..num_pairs {
        let lhs_offset = i * COEFFS_PER_MV;
        let rhs_offset = i * COEFFS_PER_MV;
        let result_offset = i * COEFFS_PER_MV;

        // Create temporary multivectors from slices
        let lhs_coeffs = lhs_batch[lhs_offset..lhs_offset + COEFFS_PER_MV].to_vec();
        let rhs_coeffs = rhs_batch[rhs_offset..rhs_offset + COEFFS_PER_MV].to_vec();

        let lhs_mv = Multivector::<3, 0, 0>::from_coefficients(lhs_coeffs);
        let rhs_mv = Multivector::<3, 0, 0>::from_coefficients(rhs_coeffs);

        // Compute product using SIMD optimization
        let result_mv = geometric_product_3d_avx2(&lhs_mv, &rhs_mv);

        // Copy result back to batch array
        result_batch[result_offset..result_offset + COEFFS_PER_MV]
            .copy_from_slice(result_mv.as_slice());
    }
}

/// CPU feature detection for optimal code path selection
/// In std environments, uses runtime detection; in no_std, uses compile-time detection
pub fn select_geometric_product_impl(
) -> fn(&Multivector<3, 0, 0>, &Multivector<3, 0, 0>) -> Multivector<3, 0, 0> {
    #[cfg(all(
        not(feature = "std"),
        all(
            target_arch = "x86_64",
            target_feature = "avx2",
            target_feature = "fma"
        )
    ))]
    {
        return geometric_product_3d_avx2;
    }

    #[cfg(all(
        feature = "std",
        all(
            target_arch = "x86_64",
            target_feature = "avx2",
            target_feature = "fma"
        )
    ))]
    {
        if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma") {
            return geometric_product_3d_avx2;
        }
    }

    // Fallback to scalar implementation
    |lhs, rhs| lhs.geometric_product(rhs)
}

/// Runtime inventory (M-REP-D/P-AVX2): reports which implementation
/// `select_geometric_product_impl` selects under the compiled feature set
/// and the host's CPU features. Function-pointer identity is NOT stable
/// across compilation units and must not be used as inventory evidence.
pub fn select_geometric_product_impl_name() -> &'static str {
    #[cfg(all(
        not(feature = "std"),
        all(
            target_arch = "x86_64",
            target_feature = "avx2",
            target_feature = "fma"
        )
    ))]
    {
        return "avx2+fma";
    }

    #[cfg(all(
        feature = "std",
        all(
            target_arch = "x86_64",
            target_feature = "avx2",
            target_feature = "fma"
        )
    ))]
    {
        if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma") {
            return "avx2+fma";
        }
    }

    "scalar-fallback"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Multivector;
    use approx::assert_relative_eq;

    type Cl3 = Multivector<3, 0, 0>;

    #[test]
    #[cfg(all(
        target_arch = "x86_64",
        target_feature = "avx2",
        target_feature = "fma"
    ))]
    fn test_simd_geometric_product_correctness() {
        let e1 = Cl3::basis_vector(0);
        let e2 = Cl3::basis_vector(1);

        // Test against scalar implementation
        let scalar_result = e1.geometric_product(&e2);
        let simd_result = geometric_product_3d_avx2(&e1, &e2);

        for i in 0..8 {
            assert_relative_eq!(scalar_result.get(i), simd_result.get(i), epsilon = 1e-14);
        }
    }

    #[test]
    fn test_runtime_feature_detection() {
        let impl_fn = select_geometric_product_impl();

        let e1 = Cl3::basis_vector(0);
        let e2 = Cl3::basis_vector(1);
        let result = impl_fn(&e1, &e2);

        // Should match scalar implementation
        let expected = e1.geometric_product(&e2);
        for i in 0..8 {
            assert_relative_eq!(result.get(i), expected.get(i), epsilon = 1e-14);
        }
    }
}
