use amari_holographic::{
    memory::{CleanupResult, WeightedStatus},
    Cl3, Resonator, ResonatorConfig,
};
fn scheduled(
    initial_beta: f64,
    final_beta: f64,
    max_iterations: usize,
    reversed: bool,
) -> Resonator<Cl3> {
    let mut words = [Cl3::unit_vector(0), Cl3::unit_vector(1)];
    if reversed {
        words.reverse();
    }
    Resonator::new(
        words.to_vec(),
        ResonatorConfig {
            initial_beta,
            final_beta,
            max_iterations,
        },
    )
    .unwrap()
}
#[test]
fn r1_linear_schedule_three_updates() {
    let l = 3_f64.ln();
    let obs = Cl3::unit_vector(0);
    let result = scheduled(l, 3. * l, 3, false)
        .cleanup_weighted(&obs)
        .unwrap();
    // In the (e1,e2) plane the first mixture is (3/4,1/4). Its cosine
    // gap is 2/sqrt(10), so the middle beta = 2 ln(3) gives odds 9^(2/sqrt(10)).
    // For a mixture proportional to (odds,1), the next gap is (odds-1)/hypot(odds,1).
    let odds = 9_f64.powf(2. / 10_f64.sqrt());
    let gap = (odds - 1.) / odds.hypot(1.);
    let last_odds = 27_f64.powf(gap);
    let w = last_odds / (1. + last_odds);
    for (actual, expected) in
        result
            .candidate
            .coefficients()
            .iter()
            .zip([0., w, 1. - w, 0., 0., 0., 0., 0.])
    {
        assert!((actual - expected).abs() < 1e-12);
    }
    assert!(!result.accepted);
    assert!(matches!(result.status, WeightedStatus::BudgetExhausted));
}
#[test]
fn r1_single_update_endpoint() {
    let obs = Cl3::unit_vector(0);
    // Budget one uses the initial endpoint, not the (distinct) final beta.
    let r = scheduled(0., f64::INFINITY, 1, false)
        .cleanup_weighted(&obs)
        .unwrap();
    assert_eq!(r.candidate, Cl3::vector(0.5, 0.5, 0.));
    assert!(matches!(r.status, WeightedStatus::BudgetExhausted));
    assert!(!r.accepted);
    // Coincident hard endpoints: already at final beta on the first update.
    let r = scheduled(f64::INFINITY, f64::INFINITY, 1, false)
        .cleanup_weighted(&obs)
        .unwrap();
    assert_eq!(r.candidate, obs);
    assert!(r.accepted);
    assert!(matches!(
        r.status,
        WeightedStatus::Updated { iterations: 1 }
    ));
}
#[test]
fn r1_nearest_tie_is_order_independent() {
    let obs = Cl3::one();
    for reversed in [false, true] {
        match scheduled(0., 0., 1, reversed)
            .cleanup_nearest(&obs)
            .unwrap()
        {
            CleanupResult::NoAcceptableCandidate { best, residual } => {
                assert_eq!(best, Cl3::unit_vector(1));
                assert_eq!(residual, 1.);
            }
            other => panic!("orthogonal word accepted: {other:?}"),
        }
    }
}
#[test]
fn r1_finite_then_hard_tie() {
    let obs = Cl3::unit_vector(0);
    for reversed in [false, true] {
        // beta=0 produces (1/2,1/2), so the FINAL hard step sees a tie, unlike the initial observation.
        let r = scheduled(0., f64::INFINITY, 2, reversed)
            .cleanup_weighted(&obs)
            .unwrap();
        assert_eq!(r.candidate, Cl3::unit_vector(1));
        assert!(!r.accepted);
        assert!(matches!(r.status, WeightedStatus::BudgetExhausted));
    }
}

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
