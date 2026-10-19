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
