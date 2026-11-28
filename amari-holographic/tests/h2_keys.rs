use amari_holographic::{
    memory::MemoryElement, unbind_general, BindingAlgebra, Cl3, CliffordAlgebra as C,
    FHRRAlgebra as F, MAPAlgebra as M, ProductCliffordAlgebra as P, TypedKey,
};
#[test]
fn typed_admission_is_not_general_invertibility() {
    let m = M::new([2., -3.]);
    assert!(TypedKey::try_new(m.clone()).is_err());
    let key = TypedKey::try_new(M::new([1., -1.])).unwrap();
    assert_eq!(
        key.unbind_key(&key.bind_key(&m).unwrap())
            .unwrap()
            .components(),
        m.components()
    );
    assert_eq!(m.correlation(&m).components(), &[4., 9.]);
    assert!(TypedKey::try_new(F::new([0.], [2.])).is_err());
    let p = F::new([3.], [4.]).normalize_unitary().unwrap();
    assert_eq!(p.full_coefficients(), vec![0.6, 0.8]);
    let k = TypedKey::try_new(p).unwrap();
    let v = F::new([1.], [2.]);
    let r = k.unbind_key(&k.bind_key(&v).unwrap()).unwrap();
    for (x, y) in r.full_coefficients().iter().zip(v.full_coefficients()) {
        assert!((x - y).abs() < 1e-12);
    }
    assert!(unbind_general(&F::new([f64::from_bits(1)], [0.]), &v).is_err());
    assert!(unbind_general(&M::new([0., 1.]), &m).is_err());
}
#[test]
fn singular_and_mixed_clifford_keys() {
    let e = Cl3::unit_vector(0);
    let v = P::new([Cl3::unit_vector(1)]);
    assert!(TypedKey::try_new(P::new([Cl3::one() + e])).is_err());
    let k = TypedKey::try_new(P::new([Cl3::scalar(2.) + e])).unwrap();
    assert_eq!(
        k.unbind_key(&k.bind_key(&v).unwrap())
            .unwrap()
            .full_coefficients(),
        v.full_coefficients()
    );
}
#[test]
fn sampler_reports_domain() {
    assert!(C::<0, 0, 1>::random_versor(1).is_err());
    assert_eq!(
        C::<0, 0, 1>::random_versor(0).unwrap().to_coefficients(),
        vec![1., 0.]
    );
    let a = C::<1, 1, 0>::unit_vector(0).unwrap();
    let b = C::<1, 1, 0>::unit_vector(1).unwrap();
    assert!(TypedKey::try_new(a.superpose(&b).unwrap()).is_err());
    fastrand::seed(17);
    for _ in 0..8 {
        let x = C::<3, 0, 0>::random_versor(2).unwrap();
        assert!(TypedKey::try_new(x).is_ok());
    }
}
#[test]
fn dense_complex_convolution() {
    // Forward DFT uses the negative exponent. Compare binding to direct two-sample circular convolution.
    let x = F::new([3., -1.], [1., -3.]); // DFT of [1-i, 2+2i]
    let key = F::new([3., 1.], [0., 0.]); // DFT of [2,1]
    let bound = key.bind(&x);
    let c = bound.full_coefficients();
    assert_eq!(
        [
            (c[0] + c[2]) / 2.,
            (c[1] + c[3]) / 2.,
            (c[0] - c[2]) / 2.,
            (c[1] - c[3]) / 2.
        ],
        [4., 0., 5., 3.]
    );
    assert_eq!(
        unbind_general(&key, &bound).unwrap().full_coefficients(),
        x.full_coefficients()
    );
    assert_ne!(
        key.correlation(&bound).full_coefficients(),
        x.full_coefficients()
    );
}
#[test]
fn general_key_does_not_rebind() {
    let k = M::new([2., -3.]);
    let v = M::new([3., -4.]);
    assert_eq!(
        k.unbind(&k.bind(&v)).unwrap().full_coefficients(),
        v.full_coefficients()
    );
}
#[test]
fn nonunit_phases_are_not_inverted_by_correlation() {
    let k = F::new([2., 0.], [0., 3.]);
    let v = F::new([0., 1.], [2., 0.]);
    assert_eq!(
        k.unbind(&k.bind(&v)).unwrap().full_coefficients(),
        v.full_coefficients()
    );
}
#[test]
fn projection_rejects_a_zero_bin() {
    assert!(F::new([0., 3.], [0., 4.]).normalize_unitary().is_err());
}
