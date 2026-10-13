// SPDX-License-Identifier: MIT OR Apache-2.0

//! Local-confluence contract tests (0.25, Task 31).
//!
//! Bounded bidirectional joinability over Task 30's critical pairs:
//! deterministic witnesses, shared resource accounting, non-left-linear
//! certification blocking, and engine-replayed chains.

use amari_rewrite::analysis::{
    critical_pairs, local_confluence, ConfluenceReport, PairJoinability,
};
use amari_rewrite::relation::{RelationLimits, RelationResources};
use amari_rewrite::trs::{Rule, Term, TermSystem};
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

fn d() -> Term {
    Term::constant("d")
}

fn e() -> Term {
    Term::constant("e")
}

fn f() -> Term {
    Term::constant("f")
}

fn h() -> Term {
    Term::constant("h")
}

fn p() -> Term {
    Term::constant("p")
}

fn q() -> Term {
    Term::constant("q")
}

fn r() -> Term {
    Term::constant("r")
}

fn x() -> Term {
    Term::var("x")
}

fn y() -> Term {
    Term::var("y")
}

fn app(name: &str, args: impl IntoIterator<Item = Term>) -> Term {
    Term::sym(name, args)
}

fn rule(lhs: Term, rhs: Term) -> Rule {
    Rule::new(lhs, rhs).expect("valid rule")
}

fn system(rules: Vec<Rule>) -> TermSystem {
    TermSystem::new(rules)
}

fn report(system: &TermSystem) -> ConfluenceReport {
    let limits = RelationLimits::default();
    let mut resources = RelationResources::new(&limits);
    local_confluence(system, &mut resources).expect("confluence computed")
}

/// The classic joinable left-linear diamond `a→b`, `a→c`, `b→d`,
/// `c→d`. One critical pair whose sides join at `d`.
fn diamond() -> TermSystem {
    system(vec![
        rule(a(), b()),
        rule(a(), c()),
        rule(b(), d()),
        rule(c(), d()),
    ])
}

fn joinable_witness(outcome: &PairJoinability) -> &amari_rewrite::analysis::JoinabilityWitness {
    match outcome {
        PairJoinability::Joinable(witness) => witness,
        PairJoinability::NotJoinable => panic!("expected a joinable pair"),
    }
}

/// Test 1: the classic diamond is locally confluent and certifies.
#[test]
fn joinable_diamond_certifies_local_confluence() {
    let report = report(&diamond());
    assert_eq!(report.pairs.len(), 1);
    let (pair, outcome) = &report.pairs[0];
    assert_eq!(pair.root_step, b());
    assert_eq!(pair.position_step, c());
    let witness = joinable_witness(outcome);
    assert_eq!(witness.join_term, d());
    assert_eq!(witness.left_steps, vec![b(), d()]);
    assert_eq!(witness.right_steps, vec![c(), d()]);
    assert!(report.locally_confluent);
    assert!(report.certification_blocked.is_empty());
}

/// Test 2: two distinct irreducible reducts cannot join.
#[test]
fn nonjoinable_diamond_is_not_locally_confluent() {
    let system = system(vec![rule(a(), b()), rule(a(), c())]);
    let report = report(&system);
    assert_eq!(report.pairs.len(), 1);
    let (pair, outcome) = &report.pairs[0];
    assert_eq!(pair.root_step, b());
    assert_eq!(pair.position_step, c());
    assert_eq!(*outcome, PairJoinability::NotJoinable);
    assert!(!report.locally_confluent);
    assert!(report.certification_blocked.is_empty());
}

/// Test 3: exhausting the search budget is a typed error with no
/// partial report. The enumeration needs a fixed number of
/// operations; one more than that is not enough for the two search
/// expansions the diamond requires.
#[test]
fn search_budget_exhaustion_is_typed_without_partial_report() {
    let system = diamond();
    let mut probe = RelationResources::new(&RelationLimits::default());
    critical_pairs(&system, &mut probe).expect("enumeration completes");
    let enumeration = probe.operations();

    let limits = RelationLimits::new(4096, 64, 4096, enumeration + 1).expect("valid limits");
    let mut resources = RelationResources::new(&limits);
    let outcome = local_confluence(&system, &mut resources);
    assert!(
        matches!(outcome, Err(RewriteError::RelationLimitExceeded { .. })),
        "{outcome:?}"
    );
}

/// Test 4: the visited set terminates a cyclic successor graph; the
/// pair joins through the cycle without hanging.
#[test]
fn cycles_terminate_via_visited_set() {
    let system = system(vec![rule(a(), b()), rule(b(), a()), rule(a(), c())]);
    let report = report(&system);
    assert_eq!(report.pairs.len(), 1);
    let (pair, outcome) = &report.pairs[0];
    assert_eq!(pair.root_step, b());
    assert_eq!(pair.position_step, c());
    let witness = joinable_witness(outcome);
    assert_eq!(witness.join_term, c());
    assert_eq!(witness.left_steps, vec![b(), a(), c()]);
    assert_eq!(witness.right_steps, vec![c()]);
    assert!(report.locally_confluent);
}

