// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded Knuth–Bendix completion contract tests (0.25, Task 33).
//!
//! Orientation (including reversal), critical-pair rule addition,
//! right-hand-side simplification, alpha-canonical duplicate
//! suppression, finite converging examples, the honest `Partial`
//! stops, fatal input rejection, typed limit exhaustion, determinism,
//! and a bounded, consistent trace.

#![cfg(feature = "completion")]

use amari_rewrite::analysis::Precedence;
use amari_rewrite::completion::{
    complete, CompletionConfig, CompletionOutcome, FailedReason, PartialReason, TraceStep,
};
use amari_rewrite::relation::{RelationLimits, RelationResources};
use amari_rewrite::trs::{Rule, Symbol, Term, TermSystem};
use amari_rewrite::RewriteError;

fn precedence(entries: &[(&str, usize)]) -> Precedence {
    Precedence::new(
        entries
            .iter()
            .map(|(name, arity)| (Symbol::new(*name), *arity))
            .collect(),
    )
    .expect("valid precedence")
}

fn rule(lhs: Term, rhs: Term) -> Rule {
    Rule::new(lhs, rhs).expect("valid rule")
}

fn make_system(rules: Vec<Rule>) -> TermSystem {
    TermSystem::new(rules)
}

fn run(system: &TermSystem, order: Precedence) -> CompletionOutcome {
    let mut resources = RelationResources::new(&RelationLimits::default());
    complete(system, &CompletionConfig::new(order), &mut resources).expect("completion runs")
}

fn app(name: &str, args: impl IntoIterator<Item = Term>) -> Term {
    Term::sym(name, args)
}

fn x() -> Term {
    Term::var("x")
}

fn y() -> Term {
    Term::var("y")
}

fn a() -> Term {
    Term::constant("a")
}

fn b() -> Term {
    Term::constant("b")
}

fn f_of_x() -> Term {
    app("f", [x()])
}

fn f_of_y() -> Term {
    app("f", [y()])
}

fn g_of_x() -> Term {
    app("g", [x()])
}

fn g_of_y() -> Term {
    app("g", [y()])
}

fn h_of_x() -> Term {
    app("h", [x()])
}

/// Test 1: a ground input rule whose written direction is not
/// decreasing is reversed into a valid strictly-decreasing rule.
#[test]
fn orients_input_rule_in_reverse_when_needed() {
    // `a` outranks `b`, so `b -> a` must become `a -> b`.
    let order = precedence(&[("a", 0), ("b", 0)]);
    let sys = make_system(vec![rule(b(), a())]);
    match run(&sys, order) {
        CompletionOutcome::Complete { system, trace } => {
            assert_eq!(system, make_system(vec![rule(a(), b())]));
            assert_eq!(trace.rules_added, 0);
            assert_eq!(trace.pairs_processed, 0);
        }
        other => panic!("expected Complete, got {other:?}"),
    }
}

/// Test 2: a critical pair adds a rule and simplifies an existing
/// right-hand side.
///
/// `f(x) -> g(x)` and `f(y) -> b` overlap at the root, yielding
/// `g(cpRy) ≈ b`; `g > b` makes it `g(cpRy) -> b`, which reduces the
/// first rule's right-hand side to `b`.
#[test]
fn adds_rule_from_critical_pair_and_simplifies_rhs() {
    let order = precedence(&[("f", 1), ("g", 1), ("b", 0)]);
    let sys = make_system(vec![rule(f_of_x(), g_of_x()), rule(f_of_y(), b())]);
    match run(&sys, order) {
        CompletionOutcome::Complete { system, trace } => {
            assert_eq!(trace.rules_added, 1);
            assert_eq!(trace.pairs_processed, 1);
            assert!(trace
                .steps
                .iter()
                .any(|step| matches!(step, TraceStep::Simplified { rule_index: 0 })));
            let cp_ry = Term::var("cpRy");
            let expected = make_system(vec![
                rule(f_of_x(), b()),
                rule(f_of_y(), b()),
                rule(app("g", [cp_ry]), b()),
            ]);
            assert_eq!(system, expected);
        }
        other => panic!("expected Complete, got {other:?}"),
    }
}

