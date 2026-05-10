use amari_rewrite::{
    inverse::BackwardSearch,
    trs::{Rule, Term, TermSystem},
};

#[test]
fn backward_search_finds_one_step_predecessor() {
    let system = TermSystem::new(vec![Rule::new(
        Term::sym("add", [Term::constant("0"), Term::var("X")]),
        Term::var("X"),
    )
    .unwrap()]);

    let target = Term::constant("a");
    let predecessors: Vec<_> = BackwardSearch::new(&system, target)
        .max_depth(1)
        .max_nodes(16)
        .collect();

    assert!(predecessors.contains(&Term::sym(
        "add",
        [Term::constant("0"), Term::constant("a")]
    )));
}

#[test]
fn backward_search_honors_depth_limit() {
    let system = TermSystem::new(vec![Rule::new(
        Term::sym("add", [Term::constant("0"), Term::var("X")]),
        Term::var("X"),
    )
    .unwrap()]);

    let predecessors: Vec<_> = BackwardSearch::new(&system, Term::constant("a"))
        .max_depth(0)
        .collect();

    assert!(predecessors.is_empty());
}