/// Test 5: a non-left-linear rule blocks certification even though
/// every analyzed pair joins. The report stays complete.
#[test]
fn non_left_linear_blocks_certification_but_pairs_join() {
    let system = system(vec![
        rule(app("f", [x(), x()]), a()),
        rule(app("f", [x(), y()]), b()),
        rule(a(), c()),
        rule(b(), c()),
    ]);
    let report = report(&system);
    assert_eq!(report.pairs.len(), 1);
    let (pair, outcome) = &report.pairs[0];
    assert_eq!(pair.root_step, a());
    assert_eq!(pair.position_step, b());
    let witness = joinable_witness(outcome);
    assert_eq!(witness.join_term, c());
    assert_eq!(witness.left_steps, vec![a(), c()]);
    assert_eq!(witness.right_steps, vec![b(), c()]);
    assert_eq!(report.certification_blocked, vec![0]);
    assert!(!report.locally_confluent);
}

/// Test 6: witnesses follow the pinned bidirectional-BFS order.
#[test]
fn witnesses_are_deterministic_in_bfs_order() {
    let system = system(vec![
        rule(a(), b()),
        rule(a(), c()),
        rule(b(), d()),
        rule(b(), e()),
        rule(c(), f()),
        rule(d(), h()),
        rule(e(), h()),
        rule(f(), h()),
    ]);
    let report = report(&system);
    assert_eq!(report.pairs.len(), 2);

    let (pair, outcome) = &report.pairs[0];
    assert_eq!(pair.root_step, b());
    assert_eq!(pair.position_step, c());
    let witness = joinable_witness(outcome);
    assert_eq!(witness.join_term, h());
    assert_eq!(witness.left_steps, vec![b(), d(), h()]);
    assert_eq!(witness.right_steps, vec![c(), f(), h()]);

    let (pair, outcome) = &report.pairs[1];
    assert_eq!(pair.root_step, d());
    assert_eq!(pair.position_step, e());
    let witness = joinable_witness(outcome);
    assert_eq!(witness.join_term, h());
    assert_eq!(witness.left_steps, vec![d(), h()]);
    assert_eq!(witness.right_steps, vec![e(), h()]);

    assert!(report.locally_confluent);
}

/// Test 7: a mixed report keeps enumeration order and flags the whole
/// system non-confluent.
#[test]
fn mixed_report_orders_pairs_and_flags_non_confluence() {
    let system = system(vec![
        rule(a(), b()),
        rule(a(), c()),
        rule(b(), d()),
        rule(c(), d()),
        rule(p(), q()),
        rule(p(), r()),
    ]);
    let report = report(&system);
    assert_eq!(report.pairs.len(), 2);
    let (pair, outcome) = &report.pairs[0];
    assert_eq!(pair.root_step, b());
    assert_eq!(pair.position_step, c());
    assert_eq!(joinable_witness(outcome).join_term, d());
    let (pair, outcome) = &report.pairs[1];
    assert_eq!(pair.root_step, q());
    assert_eq!(pair.position_step, r());
    assert_eq!(*outcome, PairJoinability::NotJoinable);
    assert!(!report.locally_confluent);
    assert!(report.certification_blocked.is_empty());
}

/// Test 8: enumeration and search bill one shared pool. The exact
/// boundary is 16 enumeration operations plus 2 search expansions.
#[test]
fn metering_shares_one_pool_across_enumeration_and_search() {
    let system = diamond();
    let limits = RelationLimits::default();
    let mut generous = RelationResources::new(&limits);
    let report = local_confluence(&system, &mut generous).expect("computed");
    assert_eq!(report.pairs.len(), 1);
    let total = generous.operations();
    assert_eq!(total, 18, "16 enumeration + 2 search expansions");

    let exact_limits = RelationLimits::new(4096, 64, 4096, total).expect("valid");
    let mut exact = RelationResources::new(&exact_limits);
    let report = local_confluence(&system, &mut exact).expect("computed");
    assert_eq!(report.pairs.len(), 1);
    assert_eq!(exact.operations(), total);

    let short_limits = RelationLimits::new(4096, 64, 4096, total - 1).expect("valid");
    let mut short = RelationResources::new(&short_limits);
    let outcome = local_confluence(&system, &mut short);
    assert!(
        matches!(outcome, Err(RewriteError::RelationLimitExceeded { .. })),
        "{outcome:?}"
    );
}

