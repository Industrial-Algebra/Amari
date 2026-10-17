// SPDX-License-Identifier: MIT OR Apache-2.0

//! Completion-assisted inverse search contract tests (0.25, Task 34).
//!
//! The authority discipline is the contract under test: completion
//! traces are equational-theory evidence only. Completed rules may
//! generate candidate predecessor terms, but every positive witness
//! is replayed against the ORIGINAL directed system, and no
//! negative/exhaustion authority ever transfers from the completed
//! system.

#![cfg(feature = "completion")]

use amari_rewrite::analysis::Precedence;
use amari_rewrite::completion::inverse::{
    guided_predecessors, prepare_guidance, verify_equivalence, CandidateRejection,
    CompletionGuidance, GuidedSearchOutcome, InconclusiveReason,
};
use amari_rewrite::completion::CompletionTrace;
use amari_rewrite::inverse::InverseSearchConfig;
use amari_rewrite::relation::{RelationLimits, RelationResources};
use amari_rewrite::trs::{Rule, Symbol, Term, TermSystem};
use amari_rewrite::RewriteError;

fn precedence(entries: &[(&str, usize)]) -> Precedence {
    Precedence::new(
        entries
            .iter()
            .map(|(name, arity)| (Symbol::new(*name), *arity))
            .collect(),
    )
    .expect("valid precedence")
}

fn rule(lhs: Term, rhs: Term) -> Rule {
    Rule::new(lhs, rhs).expect("valid rule")
}

fn make_system(rules: Vec<Rule>) -> TermSystem {
    TermSystem::new(rules)
}

fn app(name: &str, args: impl IntoIterator<Item = Term>) -> Term {
    Term::sym(name, args)
}

fn x() -> Term {
    Term::var("x")
}

fn y() -> Term {
    Term::var("y")
}

fn a() -> Term {
    Term::constant("a")
}

fn b() -> Term {
    Term::constant("b")
}

fn c() -> Term {
    Term::constant("c")
}

fn f_of_x() -> Term {
    app("f", [x()])
}

fn g_of_x() -> Term {
    app("g", [x()])
}

fn h_of_x() -> Term {
    app("h", [x()])
}

fn default_resources() -> RelationResources {
    RelationResources::new(&RelationLimits::default())
}

fn guidance_of(system: &TermSystem, order: Precedence) -> CompletionGuidance {
    let mut resources = default_resources();
    prepare_guidance(system, order, &mut resources)
        .expect("guidance runs")
        .expect("a convergent completion yields guidance")
}

/// Test 1 (motivating): the original system is non-confluent, so its
/// directed reachability differs from the completed system's. The
/// completed search proposes a candidate and the original replay
/// verifies a genuine original predecessor.
#[test]
fn motivating_case_completed_candidate_replayed_against_original() {
    let original = make_system(vec![rule(f_of_x(), g_of_x()), rule(f_of_x(), h_of_x())]);
    let guidance = guidance_of(&original, precedence(&[("f", 1), ("g", 1), ("h", 1)]));
    let target = app("h", [a()]);
    let mut resources = default_resources();
    let outcome = guided_predecessors(
        &original,
        &guidance,
        &target,
        &InverseSearchConfig::default(),
        &mut resources,
    )
    .expect("guided search runs");
    match outcome {
        GuidedSearchOutcome::VerifiedWitness {
            path,
            candidates_rejected,
        } => {
            assert_eq!(path.first(), Some(&app("f", [a()])));
            assert_eq!(path.last(), Some(&target));
            // `g(a)` is a completed-only predecessor: relation-changing
            // completion generated it, and original replay rejects it.
            assert!(
                candidates_rejected.iter().any(|rejection| matches!(
                    rejection,
                    CandidateRejection::NotAnOriginalPredecessor { predecessor }
                        if *predecessor == app("g", [a()])
                )),
                "the completed-only candidate must be rejected"
            );
        }
        other => panic!("expected VerifiedWitness, got {other:?}"),
    }
}

