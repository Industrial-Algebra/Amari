// SPDX-License-Identifier: MIT OR Apache-2.0

//! Round-1 review remediations for holographic trace recall (Task 47, PR #384).
//!
//! One contract per round-1 finding, each the reviewer's reproduction pinned as
//! a behavioural contract. Every contract in this file failed against the
//! pre-remediation implementation:
//!
//! - **F1** — a recorded *intermediate* step term is content, not commentary:
//!   replacing it (with no hash touched) must not validate, so the admission
//!   covers the terms actually encoded and not just the endpoints;
//! - **F3** — the borrowed step count is capped against the trace ceiling
//!   *before* the certificate is replayed or any term is cloned;
//! - **F5** — borrowed term shapes (node and depth ceilings) are measured
//!   *before* the certificate is replayed or any term is cloned, and because
//!   every replayed state must equal a measured recorded step term (F1), the
//!   replay's own intermediate terms are metered by the same measurement;
//! - **F6** — the documented `[-1, 1]` similarity interval is a stated
//!   invariant, so the exposed lanes are clamped to it rather than carried one
//!   ulp outside it.
//!
//! F2 (charge-before-push in the term worklist) and F4 (no public raw-token
//! encoding path) are pinned closer to the code they constrain: F2 as a unit
//! contract in `holographic::encode`, and F4 as a `compile_fail` doctest on the
//! frozen encoder's module documentation.

#![cfg(feature = "holographic-guidance")]

use amari_rewrite::holographic::{
    HolographicEncoder, HolographicError, RecallConfig, RecallTrace, TraceCorpus, MAX_TRACE_TERMS,
};
use amari_rewrite::inverse::{
    BackwardDerivation, BackwardExplorer, BackwardProvenance, BackwardSearchOutcome,
    BidirectionalDerivation, ForwardStep, GuidanceMode, InverseSearchConfig, ReplayCertificate,
    ReplayDerivation, ReplayQuery, ResourceObservation, SearchMode, SymbolicPredecessor,
};
use amari_rewrite::relation::{
    ConstraintSet, RelationLimits, RelationResources, RuleId, Sha256Digest,
};
use amari_rewrite::trs::{Rule, Substitution, Term, TermSystem};
use amari_rewrite::Path;

/// The frozen encoder seed the goldens in `holographic_inverse` use.
const SEED: u64 = 0x5441_534B_3437_0001;

fn constant(name: &str) -> Term {
    Term::constant(name)
}

fn nested(term: Term, levels: usize) -> Term {
    (0..levels).fold(term, |inner, _| Term::sym("h", [inner]))
}

/// `a => b`, `b => c`: backward from `c` to `a` derives the step terms
/// `[b, a]` in derivation order (goal side first), so `b` is an intermediate
/// term and neither endpoint.
fn chain_system() -> TermSystem {
    TermSystem::new(vec![
        Rule::new(constant("a"), constant("b")).unwrap(),
        Rule::new(constant("b"), constant("c")).unwrap(),
    ])
}

fn resource_observation() -> ResourceObservation {
    ResourceObservation {
        states: 3,
        transitions: 2,
        retained_bytes: 256,
        operations: 32,
    }
}

fn backward_certificate(system: &TermSystem, target: Term, goal: Term) -> ReplayCertificate {
    let outcome = BackwardExplorer::new(system, InverseSearchConfig::default())
        .search(&target, &goal, SearchMode::BreadthFirst)
        .expect("search must run");
    let BackwardSearchOutcome::Witness(derivation) = outcome else {
        panic!("the baseline system must yield a witness");
    };
    ReplayCertificate::issue(
        system,
        ReplayQuery::Backward { target, goal },
        ReplayDerivation::Backward(derivation),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        resource_observation(),
    )
}

fn hash(term: &Term) -> Sha256Digest {
    Sha256Digest::canonical_term("amari.relation.term/v1", term)
}

fn predecessor(
    term: Term,
    rule_id: RuleId,
    target: &Term,
    position: Path,
    scope: u32,
) -> SymbolicPredecessor {
    SymbolicPredecessor {
        provenance: BackwardProvenance {
            rule_id,
            position,
            scope,
            target_hash: hash(target),
            predecessor_hash: hash(&term),
        },
        term,
        existentials: Vec::new(),
        constraints: ConstraintSet::new(),
        substitution: Substitution::new(),
        resources: RelationResources::new(&RelationLimits::default()),
    }
}

// --------------------------------------------------------------------- F1

