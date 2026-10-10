// SPDX-License-Identifier: MIT OR Apache-2.0

//! Three-valued preimage membership and monotone refinement
//! integration tests (0.25 Cohort 5, Task 28).
//!
//! These pin the soundness contract of [`classify_and_query`]:
//! `Proven` requires an actual witness (membership in an exact
//! preimage automaton or in the Task 27 witnessed lower bound),
//! `Excluded` requires a COMPLETE construction for the cell, and
//! everything else — including any truncated approximation and any
//! term outside the enumerated domain — is `Unknown`.
//!
//! They also pin [`refine_lower_bound`]: a refinement re-runs the
//! witnessed lower bound under new limits and refuses, with a typed
//! error, any result whose language does not contain the prior one.

use std::collections::{BTreeMap, BTreeSet};

use amari_rewrite::language::{
    classify_and_query, finite_horizon_lower_bound, one_step_lower_bound, refine_lower_bound,
    ApproximationEvent, CertificateAuthority, MembershipVerdict, PreimageOperation, RankedSymbol,
    TreeAutomaton, TreeAutomatonLimits, TreeState, TreeTransition,
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

/// Build a partial automaton accepting exactly `terms` over the ranked
/// alphabet described by `ranked`. Each node occurrence receives a
/// fresh state, so the accepted language is exactly the listed set.
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

fn node_limited(max_nodes: usize) -> RelationLimits {
    RelationLimits::new(
        max_nodes,
        64,
        RelationLimits::MAX_CONSTRAINTS,
        RelationLimits::MAX_OPERATIONS,
    )
    .expect("valid node-limited profile")
}

fn default_automaton_limits() -> TreeAutomatonLimits {
    TreeAutomatonLimits::default()
}

/// 1. Exact cell: member → Proven; non-member → Excluded.
#[test]
fn exact_cell_membership_is_decided() {
    // Ground system `a -> g(a)`: the one-step preimage of {g(a)} is
    // exactly {a}.
    let system = TermSystem::new(vec![Rule::new(a(), g(a())).unwrap()]);
    let language = accepts_exactly(&[("a", 0), ("g", 1)], &[g(a())]);
    let limits = RelationLimits::default();
    let automaton_limits = default_automaton_limits();

    let member = classify_and_query(
        &system,
        &language,
        PreimageOperation::OneStep,
        &a(),
        &limits,
        &automaton_limits,
    )
    .expect("ground one-step is an exact cell");
    match member.verdict() {
        MembershipVerdict::Proven(automaton) => {
            assert!(automaton.accepts(&a()).expect("ground"));
            assert!(member.certificate().verify_result(automaton));
        }
        other => panic!("a must be Proven, got {other:?}"),
    }
    assert!(member.certificate().verify(&system, &language, &limits));

    let non_member = classify_and_query(
        &system,
        &language,
        PreimageOperation::OneStep,
        &g(a()),
        &limits,
        &automaton_limits,
    )
    .expect("ground one-step is an exact cell");
    assert_eq!(non_member.verdict(), &MembershipVerdict::Excluded);
    assert!(non_member.certificate().verify(&system, &language, &limits));
}

/// 2. Approximation cell: witnessed term → Proven.
#[test]
fn approximation_witnessed_term_is_proven() {
    let system = TermSystem::new(vec![Rule::new(f(x(), x()), a()).unwrap()]);
    let language = accepts_exactly(&[("a", 0), ("g", 1), ("f", 2)], &[a()]);
    let limits = node_limited(3);
    let query = classify_and_query(
        &system,
        &language,
        PreimageOperation::OneStep,
        &f(a(), a()),
        &limits,
        &default_automaton_limits(),
    )
    .expect("non-left-linear one-step is approximation-only");

    assert!(
        matches!(query.verdict(), MembershipVerdict::Proven(_)),
        "f(a,a) replays into the language in one application, got {:?}",
        query.verdict()
    );
    assert!(matches!(
        query.certificate().authority(),
        CertificateAuthority::Partial { .. }
    ));
}

/// 3. Approximation cell, truncated enumeration, unwitnessed term →
///    Unknown (NOT Excluded). This is the soundness pin: a truncated
///    lower bound can never exclude.
#[test]
fn truncated_approximation_cell_is_unknown_not_excluded() {
    let system = TermSystem::new(vec![Rule::new(f(x(), x()), a()).unwrap()]);
    let language = accepts_exactly(&[("a", 0), ("f", 2)], &[a()]);
    // A 32-cell storage budget truncates the enumeration; `a` is in
    // the candidate domain but is not a one-step witness.
    let limits = RelationLimits::new(64, 64, 32, RelationLimits::MAX_OPERATIONS)
        .expect("valid tiny-constraint profile");
    let query = classify_and_query(
        &system,
        &language,
        PreimageOperation::OneStep,
        &a(),
        &limits,
        &default_automaton_limits(),
    )
    .expect("budget exhaustion during enumeration is not an error");

    assert_eq!(query.verdict(), &MembershipVerdict::Unknown);
    assert!(
        query
            .trace()
            .iter()
            .any(|event| matches!(event, ApproximationEvent::EnumerationTruncated { .. })),
        "the trace must record the truncation: {:?}",
        query.trace()
    );
    assert!(matches!(
        query.certificate().authority(),
        CertificateAuthority::Partial { .. }
    ));
}

/// 4. Approximation cell, COMPLETE enumeration (tiny alphabet and
///    horizon): unwitnessed term within the domain → Excluded; outside
///    the domain → Unknown.
#[test]
fn complete_enumeration_excludes_only_within_the_domain() {
    let system = TermSystem::new(vec![Rule::new(f(x(), x()), a()).unwrap()]);
    let language = accepts_exactly(&[("a", 0), ("g", 1), ("f", 2)], &[a()]);
    // Node cap 3 keeps the enumeration finite and complete; the
    // operation finite horizon is the full semantics.
    let limits = node_limited(3);

    let within = classify_and_query(
        &system,
        &language,
        PreimageOperation::FiniteHorizon(1),
        &g(a()),
        &limits,
        &default_automaton_limits(),
    )
    .expect("non-left-linear finite horizon is approximation-only");
    assert_eq!(within.verdict(), &MembershipVerdict::Excluded);
    assert!(
        !within
            .trace()
            .iter()
            .any(|event| matches!(event, ApproximationEvent::EnumerationTruncated { .. })),
        "the enumeration must have completed: {:?}",
        within.trace()
    );

    // f(g(a),g(a)) has five nodes, beyond the three-node domain, so
    // its absence from the enumeration proves nothing.
    let outside = classify_and_query(
        &system,
        &language,
        PreimageOperation::FiniteHorizon(1),
        &f(g(a()), g(a())),
        &limits,
        &default_automaton_limits(),
    )
    .expect("non-left-linear finite horizon is approximation-only");
    assert_eq!(outside.verdict(), &MembershipVerdict::Unknown);
}

/// 5. Refinement grows the lower bound: a tight run then a generous
///    run; refined ⊇ prior and new witnesses appear.
#[test]
fn refinement_grows_the_lower_bound() {
    let system = TermSystem::new(vec![Rule::new(f(x(), x()), a()).unwrap()]);
    let language = accepts_exactly(&[("a", 0), ("g", 1), ("f", 2)], &[a()]);
    let prior = one_step_lower_bound(
        &system,
        &language,
        &node_limited(3),
        &default_automaton_limits(),
    )
    .expect("approximation-only");
    let refined = refine_lower_bound(
        &system,
        &language,
        PreimageOperation::OneStep,
        &prior,
        &node_limited(5),
        &default_automaton_limits(),
    )
    .expect("a strictly larger budget refines the lower bound");

    for witness in prior.witnesses() {
        assert!(
            refined.lower().accepts(witness).expect("ground"),
            "prior witness {witness:?} must survive refinement"
        );
    }
    assert!(refined.witnesses().len() >= prior.witnesses().len());
    assert!(prior.lower().accepts(&f(a(), a())).expect("ground"));
    assert!(
        refined.lower().accepts(&f(g(a()), g(a()))).expect("ground"),
        "the generous run must witness f(g(a),g(a))"
    );
    assert!(!prior.lower().accepts(&f(g(a()), g(a()))).expect("ground"));
}

/// 6. Monotonicity rejection: a shrinking refinement is a typed error
///    and the prior outcome is left untouched.
#[test]
fn refinement_rejects_a_shrinking_lower_bound() {
    let system = TermSystem::new(vec![Rule::new(f(x(), x()), a()).unwrap()]);
    let language = accepts_exactly(&[("a", 0), ("g", 1), ("f", 2)], &[a()]);
    let prior = one_step_lower_bound(
        &system,
        &language,
        &node_limited(5),
        &default_automaton_limits(),
    )
    .expect("approximation-only");

    // A tighter node cap would drop prior witnesses.
    let tighter = refine_lower_bound(
        &system,
        &language,
        PreimageOperation::OneStep,
        &prior,
        &node_limited(3),
        &default_automaton_limits(),
    );
    assert!(
        matches!(tighter, Err(RewriteError::RefinementViolation { .. })),
        "a shrinking refinement must be a typed error, got {tighter:?}"
    );
    // The prior outcome is preserved by the rejected refinement.
    assert!(prior.lower().accepts(&f(g(a()), g(a()))).expect("ground"));

    // A mutated system produces witnesses over a different alphabet;
    // it cannot contain the prior language and must be rejected too.
    let mutated = TermSystem::new(vec![Rule::new(h(x(), x()), a()).unwrap()]);
    let mutated_result = refine_lower_bound(
        &mutated,
        &language,
        PreimageOperation::OneStep,
        &prior,
        &node_limited(5),
        &default_automaton_limits(),
    );
    assert!(
        matches!(
            mutated_result,
            Err(RewriteError::RefinementViolation { .. })
        ),
        "a mutated-system 'refinement' must be a typed error, got {mutated_result:?}"
    );
    assert!(prior.lower().accepts(&f(a(), a())).expect("ground"));
}

/// 7. Certificate: Proven/Excluded verdicts verify, and approximation
///    cells carry `Partial` authority.
#[test]
fn certificates_verify_for_proven_and_excluded() {
    // Exact cell.
    let system = TermSystem::new(vec![Rule::new(a(), g(a())).unwrap()]);
    let language = accepts_exactly(&[("a", 0), ("g", 1)], &[g(a())]);
    let limits = RelationLimits::default();
    let automaton_limits = default_automaton_limits();

    let proven = classify_and_query(
        &system,
        &language,
        PreimageOperation::OneStep,
        &a(),
        &limits,
        &automaton_limits,
    )
    .expect("exact cell");
    let proven_automaton = match proven.verdict() {
        MembershipVerdict::Proven(automaton) => automaton,
        other => panic!("a must be Proven, got {other:?}"),
    };
    assert!(proven.certificate().verify(&system, &language, &limits));
    assert!(proven.certificate().verify_result(proven_automaton));

    let excluded = classify_and_query(
        &system,
        &language,
        PreimageOperation::OneStep,
        &g(a()),
        &limits,
        &automaton_limits,
    )
    .expect("exact cell");
    assert_eq!(excluded.verdict(), &MembershipVerdict::Excluded);
    assert!(excluded.certificate().verify(&system, &language, &limits));
    assert!(excluded.certificate().is_complete());

    // Approximation cell: Partial authority.
    let approx_system = TermSystem::new(vec![Rule::new(f(x(), x()), a()).unwrap()]);
    let approx_language = accepts_exactly(&[("a", 0), ("g", 1), ("f", 2)], &[a()]);
    let approx_limits = node_limited(3);
    let approx = classify_and_query(
        &approx_system,
        &approx_language,
        PreimageOperation::OneStep,
        &f(a(), a()),
        &approx_limits,
        &default_automaton_limits(),
    )
    .expect("approximation-only");
    assert!(matches!(
        approx.certificate().authority(),
        CertificateAuthority::Partial { .. }
    ));
    assert!(approx
        .certificate()
        .verify(&approx_system, &approx_language, &approx_limits));
}

/// 8. Determinism: identical inputs yield identical verdicts and
///    traces.
#[test]
fn membership_queries_are_deterministic() {
    let system = TermSystem::new(vec![
        Rule::new(f(x(), x()), a()).unwrap(),
        Rule::new(g(x()), f(x(), x())).unwrap(),
    ]);
    let language = accepts_exactly(&[("a", 0), ("g", 1), ("f", 2)], &[a()]);
    let limits = node_limited(5);
    let first = classify_and_query(
        &system,
        &language,
        PreimageOperation::FiniteHorizon(2),
        &g(g(a())),
        &limits,
        &default_automaton_limits(),
    )
    .expect("approximation-only");
    let second = classify_and_query(
        &system,
        &language,
        PreimageOperation::FiniteHorizon(2),
        &g(g(a())),
        &limits,
        &default_automaton_limits(),
    )
    .expect("approximation-only");
    assert_eq!(first.verdict(), second.verdict());
    assert_eq!(first.trace(), second.trace());
    assert_eq!(first.certificate(), second.certificate());
}

/// 9. Budget sweeps: the verdict degrades Proven → Unknown as budgets
///    shrink, and never panics.
#[test]
fn verdicts_degrade_gracefully_as_budgets_shrink() {
    let system = TermSystem::new(vec![Rule::new(f(x(), x()), a()).unwrap()]);
    let language = accepts_exactly(&[("a", 0), ("g", 1), ("f", 2)], &[a()]);
    let term = f(a(), a());

    // Generous budget: Proven.
    let generous = classify_and_query(
        &system,
        &language,
        PreimageOperation::FiniteHorizon(1),
        &term,
        &node_limited(5),
        &default_automaton_limits(),
    )
    .expect("generous budget classifies");
    assert!(matches!(generous.verdict(), MembershipVerdict::Proven(_)));

    // Sweep budgets: every outcome is Ok (Proven/Unknown) or a typed
    // limit error, and small budgets never fabricate `Excluded`.
    let mut proved = 0usize;
    let mut unknown = 0usize;
    for constraints in [16, 24, 32, 48, 64, 128, 256, 512, 4_096] {
        let limits = RelationLimits::new(64, 64, constraints, RelationLimits::MAX_OPERATIONS)
            .expect("valid sweep profile");
        match classify_and_query(
            &system,
            &language,
            PreimageOperation::FiniteHorizon(1),
            &term,
            &limits,
            &default_automaton_limits(),
        ) {
            Ok(query) => match query.verdict() {
                MembershipVerdict::Proven(_) => proved += 1,
                MembershipVerdict::Unknown => unknown += 1,
                MembershipVerdict::Excluded => panic!(
                    "a complete finite-horizon construction must \
                     witness f(a,a), got Excluded"
                ),
            },
            Err(error) => assert!(
                matches!(error, RewriteError::RelationLimitExceeded { .. }),
                "the only legal failure is a typed limit error, got {error:?}"
            ),
        }
    }
    assert!(proved >= 1, "some budget must still prove the witness");
    assert!(unknown >= 1, "some tight budget must degrade to Unknown");
}

/// The empty language has no witnesses; on a complete enumeration
/// every candidate is excluded.
#[test]
fn empty_language_excludes_every_candidate_when_complete() {
    let system = TermSystem::new(vec![Rule::new(f(x(), x()), b()).unwrap()]);
    let language = accepts_exactly(&[("a", 0), ("b", 0), ("f", 2)], &[b()]);
    let limits = node_limited(3);
    let query = classify_and_query(
        &system,
        &language,
        PreimageOperation::FiniteHorizon(1),
        &a(),
        &limits,
        &default_automaton_limits(),
    )
    .expect("approximation-only");
    assert_eq!(query.verdict(), &MembershipVerdict::Excluded);
}

/// The finite-horizon approximation entry point and the query entry
/// point agree on the witnessed set for a complete enumeration.
#[test]
fn query_agrees_with_the_lower_bound_entry_point() {
    let system = TermSystem::new(vec![Rule::new(f(x(), x()), a()).unwrap()]);
    let language = accepts_exactly(&[("a", 0), ("g", 1), ("f", 2)], &[a()]);
    let limits = node_limited(3);
    let outcome =
        finite_horizon_lower_bound(&system, &language, 1, &limits, &default_automaton_limits())
            .expect("approximation-only");

    for candidate in [a(), b(), c(), g(a()), f(a(), a())] {
        let query = classify_and_query(
            &system,
            &language,
            PreimageOperation::FiniteHorizon(1),
            &candidate,
            &limits,
            &default_automaton_limits(),
        )
        .expect("approximation-only");
        let expected = outcome.lower().accepts(&candidate).expect("ground");
        match query.verdict() {
            MembershipVerdict::Proven(automaton) => {
                assert!(expected, "{candidate:?} was proven but not witnessed");
                assert!(automaton.accepts(&candidate).expect("ground"));
            }
            MembershipVerdict::Excluded | MembershipVerdict::Unknown => {
                assert!(!expected, "{candidate:?} is a witness but was not proven");
            }
        }
    }
}

/// Review round 1 (P1): replay incompleteness must prohibit
/// exclusion. `g(a)` rewrites to the 5-node `f(g(a), g(a))` (rule
/// `x -> f(x,x)`) and then to `b` — it IS in the concrete 2-step
/// preimage — but under a 3-node replay ceiling the oversized
/// intermediate is silently discarded. Non-membership in the lower
/// bound is then incompleteness, not exclusion evidence: the verdict
/// must be `Unknown`, never `Excluded`.
#[test]
fn oversized_intermediate_makes_the_replay_incomplete() {
    let system = TermSystem::new(vec![
        Rule::new(x(), f(x(), x())).unwrap(),
        Rule::new(f(x(), x()), b()).unwrap(),
    ]);
    let language = accepts_exactly(&[("a", 0), ("b", 0), ("g", 1), ("f", 2)], &[b()]);
    let limits = RelationLimits::new(3, 64, RelationLimits::MAX_CONSTRAINTS, 1_000_000)
        .expect("valid limits");
    let query = classify_and_query(
        &system,
        &language,
        PreimageOperation::FiniteHorizon(2),
        &g(a()),
        &limits,
        &TreeAutomatonLimits::default(),
    )
    .expect("query succeeds");
    assert!(
        matches!(query.verdict(), MembershipVerdict::Unknown),
        "an incomplete replay must yield Unknown, got {:?}",
        query.verdict()
    );
}

/// Review round 1 (P1), one-step variant: the same oversized
/// intermediate must block `Excluded` even when the language is the
/// intermediate itself (so the concrete one-step preimage DOES contain
/// `g(a)`).
#[test]
fn one_step_exclusion_respects_replay_incompleteness() {
    let system = TermSystem::new(vec![
        Rule::new(x(), f(x(), x())).unwrap(),
        Rule::new(f(x(), x()), b()).unwrap(),
    ]);
    let language = accepts_exactly(
        &[("a", 0), ("b", 0), ("g", 1), ("f", 2)],
        &[f(g(a()), g(a()))],
    );
    let limits = RelationLimits::new(3, 64, RelationLimits::MAX_CONSTRAINTS, 1_000_000)
        .expect("valid limits");
    let query = classify_and_query(
        &system,
        &language,
        PreimageOperation::OneStep,
        &g(a()),
        &limits,
        &TreeAutomatonLimits::default(),
    )
    .expect("query succeeds");
    assert!(
        matches!(query.verdict(), MembershipVerdict::Unknown),
        "one-step exclusion must respect replay incompleteness, got {:?}",
        query.verdict()
    );
}

/// Review round 1 (P2): the exact path shares ONE resource pool
/// across construction and membership — a 101-operation construction
/// plus the membership charge must exceed a 101-operation ceiling.
#[test]
fn exact_query_shares_one_resource_pool() {
    let system = TermSystem::new(vec![Rule::new(a(), b()).unwrap()]);
    let language = accepts_exactly(&[("a", 0), ("b", 0)], &[b()]);
    let limits = RelationLimits::new(4_096, 64, RelationLimits::MAX_CONSTRAINTS, 101)
        .expect("valid tight-operation limits");
    let result = classify_and_query(
        &system,
        &language,
        PreimageOperation::OneStep,
        &a(),
        &limits,
        &TreeAutomatonLimits::default(),
    );
    assert!(
        matches!(result, Err(RewriteError::RelationLimitExceeded { .. })),
        "construction (101 ops) + membership must exceed the shared 101-op pool, got {result:?}"
    );
}
