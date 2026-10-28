// SPDX-License-Identifier: MIT OR Apache-2.0

//! Pairwise inverse-ranking training-data derivation contracts
//! (0.25 Cohort 8, Task 41).
//!
//! The dataset is a pure function of `(trace set, config, schema digest)`:
//! examples are emitted only from replay-validated traces, the chosen
//! transition is always re-derived from the symbolic engine (the only
//! transition generator), negatives are the valid-but-unchosen candidates
//! of the same expansion round, and every example is ordered by a content
//! key so no permutation of the input can change the output.

#![cfg(feature = "neural")]

use amari_rewrite::inverse::{
    BackwardExplorer, BackwardProvenance, BackwardSearchOutcome, BidirectionalDerivation,
    ForwardStep, GuidanceMode, InverseSearchConfig, ReplayCertificate, ReplayDerivation,
    ReplayQuery, ResourceObservation, SearchMode, SymbolicPredecessor,
};
use amari_rewrite::neural::{
    Dataset, DatasetBuilder, DatasetConfig, LabelAuthority, NeuralError,
};
use amari_rewrite::relation::{
    ConstraintSet, RelationLimits, RelationResources, RuleId, Sha256Digest,
};
use amari_rewrite::trs::{Rule, Substitution, Term, TermSystem};
use amari_rewrite::Path;

// ── fixtures ────────────────────────────────────────────────────────

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

fn sys(rules: Vec<Rule>) -> TermSystem {
    TermSystem::new(rules)
}

fn canonical(text: &str) -> Sha256Digest {
    Sha256Digest::canonical_term("amari.relation.term/v1", &parse(text))
}

fn builder() -> DatasetBuilder {
    DatasetBuilder::new(DatasetConfig::default()).expect("default dataset config")
}

/// The add/h system: from `a`, both `add(a, zero)` and `h(a)` are valid
/// predecessors, so a trace through it always carries unchosen negatives.
fn add_h_system() -> TermSystem {
    sys(vec![
        Rule::new(parse("add(X, zero)"), parse("X")).expect("checked"),
        Rule::new(parse("h(X)"), parse("X")).expect("checked"),
    ])
}

/// A two-step backward witness: `a -> add(a,zero) -> h(add(a,zero))`.
fn backward_witness() -> (TermSystem, ReplayCertificate) {
    let system = add_h_system();
    let target = parse("a");
    let goal = parse("h(add(a, zero))");
    let outcome = BackwardExplorer::new(&system, InverseSearchConfig::default())
        .search(&target, &goal, SearchMode::BreadthFirst)
        .expect("search succeeds");
    let BackwardSearchOutcome::Witness(derivation) = outcome else {
        panic!("expected a witness, got {outcome:?}");
    };
    let certificate = ReplayCertificate::issue(
        &system,
        ReplayQuery::Backward { target, goal },
        ReplayDerivation::Backward(derivation),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        ResourceObservation {
            states: 4,
            transitions: 4,
            retained_bytes: 512,
            operations: 8,
        },
    );
    (system, certificate)
}

/// A hand-built bidirectional witness whose backward half is non-trivial:
/// forward `g(g(f(a))) -> g(f(a))`, backward `f(a) -> g(f(a))` (via the
/// `g` rule at the root). The backward state `f(a)` has three distinct
/// candidates, so it yields two unchosen negatives.
fn bidirectional_witness() -> (TermSystem, ReplayCertificate) {
    let f = Rule::new(parse("f(X)"), parse("X")).expect("checked");
    let g = Rule::new(parse("g(X)"), parse("X")).expect("checked");
    let system = sys(vec![f, g]);
    let f_id = RuleId::from_rule(&system.rules()[0]);
    let g_id = RuleId::from_rule(&system.rules()[1]);

    let source = parse("g(g(f(a)))");
    let goal = parse("f(a)");
    let meeting = parse("g(f(a))");

    let forward_step = ForwardStep {
        rule_id: g_id,
        position: Path::root(),
        source_hash: canonical("g(g(f(a)))"),
        result_hash: canonical("g(f(a))"),
    };
    let backward_step = SymbolicPredecessor {
        term: meeting.clone(),
        existentials: Vec::new(),
        constraints: ConstraintSet::new(),
        substitution: Substitution::new(),
        provenance: BackwardProvenance {
            rule_id: g_id,
            position: Path::root(),
            scope: 1,
            target_hash: canonical("f(a)"),
            predecessor_hash: canonical("g(f(a))"),
        },
        resources: RelationResources::new(&RelationLimits::default()),
    };

    let derivation = BidirectionalDerivation {
        forward_steps: vec![forward_step],
        backward_steps: vec![backward_step],
        meeting_forward: meeting.clone(),
        meeting_backward: meeting,
        unifier: Substitution::new(),
    };
    let certificate = ReplayCertificate::issue(
        &system,
        ReplayQuery::Bidirectional { source, goal },
        ReplayDerivation::Bidirectional(derivation),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        ResourceObservation {
            states: 3,
            transitions: 2,
            retained_bytes: 256,
            operations: 4,
        },
    );
    let _ = f_id;
    (system, certificate)
}

