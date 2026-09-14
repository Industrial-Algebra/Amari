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
/// the fields are private so callers cannot forge exhaustion.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
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

/// Evidence for an approximate (non-exact) search result.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct ApproximateSearchEvidence {
    /// What approximation was used and why.
    pub summary: String,
    /// States explored before the approximation was produced.
    pub explored_states: u64,
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
