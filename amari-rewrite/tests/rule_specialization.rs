// SPDX-License-Identifier: MIT OR Apache-2.0

//! Negative-example specialization contract (Task 36).
//!
//! Every returned rule must cover at least one positive example and no
//! supplied negative example; inseparable examples must surface as a
//! typed inconclusive outcome rather than as a rule that covers a
//! negative.

use amari_rewrite::{
    relation::RelationLimits,
    synthesis::{
        specialize_rules, InconclusiveReason, SpecializationConfig, SpecializationOutcome,
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

fn covers(rule: &Rule, example: &(Term, Term)) -> bool {
    rule.apply_root(&example.0)
        .map(|produced| produced == example.1)
        .unwrap_or(false)
}

fn add_zero_pair(value: Term) -> (Term, Term) {
    (f("add", [c("0"), value.clone()]), value)
}

fn limit_error_resource(error: &RewriteError) -> Option<&'static str> {
    match error {
        RewriteError::RelationLimitExceeded { resource, .. } => Some(resource),
        _ => None,
    }
}

#[test]
fn refined_rules_never_cover_a_negative_example() {
    let positives = vec![add_zero_pair(c("a")), add_zero_pair(c("b"))];
    let negatives = vec![add_zero_pair(c("c"))];

    let outcome = specialize_rules(&positives, &negatives, &SpecializationConfig::default())
        .expect("specialization succeeds");

    let SpecializationOutcome::Refined { rules, coverage: _ } = outcome else {
        panic!("expected refined rules, got {outcome:?}");
    };
    assert!(!rules.is_empty());
    for rule in &rules {
        assert!(
            positives.iter().any(|example| covers(rule, example)),
            "rule {rule:?} covers no positive"
        );
        for negative in &negatives {
            assert!(
                !covers(rule, negative),
                "rule {rule:?} covers negative {negative:?}"
            );
        }
    }
}

#[test]
fn covered_negative_forces_a_discriminating_path_split() {
    // The positive-example generalization covers the variable negative:
    // f(g(_I0), b) -> h(_I0) matches f(g(X), b) and produces h(X).
    let positives = vec![
        (f("f", [f("g", [c("a")]), c("b")]), f("h", [c("a")])),
        (f("f", [f("g", [c("c")]), c("b")]), f("h", [c("c")])),
    ];
    let negatives = vec![(f("f", [f("g", [v("X")]), c("b")]), f("h", [v("X")]))];

    let outcome = specialize_rules(&positives, &negatives, &SpecializationConfig::default())
        .expect("specialization succeeds");

    let SpecializationOutcome::Refined { rules, coverage: _ } = outcome else {
        panic!("expected refined rules, got {outcome:?}");
    };
    assert_eq!(
        rules,
        vec![
            Rule::new(positives[0].0.clone(), positives[0].1.clone()).unwrap(),
            Rule::new(positives[1].0.clone(), positives[1].1.clone()).unwrap(),
        ]
    );
    for rule in &rules {
        assert!(!covers(rule, &negatives[0]));
    }
}

#[test]
fn right_hand_disagreement_is_a_discriminating_path() {
    // Identical left-hand sides: only the right-hand side separates
    // the positives, so specialization must consider those positions.
    let positives = vec![(f("f", [c("a")]), c("b")), (f("f", [c("a")]), c("d"))];
    let negatives = vec![(f("f", [c("a")]), c("e"))];

    let outcome = specialize_rules(&positives, &negatives, &SpecializationConfig::default())
        .expect("specialization succeeds");

    let SpecializationOutcome::Refined { rules, coverage: _ } = outcome else {
        panic!("expected refined rules, got {outcome:?}");
    };
    assert_eq!(rules.len(), 2);
    for rule in &rules {
        assert!(!covers(rule, &negatives[0]));
    }
}

#[test]
fn partition_order_is_deterministic_across_runs_and_input_orders() {
    let positives = vec![
        add_zero_pair(c("a")),
        add_zero_pair(c("b")),
        add_zero_pair(c("d")),
    ];
    let negatives = vec![add_zero_pair(c("c"))];
    let config = SpecializationConfig::default();

    let baseline = specialize_rules(&positives, &negatives, &config).unwrap();
    for _ in 0..4 {
        assert_eq!(
            specialize_rules(&positives, &negatives, &config).unwrap(),
            baseline
        );
    }

    let mut reversed = positives.clone();
    reversed.reverse();
    let mut reversed_negatives = negatives.clone();
    reversed_negatives.reverse();
    assert_eq!(
        specialize_rules(&reversed, &reversed_negatives, &config).unwrap(),
        baseline
    );
}

#[test]
fn inseparable_examples_are_inconclusive() {
    // The positive is itself the rule f(X) -> X, which necessarily
    // covers f(a) -> a; no finer partition exists.
    let positives = vec![(f("f", [v("X")]), v("X"))];
    let negatives = vec![(f("f", [c("a")]), c("a"))];

    let outcome = specialize_rules(&positives, &negatives, &SpecializationConfig::default())
        .expect("inseparability is a typed outcome, not an error");

    assert_eq!(
        outcome,
        SpecializationOutcome::Inconclusive {
            reason: InconclusiveReason::Inseparable,
        }
    );
}

#[test]
fn duplicate_examples_are_deduped_deterministically() {
    let duplicated_positives = vec![
        add_zero_pair(c("a")),
        add_zero_pair(c("a")),
        add_zero_pair(c("b")),
        add_zero_pair(c("b")),
    ];
    let unique_positives = vec![add_zero_pair(c("a")), add_zero_pair(c("b"))];
    let duplicated_negatives = vec![add_zero_pair(c("c")), add_zero_pair(c("c"))];
    let unique_negatives = vec![add_zero_pair(c("c"))];
    let config = SpecializationConfig::default();

    let duplicated =
        specialize_rules(&duplicated_positives, &duplicated_negatives, &config).unwrap();
    let unique = specialize_rules(&unique_positives, &unique_negatives, &config).unwrap();
    assert_eq!(duplicated, unique);

    let SpecializationOutcome::Refined { rules, coverage } = duplicated else {
        panic!("expected refined rules");
    };
    assert_eq!(coverage.positives(), 2);
    assert_eq!(coverage.negatives(), 1);
    assert_eq!(coverage.rules().len(), rules.len());
    for entry in coverage.rules() {
        assert!(!entry.positives().is_empty());
        assert!(entry.negatives().is_empty());
        let mut sorted = entry.positives().to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted, entry.positives());
    }
}

