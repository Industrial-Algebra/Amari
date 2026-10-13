// SPDX-License-Identifier: MIT OR Apache-2.0

//! Engine-level contract tests for the regular-language preimage probe
//! (0.25 Cohort 5, Task 29).

#![cfg(feature = "standard-probes")]

use amari_discovery::{DiscoveryError, ProbeEngine, RewritePreimagesOutput};
use serde_json::json;

const PREIMAGES: &str = "amari-probe:rewrite:preimages:v1";

/// Ground rule `a -> b`.
fn ground_rule() -> serde_json::Value {
    json!({
        "lhs": {"kind": "symbol", "name": "a", "arguments": []},
        "rhs": {"kind": "symbol", "name": "b", "arguments": []}
    })
}

/// `f(X) -> g(X)`: left-linear shared (saturation approximation-only).
fn shared_rule() -> serde_json::Value {
    json!({
        "lhs": {"kind": "symbol", "name": "f", "arguments": [
            {"kind": "variable", "name": "X"}
        ]},
        "rhs": {"kind": "symbol", "name": "g", "arguments": [
            {"kind": "variable", "name": "X"}
        ]}
    })
}

/// The language `{ b }`, with `a` named in the alphabet.
fn accepts_b() -> serde_json::Value {
    json!({
        "alphabet": [{"name": "a", "arity": 0}, {"name": "b", "arity": 0}],
        "states": ["q0"],
        "transitions": [{"symbol": "b", "children": [], "parent": "q0"}],
        "finals": ["q0"]
    })
}

/// The language `{ b }` over the alphabet `{a, b, c}`.
fn accepts_b_with_c() -> serde_json::Value {
    json!({
        "alphabet": [
            {"name": "a", "arity": 0},
            {"name": "b", "arity": 0},
            {"name": "c", "arity": 0}
        ],
        "states": ["q0"],
        "transitions": [{"symbol": "b", "children": [], "parent": "q0"}],
        "finals": ["q0"]
    })
}

/// The language `{ g(a) }`.
fn accepts_g_a() -> serde_json::Value {
    json!({
        "alphabet": [
            {"name": "f", "arity": 1},
            {"name": "g", "arity": 1},
            {"name": "a", "arity": 0}
        ],
        "states": ["qa", "qg"],
        "transitions": [
            {"symbol": "a", "children": [], "parent": "qa"},
            {"symbol": "g", "children": ["qa"], "parent": "qg"}
        ],
        "finals": ["qg"]
    })
}

fn run(input: &serde_json::Value) -> serde_json::Value {
    ProbeEngine::new()
        .unwrap()
        .execute(&PREIMAGES.parse().unwrap(), input)
        .unwrap()
        .output
}

fn run_typed(input: &serde_json::Value) -> RewritePreimagesOutput {
    serde_json::from_value(run(input)).unwrap()
}

fn run_error(input: &serde_json::Value) -> DiscoveryError {
    ProbeEngine::new()
        .unwrap()
        .execute(&PREIMAGES.parse().unwrap(), input)
        .unwrap_err()
}

#[test]
fn classify_reports_class_and_capability() {
    let exact = run_typed(&json!({
        "operation": "classify",
        "preimage": "one_step",
        "rules": [ground_rule()]
    }));
    assert_eq!(exact.outcome, "ok");
    assert_eq!(exact.class, "ground");
    assert_eq!(exact.capability, "exact");
    assert_eq!(exact.construction.as_deref(), Some("left_linear_one_step"));
    assert!(exact.unsupported_reasons.is_empty());

    let approximate = run_typed(&json!({
        "operation": "classify",
        "preimage": "saturation",
        "rules": [shared_rule()]
    }));
    assert_eq!(approximate.class, "left_linear_shared");
    assert_eq!(approximate.capability, "approximation_only");
    assert_eq!(
        approximate.construction.as_deref(),
        Some("witnessed_lower_bound")
    );
    assert!(
        !approximate.unsupported_reasons.is_empty(),
        "approximation-only cells must state why"
    );

    // `preimage` defaults to one_step when omitted.
    let defaulted = run_typed(&json!({
        "operation": "classify",
        "rules": [ground_rule()]
    }));
    assert_eq!(defaulted.capability, "exact");
}

