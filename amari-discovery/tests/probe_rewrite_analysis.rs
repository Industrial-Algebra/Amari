// SPDX-License-Identifier: MIT OR Apache-2.0

//! Engine-level contract tests for the Cohort 6 analysis and
//! completion probes (0.25 Task 35).
//!
//! These cover the direct-call surface, the process-isolated surface,
//! and the adversarial ceilings. Semantic catalog wiring lives in
//! `rewrite_discovery_analysis.rs`.

#![cfg(feature = "standard-probes")]

use amari_discovery::{
    DiscoveryError, ProbeEngine, RewriteAnalysisOutput, RewriteCompletionOutput,
};
use assert_cmd::Command;
use serde_json::{json, Value};
use tempfile::TempDir;

const ANALYSIS: &str = "amari-probe:rewrite:analysis:v1";
const COMPLETION: &str = "amari-probe:rewrite:completion:v1";

fn var(name: &str) -> Value {
    json!({"kind": "variable", "name": name})
}

fn sym0(name: &str) -> Value {
    json!({"kind": "symbol", "name": name, "arguments": []})
}

fn sym1(name: &str, argument: Value) -> Value {
    json!({"kind": "symbol", "name": name, "arguments": [argument]})
}

fn sym2(name: &str, left: Value, right: Value) -> Value {
    json!({"kind": "symbol", "name": name, "arguments": [left, right]})
}

fn rule(lhs: Value, rhs: Value) -> Value {
    json!({"lhs": lhs, "rhs": rhs})
}

/// `f(x) -> a` and `f(x) -> b`: one root overlap.
fn overlapping_rules() -> Vec<Value> {
    vec![
        rule(sym1("f", var("x")), sym0("a")),
        rule(sym1("f", var("x")), sym0("b")),
    ]
}

/// The overlapping pair plus `a -> c` and `b -> c`, so the pair joins.
fn joinable_rules() -> Vec<Value> {
    let mut rules = overlapping_rules();
    rules.push(rule(sym0("a"), sym0("c")));
    rules.push(rule(sym0("b"), sym0("c")));
    rules
}

/// `a -> b`.
fn ground_rule() -> Vec<Value> {
    vec![rule(sym0("a"), sym0("b"))]
}

fn precedence(entries: &[(&str, u64)]) -> Vec<Value> {
    entries
        .iter()
        .map(|(name, arity)| json!({"name": name, "arity": arity}))
        .collect()
}

fn run(probe: &str, input: &Value) -> Value {
    ProbeEngine::new()
        .unwrap()
        .execute(&probe.parse().unwrap(), input)
        .unwrap()
        .output
}

fn run_analysis(input: &Value) -> RewriteAnalysisOutput {
    serde_json::from_value(run(ANALYSIS, input)).unwrap()
}

fn run_completion(input: &Value) -> RewriteCompletionOutput {
    serde_json::from_value(run(COMPLETION, input)).unwrap()
}

fn run_error(probe: &str, input: &Value) -> DiscoveryError {
    ProbeEngine::new()
        .unwrap()
        .execute(&probe.parse().unwrap(), input)
        .unwrap_err()
}

fn command_json(arguments: &[&str]) -> Value {
    let output = Command::cargo_bin("amari")
        .unwrap()
        .args(arguments)
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).unwrap()
}

fn is_typed_resource_or_input_error(error: &DiscoveryError) -> bool {
    matches!(
        error,
        DiscoveryError::InvalidInput(_) | DiscoveryError::LimitExceeded(_)
    )
}

#[test]
fn critical_pairs_reports_overlap_evidence() {
    let input = json!({
        "operation": "critical_pairs",
        "rules": overlapping_rules()
    });
    let output = run_analysis(&input);
    assert_eq!(output.outcome, "ok");
    assert_eq!(output.operation, "critical_pairs");
    assert_eq!(output.rule_count, 2);
    assert_eq!(output.pair_count, Some(1));
    assert_eq!(output.trivial_excluded, Some(0));
    assert!(output.non_left_linear_rules.is_empty());
    assert_eq!(output.pairs_truncated, Some(false));
    assert_eq!(output.pairs.len(), 1);
    let pair = &output.pairs[0];
    assert_eq!((pair.left_rule, pair.right_rule), (0, 1));
    assert!(pair.path.is_empty(), "root overlap");
    assert_eq!(pair.outcome, "overlap");
    assert_eq!(
        serde_json::to_value(pair.overlap.as_ref().expect("overlap term")).unwrap()["name"],
        json!("f")
    );
    assert_eq!(
        serde_json::to_value(pair.overlap.as_ref().unwrap()).unwrap()["arguments"]
            .as_array()
            .map(Vec::len),
        Some(1),
        "the unified overlap keeps one argument position"
    );
    assert_eq!(
        pair.root_step.as_ref(),
        Some(&serde_json::from_value(sym0("a")).unwrap())
    );
    assert_eq!(
        pair.position_step.as_ref(),
        Some(&serde_json::from_value(sym0("b")).unwrap())
    );
    assert!(pair.join_term.is_none());

    // Deterministic: the same request produces the same evidence.
    assert_eq!(run_analysis(&input), output);
}

