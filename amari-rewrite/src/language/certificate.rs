// SPDX-License-Identifier: MIT OR Apache-2.0

//! Evidence certificates for preimage operations (ADR 0001).
//!
//! A [`PreimageCertificate`] binds a preimage operation to its system,
//! language, classifier verdict, construction, limits, and — once
//! [`PreimageCertificate::complete`] is called — the exact result
//! language. All bindings are framed SHA-256 digests; this is a
//! content-binding scheme, not a cryptographic commitment scheme
//! (explicitly out of scope per the closure matrix). Certificates are
//! issued ONLY for class/operation cells the ADR approves as exact;
//! approximation-only cells are hard `UnsupportedPreimage` errors.
//!
//! ## Trust model
//!
//! Verification establishes everything decidable WITHOUT re-running
//! the construction: input/limit bindings, capability-table
//! consistency, alphabet compatibility, and the constructions whose
//! result is determined by the inputs alone (`Identity`; any operation
//! over an empty system). For a non-`Identity` construction over a
//! non-empty system, the correctness of the bound result is the
//! responsibility of the in-crate construction that produced it —
//! [`PreimageCertificate::complete`] is crate-visible, so completed
//! `Exact` evidence can originate only from those trusted
//! constructions (Tasks 25-26). A certificate is content-bound
//! evidence, not authentication.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::error::{RewriteError, RewriteResult};
use crate::language::classify::{
    classify_system, validate_alphabet, PreimageCapability, PreimageConstruction,
    PreimageOperation, TrsClass,
};
use crate::language::TreeAutomaton;
use crate::relation::{RelationLimits, Sha256Digest};
use crate::trs::TermSystem;

/// Digest frame for individual terms inside certificate bindings.
const TERM_FRAME: &str = "amari-rewrite/preimage/term/v1";
/// Digest frame for individual rules inside certificate bindings.
const RULE_FRAME: &str = "amari-rewrite/preimage/rule/v1";
/// Digest frame for a whole rule system.
const SYSTEM_FRAME: &str = "amari-rewrite/preimage/system/v1";
/// Digest frame for a language automaton.
const LANGUAGE_FRAME: &str = "amari-rewrite/preimage/language/v1";

/// Bind a term rewriting system. Order-insensitive over rules: the
/// rewriting relation does not depend on rule order.
pub fn system_digest(system: &TermSystem) -> Sha256Digest {
    let mut rule_digests: Vec<[u8; 32]> = system
        .rules()
        .iter()
        .map(|rule| {
            let left = Sha256Digest::canonical_term(TERM_FRAME, rule.lhs());
            let right = Sha256Digest::canonical_term(TERM_FRAME, rule.rhs());
            let mut payload = Vec::with_capacity(64);
            payload.extend_from_slice(left.as_bytes());
            payload.extend_from_slice(right.as_bytes());
            *Sha256Digest::framed(RULE_FRAME, &payload).as_bytes()
        })
        .collect();
    rule_digests.sort();
    let mut payload = Vec::with_capacity(32 * rule_digests.len());
    for digest in &rule_digests {
        payload.extend_from_slice(digest);
    }
    Sha256Digest::framed(SYSTEM_FRAME, &payload)
}

/// Bind a language automaton by its canonical encoding.
pub fn language_digest(language: &TreeAutomaton) -> Sha256Digest {
    Sha256Digest::framed(LANGUAGE_FRAME, &language.canonical_bytes())
}

/// The authority level a certificate asserts.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub enum CertificateAuthority {
    /// The construction completed under the recorded limits; the
    /// certificate binds an exact result language.
    Exact,
    /// Reserved for Tasks 26-27: a partial frontier with the named
    /// exhausted resource. `PreimageCertificate::issue` never produces
    /// this variant.
    Partial {
        /// The exhausted resource or boundary detail.
        detail: String,
    },
}

