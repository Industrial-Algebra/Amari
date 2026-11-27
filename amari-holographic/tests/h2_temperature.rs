use amari_holographic::{
    algebra::{log_mean_exp, log_sum_exp},
    BindingAlgebra, MAPAlgebra as M,
};
#[test]
fn distinct_zero_limits() {
    assert!(log_sum_exp(0., 0., 0.).is_err());
    assert_eq!(log_mean_exp(-3., 7., 0.).unwrap(), 2.);
    for b in [1e-200, 1e-6, 0.5, 1e200, f64::INFINITY] {
        assert_eq!(log_mean_exp(2., 2., b).unwrap(), 2.);
    }
    assert!((log_sum_exp(0., 0., 1e-6).unwrap() - 693147.1805599453).abs() < 1e-9);
    assert_eq!(log_mean_exp(0., 0., 1e-6).unwrap(), 0.);
}
#[test]
fn bind_distributes_over_addition() {
    let k = M::new([-1., 2.]);
    let a = M::new([1., 3.]);
    let b = M::new([2., -4.]);
    assert_eq!(
        k.bind(&a.superpose(&b).unwrap()).components(),
        k.bind(&a).superpose(&k.bind(&b)).unwrap().components()
    );
}
