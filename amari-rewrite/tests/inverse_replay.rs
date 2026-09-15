//! Replay certificate binding/hardening tests
//! (0.25 Cohort 3, Task 16).

use amari_rewrite::inverse::{
    BackwardExplorer, BackwardSearchOutcome, GuidanceMode, InverseSearchConfig, ReplayCertificate,
    ReplayDerivation, ReplayQuery, ResourceObservation, SearchMode,
};
use amari_rewrite::trs::{Rule, Term, TermSystem};

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
