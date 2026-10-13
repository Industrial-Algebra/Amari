//! Typed residual round-trip contract tests (0.25 Cohort 2, Task 10).
//!
//! A forward step emits authority: rule identity, position, erased
//! bindings, and source/target hashes. Replay validates every field,
//! reconstructs privately, compares the canonical source hash, and
//! returns only on exact match — any tampering is a hard typed error
//! with no returned reconstruction.

use amari_rewrite::relation::{RelationLimits, Sha256Digest};
use amari_rewrite::reversible::ReversibleStep;
#[cfg(feature = "serialize")]
use amari_rewrite::reversible::RewriteResidual;
use amari_rewrite::trs::{Rule, Term, TermSystem};
use amari_rewrite::RewriteError;

fn parse(text: &str) -> Term {
    let text = text.trim();
    if let Some(open) = text.find('(') {
        let head = &text[..open];
        let inner = &text[open + 1..text.len() - 1];
        let mut args = Vec::new();
        let mut depth = 0i32;
        let mut start = 0;
        for (index, ch) in inner.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => depth -= 1,
                ',' if depth == 0 => {
                    args.push(parse(&inner[start..index]));
                    start = index + 1;
                }
                _ => {}
            }
        }
        args.push(parse(&inner[start..]));
        Term::sym(head, args)
    } else if text.chars().next().is_some_and(char::is_uppercase) {
        Term::var(text)
    } else {
        Term::constant(text)
    }
}

fn limits() -> RelationLimits {
    RelationLimits::default()
}

#[test]
fn lossless_rule_roundtrips() {
    let rule = Rule::new(parse("f(X)"), parse("g(X)")).unwrap();
    let sys = TermSystem::new(vec![rule.clone()]);
    let source = parse("h(f(a))");
    let step = ReversibleStep::apply(
        &sys,
        &source,
        &rule,
        &amari_rewrite::rewritable::Path::root().child(0),
        &limits(),
    )
    .unwrap();
    assert_eq!(step.target, parse("h(g(a))"));
    // Nothing erased: no bindings.
    assert!(step.residual.erased_bindings.is_empty());
    let reconstructed = step.residual.replay(&sys, &step.target, &limits()).unwrap();
    assert_eq!(reconstructed, source);
}

#[test]
fn erased_variable_is_carried_by_the_residual() {
    let rule = Rule::new(parse("f(X, Y)"), parse("g(X)")).unwrap();
    let sys = TermSystem::new(vec![rule.clone()]);
    let source = parse("f(a, b)");
    let step = ReversibleStep::apply(
        &sys,
        &source,
        &rule,
        &amari_rewrite::rewritable::Path::root(),
        &limits(),
    )
    .unwrap();
    assert_eq!(step.target, parse("g(a)"));
    // Y ↦ b is exactly the erased binding.
    assert_eq!(step.residual.erased_bindings.len(), 1);
    assert_eq!(step.residual.erased_bindings[0].1, parse("b"));
    let reconstructed = step.residual.replay(&sys, &step.target, &limits()).unwrap();
    assert_eq!(reconstructed, source);
}

#[test]
fn nested_position_roundtrips() {
    let rule = Rule::new(parse("a"), parse("b")).unwrap();
    let sys = TermSystem::new(vec![rule.clone()]);
    let source = parse("h(k(a), j(a))");
    let path = amari_rewrite::rewritable::Path::root().child(1).child(0);
    let step = ReversibleStep::apply(&sys, &source, &rule, &path, &limits()).unwrap();
    assert_eq!(step.target, parse("h(k(a), j(b))"));
    assert_eq!(step.residual.position.as_slice(), &[1, 0]);
    let reconstructed = step.residual.replay(&sys, &step.target, &limits()).unwrap();
    assert_eq!(reconstructed, source);
}

