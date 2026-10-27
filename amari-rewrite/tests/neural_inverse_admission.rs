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
//! 16 MB name) is its memory, not admission's. The whole test body holds the
//! lock, fixtures included — a neighbouring test constructing its own 16 MB
//! fixture mid-window would otherwise be charged to this probe.
//!
//! The thresholds are deliberately generous (256 KiB against 16-32 MiB for
//! the unfixed paths): this asserts an order-of-magnitude property, not an
//! exact allocation count.

#![cfg(feature = "neural")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};

use amari_rewrite::inverse::BackwardProvenance;
use amari_rewrite::neural::{
    CandidateInput, FeatureEncoder, MlpRanker, NeuralError, RankerConfig, RankerWeights,
    SearchContext,
};
use amari_rewrite::relation::{
    ConstraintSet, RelationLimits, RuleId, Sha256Digest, TermConstraint,
};
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

/// Take the measurement lock for the whole test body.
fn exclusive() -> MutexGuard<'static, ()> {
    MEASURE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Bytes the global allocator handed out while `body` ran. The caller holds
/// [`exclusive`], so nothing else in the process can allocate inside the
/// window.
fn allocated_during<T>(_guard: &MutexGuard<'static, ()>, body: impl FnOnce() -> T) -> (T, usize) {
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

/// Diagnostics and validation must not copy an unbounded caller input, so
/// their ceiling is far tighter than the admission ceiling.
const DIAGNOSTIC_ALLOCATION_CEILING: usize = 8 * 1024;

#[test]
fn a_wide_term_is_rejected_before_its_frontier_is_built() {
    let guard = exclusive();
    let wide = arity_heavy_term();
    let constraints = ConstraintSet::new();
    let provenance = provenance();
    let limits = RelationLimits::new(1, 64, 4_096, 1_000_000).expect("tightened limits");
    let encoder = FeatureEncoder::new(limits);

    let (outcome, allocated) = allocated_during(&guard, || {
        encoder.encode(&candidate(&wide, &constraints, &provenance))
    });

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
    let guard = exclusive();
    let wide = million_child_term();
    let constraints = ConstraintSet::new();
    let provenance = provenance();
    let limits = RelationLimits::new(4_096, 64, 4_096, 1).expect("tightened limits");
    let encoder = FeatureEncoder::new(limits);

    let (outcome, allocated) = allocated_during(&guard, || {
        encoder.encode(&candidate(&wide, &constraints, &provenance))
    });

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
    let guard = exclusive();
    let term = Term::constant("x".repeat(16_000_000));
    let constraints = ConstraintSet::new();
    let provenance = provenance();
    let encoder = FeatureEncoder::default();

    let (outcome, allocated) = allocated_during(&guard, || {
        encoder.encode(&candidate(&term, &constraints, &provenance))
    });

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

#[test]
fn a_sixteen_megabyte_constraint_side_name_is_rejected_before_the_digest_runs() {
    let guard = exclusive();
    // PR #353 round 3 P2a: ranking used to accept this and allocate 192 MB
    // in the canonical digest, because the side walk never checked symbol
    // names.
    let term = Term::constant("a");
    let mut constraints = ConstraintSet::new();
    constraints.insert(TermConstraint::Equal(
        Term::constant("z".repeat(16_000_000)),
        Term::constant("b"),
    ));
    let provenance = provenance();
    let ranker = constant_ranker(&[2]);

    let (outcome, allocated) = allocated_during(&guard, || {
        ranker.rank(
            &FeatureEncoder::default(),
            &[candidate(&term, &constraints, &provenance)],
        )
    });

    assert!(
        matches!(
            outcome,
            Err(NeuralError::LimitExceeded {
                resource: "identifier bytes",
                ..
            })
        ),
        "expected the identifier-length rejection, got {outcome:?}"
    );
    assert!(
        allocated < ALLOCATION_CEILING,
        "ranking allocated {allocated} bytes before rejecting a 16 MB side name"
    );
}

#[test]
fn an_unknown_sixteen_megabyte_tensor_name_bounds_its_diagnostic() {
    let guard = exclusive();
    // PR #353 round 3 P2b: quoting the caller's name verbatim allocated
    // 48 MB to build a rejection message.
    let config = spec(&[2]);
    let name = "q".repeat(16_000_000);
    let (outcome, allocated) = allocated_during(&guard, || {
        RankerWeights::from_tensors(&config, &[(&name, &[])])
    });
    match outcome {
        Err(NeuralError::InvalidWeights { message }) => assert!(
            message.len() < 512,
            "the diagnostic must stay bounded, got {} bytes",
            message.len()
        ),
        other => panic!("expected a typed unknown-name rejection, got {other:?}"),
    }
    assert!(
        allocated < DIAGNOSTIC_ALLOCATION_CEILING,
        "the rejection allocated {allocated} bytes for a diagnostic"
    );
}

#[test]
fn a_million_duplicate_tensor_names_are_rejected_without_collecting_them() {
    let guard = exclusive();
    // PR #353 round 3 P2b: uniqueness validation collected every occurrence
    // before reporting, allocating 33 MB for a million duplicates.
    let config = spec(&[2]);
    let duplicates = vec![("hidden.0.weight", &[] as &[f32]); 1_000_000];
    let (outcome, allocated) =
        allocated_during(&guard, || RankerWeights::from_tensors(&config, &duplicates));
    assert!(
        matches!(outcome, Err(NeuralError::InvalidWeights { .. })),
        "expected a typed duplicate rejection, got {outcome:?}"
    );
    assert!(
        allocated < DIAGNOSTIC_ALLOCATION_CEILING,
        "the duplicate rejection allocated {allocated} bytes"
    );
}

#[test]
fn a_hundred_thousand_layer_spec_is_rejected_before_tensor_metadata() {
    let guard = exclusive();
    // PR #353 round 3 P2c: the spec validation derived the full expected
    // tensor list (two Strings per layer) before checking the layer count.
    let config = spec(&[2]);
    let mut spec = config.spec().clone();
    spec.hidden = vec![spec.hidden[0].clone(); 100_000];

    let (outcome, allocated) = allocated_during(&guard, || RankerConfig::from_spec(spec));
    match outcome {
        Err(NeuralError::InvalidConfig { message }) => assert!(
            message.contains("hidden layers"),
            "unexpected message: {message}"
        ),
        other => panic!("expected a typed layer-count rejection, got {other:?}"),
    }
    assert!(
        allocated < DIAGNOSTIC_ALLOCATION_CEILING,
        "the rejection allocated {allocated} bytes before checking the layer count"
    );
}

/// A validated ranker spec with the given hidden widths.
fn spec(hidden: &[u32]) -> RankerConfig {
    let hidden = hidden
        .iter()
        .map(|width| format!("{{\"width\":{width}}}"))
        .collect::<Vec<_>>()
        .join(",");
    let json = format!(
        "{{\"model_type\":\"ia-mlp\",\"schema_version\":1,\"input_width\":64,\
         \"hidden\":[{hidden}],\"head\":{{\"width\":1}},\
         \"hidden_activation\":\"relu\"}}"
    );
    RankerConfig::from_spec_json(&json).expect("valid spec")
}

/// A ranker with zero weights: every candidate scores 0.0.
fn constant_ranker(hidden: &[u32]) -> MlpRanker {
    let config = spec(hidden);
    let tensors: Vec<(String, Vec<f32>)> = config
        .expected_tensors()
        .into_iter()
        .map(|(name, shape)| {
            let elements: usize = shape.iter().product();
            (name, vec![0.0f32; elements])
        })
        .collect();
    let borrowed: Vec<(&str, &[f32])> = tensors
        .iter()
        .map(|(name, values)| (name.as_str(), values.as_slice()))
        .collect();
    let weights =
        RankerWeights::from_tensors(&config, &borrowed).expect("zero weights are admissible");
    MlpRanker::new(config, weights).expect("matching weights")
}
