//! Normalized constraint-set contract tests (0.25 Cohort 2, Task 7).
//!
//! `ConstraintSet` normalization solves equalities through the Task 7
//! unifier, applies the substitution, retains canonical disequality
//! residuals, and classifies tautology/contradiction deterministically.

use amari_rewrite::relation::{ConstraintOutcome, ConstraintSet, RelationLimits, TermConstraint};
use amari_rewrite::trs::Term;

fn eq(left: Term, right: Term) -> TermConstraint {
    TermConstraint::Equal(left, right)
}

fn ne(left: Term, right: Term) -> TermConstraint {
    TermConstraint::NotEqual(left, right)
}

#[test]
fn tautologies_are_dropped_and_disequalities_kept() {
    // Equal(t, t) is a tautology and vanishes.
    let mut set = ConstraintSet::new();
    set.insert(eq(Term::var("X"), Term::var("X")));
    let outcome = set.normalize(&RelationLimits::default()).unwrap();
    let ConstraintOutcome::Satisfiable {
        substitution,
        residuals,
    } = outcome
    else {
        panic!("expected satisfiable");
    };
    assert!(substitution.is_empty());
    assert!(residuals.is_empty());

    // NotEqual(f(a), g(b)) is non-unifiable: always true, dropped.
    let mut set = ConstraintSet::new();
    set.insert(ne(
        Term::sym("f", vec![Term::constant("a")]),
        Term::sym("g", vec![Term::constant("b")]),
    ));
    let outcome = set.normalize(&RelationLimits::default()).unwrap();
    let ConstraintOutcome::Satisfiable { residuals, .. } = outcome else {
        panic!("expected satisfiable");
    };
    assert!(residuals.is_empty());

    // NotEqual(X, a) is a real residual: X could equal a.
    let mut set = ConstraintSet::new();
    set.insert(ne(Term::var("X"), Term::constant("a")));
    let outcome = set.normalize(&RelationLimits::default()).unwrap();
    let ConstraintOutcome::Satisfiable { residuals, .. } = outcome else {
        panic!("expected satisfiable");
    };
    assert_eq!(residuals.len(), 1);
}

#[test]
fn contradictions_are_unsatisfiable() {
    // Direct: t ≠ t.
    let mut set = ConstraintSet::new();
    set.insert(ne(Term::constant("a"), Term::constant("a")));
    assert!(matches!(
        set.normalize(&RelationLimits::default()),
        Ok(ConstraintOutcome::Unsatisfiable)
    ));

    // Via equality solving: X = a contradicts X ≠ a.
    let mut set = ConstraintSet::new();
    set.insert(eq(Term::var("X"), Term::constant("a")));
    set.insert(ne(Term::var("X"), Term::constant("a")));
    assert!(matches!(
        set.normalize(&RelationLimits::default()),
        Ok(ConstraintOutcome::Unsatisfiable)
    ));

    // Via unification failure in equalities.
    let mut set = ConstraintSet::new();
    set.insert(eq(Term::constant("a"), Term::constant("b")));
    assert!(matches!(
        set.normalize(&RelationLimits::default()),
        Ok(ConstraintOutcome::Unsatisfiable)
    ));
}

#[test]
fn equalities_solve_and_apply_to_residuals() {
    // X = g(Y) with X ≠ g(a) reduces to residual g(Y) ≠ g(a).
    let mut set = ConstraintSet::new();
    set.insert(eq(Term::var("X"), Term::sym("g", vec![Term::var("Y")])));
    set.insert(ne(
        Term::var("X"),
        Term::sym("g", vec![Term::constant("a")]),
    ));
    let outcome = set.normalize(&RelationLimits::default()).unwrap();
    let ConstraintOutcome::Satisfiable {
        substitution,
        residuals,
    } = outcome
    else {
        panic!("expected satisfiable");
    };
    assert_eq!(
        substitution.get("X"),
        Some(&Term::sym("g", vec![Term::var("Y")]))
    );
    assert_eq!(residuals.len(), 1);
    let TermConstraint::NotEqual(left, right) = &residuals[0] else {
        panic!("expected disequality residual");
    };
    // The residual has the solved substitution applied.
    let rendered = format!("{left:?} != {right:?}");
    assert!(rendered.contains('g'));
    assert!(!rendered.contains("\"X\""));
}