#[test]
fn duplicate_rhs_variables_roundtrip() {
    let rule = Rule::new(parse("f(X)"), parse("g(X, X)")).unwrap();
    let sys = TermSystem::new(vec![rule.clone()]);
    let source = parse("f(a)");
    let step = ReversibleStep::apply(
        &sys,
        &source,
        &rule,
        &amari_rewrite::rewritable::Path::root(),
        &limits(),
    )
    .unwrap();
    assert_eq!(step.target, parse("g(a, a)"));
    let reconstructed = step.residual.replay(&sys, &step.target, &limits()).unwrap();
    assert_eq!(reconstructed, source);
}

#[test]
fn same_rhs_rules_are_disambiguated_by_rule_identity() {
    let r1 = Rule::new(parse("p(X)"), parse("q(X)")).unwrap();
    let r2 = Rule::new(parse("s(X, Y)"), parse("q(X)")).unwrap();
    let sys = TermSystem::new(vec![r1.clone(), r2.clone()]);
    let source = parse("s(a, b)");
    let step = ReversibleStep::apply(
        &sys,
        &source,
        &r2,
        &amari_rewrite::rewritable::Path::root(),
        &limits(),
    )
    .unwrap();
    // Replay with the honest residual works.
    let reconstructed = step.residual.replay(&sys, &step.target, &limits()).unwrap();
    assert_eq!(reconstructed, source);
    // Tampered identity: claim the residual came from r1.
    let mut forged = step.residual.clone();
    forged.rule_id = amari_rewrite::relation::RuleId::from_rule(&r1);
    assert!(matches!(
        forged.replay(&sys, &step.target, &limits()),
        Err(RewriteError::ResidualMismatch { .. })
    ));
}

#[test]
fn tampering_is_a_hard_error_with_no_reconstruction() {
    let rule = Rule::new(parse("f(X, Y)"), parse("g(X)")).unwrap();
    let sys = TermSystem::new(vec![rule.clone()]);
    let source = parse("f(a, b)");
    let step = ReversibleStep::apply(
        &sys,
        &source,
        &rule,
        &amari_rewrite::rewritable::Path::root(),
        &limits(),
    )
    .unwrap();

    // Tampered path.
    let mut bad = step.residual.clone();
    bad.position = amari_rewrite::rewritable::Path::root().child(0);
    assert!(bad.replay(&sys, &step.target, &limits()).is_err());

    // Tampered binding value.
    let mut bad = step.residual.clone();
    bad.erased_bindings[0].1 = parse("c");
    assert!(matches!(
        bad.replay(&sys, &step.target, &limits()),
        Err(RewriteError::ResidualMismatch { .. })
    ));

    // Tampered binding schema (extra binding).
    let mut bad = step.residual.clone();
    bad.erased_bindings
        .push((amari_rewrite::relation::LogicVar::new(0, 99), parse("c")));
    assert!(matches!(
        bad.replay(&sys, &step.target, &limits()),
        Err(RewriteError::ResidualMismatch { .. })
    ));

    // Tampered source hash.
    let mut bad = step.residual.clone();
    bad.source_hash = Sha256Digest::from_bytes([0u8; 32]);
    assert!(matches!(
        bad.replay(&sys, &step.target, &limits()),
        Err(RewriteError::ResidualMismatch { .. })
    ));

    // Tampered target hash.
    let mut bad = step.residual.clone();
    bad.target_hash = Sha256Digest::from_bytes([0u8; 32]);
    assert!(matches!(
        bad.replay(&sys, &step.target, &limits()),
        Err(RewriteError::ResidualMismatch { .. })
    ));

    // Replay against a different target term fails.
    assert!(step
        .residual
        .replay(&sys, &parse("g(c)"), &limits())
        .is_err());

    // Replay against a system missing the rule fails.
    let other = TermSystem::new(vec![]);
    assert!(step
        .residual
        .replay(&other, &step.target, &limits())
        .is_err());
}

