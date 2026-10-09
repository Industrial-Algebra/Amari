// SPDX-License-Identifier: MIT OR Apache-2.0

//! Exact unbounded-saturation preimage integration tests (0.25 Cohort 5,
//! Task 26).
//!
//! These tests pin the target-specialized right-ground backward epsilon
//! saturation against the concrete expected sets of the reviewed design
//! (§5). Membership is checked with the automaton's own `accepts`, and the
//! negative claims are explicit mathematical expectations (not depth
//! cutoffs). Deep growing systems are built iteratively.

use amari_rewrite::language::{
    saturation_preimage, CertificateAuthority, PreimageCertificate, PreimageConstruction,
    PreimageOperation, RankedSymbol, TreeAutomaton, TreeAutomatonLimits, TreeState, TreeTransition,
};
use amari_rewrite::relation::RelationLimits;
use amari_rewrite::trs::{Rule, Symbol, Term, TermSystem};
use amari_rewrite::RewriteError;

fn a() -> Term {
    Term::constant("a")
}

fn b() -> Term {
    Term::constant("b")
}

/// A partial singleton automaton accepting exactly `b` over the ranked
/// alphabet {a/0, b/0, g/1} (the review probe's `singleton`).
fn singleton_language() -> TreeAutomaton {
    let q = TreeState::new("q");
    TreeAutomaton::new(
        vec![
            RankedSymbol::new(Symbol::new("a"), 0),
            RankedSymbol::new(Symbol::new("b"), 0),
            RankedSymbol::new(Symbol::new("g"), 1),
        ],
        vec![q.clone()],
        vec![TreeTransition::new(Symbol::new("b"), vec![], q.clone())],
        vec![q],
        TreeAutomatonLimits::default(),
    )
    .expect("fixture automaton is valid")
}

fn c() -> Term {
    Term::constant("c")
}

fn d() -> Term {
    Term::constant("d")
}

fn f(left: Term, right: Term) -> Term {
    Term::sym("f", [left, right])
}

/// Unary `f` used by the feedback system.
fn f1(inner: Term) -> Term {
    Term::sym("f", [inner])
}

fn g(inner: Term) -> Term {
    Term::sym("g", [inner])
}

fn x() -> Term {
    Term::var("x")
}

fn y() -> Term {
    Term::var("y")
}

/// Build a (possibly partial, possibly nondeterministic) automaton
/// accepting exactly `terms` over `alphabet`, using a fresh state per node
/// occurrence. When `terms` is empty the automaton accepts nothing.
fn partial_singleton(alphabet: &[(&str, u16)], terms: &[Term]) -> TreeAutomaton {
    let mut transitions: Vec<TreeTransition> = Vec::new();
    let mut finals: Vec<TreeState> = Vec::new();
    let mut counter = 0usize;
    for term in terms {
        let mut states_by_path: std::collections::BTreeMap<Vec<usize>, TreeState> =
            std::collections::BTreeMap::new();
        let mut positions = term.positions();
        positions.sort_by_key(|path| std::cmp::Reverse(path.as_slice().len()));
        for position in &positions {
            let Term::Sym(symbol, arguments) = term.subterm(position).expect("valid position")
            else {
                panic!("partial_singleton requires ground terms");
            };
            counter += 1;
            let state = TreeState::new(format!("s{counter}"));
            let children: Vec<TreeState> = (0..arguments.len())
                .map(|child| {
                    states_by_path
                        .get(position.child(child).as_slice())
                        .cloned()
                        .expect("children precede parents")
                })
                .collect();
            transitions.push(TreeTransition::new(symbol.clone(), children, state.clone()));
            states_by_path.insert(position.as_slice().to_vec(), state);
        }
        finals.push(
            states_by_path
                .get(&Vec::<usize>::new())
                .cloned()
                .expect("the root is evaluated"),
        );
    }
    let mut state_set: std::collections::BTreeSet<TreeState> = std::collections::BTreeSet::new();
    for transition in &transitions {
        state_set.insert(transition.parent().clone());
        state_set.extend(transition.children().iter().cloned());
    }
    finals.sort();
    finals.dedup();
    TreeAutomaton::new(
        alphabet
            .iter()
            .map(|(name, arity)| RankedSymbol::new(Symbol::new(*name), *arity))
            .collect(),
        state_set.into_iter().collect(),
        transitions,
        finals,
        TreeAutomatonLimits::default(),
    )
    .expect("fixture automaton is valid")
}

