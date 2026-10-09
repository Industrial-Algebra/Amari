// Copyright (C) 2026 Industrial Algebra
// SPDX-License-Identifier: Apache-2.0

use amari_core::Multivector;

type E3 = Multivector<3, 0, 0>;
const LARGE: f64 = (1_u64 << 50) as f64;
const SMALL: f64 = 1.0 / LARGE;

fn blade<const P: usize, const Q: usize, const R: usize>(
    index: usize,
    coefficient: f64,
) -> Multivector<P, Q, R> {
    let mut value = Multivector::zero();
    value.set(index, coefficient);
    value
}

#[test]
fn gp_small_finite_factor_is_not_zero() {
    let result = E3::scalar(SMALL).geometric_product(&E3::scalar(LARGE));
    assert_eq!(result.as_slice(), E3::scalar(1.0).as_slice());
    let vector = E3::basis_vector(0) * SMALL;
    assert_eq!(
        E3::scalar(1.0).geometric_product(&vector).as_slice(),
        vector.as_slice(),
    );
}

#[test]
fn gp_associativity_does_not_drop_a_small_intermediate() {
    let scale = (1_u64 << 25) as f64;
    let a = E3::scalar(1.0 / scale);
    let b = E3::scalar(1.0 / scale);
    let c = E3::scalar(LARGE);
    let left = a.geometric_product(&b).geometric_product(&c);
    let right = a.geometric_product(&b.geometric_product(&c));
    assert_eq!(left.as_slice(), E3::scalar(1.0).as_slice());
    assert_eq!(right.as_slice(), E3::scalar(1.0).as_slice());
}

#[test]
fn projected_outer_product_keeps_small_grade_components() {
    let a = E3::basis_vector(0) * SMALL;
    let b = E3::basis_vector(1) * LARGE;
    assert_eq!(
        a.outer_product(&b).as_slice(),
        blade::<3, 0, 0>(3, 1.0).as_slice()
    );
    type Pga = Multivector<1, 0, 1>;
    let a = Pga::basis_vector(0) * SMALL;
    let b = Pga::basis_vector(1) * LARGE;
    assert_eq!(
        a.outer_product(&b).as_slice(),
        blade::<1, 0, 1>(3, 1.0).as_slice()
    );
}

#[test]
fn projected_inner_and_contractions_keep_small_grade_components() {
    let a = E3::basis_vector(0) * SMALL;
    let same = E3::basis_vector(0) * LARGE;
    assert_eq!(
        a.inner_product(&same).as_slice(),
        E3::scalar(1.0).as_slice()
    );
    let b = blade::<3, 0, 0>(3, LARGE);
    assert_eq!(
        a.left_contraction(&b).as_slice(),
        E3::basis_vector(1).as_slice()
    );
    assert_eq!(
        b.right_contraction(&a).as_slice(),
        (-E3::basis_vector(1)).as_slice()
    );
    type Mink = Multivector<1, 1, 0>;
    let negative = Mink::basis_vector(1) * SMALL;
    let scaled_negative = Mink::basis_vector(1) * LARGE;
    assert_eq!(negative.inner_product(&scaled_negative).scalar_part(), -1.0);
}

#[test]
fn structural_hodge_transport_is_linear_at_small_scale() {
    let small = E3::basis_vector(0) * SMALL;
    assert_eq!(
        small.hodge_dual().as_slice(),
        blade::<3, 0, 0>(6, SMALL).as_slice()
    );
}

#[test]
fn structural_grade_reports_a_small_nonzero_component() {
    let value = E3::scalar(2.0) + E3::basis_vector(0) * SMALL;
    assert_eq!(value.grade(), 1);
    assert_eq!(E3::zero().grade(), 0);
}
