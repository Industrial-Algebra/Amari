use amari_rewrite::{
    synthesis::anti_unify,
    trs::{match_pattern, Term},
};

#[test]
fn identical_terms_generalize_to_themselves() {
    let zero = Term::constant("0");
    assert_eq!(anti_unify(&zero, &zero), zero);
}

#[test]
fn nested_terms_generalize_at_disagreement() {
    let a = Term::sym(
        "add",
        [Term::constant("0"), Term::sym("s", [Term::constant("0")])],
    );
    let b = Term::sym(
        "add",
        [
            Term::constant("0"),
            Term::sym("s", [Term::sym("s", [Term::constant("0")])]),
        ],
    );

    let generalized = anti_unify(&a, &b);

    match &generalized {
        Term::Sym(symbol, args) => {
            assert_eq!(symbol.as_str(), "add");
            assert_eq!(args[0], Term::constant("0"));
            assert!(
                matches!(&args[1], Term::Sym(s, inner) if s.as_str() == "s" && matches!(inner[0], Term::Var(_)))
            );
        }
        _ => panic!("expected symbolic generalization"),
    }

    assert!(match_pattern(&generalized, &a).is_some());
    assert!(match_pattern(&generalized, &b).is_some());
}