#[test]
fn confluence_certifies_joinability_and_blocks_non_left_linear() {
    let joinable = run_analysis(&json!({
        "operation": "confluence",
        "rules": joinable_rules()
    }));
    assert_eq!(joinable.pair_count, Some(1));
    assert_eq!(joinable.joinable_count, Some(1));
    assert_eq!(joinable.not_joinable_count, Some(0));
    assert_eq!(joinable.locally_confluent, Some(true));
    assert!(joinable.certification_blocked.is_empty());
    assert_eq!(joinable.pairs[0].outcome, "joinable");
    assert_eq!(
        joinable.pairs[0].join_term,
        Some(serde_json::from_value(sym0("c")).unwrap())
    );

    let non_joinable = run_analysis(&json!({
        "operation": "confluence",
        "rules": overlapping_rules()
    }));
    assert_eq!(non_joinable.joinable_count, Some(0));
    assert_eq!(non_joinable.not_joinable_count, Some(1));
    assert_eq!(non_joinable.locally_confluent, Some(false));
    assert_eq!(non_joinable.pairs[0].outcome, "not_joinable");
    assert!(non_joinable.pairs[0].join_term.is_none());

    // A non-left-linear rule blocks certification even with no pairs.
    let blocked = run_analysis(&json!({
        "operation": "confluence",
        "rules": [rule(sym2("f", var("x"), var("x")), sym0("a"))]
    }));
    assert_eq!(blocked.pair_count, Some(0));
    assert_eq!(blocked.certification_blocked, vec![0]);
    assert_eq!(blocked.locally_confluent, Some(false));
}

#[test]
fn termination_reports_proof_and_not_decreasing() {
    let proved = run_analysis(&json!({
        "operation": "termination",
        "rules": ground_rule(),
        "precedence": precedence(&[("a", 0), ("b", 0)])
    }));
    assert_eq!(proved.termination.as_deref(), Some("proved_terminating"));
    let report = proved.termination_report.expect("termination report");
    assert_eq!((report.rules_checked, report.decreasing_rules), (1, 1));
    assert!(proved.failing_rule.is_none());

    let failing = run_analysis(&json!({
        "operation": "termination",
        "rules": ground_rule(),
        "precedence": precedence(&[("b", 0), ("a", 0)])
    }));
    assert_eq!(failing.termination.as_deref(), Some("not_decreasing"));
    assert_eq!(failing.failing_rule, Some(0));
    assert!(failing.termination_report.is_none());

    // The precedence is required and must cover every used symbol.
    let missing = run_error(
        ANALYSIS,
        &json!({"operation": "termination", "rules": ground_rule()}),
    );
    assert!(
        matches!(missing, DiscoveryError::InvalidInput(ref message) if message.contains("precedence")),
        "unexpected error: {missing}"
    );
    let uncovered = run_error(
        ANALYSIS,
        &json!({
            "operation": "termination",
            "rules": ground_rule(),
            "precedence": precedence(&[("a", 0)])
        }),
    );
    assert!(
        matches!(uncovered, DiscoveryError::InvalidInput(ref message) if message.contains("termination")),
        "unexpected error: {uncovered}"
    );
    let duplicate = run_error(
        ANALYSIS,
        &json!({
            "operation": "termination",
            "rules": ground_rule(),
            "precedence": precedence(&[("a", 0), ("a", 0)])
        }),
    );
    assert!(
        matches!(duplicate, DiscoveryError::InvalidInput(_)),
        "{duplicate}"
    );
}