#[test]
fn normalization_is_deterministic_regardless_of_insertion_order() {
    let build = |first: TermConstraint, second: TermConstraint, third: TermConstraint| {
        let mut set = ConstraintSet::new();
        set.insert(first);
        set.insert(second);
        set.insert(third);
        set.normalize(&RelationLimits::default()).unwrap()
    };
    let a = ne(Term::var("X"), Term::constant("a"));
    let b = ne(Term::var("Y"), Term::constant("b"));
    let c = ne(Term::var("Z"), Term::constant("c"));
    let forward = build(a.clone(), b.clone(), c.clone());
    let reverse = build(c, b, a);
    let ConstraintOutcome::Satisfiable { residuals: r1, .. } = forward else {
        panic!("expected satisfiable");
    };
    let ConstraintOutcome::Satisfiable { residuals: r2, .. } = reverse else {
        panic!("expected satisfiable");
    };
    assert_eq!(r1, r2, "canonical residual order is insertion-independent");
}

#[test]
fn alpha_renamed_sets_normalize_identically() {
    let build = |x: &str, y: &str| {
        let mut set = ConstraintSet::new();
        set.insert(ne(Term::var(x), Term::constant("a")));
        set.insert(ne(Term::var(y), Term::constant("b")));
        set.normalize(&RelationLimits::default()).unwrap()
    };
    let first = build("X", "Y");
    let second = build("P", "Q");
    let ConstraintOutcome::Satisfiable { residuals: r1, .. } = first else {
        panic!("expected satisfiable");
    };
    let ConstraintOutcome::Satisfiable { residuals: r2, .. } = second else {
        panic!("expected satisfiable");
    };
    // Canonical alpha-renumbered digests agree even though names differ.
    let digest = |constraints: &[TermConstraint]| {
        constraints
            .iter()
            .map(TermConstraint::canonical_digest)
            .collect::<Vec<_>>()
    };
    assert_eq!(digest(&r1), digest(&r2));
}

#[test]
fn constraint_limits_are_enforced() {
    let limits = RelationLimits::new(4_096, 64, 2, 1_000_000).unwrap();
    let mut set = ConstraintSet::new();
    set.insert(ne(Term::var("X"), Term::constant("a")));
    set.insert(ne(Term::var("Y"), Term::constant("b")));
    set.insert(ne(Term::var("Z"), Term::constant("c")));
    assert!(matches!(
        set.normalize(&limits),
        Err(amari_rewrite::RewriteError::RelationLimitExceeded { .. })
    ));
}

#[test]
fn normalization_propagates_unification_budget_exhaustion() {
    let limits = RelationLimits::new(4_096, 64, 4_096, 4).unwrap();
    let mut set = ConstraintSet::new();
    set.insert(eq(Term::var("X"), Term::constant("a")));
    let error = set
        .normalize(&limits)
        .expect_err("operation exhaustion is not a proof of unsatisfiability");
    assert!(
        matches!(
            error,
            amari_rewrite::RewriteError::RelationLimitExceeded {
                resource: "operations",
                limit: 4,
            }
        ),
        "the typed unification budget error must propagate, got {error}"
    );
}

#[cfg(feature = "serialize")]
#[test]
fn constraints_serde_roundtrip() {
    let constraint = ne(Term::var("X"), Term::sym("f", vec![Term::constant("a")]));
    let json = serde_json::to_string(&constraint).unwrap();
    let back: TermConstraint = serde_json::from_str(&json).unwrap();
    assert_eq!(back, constraint);
}