/// The completed deterministic presentation of `automaton`.
fn complete(automaton: &TreeAutomaton) -> TreeAutomaton {
    automaton
        .determinize(&TreeAutomatonLimits::default())
        .expect("fixture determinizes")
        .completed()
        .expect("fixture completes")
}

fn accepts(automaton: &TreeAutomaton, term: &Term) -> bool {
    automaton.accepts(term).expect("well-formed ground term")
}

/// `g^n(inner)` built iteratively (no recursion on the nesting depth).
fn g_pow(n: usize, inner: Term) -> Term {
    let mut term = inner;
    for _ in 0..n {
        term = g(term);
    }
    term
}

/// Oracle P2: `{a -> g(a)}`, `L = {g(g(a))}`. The exact saturation is
/// `{a, g(a), g(g(a))}`; `g^3(a)` and beyond are rejected.
#[test]
fn oracle_p2_ground_fixpoint() {
    let system = TermSystem::new(vec![Rule::new(a(), g(a())).unwrap()]);
    let language = partial_singleton(&[("a", 0), ("g", 1)], &[g(g(a()))]);
    let outcome = saturation_preimage(
        &system,
        &language,
        &RelationLimits::default(),
        &TreeAutomatonLimits::default(),
    )
    .expect("ground saturation is approved");

    for accepted in [a(), g(a()), g(g(a()))] {
        assert!(accepts(outcome.automaton(), &accepted), "{accepted:?}");
    }
    for rejected in [g(g(g(a()))), g(g(g(g(a()))))] {
        assert!(!accepts(outcome.automaton(), &rejected), "{rejected:?}");
    }
}

/// S2: `{g(a) -> a}`, `L = {a}`. The exact saturation is `g*(a)`, an
/// unbounded set recognized by a fixed finite automaton. Deep terms are
/// built iteratively.
#[test]
fn s2_infinite_ancestors_are_recognized() {
    let system = TermSystem::new(vec![Rule::new(g(a()), a()).unwrap()]);
    let language = partial_singleton(&[("a", 0), ("b", 0), ("g", 1)], &[a()]);
    let outcome = saturation_preimage(
        &system,
        &language,
        &RelationLimits::default(),
        &TreeAutomatonLimits::default(),
    )
    .expect("ground saturation is approved");

    for depth in [0usize, 1, 2, 10] {
        let term = g_pow(depth, a());
        assert!(accepts(outcome.automaton(), &term), "g^{depth}(a)");
    }
    for rejected in [b(), g(b()), g(g(b()))] {
        assert!(!accepts(outcome.automaton(), &rejected), "{rejected:?}");
    }
}

/// Sink-conflation counterexample: `{g(a) -> b}`, `L = {b}`. The exact
/// ancestors are `{b, g(a)}`; completion's sink must not conflate shapes.
/// Both the partial input and its completed deterministic presentation
/// reject the same negatives.
#[test]
fn sink_conflation_is_rejected() {
    let system = TermSystem::new(vec![Rule::new(g(a()), b()).unwrap()]);
    let alphabet = [("a", 0), ("b", 0), ("g", 1)];
    let partial = partial_singleton(&alphabet, &[b()]);
    let completed = complete(&partial);

    for (label, language) in [("partial", &partial), ("completed", &completed)] {
        let outcome = saturation_preimage(
            &system,
            language,
            &RelationLimits::default(),
            &TreeAutomatonLimits::default(),
        )
        .unwrap_or_else(|error| panic!("{label}: saturation failed: {error:?}"));

        for accepted in [b(), g(a())] {
            assert!(
                accepts(outcome.automaton(), &accepted),
                "{label}: expected {accepted:?} to be accepted"
            );
        }
        for rejected in [a(), g(b()), g(g(a())), g(g(b()))] {
            assert!(
                !accepts(outcome.automaton(), &rejected),
                "{label}: expected {rejected:?} to be rejected"
            );
        }
    }
}