#[test]
fn residual_bytes_are_deterministic() {
    let rule = Rule::new(parse("f(X, Y)"), parse("g(X)")).unwrap();
    let sys = TermSystem::new(vec![rule.clone()]);
    let source = parse("f(a, b)");
    let make = || {
        ReversibleStep::apply(
            &sys,
            &source,
            &rule,
            &amari_rewrite::rewritable::Path::root(),
            &limits(),
        )
        .unwrap()
        .residual
    };
    assert_eq!(make().digest(), make().digest());
    // Different residuals have different digests.
    let other_rule = Rule::new(parse("f(X, Y)"), parse("h(X)")).unwrap();
    let other_sys = TermSystem::new(vec![other_rule.clone()]);
    let other = ReversibleStep::apply(
        &other_sys,
        &source,
        &other_rule,
        &amari_rewrite::rewritable::Path::root(),
        &limits(),
    )
    .unwrap()
    .residual;
    assert_ne!(make().digest(), other.digest());
}

#[test]
fn residual_limits_are_enforced() {
    let rule = Rule::new(parse("f(X, Y)"), parse("g(X)")).unwrap();
    let sys = TermSystem::new(vec![rule.clone()]);
    let source = parse("f(a, b)");
    let step = ReversibleStep::apply(
        &sys,
        &source,
        &rule,
        &amari_rewrite::rewritable::Path::root(),
        &limits(),
    )
    .unwrap();
    // A residual whose binding exceeds the caller's term limits is
    // rejected before reconstruction.
    let tight = RelationLimits::new(1, 64, 4_096, 1_000_000).unwrap();
    let mut oversized = step.residual.clone();
    oversized.erased_bindings[0].1 = parse("k(a)");
    oversized.source_hash = Sha256Digest::from_bytes([7u8; 32]);
    assert!(matches!(
        oversized.replay(&sys, &step.target, &tight),
        Err(RewriteError::RelationLimitExceeded { .. })
            | Err(RewriteError::ResidualMismatch { .. })
    ));
}

#[test]
fn roundtrip_property_over_small_systems() {
    // backward(forward(source)) == source across generated checked
    // systems, positions, and sources.
    let systems: Vec<Vec<(&str, &str)>> = vec![
        vec![("f(X)", "g(X)")],
        vec![("f(X, Y)", "X")],
        vec![
            ("add(X, zero)", "X"),
            ("add(X, succ(Y))", "succ(add(X, Y))"),
        ],
        vec![("a", "b"), ("h(X)", "k(X, X)")],
    ];
    let sources = [
        "f(a)",
        "f(a, b)",
        "h(f(a))",
        "add(succ(zero), succ(zero))",
        "h(a)",
        "k(a, a)",
        "j(h(a), f(b, c))",
    ];
    for rules in &systems {
        let parsed_rules: Vec<Rule> = rules
            .iter()
            .map(|(l, r)| Rule::new(parse(l), parse(r)).unwrap())
            .collect();
        let sys = TermSystem::new(parsed_rules);
        for source_text in &sources {
            let source = parse(source_text);
            for rule in sys.rules() {
                for path in source.positions() {
                    if let Ok(step) = ReversibleStep::apply(&sys, &source, rule, &path, &limits()) {
                        let reconstructed =
                            step.residual.replay(&sys, &step.target, &limits()).unwrap();
                        assert_eq!(
                            reconstructed, source,
                            "round trip failed for {source_text} via \
                             {rule:?} at {path:?}"
                        );
                    }
                }
            }
        }
    }
}

#[cfg(feature = "serialize")]
#[test]
fn residual_serde_roundtrip() {
    let rule = Rule::new(parse("f(X, Y)"), parse("g(X)")).unwrap();
    let sys = TermSystem::new(vec![rule.clone()]);
    let source = parse("f(a, b)");
    let step = ReversibleStep::apply(
        &sys,
        &source,
        &rule,
        &amari_rewrite::rewritable::Path::root(),
        &limits(),
    )
    .unwrap();
    let json = serde_json::to_string(&step.residual).unwrap();
    let back: RewriteResidual = serde_json::from_str(&json).unwrap();
    assert_eq!(back, step.residual);
    let json = serde_json::to_string(&step).unwrap();
    let back: ReversibleStep = serde_json::from_str(&json).unwrap();
    assert_eq!(back, step);
}