// ── RED contracts ───────────────────────────────────────────────────

#[test]
fn backward_trace_yields_proof_positive_pairs_with_unchosen_negatives() {
    let (system, certificate) = backward_witness();
    let dataset = builder().build(&system, &[certificate]).expect("build");

    assert!(!dataset.is_empty(), "a two-step trace must yield examples");
    for example in dataset.examples() {
        assert_eq!(example.authority(), LabelAuthority::ProofPositive);
        assert_ne!(example.chosen_digest(), example.unchosen_digest());
    }
    // The root expansion `a -> {add(a,zero), h(a)}` chose `add(a, zero)`,
    // leaving `h(a)` unchosen.
    let root = dataset
        .examples()
        .iter()
        .find(|example| example.target_distance() == 1)
        .expect("a root-round example");
    assert_eq!(root.chosen_digest(), canonical("add(a, zero)"));
    assert_eq!(root.unchosen_digest(), canonical("h(a)"));
}

#[test]
fn bidirectional_trace_yields_examples_from_its_backward_half() {
    let (system, certificate) = bidirectional_witness();
    assert!(certificate.verify(&system).is_ok(), "fixture must verify");

    let dataset = builder().build(&system, &[certificate]).expect("build");
    assert_eq!(dataset.len(), 2, "two unchosen negatives at the root");

    let mut unchosen: Vec<Sha256Digest> = dataset
        .examples()
        .iter()
        .map(|example| example.unchosen_digest())
        .collect();
    unchosen.sort();
    let mut expected = vec![canonical("f(f(a))"), canonical("f(g(a))")];
    expected.sort();
    assert_eq!(unchosen, expected);
    for example in dataset.examples() {
        assert_eq!(example.authority(), LabelAuthority::ProofPositive);
        assert_eq!(example.chosen_digest(), canonical("g(f(a))"));
    }
}

#[test]
fn target_distance_records_the_round_depth() {
    let (system, certificate) = backward_witness();
    let dataset = builder().build(&system, &[certificate]).expect("build");
    let distances: Vec<u64> = dataset
        .examples()
        .iter()
        .map(|example| example.target_distance())
        .collect();
    assert!(distances.contains(&1), "root round is depth 1, got {distances:?}");
    assert!(distances.contains(&2), "second round is depth 2, got {distances:?}");
}

#[test]
fn duplicate_candidates_collapse_by_digest() {
    // Two alpha-equivalent rules produce the same predecessor twice; the
    // duplicate must collapse so the unchosen set has no repeats.
    let system = sys(vec![
        Rule::new(parse("g(X)"), parse("h(X)")).expect("checked"),
        Rule::new(parse("g(Y)"), parse("h(Y)")).expect("checked"),
        Rule::new(parse("f(Z)"), parse("h(Z)")).expect("checked"),
    ]);
    let target = parse("h(a)");
    let goal = parse("g(a)");
    let outcome = BackwardExplorer::new(&system, InverseSearchConfig::default())
        .search(&target, &goal, SearchMode::BreadthFirst)
        .expect("search succeeds");
    let BackwardSearchOutcome::Witness(derivation) = outcome else {
        panic!("expected witness, got {outcome:?}");
    };
    let certificate = ReplayCertificate::issue(
        &system,
        ReplayQuery::Backward {
            target: target.clone(),
            goal: goal.clone(),
        },
        ReplayDerivation::Backward(derivation),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        ResourceObservation {
            states: 3,
            transitions: 3,
            retained_bytes: 256,
            operations: 4,
        },
    );

    let dataset = builder().build(&system, &[certificate]).expect("build");
    assert_eq!(dataset.len(), 1, "the duplicate `g(a)` must collapse");
    assert_eq!(
        dataset.examples()[0].unchosen_digest(),
        canonical("f(a)"),
        "the lone surviving unchosen candidate is `f(a)`"
    );
}

