// SPDX-License-Identifier: MIT OR Apache-2.0

//! Inverse (bidirectional) synthesis contract tests (Task 37).
//!
//! A promoted candidate must replay every positive forward, reproduce
//! no negative, and round-trip its residual evidence. An erasing rule
//! never yields an unchecked reverse rule: it yields a residual schema
//! instead. Undetermined and ambiguous evidence is reported as typed
//! [`InverseUnknownReason`] values, never resolved by guessing.

use amari_rewrite::{
    relation::{LogicVar, RelationLimits},
    synthesis::{
        synthesize_inverse, InconclusiveReason, InverseRefutation, InverseSynthesisConfig,
        InverseSynthesisOutcome, InverseUnknownReason, SpecializationConfig,
    },
    trs::{Rule, Term},
    RewriteError,
};

fn c(name: &str) -> Term {
    Term::constant(name)
}

fn v(name: &str) -> Term {
    Term::var(name)
}

fn f(name: &str, args: impl IntoIterator<Item = Term>) -> Term {
    Term::sym(name, args)
}

fn limit_error_resource(error: &RewriteError) -> Option<&'static str> {
    match error {
        RewriteError::RelationLimitExceeded { resource, .. } => Some(resource),
        _ => None,
    }
}

/// Lossless fixture: `k(X) -> m(X)`.
fn lossless_positives() -> Vec<(Term, Term)> {
    vec![
        (f("k", [c("a")]), f("m", [c("a")])),
        (f("k", [c("b")]), f("m", [c("b")])),
    ]
}

/// Erasing fixture: `pair(X, Y) -> snd(X)` erases `Y`.
fn erasing_positives() -> Vec<(Term, Term)> {
    vec![
        (f("pair", [c("a"), c("b")]), f("snd", [c("a")])),
        (f("pair", [c("c"), c("d")]), f("snd", [c("c")])),
    ]
}

/// Residual evidence that both erasing positives are real forward
/// steps of the inferred rule.
fn erasing_residuals() -> Vec<(Term, Term)> {
    erasing_positives()
}

fn candidate(
    outcome: InverseSynthesisOutcome,
) -> (
    Vec<Rule>,
    Vec<amari_rewrite::synthesis::InverseClause>,
    amari_rewrite::synthesis::InverseEvidence,
) {
    match outcome {
        InverseSynthesisOutcome::Candidate {
            rules,
            clauses,
            evidence,
        } => (rules, clauses, evidence),
        other => panic!("expected a candidate, got {other:?}"),
    }
}

#[test]
fn lossless_rule_yields_a_checked_reverse() {
    let (rules, clauses, evidence) = candidate(
        synthesize_inverse(
            &lossless_positives(),
            &[],
            &[],
            &InverseSynthesisConfig::default(),
        )
        .expect("lossless synthesis succeeds"),
    );

    assert_eq!(rules.len(), 1);
    assert_eq!(clauses.len(), 1);
    let clause = &clauses[0];
    assert_eq!(clause.forward(), &rules[0]);
    assert!(
        clause.residual_schema().is_empty(),
        "a lossless clause needs no residual authority"
    );
    assert!(clause.is_reversible());
    assert!(clause.backward().erased_variables().is_empty());

    // The derived reverse is `m(X) -> k(X)`: the checked swap.
    let reverse = clause.reverse_rule().expect("checked reverse rule");
    assert_eq!(
        reverse.lhs(),
        clause.forward().rhs(),
        "the reverse lhs must be the forward rhs"
    );
    assert_eq!(
        reverse.rhs(),
        clause.forward().lhs(),
        "the reverse rhs must be the forward lhs"
    );
    for (source, target) in lossless_positives() {
        assert_eq!(
            reverse.apply_root(&target),
            Some(source),
            "the reverse rule must reproduce every observed source"
        );
    }

    assert_eq!(evidence.positives(), 2);
    assert_eq!(evidence.negatives(), 0);
    assert_eq!(evidence.residuals(), 0);
    assert_eq!(evidence.reversible_clauses(), 1);
    assert_eq!(evidence.residual_clauses(), 0);
    assert_eq!(
        evidence.residual_roundtrips(),
        2,
        "both positive examples must be reverse-replayed"
    );
}