#[test]
fn completion_reports_complete_partial_and_failed_outcomes() {
    let complete = run_completion(&json!({
        "operation": "complete",
        "rules": ground_rule(),
        "precedence": precedence(&[("a", 0), ("b", 0)])
    }));
    assert_eq!(complete.outcome, "complete");
    assert_eq!(complete.system_rule_count, Some(1));
    assert_eq!(complete.rules_added, Some(0));
    assert_eq!(complete.pairs_processed, Some(0));
    assert_eq!(complete.trace_step_count, Some(0));
    assert!(complete.trace_steps.is_empty());
    assert_eq!(
        complete.system_hash.as_deref().map(str::len),
        Some(64),
        "canonical sha256 hex system digest"
    );
    assert!(complete.partial_reason.is_none());
    assert!(complete.failed_reason.is_none());

    // Deterministic completion digest.
    let repeated = run_completion(&json!({
        "operation": "complete",
        "rules": ground_rule(),
        "precedence": precedence(&[("a", 0), ("b", 0)])
    }));
    assert_eq!(complete.system_hash, repeated.system_hash);

    // Neither direction of `f(x,y) -> f(y,x)` is strictly decreasing.
    let partial = run_completion(&json!({
        "operation": "complete",
        "rules": [rule(sym2("f", var("x"), var("y")), sym2("f", var("y"), var("x")))],
        "precedence": precedence(&[("f", 2)])
    }));
    assert_eq!(partial.outcome, "partial");
    assert_eq!(partial.partial_reason.as_deref(), Some("unorientable_rule"));
    assert_eq!(partial.system_rule_count, Some(0));
    assert!(partial.failed_reason.is_none());

    // A precedence that does not cover the input is a fatal failure.
    let uncovered = run_completion(&json!({
        "operation": "complete",
        "rules": ground_rule(),
        "precedence": precedence(&[("a", 0)])
    }));
    assert_eq!(uncovered.outcome, "failed");
    assert_eq!(uncovered.failed_reason.as_deref(), Some("invalid_input"));
    assert!(uncovered.system_hash.is_none());

    // Non-left-linear input is a fatal failure.
    let non_left_linear = run_completion(&json!({
        "operation": "complete",
        "rules": [rule(sym2("f", var("x"), var("x")), sym0("a"))],
        "precedence": precedence(&[("f", 2), ("a", 0)])
    }));
    assert_eq!(non_left_linear.outcome, "failed");
    assert_eq!(
        non_left_linear.failed_reason.as_deref(),
        Some("invalid_input")
    );
}

#[test]
fn guided_inverse_requires_original_system_replay() {
    let verified = run_completion(&json!({
        "operation": "guided_inverse",
        "rules": ground_rule(),
        "precedence": precedence(&[("a", 0), ("b", 0)]),
        "target": sym0("b"),
        "max_depth": 4
    }));
    assert_eq!(verified.outcome, "verified_witness");
    assert_eq!(verified.guidance_available, Some(true));
    assert_eq!(
        verified.path,
        vec![
            serde_json::from_value::<amari_discovery::RewriteTerm>(sym0("a")).unwrap(),
            serde_json::from_value::<amari_discovery::RewriteTerm>(sym0("b")).unwrap(),
        ]
    );
    assert!(verified.candidates_rejected.is_empty());
    assert_eq!(verified.candidates_rejected_count, Some(0));
    assert!(verified.inconclusive_reason.is_none());

    // A partial completion offers no guidance and never starts a
    // completed-system search.
    let no_guidance = run_completion(&json!({
        "operation": "guided_inverse",
        "rules": [rule(sym2("f", var("x"), var("y")), sym2("f", var("y"), var("x")))],
        "precedence": precedence(&[("f", 2)]),
        "target": sym2("f", sym0("a"), sym0("b")),
        "max_depth": 4
    }));
    assert_eq!(no_guidance.outcome, "inconclusive");
    assert_eq!(
        no_guidance.inconclusive_reason.as_deref(),
        Some("completion_not_complete")
    );
    assert_eq!(no_guidance.guidance_available, Some(false));
    assert!(no_guidance.path.is_empty());

    // An empty completed-system search is inconclusive, never a
    // negative claim.
    let empty = run_completion(&json!({
        "operation": "guided_inverse",
        "rules": ground_rule(),
        "precedence": precedence(&[("a", 0), ("b", 0)]),
        "target": sym0("c"),
        "max_depth": 4
    }));
    assert_eq!(empty.outcome, "inconclusive");
    assert_eq!(
        empty.inconclusive_reason.as_deref(),
        Some("no_candidates_generated")
    );
    assert!(empty.path.is_empty());
}

