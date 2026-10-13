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
