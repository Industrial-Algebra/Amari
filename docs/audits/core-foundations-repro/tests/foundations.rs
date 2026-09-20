// Copyright (C) 2026 Industrial Algebra
// SPDX-License-Identifier: Apache-2.0

//! Independent law checks, NOT assertions that buggy behavior should persist.
//! At baseline 165d7cc, `oracle_` tests pass and `finding_` tests fail.

use amari_core::{Bivector, CayleyTable, Multivector, Rotor, Vector};

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-10,
        "actual={actual:.17e}, expected={expected:.17e}"
    );
}

fn coefficients<const P: usize, const Q: usize, const R: usize>(
    actual: &Multivector<P, Q, R>,
    expected: &Multivector<P, Q, R>,
) {
    for (i, (&a, &b)) in actual
        .as_slice()
        .iter()
        .zip(expected.as_slice())
        .enumerate()
    {
        assert!((a - b).abs() < 1e-10, "blade {i}: actual={a}, expected={b}");
    }
}

fn blade<const P: usize, const Q: usize, const R: usize>(index: usize) -> Multivector<P, Q, R> {
    let mut mv = Multivector::zero();
    mv.set(index, 1.0);
    mv
}

// Independent oracle: concatenate basis words, sort via adjacent swaps,
// cancel repeated generators with their metric, then rebuild the blade.
// No XOR product or implementation's bit-pair sign formula is used.
fn word_product(a: usize, b: usize, p: usize, q: usize, r: usize) -> (f64, usize) {
    let n = p + q + r;
    let mut word = Vec::new();
    for mask in [a, b] {
        for i in 0..n {
            if mask & (1 << i) != 0 {
                word.push(i);
            }
        }
    }
    let mut sign = 1.0;
    for end in (1..word.len()).rev() {
        for i in 0..end {
            if word[i] > word[i + 1] {
                word.swap(i, i + 1);
                sign = -sign;
            }
        }
    }
    let mut index = 0;
    let mut i = 0;
    while i < word.len() {
        if i + 1 < word.len() && word[i] == word[i + 1] {
            sign *= if word[i] < p {
                1.0
            } else if word[i] < p + q {
                -1.0
            } else {
                0.0
            };
            i += 2;
        } else {
            index += 1 << word[i];
            i += 1;
        }
    }
    (sign, index)
}

fn check_signature<const P: usize, const Q: usize, const R: usize>() {
    let n = P + Q + R;
    let size = 1 << n;
    let table = CayleyTable::<P, Q, R>::new();
    for a in 0..size {
        let x = blade::<P, Q, R>(a);
        let k = a.count_ones() as usize;
        for b in 0..size {
            let y = blade::<P, Q, R>(b);
            let l = b.count_ones() as usize;
            let (sign, out) = word_product(a, b, P, Q, R);
            assert_eq!(
                table.get_product(a, b),
                (sign, out),
                "Cl({P},{Q},{R}), {a}*{b}"
            );
            let product = blade::<P, Q, R>(out) * sign;
            coefficients(&x.geometric_product(&y), &product);
            let grade = out.count_ones() as usize;
            coefficients(
                &x.outer_product(&y),
                &(product.clone() * if grade == k + l { 1.0 } else { 0.0 }),
            );
            // This validates the implemented grade-difference convention,
            // including scalars; it does not label it the Hestenes convention.
            coefficients(
                &x.inner_product(&y),
                &(product.clone() * if grade == k.abs_diff(l) { 1.0 } else { 0.0 }),
            );
            coefficients(
                &x.left_contraction(&y),
                &(product.clone() * if l >= k && grade == l - k { 1.0 } else { 0.0 }),
            );
            coefficients(
                &x.right_contraction(&y),
                &(product * if k >= l && grade == k - l { 1.0 } else { 0.0 }),
            );
            coefficients(
                &x.geometric_product(&y).reverse(),
                &y.reverse().geometric_product(&x.reverse()),
            );
        }
        if R == 0 {
            let double_star_sign = if (k * (n - k) + Q) % 2 == 0 {
                1.0
            } else {
                -1.0
            };
            coefficients(&x.hodge_dual().hodge_dual(), &(x * double_star_sign));
        }
    }
}