#[test]
fn conflicting_duplicates_are_typed_errors() {
    let pair = add_zero_pair(c("a"));
    let positives = vec![pair.clone()];
    let negatives = vec![pair];
    let error = specialize_rules(&positives, &negatives, &SpecializationConfig::default())
        .expect_err("a pair cannot be both positive and negative");
    assert!(matches!(error, RewriteError::InvalidRule { .. }));
}

#[test]
fn example_ceiling_is_enforced() {
    let config = SpecializationConfig::new(
        1,
        SpecializationConfig::MAX_CANDIDATES,
        SpecializationConfig::MAX_PARTITIONS,
        RelationLimits::default(),
    )
    .unwrap();
    let positives = vec![add_zero_pair(c("a"))];
    let negatives = vec![add_zero_pair(c("c"))];

    let error = specialize_rules(&positives, &negatives, &config).unwrap_err();
    assert_eq!(limit_error_resource(&error), Some("examples"));
}

#[test]
fn term_node_ceiling_is_enforced() {
    let limits = RelationLimits::new(
        2,
        RelationLimits::MAX_TERM_DEPTH,
        RelationLimits::MAX_CONSTRAINTS,
        RelationLimits::MAX_OPERATIONS,
    )
    .unwrap();
    let config = SpecializationConfig::new(
        SpecializationConfig::MAX_EXAMPLES,
        SpecializationConfig::MAX_CANDIDATES,
        SpecializationConfig::MAX_PARTITIONS,
        limits,
    )
    .unwrap();
    // f(g(a)) has three nodes.
    let positives = vec![(f("f", [f("g", [c("a")])]), c("a"))];

    let error = specialize_rules(&positives, &[], &config).unwrap_err();
    assert_eq!(limit_error_resource(&error), Some("term nodes"));
}

