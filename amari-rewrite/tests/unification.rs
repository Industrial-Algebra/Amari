//! Unification and checked substitution contract tests
//! (0.25 Cohort 2, Task 7).
//!
//! Deterministic first-order unification with occurs check, canonical
//! binding order, checked idempotent composition, and strict
//! term/operation limits. Shared by critical pairs, inverse frontier
//! meeting, constraints, and synthesis.

use amari_rewrite::analysis::{unify, unify_with};
use amari_rewrite::relation::{RelationLimits, RelationResources};
use amari_rewrite::trs::{Substitution, Term};
use amari_rewrite::RewriteError;

fn resources() -> RelationResources {
    RelationResources::new(&RelationLimits::default())
}

#[test]
fn mgu_binds_variables_deterministically() {
    let mut budget = resources();
    let subst = unify(&Term::var("X"), &Term::constant("a"), &mut budget).unwrap();
    assert_eq!(subst.get("X"), Some(&Term::constant("a")));
    assert_eq!(subst.len(), 1);

    // Variable-variable: first side binds to the second.
    let mut budget = resources();
    let subst = unify(&Term::var("X"), &Term::var("Y"), &mut budget).unwrap();
    assert_eq!(subst.get("X"), Some(&Term::var("Y")));

    // Structured terms bind both variables.
    let mut budget = resources();
    let subst = unify(
        &Term::sym("f", vec![Term::var("X"), Term::constant("b")]),
        &Term::sym("f", vec![Term::constant("a"), Term::var("Y")]),
        &mut budget,
    )
    .unwrap();
    assert_eq!(subst.get("X"), Some(&Term::constant("a")));
    assert_eq!(subst.get("Y"), Some(&Term::constant("b")));
}

#[test]
fn mgu_makes_sides_equal() {
    let left = Term::sym(
        "f",
        vec![Term::var("X"), Term::sym("g", vec![Term::var("X")])],
    );
    let right = Term::sym(
        "f",
        vec![Term::sym("h", vec![Term::var("Y")]), Term::var("Z")],
    );
    let mut budget = resources();
    let subst = unify(&left, &right, &mut budget).unwrap();
    assert_eq!(subst.apply(&left), subst.apply(&right));
}

#[test]
fn occurs_check_rejects_cyclic_bindings() {
    let mut budget = resources();
    let result = unify(
        &Term::var("X"),
        &Term::sym("f", vec![Term::var("X")]),
        &mut budget,
    );
    assert!(matches!(
        result,
        Err(RewriteError::UnificationFailure { .. })
    ));

    // Nested: X ~ f(Y), Y ~ g(X) is cyclic through the substitution.
    let mut budget = resources();
    let result = unify(
        &Term::sym("f", vec![Term::var("X"), Term::var("Y")]),
        &Term::sym(
            "f",
            vec![
                Term::sym("g", vec![Term::var("Y")]),
                Term::sym("h", vec![Term::var("X")]),
            ],
        ),
        &mut budget,
    );
    assert!(matches!(
        result,
        Err(RewriteError::UnificationFailure { .. })
    ));
}

#[test]
fn repeated_variables_and_shape_mismatches_fail() {
    // f(X, X) cannot equal f(a, b).
    let mut budget = resources();
    let result = unify(
        &Term::sym("f", vec![Term::var("X"), Term::var("X")]),
        &Term::sym("f", vec![Term::constant("a"), Term::constant("b")]),
        &mut budget,
    );
    assert!(matches!(
        result,
        Err(RewriteError::UnificationFailure { .. })
    ));

    // Arity mismatch.
    let mut budget = resources();
    let result = unify(
        &Term::sym("f", vec![Term::var("X")]),
        &Term::sym("f", vec![Term::var("X"), Term::var("Y")]),
        &mut budget,
    );
    assert!(matches!(
        result,
        Err(RewriteError::UnificationFailure { .. })
    ));

    // Head-symbol clash.
    let mut budget = resources();
    let result = unify(
        &Term::sym("f", vec![Term::var("X")]),
        &Term::sym("g", vec![Term::var("X")]),
        &mut budget,
    );
    assert!(matches!(
        result,
        Err(RewriteError::UnificationFailure { .. })
    ));
}

#[test]
fn alpha_renamed_pairs_unify_isomorphically() {
    let left = Term::sym("f", vec![Term::var("X"), Term::var("X")]);
    let right = Term::sym("f", vec![Term::var("Y"), Term::var("Y")]);
    let mut budget = resources();
    let subst = unify(&left, &right, &mut budget).unwrap();
    assert_eq!(subst.apply(&left), subst.apply(&right));
    // Exactly one binding: X ↦ Y.
    assert_eq!(subst.len(), 1);
}