#[test]
fn oracle_all_basis_pairs_through_dimension_four() {
    macro_rules! signatures {
        ($(($p:literal, $q:literal, $r:literal)),* $(,)?) => { $(check_signature::<$p, $q, $r>();)* };
    }
    signatures!(
        (0, 0, 0),
        (1, 0, 0),
        (0, 1, 0),
        (0, 0, 1),
        (2, 0, 0),
        (1, 1, 0),
        (1, 0, 1),
        (0, 2, 0),
        (0, 1, 1),
        (0, 0, 2),
        (3, 0, 0),
        (2, 1, 0),
        (2, 0, 1),
        (1, 2, 0),
        (1, 1, 1),
        (1, 0, 2),
        (0, 3, 0),
        (0, 2, 1),
        (0, 1, 2),
        (0, 0, 3),
        (4, 0, 0),
        (3, 1, 0),
        (3, 0, 1),
        (2, 2, 0),
        (2, 1, 1),
        (2, 0, 2),
        (1, 3, 0),
        (1, 2, 1),
        (1, 1, 2),
        (1, 0, 3),
        (0, 4, 0),
        (0, 3, 1),
        (0, 2, 2),
        (0, 1, 3),
        (0, 0, 4),
    );
}

#[test]
fn finding_01_inverse_of_invertible_mixed_grade_element() {
    type M = Multivector<3, 0, 0>;
    let a = M::scalar(2.0) + M::basis_vector(0);
    let expected = (M::scalar(2.0) - M::basis_vector(0)) * (1.0 / 3.0);
    // Check the analytic reference independently before evaluating inverse().
    coefficients(&a.geometric_product(&expected), &M::scalar(1.0));
    coefficients(&expected.geometric_product(&a), &M::scalar(1.0));
    coefficients(&a.geometric_product(&a.inverse().unwrap()), &M::scalar(1.0));
}

#[test]
fn finding_01_inverse_rejects_zero_divisor() {
    type M = Multivector<3, 0, 0>;
    let a = M::scalar(1.0) + M::basis_vector(0);
    let annihilator = M::scalar(1.0) - M::basis_vector(0);
    coefficients(&a.geometric_product(&annihilator), &M::zero());
    assert!(
        a.inverse().is_none(),
        "nonzero annihilator proves no inverse exists"
    );
}

#[test]
fn finding_02_geometric_product_preserves_small_coefficients() {
    type M = Multivector<3, 0, 0>;
    close(
        M::scalar(1e-15)
            .geometric_product(&M::scalar(1e15))
            .scalar_part(),
        1.0,
    );
}

#[test]
fn finding_02_geometric_product_associativity_across_scales() {
    type M = Multivector<3, 0, 0>;
    let a = M::scalar(1e-8);
    let b = M::scalar(1e-8);
    let c = M::scalar(1e16);
    coefficients(
        &a.geometric_product(&b).geometric_product(&c),
        &a.geometric_product(&b.geometric_product(&c)),
    );
}

#[test]
fn finding_03_exponential_of_nilpotent_bivector() {
    type M = Multivector<1, 0, 1>;
    let b = blade::<1, 0, 1>(3);
    coefficients(&b.geometric_product(&b), &M::zero());
    coefficients(&b.exp(), &(M::scalar(1.0) + b));
}

#[test]
fn finding_04_exponential_of_nonsimple_bivector() {
    type M = Multivector<4, 0, 0>;
    let b = blade::<4, 0, 0>(3) + blade::<4, 0, 0>(12);
    // e12 and e34 commute; exp(e12 + e34) = exp(e12)exp(e34).
    let c = 1.0_f64.cos();
    let s = 1.0_f64.sin();
    let expected = M::scalar(c * c) + b.clone() * (s * c) + blade::<4, 0, 0>(15) * (s * s);
    coefficients(&b.exp(), &expected);
}

#[test]
fn finding_05_exponential_of_null_vector_is_not_identity() {
    type M = Multivector<1, 1, 0>;
    let v = M::basis_vector(0) + M::basis_vector(1);
    coefficients(&v.geometric_product(&v), &M::zero());
    coefficients(&v.exp(), &(M::scalar(1.0) + v));
}

