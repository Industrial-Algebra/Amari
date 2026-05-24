use amari_rewrite::trs::{match_pattern, Rule, Term};

#[test]
fn match_pattern_binds_variable() {
    let pat = Term::sym("add", [Term::constant("0"), Term::var("X")]);
    let term = Term::sym(
        "add",
        [Term::constant("0"), Term::sym("s", [Term::constant("0")])],
    );
    let subst = match_pattern(&pat, &term).unwrap();
    assert_eq!(subst.get("X"), Some(&Term::sym("s", [Term::constant("0")])));
}

#[test]
fn nonlinear_pattern_requires_consistent_binding() {
    let pat = Term::sym("f", [Term::var("X"), Term::var("X")]);
    assert!(match_pattern(
        &pat,
        &Term::sym("f", [Term::constant("a"), Term::constant("a")])
    )
    .is_some());
    assert!(match_pattern(
        &pat,
        &Term::sym("f", [Term::constant("a"), Term::constant("b")])
    )
    .is_none());
}

#[test]
fn rule_rejects_rhs_variable_missing_from_lhs() {
    let err = Rule::new(Term::var("X"), Term::var("Y")).unwrap_err();
    assert!(err.to_string().contains("rhs variable"));
}

#[test]
fn rule_applies_at_root() {
    let rule = Rule::new(
        Term::sym("add", [Term::constant("0"), Term::var("X")]),
        Term::var("X"),
    )
    .unwrap();
    let term = Term::sym("add", [Term::constant("0"), Term::constant("a")]);
    assert_eq!(rule.apply_root(&term).unwrap(), Term::constant("a"));
}
