// SPDX-License-Identifier: MIT OR Apache-2.0

//! Replay-generated state bounds (Task 47, PR #384 round 3).
//!
//! A certificate's *borrowed* terms are admitted by
//! [`amari_rewrite::holographic::RecallTrace::validate`], but a replay also
//! GENERATES states: every recorded step is re-applied through the system, so
//! the intermediate terms are derived, not borrowed. Those generated states are
//! metered here:
//!
//! - **F12a** — each derivation half's step count is validated against the
//!   embedded configuration's ceilings *before* the half is replayed. Ten
//!   thousand alternating forward steps can never come from a search whose
//!   depth ceiling is 64, so the certificate is rejected before the replay
//!   walks it. A derivation with exactly the embedded depth ceiling is still
//!   admitted (near-boundary positive control);
//! - **F12b** — every generated/instantiated/rewritten state's node and depth
//!   shape is preflighted before it is allocated, including the matcher RHS
//!   application and the rebuild at the recorded position. A forward rule whose
//!   RHS has 5,001 nodes is rejected before the intermediate is materialized,
//!   even though both borrowed endpoints are constants. A near-boundary wide
//!   intermediate (4,095 nodes) is still admitted.

use amari_rewrite::inverse::{
    BidirectionalDerivation, ForwardStep, GuidanceMode, InverseSearchConfig, ReplayCertificate,
    ReplayDerivation, ReplayQuery, ResourceObservation,
};
use amari_rewrite::relation::{RelationLimits, RuleId};
use amari_rewrite::trs::{Rule, Substitution, Term, TermSystem};
use amari_rewrite::Path;

fn observation() -> ResourceObservation {
    ResourceObservation {
        states: 4,
        transitions: 2,
        retained_bytes: 256,
        operations: 64,
    }
}

fn certificate(
    system: &TermSystem,
    source: Term,
    goal: Term,
    derivation: BidirectionalDerivation,
) -> ReplayCertificate {
    ReplayCertificate::issue(
        system,
        ReplayQuery::Bidirectional { source, goal },
        ReplayDerivation::Bidirectional(derivation),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        observation(),
    )
}

/// A recorded root forward step under `rule`.
fn step(rule: &Rule) -> ForwardStep {
    ForwardStep {
        rule_id: RuleId::from_rule(rule),
        position: Path::root(),
        source_hash: amari_rewrite::relation::Sha256Digest::framed(
            "amari.relation.term/v1",
            b"recorded-step",
        ),
        result_hash: amari_rewrite::relation::Sha256Digest::framed(
            "amari.relation.term/v1",
            b"recorded-step",
        ),
    }
}

/// F12a (P2): a derivation half longer than the embedded depth ceiling is
/// rejected before it is replayed.
///
/// Rules `a -> b` and `b -> a`, source `a`, goal `a`: ten thousand alternating
/// root steps replay successfully (and were admitted pre-remediation) although
/// no search under `max_depth = 64` can produce a 10,000-step half.
#[test]
fn replay_rejects_a_derivation_deeper_than_the_embedded_ceiling() {
    let ab = Rule::new(Term::constant("a"), Term::constant("b")).expect("checked rule");
    let ba = Rule::new(Term::constant("b"), Term::constant("a")).expect("checked rule");
    let system = TermSystem::new(vec![ab.clone(), ba.clone()]);
    let forward_steps: Vec<ForwardStep> = (0..10_000)
        .map(|index| if index % 2 == 0 { step(&ab) } else { step(&ba) })
        .collect();
    let certificate = certificate(
        &system,
        Term::constant("a"),
        Term::constant("a"),
        BidirectionalDerivation {
            forward_steps,
            backward_steps: Vec::new(),
            meeting_forward: Term::constant("a"),
            meeting_backward: Term::constant("a"),
            unifier: Substitution::new(),
        },
    );
    let error = certificate
        .verify(&system)
        .expect_err("a half deeper than the embedded ceiling must not verify");
    assert!(
        matches!(
            error,
            amari_rewrite::RewriteError::RelationLimitExceeded {
                resource: "replay forward steps",
                ..
            }
        ),
        "the typed rejection names the forward step ceiling, got {error}"
    );
}

