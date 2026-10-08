// SPDX-License-Identifier: MIT OR Apache-2.0

//! System/language pair validation and evidence-certificate suites
//! (0.25 Cohort 5, Task 24).

use amari_rewrite::language::{
    language_digest, system_digest, validate_alphabet, CertificateAuthority, PreimageCertificate,
    PreimageConstruction, PreimageOperation, RankedSymbol, TreeAutomaton, TreeAutomatonLimits,
    TreeState, TreeTransition, TrsClass,
};
use amari_rewrite::relation::RelationLimits;
use amari_rewrite::trs::{Rule, Symbol, Term, TermSystem};
use amari_rewrite::RewriteError;

fn a() -> Term {
    Term::constant("a")
}

fn f(left: Term, right: Term) -> Term {
    Term::sym("f", [left, right])
}

fn g(inner: Term) -> Term {
    Term::sym("g", [inner])
}

fn h(inner: Term) -> Term {
    Term::sym("h", [inner])
}

fn x() -> Term {
    Term::var("x")
}

fn y() -> Term {
    Term::var("y")
}

/// The universal automaton over {a/0, g/1, f/2}: one state, all terms.
fn universal_automaton() -> TreeAutomaton {
    let q = TreeState::new("q");
    TreeAutomaton::new(
        vec![
            RankedSymbol::new(Symbol::new("a"), 0),
            RankedSymbol::new(Symbol::new("g"), 1),
            RankedSymbol::new(Symbol::new("f"), 2),
        ],
        vec![q.clone()],
        vec![
            TreeTransition::new(Symbol::new("a"), vec![], q.clone()),
            TreeTransition::new(Symbol::new("g"), vec![q.clone()], q.clone()),
            TreeTransition::new(Symbol::new("f"), vec![q.clone(), q.clone()], q.clone()),
        ],
        vec![q],
        TreeAutomatonLimits::default(),
    )
    .expect("fixture automaton is valid")
}

/// A different automaton (alphabet {a/0, g/1} only), for binding checks.
fn small_automaton() -> TreeAutomaton {
    let q = TreeState::new("q");
    TreeAutomaton::new(
        vec![
            RankedSymbol::new(Symbol::new("a"), 0),
            RankedSymbol::new(Symbol::new("g"), 1),
        ],
        vec![q.clone()],
        vec![
            TreeTransition::new(Symbol::new("a"), vec![], q.clone()),
            TreeTransition::new(Symbol::new("g"), vec![q.clone()], q.clone()),
        ],
        vec![q],
        TreeAutomatonLimits::default(),
    )
    .expect("fixture automaton is valid")
}

#[test]
fn alphabet_validation_accepts_matching_symbols() {
    let system = TermSystem::new(vec![Rule::new(g(x()), f(x(), x())).unwrap()]);
    validate_alphabet(&system, &universal_automaton()).expect("matching alphabet validates");
}

#[test]
fn alphabet_validation_rejects_unknown_symbol() {
    let system = TermSystem::new(vec![Rule::new(h(x()), a()).unwrap()]);
    match validate_alphabet(&system, &universal_automaton()) {
        Err(RewriteError::UnsupportedPreimage { message }) => {
            assert!(message.contains("rule 0"), "message: {message}");
            assert!(message.contains('h'), "message: {message}");
        }
        other => panic!("expected UnsupportedPreimage, got {other:?}"),
    }
}

#[test]
fn alphabet_validation_rejects_arity_mismatch() {
    // f is binary in the language; the rule uses it unary.
    let system = TermSystem::new(vec![Rule::new(f(x(), a()), a()).unwrap()]);
    assert!(validate_alphabet(&system, &universal_automaton()).is_ok());
    let unary_f = Term::sym("f", [x()]);
    let system = TermSystem::new(vec![Rule::new(unary_f, a()).unwrap()]);
    match validate_alphabet(&system, &universal_automaton()) {
        Err(RewriteError::UnsupportedPreimage { message }) => {
            assert!(message.contains("arity 1"), "message: {message}");
        }
        other => panic!("expected UnsupportedPreimage, got {other:?}"),
    }
}

#[test]
fn issue_binds_inputs_for_approved_classes() {
    let system = TermSystem::new(vec![Rule::new(g(x()), f(x(), x())).unwrap()]);
    let language = universal_automaton();
    let certificate = PreimageCertificate::issue(
        PreimageOperation::OneStep,
        &system,
        &language,
        &RelationLimits::default(),
    )
    .expect("left-linear one-step is approved");
    assert_eq!(certificate.class(), TrsClass::LeftLinearShared);
    assert_eq!(
        certificate.construction(),
        PreimageConstruction::LeftLinearOneStep
    );
    assert_eq!(certificate.operation(), PreimageOperation::OneStep);
    assert_eq!(certificate.horizon(), None);
    assert_eq!(certificate.authority(), &CertificateAuthority::Exact);
    assert!(!certificate.is_complete());
    assert_eq!(certificate.result(), None);
    assert_eq!(certificate.system(), system_digest(&system));
    assert_eq!(certificate.language(), language_digest(&language));
}