#[test]
fn erasing_clause_without_residual_evidence_is_unknown() {
    // Without residual evidence the reconstruction is not determined:
    // the outcome must be Unknown, never a guessed binding.
    for residuals in [
        Vec::new(),
        // Evidence for no promoted clause is ignored rather than
        // treated as a refutation.
        vec![(f("zzz", [c("a")]), f("yyy", [c("a")]))],
    ] {
        let outcome = synthesize_inverse(
            &erasing_positives(),
            &[],
            &residuals,
            &InverseSynthesisConfig::default(),
        )
        .expect("synthesis succeeds");
        assert!(
            matches!(
                outcome,
                InverseSynthesisOutcome::Unknown {
                    reason: InverseUnknownReason::ResidualUnderdetermined { clause: 0 },
                }
            ),
            "expected residual underdetermination, got {outcome:?}"
        );
    }
}

#[test]
fn residual_evidence_validates_the_schema() {
    let (rules, clauses, evidence) = candidate(
        synthesize_inverse(
            &erasing_positives(),
            &[],
            &erasing_residuals(),
            &InverseSynthesisConfig::default(),
        )
        .expect("erasing synthesis succeeds with residual evidence"),
    );

    assert_eq!(rules.len(), 1);
    let clause = &clauses[0];
    let schema = clause.residual_schema();
    assert_eq!(schema.len(), 1, "exactly one variable is erased");
    let variables: Vec<&str> = schema
        .variables()
        .map(|variable| variable.as_str())
        .collect();
    assert_eq!(variables, vec!["_I1"], "the erased variable is the schema");
    assert_eq!(schema.bindings()[0].0, LogicVar::new(0, 0));
    assert_eq!(
        clause.backward().erased_variables(),
        &[String::from("_I1")],
        "the compiled backward contract agrees with the schema"
    );

    // The core inverse rule: an unchecked `rhs -> lhs` would invent the
    // erased variable, so no reverse rule is synthesized at all.
    assert!(
        !clause.is_reversible(),
        "an erasing clause must not carry a reverse rule"
    );
    assert!(
        Rule::new(
            clause.forward().rhs().clone(),
            clause.forward().lhs().clone()
        )
        .is_err(),
        "the naive swap must be an unchecked rule"
    );

    assert_eq!(evidence.residual_clauses(), 1);
    assert_eq!(evidence.reversible_clauses(), 0);
    assert_eq!(evidence.residual_roundtrips(), 2);
}

#[test]
fn every_erased_variable_gets_a_keyed_schema_entry() {
    let positives = vec![
        (f("trip", [c("a"), c("b"), c("d")]), f("fst", [c("a")])),
        (f("trip", [c("c"), c("e"), c("g")]), f("fst", [c("c")])),
    ];
    let residuals = vec![(f("trip", [c("a"), c("b"), c("d")]), f("fst", [c("a")]))];

    let (_rules, clauses, _evidence) = candidate(
        synthesize_inverse(
            &positives,
            &[],
            &residuals,
            &InverseSynthesisConfig::default(),
        )
        .expect("multi-erasure synthesis succeeds"),
    );

    let schema = clauses[0].residual_schema();
    let entries: Vec<(LogicVar, &str)> = schema
        .bindings()
        .iter()
        .map(|(key, variable)| (*key, variable.as_str()))
        .collect();
    assert_eq!(
        entries,
        vec![(LogicVar::new(0, 0), "_I1"), (LogicVar::new(0, 1), "_I2"),],
        "schema keys follow the Cohort 2 residual convention in name order"
    );
}

