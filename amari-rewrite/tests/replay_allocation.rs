// SPDX-License-Identifier: MIT OR Apache-2.0

//! Allocation-sensitive replay evidence (Task 47, PR #384 round 3, F11).
//!
//! F11's contract is not "the same typed error" — the pre-remediation replay
//! already ended in `RelationLimitExceeded` for an exploding substitution, but
//! only *after* `Substitution::apply` had materialized the whole expansion: the
//! reviewer's leaf `f(X11, ..., X0)` against the goal
//! `f(a, pair(X11,X11), ..., pair(X1,X1))` built an 8,179-node tree before
//! `record_term` rejected it. This binary therefore instruments the global
//! allocator and pins the observable difference: a checked substitution
//! application must determine the expanded shape, charge the budget, and reject
//! *before* it allocates an expansion proportional to the over-limit term.
//!
//! The positive control is the same shape one step below the ceiling: the
//! near-boundary expansion is admitted (and does allocate, as it must).
//!
//! Everything happens inside one test so the process-wide counters are not
//! perturbed by a concurrently running sibling.

#![cfg(feature = "std")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use amari_rewrite::inverse::{
    BackwardDerivation, BackwardProvenance, GuidanceMode, InverseSearchConfig, ReplayCertificate,
    ReplayDerivation, ReplayQuery, ResourceObservation, SymbolicPredecessor,
};
use amari_rewrite::relation::{ConstraintSet, RelationLimits, RelationResources, RuleId};
use amari_rewrite::trs::{Rule, Substitution, Term, TermSystem};
use amari_rewrite::Path;

struct Counting;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static PEAK_BYTES: AtomicUsize = AtomicUsize::new(0);

impl Counting {
    fn record(size: usize) {
        ALLOCATIONS.fetch_add(1, Ordering::SeqCst);
        let live = LIVE_BYTES.load(Ordering::SeqCst) + size;
        LIVE_BYTES.store(live, Ordering::SeqCst);
        let _ = PEAK_BYTES.fetch_max(live, Ordering::SeqCst);
    }
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            Self::record(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        LIVE_BYTES.fetch_sub(layout.size(), Ordering::SeqCst);
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let moved = unsafe { System.realloc(pointer, layout, new_size) };
        if !moved.is_null() {
            LIVE_BYTES.fetch_sub(layout.size(), Ordering::SeqCst);
            Self::record(new_size);
        }
        moved
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// Run `body`, returning its value with the allocation count and peak live-byte
/// growth observed while it ran.
fn measure<T>(body: impl FnOnce() -> T) -> (usize, usize, T) {
    let start = LIVE_BYTES.load(Ordering::SeqCst);
    ALLOCATIONS.store(0, Ordering::SeqCst);
    PEAK_BYTES.store(start, Ordering::SeqCst);
    let value = body();
    let allocations = ALLOCATIONS.load(Ordering::SeqCst);
    let peak = PEAK_BYTES.load(Ordering::SeqCst).saturating_sub(start);
    (allocations, peak, value)
}

/// `X{arity-1}, ..., X0` — the reviewer's leaf order.
fn exponent_leaf(arity: usize, head: &str) -> Term {
    Term::sym(
        head,
        (0..arity).rev().map(|index| Term::var(format!("X{index}"))),
    )
}

fn exponent_rule(arity: usize) -> Rule {
    let lhs = Term::sym(
        "f",
        (0..arity).rev().map(|index| Term::var(format!("U{index}"))),
    );
    Rule::new(lhs, Term::constant("c")).expect("checked rule")
}

/// The reviewer's explosion: leaf `f(X11, ..., X0)` unified with
/// `f(a, pair(X11,X11), ..., pair(X1,X1))` binds the chain
/// `X10 |-> pair(X11,X11)`, `X9 |-> pair(X10,X10)`, ... so deep application
/// builds an 8,179-node tree.
fn explosion(arity: usize) -> (TermSystem, ReplayCertificate) {
    let rule = exponent_rule(arity);
    let system = TermSystem::new(vec![rule.clone()]);
    let leaf = exponent_leaf(arity, "f");
    let mut goal_args = vec![Term::constant("a")];
    for index in (1..arity).rev() {
        let variable = Term::var(format!("X{index}"));
        goal_args.push(Term::sym("pair", [variable.clone(), variable]));
    }
    let goal = Term::sym("f", goal_args);
    let target = Term::constant("c");
    let rule_id = RuleId::from_rule(&rule);
    let certificate = ReplayCertificate::issue(
        &system,
        ReplayQuery::Backward {
            target: target.clone(),
            goal,
        },
        ReplayDerivation::Backward(BackwardDerivation {
            steps: vec![SymbolicPredecessor {
                provenance: BackwardProvenance {
                    rule_id,
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
                },
                term: leaf,
                existentials: Vec::new(),
                constraints: ConstraintSet::new(),
                substitution: Substitution::new(),
                resources: RelationResources::new(&RelationLimits::default()),
            }],
        }),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        ResourceObservation {
            states: 2,
            transitions: 1,
            retained_bytes: 256,
            operations: 16,
        },
    );
    (system, certificate)
}

#[test]
fn substitution_expansion_is_rejected_before_allocation() {
    // The over-limit expansion: rejected by the term node ceiling, and the
    // rejection must not allocate the expansion it is rejecting.
    let (system, certificate) = explosion(12);
    let (allocations, peak, outcome) = measure(|| certificate.verify(&system));
    let error = outcome.expect_err("an over-limit expansion must not verify");
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
    assert!(
        peak < 64 * 1024,
        "rejecting an 8,179-node expansion must not materialize it: peak {peak} bytes, \
         {allocations} allocations"
    );
    assert!(
        allocations < 2_000,
        "rejecting an 8,179-node expansion must not allocate node-by-node: {allocations} \
         allocations, peak {peak} bytes"
    );

    // Near-boundary positive control: the same shape three chain links shorter
    // (expands to 1,014 nodes at arity 9) still verifies, so the preflight is a
    // bound and not a rejection of substitution application as such. The
    // admitted expansion does allocate (it is materialized), so the sanity
    // ceiling here is generous.
    let (system, certificate) = explosion(9);
    let (allocations, peak, outcome) = measure(|| certificate.verify(&system));
    outcome.expect("a near-boundary expansion must verify");
    assert!(
        peak < 1024 * 1024,
        "the admitted expansion stays small: peak {peak} bytes, {allocations} allocations"
    );
}