#[test]
fn checked_composition_combines_and_validates() {
    // {Y ↦ X} then {X ↦ a} composes to {Y ↦ a, X ↦ a}.
    let inner = Substitution::new().with("Y", Term::var("X"));
    let outer = Substitution::new().with("X", Term::constant("a"));
    let composed = outer.compose(&inner).unwrap();
    assert_eq!(composed.get("Y"), Some(&Term::constant("a")));
    assert_eq!(composed.get("X"), Some(&Term::constant("a")));

    // Incompatible bindings for the same variable are rejected.
    let a = Substitution::new().with("X", Term::constant("a"));
    let b = Substitution::new().with("X", Term::constant("b"));
    assert!(matches!(
        a.compose(&b),
        Err(RewriteError::InvalidSubstitution { .. })
    ));

    // Non-idempotent results are rejected: {X ↦ f(Y)} ∘ {Y ↦ X}.
    let inner = Substitution::new().with("Y", Term::var("X"));
    let outer = Substitution::new().with("X", Term::sym("f", vec![Term::var("Y")]));
    assert!(matches!(
        outer.compose(&inner),
        Err(RewriteError::InvalidSubstitution { .. })
    ));
}

#[test]
fn unification_respects_operation_limits() {
    let limits = RelationLimits::new(4_096, 64, 4_096, 2).unwrap();
    let mut budget = RelationResources::new(&limits);
    // Three argument pairs exceed a two-operation budget.
    let result = unify(
        &Term::sym(
            "f",
            vec![
                Term::constant("a"),
                Term::constant("b"),
                Term::constant("c"),
            ],
        ),
        &Term::sym(
            "f",
            vec![
                Term::constant("a"),
                Term::constant("b"),
                Term::constant("c"),
            ],
        ),
        &mut budget,
    );
    assert!(matches!(
        result,
        Err(RewriteError::RelationLimitExceeded { .. })
    ));
}

/// The doubling chain of the F16 replay finding: leaf `f(Z, X0, ..., Xn)` and
/// goal `f(X0, pair(X1,X1), ..., pair(Xn,Xn), a)`.
fn doubling_pair(arity: usize) -> (Term, Term) {
    let mut leaf_args = vec![Term::var("Z")];
    leaf_args.extend((0..arity).map(|index| Term::var(format!("X{index}"))));
    let mut goal_args = vec![Term::var("X0")];
    // Ascending order: `pair(X1,X1)` pairs with the leaf's `X0`, and so on.
    for index in 1..arity {
        let variable = Term::var(format!("X{index}"));
        goal_args.push(Term::sym("pair", vec![variable.clone(), variable]));
    }
    goal_args.push(Term::constant("a"));
    (Term::sym("f", leaf_args), Term::sym("f", goal_args))
}

/// F16 (P1): resolve/occurs work is charged against the unified budget.
///
/// The outer worklist of the doubling chain is tiny (one pair per argument), so
/// a budget that the worklist alone fits used to be irrelevant: the occurs
/// checks resolved the whole doubling expansion unmetered. Resolution and
/// occurs inspection now share the caller's [`RelationResources`], so a budget
/// that cannot pay for the traversal is a typed operations rejection instead of
/// unbounded work.
#[test]
fn resolution_and_occurs_work_is_metered() {
    let (leaf, goal) = doubling_pair(21);
    let limits = RelationLimits::new(4_096, 64, 4_096, 256).expect("valid tightened limits");
    let mut budget = RelationResources::new(&limits);
    let result = unify(&leaf, &goal, &mut budget);
    assert!(
        matches!(
            result,
            Err(RewriteError::RelationLimitExceeded {
                resource: "operations",
                ..
            })
        ),
        "an under-budget traversal is a typed operations rejection, got {result:?}"
    );
    // Negative control: the traversal is not charged per *expansion* node, so
    // the same unification inside the default budget still succeeds with the
    // same unifier and a linear meter.
    let mut budget = resources();
    let substitution = unify(&leaf, &goal, &mut budget).expect("the chain unifies");
    assert_eq!(
        substitution.len(),
        22,
        "one binding per chain link, plus the free leading variable"
    );
    assert!(
        budget.operations() <= 4_096,
        "each reachable binding is inspected once: {} operations",
        budget.operations()
    );
}

/// F16 cycle/fixed-point semantics: a pre-existing cyclic substitution makes
/// resolve/occurs terminate instead of recursing forever.
#[test]
fn cyclic_bindings_resolve_to_their_fixed_point_during_unification() {
    let mut cyclic = Substitution::new().with("Y", Term::sym("g", vec![Term::var("Y")]));
    let mut budget = resources();
    unify_with(&mut cyclic, &Term::var("X"), &Term::var("Y"), &mut budget)
        .expect("a cyclic binding is a fixed point, not a divergence");
    assert_eq!(cyclic.get("X"), Some(&Term::sym("g", vec![Term::var("Y")])));
    // A self-referential binding resolves to the variable itself.
    let mut identity = Substitution::new().with("W", Term::var("W"));
    let mut budget = resources();
    unify_with(&mut identity, &Term::var("V"), &Term::var("W"), &mut budget)
        .expect("W := W is the identity");
    assert_eq!(identity.get("V"), Some(&Term::var("W")));
}
