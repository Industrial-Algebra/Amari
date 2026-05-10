use amari_rewrite::trs::{Substitution, Term};

#[test]
fn term_rewritable_positions_include_all_nodes() {
    let term = Term::sym("add", [Term::constant("0"), Term::var("X")]);
    assert_eq!(term.positions().len(), 3);
}

#[test]
fn substitution_replaces_variables_recursively() {
    let term = Term::sym("add", [Term::constant("0"), Term::var("X")]);
    let subst = Substitution::new().with("X", Term::sym("s", [Term::constant("0")]));
    assert_eq!(
        subst.apply(&term),
        Term::sym("add", [Term::constant("0"), Term::sym("s", [Term::constant("0")])])
    );
}
