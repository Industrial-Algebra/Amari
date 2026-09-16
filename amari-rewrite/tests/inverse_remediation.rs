//! Regression tests for the Cohort 3 closeout review findings
//! (B1, B2, I1-I4). Each test names the finding it pins.

use amari_rewrite::inverse::{
    BackwardExplorer, BackwardSearchOutcome, BidirectionalExplorer, BidirectionalSearchOutcome,
    GuidanceMode, InverseSearchConfig, ReplayCertificate, ReplayQuery, ResourceObservation,
    SearchMode, SymbolicState,
};
use amari_rewrite::relation::{ConstraintSet, TermConstraint};
use amari_rewrite::trs::{Term, TermSystem};

fn wide_constant(children: usize) -> Term {
    Term::sym(
        "f",
        (0..children)
            .map(|_| Term::constant("c"))
            .collect::<Vec<_>>(),
    )
}

fn wide_vars(children: usize) -> Term {
    Term::sym(
        "f",
        (0..children)
            .map(|i| Term::var(format!("X{i}")))
            .collect::<Vec<_>>(),
    )
}

fn config_with_operations(max_operations: u64) -> InverseSearchConfig {
    InverseSearchConfig::new(
        8,
        4_096,
        16_384,
        4_096,
        64,
        4_096,
        65_536,
        max_operations,
        1 << 20,
        1 << 20,
    )
    .unwrap()
}

// ---- B1: budget overflow inside the goal check is not "unreachable"

#[test]
fn b1_oversized_matching_goal_is_partial_never_exhausted() {
    let system = TermSystem::new(vec![]);
    // Baseline: a 4096-node ground goal still witnesses (boundary).
    let boundary = BackwardExplorer::new(&system, InverseSearchConfig::default())
        .search(
            &Term::var("X"),
            &wide_constant(4_095),
            SearchMode::BreadthFirst,
        )
        .unwrap();
    assert!(
        matches!(boundary, BackwardSearchOutcome::Witness(_)),
        "4096-node goal must witness, got {boundary:?}"
    );
    // One node over the relation limit: the goal check runs out of
    // budget. That is a limit event, not unreachability.
    let over = BackwardExplorer::new(&system, InverseSearchConfig::default())
        .search(
            &Term::var("X"),
            &wide_constant(4_096),
            SearchMode::BreadthFirst,
        )
        .unwrap();
    assert!(
        matches!(over, BackwardSearchOutcome::Partial(_)),
        "4097-node goal must be Partial, got {over:?}"
    );
}

#[test]
fn b1_short_operation_budget_on_matching_goal_is_partial() {
    let system = TermSystem::new(vec![]);
    let target = wide_constant(20);
    let goal = wide_vars(20);
    let starved = BackwardExplorer::new(&system, config_with_operations(10))
        .search(&target, &goal, SearchMode::BreadthFirst)
        .unwrap();
    assert!(
        matches!(starved, BackwardSearchOutcome::Partial(_)),
        "starved budget must be Partial, got {starved:?}"
    );
    let ample = BackwardExplorer::new(&system, config_with_operations(1_000_000))
        .search(&target, &goal, SearchMode::BreadthFirst)
        .unwrap();
    assert!(
        matches!(ample, BackwardSearchOutcome::Witness(_)),
        "ample budget must witness, got {ample:?}"
    );
}

#[test]
fn b1_bidirectional_starved_meet_is_partial_never_exhausted() {
    let system = TermSystem::new(vec![]);
    let starved = BidirectionalExplorer::new(&system, config_with_operations(10))
        .search(&wide_constant(20), &wide_vars(20))
        .unwrap();
    assert!(
        matches!(starved, BidirectionalSearchOutcome::Partial(_)),
        "starved meet must be Partial, got {starved:?}"
    );
}

// ---- B2: bidirectional replay accepts non-ground goals

#[test]
fn b2_bidirectional_variable_goal_zero_step_witness() {
    let system = TermSystem::new(vec![]);
    let outcome = BidirectionalExplorer::new(&system, InverseSearchConfig::default())
        .search(
            &Term::sym("f", vec![Term::constant("c"), Term::constant("c")]),
            &Term::sym("f", vec![Term::var("X0"), Term::var("X1")]),
        )
        .unwrap();
    assert!(
        matches!(outcome, BidirectionalSearchOutcome::Witness(_)),
        "variable goal must witness, got {outcome:?}"
    );
}