/// Test 2: a completed-system candidate witness is replayed and
/// verified against the original system, yielding the original
/// forward derivation.
#[test]
fn completed_candidate_witness_verified_as_original_derivation() {
    let original = make_system(vec![rule(f_of_x(), g_of_x()), rule(g_of_x(), h_of_x())]);
    let guidance = guidance_of(&original, precedence(&[("f", 1), ("g", 1), ("h", 1)]));
    let target = app("h", [a()]);
    let mut resources = default_resources();
    let outcome = guided_predecessors(
        &original,
        &guidance,
        &target,
        &InverseSearchConfig::default(),
        &mut resources,
    )
    .expect("guided search runs");
    match outcome {
        GuidedSearchOutcome::VerifiedWitness { path, .. } => {
            // The nearest original forward path g(a) -> h(a).
            assert_eq!(path, vec![app("g", [a()]), app("h", [a()])]);
        }
        other => panic!("expected VerifiedWitness, got {other:?}"),
    }
}

/// Test 3: completion may change the directed relation. The candidate
/// the completed system proposes must NOT be reported as a witness
/// when the original directed system cannot replay it.
#[test]
fn relation_changing_completion_candidate_is_rejected() {
    // `a -> b` is reoriented to `b -> a` by completion (b outranks a),
    // so the completed system can propose `b` as a predecessor of `a`
    // even though the original `a -> b` never reaches `a` from `b`.
    let original = make_system(vec![rule(a(), b())]);
    let guidance = guidance_of(&original, precedence(&[("b", 0), ("a", 0)]));
    assert_eq!(guidance.completed, make_system(vec![rule(b(), a())]));
    let target = a();
    let mut resources = default_resources();
    let outcome = guided_predecessors(
        &original,
        &guidance,
        &target,
        &InverseSearchConfig::default(),
        &mut resources,
    )
    .expect("guided search runs");
    match outcome {
        GuidedSearchOutcome::NoVerifiedWitness {
            candidates_rejected,
        } => {
            assert_eq!(candidates_rejected.len(), 1);
            assert!(matches!(
                &candidates_rejected[0],
                CandidateRejection::NotAnOriginalPredecessor { predecessor } if *predecessor == b()
            ));
        }
        other => panic!("expected NoVerifiedWitness, got {other:?}"),
    }
}

/// Test 4: a residual-schema candidate from the completed system
/// (here `g(cpRy)`, whose erased variable is a completion artifact)
/// is rejected while the genuine original predecessor verifies.
#[test]
fn residual_schema_difference_candidate_is_rejected() {
    let original = make_system(vec![rule(f_of_x(), g_of_x()), rule(f_of_x(), c())]);
    let guidance = guidance_of(&original, precedence(&[("f", 1), ("g", 1), ("c", 0)]));
    let target = c();
    let mut resources = default_resources();
    let outcome = guided_predecessors(
        &original,
        &guidance,
        &target,
        &InverseSearchConfig::default(),
        &mut resources,
    )
    .expect("guided search runs");
    match outcome {
        GuidedSearchOutcome::VerifiedWitness {
            candidates_rejected,
            ..
        } => {
            assert!(
                candidates_rejected.iter().any(|rejection| matches!(
                    rejection,
                    CandidateRejection::NotAnOriginalPredecessor { predecessor }
                        if *predecessor == app("g", [Term::var("cpRx")])
                )),
                "the residual-schema candidate must be rejected"
            );
        }
        other => panic!("expected VerifiedWitness, got {other:?}"),
    }
}

/// Test 5: only a convergent completion is sound equational evidence;
/// a partial run yields no guidance.
#[test]
fn partial_completion_yields_no_guidance() {
    // `h(x, y) -> f(y)` and `h(x, y) -> g(x)` overlap at the root,
    // producing an unorientable equation, so completion is Partial.
    let system = make_system(vec![
        rule(app("h", [x(), y()]), app("f", [y()])),
        rule(app("h", [x(), y()]), app("g", [x()])),
    ]);
    let order = precedence(&[("h", 2), ("f", 1), ("g", 1)]);
    let mut resources = default_resources();
    let guidance = prepare_guidance(&system, order, &mut resources).expect("completion runs");
    assert!(guidance.is_none(), "Partial completion carries no guidance");
}