#[test]
fn exact_preimage_reports_certificate_digests() {
    for (preimage, expected_construction) in [
        ("one_step", "left_linear_one_step"),
        ("finite_horizon", "finite_horizon_iteration"),
        ("saturation", "gtt_saturation"),
    ] {
        let mut input = json!({
            "operation": "preimage",
            "preimage": preimage,
            "rules": [ground_rule()],
            "language": accepts_b()
        });
        if preimage == "finite_horizon" {
            input["horizon"] = json!(2);
        }
        let output = run_typed(&input);
        assert_eq!(output.outcome, "exact", "{preimage}");
        assert_eq!(output.construction.as_deref(), Some(expected_construction));
        let hash = output.result_hash.clone().expect("result hash");
        assert_eq!(hash.len(), 64, "sha256 hex");
        let certificate = output.certificate_hash.clone().expect("certificate hash");
        assert_eq!(certificate.len(), 64, "sha256 hex");
        assert!(output.result_states.unwrap() >= 1);
        assert!(output.result_transitions.is_some());
        // Never the raw automaton.
        let raw = run(&input);
        assert!(raw.get("language").is_none(), "no raw language");
        assert!(raw.get("automaton").is_none(), "no raw automaton");
    }

    // Deterministic certificates: the same request produces the same
    // digests.
    let request = json!({
        "operation": "preimage",
        "preimage": "one_step",
        "rules": [ground_rule()],
        "language": accepts_b()
    });
    let first = run_typed(&request);
    let second = run_typed(&request);
    assert_eq!(first.result_hash, second.result_hash);
    assert_eq!(first.certificate_hash, second.certificate_hash);
}

#[test]
fn preimage_on_approximation_only_cell_is_typed_error() {
    let error = run_error(&json!({
        "operation": "preimage",
        "preimage": "saturation",
        "rules": [shared_rule()],
        "language": accepts_g_a()
    }));
    assert!(
        matches!(error, DiscoveryError::InvalidInput(ref message) if message.contains("exact")),
        "unexpected error: {error}"
    );
}

#[test]
fn approximate_reports_witnesses_and_truncation() {
    let output = run_typed(&json!({
        "operation": "approximate",
        "preimage": "saturation",
        "rules": [shared_rule()],
        "language": accepts_g_a()
    }));
    assert_eq!(output.outcome, "bounded");
    assert_eq!(output.capability, "approximation_only");
    assert_eq!(
        output.construction.as_deref(),
        Some("witnessed_lower_bound")
    );
    let count = output.witness_count.expect("witness count");
    assert!(count >= 1, "at least one replay witness");
    assert_eq!(output.witness_hashes.len() as u64, count);
    assert!(
        output.witness_hashes.iter().all(|hash| hash.len() == 64),
        "witness digests are sha256 hex"
    );
    assert!(output.truncated.is_some(), "truncation is reported");
    assert_eq!(output.certificate_hash.as_deref().map(str::len), Some(64));
}

#[test]
fn approximate_on_exact_cell_is_typed_error() {
    let error = run_error(&json!({
        "operation": "approximate",
        "preimage": "one_step",
        "rules": [ground_rule()],
        "language": accepts_b()
    }));
    assert!(matches!(error, DiscoveryError::InvalidInput(_)), "{error}");
}

#[test]
fn membership_verdicts_have_known_answers() {
    // `a` rewrites in one step into `b`, so it is in R^-1({b}).
    let proven = run_typed(&json!({
        "operation": "membership",
        "preimage": "one_step",
        "rules": [ground_rule()],
        "language": accepts_b(),
        "term": {"kind": "symbol", "name": "a", "arguments": []}
    }));
    assert_eq!(proven.verdict.as_deref(), Some("proven"));
    assert_eq!(proven.certificate_hash.as_deref().map(str::len), Some(64));

    // `b` has a complete replay and is outside the exact one-step
    // preimage, so it is definitively excluded.
    let excluded = run_typed(&json!({
        "operation": "membership",
        "preimage": "one_step",
        "rules": [ground_rule()],
        "language": accepts_b(),
        "term": {"kind": "symbol", "name": "b", "arguments": []}
    }));
    assert_eq!(excluded.verdict.as_deref(), Some("excluded"));

    // APPROXIMATION-path saturation is bounded by a derived horizon,
    // so its non-membership is never an exclusion: the verdict is
    // unknown at worst. (Exact saturation is a complete construction
    // and CAN exclude — see exact_saturation_excludes_when_complete,
    // PR #286 round 1.)
    let saturation = run_typed(&json!({
        "operation": "membership",
        "preimage": "saturation",
        "rules": [shared_rule()],
        "language": accepts_g_a(),
        "term": {"kind": "symbol", "name": "a", "arguments": []}
    }));
    assert_ne!(saturation.verdict.as_deref(), Some("excluded"));
    assert!(
        matches!(
            saturation.verdict.as_deref(),
            Some("proven") | Some("unknown")
        ),
        "unexpected verdict {:?}",
        saturation.verdict
    );
}