#[test]
fn guided_inverse_rejects_candidates_that_fail_original_replay() {
    // Completion ORIENTS `a -> b` as `b -> a` because `b` outranks
    // `a`. The completed system therefore proposes `b` as a
    // predecessor of `a`, but the ORIGINAL directed system rewrites
    // `a -> b`, so `b` cannot reach `a`. The candidate is rejected
    // evidence, never a witness.
    let output = run_completion(&json!({
        "operation": "guided_inverse",
        "rules": ground_rule(),
        "precedence": precedence(&[("b", 0), ("a", 0)]),
        "target": sym0("a"),
        "max_depth": 4
    }));
    assert_eq!(output.guidance_available, Some(true));
    assert_eq!(output.outcome, "no_verified_witness");
    assert_eq!(output.candidates_rejected_count, Some(1));
    assert_eq!(
        output.candidates_rejected,
        vec![serde_json::from_value::<amari_discovery::RewriteTerm>(sym0("b")).unwrap()]
    );
    assert!(output.path.is_empty());
}

#[test]
fn oversized_inputs_are_typed_errors_not_exhaustion() {
    // 33 rules exceed the 32-rule ceiling.
    let rules: Vec<Value> = (0..33).map(|_| rule(sym0("a"), sym0("b"))).collect();
    let error = run_error(
        ANALYSIS,
        &json!({"operation": "critical_pairs", "rules": rules}),
    );
    assert!(
        matches!(error, DiscoveryError::LimitExceeded(_)),
        "unexpected error: {error}"
    );

    // A 17-deep term exceeds the 16-depth ceiling.
    let mut deep = sym0("a");
    for _ in 0..17 {
        deep = sym1("f", deep);
    }
    let error = run_error(
        ANALYSIS,
        &json!({"operation": "critical_pairs", "rules": [rule(deep.clone(), sym0("b"))]}),
    );
    assert!(
        matches!(error, DiscoveryError::LimitExceeded(_)),
        "unexpected error: {error}"
    );

    // 17 precedence symbols exceed the 16-symbol ceiling.
    let mut symbols = precedence(&[("a", 0), ("b", 0)]);
    for index in 0..15 {
        symbols.push(json!({"name": format!("s{index}"), "arity": 0}));
    }
    let error = run_error(
        ANALYSIS,
        &json!({
            "operation": "termination",
            "rules": ground_rule(),
            "precedence": symbols
        }),
    );
    assert!(
        matches!(error, DiscoveryError::LimitExceeded(_)),
        "unexpected error: {error}"
    );

    // Arity 9 exceeds the 8-arity ceiling.
    let error = run_error(
        ANALYSIS,
        &json!({
            "operation": "termination",
            "rules": ground_rule(),
            "precedence": precedence(&[("a", 9), ("b", 0)])
        }),
    );
    assert!(
        matches!(error, DiscoveryError::LimitExceeded(_)),
        "unexpected error: {error}"
    );

    // The guided-search depth is bounded on both ends.
    for depth in [0, 9] {
        let error = run_error(
            COMPLETION,
            &json!({
                "operation": "guided_inverse",
                "rules": ground_rule(),
                "precedence": precedence(&[("a", 0), ("b", 0)]),
                "target": sym0("b"),
                "max_depth": depth
            }),
        );
        assert!(
            matches!(error, DiscoveryError::InvalidInput(_)),
            "unexpected error for depth {depth}: {error}"
        );
    }
}

