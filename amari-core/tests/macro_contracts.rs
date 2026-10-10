// Copyright (C) 2026 Industrial Algebra
// SPDX-License-Identifier: Apache-2.0
//! Downstream-style coverage for exported macros (W02 review round 1).

use amari_core::{unit, Multivector};

#[test]
fn unit_macro_normalizes_multivector_operands() {
    let a = Multivector::<3, 0, 0>::basis_vector(0) * 4.0;
    let u = unit!(a);
    assert!((u.coefficient_norm() - 1.0).abs() < 1e-12);
    let zero = Multivector::<3, 0, 0>::zero();
    assert!(unit!(zero).is_zero());
}

#[test]
fn unit_macro_handles_subnormal_operands() {
    // 1.0 / 1e-320 overflows; division-based scaling must not.
    let a = Multivector::<1, 0, 0>::scalar(1e-320);
    let u = unit!(a);
    assert_eq!(u.as_slice(), Multivector::<1, 0, 0>::scalar(1.0).as_slice());
}