#[test]
fn negative_pairs_are_never_accepted() {
    let negatives = vec![(f("k", [c("c")]), f("m", [c("c")]))];
    let (rules, _clauses, evidence) = candidate(
        synthesize_inverse(
            &lossless_positives(),
            &negatives,
            &[],
            &InverseSynthesisConfig::default(),
        )
        .expect("specialization separates the negative"),
    );

    assert!(!rules.is_empty());
    for rule in &rules {
        assert!(
            !rule
                .apply_root(&negatives[0].0)
                .map(|produced| produced == negatives[0].1)
                .unwrap_or(false),
            "rule {rule:?} reproduces the negative example"
        );
    }
    assert_eq!(evidence.negatives(), 1);
    assert_eq!(
        evidence.negative_rejections(),
        rules.len(),
        "every clause must be checked against every negative"
    );
}

#[test]
fn residual_roundtrip_failure_is_refuted() {
    // `h(a)` is not an instance of the inferred rule's left-hand side,
    // so the claimed forward step disproves the schema.
    let residuals = vec![(f("h", [c("a")]), f("snd", [c("a")]))];
    let outcome = synthesize_inverse(
        &erasing_positives(),
        &[],
        &residuals,
        &InverseSynthesisConfig::default(),
    )
    .expect("synthesis succeeds");

    let InverseSynthesisOutcome::Refuted { witness } = &outcome else {
        panic!("expected a refutation, got {outcome:?}");
    };
    assert_eq!(
        witness.refutation(),
        &InverseRefutation::ResidualRoundTrip {
            residual: 0,
            clause: 0,
        },
        "the witness must name the refuting residual and clause"
    );
}

#[test]
fn conflicting_residual_evidence_is_ambiguous() {
    // Both positives rewrite to the same target `snd(a)` from different
    // sources, so the residual evidence gives the same reverse instance
    // class two different reconstructions.
    let positives = vec![
        (f("pair", [c("a"), c("b")]), f("snd", [c("a")])),
        (f("pair", [c("a"), c("c")]), f("snd", [c("a")])),
    ];
    let residuals = positives.clone();

    let outcome = synthesize_inverse(
        &positives,
        &[],
        &residuals,
        &InverseSynthesisConfig::default(),
    )
    .expect("synthesis succeeds");
    assert!(
        matches!(
            outcome,
            InverseSynthesisOutcome::Unknown {
                reason: InverseUnknownReason::Ambiguous { clause: 0 },
            }
        ),
        "expected ambiguity rather than a guessed binding, got {outcome:?}"
    );
}

#[test]
fn underdetermined_examples_are_unknown() {
    // `X -> f(Y)` is not a checked rule, and the single example offers
    // no discriminating position: the examples determine no rule.
    let positives = vec![(v("X"), f("f", [v("Y")]))];
    let outcome = synthesize_inverse(&positives, &[], &[], &InverseSynthesisConfig::default())
        .expect("synthesis reports a typed outcome");

    assert!(
        matches!(
            outcome,
            InverseSynthesisOutcome::Unknown {
                reason: InverseUnknownReason::Underdetermined {
                    detail: InconclusiveReason::Unrepresentable,
                },
            }
        ),
        "expected underdetermination, got {outcome:?}"
    );
}

#[test]
fn empty_positives_are_rejected() {
    let error = synthesize_inverse(
        &[],
        &[],
        &[(c("a"), c("b"))],
        &InverseSynthesisConfig::default(),
    )
    .expect_err("no forward examples means no forward rule");
    assert!(matches!(error, RewriteError::InvalidRule { .. }));
}