/// F1 (P1, `recall.rs:146`): the recorded intermediate step terms are verified
/// against the replayed state sequence.
///
/// A tampered intermediate is content the encoder would embed, so it must not
/// validate — and therefore must never reach admission. Nothing but the
/// recorded term changes here: the query, system, config, and guidance hashes
/// still recompute exactly.
#[test]
fn tampered_intermediate_step_term_is_a_typed_rejection() {
    let system = chain_system();
    let certificate = backward_certificate(&system, constant("c"), constant("a"));

    // Baseline: the genuine witness replays, validates, and admits.
    RecallTrace::validate(&system, &certificate).expect("the genuine witness validates");
    let mut corpus = TraceCorpus::<64>::new(
        HolographicEncoder::new(SEED, RelationLimits::default()).unwrap(),
        RecallConfig::default(),
    );
    corpus
        .admit(&RecallTrace::validate(&system, &certificate).unwrap())
        .expect("the genuine witness admits");

    // Tamper the RECORDED intermediate `b` (index 0 is goal-side first, so
    // `b` is the intermediate and `a` the leaf that unifies with the goal).
    let mut tampered = certificate.clone();
    let ReplayDerivation::Backward(derivation) = &mut tampered.derivation else {
        unreachable!("a backward certificate holds a backward derivation");
    };
    assert_eq!(derivation.steps.len(), 2, "the chain has two steps");
    assert_eq!(derivation.steps[0].term, constant("b"));
    derivation.steps[0].term = constant("z");

    // No hash was touched, so every binding still recomputes.
    assert_eq!(tampered.query_hash, certificate.query_hash);
    assert_eq!(tampered.system_hash, certificate.system_hash);
    assert_eq!(tampered.config_hash, certificate.config_hash);
    assert_eq!(tampered.guidance_hash, certificate.guidance_hash);

    // The replay must compare each recorded term with the state it replays to.
    assert!(
        tampered.verify(&system).is_err(),
        "a fabricated intermediate term must not verify"
    );
    let error = RecallTrace::validate(&system, &tampered)
        .expect_err("a fabricated intermediate term must not mint a trace");
    assert!(
        matches!(error, HolographicError::UnvalidatedProvenance(_)),
        "the replay rejects the tampered intermediate: {error:?}"
    );

    // Admission is unreachable for the tampered content: no `RecallTrace`
    // exists, so the corpus cannot hold it.
    assert_eq!(corpus.len(), 1);
}

/// F1, bidirectional half: the recorded backward half's intermediate terms are
/// verified against the replayed sequence too, because those are the terms the
/// encoder embeds for a bidirectional certificate.
#[test]
fn tampered_bidirectional_backward_term_is_a_typed_rejection() {
    let f = Rule::new(Term::sym("f", [Term::var("X")]), Term::var("X")).expect("checked");
    let g = Rule::new(Term::sym("g", [Term::var("X")]), Term::var("X")).expect("checked");
    let system = TermSystem::new(vec![f, g]);
    let g_id = RuleId::from_rule(&system.rules()[1]);

    let source = Term::sym("g", [Term::sym("g", [Term::sym("f", [constant("a")])])]);
    let goal = Term::sym("f", [constant("a")]);

    // `g(g(f(a))) -> g(f(a))` forward, `f(a) -> g(f(a))` backward: the two
    // halves meet at `g(f(a))`.
    let meeting_term = Term::sym("g", [Term::sym("f", [constant("a")])]);
    let forward_step = ForwardStep {
        rule_id: g_id,
        position: Path::root(),
        source_hash: hash(&source),
        result_hash: hash(&meeting_term),
    };
    let backward_step = predecessor(meeting_term.clone(), g_id, &goal, Path::root(), 1);
    let derivation = BidirectionalDerivation {
        forward_steps: vec![forward_step],
        backward_steps: vec![backward_step],
        meeting_forward: meeting_term.clone(),
        meeting_backward: meeting_term,
        unifier: Substitution::new(),
    };
    let certificate = ReplayCertificate::issue(
        &system,
        ReplayQuery::Bidirectional {
            source: source.clone(),
            goal: goal.clone(),
        },
        ReplayDerivation::Bidirectional(derivation),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        resource_observation(),
    );

    RecallTrace::validate(&system, &certificate).expect("the genuine witness validates");

    // Tamper the recorded backward predecessor term only.
    let mut tampered = certificate.clone();
    let ReplayDerivation::Bidirectional(derivation) = &mut tampered.derivation else {
        unreachable!("a bidirectional certificate holds a bidirectional derivation");
    };
    assert_eq!(derivation.backward_steps.len(), 1);
    derivation.backward_steps[0].term = constant("z");

    assert!(
        tampered.verify(&system).is_err(),
        "a fabricated backward predecessor term must not verify"
    );
    let error = RecallTrace::validate(&system, &tampered)
        .expect_err("a fabricated backward predecessor must not mint a trace");
    assert!(
        matches!(error, HolographicError::UnvalidatedProvenance(_)),
        "the replay rejects the tampered predecessor: {error:?}"
    );
}

// --------------------------------------------------------------------- F3

