// SPDX-License-Identifier: MIT OR Apache-2.0

//! Deterministic first-order unification with occurs check.
//!
//! The worklist algorithm resolves both sides through the
//! accumulating substitution before every step, so bindings stay
//! idempotent by construction. Canonical binding order comes from the
//! substitution's `BTreeMap`; operation and term limits are enforced
//! through [`RelationResources`].
//!
//! ## Metered, borrowed traversal (F16, PR #384 round 4)
//!
//! Resolution and the occurs check never materialize an expansion. Both walk
//! the binding graph by reference, charge one operation per inspected node
//! against the caller's budget, and keep a per-walk visited set of variables, so
//! each reachable binding is inspected at most once. That is what bounds the
//! doubling chain `X20 |-> a`, `X19 |-> pair(X20,X20)`, ... : the old resolve
//! cloned a term fragment at every level of the unfold (millions of allocations
//! for a 21-link chain) and the old occurs check was recursive and unmetered,
//! unfolding 2,097,151 nodes for a single answer. A budget that cannot pay for
//! the walk is a typed [`RewriteError::RelationLimitExceeded`].
//!
//! Cycle semantics match [`Substitution::apply`]: a variable that re-enters its
//! own resolution is left in place, so `X |-> X` and `X |-> f(X)` resolve to
//! their fixed point instead of diverging (the old recursive resolve had no
//! guard and overflowed the stack on a cyclic binding).

use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::error::{RewriteError, RewriteResult};
use crate::relation::RelationResources;
use crate::trs::{Substitution, Term, Variable};

/// Most-general unifier of two terms under a fresh substitution.
pub fn unify(
    left: &Term,
    right: &Term,
    resources: &mut RelationResources,
) -> RewriteResult<Substitution> {
    let mut substitution = Substitution::new();
    unify_with(&mut substitution, left, right, resources)?;
    Ok(substitution)
}

/// Extend an existing substitution so `left` and `right` become equal.
///
/// On error the substitution may be partially extended; callers treat
/// failure as terminal for the current goal.
pub fn unify_with(
    substitution: &mut Substitution,
    left: &Term,
    right: &Term,
    resources: &mut RelationResources,
) -> RewriteResult<()> {
    let mut worklist: Vec<(Term, Term)> = vec![(left.clone(), right.clone())];
    while let Some((lhs, rhs)) = worklist.pop() {
        resources.record_operations(1)?;
        // Resolution is a borrowed root chase: it returns a reference into the
        // term or into the substitution and allocates nothing. The result is an
        // OWNED term by the time anything mutates the substitution.
        let lhs_borrowed = resolve(substitution, &lhs, resources)?;
        let rhs_borrowed = resolve(substitution, &rhs, resources)?;
        match (lhs_borrowed, rhs_borrowed) {
            (Term::Var(x), Term::Var(y)) if x == y => {}
            (Term::Var(x), term) => {
                let variable = x.clone();
                let bound = term.clone();
                bind(substitution, &variable, bound, resources)?;
            }
            (term, Term::Var(y)) => {
                let variable = y.clone();
                let bound = term.clone();
                bind(substitution, &variable, bound, resources)?;
            }
            (Term::Sym(f, f_args), Term::Sym(g, g_args)) => {
                if f != g {
                    return Err(failure("head symbol clash"));
                }
                if f_args.len() != g_args.len() {
                    return Err(failure("arity mismatch"));
                }
                for (f_arg, g_arg) in f_args.iter().zip(g_args.iter()) {
                    worklist.push((f_arg.clone(), g_arg.clone()));
                }
            }
        }
    }
    Ok(())
}

fn failure(reason: &str) -> RewriteError {
    RewriteError::UnificationFailure {
        reason: String::from(reason),
    }
}