/// Feedback: `{g(a) -> b, f(b) -> c}`, `L = {c}`. The rewritten material
/// `g(a)` must re-enter a pattern (`p_b`) so that `f(g(a))` is recognized.
#[test]
fn feedback_chain_accepts_semantic_redexes() {
    let system = TermSystem::new(vec![
        Rule::new(g(a()), b()).unwrap(),
        Rule::new(f1(b()), c()).unwrap(),
    ]);
    let language = partial_singleton(&[("a", 0), ("b", 0), ("c", 0), ("f", 1), ("g", 1)], &[c()]);
    let outcome = saturation_preimage(
        &system,
        &language,
        &RelationLimits::default(),
        &TreeAutomatonLimits::default(),
    )
    .expect("ground saturation is approved");

    for accepted in [c(), f1(b()), f1(g(a()))] {
        assert!(accepts(outcome.automaton(), &accepted), "{accepted:?}");
    }
    for rejected in [f1(a()), f1(g(g(a()))), g(c())] {
        assert!(!accepts(outcome.automaton(), &rejected), "{rejected:?}");
    }
}

/// Erasure: `{f(x, y) -> c}` over a partial `{c}` automaton that has no
/// argument runs. The universal `Any` state supplies the erased arguments,
/// so every `f`-rooted ground term is accepted. The extension
/// `{f(x, y) -> c, c -> d}` requires multi-step erasure propagation.
#[test]
fn erasure_uses_universal_state() {
    let system = TermSystem::new(vec![Rule::new(f(x(), y()), c()).unwrap()]);
    let alphabet = [("a", 0), ("b", 0), ("c", 0), ("f", 2), ("g", 1)];
    let language = partial_singleton(&alphabet, &[c()]);
    let outcome = saturation_preimage(
        &system,
        &language,
        &RelationLimits::default(),
        &TreeAutomatonLimits::default(),
    )
    .expect("variable-disjoint saturation is approved");

    for accepted in [c(), f(g(a()), b()), f(f(a(), b()), g(c()))] {
        assert!(accepts(outcome.automaton(), &accepted), "{accepted:?}");
    }
    assert!(!accepts(outcome.automaton(), &g(f(a(), b()))));

    let extension = TermSystem::new(vec![
        Rule::new(f(x(), y()), c()).unwrap(),
        Rule::new(c(), d()).unwrap(),
    ]);
    let extension_language = partial_singleton(
        &[("a", 0), ("b", 0), ("c", 0), ("d", 0), ("f", 2), ("g", 1)],
        &[d()],
    );
    let extended = saturation_preimage(
        &extension,
        &extension_language,
        &RelationLimits::default(),
        &TreeAutomatonLimits::default(),
    )
    .expect("variable-disjoint saturation is approved");
    assert!(accepts(extended.automaton(), &f(a(), b())));
    assert!(accepts(extended.automaton(), &d()));
}

