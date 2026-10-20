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
/// specialized; one clause derived; two forward replays; one residual
/// round trip. A metering change must re-measure it.
const EXACT_MINIMUM: usize = 325;

fn boundary_limits() -> RelationLimits {
    RelationLimits::new(4_096, 64, 4_096, RelationLimits::MAX_OPERATIONS).unwrap()
}

/// Wide term with `children + 1` nodes (one `w` node plus constants).
fn wide_term(children: usize) -> Term {
    Term::sym("w", (0..children).map(|_| c("z")))
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
