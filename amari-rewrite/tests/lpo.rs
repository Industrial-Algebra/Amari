// SPDX-License-Identifier: MIT OR Apache-2.0

//! LPO termination-certificate contract tests (0.25, Task 32).
//!
//! Validated strict total precedences, the variable condition, typed
//! resource exhaustion with no partial proof, memoized comparisons,
//! and exhaustive irreflexivity/transitivity properties over a tiny
//! generated signature.

use amari_rewrite::analysis::{lpo_greater, prove_terminating, Precedence, TerminationProof};
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

fn system(rules: Vec<Rule>) -> TermSystem {
    TermSystem::new(rules)
}

fn resources() -> RelationResources {
    RelationResources::new(&RelationLimits::default())
}

fn greater(left: &Term, right: &Term, order: &Precedence) -> bool {
    let mut budget = resources();
    lpo_greater(left, right, order, &mut budget).expect("comparison succeeds")
}

fn x() -> Term {
    Term::var("x")
}

fn y() -> Term {
    Term::var("y")
}

/// The classic addition system `add(0,y) -> y` and
/// `add(s(x),y) -> s(add(x,y))` with `add > s > 0` terminates.
fn addition_system() -> TermSystem {
    system(vec![
        rule(Term::sym("add", vec![Term::constant("0"), y()]), y()),
        rule(
            Term::sym("add", vec![Term::sym("s", vec![x()]), y()]),
            Term::sym("s", vec![Term::sym("add", vec![x(), y()])]),
        ),
    ])
}

/// Test 1: duplicate symbols are rejected at construction.
#[test]
fn precedence_rejects_duplicate_symbols() {
    let duplicate = Precedence::new(vec![(Symbol::new("f"), 1), (Symbol::new("f"), 1)]);
    assert!(
        matches!(duplicate, Err(RewriteError::InvalidRule { .. })),
        "a repeated symbol must not silently overwrite a rank"
    );
}

/// Test 2: a precedence missing a used symbol is rejected, never
/// silently extended.
#[test]
fn precedence_rejects_symbol_missing_from_the_system_order() {
    let system = system(vec![rule(
        Term::sym("f", vec![Term::constant("a")]),
        Term::constant("a"),
    )]);
    let order = precedence(&[("f", 1)]);
    let mut budget = resources();
    let result = prove_terminating(&system, &order, &mut budget);
    assert!(
        matches!(result, Err(RewriteError::InvalidRule { .. })),
        "missing symbol `a` must be a typed rejection"
    );
}

/// Test 3: a rank/arity disagreement with the system is rejected.
#[test]
fn precedence_rejects_arity_disagreement() {
    let system = system(vec![rule(
        Term::sym("f", vec![Term::constant("a")]),
        Term::constant("a"),
    )]);
    let order = precedence(&[("f", 2), ("a", 0)]);
    let mut budget = resources();
    let result = prove_terminating(&system, &order, &mut budget);
    assert!(
        matches!(result, Err(RewriteError::InvalidRule { .. })),
        "`f` declared at arity 2 but used at arity 1 must be rejected"
    );
}

/// Test 4: a bare variable is never greater than anything.
#[test]
fn bare_variables_are_never_greater() {
    let order = precedence(&[("a", 0)]);
    assert!(!greater(&x(), &Term::constant("a"), &order));
    assert!(!greater(&x(), &x(), &order));
    assert!(!greater(&Term::constant("a"), &x(), &order));
}

/// Test 5: `s > x` exactly when `x` occurs in `s` and `s != x`.
#[test]
fn term_is_greater_than_its_occurring_variables() {
    let order = precedence(&[("f", 1), ("a", 0)]);
    assert!(greater(&Term::sym("f", vec![x()]), &x(), &order));
    assert!(!greater(
        &Term::sym("f", vec![Term::constant("a")]),
        &x(),
        &order
    ));
    assert!(!greater(&Term::sym("f", vec![x()]), &y(), &order));
    // `f(x) > f(y)` is false: the variable arguments are incomparable.
    assert!(!greater(
        &Term::sym("f", vec![x()]),
        &Term::sym("f", vec![y()]),
        &order
    ));
}