/// Test 3: alpha-equivalent critical-pair equations are processed
/// once. `(0,1)` yields `g ≈ h`; `(1,2)` yields the same unordered
/// equation `h ≈ g`, which duplicate suppression drops.
#[test]
fn suppresses_duplicate_equations_under_renaming() {
    let order = precedence(&[("f", 1), ("g", 1), ("h", 1)]);
    let sys = make_system(vec![
        rule(f_of_x(), g_of_x()),
        rule(f_of_x(), h_of_x()),
        rule(f_of_x(), g_of_x()),
    ]);
    match run(&sys, order) {
        CompletionOutcome::Complete { system, trace } => {
            assert_eq!(
                trace.pairs_processed, 1,
                "the mirrored equation is a duplicate"
            );
            assert_eq!(trace.rules_added, 1);
            let expected = make_system(vec![
                rule(f_of_x(), h_of_x()),
                rule(f_of_x(), h_of_x()),
                rule(f_of_x(), h_of_x()),
                rule(app("g", [Term::var("cpRx")]), app("h", [Term::var("cpRx")])),
            ]);
            assert_eq!(system, expected);
        }
        other => panic!("expected Complete, got {other:?}"),
    }
}

/// Test 4: a finite converging example with a nontrivial join.
#[test]
fn completes_a_finite_converging_system() {
    let order = precedence(&[("f", 1), ("g", 1), ("h", 1)]);
    let sys = make_system(vec![rule(f_of_x(), g_of_x()), rule(f_of_x(), h_of_x())]);
    match run(&sys, order) {
        CompletionOutcome::Complete { system, trace } => {
            assert_eq!(trace.rules_added, 1);
            let expected = make_system(vec![
                rule(f_of_x(), h_of_x()),
                rule(f_of_x(), h_of_x()),
                rule(app("g", [Term::var("cpRx")]), app("h", [Term::var("cpRx")])),
            ]);
            assert_eq!(system, expected);
        }
        other => panic!("expected Complete, got {other:?}"),
    }
}

/// Test 5: an equation that neither direction can orient stops the
/// run with an honest intermediate system.
#[test]
fn unorientable_equation_yields_partial() {
    // `h(x, y) -> f(y)` and `h(x, y) -> g(x)` overlap at the root,
    // yielding `f(cpRy) ≈ g(cpRx)`: the sides share no variable, so
    // neither LPO direction holds.
    let order = precedence(&[("h", 2), ("f", 1), ("g", 1)]);
    let sys = make_system(vec![
        rule(app("h", [x(), y()]), app("f", [y()])),
        rule(app("h", [x(), y()]), app("g", [x()])),
    ]);
    match run(&sys, order) {
        CompletionOutcome::Partial {
            system,
            reason: PartialReason::UnorientableRule { rule_terms },
            ..
        } => {
            assert_eq!(
                rule_terms,
                (app("f", [Term::var("cpRy")]), app("g", [Term::var("cpRx")]),)
            );
            // Both input rules were oriented before the stop.
            assert_eq!(
                system,
                make_system(vec![
                    rule(app("h", [x(), y()]), app("f", [y()])),
                    rule(app("h", [x(), y()]), app("g", [x()])),
                ])
            );
        }
        other => panic!("expected Partial(UnorientableRule), got {other:?}"),
    }
}

/// Test 6: a non-left-linear input rule is a fatal precondition.
#[test]
fn non_left_linear_input_yields_failed() {
    let order = precedence(&[("e", 2), ("a", 0)]);
    let sys = make_system(vec![rule(app("e", [x(), x()]), x())]);
    match run(&sys, order) {
        CompletionOutcome::Failed {
            reason: FailedReason::InvalidInput,
        } => {}
        other => panic!("expected Failed(InvalidInput), got {other:?}"),
    }
}

/// Test 7: a generated rule that is not left-linear stops honestly.
///
/// `g(x) -> f(x, x)` and `g(y) -> a` yield `f(cpRy, cpRy) ≈ a`; the
/// oriented rule `f(x, x) -> a` has a repeated left-hand-side
/// variable, so the left-linearity postcheck refuses it.
#[test]
fn non_left_linear_generated_rule_yields_partial() {
    let order = precedence(&[("g", 1), ("f", 2), ("a", 0)]);
    let sys = make_system(vec![
        rule(g_of_x(), app("f", [x(), x()])),
        rule(g_of_y(), a()),
    ]);
    match run(&sys, order) {
        CompletionOutcome::Partial {
            reason: PartialReason::NonLeftLinear,
            ..
        } => {}
        other => panic!("expected Partial(NonLeftLinear), got {other:?}"),
    }
}

