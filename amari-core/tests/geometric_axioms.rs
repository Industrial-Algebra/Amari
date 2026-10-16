//! Geometric-product axiom instance tests for the public `amari-core` API.
//!
//! Every test asserts a concrete algebraic instance on sampled multivectors of
//! `Cl(3,0,0)` (or the listed signature). They are **executed-instance**
//! checks, not universally quantified proofs; the CORE-D07 matrix tracks
//! actual proof obligations separately (C3/C4). See
//! `docs/technical/MATHEMATICAL_RIGOR.md`.
//!
//! The module name `geometric_axioms` is the advertised pre-commit/CI named
//! suite, so the approved invocation
//! `cargo test --package amari-core geometric_axioms` selects these tests.
//! The corresponding vacuity assertion lives in `scripts/verify-named-suite.sh`.

use amari_core::{Multivector, Scalar, Vector};
use approx::assert_relative_eq;

mod geometric_axioms {
    use super::*;

    /// The geometric product is associative on sampled multivectors:
    /// `(a b) c = a (b c)`.
    #[test]
    fn associativity() {
        let a = Multivector::<3, 0, 0>::from_slice(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        let b = Multivector::<3, 0, 0>::from_slice(&[8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0]);
        let c = Multivector::<3, 0, 0>::from_slice(&[1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0]);

        let left = a.geometric_product(&b).geometric_product(&c);
        let right = a.geometric_product(&b.geometric_product(&c));

        for i in 0..8 {
            assert_relative_eq!(left.as_slice()[i], right.as_slice()[i], epsilon = 1e-10);
        }
    }

    /// The geometric product distributes over addition on sampled vectors:
    /// `a (b + c) = a b + a c`.
    #[test]
    fn distributivity() {
        let a = Vector::<3, 0, 0>::from_components(1.0, 2.0, 3.0);
        let b = Vector::<3, 0, 0>::from_components(4.0, 5.0, 6.0);
        let c = Vector::<3, 0, 0>::from_components(7.0, 8.0, 9.0);

        let left = a.geometric_product(&b.add(&c));
        let right = a.geometric_product(&b).add(&a.geometric_product(&c));

        assert_relative_eq!(left.as_slice(), right.as_slice(), epsilon = 1e-10);
    }

    /// The unit scalar is a two-sided multiplicative identity:
    /// `1 a = a 1 = a`.
    #[test]
    fn multiplicative_identity() {
        let one = Scalar::<3, 0, 0>::one();
        let a = Multivector::<3, 0, 0>::from_slice(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);

        assert_relative_eq!(one.geometric_product(&a).as_slice(), a.as_slice());
        assert_relative_eq!(
            a.geometric_product(one.as_multivector()).as_slice(),
            a.as_slice()
        );
    }

    /// The additive zero is a two-sided additive identity: `a + 0 = a`.
    #[test]
    fn additive_identity() {
        let a = Multivector::<3, 0, 0>::from_slice(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        let zero = Multivector::<3, 0, 0>::zero();

        assert_relative_eq!(a.add(&zero).as_slice(), a.as_slice());
        assert_relative_eq!(zero.add(&a).as_slice(), a.as_slice());
    }

    /// Scalar multiplication commutes with the geometric product:
    /// `(s a) = (a s)` for a scalar `s` and vector `a`.
    #[test]
    fn scalar_commutes() {
        let scalar = Scalar::<3, 0, 0>::from(2.5);
        let a = Vector::<3, 0, 0>::from_components(1.0, 2.0, 3.0);

        let scalar_first = scalar.geometric_product_with_vector(&a);
        let scalar_last = a.geometric_product_with_scalar(&scalar);

        assert_relative_eq!(scalar_first.as_slice(), scalar_last.as_slice());
    }

    /// Basis-vector squares follow the metric signature: `e_i^2 = +1` for a
    /// positive coordinate, `-1` for a negative coordinate, and `0` for a
    /// null coordinate.
    #[test]
    fn basis_vector_signature() {
        let e_pos = Vector::<3, 0, 0>::e1().expect("Cl(3,0,0) has basis vector e1");
        assert_relative_eq!(
            e_pos.geometric_product(&e_pos).scalar_part(),
            1.0,
            epsilon = 1e-12
        );

        let e_neg = Vector::<0, 1, 0>::e1().expect("Cl(0,1,0) has basis vector e1");
        assert_relative_eq!(
            e_neg.geometric_product(&e_neg).scalar_part(),
            -1.0,
            epsilon = 1e-12
        );

        let e_null = Vector::<0, 0, 1>::e1().expect("Cl(0,0,1) has basis vector e1");
        assert_relative_eq!(
            e_null.geometric_product(&e_null).scalar_part(),
            0.0,
            epsilon = 1e-12
        );
    }
}