/// Test 6: the standard addition system is proved terminating.
#[test]
fn known_terminating_system_is_proved() {
    let order = precedence(&[("add", 2), ("s", 1), ("0", 0)]);
    let mut budget = resources();
    match prove_terminating(&addition_system(), &order, &mut budget) {
        Ok(TerminationProof::ProvedTerminating { report }) => {
            assert_eq!(report.rules_checked, 2);
            assert_eq!(report.decreasing_rules, 2);
        }
        other => panic!("expected a termination proof, got {other:?}"),
    }
}

/// Test 7: a self-embedding rule is not decreasing.
#[test]
fn self_embedding_rule_is_not_decreasing() {
    let system = system(vec![rule(
        Term::sym("f", vec![x()]),
        Term::sym("f", vec![Term::sym("f", vec![x()])]),
    )]);
    let order = precedence(&[("f", 1)]);
    let mut budget = resources();
    let proof = prove_terminating(&system, &order, &mut budget).expect("analysis runs");
    assert_eq!(proof, TerminationProof::NotDecreasing { rule_index: 0 });
}

/// Test 8: an unorientable cycle reports the first failing rule.
#[test]
fn unorientable_cycle_reports_the_first_rule() {
    let system = system(vec![
        rule(Term::constant("a"), Term::constant("b")),
        rule(Term::constant("b"), Term::constant("a")),
    ]);
    // `b > a`, so `a -> b` cannot be oriented; the first rule fails.
    let order = precedence(&[("b", 0), ("a", 0)]);
    let mut budget = resources();
    let proof = prove_terminating(&system, &order, &mut budget).expect("analysis runs");
    assert_eq!(proof, TerminationProof::NotDecreasing { rule_index: 0 });
}

/// Every term over `{a, b, x}` with `f`, `g` binary up to five nodes.
fn generated_terms() -> Vec<Term> {
    let size_one = vec![Term::constant("a"), Term::constant("b"), x()];
    let mut size_three = Vec::new();
    for left in &size_one {
        for right in &size_one {
            size_three.push(Term::sym("f", vec![left.clone(), right.clone()]));
            size_three.push(Term::sym("g", vec![left.clone(), right.clone()]));
        }
    }
    let mut size_five = Vec::new();
    for (outer, inner) in [(&size_one, &size_three), (&size_three, &size_one)] {
        for left in outer {
            for right in inner {
                size_five.push(Term::sym("f", vec![left.clone(), right.clone()]));
                size_five.push(Term::sym("g", vec![left.clone(), right.clone()]));
            }
        }
    }
    let mut all = size_one;
    all.extend(size_three);
    all.extend(size_five);
    all
}

/// Test 9: LPO is irreflexive over every generated term.
#[test]
fn lpo_is_irreflexive_on_generated_terms() {
    let order = precedence(&[("f", 2), ("g", 2), ("b", 0), ("a", 0)]);
    for term in generated_terms() {
        assert!(
            !greater(&term, &term, &order),
            "irreflexivity violated at {term:?}"
        );
    }
}

/// Test 10: LPO is transitive over every generated triple.
#[test]
fn lpo_is_transitive_on_generated_terms() {
    let order = precedence(&[("f", 2), ("g", 2), ("b", 0), ("a", 0)]);
    let terms = generated_terms();
    let count = terms.len();
    let mut matrix = vec![vec![false; count]; count];
    for (i, left) in terms.iter().enumerate() {
        for (j, right) in terms.iter().enumerate() {
            matrix[i][j] = greater(left, right, &order);
        }
    }
    for (i, row) in matrix.iter().enumerate() {
        for (j, greater_ij) in row.iter().enumerate() {
            if !greater_ij {
                continue;
            }
            for (k, greater_jk) in matrix[j].iter().enumerate() {
                if *greater_jk {
                    assert!(
                        matrix[i][k],
                        "transitivity violated: {:?} > {:?} > {:?}",
                        terms[i], terms[j], terms[k]
                    );
                }
            }
        }
    }
}

