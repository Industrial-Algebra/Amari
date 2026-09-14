//! Bounded inverse rewriting for term rewriting systems.
//!
//! Inverse rewriting is predecessor generation, not a true functional inverse.
//! A rule `lhs -> rhs` is explored backward by matching `rhs` against a
//! subterm and instantiating `lhs` with the same substitution.

use alloc::collections::{BTreeSet, VecDeque};
use alloc::vec::Vec;

mod config;
mod outcome;
mod state;

pub use config::InverseSearchConfig;
pub use outcome::{
    ApproximateSearchEvidence, BackwardDerivation, BackwardFrontier, BackwardSearchOutcome,
    CertifiedExhaustion, ExhaustionAuthority, UnsupportedRelation,
};
pub use state::{SearchResources, SymbolicState};

use crate::analysis::unify;
use crate::error::RewriteResult;
use crate::relation::{
    BackwardClause, ConstraintSet, LogicVar, LogicVarNamespace, RelationLimits, RelationResources,
    RuleId, Sha256Digest,
};
use crate::rewritable::Path;
use crate::trs::{match_pattern, Substitution, Term, TermSystem};

/// Convenience helper returning all bounded predecessors as a vector.
pub fn predecessors(system: &TermSystem, target: Term, max_depth: usize) -> alloc::vec::Vec<Term> {
    BackwardSearch::new(system, target)
        .max_depth(max_depth)
        .collect()
}

/// Bounded breadth-first backward search over a `TermSystem`.
pub struct BackwardSearch<'a> {
    system: &'a TermSystem,
    queue: VecDeque<(Term, usize)>,
    visited: BTreeSet<Term>,
    max_depth: usize,
    max_nodes: usize,
    emitted: usize,
}

impl<'a> BackwardSearch<'a> {
    /// Create a search rooted at `target`.
    pub fn new(system: &'a TermSystem, target: Term) -> Self {
        let mut queue = VecDeque::new();
        let mut visited = BTreeSet::new();
        visited.insert(target.clone());
        queue.push_back((target, 0));

        Self {
            system,
            queue,
            visited,
            max_depth: 1,
            max_nodes: 1024,
            emitted: 0,
        }
    }

    /// Set the maximum backward depth.
    pub fn max_depth(mut self, max_depth: usize) -> Self {
        self.max_depth = max_depth;
        self
    }

    /// Set the maximum number of emitted predecessor nodes.
    pub fn max_nodes(mut self, max_nodes: usize) -> Self {
        self.max_nodes = max_nodes;
        self
    }

    fn one_step_predecessors(&self, term: &Term) -> alloc::vec::Vec<Term> {
        let mut out = alloc::vec::Vec::new();
        for path in term.positions() {
            let Some(subterm) = term.subterm(&path) else {
                continue;
            };

            for rule in self.system.rules() {
                if let Some(subst) = match_pattern(rule.rhs(), subterm) {
                    let predecessor_subterm = subst.apply(rule.lhs());
                    if let Ok(predecessor) = term.replace_at(&path, predecessor_subterm) {
                        if predecessor != *term {
                            out.push(predecessor);
                        }
                    }
                }
            }
        }
        out
    }
}

impl Iterator for BackwardSearch<'_> {
    type Item = Term;

    fn next(&mut self) -> Option<Self::Item> {
        if self.emitted >= self.max_nodes {
            return None;
        }

        while let Some((term, depth)) = self.queue.pop_front() {
            if depth >= self.max_depth {
                continue;
            }

            for predecessor in self.one_step_predecessors(&term) {
                if self.visited.insert(predecessor.clone()) {
                    self.queue.push_back((predecessor.clone(), depth + 1));
                    self.emitted += 1;
                    return Some(predecessor);
                }
            }
        }

        None
    }
}

/// Provenance of one symbolic backward step: stable rule identity,
/// target position, freshening scope, and authority hashes. Contains
/// no source text, filesystem paths, or backend diagnostics.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct BackwardProvenance {
    /// The checked rule's stable identity.
    pub rule_id: RuleId,
    /// Position of the replaced subterm in the target.
    pub position: Path,
    /// Freshening namespace scope of the query.
    pub scope: u32,
    /// Canonical digest of the target term.
    pub target_hash: Sha256Digest,
    /// Canonical digest of the predecessor term.
    pub predecessor_hash: Sha256Digest,
}

/// One symbolic backward transition: a predecessor term together with
/// its existentials, constraints, unifying substitution, provenance,
/// and the resources consumed while deriving it.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct SymbolicPredecessor {
    /// The predecessor term (with freshened logic variables).
    pub term: Term,
    /// Existential logic variables (LHS variables erased forward).
    pub existentials: Vec<LogicVar>,
    /// Constraints on this predecessor (empty until grounding and
    /// bidirectional analysis introduce them).
    pub constraints: ConstraintSet,
    /// The unifying substitution over freshened variables.
    pub substitution: Substitution,
    /// Step provenance.
    pub provenance: BackwardProvenance,
    /// Resource usage snapshot at the point this predecessor was
    /// derived.
    pub resources: RelationResources,
}

/// Symbolic one-step predecessors of `target` under `system`.
///
/// Enumerates target positions in preorder (position-major) and rules
/// in system order; each attempt freshens its clause from one
/// query-scoped namespace, so results are deterministic per
/// `(system, target, scope)`. Self-loop predecessors (equal to the
/// target) are excluded. Unification clashes skip the attempt;
/// resource breaches are typed errors.
pub fn symbolic_predecessors(
    system: &TermSystem,
    target: &Term,
    limits: &RelationLimits,
    scope: u32,
) -> RewriteResult<alloc::vec::Vec<SymbolicPredecessor>> {
    let mut resources = RelationResources::new(limits);
    resources.record_term(term_nodes(target), term_depth(target))?;
    let mut namespace = LogicVarNamespace::new(scope);
    let target_hash = Sha256Digest::canonical_term("amari.relation.term/v1", target);
    let mut out = alloc::vec::Vec::new();
    for path in target.positions() {
        let Some(subterm) = target.subterm(&path) else {
            continue;
        };
        for rule in system.rules() {
            let clause = BackwardClause::compile(rule)?;
            let fresh = clause.freshen(&mut namespace);
            let substitution = match unify(&fresh.rhs, subterm, &mut resources) {
                Ok(substitution) => substitution,
                Err(crate::error::RewriteError::UnificationFailure { .. }) => continue,
                Err(error) => return Err(error),
            };
            let predecessor_subterm = substitution.apply(&fresh.lhs);
            let predecessor = target.replace_at(&path, predecessor_subterm)?;
            if predecessor == *target {
                continue;
            }
            let predecessor_hash =
                Sha256Digest::canonical_term("amari.relation.term/v1", &predecessor);
            out.push(SymbolicPredecessor {
                term: predecessor,
                existentials: fresh.erased.clone(),
                constraints: ConstraintSet::new(),
                substitution,
                provenance: BackwardProvenance {
                    rule_id: clause.rule_id(),
                    position: path.clone(),
                    scope,
                    target_hash,
                    predecessor_hash,
                },
                resources: resources.clone(),
            });
        }
    }
    Ok(out)
}

fn term_nodes(term: &Term) -> usize {
    match term {
        Term::Var(_) => 1,
        Term::Sym(_, args) => 1 + args.iter().map(term_nodes).sum::<usize>(),
    }
}

fn term_depth(term: &Term) -> usize {
    match term {
        Term::Var(_) => 1,
        Term::Sym(_, args) => 1 + args.iter().map(term_depth).max().unwrap_or(0),
    }
}