/// A certificate binding a preimage operation to its system, language,
/// class, construction, limits, and (once completed) result language.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct PreimageCertificate {
    operation: PreimageOperation,
    class: TrsClass,
    construction: PreimageConstruction,
    system: Sha256Digest,
    language: Sha256Digest,
    rule_count: usize,
    horizon: Option<u32>,
    max_term_nodes: usize,
    max_term_depth: usize,
    max_constraints: usize,
    max_operations: usize,
    authority: CertificateAuthority,
    result: Option<Sha256Digest>,
}

impl PreimageCertificate {
    /// Issue an exact certificate for a validated system/language pair.
    ///
    /// Re-runs the ADR 0001 input contract (rule containment via
    /// [`classify_system`], common alphabet via [`validate_alphabet`])
    /// and refuses — [`RewriteError::UnsupportedPreimage`] — any
    /// class/operation whose capability is `ApproximationOnly`. The
    /// issued certificate has authority `Exact` and no result yet.
    pub fn issue(
        operation: PreimageOperation,
        system: &TermSystem,
        language: &TreeAutomaton,
        limits: &RelationLimits,
    ) -> RewriteResult<Self> {
        let classification = classify_system(system)?;
        validate_alphabet(system, language)?;
        let construction = match classification.capability(operation) {
            PreimageCapability::Exact(construction) => construction,
            PreimageCapability::ApproximationOnly => {
                return Err(RewriteError::UnsupportedPreimage {
                    message: format!(
                        "class {:?} has no approved exact construction for {:?} (ADR 0001)",
                        classification.class(),
                        operation
                    ),
                });
            }
        };
        let horizon = match operation {
            PreimageOperation::FiniteHorizon(bound) => Some(bound),
            _ => None,
        };
        Ok(Self {
            operation,
            class: classification.class(),
            construction,
            system: system_digest(system),
            language: language_digest(language),
            rule_count: classification.rule_count(),
            horizon,
            max_term_nodes: limits.max_term_nodes(),
            max_term_depth: limits.max_term_depth(),
            max_constraints: limits.max_constraints(),
            max_operations: limits.max_operations(),
            authority: CertificateAuthority::Exact,
            result: None,
        })
    }

    /// Attach a claimed result language, binding its digest.
    ///
    /// Crate-visible only: a completed `Exact` certificate is trusted
    /// construction evidence, and the only trusted constructions are
    /// the in-crate preimage operations (Tasks 25-26). External code
    /// can issue pending certificates and verify them, but cannot mint
    /// completed exact evidence. Verification remains the semantic
    /// authority for everything decidable without re-running the
    /// construction (bindings, capability consistency, Identity and
    /// empty-system result semantics).
    // Minted by the Task 25/26 construction code; until then only the
    // in-crate unit tests exercise it.
    #[allow(dead_code)]
    pub(crate) fn complete(mut self, result: &TreeAutomaton) -> Self {
        self.result = Some(language_digest(result));
        self
    }

    /// Recompute and compare every binding: the system, language,
    /// and limits digests, plus the certificate's internal
    /// consistency — the class must match a fresh classification of
    /// the bound system, the construction must be exactly the
    /// approved capability for the class and operation, the horizon
    /// and authority fields must be coherent, and an `Identity`
    /// construction may bind only the input language as its result.
    /// Tampered metadata (including deserialized certificates) fails.
    /// Only `Exact` authority is verifiable evidence in this
    /// revision; `Partial` is reserved for Tasks 26-27.
    pub fn verify(
        &self,
        system: &TermSystem,
        language: &TreeAutomaton,
        limits: &RelationLimits,
    ) -> bool {
        if validate_alphabet(system, language).is_err() {
            return false;
        }
        self.system == system_digest(system)
            && self.language == language_digest(language)
            && self.max_term_nodes == limits.max_term_nodes()
            && self.max_term_depth == limits.max_term_depth()
            && self.max_constraints == limits.max_constraints()
            && self.max_operations == limits.max_operations()
            && self.verify_consistency(system)
    }

