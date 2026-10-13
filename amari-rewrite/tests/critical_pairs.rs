// SPDX-License-Identifier: MIT OR Apache-2.0

//! Critical-pair contract tests (0.25, Task 30).
//!
//! Deterministic non-variable overlap enumeration with rename-apart,
//! exact provenance, root/self-overlap conventions, trivial-pair
//! exclusion, left-linearity evidence, and resource metering.

use amari_rewrite::analysis::{critical_pairs, CriticalPair, CriticalPairReport};
use amari_rewrite::relation::{RelationLimits, RelationResources};
use amari_rewrite::trs::{Rule, Term, TermSystem, Variable};
use amari_rewrite::{Path, RewriteError};

fn a() -> Term {
    Term::constant("a")
}

fn b() -> Term {
    Term::constant("b")
}

fn x() -> Term {
    Term::var("x")
}

fn y() -> Term {
    Term::var("y")
}

fn cp_rx() -> Term {
    Term::var("cpRx")
}

fn cp_ry() -> Term {
    Term::var("cpRy")
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

fn report(system: &TermSystem) -> CriticalPairReport {
    let limits = RelationLimits::default();
    let mut resources = RelationResources::new(&limits);
    critical_pairs(system, &mut resources).expect("critical pairs computed")
}

/// The 3-rule system used by the ordering and metering tests.
fn order_system() -> TermSystem {
    system(vec![
        rule(app("f", [app("f", [x()])]), a()),
        rule(app("f", [x()]), app("g", [x()])),
        rule(app("f", [a()]), b()),
    ])
}

#[test]
fn root_overlap_is_reported_once() {
    let system = system(vec![
        rule(app("f", [x()]), app("g", [x()])),
        rule(app("f", [a()]), b()),
    ]);
    let report = report(&system);
    assert_eq!(report.pairs.len(), 1);
    let pair = &report.pairs[0];
    assert_eq!(pair.left_rule, 0);
    assert_eq!(pair.right_rule, 1);
    assert_eq!(pair.path, Path::root());
    assert_eq!(pair.overlap, app("f", [a()]));
    assert_eq!(pair.root_step, app("g", [a()]));
    assert_eq!(pair.position_step, b());
    assert_eq!(report.trivial_excluded, 0);
    assert!(report.non_left_linear_rules.is_empty());
}

#[test]
fn non_root_overlap_reports_nested_step() {
    let system = system(vec![
        rule(app("h", [app("f", [x()])]), app("k", [x()])),
        rule(app("f", [a()]), b()),
    ]);
    let report = report(&system);
    assert_eq!(report.pairs.len(), 1);
    let pair = &report.pairs[0];
    assert_eq!(pair.left_rule, 0);
    assert_eq!(pair.right_rule, 1);
    assert_eq!(pair.path, Path::from([0]));
    assert_eq!(pair.overlap, app("h", [app("f", [a()])]));
    assert_eq!(pair.root_step, app("k", [a()]));
    assert_eq!(pair.position_step, app("h", [b()]));
}

#[test]
fn self_overlap_non_root_renames_variables() {
    let system = system(vec![rule(app("f", [app("f", [x()])]), app("r", [x()]))]);
    let report = report(&system);
    assert_eq!(report.pairs.len(), 1);
    let pair = &report.pairs[0];
    assert_eq!(pair.left_rule, 0);
    assert_eq!(pair.right_rule, 0);
    assert_eq!(pair.path, Path::from([0]));
    assert_eq!(pair.overlap, app("f", [app("f", [app("f", [cp_rx()])])]));
    assert_eq!(pair.root_step, app("r", [app("f", [cp_rx()])]));
    assert_eq!(pair.position_step, app("f", [app("r", [cp_rx()])]));
    assert_eq!(report.trivial_excluded, 0);
}

#[test]
fn root_self_overlap_is_excluded() {
    let system = system(vec![rule(app("f", [x()]), app("g", [x()]))]);
    let report = report(&system);
    assert!(report.pairs.is_empty());
    assert_eq!(report.trivial_excluded, 0);
    assert!(report.non_left_linear_rules.is_empty());
}

#[test]
fn right_rule_variables_are_renamed_apart() {
    let system = system(vec![
        rule(app("f", [app("g", [x()])]), x()),
        rule(app("g", [y()]), y()),
    ]);
    let report = report(&system);
    assert_eq!(report.pairs.len(), 1);
    let pair = &report.pairs[0];
    assert_eq!(pair.path, Path::from([0]));
    assert_eq!(pair.overlap, app("f", [app("g", [cp_ry()])]));
    assert_eq!(pair.root_step, cp_ry());
    assert_eq!(pair.position_step, app("f", [cp_ry()]));
    // The renamed right variable never aliases the left rule's `x`.
    let variables = pair.overlap.variables();
    assert_eq!(variables, vec![Variable::new("cpRy")]);
    assert!(!variables.contains(&Variable::new("x")));
}

#[test]
fn variable_positions_are_never_overlapped() {
    let system = system(vec![rule(app("f", [x()]), x()), rule(a(), b())]);
    let report = report(&system);
    assert!(report.pairs.is_empty());
    assert_eq!(report.trivial_excluded, 0);
}

#[test]
fn non_left_linear_rule_is_flagged_but_pairs_generated() {
    let system = system(vec![
        rule(app("e", [x(), x()]), x()),
        rule(app("e", [x(), y()]), app("f", [x(), y()])),
    ]);
    let report = report(&system);
    assert_eq!(report.non_left_linear_rules, vec![0]);
    assert_eq!(report.pairs.len(), 1);
    let pair = &report.pairs[0];
    assert_eq!(pair.left_rule, 0);
    assert_eq!(pair.right_rule, 1);
    assert_eq!(pair.path, Path::root());
    assert_eq!(pair.overlap, app("e", [cp_rx(), cp_rx()]));
    assert_eq!(pair.root_step, cp_rx());
    assert_eq!(pair.position_step, app("f", [cp_rx(), cp_rx()]));
}

#[test]
fn identical_rules_produce_only_trivial_pairs() {
    let system = system(vec![
        rule(app("f", [x()]), app("g", [x()])),
        rule(app("f", [x()]), app("g", [x()])),
    ]);
    let report = report(&system);
    assert!(report.pairs.is_empty());
    assert_eq!(report.trivial_excluded, 1);
}

#[test]
fn pair_provenance_identifies_both_rules_and_path() {
    let system = system(vec![
        rule(app("h", [app("f", [x()])]), app("k", [x()])),
        rule(app("f", [a()]), b()),
        rule(app("f", [x()]), app("m", [x()])),
    ]);
    let report = report(&system);
    assert_eq!(report.pairs.len(), 3);
    assert_eq!(
        (report.pairs[0].left_rule, report.pairs[0].right_rule),
        (0, 1)
    );
    assert_eq!(report.pairs[0].path, Path::from([0]));
    assert_eq!(
        (report.pairs[1].left_rule, report.pairs[1].right_rule),
        (0, 2)
    );
    assert_eq!(report.pairs[1].path, Path::from([0]));
    assert_eq!(
        (report.pairs[2].left_rule, report.pairs[2].right_rule),
        (1, 2)
    );
    assert_eq!(report.pairs[2].path, Path::root());
}

#[test]
fn enumeration_order_is_total_and_deterministic() {
    let report = report(&order_system());
    let expected = vec![
        CriticalPair {
            left_rule: 0,
            right_rule: 0,
            path: Path::from([0]),
            overlap: app("f", [app("f", [app("f", [cp_rx()])])]),
            root_step: a(),
            position_step: app("f", [a()]),
        },
        CriticalPair {
            left_rule: 0,
            right_rule: 1,
            path: Path::root(),
            overlap: app("f", [app("f", [x()])]),
            root_step: a(),
            position_step: app("g", [app("f", [x()])]),
        },
        CriticalPair {
            left_rule: 0,
            right_rule: 1,
            path: Path::from([0]),
            overlap: app("f", [app("f", [cp_rx()])]),
            root_step: a(),
            position_step: app("f", [app("g", [cp_rx()])]),
        },
        CriticalPair {
            left_rule: 0,
            right_rule: 2,
            path: Path::from([0]),
            overlap: app("f", [app("f", [a()])]),
            root_step: a(),
            position_step: app("f", [b()]),
        },
        CriticalPair {
            left_rule: 1,
            right_rule: 2,
            path: Path::root(),
            overlap: app("f", [a()]),
            root_step: app("g", [a()]),
            position_step: b(),
        },
    ];
    assert_eq!(report.pairs, expected);
}

#[test]
fn limits_yield_typed_errors_and_no_panic() {
    let system = order_system();

    // One accounted operation is not enough for even the first overlap.
    let limits = RelationLimits::new(4096, 64, 4096, 1).expect("valid tiny limits");
    let mut resources = RelationResources::new(&limits);
    let error = critical_pairs(&system, &mut resources).expect_err("budget must exhaust");
    assert!(matches!(error, RewriteError::RelationLimitExceeded { .. }));

    // A larger budget completes.
    let limits = RelationLimits::default();
    let mut resources = RelationResources::new(&limits);
    assert!(critical_pairs(&system, &mut resources).is_ok());

    // A zero budget is a typed construction error, never a panic.
    assert!(matches!(
        RelationLimits::new(0, 64, 4096, 1_000_000),
        Err(RewriteError::InvalidLimit { .. })
    ));
}

#[test]
fn every_pair_side_is_a_real_engine_step() {
    let systems = vec![
        system(vec![
            rule(app("f", [x()]), app("g", [x()])),
            rule(app("f", [a()]), b()),
        ]),
        system(vec![
            rule(app("h", [app("f", [x()])]), app("k", [x()])),
            rule(app("f", [a()]), b()),
        ]),
        system(vec![rule(app("f", [app("f", [x()])]), app("r", [x()]))]),
        system(vec![
            rule(app("f", [app("g", [x()])]), x()),
            rule(app("g", [y()]), y()),
        ]),
        system(vec![
            rule(app("e", [x(), x()]), x()),
            rule(app("e", [x(), y()]), app("f", [x(), y()])),
        ]),
        order_system(),
    ];

    for system in &systems {
        let report = report(system);
        for pair in &report.pairs {
            let successors = system
                .application_successors(&pair.overlap)
                .expect("successors computed");
            assert!(
                successors.contains(&pair.root_step),
                "root side is not a one-step successor: {pair:?}"
            );
            assert!(
                successors.contains(&pair.position_step),
                "position side is not a one-step successor: {pair:?}"
            );

            // Pinned against the outer rule at the root.
            let outer = &system.rules()[pair.left_rule];
            let outer_subst = outer.matches(&pair.overlap).expect("outer matches overlap");
            assert_eq!(outer_subst.apply(outer.rhs()), pair.root_step);

            // Pinned against the inner rule at the embedded path.
            let inner = &system.rules()[pair.right_rule];
            let subterm = pair.overlap.subterm(&pair.path).expect("path exists");
            let inner_subst = inner.matches(subterm).expect("inner matches subterm");
            let expected = pair
                .overlap
                .replace_at(&pair.path, inner_subst.apply(inner.rhs()))
                .expect("replacement succeeds");
            assert_eq!(expected, pair.position_step);
        }
    }
}

/// PR #301 round 1 P1: rename-apart must be a one-pass structural
/// renaming. Deep substitution through `Substitution::apply`
/// re-resolves freshly introduced names, so `x -> cpRx` and
/// `cpRx -> cpRcpRx` collapse both variables to `cpRcpRx`, dropping
/// valid pairs (the reviewer's exact repro).
#[test]
fn rename_apart_never_collapses_prefixed_variables() {
    let system = system(vec![
        rule(app("f", [a(), b()]), b()),
        rule(app("f", [x(), cp_rx()]), x()),
    ]);
    let report = report(&system);
    assert_eq!(report.trivial_excluded, 0);
    assert_eq!(report.pairs.len(), 1, "{report:#?}");
    let pair = &report.pairs[0];
    assert_eq!(pair.overlap, app("f", [a(), b()]));
    assert_eq!(pair.root_step, b());
    assert_eq!(pair.position_step, a());
}

/// Longer prefix chains: `x`, `cpRx`, `cpRcpRx` must remain three
/// DISTINCT variables after renaming.
#[test]
fn rename_apart_preserves_prefix_chain_distinctness() {
    let cp_cp_rx = || Term::var("cpRcpRx");
    let system = system(vec![
        rule(app("f", [a(), a(), a()]), a()),
        rule(
            app("f", [x(), cp_rx(), cp_cp_rx()]),
            app("g", [x(), cp_rx()]),
        ),
    ]);
    let report = report(&system);
    assert_eq!(report.pairs.len(), 1, "{report:#?}");
    // The renamed RHS keeps two distinct variables: after unification
    // cpRx := a, cpRcpRx... every renamed variable binds to `a`, so the
    // pair sides are ground here. The distinctness assertion targets
    // the OVERLAP shape indirectly: if renaming collapsed any pair of
    // variables, the LHS would not unify with f(a,a,a) into this exact
    // single pair, and the RHS `g(cpRx, cpRcpRx)` would collapse to
    // identical arguments — both sides are still g(a,a) here, so
    // additionally require the pair to be non-trivial and exact.
    let pair = &report.pairs[0];
    assert_eq!(pair.root_step, a());
    assert_eq!(pair.position_step, app("g", [a(), a()]));
}

/// Non-ground case pinning RHS variable distinctness directly:
/// the renamed RHS keeps `cpRx` and `cpRcpRx` as separate bindings.
#[test]
fn rename_apart_keeps_rhs_variables_distinct_when_unbound() {
    let cp_cp_rx = || Term::var("cpRcpRx");
    let system = system(vec![
        rule(app("f", [x(), x(), x()]), x()),
        rule(
            app("f", [x(), cp_rx(), cp_cp_rx()]),
            app("g", [cp_rx(), cp_cp_rx()]),
        ),
    ]);
    let report = report(&system);
    assert_eq!(report.non_left_linear_rules, vec![0]);
    assert_eq!(report.pairs.len(), 1, "{report:#?}");
    let pair = &report.pairs[0];
    // The mgu identifies all three distinct renamed variables with the
    // left `x`; the exact representative is the unifier's choice, so
    // assert CONSISTENCY: a single variable representative shared by
    // both sides, appearing twice in the position step.
    assert!(pair.root_step.is_var(), "{pair:#?}");
    assert_eq!(
        pair.position_step,
        app("g", [pair.root_step.clone(), pair.root_step.clone()])
    );
}

/// Collapse actually prevents a pair: `x` and `cpRx` in the right
/// rule must stay distinct for the LHS to be linear enough to unify
/// with DIFFERENT constants.
#[test]
fn rename_apart_collapse_drops_pairs() {
    let system = system(vec![
        rule(app("f", [a(), b()]), b()),
        rule(app("f", [x(), cp_rx()]), app("g", [cp_rx()])),
    ]);
    let report = report(&system);
    assert_eq!(report.trivial_excluded, 0, "{report:#?}");
    assert_eq!(report.pairs.len(), 1, "{report:#?}");
    // The distinct `cpRx` binds to `b`; a collapse to `cpRcpRx`
    // everywhere would have failed to unify and dropped this pair.
    assert_eq!(report.pairs[0].root_step, b());
    assert_eq!(report.pairs[0].position_step, app("g", [b()]));
}

/// PR #301 round 1 P1: input terms beyond the term bounds are typed
/// errors BEFORE any construction (the reviewer's exact repro).
#[test]
fn oversized_input_terms_are_typed_errors() {
    let system = system(vec![rule(a(), app("big", [b(), b()])), rule(a(), b())]);
    let limits = RelationLimits::new(1, 1, 10, 100).expect("valid limits");
    let mut resources = RelationResources::new(&limits);
    let outcome = critical_pairs(&system, &mut resources);
    assert!(
        matches!(outcome, Err(RewriteError::RelationLimitExceeded { .. })),
        "{outcome:?}"
    );
}

/// Substitution-expanded output beyond the term bounds is a typed
/// error, with no partial report.
#[test]
fn expanded_output_beyond_bounds_is_typed_error() {
    let system = system(vec![
        rule(app("f", [x()]), app("g", [x(), x()])),
        rule(app("f", [a()]), b()),
    ]);
    // Input terms fit (3 nodes each), but root_step g(a,a) is 3 nodes
    // over a 2-node budget.
    let limits = RelationLimits::new(2, 8, 10, 1_000_000).expect("valid limits");
    let mut resources = RelationResources::new(&limits);
    let outcome = critical_pairs(&system, &mut resources);
    assert!(
        matches!(outcome, Err(RewriteError::RelationLimitExceeded { .. })),
        "{outcome:?}"
    );
}

/// A constraint budget below the successful-unify count fails at
/// pair reservation, with no partial report. (`RelationLimits`
/// rejects zero constraints at construction, so the tightest
/// constructible budget is used.)
#[test]
fn constraint_budget_fails_at_reservation() {
    let system = system(vec![
        rule(app("f", [x()]), app("g", [x()])),
        rule(app("f", [a()]), b()),
        rule(app("f", [app("c", [])]), app("d", [])),
    ]);
    // Two root overlaps succeed; a one-cell budget must fail.
    let limits = RelationLimits::new(64, 64, 1, 1_000_000).expect("valid limits");
    let mut resources = RelationResources::new(&limits);
    let outcome = critical_pairs(&system, &mut resources);
    assert!(
        matches!(outcome, Err(RewriteError::RelationLimitExceeded { .. })),
        "{outcome:?}"
    );
    // Two cells cover both reservations.
    let limits = RelationLimits::new(64, 64, 2, 1_000_000).expect("valid limits");
    let mut resources = RelationResources::new(&limits);
    let report = critical_pairs(&system, &mut resources).expect("pairs computed");
    assert_eq!(report.pairs.len(), 2, "{report:#?}");
}
