use amari_rewrite::{inverse::BackwardSearch, trs::{Rule, Term, TermSystem}, RewriteResult};

fn main() -> RewriteResult<()> {
    let system = TermSystem::new(vec![
        Rule::new(
            Term::sym("add", [Term::constant("0"), Term::var("X")]),
            Term::var("X"),
        )?,
    ]);

    let target = Term::constant("a");
    let predecessors: Vec<_> = BackwardSearch::new(&system, target.clone())
        .max_depth(1)
        .max_nodes(16)
        .collect();

    println!("predecessors of {target:?}: {predecessors:?}");
    assert!(predecessors.contains(&Term::sym("add", [Term::constant("0"), Term::constant("a")])));
    Ok(())
}