/// Test 6: replay against the original directed system is mandatory.
/// A convergent completed system alone does not witness anything.
#[test]
fn original_replay_is_mandatory_for_witnesses() {
    let original = make_system(vec![rule(a(), b())]);
    let guidance = guidance_of(&original, precedence(&[("b", 0), ("a", 0)]));
    let target = a();
    let mut resources = default_resources();
    let outcome = guided_predecessors(
        &original,
        &guidance,
        &target,
        &InverseSearchConfig::default(),
        &mut resources,
    )
    .expect("guided search runs");
    // The completed system is convergent, yet no witness is returned
    // because the original system cannot replay the candidate.
    assert!(!matches!(
        outcome,
        GuidedSearchOutcome::VerifiedWitness { .. }
    ));
}

/// Test 7: negative/exhaustion authority never transfers. An empty
/// completed-system search is inconclusive, never "no witness
/// exists".
#[test]
fn empty_completed_search_is_inconclusive() {
    let original = make_system(vec![rule(f_of_x(), g_of_x()), rule(g_of_x(), h_of_x())]);
    let guidance = guidance_of(&original, precedence(&[("f", 1), ("g", 1), ("h", 1)]));
    // `k` never appears on any right-hand side, so no backward step
    // exists in the completed system.
    let target = app("k", [a()]);
    let mut resources = default_resources();
    let outcome = guided_predecessors(
        &original,
        &guidance,
        &target,
        &InverseSearchConfig::default(),
        &mut resources,
    )
    .expect("guided search runs");
    match outcome {
        GuidedSearchOutcome::Inconclusive {
            reason: InconclusiveReason::NoCandidatesGenerated,
            candidates_rejected,
        } => assert!(candidates_rejected.is_empty()),
        other => panic!("expected Inconclusive(NoCandidatesGenerated), got {other:?}"),
    }
}

/// Test 8: the relation-equivalence certificate is produced when
/// every rule equation of `a` normalizes equally under the convergent
/// system and every rule equation of the convergent system is
/// derivable in `a`.
#[test]
fn equivalence_certificate_positive() {
    let a = make_system(vec![rule(f_of_x(), g_of_x()), rule(g_of_x(), h_of_x())]);
    let b = make_system(vec![
        rule(f_of_x(), g_of_x()),
        rule(g_of_x(), h_of_x()),
        rule(f_of_x(), h_of_x()),
    ]);
    let mut resources = default_resources();
    let certificate = verify_equivalence(&a, &b, &mut resources)
        .expect("verification runs")
        .expect("equivalent systems yield a certificate");
    assert_eq!(certificate.a_normal_form_witnesses.len(), 2);
    assert_eq!(certificate.b_derivations.len(), 3);
    // `f(x) -> h(x)` is derivable in `a` in two original steps.
    let composite = certificate
        .b_derivations
        .iter()
        .find(|derivation| derivation.equation == (f_of_x(), h_of_x()))
        .expect("the composite equation is certified");
    assert_eq!(composite.path.len(), 3);
}

/// Test 9: the certificate is withheld when the two systems are not
/// equationally equivalent.
#[test]
fn equivalence_certificate_negative() {
    let a = make_system(vec![rule(f_of_x(), g_of_x())]);
    let b = make_system(vec![rule(f_of_x(), h_of_x())]);
    let mut resources = default_resources();
    let certificate = verify_equivalence(&a, &b, &mut resources).expect("verification runs");
    assert!(certificate.is_none());
}

