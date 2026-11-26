// SPDX-License-Identifier: MIT OR Apache-2.0
//! Allocation evidence for hybrid recall-lane admission (PR #386 round 1).

#![cfg(all(feature = "std", feature = "holographic-guidance"))]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use amari_rewrite::holographic::{
    HolographicEncoder, HolographicError, RecallConfig, RecallTrace, MAX_TRACE_TERMS,
};
use amari_rewrite::inverse::hybrid::{HybridConfigError, RecallLane};
use amari_rewrite::inverse::{
    BackwardExplorer, BackwardSearchOutcome, GuidanceMode, InverseSearchConfig, ReplayCertificate,
    ReplayDerivation, ReplayQuery, ResourceObservation, SearchMode,
};
use amari_rewrite::relation::RelationLimits;
use amari_rewrite::trs::{Rule, Term, TermSystem};

struct Counting;

thread_local! {
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() && ACTIVE.with(Cell::get) {
            ALLOCATIONS.with(|count| count.set(count.get() + 1));
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn measure<T>(body: impl FnOnce() -> T) -> (usize, T) {
    ALLOCATIONS.with(|count| count.set(0));
    ACTIVE.with(|active| active.set(true));
    let value = body();
    ACTIVE.with(|active| active.set(false));
    (ALLOCATIONS.with(Cell::get), value)
}

fn parse(text: &str) -> Term {
    if text == "f(c)" {
        Term::sym("f", [Term::constant("c")])
    } else {
        Term::constant(text)
    }
}

fn system() -> TermSystem {
    TermSystem::new(vec![
        Rule::new(parse("b"), parse("x")).unwrap(),
        Rule::new(parse("f(c)"), parse("x")).unwrap(),
        Rule::new(parse("d"), parse("b")).unwrap(),
        Rule::new(parse("d"), parse("f(c)")).unwrap(),
    ])
}

fn trace(system: &TermSystem, goal: &Term) -> RecallTrace {
    let outcome = BackwardExplorer::new(system, InverseSearchConfig::default())
        .search(&parse("x"), goal, SearchMode::BreadthFirst)
        .expect("fixture search runs");
    let BackwardSearchOutcome::Witness(derivation) = outcome else {
        panic!("fixture must witness");
    };
    let certificate = ReplayCertificate::issue(
        system,
        ReplayQuery::Backward {
            target: parse("x"),
            goal: goal.clone(),
        },
        ReplayDerivation::Backward(derivation),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        ResourceObservation {
            states: 3,
            transitions: 2,
            retained_bytes: 256,
            operations: 32,
        },
    );
    RecallTrace::validate(system, &certificate).expect("replay-valid trace")
}

#[test]
fn recall_trace_count_is_rejected_before_projection_or_encoding() {
    let system = system();
    let query = trace(&system, &parse("f(c)"));
    let traces = vec![trace(&system, &parse("b")), trace(&system, &parse("f(c)"))];
    let config = RecallConfig::new(1, MAX_TRACE_TERMS, 1).expect("bounded config");
    let encoder = HolographicEncoder::<64>::new(7, RelationLimits::default()).expect("encoder");
    let (allocations, outcome) =
        measure(|| RecallLane::new(&system, encoder, config, query, traces));
    let error = outcome.expect_err("two traces exceed the one-trace ceiling");
    assert!(
        matches!(
            error,
            HybridConfigError::RecallAdmission(HolographicError::LimitExceeded {
                resource: "recall traces",
                value: 2,
                limit: 1,
            })
        ),
        "the typed count rejection is preserved: {error}"
    );
    assert!(
        allocations < 20,
        "count admission precedes projection and encoding: {allocations} allocations"
    );
}