/// Test 8: a precedence that does not cover the input is a fatal
/// precondition, never a silent extension.
#[test]
fn precedence_coverage_failure_yields_failed() {
    let order = precedence(&[("f", 1)]);
    let sys = make_system(vec![rule(f_of_x(), a())]);
    match run(&sys, order) {
        CompletionOutcome::Failed {
            reason: FailedReason::InvalidInput,
        } => {}
        other => panic!("expected Failed(InvalidInput), got {other:?}"),
    }
}

/// Test 9: exhausting the budget is a typed error with no report.
#[test]
fn limit_exhaustion_is_a_typed_error() {
    let order = precedence(&[("f", 1), ("g", 1), ("b", 0)]);
    let sys = make_system(vec![rule(f_of_x(), g_of_x()), rule(f_of_y(), b())]);
    let limits = RelationLimits::new(4_096, 64, 4_096, 1).expect("valid limits");
    let mut resources = RelationResources::new(&limits);
    let outcome = complete(&sys, &CompletionConfig::new(order), &mut resources);
    assert!(
        matches!(outcome, Err(RewriteError::RelationLimitExceeded { .. })),
        "expected a typed limit error, got {outcome:?}"
    );
}

/// Test 10: the same input produces the same outcome.
#[test]
fn determinism_same_input_same_outcome() {
    let order = precedence(&[("f", 1), ("g", 1), ("b", 0)]);
    let sys = make_system(vec![rule(f_of_x(), g_of_x()), rule(f_of_y(), b())]);
    let first = run(&sys, order.clone());
    let second = run(&sys, order);
    assert_eq!(first, second);
    // A passing run is a `Complete` outcome with the same system.
    assert!(matches!(first, CompletionOutcome::Complete { .. }));
}

/// Test 11: the trace is compact and its counters are consistent.
#[test]
fn trace_is_bounded_and_consistent() {
    let order = precedence(&[("f", 1), ("g", 1), ("b", 0)]);
    let sys = make_system(vec![rule(f_of_x(), g_of_x()), rule(f_of_y(), b())]);
    match run(&sys, order) {
        CompletionOutcome::Complete { trace, .. } => {
            let pairs = trace
                .steps
                .iter()
                .filter(|step| matches!(step, TraceStep::Pair { .. }))
                .count();
            let oriented = trace
                .steps
                .iter()
                .filter(|step| matches!(step, TraceStep::Oriented { .. }))
                .count();
            let simplified = trace
                .steps
                .iter()
                .filter(|step| matches!(step, TraceStep::Simplified { .. }))
                .count();
            assert_eq!(pairs, trace.pairs_processed);
            assert_eq!(oriented, trace.rules_added);
            assert_eq!(trace.steps.len(), pairs + oriented + simplified);
            assert!(trace.steps.len() < 64, "trace must stay bounded");
        }
        other => panic!("expected Complete, got {other:?}"),
    }
}

/// Test 12: an already-convergent system is certified with no new
/// rules.
#[test]
fn already_convergent_system_completes_without_rules() {
    let order = precedence(&[("f", 1), ("g", 1), ("h", 1)]);
    let sys = make_system(vec![rule(f_of_x(), g_of_x()), rule(g_of_x(), h_of_x())]);
    match run(&sys, order) {
        CompletionOutcome::Complete { system, trace } => {
            assert_eq!(trace.rules_added, 0);
            assert_eq!(trace.pairs_processed, 0);
            assert_eq!(
                system,
                make_system(vec![rule(f_of_x(), g_of_x()), rule(g_of_x(), h_of_x())])
            );
        }
        other => panic!("expected Complete, got {other:?}"),
    }
}

/// Test 13: a ground check that the reversal path keeps orientation:
/// the completed system's single rule is strictly LPO-decreasing.
#[test]
fn reversed_ground_rule_is_strictly_decreasing() {
    let order = precedence(&[("a", 0), ("b", 0)]);
    let sys = make_system(vec![rule(b(), a())]);
    match run(&sys, order.clone()) {
        CompletionOutcome::Complete { system, .. } => {
            let mut resources = RelationResources::new(&RelationLimits::default());
            let proof = amari_rewrite::analysis::prove_terminating(&system, &order, &mut resources)
                .expect("proof runs");
            assert!(matches!(
                proof,
                amari_rewrite::analysis::TerminationProof::ProvedTerminating { .. }
            ));
        }
        other => panic!("expected Complete, got {other:?}"),
    }
}

