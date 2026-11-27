use amari_holographic::{
    AlgebraError, BindingAlgebra, Cl3, FHRRAlgebra as F, MAPAlgebra as M,
    ProductCliffordAlgebra as P,
};
#[test]
fn empty_construction() {
    assert!(matches!(
        F::<0>::try_new([], []),
        Err(AlgebraError::EmptyDimension)
    ));
    assert!(matches!(
        M::<0>::try_new([]),
        Err(AlgebraError::EmptyDimension)
    ));
    assert!(matches!(
        P::<0>::try_new([]),
        Err(AlgebraError::EmptyDimension)
    ));
    assert!(matches!(
        F::<2>::from_vecs(vec![1.; 2], vec![0.; 3]),
        Err(AlgebraError::InvalidShape)
    ));
}
#[test]
fn bin_orientation_and_extreme_shifts() {
    let f = F::new([1., 2., 3., 4.], [5., 6., 7., 8.]);
    for s in [0, 1, -1, i32::MIN, i32::MAX] {
        let g = f.rotate_bins(s);
        for i in 0..4 {
            let j = (i64::from(s) + i as i64).rem_euclid(4) as usize;
            assert_eq!(g.real(j), f.real(i));
            assert_eq!(g.imag(j), f.imag(i));
        }
        assert_eq!(
            g.geometry_coefficients(),
            f.permute(s).geometry_coefficients()
        );
    }
    assert_eq!(F::<1>::identity().permute(i32::MIN).real(0), Some(1.));
    assert_eq!(
        F::<4>::identity().rotate_bins(1).to_coefficients(),
        vec![1.; 4]
    );
}
#[test]
fn checked_bijections() {
    let p = P::new([Cl3::scalar(2.), Cl3::scalar(3.)]);
    for map in [[0, 2], [1, 1]] {
        assert!(matches!(
            p.shuffle_components(&map),
            Err(AlgebraError::NonBijectiveMap)
        ));
        assert!(F::<2>::identity().permute_bins(&map).is_err());
    }
    assert_eq!(
        p.shuffle_components(&[1, 0]).unwrap().components()[0],
        Cl3::scalar(3.)
    );
}
