//! Generic (runtime-signature) Clifford algebra operations.
//!
//! These functions work directly on coefficient slices with a runtime
//! signature (p, q, r), unlike the const-generic Multivector<P,Q,R> API.
//! They serve the WASM fallback path and any consumer that needs
//! signature selection at runtime.

/// Compute the result basis-blade index and sign for the geometric product
/// of two basis blades `i` and `j` in Cl(p, q, r).
///
/// - `p`: number of basis vectors squaring to +1
/// - `q`: number of basis vectors squaring to -1
/// - `r`: number of basis vectors squaring to 0
/// - `dim`: total dimension = p + q + r
/// - `i`, `j`: basis-blade indices (0 .. 2^dim)
///
/// Returns `(result_blade_index, sign)` where sign is +1.0, -1.0, or
/// 0.0 (if a null-squaring basis vector causes the product to vanish).
pub fn blade_product(
    p: usize,
    q: usize,
    _r: usize,
    dim: usize,
    i: usize,
    j: usize,
) -> (usize, f64) {
    let k = i ^ j; // XOR for result blade

    // Compute reordering sign: number of swaps to merge sorted bases.
    // Each basis vector in `j` must be moved past the basis vectors in
    // `i` that have lower index.
    let mut sign = 1.0f64;
    let mut swap_count = 0u32;

    // For each bit position, if it is set in j, count how many set bits
    // in i appear to its left (at higher positions).  We iterate from
    // LSB to MSB so "already processed" means lower bits.
    let mut remaining_i = i;
    for bit in 0..dim {
        if (j >> bit) & 1 == 1 {
            // Count set bits in i at positions > bit (higher index = left)
            let higher_bits = (remaining_i >> (bit + 1)).count_ones();
            swap_count += higher_bits;
        }
        remaining_i &= !(1 << bit);
    }

    if swap_count % 2 == 1 {
        sign = -sign;
    }

    // Compute metric sign: for each bit set in BOTH i and j,
    // multiply by the metric of that basis vector.
    let common = i & j;
    for bit in 0..dim {
        if (common >> bit) & 1 == 1 {
            if bit < p {
                // Positive signature: squares to +1, no sign change
            } else if bit < p + q {
                // Negative signature: squares to -1
                sign = -sign;
            } else {
                // Null signature: squares to 0 → product vanishes
                return (k, 0.0);
            }
        }
    }

    (k, sign)
}

