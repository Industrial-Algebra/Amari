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

/// Round-2 P1 (alphabet rebinding): a deserialized certificate whose
/// language/result digests were rebound to an automaton whose alphabet
/// does NOT cover the system's symbols must not verify — `verify`
/// re-runs the ADR 0001 common-alphabet validation.
#[cfg(feature = "serialize")]
#[test]
fn alphabet_rebinding_fails_verification() {
    let system = TermSystem::new(vec![Rule::new(f(a(), a()), a()).unwrap()]);
    let language = universal_automaton();
    let small = small_automaton();
    let limits = RelationLimits::default();
    assert!(validate_alphabet(&system, &small).is_err());

    let certificate =
        PreimageCertificate::issue(PreimageOperation::OneStep, &system, &language, &limits)
            .expect("approved cell");
    let mut wire = serde_json::to_value(&certificate).expect("serializes");
    wire["language"] = serde_json::to_value(language_digest(&small)).unwrap();
    wire["result"] = serde_json::to_value(Some(language_digest(&small))).unwrap();
    let rebound: PreimageCertificate = serde_json::from_value(wire).expect("deserializes");
    assert!(!rebound.verify(&system, &small, &limits));
    // verify_result is compositional (it does not see the
    // system/language pair): full validation is the conjunction, and
    // the rebound certificate fails it.
    assert!(!(rebound.verify(&system, &small, &limits) && rebound.verify_result(&small)));
}

/// Round-3 P1: deserialization must not produce completed `Exact`
/// evidence. A forged certificate — pending issue, then the wire
/// `result` field replaced with the universal-language digest — must
/// arrive PENDING (completion claims do not cross trust boundaries).
/// The forged claim here is mathematically false: the one-step
/// preimage of the empty language is empty, not universal.
#[cfg(feature = "serialize")]
#[test]
fn deserialized_certificates_are_pending() {
    // Empty language over {a/0, g/1}: g(q)->q with no constant
    // transition accepts no ground term.
    let q = TreeState::new("q");
    let empty_language = TreeAutomaton::new(
        vec![
            RankedSymbol::new(Symbol::new("a"), 0),
            RankedSymbol::new(Symbol::new("g"), 1),
        ],
        vec![q.clone()],
        vec![TreeTransition::new(
            Symbol::new("g"),
            vec![q.clone()],
            q.clone(),
        )],
        vec![q],
        TreeAutomatonLimits::default(),
    )
    .expect("fixture automaton is valid");
    assert!(empty_language.language_is_empty());

    let system = TermSystem::new(vec![Rule::new(a(), g(a())).unwrap()]);
    let limits = RelationLimits::default();
    let certificate = PreimageCertificate::issue(
        PreimageOperation::OneStep,
        &system,
        &empty_language,
        &limits,
    )
    .expect("ground one-step is approved");

    let mut wire = serde_json::to_value(&certificate).expect("serializes");
    wire["result"] = serde_json::to_value(Some(language_digest(&universal_automaton()))).unwrap();
    let forged: PreimageCertificate = serde_json::from_value(wire).expect("deserializes");

    assert!(!forged.is_complete());
    assert!(!forged.verify_result(&universal_automaton()));
    // The pending certificate itself remains valid: its parameters
    // describe an approved cell.
    assert!(forged.verify(&system, &empty_language, &limits));
}
