//! Checked `relation!` macro behavior (0.25 Cohort 7, Task 38).
//!
//! Grammar contract: `relation!([exists V {, V} .] lhs <=> rhs
//! [{where constraint {, constraint}}])` where a constraint is
//! `term ("==" | "!=") term`. The expansion yields
//! `RewriteResult<(Rule, BackwardClause, Option<Rule>, ConstraintSet)>`:
//! the checked forward rule, its compiled backward contract, the
//! checked reverse rule exactly when the reverse direction erases
//! nothing, and the declared constraints in declaration order. Every
//! element is built by a checked constructor (`Rule::new`,
//! `BackwardClause::compile`); no closure, no `Rule::new_unchecked`,
//! and no runtime registration is generated.
//!
//! Compile-time rejections (malformed constraints, incomplete
//! existential markers, forward-introduced variables, ARS/TRS arrow
//! ambiguity) live in the trybuild fixtures under
//! `amari-rewrite-macros/tests/ui/`.

#![cfg(feature = "macros")]

use amari_rewrite::relation::{
    BackwardClause, ConstraintOutcome, ConstraintSet, RelationLimits, TermConstraint,
};
use amari_rewrite::reversible::BidirectionalRule;
use amari_rewrite::trs::{Rule, Term};
use amari_rewrite::{relation, term, RewriteResult};

/// The exact expansion type: forward rule, backward contract, checked
/// reverse rule (present exactly when the reverse direction is a
/// checked rule), and declared constraints.
type Declaration = (Rule, BackwardClause, Option<Rule>, ConstraintSet);

fn declare(declaration: RewriteResult<Declaration>) -> Declaration {
    declaration.expect("checked relation declarations are rejected at macro time")
}

#[test]
fn reversible_relation_yields_both_checked_directions() {
    let (forward, backward, reverse, constraints) = declare(relation!(add(zero, X) <=> X));

    assert_eq!(forward.lhs(), &term!(add(zero, X)));
    assert_eq!(forward.rhs(), &term!(X));
    assert!(backward.erased_variables().is_empty());
    assert!(constraints.is_empty());

    let reverse = reverse.expect("a reversible relation yields a checked reverse rule");
    assert_eq!(reverse.lhs(), &term!(X));
    assert_eq!(reverse.rhs(), &term!(add(zero, X)));
}

#[test]
fn output_types_are_locked() {
    let locked: RewriteResult<(Rule, BackwardClause, Option<Rule>, ConstraintSet)> =
        relation!(add(zero, X) <=> X);
    assert!(locked.is_ok());

    let erasing: RewriteResult<(Rule, BackwardClause, Option<Rule>, ConstraintSet)> =
        relation!(pair(X, Y) <=> X);
    assert!(erasing.is_ok());
}

#[test]
fn erasing_reverse_direction_yields_the_clause_form() {
    let (forward, backward, reverse, constraints) = declare(relation!(pair(X, Y) <=> X));

    assert_eq!(forward.lhs(), &term!(pair(X, Y)));
    assert_eq!(forward.rhs(), &term!(X));
    // The reverse direction would invent `Y`, so no unchecked reverse
    // rule is fabricated: the compiled clause carries the erased
    // variable names instead (the Task 37 residual schema keys).
    assert_eq!(backward.erased_variables(), [String::from("Y")]);
    assert!(reverse.is_none());
    assert!(constraints.is_empty());
}

#[test]
fn existential_marker_matches_inferred_erasure() {
    let declared = declare(relation!(exists Y. pair(X, Y) <=> X));
    let inferred = declare(relation!(pair(X, Y) <=> X));

    assert_eq!(declared.0, inferred.0);
    assert_eq!(declared.1, inferred.1);
    assert_eq!(declared.2, inferred.2);
    assert_eq!(declared.3, inferred.3);
    assert_eq!(declared.1.erased_variables(), [String::from("Y")]);
}

#[test]
fn multiple_existential_markers_are_accepted() {
    let (forward, backward, reverse, _) = declare(relation!(exists Y, Z. triple(X, Y, Z) <=> X));

    assert_eq!(forward.lhs(), &term!(triple(X, Y, Z)));
    assert_eq!(
        backward.erased_variables(),
        ["Y".to_string(), "Z".to_string()]
    );
    assert!(reverse.is_none());
}

#[test]
fn constraints_are_carried_in_declaration_order() {
    let (_, _, reverse, constraints) = declare(relation!(
        f(X, Y) <=> g(X, Y) where X != Y, Y == zero
    ));

    assert!(reverse.is_some(), "both sides bind the same variables");
    assert_eq!(
        constraints.constraints(),
        [
            TermConstraint::NotEqual(term!(X), term!(Y)),
            TermConstraint::Equal(term!(Y), term!(zero)),
        ]
    );
}

#[test]
fn declared_constraints_normalize_through_the_checked_model() {
    let (_, _, _, constraints) = declare(relation!(
        f(X, Y) <=> g(X, Y) where X != Y, Y == zero
    ));

    let limits = RelationLimits::default();
    match constraints
        .normalize(&limits)
        .expect("normalization is infallible here")
    {
        ConstraintOutcome::Satisfiable {
            substitution,
            residuals,
        } => {
            assert_eq!(substitution.apply(&term!(Y)), term!(zero));
            assert_eq!(residuals, [TermConstraint::NotEqual(term!(X), term!(zero))]);
        }
        other => panic!("expected satisfiable residuals, got {other:?}"),
    }
}

#[test]
fn contradictory_constraints_are_unsatisfiable() {
    let (_, _, _, constraints) = declare(relation!(f(X) <=> g(X) where X != X));

    assert_eq!(
        constraints.normalize(&RelationLimits::default()).unwrap(),
        ConstraintOutcome::Unsatisfiable
    );
}

#[test]
fn expansion_is_a_pure_function_of_its_tokens() {
    let first = declare(relation!(pair(X, Y) <=> X));
    let second = declare(relation!(pair(X, Y) <=> X));
    assert_eq!(first, second);
}

#[test]
fn clause_form_matches_the_cohort_two_composite() {
    let (forward, backward, reverse, _) = declare(relation!(pair(X, Y) <=> X));

    // The Task 37 composite compiles the same checked rule to the same
    // clause, so macro-built declarations stay interchangeable with
    // runtime-built bidirectional rules.
    let composite = BidirectionalRule::compile(forward.clone()).unwrap();
    assert_eq!(composite.rule_id(), backward.rule_id());
    assert_eq!(
        composite.clause().erased_variables(),
        backward.erased_variables()
    );
    assert!(reverse.is_none());
}

#[test]
fn symbol_named_exists_still_parses_as_a_term() {
    // `exists` is only a marker when an existential variable follows:
    // followed by another identifier or a comma. Elsewhere it stays an
    // ordinary symbol name, so the keyword never becomes reserved.
    let (forward, _, reverse, _) = declare(relation!(exists <=> f(exists)));
    assert_eq!(forward.lhs(), &Term::constant("exists"));
    assert_eq!(forward.rhs(), &term!(f(exists)));
    assert!(reverse.is_some());
}