/// Self-loops and cycles: `{a -> a}` preserves `L` (including the empty
/// language), and `{a -> b, b -> a}` makes both constants mutually
/// reachable.
#[test]
fn self_loop_and_mutual_cycle_converge() {
    let system = TermSystem::new(vec![Rule::new(a(), a()).unwrap()]);
    let alphabet = [("a", 0), ("c", 0)];
    let language = partial_singleton(&alphabet, &[a()]);
    let outcome = saturation_preimage(
        &system,
        &language,
        &RelationLimits::default(),
        &TreeAutomatonLimits::default(),
    )
    .expect("ground saturation is approved");
    assert!(accepts(outcome.automaton(), &a()));
    assert!(!accepts(outcome.automaton(), &c()));

    let empty = partial_singleton(&alphabet, &[]);
    let empty_outcome = saturation_preimage(
        &system,
        &empty,
        &RelationLimits::default(),
        &TreeAutomatonLimits::default(),
    )
    .expect("ground saturation is approved");
    assert!(!accepts(empty_outcome.automaton(), &a()));

    let cycle = TermSystem::new(vec![
        Rule::new(a(), b()).unwrap(),
        Rule::new(b(), a()).unwrap(),
    ]);
    let cycle_language = partial_singleton(&[("a", 0), ("b", 0), ("c", 0)], &[a()]);
    let cycle_outcome = saturation_preimage(
        &cycle,
        &cycle_language,
        &RelationLimits::default(),
        &TreeAutomatonLimits::default(),
    )
    .expect("ground saturation is approved");
    assert!(accepts(cycle_outcome.automaton(), &a()));
    assert!(accepts(cycle_outcome.automaton(), &b()));
    assert!(!accepts(cycle_outcome.automaton(), &c()));
}

/// Edge cases: an empty system's saturation is the input language exactly
/// (with a verifiable certificate); a target already in `L` is retained;
/// and a nondeterministic presentation of `L` canonicalizes to the same
/// output as a deterministic one.
#[test]
fn edge_cases() {
    let limits = RelationLimits::default();
    let automaton_limits = TreeAutomatonLimits::default();

    // Empty system: saturation = L, and the certificate verifies against
    // its own result (Task 24 empty-system digest semantics).
    let empty_system = TermSystem::new(vec![]);
    let language = partial_singleton(&[("a", 0), ("b", 0)], &[b()]);
    let outcome = saturation_preimage(&empty_system, &language, &limits, &automaton_limits)
        .expect("empty system saturation is approved");
    assert!(accepts(outcome.automaton(), &b()));
    assert!(!accepts(outcome.automaton(), &a()));
    assert!(outcome
        .certificate()
        .verify(&empty_system, &language, &limits));
    assert!(outcome.certificate().verify_result(outcome.automaton()));

    // Target already in L is retained (reflexive saturation).
    let system = TermSystem::new(vec![Rule::new(a(), b()).unwrap()]);
    let language = partial_singleton(&[("a", 0), ("b", 0)], &[b()]);
    let outcome = saturation_preimage(&system, &language, &limits, &automaton_limits)
        .expect("ground saturation is approved");
    assert!(accepts(outcome.automaton(), &b()));
    assert!(accepts(outcome.automaton(), &a()));

    // Nondeterministic presentation of the same language {b}.
    let q1 = TreeState::new("q1");
    let q2 = TreeState::new("q2");
    let nondeterministic = TreeAutomaton::new(
        vec![
            RankedSymbol::new(Symbol::new("a"), 0),
            RankedSymbol::new(Symbol::new("b"), 0),
        ],
        vec![q1.clone(), q2.clone()],
        vec![
            TreeTransition::new(Symbol::new("b"), vec![], q1.clone()),
            TreeTransition::new(Symbol::new("b"), vec![], q2.clone()),
        ],
        vec![q1],
        automaton_limits,
    )
    .expect("nondeterministic fixture is valid");
    let deterministic = partial_singleton(&[("a", 0), ("b", 0)], &[b()]);
    let from_nd = saturation_preimage(&system, &nondeterministic, &limits, &automaton_limits)
        .expect("ground saturation is approved");
    let from_det = saturation_preimage(&system, &deterministic, &limits, &automaton_limits)
        .expect("ground saturation is approved");
    assert_eq!(
        from_nd.automaton().canonical_bytes(),
        from_det.automaton().canonical_bytes(),
        "nondeterministic and deterministic presentations must canonicalize equally"
    );
}

