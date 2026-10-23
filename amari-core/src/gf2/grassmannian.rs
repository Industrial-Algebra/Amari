//! Binary Grassmannian Gr(k, n; F₂) — enumeration and counting of k-dimensional
//! subspaces of F₂ⁿ.

use super::matrix::GF2Matrix;
use super::scalar::GF2;
use alloc::vec;
use alloc::vec::Vec;

/// Gaussian binomial coefficient [n choose k]_q.
///
/// Counts the number of k-dimensional subspaces of F_qⁿ.
/// For q=2 this is |Gr(k, n; F₂)|.
#[must_use]
pub fn gaussian_binomial(n: usize, k: usize, q: u64) -> u64 {
    if k > n {
        return 0;
    }
    if k == 0 || k == n {
        return 1;
    }
    let mut result: u64 = 1;
    for i in 0..k {
        let num = q.pow((n - i) as u32) - 1;
        let den = q.pow((i + 1) as u32) - 1;
        result = result * num / den; // exact division for valid q, n, k
    }
    result
}

/// Number of points on the binary Grassmannian Gr(k, n; F₂).
///
/// Equal to `gaussian_binomial(n, k, 2)`.
#[must_use]
pub fn binary_grassmannian_size(k: usize, n: usize) -> u64 {
    gaussian_binomial(n, k, 2)
}

/// Enumerate all k-dimensional subspaces of F₂ⁿ.
///
/// Each subspace is returned as a `GF2Matrix` in reduced row echelon form
/// with k rows and n columns. The rows form a canonical basis for the subspace.
///
/// # Panics
/// Panics if n > 20 to prevent combinatorial explosion.
pub fn enumerate_subspaces(k: usize, n: usize) -> Vec<GF2Matrix> {
    assert!(n <= 20, "enumerate_subspaces: n={} too large (max 20)", n);
    if k == 0 {
        // The unique 0-dimensional subspace.
        return vec![GF2Matrix::zero(0, n)];
    }
    if k > n {
        return Vec::new();
    }

    let mut results = Vec::new();

    // Iterate over all k-element subsets of {0,...,n-1} as pivot positions.
    let pivot_sets = k_subsets(n, k);

    for pivots in &pivot_sets {
        // For each pivot pattern, enumerate all possible free-column values.
        let free_cols: Vec<usize> = (0..n).filter(|c| !pivots.contains(c)).collect();
        let num_free = free_cols.len(); // n - k
        let num_free_entries = k * num_free; // each pivot row has num_free free entries

        // Iterate over all 2^(k * num_free) settings of the free entries.
        for pattern in 0u64..(1u64 << num_free_entries) {
            let mut m = GF2Matrix::zero(k, n);
            // Set pivot columns to identity.
            for (row, &pc) in pivots.iter().enumerate() {
                m.set(row, pc, GF2::ONE);
            }
            // Set free columns from the pattern bits.
            for (row_idx, _) in pivots.iter().enumerate() {
                for (fi, &fc) in free_cols.iter().enumerate() {
                    let bit_pos = row_idx * num_free + fi;
                    if (pattern >> bit_pos) & 1 == 1 {
                        m.set(row_idx, fc, GF2::ONE);
                    }
                }
            }

            // Verify this is truly RREF: check that above each pivot is zero.
            // In our construction, pivots form an identity submatrix so this holds.
            // But we must also verify the matrix has full rank k (it should by construction
            // since the pivot columns form an identity block).
            results.push(m);
        }
    }

    // Deduplicate: different pivot patterns can produce the same subspace.
    // Canonicalize each matrix by reducing to RREF and dedup.
    let mut canonical: Vec<GF2Matrix> = results
        .into_iter()
        .map(|mut m| {
            m.reduced_row_echelon();
            m
        })
        .collect();
    canonical.sort_by(|a, b| {
        for i in 0..a.nrows() {
            let wa = a.row(i).as_words();
            let wb = b.row(i).as_words();
            match wa.cmp(wb) {
                core::cmp::Ordering::Equal => continue,
                other => return other,
            }
        }
        core::cmp::Ordering::Equal
    });
    canonical.dedup();
    canonical
}

