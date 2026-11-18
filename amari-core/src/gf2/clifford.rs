//! Binary Clifford algebra Cl(N, R; F₂) — Clifford algebra over GF(2).
//!
//! Over GF(2), the signature distinction (P vs Q) collapses since +1 = -1.
//! Only non-degenerate (eᵢ² = 1) vs degenerate (eⱼ² = 0) matters.

use super::scalar::GF2;
use alloc::vec;
use alloc::vec::Vec;
use core::fmt;
use core::ops::{Add, Mul};

const WORD_BITS: usize = 64;

#[inline]
fn num_words(basis_count: usize) -> usize {
    basis_count.div_ceil(WORD_BITS)
}

/// Multivector over GF(2) in the Clifford algebra Cl(N, R; F₂).
///
/// `N` non-degenerate generators (eᵢ² = 1) and `R` degenerate generators (eⱼ² = 0).
/// Total dimension: 2^(N+R) basis blades, each with a GF(2) coefficient.
/// Stored as a single bit-packed vector.
///
/// Over GF(2), the geometric product simplifies:
/// - No sign from reordering (since -1 = 1 mod 2)
/// - eₐeᵦ = eₐ⊕ᵦ if A∩B contains only non-degenerate generators
/// - eₐeᵦ = 0 if A∩B contains any degenerate generator
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BinaryMultivector<const N: usize, const R: usize> {
    words: Vec<u64>,
}

impl<const N: usize, const R: usize> BinaryMultivector<N, R> {
    /// Total algebra dimension (number of generators).
    pub const DIM: usize = N + R;
    /// Number of basis blades.
    pub const BASIS_COUNT: usize = 1 << Self::DIM;

    /// Bitmask for degenerate generators: bits N..N+R are set.
    #[inline]
    fn degenerate_mask() -> usize {
        if R == 0 {
            0
        } else {
            ((1usize << R) - 1) << N
        }
    }

    /// Zero multivector.
    #[must_use]
    pub fn zero() -> Self {
        Self {
            words: vec![0u64; num_words(Self::BASIS_COUNT)],
        }
    }

    /// Scalar 1.
    #[must_use]
    pub fn one() -> Self {
        let mut mv = Self::zero();
        mv.set(0, GF2::ONE);
        mv
    }

    /// Single basis blade by index.
    #[must_use]
    pub fn basis_blade(index: usize) -> Self {
        assert!(index < Self::BASIS_COUNT, "blade index out of range");
        let mut mv = Self::zero();
        mv.set(index, GF2::ONE);
        mv
    }

    /// Basis vector e_{i+1} (0-indexed).
    #[must_use]
    pub fn basis_vector(i: usize) -> Self {
        assert!(i < Self::DIM, "vector index out of range");
        Self::basis_blade(1 << i)
    }

    /// Create from a slice of 0/1 u8 values (one per basis blade).
    #[must_use]
    pub fn from_bits(bits: &[u8]) -> Self {
        assert!(bits.len() <= Self::BASIS_COUNT);
        let mut mv = Self::zero();
        for (i, &b) in bits.iter().enumerate() {
            if b & 1 != 0 {
                let w = i / WORD_BITS;
                let b_idx = i % WORD_BITS;
                mv.words[w] |= 1u64 << b_idx;
            }
        }
        mv
    }

    /// Get the coefficient of basis blade `index`.
    #[inline]
    #[must_use]
    pub fn get(&self, index: usize) -> GF2 {
        if index >= Self::BASIS_COUNT {
            return GF2::ZERO;
        }
        let w = index / WORD_BITS;
        let b = index % WORD_BITS;
        GF2::new(((self.words[w] >> b) & 1) as u8)
    }

    /// Set the coefficient of basis blade `index`.
    #[inline]
    pub fn set(&mut self, index: usize, value: GF2) {
        assert!(index < Self::BASIS_COUNT, "blade index out of range");
        let w = index / WORD_BITS;
        let b = index % WORD_BITS;
        self.words[w] = (self.words[w] & !(1u64 << b)) | ((value.value() as u64) << b);
    }

