// SPDX-License-Identifier: MIT OR Apache-2.0

//! Typed replay resource errors (Task 47, PR #384 round 5, F26).
//!
//! The replay shares one [`RelationResources`] meter between borrowed admission
//! and the unification it performs, so an exhausted budget surfaces from
//! `unify` as [`RewriteError::RelationLimitExceeded`]. The pre-remediation
//! replay discarded EVERY unification error with `map_err(|_| mismatch(..))`,
//! so a certificate that exhausted the operation budget was reported as a
//! `ResidualMismatch` — "the witness does not unify" — which is a different,
//! wrong diagnosis and forfeits the typed limit a caller must be able to see.
//!
//! Only a genuine [`RewriteError::UnificationFailure`] is a replay mismatch.
//! These contracts pin both branches:
//!
//! - a budget-exhausting zero-step witness and a budget-exhausting leaf/goal
//!   unifier both surface `RelationLimitExceeded { resource: "operations" }`;
//! - a genuine head clash still surfaces `ResidualMismatch`.
//!
//! The fixture is the doubling chain (F16/F25): the goal's `pair(X{k+1},X{k+1})`
//! arguments make each successive occurs check walk the whole chain below it, so
//! the boundary unification costs ~1.5·n² operations and honestly exceeds the
//! replay's 1,000,000-operation budget at `n = 1300` — with the input terms
//! themselves comfortably inside the 4,096-node ceiling.

use amari_rewrite::inverse::{
    BackwardDerivation, BackwardProvenance, GuidanceMode, InverseSearchConfig, ReplayCertificate,
    ReplayDerivation, ReplayQuery, ResourceObservation, SymbolicPredecessor,
};
use amari_rewrite::relation::{ConstraintSet, RelationLimits, RelationResources, RuleId};
use amari_rewrite::trs::{Rule, Substitution, Term, TermSystem};
use amari_rewrite::Path;
use amari_rewrite::RewriteError;

/// The number of links whose doubling goal is inside the node ceiling but whose
/// occurs walk is honest budget exhaustion.
const ARITY: usize = 1_300;

/// `f(Z, X0, ..., X{arity-1})` — the leaf of the doubling chain.
fn doubling_leaf(arity: usize) -> Term {
    let mut args = vec![Term::var("Z")];
    args.extend((0..arity).map(|index| Term::var(format!("X{index}"))));
    Term::sym("f", args)
}

/// `f(X0, pair(X1,X1), ..., pair(X{arity-1},X{arity-1}), a)`.
fn doubling_goal(arity: usize) -> Term {
    let mut args = vec![Term::var("X0")];
    for index in 1..arity {
        let variable = Term::var(format!("X{index}"));
        args.push(Term::sym("pair", vec![variable.clone(), variable]));
    }
    args.push(Term::constant("a"));
    Term::sym("f", args)
}

/// `f(X0, X0, X1, ..., X{arity-1})` — the zero-step target of the finding.
fn doubling_target(arity: usize) -> Term {
    let mut args = vec![Term::var("X0"), Term::var("X0")];
    args.extend((1..arity).map(|index| Term::var(format!("X{index}"))));
    Term::sym("f", args)
}

/// Iterative node/depth measurement, so the shape assertion does not add a
/// recursive walk to the fixture.
fn shape(term: &Term) -> (usize, usize) {
    let mut nodes = 0usize;
    let mut depth = 1usize;
    let mut stack = vec![(term, 1usize)];
    while let Some((node, node_depth)) = stack.pop() {
        nodes += 1;
        depth = depth.max(node_depth);
        if let Term::Sym(_, args) = node {
            for arg in args {
                stack.push((arg, node_depth + 1));
            }
        }
    }
    (nodes, depth)
}

fn observation() -> ResourceObservation {
    ResourceObservation {
        states: 1,
        transitions: 0,
        retained_bytes: 64,
        operations: 0,
    }
}

/// A zero-step backward certificate (the target/goal unification is the whole
/// derivation).
fn zero_step_certificate(system: &TermSystem, target: Term, goal: Term) -> ReplayCertificate {
    ReplayCertificate::issue(
        system,
        ReplayQuery::Backward { target, goal },
        ReplayDerivation::Backward(BackwardDerivation { steps: Vec::new() }),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        observation(),
    )
}

