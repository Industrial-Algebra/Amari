use amari_rewrite::{
    trs::{Rule, Term, TermSystem},
    RewriteResult,
};

fn main() -> RewriteResult<()> {
    let system = TermSystem::new(vec![Rule::new(
        Term::sym("add", [Term::constant("0"), Term::var("X")]),
        Term::var("X"),
    )?]);

    let term = Term::sym(
        "add",
        [
            Term::constant("0"),
            Term::sym(
                "add",
                [Term::constant("0"), Term::sym("s", [Term::constant("0")])],
            ),
        ],
    );

    let normalized = system.normalize_with_limit(&term, 8)?;
    println!("{term:?} => {normalized:?}");
    assert_eq!(normalized, Term::sym("s", [Term::constant("0")]));
    Ok(())
}