    /// Geometric product over GF(2).
    ///
    /// For non-degenerate algebras (R=0): eₐeᵦ = eₐ⊕ᵦ (XOR blade indices).
    /// For degenerate algebras: eₐeᵦ = 0 if any shared basis vector is degenerate.
    #[must_use]
    pub fn geometric_product(&self, other: &Self) -> Self {
        let mut result = Self::zero();
        let degen_mask = Self::degenerate_mask();

        for a in self.nonzero_blades() {
            for b in other.nonzero_blades() {
                let shared = a & b;
                // If any shared generator is degenerate, product is zero.
                if shared & degen_mask != 0 {
                    continue;
                }
                let product_blade = a ^ b;
                // Toggle the coefficient (XOR in GF(2)).
                let w = product_blade / WORD_BITS;
                let bit = product_blade % WORD_BITS;
                result.words[w] ^= 1u64 << bit;
            }
        }
        result
    }

    /// Outer (wedge) product over GF(2).
    ///
    /// eₐ ∧ eᵦ = eₐeᵦ if A ∩ B = ∅, else 0.
    #[must_use]
    pub fn outer_product(&self, other: &Self) -> Self {
        let mut result = Self::zero();

        for a in self.nonzero_blades() {
            for b in other.nonzero_blades() {
                if a & b != 0 {
                    continue; // shared basis vectors → 0
                }
                let product_blade = a ^ b; // = a | b when disjoint
                let w = product_blade / WORD_BITS;
                let bit = product_blade % WORD_BITS;
                result.words[w] ^= 1u64 << bit;
            }
        }
        result
    }

    /// Inner product over GF(2) (left contraction).
    ///
    /// The left contraction a ⌋ b keeps only the terms where grade(result) = grade(b) - grade(a).
    #[must_use]
    pub fn inner_product(&self, other: &Self) -> Self {
        let mut result = Self::zero();
        let degen_mask = Self::degenerate_mask();

        for a in self.nonzero_blades() {
            let grade_a = (a as u32).count_ones();
            for b in other.nonzero_blades() {
                let grade_b = (b as u32).count_ones();
                if grade_a > grade_b {
                    continue;
                }
                // Check that a's basis vectors are a subset of b's.
                if a & b != a {
                    continue;
                }
                let shared = a & b; // = a
                if shared & degen_mask != 0 {
                    continue;
                }
                let product_blade = a ^ b;
                let grade_result = (product_blade as u32).count_ones();
                if grade_result != grade_b - grade_a {
                    continue;
                }
                let w = product_blade / WORD_BITS;
                let bit = product_blade % WORD_BITS;
                result.words[w] ^= 1u64 << bit;
            }
        }
        result
    }

    /// Grade of the highest nonzero blade.
    #[must_use]
    pub fn grade(&self) -> usize {
        let mut max_grade = 0;
        for blade in self.nonzero_blades() {
            let g = (blade as u32).count_ones() as usize;
            if g > max_grade {
                max_grade = g;
            }
        }
        max_grade
    }

    /// Grade projection: keep only blades of the given grade.
    #[must_use]
    pub fn grade_projection(&self, grade: usize) -> Self {
        let mut result = Self::zero();
        for blade in self.nonzero_blades() {
            if (blade as u32).count_ones() as usize == grade {
                let w = blade / WORD_BITS;
                let bit = blade % WORD_BITS;
                result.words[w] |= 1u64 << bit;
            }
        }
        result
    }

    /// Addition (XOR of all coefficients).
    #[must_use]
    pub fn gf2_add(&self, other: &Self) -> Self {
        let words: Vec<u64> = self
            .words
            .iter()
            .zip(other.words.iter())
            .map(|(a, b)| a ^ b)
            .collect();
        Self { words }
    }

    /// Reverse: for each blade eₐ of grade g, multiply by (-1)^(g(g-1)/2).
    /// Over GF(2), this keeps blades where g(g-1)/2 is even (grades 0,1,4,5,8,9,...)
    /// and zeros out blades where g(g-1)/2 is odd (grades 2,3,6,7,10,11,...).
    #[must_use]
    pub fn reverse(&self) -> Self {
        let mut result = Self::zero();
        for blade in self.nonzero_blades() {
            let g = (blade as u32).count_ones() as usize;
            let flip = if g >= 2 { (g * (g - 1) / 2) % 2 } else { 0 };
            if flip == 0 {
                let w = blade / WORD_BITS;
                let bit = blade % WORD_BITS;
                result.words[w] |= 1u64 << bit;
            }
        }
        result
    }

