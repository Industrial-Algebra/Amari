// SPDX-License-Identifier: MIT OR Apache-2.0

//! Exact one-step and finite-horizon preimage integration tests
//! (0.25 Cohort 5, Task 25).
//!
//! These tests pin the left-linear construction of ADR 0001 against
//! concrete oracles. They use the application relation
//! ([`TermSystem::application_successors`]), under which the
//! swap-rule self-step is a genuine successor — a divergence from the
//! strict [`TermSystem::successors`] that test 3 pins.

use amari_rewrite::language::{
    finite_horizon_preimage, identity_preimage, one_step_preimage, CertificateAuthority,
    PreimageConstruction, RankedSymbol, TreeAutomaton, TreeAutomatonLimits, TreeState,
    TreeTransition,
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

#[allow(dead_code)]
fn h(inner: Term) -> Term {
    Term::sym("h", [inner])
}

fn x() -> Term {
    Term::var("x")
}

fn y() -> Term {
    Term::var("y")
}

/// Build a (possibly partial) automaton accepting exactly `terms`
/// over the ranked alphabet {a/0, g/1, f/2}. Every node occurrence in
/// every term receives a fresh state `s{n}`, so the accepted language
/// is exactly the listed set; the construction completes the result.
fn automaton_accepting(terms: &[Term]) -> TreeAutomaton {
    let mut transitions: Vec<TreeTransition> = Vec::new();
    let mut finals: Vec<TreeState> = Vec::new();
    let mut counter = 0usize;
    for term in terms {
        let mut states_by_path: std::collections::BTreeMap<Vec<usize>, TreeState> =
            std::collections::BTreeMap::new();
        let mut positions = term.positions();
        positions.sort_by_key(|path| std::cmp::Reverse(path.as_slice().len()));
        for position in &positions {
            let subterm = term.subterm(position).expect("positions are valid");
            let (symbol, arguments) = match subterm {
                Term::Var(_) => panic!("automaton_accepting requires ground terms"),
                Term::Sym(symbol, arguments) => (symbol, arguments),
            };
            counter += 1;
            let state = TreeState::new(format!("s{counter}"));
            let children: Vec<TreeState> = (0..arguments.len())
                .map(|index| {
                    states_by_path
                        .get(position.child(index).as_slice())
                        .cloned()
                        .expect("children are evaluated before their parent")
                })
                .collect();
            transitions.push(TreeTransition::new(symbol.clone(), children, state.clone()));
            states_by_path.insert(position.as_slice().to_vec(), state);
        }
        finals.push(
            states_by_path
                .get(&Vec::<usize>::new())
                .cloned()
                .expect("the root is always evaluated"),
        );
    }
    let mut state_set: std::collections::BTreeSet<TreeState> = std::collections::BTreeSet::new();
    for transition in &transitions {
        state_set.insert(transition.parent().clone());
        state_set.extend(transition.children().iter().cloned());
    }
    TreeAutomaton::new(
        vec![
            RankedSymbol::new(Symbol::new("a"), 0),
            RankedSymbol::new(Symbol::new("g"), 1),
            RankedSymbol::new(Symbol::new("f"), 2),
        ],
        state_set.into_iter().collect(),
        transitions,
        finals,
        TreeAutomatonLimits::default(),
    )
    .expect("fixture automaton is valid")
}

fn accepts(automaton: &TreeAutomaton, term: &Term) -> bool {
    automaton.accepts(term).expect("well-formed ground term")
}

#[test]
fn oracle_p1_single_target() {
    let system = TermSystem::new(vec![Rule::new(g(x()), f(x(), x())).unwrap()]);
    let language = automaton_accepting(&[f(a(), a())]);
    let outcome = one_step_preimage(
        &system,
        &language,
        &RelationLimits::default(),
        &TreeAutomatonLimits::default(),
    )
    .expect("ground one-step is approved");

    assert!(accepts(outcome.automaton(), &g(a())));
    for rejected in [a(), f(a(), a()), g(g(a())), f(g(a()), g(a()))] {
        assert!(!accepts(outcome.automaton(), &rejected), "{rejected:?}");
    }
}

#[test]
fn oracle_p1_two_targets() {
    let system = TermSystem::new(vec![Rule::new(g(x()), f(x(), x())).unwrap()]);
    let language = automaton_accepting(&[f(a(), a()), f(g(a()), g(a()))]);
    let outcome = one_step_preimage(
        &system,
        &language,
        &RelationLimits::default(),
        &TreeAutomatonLimits::default(),
    )
    .expect("ground one-step is approved");

    assert!(accepts(outcome.automaton(), &g(a())));
    assert!(accepts(outcome.automaton(), &g(g(a()))));
    for rejected in [a(), g(g(g(a()))), f(a(), a())] {
        assert!(!accepts(outcome.automaton(), &rejected), "{rejected:?}");
    }
}

#[test]
fn swap_self_step_is_in_the_application_relation() {
    let system = TermSystem::new(vec![Rule::new(f(x(), y()), f(y(), x())).unwrap()]);
    let language = automaton_accepting(&[f(a(), a())]);
    let outcome = one_step_preimage(
        &system,
        &language,
        &RelationLimits::default(),
        &TreeAutomatonLimits::default(),
    )
    .expect("left-linear shared one-step is approved");

    assert!(accepts(outcome.automaton(), &f(a(), a())));

    let application = system
        .application_successors(&f(a(), a()))
        .expect("valid term");
    assert!(
        application.contains(&f(a(), a())),
        "the application relation keeps the self-step"
    );
    let strict = system.successors(&f(a(), a())).expect("valid term");
    assert!(
        !strict.contains(&f(a(), a())),
        "the strict successors relation filters the identity instance"
    );
}

#[test]
fn erased_variable_matches_any_argument() {
    let system = TermSystem::new(vec![Rule::new(f(x(), y()), x()).unwrap()]);
    let language = automaton_accepting(&[a()]);
    let outcome = one_step_preimage(
        &system,
        &language,
        &RelationLimits::default(),
        &TreeAutomatonLimits::default(),
    )
    .expect("left-linear shared one-step is approved");

    for accepted in [f(a(), a()), f(a(), g(a())), f(a(), f(a(), a()))] {
        assert!(accepts(outcome.automaton(), &accepted), "{accepted:?}");
    }
    for rejected in [a(), f(g(a()), a()), g(a())] {
        assert!(!accepts(outcome.automaton(), &rejected), "{rejected:?}");
    }
}

#[test]
fn ground_rule_finite_horizon_matches_oracle() {
    let system = TermSystem::new(vec![Rule::new(a(), g(a())).unwrap()]);
    let language = automaton_accepting(&[g(g(a()))]);
    let limits = RelationLimits::default();
    let automaton_limits = TreeAutomatonLimits::default();

    let one_step = one_step_preimage(&system, &language, &limits, &automaton_limits)
        .expect("ground one-step is approved");
    assert!(accepts(one_step.automaton(), &g(a())));
    assert!(!accepts(one_step.automaton(), &a()));
    assert!(!accepts(one_step.automaton(), &g(g(a()))));

    let horizon = finite_horizon_preimage(&system, &language, 2, &limits, &automaton_limits)
        .expect("ground finite horizon is approved");
    assert!(accepts(horizon.automaton(), &a()));
    assert!(accepts(horizon.automaton(), &g(a())));
    assert!(accepts(horizon.automaton(), &g(g(a()))));
    assert!(!accepts(horizon.automaton(), &g(g(g(a())))));
}

#[test]
fn finite_horizon_zero_is_the_identity() {
    let system = TermSystem::new(vec![Rule::new(f(x(), y()), f(y(), x())).unwrap()]);
    let language = automaton_accepting(&[f(a(), a())]);
    let limits = RelationLimits::default();
    let outcome = finite_horizon_preimage(
        &system,
        &language,
        0,
        &limits,
        &TreeAutomatonLimits::default(),
    )
    .expect("horizon zero is exact for every class");

    assert_eq!(
        outcome.certificate().construction(),
        PreimageConstruction::Identity
    );
    assert!(accepts(outcome.automaton(), &f(a(), a())));
    assert!(!accepts(outcome.automaton(), &a()));
    assert!(!accepts(outcome.automaton(), &g(a())));
    assert!(outcome.certificate().verify(&system, &language, &limits));
    assert!(outcome.certificate().verify_result(&language));
}

#[test]
fn certificate_verifies_against_the_result() {
    let system = TermSystem::new(vec![Rule::new(g(x()), f(x(), x())).unwrap()]);
    let language = automaton_accepting(&[f(a(), a())]);
    let limits = RelationLimits::default();
    let outcome = one_step_preimage(&system, &language, &limits, &TreeAutomatonLimits::default())
        .expect("ground one-step is approved");

    let certificate = outcome.certificate();
    assert!(certificate.is_complete());
    assert_eq!(certificate.authority(), &CertificateAuthority::Exact);
    assert_eq!(
        certificate.construction(),
        PreimageConstruction::LeftLinearOneStep
    );
    assert!(certificate.verify(&system, &language, &limits));
    assert!(certificate.verify_result(outcome.automaton()));

    #[cfg(feature = "serialize")]
    {
        let json = serde_json::to_string(certificate).expect("serializes");
        let restored: amari_rewrite::language::PreimageCertificate =
            serde_json::from_str(&json).expect("deserializes");
        // Deserialization drops the completion claim (Task 24 round-3
        // semantics): pending, still verifiable, no bound result.
        assert!(!restored.is_complete());
        assert!(restored.verify(&system, &language, &limits));
        assert!(!restored.verify_result(outcome.automaton()));
    }
}

#[test]
fn non_left_linear_is_rejected() {
    let system = TermSystem::new(vec![Rule::new(f(x(), x()), x()).unwrap()]);
    let language = automaton_accepting(&[a()]);
    let result = one_step_preimage(
        &system,
        &language,
        &RelationLimits::default(),
        &TreeAutomatonLimits::default(),
    );
    assert!(matches!(
        result,
        Err(RewriteError::UnsupportedPreimage { .. })
    ));
}

#[test]
fn limit_exhaustion_is_a_typed_outcome() {
    let system = TermSystem::new(vec![Rule::new(g(x()), f(x(), x())).unwrap()]);
    let language = automaton_accepting(&[f(a(), a())]);
    let limits = RelationLimits::new(4_096, 64, 1_024, 1).expect("valid tightened limits");
    let result = one_step_preimage(&system, &language, &limits, &TreeAutomatonLimits::default());
    assert!(matches!(
        result,
        Err(RewriteError::RelationLimitExceeded { .. })
    ));
}

#[test]
fn construction_is_deterministic() {
    let system = TermSystem::new(vec![Rule::new(g(x()), f(x(), x())).unwrap()]);
    let language = automaton_accepting(&[f(a(), a())]);
    let limits = RelationLimits::default();
    let automaton_limits = TreeAutomatonLimits::default();

    let first = one_step_preimage(&system, &language, &limits, &automaton_limits)
        .expect("ground one-step is approved");
    let second = one_step_preimage(&system, &language, &limits, &automaton_limits)
        .expect("ground one-step is approved");

    assert_eq!(
        first.automaton().canonical_bytes(),
        second.automaton().canonical_bytes()
    );
    assert_eq!(first.certificate().result(), second.certificate().result());
}

#[test]
fn repetition_on_the_right_reuses_state_values() {
    let system = TermSystem::new(vec![Rule::new(f(x(), y()), a()).unwrap()]);
    let language = automaton_accepting(&[a()]);
    let outcome = one_step_preimage(
        &system,
        &language,
        &RelationLimits::default(),
        &TreeAutomatonLimits::default(),
    )
    .expect("left-linear variable-disjoint one-step is approved");

    assert!(accepts(outcome.automaton(), &f(g(a()), f(a(), a()))));
    assert!(accepts(outcome.automaton(), &f(a(), a())));
    assert!(!accepts(outcome.automaton(), &a()));
    assert!(!accepts(outcome.automaton(), &g(a())));
}

#[test]
fn identity_preimage_returns_the_input() {
    let system = TermSystem::new(vec![Rule::new(g(x()), f(x(), x())).unwrap()]);
    let language = automaton_accepting(&[f(a(), a()), g(a())]);
    let limits = RelationLimits::default();
    let outcome =
        identity_preimage(&system, &language, &limits).expect("identity is exact for every class");

    assert_eq!(
        outcome.certificate().construction(),
        PreimageConstruction::Identity
    );
    assert!(accepts(outcome.automaton(), &f(a(), a())));
    assert!(accepts(outcome.automaton(), &g(a())));
    assert!(!accepts(outcome.automaton(), &a()));
    assert!(!accepts(outcome.automaton(), &g(g(a()))));
    assert!(outcome.certificate().verify(&system, &language, &limits));
    assert!(outcome.certificate().verify_result(&language));
}