    /// Internal consistency against the ADR 0001 capability table.
    fn verify_consistency(&self, system: &TermSystem) -> bool {
        let Ok(classification) = classify_system(system) else {
            return false;
        };
        if classification.class() != self.class || classification.rule_count() != self.rule_count {
            return false;
        }
        if classification.capability(self.operation) != PreimageCapability::Exact(self.construction)
        {
            return false;
        }
        let horizon_ok = match self.operation {
            PreimageOperation::FiniteHorizon(bound) => self.horizon == Some(bound),
            _ => self.horizon.is_none(),
        };
        let construction_ok = match (self.operation, self.construction) {
            (PreimageOperation::FiniteHorizon(0), PreimageConstruction::Identity) => true,
            (
                PreimageOperation::FiniteHorizon(bound),
                PreimageConstruction::FiniteHorizonIteration(recorded),
            ) => bound >= 1 && bound == recorded,
            (PreimageOperation::OneStep, PreimageConstruction::LeftLinearOneStep) => true,
            (PreimageOperation::Saturation, PreimageConstruction::GttSaturation) => true,
            _ => false,
        };
        let authority_ok = matches!(self.authority, CertificateAuthority::Exact);
        horizon_ok && construction_ok && authority_ok && self.identity_result_ok()
    }

    /// An `Identity` construction's result is the input language
    /// itself; any other bound result is inconsistent.
    fn identity_result_ok(&self) -> bool {
        self.construction != PreimageConstruction::Identity
            || self.result.is_none_or(|result| result == self.language)
    }

    /// Recompute and compare the result binding, including the
    /// construction semantics decidable without re-running the
    /// construction: an `Identity` result must be the input language,
    /// and for an EMPTY system the one-step preimage must be the
    /// empty language while every other operation is the identity.
    /// False when the certificate carries no result. Full validation
    /// of a completed certificate is `verify(...) && verify_result(...)`;
    /// the empty-system checks trust the bound `rule_count`, which
    /// `verify` revalidates against the system.
    pub fn verify_result(&self, result: &TreeAutomaton) -> bool {
        self.result == Some(language_digest(result))
            && self.identity_result_ok()
            && self.empty_system_result_ok(result)
    }

    /// Decidable result semantics for systems with no rules.
    fn empty_system_result_ok(&self, result: &TreeAutomaton) -> bool {
        if self.rule_count != 0 {
            return true;
        }
        match self.construction {
            // No rules: no application steps, so the exact one-step
            // preimage of any language is empty.
            PreimageConstruction::LeftLinearOneStep => result.language_is_empty(),
            // Horizon unions and saturation include zero steps.
            PreimageConstruction::Identity
            | PreimageConstruction::FiniteHorizonIteration(_)
            | PreimageConstruction::GttSaturation => self.result == Some(self.language),
        }
    }

    /// The preimage operation.
    pub fn operation(&self) -> PreimageOperation {
        self.operation
    }

    /// The classifier verdict.
    pub fn class(&self) -> TrsClass {
        self.class
    }

    /// The approved construction.
    pub fn construction(&self) -> PreimageConstruction {
        self.construction
    }

    /// The bound system digest.
    pub fn system(&self) -> Sha256Digest {
        self.system
    }

    /// The bound language digest.
    pub fn language(&self) -> Sha256Digest {
        self.language
    }

    /// The horizon bound, when the operation is `FiniteHorizon`.
    pub fn horizon(&self) -> Option<u32> {
        self.horizon
    }

    /// The authority level.
    pub fn authority(&self) -> &CertificateAuthority {
        &self.authority
    }

    /// The bound result digest, once completed.
    pub fn result(&self) -> Option<Sha256Digest> {
        self.result
    }

