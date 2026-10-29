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
    ReplayQuery, ResourceObservation, SearchMode, SymbolicPredecessor, SymbolicState,
};
use amari_rewrite::neural::{
    CandidateInput, DatasetBuilder, DatasetConfig, LabelAuthority, NeuralError, SearchContext,
};
use amari_rewrite::relation::{
    ConstraintSet, LogicVar, RelationLimits, RelationResources, RuleId, Sha256Digest,
    TermConstraint,
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

/// The joint alpha-canonical state digest — the identity Task 40's
/// candidate ranking keys on.
fn state_digest(text: &str) -> Sha256Digest {
    SymbolicState::new(parse(text), ConstraintSet::new()).canonical_digest()
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
    // The root expansion `a -> {add(a,zero), h(a)}` chose `h(a)` (the
    // lower-growth candidate is expanded first), leaving `add(a, zero)`
    // unchosen.
    let root = dataset
        .examples()
        .iter()
        .find(|example| example.target_distance() == 1)
        .expect("a root-round example");
    assert_eq!(root.chosen_digest(), state_digest("h(a)"));
    assert_eq!(root.unchosen_digest(), state_digest("add(a, zero)"));
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
    let mut expected = vec![state_digest("f(f(a))"), state_digest("f(g(a))")];
    expected.sort();
    assert_eq!(unchosen, expected);
    for example in dataset.examples() {
        assert_eq!(example.authority(), LabelAuthority::ProofPositive);
        assert_eq!(example.chosen_digest(), state_digest("g(f(a))"));
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
    assert!(
        distances.contains(&1),
        "root round is depth 1, got {distances:?}"
    );
    assert!(
        distances.contains(&2),
        "second round is depth 2, got {distances:?}"
    );
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
        state_digest("f(a)"),
        "the lone surviving unchosen candidate is `f(a)`"
    );
}

#[test]
fn explicit_seed_sampling_is_deterministic_and_recorded() {
    let (system, certificate) = backward_witness();
    // A one-example ceiling over a two-example trace forces sub-sampling.
    let config =
        DatasetConfig::new(1, 16, 64, false, Some(7), RelationLimits::default()).expect("config");
    let first = DatasetBuilder::new(config)
        .expect("builder")
        .build(&system, std::slice::from_ref(&certificate))
        .expect("build");
    let second = DatasetBuilder::new(config)
        .expect("builder")
        .build(&system, std::slice::from_ref(&certificate))
        .expect("build");
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
    assert!(
        derivation.steps.is_empty(),
        "fixture must be a zero-step witness"
    );
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

    let config =
        DatasetConfig::new(100, 16, 64, true, None, RelationLimits::default()).expect("config");
    let dataset = DatasetBuilder::new(config)
        .expect("builder")
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
    let config =
        DatasetConfig::new(100, 1, 64, false, None, RelationLimits::default()).expect("config");
    assert!(matches!(
        DatasetBuilder::new(config)
            .expect("builder")
            .build(&system, &[certificate.clone(), certificate]),
        Err(NeuralError::LimitExceeded {
            resource: "traces",
            ..
        })
    ));
}

#[test]
fn example_ceiling_without_seed_is_a_typed_limit() {
    let (system, certificate) = backward_witness();
    let config =
        DatasetConfig::new(1, 16, 64, false, None, RelationLimits::default()).expect("config");
    assert!(matches!(
        DatasetBuilder::new(config)
            .expect("builder")
            .build(&system, &[certificate]),
        Err(NeuralError::LimitExceeded {
            resource: "examples",
            ..
        })
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
    assert!(
        drifted.verify(&system).is_ok(),
        "verify replays provenance only"
    );
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
    // recurses. The fixture is moved into the trace (no clone, which would
    // recurse) and the whole trace is leaked (never unwound).
    let (system, certificate) = backward_witness();
    let json = serde_json::to_string(&certificate).expect("serialize");
    let mut restored: ReplayCertificate = serde_json::from_str(&json).expect("deserialize");

    let mut deep = Term::constant("a");
    for _ in 0..10_000 {
        deep = Term::sym("f", [deep]);
    }
    match &mut restored.derivation {
        ReplayDerivation::Backward(derivation) => {
            let _ = std::mem::replace(&mut derivation.steps[0].term, deep);
        }
        ReplayDerivation::Bidirectional(_) => unreachable!(),
    }

    let result = builder().build(&system, std::slice::from_ref(&restored));
    std::mem::forget(restored);

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

// ── PR #357 round 1 regression contracts ────────────────────────────

/// A one-step certificate whose recorded step is the engine's `chosen`-th
/// predecessor of `target` — the fixture that lets a test pin one specific
/// candidate (including a duplicate state produced by a distinct rule).
fn one_step(system: &TermSystem, target: Term, goal: Term, chosen: usize) -> ReplayCertificate {
    let candidates = amari_rewrite::inverse::symbolic_predecessors(
        system,
        &target,
        &RelationLimits::default(),
        1,
    )
    .expect("predecessors");
    let outcome = BackwardExplorer::new(system, InverseSearchConfig::default())
        .search(&target, &goal, SearchMode::BreadthFirst)
        .expect("search succeeds");
    let BackwardSearchOutcome::Witness(mut derivation) = outcome else {
        panic!("expected a witness");
    };
    derivation.steps = vec![candidates[chosen].clone()];
    ReplayCertificate::issue(
        system,
        ReplayQuery::Backward { target, goal },
        ReplayDerivation::Backward(derivation),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        ResourceObservation {
            states: 3,
            transitions: 3,
            retained_bytes: 256,
            operations: 4,
        },
    )
}

#[test]
fn an_oversized_query_identifier_is_rejected_before_verification() {
    // P1a: a 1 MiB variable name in the certified goal must be rejected by
    // admission — before verification hashes or clones the query. Admission
    // covers every certificate field, not only the derivation terms.
    let (system, certificate) = backward_witness();
    let mut query = certificate.query.clone();
    if let ReplayQuery::Backward { ref mut goal, .. } = query {
        *goal = Term::var("V".repeat(1_048_576));
    }
    let oversized = ReplayCertificate::issue(
        &system,
        query,
        certificate.derivation.clone(),
        certificate.config,
        certificate.guidance.clone(),
        certificate.resources,
    );

    let result = builder().build(&system, &[oversized]);
    assert!(
        matches!(result, Err(NeuralError::LimitExceeded { .. })),
        "oversized identifier must be rejected before verification hashes/clones query, got {result:?}"
    );
}

#[test]
fn the_chosen_transition_survives_digest_deduplication() {
    // P1b: `f(X) -> X` and `f(a) -> a` both derive the state `f(a)` from
    // `a`. Choosing the second predecessor records the second rule's
    // provenance; locating the chosen transition must happen BEFORE the
    // duplicate states collapse, or the survivor's provenance disagrees and
    // a valid certificate is misreported as TraceDrift.
    let system = sys(vec![
        Rule::new(parse("f(X)"), parse("X")).expect("checked"),
        Rule::new(parse("f(a)"), parse("a")).expect("checked"),
        Rule::new(parse("g(X)"), parse("X")).expect("checked"),
    ]);
    let certificate = one_step(&system, parse("a"), parse("f(a)"), 1);
    certificate.verify(&system).expect("fixture must verify");

    let result = builder().build(&system, &[certificate]);
    assert!(
        result.is_ok(),
        "valid rederived chosen should survive dedup, got {result:?}"
    );
}

#[test]
fn branch_factor_reflects_the_pre_dedup_candidate_set() {
    // P1c: the search saw four candidates (two of them duplicates by
    // state); the branch-factor lane must encode the branching the engine
    // actually faced, not the post-dedup negative count.
    let system = sys(vec![
        Rule::new(parse("f(X)"), parse("X")).expect("checked"),
        Rule::new(parse("f(a)"), parse("a")).expect("checked"),
        Rule::new(parse("g(X)"), parse("X")).expect("checked"),
        Rule::new(parse("g(a)"), parse("a")).expect("checked"),
    ]);
    let certificate = one_step(&system, parse("a"), parse("f(a)"), 0);
    let dataset = builder().build(&system, &[certificate]).expect("build");
    let lane = amari_rewrite::neural::FeatureSchemaV1::lane_index("search.branch_factor")
        .expect("the branch-factor lane is part of the frozen schema");
    let observed = dataset.examples()[0]
        .chosen()
        .lane(lane)
        .expect("lane in range");

    assert_eq!(
        observed,
        3.0_f32 / 17.0,
        "live ranker sees four candidates before dedup"
    );
}

#[test]
fn trace_digest_covers_the_certified_query() {
    // P2: two certificates over the same derivation but different certified
    // goals are different statements, so their trace identities must differ.
    let (system, certificate) = backward_witness();
    let mut query = certificate.query.clone();
    if let ReplayQuery::Backward { ref mut goal, .. } = query {
        *goal = parse("Anything");
    }
    let other = ReplayCertificate::issue(
        &system,
        query,
        certificate.derivation.clone(),
        certificate.config,
        certificate.guidance.clone(),
        certificate.resources,
    );
    other.verify(&system).expect("fixture must verify");

    let baseline = builder().build(&system, &[certificate]).expect("build");
    let changed = builder().build(&system, &[other]).expect("build");
    assert_ne!(
        baseline.manifest().trace_digests(),
        changed.manifest().trace_digests(),
        "different certified query should have different trace identity"
    );
}

// ── PR #357 round 2 regression contracts ────────────────────────────

/// A one-step certificate whose recorded leaf is the engine's variable
/// predecessor of `a` under `f(X) -> a`. The last step term is only used
/// for goal unification during replay, and it is not a state of any
/// later round, so it can be replaced with another admissible instance.
fn variable_leaf_certificate() -> (TermSystem, ReplayCertificate) {
    let system = sys(vec![Rule::new(parse("f(X)"), parse("a")).expect("checked")]);
    let target = parse("a");
    let goal = Term::var("Goal");
    let candidates = amari_rewrite::inverse::symbolic_predecessors(
        &system,
        &target,
        &RelationLimits::default(),
        1,
    )
    .expect("predecessors");
    let outcome = BackwardExplorer::new(&system, InverseSearchConfig::default())
        .search(&target, &goal, SearchMode::BreadthFirst)
        .expect("search succeeds");
    let BackwardSearchOutcome::Witness(mut derivation) = outcome else {
        panic!("expected a witness");
    };
    derivation.steps = vec![candidates[0].clone()];
    let certificate = ReplayCertificate::issue(
        &system,
        ReplayQuery::Backward { target, goal },
        ReplayDerivation::Backward(derivation),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        ResourceObservation {
            states: 2,
            transitions: 1,
            retained_bytes: 128,
            operations: 2,
        },
    );
    certificate.verify(&system).expect("fixture must verify");
    (system, certificate)
}

/// Build the baseline and one mutated certificate, returning both trace
/// digest lists. The mutation must itself be replay-valid: a digest test
/// that changes the certified statement would only exercise `verify`.
fn mutated_trace_digests(
    system: &TermSystem,
    baseline: &ReplayCertificate,
    mutate: impl FnOnce(&mut ReplayDerivation),
) -> (Vec<Sha256Digest>, Vec<Sha256Digest>) {
    let mut changed = baseline.clone();
    mutate(&mut changed.derivation);
    changed
        .verify(system)
        .expect("the mutated fixture must still replay");
    let baseline = builder()
        .build(system, std::slice::from_ref(baseline))
        .expect("baseline build")
        .manifest()
        .trace_digests()
        .to_vec();
    let changed = builder()
        .build(system, std::slice::from_ref(&changed))
        .expect("changed build")
        .manifest()
        .trace_digests()
        .to_vec();
    (baseline, changed)
}

#[test]
fn unifier_transitive_chain_is_bounded_before_replay() {
    // P1a: `V0 -> V1 -> ... -> V20000 -> a` has individually tiny ranges and
    // a tiny entry ceiling violation only detectable by counting, but replay's
    // recursive resolution would walk all 20,001 links. Admission must reject
    // it typed before `verify` can recurse. The fixture is leaked because a
    // pre-fix run aborts the stack (deep-drop discipline).
    let (system, mut certificate) = bidirectional_witness();
    if let ReplayDerivation::Bidirectional(derivation) = &mut certificate.derivation {
        derivation.meeting_backward = parse("g(f(V0))");
        for index in 0..20_000 {
            derivation
                .unifier
                .insert(format!("V{index}"), Term::var(format!("V{}", index + 1)));
        }
        derivation.unifier.insert("V20000", parse("a"));
    }
    let result = builder().build(&system, std::slice::from_ref(&certificate));
    std::mem::forget(certificate);
    assert!(
        matches!(
            result,
            Err(NeuralError::LimitExceeded {
                resource: "substitution entries",
                ..
            })
        ),
        "a 20,001-entry unifier must be rejected at admission, got {result:?}"
    );
}

#[test]
fn unifier_chain_depth_is_bounded_independently_of_entry_count() {
    // P1a: the entry ceiling alone is not enough. 129 entries is below the
    // 4,096 admission ceiling, but the transitive chain is 128 links deep —
    // longer than any admitted term-depth walk. Admission must bound the
    // chain itself, iteratively, with a typed limit naming the resource.
    let (system, mut certificate) = bidirectional_witness();
    if let ReplayDerivation::Bidirectional(derivation) = &mut certificate.derivation {
        derivation.meeting_backward = parse("g(f(V0))");
        for index in 0..128 {
            derivation
                .unifier
                .insert(format!("V{index}"), Term::var(format!("V{}", index + 1)));
        }
        derivation.unifier.insert("V128", parse("a"));
    }
    let result = builder().build(&system, std::slice::from_ref(&certificate));
    assert!(
        matches!(
            result,
            Err(NeuralError::LimitExceeded {
                resource: "substitution chain depth",
                ..
            })
        ),
        "a 128-link unifier chain must be rejected at admission, got {result:?}"
    );
}

#[test]
fn unifier_chain_through_term_structure_is_bounded() {
    // P1a: a range need not be a bare variable to extend a chain.
    // `V0 -> f(V1) -> f(f(V2)) -> ...` resolves 128 variables even though
    // every individual range is a tiny two-node term, so the bound must
    // follow bound variables *inside* ranges, not only bare-variable ranges.
    let (system, mut certificate) = bidirectional_witness();
    if let ReplayDerivation::Bidirectional(derivation) = &mut certificate.derivation {
        derivation.meeting_backward = parse("g(f(V0))");
        for index in 0..128 {
            derivation.unifier.insert(
                format!("V{index}"),
                Term::sym("f", [Term::var(format!("V{}", index + 1))]),
            );
        }
        derivation.unifier.insert("V128", parse("a"));
    }
    let result = builder().build(&system, std::slice::from_ref(&certificate));
    assert!(
        matches!(
            result,
            Err(NeuralError::LimitExceeded {
                resource: "substitution chain depth",
                ..
            })
        ),
        "a 128-link chain through range structure must be rejected, got {result:?}"
    );
}

#[test]
fn unifier_expanded_depth_is_bounded_across_composed_ranges() {
    // P1 (round 3): the per-range depth ceiling and the chain ceiling are
    // each necessary but neither is sufficient. Here every range is a
    // 63-wrapper `f(...(V{i+1}))` term (depth 64, admitted) and the chain is
    // 64 links (admitted), yet resolving `V0` transitively *expands* to a
    // term of depth ~3970. Replay's recursive application walks that depth
    // and aborts the stack, so admission must bound the EXPANDED structure —
    // the depth after transitively resolving each variable through the
    // unifier — iteratively, before recursion. The fixture is leaked because
    // a pre-fix run aborts (deep-drop discipline).
    let (system, mut certificate) = bidirectional_witness();
    if let ReplayDerivation::Bidirectional(derivation) = &mut certificate.derivation {
        derivation.meeting_backward = parse("g(f(V0))");
        for index in 0..63 {
            let mut range = Term::var(format!("V{}", index + 1));
            for _ in 0..63 {
                range = Term::sym("f", [range]);
            }
            derivation.unifier.insert(format!("V{index}"), range);
        }
        derivation.unifier.insert("V63", parse("a"));
    }
    let result = builder().build(&system, std::slice::from_ref(&certificate));
    std::mem::forget(certificate);
    assert!(
        matches!(
            result,
            Err(NeuralError::LimitExceeded {
                resource: "expanded substitution depth",
                ..
            })
        ),
        "composed expansion depth (~3970) must be rejected at admission, got {result:?}"
    );
}

#[test]
fn unifier_expanded_nodes_are_bounded_across_composed_ranges() {
    // P1 (round 3), node half: eight ranges of `h(V{i+1}, V{i+1}, V{i+1},
    // V{i+1})` are four nodes each with depth 2, and the chain is nine links —
    // all admitted. Expansion is multiplicative: `V0` resolves to 4^8 = 65,536
    // nodes at depth 9. The expanded NODE count must be bounded too, not only
    // the depth.
    let (system, mut certificate) = bidirectional_witness();
    if let ReplayDerivation::Bidirectional(derivation) = &mut certificate.derivation {
        derivation.meeting_backward = parse("g(f(V0))");
        for index in 0..8 {
            let next = Term::var(format!("V{}", index + 1));
            derivation.unifier.insert(
                format!("V{index}"),
                Term::sym("h", [next.clone(), next.clone(), next.clone(), next]),
            );
        }
        derivation.unifier.insert("V8", parse("a"));
    }
    let result = builder().build(&system, std::slice::from_ref(&certificate));
    assert!(
        matches!(
            result,
            Err(NeuralError::LimitExceeded {
                resource: "expanded substitution nodes",
                ..
            })
        ),
        "a 65,536-node composed expansion must be rejected at admission, got {result:?}"
    );
}

#[test]
fn residual_constraint_digest_preserves_variable_sharing() {
    // P2a (round 3): hashing each constraint independently renumbers its
    // variables from zero, so `{X != a, X != b}` and `{X != a, Y != b}` —
    // genuinely different residual sets — hashed identically. The constraint
    // set is one identity scope: encoding every constraint under a SHARED
    // variable-renaming environment preserves co-occurrence.
    let (system, baseline) = backward_witness();
    let with_constraints = |pairs: [(&str, &str); 2]| {
        let mut certificate = baseline.clone();
        if let ReplayDerivation::Backward(derivation) = &mut certificate.derivation {
            derivation.steps[0].constraints = ConstraintSet::new();
            for (left, right) in pairs {
                derivation.steps[0]
                    .constraints
                    .insert(TermConstraint::NotEqual(parse(left), parse(right)));
            }
        }
        certificate
            .verify(&system)
            .expect("constraints do not affect replay");
        certificate
    };
    let shared = with_constraints([("X", "a"), ("X", "b")]);
    let distinct = with_constraints([("X", "a"), ("Y", "b")]);
    let shared_digests = builder()
        .build(&system, std::slice::from_ref(&shared))
        .expect("shared-variable build")
        .manifest()
        .trace_digests()
        .to_vec();
    let distinct_digests = builder()
        .build(&system, std::slice::from_ref(&distinct))
        .expect("distinct-variable build")
        .manifest()
        .trace_digests()
        .to_vec();
    assert_ne!(
        shared_digests, distinct_digests,
        "a set whose second constraint reuses the first constraint's variable is not \
         alpha-equivalent to one that does not (P2a)"
    );
}

#[test]
fn alpha_equivalent_constraint_sets_share_a_trace_digest() {
    // P2a settled invariant: the fix must keep the alpha-invariance that the
    // residual identity promises. `{X != a, X != b}` and `{W != a, W != b}`
    // are the same set up to variable renaming, so they share one digest.
    let (system, baseline) = backward_witness();
    let renamed = |variable: &str| {
        let mut certificate = baseline.clone();
        if let ReplayDerivation::Backward(derivation) = &mut certificate.derivation {
            derivation.steps[0].constraints = ConstraintSet::new();
            for side in ["a", "b"] {
                derivation.steps[0]
                    .constraints
                    .insert(TermConstraint::NotEqual(parse(variable), parse(side)));
            }
        }
        certificate
            .verify(&system)
            .expect("constraints do not affect replay");
        certificate
    };
    let first = builder()
        .build(&system, std::slice::from_ref(&renamed("X")))
        .expect("X build")
        .manifest()
        .trace_digests()
        .to_vec();
    let second = builder()
        .build(&system, std::slice::from_ref(&renamed("W")))
        .expect("W build")
        .manifest()
        .trace_digests()
        .to_vec();
    assert_eq!(
        first, second,
        "alpha-equivalent residual constraint sets must share a trace digest"
    );
}

#[test]
fn existential_vectors_are_held_to_an_admission_ceiling() {
    // P2b (round 3): a step's existential vector is admitted like every other
    // auxiliary payload — bounded by the per-step constraint ceiling at
    // admission, before verification or hashing. A million-entry vector used
    // to be accepted and hashed whole.
    let (system, mut certificate) = backward_witness();
    if let ReplayDerivation::Backward(derivation) = &mut certificate.derivation {
        derivation.steps[0].existentials = (0..1_000_000)
            .map(|index| LogicVar::new(1, index))
            .collect();
    }
    let result = builder().build(&system, std::slice::from_ref(&certificate));
    assert!(
        matches!(
            result,
            Err(NeuralError::LimitExceeded {
                resource: "existentials",
                value: 1_000_000,
                limit: RelationLimits::MAX_CONSTRAINTS,
            })
        ),
        "a 1,000,000-entry existential vector must be rejected at admission, got {result:?}"
    );
}

#[test]
fn forward_derivation_half_is_held_to_the_step_ceiling() {
    // P1b: the step ceiling used to apply only to the backward half. 65,537
    // identity forward steps replay fine and were admitted; both halves must
    // be counted at admission, and the forward count must name its resource.
    let (system, mut certificate) = bidirectional_witness();
    let mut rules = system.rules().to_vec();
    rules.push(Rule::new(parse("g(X)"), parse("g(X)")).expect("checked"));
    let system = sys(rules);
    let identity = RuleId::from_rule(&system.rules()[2]);
    if let ReplayDerivation::Bidirectional(derivation) = &mut certificate.derivation {
        derivation.forward_steps = vec![
            ForwardStep {
                rule_id: identity,
                position: Path::root(),
                source_hash: canonical("g(f(a))"),
                result_hash: canonical("g(f(a))"),
            };
            65_537
        ];
    }
    let certificate = ReplayCertificate::issue(
        &system,
        ReplayQuery::Bidirectional {
            source: parse("g(f(a))"),
            goal: parse("f(a)"),
        },
        certificate.derivation,
        certificate.config,
        certificate.guidance,
        certificate.resources,
    );
    let result = builder().build(&system, &[certificate]);
    assert!(
        matches!(
            result,
            Err(NeuralError::LimitExceeded {
                resource: "forward steps",
                ..
            })
        ),
        "65,537 forward steps must hit the trace-step ceiling, got {result:?}"
    );
}

#[test]
fn trace_digest_covers_backward_step_payloads() {
    // P2: the documented identity scope is "query + full derivation". A
    // mutable derivation field that replay ignores (substitution, residual
    // constraints, existentials) must still change the trace digest.
    let (system, baseline) = backward_witness();

    let (base, changed) = mutated_trace_digests(&system, &baseline, |derivation| {
        let ReplayDerivation::Backward(backward) = derivation else {
            panic!("backward fixture");
        };
        backward.steps[0]
            .substitution
            .insert("Unused", parse("different"));
    });
    assert_ne!(
        base, changed,
        "a backward step's substitution must enter the trace identity (P2)"
    );

    let (base, changed) = mutated_trace_digests(&system, &baseline, |derivation| {
        let ReplayDerivation::Backward(backward) = derivation else {
            panic!("backward fixture");
        };
        backward.steps[0]
            .constraints
            .insert(TermConstraint::Equal(parse("a"), parse("b")));
    });
    assert_ne!(
        base, changed,
        "a backward step's constraints must enter the trace identity (P2)"
    );

    let (base, changed) = mutated_trace_digests(&system, &baseline, |derivation| {
        let ReplayDerivation::Backward(backward) = derivation else {
            panic!("backward fixture");
        };
        backward.steps[0].existentials.push(LogicVar::new(0, 7));
    });
    assert_ne!(
        base, changed,
        "a backward step's existentials must enter the trace identity (P2)"
    );
}

#[test]
fn trace_digest_covers_the_backward_step_term() {
    // P2: the step term itself is part of the recorded derivation. Two
    // replay-valid instances of the same rule (`f(?v)` and `f(h(a))` both
    // replay `a -> f(..)`) must not share a trace identity.
    let (system, baseline) = variable_leaf_certificate();
    let (base, changed) = mutated_trace_digests(&system, &baseline, |derivation| {
        let ReplayDerivation::Backward(backward) = derivation else {
            panic!("backward fixture");
        };
        backward.steps[0].term = parse("f(h(a))");
    });
    assert_ne!(
        base, changed,
        "a backward step's term must enter the trace identity (P2)"
    );
}

#[test]
fn trace_digest_covers_forward_step_payloads() {
    // P2 forward audit: every `ForwardStep` field is identity-bearing, and
    // the engine hashes are the only record of the rewritten terms. Mutating
    // one that replay does not consult must still change the digest.
    let (system, baseline) = bidirectional_witness();
    let (base, changed) = mutated_trace_digests(&system, &baseline, |derivation| {
        let ReplayDerivation::Bidirectional(bidirectional) = derivation else {
            panic!("bidirectional fixture");
        };
        bidirectional.forward_steps[0].result_hash = canonical("changed");
    });
    assert_ne!(
        base, changed,
        "a forward step's result hash must enter the trace identity (P2)"
    );
}

#[test]
fn alpha_equivalent_candidates_are_feature_and_context_equivalent() {
    // P3: `f(X,Y) -> X` and `f(A,B) -> A` are alpha-equivalent rules, so
    // they share one stable `RuleId` and one `BackwardProvenance`; the
    // candidates differ only in freshly erased variable names. Chosen
    // matching is therefore modulo alpha-equivalence. The silent pick of the
    // first matching candidate is sound only because alpha-equivalent
    // candidates encode to identical features and context — assert exactly
    // that, so the equivalence is a checked guarantee, not an assumption.
    let system = sys(vec![
        Rule::new(parse("f(X,Y)"), parse("X")).expect("checked"),
        Rule::new(parse("f(A,B)"), parse("A")).expect("checked"),
        Rule::new(parse("g(X)"), parse("X")).expect("checked"),
    ]);
    let candidates = amari_rewrite::inverse::symbolic_predecessors(
        &system,
        &parse("a"),
        &RelationLimits::default(),
        1,
    )
    .expect("predecessors");
    assert_ne!(
        candidates[0].term, candidates[1].term,
        "alpha-equivalent candidates must differ in fresh names"
    );
    assert_eq!(
        candidates[0].provenance, candidates[1].provenance,
        "alpha-equivalent candidates share provenance"
    );

    // Encode the two candidates under one identical context: the frozen
    // feature vector must not depend on the fresh erased-variable names.
    let encoder = *builder().encoder();
    let context = SearchContext::new(
        1,
        candidates.len() as u64,
        false,
        0,
        0,
        candidates[0].existentials.len(),
        0,
    );
    let first_features = encoder
        .encode(&CandidateInput::new(
            &candidates[0].term,
            &candidates[0].constraints,
            &candidates[0].provenance,
            context,
        ))
        .expect("first candidate encodes");
    let second_features = encoder
        .encode(&CandidateInput::new(
            &candidates[1].term,
            &candidates[1].constraints,
            &candidates[1].provenance,
            context,
        ))
        .expect("second candidate encodes");
    assert_eq!(
        first_features, second_features,
        "alpha-equivalent candidates must encode to identical features"
    );

    let first = one_step(&system, parse("a"), candidates[0].term.clone(), 0);
    let second = one_step(&system, parse("a"), candidates[1].term.clone(), 1);
    first.verify(&system).expect("first fixture verifies");
    second.verify(&system).expect("second fixture verifies");
    let first = builder().build(&system, &[first]).expect("first build");
    let second = builder().build(&system, &[second]).expect("second build");
    assert_eq!(
        first.examples(),
        second.examples(),
        "alpha-equivalent chosen candidates must featurize identically"
    );
}

// ── PR #357 round 4 regression contracts ────────────────────────────

/// Trace digests for one certificate through the full dataset pipeline.
fn trace_digests_of(system: &TermSystem, certificate: &ReplayCertificate) -> Vec<Sha256Digest> {
    builder()
        .build(system, std::slice::from_ref(certificate))
        .expect("build")
        .manifest()
        .trace_digests()
        .to_vec()
}

/// The two-variable-carrier system: from `a`, `f(X,Y,Z) -> X` derives
/// `f(a,U,V)` and `g(X) -> X` derives `g(a)`, so the chosen one-step
/// backward transition carries a term with two distinct variable positions
/// and a third unchosen negative.
fn association_system() -> TermSystem {
    sys(vec![
        Rule::new(parse("f(X,Y,Z)"), parse("X")).expect("checked"),
        Rule::new(parse("g(X)"), parse("X")).expect("checked"),
    ])
}

/// The step term is `f(a,U,V)`; residual constraints are replaced by the
/// single disequality `arguments[index] != b`.
fn residual_on_argument(system: &TermSystem, index: usize) -> ReplayCertificate {
    let mut certificate = one_step(system, parse("a"), parse("f(a,U,V)"), 0);
    let ReplayDerivation::Backward(derivation) = &mut certificate.derivation else {
        panic!("backward fixture");
    };
    let Term::Sym(_, arguments) = &derivation.steps[0].term else {
        panic!("the step term is an application");
    };
    let variable = arguments[index].clone();
    derivation.steps[0].constraints = ConstraintSet::new();
    derivation.steps[0]
        .constraints
        .insert(TermConstraint::NotEqual(variable, parse("b")));
    certificate
        .verify(system)
        .expect("residual constraints do not affect replay");
    certificate
}

#[test]
fn step_term_and_residual_constraints_share_one_renaming_environment() {
    // P2a (round 4): the residual set used to be hashed in its own renaming
    // environment, so a step `(f(a,U,V), U != b)` and the step
    // `(f(a,U,V), V != b)` — different residuals over the same term — hashed
    // identically. Which variable of the term a residual mentions is part of
    // the step identity, so the term and its residuals are encoded under ONE
    // shared renaming environment.
    let system = association_system();
    let on_first = trace_digests_of(&system, &residual_on_argument(&system, 1));
    let on_second = trace_digests_of(&system, &residual_on_argument(&system, 2));
    assert_ne!(
        on_first, on_second,
        "a residual naming the term's first variable is not the same step as one \
         naming its second (P2a round 4)"
    );
}

#[test]
fn step_substitution_ranges_share_one_renaming_environment() {
    // P2b (round 4): ranges were digested independently, so two ranges that
    // mention the SAME variable and two ranges that mention different
    // variables hashed identically — the sharing structure of a substitution
    // is part of its identity. Domains and ranges were also hashed as
    // literals, which lost alpha-invariance. One shared environment per step
    // fixes both.
    let (system, baseline) = backward_witness();
    let with_ranges = |second_range: &str| {
        let mut certificate = baseline.clone();
        let ReplayDerivation::Backward(derivation) = &mut certificate.derivation else {
            panic!("backward fixture");
        };
        derivation.steps[0]
            .substitution
            .insert("Unused1", Term::var("XX"));
        derivation.steps[0]
            .substitution
            .insert("Unused2", Term::var(second_range));
        certificate
            .verify(&system)
            .expect("a substitution does not affect replay");
        certificate
    };
    let shared = trace_digests_of(&system, &with_ranges("XX"));
    let distinct = trace_digests_of(&system, &with_ranges("YY"));
    assert_ne!(
        shared, distinct,
        "`{{U1 -> XX, U2 -> XX}}` and `{{U1 -> XX, U2 -> YY}}` are different \
         substitutions (P2b round 4)"
    );
}

#[test]
fn step_substitution_and_term_share_one_renaming_environment() {
    // Same family as P2a/P2b: a range that mentions one of the step term's
    // variables is not interchangeable with one that mentions another. If the
    // substitution kept its own environment the term/substitution association
    // would be invisible.
    let system = association_system();
    let with_range_on = |index: usize| {
        let mut certificate = residual_on_argument(&system, 1);
        let ReplayDerivation::Backward(derivation) = &mut certificate.derivation else {
            panic!("backward fixture");
        };
        let Term::Sym(_, arguments) = &derivation.steps[0].term else {
            panic!("the step term is an application");
        };
        let variable = arguments[index].clone();
        derivation.steps[0].substitution.insert("Extra", variable);
        certificate
            .verify(&system)
            .expect("a substitution does not affect replay");
        certificate
    };
    let on_first = trace_digests_of(&system, &with_range_on(1));
    let on_second = trace_digests_of(&system, &with_range_on(2));
    assert_ne!(
        on_first, on_second,
        "a substitution range naming the term's first variable is not the same \
         step as one naming its second (round 4 family)"
    );
}

#[test]
fn alpha_equivalent_steps_share_a_trace_digest() {
    // The settled invariant, reasserted against the joint per-step
    // environment: consistently renaming every variable of a step — the term's
    // variables, the substitution's domains AND range variables, and a
    // residual's variable — must not change the step identity. The rename is
    // chosen so that `BTreeMap` order changes (`A1,B1,C1` becomes
    // `A2,B2,Z1`), which is why the substitution's entry order cannot be the
    // name order alone: equal-shape runs are enumerated and the smallest
    // encoding wins.
    let system = association_system();
    let build = |variables: [&str; 2], names: [&str; 6]| {
        let [first, second] = variables;
        let mut certificate = one_step(
            &system,
            parse("a"),
            parse(&format!("f(a,{first},{second})")),
            0,
        );
        let ReplayDerivation::Backward(derivation) = &mut certificate.derivation else {
            panic!("backward fixture");
        };
        let [domain_a, domain_b, domain_c, shared, distinct, residual] = names;
        derivation.steps[0]
            .substitution
            .insert(domain_a, Term::var(shared));
        derivation.steps[0]
            .substitution
            .insert(domain_b, Term::var(shared));
        derivation.steps[0]
            .substitution
            .insert(domain_c, Term::var(distinct));
        derivation.steps[0].constraints = ConstraintSet::new();
        derivation.steps[0]
            .constraints
            .insert(TermConstraint::NotEqual(Term::var(residual), parse("b")));
        certificate
            .verify(&system)
            .expect("the renamed fixture must still replay");
        certificate
    };
    let first = build(["U", "V"], ["A1", "B1", "C1", "XX", "YY", "P"]);
    let second = build(["U2", "V2"], ["Z1", "A2", "B2", "PP", "QQ", "Q"]);
    assert_eq!(
        trace_digests_of(&system, &first),
        trace_digests_of(&system, &second),
        "alpha-equivalent steps must share a trace digest"
    );
}

// ── PR #357 round 5 regression contracts ────────────────────────────

/// The two-variable-carrier step from [`association_system`]: its term is
/// `f(a,?1.1,?1.2)` with `?1.1` recorded as the existential.
fn existential_step(system: &TermSystem, swap_arguments: bool) -> ReplayCertificate {
    let mut certificate = one_step(system, parse("a"), parse("f(a,B,C)"), 0);
    let ReplayDerivation::Backward(derivation) = &mut certificate.derivation else {
        panic!("backward fixture");
    };
    derivation.steps[0].substitution = Substitution::new();
    derivation.steps[0].constraints = ConstraintSet::new();
    derivation.steps[0].existentials.truncate(1);
    if swap_arguments {
        let Term::Sym(_, arguments) = &mut derivation.steps[0].term else {
            panic!("the step term is an application");
        };
        arguments.swap(1, 2);
    }
    certificate
        .verify(system)
        .expect("existentials and variable order do not affect replay");
    certificate
}

#[test]
fn existentials_are_references_through_the_shared_environment() {
    // P2a (round 5): existentials used to be encoded as their `(scope,
    // index)` names, outside the shared environment, so swapping two
    // variable arguments — an alpha-equivalent TERM — left a step whose
    // existential list still names `?1.1`, and both hashed identically. The
    // two steps are different claims: the existential names a different
    // argument. Routing existential references through the same per-step
    // environment makes which argument is existential part of the identity.
    let system = association_system();
    let unswapped = trace_digests_of(&system, &existential_step(&system, false));
    let swapped = trace_digests_of(&system, &existential_step(&system, true));
    assert_ne!(
        unswapped, swapped,
        "an existential naming the second argument is not the same step as one \
         naming the third (P2a round 5)"
    );
}

#[test]
fn equal_shape_ties_are_resolved_on_the_whole_step_payload() {
    // P2b (round 5): `{AA -> a, ZZ -> a}` has two equal-shape substitution
    // entries, so two arrangements exist and their entry encodings are
    // byte-identical — but they commit different environments. Choosing the
    // arrangement by the substitution segment alone left the ENVIRONMENT
    // alpha-dependent: `AA != b` and `ZZ != b` are alpha-equivalent residuals
    // over alpha-equivalent steps, yet the committed environments differed and
    // so did the digests. The tie is now resolved on the entire framed step
    // payload, so the arrangement follows the residual that consumes the
    // environment.
    let (system, baseline) = backward_witness();
    let with_residual = |variable: &str| {
        let mut certificate = baseline.clone();
        let ReplayDerivation::Backward(derivation) = &mut certificate.derivation else {
            panic!("backward fixture");
        };
        derivation.steps[0].substitution = Substitution::new();
        derivation.steps[0].substitution.insert("AA", parse("a"));
        derivation.steps[0].substitution.insert("ZZ", parse("a"));
        derivation.steps[0].constraints = ConstraintSet::new();
        derivation.steps[0]
            .constraints
            .insert(TermConstraint::NotEqual(Term::var(variable), parse("b")));
        certificate
            .verify(&system)
            .expect("a substitution and a residual do not affect replay");
        certificate
    };
    assert_eq!(
        trace_digests_of(&system, &with_residual("AA")),
        trace_digests_of(&system, &with_residual("ZZ")),
        "`{{AA -> a, ZZ -> a}}` with `AA != b` and `ZZ != b` are \
         alpha-equivalent steps (P2b round 5)"
    );
}
