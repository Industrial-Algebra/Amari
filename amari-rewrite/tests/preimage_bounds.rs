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
    finite_horizon_lower_bound, finite_horizon_preimage, language_digest, one_step_lower_bound,
    saturation_lower_bound, saturation_preimage, ApproximationEvent, CertificateAuthority,
    RankedSymbol, TreeAutomaton, TreeAutomatonLimits, TreeState, TreeTransition,
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
fn replays_into(
    system: &TermSystem,
    language: &TreeAutomaton,
    term: &Term,
    budget: u32,
    min_steps: u32,
) -> bool {
    let mut visited: BTreeSet<Term> = BTreeSet::new();
    let mut frontier: Vec<Term> = vec![term.clone()];
    visited.insert(term.clone());
    if min_steps == 0 && language.accepts(term).expect("ground term") {
        return true;
    }
    let mut step = 0u32;
    loop {
        if step >= budget || frontier.is_empty() {
            return false;
        }
        let mut next: Vec<Term> = Vec::new();
        for current in &frontier {
            for successor in system
                .application_successors(current)
                .expect("application successors")
            {
                // Genuine self-applications count even when visited.
                if step + 1 >= min_steps && language.accepts(&successor).expect("ground term") {
                    return true;
                }
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

fn assert_sound(
    system: &TermSystem,
    language: &TreeAutomaton,
    lower: &TreeAutomaton,
    budget: u32,
    min_steps: u32,
) {
    for candidate in [a(), b(), c(), g(a()), f(a(), a()), h(a(), a()), g(g(a()))] {
        if lower.accepts(&candidate).expect("ground term") {
            assert!(
                replays_into(system, language, &candidate, budget, min_steps),
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
    // f(a,a), g(g(a))}; the only one-step witness is f(a,a) — the
    // one-step relation is EXACTLY ONE application, so `a` itself is
    // not a witness (review round 1).
    let limits = node_limited(3);
    let outcome =
        one_step_lower_bound(&system, &language, &limits, &TreeAutomatonLimits::default())
            .expect("non-left-linear one-step is approximation-only");

    assert!(!outcome.lower().accepts(&a()).expect("ground"));
    assert!(outcome.lower().accepts(&f(a(), a())).expect("ground"));
    assert!(!outcome.lower().accepts(&g(a())).expect("ground"));
    assert!(!outcome.lower().accepts(&g(g(a()))).expect("ground"));
    assert_sound(&system, &language, outcome.lower(), 1, 1);

    assert!(outcome.upper().is_none());
    let admitted = outcome
        .trace()
        .iter()
        .filter(|event| matches!(event, ApproximationEvent::WitnessAdmitted { .. }))
        .count();
    assert_eq!(admitted, 1, "trace: {:?}", outcome.trace());
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
fn tiny_constraint_budget_truncates_without_error() {
    let system = TermSystem::new(vec![Rule::new(f(x(), x()), a()).unwrap()]);
    let language = accepts_exactly(&[("a", 0), ("f", 2)], &[a()]);
    // A 32-cell storage budget truncates the enumeration after a few
    // retained terms; the small truncated witness set then assembles
    // within the SAME shared per-query pool — the reserved headroom
    // (one eighth of the limit) is what assembly draws down, and
    // operations are generous here, so the outcome is Ok.
    let limits = RelationLimits::new(64, 64, 32, RelationLimits::MAX_OPERATIONS)
        .expect("valid tiny-constraint profile");
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
    assert_sound(&system, &language, outcome.lower(), 1, 1);
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

    assert!(!outcome.lower().accepts(&a()).expect("ground"));
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

/// Review round 1 (P1): the one-step relation is EXACTLY ONE
/// application — no reflexive closure. Under {f(x,x) → a} with
/// L = {a}, `a` has no application successors and is NOT a member of
/// the concrete one-step preimage; the lower bound must not accept
/// it. (Zero-step membership remains correct for finite horizon and
/// saturation.)
#[test]
fn one_step_requires_an_actual_application() {
    let system = TermSystem::new(vec![Rule::new(f(x(), x()), a()).unwrap()]);
    let language = accepts_exactly(&[("a", 0), ("f", 2)], &[a()]);
    let outcome = one_step_lower_bound(
        &system,
        &language,
        &RelationLimits::default(),
        &TreeAutomatonLimits::default(),
    )
    .expect("approximation-only cell");
    assert!(
        !outcome
            .lower()
            .accepts(&a())
            .expect("acceptance is decidable"),
        "a has no application successor in the language — not a one-step witness"
    );
    assert!(
        outcome
            .lower()
            .accepts(&f(a(), a()))
            .expect("acceptance is decidable"),
        "f(a,a) rewrites to a language member in one application"
    );
    let limits = RelationLimits::default();
    assert!(outcome.certificate().verify(&system, &language, &limits));
}

/// Review round 1 (P1): a GENUINE self-application (the rule a → a
/// applies and returns `a`) is an application, so `a` IS a one-step
/// witness of {a} — the fix must not suppress rule-produced
/// self-loops.
#[test]
fn one_step_counts_genuine_self_applications() {
    // The f(x,x) rule keeps the class approximation-only; the a -> a
    // rule is the genuine self-application under test.
    let system = TermSystem::new(vec![
        Rule::new(a(), a()).unwrap(),
        Rule::new(f(x(), x()), b()).unwrap(),
    ]);
    let language = accepts_exactly(&[("a", 0), ("b", 0), ("f", 2)], &[a()]);
    let outcome = one_step_lower_bound(
        &system,
        &language,
        &RelationLimits::default(),
        &TreeAutomatonLimits::default(),
    )
    .expect("approximation-only cell");
    assert!(
        outcome
            .lower()
            .accepts(&a())
            .expect("acceptance is decidable"),
        "a rewrites to itself by a genuine rule application"
    );
}

/// Review round 1 (P1): output assembly is bounded by the caller's
/// operation budget. An unused rank-16 alphabet symbol makes
/// determinization explore an astronomical odometer space BEFORE any
/// state ceiling can fire; with a 200-operation budget the call must
/// return a typed limit error promptly instead of running away.
#[test]
fn assembly_work_is_bounded_by_the_caller_budget() {
    let system = TermSystem::new(vec![Rule::new(f(x(), x()), a()).unwrap()]);
    let q = TreeState::new("q");
    let language = TreeAutomaton::new(
        vec![
            RankedSymbol::new(Symbol::new("a"), 0),
            RankedSymbol::new(Symbol::new("z"), 16),
        ],
        vec![q.clone()],
        vec![TreeTransition::new(Symbol::new("a"), vec![], q.clone())],
        vec![q],
        TreeAutomatonLimits::default(),
    )
    .expect("fixture automaton is valid");
    let tight = RelationLimits::new(4_096, 64, 4_096, 200).expect("valid tight limits");
    let result = one_step_lower_bound(&system, &language, &tight, &TreeAutomatonLimits::default());
    assert!(
        matches!(result, Err(RewriteError::RelationLimitExceeded { .. })),
        "runaway assembly must become a typed limit outcome, got {result:?}"
    );
}

/// Review round 1 (P2): successor work is preflighted before the
/// successor vector is allocated — a 20,000-rule system replaying one
/// candidate against a 20-operation budget truncates immediately
/// (behavioral guard; the allocation itself is instrumented in the
/// reviewer's reproduction).
#[test]
fn successor_work_is_preflighted_before_allocation() {
    let mut rules: Vec<Rule> = (0..20_000).map(|_| Rule::new(a(), b()).unwrap()).collect();
    rules.push(Rule::new(f(x(), x()), a()).unwrap());
    let system = TermSystem::new(rules);
    let language = accepts_exactly(&[("a", 0), ("b", 0), ("f", 2)], &[a()]);
    let tight = RelationLimits::new(4_096, 64, 4_096, 20).expect("valid tight limits");
    let outcome = one_step_lower_bound(&system, &language, &tight, &TreeAutomatonLimits::default())
        .expect("truncation is not an error");
    assert!(
        outcome
            .trace()
            .iter()
            .any(|event| matches!(event, ApproximationEvent::EnumerationTruncated { .. })),
        "the tiny budget must truncate during replay preflight"
    );
}

/// Review round 2 (P2): the successor preflight bills the COMPLETE
/// potential replacement storage — a rule whose right side has 4,096
/// nodes truncates the replay before the 4,096-node successor is ever
/// constructed (behavioral guard; the avoided allocation is
/// instrumented in the reviewer's reproduction).
#[test]
fn huge_right_side_truncates_at_the_storage_preflight() {
    let wide_rhs = Term::sym("z", (0..4_095).map(|_| b()).collect::<Vec<_>>());
    let system = TermSystem::new(vec![
        Rule::new(a(), wide_rhs).unwrap(),
        Rule::new(f(x(), x()), a()).unwrap(),
    ]);
    // The wide symbol appears only on the rule's right side — it
    // never enters a tree automaton (which would reject rank 4,095).
    let language = accepts_exactly(&[("a", 0), ("b", 0), ("f", 2)], &[b()]);
    let tight = RelationLimits::new(4_096, 64, 8, 20).expect("valid tight limits");
    let outcome = one_step_lower_bound(&system, &language, &tight, &TreeAutomatonLimits::default())
        .expect("truncation is not an error");
    assert!(
        outcome
            .trace()
            .iter()
            .any(|event| matches!(event, ApproximationEvent::EnumerationTruncated { .. })),
        "the 4,096-node replacement must trip the storage preflight, trace: {:?}",
        outcome.trace()
    );
}

/// Review round 2 (P2): one per-query budget. Enumeration truncates
/// at the headroom boundary; when the witness set cannot be assembled
/// within the SAME budget (here: the determinization odometer over a
/// rank-16 symbol needs billions of steps), the outcome is a typed
/// limit error — never a second implicit grant.
#[test]
fn assembly_beyond_the_shared_budget_is_typed() {
    let system = TermSystem::new(vec![Rule::new(f(x(), x()), a()).unwrap()]);
    let q = TreeState::new("q");
    let language = TreeAutomaton::new(
        vec![
            RankedSymbol::new(Symbol::new("a"), 0),
            RankedSymbol::new(Symbol::new("z"), 16),
        ],
        vec![q.clone()],
        vec![TreeTransition::new(Symbol::new("a"), vec![], q.clone())],
        vec![q],
        TreeAutomatonLimits::default(),
    )
    .expect("fixture automaton is valid");
    let tight = RelationLimits::new(4_096, 64, 4_096, 512).expect("valid tight limits");
    let result = one_step_lower_bound(&system, &language, &tight, &TreeAutomatonLimits::default());
    assert!(
        matches!(result, Err(RewriteError::RelationLimitExceeded { .. })),
        "assembly beyond the shared per-query budget must be typed, got {result:?}"
    );
}

/// Review round 3 (P2), adversary 1: a variable-duplicating right
/// side. The static right-side node count undercounts the
/// INSTANTIATED replacement (each of 16 variable occurrences expands
/// to a full binding copy), so the preflight must bound substitution
/// expansion AND position multiplicity. At a 256-constraint budget —
/// past the static estimate (~22 cells) but below the conservative
/// bound — the witness `f(g(a), g(a))` is admitted pre-fix and
/// rejected post-fix.
#[test]
fn duplicated_variable_preflight_bounds_substitution_expansion() {
    // R = { f(x,x) -> z(x, ..., x) } with 16 occurrences of x.
    let duplicated_rhs = Term::sym("z", (0..16).map(|_| x()).collect::<Vec<_>>());
    let system = TermSystem::new(vec![Rule::new(f(x(), x()), duplicated_rhs).unwrap()]);
    // L = { z(g(a), ..., g(a)) } — the instantiated successor of
    // f(g(a), g(a)), 33 nodes, all ranks within the ceiling.
    let instantiated = Term::sym("z", (0..16).map(|_| g(a())).collect::<Vec<_>>());
    assert_eq!(term_node_count(&instantiated), 33);
    let language = accepts_exactly(
        &[("a", 0), ("g", 1), ("f", 2), ("z", 16)],
        std::slice::from_ref(&instantiated),
    );
    let witness = f(g(a()), g(a()));
    // Bisected divergence window: at 768 the static-estimate code
    // admits the witness (its true charges fit), while the
    // conservative bound correctly refuses the replay.
    let limits = RelationLimits::new(4_096, 64, 768, 4_096).expect("valid mid limits");
    let outcome =
        one_step_lower_bound(&system, &language, &limits, &TreeAutomatonLimits::default())
            .expect("truncation is not an error");
    assert!(
        outcome
            .trace()
            .iter()
            .any(|event| matches!(event, ApproximationEvent::EnumerationTruncated { .. })),
        "the duplicated-variable replacement must trip the preflight, trace: {:?}",
        outcome.trace()
    );
    assert!(
        !outcome.lower().accepts(&witness).expect("accepts"),
        "the witness must not be admitted: its replacement was never affordable"
    );
    assert!(outcome.certificate().verify(&system, &language, &limits));
    assert!(outcome.certificate().verify_result(outcome.lower()));
}

/// Review round 3 (P2), adversary 2: one rule matching at MULTIPLE
/// positions of the same term emits one instantiated replacement per
/// position. The preflight bounds position multiplicity (positions ×
/// per-position worst case); at a tight budget the replay truncates
/// cleanly. (The accounting gap itself — prebill 108 vs 207 actual —
/// is instrumented in the reviewer's probe; the eventual post-hoc
/// node-count charges reconcile the books.)
#[test]
fn multi_position_rule_preflight_bounds_output_multiplicity() {
    let wide_rhs = Term::sym("z", (0..100).map(|_| b()).collect::<Vec<_>>());
    let system = TermSystem::new(vec![
        Rule::new(a(), wide_rhs).unwrap(),
        Rule::new(f(x(), x()), a()).unwrap(),
    ]);
    let language = accepts_exactly(&[("a", 0), ("b", 0), ("f", 2)], &[a()]);
    let tight = RelationLimits::new(4_096, 64, 16, 4_096).expect("valid tight limits");
    let outcome = one_step_lower_bound(&system, &language, &tight, &TreeAutomatonLimits::default())
        .expect("truncation is not an error");
    assert!(
        outcome
            .trace()
            .iter()
            .any(|event| matches!(event, ApproximationEvent::EnumerationTruncated { .. })),
        "the multi-position successor vector must trip the storage preflight, trace: {:?}",
        outcome.trace()
    );
}

/// Node-count helper for the adversary fixtures above.
fn term_node_count(term: &Term) -> usize {
    let mut nodes = 0usize;
    let mut stack = vec![term];
    while let Some(node) = stack.pop() {
        nodes += 1;
        if let Term::Sym(_, arguments) = node {
            stack.extend(arguments.iter());
        }
    }
    nodes
}

/// Review round 4 (P2): replacing at a deep position REBUILDS the
/// context — every ancestor clones its argument subtree — so the
/// per-position bound now includes the cumulative context work
/// (nodes² dominates: ≤ depth ancestors × whole-term clone). This is
/// the reviewer's nested-context adversary (g^15(a) replay measured
/// 1,360 constructed nodes where the round-3 bound estimated 144
/// operations). The gap is instrumentation-level: within the hard
/// ceilings the witness is unreachable (storage retention binds
/// first) and tight operation budgets are consumed by output
/// assembly, so no black-box admission divergence exists — the
/// reviewer's allocator-instrumented probe measures the prebill
/// directly. This behavioral guard pins that the adversary resolves
/// quickly and typed (never a hang, never an unbounded run).
#[test]
fn deep_context_rebuild_work_is_bounded() {
    let deep = (0..15).fold(a(), |term, _| g(term));
    assert_eq!(term_node_count(&deep), 16);
    let system = TermSystem::new(vec![
        Rule::new(g(x()), a()).unwrap(),
        Rule::new(f(x(), x()), a()).unwrap(),
    ]);
    let language = accepts_exactly(&[("a", 0), ("g", 1), ("f", 2)], &[a()]);
    let result = one_step_lower_bound(
        &system,
        &language,
        &RelationLimits::default(),
        &TreeAutomatonLimits::default(),
    );
    match &result {
        Ok(outcome) => assert!(
            outcome
                .trace()
                .iter()
                .any(|event| matches!(event, ApproximationEvent::EnumerationTruncated { .. })),
            "an Ok outcome must have truncated, trace: {:?}",
            outcome.trace()
        ),
        Err(error) => assert!(
            matches!(error, RewriteError::RelationLimitExceeded { .. }),
            "the only legal failure is a typed limit error, got {error:?}"
        ),
    }
}

/// Review round 5 (P2): substitution clones each binding and then
/// RECONSTRUCTS it into the output, so the construction-work bound
/// doubles the variable-expansion term (x → z(x ×16) applied to `a`
/// constructs 34 nodes where a single-copy bound estimates 20). The
/// doubled context term likewise covers per-position path buffers.
/// Instrumentation-level like round 4 (the reviewer's probe measures
/// the prebill on the public TRS path); this guard pins the exact
/// adversary resolving quickly and typed.
#[test]
fn binding_reconstruction_work_is_bounded() {
    let duplicated_rhs = Term::sym("z", (0..16).map(|_| x()).collect::<Vec<_>>());
    let system = TermSystem::new(vec![
        Rule::new(x(), duplicated_rhs).unwrap(),
        Rule::new(f(x(), x()), a()).unwrap(),
    ]);
    let language = accepts_exactly(&[("a", 0), ("f", 2)], &[a()]);
    let result = one_step_lower_bound(
        &system,
        &language,
        &RelationLimits::default(),
        &TreeAutomatonLimits::default(),
    );
    match &result {
        Ok(outcome) => assert!(
            outcome
                .trace()
                .iter()
                .any(|event| matches!(event, ApproximationEvent::EnumerationTruncated { .. })),
            "an Ok outcome must have truncated, trace: {:?}",
            outcome.trace()
        ),
        Err(error) => assert!(
            matches!(error, RewriteError::RelationLimitExceeded { .. }),
            "the only legal failure is a typed limit error, got {error:?}"
        ),
    }
}

#[test]
fn identity_shortcuts_enforce_supplied_automaton_ceilings() {
    // Cohort 5 closeout F2: the identity constructions (horizon zero,
    // empty-system finite horizon, empty-system saturation) must
    // reject an input automaton that exceeds the CALLER's supplied
    // ceilings, not return it unexamined.
    let two_state_language = TreeAutomaton::new(
        Vec::new(),
        vec![TreeState::new("q0"), TreeState::new("q1")],
        Vec::new(),
        Vec::new(),
        TreeAutomatonLimits::default(),
    )
    .unwrap();
    let tight = TreeAutomatonLimits::new(1, 1, 1).unwrap();
    let limits = RelationLimits::new(16, 8, 16, 128).unwrap();
    let empty = TermSystem::new(Vec::new());

    let outcome = finite_horizon_preimage(&empty, &two_state_language, 0, &limits, &tight);
    assert!(
        matches!(outcome, Err(RewriteError::InvalidLimit { .. })),
        "horizon-0 identity: {outcome:?}"
    );
    let outcome = finite_horizon_preimage(&empty, &two_state_language, 1, &limits, &tight);
    assert!(
        matches!(outcome, Err(RewriteError::InvalidLimit { .. })),
        "empty-system finite horizon: {outcome:?}"
    );
    let outcome = saturation_preimage(&empty, &two_state_language, &limits, &tight);
    assert!(
        matches!(outcome, Err(RewriteError::InvalidLimit { .. })),
        "empty-system saturation: {outcome:?}"
    );

    // Within the ceilings the identity still returns the input
    // representation (digest equality preserved, Task 24).
    let one_state_language = TreeAutomaton::new(
        Vec::new(),
        vec![TreeState::new("q0")],
        Vec::new(),
        Vec::new(),
        TreeAutomatonLimits::default(),
    )
    .unwrap();
    let outcome = finite_horizon_preimage(&empty, &one_state_language, 0, &limits, &tight)
        .expect("identity within ceilings");
    let (automaton, certificate) = outcome.into_parts();
    assert_eq!(
        language_digest(&automaton),
        language_digest(&one_state_language)
    );
    assert!(certificate.verify(&empty, &one_state_language, &limits));
}

#[test]
fn identity_clone_accounts_for_alphabet_storage() {
    // PR #288 round 1: the identity clone retains the ALPHABET too —
    // states + transitions alone undercount the retained buffers.
    let alphabet: Vec<RankedSymbol> = (0..100)
        .map(|i| RankedSymbol::new(Symbol::new(format!("c{i}")), 0))
        .collect();
    let language = TreeAutomaton::new(
        alphabet,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        TreeAutomatonLimits::default(),
    )
    .unwrap();
    let limits = RelationLimits::new(16, 8, 1, 128).unwrap();
    let automaton_limits = TreeAutomatonLimits::default();
    let empty = TermSystem::new(Vec::new());

    for (name, outcome) in [
        (
            "horizon-0",
            finite_horizon_preimage(&empty, &language, 0, &limits, &automaton_limits),
        ),
        (
            "empty-system horizon-1",
            finite_horizon_preimage(&empty, &language, 1, &limits, &automaton_limits),
        ),
        (
            "empty-system saturation",
            saturation_preimage(&empty, &language, &limits, &automaton_limits),
        ),
    ] {
        assert!(
            matches!(outcome, Err(RewriteError::RelationLimitExceeded { .. })),
            "{name}: expected a typed resource-limit error"
        );
    }
}

#[test]
fn identity_clone_accounts_for_finals_storage() {
    // PR #288 round 2: the identity clone retains the finals set as
    // well — the billed cells are states + transitions + alphabet +
    // finals.
    let states: Vec<TreeState> = (0..100)
        .map(|i| TreeState::new(Symbol::new(format!("q{i}"))))
        .collect();
    let language = TreeAutomaton::new(
        Vec::new(),
        states.clone(),
        Vec::new(),
        states,
        TreeAutomatonLimits::default(),
    )
    .unwrap();
    let automaton_limits = TreeAutomatonLimits::default();
    let empty = TermSystem::new(Vec::new());
    let tight = |constraints| RelationLimits::new(16, 8, constraints, 1024).unwrap();

    // 100 states + 100 finals = 200 retained cells: budget 199 must
    // fail, budget 200 must succeed, on every identity route.
    for (name, outcome) in [
        (
            "horizon-0",
            finite_horizon_preimage(&empty, &language, 0, &tight(199), &automaton_limits),
        ),
        (
            "empty-system horizon-1",
            finite_horizon_preimage(&empty, &language, 1, &tight(199), &automaton_limits),
        ),
        (
            "empty-system saturation",
            saturation_preimage(&empty, &language, &tight(199), &automaton_limits),
        ),
    ] {
        assert!(
            matches!(outcome, Err(RewriteError::RelationLimitExceeded { .. })),
            "{name} at budget 199: {outcome:?}"
        );
    }
    for (name, outcome) in [
        (
            "horizon-0",
            finite_horizon_preimage(&empty, &language, 0, &tight(200), &automaton_limits),
        ),
        (
            "empty-system horizon-1",
            finite_horizon_preimage(&empty, &language, 1, &tight(200), &automaton_limits),
        ),
        (
            "empty-system saturation",
            saturation_preimage(&empty, &language, &tight(200), &automaton_limits),
        ),
    ] {
        assert!(outcome.is_ok(), "{name} at budget 200: {outcome:?}");
    }
}
