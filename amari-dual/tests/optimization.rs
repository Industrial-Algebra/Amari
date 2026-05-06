use amari_dual::{BranchPolicy, DualNumber, MultiDualNumber, StaticMultiDual};

#[test]
fn dynamic_and_static_gradients_agree_for_small_quadratic_score() {
    let dynamic_vars = MultiDualNumber::variables(&[0.5, 1.25]);
    let dx = dynamic_vars[0].clone();
    let dy = dynamic_vars[1].clone();
    let dynamic_score = dx.clone() * dx.clone() + MultiDualNumber::constant(3.0, 2) * dy;

    let [sx, sy] = StaticMultiDual::<f64, 2>::variables([0.5, 1.25]);
    let static_score = sx * sx + StaticMultiDual::constant(3.0) * sy;

    assert_eq!(dynamic_score.get_value(), static_score.get_value());
    assert_eq!(dynamic_score.get_gradient(), static_score.get_gradient());
}

#[test]
fn branch_policies_make_tie_behavior_explicit() {
    let left = DualNumber::new(2.0, 1.0);
    let right = DualNumber::new(2.0, 5.0);

    assert_eq!(left.max(right).derivative(), 1.0);
    assert_eq!(
        left.max_by_policy(right, BranchPolicy::Left).derivative(),
        1.0
    );
    assert_eq!(
        left.max_by_policy(right, BranchPolicy::Right).derivative(),
        5.0
    );
    assert_eq!(
        left.max_by_policy(right, BranchPolicy::Average)
            .derivative(),
        3.0
    );
}

#[test]
fn multi_dual_seed_helper_matches_manual_basis_vectors() {
    let seeded = MultiDualNumber::variables(&[2.0, 3.0, 5.0]);
    let manual = [
        MultiDualNumber::variable(2.0, 0, 3),
        MultiDualNumber::variable(3.0, 1, 3),
        MultiDualNumber::variable(5.0, 2, 3),
    ];

    assert_eq!(seeded, manual);
}

#[test]
fn static_multi_dual_average_policy_matches_expected_gradient_split() {
    let left = StaticMultiDual::<f64, 2>::new(1.0, [1.0, 0.0]);
    let right = StaticMultiDual::<f64, 2>::new(1.0, [0.0, 1.0]);

    let averaged = left.max_by_policy(right, BranchPolicy::Average);

    assert_eq!(averaged.get_value(), 1.0);
    assert_eq!(averaged.get_gradient(), &[0.5, 0.5]);
}