#[test]
fn term_depth_ceiling_is_enforced() {
    let limits = RelationLimits::new(
        RelationLimits::MAX_TERM_NODES,
        1,
        RelationLimits::MAX_CONSTRAINTS,
        RelationLimits::MAX_OPERATIONS,
    )
    .unwrap();
    let config = SpecializationConfig::new(
        SpecializationConfig::MAX_EXAMPLES,
        SpecializationConfig::MAX_CANDIDATES,
        SpecializationConfig::MAX_PARTITIONS,
        limits,
    )
    .unwrap();
    // f(g(a)) has depth two.
    let positives = vec![(f("f", [f("g", [c("a")])]), c("a"))];

    let error = specialize_rules(&positives, &[], &config).unwrap_err();
    assert_eq!(limit_error_resource(&error), Some("term depth"));
}

#[test]
fn candidate_ceiling_is_enforced() {
    let config = SpecializationConfig::new(
        SpecializationConfig::MAX_EXAMPLES,
        1,
        SpecializationConfig::MAX_PARTITIONS,
        RelationLimits::default(),
    )
    .unwrap();
    // The first candidate covers the negative, so a second candidate is
    // required after the discriminating split.
    let positives = vec![add_zero_pair(c("a")), add_zero_pair(c("b"))];
    let negatives = vec![add_zero_pair(c("c"))];

    let error = specialize_rules(&positives, &negatives, &config).unwrap_err();
    assert_eq!(limit_error_resource(&error), Some("candidates"));
}

#[test]
fn partition_ceiling_is_enforced() {
    let config = SpecializationConfig::new(
        SpecializationConfig::MAX_EXAMPLES,
        SpecializationConfig::MAX_CANDIDATES,
        1,
        RelationLimits::default(),
    )
    .unwrap();
    let positives = vec![add_zero_pair(c("a")), add_zero_pair(c("b"))];
    let negatives = vec![add_zero_pair(c("c"))];

    let error = specialize_rules(&positives, &negatives, &config).unwrap_err();
    assert_eq!(limit_error_resource(&error), Some("partitions"));
}

#[test]
fn operation_ceiling_is_enforced() {
    let limits = RelationLimits::new(
        RelationLimits::MAX_TERM_NODES,
        RelationLimits::MAX_TERM_DEPTH,
        RelationLimits::MAX_CONSTRAINTS,
        2,
    )
    .unwrap();
    let config = SpecializationConfig::new(
        SpecializationConfig::MAX_EXAMPLES,
        SpecializationConfig::MAX_CANDIDATES,
        SpecializationConfig::MAX_PARTITIONS,
        limits,
    )
    .unwrap();
    let positives = vec![add_zero_pair(c("a"))];
    let negatives = vec![add_zero_pair(c("c"))];

    let error = specialize_rules(&positives, &negatives, &config).unwrap_err();
    assert_eq!(limit_error_resource(&error), Some("operations"));
}

#[test]
fn invalid_config_values_are_rejected() {
    for (examples, candidates, partitions) in [
        (0, 8, 8),
        (SpecializationConfig::MAX_EXAMPLES + 1, 8, 8),
        (8, 0, 8),
        (8, SpecializationConfig::MAX_CANDIDATES + 1, 8),
        (8, 8, 0),
        (8, 8, SpecializationConfig::MAX_PARTITIONS + 1),
    ] {
        let result =
            SpecializationConfig::new(examples, candidates, partitions, RelationLimits::default());
        assert!(
            matches!(result, Err(RewriteError::InvalidLimit { .. })),
            "config ({examples}, {candidates}, {partitions}) was accepted"
        );
    }
}

/// Deterministic small-congruence generator (no RNG dependency).
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    fn below(&mut self, bound: u64) -> u64 {
        self.next() % bound
    }
}

fn generated_term(lcg: &mut Lcg, depth: usize) -> Term {
    if depth == 0 || lcg.below(2) == 0 {
        let constants = ["a", "b", "c"];
        return c(constants[lcg.below(3) as usize]);
    }
    let symbols = ["f", "g"];
    let symbol = symbols[lcg.below(2) as usize];
    f(symbol, [generated_term(lcg, depth - 1)])
}

fn generated_pair(lcg: &mut Lcg) -> (Term, Term) {
    (generated_term(lcg, 2), generated_term(lcg, 2))
}