/// Distinct-pair comparisons performed while proving `system`.
fn operations_for(system: &TermSystem, order: &Precedence) -> usize {
    let mut budget = resources();
    let proof = prove_terminating(system, order, &mut budget).expect("analysis runs");
    assert!(matches!(proof, TerminationProof::ProvedTerminating { .. }));
    budget.operations()
}

/// Test 11: the memoized comparison charges each distinct pair once,
/// so a repeated rule adds no comparison work.
#[test]
fn memoization_charges_each_distinct_pair_once() {
    let order = precedence(&[("f", 1), ("g", 1)]);
    let repeated = rule(
        Term::sym("f", vec![Term::sym("g", vec![x()])]),
        Term::sym("g", vec![Term::sym("f", vec![x()])]),
    );
    let single = system(vec![repeated.clone()]);
    let duplicated = system(vec![repeated.clone(), repeated]);
    let once = operations_for(&single, &order);
    let twice = operations_for(&duplicated, &order);
    assert!(
        once > 0,
        "the comparison must charge at least one operation"
    );
    assert_eq!(
        twice, once,
        "an identical memoized rule must not re-charge comparisons"
    );
}

/// Test 12: exceeding a term bound is a typed error, not a verdict.
#[test]
fn term_node_limit_is_typed() {
    let system = system(vec![rule(
        Term::sym("f", vec![Term::constant("a")]),
        Term::constant("a"),
    )]);
    let order = precedence(&[("f", 1), ("a", 0)]);
    let limits = RelationLimits::new(1, 64, 4_096, 1_000_000).expect("limit");
    let mut budget = RelationResources::new(&limits);
    assert!(matches!(
        prove_terminating(&system, &order, &mut budget),
        Err(RewriteError::RelationLimitExceeded { .. })
    ));
}

/// Test 13: exceeding the operation budget aborts without a proof.
#[test]
fn operation_limit_is_typed() {
    let system = system(vec![rule(
        Term::sym("f", vec![Term::sym("g", vec![x()])]),
        Term::sym("g", vec![Term::sym("f", vec![x()])]),
    )]);
    let order = precedence(&[("f", 1), ("g", 1)]);
    let limits = RelationLimits::new(4_096, 64, 4_096, 1).expect("limit");
    let mut budget = RelationResources::new(&limits);
    assert!(matches!(
        prove_terminating(&system, &order, &mut budget),
        Err(RewriteError::RelationLimitExceeded { .. })
    ));
}

/// PR #305 round 1 P1: bounds validation must be iterative and
/// EARLY — a term built iteratively far beyond the depth ceiling
/// must fail with typed RelationLimitExceeded, never overflow the
/// stack through recursive helpers.
#[test]
fn deeply_nested_term_fails_with_typed_limit_not_crash() {
    let mut deep = Term::var("x");
    for _ in 0..20_000 {
        deep = Term::sym("f", [deep]);
    }
    let precedence = precedence(&[("f", 1)]);
    let mut resources = RelationResources::new(&RelationLimits::new(1, 1, 1, 1).unwrap());
    let outcome = lpo_greater(&deep, &Term::var("x"), &precedence, &mut resources);
    assert!(
        matches!(outcome, Err(RewriteError::RelationLimitExceeded { .. })),
        "expected typed limit, got {outcome:?}"
    );
    // A term built adversarially OUTSIDE the library exceeds the
    // depth ceiling; its recursive `Drop` would itself overflow the
    // stack. Leak it instead — recursive drop of unvalidated terms is
    // a pre-existing Term hazard tracked separately.
    core::mem::forget(deep);
}