    /// Whether this is the zero multivector.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.words.iter().all(|&w| w == 0)
    }

    /// Number of nonzero coefficients.
    #[must_use]
    pub fn weight(&self) -> usize {
        self.words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Blade index iterator: yields indices of nonzero basis blades.
    pub fn nonzero_blades(&self) -> impl Iterator<Item = usize> + '_ {
        self.words.iter().enumerate().flat_map(|(wi, &word)| {
            let base = wi * WORD_BITS;
            BitIter::new(word).map(move |bit| base + bit)
        })
    }

    /// Convert to a real-valued Multivector<N, 0, R> by mapping
    /// GF(2) coefficients to f64 (0 -> 0.0, 1 -> 1.0).
    pub fn to_real(&self) -> crate::Multivector<N, 0, R> {
        let mut mv = crate::Multivector::<N, 0, R>::zero();
        for i in self.nonzero_blades() {
            mv.set(i, 1.0);
        }
        mv
    }

    /// Convert from a real-valued Multivector by reducing coefficients mod 2.
    ///
    /// Coefficients are rounded to nearest integer then taken mod 2.
    pub fn from_real(mv: &crate::Multivector<N, 0, R>) -> Self {
        let mut result = Self::zero();
        for i in 0..Self::BASIS_COUNT {
            let coeff = mv.get(i);
            let rounded = crate::math_dispatch::round_f64(coeff) as i64;
            if rounded.rem_euclid(2) == 1 {
                result.set(i, GF2::ONE);
            }
        }
        result
    }
}

// --- Operator impls ---

impl<const N: usize, const R: usize> Add for &BinaryMultivector<N, R> {
    type Output = BinaryMultivector<N, R>;
    fn add(self, rhs: Self) -> BinaryMultivector<N, R> {
        self.gf2_add(rhs)
    }
}

impl<const N: usize, const R: usize> Mul for &BinaryMultivector<N, R> {
    type Output = BinaryMultivector<N, R>;
    fn mul(self, rhs: Self) -> BinaryMultivector<N, R> {
        self.geometric_product(rhs)
    }
}

impl<const N: usize, const R: usize> fmt::Display for BinaryMultivector<N, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for blade in self.nonzero_blades() {
            if !first {
                write!(f, " + ")?;
            }
            first = false;
            if blade == 0 {
                write!(f, "1")?;
            } else {
                write!(f, "e")?;
                for bit in 0..Self::DIM {
                    if blade & (1 << bit) != 0 {
                        write!(f, "{}", bit + 1)?;
                    }
                }
            }
        }
        if first {
            write!(f, "0")?;
        }
        Ok(())
    }
}

/// Iterator over set bits of a u64.
struct BitIter {
    word: u64,
}

impl BitIter {
    fn new(word: u64) -> Self {
        Self { word }
    }
}