#[test]
fn property_refined_rules_cover_a_positive_and_zero_negatives() {
    let config = SpecializationConfig::default();
    let mut lcg = Lcg(0x5eed_1234_9abc_def0);

    for _ in 0..32 {
        let positives: Vec<(Term, Term)> = (0..3).map(|_| generated_pair(&mut lcg)).collect();
        let negatives: Vec<(Term, Term)> = (0..2).map(|_| generated_pair(&mut lcg)).collect();

        match specialize_rules(&positives, &negatives, &config) {
            Ok(SpecializationOutcome::Refined { rules, coverage }) => {
                assert!(!rules.is_empty());
                assert_eq!(coverage.rules().len(), rules.len());
                // Coverage indices are in canonical deduped order.
                let mut canonical_positives = positives.clone();
                canonical_positives.sort();
                canonical_positives.dedup();
                let mut covered_positives = 0usize;
                for (index, rule) in rules.iter().enumerate() {
                    let hits: Vec<usize> = (0..canonical_positives.len())
                        .filter(|&positive| covers(rule, &canonical_positives[positive]))
                        .collect();
                    assert!(
                        !hits.is_empty(),
                        "rule {rule:?} covers no positive of {positives:?}"
                    );
                    assert_eq!(
                        hits,
                        coverage.rules()[index].positives(),
                        "coverage report disagrees with replay"
                    );
                    for negative in &negatives {
                        assert!(
                            !covers(rule, negative),
                            "rule {rule:?} covers negative {negative:?}"
                        );
                    }
                    covered_positives += hits.len();
                }
                assert!(covered_positives >= canonical_positives.len());
            }
            Ok(SpecializationOutcome::Inconclusive { reason }) => {
                assert!(matches!(
                    reason,
                    InconclusiveReason::Inseparable | InconclusiveReason::Unrepresentable
                ));
            }
            Err(RewriteError::InvalidRule { .. }) => {
                // Only a positive/negative duplicate may be rejected.
                assert!(positives
                    .iter()
                    .any(|pair| negatives.iter().any(|other| other == pair)));
            }
            Err(other) => panic!("unexpected error: {other:?}"),
        }
    }
}

/// Dismantle a deep term iteratively so dropping it cannot recurse.
fn dismantle(term: Term) {
    let mut stack = vec![term];
    while let Some(mut current) = stack.pop() {
        if let Term::Sym(_, args) = &mut current {
            stack.append(args);
        }
    }
}

#[test]
fn over_depth_examples_are_rejected_before_cloning() {
    // Twenty thousand levels: admission must reject the borrowed input
    // iteratively, before any recursive clone, sort, dedup, or
    // comparison runs (PR #320 round 1 P1). The recursion in the
    // unfixed path aborts the process here.
    let mut deep = c("a");
    for _ in 0..20_000 {
        deep = f("f", [deep]);
    }
    let positives = vec![(deep, c("b"))];

    let error = specialize_rules(&positives, &[], &SpecializationConfig::default())
        .expect_err("over-depth input must be a typed error");
    assert_eq!(limit_error_resource(&error), Some("term depth"));

    for (lhs, rhs) in positives {
        dismantle(lhs);
        dismantle(rhs);
    }
}

#[cfg(feature = "serialize")]
fn config_json(
    examples: usize,
    candidates: usize,
    partitions: usize,
    nodes: usize,
    depth: usize,
    constraints: usize,
    operations: usize,
) -> String {
    format!(
        concat!(
            "{{\"max_examples\":{},\"max_candidates\":{},\"max_partitions\":{},",
            "\"limits\":{{\"max_term_nodes\":{},\"max_term_depth\":{},",
            "\"max_constraints\":{},\"max_operations\":{}}}}}"
        ),
        examples, candidates, partitions, nodes, depth, constraints, operations
    )
}