/// F12a positive control: exactly the embedded depth ceiling verifies.
#[test]
fn replay_admits_a_derivation_at_the_embedded_depth_ceiling() {
    let ab = Rule::new(Term::constant("a"), Term::constant("b")).expect("checked rule");
    let ba = Rule::new(Term::constant("b"), Term::constant("a")).expect("checked rule");
    let system = TermSystem::new(vec![ab.clone(), ba.clone()]);
    let depth = InverseSearchConfig::default().max_depth() as usize;
    assert_eq!(depth, 64, "the control is the exact embedded ceiling");
    let forward_steps: Vec<ForwardStep> = (0..depth)
        .map(|index| if index % 2 == 0 { step(&ab) } else { step(&ba) })
        .collect();
    let certificate = certificate(
        &system,
        Term::constant("a"),
        Term::constant("a"),
        BidirectionalDerivation {
            forward_steps,
            backward_steps: Vec::new(),
            meeting_forward: Term::constant("a"),
            meeting_backward: Term::constant("a"),
            unifier: Substitution::new(),
        },
    );
    certificate
        .verify(&system)
        .expect("a half at the embedded depth ceiling must verify");
}

/// Rules `a -> wide(b, ..., b)` and `wide(X1, ..., Xn) -> a` with `n` children.
fn wide_rules(children: usize) -> (TermSystem, Rule, Rule) {
    let wide = |head: &str| Term::sym(head, (0..children).map(|_| Term::constant("b")));
    let pattern = Term::sym(
        "wide",
        (0..children).map(|index| Term::var(format!("X{index}"))),
    );
    let up = Rule::new(Term::constant("a"), wide("wide")).expect("checked rule");
    let down = Rule::new(pattern, Term::constant("a")).expect("checked rule");
    let system = TermSystem::new(vec![up.clone(), down.clone()]);
    (system, up, down)
}

/// F12b (P2): a generated intermediate larger than the term ceiling is
/// rejected before it is materialized.
///
/// Rules `a -> wide(b x 5000)` and `wide(X1, ..., X5000) -> a`, source `a`,
/// goal `a`. Both borrowed endpoints are constants, but the forward half
/// rewrites `a` into a 5,001-node intermediate: generated content, so it is
/// bounded by the same node ceiling as any other replay state.
#[test]
fn replay_rejects_an_over_limit_generated_intermediate() {
    let (system, up, down) = wide_rules(5_000);
    let certificate = certificate(
        &system,
        Term::constant("a"),
        Term::constant("a"),
        BidirectionalDerivation {
            forward_steps: vec![step(&up), step(&down)],
            backward_steps: Vec::new(),
            meeting_forward: Term::constant("a"),
            meeting_backward: Term::constant("a"),
            unifier: Substitution::new(),
        },
    );
    let error = certificate
        .verify(&system)
        .expect_err("an over-limit generated intermediate must not verify");
    assert!(
        matches!(
            error,
            amari_rewrite::RewriteError::RelationLimitExceeded {
                resource: "term nodes",
                limit,
            } if limit == RelationLimits::MAX_TERM_NODES
        ),
        "the typed rejection is the term node ceiling, got {error}"
    );
}

/// F12b positive control: a near-boundary wide intermediate verifies.
///
/// The generated intermediate has 4,095 nodes (4,094 children plus its own
/// node), one node below the 4,096 ceiling.
#[test]
fn replay_admits_a_near_boundary_generated_intermediate() {
    let children = RelationLimits::MAX_TERM_NODES - 2;
    let (system, up, down) = wide_rules(children);
    let certificate = certificate(
        &system,
        Term::constant("a"),
        Term::constant("a"),
        BidirectionalDerivation {
            forward_steps: vec![step(&up), step(&down)],
            backward_steps: Vec::new(),
            meeting_forward: Term::constant("a"),
            meeting_backward: Term::constant("a"),
            unifier: Substitution::new(),
        },
    );
    certificate
        .verify(&system)
        .expect("a near-boundary generated intermediate must verify");
}