impl Iterator for BitIter {
    type Item = usize;
    fn next(&mut self) -> Option<usize> {
        if self.word == 0 {
            None
        } else {
            let bit = self.word.trailing_zeros() as usize;
            self.word &= self.word - 1; // clear lowest set bit
            Some(bit)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Cl3 = BinaryMultivector<3, 0>;
    type Cl21 = BinaryMultivector<2, 1>;

    #[test]
    fn test_basis_vector_square_nondegenerate() {
        // In Cl(3,0; F₂), eᵢ² = 1 for all i.
        for i in 0..3 {
            let ei = Cl3::basis_vector(i);
            let sq = ei.geometric_product(&ei);
            assert_eq!(sq, Cl3::one(), "e{}² should be 1", i + 1);
        }
    }

    #[test]
    fn test_basis_vector_square_degenerate() {
        // In Cl(2,1; F₂), e3² = 0 (degenerate).
        let e3 = Cl21::basis_vector(2);
        let sq = e3.geometric_product(&e3);
        assert!(sq.is_zero(), "degenerate e3² should be 0");

        // Non-degenerate basis vectors still square to 1.
        let e1 = Cl21::basis_vector(0);
        assert_eq!(e1.geometric_product(&e1), Cl21::one());
    }

    #[test]
    fn test_geometric_product_associativity() {
        let e1 = Cl3::basis_vector(0);
        let e2 = Cl3::basis_vector(1);
        let e3 = Cl3::basis_vector(2);

        let lhs = e1.geometric_product(&e2).geometric_product(&e3);
        let rhs = e1.geometric_product(&e2.geometric_product(&e3));
        assert_eq!(lhs, rhs, "geometric product must be associative");
    }

    #[test]
    fn test_outer_product_independent_blades() {
        let e1 = Cl3::basis_vector(0);
        let e2 = Cl3::basis_vector(1);
        let e12 = e1.outer_product(&e2);
        assert_eq!(e12, Cl3::basis_blade(0b011)); // e1 ∧ e2 = e12
    }

    #[test]
    fn test_outer_product_dependent() {
        let e1 = Cl3::basis_vector(0);
        let e1_wedge_e1 = e1.outer_product(&e1);
        assert!(e1_wedge_e1.is_zero(), "e1 ∧ e1 = 0");
    }

    #[test]
    fn test_grade_projection() {
        // 1 + e1 + e12 + e123
        let mut mv = Cl3::one();
        mv.set(0b001, GF2::ONE); // e1
        mv.set(0b011, GF2::ONE); // e12
        mv.set(0b111, GF2::ONE); // e123

        assert_eq!(mv.grade_projection(0), Cl3::one());
        assert_eq!(mv.grade_projection(1), Cl3::basis_vector(0));
        assert_eq!(mv.grade_projection(2), Cl3::basis_blade(0b011));
        assert_eq!(mv.grade_projection(3), Cl3::basis_blade(0b111));
    }

    #[test]
    fn test_reverse() {
        // Reverse keeps grades 0,1 and flips grades 2,3.
        // Over GF(2), flipping means zeroing out.
        let mut mv = Cl3::zero();
        mv.set(0, GF2::ONE); // grade 0 → kept
        mv.set(0b001, GF2::ONE); // grade 1 → kept
        mv.set(0b011, GF2::ONE); // grade 2 → dropped
        mv.set(0b111, GF2::ONE); // grade 3 → dropped

        let rev = mv.reverse();
        assert_eq!(rev.get(0), GF2::ONE); // grade 0 kept
        assert_eq!(rev.get(0b001), GF2::ONE); // grade 1 kept
        assert_eq!(rev.get(0b011), GF2::ZERO); // grade 2 dropped
        assert_eq!(rev.get(0b111), GF2::ZERO); // grade 3 dropped
    }

    #[test]
    fn test_to_real_from_real_roundtrip() {
        let mut mv = Cl3::zero();
        mv.set(0, GF2::ONE); // scalar 1
        mv.set(0b001, GF2::ONE); // e1
        mv.set(0b111, GF2::ONE); // e123

        let real = mv.to_real();
        assert!((real.get(0) - 1.0).abs() < 1e-14);
        assert!((real.get(0b001) - 1.0).abs() < 1e-14);
        assert!((real.get(0b111) - 1.0).abs() < 1e-14);
        assert!((real.get(0b010)).abs() < 1e-14);

        let back = Cl3::from_real(&real);
        assert_eq!(back, mv);
    }

    #[test]
    fn test_zero_and_identity() {
        assert!(Cl3::zero().is_zero());
        assert!(!Cl3::one().is_zero());
        assert_eq!(Cl3::one().weight(), 1);
    }

    #[test]
    fn test_pseudoscalar_product() {
        // e1 * e2 * e3 = e123
        let e1 = Cl3::basis_vector(0);
        let e2 = Cl3::basis_vector(1);
        let e3 = Cl3::basis_vector(2);
        let ps = e1.geometric_product(&e2).geometric_product(&e3);
        assert_eq!(ps, Cl3::basis_blade(0b111));
    }

    #[test]
    fn test_commutativity_over_gf2() {
        // Over GF(2), eᵢeⱼ = -eⱼeᵢ = eⱼeᵢ for i ≠ j (since -1 = 1).
        let e1 = Cl3::basis_vector(0);
        let e2 = Cl3::basis_vector(1);
        assert_eq!(
            e1.geometric_product(&e2),
            e2.geometric_product(&e1),
            "antisymmetric part commutes over GF(2)"
        );
    }
}