#[test]
fn strict_nested_dtos_reject_extra_fields() {
    // Extra field on a nested rule.
    let error = run_error(&json!({
        "operation": "classify",
        "rules": [{
            "lhs": {"kind": "symbol", "name": "a", "arguments": []},
            "rhs": {"kind": "symbol", "name": "b", "arguments": []},
            "extra": 1
        }]
    }));
    assert!(matches!(error, DiscoveryError::InvalidInput(_)), "{error}");

    // Extra field on a nested term.
    let error = run_error(&json!({
        "operation": "membership",
        "preimage": "one_step",
        "rules": [ground_rule()],
        "language": accepts_b(),
        "term": {"kind": "symbol", "name": "a", "arguments": [], "extra": 1}
    }));
    assert!(matches!(error, DiscoveryError::InvalidInput(_)), "{error}");

    // Extra field at the top level.
    let error = run_error(&json!({
        "operation": "classify",
        "rules": [],
        "bogus": true
    }));
    assert!(matches!(error, DiscoveryError::InvalidInput(_)), "{error}");
}

#[test]
fn tightened_limits_are_enforced_without_clamping() {
    // Nine states exceed the eight-state preimage ceiling.
    let states: Vec<String> = (0..9).map(|index| format!("q{index}")).collect();
    let error = run_error(&json!({
        "operation": "preimage",
        "preimage": "one_step",
        "rules": [ground_rule()],
        "language": {
            "alphabet": [{"name": "a", "arity": 0}],
            "states": states,
            "transitions": [],
            "finals": []
        }
    }));
    assert!(
        matches!(error, DiscoveryError::InvalidInput(ref message) if message.contains("ceiling")),
        "unexpected error: {error}"
    );

    // A horizon above four is rejected.
    let error = run_error(&json!({
        "operation": "preimage",
        "preimage": "finite_horizon",
        "rules": [ground_rule()],
        "language": accepts_b(),
        "horizon": 5
    }));
    assert!(matches!(error, DiscoveryError::InvalidInput(_)), "{error}");

    // A 33-node membership term exceeds the 32-node ceiling.
    let mut term = json!({"kind": "symbol", "name": "a", "arguments": []});
    for _ in 0..32 {
        term = json!({"kind": "symbol", "name": "f", "arguments": [term]});
    }
    let error = run_error(&json!({
        "operation": "membership",
        "preimage": "one_step",
        "rules": [ground_rule()],
        "language": accepts_b(),
        "term": term
    }));
    assert!(
        matches!(error, DiscoveryError::LimitExceeded(_)),
        "unexpected error: {error}"
    );

    // More than 32 rules exceeds the rule ceiling.
    let rules: Vec<serde_json::Value> = (0..33).map(|_| ground_rule()).collect();
    let error = run_error(&json!({
        "operation": "classify",
        "rules": rules
    }));
    assert!(
        matches!(error, DiscoveryError::LimitExceeded(_)),
        "unexpected error: {error}"
    );
}

#[test]
fn unknown_operation_and_preimage_are_typed_errors() {
    let error = run_error(&json!({
        "operation": "frobnicate",
        "rules": [ground_rule()]
    }));
    assert!(
        matches!(error, DiscoveryError::InvalidInput(ref message) if message.contains("operation")),
        "unexpected error: {error}"
    );
    let error = run_error(&json!({
        "operation": "classify",
        "preimage": "frobnicate",
        "rules": [ground_rule()]
    }));
    assert!(
        matches!(error, DiscoveryError::InvalidInput(ref message) if message.contains("preimage")),
        "unexpected error: {error}"
    );
}

