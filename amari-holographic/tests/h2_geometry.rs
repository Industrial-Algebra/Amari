use amari_holographic::{
    AlgebraError, BindingAlgebra, Cl3, CliffordAlgebra as C, FHRRAlgebra as F, MAPAlgebra as M,
    ProductCliffordAlgebra as P,
};
fn relative(x: f64, y: f64) {
    assert!((x / y - 1.).abs() <= 1e-12, "{x} vs {y}");
}
#[test]
fn u2_small_nonzero_norm() {
    relative(Cl3::scalar(1e-200).norm().unwrap(), 1e-200);
}
fn check<A: BindingAlgebra>(a: A, b: A, s: f64) {
    relative(a.norm().unwrap(), 5. * s);
    assert_eq!(a.similarity(&a).unwrap().to_bits(), 1f64.to_bits());
    relative(a.similarity(&b).unwrap(), -1.);
    let u = a.normalize().unwrap();
    relative(u.norm().unwrap(), 1.);
    for (x, y) in u
        .geometry_coefficients()
        .into_iter()
        .zip(a.geometry_coefficients())
    {
        if y == 0. {
            assert!(x.abs() <= 1e-12)
        } else {
            relative(x, (y / s) / 5.);
        }
    }
}
#[test]
fn scales_and_directions() {
    for s in [1e-200, 1e200] {
        let a = Cl3::vector(3. * s, 4. * s, 0.);
        check(a, a.scale(-1.), s);
        check(P::new([a]), P::new([a.scale(-1.)]), s);
        check(M::new([3. * s, 4. * s]), M::new([-3. * s, -4. * s]), s);
        check(F::new([3. * s], [4. * s]), F::new([-3. * s], [-4. * s]), s);
        check(
            C::<1, 0, 0>::from_coefficients(&[3. * s, 4. * s]).unwrap(),
            C::from_coefficients(&[-3. * s, -4. * s]).unwrap(),
            s,
        );
    }
}
#[test]
fn domains() {
    for x in [0., f64::NAN, f64::INFINITY] {
        let v = F::new([0.], [x]);
        assert!(v.similarity(&v).is_err());
    }
    assert!(Cl3::scalar(f64::from_bits(1)).norm().is_err());
    let huge = M::new([f64::MAX; 2]);
    assert!(huge.norm().is_err());
    relative(huge.normalize().unwrap().components()[0], 0.5f64.sqrt());
    let a = C::<0, 1, 0>::unit_vector(0).unwrap();
    assert_eq!(
        a.coefficient_cosine(&a),
        Err(AlgebraError::UnsupportedSignature)
    );
    assert_eq!(a.normalized_metric_pairing(&a).unwrap(), -1.);
    let n = C::<0, 0, 1>::unit_vector(0).unwrap();
    assert_eq!(n.similarity(&n), Err(AlgebraError::UnsupportedSignature));
    assert_eq!(n.normalized_metric_pairing(&n).unwrap(), 0.);
    assert_eq!(
        Cl3::unit_vector(0)
            .similarity(&Cl3::unit_vector(1))
            .unwrap(),
        0.
    );
}