#[cfg(feature = "serialize")]
#[test]
fn deserialized_config_is_validated() {
    // Derive-only Deserialize would bypass the checked constructor
    // (PR #320 round 1 P2).
    let defaults = (4_096, 64, 4_096, 1_000_000);
    let (nodes, depth, constraints, operations) = defaults;

    for json in [
        config_json(0, 8, 8, nodes, depth, constraints, operations),
        config_json(
            SpecializationConfig::MAX_EXAMPLES + 1,
            8,
            8,
            nodes,
            depth,
            constraints,
            operations,
        ),
        config_json(8, 0, 8, nodes, depth, constraints, operations),
        config_json(8, 8, 0, nodes, depth, constraints, operations),
        config_json(
            8,
            SpecializationConfig::MAX_CANDIDATES + 1,
            8,
            nodes,
            depth,
            constraints,
            operations,
        ),
        config_json(
            8,
            8,
            SpecializationConfig::MAX_PARTITIONS + 1,
            nodes,
            depth,
            constraints,
            operations,
        ),
        // The enclosed RelationLimits derive bypasses its own
        // constructor, so the config must revalidate it.
        config_json(8, 8, 8, 0, depth, constraints, operations),
        config_json(8, 8, 8, nodes, 65, constraints, operations),
        config_json(8, 8, 8, nodes, depth, 0, operations),
        config_json(8, 8, 8, nodes, depth, constraints, 0),
    ] {
        let parsed: Result<SpecializationConfig, _> = serde_json::from_str(&json);
        assert!(parsed.is_err(), "invalid config JSON was accepted: {json}");
    }

    let valid = config_json(8, 8, 8, nodes, depth, constraints, operations);
    let parsed: SpecializationConfig = serde_json::from_str(&valid).expect("valid config");
    assert_eq!(parsed.max_examples(), 8);
    assert_eq!(parsed.max_candidates(), 8);
    assert_eq!(parsed.max_partitions(), 8);
    assert_eq!(parsed.limits().max_term_nodes(), nodes);
    assert_eq!(parsed.limits().max_term_depth(), depth);
}

#[cfg(feature = "serialize")]
#[test]
fn constructor_revalidates_deserialized_relation_limits() {
    // A raw RelationLimits deserializes through its derive, bypassing
    // RelationLimits::new; the config constructor must revalidate
    // every enclosed invariant (PR #320 round 1 P2).
    let limits: RelationLimits = serde_json::from_str(
        r#"{"max_term_nodes":4096,"max_term_depth":64,"max_constraints":4096,"max_operations":0}"#,
    )
    .expect("the limits derive accepts the raw value");
    let error = SpecializationConfig::new(4, 4, 4, limits)
        .expect_err("zero operations must be rejected at construction");
    assert!(matches!(error, RewriteError::InvalidLimit { .. }));
}

#[test]
fn raw_duplicate_stream_exhausts_the_operation_budget() {
    // Ten thousand duplicates must be metered as a stream even though
    // only one deduped example survives (PR #320 round 1 P3).
    let limits = RelationLimits::new(4_096, 64, 4_096, 3).unwrap();
    let config = SpecializationConfig::new(
        SpecializationConfig::MAX_EXAMPLES,
        SpecializationConfig::MAX_CANDIDATES,
        SpecializationConfig::MAX_PARTITIONS,
        limits,
    )
    .unwrap();
    let example = add_zero_pair(c("a"));
    let positives: Vec<(Term, Term)> = (0..10_000).map(|_| example.clone()).collect();

    let error = specialize_rules(&positives, &[], &config).unwrap_err();
    assert_eq!(limit_error_resource(&error), Some("operations"));
}

#[test]
fn admission_meters_term_traversal_before_generalization() {
    // A single five-node example under a ten-operation budget must be
    // rejected while the shape/variable walks run, not silently
    // accepted (PR #320 round 1 P3).
    let limits = RelationLimits::new(4_096, 64, 4_096, 10).unwrap();
    let config = SpecializationConfig::new(
        SpecializationConfig::MAX_EXAMPLES,
        SpecializationConfig::MAX_CANDIDATES,
        SpecializationConfig::MAX_PARTITIONS,
        limits,
    )
    .unwrap();
    let positives = vec![(f("f", [f("g", [f("h", [f("k", [c("a")])])])]), c("b"))];

    let error = specialize_rules(&positives, &[], &config).unwrap_err();
    assert_eq!(limit_error_resource(&error), Some("operations"));
}