/// F3 (P2, `recall.rs:150`): the borrowed step count is checked against the
/// trace ceiling *before* any replay or cloning.
///
/// The certificate here carries genuine binding hashes but a derivation that
/// cannot replay, so the answer identifies which check ran first: pre-remediation
/// the replay ran and reported the reconstruction failure, post-remediation the
/// cap answers immediately and 10,001 terms are never cloned.
#[test]
fn step_count_cap_precedes_replay_and_cloning() {
    let system = chain_system();
    let rule_id = RuleId::from_rule(&system.rules()[0]);
    let target = constant("c");
    let goal = constant("a");
    // 10,000 recorded steps: one more than the 64-term ceiling can ever admit.
    let steps: Vec<SymbolicPredecessor> = (0..10_000)
        .map(|_| predecessor(constant("b"), rule_id, &constant("a"), Path::root(), 1))
        .collect();
    let certificate = ReplayCertificate::issue(
        &system,
        ReplayQuery::Backward {
            target: target.clone(),
            goal: goal.clone(),
        },
        ReplayDerivation::Backward(BackwardDerivation { steps }),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        resource_observation(),
    );

    let error = RecallTrace::validate(&system, &certificate)
        .expect_err("a 10,001-term certificate exceeds the trace ceiling");
    assert_eq!(
        error,
        HolographicError::LimitExceeded {
            resource: "trace terms",
            value: 10_001,
            limit: MAX_TRACE_TERMS,
        },
        "the borrowed step count is capped before the replay is attempted"
    );
}

// --------------------------------------------------------------------- F5

/// F5 (P2, `recall.rs:141`): borrowed endpoint shapes are measured before any
/// replay or cloning, so a depth-129 endpoint never reaches the replay.
#[test]
fn borrowed_endpoint_shapes_are_measured_before_replay() {
    let system = TermSystem::new(Vec::new());
    let deep_left = nested(constant("a"), 129);
    let deep_right = nested(constant("b"), 129);

    // These two endpoints do not unify, so the answer tells the checks apart:
    // pre-remediation the replay ran first and reported the failed unify;
    // post-remediation the shape ceiling answers first.
    let mismatched = ReplayCertificate::issue(
        &system,
        ReplayQuery::Backward {
            target: deep_left.clone(),
            goal: deep_right.clone(),
        },
        ReplayDerivation::Backward(BackwardDerivation { steps: Vec::new() }),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        resource_observation(),
    );
    assert_eq!(
        RecallTrace::validate(&system, &mismatched).expect_err("a depth-130 endpoint is rejected"),
        HolographicError::LimitExceeded {
            resource: "trace term depth",
            value: RelationLimits::MAX_TERM_DEPTH + 1,
            limit: RelationLimits::MAX_TERM_DEPTH,
        }
    );

    // The same shape is rejected even when the endpoints DO unify: a zero-step
    // certificate whose two endpoints are the identical depth-129 term used to
    // validate and only failed later, at the encoder's depth ceiling.
    let unified = ReplayCertificate::issue(
        &system,
        ReplayQuery::Backward {
            target: deep_left.clone(),
            goal: deep_left,
        },
        ReplayDerivation::Backward(BackwardDerivation { steps: Vec::new() }),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        resource_observation(),
    );
    assert_eq!(
        RecallTrace::validate(&system, &unified)
            .expect_err("a depth-130 endpoint is rejected before encoding"),
        HolographicError::LimitExceeded {
            resource: "trace term depth",
            value: RelationLimits::MAX_TERM_DEPTH + 1,
            limit: RelationLimits::MAX_TERM_DEPTH,
        }
    );
}

// --------------------------------------------------------------------- F6

/// F6 (P3, `recall.rs:34`): the documented `[-1, 1]` similarity interval is an
/// invariant of the surface, so every exposed lane is clamped to it.
///
/// The canonical inner product can overshoot by one unit in the last place: at
/// `D = 3` the goal lane of an identical trace scores `1.0000000000000002`, and
/// at `D = 5` the ranked `combined` lane does.
#[test]
fn similarity_lanes_are_clamped_to_the_documented_interval() {
    let trace = RecallTrace::validate(
        &chain_system(),
        &backward_certificate(&chain_system(), constant("c"), constant("a")),
    )
    .expect("the genuine witness validates");

    // D = 3: the goal lane overshoots `1.0` by one ulp.
    let mut small = TraceCorpus::<3>::new(
        HolographicEncoder::new(SEED, RelationLimits::default()).unwrap(),
        RecallConfig::default(),
    );
    small.admit(&trace).expect("admission");
    let score = small.recall(&trace, 4).expect("recall must run").matches[0].score;
    assert_eq!(
        score.combined, 1.0,
        "an identical trace ranks at the top of the interval"
    );
    assert_eq!(
        score.goal_alignment, 1.0,
        "the goal lane is clamped to the documented maximum"
    );
    for lane in [score.combined, score.goal_alignment, score.path_alignment] {
        assert!(
            (-1.0..=1.0).contains(&lane),
            "lane {lane} is inside the documented interval"
        );
    }

    // D = 5: the ranked `combined` lane overshoots, so the ranked scalar is an
    // interval value too, not just an approximately bounded one.
    let mut wide = TraceCorpus::<5>::new(
        HolographicEncoder::new(SEED, RelationLimits::default()).unwrap(),
        RecallConfig::default(),
    );
    wide.admit(&trace).expect("admission");
    let score = wide.recall(&trace, 4).expect("recall must run").matches[0].score;
    assert_eq!(
        score.combined, 1.0,
        "the ranked lane is clamped to the documented maximum"
    );
    for lane in [score.combined, score.goal_alignment, score.path_alignment] {
        assert!(
            (-1.0..=1.0).contains(&lane),
            "lane {lane} is inside the documented interval"
        );
    }
}