#[test]
fn over_depth_examples_are_rejected_before_cloning() {
    // Twenty thousand levels: admission must reject the borrowed input
    // iteratively, before any recursive clone, sort, dedup, or
    // comparison runs. The recursion in the unfixed path aborts the
    // process here.
    let mut deep = c("a");
    for _ in 0..20_000 {
        deep = f("f", [deep]);
    }
    let positives = vec![(deep, c("b"))];

    let result = synthesize_inverse(&positives, &[], &[], &InverseSynthesisConfig::default());
    // Leak the deep term deliberately: a failing assertion unwinds, and
    // a recursive Drop over the 20,000 levels would abort the process
    // instead of reporting the assertion (adversarial-shape REDs must
    // keep their signal).
    std::mem::forget(positives);

    let error = result.expect_err("over-depth input must be a typed error");
    assert_eq!(limit_error_resource(&error), Some("term depth"));
}

#[test]
fn residual_ceiling_is_enforced() {
    let config =
        InverseSynthesisConfig::new(SpecializationConfig::default(), 1).expect("valid config");
    let residuals = vec![
        (f("pair", [c("a"), c("b")]), f("snd", [c("a")])),
        (f("pair", [c("c"), c("d")]), f("snd", [c("c")])),
    ];

    let error = synthesize_inverse(&erasing_positives(), &[], &residuals, &config)
        .expect_err("two residuals exceed a one-residual ceiling");
    assert_eq!(limit_error_resource(&error), Some("residuals"));
}

#[test]
fn invalid_config_values_are_rejected() {
    for residuals in [0, InverseSynthesisConfig::MAX_RESIDUALS + 1] {
        let result = InverseSynthesisConfig::new(SpecializationConfig::default(), residuals);
        assert!(
            matches!(result, Err(RewriteError::InvalidLimit { .. })),
            "residual ceiling {residuals} was accepted"
        );
    }
}

#[test]
fn determinism_permuted_inputs_are_byte_identical() {
    let positives = vec![
        (f("pair", [c("a"), c("b")]), f("snd", [c("a")])),
        (f("pair", [c("c"), c("d")]), f("snd", [c("c")])),
        (f("pair", [c("e"), c("g")]), f("snd", [c("e")])),
    ];
    let residuals = positives.clone();
    let config = InverseSynthesisConfig::default();

    let straight = synthesize_inverse(&positives, &[], &residuals, &config).expect("synthesis");
    let mut rotated_positives = positives.clone();
    rotated_positives.rotate_left(1);
    let mut rotated_residuals = residuals.clone();
    rotated_residuals.rotate_left(2);
    let rotated = synthesize_inverse(&rotated_positives, &[], &rotated_residuals, &config)
        .expect("synthesis");

    assert!(
        matches!(straight, InverseSynthesisOutcome::Candidate { .. }),
        "fixture must promote: {straight:?}"
    );
    assert_eq!(
        format!("{straight:?}"),
        format!("{rotated:?}"),
        "permuted input must produce byte-identical output"
    );
}

#[cfg(feature = "serialize")]
#[test]
fn deserialized_config_is_validated() {
    let defaults = (4_096, 8_192, 8_192, 4_096, 64, 4_096, 1_000_000);
    let (examples, candidates, partitions, nodes, depth, constraints, operations) = defaults;
    let nested = |operations: usize, max_residuals: usize| {
        format!(
            concat!(
                "{{\"specialization\":{{\"max_examples\":{},\"max_candidates\":{},",
                "\"max_partitions\":{},\"limits\":{{\"max_term_nodes\":{},",
                "\"max_term_depth\":{},\"max_constraints\":{},",
                "\"max_operations\":{}}}}},\"max_residuals\":{}}}"
            ),
            examples, candidates, partitions, nodes, depth, constraints, operations, max_residuals
        )
    };

    for json in [
        nested(operations, 0),
        nested(operations, InverseSynthesisConfig::MAX_RESIDUALS + 1),
        // The enclosed specialization config must be revalidated too.
        nested(0, 8),
        // Unknown fields are rejected rather than silently ignored.
        format!(
            "{},\"extra\":1",
            &nested(operations, 8)[..nested(operations, 8).len() - 1]
        ),
    ] {
        let parsed: Result<InverseSynthesisConfig, _> = serde_json::from_str(&json);
        assert!(parsed.is_err(), "invalid config JSON was accepted: {json}");
    }

    let parsed: InverseSynthesisConfig =
        serde_json::from_str(&nested(operations, 8)).expect("valid config");
    assert_eq!(parsed.max_residuals(), 8);
    assert_eq!(parsed.limits().max_term_depth(), depth);
}