#[test]
fn strict_dtos_reject_unknown_fields_and_operations() {
    // Unknown top-level field.
    let error = run_error(
        ANALYSIS,
        &json!({"operation": "critical_pairs", "rules": [], "bogus": true}),
    );
    assert!(is_typed_resource_or_input_error(&error));

    // Unknown nested rule field.
    let error = run_error(
        ANALYSIS,
        &json!({
            "operation": "critical_pairs",
            "rules": [{
                "lhs": sym0("a"),
                "rhs": sym0("b"),
                "extra": 1
            }]
        }),
    );
    assert!(is_typed_resource_or_input_error(&error));

    // Unknown nested precedence field.
    let error = run_error(
        ANALYSIS,
        &json!({
            "operation": "termination",
            "rules": ground_rule(),
            "precedence": [{"name": "a", "arity": 0, "extra": 1}]
        }),
    );
    assert!(is_typed_resource_or_input_error(&error));

    // Unknown nested target term field on the completion probe.
    let error = run_error(
        COMPLETION,
        &json!({
            "operation": "guided_inverse",
            "rules": ground_rule(),
            "precedence": precedence(&[("a", 0), ("b", 0)]),
            "target": {"kind": "symbol", "name": "b", "arguments": [], "extra": 1}
        }),
    );
    assert!(is_typed_resource_or_input_error(&error));

    // Unknown operations are typed input errors.
    let error = run_error(ANALYSIS, &json!({"operation": "frobnicate", "rules": []}));
    assert!(
        matches!(error, DiscoveryError::InvalidInput(ref message) if message.contains("operation")),
        "unexpected error: {error}"
    );
    let error = run_error(
        COMPLETION,
        &json!({
            "operation": "frobnicate",
            "rules": [],
            "precedence": precedence(&[("a", 0)])
        }),
    );
    assert!(
        matches!(error, DiscoveryError::InvalidInput(ref message) if message.contains("operation")),
        "unexpected error: {error}"
    );
}

#[test]
fn supplied_fields_are_validated_even_when_unused() {
    // `critical_pairs` consumes no precedence, but an over-ceiling
    // one is still rejected.
    let mut symbols = precedence(&[("a", 0), ("b", 0)]);
    for index in 0..15 {
        symbols.push(json!({"name": format!("s{index}"), "arity": 0}));
    }
    let error = run_error(
        ANALYSIS,
        &json!({
            "operation": "critical_pairs",
            "rules": ground_rule(),
            "precedence": symbols
        }),
    );
    assert!(
        matches!(error, DiscoveryError::LimitExceeded(_)),
        "unexpected error: {error}"
    );

    // `complete` consumes no target, but an over-ceiling one is still
    // rejected.
    let mut deep = sym0("a");
    for _ in 0..17 {
        deep = sym1("f", deep);
    }
    let error = run_error(
        COMPLETION,
        &json!({
            "operation": "complete",
            "rules": ground_rule(),
            "precedence": precedence(&[("a", 0), ("b", 0)]),
            "target": deep
        }),
    );
    assert!(
        matches!(error, DiscoveryError::LimitExceeded(_)),
        "unexpected error: {error}"
    );

    // `complete` consumes no depth, but an out-of-range one is still
    // rejected.
    let error = run_error(
        COMPLETION,
        &json!({
            "operation": "complete",
            "rules": ground_rule(),
            "precedence": precedence(&[("a", 0), ("b", 0)]),
            "max_depth": 9
        }),
    );
    assert!(
        matches!(error, DiscoveryError::InvalidInput(_)),
        "unexpected error: {error}"
    );
}

#[test]
fn completion_feature_surface_is_available_in_the_probe_build() {
    // The completion surface lives behind amari-rewrite's non-default
    // `completion` feature; the probe build must expose it.
    let engine = ProbeEngine::new().unwrap();
    assert!(
        engine.is_executable(&COMPLETION.parse().unwrap()),
        "the completion probe must be executable"
    );
    let output = run_completion(&json!({
        "operation": "complete",
        "rules": ground_rule(),
        "precedence": precedence(&[("a", 0), ("b", 0)])
    }));
    assert_eq!(output.outcome, "complete");
}

#[test]
fn direct_and_process_execution_agree() {
    let temporary = TempDir::new().unwrap();

    for (probe, input) in [
        (
            ANALYSIS,
            json!({
                "operation": "termination",
                "rules": ground_rule(),
                "precedence": precedence(&[("a", 0), ("b", 0)])
            }),
        ),
        (
            COMPLETION,
            json!({
                "operation": "complete",
                "rules": ground_rule(),
                "precedence": precedence(&[("a", 0), ("b", 0)])
            }),
        ),
    ] {
        let path = temporary.path().join("request.json");
        std::fs::write(&path, serde_json::to_vec(&input).unwrap()).unwrap();
        let cli = command_json(&["probe", "run", probe, "--input", path.to_str().unwrap()]);
        let direct = ProbeEngine::new()
            .unwrap()
            .execute(&probe.parse().unwrap(), &input)
            .unwrap();
        assert_eq!(
            cli["data"]["result"]["output"], direct.output,
            "stdout parity for {probe}"
        );
        assert_eq!(cli["data"]["isolation"], json!("process"));
        assert_eq!(cli["data"]["crash_isolation"], json!(true));
    }
}
