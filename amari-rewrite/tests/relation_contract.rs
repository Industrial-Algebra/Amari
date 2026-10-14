//! Relation authority contract tests (0.25 Cohort 2, Task 6).
//!
//! Covers the foundations every later relation module depends on:
//! caller-tightenable limits against fixed ceilings, deterministic
//! query-scoped logic-variable namespaces, alpha-canonical term
//! digests with framed SHA-256, and resource accounting. Runs under
//! default, no-default, and serialize feature combinations.

use amari_rewrite::relation::{LogicVarNamespace, RelationLimits, RelationResources, Sha256Digest};
use amari_rewrite::trs::Term;
use amari_rewrite::RewriteError;

#[test]
fn zero_limits_are_rejected() {
    for args in [
        (0, 64, 4_096, 1_000_000),
        (4_096, 0, 4_096, 1_000_000),
        (4_096, 64, 0, 1_000_000),
        (4_096, 64, 4_096, 0),
    ] {
        let result = RelationLimits::new(args.0, args.1, args.2, args.3);
        assert!(
            matches!(result, Err(RewriteError::InvalidLimit { .. })),
            "zero limit must be rejected: {args:?}"
        );
    }
}

#[test]
fn oversized_limits_are_rejected_against_fixed_ceilings() {
    let over = RelationLimits::new(RelationLimits::MAX_TERM_NODES + 1, 64, 4_096, 1_000_000);
    assert!(matches!(over, Err(RewriteError::InvalidLimit { .. })));
    let over_depth =
        RelationLimits::new(4_096, RelationLimits::MAX_TERM_DEPTH + 1, 4_096, 1_000_000);
    assert!(matches!(over_depth, Err(RewriteError::InvalidLimit { .. })));
}

#[test]
fn tightened_limits_are_accepted_and_clamped() {
    let limits = RelationLimits::new(16, 4, 8, 100).unwrap();
    assert_eq!(limits.max_term_nodes(), 16);
    assert_eq!(limits.max_term_depth(), 4);
    assert_eq!(limits.max_constraints(), 8);
    assert_eq!(limits.max_operations(), 100);

    let ceilings = RelationLimits::default();
    assert_eq!(ceilings.max_term_nodes(), RelationLimits::MAX_TERM_NODES);
    assert_eq!(ceilings.max_term_depth(), RelationLimits::MAX_TERM_DEPTH);
    assert_eq!(ceilings.max_constraints(), RelationLimits::MAX_CONSTRAINTS);
    assert_eq!(ceilings.max_operations(), RelationLimits::MAX_OPERATIONS);
}

#[test]
fn namespaces_are_deterministic_and_query_scoped() {
    let mut first = LogicVarNamespace::new(7);
    let mut second = LogicVarNamespace::new(7);
    let mut other = LogicVarNamespace::new(8);

    let a: Vec<_> = (0..4).map(|_| first.fresh()).collect();
    let b: Vec<_> = (0..4).map(|_| second.fresh()).collect();
    let c: Vec<_> = (0..4).map(|_| other.fresh()).collect();

    // Same query scope: identical sequences.
    assert_eq!(a, b);
    // Different scopes: disjoint identities.
    assert!(a.iter().all(|v| !c.contains(v)));
    // Distinct indices within a namespace.
    for (index, var) in a.iter().enumerate() {
        assert_eq!(a.iter().filter(|v| *v == var).count(), 1);
        let _ = index;
    }
}

#[test]
fn canonical_digests_are_alpha_invariant() {
    let left = Term::sym(
        "f",
        vec![Term::var("X"), Term::sym("g", vec![Term::var("X")])],
    );
    let right = Term::sym(
        "f",
        vec![Term::var("Y"), Term::sym("g", vec![Term::var("Y")])],
    );
    assert_eq!(
        Sha256Digest::canonical_term("amari.relation.term/v1", &left),
        Sha256Digest::canonical_term("amari.relation.term/v1", &right),
        "alpha-equivalent terms must hash identically"
    );

    let distinct = Term::sym("f", vec![Term::var("X"), Term::var("Y")]);
    let repeated = Term::sym("f", vec![Term::var("X"), Term::var("X")]);
    assert_ne!(
        Sha256Digest::canonical_term("amari.relation.term/v1", &distinct),
        Sha256Digest::canonical_term("amari.relation.term/v1", &repeated),
        "distinct structure must hash distinctly"
    );
    // Symbols and variables never collide.
    assert_ne!(
        Sha256Digest::canonical_term("amari.relation.term/v1", &Term::var("a")),
        Sha256Digest::canonical_term("amari.relation.term/v1", &Term::constant("a")),
    );
}

#[test]
fn framed_digests_are_domain_separated() {
    let payload = b"same bytes";
    let a = Sha256Digest::framed("amari.relation.term/v1", payload);
    let b = Sha256Digest::framed("amari.relation.rule/v1", payload);
    assert_ne!(a, b, "frames must separate identical payloads");
    // Deterministic.
    assert_eq!(a, Sha256Digest::framed("amari.relation.term/v1", payload));
}

