// SPDX-License-Identifier: MIT OR Apache-2.0

//! Witnessed lower-bound integration tests (0.25 Cohort 5, Task 27).
//!
//! These pin the only ADR-0001-approved under-approximation: enumerate
//! ground candidates over the candidate alphabet and retain exactly the
//! terms whose forward replay (application relation, self-steps
//! included) reaches the input language within the step budget. The
//! upper bound is always `None`; the certificate carries `Partial`
//! authority. Soundness is asserted directly by replaying every term the
//! lower automaton accepts.

use std::collections::{BTreeMap, BTreeSet};

#[cfg(feature = "serialize")]
use amari_rewrite::language::PreimageCertificate;
use amari_rewrite::language::{
    finite_horizon_lower_bound, one_step_lower_bound, saturation_lower_bound, ApproximationEvent,
    CertificateAuthority, RankedSymbol, TreeAutomaton, TreeAutomatonLimits, TreeState,
    TreeTransition,
};
use amari_rewrite::relation::RelationLimits;
use amari_rewrite::trs::{Rule, Symbol, Term, TermSystem};
use amari_rewrite::RewriteError;

fn a() -> Term {
    Term::constant("a")
}

fn b() -> Term {
    Term::constant("b")
}

fn c() -> Term {
    Term::constant("c")
}

fn f(left: Term, right: Term) -> Term {
    Term::sym("f", [left, right])
}

fn g(inner: Term) -> Term {
    Term::sym("g", [inner])
}

fn h(left: Term, right: Term) -> Term {
    Term::sym("h", [left, right])
}

fn x() -> Term {
    Term::var("x")
}

fn y() -> Term {
    Term::var("y")
}