#[test]
fn splitting_fixture_is_metered_end_to_end() {
    // This fixture needs 188 metered units end to end; one unit short
    // is a typed operations error (PR #320 round 1 P3). The wide
    // variant in `splitting_meters_every_compared_subtree_node`
    // isolates the split's per-node comparison cost.
    let limits = RelationLimits::new(4_096, 64, 4_096, 187).unwrap();
    let config = SpecializationConfig::new(
        SpecializationConfig::MAX_EXAMPLES,
        SpecializationConfig::MAX_CANDIDATES,
        SpecializationConfig::MAX_PARTITIONS,
        limits,
    )
    .unwrap();
    let positives = vec![add_zero_pair(c("a")), add_zero_pair(c("b"))];
    let negatives = vec![add_zero_pair(c("c"))];

    let error = specialize_rules(&positives, &negatives, &config).unwrap_err();
    assert_eq!(limit_error_resource(&error), Some("operations"));
}

#[test]
fn replay_never_materializes_beyond_the_term_ceiling() {
    // Replaying X -> f(X, X) against g(a) would materialize the
    // five-node f(g(a), g(a)) under a three-node ceiling. Prospective
    // accounting must reject it as a typed error instead (Task 34
    // doctrine; PR #320 round 1 P2/P4 fixture).
    let limits = RelationLimits::new(3, 64, 4_096, 1_000_000).unwrap();
    let config = SpecializationConfig::new(
        SpecializationConfig::MAX_EXAMPLES,
        SpecializationConfig::MAX_CANDIDATES,
        SpecializationConfig::MAX_PARTITIONS,
        limits,
    )
    .unwrap();
    let positives = vec![(v("X"), f("f", [v("X"), v("X")]))];
    let negatives = vec![(f("g", [c("a")]), c("b"))];

    let error = specialize_rules(&positives, &negatives, &config)
        .expect_err("prospective replay must respect the term ceiling");
    assert_eq!(limit_error_resource(&error), Some("term nodes"));
}

// ---- Round 2: bulk work is metered per visited node ----
//
// Each fixture admits its wide terms, then drives one bulk site. The
// measured minimum is the exact "one remaining operation" boundary:
// one unit less must be a typed operations error. Values are measured
// from the implementation; a metering change must re-measure them.

/// Wide term with `children + 1` nodes (one `w` node plus constants).
fn wide_term(children: usize) -> Term {
    Term::sym("w", (0..children).map(|_| c("z")))
}

fn boundary_limits() -> RelationLimits {
    RelationLimits::new(4_096, 64, 4_096, RelationLimits::MAX_OPERATIONS).unwrap()
}

fn run_with_operation_budget(
    positives: &[(Term, Term)],
    negatives: &[(Term, Term)],
    base: &RelationLimits,
    operations: usize,
) -> Result<(), RewriteError> {
    let limits = RelationLimits::new(
        base.max_term_nodes(),
        base.max_term_depth(),
        base.max_constraints(),
        operations,
    )?;
    let config = SpecializationConfig::new(
        SpecializationConfig::MAX_EXAMPLES,
        SpecializationConfig::MAX_CANDIDATES,
        SpecializationConfig::MAX_PARTITIONS,
        limits,
    )?;
    specialize_rules(positives, negatives, &config).map(|_outcome| ())
}