/// PR #310 round 1 P1: input validation must bound shapes BEFORE
/// traversing occurrences — an adversarially deep input fails typed,
/// never aborts on allocation.
#[test]
fn deeply_nested_input_fails_typed_not_abort() {
    let mut deep = Term::var("x");
    for _ in 0..20_000 {
        deep = Term::sym("f", [deep]);
    }
    // Move, never clone: derived Clone recurses over the unvalidated
    // depth (the same hazard class as #306's recursive Drop).
    let system = TermSystem::new(vec![Rule::new(deep, Term::var("x")).unwrap()]);
    let config = CompletionConfig {
        precedence: precedence(&[("f", 1)]),
    };
    let mut resources = RelationResources::new(&RelationLimits::default());
    let outcome = complete(&system, &config, &mut resources);
    assert!(
        matches!(outcome, Err(RewriteError::RelationLimitExceeded { .. })),
        "expected typed limit, got {:?}",
        outcome.map(|o| core::mem::discriminant(&o))
    );
    core::mem::forget(system);
}

/// PR #310 round 1 P2: equation fingerprints must preserve
/// cross-side variable identity — `h(x,y)` and `h(x,z)` are
/// DISTINCT equations.
#[test]
fn fingerprints_preserve_cross_side_variable_identity() {
    // The reviewer's exact system: completion must process BOTH
    // distinct pairs and then report the unorientable equation.
    let (x, y, z) = (Term::var("x"), Term::var("y"), Term::var("z"));
    let f_xyz = || Term::sym("f", vec![x.clone(), y.clone(), z.clone()]);
    let g_xyz = || Term::sym("g", vec![x.clone(), y.clone(), z.clone()]);
    let h = |l: Term, r: Term| Term::sym("h", vec![l, r]);
    let rules = vec![
        rule(f_xyz(), h(x.clone(), y.clone())),
        rule(f_xyz(), h(x.clone(), z.clone())),
        rule(g_xyz(), h(x.clone(), y.clone())),
        rule(g_xyz(), h(z.clone(), y.clone())),
        rule(h(x.clone(), y.clone()), x.clone()),
    ];
    let system = TermSystem::new(rules);
    let config = CompletionConfig {
        precedence: precedence(&[("f", 3), ("g", 3), ("h", 2)]),
    };
    let mut resources = RelationResources::new(&RelationLimits::default());
    let outcome = complete(&system, &config, &mut resources).expect("typed outcome");
    match outcome {
        CompletionOutcome::Partial { reason, trace, .. } => {
            assert!(
                matches!(reason, PartialReason::UnorientableRule { .. }),
                "expected UnorientableRule, got {reason:?}"
            );
            assert!(
                trace.pairs_processed >= 2,
                "both distinct pairs must be processed, got {}",
                trace.pairs_processed
            );
        }
        other => panic!("expected Partial, got {other:?}"),
    }
}

/// PR #310 round 1 P2: normalization is bounded by the shared
/// operation budget, not a fixed step cap — a legal input needing
/// MORE than 1,024 contractions must normalize (the cap's resource
/// string must never appear).
#[test]
fn normalization_uses_shared_operation_budget() {
    // T(0) = a; T(n+1) = f(T(n), T(n)): T(10) needs 2,047
    // contractions under {a -> b, f(b,x) -> x}. The pair (T10, x)
    // normalizes to (b, x), which is unorientable, so completion
    // returns Partial BEFORE the confluence postcheck (whose reduct
    // space for doubling trees is exponential by design).
    let mut t10 = Term::constant("a");
    for _ in 0..10 {
        t10 = Term::sym("f", [t10.clone(), t10]);
    }
    let rules = vec![
        rule(Term::sym("p", [Term::var("x")]), t10),
        rule(Term::sym("p", [Term::var("x")]), Term::var("x")),
        rule(Term::constant("a"), Term::constant("b")),
        rule(
            Term::sym("f", [Term::constant("b"), Term::var("x")]),
            Term::var("x"),
        ),
    ];
    let system = TermSystem::new(rules);
    let config = CompletionConfig {
        precedence: precedence(&[("p", 1), ("f", 2), ("a", 0), ("b", 0)]),
    };
    let mut resources = RelationResources::new(&RelationLimits::default());
    let outcome = complete(&system, &config, &mut resources).expect("typed outcome");
    assert!(
        matches!(
            outcome,
            CompletionOutcome::Partial {
                reason: PartialReason::UnorientableRule { .. },
                ..
            }
        ),
        "expected Partial(UnorientableRule) after 2,047 contractions: {:?}",
        core::mem::discriminant(&outcome)
    );
}
