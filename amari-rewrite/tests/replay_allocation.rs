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
//! Round 4 adds three more allocation-sensitive contracts, each of which the
//! pre-remediation code satisfied only by doing the work it should not have
//! done:
//!
//! - **F16** — the unifier's `resolve`/`occurs` traversal is borrowed and
//!   visited-bound. The doubling chain of `f(Z, X0, ..., X20)` against
//!   `f(X0, pair(X1,X1), ..., pair(X20,X20), a)` unfolded 2,097,151 nodes per
//!   occurs check and allocated a term fragment per level (31 million
//!   allocations) just to answer "the variable does not occur";
//! - **F19** — direct `ReplayCertificate::verify` admits borrowed shapes before
//!   `query_hash` and before cloning the source, so a 100,001-node borrowed
//!   source is rejected without an encoding or a clone proportional to it;
//! - **F21** — the aggregate admission of borrowed unifier ranges happens after
//!   the cheap certificate length ceilings and under ONE shared meter, so 100,000
//!   unused bindings cannot be walked before a length comparison rejects the
//!   certificate.
//!
//! Every test in this binary holds [`exclusive`] for its whole body, because the
//! counters are process-wide: the previous round achieved the same isolation by
//! having exactly one test.

#![cfg(feature = "std")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use amari_rewrite::inverse::{
    BackwardDerivation, BackwardProvenance, BidirectionalDerivation, ForwardStep, GuidanceMode,
    InverseSearchConfig, ReplayCertificate, ReplayDerivation, ReplayQuery, ResourceObservation,
    SymbolicPredecessor,
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

/// Serializes the allocation-sensitive tests: the counters are process-wide, so
/// a sibling test building its own fixtures while a measured closure runs would
/// be charged to that closure. Every test in this binary takes this guard for
/// its WHOLE body (setup included), which is what the single-test arrangement
/// of the previous round achieved by having exactly one test.
static MEASURE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Hold the allocation counters exclusively for the rest of the test body.
fn exclusive() -> std::sync::MutexGuard<'static, ()> {
    MEASURE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Run `body`, returning its value with the allocation count and peak live-byte
/// growth observed while it ran. The caller holds [`exclusive`].
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
    let _exclusive = exclusive();
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

/// `X{arity-1}, ..., X0` — the reviewer's leaf order, with a free leading
/// variable (F16's `f(Z, X0, ..., X20)`).
fn occurs_leaf(arity: usize) -> Term {
    // `f(Z, X0, ..., Xn)`: position `i` of the goal is `pair(X{i}, X{i})`, so
    // the LIFO worklist binds `Xn := a`, then `X{n-1} := pair(Xn, Xn)`, and so
    // on down the doubling chain.
    let mut args = vec![Term::var("Z")];
    args.extend((0..arity).map(|index| Term::var(format!("X{index}"))));
    Term::sym("f", args)
}

/// `f(X0, pair(X1,X1), ..., pair(Xn,Xn), a)` — the doubling chain that makes a
/// naive occurs walk unfold `2^(n+1) - 1` nodes.
fn occurs_goal(arity: usize) -> Term {
    let mut args = vec![Term::var("X0")];
    // Ascending order: `pair(X1,X1)` pairs with the leaf's `X0`, `pair(X2,X2)`
    // with `X1`, and so on, so the LIFO worklist builds the doubling chain
    // without binding a variable into its own expansion.
    for index in 1..arity {
        let variable = Term::var(format!("X{index}"));
        args.push(Term::sym("pair", [variable.clone(), variable]));
    }
    args.push(Term::constant("a"));
    Term::sym("f", args)
}

/// F16 (P1): unification meters (and de-duplicates) its resolve/occurs
/// traversal instead of unfolding the binding chain.
///
/// Leaf `f(Z, X0, ..., X20)` against goal `f(X0, pair(X1,X1), ..., pair(X20,X20),
/// a)`: the LIFO worklist binds `X20 := a`, then `X19 := pair(X20,X20)`, ... so
/// each occurs check resolves a chain that doubles at every level. The 21st
/// binding unfolds 2,097,151 nodes, and the old resolve cloned a term fragment
/// at every level of that unfold — millions of allocations for a traversal whose
/// answer is "X does not occur". A borrowed, visited-set walk inspects each
/// reachable binding at most once, so the same unifier costs a few thousand
/// allocations, all of them the unifier's own bound ranges.
#[test]
fn occurs_traversal_is_borrowed_and_visited_bound() {
    let _exclusive = exclusive();
    let leaf = occurs_leaf(21);
    let goal = occurs_goal(21);
    let (allocations, peak, outcome) = measure(|| {
        amari_rewrite::analysis::unify(
            &leaf,
            &goal,
            &mut RelationResources::new(&RelationLimits::default()),
        )
    });
    let substitution = outcome.expect("the doubling chain unifies");
    assert_eq!(
        substitution.len(),
        22,
        "one binding per chain link, plus the free leading variable"
    );
    assert!(
        allocations < 20_000,
        "a metered occurs walk must not allocate per unfolded node: {allocations} \
         allocations, peak {peak} bytes"
    );
    assert!(
        peak < 2 * 1024 * 1024,
        "the unifier holds bound ranges, not their expansions: peak {peak} bytes, \
         {allocations} allocations"
    );
}

