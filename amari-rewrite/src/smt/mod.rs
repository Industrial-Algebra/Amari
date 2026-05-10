//! Experimental SMT/solver integration traits.
//!
//! This module defines solver-facing interfaces without committing 0.23.0 to a
//! concrete SMT backend. Future implementations can use these traits for
//! equivalence proofs, rule validation, or solver-backed synthesis.

/// Interface for solver-backed rewrite validation.
pub trait RewriteSolver {
    /// Solver term representation.
    type Term;
    /// Proof or validation certificate.
    type Certificate;
    /// Solver error.
    type Error;

    /// Prove or validate equivalence of two terms.
    fn prove_equivalent(
        &self,
        lhs: &Self::Term,
        rhs: &Self::Term,
    ) -> Result<Self::Certificate, Self::Error>;
}
