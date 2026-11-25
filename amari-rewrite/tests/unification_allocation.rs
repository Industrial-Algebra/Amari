// SPDX-License-Identifier: MIT OR Apache-2.0

//! Allocation-sensitive unification admission evidence (Task 47, PR #384
//! round 5, F24).
//!
//! `unify_with` is a public entry point over caller-supplied, unbounded terms
//! and a caller-supplied substitution. The pre-remediation implementation
//! started by cloning BOTH complete input terms into its worklist
//! (`vec![(left.clone(), right.clone())]`) and only then charged anything, so a
//! deep term recursed through `Term::clone` with `max_operations = 1` and a wide
//! term allocated a clone of every node — the typed limit arrived after the work
//! it was supposed to bound.
//!
//! This binary instruments the global allocator and pins the corrected order:
//! borrowed shape admission (iterative, metered) for both inputs and every
//! pre-existing substitution range, and one charged enqueue per equation, all
//! BEFORE any clone. The tall/broad terms here are one node over a tightened
//! budget on purpose; the positive control shows a term at a ceiling still
//! unifies.
//!
//! Every test holds [`exclusive`] for its whole body to isolate the process-wide
//! live-byte counters. Allocation counts are thread-local to each measured
//! closure, so test-harness activity on another thread cannot inflate them.

#![cfg(feature = "std")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

use amari_rewrite::analysis::{unify, unify_with};
use amari_rewrite::relation::{RelationLimits, RelationResources};
use amari_rewrite::trs::{Substitution, Term};
use amari_rewrite::RewriteError;

struct Counting;

static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static PEAK_BYTES: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static COUNT_ON_THIS_THREAD: Cell<bool> = const { Cell::new(false) };
    static THREAD_ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

