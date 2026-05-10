use amari_rewrite::{synthesis::infer_rule, trs::Term};

#[test]
fn infer_add_zero_rule_from_positive_examples() {
    let examples = vec![
        (
            Term::sym("add", [Term::constant("0"), Term::constant("a")]),
            Term::constant("a"),
        ),
        (
            Term::sym(
                "add",
                [Term::constant("0"), Term::sym("s", [Term::constant("a")])],
            ),
            Term::sym("s", [Term::constant("a")]),
        ),
    ];

    let rule = infer_rule(&examples).unwrap();

    assert_eq!(rule.apply_root(&examples[0].0).unwrap(), examples[0].1);
    assert_eq!(rule.apply_root(&examples[1].0).unwrap(), examples[1].1);
}

#[test]
fn infer_rule_rejects_empty_examples() {
    assert!(infer_rule(&[]).is_err());
}
