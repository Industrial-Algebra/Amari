// SPDX-License-Identifier: MIT OR Apache-2.0

//! Typed backward-search outcomes.
//!
//! Outcomes are values, not booleans. `Exhausted` is only
//! constructible with certified finite/exact authority: the type has
//! no public constructor, so no caller can claim unreachability from
//! a depth, node, or operation ceiling — those are always `Partial`.

use alloc::string::String;
use alloc::vec::Vec;

use crate::inverse::{state::SymbolicState, SymbolicPredecessor};
use crate::relation::Sha256Digest;

/// A certified backward derivation: the predecessor chain witnessing
/// that the goal is reachable.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct BackwardDerivation {
    /// Ordered derivation steps, each with full provenance.
    pub steps: Vec<SymbolicPredecessor>,
}

/// The retained frontier when a search is cut by a budget.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct BackwardFrontier {
    /// Unexpanded states at the cut point (alpha-canonical).
    pub states: Vec<SymbolicState>,
    /// Depth reached when the budget cut the search.
    pub depth_reached: u64,
}

/// Certified finite/exact exhaustion authority. Constructible only
/// inside this crate with certified evidence (a fully enumerated
/// finite grounding domain, or an exact regular-language exclusion);
/// the fields are private so callers cannot forge exhaustion, and
/// deserialization is refused outright: a certificate is minted
/// only by crate-internal certification and is never accepted back
/// from callers (wire consumers read the probe DTOs instead).
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize))]
pub struct CertifiedExhaustion {
    authority: ExhaustionAuthority,
    evidence_hash: Sha256Digest,
}

/// The kind of exact authority behind a certified exhaustion.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum ExhaustionAuthority {
    /// A finite grounding domain was completely enumerated.
    FiniteGroundingDomain,
    /// An exact regular-language exclusion proof.
    RegularLanguageExclusion,
    /// The symbolic search frontier closed: every alpha-canonical
    /// state reachable by backward expansion was enumerated and
    /// expanded, with no candidate ever dropped by a cap.
    ClosedSymbolicSearch,
}

impl ExhaustionAuthority {
    /// Stable, versioned wire tag for this authority kind. These
    /// strings are part of the discovery probe contract; never
    /// expose Rust `Debug` spellings on the wire.
    pub fn wire_tag(&self) -> &'static str {
        match self {
            Self::FiniteGroundingDomain => "finite_grounding_domain",
            Self::RegularLanguageExclusion => "regular_language_exclusion",
            Self::ClosedSymbolicSearch => "closed_symbolic_search",
        }
    }
}

impl CertifiedExhaustion {
    /// Crate-internal certification. Task 14+ construct this only
    /// with the evidence named by `authority`.
    // The BackwardExplorer (Task 14) is the first production caller;
    // until then only the module test exercises certification.
    #[allow(dead_code)]
    pub(crate) fn certify(authority: ExhaustionAuthority, evidence: &[u8]) -> Self {
        Self {
            authority,
            evidence_hash: Sha256Digest::framed("amari.inverse.exhaustion/v1", evidence),
        }
    }

    /// The authority kind backing this exhaustion.
    pub fn authority(&self) -> &ExhaustionAuthority {
        &self.authority
    }

    /// Digest of the certified evidence.
    pub fn evidence_hash(&self) -> Sha256Digest {
        self.evidence_hash
    }
}

/// Evidence for an approximate (non-exact) search result. Carries
/// the dropped-candidate count and the scorer/config/guidance
/// hashes, so approximation authority is always inspectable. An
/// approximate outcome can never construct exhaustion or
/// unreachability certificates.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct ApproximateSearchEvidence {
    /// What approximation was used and why.
    pub summary: String,
    /// States explored before the approximation was produced.
    pub explored_states: u64,
    /// Candidates dropped by explicit heuristic pruning.
    pub dropped_candidates: u64,
    /// Digest of the scorer identity.
    pub scorer_hash: crate::relation::Sha256Digest,
    /// Digest of the search configuration.
    pub config_hash: crate::relation::Sha256Digest,
    /// Digest of the guidance mode.
    pub guidance_hash: crate::relation::Sha256Digest,
    /// The retained frontier at the cut, when a budget also fired.
    pub frontier: Option<BackwardFrontier>,
}

/// A relation the 0.25 engines do not support, with the reason.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct UnsupportedRelation {
    /// Why this relation is unsupported (never silent).
    pub reason: String,
}

/// Typed outcome of a bounded backward search.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub enum BackwardSearchOutcome {
    /// A certified derivation witnessing reachability.
    Witness(BackwardDerivation),
    /// Certified finite/exact exhaustion. Never constructible from
    /// budget exhaustion.
    Exhausted(CertifiedExhaustion),
    /// Budget cut the search; the retained frontier is returned.
    Partial(BackwardFrontier),
    /// An approximate result with its evidence.
    Approximate(ApproximateSearchEvidence),
    /// The relation is unsupported; the reason is carried.
    Unsupported(UnsupportedRelation),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn certified_exhaustion_carries_authority_and_evidence() {
        let certified = CertifiedExhaustion::certify(
            ExhaustionAuthority::FiniteGroundingDomain,
            b"domain:3-terms",
        );
        assert_eq!(
            certified.authority(),
            &ExhaustionAuthority::FiniteGroundingDomain
        );
        assert_eq!(certified.evidence_hash().as_bytes().len(), 32);
        let outcome = BackwardSearchOutcome::Exhausted(certified);
        assert!(matches!(outcome, BackwardSearchOutcome::Exhausted(_)));
    }
}

/// Certified exhaustion is never accepted from callers: any attempt
/// to deserialize it (including inside `BackwardSearchOutcome` or
/// `BidirectionalSearchOutcome`) is an error. This keeps the
/// `ClosedSymbolicSearch` authority unforgeable even with the
/// `serialize` feature enabled.
#[cfg(feature = "serialize")]
impl<'de> serde::Deserialize<'de> for CertifiedExhaustion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Consume the incoming value so the error is attributed to
        // this type rather than a syntax failure.
        let _ = serde::de::IgnoredAny::deserialize(deserializer)?;
        Err(serde::de::Error::custom(
            "CertifiedExhaustion cannot be deserialized: exhaustion \
             certificates are minted only inside amari-rewrite",
        ))
    }
}