#[test]
fn supplied_fields_are_validated_even_when_unused() {
    // `classify` does not consume the horizon, but a supplied
    // over-ceiling horizon is still a typed error: no supplied
    // bounded field bypasses the ceilings (PR #286 round 1).
    let error = run_error(&json!({
        "operation": "classify",
        "rules": [ground_rule()],
        "horizon": 5
    }));
    assert!(
        matches!(error, DiscoveryError::InvalidInput(_)),
        "over-ceiling supplied horizon: {error}"
    );

    // `preimage` does not consume the term either.
    let mut big = json!({"kind": "symbol", "name": "a", "arguments": []});
    for _ in 0..33 {
        big = json!({"kind": "symbol", "name": "f", "arguments": [big]});
    }
    let error = run_error(&json!({
        "operation": "preimage",
        "rules": [ground_rule()],
        "language": accepts_b(),
        "term": big
    }));
    assert!(
        matches!(error, DiscoveryError::LimitExceeded(_)),
        "over-ceiling supplied term: {error}"
    );

    // And an over-ceiling supplied language (nine states) fails
    // `classify` too.
    let error = run_error(&json!({
        "operation": "classify",
        "rules": [ground_rule()],
        "language": {
            "alphabet": [{"name": "a", "arity": 0}],
            "states": ["q0", "q1", "q2", "q3", "q4", "q5", "q6", "q7", "q8"],
            "transitions": [{"symbol": "a", "children": [], "parent": "q0"}],
            "finals": ["q0"]
        }
    }));
    assert!(
        matches!(error, DiscoveryError::InvalidInput(_)),
        "over-ceiling supplied language: {error}"
    );
}

#[test]
fn zero_horizon_finite_preimage_is_the_identity() {
    // Horizon zero is the identity construction: membership reduces to
    // membership in the language itself (PR #286 round 1).
    let member = run_typed(&json!({
        "operation": "membership",
        "preimage": "finite_horizon",
        "horizon": 0,
        "rules": [ground_rule()],
        "language": accepts_b_with_c(),
        "term": {"kind": "symbol", "name": "b", "arguments": []}
    }));
    assert_eq!(member.verdict.as_deref(), Some("proven"));

    let excluded = run_typed(&json!({
        "operation": "membership",
        "preimage": "finite_horizon",
        "horizon": 0,
        "rules": [ground_rule()],
        "language": accepts_b_with_c(),
        "term": {"kind": "symbol", "name": "c", "arguments": []}
    }));
    assert_eq!(excluded.verdict.as_deref(), Some("excluded"));
}

#[test]
fn exact_saturation_excludes_when_complete() {
    // EXACT saturation is a complete construction: non-membership is
    // sound exclusion. The never-excluded guarantee holds only for
    // the APPROXIMATION path (PR #286 round 1).
    let verdict = run_typed(&json!({
        "operation": "membership",
        "preimage": "saturation",
        "rules": [ground_rule()],
        "language": accepts_b_with_c(),
        "term": {"kind": "symbol", "name": "c", "arguments": []}
    }));
    assert_eq!(verdict.verdict.as_deref(), Some("excluded"));
}

/// The language `{ a }` (single state).
fn accepts_a() -> serde_json::Value {
    json!({
        "alphabet": [{"name": "a", "arity": 0}],
        "states": ["q0"],
        "transitions": [{"symbol": "a", "children": [], "parent": "q0"}],
        "finals": ["q0"]
    })
}

#[test]
fn certificate_digest_binds_the_recorded_limit_profile() {
    // Cohort 5 closeout F1: the certificate's recorded limit profile
    // is part of the evidence identity — the same request under two
    // engine operation budgets must yield distinct certificate
    // digests even when the mathematical result is identical.
    let input = json!({
        "operation": "preimage",
        "preimage": "finite_horizon",
        "horizon": 0,
        "rules": [],
        "language": accepts_a()
    });
    let loose = run_typed(&input);
    let tight_limits = amari_discovery::ProbeEngineLimits {
        max_operations: 10_000,
        ..Default::default()
    };
    let tight: RewritePreimagesOutput = serde_json::from_value(
        ProbeEngine::with_limits(tight_limits)
            .unwrap()
            .execute(&PREIMAGES.parse().unwrap(), &input)
            .unwrap()
            .output,
    )
    .unwrap();
    assert_eq!(loose.outcome, "exact");
    assert_eq!(tight.outcome, "exact");
    assert!(
        loose.certificate_hash.is_some(),
        "an exact preimage carries a certificate digest"
    );
    assert_ne!(
        loose.certificate_hash, tight.certificate_hash,
        "the recorded operation budget is part of the certificate's          evidence identity"
    );
}
