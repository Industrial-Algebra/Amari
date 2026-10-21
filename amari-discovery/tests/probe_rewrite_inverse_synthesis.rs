// SPDX-License-Identifier: MIT OR Apache-2.0

//! Engine-level contract tests for the Cohort 7 synthesis probes
//! (0.25 Task 39): negative-example specialization and inverse-clause
//! synthesis.
//!
//! These cover the direct-call surface, parity with the library at the
//! documented limit profile, malformed and adversarial input, budget
//! exhaustion as a typed outcome, and determinism. Semantic catalog
//! wiring lives in `rewrite_discovery_synthesis.rs`.

#![cfg(feature = "standard-probes")]

use amari_discovery::{
    DiscoveryError, ProbeEngine, ProbeEngineLimits, RewriteInverseSynthesisOutput,
    RewriteSpecializeOutput, RewriteTerm,
};
use amari_rewrite::{
    relation::RelationLimits,
    synthesis::{
        specialize_rules, synthesize_inverse, InverseSynthesisConfig, InverseSynthesisOutcome,
        SpecializationConfig, SpecializationOutcome,
    },
    trs::Term,
};
use assert_cmd::Command;
use serde_json::{json, Value};
use tempfile::TempDir;

const SPECIALIZE: &str = "amari-probe:rewrite:specialize:v1";
const SYNTHESIZE_INVERSE: &str = "amari-probe:rewrite:synthesize-inverse:v1";

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

fn example(source: Value, target: Value) -> Value {
    json!({"source": source, "target": target})
}

fn run(probe: &str, input: &Value) -> Value {
    ProbeEngine::new()
        .unwrap()
        .execute(&probe.parse().unwrap(), input)
        .unwrap()
        .output
}

fn run_specialize(input: &Value) -> RewriteSpecializeOutput {
    serde_json::from_value(run(SPECIALIZE, input)).unwrap()
}