/// Boundary rejections: a nonlinear left side and a non-ground right side
/// receive a hard `UnsupportedPreimage`; no certificate is produced.
#[test]
fn boundaries_rejected() {
    let limits = RelationLimits::default();
    let automaton_limits = TreeAutomatonLimits::default();
    let alphabet = [("a", 0), ("g", 1), ("f", 2)];

    let nonlinear = TermSystem::new(vec![Rule::new(f(x(), x()), x()).unwrap()]);
    let language = partial_singleton(&alphabet, &[a()]);
    let result = saturation_preimage(&nonlinear, &language, &limits, &automaton_limits);
    assert!(
        matches!(result, Err(RewriteError::UnsupportedPreimage { .. })),
        "nonlinear lhs must be rejected, got {result:?}"
    );

    let shared = TermSystem::new(vec![Rule::new(f(x(), y()), g(x())).unwrap()]);
    let result = saturation_preimage(&shared, &language, &limits, &automaton_limits);
    assert!(
        matches!(result, Err(RewriteError::UnsupportedPreimage { .. })),
        "non-ground rhs must be rejected, got {result:?}"
    );
}

/// Certificate integration: `GttSaturation` under operation `Saturation`,
/// full verification against inputs and result, and the serialize feature's
/// pending-deserialization semantics.
#[test]
fn certificate_integration() {
    let system = TermSystem::new(vec![Rule::new(a(), g(a())).unwrap()]);
    let language = partial_singleton(&[("a", 0), ("g", 1)], &[g(g(a()))]);
    let limits = RelationLimits::default();
    let automaton_limits = TreeAutomatonLimits::default();
    let outcome = saturation_preimage(&system, &language, &limits, &automaton_limits)
        .expect("ground saturation is approved");

    let certificate = outcome.certificate();
    assert_eq!(
        certificate.construction(),
        PreimageConstruction::GttSaturation
    );
    assert_eq!(certificate.operation(), PreimageOperation::Saturation);
    assert_eq!(certificate.authority(), &CertificateAuthority::Exact);
    assert!(certificate.is_complete());
    assert!(certificate.verify(&system, &language, &limits));
    assert!(certificate.verify_result(outcome.automaton()));

    #[cfg(feature = "serialize")]
    {
        let json = serde_json::to_string(certificate).expect("serializes");
        let restored: amari_rewrite::language::PreimageCertificate =
            serde_json::from_str(&json).expect("deserializes");
        // Deserialization drops the completion claim: still pending,
        // provenance still verifiable, no bound result.
        assert!(!restored.is_complete());
        assert!(restored.verify(&system, &language, &limits));
        assert!(!restored.verify_result(outcome.automaton()));
    }
}

/// Limits: a tiny operation budget and a tiny state ceiling are typed
/// limit outcomes, and no completed result is produced.
#[test]
fn typed_limits() {
    let system = TermSystem::new(vec![Rule::new(a(), g(a())).unwrap()]);
    let language = partial_singleton(&[("a", 0), ("g", 1)], &[g(g(a()))]);

    let tiny_operations = RelationLimits::new(4_096, 64, 4_096, 1).expect("valid tightened limits");
    let result = saturation_preimage(
        &system,
        &language,
        &tiny_operations,
        &TreeAutomatonLimits::default(),
    );
    assert!(
        matches!(result, Err(RewriteError::RelationLimitExceeded { .. })),
        "operation ceiling must be a typed limit outcome, got {result:?}"
    );

    let tiny_states = TreeAutomatonLimits::new(1, TreeAutomatonLimits::MAX_TRANSITIONS, 16)
        .expect("valid tightened limits");
    let result = saturation_preimage(&system, &language, &RelationLimits::default(), &tiny_states);
    assert!(
        matches!(result, Err(RewriteError::RelationLimitExceeded { .. })),
        "state ceiling must be a typed limit outcome, got {result:?}"
    );
}