/// Test 9: every joinable witness replays as a valid application
/// chain; both chains start at the pair sides and end at the join.
#[test]
fn joinable_witness_chains_replay_through_the_engine() {
    let systems = vec![
        diamond(),
        system(vec![rule(a(), b()), rule(b(), a()), rule(a(), c())]),
        system(vec![
            rule(a(), b()),
            rule(a(), c()),
            rule(b(), d()),
            rule(b(), e()),
            rule(c(), f()),
            rule(d(), h()),
            rule(e(), h()),
            rule(f(), h()),
        ]),
        system(vec![
            rule(app("f", [x(), x()]), a()),
            rule(app("f", [x(), y()]), b()),
            rule(a(), c()),
            rule(b(), c()),
        ]),
        system(vec![
            rule(a(), b()),
            rule(a(), c()),
            rule(b(), d()),
            rule(c(), d()),
            rule(p(), q()),
            rule(p(), r()),
        ]),
    ];

    for system in &systems {
        let report = report(system);
        for (pair, outcome) in &report.pairs {
            let PairJoinability::Joinable(witness) = outcome else {
                continue;
            };
            assert_eq!(witness.left_steps.first(), Some(&pair.root_step));
            assert_eq!(witness.right_steps.first(), Some(&pair.position_step));
            assert_eq!(witness.left_steps.last(), Some(&witness.join_term));
            assert_eq!(witness.right_steps.last(), Some(&witness.join_term));
            assert_chain(system, &witness.left_steps);
            assert_chain(system, &witness.right_steps);
        }
    }
}

/// Assert that each adjacent pair in `chain` is a live application
/// successor: the next term is reachable in one rewrite step.
fn assert_chain(system: &TermSystem, chain: &[Term]) {
    for window in chain.windows(2) {
        let successors = system
            .application_successors(&window[0])
            .expect("successors computed");
        assert!(
            successors.contains(&window[1]),
            "step {:?} -> {:?} is not an application successor",
            window[0],
            window[1]
        );
    }
}

/// PR #302 round 1 P1 (reviewer repro): a user variable literally
/// named `cpRy` collides with the critical-pair rename prefix. The
/// concrete peak from `k(c)` reaches distinct irreducibles `c` and
/// `b`; the pair must be NotJoinable and the certificate false.
#[test]
fn certificate_is_capture_free_for_prefixed_user_variables() {
    let original = system(vec![
        rule(app("k", [y()]), app("f", [y(), a()])),
        rule(app("k", [y()]), b()),
        rule(app("f", [x(), Term::var("cpRy")]), x()),
        rule(a(), b()),
    ]);
    let limits = RelationLimits::new(64, 16, 64, 1_000_000).expect("valid limits");
    let mut resources = RelationResources::new(&limits);
    let outcome = local_confluence(&original, &mut resources).expect("report computed");
    assert_eq!(outcome.pairs.len(), 1, "{outcome:#?}");
    assert!(
        matches!(outcome.pairs[0].1, PairJoinability::NotJoinable),
        "{outcome:#?}"
    );
    assert!(!outcome.locally_confluent, "{outcome:#?}");
    // Renaming the user variable must not change the verdict.
    let renamed_system = system(vec![
        rule(app("k", [y()]), app("f", [y(), a()])),
        rule(app("k", [y()]), b()),
        rule(app("f", [x(), Term::var("w")]), x()),
        rule(a(), b()),
    ]);
    let mut resources = RelationResources::new(&limits);
    let renamed = local_confluence(&renamed_system, &mut resources).expect("report computed");
    assert_eq!(renamed.locally_confluent, outcome.locally_confluent);
    assert!(
        matches!(renamed.pairs[0].1, PairJoinability::NotJoinable),
        "{renamed:#?}"
    );
}

/// PR #302 round 1 P1 (allocation bypass): an oversized candidate
/// must be shape-checked BEFORE construction, so rule order cannot
/// skip its validation. Both orders of the `f` rules must produce
/// the same typed error.
#[test]
fn oversized_successor_is_rejected_regardless_of_rule_order() {
    // wide(x) = g(x, a x40): 42 nodes. big = wide(a) is 42 nodes
    // (fits the 70-node limit); wide(big) is 83 nodes (does not).
    let wide = |inner: Term| -> Term {
        let mut args = Vec::from([inner]);
        for _ in 0..40 {
            args.push(a());
        }
        app("g", args)
    };
    let big = wide(a());
    assert_eq!(big.positions().len(), 42);
    let duplicating = || rule(app("f", [x()]), wide(x()));
    let small_rule = rule(app("f", [x()]), app("end", []));
    let base = vec![
        rule(Term::var("z"), app("f", [big.clone()])),
        rule(Term::var("z"), app("end", [])),
    ];
    let mut forward = base.clone();
    forward.extend([small_rule.clone(), duplicating()]);
    let mut reversed = base;
    reversed.extend([duplicating(), small_rule]);
    for rules in [forward, reversed] {
        let system = system(rules);
        let limits = RelationLimits::new(70, 16, 64, 1_000_000).expect("valid limits");
        let mut resources = RelationResources::new(&limits);
        let outcome = local_confluence(&system, &mut resources);
        assert!(
            matches!(outcome, Err(RewriteError::RelationLimitExceeded { .. })),
            "{outcome:?}"
        );
    }
}
