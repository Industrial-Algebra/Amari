use amari_rewrite::trs::{Rule, Term, TermSystem};

#[test]
fn peano_add_zero_normalizes_nested_term() {
    let system = TermSystem::new(vec![Rule::new(
        Term::sym("add", [Term::constant("0"), Term::var("X")]),
        Term::var("X"),
    )
    .unwrap()]);

    let term = Term::sym(
        "add",
        [
            Term::constant("0"),
            Term::sym("add", [Term::constant("0"), Term::constant("a")]),
        ],
    );

    assert_eq!(
        system.normalize_with_limit(&term, 8).unwrap(),
        Term::constant("a")
    );
}