/// Determinism: two identical runs produce byte-identical canonical output
/// and identical result digests.
#[test]
fn determinism() {
    let system = TermSystem::new(vec![Rule::new(a(), g(a())).unwrap()]);
    let language = partial_singleton(&[("a", 0), ("g", 1)], &[g(g(a()))]);
    let limits = RelationLimits::default();
    let automaton_limits = TreeAutomatonLimits::default();

    let first = saturation_preimage(&system, &language, &limits, &automaton_limits)
        .expect("ground saturation is approved");
    let second = saturation_preimage(&system, &language, &limits, &automaton_limits)
        .expect("ground saturation is approved");

    assert_eq!(
        first.automaton().canonical_bytes(),
        second.automaton().canonical_bytes()
    );
    assert_eq!(first.certificate().result(), second.certificate().result());
}

/// Review round 1 (P1): an unchecked deep rule side must be a typed
/// limit outcome, never a stack overflow. Certificate issuance must
/// preflight term bounds ITERATIVELY before any recursive traversal
/// (classification, digest framing). Runs on a small thread stack;
/// the term is leaked to avoid the recursive Drop (house rule).
#[test]
fn deep_unchecked_terms_are_a_typed_outcome() {
    std::thread::Builder::new()
        .stack_size(128 * 1024)
        .spawn(|| {
            let mut term = a();
            for _ in 0..10_000 {
                term = g(term);
            }
            let system = TermSystem::new(vec![Rule::new_unchecked(term, b())]);
            let language = singleton_language();
            let limits = RelationLimits::new(4_096, 1, 4_096, 1).expect("valid tight limits");
            let result =
                saturation_preimage(&system, &language, &limits, &TreeAutomatonLimits::default());
            assert!(
                matches!(result, Err(RewriteError::RelationLimitExceeded { .. })),
                "deep unchecked terms must be a typed outcome, got {result:?}"
            );
            std::mem::forget(system);
        })
        .expect("thread spawns")
        .join()
        .expect("the call returns instead of overflowing the stack");
}

/// Review round 1 (P2): reserved relation storage is charged as
/// constraints BEFORE allocation — closure matrix (N²), right-side
/// evaluation sets (H·N), and the edge set (min(m·N, N²)). With N=3
/// the reservation is 9+3+3=15 cells, so a 9-cell budget must fail.
#[test]
fn constraints_accounting_covers_reserved_storage() {
    let system = TermSystem::new(vec![Rule::new(a(), b()).unwrap()]);
    let language = singleton_language();
    let tight = RelationLimits::new(4_096, 64, 54, 1_000_000).expect("valid tight limits");
    let result = saturation_preimage(&system, &language, &tight, &TreeAutomatonLimits::default());
    assert!(
        matches!(result, Err(RewriteError::RelationLimitExceeded { .. })),
        "the reserved-storage reservation must exhaust a 9-cell budget, got {result:?}"
    );
    let generous = RelationLimits::default();
    let outcome = saturation_preimage(
        &system,
        &language,
        &generous,
        &TreeAutomatonLimits::default(),
    )
    .expect("generous limits succeed");
    assert!(outcome.certificate().verify(&system, &language, &generous));
}

/// Review round 2 (P1): verification and the public digest helpers
/// must not recursively traverse unmeasured terms either — verifying
/// against a deep unchecked system must return false (digest
/// mismatch), never abort. Small thread stack; the term is leaked to
/// avoid the recursive Drop.
#[test]
fn deep_system_verification_is_safe() {
    let system = TermSystem::new(vec![Rule::new(a(), b()).unwrap()]);
    let language = singleton_language();
    let limits = RelationLimits::default();
    let certificate =
        saturation_preimage(&system, &language, &limits, &TreeAutomatonLimits::default())
            .expect("small system is approved")
            .certificate()
            .clone();
    std::thread::Builder::new()
        .stack_size(128 * 1024)
        .spawn(move || {
            let mut deep = a();
            for _ in 0..10_000 {
                deep = g(deep);
            }
            let deep_system = TermSystem::new(vec![Rule::new_unchecked(deep, b())]);
            let _ = amari_rewrite::language::system_digest(&deep_system);
            assert!(!certificate.verify(&deep_system, &language, &limits));
            std::mem::forget(deep_system);
        })
        .expect("thread spawns")
        .join()
        .expect("verification returns instead of overflowing the stack");
}