// ---- Metered work: measured minimum budgets ----

fn run_with_operation_budget(
    positives: &[(Term, Term)],
    negatives: &[(Term, Term)],
    residuals: &[(Term, Term)],
    base: &RelationLimits,
    operations: usize,
) -> Result<InverseSynthesisOutcome, RewriteError> {
    let limits = RelationLimits::new(
        base.max_term_nodes(),
        base.max_term_depth(),
        base.max_constraints(),
        operations,
    )?;
    let specialization = SpecializationConfig::new(
        SpecializationConfig::MAX_EXAMPLES,
        SpecializationConfig::MAX_CANDIDATES,
        SpecializationConfig::MAX_PARTITIONS,
        limits,
    )?;
    let config =
        InverseSynthesisConfig::new(specialization, InverseSynthesisConfig::MAX_RESIDUALS)?;
    synthesize_inverse(positives, negatives, residuals, &config)
}

/// Smallest operation budget that completes the run. Success is
/// monotone in the budget, so a binary search finds the boundary.
fn minimal_operation_budget(
    positives: &[(Term, Term)],
    negatives: &[(Term, Term)],
    residuals: &[(Term, Term)],
    base: &RelationLimits,
) -> usize {
    let mut low = 1usize;
    let mut high = RelationLimits::MAX_OPERATIONS;
    assert!(
        run_with_operation_budget(positives, negatives, residuals, base, high).is_ok(),
        "fixture must succeed under the hard operation ceiling"
    );
    while low < high {
        let middle = low + (high - low) / 2;
        if run_with_operation_budget(positives, negatives, residuals, base, middle).is_ok() {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    low
}

/// Assert the one-remaining-operation boundary at one measured site.
fn assert_operation_boundary(
    positives: &[(Term, Term)],
    negatives: &[(Term, Term)],
    residuals: &[(Term, Term)],
    base: &RelationLimits,
    minimum: usize,
    site: &str,
) {
    assert_eq!(
        minimal_operation_budget(positives, negatives, residuals, base),
        minimum,
        "{site}: metered minimum changed"
    );
    let error = run_with_operation_budget(positives, negatives, residuals, base, minimum - 1)
        .expect_err("one operation short must fail");
    assert_eq!(limit_error_resource(&error), Some("operations"), "{site}");
    assert!(
        run_with_operation_budget(positives, negatives, residuals, base, minimum).is_ok(),
        "{site}: the measured minimum must succeed"
    );
}

/// Exact end-to-end operation minimum of
/// `residual_roundtrip_fixture_is_metered_end_to_end`, measured from
/// the implementation: two positives admitted, canonicalized, and
/// specialized; one clause compiled (metered, including its rule
/// digest); two forward replays; one residual round trip. A metering
/// change must re-measure it.
const EXACT_MINIMUM: usize = 397;

/// Exact minima for `backward_compilation_is_metered`, measured from
/// the implementation (8 and 64 distinct variables, lossless).
const BACKWARD_COMPILATION_8: usize = 1_370;
const BACKWARD_COMPILATION_64: usize = 44_358;

/// Exact minima for `residual_schema_selection_is_metered`, measured
/// from the implementation (8 and 64 distinct variables, erasing).
const RESIDUAL_SELECTION_8: usize = 830;
const RESIDUAL_SELECTION_64: usize = 21_918;

/// Exact minimum for `sort_comparisons_abort_with_the_typed_error`
/// (64 distinct variable names, lossless), measured from the
/// implementation.
const SORT_FIXTURE_MINIMUM: usize = 44_358;

/// Sort and dedup charges of that 64-name compilation, measured as the
/// difference the per-comparison metering added to its minimum.
const SORT_AND_DEDUP_64: usize = 286;

/// Exact minima for `residual_traversal_is_metered_per_visited_clause`
/// (64 erasing clauses, with and without 64 unrelated residuals),
/// measured from the implementation.
const TRAVERSAL_WITHOUT_UNRELATED: usize = 55_694;
const TRAVERSAL_WITH_UNRELATED: usize = 66_015;

/// Exact minimum for `residual_traversal_is_bypassed_without_erasing_clauses`
/// (64 lossless clauses plus 64 unrelated residuals), measured from the
/// implementation.
const BYPASS_MINIMUM: usize = 20_730;

fn boundary_limits() -> RelationLimits {
    RelationLimits::new(4_096, 64, 4_096, RelationLimits::MAX_OPERATIONS).unwrap()
}

/// Wide term with `children + 1` nodes (one `w` node plus constants).
fn wide_term(children: usize) -> Term {
    Term::sym("w", (0..children).map(|_| c("z")))
}

#[test]
fn residual_evidence_is_scoped_to_compatible_clauses() {
    // Two erasing clauses share the reverse target symbol `out`, so a
    // residual's target matches BOTH clauses' right-hand sides. Each
    // residual is only obligated to the clause whose left-hand side
    // accepts it; the other clause must stay silent (PR #335 round 1
    // P1: a residual was refuted by every overlapping-RHS clause).
    let positives = vec![
        (f("f", [v("X"), v("Y")]), f("out", [v("X")])),
        (f("g", [v("Z"), v("W")]), f("out", [v("Z")])),
    ];
    let residuals = positives.clone();

    let (rules, clauses, evidence) = candidate(
        synthesize_inverse(
            &positives,
            &[],
            &residuals,
            &InverseSynthesisConfig::default(),
        )
        .expect("compatible residual evidence must promote"),
    );

    assert_eq!(rules.len(), 2, "one clause per constructor family");
    assert_eq!(clauses.len(), 2);
    for clause in &clauses {
        assert_eq!(
            clause.residual_schema().len(),
            1,
            "each family erases exactly one variable"
        );
        assert!(!clause.is_reversible());
    }
    assert_eq!(evidence.residual_clauses(), 2);
    assert_eq!(
        evidence.residual_roundtrips(),
        2,
        "both residuals are attributed to their compatible clause"
    );
}

/// `p(X0,..,Xn-1) -> q(X0,..,Xn-1)`: lossless, so the backward
/// compilation and its rule digest are the only quadratic sites.
fn many_variable_lossless(count: usize) -> (Term, Term) {
    let variables: Vec<Term> = (0..count).map(|index| v(&format!("X{index}"))).collect();
    (Term::sym("p", variables.clone()), f("q", variables))
}

/// `p(X0,..,Xn-1) -> q(X0)`: `n - 1` erased variables, so the schema
/// selection and residual instance capture run quadratically too.
fn many_variable_erasing(count: usize) -> (Term, Term) {
    let variables: Vec<Term> = (0..count).map(|index| v(&format!("X{index}"))).collect();
    (Term::sym("p", variables), f("q", [v("X0")]))
}

#[test]
fn backward_compilation_is_metered() {
    // Compiling the backward contract renumbers every variable
    // occurrence, scans the accumulated names, and hashes the payload;
    // all of it is quadratic in the variable count and must be charged
    // per visited entry (PR #335 round 1 P2a). The 8- and 64-variable
    // twins pin the measured minima: a 33x spread is the per-entry
    // signal, since a per-term charge would leave them nearly equal.
    let wide = many_variable_lossless(64);
    let narrow = many_variable_lossless(8);
    assert_operation_boundary(
        std::slice::from_ref(&narrow),
        &[],
        &[],
        &boundary_limits(),
        BACKWARD_COMPILATION_8,
        "backward compilation (8 variables)",
    );
    assert_operation_boundary(
        std::slice::from_ref(&wide),
        &[],
        &[],
        &boundary_limits(),
        BACKWARD_COMPILATION_64,
        "backward compilation (64 variables)",
    );
}

#[test]
fn residual_schema_selection_is_metered() {
    // Selecting the reverse instance keys scans every left-hand
    // variable against every schema entry; the scan is charged per
    // visited entry rather than per variable (PR #335 round 1 P2b).
    // The 8- and 64-variable twins pin the measured minima: a 27x
    // spread is the per-entry signal.
    let wide = many_variable_erasing(64);
    let narrow = many_variable_erasing(8);
    assert_operation_boundary(
        std::slice::from_ref(&narrow),
        &[],
        std::slice::from_ref(&narrow),
        &boundary_limits(),
        RESIDUAL_SELECTION_8,
        "residual schema selection (8 variables)",
    );
    assert_operation_boundary(
        std::slice::from_ref(&wide),
        &[],
        std::slice::from_ref(&wide),
        &boundary_limits(),
        RESIDUAL_SELECTION_64,
        "residual schema selection (64 variables)",
    );
}

/// 64 lossless clauses from 64 structurally distinct constants: one
/// ground rule per positive, none of them erasing.
fn many_lossless_clauses(count: usize) -> Vec<(Term, Term)> {
    (0..count)
        .map(|index| {
            let key = format!("c{index}");
            (f("k", [c(&key)]), f("m", [c(&key)]))
        })
        .collect()
}

/// Residual examples that match no promoted clause's reverse target.
fn unrelated_residuals(count: usize, tag: &str) -> Vec<(Term, Term)> {
    (0..count)
        .map(|index| {
            let key = format!("{tag}{index}");
            (f("zzz", [c(&key)]), f("yyy", [c(&key)]))
        })
        .collect()
}

#[test]
fn sort_comparisons_abort_with_the_typed_error() {
    // Sorting the variable names of a 64-name rule performs far more
    // comparisons than entries; the measured minimum pins them, and a
    // budget spent during the sort aborts promptly with the typed
    // error instead of finishing the sort and truncating the outcome
    // (PR #335 round 2 P2a).
    let wide = many_variable_lossless(64);
    let minimum =
        minimal_operation_budget(std::slice::from_ref(&wide), &[], &[], &boundary_limits());
    assert_eq!(
        minimum, SORT_FIXTURE_MINIMUM,
        "sort fixture: metered minimum changed"
    );

    // Everything below the minimum fails, and the failures that land
    // inside the sort window (the units between the pre-sort cost and
    // the post-sort cost) are typed instead of silent.
    for shortfall in [1, SORT_AND_DEDUP_64] {
        let error = run_with_operation_budget(
            std::slice::from_ref(&wide),
            &[],
            &[],
            &boundary_limits(),
            minimum - shortfall,
        )
        .expect_err("a budget short by the sort charge must fail");
        assert_eq!(limit_error_resource(&error), Some("operations"));
    }
    assert!(
        run_with_operation_budget(
            std::slice::from_ref(&wide),
            &[],
            &[],
            &boundary_limits(),
            minimum
        )
        .is_ok(),
        "the measured minimum must succeed"
    );
}

/// `count` erasing families: `p{i}(X, Y) -> out{i}(X)` each erases
/// `Y`, and their reverse targets are disjoint.
fn erasing_families(count: usize) -> Vec<(Term, Term)> {
    (0..count)
        .map(|index| {
            (
                f(&format!("p{index}"), [v("X"), v("Y")]),
                f(&format!("out{index}"), [v("X")]),
            )
        })
        .collect()
}

#[test]
fn residual_traversal_is_metered_per_visited_clause() {
    // Every unrelated residual is offered to every erasing clause
    // before it can be dismissed, so the traversal costs one unit per
    // attempt plus one per visited clause. With 64 clauses and 64
    // unrelated residuals that is 4,160 charged units; a per-residual
    // charge could not cover the clause visits (PR #335 round 2 P2b).
    let positives = erasing_families(64);
    let own = positives.clone();
    let unrelated = unrelated_residuals(64, "u");

    let without = minimal_operation_budget(&positives, &[], &own, &boundary_limits());
    let mut with_residuals = own.clone();
    with_residuals.extend(unrelated);
    let with = minimal_operation_budget(&positives, &[], &with_residuals, &boundary_limits());

    assert_eq!(
        without, TRAVERSAL_WITHOUT_UNRELATED,
        "traversal fixture: metered minimum changed"
    );
    assert_eq!(
        with, TRAVERSAL_WITH_UNRELATED,
        "traversal fixture with unrelated residuals: metered minimum changed"
    );
    assert!(
        with >= without + 4_000,
        "every visited clause must be charged: without={without} with={with}"
    );
}

#[test]
fn residual_traversal_is_bypassed_without_erasing_clauses() {
    // No clause carries a schema, so no residual can be obligated: the
    // traversal is bypassed entirely and the cost of the fixture is
    // exactly its no-residual cost plus the charged admission and
    // canonicalization of the residual stream. The establishing clause
    // scan is charged before the bypass (PR #335 round 2 P2b).
    let positives = many_lossless_clauses(64);
    let residuals = unrelated_residuals(64, "v");
    let outcome = synthesize_inverse(
        &positives,
        &[],
        &residuals,
        &InverseSynthesisConfig::default(),
    )
    .expect("synthesis succeeds");
    assert!(
        matches!(outcome, InverseSynthesisOutcome::Candidate { .. }),
        "lossless evidence must promote: {outcome:?}"
    );

    let with = minimal_operation_budget(&positives, &[], &residuals, &boundary_limits());
    assert_eq!(
        with, BYPASS_MINIMUM,
        "bypass fixture: metered minimum changed (a charged traversal \
         would add one unit per attempt plus one per visited clause)"
    );
}

#[test]
fn residual_roundtrip_fixture_is_metered_end_to_end() {
    // This fixture needs EXACT operations end to end (measured from the
    // implementation; a metering change must re-measure it): two
    // positives admitted, canonicalized, specialized; one clause
    // derived; two forward replays; one residual round trip.
    let positives = erasing_positives();
    let residuals = erasing_residuals();
    let narrow = [
        (f("pair", [c("a"), c("b")]), f("snd", [c("a")])),
        (f("pair", [c("c"), c("d")]), f("snd", [c("c")])),
    ];
    assert_operation_boundary(
        &positives,
        &[],
        &residuals,
        &boundary_limits(),
        EXACT_MINIMUM,
        "residual round trip",
    );
    let narrow_minimum =
        minimal_operation_budget(&narrow[..1], &[], &narrow[..1], &boundary_limits());
    assert!(
        narrow_minimum < EXACT_MINIMUM / 2,
        "a single-example run must cost less than half the two-example run"
    );
}

#[test]
fn residual_evidence_meters_every_visited_node() {
    // The erased binding is cloned into the residual instance, so the
    // measured minimum must grow with the width of that binding.
    let positives = erasing_positives();
    let wide = vec![(f("pair", [c("a"), wide_term(4_092)]), f("snd", [c("a")]))];
    let narrow = vec![(f("pair", [c("a"), c("b")]), f("snd", [c("a")]))];
    let wide_minimum = minimal_operation_budget(&positives, &[], &wide, &boundary_limits());
    let narrow_minimum = minimal_operation_budget(&positives, &[], &narrow, &boundary_limits());
    assert!(
        wide_minimum >= narrow_minimum + 4_000,
        "residual evidence must charge per visited node: \
         narrow={narrow_minimum} wide={wide_minimum}"
    );

    let error =
        run_with_operation_budget(&positives, &[], &wide, &boundary_limits(), wide_minimum - 1)
            .expect_err("one operation short must fail");
    assert_eq!(limit_error_resource(&error), Some("operations"));
}