impl Counting {
    fn record(size: usize) {
        if COUNT_ON_THIS_THREAD.with(Cell::get) {
            THREAD_ALLOCATIONS.with(|count| count.set(count.get() + 1));
        }
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

/// Serializes the allocation-sensitive tests: the counters are process-wide.
static MEASURE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn exclusive() -> std::sync::MutexGuard<'static, ()> {
    MEASURE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Run `body`, returning its value with the allocation count and peak live-byte
/// growth observed while it ran. The caller holds [`exclusive`].
fn measure<T>(body: impl FnOnce() -> T) -> (usize, usize, T) {
    let start = LIVE_BYTES.load(Ordering::SeqCst);
    THREAD_ALLOCATIONS.with(|count| count.set(0));
    COUNT_ON_THIS_THREAD.with(|active| active.set(true));
    PEAK_BYTES.store(start, Ordering::SeqCst);
    let value = body();
    COUNT_ON_THIS_THREAD.with(|active| active.set(false));
    let allocations = THREAD_ALLOCATIONS.with(Cell::get);
    let peak = PEAK_BYTES.load(Ordering::SeqCst).saturating_sub(start);
    (allocations, peak, value)
}

/// A chain `f(f(... f(a)))` of `depth` symbols, built without recursion.
fn deep(depth: usize) -> Term {
    let mut term = Term::constant("a");
    for _ in 0..depth {
        term = Term::sym("f", vec![term]);
    }
    term
}

/// A single symbol with `children` arguments, all the constant `a`.
fn wide(children: usize) -> Term {
    Term::sym("f", (0..children).map(|_| Term::constant("a")))
}

/// F24 (P1): a deep borrowed input is rejected BEFORE any recursive clone.
///
/// The term is 5,000 symbols deep, `max_operations = 1`, and the depth ceiling
/// is the default 64. The admission walk is iterative and metered, so the
/// operation ceiling answers after two nodes; `Term::clone` (recursive, depth
/// proportional to the term) is never reached.
#[test]
fn a_deep_borrowed_input_is_rejected_before_any_clone() {
    let _exclusive = exclusive();
    let input = deep(5_000);
    let limits = RelationLimits::new(4_096, 64, 4_096, 1).expect("valid tightened limits");
    let (allocations, peak, outcome) = measure(|| {
        let mut substitution = Substitution::new();
        let mut budget = RelationResources::new(&limits);
        unify_with(&mut substitution, &input, &Term::constant("b"), &mut budget)
    });
    let error = outcome.expect_err("a 5,000-deep input cannot fit a one-operation budget");
    assert!(
        matches!(
            error,
            RewriteError::RelationLimitExceeded {
                resource: "operations",
                ..
            }
        ),
        "the typed rejection is the admission meter, got {error}"
    );
    assert!(
        allocations < 200,
        "the deep input must not be cloned before admission: {allocations} allocations, \
         peak {peak} bytes"
    );
    assert!(
        peak < 64 * 1024,
        "no clone proportional to the rejected input: peak {peak} bytes, {allocations} \
         allocations"
    );
    drop(input);
}

/// F24 (P1): a wide borrowed input is rejected BEFORE its nodes are cloned.
#[test]
fn a_wide_borrowed_input_is_rejected_before_any_clone() {
    let _exclusive = exclusive();
    let input = wide(4_000);
    let limits = RelationLimits::new(4_096, 64, 4_096, 1).expect("valid tightened limits");
    let (allocations, peak, outcome) = measure(|| {
        let mut substitution = Substitution::new();
        let mut budget = RelationResources::new(&limits);
        unify_with(&mut substitution, &input, &Term::constant("b"), &mut budget)
    });
    let error = outcome.expect_err("a 4,001-node input cannot fit a one-operation budget");
    assert!(
        matches!(
            error,
            RewriteError::RelationLimitExceeded {
                resource: "operations",
                ..
            }
        ),
        "the typed rejection is the admission meter, got {error}"
    );
    assert!(
        allocations < 200,
        "the wide input must not be cloned before admission: {allocations} allocations, \
         peak {peak} bytes"
    );
    assert!(
        peak < 16 * 1024,
        "the last operation must not queue the full wide root: peak {peak} bytes"
    );
    drop(input);
}

/// F24 (P1): every pre-existing substitution range is admitted under the same
/// meter, before the equation is examined.
///
/// The range is 3,001 nodes and the bound variable never appears in the
/// equation, so the pre-remediation unifier answered "unified" without ever
/// looking at it — carrying an unbounded, unmeasured range through the whole
/// call. Admission walks every existing range first, so the same tiny budget is
/// a typed operations rejection.
#[test]
fn pre_existing_ranges_are_admitted_under_the_shared_meter() {
    let _exclusive = exclusive();
    let mut substitution = Substitution::new().with("Z", wide(3_000));
    let limits = RelationLimits::new(4_096, 64, 4_096, 1).expect("valid tightened limits");
    let (allocations, peak, outcome) = measure(|| {
        let mut budget = RelationResources::new(&limits);
        unify_with(
            &mut substitution,
            &Term::var("X"),
            &Term::var("Y"),
            &mut budget,
        )
    });
    let error = outcome.expect_err("an unmeasured borrowed range must not be admitted");
    assert!(
        matches!(
            error,
            RewriteError::RelationLimitExceeded {
                resource: "operations",
                ..
            }
        ),
        "the typed rejection is the admission meter, got {error}"
    );
    assert!(
        allocations < 200,
        "the range must not be cloned before admission: {allocations} allocations, \
         peak {peak} bytes"
    );
}

/// F24 positive control: a term that fits the ceilings still unifies.
///
/// The chain is exactly the depth ceiling (64 symbols), so the admission walk
/// admits it and the ordinary worklist resolves it. This pins that admission is
/// a bound, not a rejection of deep input as such.
#[test]
fn a_term_at_the_depth_ceiling_still_unifies() {
    let _exclusive = exclusive();
    let deep_ceiling = RelationLimits::MAX_TERM_DEPTH;
    // `deep(n)` builds `n` symbols over one leaf: `n + 1` nodes, depth `n + 1`.
    let left = deep(deep_ceiling - 1);
    let right = left.clone();
    let (_, _, outcome) = measure(|| {
        unify(
            &left,
            &right,
            &mut RelationResources::new(&RelationLimits::default()),
        )
    });
    let substitution = outcome.expect("an input at the exact depth ceiling is admitted");
    assert!(
        substitution.is_empty(),
        "an identical deep pair needs no binding"
    );
}

/// F31 (P1): after both inputs are admitted, an exhausted enqueue budget
/// rejects before any child equation is cloned.
#[test]
fn child_equations_are_not_cloned_before_their_enqueue_budget() {
    let _exclusive = exclusive();
    let left = wide(4_000);
    let right = wide(4_000);
    // Admission costs 8,002 operations; root enqueue/pop and two root
    // resolves consume the remaining four. No child enqueue is affordable.
    let limits = RelationLimits::new(4_096, 64, 4_096, 8_006).expect("valid tightened limits");
    let (allocations, peak, outcome) =
        measure(|| unify(&left, &right, &mut RelationResources::new(&limits)));
    let error = outcome.expect_err("no child equation fits the remaining budget");
    assert!(
        matches!(
            error,
            RewriteError::RelationLimitExceeded {
                resource: "operations",
                ..
            }
        ),
        "the child enqueue meter must answer, got {error}"
    );
    assert!(
        allocations < 10_000,
        "only the admitted root pair may be cloned before this rejection: \
         {allocations} allocations, peak {peak} bytes"
    );
}
