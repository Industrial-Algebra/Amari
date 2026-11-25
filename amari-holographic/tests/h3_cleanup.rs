use amari_holographic::{
    memory::{CleanupResult, WeightedStatus},
    Cl3, Resonator, ResonatorConfig,
};
#[test]
fn u5_original_observation_certificate() {
    let one = Cl3::one();
    let r = Resonator::new(vec![Cl3::scalar(1.0001)], Default::default()).unwrap();
    assert!(matches!(
        r.cleanup_nearest(&one).unwrap(),
        CleanupResult::NoAcceptableCandidate { .. }
    ));
    let poor = r.cleanup_weighted(&one).unwrap();
    assert!(!poor.accepted);
    assert!(matches!(poor.status, WeightedStatus::Stalled { .. }));
}
#[test]
fn u5_real_update_and_hard_selection() {
    let a = Cl3::unit_vector(0);
    let b = Cl3::unit_vector(1);
    for (beta, x) in [(0., 0.5), (3_f64.ln(), 0.75), (f64::INFINITY, 1.)] {
        let r = Resonator::new(
            vec![a, b],
            ResonatorConfig {
                initial_beta: beta,
                final_beta: beta,
                max_iterations: 1,
            },
        )
        .unwrap();
        let result = r.cleanup_weighted(&a).unwrap();
        assert!((result.candidate.coeff(1) - x).abs() < 1e-12);
        assert!((result.candidate.coeff(2) - (1. - x)).abs() < 1e-12);
        assert_eq!(result.accepted, x == 1.);
    }
}
#[test]
fn u5_orthogonal_and_invalid() {
    let a = Cl3::unit_vector(0);
    let b = Cl3::unit_vector(1);
    let r = Resonator::new(vec![b], Default::default()).unwrap();
    assert!(!r.cleanup_weighted(&a).unwrap().accepted);
    for cfg in [
        ResonatorConfig {
            max_iterations: 0,
            ..Default::default()
        },
        ResonatorConfig {
            initial_beta: f64::NAN,
            ..Default::default()
        },
    ] {
        assert!(Resonator::new(vec![a], cfg).is_err());
    }
}