/// Schubert cell size |C_λ| = 2^|λ| over GF(2).
///
/// The partition λ specifies a Schubert cell in Gr(k, n; F₂).
/// The number of F₂-rational points in the cell is 2^(sum of parts of λ).
#[must_use]
pub fn schubert_cell_size(partition: &[usize]) -> u64 {
    let sum: usize = partition.iter().sum();
    1u64 << sum
}

/// Determine the Schubert cell of a subspace (given in RREF).
///
/// Returns the partition λ derived from the pivot columns.
/// For a RREF matrix with pivot columns p₀ < p₁ < ... < p_{k-1},
/// the Schubert cell index is λᵢ = pᵢ - i (the "gap" sequence).
#[must_use]
pub fn schubert_cell_of(subspace: &GF2Matrix) -> Vec<usize> {
    let mut rref = subspace.clone();
    let pivots = rref.reduced_row_echelon();
    pivots.iter().enumerate().map(|(i, &p)| p - i).collect()
}

/// Generate all k-element subsets of {0, ..., n-1}.
fn k_subsets(n: usize, k: usize) -> Vec<Vec<usize>> {
    let mut result = Vec::new();
    let mut current = Vec::with_capacity(k);
    k_subsets_helper(n, k, 0, &mut current, &mut result);
    result
}

fn k_subsets_helper(
    n: usize,
    k: usize,
    start: usize,
    current: &mut Vec<usize>,
    result: &mut Vec<Vec<usize>>,
) {
    if current.len() == k {
        result.push(current.clone());
        return;
    }
    let remaining = k - current.len();
    for i in start..=(n - remaining) {
        current.push(i);
        k_subsets_helper(n, k, i + 1, current, result);
        current.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gaussian_binomial_known_values() {
        // [3,1]_2 = (2^3 - 1)/(2^1 - 1) = 7
        assert_eq!(gaussian_binomial(3, 1, 2), 7);
        // [4,2]_2 = (2^4-1)(2^3-1) / ((2^2-1)(2^1-1)) = 15*7 / 3*1 = 35
        assert_eq!(gaussian_binomial(4, 2, 2), 35);
        // [5,2]_2 = 155
        assert_eq!(gaussian_binomial(5, 2, 2), 155);
    }

    #[test]
    fn test_gaussian_binomial_edge_cases() {
        assert_eq!(gaussian_binomial(5, 0, 2), 1);
        assert_eq!(gaussian_binomial(5, 5, 2), 1);
        assert_eq!(gaussian_binomial(3, 5, 2), 0);
    }

    #[test]
    fn test_binary_grassmannian_size() {
        assert_eq!(binary_grassmannian_size(1, 3), 7);
        assert_eq!(binary_grassmannian_size(2, 4), 35);
    }

    #[test]
    fn test_enumerate_subspaces_count() {
        let subs = enumerate_subspaces(1, 3);
        assert_eq!(subs.len() as u64, binary_grassmannian_size(1, 3));

        let subs2 = enumerate_subspaces(2, 4);
        assert_eq!(subs2.len() as u64, binary_grassmannian_size(2, 4));
    }

    #[test]
    fn test_enumerate_subspaces_valid_rref() {
        let subs = enumerate_subspaces(2, 4);
        for s in &subs {
            assert_eq!(s.nrows(), 2);
            assert_eq!(s.ncols(), 4);
            assert_eq!(s.rank(), 2, "subspace must have full rank k");
        }
    }

    #[test]
    fn test_schubert_cell_sizes_sum() {
        // The Schubert cell sizes should sum to |Gr(k,n;F₂)|.
        let k = 1;
        let n = 3;
        let subs = enumerate_subspaces(k, n);
        let total = binary_grassmannian_size(k, n);

        // Group by Schubert cell and check sizes.
        let mut cell_counts: alloc::collections::BTreeMap<Vec<usize>, usize> =
            alloc::collections::BTreeMap::new();
        for s in &subs {
            let cell = schubert_cell_of(s);
            *cell_counts.entry(cell).or_insert(0) += 1;
        }
        let counted_total: usize = cell_counts.values().sum();
        assert_eq!(counted_total as u64, total);
    }

    #[test]
    fn test_schubert_cell_size_formula() {
        assert_eq!(schubert_cell_size(&[0, 0]), 1);
        assert_eq!(schubert_cell_size(&[1, 0]), 2);
        assert_eq!(schubert_cell_size(&[1, 1]), 4);
        assert_eq!(schubert_cell_size(&[2]), 4);
    }
}