#[test]
fn digest_hex_roundtrips_and_validates() {
    let digest = Sha256Digest::framed("amari.relation.term/v1", b"payload");
    let hex = digest.to_hex();
    assert_eq!(hex.len(), 64);
    let parsed = Sha256Digest::parse_hex(&hex).unwrap();
    assert_eq!(parsed, digest);

    assert!(matches!(
        Sha256Digest::parse_hex("abcd"),
        Err(RewriteError::InvalidDigest { .. })
    ));
    assert!(matches!(
        Sha256Digest::parse_hex(&"z".repeat(64)),
        Err(RewriteError::InvalidDigest { .. })
    ));
    // Uppercase hex is rejected: canonical encoding is lowercase.
    assert!(matches!(
        Sha256Digest::parse_hex(&"A".repeat(64)),
        Err(RewriteError::InvalidDigest { .. })
    ));
}

#[cfg(feature = "serialize")]
#[test]
fn authority_types_serde_roundtrip() {
    let mut namespace = LogicVarNamespace::new(3);
    let var = namespace.fresh().expect("allocation succeeds");
    let limits = RelationLimits::new(16, 4, 8, 100).unwrap();
    let mut resources = RelationResources::new(&limits);
    resources.record_operations(2).unwrap();
    let digest = Sha256Digest::framed("amari.relation.term/v1", b"x");

    let var_json = serde_json::to_string(&var).unwrap();
    assert_eq!(
        serde_json::from_str::<amari_rewrite::relation::LogicVar>(&var_json).unwrap(),
        var
    );
    for value in [
        serde_json::to_string(&limits).unwrap(),
        serde_json::to_string(&resources).unwrap(),
        serde_json::to_string(&digest).unwrap(),
        serde_json::to_string(&namespace).unwrap(),
    ] {
        assert!(!value.is_empty());
    }
    let limits_json = serde_json::to_string(&limits).unwrap();
    assert_eq!(
        serde_json::from_str::<RelationLimits>(&limits_json).unwrap(),
        limits
    );
}

#[test]
fn resources_account_terms_constraints_and_operations() {
    let limits = RelationLimits::new(8, 2, 3, 5).unwrap();
    let mut resources = RelationResources::new(&limits);

    // Term accounting: nodes and depth bounded independently.
    resources.record_term(4, 1).unwrap();
    assert!(matches!(
        resources.record_term(9, 1),
        Err(RewriteError::RelationLimitExceeded { .. })
    ));
    assert!(matches!(
        resources.record_term(1, 3),
        Err(RewriteError::RelationLimitExceeded { .. })
    ));

    // Constraint accounting is cumulative.
    resources.record_constraints(2).unwrap();
    assert!(matches!(
        resources.record_constraints(2),
        Err(RewriteError::RelationLimitExceeded { .. })
    ));
    resources.record_constraints(1).unwrap();

    // Operation accounting is cumulative.
    resources.record_operations(4).unwrap();
    assert!(matches!(
        resources.record_operations(2),
        Err(RewriteError::RelationLimitExceeded { .. })
    ));
    resources.record_operations(1).unwrap();
    assert_eq!(resources.operations(), 5);
}

/// PR #302 round 5 P2: a namespace whose cursor is saturated with
/// the candidate name reserved must fail with a typed error, never
/// spin (the saturating counter re-tested the same reserved name
/// forever on a hostile deserialized boundary state).
#[test]
fn exhausted_reserved_namespace_is_a_typed_error() {
    // Reserve every name the cursor can still produce: at the u32
    // boundary the only remaining candidate is `?0.u32::MAX`.
    let mut namespace = LogicVarNamespace::with_reserved(0, [String::from("?0.4294967295")]);
    // Drive the cursor to the boundary deterministically via the
    // crate-visible test hook rather than 2^32 allocations.
    namespace.advance_to(u32::MAX);
    assert!(namespace.fresh().is_none(), "exhaustion must be typed");
}

/// PR #302 round 5 P3: `allocated()` counts SUCCESSFUL allocations,
/// not cursor advances — reserved skips must not inflate the count.
#[test]
fn allocated_counts_successful_allocations_not_cursor_advances() {
    let mut namespace = LogicVarNamespace::with_reserved(0, [String::from("?0.0")]);
    let first = namespace.fresh().expect("allocation succeeds");
    assert_eq!(first.to_string(), "?0.1");
    assert_eq!(namespace.allocated(), 1);
}

/// PR #302 round 6 P1: the allocation counter must not overflow on
/// a hostile boundary state — saturating u64 accounting, no debug
/// panic, no wraparound.
#[cfg(feature = "serialize")]
#[test]
fn allocation_counter_saturates_without_panicking() {
    let mut namespace: LogicVarNamespace = serde_json::from_str(
        r#"{"scope":0,"next":4294967295,"allocated":18446744073709551615,"exhausted":false}"#,
    )
    .expect("boundary state deserializes");
    let terminal = namespace.fresh().expect("terminal variable available");
    assert_eq!(terminal.to_string(), "?0.4294967295");
    assert_eq!(namespace.allocated(), u64::MAX, "saturated, not wrapped");
    assert!(namespace.fresh().is_none(), "namespace now exhausted");
}

/// PR #302 round 6 P3 (pinned documentation): legacy payloads lack
/// the accounting fields; counts reset to zero on deserialization
/// while the cursor is preserved.
#[cfg(feature = "serialize")]
#[test]
fn legacy_payloads_reset_counts_and_preserve_cursor() {
    let mut namespace: LogicVarNamespace =
        serde_json::from_str(r#"{"scope":9,"next":7}"#).expect("legacy payload deserializes");
    assert_eq!(namespace.allocated(), 0);
    let var = namespace.fresh().expect("cursor preserved");
    assert_eq!(var.to_string(), "?9.7");
    assert_eq!(namespace.allocated(), 1);
}
