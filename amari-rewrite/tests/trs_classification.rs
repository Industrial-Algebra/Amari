// SPDX-License-Identifier: MIT OR Apache-2.0

//! TRS classifier suites (0.25 Cohort 5, Task 24): taxonomy,
//! capability table, and the ADR 0001 rule-contract boundary.

use amari_rewrite::language::{
    classify_system, PreimageCapability, PreimageConstruction, PreimageOperation, TrsClass,
};
use amari_rewrite::trs::{Rule, Term, TermSystem};
use amari_rewrite::RewriteError;

fn a() -> Term {
    Term::constant("a")
}

fn f(left: Term, right: Term) -> Term {
    Term::sym("f", [left, right])
}

fn g(inner: Term) -> Term {
    Term::sym("g", [inner])
}

fn x() -> Term {
    Term::var("x")
}

fn y() -> Term {
    Term::var("y")
}

fn classify(rules: Vec<Rule>) -> TrsClass {
    classify_system(&TermSystem::new(rules))
        .expect("checked rules classify")
        .class()
}

#[test]
fn empty_system_is_ground() {
    let classification = classify_system(&TermSystem::new(Vec::new())).expect("empty classifies");
    assert_eq!(classification.class(), TrsClass::Ground);
    assert_eq!(classification.rule_count(), 0);
}

#[test]
fn ground_system_classifies_ground() {
    let rules = vec![
        Rule::new(a(), g(a())).unwrap(),
        Rule::new(f(a(), a()), a()).unwrap(),
    ];
    assert_eq!(classify(rules), TrsClass::Ground);
}

#[test]
fn right_ground_linear_system_is_variable_disjoint() {
    let rules = vec![Rule::new(g(x()), g(a())).unwrap()];
    assert_eq!(classify(rules), TrsClass::LinearVariableDisjoint);
    // A mix of ground and right-ground rules stays in the class.
    let rules = vec![
        Rule::new(g(x()), g(a())).unwrap(),
        Rule::new(a(), f(a(), a())).unwrap(),
    ];
    assert_eq!(classify(rules), TrsClass::LinearVariableDisjoint);
}

#[test]
fn shared_variable_linear_system_is_left_linear_shared() {
    let rules = vec![Rule::new(g(x()), f(x(), x())).unwrap()];
    assert_eq!(classify(rules), TrsClass::LeftLinearShared);
    // Erasure shares a variable with a non-ground right side? No:
    // g(x) -> a is right-ground (LVD); f(x,y) -> g(x) has non-ground
    // right side sharing x.
    let rules = vec![Rule::new(f(x(), y()), g(x())).unwrap()];
    assert_eq!(classify(rules), TrsClass::LeftLinearShared);
}

#[test]
fn repeated_left_variable_is_non_left_linear() {
    let rules = vec![Rule::new(f(x(), x()), a()).unwrap()];
    assert_eq!(classify(rules), TrsClass::NonLeftLinear);
    // Even one non-left-linear rule moves the whole system.
    let rules = vec![
        Rule::new(g(x()), a()).unwrap(),
        Rule::new(f(y(), y()), g(y())).unwrap(),
    ];
    assert_eq!(classify(rules), TrsClass::NonLeftLinear);
}

#[test]
fn unchecked_rule_contract_violation_is_a_boundary_error() {
    let system = TermSystem::new(vec![Rule::new_unchecked(g(x()), y())]);
    match classify_system(&system) {
        Err(RewriteError::InvalidRule { message }) => {
            assert!(message.contains("rule 0"), "message: {message}");
            assert!(message.contains('y'), "message: {message}");
        }
        other => panic!("expected InvalidRule, got {other:?}"),
    }
    // The violating rule is named by index.
    let system = TermSystem::new(vec![
        Rule::new(g(x()), g(a())).unwrap(),
        Rule::new_unchecked(g(x()), y()),
    ]);
    match classify_system(&system) {
        Err(RewriteError::InvalidRule { message }) => {
            assert!(message.contains("rule 1"), "message: {message}");
        }
        other => panic!("expected InvalidRule, got {other:?}"),
    }
}

#[test]
fn capability_table_matches_the_approved_matrix() {
    let ground = classify(vec![Rule::new(a(), g(a())).unwrap()]);
    let disjoint = classify(vec![Rule::new(g(x()), g(a())).unwrap()]);
    let shared = classify(vec![Rule::new(g(x()), f(x(), x())).unwrap()]);
    let non_left_linear = classify(vec![Rule::new(f(x(), x()), a()).unwrap()]);

    for class in [ground, disjoint] {
        assert_eq!(
            class.capability(PreimageOperation::OneStep),
            PreimageCapability::Exact(PreimageConstruction::LeftLinearOneStep)
        );
        assert_eq!(
            class.capability(PreimageOperation::FiniteHorizon(0)),
            PreimageCapability::Exact(PreimageConstruction::Identity)
        );
        assert_eq!(
            class.capability(PreimageOperation::FiniteHorizon(3)),
            PreimageCapability::Exact(PreimageConstruction::FiniteHorizonIteration(3))
        );
        assert_eq!(
            class.capability(PreimageOperation::Saturation),
            PreimageCapability::Exact(PreimageConstruction::GttSaturation)
        );
    }

    assert_eq!(
        shared.capability(PreimageOperation::OneStep),
        PreimageCapability::Exact(PreimageConstruction::LeftLinearOneStep)
    );
    assert_eq!(
        shared.capability(PreimageOperation::FiniteHorizon(0)),
        PreimageCapability::Exact(PreimageConstruction::Identity)
    );
    assert_eq!(
        shared.capability(PreimageOperation::FiniteHorizon(3)),
        PreimageCapability::Exact(PreimageConstruction::FiniteHorizonIteration(3))
    );
    assert_eq!(
        shared.capability(PreimageOperation::Saturation),
        PreimageCapability::ApproximationOnly
    );

    assert_eq!(
        non_left_linear.capability(PreimageOperation::OneStep),
        PreimageCapability::ApproximationOnly
    );
    assert_eq!(
        non_left_linear.capability(PreimageOperation::FiniteHorizon(0)),
        PreimageCapability::Exact(PreimageConstruction::Identity)
    );
    assert_eq!(
        non_left_linear.capability(PreimageOperation::FiniteHorizon(3)),
        PreimageCapability::ApproximationOnly
    );
    assert_eq!(
        non_left_linear.capability(PreimageOperation::Saturation),
        PreimageCapability::ApproximationOnly
    );
}