/// Review round 2 (P2): the elimination pass's own structures
/// (closure + preimage maps) and each round's evaluation buffers are
/// reserved relation storage too — a 15-cell budget covers the
/// saturation reservations exactly and must now be exhausted by the
/// remaining allocations.
#[test]
fn elimination_storage_is_reserved() {
    let system = TermSystem::new(vec![Rule::new(a(), b()).unwrap()]);
    let language = singleton_language();
    let tight = RelationLimits::new(4_096, 64, 54, 1_000_000).expect("valid tight limits");
    let result = saturation_preimage(&system, &language, &tight, &TreeAutomatonLimits::default());
    assert!(
        matches!(result, Err(RewriteError::RelationLimitExceeded { .. })),
        "a 15-cell budget must not cover the whole pipeline, got {result:?}"
    );
}

/// Review round 3 (P1): verification must be safe for ANY certificate
/// input — including a deserialized pending certificate whose recorded
/// digest matches a deep unchecked system. classify_system's linearity
/// counting must not recurse; verification re-measures term bounds.
#[cfg(feature = "serialize")]
#[test]
fn deep_system_verification_with_pending_certificate_is_safe() {
    let system = TermSystem::new(vec![Rule::new(a(), b()).unwrap()]);
    let language = singleton_language();
    let limits = RelationLimits::default();
    let certificate =
        saturation_preimage(&system, &language, &limits, &TreeAutomatonLimits::default())
            .expect("small system is approved")
            .certificate()
            .clone();
    std::thread::Builder::new()
        .stack_size(128 * 1024)
        .spawn(move || {
            let mut deep = a();
            for _ in 0..10_000 {
                deep = g(deep);
            }
            let deep_system = TermSystem::new(vec![Rule::new_unchecked(deep, b())]);
            // Craft a pending certificate bound to the deep system by
            // patching the wire form's digests (computation of the
            // digest itself is iterative and safe on this small stack).
            let mut wire: serde_json::Value =
                serde_json::to_value(&certificate).expect("certificate serializes");
            wire["system"] =
                serde_json::to_value(amari_rewrite::language::system_digest(&deep_system))
                    .expect("digest serializes");
            wire["result"] = serde_json::Value::Null;
            let pending: PreimageCertificate =
                serde_json::from_value(wire).expect("patched wire form deserializes");
            assert!(!pending.verify(&deep_system, &language, &limits));
            std::mem::forget(deep_system);
        })
        .expect("thread spawns")
        .join()
        .expect("verification returns instead of overflowing the stack");
}

/// Review round 6 (P2): the right-side evaluation's freshly allocated
/// raw/closed sets and the edge-conversion/assembly clones are billed
/// as constraints. The reviewer's `{a → a}` singleton case charges 21
/// cells without them; the full buffer inventory requires more, so a
/// 54-cell budget must now exhaust (54 is the pre-fix total).
#[test]
fn evaluation_buffers_and_assembly_clones_are_reserved() {
    let system = TermSystem::new(vec![Rule::new(a(), a()).unwrap()]);
    let language = singleton_language();
    let tight = RelationLimits::new(4_096, 64, 54, 1_000_000).expect("valid tight limits");
    let result = saturation_preimage(&system, &language, &tight, &TreeAutomatonLimits::default());
    assert!(
        matches!(result, Err(RewriteError::RelationLimitExceeded { .. })),
        "the full evaluation/assembly buffer inventory must be billed, got {result:?}"
    );
}