/// F19 (P1): direct verification admits borrowed shapes before it hashes or
/// clones them.
///
/// A bidirectional certificate whose source has 100,000 children is over the
/// term-node ceiling on the source alone. `verify` used to recompute
/// `query_hash` (encoding the whole term) and clone the source before
/// admitting it, so rejecting the certificate cost work proportional to the
/// term it was rejecting. Admission is now the first thing after the cheap
/// configuration and step checks, and the rejection allocates nothing
/// proportional to the source (`RecallTrace`'s admission had the same defect;
/// this pins the direct path).
#[test]
fn direct_verify_admits_borrowed_terms_before_hashing_or_cloning() {
    let _exclusive = exclusive();
    let system = TermSystem::new(vec![
        Rule::new(Term::constant("a"), Term::constant("b")).expect("checked rule")
    ]);
    let wide = Term::sym("wide", (0..100_000).map(|_| Term::constant("a")));
    let certificate = ReplayCertificate::issue(
        &system,
        ReplayQuery::Bidirectional {
            source: wide.clone(),
            goal: wide.clone(),
        },
        ReplayDerivation::Bidirectional(BidirectionalDerivation {
            forward_steps: Vec::new(),
            backward_steps: Vec::new(),
            meeting_forward: wide.clone(),
            meeting_backward: wide,
            unifier: Substitution::new(),
        }),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        ResourceObservation {
            states: 2,
            transitions: 0,
            retained_bytes: 128,
            operations: 0,
        },
    );
    let (allocations, peak, outcome) = measure(|| certificate.verify(&system));
    let error = outcome.expect_err("a 100,001-node borrowed source must not verify");
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
        allocations < 1_000,
        "admitting the source rejects it before hashing or cloning: {allocations} \
         allocations, peak {peak} bytes"
    );
    assert!(
        peak < 256 * 1024,
        "no encoding or clone proportional to the rejected source: peak {peak} bytes, \
         {allocations} allocations"
    );
}

/// F21 (P2): every borrowed unifier range is admitted under ONE shared meter.
///
/// The recorded meeting substitution is caller-supplied borrowed data. A range
/// one node over the term ceiling is inadmissible whether or not the recorded
/// trace happens to reach it, and the aggregate admission runs under one
/// budget rather than one fresh unmetered walk per binding.
#[test]
fn verify_admits_every_borrowed_unifier_range_under_one_meter() {
    let _exclusive = exclusive();
    let system = TermSystem::new(Vec::new());
    let meeting = Term::constant("a");
    // `Z` never appears in the query or the derivation: the range is unused
    // data, and it is still bounded before any replay work.
    let oversized = Term::sym(
        "f",
        (0..RelationLimits::MAX_TERM_NODES).map(|_| Term::constant("a")),
    );
    let certificate = ReplayCertificate::issue(
        &system,
        ReplayQuery::Bidirectional {
            source: meeting.clone(),
            goal: meeting.clone(),
        },
        ReplayDerivation::Bidirectional(BidirectionalDerivation {
            forward_steps: Vec::new(),
            backward_steps: Vec::new(),
            meeting_forward: meeting.clone(),
            meeting_backward: meeting,
            unifier: Substitution::new().with("Z", oversized),
        }),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        ResourceObservation {
            states: 1,
            transitions: 0,
            retained_bytes: 64,
            operations: 0,
        },
    );
    let error = certificate
        .verify(&system)
        .expect_err("an over-limit borrowed range must not verify");
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

/// F21 (P2): cheap certificate length ceilings are checked before the aggregate
/// substitution admission.
///
/// The certificate below carries 100,000 tiny unused unifier bindings and a
/// forward half longer than its embedded depth ceiling. Walking every range
/// first cost one fresh admission walk per binding (one worklist allocation
/// each) before a cheap integer comparison rejected the half. The length
/// ceiling is checked first, so the rejection is reachable without touching the
/// substitution at all.
#[cfg(feature = "holographic-guidance")]
#[test]
fn admission_checks_cheap_lengths_before_walking_unifier_ranges() {
    let _exclusive = exclusive();
    use amari_rewrite::holographic::{HolographicError, RecallTrace};

    let ab = Rule::new(Term::constant("a"), Term::constant("b")).expect("checked rule");
    let ba = Rule::new(Term::constant("b"), Term::constant("a")).expect("checked rule");
    let system = TermSystem::new(vec![ab.clone(), ba.clone()]);
    let tight = InverseSearchConfig::new(
        2,
        1_024,
        4_096,
        4_096,
        64,
        4_096,
        65_536,
        100_000,
        1 << 20,
        1 << 20,
    )
    .expect("a tightened depth ceiling is valid");
    let forward_steps: Vec<ForwardStep> = (0..10)
        .map(|index| {
            let rule = if index % 2 == 0 { &ab } else { &ba };
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
        })
        .collect();
    let mut unifier = Substitution::new();
    for index in 0..100_000 {
        unifier.insert(format!("Z{index}"), Term::constant("a"));
    }
    let certificate = ReplayCertificate::issue(
        &system,
        ReplayQuery::Bidirectional {
            source: Term::constant("a"),
            goal: Term::constant("a"),
        },
        ReplayDerivation::Bidirectional(BidirectionalDerivation {
            forward_steps,
            backward_steps: Vec::new(),
            meeting_forward: Term::constant("a"),
            meeting_backward: Term::constant("a"),
            unifier,
        }),
        tight,
        GuidanceMode::CompleteWithinLimits,
        ResourceObservation {
            states: 4,
            transitions: 4,
            retained_bytes: 256,
            operations: 8,
        },
    );
    let (allocations, _peak, outcome) = measure(|| RecallTrace::validate(&system, &certificate));
    let error = outcome.expect_err("a half deeper than the embedded ceiling must not admit");
    assert!(
        matches!(
            error,
            HolographicError::LimitExceeded {
                resource: "replay forward steps",
                limit: 2,
                ..
            }
        ),
        "the typed rejection is the forward step ceiling, got {error}"
    );
    assert!(
        allocations < 2_000,
        "the cheap length ceiling is decided before 100,000 range walks: {allocations} \
         allocations"
    );
}
