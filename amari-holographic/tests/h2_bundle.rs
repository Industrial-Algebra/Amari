#[cfg(feature = "rayon")]
use amari_holographic::ProductCliffordAlgebra as P;
use amari_holographic::{AlgebraError, BindingAlgebra, Cl3, MAPAlgebra as M};

#[test]
fn boundaries_and_ties() {
    let first = M::new([-1.]);
    let second = M::new([1.]);
    assert_eq!(
        M::majority(&[first.clone(), second.clone()])
            .unwrap()
            .components(),
        &[-1.]
    );
    assert_eq!(M::majority(&[second, first]).unwrap().components(), &[1.]);
    for beta in [-1., f64::NAN, f64::NEG_INFINITY] {
        assert_eq!(Cl3::bundle_all(&[], beta), Err(AlgebraError::Beta));
        assert_eq!(
            Cl3::bundle_all(&[Cl3::one()], beta),
            Err(AlgebraError::Beta)
        );
    }
    assert_eq!(M::<1>::bundle_all(&[], 0.).unwrap().components(), &[0.]);
    assert_eq!(
        M::bundle_all(&[M::new([3.])], 0.).unwrap().components(),
        &[3.]
    );
    assert_eq!(
        M::new([2.]).bundle(&M::new([2.]), 0.).unwrap().components(),
        &[0.]
    );
}
#[cfg(feature = "rayon")]
#[test]
fn pool_sizes_match_ordered_formula_and_suffix_errors() {
    for n in [3, 17, 64] {
        let scalars: Vec<_> = (0..n).map(|i| (i % 7) as f64 - 3.).collect();
        let items: Vec<_> = scalars.iter().map(|&s| P::new([Cl3::scalar(s)])).collect();
        for beta in [0., 0.25, 2.] {
            let weights: Vec<_> = scalars.iter().map(|s| (beta * s.abs()).exp()).collect();
            let oracle = scalars
                .iter()
                .zip(&weights)
                .map(|(x, w)| x * w)
                .sum::<f64>()
                / weights.iter().sum::<f64>();
            let seq = P::bundle_all(&items, beta).unwrap();
            assert!((seq.get(0).unwrap() - oracle).abs() < 1e-12);
            for t in [1, 2, 4] {
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(t)
                    .build()
                    .unwrap();
                let par = pool.install(|| P::par_bundle_all(&items, beta)).unwrap();
                assert_eq!(
                    seq.geometry_coefficients()
                        .iter()
                        .map(|x| x.to_bits())
                        .collect::<Vec<_>>(),
                    par.geometry_coefficients()
                        .iter()
                        .map(|x| x.to_bits())
                        .collect::<Vec<_>>()
                );
                let mut bad = items.clone();
                bad.push(P::new([Cl3::scalar(f64::INFINITY)]));
                assert_eq!(
                    P::bundle_all(&bad, beta).unwrap_err(),
                    AlgebraError::NonFinite
                );
                assert_eq!(
                    pool.install(|| P::par_bundle_all(&bad, beta)).unwrap_err(),
                    AlgebraError::NonFinite
                );
            }
        }
    }
}
#[cfg(feature = "rayon")]
#[test]
fn rayon_identity_pair() {
    let a = P::<1>::identity();
    let list = [a.clone(), a];
    let single = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .unwrap();
    let p = single.install(|| P::par_bundle_all(&list, 0.).unwrap());
    assert_eq!(p.get(0).unwrap(), 1.);
}
#[test]
fn whole_list_not_fold() {
    let x = [0., 0., 4.].map(Cl3::scalar);
    assert_eq!(Cl3::bundle_all(&x, 0.).unwrap().coeff(0), 4. / 3.);
}
#[test]
fn map_pair_list() {
    let a = M::new([2.]);
    let b = M::new([0.]);
    assert_eq!(
        a.bundle(&b, 1.).unwrap().components(),
        M::bundle_all(&[a, b], 1.).unwrap().components()
    );
}