/// Smallest operation budget that completes the run. Success is
/// monotone in the budget, so a binary search finds the boundary.
fn minimal_operation_budget(
    positives: &[(Term, Term)],
    negatives: &[(Term, Term)],
    base: &RelationLimits,
) -> usize {
    let mut low = 1usize;
    let mut high = RelationLimits::MAX_OPERATIONS;
    assert!(
        run_with_operation_budget(positives, negatives, base, high).is_ok(),
        "fixture must succeed under the hard operation ceiling"
    );
    while low < high {
        let middle = low + (high - low) / 2;
        if run_with_operation_budget(positives, negatives, base, middle).is_ok() {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    low
}

/// Assert the one-remaining-operation boundary at one bulk site.
fn assert_operation_boundary(
    positives: &[(Term, Term)],
    negatives: &[(Term, Term)],
    base: &RelationLimits,
    minimum: usize,
    site: &str,
) {
    assert_eq!(
        minimal_operation_budget(positives, negatives, base),
        minimum,
        "{site}: metered minimum changed"
    );
    let error = run_with_operation_budget(positives, negatives, base, minimum - 1)
        .expect_err("one operation short must fail");
    assert_eq!(limit_error_resource(&error), Some("operations"), "{site}");
    assert!(run_with_operation_budget(positives, negatives, base, minimum).is_ok());
}

#[test]
fn equality_meters_every_compared_node() {
    // Two positives share a 4,096-node left-hand side, so the shared
    // disagreement table must compare it in full and clone it in full.
    let wide = wide_term(4_095);
    let narrow = wide_term(2);
    let positives = vec![(wide.clone(), c("a")), (wide, c("b"))];
    let narrow_positives = vec![(narrow.clone(), c("a")), (narrow, c("b"))];
    assert_operation_boundary(&positives, &[], &boundary_limits(), 127_029, "equality");
    assert!(minimal_operation_budget(&narrow_positives, &[], &boundary_limits()) < 127_029 / 2);
}

#[test]
fn binding_traversal_meters_every_borrowed_node() {
    // The rule variable binds the 4,096-node negative left-hand side;
    // the binding is borrowed and its traversal is charged per node.
    let wide = wide_term(4_095);
    let narrow = wide_term(2);
    let positives = vec![(v("X"), v("X"))];
    let negatives = vec![(wide, c("b"))];
    let narrow_negatives = vec![(narrow, c("b"))];
    assert_operation_boundary(
        &positives,
        &negatives,
        &boundary_limits(),
        16_436,
        "binding",
    );
    assert!(
        minimal_operation_budget(&positives, &narrow_negatives, &boundary_limits()) < 16_436 / 2
    );
}

#[test]
fn canonicalization_meters_every_comparison_node() {
    // Two identical wide examples force a full structural comparison
    // while sorting and again while deduplicating.
    let wide = wide_term(4_095);
    let narrow = wide_term(2);
    let positives = vec![(wide.clone(), c("a")), (wide, c("a"))];
    let narrow_positives = vec![(narrow.clone(), c("a")), (narrow, c("a"))];
    assert_operation_boundary(
        &positives,
        &[],
        &boundary_limits(),
        45_077,
        "canonicalization",
    );
    assert!(minimal_operation_budget(&narrow_positives, &[], &boundary_limits()) < 45_077 / 2);
}

#[test]
fn candidate_construction_meters_clones_and_validation() {
    // Generalizing a single wide example clones it once and rescans
    // both sides for RHS variable validation.
    let wide = wide_term(4_095);
    let narrow = wide_term(2);
    let positives = vec![(wide, c("a"))];
    let narrow_positives = vec![(narrow, c("a"))];
    assert_operation_boundary(
        &positives,
        &[],
        &boundary_limits(),
        32_782,
        "candidate construction",
    );
    assert!(minimal_operation_budget(&narrow_positives, &[], &boundary_limits()) < 32_782 / 2);
}

#[test]
fn splitting_meters_every_compared_subtree_node() {
    // The candidate covers the negative, so the split must group the
    // positives; position [0] compares the shared wide subtree in
    // full, and position [1] discriminates.
    let wide = wide_term(4_089);
    let narrow = wide_term(2);
    let positives = vec![
        (f("h", [wide.clone(), c("a")]), c("u")),
        (f("h", [wide, c("b")]), c("v")),
    ];
    let negatives = vec![(f("h", [wide_term(4_089), c("c")]), c("u"))];
    let narrow_positives = vec![
        (f("h", [narrow.clone(), c("a")]), c("u")),
        (f("h", [narrow, c("b")]), c("v")),
    ];
    let narrow_negatives = vec![(f("h", [wide_term(2), c("c")]), c("u"))];
    assert_operation_boundary(
        &positives,
        &negatives,
        &boundary_limits(),
        114_651,
        "splitting",
    );
    assert!(
        minimal_operation_budget(&narrow_positives, &narrow_negatives, &boundary_limits())
            < 114_651 / 2
    );
}

// ---- Round 3: binding lookups charge every visited entry ----

/// A term with `count` distinct variables: every lookup misses the
/// whole binding list accumulated so far.
fn distinct_variables_term(count: usize) -> Term {
    Term::sym("f", (0..count).map(|index| Term::var(format!("X{index}"))))
}

#[test]
fn binding_lookups_charge_every_visited_entry() {
    // k distinct variables cost k(k-1)/2 entry visits per replay and
    // replay runs twice (partition check plus final coverage), so the
    // measured minimum must grow quadratically, not linearly.
    let small = minimal_operation_budget(
        &[(distinct_variables_term(400), c("a"))],
        &[],
        &boundary_limits(),
    );
    let large = minimal_operation_budget(
        &[(distinct_variables_term(800), c("a"))],
        &[],
        &boundary_limits(),
    );
    assert!(
        small >= 79_800,
        "small fixture minimum {small} does not cover one replay's entry visits"
    );
    assert!(
        large >= 3 * small,
        "binding-entry metering must scale quadratically: small={small} large={large}"
    );
    let error = run_with_operation_budget(
        &[(distinct_variables_term(800), c("a"))],
        &[],
        &boundary_limits(),
        large - 1,
    )
    .expect_err("one operation short must fail");
    assert_eq!(limit_error_resource(&error), Some("operations"));
}

#[test]
fn wide_binding_lookup_is_rejected_under_a_fixed_budget() {
    // The reviewer's shape: 4,095 distinct variables need about 8.4
    // million binding-entry visits per replay, far beyond 50,000
    // operations, so the scan must abort promptly with a typed error.
    let limits = RelationLimits::new(4_096, 64, 4_096, 50_000).unwrap();
    let config = SpecializationConfig::new(
        SpecializationConfig::MAX_EXAMPLES,
        SpecializationConfig::MAX_CANDIDATES,
        SpecializationConfig::MAX_PARTITIONS,
        limits,
    )
    .unwrap();
    let positives = vec![(distinct_variables_term(4_095), c("a"))];

    let error = specialize_rules(&positives, &[], &config)
        .expect_err("a linear binding scan cannot be funded by 50,000 operations");
    assert_eq!(limit_error_resource(&error), Some("operations"));
}

// ---- Round 4: fresh names, keyed comparisons, retention cloning ----

#[test]
fn fresh_name_attempts_charge_per_attempt() {
    // Reserved `_I0.._I4090` names live in a wide negative example, so
    // the candidate pattern stays small and the fresh-name loop (4,092
    // attempts for the first generated variable) is what dominates.
    let reserved: Vec<Term> = (0..4_091)
        .map(|index| Term::var(format!("_I{index}")))
        .collect();
    let positives = vec![add_zero_pair(c("a")), add_zero_pair(c("b"))];
    let negatives = vec![(f("f", reserved), c("w"))];
    let narrow_negatives = vec![(f("f", [c("z")]), c("w"))];
    assert_operation_boundary(
        &positives,
        &negatives,
        &boundary_limits(),
        12_392,
        "fresh names",
    );
    assert!(
        minimal_operation_budget(&positives, &narrow_negatives, &boundary_limits()) < 12_392 / 2
    );
}

#[test]
fn disagreement_key_comparisons_charge_per_node() {
    // The second disagreement key shares a 2,038-node prefix with the
    // first, so the keyed lookup must charge every compared node pair.
    let prefix: Vec<Term> = (0..2_038).map(|_| c("z")).collect();
    let key = |tag: &str| {
        let mut args = prefix.clone();
        args.push(c(tag));
        f("w", args)
    };
    let positives = vec![
        (f("g", [c("p"), key("x"), key("y")]), c("u")),
        (f("g", [c("p"), key("z"), key("t")]), c("v")),
    ];
    let small = |tag: &str| f("w", [c("z"), c(tag)]);
    let narrow_positives = vec![
        (f("g", [c("p"), small("x"), small("y")]), c("u")),
        (f("g", [c("p"), small("z"), small("t")]), c("v")),
    ];
    assert_operation_boundary(
        &positives,
        &[],
        &boundary_limits(),
        98_041,
        "disagreement keys",
    );
    assert!(minimal_operation_budget(&narrow_positives, &[], &boundary_limits()) < 98_041 / 2);
}

#[test]
fn retention_clone_charges_per_node() {
    // The accepted rule is wide on both sides; copying it into the
    // result is charged per node before the clone is allocated.
    let wide = wide_term(4_095);
    let narrow = wide_term(2);
    let positives = vec![(wide.clone(), wide)];
    let narrow_positives = vec![(narrow.clone(), narrow)];
    assert_operation_boundary(&positives, &[], &boundary_limits(), 73_732, "retention");
    assert!(minimal_operation_budget(&narrow_positives, &[], &boundary_limits()) < 73_732 / 2);
}