fn run_inverse(input: &Value) -> RewriteInverseSynthesisOutput {
    serde_json::from_value(run(SYNTHESIZE_INVERSE, input)).unwrap()
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

/// The specialization profile the probe builds for default engine
/// limits: 32 examples, 64 candidates, 64 partitions, and
/// `RelationLimits::new(64, 16, MAX_CONSTRAINTS, 20_000)`.
fn probe_specialization_config() -> SpecializationConfig {
    SpecializationConfig::new(
        32,
        64,
        64,
        RelationLimits::new(64, 16, RelationLimits::MAX_CONSTRAINTS, 20_000).unwrap(),
    )
    .unwrap()
}

/// The inverse-synthesis profile the probe builds: the specialization
/// profile plus a 32-residual ceiling.
fn probe_inverse_config() -> InverseSynthesisConfig {
    InverseSynthesisConfig::new(probe_specialization_config(), 32).unwrap()
}

/// Wire shape of a checked library term.
fn term_json(term: &Term) -> Value {
    match term {
        Term::Var(variable) => json!({"kind": "variable", "name": variable.as_str()}),
        Term::Sym(symbol, arguments) => json!({
            "kind": "symbol",
            "name": symbol.as_str(),
            "arguments": arguments.iter().map(term_json).collect::<Vec<_>>(),
        }),
    }
}

fn library_example(source: &RewriteTerm, target: &RewriteTerm) -> (Term, Term) {
    // Round-trip through the probe DTO so the library call consumes
    // exactly the terms the adapter consumes.
    let source: Term = term_from_json(&serde_json::to_value(source).unwrap());
    let target: Term = term_from_json(&serde_json::to_value(target).unwrap());
    (source, target)
}

fn term_from_json(value: &Value) -> Term {
    match value["kind"].as_str().unwrap() {
        "variable" => Term::var(value["name"].as_str().unwrap()),
        "symbol" => Term::sym(
            value["name"].as_str().unwrap(),
            value["arguments"]
                .as_array()
                .unwrap()
                .iter()
                .map(term_from_json)
                .collect::<Vec<_>>(),
        ),
        other => panic!("unexpected term kind {other}"),
    }
}

/// `add(0, a) -> a` and `add(0, b) -> b`, with `add(0, c) -> c`
/// supplied as a negative so the generalization must be split.
fn add_zero_evidence() -> Value {
    json!({
        "positives": [
            example(sym2("add", sym0("0"), sym0("a")), sym0("a")),
            example(sym2("add", sym0("0"), sym0("b")), sym0("b")),
        ],
        "negatives": [example(sym2("add", sym0("0"), sym0("c")), sym0("c"))],
    })
}

/// Lossless fixture: `k(a) -> m(a)` and `k(b) -> m(b)`.
fn lossless_positives() -> Vec<Value> {
    vec![
        example(sym1("k", sym0("a")), sym1("m", sym0("a"))),
        example(sym1("k", sym0("b")), sym1("m", sym0("b"))),
    ]
}

/// Erasing fixture: `pair(a, b) -> snd(a)` and `pair(c, d) -> snd(c)`
/// erase the second argument.
fn erasing_positives() -> Vec<Value> {
    vec![
        example(sym2("pair", sym0("a"), sym0("b")), sym1("snd", sym0("a"))),
        example(sym2("pair", sym0("c"), sym0("d")), sym1("snd", sym0("c"))),
    ]
}

#[test]
fn specialization_parity_with_the_direct_library_call() {
    let input = add_zero_evidence();
    let request: amari_discovery::RewriteSpecializeRequest =
        serde_json::from_value(input.clone()).unwrap();
    let positives = request
        .positives
        .iter()
        .map(|example| library_example(&example.source, &example.target))
        .collect::<Vec<_>>();
    let negatives = request
        .negatives
        .clone()
        .unwrap_or_default()
        .iter()
        .map(|example| library_example(&example.source, &example.target))
        .collect::<Vec<_>>();

    let direct = specialize_rules(&positives, &negatives, &probe_specialization_config())
        .expect("specialization succeeds");
    let SpecializationOutcome::Refined {
        rules,
        coverage: direct_coverage,
    } = direct
    else {
        panic!("expected refined rules, got {direct:?}");
    };

    let output = run_specialize(&input);
    assert_eq!(output.outcome, "refined");
    assert_eq!(output.operation, "specialize");
    assert_eq!(output.positive_count, 2);
    assert_eq!(output.negative_count, 1);
    assert_eq!(output.rule_count, Some(rules.len() as u64));
    assert_eq!(output.rules_truncated, Some(false));
    assert!(output.inconclusive_reason.is_none());
    assert!(output.failure_reason.is_none());

    // Rule parity: the probe reports exactly the library rules, in
    // library order.
    let expected: Vec<Value> = rules
        .iter()
        .map(|rule| {
            json!({
                "lhs": term_json(rule.lhs()),
                "rhs": term_json(rule.rhs()),
            })
        })
        .collect();
    assert_eq!(
        serde_json::to_value(&output.rules).unwrap(),
        json!(expected)
    );

    // Coverage parity: indices and per-rule coverage agree.
    let expected_coverage: Vec<Value> = direct_coverage
        .rules()
        .iter()
        .map(|entry| {
            json!({
                "rule_index": entry.rule_index() as u64,
                "positives": entry.positives().iter().map(|index| *index as u64).collect::<Vec<_>>(),
                "negatives": entry.negatives().iter().map(|index| *index as u64).collect::<Vec<_>>(),
            })
        })
        .collect();
    assert_eq!(
        serde_json::to_value(&output.coverage).unwrap(),
        json!(expected_coverage)
    );

    // Every refined rule covers at least one positive and no negative.
    for entry in &output.coverage {
        assert!(!entry.positives.is_empty(), "rule covers no positive");
        assert!(entry.negatives.is_empty(), "rule covers a negative");
    }
}

#[test]
fn inverse_synthesis_parity_with_the_direct_library_call() {
    let input = json!({"positives": lossless_positives()});
    let request: amari_discovery::RewriteInverseSynthesisRequest =
        serde_json::from_value(input.clone()).unwrap();
    let positives = request
        .positives
        .iter()
        .map(|example| library_example(&example.source, &example.target))
        .collect::<Vec<_>>();

    let direct = synthesize_inverse(&positives, &[], &[], &probe_inverse_config())
        .expect("inverse synthesis succeeds");
    let InverseSynthesisOutcome::Candidate {
        rules,
        clauses,
        evidence,
    } = direct
    else {
        panic!("expected a candidate, got {direct:?}");
    };

    let output = run_inverse(&input);
    assert_eq!(output.outcome, "candidate");
    assert_eq!(output.operation, "synthesize_inverse");
    assert_eq!(output.positive_count, 2);
    assert_eq!(output.negative_count, 0);
    assert_eq!(output.residual_count, 0);
    assert_eq!(output.rule_count, Some(rules.len() as u64));
    assert_eq!(output.clause_count, Some(clauses.len() as u64));
    assert_eq!(output.clauses_truncated, Some(false));
    assert!(output.refutation.is_none());
    assert!(output.unknown_reason.is_none());
    assert!(output.failure_reason.is_none());

    let expected: Vec<Value> = clauses
        .iter()
        .map(|clause| {
            json!({
                "forward": {
                    "lhs": term_json(clause.forward().lhs()),
                    "rhs": term_json(clause.forward().rhs()),
                },
                "reverse": clause.reverse_rule().map(|rule| json!({
                    "lhs": term_json(rule.lhs()),
                    "rhs": term_json(rule.rhs()),
                })),
                "backward_rule_id": clause.backward().rule_id().to_string(),
                "erased_variables": clause.backward().erased_variables().to_vec(),
                "residual_schema": clause
                    .residual_schema()
                    .bindings()
                    .iter()
                    .map(|(key, variable)| json!({
                        "key_index": u64::from(key.index()),
                        "variable": variable.as_str(),
                    }))
                    .collect::<Vec<_>>(),
            })
        })
        .collect();
    assert_eq!(
        serde_json::to_value(&output.clauses).unwrap(),
        json!(expected)
    );

    let probe_evidence = output.evidence.expect("candidate evidence");
    assert_eq!(probe_evidence.positives as usize, evidence.positives());
    assert_eq!(probe_evidence.negatives as usize, evidence.negatives());
    assert_eq!(probe_evidence.residuals as usize, evidence.residuals());
    assert_eq!(
        probe_evidence.forward_replays as usize,
        evidence.forward_replays()
    );
    assert_eq!(
        probe_evidence.negative_rejections as usize,
        evidence.negative_rejections()
    );
    assert_eq!(
        probe_evidence.residual_roundtrips as usize,
        evidence.residual_roundtrips()
    );
    assert_eq!(
        probe_evidence.reversible_clauses as usize,
        evidence.reversible_clauses()
    );
    assert_eq!(
        probe_evidence.residual_clauses as usize,
        evidence.residual_clauses()
    );
    assert_eq!(probe_evidence.reversible_clauses, 1);
    assert_eq!(probe_evidence.residual_clauses, 0);
}

#[test]
fn erasing_clause_reports_a_residual_schema_instead_of_a_reverse_rule() {
    let output = run_inverse(&json!({
        "positives": erasing_positives(),
        "residuals": erasing_positives(),
    }));
    assert_eq!(output.outcome, "candidate");
    assert_eq!(output.clause_count, Some(1));
    let clause = &output.clauses[0];
    assert!(
        clause.reverse.is_none(),
        "an erasing clause must not invent a reverse rule"
    );
    assert_eq!(clause.erased_variables.len(), 1);
    assert_eq!(clause.residual_schema.len(), 1);
    assert_eq!(clause.residual_schema[0].key_index, 0);
    assert_eq!(
        clause.residual_schema[0].variable, clause.erased_variables[0],
        "schema keys follow the sorted erased-variable order"
    );
    let evidence = output.evidence.expect("candidate evidence");
    assert_eq!(evidence.residual_clauses, 1);
    assert_eq!(evidence.reversible_clauses, 0);
}

#[test]
fn erasing_clause_without_residual_evidence_is_unknown() {
    let output = run_inverse(&json!({"positives": erasing_positives()}));
    assert_eq!(output.outcome, "unknown");
    let unknown = output.unknown_reason.expect("unknown reason");
    assert_eq!(unknown.kind, "residual_underdetermined");
    assert_eq!(unknown.clause, Some(0));
    assert!(output.clauses.is_empty());
    assert!(output.evidence.is_none());
}

#[test]
fn conflicting_residual_evidence_is_ambiguous() {
    let positives = vec![
        example(sym2("pair", sym0("a"), sym0("b")), sym1("snd", sym0("a"))),
        example(sym2("pair", sym0("a"), sym0("c")), sym1("snd", sym0("a"))),
    ];
    let output = run_inverse(&json!({
        "positives": positives.clone(),
        "residuals": positives,
    }));
    assert_eq!(output.outcome, "unknown");
    let unknown = output.unknown_reason.expect("unknown reason");
    assert_eq!(unknown.kind, "ambiguous");
    assert_eq!(unknown.clause, Some(0));
}

#[test]
fn underdetermined_examples_are_unknown_not_guessed() {
    let output = run_inverse(&json!({
        "positives": [example(var("X"), sym1("f", var("Y")))],
    }));
    assert_eq!(output.outcome, "unknown");
    let unknown = output.unknown_reason.expect("unknown reason");
    assert_eq!(unknown.kind, "underdetermined");
    assert_eq!(unknown.detail.as_deref(), Some("unrepresentable"));
    assert!(unknown.clause.is_none());
}

#[test]
fn refuting_residual_evidence_is_reported_as_a_typed_witness() {
    // `h(a) -> snd(a)` is not an instance of the inferred left-hand
    // side, so the claimed forward step disproves the schema.
    let output = run_inverse(&json!({
        "positives": erasing_positives(),
        "residuals": [example(sym1("h", sym0("a")), sym1("snd", sym0("a")))],
    }));
    assert_eq!(output.outcome, "refuted");
    let refutation = output.refutation.expect("refutation witness");
    assert_eq!(refutation.kind, "residual_round_trip");
    assert_eq!(refutation.residual, Some(0));
    assert_eq!(refutation.clause, Some(0));
    assert!(refutation.positive.is_none());
    assert!(refutation.negative.is_none());
}

#[test]
fn inseparable_examples_are_inconclusive() {
    // The positive is itself the rule `f(X) -> X`, which necessarily
    // covers `f(a) -> a`; no finer partition exists, so the bounded
    // search reports inconclusiveness instead of a covering rule.
    let output = run_specialize(&json!({
        "positives": [example(sym1("f", var("X")), var("X"))],
        "negatives": [example(sym1("f", sym0("a")), sym0("a"))],
    }));
    assert_eq!(output.outcome, "inconclusive");
    assert_eq!(output.inconclusive_reason.as_deref(), Some("inseparable"));
    assert_eq!(output.rule_count, None);
    assert!(output.rules.is_empty());
    assert!(output.coverage.is_empty());
    assert!(output.failure_reason.is_none());
}

#[test]
fn a_pair_supplied_as_both_positive_and_negative_is_a_typed_domain_failure() {
    let pair = example(sym2("add", sym0("0"), sym0("a")), sym0("a"));
    let output = run_specialize(&json!({
        "positives": [pair.clone()],
        "negatives": [pair],
    }));
    assert_eq!(output.outcome, "error");
    assert!(
        output
            .failure_reason
            .as_deref()
            .is_some_and(|reason| !reason.is_empty()),
        "the library's refusal must be a typed outcome: {output:?}"
    );
}

/// PR #338 round 1 P2b: the request DTOs validate on deserialization
/// (Task 36 `SpecializationConfig` doctrine), so a well-formed but
/// invalid request never reaches an adapter — and the adapter still
/// revalidates, so a request built through `new` is checked too.
#[test]
fn invalid_requests_are_rejected_at_deserialization() {
    let error = serde_json::from_value::<amari_discovery::RewriteSpecializeRequest>(
        json!({"positives": [], "negatives": null}),
    )
    .expect_err("an empty positive list must not deserialize");
    assert!(
        error.to_string().contains("at least one positive example"),
        "unexpected error: {error}"
    );

    let error = serde_json::from_value::<amari_discovery::RewriteInverseSynthesisRequest>(
        json!({"positives": [], "negatives": null, "residuals": null}),
    )
    .expect_err("an empty positive list must not deserialize");
    assert!(
        error.to_string().contains("at least one positive example"),
        "unexpected error: {error}"
    );

    // Deserialization also rejects unknown fields, matching the wire
    // contract's `deny_unknown_fields`.
    let error = serde_json::from_value::<amari_discovery::RewriteSpecializeRequest>(
        json!({"positives": [], "bogus": 1}),
    )
    .expect_err("unknown fields must not deserialize");
    assert!(
        error.to_string().contains("bogus"),
        "unexpected error: {error}"
    );

    // A well-formed request still round-trips through both entry points.
    let request: amari_discovery::RewriteSpecializeRequest =
        serde_json::from_value(add_zero_evidence()).unwrap();
    assert_eq!(request.positives.len(), 2);
    assert!(request.validate().is_ok());
    let rebuilt = amari_discovery::RewriteSpecializeRequest::new(
        request.positives.clone(),
        request.negatives.clone(),
    )
    .unwrap();
    assert_eq!(rebuilt, request);

    let inverse: amari_discovery::RewriteInverseSynthesisRequest =
        serde_json::from_value(json!({"positives": erasing_positives()})).unwrap();
    assert!(inverse.residuals.is_none());
    let rebuilt = amari_discovery::RewriteInverseSynthesisRequest::new(
        inverse.positives.clone(),
        None,
        Some(inverse.positives.clone()),
    )
    .unwrap();
    assert!(rebuilt.residuals.is_some());
    assert!(
        amari_discovery::RewriteInverseSynthesisRequest::new(Vec::new(), None, None).is_err(),
        "the checked constructor rejects an empty positive list"
    );

    // The probe surface rejects the same request as a typed input
    // error: deserialization is the first gate, not the only one.
    let error = run_error(SPECIALIZE, &json!({"positives": []}));
    assert!(
        matches!(error, DiscoveryError::InvalidInput(ref message) if message.contains("positive")),
        "unexpected error: {error}"
    );
}

/// A term nested `depth` levels deep.
fn nested_term(depth: usize) -> Value {
    let mut term = sym0("a");
    for index in 0..depth {
        term = sym1(&format!("f{index}"), term);
    }
    term
}

/// PR #338 round 2 P2a: every bound the wire contract declares is
/// enforced by DTO validation, so direct deserialization rejects each
/// oversized shape — and the exact-ceiling twin of each still parses.
#[test]
fn every_declared_bound_is_enforced_at_deserialization() {
    let one = |source: Value| json!([example(source, sym0("b"))]);
    let many = |count: usize| {
        let entries: Vec<Value> = (0..count)
            .map(|_| example(sym1("k", sym0("a")), sym1("m", sym0("a"))))
            .collect();
        json!(entries)
    };

    // Example count: 32 parses, 33 fails, for both request types.
    let _: amari_discovery::RewriteSpecializeRequest =
        serde_json::from_value(json!({"positives": many(32)})).expect("32 positives parse");
    let error = serde_json::from_value::<amari_discovery::RewriteSpecializeRequest>(json!({
        "positives": many(33)
    }))
    .expect_err("33 positives must not deserialize");
    assert!(
        error.to_string().contains("positive examples"),
        "unexpected error: {error}"
    );
    let error = serde_json::from_value::<amari_discovery::RewriteInverseSynthesisRequest>(json!({
        "positives": many(33)
    }))
    .expect_err("33 positives must not deserialize");
    assert!(
        error.to_string().contains("positive examples"),
        "unexpected error: {error}"
    );
    let _: amari_discovery::RewriteInverseSynthesisRequest =
        serde_json::from_value(json!({"positives": many(32)})).expect("32 positives parse");

    // Residual count: 32 parses, 33 fails.
    let residuals = many(33);
    let error = serde_json::from_value::<amari_discovery::RewriteInverseSynthesisRequest>(json!({
        "positives": lossless_positives(),
        "residuals": residuals
    }))
    .expect_err("33 residuals must not deserialize");
    assert!(
        error.to_string().contains("residual examples"),
        "unexpected error: {error}"
    );

    // Node ceiling: 64 nodes parse, 65 fail, for both request types.
    let _: amari_discovery::RewriteSpecializeRequest =
        serde_json::from_value(json!({"positives": one(wide_term(63))}))
            .expect("a 64-node term parses");
    for probe in [SPECIALIZE, SYNTHESIZE_INVERSE] {
        let name = if probe == SPECIALIZE {
            "RewriteSpecializeRequest"
        } else {
            "RewriteInverseSynthesisRequest"
        };
        let oversized = json!({"positives": one(wide_term(64))});
        let rejected = match probe {
            SPECIALIZE => serde_json::from_value::<amari_discovery::RewriteSpecializeRequest>(
                oversized.clone(),
            )
            .map(|_| ())
            .map_err(|error| error.to_string()),
            _ => serde_json::from_value::<amari_discovery::RewriteInverseSynthesisRequest>(
                oversized.clone(),
            )
            .map(|_| ())
            .map_err(|error| error.to_string()),
        };
        assert!(
            rejected
                .expect_err(&format!("{name} must reject a 65-node term"))
                .contains("nodes"),
            "{name}: the node ceiling must be the reported bound"
        );
    }

    // Name bytes: a 256-byte name parses, a 257-byte name fails.
    let long = "x".repeat(257);
    let error = serde_json::from_value::<amari_discovery::RewriteSpecializeRequest>(json!({
        "positives": one(json!({"kind": "symbol", "name": long, "arguments": []}))
    }))
    .expect_err("a 257-byte name must not deserialize");
    assert!(
        error.to_string().contains("name"),
        "unexpected error: {error}"
    );
    let _: amari_discovery::RewriteSpecializeRequest = serde_json::from_value(json!({
        "positives": one(json!({"kind": "symbol", "name": "x".repeat(256), "arguments": []}))
    }))
    .expect("a 256-byte name parses");

    // Depth: 16 levels parse, 17 fail. Depth is enforced by the same
    // DTO validation as the other ceilings (documented in
    // `RewriteSpecializeRequest::validate`), and the adapter re-asserts
    // the identical bound.
    let _: amari_discovery::RewriteSpecializeRequest =
        serde_json::from_value(json!({"positives": one(nested_term(15))}))
            .expect("a 16-deep term parses");
    let error = serde_json::from_value::<amari_discovery::RewriteSpecializeRequest>(json!({
        "positives": one(nested_term(17))
    }))
    .expect_err("a 17-deep term must not deserialize");
    assert!(
        error.to_string().contains("depth"),
        "unexpected error: {error}"
    );
}

/// PR #338 round 2 P2b: the inconclusive, refuted, unknown, and error
/// arms assemble wire evidence too, so their tags are metered. Each
/// delta below is `SYNTHESIS_REASON_UNITS` (2) plus one unit per byte
/// of the emitted kind/detail/operation text plus one unit per
/// populated index slot (measured values, pinned).
#[test]
fn non_success_output_assembly_is_charged() {
    let variable = |name: &str| json!({"kind": "variable", "name": name});
    let cases: [(&str, &str, Value, u64); 6] = [
        (
            "inconclusive",
            SPECIALIZE,
            json!({
                "positives": [example(sym1("f", variable("X")), variable("X"))],
                "negatives": [example(sym1("f", sym0("a")), sym0("a"))],
            }),
            // 2 + "inseparable" (11) = 13.
            13,
        ),
        (
            "error",
            SPECIALIZE,
            json!({
                "positives": [example(sym2("add", sym0("0"), sym0("a")), sym0("a"))],
                "negatives": [example(sym2("add", sym0("0"), sym0("a")), sym0("a"))],
            }),
            // 2 + "specialization" (14) + failure text (72) = 88. The
            // failure text embeds the library message and its limit, so
            // this total is fixture-specific.
            88,
        ),
        (
            "refuted",
            SYNTHESIZE_INVERSE,
            json!({
                "positives": erasing_positives(),
                "residuals": [example(sym1("h", sym0("a")), sym1("snd", sym0("a")))],
            }),
            // 2 + "residual_round_trip" (19) + 2 index slots = 23.
            23,
        ),
        (
            "unknown_residual",
            SYNTHESIZE_INVERSE,
            json!({"positives": erasing_positives()}),
            // 2 + "residual_underdetermined" (24) + 1 clause slot = 27.
            27,
        ),
        (
            "unknown_underdetermined",
            SYNTHESIZE_INVERSE,
            json!({"positives": [example(variable("X"), sym1("f", variable("Y")))]}),
            // 2 + "underdetermined" (15) + 1 detail slot + 15 bytes = 33.
            33,
        ),
        (
            "unknown_ambiguous",
            SYNTHESIZE_INVERSE,
            json!({
                "positives": [
                    example(sym2("pair", sym0("a"), sym0("b")), sym1("snd", sym0("a"))),
                    example(sym2("pair", sym0("a"), sym0("c")), sym1("snd", sym0("a"))),
                ],
                "residuals": [
                    example(sym2("pair", sym0("a"), sym0("b")), sym1("snd", sym0("a"))),
                    example(sym2("pair", sym0("a"), sym0("c")), sym1("snd", sym0("a"))),
                ],
            }),
            // 2 + "ambiguous" (9) + 1 clause slot = 12.
            12,
        ),
    ];

    for (label, probe, input, assembly) in cases {
        let execution = ProbeEngine::new()
            .unwrap()
            .execute(&probe.parse().unwrap(), &input)
            .unwrap();
        assert_ne!(
            execution.output["outcome"],
            json!("refined"),
            "{label}: fixture must not succeed"
        );
        assert_eq!(
            execution.resources.operations,
            json_node_count(&input) + assembly,
            "{label}: reported operations must cover validation plus the reason tag"
        );
    }
}

#[test]
fn malformed_examples_are_typed_input_errors() {
    for probe in [SPECIALIZE, SYNTHESIZE_INVERSE] {
        // Missing example target.
        let error = run_error(probe, &json!({"positives": [{"source": sym0("a")}]}));
        assert!(
            is_typed_resource_or_input_error(&error),
            "{probe}: unexpected error {error}"
        );

        // Example source is not a term.
        let error = run_error(probe, &json!({"positives": [{"source": 7, "target": 8}]}));
        assert!(
            is_typed_resource_or_input_error(&error),
            "{probe}: unexpected error {error}"
        );

        // Positives are required.
        let error = run_error(probe, &json!({"positives": []}));
        assert!(
            matches!(error, DiscoveryError::InvalidInput(ref message) if message.contains("positive")),
            "{probe}: unexpected error {error}"
        );

        // An unknown term kind is rejected.
        let error = run_error(
            probe,
            &json!({"positives": [example(json!({"kind": "atom", "name": "a"}), sym0("b"))]}),
        );
        assert!(
            is_typed_resource_or_input_error(&error),
            "{probe}: unexpected error {error}"
        );
    }
}

#[test]
fn strict_dtos_reject_unknown_fields() {
    for probe in [SPECIALIZE, SYNTHESIZE_INVERSE] {
        // Unknown top-level field.
        let error = run_error(probe, &json!({"positives": [], "bogus": true}));
        assert!(
            is_typed_resource_or_input_error(&error),
            "{probe}: unexpected error {error}"
        );

        // Unknown nested example field.
        let error = run_error(
            probe,
            &json!({
                "positives": [{
                    "source": sym0("a"),
                    "target": sym0("b"),
                    "extra": 1
                }]
            }),
        );
        assert!(
            is_typed_resource_or_input_error(&error),
            "{probe}: unexpected error {error}"
        );

        // Unknown nested term field.
        let error = run_error(
            probe,
            &json!({
                "positives": [example(
                    json!({"kind": "symbol", "name": "a", "arguments": [], "extra": 1}),
                    sym0("b")
                )]
            }),
        );
        assert!(
            is_typed_resource_or_input_error(&error),
            "{probe}: unexpected error {error}"
        );
    }

    // The inverse probe rejects an unknown nested residuals entry too.
    let error = run_error(
        SYNTHESIZE_INVERSE,
        &json!({
            "positives": lossless_positives(),
            "residuals": [{"source": sym0("a"), "target": sym0("b"), "extra": 1}]
        }),
    );
    assert!(is_typed_resource_or_input_error(&error));
}

#[test]
fn every_tightened_limit_is_enforced() {
    // Example-count ceiling: 33 positives exceed the 32 ceiling.
    let positives: Vec<Value> = (0..33)
        .map(|_| example(sym1("k", sym0("a")), sym1("m", sym0("a"))))
        .collect();
    let error = run_error(SPECIALIZE, &json!({"positives": positives}));
    assert!(
        matches!(error, DiscoveryError::LimitExceeded(ref message) if message.contains("positive examples")),
        "unexpected error: {error}"
    );
    let error = run_error(SYNTHESIZE_INVERSE, &json!({"positives": positives}));
    assert!(
        matches!(error, DiscoveryError::LimitExceeded(ref message) if message.contains("positive examples")),
        "unexpected error: {error}"
    );

    // Residual-count ceiling: 33 residuals exceed the 32 ceiling.
    let residuals: Vec<Value> = (0..33)
        .map(|_| example(sym2("pair", sym0("a"), sym0("b")), sym1("snd", sym0("a"))))
        .collect();
    let error = run_error(
        SYNTHESIZE_INVERSE,
        &json!({"positives": erasing_positives(), "residuals": residuals}),
    );
    assert!(
        matches!(error, DiscoveryError::LimitExceeded(ref message) if message.contains("residual examples")),
        "unexpected error: {error}"
    );

    // Depth ceiling: a 17-deep term exceeds the 16-depth ceiling.
    let mut deep = sym0("a");
    for _ in 0..17 {
        deep = sym1("f", deep);
    }
    for probe in [SPECIALIZE, SYNTHESIZE_INVERSE] {
        let error = run_error(
            probe,
            &json!({"positives": [example(deep.clone(), sym0("b"))]}),
        );
        assert!(
            matches!(error, DiscoveryError::LimitExceeded(ref message) if message.contains("depth")),
            "{probe}: unexpected error {error}"
        );
    }
}

/// A shallow WIDE term with exactly `arguments + 1` nodes at depth 2,
/// so node-limit coverage never depends on the depth ceiling.
fn wide_term(arguments: usize) -> Value {
    let children: Vec<Value> = (0..arguments).map(|_| sym0("a")).collect();
    json!({"kind": "symbol", "name": "g", "arguments": children})
}

#[test]
fn the_node_limit_is_enforced_independently_of_depth() {
    // 63 arguments -> 64 nodes at depth 2: exactly the ceiling.
    let at_limit = wide_term(63);
    // 64 arguments -> 65 nodes at depth 2: one past the ceiling.
    let over_limit = wide_term(64);
    for probe in [SPECIALIZE, SYNTHESIZE_INVERSE] {
        let output = run(
            probe,
            &json!({"positives": [example(at_limit.clone(), sym0("b"))]}),
        );
        assert!(
            output["outcome"].is_string(),
            "{probe}: a 64-node, depth-2 term must be accepted"
        );

        let error = run_error(
            probe,
            &json!({"positives": [example(over_limit.clone(), sym0("b"))]}),
        );
        assert!(
            matches!(error, DiscoveryError::LimitExceeded(ref message) if message.contains("nodes")),
            "{probe}: the 65-node, depth-2 term must fail the NODE ceiling, got {error}"
        );
    }
}

#[test]
fn a_tightened_iterations_budget_is_a_typed_domain_outcome() {
    // The library's example ceiling follows the caller's iterations
    // budget, so the run reports a typed outcome with the meter
    // instead of a silently widened allowance.
    let input = json!({
        "positives": [
            example(sym1("k", sym0("a")), sym1("m", sym0("a"))),
            example(sym1("k", sym0("b")), sym1("m", sym0("b"))),
        ]
    });
    let limits = ProbeEngineLimits {
        max_iterations: 1,
        ..ProbeEngineLimits::default()
    };
    let engine = ProbeEngine::with_limits(limits).unwrap();
    for probe in [SPECIALIZE, SYNTHESIZE_INVERSE] {
        let execution = engine
            .execute(&probe.parse().unwrap(), &input)
            .expect("a library limit violation is a typed outcome");
        let outcome = execution.output["outcome"].as_str().unwrap().to_owned();
        assert_eq!(outcome, "error", "{probe} must report a typed failure");
        let reason = execution.output["failure_reason"]
            .as_str()
            .expect("failure_reason describes the domain failure")
            .to_owned();
        assert!(!reason.is_empty(), "{probe} needs a failure reason");
        assert!(
            execution.resources.operations > 0,
            "{probe}: observations must accompany the outcome"
        );
    }
}

/// Node count of a request exactly as the engine's bounds guard counts
/// it (every JSON value; keys are not values).
fn json_node_count(value: &Value) -> u64 {
    let mut nodes = 0_u64;
    let mut stack: Vec<&Value> = vec![value];
    while let Some(current) = stack.pop() {
        nodes += 1;
        match current {
            Value::Array(items) => stack.extend(items.iter()),
            Value::Object(map) => stack.extend(map.values()),
            _ => {}
        }
    }
    nodes
}

/// PR #338 round 1 P2a: output assembly is adapter work and is metered
/// per node and per entry, so the reported operations exceed the
/// request-validation charge instead of equalling it.
#[test]
fn output_assembly_is_charged_per_node_and_entry() {
    let input = json!({
        "positives": [
            example(sym1("k", sym0("a")), sym1("m", sym0("a"))),
            example(sym1("k", sym0("b")), sym1("m", sym0("b"))),
        ]
    });
    let nodes = json_node_count(&input);
    // Measured boundaries for this fixture (node count 36):
    // - specialize emits one rule `k(_I0) -> m(_I0)` (1 entry + 4 nodes
    //   = 5) and one coverage entry (1 + 2 positives = 3) -> 8 units;
    // - inverse emits one clause: forward rule 5, checked reverse rule
    //   5, 64-hex backward identity 65, no erased names, no residual
    //   schema, and 8 evidence counters -> 83 units.
    for (probe, assembly) in [(SPECIALIZE, 8_u64), (SYNTHESIZE_INVERSE, 83_u64)] {
        let execution = ProbeEngine::new()
            .unwrap()
            .execute(&probe.parse().unwrap(), &input)
            .unwrap();
        assert_eq!(
            execution.resources.operations,
            nodes + assembly,
            "{probe}: reported operations must cover validation plus output assembly"
        );
    }
}

/// Run one probe under an operation ceiling, keeping the typed error.
fn run_at(probe: &str, input: &Value, budget: u64) -> Result<Value, DiscoveryError> {
    let limits = ProbeEngineLimits {
        max_operations: budget,
        ..ProbeEngineLimits::default()
    };
    ProbeEngine::with_limits(limits)
        .unwrap()
        .execute(&probe.parse().unwrap(), input)
        .map(|execution| {
            serde_json::json!({
                "outcome": execution.output["outcome"],
                "reason": execution.output["failure_reason"],
                "operations": execution.resources.operations,
            })
        })
}

#[test]
fn an_exhausted_operations_budget_is_a_typed_outcome_with_observations() {
    // One pair supplied as both positive and negative is a library
    // error for either probe, so the ladder below is deterministic:
    // entry refusal, then tag refusal, then a typed outcome.
    let pair = example(sym2("add", sym0("0"), sym0("a")), sym0("a"));
    let input = json!({"positives": [pair.clone()], "negatives": [pair]});
    let nodes = json_node_count(&input);
    // Measured tag charges for this fixture (node count 37):
    // - specialize: 2 + "specialization" (14) + 72 bytes of failure
    //   text = 88, so the typed outcome appears first at 125;
    // - inverse: 2 + "inverse synthesis" (17) + 75 bytes of failure
    //   text = 94, so it appears first at 131.
    let expected = [(SPECIALIZE, 88_u64), (SYNTHESIZE_INVERSE, 94_u64)];
    for (probe, tag) in expected {
        // Below the request-validation charge the entry meter refuses the
        // request outright: a probe limit error, not an outcome.
        let error = run_at(probe, &input, nodes - 1).unwrap_err();
        assert!(
            matches!(
                error,
                DiscoveryError::LimitExceeded(ref message) if message.contains("request validation")
            ),
            "{probe}: an exhausted entry charge is a typed limit error, got {error}"
        );

        // Between the validation charge and the tag charge the failure
        // cannot be assembled: also a limit error, named for the output.
        let error = run_at(probe, &input, nodes).unwrap_err();
        assert!(
            matches!(
                error,
                DiscoveryError::LimitExceeded(ref message) if message.contains("output assembly")
            ),
            "{probe}: an exhausted tag charge is a typed limit error, got {error}"
        );

        // Once the budget covers validation plus the tag, the library
        // failure is reported as a typed outcome that keeps the meter.
        let execution = run_at(probe, &input, nodes + tag)
            .unwrap_or_else(|error| panic!("{probe}: typed outcome expected, got {error}"));
        assert_eq!(execution["outcome"], json!("error"), "{probe}");
        assert!(
            execution["reason"]
                .as_str()
                .is_some_and(|reason| !reason.is_empty()),
            "{probe}: failure_reason must describe the domain failure"
        );
        assert_eq!(
            execution["operations"],
            json!(nodes + tag),
            "{probe}: reported operations cover validation plus the tag"
        );

        // One unit below that boundary is the tag-charge refusal.
        let error = run_at(probe, &input, nodes + tag - 1).unwrap_err();
        assert!(
            matches!(
                error,
                DiscoveryError::LimitExceeded(ref message) if message.contains("output assembly")
            ),
            "{probe}: the budget below the boundary must fail the tag charge, got {error}"
        );
    }
}

#[test]
fn identical_inputs_produce_identical_bytes_in_any_order() {
    let positives = lossless_positives();
    let input = json!({"positives": positives.clone()});
    let baseline = serde_json::to_vec(&run_inverse(&input)).unwrap();
    for _ in 0..3 {
        assert_eq!(serde_json::to_vec(&run_inverse(&input)).unwrap(), baseline);
    }

    // Permuting the input lists leaves the outcome byte-identical:
    // both input lists are canonicalized by the library.
    let mut reversed = positives;
    reversed.reverse();
    let permuted = json!({"positives": reversed});
    assert_eq!(
        serde_json::to_vec(&run_inverse(&permuted)).unwrap(),
        baseline
    );

    let specialize_input = add_zero_evidence();
    let specialize_baseline = serde_json::to_vec(&run_specialize(&specialize_input)).unwrap();
    let mut reordered = specialize_input.clone();
    let positives = reordered["positives"].as_array_mut().unwrap();
    positives.reverse();
    assert_eq!(
        serde_json::to_vec(&run_specialize(&reordered)).unwrap(),
        specialize_baseline
    );
}

#[test]
fn direct_and_process_execution_agree() {
    let temporary = TempDir::new().unwrap();

    for (probe, input) in [
        (SPECIALIZE, add_zero_evidence()),
        (
            SYNTHESIZE_INVERSE,
            json!({
                "positives": erasing_positives(),
                "residuals": erasing_positives(),
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

#[test]
fn deep_json_input_returns_a_typed_error() {
    // The engine's JSON bounds guard rejects anything deeper than 256
    // levels before recursive deserialization runs.
    let mut deep = sym0("a");
    for _ in 0..300 {
        deep = sym1("f", deep);
    }
    for probe in [SPECIALIZE, SYNTHESIZE_INVERSE] {
        let input = json!({"positives": [example(deep.clone(), sym0("b"))]});
        let error = run_error(probe, &input);
        assert!(
            is_typed_resource_or_input_error(&error),
            "{probe} must reject deep input with a typed error: {error:?}"
        );
        // Value's recursive Drop would exhaust the test stack on the
        // depth-1600 fixture; the probe already rejected it.
        std::mem::forget(input);
    }
    std::mem::forget(deep);
}