#[test]
fn issue_refuses_approximation_only_cells() {
    let non_left_linear = TermSystem::new(vec![Rule::new(f(x(), x()), a()).unwrap()]);
    let shared = TermSystem::new(vec![Rule::new(g(x()), f(x(), x())).unwrap()]);
    let language = universal_automaton();
    let limits = RelationLimits::default();
    assert!(matches!(
        PreimageCertificate::issue(
            PreimageOperation::OneStep,
            &non_left_linear,
            &language,
            &limits
        ),
        Err(RewriteError::UnsupportedPreimage { .. })
    ));
    assert!(matches!(
        PreimageCertificate::issue(PreimageOperation::Saturation, &shared, &language, &limits),
        Err(RewriteError::UnsupportedPreimage { .. })
    ));
    assert!(matches!(
        PreimageCertificate::issue(
            PreimageOperation::Saturation,
            &non_left_linear,
            &language,
            &limits
        ),
        Err(RewriteError::UnsupportedPreimage { .. })
    ));
}

#[test]
fn horizon_zero_is_exact_for_every_class() {
    let non_left_linear = TermSystem::new(vec![Rule::new(f(x(), x()), a()).unwrap()]);
    let certificate = PreimageCertificate::issue(
        PreimageOperation::FiniteHorizon(0),
        &non_left_linear,
        &universal_automaton(),
        &RelationLimits::default(),
    )
    .expect("horizon zero is exact for every class");
    assert_eq!(certificate.class(), TrsClass::NonLeftLinear);
    assert_eq!(certificate.construction(), PreimageConstruction::Identity);
    assert_eq!(certificate.horizon(), Some(0));
}

#[test]
fn issue_revalidates_the_rule_contract() {
    let system = TermSystem::new(vec![Rule::new_unchecked(g(x()), y())]);
    match PreimageCertificate::issue(
        PreimageOperation::OneStep,
        &system,
        &universal_automaton(),
        &RelationLimits::default(),
    ) {
        Err(RewriteError::InvalidRule { .. }) => {}
        other => panic!("expected InvalidRule, got {other:?}"),
    }
}

#[test]
fn complete_and_verify_round_trip() {
    let system = TermSystem::new(vec![Rule::new(a(), g(a())).unwrap()]);
    let language = universal_automaton();
    let limits = RelationLimits::default();
    let certificate =
        PreimageCertificate::issue(PreimageOperation::Saturation, &system, &language, &limits)
            .expect("ground saturation is approved");
    assert_eq!(certificate.class(), TrsClass::Ground);
    assert_eq!(
        certificate.construction(),
        PreimageConstruction::GttSaturation
    );

    let certificate = certificate.complete(&language);
    assert!(certificate.is_complete());
    assert!(certificate.verify(&system, &language, &limits));
    assert!(certificate.verify_result(&language));

    // A different system does not verify.
    let other_system = TermSystem::new(vec![
        Rule::new(a(), g(a())).unwrap(),
        Rule::new(g(a()), a()).unwrap(),
    ]);
    assert!(!certificate.verify(&other_system, &language, &limits));

    // A different result does not verify.
    assert!(!certificate.verify_result(&small_automaton()));

    // Tightened limits do not verify.
    let tighter = RelationLimits::new(4096, 64, limits.max_constraints(), 500_000)
        .expect("valid tightened limits");
    assert!(!certificate.verify(&system, &language, &tighter));
}

#[test]
fn digests_are_deterministic_and_rule_order_insensitive() {
    let rules_a = vec![
        Rule::new(a(), g(a())).unwrap(),
        Rule::new(g(x()), a()).unwrap(),
    ];
    let rules_b = vec![
        Rule::new(g(x()), a()).unwrap(),
        Rule::new(a(), g(a())).unwrap(),
    ];
    assert_eq!(
        system_digest(&TermSystem::new(rules_a.clone())),
        system_digest(&TermSystem::new(rules_b)),
        "rule order must not change the binding"
    );
    let different = vec![Rule::new(a(), g(g(a()))).unwrap()];
    assert_ne!(
        system_digest(&TermSystem::new(rules_a)),
        system_digest(&TermSystem::new(different))
    );
    assert_eq!(
        language_digest(&universal_automaton()),
        language_digest(&universal_automaton())
    );
    assert_ne!(
        language_digest(&universal_automaton()),
        language_digest(&small_automaton())
    );
}