/// Resolve a term's ROOT through the substitution, iteratively.
///
/// The returned reference is either a subterm of `term` or a borrowed binding
/// range; nothing is cloned and no expansion is materialized. Only the root is
/// resolved (as before): the arguments of a resolved symbol are inspected by
/// [`occurs`] and by the caller's worklist, which is what keeps binding ranges
/// verbatim.
///
/// A variable that re-enters its own resolution is the fixed point, so it is
/// returned in place; each binding on the chain is charged one operation.
fn resolve<'a>(
    substitution: &'a Substitution,
    term: &'a Term,
    resources: &mut RelationResources,
) -> RewriteResult<&'a Term> {
    let mut visited: BTreeSet<&Variable> = BTreeSet::new();
    let mut current = term;
    loop {
        resources.record_operations(1)?;
        let Term::Var(variable) = current else {
            return Ok(current);
        };
        if visited.contains(variable) {
            // Fixed point: leave the variable in place.
            return Ok(current);
        }
        match substitution.get_var(variable) {
            Some(bound) => {
                let _ = visited.insert(variable);
                current = bound;
            }
            None => return Ok(current),
        }
    }
}

fn bind(
    substitution: &mut Substitution,
    variable: &Variable,
    term: Term,
    resources: &mut RelationResources,
) -> RewriteResult<()> {
    resources.record_term(term_nodes(&term), term_depth(&term))?;
    if occurs(substitution, variable, &term, resources)? {
        return Err(failure("occurs check violation"));
    }
    // `resolve` guarantees the variable is unbound here, and the term
    // is resolved, so insertion preserves idempotence.
    substitution.insert(variable.clone(), term);
    Ok(())
}

/// Does `variable` occur in the resolution of `term`?
///
/// An iterative, borrowed walk over the binding graph: every node is charged one
/// operation, every binding reached is visited at most once, and a variable that
/// re-enters its own resolution is the fixed point (left in place, exactly as
/// [`Substitution::apply`] leaves it), so the answer is the same as the old
/// recursive formulation while the walk is linear in the reachable bindings plus
/// the syntactic nodes they mention.
fn occurs(
    substitution: &Substitution,
    variable: &Variable,
    term: &Term,
    resources: &mut RelationResources,
) -> RewriteResult<bool> {
    let mut visited: BTreeSet<&Variable> = BTreeSet::new();
    let mut stack: Vec<&Term> = vec![term];
    while let Some(node) = stack.pop() {
        resources.record_operations(1)?;
        match node {
            Term::Var(other) => {
                if other == variable {
                    return Ok(true);
                }
                if visited.contains(other) {
                    continue;
                }
                if let Some(bound) = substitution.get_var(other) {
                    let _ = visited.insert(other);
                    stack.push(bound);
                }
            }
            Term::Sym(_, args) => {
                for arg in args.iter().rev() {
                    stack.push(arg);
                }
            }
        }
    }
    Ok(false)
}

/// Count term nodes (variables and symbols).
///
/// Iterative (explicit stack), like the resolve/occurs walks: a caller-supplied
/// term of unbounded depth must be measurable without consuming call-stack
/// proportional to it (F16, PR #384 round 4).
pub(crate) fn term_nodes(term: &Term) -> usize {
    let mut nodes = 0usize;
    let mut stack: Vec<&Term> = vec![term];
    while let Some(node) = stack.pop() {
        nodes = nodes.saturating_add(1);
        if let Term::Sym(_, args) = node {
            for arg in args {
                stack.push(arg);
            }
        }
    }
    nodes
}

/// Measure term depth (leaf = 1).
///
/// Iterative for the same reason as [`term_nodes`].
pub(crate) fn term_depth(term: &Term) -> usize {
    let mut depth = 1usize;
    let mut stack: Vec<(&Term, usize)> = vec![(term, 1)];
    while let Some((node, node_depth)) = stack.pop() {
        depth = depth.max(node_depth);
        if let Term::Sym(_, args) = node {
            for arg in args {
                stack.push((arg, node_depth + 1));
            }
        }
    }
    depth
}