/// Compute the full geometric product of two multivectors in Cl(p, q, r).
///
/// `a` and `b` are coefficient slices of length 2^(p+q+r).  Returns a new
/// coefficient vector of the same length.
pub fn generic_geometric_product(p: usize, q: usize, r: usize, a: &[f64], b: &[f64]) -> Vec<f64> {
    let dim = p + q + r;
    let basis_count = 1 << dim;
    let mut result = vec![0.0; basis_count];

    for (i, &ai) in a.iter().enumerate() {
        if ai.abs() < f64::MIN_POSITIVE {
            continue;
        }

        for (j, &bj) in b.iter().enumerate() {
            if bj.abs() < f64::MIN_POSITIVE {
                continue;
            }

            let (k, sign) = blade_product(p, q, r, dim, i, j);
            if sign != 0.0 {
                result[k] += sign * ai * bj;
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Multivector;

    // ---- blade_product tests ----

    #[test]
    fn test_blade_product_e1_e1_cl300() {
        // e1 * e1 = 1 in Cl(3,0,0)
        let (k, sign) = blade_product(3, 0, 0, 3, 1, 1);
        assert_eq!(k, 0); // scalar
        assert!((sign - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_blade_product_e1_e2_cl300() {
        // e1 * e2 = e12 in Cl(3,0,0)
        let (k, sign) = blade_product(3, 0, 0, 3, 1, 2);
        assert_eq!(k, 3); // e12 (binary 011)
        assert!((sign - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_blade_product_e2_e1_cl300() {
        // e2 * e1 = -e12 in Cl(3,0,0) — anticommutative
        let (k, sign) = blade_product(3, 0, 0, 3, 2, 1);
        assert_eq!(k, 3);
        assert!((sign + 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_blade_product_e3_e3_cl210() {
        // e3 * e3 = -1 in Cl(2,1,0) — negative signature
        // e3 is index 4 (binary 100, bit 2)
        let (k, sign) = blade_product(2, 1, 0, 3, 4, 4);
        assert_eq!(k, 0);
        assert!((sign + 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_blade_product_null_signature() {
        // e1 * e1 = 0 when e1 is null (Cl(0,0,1))
        let (k, sign) = blade_product(0, 0, 1, 1, 1, 1);
        assert_eq!(k, 0);
        assert!((sign - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_blade_product_e12_e1_cl300() {
        // e12 * e1 = -e2 in Cl(3,0,0)
        // e12 = index 3, e1 = index 1, result e2 = index 2, sign = -1
        let (k, sign) = blade_product(3, 0, 0, 3, 3, 1);
        assert_eq!(k, 2); // e2
        assert!((sign + 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_blade_product_scalar_scalar() {
        // scalar * scalar = scalar
        let (k, sign) = blade_product(3, 0, 0, 3, 0, 0);
        assert_eq!(k, 0);
        assert!((sign - 1.0).abs() < 1e-10);
    }

    // ---- generic_geometric_product tests ----

    #[test]
    fn test_generic_gp_matches_const_generic_cl300() {
        let mv_a = Multivector::<3, 0, 0>::basis_vector(0);
        let mv_b = Multivector::<3, 0, 0>::basis_vector(1);
        let expected = mv_a.geometric_product(&mv_b);

        let a_coeffs: Vec<f64> = (0..8).map(|i| mv_a.get(i)).collect();
        let b_coeffs: Vec<f64> = (0..8).map(|i| mv_b.get(i)).collect();
        let result = generic_geometric_product(3, 0, 0, &a_coeffs, &b_coeffs);

        for i in 0..8 {
            assert!(
                (result[i] - expected.get(i)).abs() < 1e-10,
                "Coefficient {} differs: {} vs {}",
                i,
                result[i],
                expected.get(i)
            );
        }
    }

    #[test]
    fn test_generic_gp_matches_const_generic_cl210() {
        // Cl(2,1,0): e3 squares to -1
        let mv_a = Multivector::<2, 1, 0>::basis_vector(2);
        let mv_b = Multivector::<2, 1, 0>::basis_vector(2);
        let expected = mv_a.geometric_product(&mv_b);

        let a_coeffs: Vec<f64> = (0..8).map(|i| mv_a.get(i)).collect();
        let b_coeffs: Vec<f64> = (0..8).map(|i| mv_b.get(i)).collect();
        let result = generic_geometric_product(2, 1, 0, &a_coeffs, &b_coeffs);

        for i in 0..8 {
            assert!(
                (result[i] - expected.get(i)).abs() < 1e-10,
                "Coefficient {} differs: {} vs {}",
                i,
                result[i],
                expected.get(i)
            );
        }
    }

    #[test]
    fn test_generic_gp_random_cl410() {
        // Cl(4,1,0) — 32 coefficients, CGA signature
        // Use deterministic values to avoid rand API version churn
        let a_coeffs: Vec<f64> = (0..32).map(|i| ((i as f64) * 0.137) % 1.5 - 0.75).collect();
        let b_coeffs: Vec<f64> = (0..32)
            .map(|i| ((i as f64 + 17.0) * 0.097) % 1.5 - 0.75)
            .collect();

        let mv_a = Multivector::<4, 1, 0>::from_coefficients(a_coeffs.clone());
        let mv_b = Multivector::<4, 1, 0>::from_coefficients(b_coeffs.clone());
        let expected = mv_a.geometric_product(&mv_b);

        let result = generic_geometric_product(4, 1, 0, &a_coeffs, &b_coeffs);

        for i in 0..32 {
            assert!(
                (result[i] - expected.get(i)).abs() < 1e-8,
                "Coefficient {} differs: {} vs {}",
                i,
                result[i],
                expected.get(i)
            );
        }
    }

    #[test]
    fn test_generic_gp_scalar_only() {
        // Scalar algebra Cl(0,0,0) — only the scalar component
        let a = vec![2.0];
        let b = vec![3.0];
        let result = generic_geometric_product(0, 0, 0, &a, &b);
        assert_eq!(result.len(), 1);
        assert!((result[0] - 6.0).abs() < 1e-10);
    }
}
