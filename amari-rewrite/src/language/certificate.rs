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
            horizon,
            max_term_nodes: limits.max_term_nodes(),
            max_term_depth: limits.max_term_depth(),
            max_constraints: limits.max_constraints(),
            max_operations: limits.max_operations(),
            authority: CertificateAuthority::Exact,
            result: None,
        })
    }

    /// Attach a claimed result language, binding its digest. Binding
    /// is deliberately infallible: [`PreimageCertificate::verify`]
    /// and [`PreimageCertificate::verify_result`] are the semantic
    /// authority (a deserialized certificate bypasses this method),
    /// and they reject a result inconsistent with the construction —
    /// e.g. `Identity` binds only the input language.
    pub fn complete(mut self, result: &TreeAutomaton) -> Self {
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
        if classification.class() != self.class {
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
            || self.result.map_or(true, |result| result == self.language)
    }

    /// Recompute and compare the result binding. False when the
    /// certificate carries no result, and false for an `Identity`
    /// construction whose result is not the input language.
    pub fn verify_result(&self, result: &TreeAutomaton) -> bool {
        self.result == Some(language_digest(result)) && self.identity_result_ok()
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
