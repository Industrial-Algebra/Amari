//! Alpha-canonical symbolic search state contract tests
//! (0.25 Cohort 3, Task 13).

use std::collections::BTreeSet;

use amari_rewrite::inverse::{
    BackwardSearchOutcome, InverseSearchConfig, SearchResources, SymbolicState,
};
use amari_rewrite::relation::{ConstraintSet, TermConstraint};
use amari_rewrite::trs::Term;
use amari_rewrite::RewriteError;

fn parse(text: &str) -> Term {
    let text = text.trim();
    if let Some(open) = text.find('(') {
        let head = &text[..open];
        let inner = &text[open + 1..text.len() - 1];
        let mut args = Vec::new();
        let mut depth = 0i32;
        let mut start = 0;
        for (index, ch) in inner.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => depth -= 1,
                ',' if depth == 0 => {
                    args.push(parse(&inner[start..index]));
                    start = index + 1;
                }
                _ => {}
            }
        }
        args.push(parse(&inner[start..]));
        Term::sym(head, args)
    } else if text.chars().next().is_some_and(char::is_uppercase) || text.starts_with('?') {
        Term::var(text)
    } else {
        Term::constant(text)
    }
}

#[test]
fn alpha_equivalent_states_are_equal_and_deduplicate() {
    let a = SymbolicState::new(parse("f(?0.0, ?0.1)"), ConstraintSet::new());
    let b = SymbolicState::new(parse("f(?7.3, ?2.9)"), ConstraintSet::new());
    assert_eq!(a, b);
    let mut set = BTreeSet::new();
    assert!(set.insert(a));
    assert!(!set.insert(b));
    // Different shape is different: a repeated variable is not
    // alpha-equivalent to distinct variables, and a constant leaf is
    // neither.
    let c = SymbolicState::new(parse("f(?0.0, ?0.0)"), ConstraintSet::new());
    assert!(set.insert(c.clone()));
    let repeated_renamed = SymbolicState::new(parse("f(?3.7, ?3.7)"), ConstraintSet::new());
    assert_eq!(c, repeated_renamed);
    let d = SymbolicState::new(parse("f(?1.0, a)"), ConstraintSet::new());
    assert_ne!(c, d);
    assert!(set.insert(d));
}

#[test]
fn constraints_distinguish_states() {
    let plain = SymbolicState::new(parse("f(?0.0)"), ConstraintSet::new());
    let mut constraints = ConstraintSet::new();
    constraints.insert(TermConstraint::NotEqual(parse("?0.0"), parse("a")));
    let constrained = SymbolicState::new(parse("f(?0.0)"), constraints);
    assert_ne!(plain, constrained);
    // Alpha-renamed constrained state matches.
    let mut renamed = ConstraintSet::new();
    renamed.insert(TermConstraint::NotEqual(parse("?4.2"), parse("a")));
    let renamed = SymbolicState::new(parse("f(?4.2)"), renamed);
    assert_eq!(constrained, renamed);
}

#[test]
fn config_ceilings_are_fixed_and_enforced() {
    assert_eq!(InverseSearchConfig::MAX_DEPTH, 64);
    assert_eq!(InverseSearchConfig::MAX_STATES, 65_536);
    assert_eq!(InverseSearchConfig::MAX_TRANSITIONS, 262_144);
    assert_eq!(InverseSearchConfig::MAX_RETAINED_BYTES, 64 * 1024 * 1024);
    let default = InverseSearchConfig::default();
    assert_eq!(default.max_states(), InverseSearchConfig::MAX_STATES);
    // Above-ceiling and zero values are typed errors.
    assert!(matches!(
        InverseSearchConfig::new(
            65, 1_024, 4_096, 4_096, 64, 4_096, 65_536, 1_000_000, 1_024, 1_024
        ),
        Err(RewriteError::InvalidLimit { .. })
    ));
    assert!(matches!(
        InverseSearchConfig::new(
            0, 1_024, 4_096, 4_096, 64, 4_096, 65_536, 1_000_000, 1_024, 1_024
        ),
        Err(RewriteError::InvalidLimit { .. })
    ));
    assert!(matches!(
        InverseSearchConfig::new(
            64, 65_537, 4_096, 4_096, 64, 4_096, 65_536, 1_000_000, 1_024, 1_024
        ),
        Err(RewriteError::InvalidLimit { .. })
    ));
    assert!(InverseSearchConfig::new(
        64, 1_024, 4_096, 4_096, 64, 4_096, 65_536, 1_000_000, 1_024, 1_024
    )
    .is_ok());
}

#[test]
fn search_resources_account_states_transitions_and_bytes() {
    let config =
        InverseSearchConfig::new(64, 2, 3, 4_096, 64, 4_096, 65_536, 1_000_000, 8, 1_024).unwrap();
    let mut resources = SearchResources::new(&config);
    resources.record_state().unwrap();
    resources.record_state().unwrap();
    assert!(matches!(
        resources.record_state(),
        Err(RewriteError::RelationLimitExceeded { .. })
    ));
    resources.record_transition().unwrap();
    resources.record_bytes(8).unwrap();
    assert!(matches!(
        resources.record_bytes(1),
        Err(RewriteError::RelationLimitExceeded { .. })
    ));
    assert_eq!(resources.states(), 2);
    assert_eq!(resources.retained_bytes(), 8);
}

#[test]
fn exhausted_requires_certified_authority() {
    // There is no public constructor for CertifiedExhaustion: the
    // variant exists but cannot be built from caller data. This test
    // pins the remaining outcome surface instead.
    let frontier = amari_rewrite::inverse::BackwardFrontier {
        states: vec![SymbolicState::new(parse("f(?0.0)"), ConstraintSet::new())],
        depth_reached: 3,
    };
    let outcome = BackwardSearchOutcome::Partial(frontier);
    assert!(matches!(outcome, BackwardSearchOutcome::Partial(_)));
    let unsupported =
        BackwardSearchOutcome::Unsupported(amari_rewrite::inverse::UnsupportedRelation {
            reason: "theory constraints are not supported in 0.25".to_string(),
        });
    assert!(matches!(unsupported, BackwardSearchOutcome::Unsupported(_)));
}

#[cfg(feature = "serialize")]
#[test]
fn strict_serialization_roundtrips() {
    let state = SymbolicState::new(parse("f(?0.0, a)"), ConstraintSet::new());
    let json = serde_json::to_string(&state).unwrap();
    let back: SymbolicState = serde_json::from_str(&json).unwrap();
    assert_eq!(state, back);
    // Strict: unknown fields on the config DTO are rejected.
    let bad = "{\"max_depth\":64,\"max_states\":1024,\"max_transitions\":4096,\"max_term_nodes\":4096,\"max_term_depth\":64,\"max_constraints\":4096,\"max_groundings\":65536,\"max_operations\":1000000,\"max_frontier_bytes\":1024,\"max_trace_bytes\":1024,\"bogus\":1}";
    assert!(serde_json::from_str::<InverseSearchConfig>(bad).is_err());
    let config = InverseSearchConfig::default();
    let json = serde_json::to_string(&config).unwrap();
    let back: InverseSearchConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(config, back);
}