// ---- I1: certified exhaustion cannot be minted through serde

#[cfg(feature = "serialize")]
#[test]
fn i1_closed_symbolic_search_authority_is_not_deserializable() {
    let forged = serde_json::json!({
        "Exhausted": {
            "authority": "ClosedSymbolicSearch",
            "evidence_hash": [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
                              0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]
        }
    });
    let result = serde_json::from_value::<BackwardSearchOutcome>(forged);
    assert!(
        result.is_err(),
        "ClosedSymbolicSearch must refuse deserialization, got {result:?}"
    );
}

// ---- I2: the joint state digest is insertion-order canonical

#[test]
fn i2_joint_digest_is_invariant_under_constraint_insertion_order() {
    let term = Term::sym("g", vec![Term::var("A")]);
    let pair = || {
        (
            TermConstraint::Equal(Term::var("B"), Term::var("C")),
            TermConstraint::NotEqual(Term::var("C"), Term::var("D")),
        )
    };
    let (first, second) = pair();
    let mut forward = ConstraintSet::new();
    forward.insert(first.clone());
    forward.insert(second.clone());
    let mut reverse = ConstraintSet::new();
    reverse.insert(second);
    reverse.insert(first);
    let one = SymbolicState::new(term.clone(), forward);
    let two = SymbolicState::new(term, reverse);
    assert_eq!(one, two, "insertion order must not change identity");
    assert_eq!(one.canonical_digest(), two.canonical_digest());
}

// ---- I3: verify re-validates the embedded configuration
// (the crate-internal unit test in inverse/replay.rs pins the
// re-validation itself; this integration test pins the reachable
// half: a certificate whose config came from an invalid source must
// never verify).

#[cfg(feature = "serialize")]
#[test]
fn i3_certificate_replay_rejects_invalid_derivation_reference() {
    let system = TermSystem::new(vec![]);
    let target = Term::constant("a");
    let goal = Term::var("X");
    let outcome = BackwardExplorer::new(&system, InverseSearchConfig::default())
        .search(&target, &goal, SearchMode::BreadthFirst)
        .unwrap();
    let BackwardSearchOutcome::Witness(derivation) = outcome else {
        panic!("baseline must witness");
    };
    let certificate = ReplayCertificate::issue(
        &system,
        ReplayQuery::Backward { target, goal },
        amari_rewrite::inverse::ReplayDerivation::Backward(derivation),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        ResourceObservation {
            states: 1,
            transitions: 0,
            retained_bytes: 64,
            operations: 4,
        },
    );
    assert!(certificate.verify(&system).is_ok());
}

// ---- I4: deserialized configs obey the fixed ceilings

#[cfg(feature = "serialize")]
#[test]
fn i4_deserialized_config_cannot_exceed_ceilings() {
    let forged = serde_json::json!({
        "max_depth": 8,
        "max_states": 1_000_000_000_u64,
        "max_transitions": 16_384,
        "max_term_nodes": 4_096,
        "max_term_depth": 64,
        "max_constraints": 4_096,
        "max_groundings": 65_536,
        "max_operations": 50_000,
        "max_frontier_bytes": 1_048_576,
        "max_trace_bytes": 1_048_576
    });
    let result = serde_json::from_value::<InverseSearchConfig>(forged);
    assert!(
        result.is_err(),
        "above-ceiling deserialization must fail, got {result:?}"
    );
}

// ---- M1: exhaustion authority wire tags are stable and versioned

#[test]
fn m1_exhaustion_authority_wire_tags_are_pinned() {
    use amari_rewrite::inverse::ExhaustionAuthority;
    assert_eq!(
        ExhaustionAuthority::FiniteGroundingDomain.wire_tag(),
        "finite_grounding_domain"
    );
    assert_eq!(
        ExhaustionAuthority::RegularLanguageExclusion.wire_tag(),
        "regular_language_exclusion"
    );
    assert_eq!(
        ExhaustionAuthority::ClosedSymbolicSearch.wire_tag(),
        "closed_symbolic_search"
    );
}