/// PR #305 round 1 P2: deserialization must route through validated
/// construction — mismatched domains and duplicate ranks reject.
#[cfg(feature = "serialize")]
#[test]
fn precedence_deserialization_validates_invariants() {
    let mismatched: Result<Precedence, _> =
        serde_json::from_str(r#"{"rank":{},"arity":{"f":1,"a":0}}"#);
    assert!(mismatched.is_err(), "domain mismatch must reject");
    let duplicate: Result<Precedence, _> =
        serde_json::from_str(r#"{"rank":{"f":0,"a":0},"arity":{"f":1,"a":0}}"#);
    assert!(duplicate.is_err(), "duplicate ranks must reject");
    let valid: Precedence =
        serde_json::from_str(r#"{"rank":{"f":0,"a":1},"arity":{"f":1,"a":0}}"#).unwrap();
    assert_eq!(valid, precedence(&[("f", 1), ("a", 0)]));
}

/// PR #305 round 1 P2: bare comparisons must reject missing symbols
/// BEFORE equality or subterm short-circuits.
#[test]
fn bare_comparison_rejects_missing_symbols_before_short_circuits() {
    let empty = precedence(&[]);
    let f_of_a = Term::sym("f", [Term::constant("a")]);
    let mut resources = RelationResources::new(&RelationLimits::default());
    let first = lpo_greater(&f_of_a, &Term::constant("a"), &empty, &mut resources);
    assert!(
        matches!(first, Err(RewriteError::InvalidRule { .. })),
        "got {first:?}"
    );
    let equal = lpo_greater(&f_of_a, &f_of_a, &empty, &mut resources);
    assert!(
        matches!(equal, Err(RewriteError::InvalidRule { .. })),
        "got {equal:?}"
    );
}

/// PR #305 round 2 P2: validation must bound its frontier BEFORE
/// enqueueing children — an adversarially WIDE term fails typed
/// without materializing a 100k-entry work stack.
#[test]
fn wide_term_fails_typed_before_frontier_growth() {
    let wide = Term::sym("f", vec![Term::constant("a"); 100_000]);
    let order = precedence(&[("f", 100_000), ("a", 0)]);
    // Node budget at the 4,096 ceiling admits the comparison itself
    // but not the 100k-wide frontier; the op budget is generous so
    // only the frontier bound can fire first.
    let limits = RelationLimits::new(4_096, 64, 1, 1_000_000).unwrap();
    let mut resources = RelationResources::new(&limits);
    let outcome = lpo_greater(&wide, &Term::constant("a"), &order, &mut resources);
    assert!(
        matches!(outcome, Err(RewriteError::RelationLimitExceeded { .. })),
        "expected typed limit, got {outcome:?}"
    );
    // The discriminating signal: a validator that enqueues before
    // enforcing walks ~100k nodes (and allocates the frontier);
    // early frontier rejection charges only the root.
    assert!(
        resources.operations() < 100,
        "frontier must reject early, charged {} ops",
        resources.operations()
    );
    core::mem::forget(wide);
}

/// PR #305 round 3 P1: constants at the exact depth boundary must
/// not be charged a nonexistent child depth — `a > a` under depth
/// limit 1 is `false`, not an error.
#[test]
fn constant_at_exact_depth_boundary_compares() {
    let order = precedence(&[("a", 0)]);
    let limits = RelationLimits::new(4_096, 1, 1, 1_000_000).unwrap();
    let mut resources = RelationResources::new(&limits);
    let outcome = lpo_greater(
        &Term::constant("a"),
        &Term::constant("a"),
        &order,
        &mut resources,
    );
    assert_eq!(outcome, Ok(false), "got {outcome:?}");
}

/// PR #305 round 3 P1: `f^63(a) > a` under default limits (leaf at
/// depth 64, the exact ceiling) returns `true`, and the rule
/// certifies.
#[test]
fn deep_legal_term_at_ceiling_compares_and_certifies() {
    let mut deep = Term::constant("a");
    for _ in 0..63 {
        deep = Term::sym("f", [deep]);
    }
    let order = precedence(&[("f", 1), ("a", 0)]);
    let mut resources = RelationResources::new(&RelationLimits::default());
    assert_eq!(
        lpo_greater(&deep, &Term::constant("a"), &order, &mut resources),
        Ok(true)
    );
    let mut resources = RelationResources::new(&RelationLimits::default());
    let proof = prove_terminating(
        &system(vec![rule(deep, Term::constant("a"))]),
        &order,
        &mut resources,
    )
    .expect("proof completes");
    assert!(matches!(proof, TerminationProof::ProvedTerminating { .. }));
}
