use amari_rewrite::{synthesis::infer_rule, trs::Term, RewriteResult};

fn main() -> RewriteResult<()> {
    let examples = vec![
        (
            Term::sym("add", [Term::constant("0"), Term::constant("a")]),
            Term::constant("a"),
        ),
        (
            Term::sym("add", [Term::constant("0"), Term::sym("s", [Term::constant("a")])]),
            Term::sym("s", [Term::constant("a")]),
        ),
    ];

    let rule = infer_rule(&examples)?;
    println!("inferred rule: {:?} -> {:?}", rule.lhs(), rule.rhs());

    assert_eq!(rule.apply_root(&examples[0].0).unwrap(), examples[0].1);
    assert_eq!(rule.apply_root(&examples[1].0).unwrap(), examples[1].1);
    Ok(())
}