#[test]
fn explicit_seed_sampling_is_deterministic_and_recorded() {
    let (system, certificate) = backward_witness();
    // A one-example ceiling over a two-example trace forces sub-sampling.
    let config = DatasetConfig::new(1, 16, 64, false, Some(7), RelationLimits::default())
        .expect("config");
    let first = DatasetBuilder::new(config).build(&system, &[certificate]).expect("build");
    let second = DatasetBuilder::new(config).build(&system, &[certificate]).expect("build");
    assert_eq!(first, second, "sub-sampling must be deterministic");
    assert_eq!(first.len(), 1);
    assert_eq!(first.manifest().seed(), Some(7), "the seed is recorded");
}

#[test]
fn empty_trace_is_rejected() {
    let system = add_h_system();
    let target = parse("a");
    let goal = parse("a");
    let outcome = BackwardExplorer::new(&system, InverseSearchConfig::default())
        .search(&target, &goal, SearchMode::BreadthFirst)
        .expect("search succeeds");
    let BackwardSearchOutcome::Witness(derivation) = outcome else {
        panic!("expected zero-step witness");
    };
    assert!(derivation.steps.is_empty(), "fixture must be a zero-step witness");
    let certificate = ReplayCertificate::issue(
        &system,
        ReplayQuery::Backward { target, goal },
        ReplayDerivation::Backward(derivation),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        ResourceObservation {
            states: 1,
            transitions: 0,
            retained_bytes: 32,
            operations: 1,
        },
    );
    assert!(matches!(
        builder().build(&system, &[certificate]),
        Err(NeuralError::EmptyTrace)
    ));
}

#[test]
fn approximate_trace_is_rejected_by_default() {
    let (system, certificate) = backward_witness();
    let mut approximate = certificate;
    approximate.guidance = GuidanceMode::HeuristicPruning { beam_width: 1 };
    approximate.guidance_hash = amari_rewrite::inverse::guidance_hash(&approximate.guidance);
    assert!(matches!(
        builder().build(&system, &[approximate]),
        Err(NeuralError::ApproximateTraceRejected)
    ));
}

#[test]
fn approximate_trace_is_labeled_heuristic_guidance_when_allowed() {
    let (system, certificate) = backward_witness();
    let mut approximate = certificate;
    approximate.guidance = GuidanceMode::HeuristicPruning { beam_width: 1 };
    approximate.guidance_hash = amari_rewrite::inverse::guidance_hash(&approximate.guidance);

    let config = DatasetConfig::new(100, 16, 64, true, None, RelationLimits::default())
        .expect("config");
    let dataset = DatasetBuilder::new(config)
        .build(&system, &[approximate])
        .expect("build");
    assert!(!dataset.is_empty());
    for example in dataset.examples() {
        assert_eq!(example.authority(), LabelAuthority::HeuristicGuidance);
        assert_ne!(example.authority(), LabelAuthority::ProofPositive);
    }
    assert_eq!(dataset.manifest().proof_positive(), 0);
    assert_eq!(dataset.manifest().heuristic_guidance(), dataset.len());
}

#[test]
fn trace_ceiling_is_a_typed_limit() {
    let (system, certificate) = backward_witness();
    let config = DatasetConfig::new(100, 1, 64, false, None, RelationLimits::default())
        .expect("config");
    assert!(matches!(
        DatasetBuilder::new(config).build(&system, &[certificate.clone(), certificate]),
        Err(NeuralError::LimitExceeded { resource: "traces", .. })
    ));
}