/// Test 10: the certificate makes no equivalence claim for a
/// non-known-convergent `a` unless the convergent system's rules are
/// directionally derivable in `a`. This is deliberately conservative:
/// directed derivability, not equational equality, is the authority.
#[test]
fn equivalence_certificate_is_conservative_for_non_convergent_a() {
    // `a` is non-confluent; `g(x) = h(x)` holds equationally (a
    // critical pair) but neither side reaches the other by directed
    // rewriting in `a`.
    let a = make_system(vec![rule(f_of_x(), g_of_x()), rule(f_of_x(), h_of_x())]);
    let b = make_system(vec![rule(f_of_x(), h_of_x()), rule(g_of_x(), h_of_x())]);
    let mut resources = default_resources();
    let certificate = verify_equivalence(&a, &b, &mut resources).expect("verification runs");
    assert!(
        certificate.is_none(),
        "directed derivability cannot certify this equation"
    );
}

/// Test 11: exhausting the shared operation budget is a typed error,
/// never a silent partial or a negative witness.
#[test]
fn guided_search_limit_exhaustion_is_typed() {
    let original = make_system(vec![rule(f_of_x(), g_of_x()), rule(g_of_x(), h_of_x())]);
    let guidance = guidance_of(&original, precedence(&[("f", 1), ("g", 1), ("h", 1)]));
    let target = app("h", [a()]);
    let limits = RelationLimits::new(4_096, 64, 4_096, 1).expect("valid limits");
    let mut resources = RelationResources::new(&limits);
    let outcome = guided_predecessors(
        &original,
        &guidance,
        &target,
        &InverseSearchConfig::default(),
        &mut resources,
    );
    assert!(
        matches!(outcome, Err(RewriteError::RelationLimitExceeded { .. })),
        "expected a typed limit error, got {outcome:?}"
    );
}

/// Test 12: the same input produces the same guided outcome.
#[test]
fn guided_search_is_deterministic() {
    let original = make_system(vec![rule(f_of_x(), g_of_x()), rule(f_of_x(), h_of_x())]);
    let guidance = guidance_of(&original, precedence(&[("f", 1), ("g", 1), ("h", 1)]));
    let target = app("h", [a()]);
    let config = InverseSearchConfig::default();
    let first = guided_predecessors(
        &original,
        &guidance,
        &target,
        &config,
        &mut default_resources(),
    )
    .expect("guided search runs");
    let second = guided_predecessors(
        &original,
        &guidance,
        &target,
        &config,
        &mut default_resources(),
    )
    .expect("guided search runs");
    assert_eq!(first, second);
}

/// PR #312 round 1 P1: equivalence certificates must preserve
/// variable identity — {f(x,y)->x} and {f(x,y)->y} are NOT
/// equivalent (independent canonicalization conflated x with y).
#[test]
fn equivalence_certificate_distinguishes_variables() {
    let a = TermSystem::new(vec![rule(
        Term::sym("f", [Term::var("x"), Term::var("y")]),
        Term::var("x"),
    )]);
    let b = TermSystem::new(vec![rule(
        Term::sym("f", [Term::var("x"), Term::var("y")]),
        Term::var("y"),
    )]);
    let mut resources = RelationResources::new(&RelationLimits::default());
    let certificate = verify_equivalence(&a, &b, &mut resources).expect("typed outcome");
    assert!(
        certificate.is_none(),
        "x-projecting and y-projecting systems are not equivalent"
    );
}

/// PR #312 round 1 P2: a candidate valid in the completed system
/// but not derivable in the original must be REJECTED — alpha-only
/// lookup accepted f(x,z) whose original successor is g(x).
#[test]
fn candidate_preserving_target_variables_is_verified_exactly() {
    let original = TermSystem::new(vec![rule(
        Term::sym("f", [Term::var("x"), Term::var("y")]),
        Term::sym("g", [Term::var("x")]),
    )]);
    // Tampered "completed" system (public guidance fields): sends
    // the candidate down the WRONG variable.
    let tampered = TermSystem::new(vec![rule(
        Term::sym("f", [Term::var("x"), Term::var("y")]),
        Term::sym("g", [Term::var("y")]),
    )]);
    let guidance = CompletionGuidance {
        completed: tampered,
        trace: CompletionTrace::default(),
    };
    let config = InverseSearchConfig::default();
    let mut resources = RelationResources::new(&RelationLimits::default());
    let outcome = guided_predecessors(
        &original,
        &guidance,
        &Term::sym("g", [Term::var("z")]),
        &config,
        &mut resources,
    )
    .expect("typed outcome");
    assert!(
        !matches!(outcome, GuidedSearchOutcome::VerifiedWitness { .. }),
        "no witness may verify through the wrong variable: {outcome:?}"
    );
}

