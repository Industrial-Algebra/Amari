// SPDX-License-Identifier: MIT OR Apache-2.0

//! The derive must resolve `amari-rewrite` through the user's chosen
//! dependency name (`rewrite_lib` here) via proc-macro-crate.

use amari_rewrite_macros::Rewritable;

#[derive(Clone, Debug, PartialEq, Rewritable)]
enum Expr {
    Zero,
    Succ(#[rewritable(child)] Box<Expr>),
}

fn main() {
    let one = Expr::Succ(Box::new(Expr::Zero));
    assert_eq!(rewrite_lib::Rewritable::child_count(&one), 1);
    assert_eq!(
        rewrite_lib::Rewritable::child(&one, 0),
        Some(&Expr::Zero)
    );

    // term!/rule! must resolve through the renamed crate as well.
    let lhs = amari_rewrite_macros::term!(add(zero, X));
    let built = amari_rewrite_macros::rule!(add(zero, X) => X).unwrap();
    assert_eq!(built.lhs(), &lhs);
    assert_eq!(built.rhs(), &amari_rewrite_macros::term!(X));

    // relation! must resolve through the renamed crate too, in both the
    // reversible and the erasing (clause) form.
    let declared: rewrite_lib::RewriteResult<(
        rewrite_lib::trs::Rule,
        rewrite_lib::relation::BackwardClause,
        Option<rewrite_lib::trs::Rule>,
        rewrite_lib::relation::ConstraintSet,
    )> = amari_rewrite_macros::relation!(exists Y. pair(X, Y) <=> X);
    let (forward, backward, reverse, constraints) = declared.unwrap();
    assert_eq!(forward.rhs(), &amari_rewrite_macros::term!(X));
    assert_eq!(backward.erased_variables(), [String::from("Y")]);
    assert!(reverse.is_none());
    assert!(constraints.is_empty());
}
