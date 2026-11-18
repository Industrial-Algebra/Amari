// SPDX-License-Identifier: MIT OR Apache-2.0

//! Normalized first-order constraint sets.
//!
//! Normalization solves equalities through the deterministic unifier,
//! applies the resulting substitution to disequalities, drops
//! tautologies, detects contradictions, and retains canonical
//! residuals. Canonical order and side order use the alpha-canonical
//! encoding, so results are insertion-order- and name-independent.

use alloc::vec::Vec;

use crate::analysis::unify_with;
use crate::error::RewriteResult;
use crate::relation::{RelationLimits, RelationResources, Sha256Digest};
use crate::trs::{Substitution, Term};

/// A first-order term constraint.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub enum TermConstraint {
    /// Two terms must be equal.
    Equal(Term, Term),
    /// Two terms must remain distinct.
    NotEqual(Term, Term),
}

impl TermConstraint {
    /// Canonical alpha-invariant digest: the constraint is encoded as
    /// one structure so variables renumber consistently across both
    /// sides.
    pub fn canonical_digest(&self) -> Sha256Digest {
        let mut variables = Vec::new();
        let mut encoding = Vec::new();
        match self {
            Self::Equal(left, right) => {
                encoding.push(0x10);
                crate::relation::digest::encode_term(left, &mut variables, &mut encoding);
                crate::relation::digest::encode_term(right, &mut variables, &mut encoding);
            }
            Self::NotEqual(left, right) => {
                encoding.push(0x11);
                crate::relation::digest::encode_term(left, &mut variables, &mut encoding);
                crate::relation::digest::encode_term(right, &mut variables, &mut encoding);
            }
        }
        Sha256Digest::framed("amari.relation.constraint/v1", &encoding)
    }

    fn canonical_side_order(self) -> Self {
        match self {
            Self::NotEqual(left, right) => {
                let mut variables = Vec::new();
                let mut left_bytes = Vec::new();
                crate::relation::digest::encode_term(&left, &mut variables, &mut left_bytes);
                let mut variables = Vec::new();
                let mut right_bytes = Vec::new();
                crate::relation::digest::encode_term(&right, &mut variables, &mut right_bytes);
                if right_bytes < left_bytes {
                    Self::NotEqual(right, left)
                } else {
                    Self::NotEqual(left, right)
                }
            }
            other => other,
        }
    }
}

/// The result of normalizing a constraint set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConstraintOutcome {
    /// Equalities solved; residuals are canonical disequalities.
    Satisfiable {
        /// The solving substitution.
        substitution: Substitution,
        /// Canonical residual disequalities (alpha-canonically
        /// ordered, sides canonicalized).
        residuals: Vec<TermConstraint>,
    },
    /// The set is contradictory.
    Unsatisfiable,
    /// A constraint requires a theory this engine does not support.
    /// Reserved for theory constraints (SMT bridge cohort); no term
    /// constraint is ever unsupported.
    UnsupportedTheory,
}

/// A set of first-order term constraints.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct ConstraintSet {
    constraints: Vec<TermConstraint>,
}

impl ConstraintSet {
    /// An empty constraint set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a constraint.
    pub fn insert(&mut self, constraint: TermConstraint) {
        self.constraints.push(constraint);
    }

    /// Borrow the raw constraints (insertion order).
    pub fn constraints(&self) -> &[TermConstraint] {
        &self.constraints
    }

    /// Number of constraints.
    pub fn len(&self) -> usize {
        self.constraints.len()
    }

    /// Canonical alpha-invariant digest of the WHOLE set.
    ///
    /// Every constraint is encoded under one variable-renaming environment
    /// shared across the set, so variables are renumbered by first structural
    /// occurrence over the whole set (preorder, in set order) and
    /// co-occurrence relationships are part of the identity. Per-constraint
    /// digests cannot express the identity of a *set*: each restarts
    /// renumbering at zero, so `{X != a, X != b}` and `{X != a, Y != b}` —
    /// different sets, because only the first shares one variable across its
    /// two constraints — hashed identically (PR #357 round 3 P2a).
    ///
    /// Alpha-equivalent sets still share a digest: a consistently renamed set
    /// produces the same index sequence. The frame is distinct from the
    /// single-constraint frame, so a set digest never stands in for one.
    pub fn canonical_digest(&self) -> Sha256Digest {
        let mut variables = Vec::new();
        let mut encoding = Vec::new();
        encoding.extend_from_slice(&(self.constraints.len() as u32).to_le_bytes());
        for constraint in &self.constraints {
            match constraint {
                TermConstraint::Equal(left, right) => {
                    encoding.push(0x10);
                    crate::relation::digest::encode_term(left, &mut variables, &mut encoding);
                    crate::relation::digest::encode_term(right, &mut variables, &mut encoding);
                }
                TermConstraint::NotEqual(left, right) => {
                    encoding.push(0x11);
                    crate::relation::digest::encode_term(left, &mut variables, &mut encoding);
                    crate::relation::digest::encode_term(right, &mut variables, &mut encoding);
                }
            }
        }
        Sha256Digest::framed("amari.relation.constraints/v1", &encoding)
    }

    /// True when empty.
    pub fn is_empty(&self) -> bool {
        self.constraints.is_empty()
    }

    /// Normalize: solve equalities, apply the substitution, drop
    /// tautologies, detect contradictions, and retain canonical
    /// disequality residuals.
    pub fn normalize(&self, limits: &RelationLimits) -> RewriteResult<ConstraintOutcome> {
        let mut resources = RelationResources::new(limits);
        resources.record_constraints(self.constraints.len())?;

        let mut substitution = Substitution::new();
        let mut disequalities: Vec<(Term, Term)> = Vec::new();
        for constraint in &self.constraints {
            match constraint {
                TermConstraint::Equal(left, right) => {
                    if unify_with(&mut substitution, left, right, &mut resources).is_err() {
                        return Ok(ConstraintOutcome::Unsatisfiable);
                    }
                }
                TermConstraint::NotEqual(left, right) => {
                    disequalities.push((left.clone(), right.clone()));
                }
            }
        }

        let mut residuals: Vec<TermConstraint> = Vec::new();
        for (left, right) in disequalities {
            let left = substitution.apply(&left);
            let right = substitution.apply(&right);
            if left == right {
                return Ok(ConstraintOutcome::Unsatisfiable);
            }
            // Structural tautology check: sides that can never unify
            // (disjoint heads/arity at every corresponding position)
            // make the disequality vacuously true. This is a
            // conservative syntactic check — it never binds variables
            // and never spends the operation budget on probes.
            if definitely_distinct(&left, &right) {
                continue;
            }
            residuals.push(TermConstraint::NotEqual(left, right).canonical_side_order());
        }
        residuals.sort_by_key(TermConstraint::canonical_digest);
        residuals.dedup();
        Ok(ConstraintOutcome::Satisfiable {
            substitution,
            residuals,
        })
    }
}

/// Conservative structural check: true when the terms can never unify
/// (disjoint symbol heads or arities at a corresponding position).
/// Either side being a variable means "not definitely distinct".
fn definitely_distinct(left: &Term, right: &Term) -> bool {
    match (left, right) {
        (Term::Sym(f, f_args), Term::Sym(g, g_args)) => {
            f != g
                || f_args.len() != g_args.len()
                || f_args
                    .iter()
                    .zip(g_args.iter())
                    .any(|(f_arg, g_arg)| definitely_distinct(f_arg, g_arg))
        }
        _ => false,
    }
}