/// PR #312 round 1 P2: normalization must charge the replacement
/// shape BEFORE materialization and meter individual rule attempts
/// — a 3,000-ary expansion past the node ceiling is a typed error
/// with observable per-attempt accounting.
#[test]
fn normalization_charges_before_materializing_expansions() {
    // `a`'s ground rule side f(g(a)) is normalized under `b`'s
    // exploding rule f(x) -> h(x, ..., x): the replacement is
    // 6,001 nodes — over the 4,096 ceiling.
    let big_rhs = Term::sym("h", vec![Term::var("x"); 3_000]);
    // Decoy j-rule in `b` so the exploding f-rule is the SECOND
    // attempt at the root — per-attempt metering is observable.
    let b = TermSystem::new(vec![
        rule(Term::sym("j", [Term::var("x")]), Term::var("x")),
        rule(Term::sym("f", [Term::var("x")]), big_rhs),
    ]);
    let a = TermSystem::new(vec![rule(
        Term::sym("f", [Term::sym("g", [Term::constant("a")])]),
        Term::constant("c"),
    )]);
    let mut resources = RelationResources::new(&RelationLimits::default());
    let outcome = verify_equivalence(&a, &b, &mut resources);
    assert!(
        matches!(outcome, Err(RewriteError::RelationLimitExceeded { .. })),
        "expected typed limit, got {outcome:?}"
    );
    // Per-attempt metering: the decoy j-rule and the exploding
    // f-rule attempts at the root are each charged. Unmetered
    // apply_once charged only the loop iteration (1 op).
    assert!(
        resources.operations() >= 3,
        "individual rule attempts must be metered, got {}",
        resources.operations()
    );
}

/// PR #312 round 2 P2: original replay must respect the configured
/// search depth — with max_depth 1, only the 1-step derivation
/// verifies; a 2-step derivation is truncated, not verified and not
/// exhaustively rejected.
#[test]
fn original_replay_respects_the_configured_depth() {
    let original = TermSystem::new(vec![
        rule(Term::constant("a"), Term::constant("m")),
        rule(Term::constant("m"), Term::constant("z")),
    ]);
    let completed = TermSystem::new(vec![
        rule(Term::constant("a"), Term::constant("z")),
        rule(Term::constant("m"), Term::constant("z")),
    ]);
    let guidance = CompletionGuidance {
        completed,
        trace: CompletionTrace::default(),
    };
    let defaults = InverseSearchConfig::default();
    let config = InverseSearchConfig::new(
        1, // max_depth: only one-step derivations verify
        defaults.max_states(),
        defaults.max_transitions(),
        defaults.max_term_nodes(),
        defaults.max_term_depth(),
        defaults.max_constraints(),
        defaults.max_groundings(),
        defaults.max_operations(),
        defaults.max_frontier_bytes(),
        defaults.max_trace_bytes(),
    )
    .expect("valid config");
    let mut resources = RelationResources::new(&RelationLimits::default());
    let outcome = guided_predecessors(
        &original,
        &guidance,
        &Term::constant("z"),
        &config,
        &mut resources,
    )
    .expect("typed outcome");
    match outcome {
        GuidedSearchOutcome::VerifiedWitness { path, .. } => {
            assert_eq!(
                path,
                vec![Term::constant("m"), Term::constant("z")],
                "the 1-step witness, not the 2-step one: {path:?}"
            );
        }
        other => panic!("expected the 1-step VerifiedWitness, got {other:?}"),
    }
}