/// Build a partial automaton accepting exactly `terms` over the ranked
/// alphabet described by `ranked`. Each node occurrence receives a fresh
/// state, so the accepted language is exactly the listed set; the lower
/// construction completes the automaton.
fn accepts_exactly(ranked: &[(&str, u16)], terms: &[Term]) -> TreeAutomaton {
    let alphabet: Vec<RankedSymbol> = ranked
        .iter()
        .map(|(name, arity)| RankedSymbol::new(Symbol::new(*name), *arity))
        .collect();
    let mut transitions: Vec<TreeTransition> = Vec::new();
    let mut finals: Vec<TreeState> = Vec::new();
    let mut counter = 0usize;
    for term in terms {
        let mut states_by_path: BTreeMap<Vec<usize>, TreeState> = BTreeMap::new();
        let mut positions = term.positions();
        positions.sort_by_key(|path| std::cmp::Reverse(path.as_slice().len()));
        for position in &positions {
            let subterm = term.subterm(position).expect("positions are valid");
            let (symbol, arguments) = match subterm {
                Term::Var(_) => panic!("language fixtures are ground"),
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
                .get(Vec::<usize>::new().as_slice())
                .cloned()
                .expect("the root is always evaluated"),
        );
    }
    let mut state_set: BTreeSet<TreeState> = BTreeSet::new();
    for transition in &transitions {
        state_set.insert(transition.parent().clone());
        state_set.extend(transition.children().iter().cloned());
    }
    TreeAutomaton::new(
        alphabet,
        state_set.into_iter().collect(),
        transitions,
        finals,
        TreeAutomatonLimits::default(),
    )
    .expect("fixture automaton is valid")
}

/// The empty language over `ranked`.
fn empty_language(ranked: &[(&str, u16)]) -> TreeAutomaton {
    let alphabet: Vec<RankedSymbol> = ranked
        .iter()
        .map(|(name, arity)| RankedSymbol::new(Symbol::new(*name), *arity))
        .collect();
    TreeAutomaton::new(
        alphabet,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        TreeAutomatonLimits::default(),
    )
    .expect("empty fixture automaton is valid")
}

/// Whether `term` replays into `language` within `budget` application
/// steps, using the same relation the construction uses.
fn replays_into(system: &TermSystem, language: &TreeAutomaton, term: &Term, budget: u32) -> bool {
    let mut visited: BTreeSet<Term> = BTreeSet::new();
    let mut frontier: Vec<Term> = vec![term.clone()];
    visited.insert(term.clone());
    let mut step = 0u32;
    loop {
        for current in &frontier {
            if language.accepts(current).expect("ground term") {
                return true;
            }
        }
        if step >= budget || frontier.is_empty() {
            return false;
        }
        let mut next: Vec<Term> = Vec::new();
        for current in &frontier {
            for successor in system
                .application_successors(current)
                .expect("application successors")
            {
                if visited.insert(successor.clone()) {
                    next.push(successor);
                }
            }
        }
        frontier = next;
        step += 1;
    }
}

fn node_limited(max_nodes: usize) -> RelationLimits {
    RelationLimits::new(
        max_nodes,
        64,
        RelationLimits::MAX_CONSTRAINTS,
        RelationLimits::MAX_OPERATIONS,
    )
    .expect("valid node-limited profile")
}

fn assert_sound(system: &TermSystem, language: &TreeAutomaton, lower: &TreeAutomaton, budget: u32) {
    for candidate in [a(), b(), c(), g(a()), f(a(), a()), h(a(), a()), g(g(a()))] {
        if lower.accepts(&candidate).expect("ground term") {
            assert!(
                replays_into(system, language, &candidate, budget),
                "accepted witness {candidate:?} must replay into the language"
            );
        }
    }
}

#[test]
fn non_left_linear_one_step_lower_bound() {
    let system = TermSystem::new(vec![Rule::new(f(x(), x()), a()).unwrap()]);
    let language = accepts_exactly(&[("a", 0), ("g", 1), ("f", 2)], &[a()]);
    // A three-node budget makes the enumerated set exactly {a, g(a),
    // f(a,a), g(g(a))}, so the only one-step witnesses are a (zero
    // steps) and f(a,a) (the rule).
    let limits = node_limited(3);
    let outcome =
        one_step_lower_bound(&system, &language, &limits, &TreeAutomatonLimits::default())
            .expect("non-left-linear one-step is approximation-only");

    assert!(outcome.lower().accepts(&a()).expect("ground"));
    assert!(outcome.lower().accepts(&f(a(), a())).expect("ground"));
    assert!(!outcome.lower().accepts(&g(a())).expect("ground"));
    assert!(!outcome.lower().accepts(&g(g(a()))).expect("ground"));
    assert_sound(&system, &language, outcome.lower(), 1);

    assert!(outcome.upper().is_none());
    let admitted = outcome
        .trace()
        .iter()
        .filter(|event| matches!(event, ApproximationEvent::WitnessAdmitted { .. }))
        .count();
    assert_eq!(admitted, 2, "trace: {:?}", outcome.trace());
    let upper_unavailable = outcome
        .trace()
        .iter()
        .filter(|event| matches!(event, ApproximationEvent::UpperUnavailable { .. }))
        .count();
    assert_eq!(upper_unavailable, 1, "trace: {:?}", outcome.trace());

    assert!(matches!(
        outcome.certificate().authority(),
        CertificateAuthority::Partial { .. }
    ));
    assert!(outcome.certificate().verify(&system, &language, &limits));
    assert!(outcome.certificate().verify_result(outcome.lower()));
}

#[test]
fn class_gate_rejects_exact_approved_cells() {
    let language = accepts_exactly(&[("a", 0), ("g", 1), ("f", 2)], &[f(a(), a())]);
    let limits = RelationLimits::default();

    // Left-linear one-step is exact.
    let left_linear = TermSystem::new(vec![Rule::new(g(x()), f(x(), x())).unwrap()]);
    assert!(matches!(
        one_step_lower_bound(
            &left_linear,
            &language,
            &limits,
            &TreeAutomatonLimits::default()
        ),
        Err(RewriteError::UnsupportedPreimage { .. })
    ));

    // Right-ground saturation is exact.
    let right_ground = TermSystem::new(vec![Rule::new(f(x(), y()), a()).unwrap()]);
    assert!(matches!(
        saturation_lower_bound(
            &right_ground,
            &language,
            4,
            &limits,
            &TreeAutomatonLimits::default()
        ),
        Err(RewriteError::UnsupportedPreimage { .. })
    ));

    // Horizon zero is the exact identity for every class.
    assert!(matches!(
        finite_horizon_lower_bound(
            &left_linear,
            &language,
            0,
            &limits,
            &TreeAutomatonLimits::default()
        ),
        Err(RewriteError::UnsupportedPreimage { .. })
    ));
}

#[test]
fn empty_witness_set_yields_empty_lower() {
    // A language with no members: no candidate can replay into it, and
    // the produced lower bound is the empty language.
    let system = TermSystem::new(vec![Rule::new(f(x(), x()), b()).unwrap()]);
    let language = empty_language(&[("a", 0), ("b", 0), ("f", 2)]);
    let limits = node_limited(3);
    let outcome =
        one_step_lower_bound(&system, &language, &limits, &TreeAutomatonLimits::default())
            .expect("non-left-linear one-step is approximation-only");

    assert!(outcome.lower().language_is_empty());
    assert!(!outcome
        .trace()
        .iter()
        .any(|event| matches!(event, ApproximationEvent::WitnessAdmitted { .. })));
}

#[test]
fn lower_bound_is_monotone_in_the_node_budget() {
    let system = TermSystem::new(vec![Rule::new(f(x(), x()), a()).unwrap()]);
    let language = accepts_exactly(&[("a", 0), ("g", 1), ("f", 2)], &[a()]);
    let small = node_limited(3);
    let large = RelationLimits::default();
    let small_outcome =
        one_step_lower_bound(&system, &language, &small, &TreeAutomatonLimits::default())
            .expect("approximation-only");
    let large_outcome =
        one_step_lower_bound(&system, &language, &large, &TreeAutomatonLimits::default())
            .expect("approximation-only");

    for candidate in [a(), g(a()), f(a(), a()), g(g(a())), f(g(a()), g(a()))] {
        if small_outcome.lower().accepts(&candidate).expect("ground") {
            assert!(
                large_outcome.lower().accepts(&candidate).expect("ground"),
                "small-budget witness {candidate:?} must survive the larger budget"
            );
        }
    }
}

#[test]
fn finite_horizon_needs_exactly_two_steps() {
    // g(a) -> f(a,a) -> a and g(g(a)) -> f(g(a),g(a)) -> a need exactly
    // two application steps.
    let system = TermSystem::new(vec![
        Rule::new(f(x(), x()), a()).unwrap(),
        Rule::new(g(x()), f(x(), x())).unwrap(),
    ]);
    let language = accepts_exactly(&[("a", 0), ("f", 2), ("g", 1)], &[a()]);
    // The two-step witness g(g(a)) replays through the five-node
    // intermediate f(g(a),g(a)), so the term budget must admit it.
    let limits = node_limited(5);

    let horizon_one = finite_horizon_lower_bound(
        &system,
        &language,
        1,
        &limits,
        &TreeAutomatonLimits::default(),
    )
    .expect("non-left-linear finite horizon is approximation-only");
    let horizon_two = finite_horizon_lower_bound(
        &system,
        &language,
        2,
        &limits,
        &TreeAutomatonLimits::default(),
    )
    .expect("non-left-linear finite horizon is approximation-only");

    assert!(horizon_one.lower().accepts(&a()).expect("ground"));
    assert!(horizon_one.lower().accepts(&f(a(), a())).expect("ground"));
    assert!(!horizon_one.lower().accepts(&g(a())).expect("ground"));
    assert!(!horizon_one.lower().accepts(&g(g(a()))).expect("ground"));

    assert!(horizon_two.lower().accepts(&g(a())).expect("ground"));
    assert!(horizon_two.lower().accepts(&g(g(a()))).expect("ground"));
}

#[test]
fn tiny_operation_budget_truncates_without_error() {
    let system = TermSystem::new(vec![Rule::new(f(x(), x()), a()).unwrap()]);
    let language = accepts_exactly(&[("a", 0), ("f", 2)], &[a()]);
    let limits = RelationLimits::new(64, 64, RelationLimits::MAX_CONSTRAINTS, 3)
        .expect("valid tiny-operation profile");
    let outcome =
        one_step_lower_bound(&system, &language, &limits, &TreeAutomatonLimits::default())
            .expect("budget exhaustion during enumeration is not an error");

    assert!(
        outcome
            .trace()
            .iter()
            .any(|event| { matches!(event, ApproximationEvent::EnumerationTruncated { .. }) }),
        "trace: {:?}",
        outcome.trace()
    );
    assert_sound(&system, &language, outcome.lower(), 1);
}

#[cfg(feature = "serialize")]
#[test]
fn certificate_round_trips_to_a_pending_partial() {
    let system = TermSystem::new(vec![Rule::new(f(x(), x()), a()).unwrap()]);
    let language = accepts_exactly(&[("a", 0), ("g", 1), ("f", 2)], &[a()]);
    let limits = node_limited(3);
    let outcome =
        one_step_lower_bound(&system, &language, &limits, &TreeAutomatonLimits::default())
            .expect("approximation-only");

    let json = serde_json::to_string(outcome.certificate()).expect("serializes");
    let restored: PreimageCertificate = serde_json::from_str(&json).expect("deserializes");
    assert!(!restored.is_complete());
    assert!(matches!(
        restored.authority(),
        CertificateAuthority::Partial { .. }
    ));
    assert!(restored.verify(&system, &language, &limits));
}

#[test]
fn verification_rejects_perturbed_bindings() {
    let system = TermSystem::new(vec![Rule::new(f(x(), x()), a()).unwrap()]);
    let language = accepts_exactly(&[("a", 0), ("g", 1), ("f", 2)], &[a()]);
    let limits = node_limited(3);
    let outcome =
        one_step_lower_bound(&system, &language, &limits, &TreeAutomatonLimits::default())
            .expect("approximation-only");
    assert!(outcome.certificate().verify(&system, &language, &limits));

    let perturbed_system = TermSystem::new(vec![Rule::new(f(x(), x()), g(a())).unwrap()]);
    assert!(!outcome
        .certificate()
        .verify(&perturbed_system, &language, &limits));

    let perturbed_language = accepts_exactly(&[("a", 0), ("g", 1), ("f", 2)], &[g(a())]);
    assert!(!outcome
        .certificate()
        .verify(&system, &perturbed_language, &limits));
}

#[test]
fn saturation_lower_bound_respects_the_step_budget() {
    // a -> b -> c and a shared-variable rule make the class
    // LeftLinearShared, which the ADR approves no exact saturation for.
    let system = TermSystem::new(vec![
        Rule::new(g(x()), f(x(), x())).unwrap(),
        Rule::new(a(), b()).unwrap(),
        Rule::new(b(), c()).unwrap(),
    ]);
    let language = accepts_exactly(&[("a", 0), ("b", 0), ("c", 0), ("f", 2), ("g", 1)], &[c()]);
    let limits = node_limited(3);

    let one_step = saturation_lower_bound(
        &system,
        &language,
        1,
        &limits,
        &TreeAutomatonLimits::default(),
    )
    .expect("left-linear shared saturation is approximation-only");
    let two_steps = saturation_lower_bound(
        &system,
        &language,
        2,
        &limits,
        &TreeAutomatonLimits::default(),
    )
    .expect("left-linear shared saturation is approximation-only");

    assert!(one_step.lower().accepts(&c()).expect("ground"));
    assert!(one_step.lower().accepts(&b()).expect("ground"));
    assert!(!one_step.lower().accepts(&a()).expect("ground"));

    assert!(two_steps.lower().accepts(&a()).expect("ground"));
    assert!(two_steps.lower().accepts(&b()).expect("ground"));
    assert!(two_steps.lower().accepts(&c()).expect("ground"));
}

#[test]
fn witness_symbol_outside_the_language_alphabet() {
    // The rule left side uses h/2, which the language alphabet does not
    // mention; the candidate alphabet unions it in and the replayed
    // witness h(a,a) appears in the lower automaton.
    let system = TermSystem::new(vec![Rule::new(h(x(), x()), a()).unwrap()]);
    let language = accepts_exactly(&[("a", 0)], &[a()]);
    let limits = node_limited(3);
    let outcome =
        one_step_lower_bound(&system, &language, &limits, &TreeAutomatonLimits::default())
            .expect("non-left-linear one-step is approximation-only");

    assert!(outcome.lower().accepts(&a()).expect("ground"));
    assert!(outcome.lower().accepts(&h(a(), a())).expect("ground"));
    assert!(language
        .alphabet()
        .iter()
        .all(|ranked| ranked.symbol().as_str() != "h"));
    assert!(
        outcome
            .lower()
            .alphabet()
            .iter()
            .any(|ranked| ranked.symbol().as_str() == "h" && ranked.arity() == 2),
        "alphabet: {:?}",
        outcome.lower().alphabet()
    );
    assert!(outcome.certificate().verify(&system, &language, &limits));
    assert!(outcome.certificate().verify_result(outcome.lower()));
}
