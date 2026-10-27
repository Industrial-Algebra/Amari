//! Admission is bounded: an illegal input is rejected from its *shape*,
//! before the encoder allocates anything for it (0.25 Cohort 8, Task 40;
//! PR #353 round 2 P2).
//!
//! The metered walk charges one operation per visited node, but a charge
//! that lands after a million-frame frontier is pushed is not a bound. These
//! tests measure the allocations admission performs, so "the frontier never
//! exceeds the remaining budget" is evidence rather than a claim.
//!
//! A global allocation counter is the only direct way to assert that, and a
//! process-wide counter can be measured only in isolation — hence a test
//! binary of its own, with every measurement serialized behind a lock so no
//! other test can allocate inside the window. The fixtures themselves are
//! built *outside* the window: the caller's own term (a million children, a
//! 16 MB name) is its memory, not admission's.
//!
//! The thresholds are deliberately generous (256 KiB against 16-32 MiB for
//! the unfixed paths): this asserts an order-of-magnitude property, not an
//! exact allocation count.

#![cfg(feature = "neural")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};

use amari_rewrite::inverse::BackwardProvenance;
use amari_rewrite::neural::{CandidateInput, FeatureEncoder, NeuralError, SearchContext};
use amari_rewrite::relation::{ConstraintSet, RelationLimits, RuleId, Sha256Digest};
use amari_rewrite::trs::{Rule, Term};
use amari_rewrite::Path;

/// Bytes requested from the global allocator, cumulative.
static ALLOCATED: AtomicUsize = AtomicUsize::new(0);

/// Serializes measurements: one probe at a time, no other test allocating.
static MEASURE: Mutex<()> = Mutex::new(());

/// The system allocator with an allocation counter in front of it.
struct Counting;

// SAFETY: every method forwards to `System` with the same arguments, after
// accounting the requested size in an atomic. No allocation is reimplemented,
// reordered, or inspected.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATED.fetch_add(layout.size(), Ordering::Relaxed);
        // SAFETY: forwarding the caller's own layout to the system allocator.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: forwarding the caller's own pointer and layout.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCATED.fetch_add(layout.size(), Ordering::Relaxed);
        // SAFETY: forwarding the caller's own layout to the system allocator.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATED.fetch_add(new_size.saturating_sub(layout.size()), Ordering::Relaxed);
        // SAFETY: forwarding the caller's own pointer, layout, and new size.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// Bytes the global allocator handed out while `body` ran.
fn allocated_during<T>(body: impl FnOnce() -> T) -> (T, usize) {
    let _guard: MutexGuard<'_, ()> = MEASURE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let before = ALLOCATED.load(Ordering::Relaxed);
    let value = body();
    let after = ALLOCATED.load(Ordering::Relaxed);
    (value, after.saturating_sub(before))
}

/// A one-off provenance: the fixtures are about admission, not provenance.
fn provenance() -> BackwardProvenance {
    let rule = Rule::new(Term::sym("f", [Term::var("x")]), Term::var("x")).expect("checked rule");
    let anchor = Term::constant("a");
    BackwardProvenance {
        rule_id: RuleId::from_rule(&rule),
        position: Path::root(),
        scope: 0,
        target_hash: Sha256Digest::canonical_term("amari.relation.term/v1", &anchor),
        predecessor_hash: Sha256Digest::canonical_term("amari.relation.term/v1", &anchor),
    }
}

fn candidate<'a>(
    term: &'a Term,
    constraints: &'a ConstraintSet,
    provenance: &'a BackwardProvenance,
) -> CandidateInput<'a> {
    CandidateInput::new(term, constraints, provenance, SearchContext::default())
}

/// One million children under one root: an arity no one-operation budget can
/// pay for, so no frontier may be built for it.
fn million_child_term() -> Term {
    Term::sym("f", (0..1_000_000).map(|_| Term::constant("a")))
}

/// 999,000 children: inside a one-million-operation budget, so the *node*
/// ceiling is the constraint that fires.
fn arity_heavy_term() -> Term {
    Term::sym("f", (0..999_000).map(|_| Term::constant("a")))
}

const ALLOCATION_CEILING: usize = 256 * 1024;

#[test]
fn a_wide_term_is_rejected_before_its_frontier_is_built() {
    let wide = arity_heavy_term();
    let constraints = ConstraintSet::new();
    let provenance = provenance();
    let limits = RelationLimits::new(1, 64, 4_096, 1_000_000).expect("tightened limits");
    let encoder = FeatureEncoder::new(limits);

    let (outcome, allocated) =
        allocated_during(|| encoder.encode(&candidate(&wide, &constraints, &provenance)));

    assert!(
        matches!(
            outcome,
            Err(NeuralError::LimitExceeded {
                resource: "term nodes",
                value: 999_001,
                limit: 1,
            })
        ),
        "expected the arity-derived node rejection, got {outcome:?}"
    );
    assert!(
        allocated < ALLOCATION_CEILING,
        "admission allocated {allocated} bytes before rejecting a 999,000-child term"
    );
}

#[test]
fn a_million_child_term_is_rejected_before_the_operation_budget_is_spent() {
    let wide = million_child_term();
    let constraints = ConstraintSet::new();
    let provenance = provenance();
    let limits = RelationLimits::new(4_096, 64, 4_096, 1).expect("tightened limits");
    let encoder = FeatureEncoder::new(limits);

    let (outcome, allocated) =
        allocated_during(|| encoder.encode(&candidate(&wide, &constraints, &provenance)));

    assert!(
        matches!(
            outcome,
            Err(NeuralError::BudgetExhausted {
                resource: "operations",
                limit: 1,
            })
        ),
        "expected the operation-budget rejection, got {outcome:?}"
    );
    assert!(
        allocated < ALLOCATION_CEILING,
        "admission allocated {allocated} bytes before exhausting a one-operation budget"
    );
}

#[test]
fn a_sixteen_megabyte_identifier_is_rejected_before_it_is_copied() {
    let term = Term::constant("x".repeat(16_000_000));
    let constraints = ConstraintSet::new();
    let provenance = provenance();
    let encoder = FeatureEncoder::default();

    let (outcome, allocated) =
        allocated_during(|| encoder.encode(&candidate(&term, &constraints, &provenance)));

    assert!(
        matches!(
            outcome,
            Err(NeuralError::LimitExceeded {
                resource: "identifier bytes",
                value: 16_000_000,
                limit: FeatureEncoder::MAX_IDENTIFIER_BYTES,
            })
        ),
        "expected the identifier-length rejection, got {outcome:?}"
    );
    assert!(
        allocated < ALLOCATION_CEILING,
        "admission allocated {allocated} bytes before rejecting a 16 MB name"
    );
}