#[test]
fn finding_06_scalar_exponential_converges() {
    type M = Multivector<3, 0, 0>;
    let actual = M::scalar(20.0).exp().scalar_part();
    let expected = 20.0_f64.exp();
    assert!(
        (actual / expected - 1.0).abs() < 1e-10,
        "actual={actual}, expected={expected}"
    );
}

#[test]
fn finding_07_approximate_equality_distinguishes_null_vector_from_zero() {
    type M = Multivector<1, 1, 0>;
    let v = M::basis_vector(0) + M::basis_vector(1);
    assert!(
        !v.approx_eq(&M::zero(), 1e-12),
        "nonzero null vector is not the zero multivector"
    );
}

#[test]
fn finding_08_degenerate_hodge_matches_documented_formal_complement() {
    let e_null = Multivector::<2, 0, 1>::basis_vector(2);
    // Documented permutation-only complement of e3 is e12, not zero.
    coefficients(&e_null.hodge_dual(), &blade::<2, 0, 1>(3));
}

#[test]
fn finding_09_vector_dual_preserves_declared_wrapper_grade() {
    let dual: Bivector<4, 0, 0> = Vector::<4, 0, 0>::e1().hodge_dual();
    coefficients(&dual.mv, &dual.mv.grade_projection(2));
}

#[test]
fn finding_10_slerp_supports_two_dimensional_rotors() {
    let r = Rotor::<2, 0, 0>::identity();
    coefficients(r.slerp(&r, 0.5).as_multivector(), r.as_multivector());
}

#[test]
fn finding_10_slerp_preserves_four_dimensional_endpoint() {
    let b = blade::<4, 0, 0>(12);
    let r = Rotor::from_multivector_bivector(&b, 1.0);
    coefficients(
        Rotor::identity().slerp(&r, 1.0).as_multivector(),
        r.as_multivector(),
    );
}

#[test]
fn finding_11_boost_power_one_preserves_rotor() {
    let r = Rotor::<1, 1, 0>::from_bivector(&Bivector::e12(), 1.0);
    coefficients(r.power(1.0).as_multivector(), r.as_multivector());
}

#[test]
fn finding_11_small_rotation_power_one_preserves_rotor() {
    let r = Rotor::<3, 0, 0>::from_bivector(&Bivector::e12(), 1e-8);
    coefficients(r.power(1.0).as_multivector(), r.as_multivector());
}

#[test]
fn finding_11_negative_identity_power_one_preserves_double_cover() {
    let r = Rotor::<3, 0, 0>::from_bivector(&Bivector::e12(), 2.0 * std::f64::consts::PI);
    coefficients(r.power(1.0).as_multivector(), r.as_multivector());
}

#[test]
fn finding_12_rotor_from_negative_unit_vector_to_itself() {
    let v = Vector::<0, 2, 0>::e1();
    let r = Rotor::from_vectors(&v, &v).expect("identity maps any vector to itself");
    coefficients(&r.apply(&v.mv), &v.mv);
}

#[test]
fn finding_12_rotor_between_negative_unit_vectors_maps_direction() {
    let a = Vector::<0, 2, 0>::e1();
    let b = Vector::<0, 2, 0>::e2();
    let r = Rotor::from_vectors(&a, &b).unwrap();
    coefficients(&r.apply(&a.mv), &b.mv);
}

#[test]
fn finding_13_reflection_in_hyperplane_negates_normal() {
    let n = Multivector::<3, 0, 0>::basis_vector(0);
    coefficients(&amari_core::rotor::reflect(&n, &n), &(-n));
}

#[test]
fn finding_14_raw_rotor_constructor_preserves_even_grade() {
    let r = Rotor::from_multivector_bivector(&Multivector::<3, 0, 0>::basis_vector(0), 1.0);
    coefficients(
        &r.as_multivector().grade_projection(1),
        &Multivector::zero(),
    );
}

#[test]
fn finding_15_bivector_setter_handles_all_four_dimensional_components() {
    let mut b = Multivector::<4, 0, 0>::zero();
    b.set_bivector_component(3, 1.0);
    // In four dimensions there are six independent components: valid index 3
    // must not disappear regardless of the chosen component ordering.
    close(b.as_slice().iter().map(|x| x.abs()).sum(), 1.0);
}
