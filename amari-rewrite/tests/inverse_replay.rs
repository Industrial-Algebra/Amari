//! Replay certificate binding/hardening tests
//! (0.25 Cohort 3, Task 16).

use amari_rewrite::inverse::{
    BackwardDerivation, BackwardExplorer, BackwardProvenance, BackwardSearchOutcome,
    BidirectionalDerivation, BidirectionalExplorer, BidirectionalSearchOutcome, GuidanceMode,
    InverseSearchConfig, ReplayCertificate, ReplayDerivation, ReplayQuery, ResourceObservation,
    SearchMode, SymbolicPredecessor,
};
use amari_rewrite::relation::{ConstraintSet, RelationLimits, RelationResources, RuleId};
use amari_rewrite::trs::{Rule, Substitution, Term, TermSystem};
use amari_rewrite::Path;

fn parse(text: &str) -> Term {
    let text = text.trim();
    if let Some(open) = text.find('(') {
        let head = &text[..open];
        let inner = &text[open + 1..text.len() - 1];
        let mut args = Vec::new();
        let mut depth = 0i32;
        let mut start = 0;
        for (index, ch) in inner.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => depth -= 1,
                ',' if depth == 0 => {
                    args.push(parse(&inner[start..index]));
                    start = index + 1;
                }
                _ => {}
            }
        }
        args.push(parse(&inner[start..]));
        Term::sym(head, args)
    } else if text.chars().next().is_some_and(char::is_uppercase) || text.starts_with('?') {
        Term::var(text)
    } else {
        Term::constant(text)
    }
}

fn system(rules: Vec<Rule>) -> TermSystem {
    TermSystem::new(rules)
}