#[test]
fn example_ceiling_without_seed_is_a_typed_limit() {
    let (system, certificate) = backward_witness();
    let config = DatasetConfig::new(1, 16, 64, false, None, RelationLimits::default())
        .expect("config");
    assert!(matches!(
        DatasetBuilder::new(config).build(&system, &[certificate]),
        Err(NeuralError::LimitExceeded { resource: "examples", .. })
    ));
}

#[test]
fn dataset_is_invariant_to_trace_permutation() {
    let (system, certificate) = backward_witness();
    // A second independent witness for a distinct goal, so the trace set
    // has more than one element to permute.
    let target = parse("a");
    let goal = parse("h(a)");
    let outcome = BackwardExplorer::new(&system, InverseSearchConfig::default())
        .search(&target, &goal, SearchMode::BreadthFirst)
        .expect("search succeeds");
    let BackwardSearchOutcome::Witness(derivation) = outcome else {
        panic!("expected witness");
    };
    let other = ReplayCertificate::issue(
        &system,
        ReplayQuery::Backward { target, goal },
        ReplayDerivation::Backward(derivation),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        ResourceObservation {
            states: 2,
            transitions: 2,
            retained_bytes: 128,
            operations: 3,
        },
    );

    let forward = builder()
        .build(&system, &[certificate.clone(), other.clone()])
        .expect("build");
    let reversed = builder()
        .build(&system, &[other, certificate])
        .expect("build");
    assert_eq!(forward, reversed, "permutation must not change the dataset");
    assert_eq!(
        forward.manifest().trace_digests(),
        reversed.manifest().trace_digests(),
        "per-trace digests are canonically ordered"
    );
}

#[test]
fn trace_drift_is_rejected() {
    let (system, certificate) = backward_witness();
    let mut drifted = certificate;
    // Tamper an intermediate predecessor term. Verify replays through
    // provenance alone and still passes, but re-derivation from the
    // tampered state can no longer find the next chosen transition.
    match &mut drifted.derivation {
        ReplayDerivation::Backward(derivation) => {
            assert!(derivation.steps.len() >= 2, "fixture needs two steps");
            derivation.steps[0].term = Term::constant("bogus");
        }
        ReplayDerivation::Bidirectional(_) => unreachable!(),
    }
    assert!(drifted.verify(&system).is_ok(), "verify replays provenance only");
    assert!(matches!(
        builder().build(&system, &[drifted]),
        Err(NeuralError::TraceDrift)
    ));
}

#[test]
fn manifest_records_schema_version_digest_counts_and_trace_digests() {
    let (system, certificate) = backward_witness();
    let dataset = builder().build(&system, &[certificate]).expect("build");
    let manifest = dataset.manifest();
    assert_eq!(manifest.schema_version(), 1);
    assert_eq!(
        manifest.feature_schema_digest(),
        amari_rewrite::neural::FeatureSchemaV1.digest()
    );
    assert_eq!(manifest.proof_positive(), dataset.len());
    assert_eq!(manifest.heuristic_guidance(), 0);
    assert_eq!(manifest.trace_digests().len(), 1);
    assert_eq!(manifest.seed(), None, "no sub-sampling occurred");
}

#[cfg(feature = "serialize")]
#[test]
fn a_deeply_nested_trace_is_rejected_before_materializing() {
    // A deeply nested predecessor term deserialized into a certificate
    // must be rejected by the bounded admission walk before `Term::clone`
    // recurses. Deep-drop discipline: the fixture is leaked, never unwound.
    let mut deep = Term::constant("a");
    for _ in 0..10_000 {
        deep = Term::sym("f", [deep]);
    }
    let (system, certificate) = backward_witness();
    let json = serde_json::to_string(&certificate).expect("serialize");
    let mut restored: ReplayCertificate = serde_json::from_str(&json).expect("deserialize");
    match &mut restored.derivation {
        ReplayDerivation::Backward(derivation) => {
            derivation.steps[0].term = deep.clone();
        }
        ReplayDerivation::Bidirectional(_) => unreachable!(),
    }
    std::mem::forget(deep);

    let result = builder().build(&system, &[restored]);
    assert!(
        matches!(
            result,
            Err(NeuralError::LimitExceeded {
                resource: "term depth",
                ..
            })
        ),
        "expected the bounded depth rejection, got {result:?}"
    );
}
