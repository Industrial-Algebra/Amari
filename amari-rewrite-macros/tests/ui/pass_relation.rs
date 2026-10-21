// SPDX-License-Identifier: MIT OR Apache-2.0

//! relation! shapes and the exact expanded output type.
//!
//! The expansion is always
//! `RewriteResult<(Rule, BackwardClause, Option<Rule>, ConstraintSet)>`:
//! the reverse rule is `Some` exactly when the reverse direction is a
//! checked rule, and `None` when it erases variables (the clause's
//! `erased_variables()` then carries the residual schema names).

use amari_rewrite::relation::{BackwardClause, ConstraintSet, TermConstraint};
use amari_rewrite::trs::{Rule, Term};
use amari_rewrite::RewriteResult;
use amari_rewrite_macros::{relation, term};

type Declaration = (Rule, BackwardClause, Option<Rule>, ConstraintSet);

fn main() {
    // Reversible: both directions are checked rules.
    let reversible: RewriteResult<Declaration> = relation!(add(zero, X) <=> X);
    let (forward, backward, reverse, constraints) = reversible.unwrap();
    assert_eq!(forward.lhs(), &term!(add(zero, X)));
    assert_eq!(forward.rhs(), &term!(X));
    assert!(backward.erased_variables().is_empty());
    assert_eq!(reverse.as_ref().map(Rule::lhs), Some(&term!(X)));
    assert_eq!(reverse.as_ref().map(Rule::rhs), Some(&term!(add(zero, X))));
    assert!(constraints.is_empty());

    // Erasing reverse direction: the clause form, never an unchecked
    // reverse rule.
    let erasing: RewriteResult<Declaration> = relation!(pair(X, Y) <=> X);
    let (_, clause, reverse, _) = erasing.unwrap();
    assert_eq!(clause.erased_variables(), [String::from("Y")]);
    assert!(reverse.is_none());

    // Explicit existential markers are validated against the actual
    // erasure and expand identically to the inferred form.
    let marked: Declaration = relation!(exists Y. pair(X, Y) <=> X).unwrap();
    assert_eq!(marked.1.erased_variables(), [String::from("Y")]);
    assert!(marked.2.is_none());

    // Constraints are carried in declaration order.
    let constrained: Declaration =
        relation!(f(X, Y) <=> g(X, Y) where X != Y, Y == zero).unwrap();
    assert_eq!(
        constrained.3.constraints(),
        [
            TermConstraint::NotEqual(term!(X), term!(Y)),
            TermConstraint::Equal(term!(Y), term!(zero)),
        ]
    );

    // A symbol spelled `exists` is not reserved: the marker is only
    // recognised when an existential variable follows it.
    let symbol: Declaration = relation!(exists <=> f(exists)).unwrap();
    assert_eq!(symbol.0.lhs(), &Term::constant("exists"));
}