/// A one-step backward certificate with the supplied recorded leaf, so the
/// LEAF/GOAL unification is the deciding step.
fn leaf_goal_certificate(
    system: &TermSystem,
    target: Term,
    goal: Term,
    leaf: Term,
) -> ReplayCertificate {
    let rule = Rule::new(Term::constant("a"), Term::constant("b")).expect("checked rule");
    let provenance = BackwardProvenance {
        rule_id: RuleId::from_rule(&rule),
        position: Path::root(),
        scope: 1,
        target_hash: amari_rewrite::relation::Sha256Digest::canonical_term(
            "amari.relation.term/v1",
            &target,
        ),
        predecessor_hash: amari_rewrite::relation::Sha256Digest::canonical_term(
            "amari.relation.term/v1",
            &leaf,
        ),
    };
    let step = SymbolicPredecessor {
        provenance,
        term: leaf,
        existentials: Vec::new(),
        constraints: ConstraintSet::new(),
        substitution: Substitution::new(),
        resources: RelationResources::new(&RelationLimits::default()),
    };
    ReplayCertificate::issue(
        system,
        ReplayQuery::Backward { target, goal },
        ReplayDerivation::Backward(BackwardDerivation { steps: vec![step] }),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        ResourceObservation {
            states: 1,
            transitions: 1,
            retained_bytes: 128,
            operations: 4,
        },
    )
}

/// The doubling fixture is inside the node and depth ceilings, so the rejection
/// these contracts observe comes from the unification budget and not from a
/// shape ceiling.
#[test]
fn the_budget_exhausting_fixture_is_shape_legal() {
    for term in [
        doubling_goal(ARITY),
        doubling_target(ARITY),
        doubling_leaf(ARITY),
    ] {
        let (nodes, depth) = shape(&term);
        assert!(
            nodes <= RelationLimits::MAX_TERM_NODES,
            "the fixture is inside the node ceiling: {nodes} nodes"
        );
        assert!(
            depth <= RelationLimits::MAX_TERM_DEPTH,
            "the fixture is inside the depth ceiling: depth {depth}"
        );
    }
}

/// F26 (P2): a zero-step witness that exhausts the unification budget is a typed
/// resource rejection, NOT a `ResidualMismatch`.
#[test]
fn a_zero_step_budget_exhaustion_is_not_a_replay_mismatch() {
    let system = TermSystem::new(Vec::new());
    let certificate = zero_step_certificate(&system, doubling_target(ARITY), doubling_goal(ARITY));
    let error = certificate
        .verify(&system)
        .expect_err("the boundary unification exceeds the replay budget");
    assert!(
        matches!(
            error,
            RewriteError::RelationLimitExceeded {
                resource: "operations",
                ..
            }
        ),
        "the zero-step witness keeps its typed resource error, got {error:?}"
    );
}

/// F26 (P2): the same through the LEAF/GOAL unification branch.
#[test]
fn a_leaf_goal_budget_exhaustion_is_not_a_replay_mismatch() {
    let system = TermSystem::new(Vec::new());
    let certificate = leaf_goal_certificate(
        &system,
        Term::constant("b"),
        doubling_goal(ARITY),
        doubling_leaf(ARITY),
    );
    let error = certificate
        .verify(&system)
        .expect_err("the leaf/goal unification exceeds the replay budget");
    assert!(
        matches!(
            error,
            RewriteError::RelationLimitExceeded {
                resource: "operations",
                ..
            }
        ),
        "the leaf/goal unifier keeps its typed resource error, got {error:?}"
    );
}

/// F26 control: a GENUINE unification failure is still a replay mismatch.
///
/// Only `UnificationFailure` is translated, so both branches keep their mismatch
/// diagnosis for a head clash.
#[test]
fn a_genuine_unification_failure_is_still_a_replay_mismatch() {
    let system = TermSystem::new(Vec::new());
    let zero_step = zero_step_certificate(
        &system,
        Term::sym("f", vec![Term::constant("a")]),
        Term::sym("f", vec![Term::constant("b")]),
    );
    let error = zero_step
        .verify(&system)
        .expect_err("a head clash does not unify");
    assert!(
        matches!(error, RewriteError::ResidualMismatch { .. }),
        "a genuine clash stays a certificate mismatch, got {error:?}"
    );

    let leaf_goal = leaf_goal_certificate(
        &system,
        Term::constant("b"),
        Term::sym("g", vec![Term::var("Y")]),
        Term::sym("f", vec![Term::constant("z")]),
    );
    let error = leaf_goal
        .verify(&system)
        .expect_err("a leaf/goal head clash does not unify");
    assert!(
        matches!(error, RewriteError::ResidualMismatch { .. }),
        "a genuine leaf/goal clash stays a certificate mismatch, got {error:?}"
    );
}
