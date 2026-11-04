//! Admission precedes allocation for both training entry points
//! (0.25 Cohort 8, Task 42; PR #367 round 1 P2a).
//!
//! [`train_from_seed`] materializes the seeded parameter buffers *and then*
//! hands them to [`train`], which is where the corpus/step/architecture
//! ceilings are checked. A caller that asks for an oversized corpus therefore
//! pays for a full initialization it immediately discards: the rejection is
//! typed either way, but the bound is not a bound if the rejected work already
//! happened.
//!
//! Bytes and allocation *counts* are measured with a global counter, which is
//! only meaningful in a process where nothing else allocates inside the
//! window. That is why this is a test binary of its own (the same discipline
//! `neural_inverse_admission.rs` uses for the encoder): every measurement
//! holds [`exclusive`], and the fixtures — including the 32 MiB oversized
//! corpus — are built *outside* the window, because the caller's own data is
//! its memory, not admission's.
//!
//! A positive control asserts that a *successful* run does allocate, so a zero
//! reading cannot come from a counter that was never wired up.

#![cfg(feature = "neural")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};

use amari_rewrite::neural::{
    seeded_weights, train, train_from_seed, ExampleOrder, FeatureVector, RankerConfig, TrainConfig,
    TrainingError, TrainingExample, CANDIDATE_FEATURE_WIDTH,
};

/// Allocation calls seen by the counter, cumulative.
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
/// Bytes requested from the global allocator, cumulative.
static ALLOCATED: AtomicUsize = AtomicUsize::new(0);

/// Serializes measurements: one probe at a time, no other test allocating.
static MEASURE: Mutex<()> = Mutex::new(());

/// The system allocator with an allocation counter in front of it.
struct Counting;

// SAFETY: every method forwards to `System` with the caller's own arguments,
// after accounting in two relaxed atomics. No allocation is reimplemented,
// reordered, or inspected.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED.fetch_add(layout.size(), Ordering::Relaxed);
        // SAFETY: forwarding the caller's own layout to the system allocator.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: forwarding the caller's own pointer and layout.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED.fetch_add(layout.size(), Ordering::Relaxed);
        // SAFETY: forwarding the caller's own layout to the system allocator.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        ALLOCATED.fetch_add(new_size.saturating_sub(layout.size()), Ordering::Relaxed);
        // SAFETY: forwarding the caller's own pointer, layout, and new size.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// Take the measurement lock for the whole test body.
fn exclusive() -> MutexGuard<'static, ()> {
    MEASURE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// `(allocation calls, bytes)` the global allocator handed out while `body`
/// ran. The caller holds [`exclusive`], so nothing else in the process can
/// allocate inside the window.
fn measured<T>(_guard: &MutexGuard<'static, ()>, body: impl FnOnce() -> T) -> (T, usize, usize) {
    let calls_before = ALLOCATIONS.load(Ordering::Relaxed);
    let bytes_before = ALLOCATED.load(Ordering::Relaxed);
    let value = body();
    let calls = ALLOCATIONS
        .load(Ordering::Relaxed)
        .saturating_sub(calls_before);
    let bytes = ALLOCATED
        .load(Ordering::Relaxed)
        .saturating_sub(bytes_before);
    (value, calls, bytes)
}

// ── fixtures (built outside every measurement window) ───────────────

fn config(hidden: &[u32]) -> RankerConfig {
    let hidden = hidden
        .iter()
        .map(|width| format!("{{\"width\":{width}}}"))
        .collect::<Vec<_>>()
        .join(",");
    let json = format!(
        "{{\"model_type\":\"ia-mlp\",\"schema_version\":1,\"input_width\":{},\
         \"hidden\":[{hidden}],\"head\":{{\"width\":1}},\
         \"hidden_activation\":\"relu\"}}",
        CANDIDATE_FEATURE_WIDTH
    );
    RankerConfig::from_spec_json(&json).expect("valid ranker spec")
}

fn example() -> TrainingExample {
    let mut chosen = [0.0f32; CANDIDATE_FEATURE_WIDTH];
    let mut unchosen = [0.0f32; CANDIDATE_FEATURE_WIDTH];
    chosen[0] = 1.0;
    unchosen[0] = 0.25;
    TrainingExample::new(
        FeatureVector::new(chosen).expect("valid lanes"),
        FeatureVector::new(unchosen).expect("valid lanes"),
    )
}

fn settings(max_steps: usize) -> TrainConfig {
    TrainConfig::new(
        0.01,
        0.9,
        0.999,
        1e-8,
        0.0,
        1.0,
        max_steps,
        7,
        ExampleOrder::Canonical,
    )
    .expect("valid training config")
}

// ── the contract ────────────────────────────────────────────────────

#[test]
fn a_rejected_corpus_allocates_nothing_on_either_entry_point() {
    let guard = exclusive();
    let config = config(&[4]);
    let oversized = vec![example(); TrainConfig::MAX_TRAIN_EXAMPLES + 1];
    let init = seeded_weights(&config, 7).expect("seeded weights");
    let settings = settings(4);

    // Direct `train` is the reference: it admits from scalars and never sizes
    // a buffer it will not use.
    let (direct, calls, bytes) = measured(&guard, || train(&config, &init, &settings, &oversized));
    assert!(matches!(
        direct,
        Err(TrainingError::LimitExceeded {
            resource: "training examples",
            ..
        })
    ));
    assert_eq!(
        (calls, bytes),
        (0, 0),
        "rejecting an oversized corpus must not allocate (train)"
    );

    // `train_from_seed` must reject on the same scalars *before* it
    // materializes the seeded initialization (PR #367 round 1 P2a).
    let (from_seed, calls, bytes) =
        measured(&guard, || train_from_seed(&config, &settings, &oversized));
    assert!(matches!(
        from_seed,
        Err(TrainingError::LimitExceeded {
            resource: "training examples",
            ..
        })
    ));
    assert_eq!(
        (calls, bytes),
        (0, 0),
        "rejecting an oversized corpus must not allocate (train_from_seed)"
    );
}

#[test]
fn an_empty_corpus_allocates_nothing_on_either_entry_point() {
    let guard = exclusive();
    let config = config(&[4]);
    let init = seeded_weights(&config, 7).expect("seeded weights");
    let settings = settings(4);

    let (direct, calls, _) = measured(&guard, || train(&config, &init, &settings, &[]));
    assert_eq!(direct, Err(TrainingError::EmptyDataset));
    assert_eq!(calls, 0, "an empty corpus must be refused from its length");

    let (from_seed, calls, _) = measured(&guard, || train_from_seed(&config, &settings, &[]));
    assert_eq!(from_seed, Err(TrainingError::EmptyDataset));
    assert_eq!(calls, 0, "an empty corpus must be refused from its length");
}

#[test]
fn a_successful_run_does_allocate_so_the_counter_is_wired() {
    let guard = exclusive();
    let config = config(&[4]);
    let corpus = vec![example(); 3];
    let settings = settings(32);

    // The positive control: a training run sizes parameter, gradient, cache,
    // optimizer, and order buffers, so a zero reading above cannot be an
    // artifact of a counter that never fires.
    let (outcome, calls, bytes) = measured(&guard, || train_from_seed(&config, &settings, &corpus));
    outcome.expect("the fixture trains");
    assert!(
        calls > 0 && bytes > 0,
        "a training run must allocate (calls {calls}, bytes {bytes})"
    );
}