fn small_config() -> InverseSearchConfig {
    InverseSearchConfig::new(
        8,
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
    .unwrap()
}

fn witness_certificate() -> (TermSystem, ReplayCertificate) {
    let rules = system(vec![
        Rule::new(parse("add(X, zero)"), parse("X")).unwrap(),
        Rule::new(parse("h(X)"), parse("X")).unwrap(),
    ]);
    let target = parse("a");
    let goal = parse("h(add(a, zero))");
    let outcome = BackwardExplorer::new(&rules, small_config())
        .search(&target, &goal, SearchMode::BreadthFirst)
        .unwrap();
    let BackwardSearchOutcome::Witness(derivation) = outcome else {
        panic!("expected witness");
    };
    let certificate = ReplayCertificate::issue(
        &rules,
        ReplayQuery::Backward {
            target: target.clone(),
            goal: goal.clone(),
        },
        ReplayDerivation::Backward(derivation),
        small_config(),
        GuidanceMode::CompleteWithinLimits,
        ResourceObservation {
            states: 3,
            transitions: 4,
            retained_bytes: 512,
            operations: 3,
        },
    );
    (rules, certificate)
}

#[test]
fn well_formed_certificate_verifies() {
    let (rules, certificate) = witness_certificate();
    certificate.verify(&rules).unwrap();
}

#[test]
fn tampered_system_is_rejected() {
    let (_rules, certificate) = witness_certificate();
    let other = system(vec![Rule::new(parse("add(X, zero)"), parse("X")).unwrap()]);
    assert!(certificate.verify(&other).is_err());
}

#[test]
fn tampered_hashes_are_rejected() {
    let (rules, certificate) = witness_certificate();
    for field in ["system", "query", "config", "guidance"] {
        let mut tampered = certificate.clone();
        let bogus = amari_rewrite::relation::Sha256Digest::framed(
            "amari.inverse.exhaustion/v1",
            b"tampered",
        );
        match field {
            "system" => tampered.system_hash = bogus,
            "query" => tampered.query_hash = bogus,
            "config" => tampered.config_hash = bogus,
            _ => tampered.guidance_hash = bogus,
        }
        assert!(
            tampered.verify(&rules).is_err(),
            "tampered {field} hash must be rejected"
        );
    }
}

#[test]
fn tampered_derivation_is_rejected() {
    let (rules, certificate) = witness_certificate();
    // Swap in a derivation for a different goal: the recorded steps
    // do not replay to the certificate's query.
    let other_goal = parse("add(a, zero)");
    let outcome = BackwardExplorer::new(&rules, small_config())
        .search(&parse("a"), &other_goal, SearchMode::BreadthFirst)
        .unwrap();
    let BackwardSearchOutcome::Witness(derivation) = outcome else {
        panic!("expected witness");
    };
    let mut tampered = certificate.clone();
    tampered.derivation = ReplayDerivation::Backward(derivation);
    assert!(tampered.verify(&rules).is_err());
}

#[test]
fn resources_above_config_ceilings_are_rejected() {
    let (rules, certificate) = witness_certificate();
    let mut tampered = certificate.clone();
    tampered.resources.operations = u64::MAX;
    assert!(tampered.verify(&rules).is_err());
    let mut tampered = certificate.clone();
    tampered.resources.states = u64::MAX;
    assert!(tampered.verify(&rules).is_err());
}

#[cfg(feature = "serialize")]
#[test]
fn replay_verification_is_deterministic_across_serializations() {
    let (rules, certificate) = witness_certificate();
    // Serialize and re-verify: the byte interface is the
    // cross-process boundary, and verdicts must be identical.
    let json = serde_json::to_string(&certificate).unwrap();
    let restored: ReplayCertificate = serde_json::from_str(&json).unwrap();
    assert!(restored.verify(&rules).is_ok());
    assert!(restored.verify(&rules).is_ok());
    assert_eq!(certificate, restored);
}

#[test]
fn bidirectional_certificates_verify() {
    use amari_rewrite::inverse::{BidirectionalExplorer, BidirectionalSearchOutcome};
    let rules = system(vec![Rule::new(parse("add(X, zero)"), parse("X")).unwrap()]);
    let source = parse("add(a, zero)");
    let goal = parse("a");
    let outcome = BidirectionalExplorer::new(&rules, small_config())
        .search(&source, &goal)
        .unwrap();
    let BidirectionalSearchOutcome::Witness(derivation) = outcome else {
        panic!("expected witness");
    };
    let certificate = ReplayCertificate::issue(
        &rules,
        ReplayQuery::Bidirectional {
            source: source.clone(),
            goal: goal.clone(),
        },
        ReplayDerivation::Bidirectional(derivation),
        small_config(),
        GuidanceMode::CompleteWithinLimits,
        ResourceObservation {
            states: 2,
            transitions: 1,
            retained_bytes: 256,
            operations: 1,
        },
    );
    certificate.verify(&rules).unwrap();
    // Wrong query kind is rejected.
    let mut tampered = certificate.clone();
    tampered.query = ReplayQuery::Backward {
        target: source,
        goal,
    };
    assert!(tampered.verify(&rules).is_err());
}

/// PR #302 round 3 P1 (reviewer repro): the bidirectional search's
/// forward half used deep application on matcher bindings, recording
/// `a` as the forward reduct of `f(Y, a)` under `f(X, Y) -> X`
/// (correct: the source variable `Y`). Certificates issued from such
/// witnesses failed verification ("forward half does not reach the
/// meeting term"). The witness must record `Y`; verification passes.
#[test]
fn bidirectional_witness_is_capture_free_and_verifies() {
    let rules = system(vec![Rule::new(parse("f(X, Y)"), parse("X")).unwrap()]);
    let source = parse("f(Y, a)");
    let goal = parse("a");
    let outcome = BidirectionalExplorer::new(&rules, small_config())
        .search(&source, &goal)
        .unwrap();
    let BidirectionalSearchOutcome::Witness(derivation) = outcome else {
        panic!("expected witness, got {outcome:?}");
    };
    assert_eq!(
        derivation.meeting_forward,
        parse("Y"),
        "forward reduct must be the source variable, not its capture"
    );
    let certificate = ReplayCertificate::issue(
        &rules,
        ReplayQuery::Bidirectional {
            source: source.clone(),
            goal: goal.clone(),
        },
        ReplayDerivation::Bidirectional(derivation),
        small_config(),
        GuidanceMode::CompleteWithinLimits,
        ResourceObservation {
            states: 2,
            transitions: 1,
            retained_bytes: 256,
            operations: 2,
        },
    );
    certificate.verify(&rules).unwrap();
}

// -------------------------------------------------------------------------
// Round-2 review findings (PR #384): one assignment per replay.
//
// Both pre-remediation behaviours below were independently plausible per
// record and jointly inconsistent, which is the defect: a certificate admits
// exactly the traces that ONE instantiation denotes.
// -------------------------------------------------------------------------

fn observation() -> ResourceObservation {
    ResourceObservation {
        states: 2,
        transitions: 1,
        retained_bytes: 256,
        operations: 16,
    }
}

/// A recorded backward step at the root position. Provenance hashes are the
/// canonical digests of the terms involved; the replay never trusts them, so a
/// hand-recorded derivation pins the replay semantics exactly.
fn recorded(term: Term, rule_id: RuleId, target: &Term, scope: u32) -> SymbolicPredecessor {
    SymbolicPredecessor {
        provenance: BackwardProvenance {
            rule_id,
            position: Path::root(),
            scope,
            target_hash: amari_rewrite::relation::Sha256Digest::canonical_term(
                "amari.relation.term/v1",
                target,
            ),
            predecessor_hash: amari_rewrite::relation::Sha256Digest::canonical_term(
                "amari.relation.term/v1",
                &term,
            ),
        },
        term,
        existentials: Vec::new(),
        constraints: ConstraintSet::new(),
        substitution: Substitution::new(),
        resources: RelationResources::new(&RelationLimits::default()),
    }
}

/// F7 (P1, `inverse/replay.rs:266`): the leaf/goal unifier is ONE assignment,
/// applied consistently to the declared target and every recorded step term.
///
/// Rule `k(a, V) -> h(V)`, backward target `h(Q)`, goal `k(Q, b)`, one root
/// predecessor `k(a, R)`. The leaf unifies with the goal only under
/// `{Q |-> a, R |-> b}`, so the consistent endpoint instance is `h(a)` — but the
/// replay reconstructs `h(R)`, which independently alpha-matches `h(Q)`.
/// Discarding the assignment (pre-remediation) admits the certificate; keeping
/// it rejects the inconsistent instantiation.
#[test]
fn backward_replay_applies_the_leaf_goal_unifier_consistently() {
    let rules = system(vec![
        Rule::new(parse("k(a, V)"), parse("h(V)")).expect("checked rule")
    ]);
    let rule_id = RuleId::from_rule(&rules.rules()[0]);
    let target = parse("h(Q)");
    let goal = parse("k(Q, b)");
    let derivation = BackwardDerivation {
        steps: vec![recorded(parse("k(a, R)"), rule_id, &target, 1)],
    };
    let certificate = ReplayCertificate::issue(
        &rules,
        ReplayQuery::Backward {
            target: target.clone(),
            goal,
        },
        ReplayDerivation::Backward(derivation),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        observation(),
    );
    let error = certificate
        .verify(&rules)
        .expect_err("the reconstructed target must equal the declared target under one assignment");
    assert!(
        error.to_string().contains("reconstruct"),
        "the typed rejection names the reconstruction failure, got {error}"
    );
}

/// F7 positive control: a genuine symbolic backward witness still verifies, and
/// a variable the leaf/goal unifier does not fix stays symbolic.
///
/// Rule `k(V, W) -> h(V, W)`, backward target `h(Q, W)`, goal `k(a, Y)`: the
/// leaf `k(Q, W)` unifies with the goal under `{Q |-> a, W |-> Y}`, leaving `Y`
/// free. The reconstructed target is `h(a, Y)` under that assignment, exactly
/// the declared target.
#[test]
fn backward_replay_admits_a_consistent_symbolic_witness() {
    let rules = system(vec![
        Rule::new(parse("k(V, W)"), parse("h(V, W)")).expect("checked rule")
    ]);
    let target = parse("h(Q, W)");
    let goal = parse("k(a, Y)");
    let outcome = BackwardExplorer::new(&rules, InverseSearchConfig::default())
        .search(&target, &goal, SearchMode::BreadthFirst)
        .expect("search must run");
    let BackwardSearchOutcome::Witness(derivation) = outcome else {
        panic!("the symbolic witness must be found, got {outcome:?}");
    };
    let certificate = ReplayCertificate::issue(
        &rules,
        ReplayQuery::Backward {
            target: target.clone(),
            goal,
        },
        ReplayDerivation::Backward(derivation),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        observation(),
    );
    certificate
        .verify(&rules)
        .expect("a consistent symbolic witness must verify");
}

/// F8 (P1, `inverse/replay.rs:336`): the backward half of a bidirectional
/// replay uses ONE recorded-term-to-replay correspondence, extended step by
/// step and carried into the goal check.
///
/// Ground rules `f(a) -> g(b)` and `g(b) -> c`, source `f(a)`, goal `c`, zero
/// forward steps, both meetings `f(a)`, empty unifier. The recorded backward
/// half is `[g(Z), f(Z)]` with the SAME `Z` (goal side first). Matching each
/// record independently binds `Z |-> a` then `Z |-> b` and admits the trace,
/// although no single instantiation of `Z` denotes that replay.
#[test]
fn bidirectional_replay_uses_one_correspondence_across_the_backward_half() {
    let rules = system(vec![
        Rule::new(parse("f(a)"), parse("g(b)")).expect("checked rule"),
        Rule::new(parse("g(b)"), parse("c")).expect("checked rule"),
    ]);
    let f_id = RuleId::from_rule(&rules.rules()[0]);
    let g_id = RuleId::from_rule(&rules.rules()[1]);
    let source = parse("f(a)");
    let goal = parse("c");
    let derivation = BidirectionalDerivation {
        forward_steps: Vec::new(),
        backward_steps: vec![
            recorded(parse("g(Z)"), g_id, &goal, 1),
            recorded(parse("f(Z)"), f_id, &parse("g(Z)"), 2),
        ],
        meeting_forward: source.clone(),
        meeting_backward: source.clone(),
        unifier: Substitution::new(),
    };
    let certificate = ReplayCertificate::issue(
        &rules,
        ReplayQuery::Bidirectional {
            source: source.clone(),
            goal,
        },
        ReplayDerivation::Bidirectional(derivation),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        observation(),
    );
    let error = certificate
        .verify(&rules)
        .expect_err("a conflicting binding for one recorded variable must not verify");
    assert!(
        error.to_string().contains("recorded backward term"),
        "the typed rejection names the recorded backward term, got {error}"
    );
}

/// F8 positive control: genuine generalization is accepted when ONE
/// substitution instantiates the whole recorded trace and source consistently.
///
/// Rules `f(Z) -> g(Z)` and `g(Z) -> c`; backward from `c` the recorded half is
/// `[g(Z), f(Z)]` sharing `Z`, and the replay from `f(a)` instantiates `Z = a`
/// once for every record and the goal.
#[test]
fn bidirectional_replay_admits_one_consistent_instantiation() {
    let rules = system(vec![
        Rule::new(parse("f(Z)"), parse("g(Z)")).expect("checked rule"),
        Rule::new(parse("g(Z)"), parse("c")).expect("checked rule"),
    ]);
    let f_id = RuleId::from_rule(&rules.rules()[0]);
    let g_id = RuleId::from_rule(&rules.rules()[1]);
    let source = parse("f(a)");
    let goal = parse("c");
    let derivation = BidirectionalDerivation {
        forward_steps: Vec::new(),
        backward_steps: vec![
            recorded(parse("g(Z)"), g_id, &goal, 1),
            recorded(parse("f(Z)"), f_id, &parse("g(Z)"), 2),
        ],
        meeting_forward: source.clone(),
        meeting_backward: source.clone(),
        unifier: Substitution::new(),
    };
    let certificate = ReplayCertificate::issue(
        &rules,
        ReplayQuery::Bidirectional {
            source: source.clone(),
            goal,
        },
        ReplayDerivation::Bidirectional(derivation),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        observation(),
    );
    certificate
        .verify(&rules)
        .expect("one consistent instantiation of the recorded trace must verify");
}
