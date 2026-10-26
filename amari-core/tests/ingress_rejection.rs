// SPDX-License-Identifier: MIT OR Apache-2.0
//! C2 dynamic rejection suite (contract #350, S3) — every documented
//! runtime `Err` of the verified-core ingress surface, recorded
//! SEPARATELY from the static (compile-fail) rejections. The unit-norm
//! band edges pin the documented `|norm - 1| <= 1e-4` acceptance
//! tolerance at both boundaries.

use amari_core::verified::{Vector, VerifiedMultivector, VerifiedRotor};

fn err_of<T>(r: Result<T, &'static str>) -> &'static str {
    match r {
        Ok(_) => panic!("expected Err, got Ok"),
        Err(e) => e,
    }
}

#[test]
fn multivector_new_rejects_wrong_length() {
    assert_eq!(
        err_of(VerifiedMultivector::<f64, 3, 0, 0>::new(vec![0.0; 7])),
        "Coefficient array size must equal 2^(P+Q+R)"
    );
}

#[test]
fn multivector_new_rejects_nonfinite() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            err_of(VerifiedMultivector::<f64, 3, 0, 0>::new(vec![bad; 8])),
            "All coefficients must be finite (no NaN or infinity)"
        );
    }
}

#[test]
fn scalar_ingress_rejects_nonfinite() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            err_of(VerifiedMultivector::<f64, 3, 0, 0>::scalar(bad)),
            "Scalar coefficient must be finite (no NaN or infinity)"
        );
    }
}

#[test]
fn basis_vector_rejects_out_of_range() {
    assert_eq!(
        err_of(VerifiedMultivector::<f64, 3, 0, 0>::basis_vector(3)),
        "Basis vector index exceeds dimension"
    );
}

#[test]
fn rotor_nonfinite_check_is_defensive_depth() {
    // The rotor's nonfinite gate cannot be reached through public
    // ingress: VerifiedMultivector::new rejects nonfinite coefficients
    // first (verified above). The check is defensive depth — recorded
    // as an invariant, with the blocking demonstrated:
    let mut coeffs = vec![0.0f64; 8];
    coeffs[0] = f64::NAN;
    assert_eq!(
        err_of(VerifiedMultivector::<f64, 3, 0, 0>::new(coeffs)),
        "All coefficients must be finite (no NaN or infinity)"
    );
    // VerifiedRotor::new's own string exists for interior/unsafe
    // callers; its Err branch is unreachable through the public API.
}

#[test]
fn rotor_new_rejects_odd_grade() {
    // e1: grade-1 component — odd grade.
    let mv = VerifiedMultivector::<f64, 3, 0, 0>::basis_vector(0).unwrap();
    assert_eq!(
        err_of(VerifiedRotor::<f64, 3, 0, 0>::new(mv)),
        "Rotor must have even grade"
    );
}

#[test]
fn rotor_unit_norm_band_edges() {
    // e1e2 bivector scaled by t: norm = |t|; accepted iff |t| within
    // 1e-4 of 1. Pin BOTH documented band edges (inclusive per the
    // documented `<=`).
    let mk = |t: f64| {
        let mut coeffs = vec![0.0f64; 8];
        coeffs[0b0011] = t; // e1^e2 blade
        VerifiedMultivector::<f64, 3, 0, 0>::new(coeffs).unwrap()
    };
    // Inside/edge: accepted.
    for t in [1.0, 1.0 + 1e-4, 1.0 - 1e-4, -1.0, -(1.0 - 1e-4)] {
        assert!(
            VerifiedRotor::<f64, 3, 0, 0>::new(mk(t)).is_ok(),
            "t={t} must be accepted (inside band)"
        );
    }
    // Outside: rejected with the documented string.
    for t in [1.0 + 2e-4, 1.0 - 2e-4, -(1.0 + 2e-4), 0.5, 2.0] {
        assert_eq!(
            err_of(VerifiedRotor::<f64, 3, 0, 0>::new(mk(t))),
            "Rotor must have unit norm"
        );
    }
}

#[test]
fn compose_reports_degenerate_instead_of_dividing_by_zero() {
    // In Cl(1,1,0) the even elements 1+e1e2 and 1-e1e2 are null: their
    // geometric product is exactly zero. Both scale to unit norm, so
    // both pass rotor ingress — their composition must be reported
    // degenerate, never a divide-by-zero NaN leak.
    let s = std::f64::consts::FRAC_1_SQRT_2;
    let mut a = vec![0.0f64; 4]; // 2^(1+1+0) basis blades
    a[0] = s;
    a[0b11] = s; // (1 + e1e2)/sqrt(2)
    let mut b = vec![0.0f64; 4];
    b[0] = s;
    b[0b11] = -s; // (1 - e1e2)/sqrt(2)
    let r1 =
        VerifiedRotor::<f64, 1, 1, 0>::new(VerifiedMultivector::<f64, 1, 1, 0>::new(a).unwrap())
            .unwrap();
    let r2 =
        VerifiedRotor::<f64, 1, 1, 0>::new(VerifiedMultivector::<f64, 1, 1, 0>::new(b).unwrap())
            .unwrap();
    assert_eq!(err_of(r1.compose(&r2)), "composition is degenerate");
    assert_eq!(err_of(r2.compose(&r1)), "composition is degenerate");
}

#[test]
fn vector_new_rejects_wrong_length() {
    assert_eq!(
        err_of(Vector::<f64, 2>::new(vec![1.0, 0.0, 0.0])),
        "Vector data length must match dimension"
    );
}

#[test]
fn vector_new_rejects_nonfinite() {
    for bad in [f64::NAN, f64::INFINITY] {
        assert_eq!(
            err_of(Vector::<f64, 2>::new(vec![1.0, bad])),
            "All vector components must be finite (no NaN or infinity)"
        );
    }
}

#[test]
fn verified_self_dot_rejects_nonfinite() {
    // Ingress closure blocks nonfinite components, so verified_self_dot's
    // documented Err is defensive depth (interior callers); the public
    // surface is exercised on the Ok path and the ingress block:
    let v = Vector::<f64, 2>::new(vec![1.0, 2.0]).unwrap();
    assert!(v.verified_self_dot().is_ok());
    assert_eq!(
        err_of(Vector::<f64, 2>::new(vec![f64::NAN, 0.0])),
        "All vector components must be finite (no NaN or infinity)"
    );
}