    /// Whether a result language has been bound.
    pub fn is_complete(&self) -> bool {
        self.result.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::{RankedSymbol, TreeAutomatonLimits, TreeState, TreeTransition};
    use crate::trs::{Rule, Symbol, Term};
    use alloc::vec;

    fn a() -> Term {
        Term::constant("a")
    }

    fn f(left: Term, right: Term) -> Term {
        Term::sym("f", [left, right])
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

    fn universal_automaton() -> TreeAutomaton {
        let q = TreeState::new("q");
        TreeAutomaton::new(
            vec![
                RankedSymbol::new(Symbol::new("a"), 0),
                RankedSymbol::new(Symbol::new("g"), 1),
                RankedSymbol::new(Symbol::new("f"), 2),
            ],
            vec![q.clone()],
            vec![
                TreeTransition::new(Symbol::new("a"), vec![], q.clone()),
                TreeTransition::new(Symbol::new("g"), vec![q.clone()], q.clone()),
                TreeTransition::new(Symbol::new("f"), vec![q.clone(), q.clone()], q.clone()),
            ],
            vec![q],
            TreeAutomatonLimits::default(),
        )
        .expect("fixture automaton is valid")
    }

    /// A different automaton (alphabet {a/0, g/1} only).
    fn small_automaton() -> TreeAutomaton {
        let q = TreeState::new("q");
        TreeAutomaton::new(
            vec![
                RankedSymbol::new(Symbol::new("a"), 0),
                RankedSymbol::new(Symbol::new("g"), 1),
            ],
            vec![q.clone()],
            vec![
                TreeTransition::new(Symbol::new("a"), vec![], q.clone()),
                TreeTransition::new(Symbol::new("g"), vec![q.clone()], q.clone()),
            ],
            vec![q],
            TreeAutomatonLimits::default(),
        )
        .expect("fixture automaton is valid")
    }

    /// An automaton over {a/0} with no transitions: the empty language.
    fn empty_language() -> TreeAutomaton {
        let q = TreeState::new("q");
        TreeAutomaton::new(
            vec![RankedSymbol::new(Symbol::new("a"), 0)],
            vec![q],
            vec![],
            vec![],
            TreeAutomatonLimits::default(),
        )
        .expect("fixture automaton is valid")
    }

    /// Round-2 P1: with an EMPTY system the exact one-step preimage is
    /// the empty language; a completed certificate claiming the
    /// universal language must not verify. Saturation/horizon of an
    /// empty system is the identity, so the universal language IS a
    /// valid result there.
    #[test]
    fn empty_system_result_semantics_are_enforced() {
        let system = TermSystem::new(vec![]);
        let language = universal_automaton();
        let limits = RelationLimits::default();

        let one_step =
            PreimageCertificate::issue(PreimageOperation::OneStep, &system, &language, &limits)
                .expect("ground one-step is approved");
        assert!(!one_step
            .clone()
            .complete(&language)
            .verify_result(&language));
        assert!(one_step
            .complete(&empty_language())
            .verify_result(&empty_language()));

        let saturation =
            PreimageCertificate::issue(PreimageOperation::Saturation, &system, &language, &limits)
                .expect("ground saturation is approved");
        assert!(saturation
            .clone()
            .complete(&language)
            .verify_result(&language));
        assert!(!saturation
            .complete(&empty_language())
            .verify_result(&empty_language()));
    }

    #[test]
    fn complete_and_verify_round_trip() {
        let system = TermSystem::new(vec![Rule::new(a(), g(a())).unwrap()]);
        let language = universal_automaton();
        let limits = RelationLimits::default();
        let certificate =
            PreimageCertificate::issue(PreimageOperation::Saturation, &system, &language, &limits)
                .expect("ground saturation is approved");
        assert_eq!(certificate.class(), TrsClass::Ground);
        assert_eq!(
            certificate.construction(),
            PreimageConstruction::GttSaturation
        );

        let certificate = certificate.complete(&language);
        assert!(certificate.is_complete());
        assert!(certificate.verify(&system, &language, &limits));
        assert!(certificate.verify_result(&language));

        // A different system does not verify.
        let other_system = TermSystem::new(vec![
            Rule::new(a(), g(a())).unwrap(),
            Rule::new(g(a()), a()).unwrap(),
        ]);
        assert!(!certificate.verify(&other_system, &language, &limits));

        // A different result does not verify.
        assert!(!certificate.verify_result(&small_automaton()));

        // Tightened limits do not verify.
        let tighter = RelationLimits::new(4096, 64, limits.max_constraints(), 500_000)
            .expect("valid tightened limits");
        assert!(!certificate.verify(&system, &language, &tighter));
    }

    #[test]
    fn identity_completion_binds_only_the_input_language() {
        let system = TermSystem::new(vec![Rule::new(f(x(), x()), a()).unwrap()]);
        let language = universal_automaton();
        let limits = RelationLimits::default();
        let certificate = PreimageCertificate::issue(
            PreimageOperation::FiniteHorizon(0),
            &system,
            &language,
            &limits,
        )
        .expect("horizon zero is exact for every class");

        // The Identity construction's result is the input language: a
        // different bound language is inconsistent and must not verify.
        let mismatched = certificate.clone().complete(&small_automaton());
        assert!(!mismatched.verify(&system, &language, &limits));
        assert!(!mismatched.verify_result(&small_automaton()));

        let matched = certificate.complete(&language);
        assert!(matched.verify(&system, &language, &limits));
        assert!(matched.verify_result(&language));
    }

    #[cfg(feature = "serialize")]
    #[test]
    fn tampered_metadata_fails_verification() {
        let system = TermSystem::new(vec![Rule::new(g(x()), f(x(), x())).unwrap()]);
        let language = universal_automaton();
        let limits = RelationLimits::default();
        let certificate =
            PreimageCertificate::issue(PreimageOperation::OneStep, &system, &language, &limits)
                .expect("left-linear one-step is approved")
                .complete(&language);
        assert!(certificate.verify(&system, &language, &limits));

        let mut wire = serde_json::to_value(&certificate).expect("serializes");
        // Tamper: promotion to an ApproximationOnly cell.
        wire["operation"] = serde_json::json!("Saturation");
        wire["construction"] = serde_json::json!("GttSaturation");
        let tampered: PreimageCertificate =
            serde_json::from_value(wire.clone()).expect("deserializes");
        assert!(!tampered.verify(&system, &language, &limits));

        // Tamper: horizon metadata inconsistent with the operation.
        wire["operation"] = serde_json::to_value(PreimageOperation::OneStep).unwrap();
        wire["construction"] =
            serde_json::to_value(PreimageConstruction::LeftLinearOneStep).unwrap();
        wire["horizon"] = serde_json::json!(5);
        let tampered: PreimageCertificate =
            serde_json::from_value(wire.clone()).expect("deserializes");
        assert!(!tampered.verify(&system, &language, &limits));

        // Tamper: class field disagrees with the bound system.
        wire["horizon"] = serde_json::json!(null);
        wire["class"] = serde_json::json!("Ground");
        let tampered: PreimageCertificate =
            serde_json::from_value(wire.clone()).expect("deserializes");
        assert!(!tampered.verify(&system, &language, &limits));

        // Tamper: downgraded authority is not exact evidence.
        wire["class"] = serde_json::json!("LeftLinearShared");
        wire["authority"] = serde_json::json!({"Partial": {"detail": "operations"}});
        let tampered: PreimageCertificate =
            serde_json::from_value(wire.clone()).expect("deserializes");
        assert!(!tampered.verify(&system, &language, &limits));
    }

    #[test]
    fn contract_violating_system_fails_verification() {
        let system = TermSystem::new(vec![Rule::new(g(x()), f(x(), x())).unwrap()]);
        let language = universal_automaton();
        let limits = RelationLimits::default();
        let certificate =
            PreimageCertificate::issue(PreimageOperation::OneStep, &system, &language, &limits)
                .expect("approved cell")
                .complete(&language);
        // Same digest requires the same rules; a contract-violating
        // system additionally fails classification re-derivation.
        let violating = TermSystem::new(vec![Rule::new_unchecked(g(x()), y())]);
        assert!(!certificate.verify(&violating, &language, &limits));
    }
}
